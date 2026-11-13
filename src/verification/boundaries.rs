use super::*;

fn business_outputs(outputs: Vec<Output>) -> Vec<BusinessRequest> {
    outputs
        .into_iter()
        .filter_map(|output| match output.body {
            OutputBody::BusinessRequest { request } => Some(request),
            _ => None,
        })
        .collect()
}

fn receipt(request: &BusinessRequest, outcome: ReceiptOutcome) -> BusinessReceipt {
    BusinessReceipt {
        key: create_request().key,
        operation_id: request.operation_id.clone(),
        attempt_id: request.attempt_id,
        outcome,
    }
}

fn view(h: &Harness) -> Snapshot {
    h.engine
        .world
        .as_ref()
        .expect("live world")
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
}

fn deploying() -> Harness {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    h.pump().unwrap();
    h
}

fn published(h: &mut Harness) -> Output {
    h.engine
        .drain()
        .into_iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .expect("full publication")
}

fn transport(h: &mut Harness, output: &Output, outcome: TransportOutcome) -> Result<Status> {
    h.engine.transport(TransportReceipt {
        key: create_request().key,
        output_id: output.id.clone(),
        recipient_id: "A".into(),
        connection: create_request().connection,
        outcome,
    })
}

fn pending_settlement(outcome: Outcome) -> (Harness, BusinessRequest) {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    match outcome {
        Outcome::Death => {
            h.action(Action::Leave {}).unwrap();
        }
        Outcome::Extraction => {
            h.advance(180000).unwrap();
            h.action(Action::ExtractStart {}).unwrap();
            while h.engine.status().phase == Phase::Active {
                h.engine.tick();
            }
        }
        _ => unreachable!("legacy harness only supports extraction and death"),
    }
    let request = business_outputs(h.engine.drain())
        .into_iter()
        .find(|r| matches!(r.body, Business::CommitSettlement { .. }))
        .unwrap();
    (h, request)
}

fn reconcile_request(engine: &mut Engine) -> BusinessRequest {
    business_outputs(engine.drain())
        .into_iter()
        .find(|r| matches!(r.body, Business::ReconcileOperation { .. }))
        .expect("explicit reconciliation")
}

#[test]
fn b1_k01_invalid_create_and_loaded_facts_have_no_partial_effects() {
    let mut invalid = Vec::new();
    let mut create = create_request();
    create.protocol_version = 0;
    invalid.push((create, Error::VersionMismatch));
    let mut create = create_request();
    create.config_id.push('x');
    invalid.push((create, Error::ConfigMismatch));
    let mut create = create_request();
    create.key.id = "x".repeat(65);
    invalid.push((create, Error::InvalidMessage));
    let mut create = create_request();
    create.connection.generation = 0;
    invalid.push((create, Error::InvalidMessage));
    for (create, error) in invalid {
        let mut engine = Engine::new(Config::frozen());
        assert_eq!(engine.create(create), Err(error));
        assert!(engine.drain().is_empty());
        assert_eq!(engine.live_worlds(), 0);
    }
    for case in 0..5 {
        let mut h = Harness::new();
        h.engine.create(create_request()).unwrap();
        let request = business_outputs(h.engine.drain()).remove(0);
        let ReceiptOutcome::Loaded { mut facts } = h.store.execute(&request).unwrap() else {
            panic!("facts")
        };
        let expected = match case {
            0 => {
                facts.principal = "other-user".into();
                Error::Unauthorized
            }
            1 => {
                facts.hp_points = 0;
                Error::NoResource
            }
            2 => {
                facts.atk_points = 1_000_001;
                Error::NoResource
            }
            3 => {
                facts.warehouse_quantity = 4;
                Error::NoResource
            }
            _ => {
                facts.revision = u32::MAX;
                Error::InvalidMessage
            }
        };
        assert_eq!(
            h.engine
                .business(receipt(&request, ReceiptOutcome::Loaded { facts })),
            Err(expected)
        );
        assert!(h.engine.drain().is_empty());
        assert_eq!(h.engine.live_worlds(), 0);
        assert_eq!(h.store.warehouse, 100);
    }
}

