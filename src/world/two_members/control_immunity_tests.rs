use super::combat::DirectContact;
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_control_immunity_config_is_limited_to_own_self_status_skill() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-control-immunity-host/arena-a.json"
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
    old["mechanism_version"] = json!(83);
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
        invalid["skill"][field] = json!(["control-guard"]);
        assert!(load(&invalid).is_err());
        invalid = doc.clone();
        invalid["pve"]["additional_templates"]["silence-probe"]["combat"]["projectile"][field] =
            json!(["control-guard"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] = json!({id:{"heal_hp":10,"on_use_self_statuses":["control-guard"]}});
    assert!(load(&item).is_err());
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, mut w) = super::npc_silence_tests::setup();
    c.mechanism_version = 84;
    c.additional_skills.push(
        serde_json::from_value(json!({
            "mechanism":"self_status","id":"protect-self","cost":2,"windup_ms":30,
            "recovery_ms":0,"cooldown_ms":0,"on_commit_self_statuses":["control-guard"]
        }))
        .unwrap(),
    );
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_control","attribute":"control_immunity","id":"control-guard",
            "duration_ms":600,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    w.members[0].hp = w.members[0].facts.hp_points;
    (c, w)
}

fn commit(w: &mut World, c: &Config, now: u64) -> Events {
    let pending = w.members[0].pending.take().unwrap();
    let mut events = vec![];
    w.commit(0, pending, c, now, &mut events);
    events
}

fn immune(w: &mut World, c: &Config, now: u64) -> Events {
    w.action(
        0,
        &Action::CastSelf {
            skill_id: "protect-self".into(),
        },
        "immunity",
        c,
        now,
    )
    .unwrap();
    commit(w, c, now + 30)
}

pub(super) fn hit(w: &mut World, c: &Config, ids: &[&str], now: u64) -> Events {
    let source = w.npcs[0].entity.clone();
    let mut events = vec![];
    let statuses = ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
    w.direct_contact(
        Owner::Npc(0),
        Owner::Member(0),
        DirectContact {
            origin: None,
            action_id: None,
            rule: None,
            ignore_armor: false,
            unavoidable: false,
            dead_source: false,
            source: &source,
            raw: 10,
            metal: true,
            hit_statuses: &statuses,
        },
        c,
        now,
        &mut events,
    );
    events
}

pub(super) fn own_events(events: &Events) -> Vec<&Event> {
    events
        .iter()
        .filter_map(|(recipient, row)| match row {
            EventV4::Private { event } => {
                assert_eq!(*recipient, Some(0));
                Some(event)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn member_control_immunity_blocks_silence_without_blocking_damage_or_other_hit_status() {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 2;
    immune(&mut w, &c, 0);
    w.action(
        0,
        &Action::Cast {
            skill_id: "cast".into(),
            direction: [1., 0.],
        },
        "cast",
        &c,
        60,
    )
    .unwrap();
    let hp = w.members[0].hp;
    let events = hit(&mut w, &c, &["npc-silence", "wound"], 90);
    assert!(w.members[0].hp < hp);
    assert!(matches!(own_events(&events).as_slice(), [
        Event::StatusRejected {source,target,definition_ids,reason:Error::Immune},
        Event::StatusApplied {status}
    ] if definition_ids==&["npc-silence"] && status.definition_id=="wound"
        && *source==status_entity(&w.npcs[0].entity) && *target==status_entity(&w.members[0].reference.entity)));
    assert_eq!(w.statuses.entries().len(), 2);
    assert!(w.members[0].pending.is_some());
    let committed = commit(&mut w, &c, 120);
    assert!(matches!(
        own_events(&committed).as_slice(),
        [Event::ActionCommitted { .. }]
    ));
    let hit = hit(&mut w, &c, &["npc-silence"], 630);
    assert!(
        own_events(&hit)
            .iter()
            .any(|event| matches!(event,Event::StatusApplied {status}
        if status.attribute==Some(StatusAttribute::Silence)))
    );
    assert!(w.skill_silenced(0, 630));
}

#[test]
fn member_control_immunity_does_not_extend_or_remove_an_existing_silence() {
    let (c, mut w) = setup();
    hit(&mut w, &c, &["npc-silence"], 30);
    let silence = w.statuses.entries()[0].clone();
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "control-guard", 60);
    let blocked = hit(&mut w, &c, &["npc-silence"], 200);
    assert!(matches!(
        own_events(&blocked).as_slice(),
        [Event::StatusRejected {
            reason: Error::Immune,
            ..
        }]
    ));
    assert_eq!(w.statuses.entries()[0], silence);
    assert!(w.skill_silenced(0, 629));
    pulse(&mut w, &c, 630);
    assert!(!w.skill_silenced(0, 630));
    assert!(matches!(
        own_events(&hit(&mut w, &c, &["npc-silence"], 659)).as_slice(),
        [Event::StatusRejected {
            reason: Error::Immune,
            ..
        }]
    ));
    hit(&mut w, &c, &["npc-silence"], 660);
    assert!(w.skill_silenced(0, 660));
}

#[test]
fn member_control_immunity_refreshes_at_capacity_and_cleans_only_its_own_view() {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    immune(&mut w, &c, 0);
    let id = w.statuses.entries()[0].instance_id.clone();
    let refreshed = immune(&mut w, &c, 300);
    assert!(matches!(own_events(&refreshed).as_slice(), [
        Event::StatusRefreshed {status},Event::ActionCommitted {..}
    ] if status.instance_id==id && status.expires_at_ms==930));
    assert!(
        w.snapshot(1, &c, Phase::Active, 330)
            .own
            .statuses
            .is_empty()
    );
    let left = w.action(0, &Action::Leave {}, "leave", &c, 400).unwrap();
    assert!(
        own_events(&left)
            .iter()
            .any(|event| matches!(event,Event::StatusRemoved {
        instance_ids,reason:StatusRemovalReason::TargetInvalid
    } if instance_ids==std::slice::from_ref(&id)))
    );
    assert!(w.statuses.is_empty());
}
