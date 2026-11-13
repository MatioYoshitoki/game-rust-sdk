use super::*;

fn preparation(value: &mut Value) {
    super::cast_movement::configure(value);
    value["mechanism_version"] = json!(105);
    value["skill"]["cast_movement"]["preparation"] =
        json!({"kind":"interrupt_on_new_move","min_turn_degrees":15,"min_destination_distance":4.});
}
fn recovery(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(105);
    value["skill"] = json!({"mechanism":"self_status","id":"recover","cost":2,"windup_ms":60,"recovery_ms":120,"cooldown_ms":600,
        "channel":{"duration_ms":180,"interval_ms":60,"spirit_per_pulse":1,"pulse_count":3,
            "recovery":{"hp":{"fixed":8,"source_attack_bps":0,"target_max_hp_bps":0},"spirit":{"fixed":11,"source_attack_bps":0,"target_max_spirit_bps":0}}},
        "cast_movement":{"preparation":{"kind":"free"},"recovery":{"kind":"free"},"active":{"kind":"free"}}});
}
fn queue(value: &mut Value) {
    super::charge::configure(value);
    value["mechanism_version"] = json!(105);
    value["skill"]["charge"]["early_release"] = json!({"kind":"queue_until_min"});
}
#[test]
fn cast_lifecycle_configuration_is_strict_versioned_and_checks_all_slots() {
    for f in [preparation as fn(&mut Value), recovery, queue] {
        assert!(delta_catalog(8, f).is_ok());
    }
    for (path, bad) in [
        ("/skill/channel/pulse_count", json!(0)),
        ("/skill/channel/pulse_count", json!(129)),
        ("/skill/channel/pulse_count", Value::Null),
        ("/skill/channel/pulse_count", json!(2)),
        ("/skill/channel/recovery", json!({})),
        ("/skill/channel/recovery", Value::Null),
        ("/skill/channel/recovery/hp/fixed", json!(-1)),
        ("/skill/channel/recovery/hp/target_max_hp_bps", json!(10001)),
        ("/skill/cast_movement/active", Value::Null),
        ("/mechanism_version", json!(104)),
    ] {
        assert!(
            delta_catalog(8, |v| {
                recovery(v);
                *v.pointer_mut(path).unwrap() = bad;
            })
            .is_err(),
            "{path}"
        );
    }
    for policy in [
        json!({"kind":"interrupt_on_new_move","min_turn_degrees":0,"min_destination_distance":4.}),
        json!({"kind":"interrupt_on_new_move","min_turn_degrees":181,"min_destination_distance":4.}),
        json!({"kind":"interrupt_on_new_move","min_turn_degrees":15,"min_destination_distance":0.}),
        json!({"kind":"interrupt_on_new_move","min_turn_degrees":15}),
        json!({"kind":"interrupt_on_new_move","min_turn_degrees":15,"min_destination_distance":4.,"extra":1}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                preparation(v);
                v["skill"]["cast_movement"]["preparation"] = policy;
            })
            .is_err()
        );
    }
    for bad in [
        Value::Null,
        json!({}),
        json!({"kind":"queue_until_min","extra":1}),
        json!("queue_until_min"),
    ] {
        assert!(
            delta_catalog(8, |v| {
                queue(v);
                v["skill"]["charge"]["early_release"] = bad;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            queue(v);
            v["mechanism_version"] = json!(104);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            preparation(v);
            v["skill"]["cast_movement"]["recovery"] =
                v["skill"]["cast_movement"]["preparation"].clone();
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            recovery(v);
            v["skill"]["channel"]["pulse_count"] = json!(1);
            v["skill"]["channel"]["duration_ms"] = json!(60);
        })
        .is_ok()
    );
}
#[test]
fn cast_lifecycle_queued_release_publishes_locked_future_commit_and_deduplicates() {
    let mut h = DeltaHarness::new(2, queue);
    h.active();
    let charge = super::charge::start(&mut h, 0);
    let before = h.snapshot(0).own.clone();
    let input = h.input(
        0,
        Action::ReleaseCharge {
            action_id: charge.action_id.clone(),
            direction: None,
        },
    );
    let accepted = h.engine.input(input.clone()).unwrap();
    h.pump();
    assert_eq!(h.engine.input(input.clone()), Ok(accepted));
    h.pump();
    let pending = h.snapshot(0).own.pending_action.as_ref().unwrap();
    assert_eq!(pending.commits_at_ms, charge.min_release_at_ms);
    assert_eq!(
        pending.charge.as_ref().unwrap().released_at_ms,
        Some(h.engine.now)
    );
    assert_eq!(h.snapshot(0).own.spirit, before.spirit);
    h.ticks(3);
    assert!(h.snapshot(0).own.pending_action.is_none());
    assert_eq!(
        h.snapshot(0).own.spirit,
        before.spirit - h.engine.config.skill.cost
    );
    assert_eq!(h.engine.input(input), Ok(accepted));
    h.pump();
    assert_eq!(
        h.events
            .iter()
            .filter(|(pid, e)| pid.as_deref() == Some("P00")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionCommitted {
                            charge: Some(_),
                            ..
                        }
                    }
                ))
            .count(),
        1
    );
    h.close();
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn cast_lifecycle_recovery_full_and_delta_accept_final_scheduled_endpoint() {
    let mut h = DeltaHarness::new(2, recovery);
    h.active();
    h.action(
        0,
        Action::CastSelf {
            skill_id: "recover".into(),
        },
    )
    .unwrap();
    h.ticks(2);
    assert_eq!(
        h.snapshot(0)
            .own
            .active_channel
            .as_ref()
            .unwrap()
            .total_pulses,
        Some(3)
    );
    h.ticks(4);
    let channel = h.snapshot(0).own.active_channel.as_ref().unwrap();
    assert_eq!(channel.pulses, 2);
    assert_eq!(channel.next_pulse_at_ms, Some(channel.ends_at_ms));
    h.ticks(2);
    assert!(h.snapshot(0).own.active_channel.is_none());
    let rows: Vec<_> = h
        .events
        .iter()
        .filter_map(|(pid, e)| match e {
            EventV4::Private {
                event:
                    Event::ChannelRecoveryPulse {
                        raw_hp, raw_spirit, ..
                    },
            } => {
                assert_eq!(pid.as_deref(), Some("P00"));
                Some((*raw_hp, *raw_spirit))
            }
            _ => None,
        })
        .collect();
    assert_eq!(rows, vec![(3, 4), (3, 4), (2, 3)]);
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

fn large_resources(v: &mut Value) {
    super::hp_cost::configure(v);
    v["mechanism_version"] = json!(105);
    v["resource_limits"] = json!({"max_hp":8000000,"max_spirit":8000000,"max_spirit_cost":8000000});
    v["skill"]["cost"] = json!(8000000);
    v["skill"]["windup_ms"] = json!(60);
    v["skill"]["hp_cost"]["value"] = json!(8000000);
}
#[test]
fn resource_limits_explicit_profile_loads_eight_million_and_rejects_invalid_or_legacy_profiles() {
    assert!(delta_catalog(8, large_resources).is_ok());
    for (path, bad) in [
        ("/resource_limits", Value::Null),
        ("/resource_limits/max_hp", json!(0)),
        ("/resource_limits/max_spirit", json!(1000000001)),
        ("/resource_limits/max_spirit", json!(7999999)),
        ("/resource_limits/max_spirit_cost", json!(8000001)),
        ("/skill/cost", json!(8000001)),
        ("/mechanism_version", json!(104)),
    ] {
        assert!(
            delta_catalog(8, |v| {
                large_resources(v);
                *v.pointer_mut(path).unwrap() = bad;
            })
            .is_err(),
            "{path}"
        );
    }
    assert!(
        delta_catalog(8, |v| {
            large_resources(v);
            v.as_object_mut().unwrap().remove("resource_limits");
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            large_resources(v);
            v["additional_skills"][1]["cost"] = json!(8000001);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            large_resources(v);
            v["pve"]["combat"]["projectile"]["cost"] = json!(8000001);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            large_resources(v);
            v["skill"]["hp_cost"]["value"] = json!(8000001);
        })
        .is_err()
    );
}
#[test]
fn resource_limits_real_admission_cancellation_member_isolation_and_eight_million_commit() {
    let mut h = DeltaHarness::new(2, large_resources);
    h.stores[0].hp = 8_000_000;
    h.stores[0].spirit = 8_000_000;
    h.active();
    assert_eq!(
        (h.snapshot(0).own.max_hp, h.snapshot(0).own.max_spirit),
        (8_000_000, 8_000_000)
    );
    let skill = h.engine.config.skill.id.clone();
    let action = Action::Cast {
        skill_id: skill,
        direction: [1., 0.],
    };
    assert_eq!(h.action(1, action.clone()), Err(Error::NoResource));
    h.action(0, action.clone()).unwrap();
    h.action(0, Action::CancelAction {}).unwrap();
    h.ticks(2);
    assert_eq!(h.snapshot(0).own.spirit, 8_000_000);
    assert!(
        h.snapshot(0)
            .own
            .player_skills
            .iter()
            .all(|s| s.ready_at_ms == 0)
    );
    let input = h.input(0, action);
    let accepted = h.engine.input(input.clone()).unwrap();
    h.pump();
    h.ticks(2);
    assert_eq!(h.snapshot(0).own.spirit, 0);
    assert_eq!(h.snapshot(0).own.hp, 1);
    assert_eq!(h.snapshot(1).own.spirit, 200);
    assert_eq!(h.engine.input(input), Ok(accepted));
    h.pump();
    assert_eq!(
        h.events
            .iter()
            .filter(|(pid, e)| pid.as_deref() == Some("P00")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionCommitted {
                            spirit_cost: 8_000_000,
                            ..
                        }
                    }
                ))
            .count(),
        1
    );
    h.close();
    assert_eq!(h.engine.pending(), 0);
    assert!(
        h.stores
            .iter()
            .all(|s| s.admissions == 1 && s.settlements == 1)
    );
}
#[test]
fn resource_limits_one_less_than_cost_rejects_without_consuming_or_creating_an_action() {
    let mut h = DeltaHarness::new(2, large_resources);
    h.stores[0].spirit = 7_999_999;
    h.active();
    let hp = h.snapshot(0).own.hp;
    let id = h.engine.config.skill.id.clone();
    assert_eq!(
        h.action(
            0,
            Action::Cast {
                skill_id: id,
                direction: [1., 0.]
            }
        ),
        Err(Error::NoResource)
    );
    assert_eq!(
        (h.snapshot(0).own.hp, h.snapshot(0).own.spirit),
        (hp, 7_999_999)
    );
    assert!(h.snapshot(0).own.pending_action.is_none());
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn expanded_recovery_and_movement_validate_every_skill_kind_and_legacy_gates() {
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/skill-hp-cost-host/arena-b.json"
    ))
    .unwrap();
    let mut skills = vec![sample["skill"].clone()];
    skills.extend(sample["additional_skills"].as_array().unwrap().clone());
    skills.push(json!({"mechanism":"ally_heal","id":"friend-heal","cost":1,"windup_ms":30,"recovery_ms":300,"cooldown_ms":600,"range":500,"heal":{"amount":1}}));
    skills.push(json!({"mechanism":"ally_shield","id":"friend-shield","cost":1,"windup_ms":30,"recovery_ms":300,"cooldown_ms":600,"range":500,"shield_status":"ward"}));
    for mut skill in skills {
        skill["windup_ms"] = json!(30);
        skill["recovery_ms"] = json!(300);
        skill["recovery_cancel"] = json!({"after_ms":0,"by_move":true,"by_explicit_action":true});
        skill["cast_movement"] =
            json!({"preparation":{"kind":"locked"},"recovery":{"kind":"locked"}});
        assert!(
            delta_catalog(2, |v| {
                super::hp_cost::configure(v);
                v["mechanism_version"] = json!(105);
                v["skill"] = skill.clone();
                v["additional_skills"] = json!([]);
            })
            .is_ok(),
            "{}",
            skill["mechanism"]
        );
    }
    for value in [Value::Null, json!(1), json!("true")] {
        assert!(
            delta_catalog(2, |v| {
                recovery(v);
                v["skill"]["recovery_cancel"] = json!({"after_ms":0,"by_move":value});
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(2, |v| {
            preparation(v);
            v["mechanism_version"] = json!(104);
            v["skill"]["cast_movement"]["preparation"] = json!({"kind":"free"});
            v["skill"]["recovery_cancel"] = json!({"after_ms":0,"by_move":true});
        })
        .is_err()
    );
}
#[test]
fn source_charge_windup_full_and_delta_publish_quantized_commit_and_locked_release() {
    let mut h = DeltaHarness::new(2, |v| {
        queue(v);
        v["skill"]["charge"] = json!({"min_ms":350,"max_ms":1400,"min_damage_bps":7500,"early_release":{"kind":"queue_until_min"},"release_windup_ms":100});
    });
    h.active();
    let charge = super::charge::start(&mut h, 0);
    let initial = h.snapshot(0).own.spirit;
    assert_eq!(charge.min_release_at_ms - charge.started_at_ms, 360);
    assert_eq!(charge.auto_commit_at_ms - charge.started_at_ms, 1530);
    assert_eq!(charge.release_windup_ms, Some(120));
    h.action(
        0,
        Action::ReleaseCharge {
            action_id: charge.action_id.clone(),
            direction: None,
        },
    )
    .unwrap();
    h.ticks(15);
    assert_eq!(h.snapshot(0).own.spirit, initial);
    assert_eq!(
        h.snapshot(0)
            .own
            .pending_action
            .as_ref()
            .unwrap()
            .commits_at_ms
            - charge.started_at_ms,
        480
    );
    h.ticks(1);
    assert!(h.snapshot(0).own.pending_action.is_none());
    let commits: Vec<_> = h
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event:
                    Event::ActionCommitted {
                        charge: Some(c), ..
                    },
            } => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(commits.len(), 1);
    assert_eq!(
        (commits[0].held_ms, commits[0].damage_factor_bps),
        (350, 7500)
    );
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn mobile_fixed_windup_limit_is_explicit_and_does_not_replace_preparation_policies() {
    for (policy, windup, valid) in [
        (json!({"kind":"free"}), 200, true),
        (json!({"kind":"free"}), 201, false),
        (json!({"kind":"locked"}), 900, true),
        (json!({"kind":"slow","speed_bps":5000}), 900, true),
        (
            json!({"kind":"interrupt_on_new_move","min_turn_degrees":15,"min_destination_distance":4.}),
            900,
            true,
        ),
    ] {
        assert_eq!(
            delta_catalog(2, |v| {
                preparation(v);
                v["additional_skills"] = json!([]);
                v["cast_limits"] = json!({"mobile_fixed_windup_max_ms":200});
                v["skill"]["windup_ms"] = json!(windup);
                v["skill"]["cast_movement"]["preparation"] = policy;
            })
            .is_ok(),
            valid
        );
    }
    for bad in [
        Value::Null,
        json!({}),
        json!({"mobile_fixed_windup_max_ms":-1}),
        json!({"mobile_fixed_windup_max_ms":3600001}),
        json!({"mobile_fixed_windup_max_ms":200,"unknown":1}),
    ] {
        assert!(
            delta_catalog(2, |v| {
                preparation(v);
                v["cast_limits"] = bad;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(2, |v| {
            queue(v);
            v["cast_limits"] = json!({"mobile_fixed_windup_max_ms":200});
        })
        .is_ok()
    );
    assert!(
        delta_catalog(2, |v| {
            queue(v);
            v["mechanism_version"] = json!(104);
            v["skill"]["charge"]
                .as_object_mut()
                .unwrap()
                .remove("early_release");
            v["cast_limits"] = json!({"mobile_fixed_windup_max_ms":200});
        })
        .is_err()
    );
}

#[test]
fn resource_limits_high_realm_resources_survive_real_admission_and_wide_costs() {
    let mut h = DeltaHarness::new(2, |v| {
        large_resources(v);
        v["resource_limits"]["max_hp"] = json!(1000000000);
        v["resource_limits"]["max_spirit"] = json!(1000000000);
        v["pve"]["stats"]["hp"] = json!(120000000);
        v["pve"]["stats"]["spirit"] = json!(120000000);
    });
    h.stores[0].hp = 1_000_000_000;
    h.stores[0].spirit = 1_000_000_000;
    h.active();
    assert_eq!(
        (h.snapshot(0).own.max_hp, h.snapshot(0).own.max_spirit),
        (1_000_000_000, 1_000_000_000)
    );
    assert!(
        h.snapshot(0)
            .monsters
            .iter()
            .any(|n| n.max_hp == 120_000_000)
    );
    let id = h.engine.config.skill.id.clone();
    h.action(
        0,
        Action::Cast {
            skill_id: id,
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(2);
    assert_eq!(
        (h.snapshot(0).own.hp, h.snapshot(0).own.spirit),
        (992_000_000, 992_000_000)
    );
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn channel_recovery_remainder_loader_accepts_only_named_string_policies() {
    for name in ["earliest", "final"] {
        assert!(
            delta_catalog(2, |v| {
                recovery(v);
                v["skill"]["channel"]["recovery"]["remainder_policy"] = json!(name);
            })
            .is_ok()
        );
    }
    for value in [
        Value::Null,
        json!("other"),
        json!({"final":null}),
        json!(false),
    ] {
        assert!(
            delta_catalog(2, |v| {
                recovery(v);
                v["skill"]["channel"]["recovery"]["remainder_policy"] = value;
            })
            .is_err()
        );
    }
}
