use super::control_immunity_tests::{hit, own_events};
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_movement_haste_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-movement-haste-host/arena-a.json"
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
    old["mechanism_version"] = json!(90);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(-1)),
        ("modifier_bps", json!(0)),
        ("modifier_bps", json!(10001)),
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
        invalid["skill"][field] = json!(["haste-guard"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["slow-probe"]["combat"]["projectile"][field] =
            json!(["haste-guard"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] =
        json!({(id.as_str()):{"heal_hp":10,"on_use_self_statuses":["haste-guard"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({id:{"cleanse_self_statuses":["haste-guard"]}});
    assert!(load(&item).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, w) = super::npc_movement_slow_tests::setup();
    c.mechanism_version = 91;
    c.additional_skills.push(
        serde_json::from_value(json!({
            "mechanism":"self_status","id":"accelerate","cost":2,"windup_ms":30,
            "recovery_ms":0,"cooldown_ms":0,"on_commit_self_statuses":["haste-a"]
        }))
        .unwrap(),
    );
    let pve = c.pve_mut();
    for (id, duration, modifier) in [("haste-a", 600, 3000), ("haste-b", 300, 5000)] {
        pve.status_definitions.push(serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"movement_haste","id":id,
            "duration_ms":duration,"modifier_bps":modifier,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    (c, w)
}

fn buff(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now)
}

fn assert_distance(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "{actual} versus {expected}"
    );
}

#[test]
fn member_movement_haste_preserves_motion_and_uses_the_shared_budget() {
    for action in [
        Action::Move {
            direction: [1., 0.],
        },
        Action::Navigate {
            destination: [440., 400.],
        },
    ] {
        let (c, mut w) = setup();
        w.action(0, &action, "movement", &c, 0).unwrap();
        w.action(
            0,
            &Action::CastSelf {
                skill_id: "accelerate".into(),
            },
            "haste",
            &c,
            0,
        )
        .unwrap();
        let pending = w.members[0].pending.take().unwrap();
        let mut events = vec![];
        w.commit(0, pending, &c, 30, &mut events);
        assert!(matches!(own_events(&events).as_slice(),
            [Event::StatusApplied {status}, Event::ActionCommitted {..}]
            if status.attribute == Some(StatusAttribute::MovementHaste) && status.modifier_bps == 3000));
        let base = w.member_movement_budget(0, &c, 1000, c.step_ms as f64 / 1000.);
        let origin = w.members[0].position;
        w.tick(&c, 60);
        assert_distance(w.members[0].position[0] - origin[0], base * 1.3);
        hit(&mut w, &c, &["npc-slow"], 60);
        let slowed = w.members[0].position;
        w.tick(&c, 90);
        assert_distance(w.members[0].position[0] - slowed[0], base * 1.05);
        hit(&mut w, &c, &["npc-root"], 90);
        let rooted = w.members[0].position;
        w.tick(&c, 120);
        assert_eq!(w.members[0].position, rooted);
    }
    let (c, mut w) = setup();
    let far = Action::AttackUnit {
        target: World::unit_wire(&w.npcs[1].entity),
    };
    w.action(0, &far, "pursuit", &c, 0).unwrap();
    buff(&mut w, &c, "haste-a", 30);
    let pos = w.members[0].position;
    let base = w.member_movement_budget(0, &c, 1000, c.step_ms as f64 / 1000.);
    w.tick(&c, 60);
    assert_distance(w.members[0].position[0] - pos[0], base * 1.3);
    assert_eq!(
        w.members[0].basic_pursuit.as_ref().unwrap().view.action_id,
        "pursuit"
    );
}

#[test]
fn member_movement_haste_selects_the_strongest_and_refreshes_independently() {
    let (c, mut w) = setup();
    let first = buff(&mut w, &c, "haste-a", 30);
    let [Event::StatusApplied { status }] = own_events(&first)[..] else {
        panic!("missing own haste");
    };
    let id = status.instance_id.clone();
    buff(&mut w, &c, "haste-b", 60);
    hit(&mut w, &c, &["npc-slow", "npc-slow-strong"], 90);
    let refreshed = buff(&mut w, &c, "haste-a", 120);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == id && status.expires_at_ms == 720)
    );
    let base = w.member_movement_budget(0, &c, 1000, 1.);
    for (now, multiplier) in [(359, 1.1), (360, 0.9), (390, 1.05), (691, 1.3), (720, 1.)] {
        assert_distance(w.member_movement_budget(0, &c, now, 1.), base * multiplier);
    }
    assert!(
        w.snapshot(1, &c, Phase::Active, 390)
            .own
            .statuses
            .is_empty()
    );
    let left = w.action(0, &Action::Leave {}, "leave", &c, 700).unwrap();
    assert!(own_events(&left).iter().any(|event| matches!(event,
        Event::StatusRemoved {reason: StatusRemovalReason::TargetInvalid, instance_ids}
        if instance_ids.contains(&id))));
    assert!(w.statuses.is_empty());
}

#[test]
fn member_movement_haste_keeps_hostile_control_mitigation_and_cleanse_separate() {
    let (mut c, mut w) = setup();
    buff(&mut w, &c, "control-guard", 0);
    buff(&mut w, &c, "reduction", 0);
    let rejected = hit(&mut w, &c, &["npc-slow"], 30);
    assert!(matches!(
        own_events(&rejected).as_slice(),
        [Event::StatusRejected {
            reason: Error::Immune,
            ..
        }]
    ));
    buff(&mut w, &c, "haste-a", 120);
    pulse(&mut w, &c, 600);
    let events = hit(&mut w, &c, &["npc-slow"], 600);
    assert!(
        matches!(own_events(&events).as_slice(), [Event::StatusApplied {status}]
        if status.expires_at_ms == 1050 && status.modifier_bps == -2500)
    );
    let base = w.member_movement_budget(0, &c, 1200, 1.);
    assert_distance(w.member_movement_budget(0, &c, 600, 1.), base * 1.05);
    let item = "xuan-tie";
    c.member_item_use =
        serde_json::from_value(json!({item:{"cleanse_self_statuses":["npc-slow"]}})).unwrap();
    w.members[0].inventory.insert(item.into(), 1);
    w.members[0].looted.insert(item.into(), 1);
    let action = Action::UseItem {
        item_id: item.into(),
        expected_revision: w.members[0].revision,
    };
    w.action(0, &action, "cleanse", &c, 630).unwrap();
    assert!(w.member_has_status(0, StatusAttribute::MovementHaste, 630));
    assert_distance(w.member_movement_budget(0, &c, 630, 1.), base * 1.3);
}
