use super::*;

fn impact_contacts(h: &DeltaHarness) -> Vec<(&str, &str, u64)> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::BasicImpactContact {
                source,
                target,
                scheduled_at_ms,
                ..
            } => Some((source.id.as_str(), target.id.as_str(), *scheduled_at_ms)),
            _ => None,
        })
        .collect()
}

fn basic_heals(h: &DeltaHarness) -> Vec<(u32, u32, u32)> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::Private {
                event:
                    Event::HealResolved {
                        heal_kind: HealKind::BasicAttack,
                        raw_heal,
                        multiplier_bps,
                        hp_restored,
                        ..
                    },
            } => Some((*raw_heal, *multiplier_bps, *hp_restored)),
            _ => None,
        })
        .collect()
}

fn projectile_wall(v: &mut Value, column: usize, row: usize) {
    let mut wall = v["pve"]["palette"][0].clone();
    wall["blocksProjectiles"] = json!(true);
    v["pve"]["palette"].as_array_mut().unwrap().push(wall);
    let cell = row * 32 + column;
    v["pve"]["cells_rle"] = json!([[0, cell], [1, 1], [0, 1024 - cell - 1]]);
}

#[test]
fn basic_impact_mirror_and_cauldron_in_one_world_keep_distinct_clocks_and_rewards() {
    let mut h = profile_host([Some("mirror"), Some("cauldron")], |v| {
        v["pve"]["monster_positions"] = json!([[700, 400], [790, 400], [700, 490], [440, 480]]);
    });
    h.engine.world.as_mut().unwrap().members[1].hp = 970;
    let ids = h
        .snapshot(0)
        .monsters
        .iter()
        .map(|n| n.entity.id.clone())
        .collect::<Vec<_>>();
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.action(1, attack(&h, 1, 3)).unwrap();
    h.ticks(3);
    assert!(impact_contacts(&h).is_empty());
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 0);
    h.ticks(1);
    assert_eq!(
        impact_contacts(&h),
        [("A", ids[0].as_str(), 120), ("A", ids[1].as_str(), 120)]
    );
    assert_eq!(
        h.snapshot(0)
            .monsters
            .iter()
            .map(|n| n.hp)
            .collect::<Vec<_>>(),
        [9910, 9935, 10000, 10000]
    );
    assert_eq!(h.snapshot(0).own.spirit, 102);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1020);
    assert_eq!(h.snapshot(0).basic_impacts.as_deref(), Some([].as_slice()));
    assert!(h.snapshot(0).projectiles.is_empty());

    h.ticks(2);
    let plan = h.snapshot(0).basic_impacts.as_ref().unwrap()[0].clone();
    assert_eq!(plan.kind, BasicImpactKind::FixedDelay);
    assert_eq!(plan.plan_id, "cauldron-impact");
    assert_eq!((plan.released_at_ms, plan.impact_at_ms), (180, 530));
    assert_eq!(
        h.snapshot(1).basic_impacts.as_deref(),
        Some(std::slice::from_ref(&plan))
    );
    assert_eq!(h.snapshot(1).own.basic_ready_at_ms, 530);
    h.ticks(11);
    assert_eq!(h.snapshot(1).own.hp, 970);
    assert_eq!(h.snapshot(1).own.spirit, 100);
    assert!(basic_heals(&h).is_empty());

    h.ticks(1);
    assert_eq!(
        impact_contacts(&h).last(),
        Some(&("B", ids[3].as_str(), 530))
    );
    assert_eq!(h.snapshot(1).monsters[3].hp, 9922);
    assert_eq!(h.snapshot(1).own.hp, 995);
    assert_eq!(h.snapshot(1).own.spirit, 102);
    assert_eq!(h.snapshot(1).own.basic_ready_at_ms, 1440);
    assert_eq!(basic_heals(&h), [(25, 10000, 25)]);
    assert_eq!(h.snapshot(0).basic_impacts.as_deref(), Some([].as_slice()));
    assert!(h.frames.iter().any(|frame| matches!(&frame.payload,
        PublicationPayloadV6::Delta { delta, .. } if delta.basic_impacts.as_ref().is_some_and(Vec::is_empty)
    )));
    assert!(h.snapshot(0).projectiles.is_empty());
    h.close();
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}

