use super::*;
pub(super) fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-9")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = c.spawn;
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.monster_positions.clear();
    p.encounters.clear();
    p.extra_spawns = vec![crate::config::MonsterSpawn {
        id: "area-owner".into(),
        template_id: "verification-phase-boss".into(),
        position: [500.0, 400.0],
    }];
    p.chest_spawns[0].position = [800.0, 400.0];
    p.chest_spawns[1].position = [800.0, 650.0];
    let t = p
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    for (k, v) in [
        ("hp", 300),
        ("spirit", 100),
        ("atk", 100),
        ("def", 0),
        ("spd", 0),
        ("crit", 0),
        ("dodge", 0),
    ] {
        t.stats.insert(k.into(), v);
    }
    t.combat.melee_range = 1.0;
    for phase in &mut t.boss.as_mut().unwrap().phases {
        phase.cast_chance = 0.0;
    }
    let a = t.area.as_mut().unwrap();
    a.windup_ms = 300;
    a.cooldown_ms = 3000;
    a.recovery_ms = 300;
    a.damage_bps = 10000;
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
    }
    w
}
pub(super) fn start(w: &mut World, c: &Config, i: usize, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.try_start_area(w, c, i, now, &mut e);
    w.pve = Some(a);
    e
}
pub(super) fn resolve(w: &mut World, c: &Config, i: usize, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_area(w, c, i, now, &mut e);
    w.pve = Some(a);
    e
}
fn hurt(w: &mut World, c: &Config, raw: u32) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hurt_monster(w, c, 0, raw, false, &mut e);
    w.pve = Some(a);
    e
}
fn warning(w: &World) -> AreaWarningView {
    w.pve.as_ref().unwrap().monsters[0].area_view().unwrap()
}
fn result(e: &[Event]) -> (bool, u32) {
    e.iter()
        .find_map(|e| {
            if let Event::AreaResolved { hit, damage, .. } = e {
                Some((*hit, *damage))
            } else {
                None
            }
        })
        .unwrap()
}
fn tile(c: &mut Config, vision: bool, projectile: bool) {
    let p = c.pve_mut();
    let mut t = p.palette[0].clone();
    t.blocks_vision = vision;
    t.blocks_projectiles = projectile;
    t.walkable = true;
    p.palette = vec![p.palette[0].clone(), t];
    let i = 12 * 32 + 14;
    p.cells_rle = vec![(0, i), (1, 1), (0, 1023 - i)];
}
#[test]
fn start_locks_point_deadline_and_pays_once() {
    let c = config();
    let mut w = world(&c);
    let e = start(&mut w, &c, 0, 0);
    assert!(matches!(
        e.as_slice(),
        [Event::AreaStarted { cast_id: 1, .. }]
    ));
    let a = warning(&w);
    assert_eq!(
        (a.center, a.started_at_ms, a.resolves_at_ms),
        ([400.0, 400.0], 0, 300)
    );
    assert!(start(&mut w, &c, 0, 1).is_empty());
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!((m.spirit, m.area_ready), (88, 3000));
}
#[test]
fn before_equal_after_deadline_and_repeated_steps_resolve_once() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    assert!(resolve(&mut w, &c, 0, 299).is_empty());
    assert_eq!(w.hp, 1000);
    assert_eq!(result(&resolve(&mut w, &c, 0, 300)), (true, 100));
    assert_eq!(w.hp, 900);
    assert!(resolve(&mut w, &c, 0, 301).is_empty());
}
#[test]
fn fixed_circle_uses_inclusive_player_footprint_boundary() {
    let c = config();
    for (x, hit) in [(511.999, true), (512.0, true), (512.001, false)] {
        let mut w = world(&c);
        start(&mut w, &c, 0, 0);
        w.position = [x, 400.0];
        assert_eq!(result(&resolve(&mut w, &c, 0, 300)).0, hit);
    }
}
#[test]
fn world_movement_precedes_same_step_area_judgement() {
    let mut c = config();
    c.pve_mut().extra_spawns[0].position = [700.0, 400.0];
    for (inside, direction, hit) in [(true, 1.0, false), (false, -1.0, true)] {
        let mut w = world(&c);
        start(&mut w, &c, 0, 0);
        w.position = [if inside { 511.0 } else { 513.0 }, 400.0];
        w.movement = [direction, 0.0];
        let e = w.tick(&c, 300);
        assert_eq!(result(&e).0, hit);
    }
}
#[test]
fn range_boundary_is_checked_only_at_start() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .area
        .as_mut()
        .unwrap()
        .range = 100.0;
    let mut w = world(&c);
    assert!(!start(&mut w, &c, 0, 0).is_empty());
    w.position = [390.0, 400.0];
    assert!(result(&resolve(&mut w, &c, 0, 300)).0);
    let mut w = world(&c);
    w.position = [399.999, 400.0];
    assert!(start(&mut w, &c, 0, 0).is_empty());
}
#[test]
fn vision_blocker_prevents_start_without_spending_or_rng() {
    let mut c = config();
    tile(&mut c, true, false);
    let mut w = world(&c);
    let rng = w.rng;
    assert!(start(&mut w, &c, 0, 0).is_empty());
    assert_eq!(w.rng, rng);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 100);
}
#[test]
fn projectile_only_blocker_prevents_effect_but_not_start() {
    let mut c = config();
    tile(&mut c, false, true);
    c.pve_mut().extra_spawns[0].position = [700.0, 400.0];
    let mut w = world(&c);
    assert!(!start(&mut w, &c, 0, 0).is_empty());
    w.position = [500.0, 400.0];
    assert_eq!(result(&resolve(&mut w, &c, 0, 300)), (false, 0));
}
#[test]
fn vision_only_blocker_does_not_block_committed_area_effect() {
    let mut c = config();
    tile(&mut c, true, false);
    c.pve_mut().extra_spawns[0].position = [500.0, 500.0];
    let mut w = world(&c);
    assert!(!start(&mut w, &c, 0, 0).is_empty());
    w.position = [500.0, 400.0];
    assert!(result(&resolve(&mut w, &c, 0, 300)).0);
}
#[test]
fn miss_preserves_cost_and_cooldown_without_damage_rng() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    let rng = w.rng;
    w.position = [400.0, 600.0];
    assert_eq!(result(&resolve(&mut w, &c, 0, 300)), (false, 0));
    assert_eq!(w.rng, rng);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!((m.spirit, m.area_ready), (88, 3000));
}
#[test]
fn configured_zero_and_full_probability_choose_start() {
    for (chance, count) in [(0.0, 0), (1.0, 1)] {
        let mut c = config();
        c.pve_mut()
            .additional_templates
            .get_mut("verification-phase-boss")
            .unwrap()
            .area
            .as_mut()
            .unwrap()
            .cast_chance = chance;
        let mut w = world(&c);
        assert_eq!(start(&mut w, &c, 0, 0).len(), count);
    }
}
#[test]
fn phase_gate_waits_for_real_threshold_then_uses_same_ability() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .area
        .as_mut()
        .unwrap()
        .min_phase_index = 2;
    let mut w = world(&c);
    let rng = w.rng;
    assert!(start(&mut w, &c, 0, 0).is_empty());
    assert_eq!(w.rng, rng);
    hurt(&mut w, &c, 200);
    assert_eq!(start(&mut w, &c, 0, 0).len(), 1);
}
#[test]
fn actor_resource_and_lifecycle_gates_do_not_roll_probability() {
    for mode in 0..10 {
        let c = config();
        let mut w = world(&c);
        {
            let m = &mut w.pve.as_mut().unwrap().monsters[0];
            match mode {
                0 => m.spirit = 11,
                1 => m.area_ready = 1,
                2 => m.aggro = false,
                3 => m.returning = true,
                4 => m.casting = Some((0, 999)),
                5 => m.heal = Some((999, 0, 10)),
                6 => m.recovery = 1,
                7 => w.protected = true,
                8 => m.enabled = false,
                _ => w.desired_outcome = Some(Outcome::Extraction),
            }
        }
        let rng = w.rng;
        assert!(start(&mut w, &c, 0, 0).is_empty(), "mode {mode}");
        assert_eq!(w.rng, rng);
    }
}
#[test]
fn windup_stops_monster_movement_melee_and_legacy_spells() {
    let mut c = config();
    let t = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    t.stats.insert("spd".into(), 8);
    t.combat.melee_range = 200.0;
    t.boss.as_mut().unwrap().phases[0].cast_chance = 1.0;
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    w.pve.as_mut().unwrap().monsters[0].next_attack = 0;
    let e = w.tick(&c, 150);
    assert_eq!(w.hp, 1000);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].position, [500.0, 400.0]);
    assert!(!e.iter().any(|e| matches!(e, Event::MonsterSkill { .. })));
}
#[test]
fn nonlethal_damage_and_phase_change_keep_committed_warning() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    let before = warning(&w);
    let e = hurt(&mut w, &c, 200);
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::BossPhaseChanged { .. }))
    );
    assert!(!e.iter().any(|e| matches!(e, Event::AreaCancelled { .. })));
    assert_eq!(warning(&w), before);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 88);
}
#[test]
fn entering_return_cancels_before_deadline_and_keeps_paid_cd() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    w.position = [1000.0, 1000.0];
    let e = w.tick(&c, 30);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::AreaCancelled {
            reason: AreaCancelReason::Returning,
            ..
        }
    )));
    assert!(w.pve.as_ref().unwrap().monsters[0].area_cast.is_none());
    assert_eq!(
        (
            w.pve.as_ref().unwrap().monsters[0].spirit,
            w.pve.as_ref().unwrap().monsters[0].area_ready
        ),
        (88, 3000)
    );
}
#[test]
fn home_phase_reset_keeps_area_mp_and_cooldown() {
    let c = config();
    let mut w = world(&c);
    hurt(&mut w, &c, 200);
    start(&mut w, &c, 0, 0);
    w.position = [1000.0, 1000.0];
    w.tick(&c, 30);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!(
        (m.boss_phase, m.boss_epoch, m.spirit, m.area_ready),
        (0, 2, 88, 3000)
    );
}
#[test]
fn background_cancels_and_foreground_cannot_revive_old_warning() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    let e = w
        .action(&Action::Background {}, "background", &c, 100)
        .unwrap();
    assert!(matches!(
        &e[0],
        Event::AreaCancelled {
            reason: AreaCancelReason::Background,
            ..
        }
    ));
    w.tick(&c, 300);
    assert_eq!(w.hp, 1000);
    w.action(&Action::Foreground {}, "front", &c, 400).unwrap();
    w.action(
        &Action::CombatReady {
            publication_id: "fixture".into(),
        },
        "ready",
        &c,
        400,
    )
    .unwrap();
    assert!(start(&mut w, &c, 0, 400).is_empty());
    assert!(w.pve.as_ref().unwrap().monsters[0].area_cast.is_none());
}
#[test]
fn owner_death_cancels_without_impact_and_scores_only_one_kill() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    let e = hurt(&mut w, &c, 10000);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::AreaCancelled {
            reason: AreaCancelReason::OwnerDied,
            ..
        }
    )));
    assert!(resolve(&mut w, &c, 0, 300).is_empty());
    assert_eq!(w.hp, 1000);
    assert_eq!(
        c.score_summary(&w.pve.as_ref().unwrap().score_tally)
            .kill_score,
        100
    );
}
#[test]
fn lethal_area_cancels_other_instances_without_late_damage() {
    let mut c = config();
    let p = c.pve_mut();
    let mut s = p.extra_spawns[0].clone();
    s.id = "second".into();
    s.position = [700.0, 400.0];
    p.extra_spawns.push(s);
    let mut w = world(&c);
    w.hp = 10;
    start(&mut w, &c, 0, 0);
    start(&mut w, &c, 1, 0);
    let e = w.tick(&c, 300);
    assert_eq!(result(&e), (true, 10));
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::AreaResolved { .. }))
            .count(),
        1
    );
    assert!(e.iter().any(
        |e| matches!(e,Event::AreaCancelled{reason:AreaCancelReason::Closed,casts}if casts.len()==1)
    ));
    assert!(
        w.pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .all(|m| m.area_cast.is_none())
    );
}
#[test]
fn leave_and_absolute_terminal_clear_pending_warning() {
    for leave in [true, false] {
        let mut c = config();
        c.terminal_ms = 300;
        let mut w = world(&c);
        start(&mut w, &c, 0, 0);
        let e = if leave {
            w.action(&Action::Leave {}, "leave", &c, 30).unwrap()
        } else {
            w.tick(&c, 300)
        };
        assert!(e.iter().any(|e| matches!(
            e,
            Event::AreaCancelled {
                reason: AreaCancelReason::Closed,
                ..
            }
        )));
        assert!(!e.iter().any(|e| matches!(e, Event::AreaResolved { .. })));
        assert_eq!(w.desired_outcome, Some(Outcome::Death));
        assert!(w.tick(&c, 1000).is_empty());
    }
}
#[test]
fn repeated_cancel_and_resolve_do_not_repeat_events_or_refund() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    let mut e = vec![];
    let a = w.pve.as_mut().unwrap();
    a.cancel_all_areas(AreaCancelReason::Background, 100, &mut e);
    a.cancel_all_areas(AreaCancelReason::Background, 100, &mut e);
    assert_eq!(e.len(), 1);
    assert!(resolve(&mut w, &c, 0, 300).is_empty());
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 88);
}
#[test]
fn resolve_recovery_and_cd_boundary_prevent_same_step_recast() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    let e = w.tick(&c, 300);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::AreaStarted { .. }))
            .count(),
        0
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].recovery, 600);
    assert!(start(&mut w, &c, 0, 2999).is_empty());
    assert_eq!(start(&mut w, &c, 0, 3000).len(), 1);
    assert_eq!(warning(&w).cast_id, 2);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 76);
}
#[test]
fn zero_damage_is_one_hit_and_uses_existing_extraction_interrupt_rule() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .area
        .as_mut()
        .unwrap()
        .damage_bps = 0;
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    w.extraction_end = Some(300);
    let e = w.tick(&c, 300);
    assert_eq!(result(&e), (true, 0));
    assert_eq!(w.hp, 1000);
    assert!(w.extraction_end.is_none());
    assert!(e.iter().any(|e| matches!(e, Event::ExtractionInterrupted)));
}
#[test]
fn armor_critical_lifesteal_are_reused_without_adding_sword_marks() {
    let mut c = config();
    let t = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    t.stats.insert("crit".into(), 100);
    t.combat.lifesteal_ratio = 0.5;
    t.combat.mark_max = 2;
    let mut w = world(&c);
    w.facts.def_points = 10;
    w.pve.as_mut().unwrap().monsters[0].hp = 100;
    start(&mut w, &c, 0, 0);
    let expected = (100.0
        * (1.0
            - (10.0 / (10.0 + c.params.defense_scale * f64::from(c.defender_realm_atk)))
                .min(c.defense_cap))
        * c.params.critical_multiplier)
        .floor() as u32;
    assert_eq!(result(&resolve(&mut w, &c, 0, 300)), (true, expected));
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 100 + expected / 2);
    assert_eq!(w.pve.as_ref().unwrap().marks, 0);
}
#[test]
fn impact_precedes_same_step_seal_and_extraction_completion() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .area
        .as_mut()
        .unwrap()
        .windup_ms = 1200;
    let mut w = world(&c);
    w.position = [800.0, 400.0];
    w.action(
        &Action::OpenChest {
            chest_id: "area-cache-1".into(),
            expected_generation: 1,
        },
        "seal",
        &c,
        0,
    )
    .unwrap();
    start(&mut w, &c, 0, 0);
    w.extraction_end = Some(1200);
    let e = w.tick(&c, 1200);
    assert!(result(&e).0);
    assert!(w.desired_outcome.is_none());
    let p = w.snapshot(&c, 1200).pve.unwrap();
    assert_eq!(p.chests[0].seal.as_ref().unwrap().completed_layers, 0);
    assert!(!p.chests[0].seal.as_ref().unwrap().unlocking);
}
#[test]
fn guardian_replacement_cancels_old_identity_and_restarts_sequence() {
    let mut c = config();
    let mut g = crate::config::default_catalog()
        .active("b3-4")
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .guardian()
        .unwrap()
        .clone();
    g.template_id = "verification-phase-boss".into();
    g.chance = 1.0;
    g.spawn_offsets = vec![[64.0, 0.0]];
    let p = c.pve_mut();
    p.extra_spawns.clear();
    p.chest_spawns.truncate(1);
    let r = p.treasure_rules.as_mut().unwrap();
    r.template.guardian = Some(g);
    r.template.respawn_delay_ms = 1000;
    p.additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .combat
        .aggro_range = 1000.0;
    let mut w = world(&c);
    w.tick(&c, 0);
    let old = w.pve.as_ref().unwrap().monsters[0].id.clone();
    hurt(&mut w, &c, 10000);
    w.position = [800.0, 400.0];
    let open = || Action::OpenChest {
        chest_id: "area-cache-1".into(),
        expected_generation: 1,
    };
    w.action(&open(), "unseal", &c, 30).unwrap();
    w.tick(&c, 2430);
    w.action(&open(), "open", &c, 2430).unwrap();
    // Defensive cleanup also removes a late old-generation warning before the slot is reused.
    {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.area_cast = Some(area::AreaCast {
            id: 999,
            center: w.position,
            started: 2430,
            due: 99999,
        });
    }
    let e = w.tick(&c, 3430);
    assert!(e.iter().any(|e|matches!(e,Event::AreaCancelled{reason:AreaCancelReason::Replaced,casts}if casts[0].monster_id==old&&casts[0].cast_id==999)));
    assert_ne!(w.pve.as_ref().unwrap().monsters[0].id, old);
    assert_eq!(warning(&w).cast_id, 1);
}

