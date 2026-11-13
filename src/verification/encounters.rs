use super::*;
fn harness() -> Harness {
    let c = crate::config::default_catalog().active("b3-5").unwrap();
    let mut h = Harness::new();
    h.engine = Engine::new(c.as_ref().clone());
    let mut r = create_request();
    r.config_id = c.id().into();
    r.config_hash = c.content_hash().into();
    h.engine.create(r).unwrap();
    h.pump().unwrap();
    h.action(Action::Join {}).unwrap();
    h.pump().unwrap();
    h.advance(30).unwrap();
    h
}
fn open() -> Action {
    Action::OpenChest {
        chest_id: "encounter-cache-1".into(),
        expected_generation: 1,
    }
}
fn fight(h: &mut Harness) {
    let center = h.engine.config.pve.as_ref().unwrap().encounters[0]
        .trigger
        .center;
    h.action(Action::Navigate {
        destination: center,
    })
    .unwrap();
    h.advance(1800).unwrap();
    for _ in 0..40 {
        h.pump().unwrap();
        if h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .encounters[0]
            .phase
            == EncounterPhase::Completed
        {
            return;
        }
        let target = h
            .snapshot
            .as_ref()
            .unwrap()
            .target
            .as_ref()
            .unwrap()
            .position;
        h.action(Action::Navigate {
            destination: target,
        })
        .unwrap();
        h.advance(1200).unwrap();
        match h.action(Action::Attack {}) {
            Ok(_) => {}
            Err(Error::NoResource | Error::OutOfRange | Error::InvalidState) => {}
            Err(e) => panic!("{e:?}"),
        };
        h.advance(1500).unwrap();
    }
    panic!("encounter did not finish")
}
#[test]
fn public_locked_requests_are_idempotent_and_cannot_forge_progress() {
    let mut h = harness();
    assert_eq!(h.action(open()), Err(Error::EncounterLocked));
    let c = create_request();
    let input = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: c.connection,
        sequence: h.sequence,
        request_id: format!("r-{}", h.sequence),
        action: open(),
    };
    assert_eq!(h.engine.input(input), Err(Error::EncounterLocked));
    assert!(serde_json::from_value::<Action>(serde_json::json!({"kind":"open_chest","chest_id":"encounter-cache-1","expected_generation":1,"encounter_completed":true})).is_err());
    h.pump().unwrap();
    assert!(h.snapshot.as_ref().unwrap().resources.is_empty());
}
#[test]
fn public_navigation_and_combat_complete_once_without_auto_reward() {
    let mut h = harness();
    fight(&mut h);
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                &o.body,
                OutputBody::Event {
                    event: Event::EncounterTriggered { .. }
                }
            ))
            .count(),
        1
    );
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                &o.body,
                OutputBody::Event {
                    event: Event::EncounterCompleted { .. }
                }
            ))
            .count(),
        1
    );
    assert!(!h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().chests[0].opened);
    assert_eq!(h.store.warehouse, 95);
}
#[test]
fn encounter_kill_settlement_restores_closed_and_never_spawns_again() {
    let mut h = harness();
    fight(&mut h);
    h.action(Action::Leave {}).unwrap();
    let cp = h
        .engine
        .drain()
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone();
    let c = crate::config::default_catalog().active("b3-5").unwrap();
    let mut r = create_request();
    r.config_id = c.id().into();
    r.config_hash = c.content_hash().into();
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    let mut e = Engine::new(Config::frozen());
    e.create(r).unwrap();
    assert_eq!(e.live_worlds(), 0);
    assert_eq!(e.status().phase, Phase::Closed);
    assert!(e.drain().iter().all(|o| !matches!(
        &o.body,
        OutputBody::Event {
            event: Event::EncounterStarted { .. } | Event::EncounterTriggered { .. }
        }
    )));
}
#[test]
fn connection_handover_preserves_encounter_and_fences_old_reward_input() {
    let mut h = harness();
    h.action(Action::Navigate {
        destination: [3952.0, 1488.0],
    })
    .unwrap();
    h.advance(900).unwrap();
    h.pump().unwrap();
    let before = h
        .snapshot
        .as_ref()
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .encounters
        .clone();
    let c = create_request();
    h.sequence += 1;
    let i = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "next".into(),
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
    old.sequence += 1;
    old.request_id = "old-reward".into();
    old.action = open();
    assert_eq!(h.engine.input(old), Err(Error::StaleConnection));
    assert_eq!(
        before,
        h.engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(&h.engine.config, h.engine.status().logical_ms)
            .pve
            .unwrap()
            .encounters
    );
}
#[test]
fn simultaneous_encounters_respect_output_capacity_and_freeze_assets_on_overflow() {
    for capacity in [32, 64] {
        let mut h = harness();
        let mut c = h.engine.config.clone();
        c.output_capacity = capacity;
        c.spawn = [400.0, 400.0];
        c.extraction_point = [900.0, 900.0];
        c.geometry.max = [1024.0, 1024.0];
        let p = c.pve_mut();
        p.width = 32;
        p.height = 32;
        p.cells_rle = vec![(0, 1024)];
        p.combat.cast_chance = 0.0;
        p.stats.insert("spd".into(), 0);
        let base = p.encounters[0].clone();
        p.encounters.clear();
        p.chest_spawns.clear();
        for n in 0..8 {
            let mut e = base.clone();
            e.id = format!("group-{n}");
            e.trigger.center = [400.0, 400.0];
            e.trigger.radius = 100.0;
            e.monsters.truncate(1);
            e.monsters[0].id = format!("group-target-{n}");
            e.monsters[0].position = [
                600.0 + (n % 4) as f64 * 100.0,
                400.0 + (n / 4) as f64 * 200.0,
            ];
            e.required_monster_ids = vec![e.monsters[0].id.clone()];
            let chest = format!("reward-{n}");
            e.reward_chest_ids = vec![chest.clone()];
            p.chest_spawns.push(crate::config::ChestSpawn {
                id: chest,
                position: [100.0 + n as f64 * 100.0, 800.0],
            });
            p.encounters.push(e);
        }
        let facts = h.engine.world.as_ref().unwrap().facts.clone();
        let mut w = crate::world::World::new(&c, facts, 7);
        w.phase = Phase::Active;
        w.ready = true;
        w.joined = true;
        w.foreground = true;
        w.protected = false;
        w.active_at = Some(0);
        h.engine.config = c;
        h.engine.world = Some(w);
        h.engine.drain();
        for _ in 0..16 {
            h.action(Action::Foreground {}).unwrap();
        }
        assert_eq!(h.engine.queued_output_count(), 16);
        h.engine.tick();
        let outputs = h.engine.drain();
        assert!(outputs.len() <= capacity);
        if capacity == 32 {
            assert_eq!(h.engine.status().phase, Phase::Faulted);
            assert_eq!(h.engine.live_worlds(), 0);
            let settlement = outputs
                .iter()
                .find_map(|o| match &o.body {
                    OutputBody::BusinessRequest { request } => match &request.body {
                        Business::CommitSettlement {
                            carried,
                            looted,
                            kept,
                            dropped,
                            cultivation,
                            ..
                        } => Some((*carried, *looted, *kept, *dropped, *cultivation)),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap();
            assert_eq!(settlement.0 + settlement.1, settlement.2 + settlement.3);
            assert_eq!((settlement.1, settlement.4), (0, 0));
            assert_eq!(outputs.iter().filter(|o|matches!(&o.body,OutputBody::BusinessRequest{request} if matches!(request.body,Business::CommitSettlement{..}))).count(),1);
            assert!(outputs.iter().any(|o| matches!(
                &o.body,
                OutputBody::Event {
                    event: Event::Fault {
                        code: Error::QueueFull
                    }
                }
            )));
        } else {
            assert_eq!(h.engine.status().phase, Phase::Active);
            assert_eq!(
                outputs
                    .iter()
                    .filter(|o| matches!(
                        &o.body,
                        OutputBody::Event {
                            event: Event::EncounterStarted { .. }
                        }
                    ))
                    .count(),
                8
            );
        }
    }
}