#[test]
fn basic_impact_refraction_filters_projectile_walls_and_does_not_fill_missing_targets() {
    for available in [false, true] {
        let mut h = profile_host([Some("mirror"), None], |v| {
            v["pve"]["monster_positions"] = if available {
                json!([[700, 400], [790, 400], [700, 500]])
            } else {
                json!([[700, 400], [790, 400], [700, 630]])
            };
            projectile_wall(v, 19, 10);
        });
        h.action(0, attack(&h, 0, 0)).unwrap();
        h.ticks(4);
        let ids = h
            .snapshot(0)
            .monsters
            .iter()
            .map(|n| n.entity.id.as_str())
            .collect::<Vec<_>>();
        let expected = if available {
            vec![("A", ids[0], 120), ("A", ids[2], 120)]
        } else {
            vec![("A", ids[0], 120)]
        };
        assert_eq!(impact_contacts(&h), expected);
        assert_eq!(h.snapshot(0).monsters[1].hp, 10000);
    }
}

#[test]
fn basic_impact_dodged_primary_prevents_refraction_and_recovery() {
    let mut h = profile_host([Some("mirror"), None], |v| {
        v["pve"]["monster_positions"] = json!([[490, 400], [520, 400]]);
        v["combat_rules"]["basic_attack"]["unavoidable"] = json!(false);
        v["combat_rules"]["dodge_cap_bps"] = json!(10000);
        v["combat_rules"]["profiles"]["member"]["hit_bps"] = json!(0);
        v["combat_rules"]["profiles"]["npc"]["dodge_bps"] = json!(10000);
    });
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.ticks(4);
    assert_eq!(impact_contacts(&h).len(), 1);
    assert!(h.snapshot(0).monsters.iter().all(|m| m.hp == 10000));
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1020);
}

#[test]
fn basic_impact_fixed_delay_survives_distance_change_and_uses_live_healing_received() {
    for initial in [450, 650] {
        let mut h = profile_host([Some("cauldron"), None], |v| {
            v["pve"]["monster_positions"] = json!([[initial, 400]]);
            v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({
                "id":"heal-probe","mechanism":"timed_attribute_modifier","attribute":"healing_received",
                "modifier_bps":5000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true
            }));
            v["combat_rules"]["basic_profiles"]["cauldron"]["effects"]["on_release_self_status"] =
                json!("heal-probe");
        });
        h.engine.world.as_mut().unwrap().members[0].hp = 980;
        h.action(0, Action::Attack {}).unwrap();
        h.ticks(6);
        assert_eq!(
            h.snapshot(0).basic_impacts.as_ref().unwrap()[0].impact_at_ms,
            530
        );
        h.engine
            .world
            .as_mut()
            .unwrap()
            .place_wave_test_npc(0, [f64::from(initial) + 25., 400.]);
        h.ticks(12);
        assert_eq!(h.snapshot(0).monsters[0].hp, 9922);
        assert_eq!(h.snapshot(0).own.hp, 1000);
        assert_eq!(basic_heals(&h), [(25, 15000, 20)]);
    }
}

#[test]
fn basic_impact_fixed_delay_skips_stale_or_occluded_targets_and_still_settles_cooldown() {
    for stale in [false, true] {
        let mut h = profile_host([Some("cauldron"), None], |v| {
            v["pve"]["monster_positions"] = json!([[510, 400]]);
            projectile_wall(v, 13, 10);
        });
        h.engine.world.as_mut().unwrap().members[0].hp = 900;
        h.action(0, Action::Attack {}).unwrap();
        h.ticks(6);
        if stale {
            h.engine.world.as_mut().unwrap().replace_wave_test_npc(0);
        } else {
            h.engine
                .world
                .as_mut()
                .unwrap()
                .place_wave_test_npc(0, [525., 400.]);
        }
        h.ticks(12);
        assert!(impact_contacts(&h).is_empty());
        assert!(basic_heals(&h).is_empty());
        assert!(h.events.iter().any(|(_, event)| matches!(
            event,
            EventV4::BasicImpactEnded {
                reason: BasicImpactEndReason::TargetInvalid,
                ..
            }
        )));
        assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1440);
        assert_eq!(h.snapshot(0).own.spirit, 100);
        assert_eq!(h.snapshot(0).monsters[0].hp, 10000);
    }
}

