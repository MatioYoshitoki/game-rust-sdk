use super::control_immunity_tests::{hit, own_events};
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_npc_stun_validates_the_hit_and_cleanse_bindings() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-npc-stun-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let config = load(&doc).unwrap().default_config();
    let defs = &config.pve.as_ref().unwrap().status_definitions;
    let index = defs.iter().position(|d| d.id == "npc-stun").unwrap();
    assert_eq!(
        serde_json::to_value(&defs[index]).unwrap(),
        doc["pve"]["status_definitions"][index]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(88);
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
        invalid["skill"][field] = json!(["npc-stun"]);
        assert!(load(&invalid).is_err());
    }
    let mut invalid = doc.clone();
    invalid["pve"]["additional_templates"]["stun-probe"]["combat"]["projectile"]["on_commit_self_statuses"] =
        json!(["npc-stun"]);
    assert!(load(&invalid).is_err());
    let mut item = doc;
    item["member_item_use"] =
        json!({"xuan-tie":{"heal_hp":10,"on_use_self_statuses":["npc-stun"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({"xuan-tie":{"cleanse_self_statuses":["npc-stun"]}});
    assert!(load(&item).is_ok());
}

fn setup() -> (Config, World) {
    let (mut c, w) = super::npc_root_tests::setup();
    c.mechanism_version = 89;
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_control","attribute":"stun","id":"npc-stun",
            "duration_ms":601,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    (c, w)
}

#[test]
fn member_npc_stun_interrupts_uncommitted_actions_and_restores_input_at_deadline() {
    let (mut c, mut w) = setup();
    let cast = Action::Cast {
        skill_id: "cast".into(),
        direction: [1., 0.],
    };
    let move_east = Action::Move {
        direction: [1., 0.],
    };
    let target = World::unit_wire(&w.npcs[0].entity);
    let position = w.members[0].position;
    w.action(0, &move_east, "moving", &c, 0).unwrap();
    w.action(
        0,
        &Action::Navigate {
            destination: [440., 400.],
        },
        "path",
        &c,
        0,
    )
    .unwrap();
    w.action(0, &cast, "cast", &c, 0).unwrap();
    w.members[0].queued_cast = Some(QueuedCastView {
        action_id: "queued-cast".into(),
        skill_id: "cast".into(),
        direction: [1., 0.],
        expires_at_ms: 600,
    });
    w.members[0].queued_basic = Some(BasicFollowupView {
        action_id: "queued-basic".into(),
        target: target.clone(),
        promotes_at_ms: 60,
        expires_at_ms: 600,
    });
    let (hp, spirit) = (w.members[0].hp, w.members[0].spirit);
    let events = hit(&mut w, &c, &["npc-stun"], 30);
    assert!(
        matches!(own_events(&events).as_slice(), [Event::StatusApplied {status}, Event::ActionInterrupted]
        if status.attribute == Some(StatusAttribute::Stun)
            && status.source == status_entity(&w.npcs[0].entity)
            && status.target == status_entity(&w.members[0].reference.entity))
    );
    assert!(w.members[0].hp < hp);
    assert_eq!(w.members[0].spirit, spirit);
    assert!(w.members[0].pending.is_none() && w.members[0].cooldowns.is_empty());
    assert!(w.members[0].queued_cast.is_none() && w.members[0].queued_basic.is_none());
    assert!(w.members[0].path.is_empty() && w.members[0].movement == [0., 0.]);
    for action in [
        cast.clone(),
        move_east.clone(),
        Action::Navigate {
            destination: [440., 400.],
        },
        Action::Attack {},
        Action::AttackUnit { target },
        Action::ExtractStart {},
        Action::RecastProjectile {
            action_id: "committed".into(),
            recast_index: 1,
            direction: [1., 0.],
        },
    ] {
        assert_eq!(
            w.action(0, &action, "blocked", &c, 630),
            Err(Error::InvalidState)
        );
    }
    assert!(w.action(0, &Action::Stop {}, "stop", &c, 630).is_ok());
    assert!(
        w.snapshot(1, &c, Phase::Active, 630)
            .own
            .statuses
            .is_empty()
    );
    w.tick(&c, 630);
    assert_eq!(w.members[0].position, position);
    w.action(0, &move_east, "resumed-move", &c, 631).unwrap();
    w.action(0, &cast, "resumed-cast", &c, 631).unwrap();
    w.tick(&c, 660);
    assert!(w.members[0].position[0] > position[0]);

    for pursuit in [false, true] {
        let (_, mut w) = setup();
        c.extraction_unlock_ms = 0;
        c.extraction_point = w.members[0].position;
        let action = if pursuit {
            Action::AttackUnit {
                target: World::unit_wire(&w.npcs[1].entity),
            }
        } else {
            Action::ExtractStart {}
        };
        w.action(0, &action, "begin", &c, 0).unwrap();
        let target = w.members[0].reference.entity.clone();
        let events = bind(&mut w, &c, Owner::Npc(0), target, "npc-stun", 30);
        assert!(
            matches!(own_events(&events).as_slice(), [Event::StatusApplied {..}, event]
            if if pursuit { matches!(event, Event::ActionInterrupted) }
               else { matches!(event, Event::ExtractionInterrupted) })
        );
        assert!(w.members[0].basic_pursuit.is_none() && w.members[0].extract_end.is_none());
    }
}

#[test]
fn member_npc_stun_stops_channel_without_refunding_or_cancelling_paid_shots() {
    let (mut c, mut w) = setup();
    c.skill.channel = serde_json::from_value(json!({
        "duration_ms":360, "interval_ms":90, "spirit_per_pulse":3
    }))
    .unwrap();
    let action = Action::Cast {
        skill_id: "cast".into(),
        direction: [1., 0.],
    };
    w.action(0, &action, "channel", &c, 0).unwrap();
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 60, &mut vec![]);
    w.advance_channel(0, &c, 150, &mut vec![]);
    let shot = w.shots[0].view.id.clone();
    let spirit = w.members[0].spirit;
    let cooldowns = w.members[0].cooldowns.clone();
    let events = hit(&mut w, &c, &["npc-stun"], 151);
    assert!(matches!(
        own_events(&events).as_slice(),
        [
            Event::StatusApplied { .. },
            Event::ChannelEnded {
                reason: ChannelEndReason::Interrupted,
                ..
            }
        ]
    ));
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.members[0].cooldowns, cooldowns);
    let hp = w.npcs[0].hp;
    w.advance_shots(&c, 180, 1., &mut vec![]);
    assert!(w.npcs[0].hp < hp && w.shots.iter().all(|projectile| projectile.view.id != shot));
    let mut future = vec![];
    w.advance_channel(0, &c, 240, &mut future);
    assert!(future.is_empty());
    assert_eq!(w.members[0].spirit, spirit);
}

#[test]
fn member_npc_stun_reuses_shared_control_mitigation_and_cleanse() {
    let (mut c, mut w) = setup();
    let own = w.members[0].reference.entity.clone();
    for id in ["control-guard", "first-guard", "reduction"] {
        bind(&mut w, &c, Owner::Member(0), own.clone(), id, 0);
    }
    let contact = ["npc-stun", "npc-root"];
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
        .find(|s| s.definition_id == "npc-stun")
        .unwrap()
        .clone();
    assert_eq!(first.expires_at_ms, 1080);
    let refreshed = hit(&mut w, &c, &["npc-stun"], 660);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == first.instance_id && status.expires_at_ms == 1110)
    );
    let item = "xuan-tie";
    c.member_item_use =
        serde_json::from_value(json!({item:{"cleanse_self_statuses":["npc-stun"]}})).unwrap();
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
    assert!(
        !w.member_has_status(0, StatusAttribute::Stun, 690)
            && w.member_has_status(0, StatusAttribute::Root, 690)
    );
    w.action(0, &Action::Attack {}, "attack", &c, 690).unwrap();
}
