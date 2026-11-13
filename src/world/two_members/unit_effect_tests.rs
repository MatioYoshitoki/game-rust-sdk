use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

fn skill(channel: bool, ally: bool) -> SkillConfig {
    let mut s = json!({"mechanism":if ally {"ally_heal"} else {"unit_effect"},"id":"unit-effect","cost":2,"windup_ms":60,"recovery_ms":120,"cooldown_ms":600,"range":180});
    if !ally {
        s["damage_bps"] = json!(10000);
        s["unavoidable"] = json!(true);
        s["ignore_armor"] = json!(true);
    }
    if channel {
        s["channel"] =
            json!({"duration_ms":180,"interval_ms":60,"spirit_per_pulse":1,"pulse_count":3});
        if ally {
            s["channel"]["recovery"] = json!({"remainder_policy":"final","hp":{"fixed":8,"source_attack_bps":0,"target_max_hp_bps":0},"spirit":{"fixed":11,"source_attack_bps":0,"target_max_spirit_bps":0}});
        } else {
            s["channel"]["damage_distribution"] = json!("total");
        }
    }
    serde_json::from_value(s).unwrap()
}
pub(super) fn setup(channel: bool, ally: bool) -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(4);
    c.mechanism_version = 105;
    c.skill = skill(channel, ally);
    let d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/arena-a.json"
    ))
    .unwrap();
    c.combat_rules = Some(serde_json::from_value(d["combat_rules"].clone()).unwrap());
    let r = c.combat_rules.as_mut().unwrap();
    r.triggers.clear();
    r.status_policies.clear();
    r.death_guards.clear();
    for p in r.profiles.values_mut() {
        p.power_points_bps = 0;
    }

    c.combat_rules.as_mut().unwrap().skills.insert(
        c.skill.id.clone(),
        crate::config::AttackRule {
            base_power: Some(8),
            can_crit: false,
            unavoidable: true,
            ignore_armor: true,
            ..Default::default()
        },
    );
    for m in &mut w.members {
        m.position = [400., 400.];
        m.hp = 50;
        m.spirit = 50;
        m.facts.hp_points = 100;
        m.facts.spirit_points = 100;
    }
    for n in &mut w.npcs {
        n.position = [440., 400.];
        n.hp = 1000;
        n.returning = false;
    }
    (c, w)
}
pub(super) fn start(c: &Config, w: &mut World, ally: bool) -> Events {
    let target = if ally {
        w.members[1].reference.entity.clone()
    } else {
        w.npcs[0].entity.clone()
    };
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&target),
        },
        "unit-input",
        c,
        0,
    )
    .unwrap();
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, c, 60, &mut e);
    e
}
fn pulses(e: &Events) -> Vec<(u32, u32)> {
    e.iter()
        .filter_map(|(recipient, e)| {
            if let EventV4::Private {
                event:
                    Event::UnitChannelPulse {
                        pulse, raw_damage, ..
                    },
            } = e
            {
                assert_eq!(*recipient, Some(0));
                Some((*pulse, *raw_damage))
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn unit_effect_loader_roundtrip_and_strict_channel_bindings() {
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/arena-a.json"
    ))
    .unwrap();
    let mut s = serde_json::to_value(skill(true, false)).unwrap();
    s["id"] = d["skill"]["id"].clone();
    d["skill"] = s;
    p.write(&d);
    let cat = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(&cat.default_config().skill).unwrap(),
        d["skill"]
    );
    for case in 0..16 {
        let mut bad = d.clone();
        match case {
            0 => bad["mechanism_version"] = json!(104),
            1 => bad["skill"]["speed"] = json!(10),
            2 => bad["skill"]["radius"] = json!(0),
            3 => bad["skill"]["channel"] = Value::Null,
            4 => bad["skill"]["channel"]["duration_ms"] = json!(0),
            5 => bad["skill"]["channel"]["pulse_count"] = json!(0),
            6 => bad["skill"]["channel"]["damage_distribution"] = json!({"total":null}),
            7 => bad["skill"]["channel"]["on_success"] = json!([]),
            8 => {
                bad["skill"]["channel"]["on_success"] =
                    json!([{"after_pulse":4,"self_statuses":["ward"]}])
            }
            9 => {
                bad["skill"]["channel"]["on_success"] =
                    json!([{"after_pulse":2,"self_statuses":["missing"]}])
            }
            10 => {
                bad["skill"]["channel"]["on_success"] =
                    json!([{"after_pulse":2,"self_statuses":["wound"]}])
            }
            11 => {
                bad["skill"]["channel"]
                    .as_object_mut()
                    .unwrap()
                    .remove("pulse_count");
            }
            12 => bad["skill"]["channel"]["on_success"] = json!([{"after_pulse":2}]),
            13 => bad["skill"]["channel"]["damage_distribution"] = Value::Null,
            14 => {
                bad["skill"]["channel"]["recovery"] =
                    json!({"hp":{"fixed":8,"source_attack_bps":0,"target_max_hp_bps":0}})
            }
            _ => bad["skill"]["channel"]["interval_ms"] = json!(0),
        };
        p.write(&bad);
        assert!(p.load().is_err(), "case {case}");
    }
    let mut s = serde_json::to_value(skill(true, true)).unwrap();
    s["id"] = d["skill"]["id"].clone();
    d["combat_rules"]["skills"].as_object_mut().unwrap().clear();
    d["skill"] = s;
    p.write(&d);
    assert!(p.load().is_ok());
}
#[test]
fn unit_effect_instant_has_no_flight_and_does_not_hit_intervening_npc() {
    let (c, mut w) = setup(false, false);
    w.npcs[1].position = [420., 400.];
    let old = w.npcs[1].hp;
    let hp = w.npcs[0].hp;
    let e = start(&c, &mut w, false);
    assert!(w.shots.is_empty());
    assert_eq!(w.npcs[1].hp, old);
    assert!(w.npcs[0].hp < hp);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ActionCommitted { .. }
        }
    )));
    assert_eq!(w.members[0].spirit, 48);
}
#[test]
fn unit_effect_strict_target_lost_or_range_before_commit_is_free() {
    for case in 0..3 {
        let (c, mut w) = setup(false, false);
        let target = World::unit_wire(&w.npcs[0].entity);
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target,
            },
            "unit",
            &c,
            0,
        )
        .unwrap();
        match case {
            0 => w.npcs[0].hp = 0,
            1 => w.npcs[0].entity.generation += 1,
            _ => w.npcs[0].position = [800., 400.],
        };
        let p = w.members[0].pending.take().unwrap();
        let mut e = vec![];
        w.commit(0, p, &c, 60, &mut e);
        assert_eq!(w.members[0].spirit, 50);
        assert!(w.members[0].cooldowns.is_empty());
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected { .. }
            }
        )));
    }
}
#[test]
fn unit_effect_channel_freezes_total_and_allocates_final_remainder_exactly() {
    let (mut c, mut w) = setup(true, false);
    let e = start(&c, &mut w, false);
    assert!(pulses(&e).is_empty());
    let raw = w.members[0].channel.as_ref().unwrap().raw;
    assert_eq!(raw, 8);
    w.members[0].facts.atk_points += 900;
    let mut values = vec![];
    for now in [120, 180, 240] {
        let mut e = vec![];
        w.advance_channel(0, &c, now, &mut e);
        values.extend(pulses(&e));
    }
    assert_eq!(values, vec![(1, 2), (2, 2), (3, 4)]);
    assert!(w.shots.is_empty());
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.members[0].spirit, 45);
    assert_eq!(w.members[0].recovery_until, 360);
    c.skill.cost = 99;
}
#[test]
fn unit_effect_channel_interrupt_prefix_never_refunds_or_replays() {
    for prefix in 0..3 {
        let (c, mut w) = setup(true, false);
        start(&c, &mut w, false);
        for i in 0..prefix {
            w.advance_channel(0, &c, 120 + i * 60, &mut vec![]);
        }
        let old = w.npcs[0].hp;
        w.action(0, &Action::CancelAction {}, "cancel", &c, 121 + prefix * 60)
            .unwrap();
        for now in [240, 300, 600] {
            let mut e = vec![];
            w.advance_channel(0, &c, now, &mut e);
            assert!(pulses(&e).is_empty());
        }
        assert_eq!(w.npcs[0].hp, old);
        assert_eq!(w.members[0].spirit, 48 - prefix as u32);
    }
}
#[test]
fn unit_effect_channel_revalidates_identity_range_and_target_eligibility_each_pulse() {
    for case in 0..4 {
        let (c, mut w) = setup(true, false);
        start(&c, &mut w, false);
        w.advance_channel(0, &c, 120, &mut vec![]);
        match case {
            0 => w.npcs[0].entity.generation += 1,
            1 => w.npcs[0].hp = 0,
            2 => w.npcs[0].returning = true,
            _ => w.npcs[0].position = [800., 400.],
        };
        let spirit = w.members[0].spirit;
        let mut e = vec![];
        w.advance_channel(0, &c, 180, &mut e);
        assert!(w.members[0].channel.is_none());
        assert!(pulses(&e).is_empty());
        assert_eq!(w.members[0].spirit, spirit);
    }
}
#[test]
fn unit_effect_ally_channel_recovers_only_frozen_target_with_two_private_recipients() {
    let (c, mut w) = setup(true, true);
    start(&c, &mut w, true);
    assert_eq!(w.members[1].hp, 50);
    w.members[1].facts.hp_points = 1000;
    w.members[0].facts.atk_points = 5000;
    let mut recipients = vec![];
    for now in [120, 180, 240] {
        let mut e = vec![];
        w.advance_channel(0, &c, now, &mut e);
        recipients.extend(e.iter().filter_map(|(r, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::HealResolved { .. }
                }
            )
            .then_some(*r)
        }));
    }
    assert_eq!(w.members[1].hp, 58);
    assert_eq!(w.members[1].spirit, 61);
    assert_eq!(w.members[0].hp, 50);
    assert_eq!(w.members[0].spirit, 45);
    assert_eq!(
        recipients,
        vec![Some(0), Some(1), Some(0), Some(1), Some(0), Some(1)]
    );
}
#[test]
fn unit_effect_full_health_legal_ticks_apply_full_status_once_at_second_tick() {
    let (mut c, mut w) = setup(true, true);
    let d=serde_json::from_value(json!({"id":"gate-grant","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":1000,"duration_ms":1200,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true})).unwrap();
    c.pve_mut().status_definitions.push(d);
    c.skill.channel.as_mut().unwrap().on_success =
        serde_json::from_value(json!([{"after_pulse":2,"self_statuses":["gate-grant"]}])).unwrap();
    w.members[1].hp = 100;
    start(&c, &mut w, true);
    for now in [120, 180, 240] {
        w.advance_channel(0, &c, now, &mut vec![]);
        let matches = w
            .statuses
            .entries()
            .iter()
            .filter(|s| s.definition_id == "gate-grant")
            .collect::<Vec<_>>();
        if now == 120 {
            assert!(matches.is_empty());
        } else {
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].stacks, Some(1));
            assert_eq!(matches[0].expires_at_ms, 1380);
        }
    }
}
fn gate(c: &mut Config) {
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"gate-grant","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":1000,"duration_ms":1200,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true})).unwrap());
    c.skill.channel.as_mut().unwrap().on_success =
        serde_json::from_value(json!([{"after_pulse":2,"self_statuses":["gate-grant"]}])).unwrap();
}
#[test]
fn unit_effect_immunity_is_a_legal_tick_but_failed_resource_is_not() {
    for resource_failure in [false, true] {
        let (mut c, mut w) = setup(true, false);
        gate(&mut c);
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"unit-immune","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
        let target = w.npcs[0].entity.clone();
        super::periodic_tests::bind(&mut w, &c, Owner::Npc(0), target, "unit-immune", 0);
        start(&c, &mut w, false);
        let hp = w.npcs[0].hp;
        w.advance_channel(0, &c, 120, &mut vec![]);
        if resource_failure {
            w.members[0].spirit = 0;
        }
        w.advance_channel(0, &c, 180, &mut vec![]);
        assert_eq!(w.npcs[0].hp, hp);
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .filter(|s| s.definition_id == "gate-grant")
                .count(),
            usize::from(!resource_failure)
        );
        if resource_failure {
            assert!(w.members[0].channel.is_none());
        }
    }
}
#[test]
fn unit_effect_channel_los_loss_before_next_deadline_ends_without_cost_or_gate() {
    let (mut c, mut w) = setup(true, false);
    gate(&mut c);
    start(&c, &mut w, false);
    w.advance_channel(0, &c, 120, &mut vec![]);
    let spirit = w.members[0].spirit;
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
    w.advance_channel(0, &c, 150, &mut vec![]);
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.members[0].spirit, spirit);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gate-grant")
    );
}
#[test]
fn unit_effect_revocation_and_regrant_preserve_cooldown_and_completed_prefix() {
    let (c, mut w) = setup(true, false);
    start(&c, &mut w, false);
    w.advance_channel(0, &c, 120, &mut vec![]);
    let hp = w.npcs[0].hp;
    let ready = w.members[0].cooldowns[&c.skill.id];
    let spirit = w.members[0].spirit;
    w.replace_skill_loadout(
        0,
        SkillLoadoutState {
            declared: true,
            revision: 1,
            loadout: SkillLoadout {
                owned_skill_ids: vec![c.skill.id.clone()],
                equipped_skill_ids: vec![],
            },
        },
        180,
    );
    w.advance_channel(0, &c, 180, &mut vec![]);
    assert_eq!(w.npcs[0].hp, hp);
    assert_eq!(w.members[0].spirit, spirit);
    w.replace_skill_loadout(
        0,
        SkillLoadoutState {
            declared: true,
            revision: 2,
            loadout: SkillLoadout {
                owned_skill_ids: vec![c.skill.id.clone()],
                equipped_skill_ids: vec![c.skill.id.clone()],
            },
        },
        181,
    );
    assert_eq!(w.members[0].cooldowns[&c.skill.id], ready);
    assert!(w.members[0].channel.is_none());
    assert!(
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: World::unit_wire(&w.npcs[0].entity)
            },
            "again",
            &c,
            190
        )
        .is_err()
    );
}
#[test]
fn unit_effect_new_movement_cancels_before_exact_due_pulse() {
    let (mut c, mut w) = setup(true, false);
    gate(&mut c);
    c.skill.cast_movement=serde_json::from_value(json!({"preparation":{"kind":"free"},"recovery":{"kind":"free"},"active":{"kind":"interrupt_on_new_move","min_turn_degrees":5,"min_destination_distance":2}})).unwrap();
    start(&c, &mut w, false);
    w.advance_channel(0, &c, 120, &mut vec![]);
    let hp = w.npcs[0].hp;
    w.action(
        0,
        &Action::Move {
            direction: [1., 0.],
        },
        "move",
        &c,
        180,
    )
    .unwrap();
    w.advance_channel(0, &c, 180, &mut vec![]);
    assert_eq!(w.npcs[0].hp, hp);
    assert!(w.members[0].channel.is_none());
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gate-grant")
    );
}
#[test]
fn unit_effect_target_movement_precedes_due_tick_for_both_seat_orders() {
    for source in [0, 1] {
        let (mut c, mut w) = setup(true, true);
        c.params.regen.near_ratio = 0.;
        c.params.regen.far_ratio = 0.;
        for n in &mut w.npcs {
            n.hp = 0;
        }
        let target = 1 - source;
        w.members[source].position = [400., 400.];
        w.members[target].position = [579., 400.];
        w.members[target].facts.spd_points = 1000;
        w.action(
            source,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: World::unit_wire(&w.members[target].reference.entity),
            },
            "heal",
            &c,
            0,
        )
        .unwrap();
        let p = w.members[source].pending.take().unwrap();
        w.commit(source, p, &c, 60, &mut vec![]);
        w.action(
            target,
            &Action::Move {
                direction: [1., 0.],
            },
            "escape",
            &c,
            119,
        )
        .unwrap();
        let hp = w.members[target].hp;
        w.tick(&c, 120);
        assert!(w.members[target].position[0] > 580.);
        assert_eq!(w.members[target].hp, hp);
        assert!(w.members[source].channel.is_none());
        assert_eq!(w.members[source].spirit, 48);
    }
}
#[test]
fn unit_effect_target_protection_and_source_death_cancel_without_next_tick() {
    for dead_source in [false, true] {
        let (c, mut w) = setup(true, true);
        start(&c, &mut w, true);
        w.advance_channel(0, &c, 120, &mut vec![]);
        if dead_source {
            w.members[0].hp = 0;
        } else {
            w.members[1].protected = true;
        }
        let hp = w.members[1].hp;
        let sp = w.members[0].spirit;
        w.advance_channel(0, &c, 150, &mut vec![]);
        assert!(w.members[0].channel.is_none());
        w.advance_channel(0, &c, 180, &mut vec![]);
        assert_eq!(w.members[1].hp, hp);
        assert_eq!(w.members[0].spirit, sp);
    }
}
#[test]
fn unit_effect_source_three_absolute_ticks_and_second_tick_full_once_projection() {
    let row: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_unit_channel_vectors.json"
    ))
    .unwrap();
    assert_eq!(row["fullSourceSkillEffectsAccepted"], false);
    let (mut c, mut w) = setup(true, false);
    gate(&mut c);
    c.skill.windup_ms = 180;
    c.skill.damage_bps = row["factors"][0]["params"][3].as_u64().unwrap() as u32;
    let channel = c.skill.channel.as_mut().unwrap();
    channel.duration_ms = row["castLifecycle"]["active"]["durationMs"]
        .as_u64()
        .unwrap();
    channel.interval_ms = row["castLifecycle"]["active"]["tickIntervalMs"]
        .as_u64()
        .unwrap();
    channel.pulse_count = Some(row["channelExecution"]["tickCount"].as_u64().unwrap() as u32);
    channel.on_success[0].after_pulse =
        row["channelExecution"]["factorSchedules"][2]["successfulTickCount"]
            .as_u64()
            .unwrap() as u32;
    let d = c.pve_mut().status_definitions.last_mut().unwrap();
    d.duration_ms = row["factors"][2]["params"][1].as_u64().unwrap();
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        },
        "source-unit",
        &c,
        0,
    )
    .unwrap();
    let p = w.members[0].pending.take().unwrap();
    w.commit(0, p, &c, 180, &mut vec![]);
    let mut clocks = vec![];
    for now in (210..=1410).step_by(30) {
        let mut e = vec![];
        w.advance_channel(0, &c, now, &mut e);
        for (_, e) in e {
            if let EventV4::Private {
                event:
                    Event::UnitChannelPulse {
                        raw_damage,
                        scheduled_at_ms,
                        executed_at_ms,
                        ..
                    },
            } = e
            {
                clocks.push((raw_damage, scheduled_at_ms, executed_at_ms));
            }
        }
    }
    assert_eq!(clocks, vec![(8, 580, 600), (8, 980, 990), (8, 1380, 1380)]);
    let d = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "gate-grant")
        .unwrap();
    assert_eq!(d.stacks, Some(1));
    assert_eq!(d.expires_at_ms, 12990);
}
#[test]
fn unit_effect_channel_rust_replica_validates_bound_identity_and_atomic_delta() {
    use crate::replication::{DecodeError, ReplicaV6};
    for ally in [true, false] {
        let (c, mut w) = setup(true, ally);
        start(&c, &mut w, ally);
        let snapshot = w.snapshot(0, &c, Phase::Active, 60);
        let key = MatchKey {
            id: "unit-codec".into(),
            incarnation: 1,
        };
        let f = PublicationEnvelopeV6 {
            key: key.clone(),
            publication_id: "unit-1".into(),
            recipient_id: "P00".into(),
            connection: Connection {
                id: "c".into(),
                generation: 1,
            },
            stream_epoch: 1,
            revision: 1,
            payload: PublicationPayloadV6::Full {
                snapshot: Box::new(snapshot.clone()),
                reason: FullReason::Initial,
            },
        };
        let mut r = ReplicaV6::new(key, "P00".into());
        r.apply(&f).unwrap();
        for case in 0..6 {
            let mut bad = snapshot.clone();
            let ch = bad.own.active_channel.as_mut().unwrap();
            match case {
                0 => ch.target.as_mut().unwrap().generation += 1,
                1 => {
                    ch.target.as_mut().unwrap().kind = if ally {
                        StatusEntityKind::Monster
                    } else {
                        StatusEntityKind::Participant
                    }
                }
                2 => ch.target.as_mut().unwrap().id = "unknown".into(),
                3 => ch.total_pulses = None,
                4 => ch.target = None,
                _ => {
                    ch.target = Some(if ally {
                        snapshot.members[2].entity.clone()
                    } else {
                        snapshot.monsters[1].entity.clone()
                    })
                }
            };
            let d = PublicationEnvelopeV6 {
                publication_id: "unit-2".into(),
                revision: 2,
                payload: PublicationPayloadV6::Delta {
                    base_publication_id: "unit-1".into(),
                    base_revision: 1,
                    delta: Box::new(crate::replication::diff(&snapshot, &bad)),
                },
                ..f.clone()
            };
            assert_eq!(r.apply(&d), Err(DecodeError::InvalidFrame));
            assert_eq!(r.snapshot(), Some(&snapshot));
        }
    }
}
#[test]
fn unit_effect_spirit_only_ally_channel_has_no_fictitious_hp_event() {
    let (mut c, mut w) = setup(true, true);
    c.skill
        .channel
        .as_mut()
        .unwrap()
        .recovery
        .as_mut()
        .unwrap()
        .hp = None;
    start(&c, &mut w, true);
    let mut e = vec![];
    w.advance_channel(0, &c, 120, &mut e);
    assert_eq!(w.members[1].spirit, 53);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ChannelRecoveryPulse {
                raw_hp: 0,
                spirit_restored: 3,
                ..
            }
        }
    )));
    assert!(!e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::HealResolved { .. }
        }
    )));
}
