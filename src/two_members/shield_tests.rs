use super::*;
pub(super) fn configure(v: &mut Value) {
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/multiplayer-shield-host/arena-a.json"
    ))
    .unwrap();
    v["mechanism_version"] = json!(39);
    v["pve"]["status_definitions"] = sample["pve"]["status_definitions"].clone();
    v["pve"]["status_rules"] = sample["pve"]["status_rules"].clone();
    v["pve"]["status_rules"]["max_active_per_entity"] = json!(10);
    v["skill"]["on_hit_target_statuses"] = json!(["wound", "barrier"]);
    v["additional_skills"][0]["on_commit_self_statuses"] = json!(["renewal", "ward"]);
    v["pve"]["stats"]["hp"] = json!(2000);
    v["pve"]["combat"]["cast_chance"] = json!(1);
    v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!(["wound", "barrier"]);
    v["pve"]["combat"]["projectile"]["on_commit_self_statuses"] = json!(["renewal", "ward"]);
    v["pve"]["combat"]["projectile"]["cooldown_ms"] = json!(60000);
    v["params"]["regen"]["near_ratio"] = json!(0);
    v["params"]["regen"]["far_ratio"] = json!(0);
}
fn harness(n: usize) -> DeltaHarness {
    DeltaHarness::new(n, configure)
}
fn cast(h: &mut DeltaHarness, i: usize) {
    let id = h.engine.config.skill.id.clone();
    let target = h.snapshot(i).monsters[0].position;
    let p = h.snapshot(i).own.position;
    h.action(
        i,
        Action::Cast {
            skill_id: id,
            direction: [target[0] - p[0], target[1] - p[1]],
        },
    )
    .unwrap();
}
fn renew(h: &mut DeltaHarness, i: usize) {
    let id = h.engine.config.additional_skills[0].id.clone();
    h.action(i, Action::CastSelf { skill_id: id }).unwrap();
}
#[test]
fn mp_shield_configuration_rejects_wrong_version_bindings_budgets_before_io() {
    for change in [
        |v: &mut Value| v["mechanism_version"] = json!(38),
        |v: &mut Value| v["pve"]["status_rules"]["max_active_per_entity"] = json!(32),
        |v: &mut Value| v["skill"]["on_commit_self_statuses"] = json!(["wound", "barrier"]),
        |v: &mut Value| {
            v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!(["renewal", "ward"])
        },
        |v: &mut Value| {
            v["pve"]["combat"]["projectile"]["on_commit_self_statuses"] =
                json!(["wound", "barrier"])
        },
        |v: &mut Value| v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(0),
        |v: &mut Value| v["pve"]["status_definitions"][1]["heal_bps"] = json!(10001),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                change(v);
            })
            .is_err()
        );
    }
    assert!(
        capacity_catalog(8, |v| {
            configure(v);
        })
        .is_err()
    );
    let mut old = DeltaHarness::new(2, |_| {});
    old.active();
    assert!(
        old.snapshot(0)
            .monsters
            .iter()
            .all(|m| m.statuses.is_none())
    );
}
#[test]
fn mp_shield_public_duplicate_cast_is_once_and_peer_hot_costs_remain_independent() {
    let mut h = harness(2);
    h.active();
    let request = h.input(
        0,
        Action::CastSelf {
            skill_id: h.engine.config.additional_skills[0].id.clone(),
        },
    );
    h.engine.input(request.clone()).unwrap();
    h.engine.input(request).unwrap();
    h.ticks(12);
    let a = h.snapshot(0).own.clone();
    assert_eq!(
        a.statuses
            .iter()
            .filter(|s| s
                .periodic
                .as_ref()
                .is_some_and(|p| p.kind == PeriodicEffectKind::MaxHpHeal))
            .count(),
        1
    );
    assert!(h.snapshot(1).own.statuses.iter().all(|s| {
        s.periodic
            .as_ref()
            .is_none_or(|p| p.kind != PeriodicEffectKind::MaxHpHeal)
    }));
    assert!(h.snapshot(1).own.spirit > a.spirit);
    assert_eq!(h.stores[0].admissions, 1);
}
#[test]
fn mp_shield_public_reconnect_preserves_instance_cadence_and_old_connection_fences() {
    let mut h = harness(8);
    h.active();
    renew(&mut h, 0);
    h.ticks(12);
    let old = h
        .snapshot(0)
        .own
        .statuses
        .iter()
        .find(|s| s.shield.is_some())
        .unwrap()
        .clone();
    let balance = old.shield.as_ref().unwrap().remaining;
    let previous = h.engine.seats[0].connection.clone();
    let connection = Connection {
        id: "new-periodic-connection".into(),
        generation: 2,
    };
    let r = Input {
        key: h.engine.create.key.clone(),
        participant_id: "P00".into(),
        entity_generation: 1,
        connection: connection.clone(),
        request_id: "reconnect-periodic".into(),
        sequence: 1,
        action: Action::Reconnect {
            previous_connection: previous.clone(),
            authenticated_principal: "principal-P00".into(),
        },
    };
    h.engine.input(r).unwrap();
    h.sequences[0] = 1;
    h.pump();
    let kept = h
        .snapshot(0)
        .own
        .statuses
        .iter()
        .find(|s| s.instance_id == old.instance_id)
        .unwrap();
    assert_eq!(kept.shield.as_ref().unwrap().remaining, balance);
    assert_eq!(kept.expires_at_ms, old.expires_at_ms);
    let ready = h.replicas[0].last_full_id().unwrap().to_string();
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    let ready = h.replicas[0].last_full_id().unwrap_or(&ready).to_string();
    h.action(
        0,
        Action::CombatReady {
            publication_id: ready,
        },
    )
    .unwrap();
    h.ticks(6);
    assert_eq!(
        h.snapshot(0)
            .own
            .statuses
            .iter()
            .find(|s| s.instance_id == old.instance_id)
            .unwrap()
            .applied_at_ms,
        old.applied_at_ms
    );
    let mut stale = h.input(0, Action::Stop {});
    stale.connection = previous;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
}
#[test]
fn mp_shield_unknown_settlement_cleans_source_at_exit_and_other_sources_continue() {
    let mut h = harness(8);
    h.active();
    cast(&mut h, 0);
    cast(&mut h, 1);
    h.ticks(16);
    assert!(h.snapshot(0).monsters.iter().any(|m| {
        m.statuses
            .as_ref()
            .unwrap()
            .iter()
            .any(|s| s.source.id == "P00")
    }));
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Settling);
    h.ticks(25);
    assert!(
        h.snapshot(1)
            .monsters
            .iter()
            .flat_map(|m| m.statuses.as_ref().unwrap())
            .any(|s| s.source.id == "P01")
    );
    assert!(h.events.iter().any(|(pid, e)| pid.as_deref() == Some("P01")
        && matches!(e,EventV4::Private{event:Event::PeriodicTick{source,..}}if source.id=="P01")));
    assert_eq!(h.engine.phase, Phase::Active);
    assert!(h.engine.pending() > 0);
    let frozen = h.snapshot(0).clone();
    h.action(
        0,
        Action::RequestFull {
            stream_epoch: h.engine.seats[0].epoch,
        },
    )
    .unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).monsters, frozen.monsters);
    h.hold_settle[0] = false;
    for (i, r) in std::mem::take(&mut h.held) {
        let proof = h.stores[i].execute(&r);
        h.receipt(i, &r, proof).unwrap();
    }
    h.pump();
    h.close();
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn mp_shield_full_delta_and_cache_resync_reconstruct_real_projected_statuses() {
    let mut h = harness(8);
    h.active();
    for i in 0..8 {
        renew(&mut h, i);
    }
    h.ticks(12);
    for i in 0..8 {
        cast(&mut h, i);
    }
    h.ticks(20);
    assert!(h.vectors.iter().any(|v| {
        v["snapshot"]["monsters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["statuses"].as_array().is_some_and(|s| !s.is_empty()))
    }));
    let epoch = h.engine.seats[3].epoch;
    h.action(
        3,
        Action::RequestFull {
            stream_epoch: epoch,
        },
    )
    .unwrap();
    h.replicas[3].forget_baseline();
    h.pump();
    h.ticks(150);
    h.close();
    if let Ok(path) = std::env::var("SDK522_INTEROP_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&h.vectors).unwrap()).unwrap();
    }
}
#[test]
fn mp_shield_financial_recovery_does_not_restore_effects_or_world() {
    let mut h = harness(2);
    h.active();
    renew(&mut h, 0);
    h.ticks(12);
    assert!(!h.snapshot(0).own.statuses.is_empty());
    h.hold_settle[0] = true;
    h.close();
    let cp = h.checkpoint.clone().unwrap();
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: cp,
    });
    let c = h.engine.config.clone();
    let catalog = delta_catalog(2, configure).unwrap();
    let mut recovered = TwoMembers::create(c, catalog, r).unwrap();
    assert!(recovered.world.is_none());
    assert_eq!(recovered.phase, Phase::Closed);
    assert!(
        recovered
            .drain()
            .iter()
            .all(|o| !matches!(o.body, OutputBody::PublicationV6 { .. }))
    );
}
#[test]
fn mp_shield_npc_dot_death_settles_only_target_while_seven_peers_continue() {
    let mut h = DeltaHarness::new(8, |v| {
        configure(v);
        v["pve"]["monster_positions"] = json!([[460, 400]]);
        v["pve"]["extra_spawns"] = json!([]);
        v["pve"]["combat"]["attack_interval_ms"] = json!(60000);
        v["pve"]["combat"]["projectile"]["damage_bps"] = json!(0);
        v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!(["wound"]);
    });
    h.stores[0].hp = 2;
    h.active();
    h.ticks(40);
    assert!(h.events.iter().any(|(p, e)| p.as_deref() == Some("P00")
        && matches!(
            e,
            EventV4::Private {
                event: Event::PeriodicTick { hp_damage: 1, .. }
            }
        )));
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Terminal);
    assert_eq!(h.stores[0].settlements, 1);
    assert!(h.stores[1..].iter().all(|s| s.settlements == 0));
    assert_eq!(h.engine.phase, Phase::Active);
    h.close();
}
#[test]
fn mp_shield_disconnect_timeout_cleans_lost_source_without_closing_peers() {
    let mut h = DeltaHarness::new(8, |v| {
        configure(v);
        v["params"]["runtime"]["reconnect_grace_ms"] = json!(600);
    });
    h.active();
    cast(&mut h, 0);
    cast(&mut h, 1);
    h.ticks(16);
    h.action(0, Action::ConnectionLost {}).unwrap();
    h.ticks(24);
    assert_eq!(h.stores[0].settlements, 1);
    assert_eq!(h.engine.phase, Phase::Active);
    assert!(h.events.iter().any(|(p, e)| p.as_deref() == Some("P00")
        && matches!(
            e,
            EventV4::Private {
                event: Event::StatusRemoved {
                    reason: StatusRemovalReason::SourceInvalid,
                    ..
                }
            }
        )));
    assert!(
        h.snapshot(1)
            .monsters
            .iter()
            .flat_map(|n| n.statuses.as_ref().unwrap())
            .any(|s| s.source.id == "P01")
    );
    h.close();
}
#[test]
fn mp_shield_decoder_refuses_foreign_source_and_wrong_target_atomically() {
    let mut h = harness(2);
    h.active();
    cast(&mut h, 0);
    h.ticks(16);
    let before = h.snapshot(0).clone();
    let mut malformed = serde_json::to_value(&before).unwrap();
    malformed["monsters"][0]["statuses"] = Value::Null;
    assert!(serde_json::from_value::<SnapshotV4>(malformed).is_err());
    for bad_target in [false, true] {
        let mut snapshot = before.clone();
        let status = snapshot
            .monsters
            .iter_mut()
            .flat_map(|n| n.statuses.as_mut().unwrap())
            .find(|s| s.source.kind == StatusEntityKind::Participant)
            .unwrap();
        if bad_target {
            status.target.generation += 1;
        } else {
            status.source.id = "P01".into();
        }
        let mut e = h.latest(0);
        e.revision += 100;
        e.publication_id = format!("invalid-{bad_target}");
        e.payload = PublicationPayloadV6::Full {
            snapshot: Box::new(snapshot),
            reason: FullReason::Resync,
        };
        assert_eq!(h.replicas[0].apply(&e), Err(DecodeError::InvalidFrame));
        assert_eq!(h.snapshot(0), &before);
    }
}
#[test]
fn mp_shield_hundred_public_runtimes_with_active_shield_release_world_and_financial_debts() {
    let cat=delta_catalog(8,|v|{configure(v);v["step_ms"]=json!(100);v["params"]["runtime"]["max_lag_steps"]=json!(100);v["params"]["runtime"]["max_catchup_steps"]=json!(10);v["params"]["runtime"]["call_timeout_ms"]=json!(10000);v["replication"]=json!({"ack_timeout_ms":30000,"baseline_ttl_ms":60000,"full_interval_ms":60000,"terminal_grace_ms":60000});}).unwrap();
    let c = cat.default_config().as_ref().clone();
    fn pump(
        client: &crate::Client,
        key: &MatchKey,
        stores: &mut [Store],
        replicas: &mut [ReplicaV6],
        checkpoint: &mut Option<IoCheckpointV4>,
        seen_hot: &mut [bool; 8],
    ) {
        for _ in 0..4 {
            let outputs = client.drain_outputs().unwrap();
            if outputs.is_empty() {
                break;
            }
            for o in outputs {
                match &o.body {
                    OutputBody::BusinessRequestV4 {
                        participant_id,
                        request,
                        ..
                    } => {
                        let i = participant_id[1..].parse::<usize>().unwrap();
                        let outcome = stores[i].execute(request);
                        client
                            .submit_business_receipt(MemberBusinessReceipt {
                                key: key.clone(),
                                participant_id: participant_id.clone(),
                                operation_id: request.operation_id.clone(),
                                attempt_id: request.attempt_id,
                                outcome,
                            })
                            .unwrap();
                    }
                    OutputBody::PublicationV6 {
                        recipient_id,
                        connection,
                        ..
                    } => {
                        let i = recipient_id[1..].parse::<usize>().unwrap();
                        let e = PublicationEnvelopeV6::from_output(&o).unwrap();
                        let state = replicas[i].apply(&e).unwrap();
                        seen_hot[i] |= state.own.shield_total.unwrap() > 0
                            && state.own.statuses.iter().any(|s| s.shield.is_some())
                            && state.own.phase == MemberPhase::Active
                            && state.own.statuses.iter().any(|s| {
                                s.periodic
                                    .as_ref()
                                    .is_some_and(|p| p.kind == PeriodicEffectKind::MaxHpHeal)
                            });
                        client
                            .submit_transport_receipt(TransportReceipt {
                                key: key.clone(),
                                recipient_id: recipient_id.clone(),
                                connection: connection.clone(),
                                output_id: o.id,
                                outcome: TransportOutcome::Sent,
                            })
                            .unwrap();
                    }
                    OutputBody::ResponsibilityV4 { checkpoint: cp } => {
                        *checkpoint = Some((**cp).clone())
                    }
                    _ => {}
                }
            }
        }
    }
    for cycle in 0..100 {
        let runtime = crate::Runtime::with_catalog(cat.clone());
        let client = runtime.client();
        let mut r = delta_request(&c);
        r.key.id = format!("shield-public-{cycle}");
        let key = r.key.clone();
        let mut stores: Vec<_> = (0..8)
            .map(|i| {
                let mut s = Store::new(i, &c);
                s.principal = format!("principal-P{i:02}");
                s
            })
            .collect();
        let mut replicas: Vec<_> = (0..8)
            .map(|i| ReplicaV6::new(key.clone(), format!("P{i:02}")))
            .collect();
        let mut cp = None;
        let mut seen_hot = [false; 8];
        client.create(typed6(r.clone())).unwrap();
        pump(
            &client,
            &key,
            &mut stores,
            &mut replicas,
            &mut cp,
            &mut seen_hot,
        );
        for s in &r.participants {
            client
                .submit_input(Input {
                    key: key.clone(),
                    participant_id: s.participant_id.clone(),
                    entity_generation: 1,
                    connection: s.connection.clone(),
                    request_id: "join".into(),
                    sequence: 1,
                    action: Action::Join {},
                })
                .unwrap();
        }
        pump(
            &client,
            &key,
            &mut stores,
            &mut replicas,
            &mut cp,
            &mut seen_hot,
        );
        for (i, s) in r.participants.iter().enumerate() {
            client
                .submit_input(Input {
                    key: key.clone(),
                    participant_id: s.participant_id.clone(),
                    entity_generation: 1,
                    connection: s.connection.clone(),
                    request_id: "ready".into(),
                    sequence: 2,
                    action: Action::CombatReady {
                        publication_id: replicas[i].last_full_id().unwrap().into(),
                    },
                })
                .unwrap();
        }
        for s in &r.participants {
            client
                .submit_input(Input {
                    key: key.clone(),
                    participant_id: s.participant_id.clone(),
                    entity_generation: 1,
                    connection: s.connection.clone(),
                    request_id: "renew".into(),
                    sequence: 3,
                    action: Action::CastSelf {
                        skill_id: c.additional_skills[0].id.clone(),
                    },
                })
                .unwrap();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            pump(
                &client,
                &key,
                &mut stores,
                &mut replicas,
                &mut cp,
                &mut seen_hot,
            );
            // Each asynchronous recipient must have observed a real live HOT;
            // it may legitimately expire while another recipient is drained.
            if seen_hot.iter().all(|v| *v) {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        client
            .close(Close {
                key: key.clone(),
                request_id: "close".into(),
            })
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            pump(
                &client,
                &key,
                &mut stores,
                &mut replicas,
                &mut cp,
                &mut seen_hot,
            );
            if cp.as_ref().is_some_and(|cp| {
                cp.state.members.iter().all(|m| {
                    m.checkpoint
                        .state
                        .operations
                        .iter()
                        .all(|o| o.terminal.is_some())
                })
            }) && replicas.iter().all(|r| {
                r.snapshot()
                    .is_some_and(|s| s.own.phase == MemberPhase::Terminal)
            }) {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(replicas.iter().all(|r| {
            r.snapshot()
                .is_some_and(|s| s.own.phase == MemberPhase::Terminal && s.own.statuses.is_empty())
        }));
        assert!(cp.unwrap().state.members.iter().all(|m| {
            m.checkpoint
                .state
                .operations
                .iter()
                .all(|o| o.terminal.is_some())
        }));
        assert!(
            stores
                .iter()
                .all(|s| s.admissions == 1 && s.settlements == 1)
        );
        drop(runtime);
        assert_eq!(client.drain_outputs(), Err(Error::Closed));
    }
}

#[test]
fn mp_shield_decoder_balances_null_and_invalid_sources_are_atomic() {
    let mut h = harness(2);
    h.active();
    renew(&mut h, 0);
    h.ticks(12);
    let before = h.snapshot(0).clone();
    let mut value = serde_json::to_value(&before).unwrap();
    value["self"]["shield_total"] = Value::Null;
    assert!(serde_json::from_value::<SnapshotV4>(value).is_err());
    for mode in 0..6 {
        let mut next = before.clone();
        match mode {
            0 => next.own.shield_total = Some(u32::MAX),
            1 => next.own.shield_total = None,
            2 => {
                next.own
                    .statuses
                    .iter_mut()
                    .find(|s| s.shield.is_some())
                    .unwrap()
                    .shield
                    .as_mut()
                    .unwrap()
                    .remaining = u32::MAX
            }
            3 => next.own.statuses[0].source.generation += 1,
            4 => next.own.statuses[0].target.generation += 1,
            _ => next.own.statuses.push(next.own.statuses[0].clone()),
        }
        let mut e = h.latest(0);
        e.revision += 100;
        e.publication_id = format!("bad-shield-{mode}");
        e.payload = PublicationPayloadV6::Full {
            snapshot: Box::new(next),
            reason: FullReason::Resync,
        };
        assert_eq!(h.replicas[0].apply(&e), Err(DecodeError::InvalidFrame));
        assert_eq!(h.snapshot(0), &before);
    }
    h.close();
}
