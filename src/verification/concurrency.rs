use super::*;
use std::sync::{Arc, Barrier};

#[test]
fn abandoned_drain_reply_retains_the_original_frozen_request() {
    let runtime = crate::Runtime::spawn();
    let client = runtime.client();
    client.create(create_request()).unwrap();
    client.abandon_drain_for_test();
    client.create(create_request()).unwrap(); // FIFO barrier after the abandoned receiver.
    let outputs = client.drain_outputs().unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].id, "b1:1:out:1");
    assert!(matches!(
        outputs[0].body,
        OutputBody::BusinessRequest { .. }
    ));
    assert!(client.drain_outputs().unwrap().is_empty());
}

#[test]
fn concurrent_duplicate_creates_generate_only_one_load() {
    let runtime = crate::Runtime::spawn();
    let gate = Arc::new(Barrier::new(16));
    std::thread::scope(|scope| {
        let mut threads = Vec::new();
        for _ in 0..16 {
            let client = runtime.client();
            let gate = gate.clone();
            threads.push(scope.spawn(move || {
                gate.wait();
                client.create(create_request())
            }));
        }
        for thread in threads {
            assert!(thread.join().unwrap().is_ok());
        }
    });
    assert_eq!(runtime.client().drain_outputs().unwrap().len(), 1);
}

#[test]
fn competing_create_identities_cannot_share_a_runtime() {
    let runtime = crate::Runtime::spawn();
    let gate = Arc::new(Barrier::new(8));
    std::thread::scope(|scope| {
        let mut threads = Vec::new();
        for index in 0..8 {
            let client = runtime.client();
            let gate = gate.clone();
            threads.push(scope.spawn(move || {
                let mut request = create_request();
                request.key.id = format!("race-{index}");
                gate.wait();
                client.create(request)
            }));
        }
        let results: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Err(Error::Conflict))
                .count(),
            7
        );
    });
    assert_eq!(runtime.client().drain_outputs().unwrap().len(), 1);
}

#[test]
fn concurrent_receipts_cannot_duplicate_admission() {
    let runtime = crate::Runtime::spawn();
    let client = runtime.client();
    client.create(create_request()).unwrap();
    let output = client.drain_outputs().unwrap().remove(0);
    let OutputBody::BusinessRequest { request } = output.body else {
        panic!("load")
    };
    let mut store = Store {
        warehouse: 100,
        ..Default::default()
    };
    let receipt = BusinessReceipt {
        key: create_request().key,
        operation_id: request.operation_id.clone(),
        attempt_id: request.attempt_id,
        outcome: store.execute(&request).unwrap(),
    };
    let gate = Arc::new(Barrier::new(16));
    std::thread::scope(|scope| {
        let mut threads = Vec::new();
        for _ in 0..16 {
            let client = client.clone();
            let receipt = receipt.clone();
            let gate = gate.clone();
            threads.push(scope.spawn(move || {
                gate.wait();
                client.submit_business_receipt(receipt)
            }));
        }
        for thread in threads {
            assert!(thread.join().unwrap().is_ok());
        }
    });
    let outputs = client.drain_outputs().unwrap();
    assert_eq!(
        outputs
            .iter()
            .filter(|output| matches!(
                output.body,
                OutputBody::BusinessRequest {
                    request: BusinessRequest {
                        body: Business::CommitAdmission { .. },
                        ..
                    }
                }
            ))
            .count(),
        1
    );
}

#[test]
fn saturated_outputs_preserve_the_frozen_settlement_after_world_release() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    for _ in 0..3000 {
        h.engine.tick();
    }
    assert_eq!(h.engine.live_worlds(), 0);
    assert!(h.engine.queued_output_count() <= h.engine.config.output_capacity as u32);
    let outputs = h.engine.drain();
    let frozen: Vec<_> = outputs
        .iter()
        .filter_map(|output| match &output.body {
            OutputBody::BusinessRequest { request }
                if matches!(request.body, Business::CommitSettlement { .. }) =>
            {
                Some(request.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(frozen.len(), 1);
    let receipt = BusinessReceipt {
        key: create_request().key,
        operation_id: frozen[0].operation_id.clone(),
        attempt_id: frozen[0].attempt_id,
        outcome: h.store.execute(&frozen[0]).unwrap(),
    };
    h.engine.business(receipt.clone()).unwrap();
    h.engine.business(receipt).unwrap();
    assert_eq!(h.store.settlement_commits, 1);
    assert_eq!(h.engine.pending_count(), 0);
    assert_eq!(h.engine.live_worlds(), 0);
}
