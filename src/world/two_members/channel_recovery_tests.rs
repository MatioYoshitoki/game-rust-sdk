use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::channel_tests::setup(false);
    c.mechanism_version = 105;
    c.skill=serde_json::from_value(json!({
        "mechanism":"self_status","id":"recover","cost":2,"windup_ms":60,"recovery_ms":120,"cooldown_ms":600,
        "channel":{"duration_ms":180,"interval_ms":60,"spirit_per_pulse":1,"pulse_count":3,
            "recovery":{"hp":{"fixed":8,"source_attack_bps":0,"target_max_hp_bps":0},
                "spirit":{"fixed":11,"source_attack_bps":0,"target_max_spirit_bps":0}}},
        "cast_movement":{"preparation":{"kind":"free"},"recovery":{"kind":"free"},"active":{"kind":"free"}}
    })).unwrap();
    w.members[0].hp = 50;
    w.members[0].spirit = 50;
    (c, w)
}
fn start(c: &Config, w: &mut World) {
    w.action(
        0,
        &Action::CastSelf {
            skill_id: c.skill.id.clone(),
        },
        "recover-input",
        c,
        0,
    )
    .unwrap();
    w.tick(c, 60);
    assert!(w.members[0].channel.is_some());
}
fn rows(e: &Events) -> Vec<(u32, u32, u32, u32)> {
    e.iter()
        .filter_map(|(recipient, e)| match e {
            EventV4::Private {
                event:
                    Event::ChannelRecoveryPulse {
                        raw_hp,
                        raw_spirit,
                        hp_restored,
                        spirit_restored,
                        ..
                    },
            } => {
                assert_eq!(*recipient, Some(0));
                Some((*raw_hp, *raw_spirit, *hp_restored, *spirit_restored))
            }
            _ => None,
        })
        .collect()
}
#[test]
fn channel_recovery_exact_budget_remainders_and_final_endpoint_then_recovery() {
    let (c, mut w) = setup();
    start(&c, &mut w);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (50, 48));
    let mut actual = vec![];
    for now in [120, 180, 240] {
        actual.extend(rows(&w.tick(&c, now)));
    }
    assert_eq!(actual, vec![(3, 4, 3, 4), (3, 4, 3, 4), (2, 3, 2, 3)]);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (58, 56));
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.members[0].recovery_until, 360);
    assert!(w.shots.is_empty());
    assert!(rows(&w.tick(&c, 270)).is_empty());
}
#[test]
fn channel_recovery_cancel_never_replays_remaining_budget() {
    let (c, mut w) = setup();
    start(&c, &mut w);
    assert_eq!(rows(&w.tick(&c, 120)).len(), 1);
    w.action(0, &Action::CancelAction {}, "stop-channel", &c, 121)
        .unwrap();
    let before = (w.members[0].hp, w.members[0].spirit);
    for now in [180, 240, 270] {
        assert!(rows(&w.tick(&c, now)).is_empty());
    }
    assert_eq!((w.members[0].hp, w.members[0].spirit), before);
}
#[test]
fn channel_recovery_current_receiving_and_spirit_lock_do_not_resample_totals() {
    let (mut c, mut w) = setup();
    for d in [
        json!({"id":"half","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":-5000,"duration_ms":130,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"lock","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock","modifier_bps":-10000,"duration_ms":130,"duplicate_policy":"refresh","source_scoped":true}),
    ] {
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(d).unwrap());
    }
    let target = w.members[0].reference.entity.clone();
    for id in ["half", "lock"] {
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), target.clone(), id, 0);
    }
    start(&c, &mut w);
    let first = rows(&w.tick(&c, 120));
    assert_eq!(first, vec![(3, 4, 1, 0)]);
    w.members[0].facts.atk_points += 100;
    w.members[0].facts.spirit_points += 100;
    assert_eq!(rows(&w.tick(&c, 180)), vec![(3, 4, 3, 4)]);
    assert_eq!(rows(&w.tick(&c, 240)), vec![(2, 3, 2, 3)]);
}
#[test]
fn channel_recovery_cost_failure_is_atomic_and_does_not_issue_a_pulse() {
    let (c, mut w) = setup();
    start(&c, &mut w);
    w.members[0].spirit = 0;
    let hp = w.members[0].hp;
    let e = w.tick(&c, 120);
    assert!(rows(&e).is_empty());
    assert_eq!(w.members[0].hp, hp);
    assert!(w.members[0].channel.is_none());
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ChannelEnded {
                reason: ChannelEndReason::NoResource,
                ..
            }
        }
    )));
}
#[test]
fn channel_recovery_one_tick_and_snapshot_formulas_are_bounded() {
    let (mut c, mut w) = setup();
    let channel = c.skill.channel.as_mut().unwrap();
    channel.duration_ms = 60;
    channel.pulse_count = Some(1);
    channel.recovery=serde_json::from_value(json!({"hp":{"fixed":0,"source_attack_bps":10000,"target_max_hp_bps":1000},"spirit":{"fixed":0,"source_attack_bps":0,"target_max_spirit_bps":5000}})).unwrap();
    start(&c, &mut w);
    let expected = w.members[0]
        .channel
        .as_ref()
        .unwrap()
        .recovery_budget
        .unwrap();
    w.members[0].facts.atk_points += 100;
    w.members[0].facts.hp_points += 100;
    w.members[0].facts.spirit_points += 100;
    let actual = rows(&w.tick(&c, 120));
    assert_eq!((actual[0].0, actual[0].1), (expected[0], expected[1]));
    assert!(w.members[0].channel.is_none());
}

