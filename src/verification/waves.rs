use super::*;
fn harness() -> Harness {
    let c = crate::config::default_catalog().active("b3-6").unwrap();
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
        chest_id: "wave-cache-1".into(),
        expected_generation: 1,
    }
}
fn drive(h: &mut Harness, first_only: bool) {
    let center = h.engine.config.pve.as_ref().unwrap().encounters[0]
        .trigger
        .center;
    h.action(Action::Navigate {
        destination: center,
    })
    .unwrap();
    h.advance(900).unwrap();
    for _ in 0..100 {
        h.pump().unwrap();
        let s = h.snapshot.as_ref().unwrap();
        let e = &s.pve.as_ref().unwrap().encounters[0];
        if e.phase == EncounterPhase::Completed
            || first_only && e.wave.as_ref().unwrap().completed_waves >= 1
        {
            return;
        }
        if e.phase == EncounterPhase::Waiting {
            h.advance(30).unwrap();
            continue;
        }
        let Some(target) = s.target.as_ref().map(|t| t.position) else {
            h.advance(30).unwrap();
            continue;
        };
        h.action(Action::Navigate {
            destination: target,
        })
        .unwrap();
        h.advance(900).unwrap();
        match h.action(Action::Attack {}) {
            Ok(_) => {}
            Err(Error::OutOfRange | Error::InvalidState | Error::NoResource) => {}
            Err(e) => panic!("{e:?}"),
        };
        h.advance(600).unwrap();
    }
    panic!("waves did not progress")
}
#[test]
fn public_inputs_cannot_forge_wave_or_claim_intermediate_reward() {
    let mut h = harness();
    drive(&mut h, true);
    assert_eq!(h.action(open()), Err(Error::EncounterLocked));
    let c = create_request();
    let i = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: c.connection,
        sequence: h.sequence,
        request_id: format!("r-{}", h.sequence),
        action: open(),
    };
    assert_eq!(h.engine.input(i), Err(Error::EncounterLocked));
    assert!(serde_json::from_value::<Action>(serde_json::json!({"kind":"open_chest","chest_id":"wave-cache-1","expected_generation":1,"wave_index":2,"completed":true})).is_err());
}
#[test]
fn public_navigation_combat_follows_two_waves_and_only_then_completes() {
    let mut h = harness();
    drive(&mut h, false);
    let starts: Vec<_> = h
        .trace
        .iter()
        .filter_map(|o| match &o.body {
            OutputBody::Event {
                event: Event::EncounterWaveStarted { wave_index, .. },
            } => Some(*wave_index),
            _ => None,
        })
        .collect();
    assert_eq!(starts, vec![1, 2]);
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
    assert_eq!(
        h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .encounters[0]
            .wave
            .as_ref()
            .unwrap()
            .completed_waves,
        2
    );
    assert!(!h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().chests[0].opened);
    assert_eq!(h.store.warehouse, 95);
}
#[test]
fn multiwave_kill_settlement_recovers_closed_without_replaying_waves() {
    let mut h = harness();
    drive(&mut h, false);
    h.action(Action::Leave {}).unwrap();
    let cp = h
        .engine
        .drain()
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone();
    let c = crate::config::default_catalog().active("b3-6").unwrap();
    let mut r = create_request();
    r.config_id = c.id().into();
    r.config_hash = c.content_hash().into();
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    let mut e = Engine::new(Config::frozen());
    e.create(r).unwrap();
    assert_eq!(e.status().phase, Phase::Closed);
    assert_eq!(e.live_worlds(), 0);
    assert!(e.drain().iter().all(|o| !matches!(
        &o.body,
        OutputBody::Event {
            event: Event::EncounterWaveStarted { .. } | Event::EncounterTriggered { .. }
        }
    )));
}
#[test]
fn handover_between_waves_keeps_deadline_and_fences_old_owner() {
    let mut h = harness();
    drive(&mut h, true);
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
    assert_eq!(before[0].phase, EncounterPhase::Waiting);
    let c = create_request();
    h.sequence += 1;
    let i = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "wave-next".into(),
            generation: 2,
        },
        sequence: h.sequence,
        request_id: "wave-handover".into(),
        action: Action::Reconnect {
            previous_connection: c.connection.clone(),
            authenticated_principal: c.authenticated_principal,
        },
    };
    h.engine.input(i.clone()).unwrap();
    let mut old = i;
    old.connection = c.connection;
    old.sequence += 1;
    old.request_id = "old-wave-open".into();
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
fn simultaneous_wave_events_obey_output_capacity_and_freeze_one_settlement() {
    for cap in [32, 64] {
        let mut h = harness();
        let mut c = h.engine.config.clone();
        c.output_capacity = cap;
        c.spawn = [200.0, 200.0];
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
        for n in 0..4 {
            let mut e = base.clone();
            e.id = format!("group-{n}");
            e.trigger.center = [200.0, 200.0];
            for (wi, wave) in e.waves.iter_mut().enumerate() {
                for (mi, m) in wave.monsters.iter_mut().enumerate() {
                    m.id = format!("group-{n}-{wi}-{mi}");
                    m.position = [500.0 + mi as f64 * 100.0, 100.0 + n as f64 * 200.0];
                }
                wave.required_monster_ids = wave.monsters.iter().map(|m| m.id.clone()).collect();
            }
            let chest = format!("reward-{n}");
            e.reward_chest_ids = vec![chest.clone()];
            p.chest_spawns.push(crate::config::ChestSpawn {
                id: chest,
                position: [800.0, 100.0 + n as f64 * 200.0],
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
        h.engine.tick();
        let outputs = h.engine.drain();
        assert!(outputs.len() <= cap);
        if cap == 32 {
            assert_eq!(h.engine.live_worlds(), 0);
            assert_eq!(h.engine.status().phase, Phase::Faulted);
            assert_eq!(outputs.iter().filter(|o|matches!(&o.body,OutputBody::BusinessRequest{request}if matches!(request.body,Business::CommitSettlement{..}))).count(),1);
        } else {
            assert_eq!(h.engine.status().phase, Phase::Active);
            assert_eq!(
                outputs
                    .iter()
                    .filter(|o| matches!(
                        &o.body,
                        OutputBody::Event {
                            event: Event::EncounterWaveStarted { .. }
                        }
                    ))
                    .count(),
                4
            );
        }
    }
}
