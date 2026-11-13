use super::*;

#[test]
fn interactive_publication_cadence_preserves_authoritative_gameplay() {
    fn exercise(mode: PublicationMode) -> (Snapshot, Vec<u64>) {
        let mut h = Harness::new();
        let mut request = create_request();
        request.publication_mode = mode;
        h.engine.create(request).unwrap();
        h.pump().unwrap();
        h.action(Action::Join {}).unwrap();
        h.pump().unwrap();
        h.advance(30).unwrap();
        h.action(Action::Move {
            direction: [0.0, -1.0],
        })
        .unwrap();
        h.pump().unwrap();
        h.trace.clear();
        h.advance(1500).unwrap();
        let times = h
            .trace
            .iter()
            .filter_map(|output| {
                matches!(output.body, OutputBody::Publication { .. }).then_some(output.logical_ms)
            })
            .collect();
        let snapshot = h
            .engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(&h.engine.config, h.engine.status().logical_ms);
        (snapshot, times)
    }
    let (batched, slow) = exercise(PublicationMode::Batched);
    let (interactive, fast) = exercise(PublicationMode::Interactive);
    assert_eq!(
        serde_json::to_value(batched).unwrap(),
        serde_json::to_value(interactive).unwrap()
    );
    assert_eq!(fast.len(), 50);
    assert!(fast.windows(2).all(|pair| pair[1] - pair[0] == 30));
    assert!(slow.len() < 10);
}

#[test]
fn publication_mode_is_optional_finite_and_part_of_create_identity() {
    let mut wire = serde_json::to_value(create_request()).unwrap();
    wire.as_object_mut().unwrap().remove("publication_mode");
    assert_eq!(
        serde_json::from_value::<Create>(wire.clone())
            .unwrap()
            .publication_mode,
        PublicationMode::Batched
    );
    wire["publication_mode"] = serde_json::json!("arbitrary_tick");
    assert!(serde_json::from_value::<Create>(wire).is_err());
    let mut h = Harness::new();
    h.start().unwrap();
    let mut request = create_request();
    request.publication_mode = PublicationMode::Interactive;
    assert_eq!(h.engine.create(request), Err(Error::Conflict));
}

fn next_business(engine: &mut Engine) -> Output {
    engine
        .drain()
        .into_iter()
        .find(|output| matches!(output.body, OutputBody::BusinessRequest { .. }))
        .expect("business output")
}

fn admission_pending() -> (Engine, Store, BusinessRequest) {
    let mut engine = Engine::new(Config::frozen());
    let mut store = Store {
        warehouse: 100,
        ..Store::default()
    };
    engine.create(create_request()).unwrap();
    let OutputBody::BusinessRequest { request } = next_business(&mut engine).body else {
        panic!("load request")
    };
    let outcome = store.execute(&request).unwrap();
    engine
        .business(BusinessReceipt {
            key: create_request().key,
            operation_id: request.operation_id,
            attempt_id: 1,
            outcome,
        })
        .unwrap();
    let OutputBody::BusinessRequest { request } = next_business(&mut engine).body else {
        panic!("admission request")
    };
    (engine, store, request)
}

fn receipt(request: &BusinessRequest, outcome: ReceiptOutcome) -> BusinessReceipt {
    BusinessReceipt {
        key: create_request().key,
        operation_id: request.operation_id.clone(),
        attempt_id: request.attempt_id,
        outcome,
    }
}

#[test]
fn extraction_and_death_have_exact_asset_results() {
    let extraction = run(Outcome::Extraction).unwrap();
    assert_eq!(
        (
            extraction.warehouse,
            extraction.cultivation,
            extraction.charged_entry_years
        ),
        (102, 30, 1)
    );
    let death = run(Outcome::Death).unwrap();
    assert_eq!((death.warehouse, death.cultivation), (99, 3));
}

#[test]
fn fixed_seed_replays_and_one_hundred_worlds_close() {
    let first = run(Outcome::Extraction).unwrap();
    assert_eq!(first, run(Outcome::Extraction).unwrap());
    for _ in 0..100 {
        assert_eq!(run(Outcome::Death).unwrap().live_worlds, 0);
    }
}

