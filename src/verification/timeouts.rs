use super::*;

fn requests(h: &mut Harness) -> Vec<BusinessRequest> {
    h.engine
        .drain()
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
fn repair(h: &mut Harness, original: &BusinessRequest, manual: &BusinessRequest) {
    let ReceiptOutcome::Committed {
        revision,
        commit_id,
    } = h.store.execute(original).unwrap()
    else {
        panic!("controlled commit proof")
    };
    h.engine
        .business(receipt(
            manual,
            ReceiptOutcome::Reconciled {
                revision,
                commit_id,
            },
        ))
        .unwrap();
}
fn admission() -> (Harness, BusinessRequest) {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let load = requests(&mut h).remove(0);
    let facts = h.store.execute(&load).unwrap();
    h.engine.business(receipt(&load, facts)).unwrap();
    let admit = requests(&mut h).remove(0);
    (h, admit)
}
fn settlement() -> (Harness, BusinessRequest) {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    h.action(Action::Leave {}).unwrap();
    let request = requests(&mut h)
        .into_iter()
        .find(|r| matches!(r.body, Business::CommitSettlement { .. }))
        .unwrap();
    (h, request)
}

#[test]
fn silent_load_expires_at_the_service_deadline_without_admission_or_late_resurrection() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let load = requests(&mut h).remove(0);
    h.engine.poll_io(4999);
    assert_eq!(h.engine.status().phase, Phase::Preparing);
    h.engine.poll_io(5000);
    assert_eq!(
        (
            h.engine.status().phase,
            h.engine.status().logical_ms,
            h.engine.pending_count()
        ),
        (Phase::Closed, 0, 0)
    );
    let facts = h.store.execute(&load).unwrap();
    assert_eq!(
        h.engine.business(receipt(&load, facts)),
        Err(Error::Conflict)
    );
    assert!(requests(&mut h).is_empty());
    assert_eq!(h.store.admission_commits, 0);
}

#[test]
fn gameplay_ticks_do_not_spend_the_monotonic_io_budget() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    for _ in 0..1000 {
        h.engine.tick();
    }
    assert_eq!(h.engine.status().phase, Phase::Preparing);
    h.engine.poll_io(4999);
    assert_eq!(h.engine.status().phase, Phase::Preparing);
    h.engine.poll_io(5000);
    assert_eq!(h.engine.status().phase, Phase::Closed);
}

#[test]
fn admission_timeout_queries_then_compensates_a_late_single_commit() {
    let (mut h, admit) = admission();
    h.engine.poll_io(5000);
    let query = requests(&mut h).remove(0);
    assert!(matches!(query.body, Business::QueryOperation { .. }));
    assert_eq!(h.engine.status().phase, Phase::Closed);
    let committed = h.store.execute(&admit).unwrap();
    h.engine
        .business(receipt(&admit, committed.clone()))
        .unwrap();
    h.engine.business(receipt(&admit, committed)).unwrap();
    h.pump().unwrap();
    assert_eq!(
        (
            h.store.admission_commits,
            h.store.warehouse,
            h.store.years,
            h.engine.live_worlds(),
            h.engine.pending_count()
        ),
        (1, 100, 0, 0, 0)
    );
}

