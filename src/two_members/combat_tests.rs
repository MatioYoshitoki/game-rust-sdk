use super::*;
pub(super) fn configure(v: &mut Value) {
    super::shields::configure(v);
    v["mechanism_version"] = json!(40);
    v["skill"]["mechanism"] = json!("unit_projectile");
    v["skill"]["on_commit_self_statuses"] = json!(["ward"]);
}
fn harness(n: usize) -> DeltaHarness {
    DeltaHarness::new(n, configure)
}
fn cast(h: &mut DeltaHarness, i: usize) {
    let target = h.snapshot(i).monsters[0].entity.clone();
    h.action(
        i,
        Action::CastUnit {
            skill_id: h.engine.config.skill.id.clone(),
            target: UnitTarget {
                id: target.id,
                generation: target.generation,
            },
        },
    )
    .unwrap();
}
#[test]
fn unified_public_config_rejects_legacy_versions_unsupported_actions_and_budgets() {
    for change in [
        |v: &mut Value| v["mechanism_version"] = json!(39),
        |v: &mut Value| v["schema_version"] = json!(6),
        |v: &mut Value| v["skill"]["mechanism"] = json!("ground_circle"),
        |v: &mut Value| v["skill"]["range"] = json!(0),
        |v: &mut Value| v["skill"]["target_relation"] = json!("friendly"),
        |v: &mut Value| v["pve"]["status_rules"]["max_active_per_entity"] = json!(32),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                change(v)
            })
            .is_err()
        );
    }
}
#[test]
fn unified_public_one_two_four_eight_unit_inputs_and_independent_members() {
    for n in [1, 2, 4, 8] {
        let mut h = harness(n);
        h.active();
        for i in 0..n {
            cast(&mut h, i);
        }
        h.ticks(12);
        for i in 0..n {
            assert!(
                h.events
                    .iter()
                    .any(|(pid, e)| pid.as_deref() == Some(&format!("P{i:02}"))
                        && matches!(
                            e,
                            EventV4::Private {
                                event: Event::ActionCommitted {
                                    unit_target: Some(_),
                                    ..
                                }
                            }
                        ))
            );
            assert_eq!(h.stores[i].admissions, 1);
        }
        h.close();
        assert_eq!(h.engine.pending(), 0);
    }
}
#[test]
fn unified_public_duplicate_unit_cost_and_second_skill_slot_are_independent() {
    for swap in [false, true] {
        let mut h = DeltaHarness::new(2, |v| {
            configure(v);
            if swap {
                let s = v["skill"].clone();
                v["skill"] = v["additional_skills"][0].clone();
                v["additional_skills"][0] = s;
            }
        });
        h.active();
        let skill = h
            .engine
            .config
            .player_skills()
            .find(|s| s.is_unit())
            .unwrap()
            .id
            .clone();
        let target = h.snapshot(0).monsters[0].entity.clone();
        let request = h.input(
            0,
            Action::CastUnit {
                skill_id: skill.clone(),
                target: UnitTarget {
                    id: target.id,
                    generation: target.generation,
                },
            },
        );
        h.engine.input(request.clone()).unwrap();
        h.engine.input(request).unwrap();
        h.ticks(12);
        assert_eq!(h.events.iter().filter(|(pid,e)|pid.as_deref()==Some("P00")&&matches!(e,EventV4::Private{event:Event::ActionCommitted{skill_id:Some(id),unit_target:Some(_),..}}if id==&skill)).count(),1);
        assert!(
            h.snapshot(1)
                .own
                .player_skills
                .iter()
                .all(|s| s.ready_at_ms == 0)
        );
        h.close();
    }
}
#[test]
fn unified_public_bad_identity_fails_without_cost_or_foreign_pending() {
    let mut h = harness(2);
    h.active();
    let spirit = h.snapshot(0).own.spirit;
    for t in [
        UnitTarget {
            id: "none".into(),
            generation: 1,
        },
        UnitTarget {
            id: "P01".into(),
            generation: 1,
        },
        UnitTarget {
            id: h.snapshot(0).monsters[0].entity.id.clone(),
            generation: 0,
        },
        UnitTarget {
            id: h.snapshot(0).monsters[0].entity.id.clone(),
            generation: 99,
        },
    ] {
        assert!(
            h.action(
                0,
                Action::CastUnit {
                    skill_id: h.engine.config.skill.id.clone(),
                    target: t
                }
            )
            .is_err()
        );
    }
    assert_eq!(h.snapshot(0).own.spirit, spirit);
    assert!(h.snapshot(1).own.pending_action.is_none());
    h.close();
}
#[test]
fn unified_public_unit_plan_reconnect_keeps_identity_and_rejects_old_connection() {
    let mut h = DeltaHarness::new(8, |v| {
        configure(v);
        v["skill"]["speed"] = json!(20);
    });
    h.active();
    cast(&mut h, 0);
    h.ticks(6);
    let plan = h
        .snapshot(0)
        .projectiles
        .iter()
        .find(|p| p.source.id == "P00" && p.unit_target.is_some())
        .unwrap()
        .clone();
    let previous = h.engine.seats[0].connection.clone();
    let connection = Connection {
        id: "new-unit-connection".into(),
        generation: 2,
    };
    h.engine
        .input(Input {
            key: h.engine.create.key.clone(),
            participant_id: "P00".into(),
            entity_generation: 1,
            connection,
            request_id: "unit-reconnect".into(),
            sequence: 1,
            action: Action::Reconnect {
                previous_connection: previous.clone(),
                authenticated_principal: "principal-P00".into(),
            },
        })
        .unwrap();
    h.sequences[0] = 1;
    h.pump();
    assert!(
        h.snapshot(0)
            .projectiles
            .iter()
            .any(|p| p.id == plan.id && p.unit_target == plan.unit_target)
    );
    let mut stale = h.input(0, Action::Stop {});
    stale.connection = previous;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    assert_eq!(
        h.events
            .iter()
            .filter(|(pid, e)| pid.as_deref() == Some("P00")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionCommitted {
                            unit_target: Some(_),
                            ..
                        }
                    }
                ))
            .count(),
        1
    );
    h.close();
}
#[test]
fn unified_public_unknown_settlement_clears_plan_and_other_member_continues() {
    let mut h = DeltaHarness::new(8, |v| {
        configure(v);
        v["skill"]["speed"] = json!(20);
    });
    h.active();
    cast(&mut h, 0);
    cast(&mut h, 1);
    h.ticks(6);
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.ticks(6);
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Settling);
    assert!(
        h.snapshot(1)
            .projectiles
            .iter()
            .all(|p| p.source.id != "P00")
    );
    assert!(
        h.snapshot(1)
            .projectiles
            .iter()
            .any(|p| p.source.id == "P01" && p.unit_target.is_some())
    );
    h.hold_settle[0] = false;
    for (i, r) in std::mem::take(&mut h.held) {
        let proof = h.stores[i].execute(&r);
        h.receipt(i, &r, proof).unwrap();
    }
    h.pump();
    h.close();
}
#[test]
fn unified_public_unit_frames_decoder_resync_and_cross_language_vectors() {
    let mut h = harness(8);
    h.active();
    for i in 0..8 {
        h.action(
            i,
            Action::CastSelf {
                skill_id: h.engine.config.additional_skills[0].id.clone(),
            },
        )
        .unwrap();
    }
    h.ticks(12);
    for i in 0..8 {
        cast(&mut h, i);
    }
    h.ticks(20);
    let epoch = h.engine.seats[3].epoch;
    h.replicas[3].forget_baseline();
    h.action(
        3,
        Action::RequestFull {
            stream_epoch: epoch,
        },
    )
    .unwrap();
    h.pump();
    h.ticks(210);
    h.close();
    assert!(h.vectors.iter().any(|v| {
        v["snapshot"]["self"]["pending_action"]
            .get("unit_target")
            .is_some()
    }));
    assert!(h.vectors.iter().any(|v| {
        v["snapshot"]["projectiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.get("unit_target").is_some())
    }));
    if let Ok(path) = std::env::var("SDK523_INTEROP_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&h.vectors).unwrap()).unwrap();
    }
}
#[test]
fn unified_public_decoder_rejects_wrong_unit_target_atomically() {
    let mut h = DeltaHarness::new(2, |v| {
        configure(v);
        v["skill"]["speed"] = json!(20);
    });
    h.active();
    cast(&mut h, 0);
    h.ticks(6);
    let before = h.snapshot(0).clone();
    let p = before
        .projectiles
        .iter()
        .position(|p| p.unit_target.is_some())
        .unwrap();
    let mut value = serde_json::to_value(&before).unwrap();
    value["projectiles"][p]["unit_target"] = Value::Null;
    assert!(serde_json::from_value::<SnapshotV4>(value).is_err());
    for mode in 0..3 {
        let mut next = before.clone();
        if mode == 0 {
            next.projectiles[p].unit_target.as_mut().unwrap().generation += 1;
        } else if mode == 1 {
            next.projectiles[p].unit_target.as_mut().unwrap().id = "P01".into();
        } else {
            next.projectiles[p].source.generation += 1;
        }
        let mut e = h.latest(0);
        e.revision += 100;
        e.publication_id = format!("bad-unit-{mode}");
        e.payload = PublicationPayloadV6::Full {
            snapshot: Box::new(next),
            reason: FullReason::Resync,
        };
        assert_eq!(h.replicas[0].apply(&e), Err(DecodeError::InvalidFrame));
        assert_eq!(h.snapshot(0), &before);
    }
    h.close();
}
#[test]
fn unified_public_financial_recovery_never_restores_unit_plans() {
    let mut h = DeltaHarness::new(2, |v| {
        configure(v);
        v["skill"]["speed"] = json!(20);
    });
    h.active();
    cast(&mut h, 0);
    h.ticks(6);
    assert!(
        h.snapshot(0)
            .projectiles
            .iter()
            .any(|p| p.unit_target.is_some())
    );
    h.hold_settle[0] = true;
    h.close();
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: h.checkpoint.clone().unwrap(),
    });
    let catalog = delta_catalog(2, configure).unwrap();
    let mut recovered = TwoMembers::create(h.engine.config.clone(), catalog, r).unwrap();
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
fn unified_hundred_public_runtimes_with_live_unit_plans_release_all_responsibilities() {
    let cat=delta_catalog(8,|v|{configure(v);v["skill"]["speed"]=json!(20);v["step_ms"]=json!(100);v["params"]["runtime"]["max_lag_steps"]=json!(100);v["params"]["runtime"]["max_catchup_steps"]=json!(10);v["params"]["runtime"]["call_timeout_ms"]=json!(10000);v["replication"]=json!({"ack_timeout_ms":30000,"baseline_ttl_ms":60000,"full_interval_ms":60000,"terminal_grace_ms":60000});}).unwrap();
    let c = cat.default_config().as_ref().clone();
    fn pump(
        client: &crate::Client,
        key: &MatchKey,
        stores: &mut [Store],
        replicas: &mut [ReplicaV6],
        checkpoint: &mut Option<IoCheckpointV4>,
        seen_unit: &mut [bool; 8],
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
                        seen_unit[i] |= state.own.phase == MemberPhase::Active
                            && state.projectiles.iter().any(|p| {
                                p.source.id == format!("P{i:02}") && p.unit_target.is_some()
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
        r.key.id = format!("unit-public-{cycle}");
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
        let mut seen_unit = [false; 8];
        client.create(typed6(r.clone())).unwrap();
        pump(
            &client,
            &key,
            &mut stores,
            &mut replicas,
            &mut cp,
            &mut seen_unit,
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
            &mut seen_unit,
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
                    action: Action::CastUnit {
                        skill_id: c.skill.id.clone(),
                        target: UnitTarget {
                            id: replicas[0].snapshot().unwrap().monsters[0]
                                .entity
                                .id
                                .clone(),
                            generation: 1,
                        },
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
                &mut seen_unit,
            );
            // Each asynchronous recipient must have observed a real live HOT;
            // it may legitimately expire while another recipient is drained.
            if seen_unit.iter().all(|v| *v) {
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
                &mut seen_unit,
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
