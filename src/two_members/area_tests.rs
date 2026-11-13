use super::*;
fn configure(v: &mut Value) {
    super::combat::configure(v);
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-area-host/arena-a.json"
    ))
    .unwrap();
    v["mechanism_version"] = json!(41);
    v["members"]["area_capacity"] = json!(8);
    v["skill"] = sample["skill"].clone();
}
fn field(v: &mut Value) {
    configure(v);
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-area-host/arena-b.json"
    ))
    .unwrap();
    v["skill"] = sample["skill"].clone();
}
fn cast(h: &mut DeltaHarness, i: usize) {
    let p = h.snapshot(i).monsters[0].position;
    h.action(
        i,
        Action::CastGround {
            skill_id: h.engine.config.skill.id.clone(),
            position: p,
        },
    )
    .unwrap();
}
#[test]
fn member_areas_configuration_capacity_policy_and_output_budget_reject_before_io() {
    for change in [
        |v: &mut Value| v["mechanism_version"] = json!(40),
        |v: &mut Value| {
            v["members"]
                .as_object_mut()
                .unwrap()
                .remove("area_capacity")
                .map(|_| ())
                .unwrap()
        },
        |v: &mut Value| v["members"]["area_capacity"] = json!(0),
        |v: &mut Value| v["members"]["area_capacity"] = json!(9),
        |v: &mut Value| v["members"]["area_capacity"] = Value::Null,
        |v: &mut Value| v["skill"]["radius"] = json!(0),
        |v: &mut Value| v["output_capacity"] = json!(512),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                change(v)
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            field(v);
            v["skill"].as_object_mut().unwrap().remove("contact_policy");
        })
        .is_err()
    );
}
#[test]
fn member_areas_public_one_two_four_eight_circle_commit_and_independent_cost() {
    for n in [1, 2, 4, 8] {
        let mut h = DeltaHarness::new(n, configure);
        h.active();
        for i in 0..n {
            cast(&mut h, i);
        }
        h.ticks(8);
        assert_eq!(h.snapshot(0).areas.as_ref().unwrap().len(), n);
        h.ticks(40);
        assert!(h.snapshot(0).areas.as_ref().unwrap().is_empty());
        h.close();
        assert_eq!(h.engine.pending(), 0);
    }
}
#[test]
fn member_areas_duplicate_ground_input_only_commits_and_charges_once() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let r = h.input(
        0,
        Action::CastGround {
            skill_id: h.engine.config.skill.id.clone(),
            position: h.snapshot(0).monsters[0].position,
        },
    );
    h.engine.input(r.clone()).unwrap();
    h.engine.input(r).unwrap();
    h.ticks(8);
    assert_eq!(h.snapshot(0).areas.as_ref().unwrap().len(), 1);
    assert_eq!(
        h.events
            .iter()
            .filter(|(pid, e)| pid.as_deref() == Some("P00")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionCommitted {
                            ground_position: Some(_),
                            ..
                        }
                    }
                ))
            .count(),
        1
    );
    assert!(
        h.snapshot(1)
            .own
            .player_skills
            .iter()
            .all(|s| s.ready_at_ms == 0)
    );
    h.close();
}
#[test]
fn member_areas_public_bad_ground_coordinate_and_wrong_input_kind_do_not_pay() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let before = h.snapshot(0).own.spirit;
    for position in [[-1., 400.], [1280., 400.], [1200., 1200.]] {
        assert!(
            h.action(
                0,
                Action::CastGround {
                    skill_id: h.engine.config.skill.id.clone(),
                    position
                }
            )
            .is_err()
        );
    }
    assert!(
        h.action(
            0,
            Action::Cast {
                skill_id: h.engine.config.skill.id.clone(),
                direction: [1., 0.]
            }
        )
        .is_err()
    );
    assert_eq!(h.snapshot(0).own.spirit, before);
    assert!(h.snapshot(0).areas.as_ref().unwrap().is_empty());
    h.close();
}
#[test]
fn member_areas_public_field_reconnect_preserves_plan_deadline_budget_and_old_connection_fences() {
    let mut h = DeltaHarness::new(8, field);
    h.active();
    cast(&mut h, 0);
    h.ticks(8);
    let plan = h.snapshot(0).areas.as_ref().unwrap()[0].clone();
    let previous = h.engine.seats[0].connection.clone();
    h.engine
        .input(Input {
            key: h.engine.create.key.clone(),
            participant_id: "P00".into(),
            entity_generation: 1,
            connection: Connection {
                id: "area-reconnect".into(),
                generation: 2,
            },
            request_id: "area-reconnect".into(),
            sequence: 1,
            action: Action::Reconnect {
                previous_connection: previous.clone(),
                authenticated_principal: "principal-P00".into(),
            },
        })
        .unwrap();
    h.sequences[0] = 1;
    h.pump();
    let kept = h
        .snapshot(0)
        .areas
        .as_ref()
        .unwrap()
        .iter()
        .find(|p| p.id == plan.id)
        .unwrap();
    assert_eq!(kept, &plan);
    let mut stale = h.input(0, Action::Stop {});
    stale.connection = previous;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.close();
}
#[test]
fn member_areas_unknown_settlement_cleans_owner_only_and_peers_continue() {
    let mut h = DeltaHarness::new(8, field);
    h.active();
    cast(&mut h, 0);
    cast(&mut h, 1);
    h.ticks(8);
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.ticks(30);
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Settling);
    let areas = h.snapshot(1).areas.as_ref().unwrap();
    assert!(areas.iter().all(|p| p.source.id != "P00"));
    assert!(
        areas
            .iter()
            .any(|p| p.source.id == "P01" && p.completed_ticks > 0)
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
fn member_areas_two_skill_shapes_full_delta_clear_and_cross_language_vectors() {
    let mut h = DeltaHarness::new(8, |v| {
        configure(v);
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/member-area-host/arena-b.json"
        ))
        .unwrap();
        v["additional_skills"] = json!([sample["skill"].clone()]);
        v["skill"]["on_commit_self_statuses"] = json!(["renewal", "ward"]);
        v["pve"]["stats"]["hp"] = json!(10000);
        v["pve"]["status_rules"]["max_active_per_entity"] = json!(12);
    });
    h.active();
    for i in 0..8 {
        cast(&mut h, i);
    }
    h.ticks(40);
    for i in 0..8 {
        h.action(
            i,
            Action::CastGround {
                skill_id: h.engine.config.additional_skills[0].id.clone(),
                position: h.snapshot(i).monsters[0].position,
            },
        )
        .unwrap();
    }
    h.ticks(24);
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
    for kind in ["circle", "field"] {
        assert!(h.vectors.iter().any(|v| {
            v["snapshot"]["areas"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["mechanism"] == kind)
        }));
    }
    assert!(
        h.vectors
            .iter()
            .any(|v| v["snapshot"]["areas"] == json!([]))
    );
    if let Ok(path) = std::env::var("SDK524_INTEROP_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&h.vectors).unwrap()).unwrap();
    }
}
#[test]
fn member_areas_malformed_full_array_identity_schedule_budget_are_atomic() {
    let mut h = DeltaHarness::new(2, field);
    h.active();
    cast(&mut h, 0);
    h.ticks(8);
    let before = h.snapshot(0).clone();
    let mut value = serde_json::to_value(&before).unwrap();
    value["areas"] = Value::Null;
    assert!(serde_json::from_value::<SnapshotV4>(value).is_err());
    for mode in 0..5 {
        let mut next = before.clone();
        let a = &mut next.areas.as_mut().unwrap()[0];
        match mode {
            0 => a.source.generation += 1,
            1 => a.next_tick_at_ms = Some(0),
            2 => a.completed_ticks = 129,
            3 => a.contact_policy.as_mut().unwrap().remaining_hits = 999,
            _ => next
                .areas
                .as_mut()
                .unwrap()
                .push(before.areas.as_ref().unwrap()[0].clone()),
        };
        let mut e = h.latest(0);
        e.revision += 100;
        e.publication_id = format!("bad-area-{mode}");
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
fn member_areas_recovery_closed_never_restores_plans_or_publications() {
    let mut h = DeltaHarness::new(2, field);
    h.active();
    cast(&mut h, 0);
    h.ticks(8);
    assert!(!h.snapshot(0).areas.as_ref().unwrap().is_empty());
    h.hold_settle[0] = true;
    h.close();
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: h.checkpoint.clone().unwrap(),
    });
    let cat = delta_catalog(2, field).unwrap();
    let mut recovered = TwoMembers::create(h.engine.config.clone(), cat, r).unwrap();
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
fn member_areas_hundred_public_runtimes_with_committed_fields_release_all_responsibilities() {
    let cat=delta_catalog(8,|v|{field(v);v["skill"]["warning_ms"]=json!(0);v["skill"]["duration_ms"]=json!(60000);v["skill"]["tick_interval_ms"]=json!(1000);v["skill"]["contact_policy"]["max_total_hits"]=json!(16);v["step_ms"]=json!(100);v["params"]["runtime"]["max_lag_steps"]=json!(100);v["params"]["runtime"]["max_catchup_steps"]=json!(10);v["params"]["runtime"]["call_timeout_ms"]=json!(10000);v["replication"]=json!({"ack_timeout_ms":30000,"baseline_ttl_ms":60000,"full_interval_ms":60000,"terminal_grace_ms":60000});}).unwrap();
    let c = cat.default_config().as_ref().clone();
    fn pump(
        client: &crate::Client,
        key: &MatchKey,
        stores: &mut [Store],
        replicas: &mut [ReplicaV6],
        checkpoint: &mut Option<IoCheckpointV4>,
        seen_area: &mut [bool; 8],
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
                        seen_area[i] |= state.own.phase == MemberPhase::Active
                            && state
                                .areas
                                .as_ref()
                                .unwrap()
                                .iter()
                                .any(|p| p.source.id == format!("P{i:02}"));
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
        r.key.id = format!("area-public-{cycle}");
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
        let mut seen_area = [false; 8];
        client.create(typed6(r.clone())).unwrap();
        pump(
            &client,
            &key,
            &mut stores,
            &mut replicas,
            &mut cp,
            &mut seen_area,
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
            &mut seen_area,
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
                    action: Action::CastGround {
                        skill_id: c.skill.id.clone(),
                        position: replicas[0].snapshot().unwrap().monsters[0].position,
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
                &mut seen_area,
            );
            // Each asynchronous recipient must have observed a real live HOT;
            // it may legitimately expire while another recipient is drained.
            if seen_area.iter().all(|v| *v) {
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
                &mut seen_area,
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
            r.snapshot().is_some_and(|s| {
                s.own.phase == MemberPhase::Terminal
                    && s.own.statuses.is_empty()
                    && s.areas.as_ref().unwrap().is_empty()
            })
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
