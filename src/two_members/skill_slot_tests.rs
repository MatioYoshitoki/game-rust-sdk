use super::*;
pub(super) fn configure(v: &mut Value) {
    super::combat::configure(v);
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-skills-host/arena-b.json"
    ))
    .unwrap();
    v["mechanism_version"] = json!(44);
    v["members"]["area_capacity"] = json!(8);
    v["members"]["projectile_capacity"] = json!(16);
    v["skill"] = sample["skill"].clone();
    v["additional_skills"] = sample["additional_skills"].clone();
    v["pve"]["status_definitions"].as_array_mut().unwrap().push(
        sample["pve"]["status_definitions"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone(),
    );
}
pub(super) fn cast_slot(h: &mut DeltaHarness, i: usize, slot: usize) {
    let s = h.engine.config.player_skills().nth(slot).unwrap().clone();
    let p = h.snapshot(i).monsters[0].position;
    let from = h.snapshot(i).own.position;
    let action = if s.is_self() {
        Action::CastSelf { skill_id: s.id }
    } else if s.is_unit() {
        Action::CastUnit {
            skill_id: s.id,
            target: UnitTarget {
                id: h.snapshot(i).monsters[0].entity.id.clone(),
                generation: h.snapshot(i).monsters[0].entity.generation,
            },
        }
    } else if s.is_ground() {
        Action::CastGround {
            skill_id: s.id,
            position: p,
        }
    } else {
        Action::Cast {
            skill_id: s.id,
            direction: [p[0] - from[0], p[1] - from[1]],
        }
    };
    h.action(i, action).unwrap();
}
#[test]
fn member_slots_configuration_count_presence_duplicate_and_last_row_validate_before_io() {
    for count in [1, 2, 4, 8] {
        let cat = delta_catalog(8, |v| {
            configure(v);
            v["additional_skills"]
                .as_array_mut()
                .unwrap()
                .truncate(count - 1);
        })
        .unwrap();
        assert_eq!(cat.default_config().player_skills().count(), count);
    }
    for change in [
        |v: &mut Value| v["mechanism_version"] = json!(43),
        |v: &mut Value| v["additional_skills"] = Value::Null,
        |v: &mut Value| {
            let last = v["additional_skills"][6].clone();
            v["additional_skills"].as_array_mut().unwrap().push(last);
        },
        |v: &mut Value| v["additional_skills"][6]["id"] = v["skill"]["id"].clone(),
        |v: &mut Value| v["additional_skills"][6]["cost"] = json!(1000001),
        |v: &mut Value| {
            v["additional_skills"][6]["on_commit_self_statuses"] = json!(["missing-state"])
        },
        |v: &mut Value| v["additional_skills"][6]["method"] = json!("patch-world"),
        |v: &mut Value| v["additional_skills"][6] = Value::Null,
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
            configure(v);
            v.as_object_mut().unwrap().remove("additional_skills");
        })
        .is_ok()
    );
}
#[test]
fn member_slots_last_row_history_and_output_budgets_are_not_trimmed() {
    for mode in 0..3 {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                let s: Value = serde_json::from_str(crate::test_data::fixture!(
                    "examples/member-piercing-host/arena-b.json"
                ))
                .unwrap();
                v["additional_skills"][6] = s["skill"].clone();
                v["additional_skills"][6]["id"] = json!("last-pierce");
                match mode {
                    0 => v["additional_skills"][6]["max_hits"] = json!(9),
                    1 => v["output_capacity"] = json!(512),
                    _ => v["additional_skills"][6]["on_hit_target_statuses"] = json!(["renewal"]),
                };
            })
            .is_err()
        );
    }
}
#[test]
fn member_slots_public_one_two_four_eight_members_and_all_slot_counts() {
    for n in [1, 2, 4, 8] {
        for count in [1, 2, 4, 8] {
            let mut h = DeltaHarness::new(n, |v| {
                configure(v);
                v["additional_skills"]
                    .as_array_mut()
                    .unwrap()
                    .truncate(count - 1);
                v["pve"]["stats"]["hp"] = json!(10000);
                v["pve"]["stats"]["atk"] = json!(1);
            });
            h.active();
            let ids = h
                .engine
                .config
                .player_skills()
                .map(|s| s.id.clone())
                .collect::<Vec<_>>();
            for slot in 0..count {
                for i in 0..n {
                    cast_slot(&mut h, i, slot);
                }
                h.ticks(30);
            }
            for i in 0..n {
                let skills = &h.snapshot(i).own.player_skills;
                assert_eq!(
                    skills
                        .iter()
                        .map(|s| s.skill_id.clone())
                        .collect::<Vec<_>>(),
                    ids
                );
                assert!(skills.iter().all(|s| s.ready_at_ms > 0));
            }
            h.close();
            assert_eq!(h.engine.pending(), 0);
        }
    }
}
#[test]
fn member_slots_duplicate_late_slot_input_charges_and_commits_once() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let id = h.engine.config.additional_skills[6].id.clone();
    let r = h.input(
        0,
        Action::CastSelf {
            skill_id: id.clone(),
        },
    );
    h.engine.input(r.clone()).unwrap();
    h.engine.input(r).unwrap();
    h.ticks(8);
    assert_eq!(h.events.iter().filter(|(pid,e)|pid.as_deref()==Some("P00")&&matches!(e,EventV4::Private{event:Event::ActionCommitted{skill_id:Some(s),..}}if *s==id)).count(),1);
    assert!(h.snapshot(0).own.player_skills[7].ready_at_ms > 0);
    assert!(
        h.snapshot(0).own.player_skills[..7]
            .iter()
            .all(|s| s.ready_at_ms == 0)
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
fn member_slots_reconnect_preserves_all_cooldowns_recovery_and_old_connection_fence() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    cast_slot(&mut h, 0, 0);
    h.ticks(15);
    cast_slot(&mut h, 0, 7);
    h.ticks(5);
    let before = h.snapshot(0).own.clone();
    let previous = h.engine.seats[0].connection.clone();
    h.engine
        .input(Input {
            key: h.engine.create.key.clone(),
            participant_id: "P00".into(),
            entity_generation: 1,
            connection: Connection {
                id: "slots-reconnect".into(),
                generation: 2,
            },
            request_id: "slots-reconnect".into(),
            sequence: 1,
            action: Action::Reconnect {
                previous_connection: previous.clone(),
                authenticated_principal: "principal-P00".into(),
            },
        })
        .unwrap();
    h.sequences[0] = 1;
    h.pump();
    let after = &h.snapshot(0).own;
    assert_eq!(after.player_skills, before.player_skills);
    assert_eq!(after.recovery_until_ms, before.recovery_until_ms);
    let mut stale = h.input(0, Action::Stop {});
    stale.connection = previous;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.close();
}
#[test]
fn member_slots_unknown_settlement_cleans_owner_while_peers_keep_slots_and_plans() {
    let mut h = DeltaHarness::new(8, configure);
    h.active();
    for i in 0..2 {
        cast_slot(&mut h, i, 0);
    }
    h.ticks(8);
    let peer = h.snapshot(1).own.player_skills.clone();
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.ticks(10);
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Settling);
    assert_eq!(h.snapshot(1).own.player_skills, peer);
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
            .any(|p| p.source.id == "P01" && p.piercing.is_some())
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
fn member_slots_all_mechanisms_private_order_full_delta_and_cross_language_vectors() {
    let mut h = DeltaHarness::new(8, |v| {
        configure(v);
        v["pve"]["stats"]["hp"] = json!(10000);
        v["pve"]["stats"]["atk"] = json!(1);
        v["pve"]["status_rules"]["max_active_per_entity"] = json!(12);
    });
    h.active();
    for slot in 0..8 {
        for i in 0..8 {
            cast_slot(&mut h, i, slot);
        }
        h.ticks(30);
        if slot == 4 {
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
        }
    }
    h.ticks(60);
    h.close();
    assert!(h.vectors.iter().any(|v| {
        v["snapshot"]["self"]["player_skills"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["ready_at_ms"].as_u64().unwrap() > 0)
    }));
    assert!(
        h.vectors
            .iter()
            .any(|v| v["snapshot"]["areas"] == json!([]))
    );
    if let Ok(path) = std::env::var("SDK527_INTEROP_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&h.vectors).unwrap()).unwrap();
    }
}
#[test]
fn member_slots_bad_lists_times_ids_and_pending_association_are_atomic() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    cast_slot(&mut h, 0, 7);
    h.pump();
    let before = h.snapshot(0).clone();
    for mode in 0..5 {
        let mut next = before.clone();
        match mode {
            0 => next
                .own
                .player_skills
                .push(next.own.player_skills[0].clone()),
            1 => next.own.player_skills[7].skill_id = next.own.player_skills[0].skill_id.clone(),
            2 => next.own.player_skills[7].skill_id = String::new(),
            3 => next.own.player_skills[7].ready_at_ms = 1u64 << 53,
            _ => next.own.pending_action.as_mut().unwrap().skill_id = Some("missing-slot".into()),
        };
        let mut e = h.latest(0);
        e.revision += 100;
        e.publication_id = format!("bad-slots-{mode}");
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
fn member_slots_recovery_closed_never_restores_cooldowns_or_world() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    cast_slot(&mut h, 0, 7);
    h.ticks(8);
    assert!(h.snapshot(0).own.player_skills[7].ready_at_ms > 0);
    h.hold_settle[0] = true;
    h.close();
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: h.checkpoint.clone().unwrap(),
    });
    let cat = delta_catalog(2, configure).unwrap();
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
fn member_slots_hundred_public_runtimes_with_multiple_committed_skills_release_all_responsibilities()
 {
    let cat=delta_catalog(8,|v|{configure(v);v["pve"]["stats"]["atk"]=json!(1);v["skill"]["ttl_ms"]=json!(60000);v["skill"]["speed"]=json!(0.1);v["skill"]["range"]=json!(10000);v["skill"]["radius"]=json!(8);v["step_ms"]=json!(100);v["params"]["runtime"]["max_lag_steps"]=json!(100);v["params"]["runtime"]["max_catchup_steps"]=json!(10);v["params"]["runtime"]["call_timeout_ms"]=json!(10000);v["replication"]=json!({"ack_timeout_ms":30000,"baseline_ttl_ms":60000,"full_interval_ms":60000,"terminal_grace_ms":60000});}).unwrap();
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
        for seat in &r.participants {
            client
                .submit_input(Input {
                    key: key.clone(),
                    participant_id: seat.participant_id.clone(),
                    entity_generation: 1,
                    connection: seat.connection.clone(),
                    request_id: "last-slot".into(),
                    sequence: 3,
                    action: Action::CastSelf {
                        skill_id: c.additional_skills.last().unwrap().id.clone(),
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
            if replicas.iter().all(|r| {
                r.snapshot().is_some_and(|s| {
                    s.own.player_skills.len() == 8
                        && s.own.player_skills[7].ready_at_ms > 0
                        && s.logical_ms >= s.own.recovery_until_ms
                        && s.own.pending_action.is_none()
                })
            }) {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for s in &r.participants {
            client
                .submit_input(Input {
                    key: key.clone(),
                    participant_id: s.participant_id.clone(),
                    entity_generation: 1,
                    connection: s.connection.clone(),
                    request_id: "renew".into(),
                    sequence: 4,
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
                    && s.own.player_skills[0].ready_at_ms > 0
                    && s.own.player_skills[7].ready_at_ms > 0
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
