use super::control_immunity_tests::{hit, own_events};
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_control_duration_reduction_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-control-duration-reduction-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let config = load(&doc).unwrap().default_config();
    let definitions = &config.pve.as_ref().unwrap().status_definitions;
    let def = definitions.len() - 1;
    assert_eq!(
        serde_json::to_value(&definitions[def]).unwrap(),
        doc["pve"]["status_definitions"][def]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(84);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(-1)),
        ("modifier_bps", json!(0)),
        ("modifier_bps", json!(4001)),
        ("duplicate_policy", json!("ignore")),
        ("source_scoped", json!(false)),
        ("mechanism", json!("timed_control")),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][def][field] = value;
        assert!(load(&invalid).is_err());
    }
    for field in ["on_commit_self_statuses", "on_hit_target_statuses"] {
        let mut invalid = doc.clone();
        invalid["skill"][field] = json!(["duration-guard"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["silence-probe"]["combat"]["projectile"][field] =
            json!(["duration-guard"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] = json!({id:{"heal_hp":10,"on_use_self_statuses":["duration-guard"]}});
    assert!(load(&item).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, w) = super::control_immunity_tests::setup();
    c.mechanism_version = 85;
    let pve = c.pve_mut();
    pve.status_rules.as_mut().unwrap().max_active_per_entity = 8;
    pve.status_definitions
        .iter_mut()
        .find(|d| d.id == "npc-silence")
        .unwrap()
        .duration_ms = 601;
    for id in ["guard-a", "guard-b"] {
        pve.status_definitions.push(
            serde_json::from_value(json!({
                "mechanism":"timed_attribute_modifier","attribute":"control_duration_reduction",
                "id":id,"duration_ms":300,"modifier_bps":2500,
                "duplicate_policy":"refresh","source_scoped":true
            }))
            .unwrap(),
        );
    }
    (c, w)
}

fn guard(w: &mut World, c: &Config, member: usize, id: &str, now: u64) -> Events {
    let target = w.members[member].reference.entity.clone();
    bind(w, c, Owner::Member(member), target, id, now)
}

fn silence(w: &World) -> &StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|status| status.attribute == Some(StatusAttribute::Silence))
        .unwrap()
}

#[test]
fn member_control_duration_reduction_uses_target_and_freezes_rounded_deadline() {
    let (c, mut w) = setup();
    guard(&mut w, &c, 0, "guard-a", 0);
    let hp = w.members[0].hp;
    let events = hit(&mut w, &c, &["npc-silence", "wound"], 60);
    assert!(w.members[0].hp < hp);
    let wound_duration = c
        .pve
        .as_ref()
        .unwrap()
        .status_definitions
        .iter()
        .find(|definition| definition.id == "wound")
        .unwrap()
        .duration_ms;
    assert!(matches!(own_events(&events).as_slice(), [
        Event::StatusApplied {status: control},
        Event::StatusApplied {status: wound},
    ] if control.expires_at_ms == 510 && control.source == status_entity(&w.npcs[0].entity)
        && control.target == status_entity(&w.members[0].reference.entity)
        && wound.definition_id == "wound" && wound.expires_at_ms == 60 + wound_duration));
    assert!(w.snapshot(1, &c, Phase::Active, 60).own.statuses.is_empty());
    let applied = silence(&w).clone();
    pulse(&mut w, &c, 300);
    assert_eq!(silence(&w), &applied);
    assert!(w.skill_silenced(0, 509));
    let expired = pulse(&mut w, &c, 510);
    assert!(own_events(&expired).iter().any(|event| matches!(event,
        Event::StatusRemoved {instance_ids, reason: StatusRemovalReason::Expired}
            if instance_ids == std::slice::from_ref(&applied.instance_id))));
    assert!(!w.skill_silenced(0, 510));
    guard(&mut w, &c, 1, "guard-b", 540);
    hit(&mut w, &c, &["npc-silence"], 540);
    assert_eq!(silence(&w).expires_at_ms, 1141);
}

#[test]
fn member_control_duration_reduction_caps_sum_and_recalculates_on_refresh() {
    let (c, mut w) = setup();
    guard(&mut w, &c, 0, "guard-a", 0);
    let refreshed = guard(&mut w, &c, 0, "guard-a", 30);
    assert!(matches!(own_events(&refreshed).as_slice(),
        [Event::StatusRefreshed { status }] if status.expires_at_ms == 330));
    guard(&mut w, &c, 0, "guard-b", 30);
    hit(&mut w, &c, &["npc-silence"], 60);
    let applied = silence(&w).clone();
    assert_eq!(applied.expires_at_ms, 420);
    let events = hit(&mut w, &c, &["npc-silence"], 330);
    assert!(matches!(own_events(&events).as_slice(),
        [Event::StatusRemoved { instance_ids, reason: StatusRemovalReason::Expired },
         Event::StatusRefreshed { status }]
            if instance_ids.len() == 2
                && status.instance_id == applied.instance_id && status.expires_at_ms == 931));
}

#[test]
fn member_control_duration_reduction_keeps_immunity_first() {
    let (c, mut w) = setup();
    hit(&mut w, &c, &["npc-silence"], 30);
    let applied = silence(&w).clone();
    guard(&mut w, &c, 0, "guard-a", 60);
    guard(&mut w, &c, 0, "control-guard", 60);
    let events = hit(&mut w, &c, &["npc-silence", "wound"], 90);
    assert!(matches!(own_events(&events).as_slice(), [
        Event::StatusRejected {definition_ids, reason: Error::Immune, ..},
        Event::StatusApplied {status}
    ] if definition_ids == &["npc-silence"] && status.definition_id == "wound"));
    assert_eq!(silence(&w), &applied);
}
