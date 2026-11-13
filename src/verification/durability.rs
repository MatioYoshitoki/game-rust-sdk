use super::*;
use sha2::{Digest, Sha256};

fn checkpoint(outputs: &[Output]) -> IoCheckpoint {
    outputs
        .iter()
        .rev()
        .find_map(|output| output.recovery.as_deref())
        .unwrap()
        .clone()
}
fn restored(cp: IoCheckpoint, generation: u32) -> Engine {
    let mut engine = Engine::new(Config::frozen());
    let mut create = create_request();
    create.recovery = Some(Recovery {
        generation,
        checkpoint: cp,
    });
    engine.create(create).unwrap();
    engine
}
fn requests(outputs: &[Output]) -> Vec<BusinessRequest> {
    outputs
        .iter()
        .filter_map(|output| match &output.body {
            OutputBody::BusinessRequest { request } => Some(request.clone()),
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
fn pending_settlement() -> (Harness, BusinessRequest, IoCheckpoint) {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    h.action(Action::Leave {}).unwrap();
    let outputs = h.engine.drain();
    let request = requests(&outputs)
        .into_iter()
        .find(|request| matches!(request.body, Business::CommitSettlement { .. }))
        .unwrap();
    (h, request, checkpoint(&outputs))
}
fn pump_recovered(engine: &mut Engine, store: &mut Store) -> Vec<Output> {
    let mut trace = Vec::new();
    for _ in 0..20 {
        let outputs = engine.drain();
        if outputs.is_empty() {
            return trace;
        }
        for output in outputs {
            if let OutputBody::BusinessRequest { request } = &output.body {
                let outcome = store.execute(request).unwrap();
                engine.business(receipt(request, outcome)).unwrap();
            }
            trace.push(output);
        }
    }
    panic!("unbounded recovery");
}
fn resign(cp: &mut IoCheckpoint) {
    cp.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&cp.state).unwrap())
    );
}

#[test]
fn interrupted_active_world_refunds_entry_without_restoring_battle_or_unfrozen_loot() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    let cp = checkpoint(&h.trace);
    let mut engine = restored(cp, 1);
    assert_eq!(
        (engine.status().phase, engine.live_worlds()),
        (Phase::Closed, 0)
    );
    let outputs = pump_recovered(&mut engine, &mut h.store);
    assert!(!outputs.iter().any(|output| matches!(
        output.body,
        OutputBody::Event {
            event: Event::SettlementApplied { .. }
        }
    )));
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.years,
            engine.pending_count()
        ),
        (100, 0, 0, 0)
    );
}

#[test]
fn committed_admission_without_receipt_is_queried_and_compensated_once() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let outputs = h.engine.drain();
    let load = requests(&outputs).remove(0);
    let outcome = h.store.execute(&load).unwrap();
    h.engine.business(receipt(&load, outcome)).unwrap();
    let outputs = h.engine.drain();
    let admit = requests(&outputs).remove(0);
    h.store.execute(&admit).unwrap();
    let mut engine = restored(checkpoint(&outputs), 1);
    let trace = pump_recovered(&mut engine, &mut h.store);
    assert!(trace.iter().any(|output| matches!(
        output.body,
        OutputBody::BusinessRequest {
            request: BusinessRequest {
                body: Business::QueryOperation { .. },
                ..
            }
        }
    )));
    assert_eq!(
        (
            h.store.warehouse,
            h.store.years,
            h.store.admission_commits,
            engine.live_worlds(),
            engine.pending_count()
        ),
        (100, 0, 1, 0, 0)
    );
}

#[test]
fn frozen_uncommitted_settlement_keeps_the_exact_payload_and_original_identity() {
    let (mut h, original, cp) = pending_settlement();
    let mut engine = restored(cp, 1);
    let trace = pump_recovered(&mut engine, &mut h.store);
    let retries: Vec<_> = requests(&trace)
        .into_iter()
        .filter(|request| matches!(request.body, Business::CommitSettlement { .. }))
        .collect();
    assert_eq!(retries.len(), 1);
    assert_eq!(retries[0].operation_id, original.operation_id);
    assert_eq!(retries[0].body, original.body);
    assert_eq!(retries[0].attempt_id, 2);
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits,
            engine.pending_count()
        ),
        (99, 3, 1, 0)
    );
}

#[test]
fn committed_settlement_without_receipt_never_writes_the_reward_twice() {
    let (mut h, settle, cp) = pending_settlement();
    h.store.execute(&settle).unwrap();
    let mut engine = restored(cp, 1);
    let trace = pump_recovered(&mut engine, &mut h.store);
    assert!(
        !requests(&trace)
            .iter()
            .any(|request| matches!(request.body, Business::CommitSettlement { .. }))
    );
    assert_eq!(
        (
            h.store.warehouse,
            h.store.cultivation,
            h.store.settlement_commits,
            engine.pending_count()
        ),
        (99, 3, 1, 0)
    );
}