#[test]
fn fenced_no_commit_after_admission_timeout_never_debits_again() {
    let (mut h, _) = admission();
    h.engine.poll_io(5000);
    let query = requests(&mut h).remove(0);
    h.engine
        .business(receipt(&query, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    assert!(requests(&mut h).is_empty());
    assert_eq!(
        (
            h.engine.pending_count(),
            h.store.admission_commits,
            h.store.warehouse
        ),
        (0, 0, 100)
    );
}

#[test]
fn draining_deadlines_run_with_frozen_gameplay_and_late_settlement_applies_once() {
    let (mut h, settle) = settlement();
    let time = h.engine.status().logical_ms;
    h.engine.poll_io(4999);
    assert_eq!(h.engine.status().phase, Phase::Draining);
    h.engine.poll_io(5000);
    let query = requests(&mut h).remove(0);
    assert_eq!(
        (
            h.engine.status().phase,
            h.engine.live_worlds(),
            h.engine.status().logical_ms
        ),
        (Phase::Closed, 0, time)
    );
    let committed = h.store.execute(&settle).unwrap();
    h.engine
        .business(receipt(&settle, committed.clone()))
        .unwrap();
    h.engine.business(receipt(&settle, committed)).unwrap();
    assert_eq!(
        h.engine
            .business(receipt(&query, ReceiptOutcome::NotCommitted {})),
        Err(Error::Conflict)
    );
    assert_eq!(
        (
            h.store.settlement_commits,
            h.store.warehouse,
            h.store.cultivation,
            h.engine.pending_count()
        ),
        (1, 99, 3, 0)
    );
    assert!(requests(&mut h).is_empty());
}

#[test]
fn silent_query_escalates_once_and_late_negative_facts_cannot_restart_writes() {
    let (mut h, settle) = settlement();
    h.engine.poll_io(5000);
    let query = requests(&mut h).remove(0);
    h.engine.poll_io(9999);
    assert!(requests(&mut h).is_empty());
    h.engine.poll_io(10000);
    let manual = requests(&mut h).remove(0);
    assert!(matches!(manual.body, Business::ReconcileOperation { .. }));
    h.engine.poll_io(100000);
    assert!(requests(&mut h).is_empty());
    h.engine
        .business(receipt(&query, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    h.engine
        .business(receipt(&settle, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    h.engine
        .business(receipt(&settle, ReceiptOutcome::Refused {}))
        .unwrap();
    assert!(requests(&mut h).is_empty());
    assert_eq!(h.engine.pending_count(), 2);
    repair(&mut h, &settle, &manual);
    assert_eq!(
        (
            h.engine.pending_count(),
            h.store.settlement_commits,
            h.store.warehouse
        ),
        (0, 1, 99)
    );
}

#[test]
fn a_negative_query_for_a_previous_attempt_cannot_skip_the_current_attempt() {
    let (mut h, original) = settlement();
    h.engine
        .business(receipt(&original, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = requests(&mut h).remove(0);
    h.engine
        .business(receipt(&original, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    let second = requests(&mut h).remove(0);
    assert_eq!(second.attempt_id, 2);
    assert_eq!(second.operation_id, original.operation_id);
    assert_eq!(second.body, original.body);
    h.engine
        .business(receipt(&query, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    assert!(requests(&mut h).is_empty());
    h.engine.poll_io(5000);
    let next_query = requests(&mut h).remove(0);
    assert!(matches!(next_query.body, Business::QueryOperation { .. }));
    assert!(next_query.operation_id.ends_with("-2"));
}

#[test]
fn invalid_receipts_and_backward_clock_samples_do_not_extend_a_deadline() {
    let (mut h, settle) = settlement();
    h.engine.poll_io(4000);
    assert_eq!(
        h.engine.business(receipt(
            &settle,
            ReceiptOutcome::Committed {
                revision: 77,
                commit_id: "bad".into()
            }
        )),
        Err(Error::RevisionConflict)
    );
    h.engine.poll_io(1000);
    h.engine.poll_io(4999);
    assert_eq!(h.engine.status().phase, Phase::Draining);
    h.engine.poll_io(5000);
    assert_eq!(h.engine.status().phase, Phase::Closed);
    assert_eq!(requests(&mut h).len(), 1);
}

#[test]
fn manual_reconciliation_does_not_hold_a_draining_world_forever() {
    let (mut h, settle) = settlement();
    h.engine
        .business(receipt(&settle, ReceiptOutcome::Refused {}))
        .unwrap();
    let manual = requests(&mut h).remove(0);
    h.engine.poll_io(14999);
    assert_eq!(h.engine.live_worlds(), 1);
    h.engine.poll_io(15000);
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(
        (h.engine.pending_count(), h.store.settlement_commits),
        (2, 0)
    );
    repair(&mut h, &settle, &manual);
    assert_eq!(
        (
            h.engine.pending_count(),
            h.store.settlement_commits,
            h.store.cultivation
        ),
        (0, 1, 3)
    );
}

#[test]
fn compensation_deadlines_continue_after_the_world_is_closed() {
    let (mut h, admit) = admission();
    let committed = h.store.execute(&admit).unwrap();
    h.engine
        .close(Close {
            key: create_request().key,
            request_id: "early".into(),
        })
        .unwrap();
    h.engine.business(receipt(&admit, committed)).unwrap();
    let compensate = requests(&mut h).remove(0);
    h.engine.poll_io(5000);
    let query = requests(&mut h).remove(0);
    h.engine
        .business(receipt(&query, ReceiptOutcome::Unknown {}))
        .unwrap();
    let manual = requests(&mut h).remove(0);
    repair(&mut h, &compensate, &manual);
    assert!(matches!(
        compensate.body,
        Business::CompensateAdmission { .. }
    ));
    assert_eq!(
        (
            h.store.warehouse,
            h.store.years,
            h.engine.live_worlds(),
            h.engine.pending_count()
        ),
        (100, 0, 0, 0)
    );
}

#[test]
fn unknown_admission_reconciliation_releases_preparing_and_keeps_the_obligation() {
    let (mut h, admit) = admission();
    h.engine
        .business(receipt(&admit, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = requests(&mut h).remove(0);
    h.engine
        .business(receipt(&query, ReceiptOutcome::Unknown {}))
        .unwrap();
    let manual = requests(&mut h).remove(0);
    assert_eq!(
        (h.engine.status().phase, h.engine.live_worlds()),
        (Phase::Closed, 0)
    );
    h.engine
        .business(receipt(&query, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    assert_eq!(h.engine.pending_count(), 2);
    repair(&mut h, &admit, &manual);
    h.pump().unwrap();
    assert_eq!(
        (h.store.warehouse, h.store.years, h.engine.pending_count()),
        (100, 0, 0)
    );
}

#[test]
fn initial_deployment_without_a_client_is_bounded_and_compensated() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    h.pump().unwrap();
    assert_eq!(
        (h.engine.status().phase, h.store.warehouse),
        (Phase::Deploying, 95)
    );
    h.advance(29970).unwrap();
    assert_eq!(h.engine.live_worlds(), 1);
    h.advance(30).unwrap();
    assert_eq!(
        (
            h.engine.status().phase,
            h.engine.live_worlds(),
            h.store.warehouse,
            h.store.years,
            h.store.settlement_commits
        ),
        (Phase::Closed, 0, 100, 0, 0)
    );
}

#[test]
fn preparing_rebind_fences_the_old_page_and_preserves_the_pending_admission() {
    let (mut h, admit) = admission();
    let old = create_request().connection;
    let current = Connection {
        id: "reload-during-io".into(),
        generation: 2,
    };
    let mut sequence = 0;
    let mut send = |engine: &mut Engine, connection: &Connection, action| {
        sequence += 1;
        engine.input(Input {
            key: create_request().key,
            participant_id: "A".into(),
            entity_generation: 1,
            connection: connection.clone(),
            request_id: format!("pending-{sequence}"),
            sequence,
            action,
        })
    };
    send(
        &mut h.engine,
        &current,
        Action::Reconnect {
            previous_connection: old.clone(),
            authenticated_principal: "principal-A".into(),
        },
    )
    .unwrap();
    let proof = h.store.execute(&admit).unwrap();
    h.engine.business(receipt(&admit, proof)).unwrap();
    assert_eq!(
        send(&mut h.engine, &old, Action::Join {}),
        Err(Error::StaleConnection)
    );
    send(&mut h.engine, &current, Action::Join {}).unwrap();
    let output = h
        .engine
        .drain()
        .into_iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap();
    h.engine
        .transport(TransportReceipt {
            key: create_request().key,
            output_id: output.id.clone(),
            recipient_id: "A".into(),
            connection: current.clone(),
            outcome: TransportOutcome::Sent,
        })
        .unwrap();
    assert_eq!(
        send(
            &mut h.engine,
            &current,
            Action::CombatReady {
                publication_id: output.id
            }
        ),
        Err(Error::NotReady)
    );
    send(&mut h.engine, &current, Action::Foreground {}).unwrap();
    let output = h
        .engine
        .drain()
        .into_iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap();
    h.engine
        .transport(TransportReceipt {
            key: create_request().key,
            output_id: output.id.clone(),
            recipient_id: "A".into(),
            connection: current.clone(),
            outcome: TransportOutcome::Sent,
        })
        .unwrap();
    send(
        &mut h.engine,
        &current,
        Action::CombatReady {
            publication_id: output.id,
        },
    )
    .unwrap();
    h.engine.tick();
    assert_eq!(
        (
            h.engine.status().phase,
            h.store.admission_commits,
            h.store.warehouse
        ),
        (Phase::Active, 1, 95)
    );
}

#[test]
fn output_backpressure_cannot_pin_a_draining_world() {
    let (mut h, settle) = settlement();
    let capacity = h.engine.config.output_capacity;
    // Model only fifteen available output slots: too little for normal dispatch,
    // but still enough for the existing reserved close path.
    h.engine.config.output_capacity = h.engine.queued_output_count() as usize + 15;
    h.engine.poll_io(5000);
    assert_eq!(
        (h.engine.status().phase, h.engine.live_worlds()),
        (Phase::Closed, 0)
    );
    assert_eq!(h.engine.pending_count(), 1);
    h.engine.drain();
    h.engine.config.output_capacity = capacity;
    h.engine.poll_io(5000);
    let query = requests(&mut h).remove(0);
    assert!(matches!(query.body, Business::QueryOperation { .. }));
    let proof = h.store.execute(&settle).unwrap();
    h.engine.business(receipt(&settle, proof)).unwrap();
    assert_eq!(
        (
            h.engine.pending_count(),
            h.store.settlement_commits,
            h.store.warehouse
        ),
        (0, 1, 99)
    );

    let (mut h, settle) = settlement();
    h.engine
        .business(receipt(&settle, ReceiptOutcome::Refused {}))
        .unwrap();
    let manual = requests(&mut h).remove(0);
    let capacity = h.engine.config.output_capacity;
    h.engine.config.output_capacity = 15;
    h.engine.poll_io(15000);
    assert_eq!(
        (
            h.engine.status().phase,
            h.engine.live_worlds(),
            h.engine.pending_count()
        ),
        (Phase::Closed, 0, 2)
    );
    h.engine.drain();
    h.engine.config.output_capacity = capacity;
    repair(&mut h, &settle, &manual);
    assert_eq!(h.engine.pending_count(), 0);

    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    requests(&mut h);
    let capacity = h.engine.config.output_capacity;
    h.engine.config.output_capacity = 15;
    h.engine.poll_io(5000);
    assert_eq!(h.engine.status().phase, Phase::Closed);
    h.engine.drain();
    h.engine.config.output_capacity = capacity;
    h.engine.poll_io(5000);
    assert_eq!(
        (h.engine.pending_count(), h.store.admission_commits),
        (0, 0)
    );
}

#[test]
fn output_pressure_defers_but_does_not_lose_an_io_timeout() {
    let mut h = Harness::new();
    h.start().unwrap();
    for _ in 0..3000 {
        h.engine.tick();
    }
    assert_eq!(h.engine.status().phase, Phase::Faulted);
    let queued = h.engine.queued_output_count();
    assert!(queued as usize + 16 > h.engine.config.output_capacity);
    h.engine.poll_io(5000);
    assert_eq!(h.engine.queued_output_count(), queued);
    let settle = requests(&mut h)
        .into_iter()
        .find(|r| matches!(r.body, Business::CommitSettlement { .. }))
        .unwrap();
    h.engine.poll_io(5000);
    let query = requests(&mut h).remove(0);
    assert!(matches!(query.body, Business::QueryOperation { .. }));
    assert_eq!(h.engine.status().phase, Phase::Faulted);
    let proof = h.store.execute(&settle).unwrap();
    h.engine.business(receipt(&settle, proof)).unwrap();
    assert_eq!(
        (h.engine.pending_count(), h.store.settlement_commits),
        (0, 1)
    );
}

#[test]
fn runtime_expires_silent_io_without_host_clock_or_phase_calls() {
    use std::time::{Duration, Instant};
    let runtime = crate::Runtime::spawn();
    let client = runtime.client();
    client.create(create_request()).unwrap();
    let started = Instant::now();
    let mut expired = false;
    let mut closed = false;
    while started.elapsed() < Duration::from_secs(7) && !closed {
        for output in client.drain_outputs().unwrap() {
            expired |= matches!(
                output.body,
                OutputBody::Event {
                    event: Event::IoTimedOut { .. }
                }
            );
            closed |= matches!(
                output.body,
                OutputBody::Event {
                    event: Event::Closed { live_worlds: 0, .. }
                }
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(expired && closed);
    assert!(started.elapsed() >= Duration::from_millis(4900));
}