#[test]
fn movement_is_normalized_and_one_step_is_exact() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Move {
        direction: [3.0, 4.0],
    })
    .unwrap();
    h.advance(30).unwrap();
    let position = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
        .position;
    assert!((position[0] - 3.24).abs() < 1e-6 && (position[1] - 4.32).abs() < 1e-6);
}

#[test]
fn movement_cannot_cross_walls_or_bounds() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    h.action(Action::Move {
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(3000).unwrap();
    assert!(h.snapshot.as_ref().unwrap().position[0] <= 204.0);
    h.action(Action::Move {
        direction: [-1.0, 0.0],
    })
    .unwrap();
    h.advance(6000).unwrap();
    assert!(h.snapshot.as_ref().unwrap().position[0] >= -496.0);
}

#[test]
fn invalid_numbers_and_old_identity_do_not_mutate_world() {
    let mut h = Harness::new();
    h.start().unwrap();
    assert_eq!(
        h.action(Action::Move {
            direction: [f64::NAN, 0.0]
        }),
        Err(Error::InvalidMessage)
    );
    let mut input = Input {
        key: create_request().key,
        participant_id: "A".into(),
        entity_generation: 1,
        connection: Connection {
            id: "old".into(),
            generation: 0,
        },
        request_id: "old".into(),
        sequence: 99,
        action: Action::Move {
            direction: [1.0, 0.0],
        },
    };
    assert_eq!(h.engine.input(input.clone()), Err(Error::StaleConnection));
    input.connection = create_request().connection;
    input.participant_id = "B".into();
    assert_eq!(h.engine.input(input), Err(Error::Unauthorized));
    h.advance(300).unwrap();
    assert_eq!(h.snapshot.as_ref().unwrap().position, [0.0, 0.0]);
}

#[test]
fn cost_is_committed_once_and_cancel_before_commit_is_free() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    let input = h.engine.history_for_test();
    assert!(h.engine.input(input).is_ok());
    h.action(Action::CancelAction {}).unwrap();
    h.advance(300).unwrap();
    assert_eq!(h.snapshot.as_ref().unwrap().spirit, 100);
    assert!(!h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::ActionCommitted { skill: true, .. }
        }
    )));
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(300).unwrap();
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::ActionCommitted {
                        skill: true,
                        spirit_cost: 20,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
    assert_eq!(
        h.action(Action::Cast {
            skill_id: "duanjin-shou".into(),
            direction: [1.0, 0.0]
        }),
        Err(Error::InvalidState)
    );
}

#[test]
fn queued_or_failed_publication_cannot_make_ready() {
    let (mut engine, mut store, request) = admission_pending();
    engine
        .business(receipt(&request, store.execute(&request).unwrap()))
        .unwrap();
    let input = Input {
        key: create_request().key,
        participant_id: "A".into(),
        entity_generation: 1,
        connection: create_request().connection,
        request_id: "join".into(),
        sequence: 1,
        action: Action::Join {},
    };
    engine.input(input.clone()).unwrap();
    let publication = engine
        .drain()
        .into_iter()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap();
    let mut ready = input;
    ready.request_id = "ready".into();
    ready.sequence = 2;
    ready.action = Action::CombatReady {
        publication_id: publication.id.clone(),
    };
    let mut transport = TransportReceipt {
        key: create_request().key,
        output_id: publication.id,
        recipient_id: "A".into(),
        connection: create_request().connection,
        outcome: TransportOutcome::Enqueued,
    };
    engine.transport(transport.clone()).unwrap();
    assert_eq!(engine.input(ready.clone()), Err(Error::NotReady));
    transport.outcome = TransportOutcome::Sent;
    engine.transport(transport).unwrap();
    engine.input(ready).unwrap();
    engine.tick();
    assert_eq!(engine.status().phase, Phase::Active);
}