#[test]
fn b1_k01_fieldless_messages_reject_unknown_fields() {
    for kind in [
        "join",
        "stop",
        "cancel_action",
        "extract_start",
        "extract_cancel",
        "background",
        "foreground",
        "leave",
    ] {
        let valid = serde_json::json!({"kind": kind});
        assert!(serde_json::from_value::<Action>(valid).is_ok());
        let invalid = serde_json::json!({"kind": kind, "world_patch": {"hp": 100}});
        assert!(serde_json::from_value::<Action>(invalid).is_err());
    }
    for kind in ["unknown", "refused", "not_committed"] {
        assert!(
            serde_json::from_value::<ReceiptOutcome>(
                serde_json::json!({"kind": kind, "revision": 2})
            )
            .is_err()
        );
    }
    assert!(
        serde_json::from_value::<Business>(
            serde_json::json!({"kind": "load_participant", "world": {}})
        )
        .is_err()
    );
}

#[test]
fn b1_k02_duplicate_create_join_ready_activate_once() {
    let mut h = deploying();
    h.engine.create(create_request()).unwrap();
    assert!(h.engine.drain().is_empty());
    h.action(Action::Join {}).unwrap();
    let first = published(&mut h);
    let original_join = h.engine.history_for_test();
    h.engine.input(original_join).unwrap();
    assert!(h.engine.drain().is_empty());
    h.action(Action::Join {}).unwrap();
    let _newer = published(&mut h);
    transport(&mut h, &first, TransportOutcome::Sent).unwrap();
    h.action(Action::CombatReady {
        publication_id: first.id.clone(),
    })
    .unwrap();
    let ready = h.engine.history_for_test();
    h.engine.input(ready.clone()).unwrap();
    h.advance(30).unwrap();
    let active_at = view(&h).active_at_ms;
    h.action(Action::CombatReady {
        publication_id: first.id,
    })
    .unwrap();
    h.engine.input(ready).unwrap();
    h.action(Action::Join {}).unwrap();
    h.advance(300).unwrap();
    assert_eq!(view(&h).active_at_ms, active_at);
    assert_eq!(h.store.admission_commits, 1);
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::Activated
                }
            ))
            .count(),
        1
    );
    assert_eq!(h.engine.live_worlds(), 1);
}

#[test]
fn b1_k02_ready_accepts_sent_full_while_newer_full_is_inflight() {
    let mut h = deploying();
    h.action(Action::Join {}).unwrap();
    let original = published(&mut h);
    for _ in 0..20 {
        h.engine.tick();
    }
    let newer = published(&mut h);
    assert_ne!(original.id, newer.id);
    transport(&mut h, &original, TransportOutcome::Sent).unwrap();
    h.action(Action::CombatReady {
        publication_id: original.id,
    })
    .unwrap();
    h.advance(30).unwrap();
    assert_eq!(h.engine.status().phase, Phase::Active);
}

#[test]
fn b1_k02_background_revokes_prior_readiness_without_resetting_active_clock() {
    let mut h = Harness::new();
    h.start().unwrap();
    let active_at = view(&h).active_at_ms;
    let old = h
        .trace
        .iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap()
        .clone();
    let old_ready = h.engine.history_for_test();
    h.action(Action::Background {}).unwrap();
    let _background = published(&mut h);
    assert_eq!(
        transport(&mut h, &old, TransportOutcome::Sent),
        Err(Error::StalePublication)
    );
    assert_eq!(
        h.action(Action::CombatReady {
            publication_id: old.id
        }),
        Err(Error::NotReady)
    );
    h.engine.input(old_ready).unwrap();
    assert!(view(&h).protected);
    h.action(Action::Foreground {}).unwrap();
    let current = published(&mut h);
    transport(&mut h, &current, TransportOutcome::Sent).unwrap();
    h.action(Action::CombatReady {
        publication_id: current.id,
    })
    .unwrap();
    h.advance(30).unwrap();
    assert!(!view(&h).protected);
    assert_eq!(view(&h).active_at_ms, active_at);
}

