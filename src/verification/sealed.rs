use super::*;
fn harness() -> Harness {
    let c = crate::config::default_catalog().active("b3-3").unwrap();
    let mut h = Harness::new();
    h.engine = Engine::new(c.as_ref().clone());
    let mut create = create_request();
    create.config_id = c.id().into();
    create.config_hash = c.content_hash().into();
    h.engine.create(create).unwrap();
    h.pump().unwrap();
    h.action(Action::Join {}).unwrap();
    h.pump().unwrap();
    h.advance(30).unwrap();
    h.engine.world.as_mut().unwrap().position = c.pve.as_ref().unwrap().chest_spawns[0].position;
    h
}
fn open() -> Action {
    Action::OpenChest {
        chest_id: "sealed-cache-1".into(),
        expected_generation: 1,
    }
}
fn last_open(h: &Harness) -> Input {
    let c = create_request();
    Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: c.connection,
        request_id: format!("r-{}", h.sequence),
        sequence: h.sequence,
        action: open(),
    }
}
#[test]
fn duplicate_start_is_idempotent_without_progress_clock_reset() {
    let mut h = harness();
    h.action(open()).unwrap();
    let request = last_open(&h);
    let result = h.engine.input(request.clone()).unwrap();
    h.advance(600).unwrap();
    h.engine.input(request.clone()).unwrap();
    h.advance(600).unwrap();
    assert_eq!(h.engine.status().phase, result.phase);
    h.pump().unwrap();
    assert_eq!(
        h.snapshot.unwrap().pve.unwrap().chests[0]
            .seal
            .as_ref()
            .unwrap()
            .completed_layers,
        1
    );
    let mut conflict = request;
    conflict.action = Action::CancelChestUnlock {
        chest_id: "sealed-cache-1".into(),
        expected_generation: 1,
    };
    assert_eq!(h.engine.input(conflict), Err(Error::Conflict));
}
#[test]
fn close_releases_active_unlock_without_loot_or_extra_asset_effect() {
    let mut h = harness();
    h.action(open()).unwrap();
    h.advance(600).unwrap();
    let close = Close {
        key: create_request().key,
        request_id: "close".into(),
    };
    h.engine.close(close.clone()).unwrap();
    h.engine.close(close).unwrap();
    h.pump().unwrap();
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(h.store.settlement_commits, 1);
    assert!(h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::ChestUnlockInterrupted {
                reason: ChestInterruptReason::Closed,
                ..
            }
        }
    )));
    assert!(h.trace.iter().all(|o| !matches!(
        o.body,
        OutputBody::Event {
            event: Event::ChestOpened { .. }
        }
    )));
}
#[test]
fn cancel_wire_rejects_patches_invalid_ids_and_stale_connections() {
    for v in [
        serde_json::json!({"kind":"cancel_chest_unlock","chest_id":"sealed-cache-1","expected_generation":1,"progress":999}),
        serde_json::json!({"kind":"cancel_chest_unlock","chest_id":"sealed-cache-1"}),
    ] {
        assert!(serde_json::from_value::<Action>(v).is_err());
    }
    let mut h = harness();
    h.action(open()).unwrap();
    let mut i = last_open(&h);
    i.sequence += 1;
    i.request_id = "bad-cancel".into();
    i.action = Action::CancelChestUnlock {
        chest_id: "sealed-cache-1".into(),
        expected_generation: 0,
    };
    assert_eq!(h.engine.input(i.clone()), Err(Error::InvalidMessage));
    i.action = Action::CancelChestUnlock {
        chest_id: "sealed-cache-1".into(),
        expected_generation: 1,
    };
    i.connection.generation += 1;
    assert_eq!(h.engine.input(i), Err(Error::StaleConnection));
}
#[test]
fn frozen_sealed_loot_recovers_closed_with_original_payload() {
    let mut h = harness();
    h.action(open()).unwrap();
    h.advance(2400).unwrap();
    h.action(open()).unwrap();
    h.pump().unwrap();
    let s = h.snapshot.as_ref().unwrap();
    let resource = s.resources.iter().find(|r| !r.orb).unwrap().id.clone();
    let revision = s.inventory_revision;
    h.action(Action::Pickup {
        resource_id: resource,
        expected_revision: revision,
    })
    .unwrap();
    h.action(Action::Leave {}).unwrap();
    let cp = h
        .engine
        .drain()
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone();
    let frozen = cp
        .state
        .operations
        .iter()
        .find(|o| matches!(o.request.body, Business::CommitSettlement { .. }))
        .unwrap()
        .request
        .clone();
    let c = crate::config::default_catalog().active("b3-3").unwrap();
    let mut create = create_request();
    create.config_id = c.id().into();
    create.config_hash = c.content_hash().into();
    create.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    let mut engine = Engine::new(Config::frozen());
    engine.create(create).unwrap();
    assert_eq!(engine.live_worlds(), 0);
    assert_eq!(engine.status().phase, Phase::Closed);
    assert!(
        engine
            .drain()
            .iter()
            .filter_map(|o| o.recovery.as_deref())
            .any(|cp| cp.state.operations.iter().any(|op| op.request == frozen))
    );
}