#[test]
fn unknown_admission_is_queried_without_double_debit() {
    let (mut engine, mut store, request) = admission_pending();
    store.execute(&request).unwrap();
    engine
        .business(receipt(&request, ReceiptOutcome::Unknown {}))
        .unwrap();
    let OutputBody::BusinessRequest { request: query } = next_business(&mut engine).body else {
        panic!("query")
    };
    let query_receipt = receipt(&query, store.execute(&query).unwrap());
    engine.business(query_receipt.clone()).unwrap();
    engine.business(query_receipt).unwrap();
    assert_eq!(
        (
            store.warehouse,
            store.admission_commits,
            engine.live_worlds()
        ),
        (95, 1, 1)
    );
}

#[test]
fn not_committed_retries_the_same_operation_id() {
    let (mut engine, mut store, request) = admission_pending();
    engine
        .business(receipt(&request, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    let OutputBody::BusinessRequest { request: retry } = next_business(&mut engine).body else {
        panic!("retry")
    };
    assert_eq!(retry.operation_id, request.operation_id);
    assert_eq!(retry.attempt_id, 2);
    let commit = receipt(&retry, store.execute(&retry).unwrap());
    engine.business(commit.clone()).unwrap();
    engine.business(commit).unwrap();
    assert_eq!((store.warehouse, store.admission_commits), (95, 1));
    assert_eq!(
        engine.business(receipt(&retry, ReceiptOutcome::Refused {})),
        Err(Error::Conflict)
    );
}

#[test]
fn refused_admission_never_injects_inventory() {
    let (mut engine, store, request) = admission_pending();
    engine
        .business(receipt(&request, ReceiptOutcome::Refused {}))
        .unwrap();
    assert_eq!(engine.status().phase, Phase::Closed);
    assert_eq!(engine.live_worlds(), 0);
    assert_eq!(store.warehouse, 100);
}

#[test]
fn late_admission_after_close_is_compensated_without_world() {
    let (mut engine, mut store, request) = admission_pending();
    let commit = receipt(&request, store.execute(&request).unwrap());
    let close = Close {
        key: create_request().key,
        request_id: "close".into(),
    };
    let closed = engine.close(close.clone()).unwrap();
    engine.business(commit.clone()).unwrap();
    engine.business(commit).unwrap();
    let compensation = engine
        .drain()
        .into_iter()
        .find_map(|o| match o.body {
            OutputBody::BusinessRequest { request }
                if matches!(request.body, Business::CompensateAdmission { .. }) =>
            {
                Some(request)
            }
            _ => None,
        })
        .unwrap();
    engine
        .business(receipt(
            &compensation,
            store.execute(&compensation).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        (engine.live_worlds(), store.warehouse, store.years),
        (0, 100, 0)
    );
    assert_eq!(engine.close(close).unwrap(), closed);
}

#[test]
fn active_close_freezes_settlement_and_late_commit_does_not_reopen() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    let close = Close {
        key: create_request().key,
        request_id: "close".into(),
    };
    h.engine.close(close.clone()).unwrap();
    assert_eq!(h.engine.live_worlds(), 0);
    h.pump().unwrap();
    h.engine.close(close).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.settlement_commits,
            h.engine.live_worlds()
        ),
        (99, 1, 0)
    );
}

#[test]
fn early_extraction_is_refused_and_cancel_does_not_settle() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    assert_eq!(h.action(Action::ExtractStart {}), Err(Error::InvalidState));
    h.advance(180000).unwrap();
    h.action(Action::ExtractStart {}).unwrap();
    h.action(Action::ExtractCancel {}).unwrap();
    h.advance(5100).unwrap();
    assert_eq!(h.store.settlement_commits, 0);
    assert_eq!(h.engine.status().phase, Phase::Active);
}

#[test]
fn background_interrupts_and_foreground_does_not_restore_movement() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Move {
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(30).unwrap();
    h.action(Action::Background {}).unwrap();
    h.action(Action::Foreground {}).unwrap();
    h.advance(300).unwrap();
    let view = h.snapshot.as_ref().unwrap();
    assert!(view.protected);
    assert!((view.position[0] - 5.4).abs() < 1e-6 && view.position[1].abs() < 1e-6);
}