#[test]
fn parallel_instances_lock_independent_points_and_resolve_independently() {
    let mut c = config();
    let p = c.pve_mut();
    let mut s = p.extra_spawns[0].clone();
    s.id = "second".into();
    s.position = [700.0, 400.0];
    p.extra_spawns.push(s);
    let mut w = world(&c);
    start(&mut w, &c, 0, 0);
    w.position = [450.0, 400.0];
    start(&mut w, &c, 1, 0);
    assert_eq!(warning(&w).center, [400.0, 400.0]);
    assert_eq!(
        w.pve.as_ref().unwrap().monsters[1]
            .area_view()
            .unwrap()
            .center,
        [450.0, 400.0]
    );
    let e = w.tick(&c, 300);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::AreaResolved { hit: true, .. }))
            .count(),
        2
    );
    assert_eq!(w.hp, 800);
}
#[test]
fn thirty_two_background_cancellations_are_one_bounded_event_plus_seal() {
    let mut c = config();
    c.pve_mut().chest_spawns[0].position = [400.0, 400.0];
    let mut w = world(&c);
    let profile = w.pve.as_ref().unwrap().monsters[0].profile.clone();
    for n in 1..32 {
        let mut m = Monster::new(
            format!("area-{n}"),
            "verification-phase-boss".into(),
            profile.clone(),
            [500.0, 400.0],
            None,
        );
        m.aggro = true;
        w.pve.as_mut().unwrap().monsters.push(m);
    }
    for i in 0..32 {
        start(&mut w, &c, i, 0);
    }
    w.action(
        &Action::OpenChest {
            chest_id: "area-cache-1".into(),
            expected_generation: 1,
        },
        "seal",
        &c,
        0,
    )
    .unwrap();
    assert_eq!(w.close_interaction_event_budget(), 2);
    let e = w
        .action(&Action::Background {}, "protect", &c, 100)
        .unwrap();
    assert!(
        e.iter()
            .any(|e| matches!(e,Event::AreaCancelled{casts,..}if casts.len()==32))
    );
    assert_eq!(e.len(), 2);
    assert!(
        w.pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .all(|m| m.spirit == 88 && m.area_ready == 3000 && m.area_cast.is_none())
    );
}