#[test]
fn source_mana_channel_executes_five_absolute_deadlines_without_interval_drift() {
    let source: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_cast_lifecycle_vectors.json"
    ))
    .unwrap();
    let row = &source["vectors"][1];
    let active = &row["castLifecycle"]["active"];
    let (mut c, mut w) = setup();
    c.step_ms = 30;
    c.skill.windup_ms = 600;
    c.skill.cost = 0;
    c.skill.hp_cost = None;
    c.skill.channel=serde_json::from_value(json!({"duration_ms":active["durationMs"],"interval_ms":active["tickIntervalMs"],"pulse_count":row["channelExecution"]["tickCount"],"spirit_per_pulse":0,"recovery":{"remainder_policy":"final","spirit":{"fixed":0,"source_attack_bps":0,"target_max_spirit_bps":row["factors"][0]["params"][0]}}})).unwrap();
    w.members[0].facts.spirit_points = 211;
    w.members[0].spirit = 0;
    w.next_regen = 60_000;
    w.action(
        0,
        &Action::CastSelf {
            skill_id: c.skill.id.clone(),
        },
        "source-mana",
        &c,
        0,
    )
    .unwrap();
    let mut facts = vec![];
    for now in (30..=5610).step_by(30) {
        for (_, e) in w.tick(&c, now) {
            if let EventV4::Private {
                event:
                    Event::ChannelRecoveryPulse {
                        raw_spirit,
                        scheduled_at_ms,
                        executed_at_ms,
                        ..
                    },
            } = e
            {
                facts.push((raw_spirit, scheduled_at_ms, executed_at_ms));
            }
        }
    }
    assert_eq!(
        facts,
        vec![
            (4, 1600, 1620),
            (4, 2600, 2610),
            (4, 3600, 3600),
            (4, 4600, 4620),
            (5, 5600, 5610)
        ]
    );
    assert_eq!(w.members[0].spirit, 21);
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.members[0].recovery_until, 5730);
}

#[test]
fn channel_recovery_remainder_policies_conserve_u32_budgets_and_distinguish_tiny_pulses() {
    for name in ["earliest", "final"] {
        for count in [1_u32, 3, 128] {
            let (mut c, _) = setup();
            let channel = c.skill.channel.as_mut().unwrap();
            channel.pulse_count = Some(count);
            channel.recovery.as_mut().unwrap().remainder_policy =
                serde_json::from_value(json!(name)).unwrap();
            for total in [0_u32, 1, 2, 8, 11, u32::MAX] {
                let parts: Vec<_> = (0..count)
                    .map(|index| channel.share(total, index))
                    .collect();
                assert_eq!(
                    parts.iter().map(|n| u64::from(*n)).sum::<u64>(),
                    u64::from(total)
                );
                if name == "final" {
                    assert!(
                        parts[..count as usize - 1]
                            .iter()
                            .all(|part| *part == total / count)
                    );
                }
            }
        }
    }
    let (mut c, mut w) = setup();
    let mut definition = serde_json::to_value(c.skill.channel.clone().unwrap()).unwrap();
    definition["recovery"] = json!({"remainder_policy":"final","spirit":{"fixed":1,"source_attack_bps":0,"target_max_spirit_bps":0}});
    c.skill.channel = serde_json::from_value(definition).unwrap();
    start(&c, &mut w);
    for now in [120, 180] {
        assert_eq!(rows(&w.tick(&c, now)), vec![(0, 0, 0, 0)]);
    }
    w.action(
        0,
        &Action::CancelAction {},
        "cancel-final-remainder",
        &c,
        181,
    )
    .unwrap();
    let spirit = w.members[0].spirit;
    assert!(rows(&w.tick(&c, 240)).is_empty());
    assert_eq!(w.members[0].spirit, spirit);
}