#[test]
fn b1_k02_background_during_deployment_revokes_initial_sent_full() {
    let mut h = deploying();
    h.action(Action::Join {}).unwrap();
    let initial = published(&mut h);
    transport(&mut h, &initial, TransportOutcome::Sent).unwrap();
    h.action(Action::Background {}).unwrap();
    let _background = published(&mut h);
    h.action(Action::Foreground {}).unwrap();
    let current = published(&mut h);
    assert_eq!(
        h.action(Action::CombatReady {
            publication_id: initial.id
        }),
        Err(Error::NotReady)
    );
    transport(&mut h, &current, TransportOutcome::Sent).unwrap();
    h.action(Action::CombatReady {
        publication_id: current.id,
    })
    .unwrap();
    h.advance(30).unwrap();
    assert_eq!(h.engine.status().phase, Phase::Active);
}

#[test]
fn b1_k02_background_ready_requires_foreground_and_reentry_revokes_unready_epoch() {
    let mut h = Harness::new();
    h.start().unwrap();
    let active_at = view(&h).active_at_ms;
    h.action(Action::Background {}).unwrap();
    let background = published(&mut h);
    transport(&mut h, &background, TransportOutcome::Sent).unwrap();
    assert_eq!(
        h.action(Action::CombatReady {
            publication_id: background.id
        }),
        Err(Error::NotReady)
    );
    assert!(view(&h).protected);
    h.action(Action::Foreground {}).unwrap();
    let unready = published(&mut h);
    transport(&mut h, &unready, TransportOutcome::Sent).unwrap();
    h.action(Action::Background {}).unwrap();
    published(&mut h);
    h.action(Action::Foreground {}).unwrap();
    let current = published(&mut h);
    assert_eq!(
        h.action(Action::CombatReady {
            publication_id: unready.id
        }),
        Err(Error::NotReady)
    );
    transport(&mut h, &current, TransportOutcome::Sent).unwrap();
    h.action(Action::CombatReady {
        publication_id: current.id,
    })
    .unwrap();
    h.advance(30).unwrap();
    assert!(!view(&h).protected);
    assert_eq!(view(&h).active_at_ms, active_at);
}

#[test]
fn b1_publication_terminal_order_matrix_and_replacement_full() {
    let outcomes = [
        TransportOutcome::Enqueued,
        TransportOutcome::Sent,
        TransportOutcome::Failed,
        TransportOutcome::Cancelled,
    ];
    for first in outcomes {
        for second in outcomes {
            let mut h = deploying();
            h.action(Action::Join {}).unwrap();
            let original = published(&mut h);
            transport(&mut h, &original, first).unwrap();
            let conflict = first != TransportOutcome::Enqueued
                && first != second
                && second != TransportOutcome::Enqueued;
            let result = transport(&mut h, &original, second);
            if conflict {
                assert_eq!(result, Err(Error::Conflict));
            } else {
                result.unwrap();
            }
            let effective = if first == TransportOutcome::Enqueued {
                second
            } else {
                first
            };
            let ready = h.action(Action::CombatReady {
                publication_id: original.id.clone(),
            });
            if effective == TransportOutcome::Sent {
                ready.unwrap();
            } else {
                assert_eq!(ready, Err(Error::NotReady));
                if matches!(
                    effective,
                    TransportOutcome::Failed | TransportOutcome::Cancelled
                ) {
                    let replacements: Vec<_> = h
                        .engine
                        .drain()
                        .into_iter()
                        .filter(|o| matches!(o.body, OutputBody::Publication { .. }))
                        .collect();
                    assert_eq!(replacements.len(), 1);
                    transport(&mut h, &replacements[0], TransportOutcome::Sent).unwrap();
                    h.action(Action::CombatReady {
                        publication_id: replacements[0].id.clone(),
                    })
                    .unwrap();
                } else {
                    transport(&mut h, &original, TransportOutcome::Sent).unwrap();
                    h.action(Action::CombatReady {
                        publication_id: original.id,
                    })
                    .unwrap();
                }
            }
            h.advance(30).unwrap();
            assert_eq!(h.engine.status().phase, Phase::Active);
            assert_eq!(h.store.admission_commits, 1);
        }
    }
}

