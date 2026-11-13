use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn policy(accept: bool) -> Value {
    json!({"reserve_at":if accept{"accept"}else{"none"},"consume_at":"commit","cooldown_starts_at":"commit","refund_before_commit":{"kind":"full"}})
}
fn setup(accept: bool) -> (Config, World) {
    let (mut c, mut w) = super::hp_cost_tests::setup(4);
    c.mechanism_version = 105;
    c.skill=serde_json::from_value(json!({"id":"reserve","mechanism":"projectile","windup_ms":60,"recovery_ms":120,"cooldown_ms":600,"range":600,"radius":6,"speed":100,"cost":10,"damage_bps":10000,"hp_cost":{"mode":"current_hp_ratio","value":0.5,"min_hp_after_cost":1},"resource_commit":policy(accept)})).unwrap();
    w.members[0].hp = 100;
    w.members[0].spirit = 100;
    (c, w)
}
fn prepare(c: &Config, w: &mut World) {
    w.action(
        0,
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "reserve-request",
        c,
        0,
    )
    .unwrap();
}
fn commit(c: &Config, w: &mut World) -> Events {
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, c, 60, &mut e);
    e
}
#[test]
fn resource_reservation_strict_load_roundtrip_and_all_skill_carriers() {
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/arena-a.json"
    ))
    .unwrap();
    d["skill"]["resource_commit"] = policy(true);
    for s in d["additional_skills"].as_array_mut().unwrap() {
        s["resource_commit"] = policy(false);
    }
    p.write(&d);
    let loaded = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(&loaded.default_config().skill).unwrap()["resource_commit"],
        policy(true)
    );
    for case in 0..10 {
        let mut bad = d.clone();
        match case {
            0 => bad["mechanism_version"] = json!(104),
            1 => bad["skill"]["resource_commit"] = Value::Null,
            2 => bad["skill"]["resource_commit"]["reserve_at"] = json!({"accept":null}),
            3 => bad["skill"]["resource_commit"]["consume_at"] = json!("accept"),
            4 => bad["skill"]["resource_commit"]["cooldown_starts_at"] = json!("channel_end"),
            5 => bad["skill"]["resource_commit"]["refund_before_commit"]["kind"] = json!("ratio"),
            6 => bad["skill"]["resource_commit"]["extra"] = json!(true),
            7 => {
                bad["skill"]["resource_commit"]
                    .as_object_mut()
                    .unwrap()
                    .remove("consume_at");
            }
            8 => bad["additional_skills"][0]["resource_commit"] = Value::Null,
            _ => bad["skill"]["resource_commit"]["refund_before_commit"]["extra"] = json!(true),
        };
        p.write(&bad);
        assert!(p.load().is_err(), "{case}");
    }
}
#[test]
fn resource_reservation_accept_freezes_hp_and_never_debits_early() {
    let (c, mut w) = setup(true);
    prepare(&c, &mut w);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (100, 100));
    let view = w
        .snapshot(0, &c, Phase::Active, 0)
        .own
        .pending_action
        .unwrap();
    assert_eq!(view.action_id.as_deref(), Some("reserve-request"));
    assert_eq!(
        view.resource_reservation,
        Some(ResourceReservationView {
            hp: 50,
            spirit: 10,
            minimum_hp_after_cost: 1
        })
    );
    w.members[0].hp = 80;
    let e = commit(&c, &mut w);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (30, 90));
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ActionCommitted {
                spirit_cost: 10,
                hp_cost: Some(50),
                ..
            }
        }
    )));
    assert_eq!(w.members[0].cooldowns[&c.skill.id], 660);
}
#[test]
fn resource_reservation_none_keeps_live_hp_formula_and_no_hold() {
    let (c, mut w) = setup(false);
    prepare(&c, &mut w);
    assert!(w.members[0].pending.as_ref().unwrap().reservation.is_none());
    w.members[0].hp = 80;
    commit(&c, &mut w);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (40, 90));
}
#[test]
fn resource_reservation_failed_hp_spirit_or_cooldown_commit_is_atomic() {
    for case in 0..3 {
        let (c, mut w) = setup(true);
        prepare(&c, &mut w);
        match case {
            0 => w.members[0].hp = 50,
            1 => w.members[0].spirit = 9,
            _ => {
                w.members[0].cooldowns.insert(c.skill.id.clone(), 1000);
            }
        }
        let old = (
            w.members[0].hp,
            w.members[0].spirit,
            w.members[0].cooldowns.clone(),
            w.shots.len(),
        );
        let e = commit(&c, &mut w);
        assert_eq!(
            (
                w.members[0].hp,
                w.members[0].spirit,
                w.members[0].cooldowns.clone(),
                w.shots.len()
            ),
            old
        );
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected { .. }
            }
        )));
        assert!(w.members[0].pending.is_none());
    }
}
#[test]
fn resource_reservation_cancel_and_skill_revoke_release_without_cost_or_cooldown() {
    for revoke in [false, true] {
        let (c, mut w) = setup(true);
        prepare(&c, &mut w);
        if revoke {
            w.replace_skill_loadout(
                0,
                SkillLoadoutState {
                    declared: true,
                    revision: 1,
                    loadout: SkillLoadout {
                        owned_skill_ids: vec![],
                        equipped_skill_ids: vec![],
                    },
                },
                30,
            );
        } else {
            w.action(0, &Action::CancelAction {}, "cancel", &c, 30)
                .unwrap();
        }
        assert!(w.members[0].pending.is_none());
        assert_eq!((w.members[0].hp, w.members[0].spirit), (100, 100));
        assert!(w.members[0].cooldowns.is_empty());
        assert!(w.shots.is_empty());
    }
}
#[test]
fn resource_reservation_new_acceptance_recomputes_after_prior_release() {
    let (c, mut w) = setup(true);
    prepare(&c, &mut w);
    w.action(0, &Action::CancelAction {}, "cancel", &c, 30)
        .unwrap();
    w.members[0].hp = 60;
    w.action(
        0,
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "next",
        &c,
        31,
    )
    .unwrap();
    assert_eq!(
        w.members[0]
            .pending
            .as_ref()
            .unwrap()
            .reservation
            .unwrap()
            .hp,
        30
    );
    commit(&c, &mut w);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (30, 90));
}
#[test]
fn resource_reservation_private_codec_freezes_amount_and_action_identity() {
    use crate::replication::{DecodeError, ReplicaV6};
    let (c, mut w) = setup(true);
    prepare(&c, &mut w);
    let snap = w.snapshot(0, &c, Phase::Active, 0);
    let key = MatchKey {
        id: "reservation".into(),
        incarnation: 1,
    };
    let f = PublicationEnvelopeV6 {
        key: key.clone(),
        publication_id: "reserve-1".into(),
        recipient_id: "P00".into(),
        connection: Connection {
            id: "c".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            snapshot: Box::new(snap.clone()),
            reason: FullReason::Initial,
        },
    };
    let mut r = ReplicaV6::new(key, "P00".into());
    r.apply(&f).unwrap();
    for case in 0..6 {
        let mut bad = snap.clone();
        bad.logical_ms = 30;
        let p = bad.own.pending_action.as_mut().unwrap();
        match case {
            0 => p.resource_reservation.as_mut().unwrap().hp += 1,
            1 => p.resource_reservation = None,
            2 => p.action_id = None,
            3 => {
                p.resource_reservation
                    .as_mut()
                    .unwrap()
                    .minimum_hp_after_cost = 0
            }
            4 => p.resource_reservation.as_mut().unwrap().spirit += 1,
            _ => {
                p.action_id = None;
                p.resource_reservation = None;
            }
        };
        let d = PublicationEnvelopeV6 {
            publication_id: "reserve-2".into(),
            revision: 2,
            payload: PublicationPayloadV6::Delta {
                base_publication_id: "reserve-1".into(),
                base_revision: 1,
                delta: Box::new(crate::replication::diff(&snap, &bad)),
            },
            ..f.clone()
        };
        assert_eq!(r.apply(&d), Err(DecodeError::InvalidFrame));
        assert_eq!(r.snapshot(), Some(&snap));
    }
}
#[test]
fn resource_reservation_unit_target_lost_releases_uncommitted_hold() {
    let (mut c, mut w) = setup(true);
    let mut skill = serde_json::to_value(&c.skill).unwrap();
    skill["mechanism"] = json!("unit_projectile");
    c.skill = serde_json::from_value(skill).unwrap();
    w.npcs[0].position = [w.members[0].position[0] + 40., w.members[0].position[1]];
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        },
        "reserve-unit",
        &c,
        0,
    )
    .unwrap();
    assert!(w.members[0].pending.as_ref().unwrap().reservation.is_some());
    w.npcs[0].entity.generation += 1;
    commit(&c, &mut w);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (100, 100));
    assert!(w.members[0].cooldowns.is_empty());
    assert!(w.shots.is_empty());
}
#[test]
fn resource_reservation_charge_release_and_auto_commit_use_one_frozen_cost() {
    for auto in [false, true] {
        let (mut c, mut w) = setup(true);
        c.skill.charge=serde_json::from_value(json!({"min_ms":60,"max_ms":180,"min_damage_bps":10000,"early_release":{"kind":"queue_until_min"},"release_windup_ms":30})).unwrap();
        w.action(
            0,
            &Action::StartCharge {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.],
            },
            "reserve-charge",
            &c,
            0,
        )
        .unwrap();
        let id = w.members[0].pending.as_ref().unwrap().id.clone();
        let reserved = w.members[0].pending.as_ref().unwrap().reservation;
        if !auto {
            w.action(
                0,
                &Action::ReleaseCharge {
                    action_id: id,
                    direction: None,
                },
                "release",
                &c,
                30,
            )
            .unwrap();
        }
        w.members[0].hp = 80;
        assert_eq!(w.members[0].pending.as_ref().unwrap().reservation, reserved);
        let p = w.members[0].pending.take().unwrap();
        let at = p.commits;
        w.commit(0, p, &c, at, &mut vec![]);
        assert_eq!((w.members[0].hp, w.members[0].spirit), (30, 90));
        assert_eq!(w.members[0].cooldowns[&c.skill.id], at + 600);
    }
}
#[test]
fn resource_reservation_queue_holds_only_after_real_preparation_and_rechecks_resources() {
    for exhausted in [false, true] {
        let (mut c, mut w) = setup(true);
        c.skill_followup = serde_json::from_value(json!({"window_ms":150})).unwrap();
        let mut later = c.skill.clone();
        later.id = "later".into();
        c.additional_skills[0] = later;
        w.members[0].recovery_skill = Some(c.skill.id.clone());
        w.members[0].recovery_until = 90;
        w.action(
            0,
            &Action::Cast {
                skill_id: "later".into(),
                direction: [1., 0.],
            },
            "queue-resource",
            &c,
            30,
        )
        .unwrap();
        assert!(w.members[0].pending.is_none());
        assert!(w.members[0].queued_cast.is_some());
        assert_eq!((w.members[0].hp, w.members[0].spirit), (100, 100));
        w.members[0].hp = 80;
        if exhausted {
            w.members[0].spirit = 9;
        }
        let mut e = vec![];
        w.promote_followup(0, &c, 90, &mut e);
        if exhausted {
            assert!(w.members[0].pending.is_none());
            assert_eq!((w.members[0].hp, w.members[0].spirit), (80, 9));
        } else {
            assert_eq!(
                w.members[0]
                    .pending
                    .as_ref()
                    .unwrap()
                    .reservation
                    .unwrap()
                    .hp,
                40
            );
            let p = w.members[0].pending.take().unwrap();
            w.commit(0, p, &c, 150, &mut e);
            assert_eq!((w.members[0].hp, w.members[0].spirit), (40, 90));
        }
    }
}
#[test]
fn resource_reservation_control_and_terminal_release_without_late_payment() {
    for death in [false, true] {
        let (mut c, mut w) = setup(true);
        prepare(&c, &mut w);
        if death {
            w.terminal_intent(0, Outcome::Death, &mut vec![]);
        } else {
            c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"reserve-stun","mechanism":"timed_control","attribute":"stun","duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
            let target = w.members[0].reference.entity.clone();
            super::periodic_tests::bind(&mut w, &c, Owner::Npc(0), target, "reserve-stun", 30);
        }
        assert!(w.members[0].pending.is_none());
        assert_eq!((w.members[0].hp, w.members[0].spirit), (100, 100));
        assert!(w.members[0].cooldowns.is_empty());
    }
}
#[test]
fn resource_reservation_source_policies_are_complete_distinct_and_keep_nonbinding_scope() {
    let f: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_resource_reservation_vectors.json"
    ))
    .unwrap();
    assert_eq!(f["fullSourceSkillEffectsAccepted"], false);
    let mut names = std::collections::BTreeSet::new();
    for (k, count) in [("accept", 58), ("none", 20), ("missing", 8)] {
        let a = f["groups"][k].as_array().unwrap();
        assert_eq!(a.len(), count);
        for id in a {
            assert!(names.insert(id.as_str().unwrap()));
        }
    }
    for row in f["vectors"].as_array().unwrap() {
        let accept = row["resources"]["reserveAt"] == "accept";
        assert_eq!(row["resources"]["consumeAt"], "commit");
        assert_eq!(row["resources"]["cooldownStartsAt"], "commit");
        assert_eq!(row["resources"]["refundBeforeCommit"]["kind"], "full");
        let (c, mut w) = setup(accept);
        prepare(&c, &mut w);
        w.members[0].hp = 80;
        commit(&c, &mut w);
        assert_eq!(w.members[0].hp, if accept { 30 } else { 40 });
    }
}
