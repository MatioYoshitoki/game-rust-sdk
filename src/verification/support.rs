use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-11")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = c.spawn;
    c.geometry.max = [1536.0, 1536.0];
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    c.skill.damage_bps = 10000;
    let p = c.pve_mut();
    p.width = 48;
    p.height = 48;
    p.cells_rle = vec![(0, 2304)];
    p.monster_positions.clear();
    p.extra_spawns.clear();
    p.stats.insert("hp".into(), 600);
    for key in ["atk", "spd", "crit", "dodge", "def"] {
        p.stats.insert(key.into(), 0);
    }
    p.stats.insert("spirit".into(), 100);
    p.combat.cast_chance = 0.0;
    for t in p.additional_templates.values_mut() {
        t.stats = p.stats.clone();
        t.combat.cast_chance = 0.0;
        t.combat.heal.hp_threshold = 0.0;
        t.area = None;
        t.boss = None;
    }
    let s = p
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap();
    s.include_self = false;
    s.range = 1000.0;
    s.windup_ms = 10000;
    s.ticks = 2;
    s.tick_interval_ms = 1000;
    s.interrupt_window_ms = 11000;
    s.heal_bps = 1000;
    s.cooldown_ms = 20000;
    s.recovery_ms = 0;
    s.interrupt_cooldown_ms = 1000;
    let mut e = p.encounters[0].clone();
    e.id = "team".into();
    e.requires.clear();
    e.trigger.center = [500.0, 400.0];
    e.trigger.radius = 200.0;
    e.monsters = vec![
        crate::config::MonsterSpawn {
            id: "target-b".into(),
            template_id: "verification-phase-boss".into(),
            position: [600.0, 400.0],
        },
        crate::config::MonsterSpawn {
            id: "support-a".into(),
            template_id: "verification-forest-support".into(),
            position: [700.0, 400.0],
        },
    ];
    e.required_monster_ids = vec!["target-b".into(), "support-a".into()];
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
    h.advance(30).unwrap();
    h
}
fn caster(h: &Harness) -> MonsterView {
    h.snapshot
        .as_ref()
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .monsters
        .iter()
        .find(|m| m.id == "support-a")
        .unwrap()
        .clone()
}
fn start(h: &mut Harness) {
    h.action(Action::Navigate {
        destination: [550.0, 400.0],
    })
    .unwrap();
    h.advance(1000).unwrap();
    h.action(Action::Stop {}).unwrap();
    h.action(Action::Attack {}).unwrap();
    h.advance(500).unwrap();
    assert!(caster(h).support.unwrap().cast.is_some());
}
fn approach(h: &mut Harness) {
    for destination in [[550.0, 500.0], [750.0, 500.0], [750.0, 400.0]] {
        h.action(Action::Navigate { destination }).unwrap();
        h.advance(1500).unwrap();
    }
    h.action(Action::Stop {}).unwrap();
    assert_eq!(
        h.snapshot.as_ref().unwrap().target.as_ref().unwrap().id,
        "support-a"
    );
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
fn recover(c: Config, cp: IoCheckpoint) -> Engine {
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
    e
}
fn interruptions(h: &Harness) -> usize {
    h.trace
        .iter()
        .filter(|o| {
            matches!(
                o.body,
                OutputBody::Event {
                    event: Event::SupportInterrupted { .. }
                }
            )
        })
        .count()
}
#[test]
fn support_public_manual_attack_interrupts_without_a_special_command() {
    let mut h = harness(config());
    start(&mut h);
    approach(&mut h);
    h.action(Action::Attack {}).unwrap();
    h.advance(500).unwrap();
    assert_eq!(interruptions(&h), 1);
    let m = caster(&h);
    assert_eq!(m.spirit, 100);
    assert_eq!(m.support.unwrap().remaining_casts, 4);
}
#[test]
fn support_public_cast_direct_contact_carries_manual_action_time() {
    let mut h = harness(config());
    start(&mut h);
    approach(&mut h);
    let cast_id = caster(&h).support.unwrap().cast.unwrap().cast_id;
    let id = h.engine.config.skill.id.clone();
    h.action(Action::Cast {
        skill_id: id,
        direction: [-1.0, 0.0],
    })
    .unwrap();
    h.advance(1000).unwrap();
    assert_eq!(interruptions(&h), 1);
    assert!(
        caster(&h)
            .support
            .unwrap()
            .cast
            .is_none_or(|a| a.cast_id != cast_id)
    );
}
#[test]
fn support_public_duplicate_attack_request_cannot_interrupt_twice() {
    let mut h = harness(config());
    start(&mut h);
    approach(&mut h);
    let r = create_request();
    h.sequence += 1;
    let input = Input {
        key: r.key,
        participant_id: r.participant_id,
        entity_generation: 1,
        connection: r.connection,
        request_id: "manual-attack-once".into(),
        sequence: h.sequence,
        action: Action::Attack {},
    };
    let first = h.engine.input(input.clone()).unwrap();
    h.pump().unwrap();
    h.advance(500).unwrap();
    assert_eq!(h.engine.input(input).unwrap(), first);
    h.pump().unwrap();
    assert_eq!(interruptions(&h), 1);
}
#[test]
fn support_public_pulses_commit_once_and_background_keeps_used_budget() {
    let mut h = harness(config());
    start(&mut h);
    let due = caster(&h).support.unwrap().cast.unwrap().commits_at_ms;
    let now = h.engine.status().logical_ms;
    h.advance(due - now).unwrap();
    let m = caster(&h);
    assert_eq!(m.spirit, 80);
    assert_eq!(m.support.unwrap().remaining_casts, 3);
    h.action(Action::Background {}).unwrap();
    h.pump().unwrap();
    h.advance(2000).unwrap();
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::SupportPulse { .. }
                }
            ))
            .count(),
        1
    );
    assert_eq!(caster(&h).spirit, 80);
    assert_eq!(caster(&h).support.unwrap().remaining_casts, 3);
}
#[test]
fn support_closed_recovery_preserves_exact_assets_and_does_not_replay_healing() {
    let c = config();
    let mut h = harness(c.clone());
    start(&mut h);
    h.advance(12000).unwrap();
    assert!(caster(&h).support.as_ref().unwrap().remaining_casts < 4);
    let cp = checkpoint(&mut h);
    let body = cp
        .state
        .operations
        .iter()
        .find(|o| matches!(o.request.body, Business::CommitSettlement { .. }))
        .unwrap()
        .request
        .body
        .clone();
    let mut e = recover(c, cp);
    let outputs = e.drain();
    assert!(!outputs.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::SupportStarted { .. } | Event::SupportPulse { .. }
        }
    )));
    assert!(
        outputs
            .iter()
            .filter_map(|o| o.recovery.as_deref())
            .any(|c| c.state.operations.iter().any(|o| o.request.body == body))
    );
}
#[test]
fn support_public_unpaid_close_emits_one_cancel_before_world_reclaim() {
    let mut h = harness(config());
    start(&mut h);
    h.action(Action::Leave {}).unwrap();
    h.pump().unwrap();
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::SupportCancelled {
                        reason: SupportCancelReason::Closed,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
    assert_eq!(h.store.admission_commits, 1);
    assert_eq!(h.store.settlement_commits, 1);
}
#[test]
fn support_legacy_configuration_has_no_support_projection_or_events() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.advance(3000).unwrap();
    assert!(h.trace.iter().all(|o| !matches!(
        o.body,
        OutputBody::Event {
            event: Event::SupportStarted { .. }
                | Event::SupportPulse { .. }
                | Event::SupportCancelled { .. }
                | Event::SupportInterrupted { .. }
        }
    )));
}
#[test]
fn support_close_reserves_cancel_event_without_mutating_on_rejection() {
    let mut h = harness(config());
    start(&mut h);
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
        request_id: "close-support".into(),
    };
    assert_eq!(h.engine.close(request.clone()), Err(Error::QueueFull));
    assert_eq!(h.engine.live_worlds(), 1);
    h.engine.drain();
    h.engine.close(request).unwrap();
    let out = h.engine.drain();
    assert!(out.len() <= 32);
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(
        out.iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::SupportCancelled {
                        reason: SupportCancelReason::Closed,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
}