#[test]
fn bounded_output_backpressure_faults_only_its_instance() {
    let mut slow = Harness::new();
    slow.start().unwrap();
    let mut other = Harness::new();
    other.start().unwrap();
    for _ in 0..3000 {
        slow.engine.tick();
    }
    assert_eq!(slow.engine.status().phase, Phase::Faulted);
    assert_eq!(slow.engine.live_worlds(), 0);
    other.advance(300).unwrap();
    assert_eq!(other.engine.status().phase, Phase::Active);
}

#[test]
fn live_runtime_ticks_without_host_timers_and_shutdown_joins() {
    let runtime = crate::Runtime::spawn();
    let client = runtime.client();
    client.create(create_request()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let status = client.create(create_request()).unwrap();
    assert!(status.logical_ms >= 60);
    drop(runtime);
    assert_eq!(client.drain_outputs(), Err(Error::Closed));
}

#[test]
fn strict_schema_rejects_unknown_commands_and_fields() {
    assert!(
        serde_json::from_str::<Action>(r#"{"kind":"advance_world","milliseconds":30}"#).is_err()
    );
    assert!(
        serde_json::from_str::<Action>(r#"{"kind":"move","direction":[1,0],"position":[10,0]}"#)
            .is_err()
    );
    assert_eq!(create_request().config_hash, config_hash());
    let mut engine = Engine::new(Config::frozen());
    let mut wrong = create_request();
    wrong.config_hash = "wrong".into();
    assert_eq!(engine.create(wrong), Err(Error::ConfigMismatch));
}

#[test]
fn projectile_does_not_travel_before_its_commit_time() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(120).unwrap();
    assert!(h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::ActionCommitted { skill: true, .. }
        }
    )));
    assert!(!h.trace.iter().any(|o| matches!(&o.body, OutputBody::Event { event: Event::Damage { source, .. } } if source == "participant")));
    h.advance(300).unwrap();
    assert!(h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::Damage { amount: 140, .. }
        }
    )));
}

#[test]
fn wall_intercepts_projectile_before_target() {
    // Named test constructor, never a production field-patch input.
    let mut config = Config::frozen();
    config.target.position = [300.0, 0.0];
    config.target.aggro_range = 0.0;
    let mut h = Harness::new();
    h.engine = Engine::new(config);
    h.start().unwrap();
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(1200).unwrap();
    assert_eq!(
        h.snapshot.as_ref().unwrap().target.as_ref().unwrap().hp,
        200
    );
}

#[test]
fn full_inventory_does_not_consume_ground_drop() {
    let mut config = Config::frozen();
    config.inventory_capacity = 1;
    config.stack_limit = 5;
    let mut h = Harness::new();
    h.engine = Engine::new(config);
    h.start().unwrap();
    h.action(Action::Attack {}).unwrap();
    h.advance(400).unwrap();
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.advance(1000).unwrap();
    let revision = h.snapshot.as_ref().unwrap().inventory_revision;
    assert_eq!(
        h.action(Action::Pickup {
            resource_id: "drop-1".into(),
            expected_revision: revision
        }),
        Err(Error::Capacity)
    );
    h.advance(300).unwrap();
    let view = h.snapshot.as_ref().unwrap();
    assert_eq!(view.inventory_quantity, 5);
    assert!(
        view.resources
            .iter()
            .any(|r| r.id == "drop-1" && r.quantity == 2)
    );
}

#[test]
fn damage_and_leaving_range_interrupt_extraction() {
    let mut config = Config::frozen();
    config.target.position = [40.0, 0.0];
    let mut h = Harness::new();
    h.engine = Engine::new(config);
    h.start().unwrap();
    h.advance(180000).unwrap();
    h.action(Action::ExtractStart {}).unwrap();
    h.advance(5100).unwrap();
    assert_eq!(h.store.settlement_commits, 0);
    assert!(h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::ExtractionInterrupted
        }
    )));

    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    h.advance(180000).unwrap();
    h.action(Action::ExtractStart {}).unwrap();
    h.action(Action::Move {
        direction: [0.0, 1.0],
    })
    .unwrap();
    h.advance(1200).unwrap();
    assert!(
        h.snapshot
            .as_ref()
            .unwrap()
            .extraction_finishes_ms
            .is_none()
    );
    assert_eq!(h.store.settlement_commits, 0);
}

