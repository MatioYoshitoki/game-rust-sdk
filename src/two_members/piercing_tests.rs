use super::*;
fn configure(v: &mut Value) {
    super::combat::configure(v);
    let s: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-piercing-host/arena-a.json"
    ))
    .unwrap();
    v["mechanism_version"] = json!(43);
    v["members"]["area_capacity"] = json!(8);
    v["members"]["projectile_capacity"] = json!(16);
    v["skill"] = s["skill"].clone();
}
fn repeat(v: &mut Value) {
    configure(v);
    let s: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-piercing-host/arena-b.json"
    ))
    .unwrap();
    v["skill"] = s["skill"].clone();
}
fn cast(h: &mut DeltaHarness, i: usize) {
    let skill_id = h
        .engine
        .config
        .player_skills()
        .find(|s| s.is_piercing())
        .unwrap()
        .id
        .clone();
    let p = h.snapshot(i).monsters[0].position;
    let from = h.snapshot(i).own.position;
    h.action(
        i,
        Action::Cast {
            skill_id,
            direction: [p[0] - from[0], p[1] - from[1]],
        },
    )
    .unwrap();
}
fn plans(h: &DeltaHarness, i: usize) -> Vec<ProjectileViewV4> {
    h.snapshot(i)
        .projectiles
        .iter()
        .filter(|p| p.piercing.is_some())
        .cloned()
        .collect()
}
#[test]
fn member_flights_configuration_geometry_history_and_output_reject_before_io() {
    for f in [
        |v: &mut Value| v["mechanism_version"] = json!(42),
        |v: &mut Value| v["skill"]["max_hits"] = json!(9),
        |v: &mut Value| v["members"]["projectile_capacity"] = json!(17),
        |v: &mut Value| v["skill"]["radius"] = json!(129),
        |v: &mut Value| v["skill"]["speed"] = json!(0),
        |v: &mut Value| v["skill"]["ttl_ms"] = json!(30),
        |v: &mut Value| v["skill"]["repeat_interval_ms"] = json!(60001),
        |v: &mut Value| v["output_capacity"] = json!(512),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                f(v)
            })
            .is_err()
        );
    }
}
#[test]
fn member_flights_primary_secondary_one_two_four_eight_shared_world() {
    for n in [1, 2, 4, 8] {
        for secondary in [false, true] {
            let mut h = DeltaHarness::new(n, |v| {
                configure(v);
                if secondary {
                    let s = v["skill"].take();
                    v["skill"] = v["additional_skills"][0].clone();
                    v["additional_skills"][0] = s;
                }
            });
            h.active();
            for i in 0..n {
                cast(&mut h, i);
            }
            h.ticks(8);
            assert_eq!(plans(&h, 0).len(), n);
            h.ticks(180);
            assert!(plans(&h, 0).is_empty());
            h.close();
            assert_eq!(h.engine.pending(), 0);
        }
    }
}
#[test]
fn member_flights_duplicate_direction_input_only_pays_and_commits_once() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let input = h.input(
        0,
        Action::Cast {
            skill_id: h.engine.config.skill.id.clone(),
            direction: [1., 0.],
        },
    );
    h.engine.input(input.clone()).unwrap();
    h.engine.input(input).unwrap();
    h.ticks(8);
    assert_eq!(plans(&h, 0).len(), 1);
    assert_eq!(
        h.events
            .iter()
            .filter(|(pid, e)| pid.as_deref() == Some("P00")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionCommitted { .. }
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
fn member_flights_wrong_kind_and_zero_direction_do_not_pay() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let before = h.snapshot(0).own.spirit;
    for action in [
        Action::Cast {
            skill_id: h.engine.config.skill.id.clone(),
            direction: [0., 0.],
        },
        Action::CastGround {
            skill_id: h.engine.config.skill.id.clone(),
            position: [460., 400.],
        },
        Action::CastSelf {
            skill_id: h.engine.config.skill.id.clone(),
        },
    ] {
        assert!(h.action(0, action).is_err());
    }
    assert!(plans(&h, 0).is_empty());
    assert_eq!(h.snapshot(0).own.spirit, before);
    h.close();
}
#[test]
fn member_flights_reconnect_preserves_flight_history_deadline_and_old_connection_fence() {
    let mut h = DeltaHarness::new(8, repeat);
    h.active();
    cast(&mut h, 0);
    h.ticks(8);
    let before = plans(&h, 0)[0].clone();
    let previous = h.engine.seats[0].connection.clone();
    h.engine
        .input(Input {
            key: h.engine.create.key.clone(),
            participant_id: "P00".into(),
            entity_generation: 1,
            connection: Connection {
                id: "flight-reconnect".into(),
                generation: 2,
            },
            request_id: "flight-reconnect".into(),
            sequence: 1,
            action: Action::Reconnect {
                previous_connection: previous.clone(),
                authenticated_principal: "principal-P00".into(),
            },
        })
        .unwrap();
    h.sequences[0] = 1;
    h.pump();
    assert_eq!(
        plans(&h, 0).iter().find(|p| p.id == before.id),
        Some(&before)
    );
    let mut stale = h.input(0, Action::Stop {});
    stale.connection = previous;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.close();
}
#[test]
fn member_flights_unknown_settlement_removes_owner_and_peers_continue() {
    let mut h = DeltaHarness::new(8, repeat);
    h.active();
    for i in 0..2 {
        cast(&mut h, i);
    }
    h.ticks(8);
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.ticks(30);
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Settling);
    let p = plans(&h, 1);
    assert!(p.iter().all(|p| p.source.id != "P00"));
    assert!(
        p.iter()
            .any(|p| p.source.id == "P01" && p.piercing.as_ref().unwrap().remaining_range < 500.)
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
fn member_flights_once_and_repeat_full_delta_clear_cross_language_vectors() {
    let mut h = DeltaHarness::new(8, |v| {
        configure(v);
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/member-piercing-host/arena-b.json"
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
    h.ticks(100);
    for i in 0..8 {
        let p = h.snapshot(i).monsters[0].position;
        let from = h.snapshot(i).own.position;
        let id = h.engine.config.additional_skills[0].id.clone();
        h.action(
            i,
            Action::Cast {
                skill_id: id,
                direction: [p[0] - from[0], p[1] - from[1]],
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
    h.ticks(300);
    h.close();
    for interval in [0, 600] {
        assert!(h.vectors.iter().any(|v| {
            v["snapshot"]["projectiles"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["piercing"]["repeat_interval_ms"] == interval)
        }));
    }
    assert!(
        h.vectors
            .iter()
            .any(|v| v["snapshot"]["projectiles"] == json!([]))
    );
    if let Ok(path) = std::env::var("SDK526_INTEROP_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&h.vectors).unwrap()).unwrap();
    }
}
#[test]
fn member_flights_malformed_geometry_counter_history_and_identity_are_atomic() {
    let mut h = DeltaHarness::new(2, repeat);
    h.active();
    cast(&mut h, 0);
    h.ticks(8);
    let before = h.snapshot(0).clone();
    let index = before
        .projectiles
        .iter()
        .position(|p| p.piercing.is_some())
        .unwrap();
    for mode in 0..8 {
        let mut next = before.clone();
        let r = &mut next.projectiles[index];
        let p = r.piercing.as_mut().unwrap();
        match mode {
            0 => r.source.generation += 1,
            1 => p.direction = [0., 0.],
            2 => p.remaining_range = 0.,
            3 => p.remaining_hits = 999,
            4 => p.created_at_ms = next.logical_ms + 1,
            5 => p.expires_at_ms = next.logical_ms,
            6 => {
                r.unit_target = Some(UnitTarget {
                    id: next.monsters[0].entity.id.clone(),
                    generation: 1,
                })
            }
            _ => p.contacts.push(ProjectileContactViewV4 {
                target: UnitTarget {
                    id: "missing".into(),
                    generation: 1,
                },
                hit_at_ms: next.logical_ms,
                next_eligible_at_ms: Some(next.logical_ms + 600),
            }),
        };
        let mut e = h.latest(0);
        e.revision += 100;
        e.publication_id = format!("bad-flight-{mode}");
        e.payload = PublicationPayloadV6::Full {
            snapshot: Box::new(next),
            reason: FullReason::Resync,
        };
        assert_eq!(h.replicas[0].apply(&e), Err(DecodeError::InvalidFrame));
        assert_eq!(h.snapshot(0), &before);
    }
    for field in ["direction", "contacts", "max_hits"] {
        let mut value = serde_json::to_value(&before).unwrap();
        value["projectiles"][index]["piercing"][field] = Value::Null;
        assert!(serde_json::from_value::<SnapshotV4>(value).is_err());
    }
    h.close();
}
#[test]
fn member_flights_recovery_closed_never_restores_flights_or_publications() {
    let mut h = DeltaHarness::new(2, repeat);
    h.active();
    cast(&mut h, 0);
    h.ticks(8);
    assert!(!plans(&h, 0).is_empty());
    h.hold_settle[0] = true;
    h.close();
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: h.checkpoint.clone().unwrap(),
    });
    let cat = delta_catalog(2, repeat).unwrap();
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
fn member_flights_hundred_public_runtimes_with_committed_flights_release_all_responsibilities() {
    let cat=delta_catalog(8,|v|{repeat(v);v["skill"]["ttl_ms"]=json!(60000);v["skill"]["speed"]=json!(0.1);v["skill"]["range"]=json!(10000);v["skill"]["radius"]=json!(8);v["step_ms"]=json!(100);v["params"]["runtime"]["max_lag_steps"]=json!(100);v["params"]["runtime"]["max_catchup_steps"]=json!(10);v["params"]["runtime"]["call_timeout_ms"]=json!(10000);v["replication"]=json!({"ack_timeout_ms":30000,"baseline_ttl_ms":60000,"full_interval_ms":60000,"terminal_grace_ms":60000});}).unwrap();
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
                        seen_area[i] |= state.projectiles.iter().any(|p| {
                            p.source == state.own.participant.entity && p.piercing.is_some()
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
                    action: Action::Cast {
                        skill_id: c.skill.id.clone(),
                        direction: [1., 0.],
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
                    && s.projectiles.iter().all(|p| p.piercing.is_none())
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
