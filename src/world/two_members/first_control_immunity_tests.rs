use super::control_immunity_tests::{hit, own_events};
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_first_control_immunity_config_is_limited_to_own_self_status_skill() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-first-control-immunity-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let def = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    let config = load(&doc).unwrap().default_config();
    assert_eq!(
        serde_json::to_value(
            config
                .pve
                .as_ref()
                .unwrap()
                .status_definitions
                .last()
                .unwrap()
        )
        .unwrap(),
        doc["pve"]["status_definitions"][def]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(85);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(1)),
        ("duplicate_policy", json!("ignore")),
        ("source_scoped", json!(false)),
        ("mechanism", json!("timed_attribute_modifier")),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][def][field] = value;
        assert!(load(&invalid).is_err());
    }
    for field in ["on_commit_self_statuses", "on_hit_target_statuses"] {
        let mut invalid = doc.clone();
        invalid["skill"][field] = json!(["first-guard"]);
        assert!(load(&invalid).is_err());
        invalid = doc.clone();
        invalid["pve"]["additional_templates"]["silence-probe"]["combat"]["projectile"][field] =
            json!(["first-guard"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] = json!({id:{"heal_hp":10,"on_use_self_statuses":["first-guard"]}});
    assert!(load(&item).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, w) = super::control_immunity_tests::setup();
    c.mechanism_version = 86;
    c.additional_skills.push(
        serde_json::from_value(json!({
            "mechanism":"self_status","id":"guard-first","cost":2,"windup_ms":30,
            "recovery_ms":0,"cooldown_ms":0,"on_commit_self_statuses":["first-guard"]
        }))
        .unwrap(),
    );
    let pve = c.pve_mut();
    pve.status_rules.as_mut().unwrap().max_active_per_entity = 8;
    for (id, duration) in [("first-guard", 1200), ("short-guard", 900)] {
        pve.status_definitions.push(
            serde_json::from_value(json!({
                "mechanism":"timed_control","attribute":"first_control_immunity","id":id,
                "duration_ms":duration,"duplicate_policy":"refresh","source_scoped":true
            }))
            .unwrap(),
        );
    }
    pve.status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"control_duration_reduction",
            "id":"reduction","duration_ms":1200,"modifier_bps":2500,
            "duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    let mut extra_silence = pve
        .status_definitions
        .iter()
        .find(|definition| definition.id == "npc-silence")
        .unwrap()
        .clone();
    extra_silence.id = "second-silence".into();
    pve.status_definitions.push(extra_silence);
    (c, w)
}

fn guard(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now)
}

fn commit_guard(w: &mut World, c: &Config, now: u64) -> Events {
    w.action(
        0,
        &Action::CastSelf {
            skill_id: "guard-first".into(),
        },
        "guard",
        c,
        now,
    )
    .unwrap();
    let pending = w.members[0].pending.take().unwrap();
    let mut events = vec![];
    w.commit(0, pending, c, now + 30, &mut events);
    events
}

#[test]
fn member_first_control_immunity_consumes_only_on_valid_silence_and_keeps_own_cast() {
    let (c, mut w) = setup();
    commit_guard(&mut w, &c, 0);
    let first = w.statuses.entries()[0].clone();
    hit(&mut w, &c, &["wound"], 60);
    assert!(w.statuses.entries().contains(&first));
    w.action(
        0,
        &Action::Cast {
            skill_id: "cast".into(),
            direction: [1., 0.],
        },
        "cast",
        &c,
        90,
    )
    .unwrap();
    let hp = w.members[0].hp;
    let events = hit(&mut w, &c, &["npc-silence", "second-silence"], 120);
    assert!(w.members[0].hp < hp && w.members[0].pending.is_some());
    assert!(matches!(own_events(&events).as_slice(), [
        Event::StatusRejected {source, target, definition_ids, reason: Error::Immune},
        Event::StatusRemoved {instance_ids, reason: StatusRemovalReason::Depleted},
    ] if *source == status_entity(&w.npcs[0].entity) && *target == first.target
        && definition_ids == &["npc-silence", "second-silence"]
        && instance_ids == std::slice::from_ref(&first.instance_id)));
    assert!(!w.skill_silenced(0, 120));
    assert!(
        w.snapshot(1, &c, Phase::Active, 120)
            .own
            .statuses
            .is_empty()
    );
    hit(&mut w, &c, &["npc-silence"], 150);
    assert!(w.skill_silenced(0, 150) && w.members[0].pending.is_none());
}

