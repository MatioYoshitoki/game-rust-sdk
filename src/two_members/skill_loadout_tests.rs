use super::*;
fn configure(v: &mut Value) {
    super::hp_cost::configure(v);
    v["mechanism_version"] = json!(105);
    v["skill"]["id"] = json!("old");
    let mut library = v["skill"].clone();
    library["id"] = json!("learned");
    v["skill_library"] = json!([library]);
}
fn plan(
    h: &DeltaHarness,
    i: usize,
    op: &str,
    revision: u32,
    owned: &[&str],
    equipped: &[&str],
) -> SetSkillLoadout {
    SetSkillLoadout {
        key: h.engine.create.key.clone(),
        participant_id: format!("P{i:02}"),
        entity_generation: 1,
        operation_id: op.into(),
        expected_revision: revision,
        loadout: SkillLoadout {
            owned_skill_ids: owned.iter().map(|s| s.to_string()).collect(),
            equipped_skill_ids: equipped.iter().map(|s| s.to_string()).collect(),
        },
    }
}
fn apply(h: &mut DeltaHarness, q: SetSkillLoadout) -> SkillLoadoutReceipt {
    let receipt = h.engine.set_skill_loadout(q).unwrap();
    h.ticks(1);
    receipt
}
#[test]
fn skill_library_is_versioned_unique_bounded_and_does_not_add_default_slots() {
    assert!(delta_catalog(8, configure).is_ok());
    for case in 0..8 {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                match case {
                    0 => v["mechanism_version"] = json!(104),
                    1 => v["skill_library"] = Value::Null,
                    2 => v["skill_library"] = json!([]),
                    3 => v["skill_library"][0]["id"] = json!("old"),
                    4 => v["skill_library"][0]["id"] = json!(""),
                    5 => {
                        v["skill_library"] =
                            json!([v["skill_library"][0].clone(), v["skill_library"][0].clone()])
                    }
                    6 => v["skill_library"][0]["hp_cost"] = Value::Null,
                    _ => v["skill_library"][0]["other"] = json!(1),
                }
            })
            .is_err(),
            "{case}"
        );
    }
    let mut h = DeltaHarness::new(4, configure);
    h.active();
    assert_eq!(h.snapshot(0).own.player_skills.len(), 8);
    assert_eq!(h.snapshot(0).own.player_skills[0].skill_id, "old");
    assert_eq!(h.snapshot(0).own.skill_loadout_revision, None);
}
#[test]
fn trusted_member_loadouts_separate_ownership_equipping_and_all_cast_entry_qualification() {
    for n in [2, 4, 8] {
        let mut h = DeltaHarness::new(n, configure);
        h.active();
        let q = plan(&h, 0, "grant", 0, &["old", "learned"], &["learned"]);
        apply(&mut h, q);
        assert_eq!(h.snapshot(0).own.player_skills[0].skill_id, "learned");
        assert_eq!(h.snapshot(0).own.skill_loadout_revision, Some(1));
        assert_eq!(h.snapshot(1).own.player_skills[0].skill_id, "old");
        let before = h.snapshot(0).own.clone();
        let target = UnitTarget {
            id: h.snapshot(0).monsters[0].entity.id.clone(),
            generation: 1,
        };
        for action in [
            Action::Cast {
                skill_id: "old".into(),
                direction: [1., 0.],
            },
            Action::StartCharge {
                skill_id: "old".into(),
                direction: [1., 0.],
            },
            Action::CastSelf {
                skill_id: "old".into(),
            },
            Action::CastUnit {
                skill_id: "old".into(),
                target: target.clone(),
            },
            Action::CastGround {
                skill_id: "old".into(),
                position: [460., 400.],
            },
        ] {
            let i = h.input(0, action);
            let accepted = h.engine.input(i);
            assert!(accepted.is_ok() || matches!(accepted, Err(Error::InvalidTarget)));
            h.ticks(1);
            assert_eq!(h.snapshot(0).own.spirit, before.spirit);
            assert!(h.snapshot(0).own.pending_action.is_none());
        }
        let q = plan(&h, 0, "empty", 1, &["old", "learned"], &[]);
        apply(&mut h, q);
        assert!(h.snapshot(0).own.player_skills.is_empty());
    }
}
#[test]
fn trusted_loadout_cas_duplicate_conflict_and_bad_grants_are_atomic() {
    let mut h = DeltaHarness::new(4, configure);
    h.active();
    let q = plan(&h, 0, "once", 0, &["old", "learned"], &["learned"]);
    let receipt = apply(&mut h, q.clone());
    let count = h.events.len();
    assert_eq!(h.engine.set_skill_loadout(q.clone()), Ok(receipt));
    h.pump();
    assert_eq!(h.events.len(), count);
    for case in 0..8 {
        let mut bad = plan(&h, 0, "bad", 1, &["old", "learned"], &["old"]);
        let expected = match case {
            0 => {
                bad.operation_id = "once".into();
                Error::Conflict
            }
            1 => {
                bad.expected_revision = 0;
                Error::RevisionConflict
            }
            2 => {
                bad.loadout.equipped_skill_ids.push("old".into());
                Error::InvalidMessage
            }
            3 => {
                bad.loadout.owned_skill_ids = vec!["learned".into()];
                Error::InvalidMessage
            }
            4 => {
                bad.loadout.owned_skill_ids.push("unknown".into());
                Error::InvalidMessage
            }
            5 => {
                bad.entity_generation = 2;
                Error::InvalidMessage
            }
            6 => {
                bad.operation_id = "".into();
                Error::InvalidMessage
            }
            _ => {
                bad.key.incarnation += 1;
                Error::StaleMatch
            }
        };
        let before = h.engine.skill_control.clone();
        assert_eq!(h.engine.set_skill_loadout(bad), Err(expected));
        assert_eq!(h.engine.skill_control, before);
    }
}
#[test]
fn removal_cancels_uncommitted_windup_without_cost_and_regrant_keeps_real_cooldown_debt() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let before = h.snapshot(0).own.spirit;
    h.action(
        0,
        Action::Cast {
            skill_id: "old".into(),
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(1);
    assert!(h.snapshot(0).own.pending_action.is_some());
    let q = plan(&h, 0, "remove-prep", 0, &["old", "learned"], &["learned"]);
    apply(&mut h, q);
    assert!(h.snapshot(0).own.pending_action.is_none());
    assert_eq!(h.snapshot(0).own.spirit, before);
    let q = plan(&h, 0, "return", 1, &["old", "learned"], &["old"]);
    apply(&mut h, q);
    h.action(
        0,
        Action::Cast {
            skill_id: "old".into(),
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(12);
    let ready = h.snapshot(0).own.player_skills[0].ready_at_ms;
    assert!(ready > h.engine.now);
    let paid = h.snapshot(0).own.spirit;
    let flights = h.snapshot(0).projectiles.clone();
    let q = plan(&h, 0, "forget", 2, &[], &[]);
    apply(&mut h, q);
    assert!(h.snapshot(0).own.player_skills.is_empty());
    let q = plan(&h, 0, "relearn", 3, &["old"], &["old"]);
    apply(&mut h, q);
    assert_eq!(h.snapshot(0).own.player_skills[0].ready_at_ms, ready);
    assert_eq!(h.snapshot(0).own.spirit, paid);
    assert!(flights.iter().all(|f|h.snapshot(0).projectiles.iter().any(|a|a.id==f.id)||h.events.iter().any(|(_,e)|matches!(e,EventV4::ProjectileEnded{projectile_id,..}if projectile_id==&f.id))));
}
#[test]
fn trusted_loadout_control_is_replay_checked_in_financial_only_process_recovery() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let q = plan(&h, 0, "audit", 0, &["old", "learned"], &["learned"]);
    let receipt = apply(&mut h, q.clone());
    let checkpoint = h.engine.checkpoint();
    assert_eq!(
        checkpoint
            .state
            .skill_control
            .as_ref()
            .unwrap()
            .operations
            .len(),
        1
    );
    let cat = delta_catalog(2, configure).unwrap();
    let c = cat.default_config().as_ref().clone();
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: checkpoint.clone(),
    });
    let mut restored = TwoMembers::create(c.clone(), cat.clone(), r.clone()).unwrap();
    assert_eq!(restored.phase, Phase::Closed);
    assert!(restored.world.is_none());
    assert_eq!(restored.set_skill_loadout(q), Ok(receipt));
    let q = plan(&h, 0, "new-after-close", 1, &["old"], &["old"]);
    assert_eq!(restored.set_skill_loadout(q), Err(Error::Closed));
    for case in 0..3 {
        let mut bad = r.clone();
        let cp = &mut bad.recovery.as_mut().unwrap().checkpoint;
        let audit = cp.state.skill_control.as_mut().unwrap();
        match case {
            0 => audit.operations[0].receipt.revision = 9,
            1 => audit.members.get_mut("P00").unwrap().revision = 9,
            _ => audit.operations[0]
                .request
                .loadout
                .owned_skill_ids
                .push("unknown".into()),
        };
        cp.sha256 = state_digest(&cp.state);
        assert!(TwoMembers::create(c.clone(), cat.clone(), bad).is_err());
    }
}
#[test]
fn initial_member_grants_are_catalog_bound_and_repeat_create_compares_ownership() {
    let cat = delta_catalog(4, configure).unwrap();
    let c = cat.default_config().as_ref().clone();
    let mut r = delta_request(&c);
    r.participants[0].skills = Some(SkillLoadout {
        owned_skill_ids: vec!["old".into(), "learned".into()],
        equipped_skill_ids: vec!["learned".into()],
    });
    r.participants[1].skills = Some(SkillLoadout {
        owned_skill_ids: vec![],
        equipped_skill_ids: vec![],
    });
    let engine = TwoMembers::create(c.clone(), cat.clone(), r.clone()).unwrap();
    assert_eq!(
        engine.skill_control.members["P00"]
            .loadout
            .equipped_skill_ids,
        vec!["learned"]
    );
    assert!(engine.repeat_create(r.clone()).is_ok());
    let mut different = r.clone();
    different.participants[0]
        .skills
        .as_mut()
        .unwrap()
        .equipped_skill_ids = vec!["old".into()];
    assert_eq!(engine.repeat_create(different), Err(Error::Conflict));
    for case in 0..4 {
        let mut bad = r.clone();
        let s = bad.participants[0].skills.as_mut().unwrap();
        match case {
            0 => s.owned_skill_ids = vec!["old".into()],
            1 => s.equipped_skill_ids.push("learned".into()),
            2 => s.owned_skill_ids.push("unknown".into()),
            _ => s.owned_skill_ids.push("old".into()),
        }
        assert!(TwoMembers::create(c.clone(), cat.clone(), bad).is_err());
    }
}
#[test]
fn trusted_loadout_output_backpressure_is_atomic_and_history_never_evicts_receipts() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let first = plan(&h, 0, "op-0", 0, &["old", "learned"], &["learned"]);
    let before = h.engine.skill_control.clone();
    let cap = h.engine.config.output_capacity;
    h.engine.config.output_capacity = 1;
    assert_eq!(
        h.engine.set_skill_loadout(first.clone()),
        Err(Error::QueueFull)
    );
    assert_eq!(h.engine.skill_control, before);
    h.engine.config.output_capacity = cap;
    let original = h.engine.set_skill_loadout(first.clone()).unwrap();
    h.pump();
    for revision in 1..128 {
        let q = plan(
            &h,
            0,
            &format!("op-{revision}"),
            revision,
            &["old", "learned"],
            &[if revision % 2 == 0 { "old" } else { "learned" }],
        );
        h.engine.set_skill_loadout(q).unwrap();
        h.pump();
    }
    let before = h.engine.skill_control.clone();
    let q = plan(&h, 0, "one-too-many", 128, &["old"], &["old"]);
    assert_eq!(h.engine.set_skill_loadout(q), Err(Error::Capacity));
    assert_eq!(h.engine.skill_control, before);
    assert_eq!(h.engine.set_skill_loadout(first), Ok(original));
    assert_eq!(h.engine.skill_control.operations.len(), 128);
    assert_eq!(h.engine.skill_control.members["P00"].revision, 128);
}
#[test]
fn revocation_in_the_same_logical_tick_cancels_accepted_preparation_without_paying() {
    let mut h = DeltaHarness::new(4, configure);
    h.active();
    let before = h.snapshot(0).own.spirit;
    h.action(
        0,
        Action::Cast {
            skill_id: "old".into(),
            direction: [1., 0.],
        },
    )
    .unwrap();
    let q = plan(&h, 0, "queued-revoke", 0, &["learned"], &["learned"]);
    h.engine.set_skill_loadout(q).unwrap();
    h.ticks(15);
    assert_eq!(h.snapshot(0).own.spirit, before);
    assert!(h.snapshot(0).own.pending_action.is_none());
    assert!(!h.events.iter().any(|(p,e)|p.as_deref()==Some("P00")&&matches!(e,EventV4::Private{event:Event::ActionCommitted{skill_id,..}}if skill_id.as_deref()==Some("old"))));
}
#[test]
fn revoking_an_active_channel_keeps_its_committed_flight_cost_and_recovery_debt() {
    let mut h = DeltaHarness::new(4, |v| {
        configure(v);
        v["skill"] = v["additional_skills"][1].clone();
        v["skill"]["id"] = json!("old");
        v["skill"]["windup_ms"] = json!(30);
        v["skill"]["recovery_ms"] = json!(300);
        v["skill"]["channel"] =
            json!({"duration_ms":600,"interval_ms":200,"pulse_count":3,"spirit_per_pulse":1});
        v["skill"]["recovery_cancel"] =
            json!({"after_ms":0,"by_move":true,"by_explicit_action":true});
    });
    h.active();
    h.action(
        0,
        Action::Cast {
            skill_id: "old".into(),
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(9);
    let channel = h.snapshot(0).own.active_channel.as_ref().unwrap();
    assert_eq!(channel.pulses, 1);
    let action = channel.action_id.clone();
    let paid = h.snapshot(0).own.spirit;
    let flights = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(0, &h.engine.config, Phase::Active, h.engine.now)
        .projectiles;
    assert!(!flights.is_empty());
    let q = plan(&h, 0, "remove-channel", 0, &[], &[]);
    h.engine.set_skill_loadout(q).unwrap();
    assert_eq!(
        h.engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(0, &h.engine.config, Phase::Active, h.engine.now)
            .projectiles,
        flights
    );
    h.ticks(1);
    assert!(h.snapshot(0).own.active_channel.is_none());
    assert!(h.snapshot(0).own.recovery_until_ms > h.engine.now);
    assert!(h.snapshot(0).own.recovery_cancel.is_none());
    assert!(h.snapshot(0).own.followup.is_none());
    assert_eq!(h.snapshot(0).own.spirit, paid);
    h.ticks(15);
    let pulses=h.events.iter().filter(|(p,e)|p.as_deref()==Some("P00")&&matches!(e,EventV4::Private{event:Event::ChannelPulse{action_id,..}}if action_id==&action)).count();
    assert_eq!(pulses, 1);
}
#[test]
fn qualification_rejects_correctly_typed_inputs_for_every_unowned_configured_cast_kind() {
    let mut h = DeltaHarness::new(4, |v| {
        configure(v);
        let mut charged = v["additional_skills"][1].clone();
        charged["id"] = json!("charged");
        charged["windup_ms"] = json!(0);
        charged["charge"] = json!({"min_ms":90,"max_ms":450,"min_damage_bps":4000});
        v["skill_library"].as_array_mut().unwrap().push(charged);
    });
    h.active();
    let q = plan(&h, 0, "only-one", 0, &["learned"], &["learned"]);
    apply(&mut h, q);
    let target = UnitTarget {
        id: h.snapshot(0).monsters[0].entity.id.clone(),
        generation: 1,
    };
    let defs: Vec<_> = h
        .engine
        .config
        .player_skills()
        .filter(|s| s.id != "learned")
        .cloned()
        .collect();
    let before = (
        h.engine.world.as_ref().unwrap().members[0].hp,
        h.engine.world.as_ref().unwrap().members[0].spirit,
    );
    let mut kinds = std::collections::BTreeSet::new();
    for skill in defs {
        let action = if skill.charge.is_some() {
            kinds.insert("charge");
            Action::StartCharge {
                skill_id: skill.id,
                direction: [1., 0.],
            }
        } else if skill.is_self() {
            kinds.insert("self");
            Action::CastSelf { skill_id: skill.id }
        } else if skill.is_unit() {
            kinds.insert("unit");
            Action::CastUnit {
                skill_id: skill.id,
                target: target.clone(),
            }
        } else if skill.is_ground() {
            kinds.insert("ground");
            Action::CastGround {
                skill_id: skill.id,
                position: [460., 400.],
            }
        } else {
            kinds.insert("direction");
            Action::Cast {
                skill_id: skill.id,
                direction: [1., 0.],
            }
        };
        let input = h.input(0, action);
        assert_eq!(h.engine.input(input), Err(Error::InvalidTarget));
        assert_eq!(
            (
                h.engine.world.as_ref().unwrap().members[0].hp,
                h.engine.world.as_ref().unwrap().members[0].spirit
            ),
            before
        );
    }
    assert_eq!(
        kinds,
        std::collections::BTreeSet::from(["charge", "self", "unit", "ground", "direction"])
    );
}
#[test]
fn revoked_recast_cannot_be_restored_by_regranting_the_skill_while_its_flight_survives() {
    let mut h = DeltaHarness::new(4, |v| {
        configure(v);
        v["skill"] = v["additional_skills"][1].clone();
        v["skill"]["id"] = json!("old");
        v["skill"]["windup_ms"] = json!(30);
        v["skill"]["speed"] = json!(60);
        v["skill"]["recast"] = json!({"window_ms":600,"spirit_cost":7});
    });
    h.active();
    h.action(
        0,
        Action::Cast {
            skill_id: "old".into(),
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(2);
    let recast = h.snapshot(0).own.projectile_recast.clone().unwrap();
    let before = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(0, &h.engine.config, Phase::Active, h.engine.now)
        .projectiles;
    let q = plan(&h, 0, "remove-recast", 0, &["learned"], &["learned"]);
    h.engine.set_skill_loadout(q).unwrap();
    assert_eq!(
        h.engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(0, &h.engine.config, Phase::Active, h.engine.now)
            .projectiles,
        before
    );
    h.ticks(1);
    assert!(h.snapshot(0).own.projectile_recast.is_none());
    let q = plan(&h, 0, "return-recast", 1, &["old", "learned"], &["old"]);
    apply(&mut h, q);
    assert!(h.snapshot(0).own.projectile_recast.is_none());
    let paid = h.engine.world.as_ref().unwrap().members[0].spirit;
    let i = h.input(
        0,
        Action::RecastProjectile {
            action_id: recast.action_id,
            recast_index: recast.next_index,
            direction: [0., 1.],
        },
    );
    assert_eq!(h.engine.input(i), Err(Error::InvalidTarget));
    assert_eq!(h.engine.world.as_ref().unwrap().members[0].spirit, paid);
}
#[test]
fn trusted_changes_reject_dead_or_exiting_members_and_keep_prior_exact_receipts() {
    let mut h = DeltaHarness::new(4, configure);
    h.active();
    let q = plan(&h, 0, "before-exit", 0, &["old"], &["old"]);
    let receipt = apply(&mut h, q.clone());
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    let next = plan(&h, 0, "after-exit", 1, &["learned"], &["learned"]);
    assert_eq!(h.engine.set_skill_loadout(next), Err(Error::InvalidState));
    assert_eq!(h.engine.set_skill_loadout(q), Ok(receipt));
    h.engine.world.as_mut().unwrap().members[1].hp = 0;
    let next = plan(&h, 1, "dead", 0, &["old"], &["old"]);
    assert_eq!(h.engine.set_skill_loadout(next), Err(Error::InvalidState));
    assert_eq!(h.engine.skill_control.members["P01"].revision, 0);
}
#[test]
fn rust_replica_loadout_versions_fence_slot_changes_and_preserve_baselines_on_invalid_delta() {
    let mut h = DeltaHarness::new(4, configure);
    h.active();
    let q = plan(&h, 0, "start", 0, &["old", "learned"], &["old"]);
    apply(&mut h, q);
    let before =
        h.engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(0, &h.engine.config, Phase::Active, h.engine.now);
    let f = PublicationEnvelopeV6 {
        key: h.engine.create.key.clone(),
        publication_id: "loadout-1".into(),
        recipient_id: "P00".into(),
        connection: Connection {
            id: "test".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            snapshot: Box::new(before.clone()),
            reason: FullReason::Initial,
        },
    };
    let mut replica = ReplicaV6::new(f.key.clone(), "P00".into());
    replica.apply(&f).unwrap();
    for case in 0..3 {
        let mut next = before.clone();
        match case {
            0 => next.own.skill_loadout_revision = Some(0),
            1 => next.own.skill_loadout_revision = None,
            _ => next.own.player_skills.clear(),
        };
        let d = PublicationEnvelopeV6 {
            publication_id: "loadout-2".into(),
            revision: 2,
            payload: PublicationPayloadV6::Delta {
                base_publication_id: "loadout-1".into(),
                base_revision: 1,
                delta: Box::new(crate::replication::diff(&before, &next)),
            },
            ..f.clone()
        };
        assert_eq!(replica.apply(&d), Err(DecodeError::InvalidFrame));
        assert_eq!(replica.snapshot(), Some(&before));
    }
    let mut empty = before.clone();
    empty.own.skill_loadout_revision = Some(2);
    empty.own.player_skills.clear();
    let d = PublicationEnvelopeV6 {
        publication_id: "loadout-2".into(),
        revision: 2,
        payload: PublicationPayloadV6::Delta {
            base_publication_id: "loadout-1".into(),
            base_revision: 1,
            delta: Box::new(crate::replication::diff(&before, &empty)),
        },
        ..f
    };
    assert_eq!(replica.apply(&d).unwrap(), &empty);
}
