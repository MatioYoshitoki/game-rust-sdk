use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-12")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = [1100.0, 900.0];
    c.geometry.max = [1536.0, 1536.0];
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    c.basic.range = 300.0;
    c.basic.windup_ms = 300;
    c.basic.projectile_speed = 1000.0;
    c.skill.damage_bps = 0;
    let p = c.pve_mut();
    p.width = 48;
    p.height = 48;
    p.cells_rle = vec![(0, 2304)];
    p.monster_positions.clear();
    p.extra_spawns.clear();
    let t = p
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    for (k, v) in [
        ("hp", 1000),
        ("spirit", 100),
        ("atk", 50),
        ("def", 0),
        ("spd", 0),
        ("crit", 0),
        ("dodge", 0),
    ] {
        t.stats.insert(k.into(), v);
    }
    t.area = None;
    t.combat.cast_chance = 0.0;
    t.combat.heal.hp_threshold = 0.0;
    t.combat.melee_range = 1.0;
    for phase in &mut t.boss.as_mut().unwrap().phases {
        phase.cast_chance = 0.0;
    }
    let h = t.heavy.as_mut().unwrap();
    h.range = 500.0;
    h.windup_ms = 3000;
    h.travel_ms = 1000;
    h.cost = 20;
    h.cooldown_ms = 60000;
    h.counter_window_ms = 10000;
    let mut e = p.encounters[0].clone();
    e.id = "team".into();
    e.requires.clear();
    e.trigger.center = [500.0, 400.0];
    e.trigger.radius = 200.0;
    e.monsters = vec![crate::config::MonsterSpawn {
        id: "heavy-owner".into(),
        template_id: "verification-phase-boss".into(),
        position: [600.0, 400.0],
    }];
    e.required_monster_ids = vec!["heavy-owner".into()];
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
    h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().monsters[0].clone()
}
fn start(h: &mut Harness) {
    for _ in 0..100 {
        if caster(h).heavy.as_ref().unwrap().cast.is_some() {
            return;
        }
        h.advance(30).unwrap();
    }
    panic!("No heavy");
}
fn dodge(h: &mut Harness) {
    start(h);
    h.action(Action::Move {
        direction: [0.0, 1.0],
    })
    .unwrap();
    h.advance(600).unwrap();
    h.action(Action::Stop {}).unwrap();
    for _ in 0..200 {
        if h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .counter
            .is_some()
        {
            return;
        }
        h.advance(30).unwrap();
    }
    panic!("No counter {:?}", h.snapshot);
}
fn triggers(h: &Harness) -> Vec<u32> {
    h.trace
        .iter()
        .filter_map(|o| match &o.body {
            OutputBody::Event {
                event: Event::CounterTriggered { damage, .. },
            } => Some(*damage),
            _ => None,
        })
        .collect()
}
#[test]
fn heavy_public_movement_dodge_attack_consumes_one_window() {
    let mut h = harness(config());
    dodge(&mut h);
    h.action(Action::Attack {}).unwrap();
    h.advance(1000).unwrap();
    assert_eq!(triggers(&h), vec![120]);
    assert!(
        h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .counter
            .is_none()
    );
    assert_eq!(caster(&h).hp, 780);
}
#[test]
fn heavy_public_zero_damage_skill_has_counter_benefit() {
    let mut h = harness(config());
    dodge(&mut h);
    let p = h.snapshot.as_ref().unwrap().position;
    h.action(Action::Cast {
        skill_id: h.engine.config.skill.id.clone(),
        direction: [600.0 - p[0], 400.0 - p[1]],
    })
    .unwrap();
    h.advance(1500).unwrap();
    assert_eq!(triggers(&h), vec![120]);
    assert_eq!(caster(&h).hp, 880);
}
#[test]
fn heavy_public_duplicate_attack_input_does_not_repeat_bonus() {
    let mut h = harness(config());
    dodge(&mut h);
    let r = create_request();
    h.sequence += 1;
    let input = Input {
        key: r.key,
        participant_id: r.participant_id,
        entity_generation: 1,
        connection: r.connection,
        request_id: "one-counter".into(),
        sequence: h.sequence,
        action: Action::Attack {},
    };
    let first = h.engine.input(input.clone()).unwrap();
    h.pump().unwrap();
    h.advance(1000).unwrap();
    assert_eq!(h.engine.input(input).unwrap(), first);
    h.pump().unwrap();
    assert_eq!(triggers(&h), vec![120]);
}
#[test]
fn heavy_public_background_preserves_paid_cost_and_clears_window() {
    for phase in 0..3 {
        let mut h = harness(config());
        start(&mut h);
        if phase == 1 {
            h.advance(3100).unwrap();
        }
        if phase == 2 {
            dodge(&mut h);
        }
        h.action(Action::Background {}).unwrap();
        h.pump().unwrap();
        assert!(caster(&h).heavy.unwrap().cast.is_none());
        assert!(
            h.snapshot
                .as_ref()
                .unwrap()
                .pve
                .as_ref()
                .unwrap()
                .counter
                .is_none()
        );
        assert_eq!(caster(&h).spirit, if phase == 0 { 100 } else { 80 });
        h.advance(5000).unwrap();
        assert!(triggers(&h).is_empty());
    }
}
#[test]
fn heavy_closed_recovery_pins_body_and_does_not_replay_combat() {
    let c = config();
    let mut h = harness(c.clone());
    dodge(&mut h);
    h.action(Action::Leave {}).unwrap();
    let out = h.engine.drain();
    let cp = out
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone();
    let body = cp
        .state
        .operations
        .iter()
        .find(|o| matches!(o.request.body, Business::CommitSettlement { .. }))
        .unwrap()
        .request
        .body
        .clone();
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
    let out = e.drain();
    assert!(!out.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::HeavyStarted { .. }
                | Event::HeavyCommitted { .. }
                | Event::CounterOpened { .. }
                | Event::CounterTriggered { .. }
        }
    )));
    assert!(
        out.iter().filter_map(|o| o.recovery.as_deref()).any(|c| c
            .state
            .operations
            .iter()
            .any(|o| o.request.body == body))
    );
}
#[test]
fn heavy_public_unpaid_close_emits_one_cancel_and_reclaims_world() {
    let mut h = harness(config());
    start(&mut h);
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
                    event: Event::HeavyCancelled {
                        reason: HeavyCancelReason::Closed,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
    assert!(h.trace.iter().all(|o| !matches!(
        o.body,
        OutputBody::Event {
            event: Event::HeavyCommitted { .. }
        }
    )));
}
#[test]
fn heavy_close_capacity_reserves_state_and_rejects_atomically() {
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
    let r = Close {
        key: create_request().key,
        request_id: "close-heavy".into(),
    };
    assert_eq!(h.engine.close(r.clone()), Err(Error::QueueFull));
    assert_eq!(h.engine.live_worlds(), 1);
    h.engine.drain();
    h.engine.close(r).unwrap();
    let out = h.engine.drain();
    assert!(out.len() <= 32);
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(
        out.iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::HeavyCancelled {
                        reason: HeavyCancelReason::Closed,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
}
#[test]
fn heavy_legacy_config_has_no_new_projection_or_events() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.advance(3000).unwrap();
    assert!(h.snapshot.as_ref().unwrap().pve.is_none());
    assert!(!h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::HeavyStarted { .. }
                | Event::HeavyCancelled { .. }
                | Event::CounterOpened { .. }
                | Event::CounterTriggered { .. }
        }
    )));
}