#[test]
fn b1_publication_identity_and_old_failure_cannot_change_new_full() {
    let mut h = deploying();
    h.action(Action::Join {}).unwrap();
    let old = published(&mut h);
    h.action(Action::Join {}).unwrap();
    let current = published(&mut h);
    let mut forged = TransportReceipt {
        key: create_request().key,
        output_id: current.id.clone(),
        recipient_id: "other".into(),
        connection: create_request().connection,
        outcome: TransportOutcome::Sent,
    };
    assert_eq!(h.engine.transport(forged.clone()), Err(Error::Unauthorized));
    forged.recipient_id = "A".into();
    forged.connection.generation = 2;
    assert_eq!(
        h.engine.transport(forged.clone()),
        Err(Error::StaleConnection)
    );
    forged.connection = create_request().connection;
    forged.key.incarnation += 1;
    assert_eq!(h.engine.transport(forged), Err(Error::StaleMatch));
    transport(&mut h, &current, TransportOutcome::Sent).unwrap();
    transport(&mut h, &old, TransportOutcome::Failed).unwrap();
    assert!(h.engine.drain().is_empty());
    h.action(Action::CombatReady {
        publication_id: current.id,
    })
    .unwrap();
    h.advance(30).unwrap();
    assert_eq!(h.engine.status().phase, Phase::Active);
}

#[test]
fn b1_input_history_eviction_rejects_replay_without_restarting_movement() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Move {
        direction: [1.0, 0.0],
    })
    .unwrap();
    let movement = h.engine.history_for_test();
    h.action(Action::Background {}).unwrap();
    h.pump().unwrap();
    h.engine.input(movement.clone()).unwrap();
    h.advance(300).unwrap();
    assert_eq!(view(&h).position, [0.0, 0.0]);
    for _ in 0..140 {
        h.action(Action::Stop {}).unwrap_err();
    }
    assert_eq!(h.engine.input(movement), Err(Error::StaleSequence));
}

#[test]
fn b1_k05_material_and_orb_are_consumed_once_and_clamped() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Attack {}).unwrap();
    h.advance(400).unwrap();
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(1000).unwrap();
    let state = view(&h);
    assert_eq!(
        state.resources.iter().filter(|r| r.id == "drop-1").count(),
        1
    );
    h.action(Action::Pickup {
        resource_id: "drop-1".into(),
        expected_revision: state.inventory_revision,
    })
    .unwrap();
    let material = h.engine.history_for_test();
    h.engine.input(material).unwrap();
    assert_eq!(view(&h).inventory_quantity, 7);
    assert_eq!(
        h.action(Action::Pickup {
            resource_id: "drop-1".into(),
            expected_revision: view(&h).inventory_revision
        }),
        Err(Error::NoResource)
    );
    h.advance(10000).unwrap();
    let before = view(&h);
    h.action(Action::Pickup {
        resource_id: "orb-1".into(),
        expected_revision: before.inventory_revision,
    })
    .unwrap();
    let after = view(&h);
    let orb = h.engine.history_for_test();
    h.engine.input(orb).unwrap();
    assert_eq!(view(&h), after);
    assert_eq!(after.spirit, (before.spirit + 15).min(100));
    assert!(after.hp <= 1000);
    assert_eq!(after.inventory_revision, before.inventory_revision);
    assert_eq!(
        h.action(Action::Pickup {
            resource_id: "orb-1".into(),
            expected_revision: after.inventory_revision
        }),
        Err(Error::NoResource)
    );
    h.pump().unwrap();
    assert_eq!(h.trace.iter().filter(|o| matches!(&o.body, OutputBody::Event { event: Event::PickedUp { resource_id, .. } } if resource_id == "orb-1")).count(), 1);
}

#[test]
fn b1_k05_wrong_inventory_revision_keeps_material_and_quantity() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Attack {}).unwrap();
    h.advance(400).unwrap();
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(1000).unwrap();
    let before = view(&h);
    assert_eq!(
        h.action(Action::Pickup {
            resource_id: "drop-1".into(),
            expected_revision: before.inventory_revision + 1
        }),
        Err(Error::RevisionConflict)
    );
    assert_eq!(view(&h), before);
}

#[test]
fn b1_k06_background_interrupts_final_channel_step_then_new_channel_can_finish() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    h.advance(180000).unwrap();
    h.action(Action::ExtractStart {}).unwrap();
    h.advance(4980).unwrap();
    h.action(Action::Background {}).unwrap();
    h.pump().unwrap();
    h.advance(60).unwrap();
    assert_eq!(h.store.settlement_commits, 0);
    assert!(view(&h).extraction_finishes_ms.is_none());
    h.action(Action::Foreground {}).unwrap();
    h.pump().unwrap();
    let publication_id = h
        .trace
        .iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap()
        .id
        .clone();
    h.action(Action::CombatReady { publication_id }).unwrap();
    h.action(Action::ExtractStart {}).unwrap();
    h.advance(5100).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits
        ),
        (102, 30, 1)
    );
}

