use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-13")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = [1100.0, 1000.0];
    c.geometry.max = [1536.0, 1536.0];
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    c.params.resource_ttl_ms = 1000;
    c.params.runtime.resource_sweep_ms = 1000;
    c.basic.range = 400.0;
    c.basic.windup_ms = 300;
    c.basic.projectile_speed = 1000.0;
    c.skill.damage_bps = 10000;
    let p = c.pve_mut();
    p.width = 48;
    p.height = 48;
    p.cells_rle = vec![(0, 2304)];
    p.monster_positions.clear();
    p.extra_spawns.clear();
    for (k, v) in [
        ("hp", 80),
        ("spirit", 100),
        ("atk", 0),
        ("def", 0),
        ("spd", 0),
        ("crit", 0),
        ("dodge", 0),
    ] {
        p.stats.insert(k.into(), v);
    }
    p.combat.cast_chance = 0.0;
    p.combat.heal.hp_threshold = 0.0;
    p.combat.melee_range = 1.0;
    for t in p.additional_templates.values_mut() {
        t.stats = p.stats.clone();
        t.combat = p.combat.clone();
        t.area = None;
        t.heavy = None;
        t.support = None;
        if let Some(b) = &mut t.boss {
            for phase in &mut b.phases {
                phase.cast_chance = 0.0;
            }
        }
    }
    let mut e = p.encounters[0].clone();
    e.id = "team".into();
    e.requires.clear();
    e.trigger.center = [500.0, 400.0];
    e.trigger.radius = 200.0;
    e.reset.as_mut().unwrap().pursuit_radius = 300.0;
    e.reset.as_mut().unwrap().delay_ms = 1000;
    e.waves.clear();
    e.monsters = vec![
        crate::config::MonsterSpawn {
            id: "a".into(),
            template_id: p.template_id.clone(),
            position: [600.0, 400.0],
        },
        crate::config::MonsterSpawn {
            id: "b".into(),
            template_id: "verification-phase-boss".into(),
            position: [700.0, 400.0],
        },
    ];
    e.required_monster_ids = vec!["a".into(), "b".into()];
    e.reward_chest_ids = vec!["sanctum-cache".into()];
    p.encounters = vec![e];
    p.chest_spawns[0].position = [1100.0, 800.0];
    p.map_objective.as_mut().unwrap().required_encounter_ids = vec!["team".into()];
    c
}
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
    h.advance(90).unwrap();
    h
}
fn enc(h: &Harness) -> EncounterView {
    h.snapshot
        .as_ref()
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .encounters[0]
        .clone()
}
fn attack(h: &mut Harness) {
    let now = h.engine.status().logical_ms;
    let ready = h.snapshot.as_ref().unwrap().basic_ready_at_ms;
    if ready > now {
        h.advance(ready - now).unwrap();
    }
    h.action(Action::Attack {}).unwrap();
    h.advance(700).unwrap();
}
fn outside(h: &mut Harness) {
    h.action(Action::Navigate {
        destination: [400.0, 900.0],
    })
    .unwrap();
    h.advance(5000).unwrap();
}
fn reenter(h: &mut Harness) {
    h.action(Action::Navigate {
        destination: [400.0, 400.0],
    })
    .unwrap();
    h.advance(3500).unwrap();
}
fn repeated(h: &mut Harness) {
    attack(h);
    outside(h);
    assert_eq!(enc(h).reset.unwrap().round, 2);
    reenter(h);
    attack(h);
    attack(h);
    assert_eq!(enc(h).phase, EncounterPhase::Completed);
}
fn checkpoint(h: &mut Harness) -> IoCheckpoint {
    h.action(Action::Leave {}).unwrap();
    h.engine
        .drain()
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone()
}
fn settlement(cp: &mut IoCheckpoint) -> &mut Business {
    &mut cp
        .state
        .operations
        .iter_mut()
        .find(|o| matches!(o.request.body, Business::CommitSettlement { .. }))
        .unwrap()
        .request
        .body
}
fn recover(c: Config, cp: IoCheckpoint) -> Result<Engine> {
    let mut r = create_request();
    r.config_id = c.id().into();
    r.config_hash = c.content_hash().into();
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    let mut e = Engine::new(c);
    e.create(r)?;
    Ok(e)
}
#[test]
fn reset_public_partial_kill_withdraw_retry_completes_three_actual_two_rewards() {
    let c = config();
    let mut h = harness(c);
    repeated(&mut h);
    let s = h
        .snapshot
        .as_ref()
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .scoring
        .as_ref()
        .unwrap();
    assert_eq!((s.kills, s.credited_kills, s.kill_score), (3, 2, 110));
    h.action(Action::Leave {}).unwrap();
    h.pump().unwrap();
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(h.store.settlement_commits, 1);
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::EncounterReset { .. }
                }
            ))
            .count(),
        1
    );
}
#[test]
fn reset_public_return_before_delay_keeps_current_round_partial_defeat() {
    let mut c = config();
    c.pve_mut().encounters[0].reset.as_mut().unwrap().delay_ms = 10000;
    let mut h = harness(c);
    attack(&mut h);
    outside(&mut h);
    assert!(enc(&h).reset.unwrap().reset_at_ms.is_some());
    reenter(&mut h);
    assert_eq!(enc(&h).reset.unwrap().round, 1);
    assert!(enc(&h).reset.unwrap().reset_at_ms.is_none());
    assert_eq!(enc(&h).defeated_required, 1);
    assert!(!h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::EncounterReset { .. }
        }
    )));
}
#[test]
fn reset_public_background_inside_preserves_round_outside_finishes_timer() {
    for out in [false, true] {
        let mut h = harness(config());
        if out {
            h.action(Action::Navigate {
                destination: [400.0, 900.0],
            })
            .unwrap();
            h.advance(2200).unwrap();
        }
        h.action(Action::Background {}).unwrap();
        h.pump().unwrap();
        h.advance(5000).unwrap();
        assert_eq!(enc(&h).reset.unwrap().round, if out { 2 } else { 1 });
        h.engine
            .close(Close {
                key: create_request().key,
                request_id: "close-background".into(),
            })
            .unwrap();
        h.pump().unwrap();
        assert_eq!(h.engine.live_worlds(), 0);
    }
}
#[test]
fn reset_recovery_preserves_frozen_unique_and_actual_proof_without_combat() {
    let c = config();
    let mut h = harness(c.clone());
    repeated(&mut h);
    let mut cp = checkpoint(&mut h);
    let body = settlement(&mut cp).clone();
    if let Business::CommitSettlement {
        reset_progress: Some(s),
        scoring: Some(score),
        ..
    } = &body
    {
        assert_eq!(s.actual_kills, 3);
        assert_eq!(score.kills, 2);
    } else {
        panic!();
    }
    let mut e = recover(c, cp).unwrap();
    assert_eq!(e.live_worlds(), 0);
    let out = e.drain();
    assert!(
        out.iter().filter_map(|o| o.recovery.as_deref()).any(|c| c
            .state
            .operations
            .iter()
            .any(|o| o.request.body == body))
    );
    assert!(!out.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::EncounterReset { .. }
                | Event::EncounterStarted { .. }
                | Event::MonsterKilled { .. }
        }
    )));
}
#[test]
fn reset_recovery_rejects_missing_forged_reordered_or_inflated_proofs() {
    let c = config();
    let mut h = harness(c.clone());
    repeated(&mut h);
    let cp = checkpoint(&mut h);
    assert!(recover(c.clone(), cp.clone()).is_ok());
    for mode in 0..15 {
        let mut bad = cp.clone();
        if let Business::CommitSettlement {
            reset_progress,
            scoring,
            map_progress,
            looted,
            ..
        } = settlement(&mut bad)
        {
            if mode == 0 {
                *reset_progress = None;
            } else {
                let s = reset_progress.as_mut().unwrap();
                let row = &mut s.encounters[0];
                match mode {
                    1 => s.actual_kills += 1,
                    2 => row.encounter_id = "other".into(),
                    3 => row.round = 0,
                    4 => row.round = u32::MAX,
                    5 => row.completed = false,
                    6 => row.defeats.push(row.defeats[0].clone()),
                    7 => row.defeats.reverse(),
                    8 => row.defeats[0].monster_id = "foreign".into(),
                    9 => row.defeats[0].deaths = 0,
                    10 => row.defeats[0].last_round = 0,
                    11 => row.defeats[0].deaths = 3,
                    12 => row.defeats[0].last_round = 1,
                    13 => {
                        row.defeats.remove(0);
                    }
                    14 => {
                        *looted = 99999;
                        scoring.as_mut().unwrap().kills += 1;
                        map_progress
                            .as_mut()
                            .unwrap()
                            .completed_encounter_ids
                            .clear();
                    }
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            matches!(recover(c.clone(), bad), Err(Error::InvalidMessage)),
            "forgery {mode}"
        );
    }
}
#[test]
fn reset_close_budget_rejects_without_mutating_countdown_then_reclaims() {
    let mut c = config();
    c.pve_mut().encounters[0].reset.as_mut().unwrap().delay_ms = 10000;
    let mut h = harness(c);
    outside(&mut h);
    assert!(enc(&h).reset.unwrap().reset_at_ms.is_some());
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
    let r = Close {
        key: create_request().key,
        request_id: "close-reset".into(),
    };
    assert_eq!(h.engine.close(r.clone()), Err(Error::QueueFull));
    assert_eq!(h.engine.live_worlds(), 1);
    h.engine.drain();
    h.engine.close(r).unwrap();
    let out = h.engine.drain();
    assert!(out.len() <= 32);
    assert_eq!(
        out.iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::EncounterResetCancelled { .. }
                }
            ))
            .count(),
        1
    );
    assert_eq!(h.engine.live_worlds(), 0);
}
#[test]
fn reset_public_old_uncommitted_cast_cannot_damage_rebuilt_group() {
    let mut c = config();
    c.skill.windup_ms = 20000;
    let mut h = harness(c);
    h.action(Action::Cast {
        skill_id: h.engine.config.skill.id.clone(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    outside(&mut h);
    reenter(&mut h);
    assert_eq!(enc(&h).reset.unwrap().round, 2);
    h.advance(13000).unwrap();
    assert!(
        h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .all(|m| m.hp == 80)
    );
    attack(&mut h);
    assert_eq!(enc(&h).defeated_required, 1);
}
#[test]
fn reset_legacy_world_has_no_round_events_or_frozen_proof() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.advance(1000).unwrap();
    let mut cp = checkpoint(&mut h);
    if let Business::CommitSettlement { reset_progress, .. } = settlement(&mut cp) {
        assert!(reset_progress.is_none());
    }
    assert!(!h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::EncounterReset { .. }
                | Event::EncounterResetScheduled { .. }
                | Event::EncounterResetCancelled { .. }
        }
    )));
}