#[test]
fn basic_impact_reconnect_rebuilds_remaining_plan_and_cancels_heal_eligibility() {
    let mut h = profile_host([Some("cauldron"), None], |_| {});
    h.engine.world.as_mut().unwrap().members[0].hp = 900;
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(7);
    let pending = h.snapshot(0).basic_impacts.clone();
    let previous_connection = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut input = h.input(
        0,
        Action::Reconnect {
            previous_connection,
            authenticated_principal: "principal-A".into(),
        },
    );
    input.connection = Connection {
        id: "impact-reconnect".into(),
        generation: 2,
    };
    input.sequence = 1;
    h.engine.input(input).unwrap();
    h.sequences[0] = 1;
    h.pump();
    assert!(matches!(
        h.latest(0).payload,
        PublicationPayloadV6::Full { .. }
    ));
    assert_eq!(h.snapshot(0).basic_impacts, pending);
    h.ticks(11);
    assert_eq!(impact_contacts(&h).len(), 1);
    assert_eq!(h.snapshot(0).monsters[0].hp, 9922);
    assert_eq!(h.snapshot(0).own.hp, 900);
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1440);
    assert!(basic_heals(&h).is_empty());
}

#[test]
fn basic_impact_source_exit_clears_plan_and_replica_rejects_expired_or_immediate_entries() {
    let mut h = profile_host([Some("cauldron"), None], |_| {});
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(6);
    let snapshot = h.snapshot(1).clone();
    let mut frame = h.latest(1);
    frame.payload = PublicationPayloadV6::Full {
        snapshot: Box::new(snapshot.clone()),
        reason: FullReason::Initial,
    };
    let mut replica = ReplicaV6::new(frame.key.clone(), frame.recipient_id.clone());
    replica.apply(&frame).unwrap();
    frame.revision += 1;
    frame.publication_id.push_str("-bad");
    for immediate in [false, true] {
        let PublicationPayloadV6::Full {
            snapshot: invalid, ..
        } = &mut frame.payload
        else {
            unreachable!()
        };
        **invalid = snapshot.clone();
        let plan = &mut invalid.basic_impacts.as_mut().unwrap()[0];
        if immediate {
            plan.kind = BasicImpactKind::Hitscan;
        } else {
            plan.impact_at_ms = snapshot.logical_ms;
        }
        assert!(matches!(
            replica.apply(&frame),
            Err(DecodeError::InvalidFrame)
        ));
        assert_eq!(replica.snapshot(), Some(&snapshot));
    }

    h.action(0, Action::Leave {}).unwrap();
    h.ticks(12);
    assert!(h.snapshot(1).basic_impacts.as_ref().unwrap().is_empty());
    assert!(impact_contacts(&h).is_empty());
    assert!(basic_heals(&h).is_empty());
    assert!(h.events.iter().any(|(_, event)| matches!(
        event,
        EventV4::BasicImpactEnded {
            reason: BasicImpactEndReason::SourceInvalid,
            ..
        }
    )));
}

#[test]
fn basic_impact_config_rejects_conflicts_and_unbounded_schedules() {
    for change in [
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["cauldron"]["impact"]["timing"]["delay_ms"] =
                json!(0)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["cauldron"]["impact"]["timing"]["delay_ms"] =
                json!(60001)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["cauldron"]["effects"]["on_end_damage_self_heal_max_hp_bps"] =
                json!(10001)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["cauldron"]["piercing"] =
                json!({"plan_id":"conflict","max_hits":2})
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["mirror"]["impact"]["timing"]["refraction"] =
                Value::Null
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["mirror"]["impact"]["timing"]["refraction"]["radius"] =
                json!(0)
        },
    ] {
        assert!(profile_catalog(change).is_err());
    }
}

#[path = "basic_hitscan_pattern_tests.rs"]
mod basic_hitscan_patterns;