#[test]
fn already_applied_settlement_recovers_closed_without_new_money_or_settlement_events() {
    let (mut h, settle, _) = pending_settlement();
    let proof = h.store.execute(&settle).unwrap();
    h.engine.business(receipt(&settle, proof)).unwrap();
    let cp = checkpoint(&h.engine.drain());
    let mut engine = restored(cp.clone(), 1);
    let trace = pump_recovered(&mut engine, &mut h.store);
    assert!(requests(&trace).is_empty());
    assert!(!trace.iter().any(|output| matches!(
        output.body,
        OutputBody::Event {
            event: Event::SettlementApplied { .. }
        }
    )));
    let mut same = create_request();
    same.recovery = Some(Recovery {
        checkpoint: cp,
        generation: 1,
    });
    engine.create(same).unwrap();
    engine.create(create_request()).unwrap();
    assert_eq!(
        (
            engine.live_worlds(),
            engine.pending_count(),
            h.store.settlement_commits
        ),
        (0, 0, 1)
    );
}

#[test]
fn refused_settlement_stays_manual_and_requires_the_matching_reconciliation_proof() {
    let (mut h, settle, _) = pending_settlement();
    // Controlled storage records the refusal, preserving the original frozen intent.
    h.store.journal.insert(
        settle.operation_id.clone(),
        (settle.body.clone(), ReceiptOutcome::Refused {}),
    );
    h.engine
        .business(receipt(&settle, ReceiptOutcome::Refused {}))
        .unwrap();
    let mut engine = restored(checkpoint(&h.engine.drain()), 1);
    let outputs = engine.drain();
    let request = requests(&outputs).remove(0);
    assert!(matches!(request.body, Business::ReconcileOperation { .. }));
    assert_eq!(
        engine.business(receipt(
            &request,
            ReceiptOutcome::Committed {
                revision: 2,
                commit_id: "wrong-type".into()
            }
        )),
        Err(Error::InvalidMessage)
    );
    let proof = h.store.execute(&request).unwrap();
    engine.business(receipt(&request, proof)).unwrap();
    assert_eq!(
        (
            engine.live_worlds(),
            engine.pending_count(),
            h.store.settlement_commits
        ),
        (0, 0, 1)
    );
}

#[test]
fn abandoned_read_recovery_never_starts_a_new_admission() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    let mut engine = restored(checkpoint(&h.engine.drain()), 1);
    let trace = pump_recovered(&mut engine, &mut h.store);
    assert!(requests(&trace).is_empty());
    assert_eq!(
        (
            engine.status().phase,
            engine.pending_count(),
            h.store.admission_commits
        ),
        (Phase::Closed, 0, 0)
    );
}

#[test]
fn corrupt_cross_match_or_forged_checkpoint_is_rejected_before_any_partial_initialization() {
    let (_, _, original) = pending_settlement();
    let mut cases = Vec::new();
    let mut bad = original.clone();
    bad.sha256 = "corrupt".into();
    cases.push(bad);
    let mut bad = original.clone();
    bad.state.identity_sha256 = "other-owner".into();
    resign(&mut bad);
    cases.push(bad);
    let mut bad = original.clone();
    for op in &mut bad.state.operations {
        if let Business::CommitSettlement { kept, .. } = &mut op.request.body {
            *kept = 10000;
        }
    }
    resign(&mut bad);
    cases.push(bad);
    let mut bad = original.clone();
    bad.state.operations.push(bad.state.operations[0].clone());
    resign(&mut bad);
    cases.push(bad);
    for cp in cases {
        let mut engine = Engine::new(Config::frozen());
        let mut create = create_request();
        create.recovery = Some(Recovery {
            checkpoint: cp,
            generation: 1,
        });
        assert_eq!(engine.create(create), Err(Error::InvalidMessage));
        assert_eq!(engine.live_worlds(), 0);
        assert!(engine.drain().is_empty());
        engine.create(create_request()).unwrap();
    }
}

#[test]
fn repeated_recovery_has_bounded_queries_and_distinct_output_namespaces() {
    let (_, _, mut cp) = pending_settlement();
    let mut previous = std::collections::BTreeSet::new();
    for generation in 1..=100 {
        let mut engine = restored(cp, generation);
        let outputs = engine.drain();
        for output in &outputs {
            assert!(previous.insert(output.id.clone()));
        }
        assert_eq!(requests(&outputs).len(), 1);
        cp = checkpoint(&outputs);
        assert!(cp.state.operations.len() <= 4);
    }
    let mut engine = Engine::new(Config::frozen());
    let mut create = create_request();
    create.recovery = Some(Recovery {
        checkpoint: cp,
        generation: 100,
    });
    assert_eq!(engine.create(create), Err(Error::InvalidMessage));
}
