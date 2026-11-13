use super::control_immunity_tests::{hit, own_events};
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_npc_movement_slow_validates_the_hit_and_cleanse_bindings() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-npc-movement-slow-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let config = load(&doc).unwrap().default_config();
    let defs = &config.pve.as_ref().unwrap().status_definitions;
    let index = defs.iter().position(|d| d.id == "npc-slow").unwrap();
    assert_eq!(
        serde_json::to_value(&defs[index]).unwrap(),
        doc["pve"]["status_definitions"][index]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(89);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(0)),
        ("modifier_bps", json!(1)),
        ("modifier_bps", json!(-10000)),
        ("duplicate_policy", json!("ignore")),
        ("source_scoped", json!(false)),
        ("mechanism", json!("timed_control")),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][index][field] = value;
        assert!(load(&invalid).is_err());
    }
    for field in ["on_commit_self_statuses", "on_hit_target_statuses"] {
        let mut invalid = doc.clone();
        invalid["skill"][field] = json!(["npc-slow"]);
        assert!(load(&invalid).is_err());
    }
    let mut invalid = doc.clone();
    invalid["pve"]["additional_templates"]["slow-probe"]["combat"]["projectile"]["on_commit_self_statuses"] =
        json!(["npc-slow"]);
    assert!(load(&invalid).is_err());
    let mut item = doc;
    item["member_item_use"] =
        json!({"xuan-tie":{"heal_hp":10,"on_use_self_statuses":["npc-slow"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({"xuan-tie":{"cleanse_self_statuses":["npc-slow"]}});
    assert!(load(&item).is_ok());
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, w) = super::npc_root_tests::setup();
    c.mechanism_version = 90;
    let pve = c.pve_mut();
    for (id, duration, modifier) in [("npc-slow", 601, -2500), ("npc-slow-strong", 300, -4000)] {
        pve.status_definitions.push(
            serde_json::from_value(json!({
                "mechanism":"timed_attribute_modifier","attribute":"movement_slow","id":id,
                "duration_ms":duration,"modifier_bps":modifier,"duplicate_policy":"refresh","source_scoped":true
            })).unwrap(),
        );
    }
    (c, w)
}

fn assert_distance(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "{actual} versus {expected}"
    );
}

#[test]
fn member_npc_movement_slow_combines_terrain_and_cast_phase_on_move_and_navigation() {
    for action in [
        Action::Move {
            direction: [1., 0.],
        },
        Action::Navigate {
            destination: [440., 400.],
        },
    ] {
        let (mut c, mut w) = setup();
        for tile in &mut Arc::make_mut(&mut w.grid.config).palette {
            tile.speed_multiplier = 0.5;
        }
        c.skill.windup_ms = 1200;
        c.skill.cast_movement = serde_json::from_value(json!({
            "preparation":{"kind":"slow","speed_bps":5000}, "recovery":{"kind":"free"}
        }))
        .unwrap();
        let cast = Action::Cast {
            skill_id: "cast".into(),
            direction: [1., 0.],
        };
        w.action(0, &action, "movement", &c, 0).unwrap();
        w.action(0, &cast, "cast", &c, 0).unwrap();
        let origin = w.members[0].position;
        let base =
            c.params.movement.speed(w.members[0].facts.spd_points) * c.step_ms as f64 / 1000.;
        let applied = hit(&mut w, &c, &["npc-slow"], 30);
        assert!(
            matches!(own_events(&applied).as_slice(), [Event::StatusApplied {status}]
            if status.expires_at_ms == 631 && status.modifier_bps == -2500)
        );
        assert!(w.members[0].pending.is_some());
        w.tick(&c, 60);
        assert_distance(
            w.members[0].position[0] - origin[0],
            base * 0.75 * 0.5 * 0.5,
        );
        let slow_position = w.members[0].position;
        w.tick(&c, 631);
        assert_distance(
            w.members[0].position[0] - slow_position[0],
            base * 0.5 * 0.5,
        );
        assert!(w.members[0].pending.is_some());
    }
}

#[test]
fn member_npc_movement_slow_preserves_active_pursuit_and_uses_the_same_budget() {
    let (c, mut w) = setup();
    let far = Action::AttackUnit {
        target: World::unit_wire(&w.npcs[1].entity),
    };
    w.action(0, &far, "pursuit", &c, 0).unwrap();
    let pos = w.members[0].position;
    let base = c.params.movement.speed(w.members[0].facts.spd_points)
        * w.grid.speed(pos)
        * c.step_ms as f64
        / 1000.;
    hit(&mut w, &c, &["npc-slow"], 30);
    assert_eq!(
        w.members[0].basic_pursuit.as_ref().unwrap().view.action_id,
        "pursuit"
    );
    w.tick(&c, 60);
    assert_distance(w.members[0].position[0] - pos[0], base * 0.75);
    let slow_pos = w.members[0].position;
    w.tick(&c, 631);
    assert_distance(w.members[0].position[0] - slow_pos[0], base);
    assert_eq!(
        w.members[0].basic_pursuit.as_ref().unwrap().view.action_id,
        "pursuit"
    );
}

#[test]
fn member_npc_movement_slow_reuses_control_mitigation_and_selects_the_strongest_active_instance() {
    let (mut c, mut w) = setup();
    let own = w.members[0].reference.entity.clone();
    for id in ["control-guard", "first-guard", "reduction"] {
        bind(&mut w, &c, Owner::Member(0), own.clone(), id, 0);
    }
    let contact = ["npc-slow", "npc-root"];
    let blocked = hit(&mut w, &c, &contact, 30);
    assert!(matches!(
        own_events(&blocked).as_slice(),
        [Event::StatusRejected {
            reason: Error::Immune,
            ..
        }]
    ));
    pulse(&mut w, &c, 600);
    let blocked = hit(&mut w, &c, &contact, 600);
    assert!(matches!(own_events(&blocked).as_slice(), [
        Event::StatusRejected {reason: Error::Immune, definition_ids, ..},
        Event::StatusRemoved {reason: StatusRemovalReason::Depleted, instance_ids}
    ] if definition_ids == &contact && instance_ids.len() == 1));
    hit(&mut w, &c, &["npc-slow"], 630);
    let first = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "npc-slow")
        .unwrap()
        .clone();
    assert_eq!(first.expires_at_ms, 1080);
    let refreshed = hit(&mut w, &c, &["npc-slow"], 660);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == first.instance_id && status.expires_at_ms == 1110)
    );
    hit(&mut w, &c, &["npc-slow-strong"], 690);
    let base = w.member_movement_budget(0, &c, 1200, 1.);
    assert_distance(w.member_movement_budget(0, &c, 914, 1.), base * 0.6);
    assert_distance(w.member_movement_budget(0, &c, 915, 1.), base * 0.75);
    assert!(
        w.snapshot(1, &c, Phase::Active, 915)
            .own
            .statuses
            .is_empty()
    );
    let item = "xuan-tie";
    c.member_item_use =
        serde_json::from_value(json!({item:{"cleanse_self_statuses":["npc-slow"]}})).unwrap();
    w.members[0].inventory.insert(item.into(), 1);
    w.members[0].looted.insert(item.into(), 1);
    let action = Action::UseItem {
        item_id: item.into(),
        expected_revision: w.members[0].revision,
    };
    let cleansed = w.action(0, &action, "cleanse", &c, 930).unwrap();
    assert!(matches!(own_events(&cleansed).as_slice(),
        [Event::StatusRemoved {reason: StatusRemovalReason::Cleansed, instance_ids}, Event::ItemUsed {..}]
            if instance_ids == std::slice::from_ref(&first.instance_id)));
    assert_distance(w.member_movement_budget(0, &c, 930, 1.), base);
}