#[test]
fn absolute_terminal_uses_the_same_death_settlement() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    h.advance(915000).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits
        ),
        (99, 3, 1)
    );
    assert_eq!(h.engine.live_worlds(), 0);
}

#[test]
fn semantic_replay_compares_events_and_random_drop_positions() {
    let recorded = || {
        let mut h = Harness::new();
        h.start().unwrap();
        h.fight_and_pickup().unwrap();
        h.action(Action::Leave {}).unwrap();
        h.pump().unwrap();
        h.trace
    };
    assert_eq!(recorded(), recorded());
}

#[test]
fn repeated_unknown_attempts_get_distinct_bounded_queries() {
    let (mut engine, mut store, request) = admission_pending();
    engine
        .business(receipt(&request, ReceiptOutcome::Unknown {}))
        .unwrap();
    let OutputBody::BusinessRequest {
        request: first_query,
    } = next_business(&mut engine).body
    else {
        panic!("query")
    };
    engine
        .business(receipt(&first_query, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    let OutputBody::BusinessRequest { request: retry } = next_business(&mut engine).body else {
        panic!("retry")
    };
    store.execute(&retry).unwrap();
    engine
        .business(receipt(&retry, ReceiptOutcome::Unknown {}))
        .unwrap();
    let OutputBody::BusinessRequest {
        request: second_query,
    } = next_business(&mut engine).body
    else {
        panic!("query")
    };
    assert_ne!(first_query.operation_id, second_query.operation_id);
    engine
        .business(receipt(
            &second_query,
            store.execute(&second_query).unwrap(),
        ))
        .unwrap();
    assert_eq!((store.admission_commits, store.warehouse), (1, 95));
}

#[test]
fn worker_shutdown_releases_each_of_one_hundred_runtime_threads() {
    for _ in 0..100 {
        let runtime = crate::Runtime::spawn();
        let client = runtime.client();
        client.create(create_request()).unwrap();
        drop(runtime);
        assert_eq!(client.drain_outputs(), Err(Error::Closed));
    }
}

#[test]
fn old_attempt_failures_cannot_retry_a_new_attempt() {
    let (mut engine, _, request) = admission_pending();
    engine
        .business(receipt(&request, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    engine.drain();
    assert_eq!(
        engine.business(receipt(&request, ReceiptOutcome::NotCommitted {})),
        Err(Error::InvalidMessage)
    );
    assert!(engine.drain().is_empty());
}

#[test]
fn unknown_read_retries_without_querying_a_write_operation() {
    let mut engine = Engine::new(Config::frozen());
    engine.create(create_request()).unwrap();
    let OutputBody::BusinessRequest { request } = next_business(&mut engine).body else {
        panic!("load")
    };
    engine
        .business(receipt(&request, ReceiptOutcome::Unknown {}))
        .unwrap();
    let OutputBody::BusinessRequest { request: retry } = next_business(&mut engine).body else {
        panic!("retry")
    };
    assert_eq!(retry.operation_id, request.operation_id);
    assert_eq!(retry.attempt_id, 2);
    assert!(matches!(retry.body, Business::LoadParticipant {}));
}

#[test]
fn output_fault_preserves_a_frozen_asset_obligation() {
    let mut h = Harness::new();
    h.start().unwrap();
    for _ in 0..3000 {
        h.engine.tick();
    }
    let settlement = h
        .engine
        .drain()
        .into_iter()
        .find_map(|o| match o.body {
            OutputBody::BusinessRequest { request }
                if matches!(request.body, Business::CommitSettlement { .. }) =>
            {
                Some(request)
            }
            _ => None,
        })
        .expect("fault freezes the carried inventory instead of discarding it");
    let outcome = h.store.execute(&settlement).unwrap();
    h.engine.business(receipt(&settlement, outcome)).unwrap();
    assert_eq!(
        (
            h.store.warehouse,
            h.store.settlement_commits,
            h.engine.live_worlds()
        ),
        (98, 1, 0)
    );
}
