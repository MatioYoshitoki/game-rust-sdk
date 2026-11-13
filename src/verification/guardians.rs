use super::*;
fn harness() -> Harness {
    let c = crate::config::default_catalog().active("b3-4").unwrap();
    let mut h = Harness::new();
    h.engine = Engine::new(c.as_ref().clone());
    let mut req = create_request();
    req.config_id = c.id().into();
    req.config_hash = c.content_hash().into();
    h.engine.create(req).unwrap();
    h.pump().unwrap();
    h.action(Action::Join {}).unwrap();
    h.pump().unwrap();
    h.advance(30).unwrap();
    h.engine.world.as_mut().unwrap().position = c.pve.as_ref().unwrap().chest_spawns[0].position;
    h
}
fn open() -> Action {
    Action::OpenChest {
        chest_id: "guarded-cache-1".into(),
        expected_generation: 1,
    }
}
fn clear_and_claim(h: &mut Harness) {
    for _ in 0..20 {
        h.pump().unwrap();
        let alive = h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().chests[0]
            .guardians
            .as_ref()
            .unwrap()
            .alive;
        if alive == 0 {
            break;
        }
        h.action(Action::Attack {}).unwrap();
        h.advance(1800).unwrap();
    }
    assert_eq!(
        h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().chests[0]
            .guardians
            .as_ref()
            .unwrap()
            .alive,
        0
    );
    h.action(open()).unwrap();
    h.advance(2400).unwrap();
    h.action(open()).unwrap();
    h.pump().unwrap();
}
#[test]
fn public_guarded_attempt_is_idempotent_and_cannot_write_guard_state() {
    let mut h = harness();
    assert_eq!(h.action(open()), Err(Error::GuardianAlive));
    let c = create_request();
    let input = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: c.connection,
        request_id: format!("r-{}", h.sequence),
        sequence: h.sequence,
        action: open(),
    };
    assert_eq!(h.engine.input(input), Err(Error::GuardianAlive));
    h.pump().unwrap();
    assert!(h.snapshot.as_ref().unwrap().resources.is_empty());
    assert!(serde_json::from_value::<Action>(serde_json::json!({"kind":"open_chest","chest_id":"guarded-cache-1","expected_generation":1,"guardians_alive":0})).is_err());
}
#[test]
fn public_combat_releases_only_after_actual_death_then_uses_existing_claim_path() {
    let mut h = harness();
    clear_and_claim(&mut h);
    assert!(h.trace.iter().any(|o|matches!(&o.body,OutputBody::Event{event:Event::MonsterKilled{monster_id}}if monster_id.starts_with("guardian-"))));
    assert!(h.trace.iter().any(|o|matches!(&o.body,OutputBody::Event{event:Event::ChestOpened{chest_id,..}}if chest_id=="guarded-cache-1")));
    assert_eq!(h.store.warehouse, 95);
    assert_eq!(h.store.settlement_commits, 0);
}
#[test]
fn guarded_item_responsibility_recovers_closed_with_no_generation_replayed() {
    let mut h = harness();
    clear_and_claim(&mut h);
    let s = h.snapshot.as_ref().unwrap();
    let id = s.resources.iter().find(|r| !r.orb).unwrap().id.clone();
    let revision = s.inventory_revision;
    h.action(Action::Pickup {
        resource_id: id,
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
    let c = crate::config::default_catalog().active("b3-4").unwrap();
    let mut req = create_request();
    req.config_id = c.id().into();
    req.config_hash = c.content_hash().into();
    req.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    let mut engine = Engine::new(Config::frozen());
    engine.create(req).unwrap();
    assert_eq!(engine.live_worlds(), 0);
    assert_eq!(engine.status().phase, Phase::Closed);
    assert!(engine.drain().iter().all(|o| !matches!(
        o.body,
        OutputBody::Event {
            event: Event::GuardiansReady { .. }
        }
    )));
}
#[test]
fn connection_handover_preserves_guard_ids_and_rejects_previous_owner() {
    let mut h = harness();
    h.pump().unwrap();
    let before = h
        .snapshot
        .as_ref()
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .chests
        .clone();
    let c = create_request();
    h.sequence += 1;
    let i = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "new-connection".into(),
            generation: 2,
        },
        sequence: h.sequence,
        request_id: "takeover".into(),
        action: Action::Reconnect {
            previous_connection: c.connection.clone(),
            authenticated_principal: c.authenticated_principal,
        },
    };
    h.engine.input(i.clone()).unwrap();
    let mut old = i;
    old.connection = c.connection;
    old.request_id = "old-open".into();
    old.sequence += 1;
    old.action = open();
    assert_eq!(h.engine.input(old), Err(Error::StaleConnection));
    let after = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
        .pve
        .unwrap()
        .chests;
    assert_eq!(before, after);
}
