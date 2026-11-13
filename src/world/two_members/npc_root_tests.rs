use super::control_immunity_tests::{hit, own_events};
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_npc_root_validates_the_hit_and_cleanse_bindings() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-npc-root-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let config = load(&doc).unwrap().default_config();
    let defs = &config.pve.as_ref().unwrap().status_definitions;
    let index = defs.iter().position(|d| d.id == "npc-root").unwrap();
    assert_eq!(
        serde_json::to_value(&defs[index]).unwrap(),
        doc["pve"]["status_definitions"][index]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(87);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(0)),
        ("duplicate_policy", json!("ignore")),
        ("source_scoped", json!(false)),
        ("mechanism", json!("timed_attribute_modifier")),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][index][field] = value;
        assert!(load(&invalid).is_err());
    }
    for field in ["on_commit_self_statuses", "on_hit_target_statuses"] {
        let mut invalid = doc.clone();
        invalid["skill"][field] = json!(["npc-root"]);
        assert!(load(&invalid).is_err());
    }
    let mut invalid = doc.clone();
    invalid["pve"]["additional_templates"]["root-probe"]["combat"]["projectile"]["on_commit_self_statuses"] =
        json!(["npc-root"]);
    assert!(load(&invalid).is_err());
    let mut item = doc;
    item["member_item_use"] =
        json!({"xuan-tie":{"heal_hp":10,"on_use_self_statuses":["npc-root"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({"xuan-tie":{"cleanse_self_statuses":["npc-root"]}});
    assert!(load(&item).is_ok());
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, mut w) = super::control_immunity_tests::setup();
    c.mechanism_version = 88;
    c.basic.range = 85.;
    c.basic_pursuit = serde_json::from_value(json!({
        "max_range":350,"duration_ms":1500,"auto_max_range":300
    }))
    .unwrap();
    w.npcs[0].position = [460., 400.];
    w.npcs[1].position = [580., 400.];
    let pve = c.pve_mut();
    pve.status_rules.as_mut().unwrap().max_active_per_entity = 8;
    for definition in [
        json!({"mechanism":"timed_control","attribute":"root","id":"npc-root","duration_ms":601}),
        json!({"mechanism":"timed_control","attribute":"first_control_immunity","id":"first-guard","duration_ms":1200}),
        json!({"mechanism":"timed_attribute_modifier","attribute":"control_duration_reduction","id":"reduction",
               "duration_ms":1200,"modifier_bps":2500}),
    ] {
        let mut definition = definition;
        definition["duplicate_policy"] = json!("refresh");
        definition["source_scoped"] = json!(true);
        pve.status_definitions
            .push(serde_json::from_value(definition).unwrap());
    }
    (c, w)
}

#[test]
fn member_npc_root_clears_motion_and_accepts_new_input_only_after_expiry() {
    let (c, mut w) = setup();
    let move_east = Action::Move {
        direction: [1., 0.],
    };
    let navigate = Action::Navigate {
        destination: [440., 400.],
    };
    let cast = Action::Cast {
        skill_id: "cast".into(),
        direction: [1., 0.],
    };
    w.action(0, &move_east, "move", &c, 0).unwrap();
    w.action(0, &navigate, "navigate", &c, 0).unwrap();
    w.action(0, &cast, "cast", &c, 0).unwrap();
    assert!(!w.members[0].path.is_empty());
    let pos = w.members[0].position;
    let events = hit(&mut w, &c, &["npc-root"], 30);
    assert!(
        matches!(own_events(&events).as_slice(), [Event::StatusApplied {status}]
        if status.target == status_entity(&w.members[0].reference.entity)
            && status.source == status_entity(&w.npcs[0].entity) && status.expires_at_ms == 631)
    );
    assert!(w.snapshot(1, &c, Phase::Active, 30).own.statuses.is_empty());
    assert_eq!(w.members[0].movement, [0., 0.]);
    assert!(w.members[0].path.is_empty());
    assert_eq!(
        w.action(0, &move_east, "blocked-move", &c, 60),
        Err(Error::InvalidState)
    );
    assert_eq!(
        w.action(0, &navigate, "blocked-path", &c, 60),
        Err(Error::InvalidState)
    );
    let events = w.tick(&c, 60);
    assert!(events.iter().any(|(recipient, event)| *recipient == Some(0) && matches!(
        event, EventV4::Private {event: Event::ActionCommitted {skill_id: Some(id), ..}} if id == "cast")));
    w.tick(&c, 630);
    assert_eq!(w.members[0].position, pos);
    w.tick(&c, 631);
    assert_eq!(w.members[0].position, pos);
    w.action(0, &move_east, "fresh-move", &c, 631).unwrap();
    w.tick(&c, 660);
    assert!(w.members[0].position[0] > pos[0]);
}

#[test]
fn member_npc_root_cancels_pursuit_and_preserves_in_range_attack() {
    let (c, mut w) = setup();
    let attack_far = Action::AttackUnit {
        target: World::unit_wire(&w.npcs[1].entity),
    };
    w.action(0, &attack_far, "chase", &c, 0).unwrap();
    assert!(w.members[0].basic_pursuit.is_some());
    let events = hit(&mut w, &c, &["npc-root"], 30);
    assert!(matches!(
        own_events(&events).as_slice(),
        [Event::StatusApplied { .. }, Event::ActionInterrupted]
    ));
    assert!(w.members[0].basic_pursuit.is_none());
    assert_eq!(
        w.action(0, &attack_far, "new-chase", &c, 60),
        Err(Error::InvalidState)
    );
    let attack_near = Action::AttackUnit {
        target: World::unit_wire(&w.npcs[0].entity),
    };
    w.action(0, &attack_near, "in-range", &c, 60).unwrap();
    assert!(w.members[0].pending.is_some());
    assert!(w.members[0].basic_pursuit.is_none());
    let pos = w.members[0].position;
    w.tick(&c, 360);
    assert_eq!(w.members[0].position, pos);
}

#[test]
fn member_npc_root_reuses_immunity_shortening_refresh_and_cleanse() {
    let (mut c, mut w) = setup();
    let own = w.members[0].reference.entity.clone();
    for id in ["control-guard", "first-guard", "reduction"] {
        bind(&mut w, &c, Owner::Member(0), own.clone(), id, 0);
    }
    let contact = ["npc-root", "npc-silence"];
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
    hit(&mut w, &c, &contact, 630);
    let first = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "npc-root")
        .unwrap()
        .clone();
    assert_eq!(first.expires_at_ms, 1080);
    let refreshed = hit(&mut w, &c, &["npc-root"], 660);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == first.instance_id && status.expires_at_ms == 1110)
    );
    let item = "xuan-tie";
    c.member_item_use =
        serde_json::from_value(json!({item:{"cleanse_self_statuses":["npc-root"]}})).unwrap();
    w.members[0].inventory.insert(item.into(), 1);
    w.members[0].looted.insert(item.into(), 1);
    let action = Action::UseItem {
        item_id: item.into(),
        expected_revision: w.members[0].revision,
    };
    let cleansed = w.action(0, &action, "cleanse", &c, 690).unwrap();
    assert!(matches!(own_events(&cleansed).as_slice(),
        [Event::StatusRemoved {reason: StatusRemovalReason::Cleansed, instance_ids}, Event::ItemUsed {..}]
            if instance_ids == std::slice::from_ref(&first.instance_id)));
    assert!(!w.member_has_status(0, StatusAttribute::Root, 690) && w.skill_silenced(0, 690));
    w.action(
        0,
        &Action::Move {
            direction: [1., 0.],
        },
        "move",
        &c,
        690,
    )
    .unwrap();
}