#[test]
fn b1_k06_same_step_damage_precedes_channel_completion() {
    let mut config = Config::frozen();
    config.target.position = [40.0, 0.0];
    config.target.interval_ms = 30;
    config.extraction_unlock_ms = 0;
    config.extraction_channel_ms = 30;
    let mut h = Harness::new();
    h.engine = Engine::new(config);
    h.start().unwrap();
    h.action(Action::ExtractStart {}).unwrap();
    h.advance(30).unwrap();
    assert_eq!(h.store.settlement_commits, 0);
    assert!(view(&h).extraction_finishes_ms.is_none());
}

#[test]
fn b1_k06_channel_cannot_commit_before_five_seconds() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    h.advance(180000).unwrap();
    let started = h.engine.status().logical_ms;
    h.action(Action::ExtractStart {}).unwrap();
    h.advance(4980).unwrap();
    assert_eq!(h.store.settlement_commits, 0);
    assert_eq!(h.engine.status().phase, Phase::Active);
    h.advance(30).unwrap();
    assert!(h.engine.status().logical_ms >= started + 5000);
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits
        ),
        (102, 30, 1)
    );
}

#[test]
fn b1_k07_natural_lethal_hit_cancels_uncommitted_skill() {
    let mut h = Harness::new();
    let mut config = Config::frozen();
    config.target.position = [40.0, 0.0];
    config.target.interval_ms = 30;
    h.engine = Engine::new(config);
    h.engine.create(create_request()).unwrap();
    let load = business_outputs(h.engine.drain()).remove(0);
    let ReceiptOutcome::Loaded { mut facts } = h.store.execute(&load).unwrap() else {
        panic!("facts")
    };
    facts.hp_points = 1;
    h.engine
        .business(receipt(&load, ReceiptOutcome::Loaded { facts }))
        .unwrap();
    h.pump().unwrap();
    h.action(Action::Join {}).unwrap();
    h.pump().unwrap();
    h.advance(30).unwrap();
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(30).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits
        ),
        (98, 0, 1)
    );
    assert!(!h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::ActionCommitted { .. }
        }
    )));
    assert_eq!(h.engine.live_worlds(), 0);
    assert!(h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::Damage { amount: 1, .. }
        }
    )));
}

#[test]
fn b1_melee_cannot_damage_through_a_static_wall() {
    let mut config = Config::frozen();
    config.spawn = [200.0, 0.0];
    config.extraction_point = config.spawn;
    config.target.position = [260.0, 0.0];
    config.target.attack_range = 80.0;
    let mut h = Harness::new();
    h.engine = Engine::new(config);
    h.start().unwrap();
    h.advance(2400).unwrap();
    assert_eq!(view(&h).hp, 1000);
}

#[test]
fn b1_k07_draining_rejects_lifecycle_inputs_and_duplicate_death() {
    let (mut h, request) = pending_settlement(Outcome::Death);
    for action in [
        Action::Join {},
        Action::Leave {},
        Action::Background {},
        Action::Foreground {},
        Action::ExtractStart {},
    ] {
        assert_eq!(h.action(action), Err(Error::InvalidState));
    }
    assert!(business_outputs(h.engine.drain()).is_empty());
    let result = h.store.execute(&request).unwrap();
    h.engine
        .business(receipt(&request, result.clone()))
        .unwrap();
    h.engine.business(receipt(&request, result)).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits
        ),
        (99, 3, 1)
    );
    assert!(h.engine.pending_for_test().is_empty());
}

