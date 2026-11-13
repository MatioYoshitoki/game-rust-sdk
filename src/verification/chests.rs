use super::*;

fn harness() -> Harness {
    let c = crate::config::default_catalog().active("b3-2").unwrap();
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
fn input(_h: &Harness, id: &str, sequence: u32) -> Input {
    Input {
        key: create_request().key,
        participant_id: "A".into(),
        entity_generation: 1,
        connection: create_request().connection,
        request_id: id.into(),
        sequence,
        action: Action::OpenChest {
            chest_id: "forest-pouch-1".into(),
            expected_generation: 1,
        },
    }
}

#[test]
fn identical_public_input_opens_one_generation_exactly_once() {
    let mut h = harness();
    let input = input(&h, "open-once", h.sequence + 1);
    let first = h.engine.input(input.clone()).unwrap();
    assert_eq!(h.engine.input(input.clone()).unwrap(), first);
    h.pump().unwrap();
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::ChestOpened { .. }
                }
            ))
            .count(),
        1
    );
    let before = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms);
    let mut retry = input;
    retry.request_id = "another-open".into();
    retry.sequence += 1;
    assert_eq!(h.engine.input(retry), Err(Error::NoResource));
    assert_eq!(
        h.engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(&h.engine.config, h.engine.status().logical_ms),
        before
    );
}

#[test]
fn invalid_payload_and_old_connection_cannot_open_after_handover() {
    let mut legacy = Harness::new();
    legacy.start().unwrap();
    assert_eq!(
        legacy.action(Action::OpenChest {
            chest_id: "forest-pouch-1".into(),
            expected_generation: 1
        }),
        Err(Error::Unsupported)
    );
    let mut h = harness();
    let original = input(&h, "open-old", h.sequence + 1);
    for (id, generation) in [
        ("".into(), 1),
        ("x".repeat(65), 1),
        ("forest-pouch-1".into(), 0),
    ] {
        let mut bad = original.clone();
        bad.action = Action::OpenChest {
            chest_id: id,
            expected_generation: generation,
        };
        assert_eq!(h.engine.input(bad), Err(Error::InvalidMessage));
    }
    let mut reconnect = original.clone();
    reconnect.connection = Connection {
        id: "new-connection".into(),
        generation: 2,
    };
    reconnect.request_id = "handover".into();
    reconnect.action = Action::Reconnect {
        previous_connection: original.connection.clone(),
        authenticated_principal: "principal-A".into(),
    };
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(original), Err(Error::StaleConnection));
    assert!(
        !h.engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(&h.engine.config, h.engine.status().logical_ms)
            .pve
            .unwrap()
            .chests[0]
            .opened
    );
}

fn pending_chest_settlement() -> IoCheckpoint {
    let mut h = harness();
    h.action(Action::OpenChest {
        chest_id: "forest-pouch-1".into(),
        expected_generation: 1,
    })
    .unwrap();
    h.pump().unwrap();
    while let Some(resource) = h.snapshot.as_ref().unwrap().resources.first().cloned() {
        let revision = h.snapshot.as_ref().unwrap().inventory_revision;
        h.action(Action::Pickup {
            resource_id: resource.id,
            expected_revision: revision,
        })
        .unwrap();
        h.pump().unwrap();
    }
    h.action(Action::Leave {}).unwrap();
    h.engine
        .drain()
        .iter()
        .rev()
        .find_map(|output| output.recovery.as_deref())
        .unwrap()
        .clone()
}
fn recovery_create(cp: IoCheckpoint) -> Create {
    let c = crate::config::default_catalog().active("b3-2").unwrap();
    let mut create = create_request();
    create.config_id = c.id().into();
    create.config_hash = c.content_hash().into();
    create.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    create
}

#[test]
fn chest_only_frozen_settlement_restores_without_world_or_kill_requirement() {
    let cp = pending_chest_settlement();
    let mut engine = Engine::new(Config::frozen());
    engine.create(recovery_create(cp)).unwrap();
    assert_eq!(engine.live_worlds(), 0);
    assert_eq!(engine.status().phase, Phase::Closed);
    assert!(engine.pending_count() > 0);
    assert!(engine.drain().iter().any(|o|matches!(&o.body,OutputBody::BusinessRequest{request} if matches!(request.body,Business::QueryOperation{..}))));
}

#[test]
fn impossible_chest_quantities_are_rejected_even_with_recomputed_digest() {
    use sha2::{Digest, Sha256};
    let mut cp = pending_chest_settlement();
    let op = cp
        .state
        .operations
        .iter_mut()
        .find(|op| matches!(op.request.body, Business::CommitSettlement { .. }))
        .unwrap();
    let Business::CommitSettlement {
        outcome,
        carried,
        looted,
        kept,
        dropped,
        cultivation,
        items,
        ..
    } = &mut op.request.body
    else {
        panic!()
    };
    *outcome = Outcome::Extraction;
    *kept = *carried + *looted;
    *dropped = 0;
    *cultivation = 22;
    for item in items {
        item.looted += 100;
        item.kept = item.looted;
        item.dropped = 0;
    }
    cp.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&cp.state).unwrap())
    );
    let mut engine = Engine::new(Config::frozen());
    assert_eq!(
        engine.create(recovery_create(cp)),
        Err(Error::InvalidMessage)
    );
    assert_eq!(engine.live_worlds(), 0);
}

#[test]
fn item_bounds_include_only_supported_chests_and_bounded_respawn_cycles() {
    let c = crate::config::default_catalog().active("b3-2").unwrap();
    assert_eq!(c.pve_loot_bound("huo-ling", 0, 0), 4);
    assert_eq!(c.pve_loot_bound("xuan-tie", 0, 300000), 8);
    assert_eq!(c.pve_loot_bound("huo-ling", 3, 915000), 22);
    assert_eq!(c.pve_loot_bound("huo-ling", 3, 1 << 52), 22);
    assert_eq!(c.pve_loot_bound("missing-item", 3, 915000), 0);
}
