use super::*;
pub(super) fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-12")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = [1000.0, 1000.0];
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    let p = c.pve_mut();
    p.width = 64;
    p.height = 64;
    p.cells_rle = vec![(0, 4096)];
    p.monster_positions.clear();
    p.encounters.clear();
    p.map_objective = None;
    p.extra_spawns = vec![crate::config::MonsterSpawn {
        id: "heavy-owner".into(),
        template_id: "verification-phase-boss".into(),
        position: [600.0, 400.0],
    }];
    let t = p
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    for (k, v) in [
        ("hp", 500),
        ("spirit", 100),
        ("atk", 100),
        ("def", 0),
        ("spd", 0),
        ("crit", 0),
        ("dodge", 0),
    ] {
        t.stats.insert(k.into(), v);
    }
    t.area = None;
    t.support = None;
    t.combat.melee_range = 1.0;
    t.combat.lifesteal_ratio = 0.0;
    t.combat.heal.hp_threshold = 0.0;
    for p in &mut t.boss.as_mut().unwrap().phases {
        p.cast_chance = 0.0;
    }
    let h = t.heavy.as_mut().unwrap();
    h.range = 500.0;
    h.half_width = 20.0;
    h.windup_ms = 300;
    h.travel_ms = 600;
    h.cost = 20;
    h.damage_bps = 10000;
    h.cooldown_ms = 5000;
    h.recovery_ms = 90;
    h.counter_window_ms = 1000;
    h.counter_damage_bps = 12000;
    c
}
pub(super) fn world(c: &Config) -> World {
    let mut w = World::new(
        c,
        ParticipantFacts {
            principal: "principal-A".into(),
            revision: 1,
            hp_points: 1000,
            spirit_points: 100,
            atk_points: 100,
            def_points: 0,
            spd_points: 8,
            warehouse_quantity: 95,
        },
        7,
    );
    w.phase = Phase::Active;
    w.joined = true;
    w.ready = true;
    w.foreground = true;
    w.protected = false;
    w.active_at = Some(0);
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.aggro = true;
        m.next_decision = u64::MAX;
        m.next_attack = u64::MAX;
    }
    w
}
pub(super) fn start(w: &mut World, c: &Config, i: usize, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.try_start_heavy(w, c, i, now, &mut e);
    w.pve = Some(a);
    e
}
pub(super) fn advance(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.advance_heavies(w, c, now, &mut e);
    w.pve = Some(a);
    e
}
fn hit(
    w: &mut World,
    c: &Config,
    raw: u32,
    metal: bool,
    accepted: Option<u64>,
    now: u64,
) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.hurt_monster_contact(w, c, 0, (raw, metal, accepted), &mut e);
    w.pve = Some(a);
    e
}
pub(super) fn window(w: &World) -> Option<CounterView> {
    w.pve
        .as_ref()
        .unwrap()
        .counter
        .as_ref()
        .map(|v| v.view.clone())
}
fn dodge(w: &mut World, c: &Config) -> Vec<Event> {
    assert_eq!(start(w, c, 0, 100).len(), 1);
    advance(w, c, 400);
    w.position = [400.0, 500.0];
    advance(w, c, 1000)
}
fn triggered(e: &[Event]) -> Vec<u32> {
    e.iter()
        .filter_map(|e| {
            if let Event::CounterTriggered { damage, .. } = e {
                Some(*damage)
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn heavy_preparation_reserves_and_commit_pays_once() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    advance(&mut w, &c, 399);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!((m.spirit, m.heavy_ready), (100, 0));
    advance(&mut w, &c, 400);
    advance(&mut w, &c, 400);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!((m.spirit, m.heavy_ready), (80, 5400));
    assert_eq!(w.hp, 1000);
}
#[test]
fn heavy_fixed_origin_direction_and_phase_keep_budget() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    let before = w.pve.as_ref().unwrap().monsters[0]
        .heavy_view()
        .unwrap()
        .cast
        .unwrap();
    w.position = [400.0, 500.0];
    w.pve.as_mut().unwrap().monsters[0].boss_phase = 1;
    advance(&mut w, &c, 400);
    let after = w.pve.as_ref().unwrap().monsters[0]
        .heavy_view()
        .unwrap()
        .cast
        .unwrap();
    assert_eq!((before.origin, before.end), (after.origin, after.end));
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 80);
}
#[test]
fn heavy_swept_contact_hits_once_after_commit() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    advance(&mut w, &c, 400);
    let e = advance(&mut w, &c, 1000);
    assert_eq!(w.hp, 900);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::HeavyResolved {
            contacted: true,
            damage: 100,
            ..
        }
    )));
    assert!(advance(&mut w, &c, 2000).is_empty());
    assert!(window(&w).is_none());
}
#[test]
fn heavy_movement_dodge_opens_frozen_single_target_window() {
    let c = config();
    let mut w = world(&c);
    let e = dodge(&mut w, &c);
    let v = window(&w).unwrap();
    assert_eq!(
        (
            v.monster_id.as_str(),
            v.started_at_ms,
            v.ends_at_ms,
            v.raw_damage
        ),
        ("heavy-owner", 1000, 2000, 120)
    );
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::CounterOpened { .. }))
            .count(),
        1
    );
    w.facts.atk_points = 999;
    assert_eq!(window(&w).unwrap().raw_damage, 120);
}
#[test]
fn heavy_no_movement_intent_does_not_create_counter() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    w.movement = [0.0, 1.0];
    advance(&mut w, &c, 400);
    advance(&mut w, &c, 1000);
    assert!(window(&w).is_none());
}
#[test]
fn heavy_start_gates_do_not_consume_random() {
    for mode in 0..7 {
        let mut c = config();
        if mode == 0 {
            c.pve_mut()
                .additional_templates
                .get_mut("verification-phase-boss")
                .unwrap()
                .heavy
                .as_mut()
                .unwrap()
                .cast_chance = 0.0;
        }
        let mut w = world(&c);
        match mode {
            1 => w.protected = true,
            2 => w.ready = false,
            3 => w.pve.as_mut().unwrap().monsters[0].spirit = 19,
            4 => w.position = [10.0, 400.0],
            5 => w.pve.as_mut().unwrap().monsters[0].returning = true,
            6 => w.position = [600.0, 400.0],
            _ => {}
        }
        let rng = w.rng;
        assert!(start(&mut w, &c, 0, 100).is_empty());
        assert_eq!(rng, w.rng);
    }
}
#[test]
fn heavy_zero_damage_contact_is_hit_and_cannot_award() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .heavy
        .as_mut()
        .unwrap()
        .damage_bps = 0;
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    advance(&mut w, &c, 400);
    let e = advance(&mut w, &c, 1000);
    assert_eq!(w.hp, 1000);
    assert!(window(&w).is_none());
    assert!(e.iter().any(|e| matches!(
        e,
        Event::HeavyResolved {
            contacted: true,
            damage: 0,
            ..
        }
    )));
}
#[test]
fn heavy_reentering_trajectory_contact_loses_dodge() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    w.position = [400.0, 500.0];
    advance(&mut w, &c, 400);
    w.position = [400.0, 400.0];
    advance(&mut w, &c, 1000);
    assert!(window(&w).is_none());
    assert_eq!(w.hp, 900);
}
#[test]
fn heavy_capsule_tangent_is_contact_and_outside_is_safe() {
    let c = config();
    let radius = 20.0 + c.params.player_radius;
    for (delta, contact) in [(0.0, true), (0.001, false)] {
        let mut w = world(&c);
        start(&mut w, &c, 0, 100);
        advance(&mut w, &c, 400);
        w.position = [400.0, 400.0 + radius + delta];
        let e = advance(&mut w, &c, 1000);
        assert!(
            e.iter()
                .any(|e| matches!(e,Event::HeavyResolved{contacted,..}if *contacted==contact))
        );
        assert_eq!(window(&w).is_some(), !contact);
    }
}
#[test]
fn heavy_wall_blocks_and_never_grants_dodge_credit() {
    let mut c = config();
    let p = c.pve_mut();
    let mut tile = p.palette[0].clone();
    tile.blocks_projectiles = true;
    tile.blocks_vision = false;
    tile.walkable = true;
    let id = p.palette.len();
    p.palette.push(tile);
    let mut cells = vec![0; 4096];
    cells[12 * 64 + 15] = id;
    p.cells_rle = cells.into_iter().map(|i| (i, 1)).collect();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    advance(&mut w, &c, 400);
    w.position = [400.0, 500.0];
    let e = advance(&mut w, &c, 1000);
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::HeavyResolved { blocked: true, .. }))
    );
    assert!(window(&w).is_none());
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    advance(&mut w, &c, 400);
    w.position = [512.0 - c.params.player_radius, 400.0];
    let e = advance(&mut w, &c, 1000);
    assert_eq!(
        w.hp, 1000,
        "wall and target at the same contact distance must block"
    );
    assert!(e.iter().any(|e| matches!(
        e,
        Event::HeavyResolved {
            blocked: true,
            contacted: false,
            ..
        }
    )));
}
#[test]
fn heavy_resource_loss_at_commit_cancels_without_cost() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    w.pve.as_mut().unwrap().monsters[0].spirit = 19;
    let e = advance(&mut w, &c, 400);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!((m.spirit, m.heavy_ready), (19, 0));
    assert!(m.heavy_cast.is_none());
    assert!(e.iter().any(|e| matches!(
        e,
        Event::HeavyCancelled {
            reason: HeavyCancelReason::Resource,
            ..
        }
    )));
}
#[test]
fn heavy_precommit_and_paid_background_keep_distinct_costs() {
    for paid in [false, true] {
        let c = config();
        let mut w = world(&c);
        start(&mut w, &c, 0, 100);
        if paid {
            advance(&mut w, &c, 400);
        }
        let e = w
            .action(&Action::Background {}, "background", &c, 450)
            .unwrap();
        assert!(e.iter().any(|e| matches!(
            e,
            Event::HeavyCancelled {
                reason: HeavyCancelReason::Background,
                ..
            }
        )));
        assert_eq!(
            w.pve.as_ref().unwrap().monsters[0].spirit,
            if paid { 80 } else { 100 }
        );
        assert!(advance(&mut w, &c, 1000).is_empty());
    }
}
#[test]
fn heavy_player_lethal_contact_cleans_other_casts() {
    let c = config();
    let mut w = world(&c);
    w.hp = 50;
    start(&mut w, &c, 0, 100);
    advance(&mut w, &c, 400);
    advance(&mut w, &c, 1000);
    assert_eq!(w.hp, 0);
    assert!(w.desired_outcome.is_some());
    assert!(!w.pve.as_ref().unwrap().has_heavy_state());
}
#[test]
fn heavy_counter_new_manual_hit_adds_physical_damage_once() {
    let c = config();
    let mut w = world(&c);
    dodge(&mut w, &c);
    let e = hit(&mut w, &c, 100, false, Some(1000), 1100);
    assert_eq!(triggered(&e), vec![120]);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 280);
    assert!(window(&w).is_none());
    assert!(triggered(&hit(&mut w, &c, 0, false, Some(1100), 1200)).is_empty());
}
#[test]
fn heavy_counter_zero_direct_skill_damage_still_benefits() {
    let c = config();
    let mut w = world(&c);
    dodge(&mut w, &c);
    let e = hit(&mut w, &c, 0, true, Some(1000), 1100);
    assert_eq!(triggered(&e), vec![120]);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 380);
}
#[test]
fn heavy_counter_old_and_unattributed_actions_do_not_consume() {
    let c = config();
    let mut w = world(&c);
    dodge(&mut w, &c);
    for a in [None, Some(999)] {
        assert!(triggered(&hit(&mut w, &c, 0, false, a, 1100)).is_empty());
        assert!(window(&w).is_some());
    }
}
#[test]
fn heavy_counter_deadline_is_inclusive() {
    for (now, works) in [(2000, true), (2001, false)] {
        let c = config();
        let mut w = world(&c);
        dodge(&mut w, &c);
        assert_eq!(
            !triggered(&hit(&mut w, &c, 0, false, Some(1000), now)).is_empty(),
            works
        );
    }
}
#[test]
fn heavy_counter_dodged_contact_preserves_window() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .stats
        .insert("dodge".into(), 100);
    let mut w = world(&c);
    dodge(&mut w, &c);
    assert!(triggered(&hit(&mut w, &c, 100, false, Some(1000), 1100)).is_empty());
    assert!(window(&w).is_some());
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 500);
}
#[test]
fn heavy_counter_extra_ignores_metal_and_uses_defense_once() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .stats
        .insert("def".into(), 100);
    let mut w = world(&c);
    dodge(&mut w, &c);
    let expected = (120.0
        * (1.0
            - (100.0 / (100.0 + c.params.defense_scale * f64::from(c.defender_realm_atk)))
                .min(c.defense_cap)))
    .floor() as u32;
    assert_eq!(
        triggered(&hit(&mut w, &c, 0, true, Some(1000), 1100)),
        vec![expected]
    );
}
#[test]
fn heavy_counter_bonus_death_uses_single_score_drop_chain() {
    let c = config();
    let mut w = world(&c);
    dodge(&mut w, &c);
    let e = hit(&mut w, &c, 499, false, Some(1000), 1100);
    assert_eq!(triggered(&e), vec![1]);
    assert_eq!(w.kills, 1);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        1
    );
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::KillScored { .. }))
            .count(),
        1
    );
}
#[test]
fn heavy_counter_base_lethal_does_not_repeat_death() {
    let c = config();
    let mut w = world(&c);
    dodge(&mut w, &c);
    let e = hit(&mut w, &c, 999, false, Some(1000), 1100);
    assert_eq!(triggered(&e), vec![0]);
    assert_eq!(w.kills, 1);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        1
    );
}
#[test]
fn heavy_caster_death_before_due_cancels_without_release() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    let e = hit(&mut w, &c, 999, false, Some(100), 400);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::HeavyCancelled {
            reason: HeavyCancelReason::OwnerDied,
            ..
        }
    )));
    assert!(advance(&mut w, &c, 400).is_empty());
    assert_eq!(w.hp, 1000);
}
#[test]
fn heavy_epoch_change_invalidates_cast_and_counter() {
    for cast in [false, true] {
        let c = config();
        let mut w = world(&c);
        if cast {
            start(&mut w, &c, 0, 100);
        } else {
            dodge(&mut w, &c);
        }
        w.pve.as_mut().unwrap().monsters[0].boss_epoch += 1;
        assert!(triggered(&hit(&mut w, &c, 0, false, Some(1000), 1100)).is_empty());
        advance(&mut w, &c, 1200);
        assert!(!w.pve.as_ref().unwrap().has_heavy_state());
    }
}
#[test]
fn heavy_counter_expiry_does_not_cancel_next_cast() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .heavy
        .as_mut()
        .unwrap()
        .cooldown_ms = 1;
    let mut w = world(&c);
    dodge(&mut w, &c);
    assert_eq!(start(&mut w, &c, 0, 1100).len(), 1);
    advance(&mut w, &c, 2100);
    assert!(window(&w).is_none());
    assert!(w.pve.as_ref().unwrap().monsters[0].heavy_cast.is_some());
}
#[test]
fn heavy_return_and_close_clear_counter_without_refund() {
    let c = config();
    let mut w = world(&c);
    dodge(&mut w, &c);
    let mut e = vec![];
    let a = w.pve.as_mut().unwrap();
    a.cancel_heavy_for(0, HeavyCancelReason::Returning, 1100, &mut e);
    assert!(a.counter.is_none());
    assert_eq!(a.monsters[0].spirit, 80);
    assert_eq!(a.monsters[0].heavy_ready, 5400);
    assert!(!e.is_empty());
}
#[test]
fn heavy_many_casts_cancel_in_one_bounded_event() {
    let c = config();
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    let profile = a.monsters[0].profile.clone();
    for i in 1..32 {
        let mut m = Monster::new(
            format!("caster-{i}"),
            "verification-phase-boss".into(),
            profile.clone(),
            [600.0, 400.0],
            None,
        );
        m.aggro = true;
        a.monsters.push(m);
    }
    for i in 0..32 {
        start(&mut w, &c, i, 100);
    }
    assert_eq!(w.close_interaction_event_budget(), 1);
    let e = w.close_container_interaction(200);
    assert_eq!(e.len(), 1);
    assert!(matches!(&e[0],Event::HeavyCancelled{casts,counter:None,..}if casts.len()==32));
}
#[test]
fn heavy_large_jump_releases_before_any_swept_impact() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    let e = advance(&mut w, &c, 10000);
    assert!(e.iter().any(|e| matches!(e, Event::HeavyCommitted { .. })));
    assert_eq!(w.hp, 1000);
    advance(&mut w, &c, 20000);
    assert_eq!(w.hp, 900);
}
#[test]
fn heavy_actual_world_movement_can_dodge_and_window_is_projected() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    w.movement = [0.0, 1.0];
    for now in (130..=1030).step_by(30) {
        w.pve_tick(&c, now, vec![]);
    }
    let v = w.pve_snapshot(&c, 1030);
    assert!(v.position[1] > 450.0);
    assert!(v.pve.unwrap().counter.is_some());
    assert_eq!(w.hp, 1000);
}
#[test]
fn heavy_counter_wrong_target_contact_cannot_consume() {
    let c = config();
    let mut w = world(&c);
    dodge(&mut w, &c);
    let mut a = w.pve.take().unwrap();
    let profile = a.monsters[0].profile.clone();
    a.monsters.push(Monster::new(
        "other".into(),
        "verification-phase-boss".into(),
        profile,
        [800.0, 600.0],
        None,
    ));
    a.now = 1100;
    let mut e = vec![];
    a.hurt_monster_contact(&mut w, &c, 1, (100, false, Some(1000)), &mut e);
    assert!(triggered(&e).is_empty());
    assert!(a.counter.is_some());
    w.pve = Some(a);
    assert_eq!(
        triggered(&hit(&mut w, &c, 0, false, Some(1000), 1200)),
        vec![120]
    );
}
#[test]
fn heavy_multiple_finish_stably_and_do_not_extend_existing_window() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .heavy
        .as_mut()
        .unwrap()
        .cooldown_ms = 1;
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    let profile = a.monsters[0].profile.clone();
    let mut m = Monster::new(
        "a-heavy".into(),
        "verification-phase-boss".into(),
        profile,
        [600.0, 400.0],
        None,
    );
    m.aggro = true;
    a.monsters.push(m);
    start(&mut w, &c, 0, 100);
    start(&mut w, &c, 1, 100);
    advance(&mut w, &c, 400);
    w.position = [400.0, 500.0];
    let e = advance(&mut w, &c, 1000);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::CounterOpened { .. }))
            .count(),
        1
    );
    let before = window(&w).unwrap();
    assert_eq!(before.monster_id, "a-heavy");
    start(&mut w, &c, 0, 1100);
    advance(&mut w, &c, 1400);
    w.position = [400.0, 650.0];
    advance(&mut w, &c, 2000);
    assert_eq!(window(&w).unwrap(), before);
}
#[test]
fn heavy_player_projectile_kill_precedes_same_step_release() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 100);
    w.pve.as_mut().unwrap().shots.push(Shot {
        strict_target: None,
        status_source: None,
        hit_statuses: vec![],
        rounds: vec![],
        manual_started: Some(100),
        position: [500.0, 400.0],
        direction: [1.0, 0.0],
        remaining: 200.0,
        speed: 10000.0,
        radius: 1.0,
        raw: 999,
        owner: None,
        metal: false,
        mark: false,
        expires: 2000,
        created: 100,
    });
    let e = w.pve_tick(&c, 400, vec![]);
    assert_eq!(w.kills, 1);
    assert!(!e.iter().any(|e| matches!(e, Event::HeavyCommitted { .. })));
    assert_eq!(w.hp, 1000);
}
#[test]
fn heavy_phase_and_other_action_gates_preserve_random() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .heavy
        .as_mut()
        .unwrap()
        .min_phase_index = 2;
    let mut w = world(&c);
    let rng = w.rng;
    assert!(start(&mut w, &c, 0, 100).is_empty());
    w.pve.as_mut().unwrap().monsters[0].boss_phase = 1;
    for mode in 0..3 {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.casting = (mode == 0).then_some((0, 2000));
        m.heal = (mode == 1).then_some((2000, 0, 10));
        m.recovery = if mode == 2 { 1000 } else { 0 };
        assert!(start(&mut w, &c, 0, 100).is_empty());
    }
    assert_eq!(w.rng, rng);
}