#[test]
fn b1_k08_refused_settlement_requires_explicit_proof_and_preserves_frozen_amounts() {
    let (mut h, request) = pending_settlement(Outcome::Extraction);
    h.store.journal.insert(
        request.operation_id.clone(),
        (request.body.clone(), ReceiptOutcome::Refused {}),
    );
    h.engine
        .business(receipt(&request, ReceiptOutcome::Refused {}))
        .unwrap();
    h.engine
        .business(receipt(&request, ReceiptOutcome::Refused {}))
        .unwrap();
    let outputs = h.engine.drain();
    assert_eq!(
        outputs
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::OperationNeedsReconciliation { .. }
                }
            ))
            .count(),
        1
    );
    let reconcile = business_outputs(outputs).remove(0);
    let before = h.engine.status();
    for _ in 0..100 {
        h.engine.tick();
    }
    assert_eq!(h.engine.status(), before);
    assert_eq!(h.store.warehouse, 95);
    assert_eq!(h.store.settlement_commits, 0);
    let proof = ReceiptOutcome::Committed {
        revision: 2,
        commit_id: "tx-2".into(),
    };
    assert_eq!(
        h.engine.business(receipt(&request, proof)),
        Err(Error::Conflict)
    );
    assert_eq!(
        h.engine.business(receipt(
            &reconcile,
            ReceiptOutcome::Reconciled {
                revision: 3,
                commit_id: "bad".into()
            }
        )),
        Err(Error::RevisionConflict)
    );
    let outcome = h.store.execute(&reconcile).unwrap();
    h.engine
        .business(receipt(&reconcile, outcome.clone()))
        .unwrap();
    h.engine.business(receipt(&reconcile, outcome)).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits
        ),
        (102, 30, 1)
    );
    assert!(h.engine.pending_for_test().is_empty());
    assert_eq!(h.engine.live_worlds(), 0);
}

#[test]
fn b1_k08_refused_settlement_recovers_after_world_is_closed() {
    let (mut h, request) = pending_settlement(Outcome::Death);
    h.store.journal.insert(
        request.operation_id.clone(),
        (request.body.clone(), ReceiptOutcome::Refused {}),
    );
    h.engine
        .business(receipt(&request, ReceiptOutcome::Refused {}))
        .unwrap();
    let reconcile = reconcile_request(&mut h.engine);
    h.engine
        .close(Close {
            key: create_request().key,
            request_id: "close".into(),
        })
        .unwrap();
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(
        h.engine
            .business(receipt(&reconcile, ReceiptOutcome::NotCommitted {})),
        Err(Error::ExternalRejected)
    );
    let outcome = h.store.execute(&reconcile).unwrap();
    h.engine.business(receipt(&reconcile, outcome)).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.engine.live_worlds()
        ),
        (99, 3, 0)
    );
    assert!(h.engine.pending_for_test().is_empty());
}

#[test]
fn b1_k08_retry_exhaustion_emits_one_reconciliation_and_never_changes_business_id() {
    let (mut h, original) = pending_settlement(Outcome::Death);
    let mut attempt = original.clone();
    for index in 1..=3 {
        assert_eq!(attempt.operation_id, original.operation_id);
        assert_eq!(attempt.body, original.body);
        assert_eq!(attempt.attempt_id, index);
        h.engine
            .business(receipt(&attempt, ReceiptOutcome::NotCommitted {}))
            .unwrap();
        attempt = business_outputs(h.engine.drain()).remove(0);
    }
    assert!(matches!(attempt.body, Business::ReconcileOperation { .. }));
    h.engine
        .business(receipt(&original, ReceiptOutcome::Unknown {}))
        .unwrap_err();
    h.store.journal.insert(
        original.operation_id.clone(),
        (original.body, ReceiptOutcome::NotCommitted {}),
    );
    let outcome = h.store.execute(&attempt).unwrap();
    h.engine.business(receipt(&attempt, outcome)).unwrap();
    assert_eq!((h.store.warehouse, h.store.settlement_commits), (99, 1));
    assert!(h.engine.pending_for_test().is_empty());
}

