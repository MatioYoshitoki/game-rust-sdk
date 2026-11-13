use super::*;
fn harness(c: Config) -> Harness {
    let mut h = Harness::new();
    h.engine = Engine::new(c.clone());
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
fn drive(h: &mut Harness, phase_only: bool) {
    let center = h.engine.config.pve.as_ref().unwrap().encounters[0]
        .trigger
        .center;
    h.action(Action::Navigate {
        destination: center,
    })
    .unwrap();
    h.advance(900).unwrap();
    for _ in 0..180 {
        h.pump().unwrap();
        let s = h.snapshot.as_ref().unwrap();
        if s.pve.as_ref().unwrap().encounters[0].phase == EncounterPhase::Completed
            || phase_only
                && s.pve
                    .as_ref()
                    .unwrap()
                    .monsters
                    .iter()
                    .any(|m| m.boss.as_ref().is_some_and(|b| b.phase_index >= 2) && m.hp > 0)
        {
            return;
        }
        if s.pve.as_ref().unwrap().encounters[0].phase == EncounterPhase::Waiting {
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
            Err(Error::NoResource | Error::OutOfRange | Error::InvalidState) => {}
            Err(e) => panic!("{e:?}"),
        };
        h.advance(600).unwrap();
    }
    panic!("Boss did not progress")
}
fn config() -> Config {
    crate::config::default_catalog()
        .active("b3-9")
        .unwrap()
        .as_ref()
        .clone()
}
fn flat(count: usize) -> Config {
    let mut c = config();
    c.spawn = [400.0, 400.0];
    c.extraction_point = c.spawn;
    c.geometry.max = [2048.0, 2048.0];
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    let p = c.pve_mut();
    p.width = 64;
    p.height = 64;
    p.cells_rle = vec![(0, 4096)];
    p.monster_positions.clear();
    p.encounters.clear();
    p.chest_spawns[0].position = [400.0, 400.0];
    p.chest_spawns[1].position = [400.0, 700.0];
    p.extra_spawns = (0..count)
        .map(|i| crate::config::MonsterSpawn {
            id: format!("area-unit-{i}"),
            template_id: "verification-phase-boss".into(),
            position: [
                700.0 + (i % 8) as f64 * 120.0,
                100.0 + (i / 8) as f64 * 250.0,
            ],
        })
        .collect();
    let t = p
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    t.combat.aggro_range = 10000.0;
    t.leash = 10000.0;
    t.memory_range = 10000.0;
    for phase in &mut t.boss.as_mut().unwrap().phases {
        phase.cast_chance = 0.0;
    }
    let a = t.area.as_mut().unwrap();
    a.range = 10000.0;
    a.windup_ms = 60000;
    a.cooldown_ms = 60000;
    c
}
fn close_checkpoint(h: &mut Harness) -> IoCheckpoint {
    h.action(Action::Leave {}).unwrap();
    h.engine
        .drain()
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone()
}
fn restore(c: Config, cp: IoCheckpoint) -> Engine {
    let mut r = create_request();
    r.config_id = c.id().into();
    r.config_hash = c.content_hash().into();
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    let mut e = Engine::new(c);
    e.create(r).unwrap();
    assert_eq!(e.live_worlds(), 0);
    assert_eq!(e.status().phase, Phase::Closed);
    e
}
#[test]
fn public_full_combat_keeps_score_and_cancels_dead_owner_warning() {
    let mut h = harness(config());
    drive(&mut h, false);
    assert_eq!(
        h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .scoring
            .as_ref()
            .unwrap()
            .kill_score,
        140
    );
    assert!(h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::AreaStarted { .. }
        }
    )));
    assert!(
        h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .all(|m| m.area_warning.is_none())
    );
    let cp = close_checkpoint(&mut h);
    assert!(cp.state.operations.iter().any(|o|matches!(&o.request.body,Business::CommitSettlement{cultivation:52,scoring:Some(s),..}if s.kill_score==140)));
}
#[test]
fn handover_cancels_pending_warning_keeps_paid_mp_and_fences_old_input() {
    let mut h = harness(flat(1));
    let before = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
        .pve
        .unwrap()
        .monsters[0]
        .clone();
    assert!(before.area_warning.is_some());
    let c = create_request();
    h.sequence += 1;
    let mut input = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "area-next".into(),
            generation: 2,
        },
        sequence: h.sequence,
        request_id: "area-handover".into(),
        action: Action::Reconnect {
            previous_connection: c.connection.clone(),
            authenticated_principal: c.authenticated_principal,
        },
    };
    h.engine.input(input.clone()).unwrap();
    input.connection = c.connection;
    input.request_id = "old-area-input".into();
    input.sequence += 1;
    input.action = Action::Attack {};
    assert_eq!(h.engine.input(input), Err(Error::StaleConnection));
    let after = &h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
        .pve
        .unwrap()
        .monsters[0];
    assert!(after.area_warning.is_none());
    assert_eq!(after.spirit, before.spirit);
}
#[test]
fn background_cancels_thirty_two_warnings_as_one_public_event() {
    let mut h = harness(flat(32));
    h.pump().unwrap();
    h.action(Action::Background {}).unwrap();
    let outputs = h.engine.drain();
    let cancels: Vec<_> = outputs
        .iter()
        .filter_map(|o| match &o.body {
            OutputBody::Event {
                event:
                    Event::AreaCancelled {
                        casts,
                        reason: AreaCancelReason::Background,
                    },
            } => Some(casts),
            _ => None,
        })
        .collect();
    assert_eq!(cancels.len(), 1);
    assert_eq!(cancels[0].len(), 32);
    assert!(outputs.len() <= 16);
    assert!(
        h.engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(&h.engine.config, h.engine.status().logical_ms)
            .pve
            .unwrap()
            .monsters
            .iter()
            .all(|m| m.area_warning.is_none())
    );
}
#[test]
fn close_reserves_one_aggregate_cancel_slot_before_reclaim() {
    let mut h = harness(flat(32));
    h.engine.config.output_capacity = 32;
    h.pump().unwrap();
    let mut held = vec![];
    for _ in 0..2 {
        for _ in 0..14 {
            h.action(Action::Foreground {}).unwrap();
        }
        held.extend(h.engine.drain());
    }
    h.engine.restore_undelivered(held);
    let request = Close {
        key: create_request().key,
        request_id: "close-area".into(),
    };
    assert_eq!(h.engine.close(request.clone()), Err(Error::QueueFull));
    assert_eq!(h.engine.live_worlds(), 1);
    h.engine.drain();
    h.engine.close(request).unwrap();
    let out = h.engine.drain();
    assert!(out.len() <= 32);
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(out.iter().filter(|o|matches!(&o.body,OutputBody::Event{event:Event::AreaCancelled{casts,..}}if casts.len()==32)).count(),1);
}
#[test]
fn large_start_burst_obeys_actual_queue_budget_and_freezes_one_responsibility() {
    for cap in [32, 128] {
        let mut c = flat(32);
        c.output_capacity = cap;
        let mut h = Harness::new();
        h.engine = Engine::new(c.clone());
        let mut r = create_request();
        r.config_id = c.id().into();
        r.config_hash = c.content_hash().into();
        h.engine.create(r).unwrap();
        h.pump().unwrap();
        h.action(Action::Join {}).unwrap();
        h.pump().unwrap();
        h.engine.tick();
        let out = h.engine.drain();
        assert!(out.len() <= cap);
        if cap == 32 {
            assert_eq!(h.engine.live_worlds(), 0);
            assert_eq!(h.engine.status().phase, Phase::Faulted);
            assert_eq!(
                out.iter()
                    .filter(|o| matches!(
                        o.body,
                        OutputBody::BusinessRequest {
                            request: BusinessRequest {
                                body: Business::CommitSettlement { .. },
                                ..
                            }
                        }
                    ))
                    .count(),
                1
            );
        } else {
            assert_eq!(
                out.iter()
                    .filter(|o| matches!(
                        o.body,
                        OutputBody::Event {
                            event: Event::AreaStarted { .. }
                        }
                    ))
                    .count(),
                32
            );
            assert_eq!(h.engine.status().phase, Phase::Active);
        }
    }
}
#[test]
fn pending_area_checkpoint_restores_closed_without_world_warning_or_damage() {
    let c = flat(1);
    let mut h = harness(c.clone());
    let cp = close_checkpoint(&mut h);
    assert!(
        !serde_json::to_value(&cp)
            .unwrap()
            .to_string()
            .contains("area_warning")
    );
    let mut e = restore(c, cp);
    assert!(e.drain().iter().all(|o| !matches!(
        o.body,
        OutputBody::Event {
            event: Event::AreaStarted { .. } | Event::AreaResolved { .. }
        }
    )));
}
#[test]
fn completed_area_fight_recovers_original_score_without_replaying_skill() {
    let c = config();
    let mut h = harness(c.clone());
    drive(&mut h, false);
    let cp = close_checkpoint(&mut h);
    let mut e = restore(c, cp);
    assert!(e.drain().iter().all(|o| !matches!(
        o.body,
        OutputBody::Event {
            event: Event::AreaStarted { .. }
                | Event::AreaResolved { .. }
                | Event::KillScored { .. }
        }
    )));
}
#[test]
fn public_schema_cannot_write_warning_or_force_area_damage() {
    assert!(
        serde_json::from_value::<Action>(serde_json::json!({"kind":"resolve_area","damage":9999}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<Action>(
            serde_json::json!({"kind":"attack","area_warning":{"center":[1,1],"resolves_at_ms":0}})
        )
        .is_err()
    );
    let mut h = harness(flat(1));
    let hp = h.snapshot.as_ref().unwrap().hp;
    let before = h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().monsters[0]
        .area_warning
        .clone();
    h.advance(900).unwrap();
    assert_eq!(h.snapshot.as_ref().unwrap().hp, hp);
    assert_eq!(
        h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().monsters[0].area_warning,
        before
    );
}