#[test]
fn member_first_control_immunity_respects_order_and_other_hit_statuses() {
    let (c, mut w) = setup();
    guard(&mut w, &c, "control-guard", 0);
    guard(&mut w, &c, "first-guard", 0);
    guard(&mut w, &c, "short-guard", 0);
    guard(&mut w, &c, "reduction", 0);
    let blocked = hit(&mut w, &c, &["npc-silence"], 570);
    assert!(matches!(
        own_events(&blocked).as_slice(),
        [Event::StatusRejected {
            reason: Error::Immune,
            ..
        }]
    ));
    assert_eq!(w.statuses.entries().len(), 4);

    pulse(&mut w, &c, 600);
    let shortest = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "short-guard")
        .unwrap()
        .instance_id
        .clone();
    let blocked = hit(&mut w, &c, &["npc-silence", "wound"], 600);
    assert!(matches!(own_events(&blocked).as_slice(), [
        Event::StatusRejected {reason: Error::Immune, ..},
        Event::StatusRemoved {instance_ids, reason: StatusRemovalReason::Depleted},
        Event::StatusApplied {status},
    ] if instance_ids == std::slice::from_ref(&shortest) && status.definition_id == "wound"));
    hit(&mut w, &c, &["npc-silence"], 630);
    assert!(!w.skill_silenced(0, 630));
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.attribute == Some(StatusAttribute::FirstControlImmunity))
    );
    let control = hit(&mut w, &c, &["npc-silence"], 660);
    assert!(
        matches!(own_events(&control).as_slice(), [Event::StatusApplied {status}]
        if status.attribute == Some(StatusAttribute::Silence) && status.expires_at_ms == 1110)
    );
}

#[test]
fn member_first_control_immunity_refreshes_expires_and_cleans_on_leave() {
    let (c, mut w) = setup();
    let target = w.members[0].reference.entity.clone();
    let invalid = bind(&mut w, &c, Owner::Member(1), target, "first-guard", 0);
    assert!(invalid.is_empty());
    commit_guard(&mut w, &c, 0);
    let first = w.statuses.entries()[0].instance_id.clone();
    let refreshed = commit_guard(&mut w, &c, 300);
    assert!(matches!(own_events(&refreshed).as_slice(),
        [Event::StatusRefreshed {status}, Event::ActionCommitted {..}]
            if status.instance_id == first && status.expires_at_ms == 1530));
    let events = hit(&mut w, &c, &["npc-silence"], 1530);
    assert!(matches!(own_events(&events).as_slice(), [
        Event::StatusRemoved {instance_ids, reason: StatusRemovalReason::Expired},
        Event::StatusApplied {status},
    ] if instance_ids == std::slice::from_ref(&first) && status.attribute == Some(StatusAttribute::Silence)));
    pulse(&mut w, &c, 2130);
    commit_guard(&mut w, &c, 2160);
    let id = w.statuses.entries()[0].instance_id.clone();
    let events = w.action(0, &Action::Leave {}, "leave", &c, 2220).unwrap();
    assert!(own_events(&events).iter().any(|event| matches!(event,
        Event::StatusRemoved {instance_ids, reason: StatusRemovalReason::TargetInvalid}
        if instance_ids == std::slice::from_ref(&id))));
    assert!(w.statuses.is_empty());
}