#[test]
fn b1_k08_unknown_query_escalates_once_and_late_commit_resolves_all_pending_reads() {
    let (mut h, request) = pending_settlement(Outcome::Death);
    let committed = h.store.execute(&request).unwrap();
    h.engine
        .business(receipt(&request, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .business(receipt(&query, ReceiptOutcome::Unknown {}))
        .unwrap();
    let reconcile = reconcile_request(&mut h.engine);
    h.engine
        .business(receipt(&query, ReceiptOutcome::Unknown {}))
        .unwrap();
    assert!(h.engine.drain().is_empty());
    h.engine
        .business(receipt(&request, committed.clone()))
        .unwrap();
    assert!(h.engine.pending_for_test().is_empty());
    assert_eq!(
        h.engine
            .business(receipt(&query, ReceiptOutcome::NotCommitted {})),
        Err(Error::Conflict)
    );
    let ReceiptOutcome::Committed {
        revision,
        commit_id,
    } = committed
    else {
        panic!("commit")
    };
    h.engine
        .business(receipt(
            &reconcile,
            ReceiptOutcome::Reconciled {
                revision,
                commit_id,
            },
        ))
        .unwrap();
    assert_eq!(h.store.settlement_commits, 1);
    assert!(business_outputs(h.engine.drain()).is_empty());
}

#[test]
fn b1_k08_wrong_receipts_cannot_resolve_or_modify_settlement() {
    let (mut h, request) = pending_settlement(Outcome::Death);
    let mut invalid = receipt(
        &request,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "tx".into(),
        },
    );
    invalid.key.incarnation = 2;
    assert_eq!(h.engine.business(invalid), Err(Error::StaleMatch));
    let mut invalid = receipt(
        &request,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "tx".into(),
        },
    );
    invalid.attempt_id = 2;
    assert_eq!(h.engine.business(invalid), Err(Error::InvalidMessage));
    assert_eq!(
        h.engine.business(receipt(
            &request,
            ReceiptOutcome::Committed {
                revision: 1,
                commit_id: "tx".into()
            }
        )),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        h.engine.business(receipt(
            &request,
            ReceiptOutcome::Reconciled {
                revision: 2,
                commit_id: "tx".into()
            }
        )),
        Err(Error::InvalidMessage)
    );
    assert_eq!(h.engine.status().phase, Phase::Draining);
    assert_eq!(h.store.warehouse, 95);
    let outcome = h.store.execute(&request).unwrap();
    h.engine.business(receipt(&request, outcome)).unwrap();
    assert_eq!(h.store.warehouse, 99);
}

#[test]
fn b1_k09_closed_admission_no_commit_never_starts_another_debit() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let load = business_outputs(h.engine.drain()).remove(0);
    let facts = h.store.execute(&load).unwrap();
    h.engine.business(receipt(&load, facts)).unwrap();
    let admit = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .business(receipt(&admit, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .close(Close {
            key: create_request().key,
            request_id: "close".into(),
        })
        .unwrap();
    h.engine
        .business(receipt(&query, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    assert!(business_outputs(h.engine.drain()).is_empty());
    assert!(h.engine.pending_for_test().is_empty());
    assert_eq!(h.store.warehouse, 100);
}

#[test]
fn b1_k09_query_can_confirm_admission_refusal_without_injecting_inventory() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let load = business_outputs(h.engine.drain()).remove(0);
    let facts = h.store.execute(&load).unwrap();
    h.engine.business(receipt(&load, facts)).unwrap();
    let admit = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .business(receipt(&admit, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .business(receipt(&query, ReceiptOutcome::Refused {}))
        .unwrap();
    assert_eq!(h.engine.status().phase, Phase::Closed);
    assert!(h.engine.pending_for_test().is_empty());
    assert_eq!(h.store.warehouse, 100);
}

#[test]
fn b1_k09_late_load_never_creates_admission_after_close() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let load = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .close(Close {
            key: create_request().key,
            request_id: "close".into(),
        })
        .unwrap();
    let outcome = h.store.execute(&load).unwrap();
    h.engine.business(receipt(&load, outcome)).unwrap();
    assert!(business_outputs(h.engine.drain()).is_empty());
    assert_eq!(h.engine.live_worlds(), 0);
    assert!(h.engine.pending_for_test().is_empty());
}

#[test]
fn b1_k09_refused_compensation_is_reconciled_once_without_world() {
    let mut h = deploying();
    h.engine
        .close(Close {
            key: create_request().key,
            request_id: "close".into(),
        })
        .unwrap();
    let compensation = business_outputs(h.engine.drain()).remove(0);
    assert_eq!(
        h.engine.business(receipt(
            &compensation,
            ReceiptOutcome::Committed {
                revision: 1,
                commit_id: "wrong".into()
            }
        )),
        Err(Error::RevisionConflict)
    );
    h.store.journal.insert(
        compensation.operation_id.clone(),
        (compensation.body.clone(), ReceiptOutcome::Refused {}),
    );
    h.engine
        .business(receipt(&compensation, ReceiptOutcome::Refused {}))
        .unwrap();
    let reconcile = reconcile_request(&mut h.engine);
    let proof = h.store.execute(&reconcile).unwrap();
    h.engine
        .business(receipt(&reconcile, proof.clone()))
        .unwrap();
    h.engine.business(receipt(&reconcile, proof)).unwrap();
    assert_eq!(
        (h.store.warehouse, h.store.years, h.engine.live_worlds()),
        (100, 0, 0)
    );
    assert!(h.engine.pending_for_test().is_empty());
}

#[test]
fn b1_k09_close_uses_reserved_capacity_when_ordinary_input_is_full() {
    let mut h = Harness::new();
    h.start().unwrap();
    for _ in 0..300 {
        if h.action(Action::Join {}) == Err(Error::QueueFull) {
            break;
        }
    }
    let close = Close {
        key: create_request().key,
        request_id: "close".into(),
    };
    let status = h.engine.close(close.clone()).unwrap();
    assert_eq!(h.engine.close(close).unwrap(), status);
    assert_eq!(h.engine.live_worlds(), 0);
    let requests = business_outputs(h.engine.drain());
    assert_eq!(requests.len(), 1);
    let outcome = h.store.execute(&requests[0]).unwrap();
    h.engine.business(receipt(&requests[0], outcome)).unwrap();
    assert_eq!(h.store.warehouse, 98);
    assert!(h.engine.pending_for_test().is_empty());
}

#[test]
fn b1_k08_settlement_retry_after_known_failure_is_not_double_applied() {
    let (mut h, original) = pending_settlement(Outcome::Extraction);
    h.engine
        .business(receipt(&original, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    let retry = business_outputs(h.engine.drain()).remove(0);
    assert_eq!(retry.operation_id, original.operation_id);
    assert_eq!(retry.body, original.body);
    let outcome = h.store.execute(&retry).unwrap();
    h.engine.business(receipt(&retry, outcome.clone())).unwrap();
    h.engine.business(receipt(&original, outcome)).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits
        ),
        (102, 30, 1)
    );
    assert!(h.engine.pending_for_test().is_empty());
}

#[test]
fn b1_k09_reconciled_admission_after_close_is_refunded_once() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let load = business_outputs(h.engine.drain()).remove(0);
    let facts = h.store.execute(&load).unwrap();
    h.engine.business(receipt(&load, facts)).unwrap();
    let admit = business_outputs(h.engine.drain()).remove(0);
    h.store.execute(&admit).unwrap();
    h.engine
        .business(receipt(&admit, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .business(receipt(&query, ReceiptOutcome::Unknown {}))
        .unwrap();
    let reconcile = reconcile_request(&mut h.engine);
    h.engine
        .close(Close {
            key: create_request().key,
            request_id: "close".into(),
        })
        .unwrap();
    let proof = h.store.execute(&reconcile).unwrap();
    h.engine
        .business(receipt(&reconcile, proof.clone()))
        .unwrap();
    h.pump().unwrap();
    h.engine.business(receipt(&reconcile, proof)).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.years,
            h.store.admission_commits,
            h.engine.live_worlds()
        ),
        (100, 0, 1, 0)
    );
    assert!(h.engine.pending_for_test().is_empty());
    assert!(business_outputs(h.engine.drain()).is_empty());
}

#[test]
fn b1_k09_reconciliation_no_commit_can_close_an_unadmitted_instance() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let load = business_outputs(h.engine.drain()).remove(0);
    let facts = h.store.execute(&load).unwrap();
    h.engine.business(receipt(&load, facts)).unwrap();
    let admit = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .business(receipt(&admit, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = business_outputs(h.engine.drain()).remove(0);
    h.engine
        .business(receipt(&query, ReceiptOutcome::Unknown {}))
        .unwrap();
    let reconcile = reconcile_request(&mut h.engine);
    h.engine
        .business(receipt(&reconcile, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    assert_eq!(
        (h.store.warehouse, h.store.years, h.engine.live_worlds()),
        (100, 0, 0)
    );
    assert!(h.engine.pending_for_test().is_empty());
}
