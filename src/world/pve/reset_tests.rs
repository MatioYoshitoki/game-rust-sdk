use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-13")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = [1100.0, 1000.0];
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    c.params.resource_ttl_ms = 1000;
    c.params.runtime.resource_sweep_ms = 1000;
    let p = c.pve_mut();
    p.width = 64;
    p.height = 64;
    p.cells_rle = vec![(0, 4096)];
    p.monster_positions.clear();
    p.extra_spawns.clear();
    for (k, v) in [
        ("hp", 300),
        ("spirit", 100),
        ("atk", 0),
        ("def", 0),
        ("spd", 0),
        ("crit", 0),
        ("dodge", 0),
    ] {
        p.stats.insert(k.into(), v);
    }
    p.drop_table=serde_json::from_value(serde_json::json!({"guaranteed":[{"itemId":"huo-ling","quantityMin":2,"quantityMax":2}],"random":[]})).unwrap();
    p.combat.cast_chance = 0.0;
    p.combat.heal.hp_threshold = 0.0;
    p.combat.melee_range = 1.0;
    for t in p.additional_templates.values_mut() {
        t.stats = p.stats.clone();
        t.combat.cast_chance = 0.0;
        t.combat.heal.hp_threshold = 0.0;
        t.combat.melee_range = 1.0;
        if let Some(a) = &mut t.area {
            a.cast_chance = 0.0;
        }
        if let Some(h) = &mut t.heavy {
            h.cast_chance = 0.0;
            h.windup_ms = 300;
            h.travel_ms = 600;
        }
        if let Some(s) = &mut t.support {
            s.cast_chance = 0.0;
        }
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
fn world(c: &Config) -> World {
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
    prepare(&mut w, 100);
    w
}
fn prepare(w: &mut World, now: u64) {
    let mut a = w.pve.take().unwrap();
    a.prepare_encounters(w, now, &mut vec![]);
    w.pve = Some(a);
}
fn advance(w: &mut World, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.now = now;
    a.advance_encounter_resets(w, now, &mut e);
    w.pve = Some(a);
    e
}
fn hit(w: &mut World, c: &Config, i: usize, raw: u32, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.hurt_monster_contact(w, c, i, (raw, false, Some(now)), &mut e);
    a.advance_encounters(w, &mut e);
    w.pve = Some(a);
    e
}
fn reset(w: &mut World) {
    w.position = [900.0, 400.0];
    advance(w, 200);
    advance(w, 1200);
}
fn reenter(w: &mut World) {
    w.position = [400.0, 400.0];
    advance(w, 1300);
    prepare(w, 1300);
}
fn round(w: &World) -> u32 {
    w.pve.as_ref().unwrap().encounters[0].round
}
fn score(w: &World, c: &Config) -> ScoreSummary {
    c.score_summary(&w.pve.as_ref().unwrap().score_tally)
}
fn shot(a: &Arena, owner: Option<usize>) -> Shot {
    Shot {
        strict_target: None,
        status_source: None,
        hit_statuses: vec![],
        rounds: a.round_stamp(),
        manual_started: Some(100),
        position: [400.0, 400.0],
        direction: [1.0, 0.0],
        remaining: 1000.0,
        speed: 10000.0,
        radius: 1.0,
        raw: 50,
        owner,
        metal: false,
        mark: false,
        expires: 10000,
        created: 100,
    }
}
#[test]
fn reset_continuous_outside_keeps_original_deadline() {
    let c = config();
    let mut w = world(&c);
    w.position = [900.0, 400.0];
    let e = advance(&mut w, 200);
    assert!(matches!(
        &e[0],
        Event::EncounterResetScheduled {
            resets_at_ms: 1200,
            ..
        }
    ));
    advance(&mut w, 700);
    assert_eq!(w.pve.as_ref().unwrap().encounters[0].reset_at, Some(1200));
    assert_eq!(round(&w), 1);
    advance(&mut w, 1200);
    assert_eq!(round(&w), 2);
}
#[test]
fn reset_radius_boundary_and_deadline_return_cancel() {
    for now in [1199, 1200] {
        let c = config();
        let mut w = world(&c);
        w.position = [900.0, 400.0];
        advance(&mut w, 200);
        w.position = [800.0, 400.0];
        let e = advance(&mut w, now);
        assert_eq!(round(&w), 1);
        assert!(
            e.iter()
                .any(|e| matches!(e, Event::EncounterResetCancelled { .. }))
        );
        assert!(w.pve.as_ref().unwrap().encounters[0].reset_at.is_none());
    }
}
#[test]
fn reset_dormant_and_completed_do_not_start_timer() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    hit(&mut w, &c, 1, 999, 100);
    assert_eq!(
        w.pve.as_ref().unwrap().encounters[0].phase,
        EncounterPhase::Completed
    );
    w.position = [900.0, 400.0];
    assert!(advance(&mut w, 10000).is_empty());
    assert_eq!(round(&w), 1);
}
#[test]
fn reset_full_rebuild_restores_enemy_state_and_monotone_identity() {
    let c = config();
    let mut w = world(&c);
    let m = &mut w.pve.as_mut().unwrap().monsters[1];
    m.hp = 40;
    m.spirit = 3;
    m.boss_phase = 1;
    m.boss_epoch = 7;
    m.support_used = 4;
    m.support_ready = 9000;
    m.cooldown = [9000, 9000];
    m.heavy_ready = 9000;
    m.area_ready = 9000;
    m.heavy_sequence = 12;
    m.area_sequence = 13;
    m.support_sequence = 14;
    m.recovery = 9000;
    reset(&mut w);
    assert_eq!(round(&w), 2);
    reenter(&mut w);
    let m = &w.pve.as_ref().unwrap().monsters[1];
    assert_eq!(
        (m.hp, m.spirit, m.boss_phase, m.boss_epoch),
        (300, 100, 0, 8)
    );
    assert_eq!(
        (
            m.support_used,
            m.support_ready,
            m.heavy_ready,
            m.area_ready,
            m.cooldown
        ),
        (0, 0, 0, 0, [0, 0])
    );
    assert_eq!(
        (m.heavy_sequence, m.area_sequence, m.support_sequence),
        (12, 13, 14)
    );
    assert!(!m.returning && !m.aggro && m.path.is_empty());
}
#[test]
fn reset_return_before_due_preserves_partial_cost_and_stage() {
    let c = config();
    let mut w = world(&c);
    let m = &mut w.pve.as_mut().unwrap().monsters[1];
    m.hp = 40;
    m.spirit = 3;
    m.boss_phase = 1;
    m.support_used = 4;
    m.heavy_ready = 9000;
    w.position = [900.0, 400.0];
    advance(&mut w, 200);
    w.position = [400.0, 400.0];
    advance(&mut w, 800);
    let m = &w.pve.as_ref().unwrap().monsters[1];
    assert_eq!(
        (m.hp, m.spirit, m.boss_phase, m.support_used, m.heavy_ready),
        (40, 3, 1, 4, 9000)
    );
    assert_eq!(round(&w), 1);
}
#[test]
fn reset_dead_member_revives_only_after_new_entry() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    reset(&mut w);
    assert!(
        w.pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .all(|m| !m.enabled && m.hp == 0)
    );
    reenter(&mut w);
    assert!(
        w.pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .all(|m| m.enabled && m.hp == 300)
    );
}
#[test]
fn reset_repeated_kill_keeps_first_score_and_item_budget() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    let first = score(&w, &c);
    let resources = w.resources.iter().filter(|r| !r.view.orb).count();
    assert_eq!(resources, 1);
    reset(&mut w);
    reenter(&mut w);
    let e = hit(&mut w, &c, 0, 999, 1400);
    assert_eq!(score(&w, &c), first);
    assert_eq!(
        w.resources.iter().filter(|r| !r.view.orb).count(),
        resources
    );
    assert_eq!(w.pve.as_ref().unwrap().actual_kills(&c), 2);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::KillScored {
            score: 0,
            kills: 2,
            ..
        }
    )));
}
#[test]
fn reset_first_unpicked_loot_and_expired_loot_never_remint() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    w.resources.clear();
    w.pve.as_mut().unwrap().resource_expiry.clear();
    w.pve.as_mut().unwrap().resource_items.clear();
    reset(&mut w);
    reenter(&mut w);
    hit(&mut w, &c, 0, 999, 1400);
    assert!(w.resources.iter().all(|r| r.view.orb));
}
#[test]
fn reset_repeated_recovery_orb_ids_are_unique() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    reset(&mut w);
    reenter(&mut w);
    hit(&mut w, &c, 0, 999, 1400);
    let ids: Vec<_> = w
        .resources
        .iter()
        .filter(|r| r.view.orb)
        .map(|r| r.view.id.clone())
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert!(ids[0].contains("r1") && ids[1].contains("r2"));
}
#[test]
fn reset_repeat_skips_loot_random_consumption() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    reset(&mut w);
    reenter(&mut w);
    let before = w.rng;
    w.random();
    let expected = w.rng;
    w.rng = before;
    hit(&mut w, &c, 0, 999, 1400);
    assert_eq!(w.rng, expected);
}
#[test]
fn reset_current_round_real_deaths_required_for_completion() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    reset(&mut w);
    reenter(&mut w);
    hit(&mut w, &c, 1, 999, 1400);
    assert_ne!(
        w.pve.as_ref().unwrap().encounters[0].phase,
        EncounterPhase::Completed
    );
    hit(&mut w, &c, 0, 999, 1500);
    assert_eq!(
        w.pve.as_ref().unwrap().encounters[0].phase,
        EncounterPhase::Completed
    );
    assert_eq!(score(&w, &c).kills, 2);
    assert_eq!(w.pve.as_ref().unwrap().actual_kills(&c), 3);
}
#[test]
fn reset_summary_and_frozen_body_conserve_three_deaths_two_rewards() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    reset(&mut w);
    reenter(&mut w);
    hit(&mut w, &c, 0, 999, 1400);
    hit(&mut w, &c, 1, 999, 1500);
    w.desired_outcome = Some(Outcome::Extraction);
    let b = w.pve_settlement(&c);
    if let Business::CommitSettlement {
        reset_progress,
        scoring,
        map_progress,
        ..
    } = b
    {
        let s = reset_progress.unwrap();
        assert_eq!(s.actual_kills, 3);
        assert!(c.valid_reset_progress(Some(&s), scoring.as_ref(), map_progress.as_ref(), 5000));
        assert_eq!(s.encounters[0].defeats[0].deaths, 2);
    } else {
        panic!();
    }
}
#[test]
fn reset_prior_player_shot_ignores_rebuilt_members() {
    let c = config();
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    a.shots.push(shot(a, None));
    reset(&mut w);
    reenter(&mut w);
    w.pve_tick(&c, 1400, vec![]);
    w.pve_tick(&c, 1430, vec![]);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 300);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 300);
}
#[test]
fn reset_new_player_shot_hits_new_round() {
    let c = config();
    let mut w = world(&c);
    reset(&mut w);
    reenter(&mut w);
    let a = w.pve.as_mut().unwrap();
    let mut s = shot(a, None);
    s.created = 1300;
    a.shots.push(s);
    w.pve_tick(&c, 1400, vec![]);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 250);
}
#[test]
fn reset_uncommitted_old_skill_keeps_acceptance_round() {
    let c = config();
    let mut w = world(&c);
    w.pending = Some(PendingAction {
        ground_position: None,
        unit_target: None,
        selected_skill: None,
        rounds: w.pve.as_ref().unwrap().round_stamp(),
        accepted_at: 100,
        id: "old-skill".into(),
        due_ms: 1400,
        skill: true,
        direction: [1.0, 0.0],
    });
    reset(&mut w);
    reenter(&mut w);
    w.pve_tick(&c, 1400, vec![]);
    for now in (1430..=2000).step_by(30) {
        w.pve_tick(&c, now, vec![]);
    }
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| m.hp == 300));
}
#[test]
fn reset_old_shot_can_still_hit_unchanged_static_target() {
    let c = config();
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    let mut m = Monster::new(
        "static".into(),
        "x".into(),
        a.monsters[0].profile.clone(),
        [900.0, 400.0],
        None,
    );
    m.next_decision = u64::MAX;
    a.monsters.push(m);
    a.shots.push(shot(a, None));
    reset(&mut w);
    reenter(&mut w);
    w.pve_tick(&c, 1400, vec![]);
    w.pve_tick(&c, 1430, vec![]);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 300);
    assert_eq!(w.pve.as_ref().unwrap().monsters[2].hp, 250);
}
#[test]
fn reset_clears_only_group_enemy_projectiles() {
    let c = config();
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    a.shots.push(shot(a, Some(0)));
    a.shots.push(shot(a, Some(99)));
    reset(&mut w);
    assert_eq!(w.pve.as_ref().unwrap().shots.len(), 1);
    assert_eq!(w.pve.as_ref().unwrap().shots[0].owner, Some(99));
}
#[test]
fn reset_clears_only_group_sword_mark_layers() {
    let c = config();
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    a.marks = 2;
    a.mark_sources = vec![0, 99];
    a.marks_until = 9000;
    reset(&mut w);
    let a = w.pve.as_ref().unwrap();
    assert_eq!(a.marks, 1);
    assert_eq!(a.mark_sources, vec![99]);
    assert_eq!(a.marks_until, 9000);
}
#[test]
fn reset_player_inventory_health_and_cooldowns_are_not_refunded() {
    let c = config();
    let mut w = world(&c);
    w.hp = 700;
    w.spirit = 40;
    w.inventory = 3;
    w.basic_ready_at = 9000;
    w.skill_ready_at = 10000;
    w.pve.as_mut().unwrap().support_interrupt_ready = 8000;
    reset(&mut w);
    assert_eq!(
        (
            w.hp,
            w.spirit,
            w.inventory,
            w.basic_ready_at,
            w.skill_ready_at
        ),
        (700, 40, 3, 9000, 10000)
    );
    assert_eq!(w.pve.as_ref().unwrap().support_interrupt_ready, 8000);
}
#[test]
fn reset_protection_inside_is_not_spatial_departure() {
    let c = config();
    let mut w = world(&c);
    w.protected = true;
    w.ready = false;
    w.foreground = false;
    assert!(advance(&mut w, 10000).is_empty());
    assert_eq!(round(&w), 1);
}
#[test]
fn reset_protection_outside_keeps_logical_countdown() {
    let c = config();
    let mut w = world(&c);
    w.position = [900.0, 400.0];
    advance(&mut w, 200);
    w.protected = true;
    w.ready = false;
    w.foreground = false;
    advance(&mut w, 1200);
    assert_eq!(round(&w), 2);
}
#[test]
fn reset_death_and_terminal_do_not_spawn_new_round() {
    for outcome in [Outcome::Death, Outcome::Extraction] {
        let c = config();
        let mut w = world(&c);
        w.position = [900.0, 400.0];
        advance(&mut w, 200);
        w.desired_outcome = Some(outcome);
        let e = advance(&mut w, 1200);
        assert_eq!(round(&w), 1);
        assert!(
            e.iter()
                .any(|e| matches!(e, Event::EncounterResetCancelled { .. }))
        );
    }
}
#[test]
fn reset_withdrawal_home_does_not_individually_refill() {
    let c = config();
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[0].hp = 80;
    w.position = [900.0, 400.0];
    w.pve_tick(&c, 200, vec![]);
    w.pve_tick(&c, 500, vec![]);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!(m.hp, 80);
    assert!(m.returning);
    assert_eq!(round(&w), 1);
}
#[test]
fn reset_waiting_and_later_wave_restart_from_first_wave() {
    let mut c = config();
    let p = c.pve_mut();
    let e = &mut p.encounters[0];
    let a = e.monsters[0].clone();
    let b = e.monsters[1].clone();
    e.monsters.clear();
    e.required_monster_ids.clear();
    e.waves = vec![
        crate::config::EncounterWave {
            id: "first".into(),
            delay_ms: 200,
            monsters: vec![a],
            required_monster_ids: vec!["a".into()],
        },
        crate::config::EncounterWave {
            id: "second".into(),
            delay_ms: 2000,
            monsters: vec![b],
            required_monster_ids: vec!["b".into()],
        },
    ];
    let mut w = world(&c);
    assert_eq!(
        w.pve.as_ref().unwrap().encounters[0].phase,
        EncounterPhase::Waiting
    );
    prepare(&mut w, 300);
    hit(&mut w, &c, 0, 999, 350);
    assert_eq!(w.pve.as_ref().unwrap().encounters[0].current_wave, 1);
    reset(&mut w);
    reenter(&mut w);
    let s = &w.pve.as_ref().unwrap().encounters[0];
    assert_eq!(
        (s.current_wave, s.completed_waves, s.ready_at),
        (0, 0, Some(1500))
    );
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| !m.enabled));
    prepare(&mut w, 1500);
    assert!(
        w.pve.as_ref().unwrap().monsters[0].enabled && !w.pve.as_ref().unwrap().monsters[1].enabled
    );
}
#[test]
fn reset_blocked_initial_spawn_remains_atomic_after_retry() {
    let c = config();
    let mut w = world(&c);
    reset(&mut w);
    w.position = [600.0, 400.0];
    advance(&mut w, 1300);
    prepare(&mut w, 1300);
    assert_eq!(
        w.pve.as_ref().unwrap().encounters[0].phase,
        EncounterPhase::Spawning
    );
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| !m.enabled));
    w.position = [400.0, 400.0];
    prepare(&mut w, 1600);
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| m.enabled));
}
#[test]
fn reset_completed_other_node_and_ground_items_are_retained() {
    let mut c = config();
    let p = c.pve_mut();
    let mut e = p.encounters[0].clone();
    e.id = "done-node".into();
    e.trigger.center = [1200.0, 400.0];
    e.reward_chest_ids.clear();
    e.monsters[0].id = "c".into();
    e.monsters[0].position = [1200.0, 400.0];
    e.monsters[1].id = "d".into();
    e.monsters[1].position = [1250.0, 400.0];
    e.required_monster_ids = vec!["c".into(), "d".into()];
    p.encounters.push(e);
    p.map_objective
        .as_mut()
        .unwrap()
        .required_encounter_ids
        .push("done-node".into());
    let mut w = world(&c);
    w.position = [1100.0, 400.0];
    prepare(&mut w, 100);
    hit(&mut w, &c, 2, 999, 100);
    hit(&mut w, &c, 3, 999, 100);
    assert_eq!(
        w.pve.as_ref().unwrap().encounters[1].phase,
        EncounterPhase::Completed
    );
    w.position = [400.0, 400.0];
    hit(&mut w, &c, 0, 999, 100);
    let ids: Vec<_> = w.resources.iter().map(|r| r.view.id.clone()).collect();
    reset(&mut w);
    let a = w.pve.as_ref().unwrap();
    assert_eq!(a.encounters[1].phase, EncounterPhase::Completed);
    assert_eq!(a.encounters[1].round, 1);
    assert_eq!(
        ids,
        w.resources
            .iter()
            .map(|r| r.view.id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(score(&w, &c).kills, 3);
}
#[test]
fn reset_zero_credit_first_death_has_items_but_no_score_farming() {
    let mut c = config();
    let p = c.pve_mut();
    let id = p.template_id.clone();
    let mut t = p.build_profiles()[&id].as_ref().clone();
    t.kill_credit = false;
    t.kill_score = Some(0);
    p.additional_templates.insert("no-credit".into(), t);
    p.encounters[0].monsters[0].template_id = "no-credit".into();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    let count = w.resources.iter().filter(|r| !r.view.orb).count();
    reset(&mut w);
    reenter(&mut w);
    hit(&mut w, &c, 0, 999, 1400);
    assert_eq!(w.kills, 0);
    assert_eq!(score(&w, &c).kill_score, 0);
    assert_eq!(count, w.resources.iter().filter(|r| !r.view.orb).count());
    assert_eq!(w.pve.as_ref().unwrap().actual_kills(&c), 2);
}
#[test]
fn reset_cancels_support_area_heavy_counter_and_keeps_sequences() {
    let mut c = config();
    let h = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .heavy
        .as_mut()
        .unwrap();
    h.cast_chance = 1.0;
    h.range = 500.0;
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    a.monsters[1].aggro = true;
    assert!(a.try_start_heavy(&mut w, &c, 1, 100, &mut vec![]));
    w.pve = Some(a);
    w.position = [900.0, 400.0];
    let e = advance(&mut w, 200);
    assert!(e.iter().any(|e| matches!(e, Event::HeavyCancelled { .. })));
    advance(&mut w, 1200);
    reenter(&mut w);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].heavy_sequence, 1);

    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    a.monsters[1].aggro = true;
    assert!(a.try_start_heavy(&mut w, &c, 1, 100, &mut vec![]));
    a.advance_heavies(&mut w, &c, 400, &mut vec![]);
    w.position = [400.0, 500.0];
    a.advance_heavies(&mut w, &c, 1000, &mut vec![]);
    assert!(a.counter.is_some());
    w.pve = Some(a);
    w.position = [900.0, 400.0];
    let e = advance(&mut w, 1100);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::HeavyCancelled {
            counter: Some(_),
            ..
        }
    )));
    advance(&mut w, 2100);
    assert!(w.pve.as_ref().unwrap().counter.is_none());
    let mut c = config();
    let p = c.pve_mut();
    p.encounters[0].monsters[0].template_id = "verification-forest-support".into();
    let s = p
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap();
    s.cast_chance = 1.0;
    s.range = 500.0;
    let a = p
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .area
        .as_mut()
        .unwrap();
    a.cast_chance = 1.0;
    a.min_phase_index = 1;
    a.cost = 15;
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    a.monsters[0].aggro = true;
    a.monsters[1].aggro = true;
    a.monsters[1].hp = 100;
    assert!(a.try_start_support(&mut w, &c, 0, 100, &mut vec![]));
    assert!(a.try_start_area(&mut w, &c, 1, 100, &mut vec![]));
    assert_eq!(a.monsters[1].spirit, 85);
    w.pve = Some(a);
    w.position = [900.0, 400.0];
    let e = advance(&mut w, 200);
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::SupportCancelled { .. }))
    );
    assert!(e.iter().any(|e| matches!(e, Event::AreaCancelled { .. })));
    advance(&mut w, 1200);
    reenter(&mut w);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].spirit, 100);
}
#[test]
fn reset_close_countdown_uses_one_reserved_event() {
    let c = config();
    let mut w = world(&c);
    w.position = [900.0, 400.0];
    advance(&mut w, 200);
    assert_eq!(w.close_interaction_event_budget(), 1);
    let e = w.close_container_interaction(300);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::EncounterResetCancelled { .. }))
            .count(),
        1
    );
    assert!(!w.pve.as_ref().unwrap().has_reset_countdown());
}
#[test]
fn reset_same_step_old_projectile_cannot_finish_previous_round() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 999, 100);
    w.position = [900.0, 400.0];
    advance(&mut w, 200);
    let a = w.pve.as_mut().unwrap();
    let mut s = shot(a, None);
    s.raw = 999;
    s.position = [680.0, 400.0];
    a.shots.push(s);
    let e = w.pve_tick(&c, 1200, vec![]);
    assert_eq!(round(&w), 2);
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::EncounterCompleted { .. }))
    );
    assert_eq!(score(&w, &c).kills, 1);
}
#[test]
fn reset_plain_configuration_has_no_round_projection_or_rng_change() {
    let c = crate::config::default_catalog()
        .active("b3-12")
        .unwrap()
        .as_ref()
        .clone();
    let w = World::new(
        &c,
        ParticipantFacts {
            principal: "p".into(),
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
    assert!(w.pve.as_ref().unwrap().round_stamp().is_empty());
    assert!(w.pve.as_ref().unwrap().reset_summary(&c).is_none());
}
#[test]
fn reset_many_elapsed_steps_produce_only_one_rebuild() {
    let c = config();
    let mut w = world(&c);
    w.position = [900.0, 400.0];
    advance(&mut w, 200);
    let e = advance(&mut w, 90000);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::EncounterReset { .. }))
            .count(),
        1
    );
    assert_eq!(round(&w), 2);
    assert!(advance(&mut w, 180000).is_empty());
}

#[test]
fn multi_reset_cancels_secondary_preparation_before_any_debit() {
    let mut c = config();
    let extra = super::player_skills_tests::config();
    c.schema_version = 4;
    c.mechanism_version = 26;
    c.asset_contract = extra.asset_contract;
    c.result_contract = extra.result_contract;
    c.additional_skills = extra.additional_skills;
    c.additional_skills[0].windup_ms = 5000;
    c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
    c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
    let mut w = world(&c);
    let spirit = w.spirit;
    w.action(
        &Action::Cast {
            skill_id: c.additional_skills[0].id.clone(),
            direction: [1., 0.],
        },
        "old-secondary",
        &c,
        100,
    )
    .unwrap();
    reset(&mut w);
    let e = w.pve_tick(&c, 1230, vec![]);
    assert!(w.pending.is_none());
    assert!(e.contains(&Event::ActionInterrupted));
    assert_eq!(w.spirit, spirit);
    assert_eq!(w.skill_ready(&c, &c.additional_skills[0].id), 0);
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    assert!(w.snapshot(&c, 1230).statuses.is_empty());
}
#[test]
fn multi_cancelled_reset_countdown_keeps_same_round_preparation() {
    let mut c = config();
    c.schema_version = 4;
    c.mechanism_version = 26;
    let extra = super::player_skills_tests::config();
    c.asset_contract = extra.asset_contract;
    c.result_contract = extra.result_contract;
    c.skill.windup_ms = 5000;
    let mut w = world(&c);
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "same-round",
        &c,
        100,
    )
    .unwrap();
    w.position = [900., 400.];
    advance(&mut w, 200);
    w.position = [400., 400.];
    advance(&mut w, 300);
    let e = w.pve_tick(&c, 330, vec![]);
    assert!(w.pending.is_some());
    assert!(!e.contains(&Event::ActionInterrupted));
    assert_eq!(round(&w), 1);
    assert_eq!(w.spirit, 100);
}
#[test]
fn self_skill_reset_cancels_preparation_before_resource_or_effect_commit() {
    let mut c = config();
    let extra = super::self_skill_tests::config();
    c.schema_version = 4;
    c.mechanism_version = 27;
    c.asset_contract = extra.asset_contract;
    c.result_contract = extra.result_contract;
    c.additional_skills = extra.additional_skills;
    c.additional_skills[0].windup_ms = 5000;
    c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
    c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
    let mut w = world(&c);
    w.action(
        &Action::CastSelf {
            skill_id: c.additional_skills[0].id.clone(),
        },
        "old-self",
        &c,
        100,
    )
    .unwrap();
    reset(&mut w);
    let e = w.pve_tick(&c, 1230, vec![]);
    assert!(w.pending.is_none());
    assert!(e.contains(&Event::ActionInterrupted));
    assert_eq!(w.spirit, 100);
    assert_eq!(w.skill_ready(&c, &c.additional_skills[0].id), 0);
    assert!(w.snapshot(&c, 1230).statuses.is_empty());
}
#[test]
fn unit_skill_actual_reset_cancels_pending_and_new_round_rejects_old_reference() {
    let mut c = config();
    let extra = super::unit_skill_tests::config();
    c.schema_version = 4;
    c.mechanism_version = 28;
    c.asset_contract = extra.asset_contract;
    c.result_contract = extra.result_contract;
    c.skill = extra.skill;
    c.skill.windup_ms = 5000;
    c.additional_skills = extra.additional_skills;
    c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
    c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
    let mut w = world(&c);
    let old = w.pve.as_ref().unwrap().unit_reference(0).unwrap();
    w.action(
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: old.clone(),
        },
        "old-unit",
        &c,
        100,
    )
    .unwrap();
    reset(&mut w);
    let e = w.pve_tick(&c, 1230, vec![]);
    assert!(e.contains(&Event::ActionInterrupted));
    assert!(w.pending.is_none());
    assert_eq!(w.spirit, 100);
    assert_eq!(w.skill_ready_at, 0);
    reenter(&mut w);
    let new = w.pve.as_ref().unwrap().unit_reference(0).unwrap();
    assert_eq!(new.id, old.id);
    assert!(new.generation > old.generation);
    assert_eq!(
        w.action(
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: old
            },
            "stale-unit",
            &c,
            1300
        ),
        Err(Error::InvalidTarget)
    );
    assert!(
        w.action(
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: new
            },
            "new-unit",
            &c,
            1300
        )
        .is_ok()
    );
}
#[test]
fn unit_skill_reset_countdown_hides_and_rejects_target_without_switching() {
    let mut c = config();
    let extra = super::unit_skill_tests::config();
    c.schema_version = 4;
    c.mechanism_version = 28;
    c.asset_contract = extra.asset_contract;
    c.result_contract = extra.result_contract;
    c.skill = extra.skill;
    c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
    c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
    let mut w = world(&c);
    let t = w.pve.as_ref().unwrap().unit_reference(0).unwrap();
    w.position = [900., 400.];
    advance(&mut w, 200);
    assert!(w.pve.as_ref().unwrap().unit_reference(0).is_none());
    assert_eq!(
        w.action(
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: t
            },
            "resetting",
            &c,
            200
        ),
        Err(Error::InvalidTarget)
    );
    assert!(w.pending.is_none());
    assert_eq!(w.spirit, 100);
}

#[test]
fn ground_skill_actual_reset_cancels_preparing_and_committed_plans_without_new_round_damage() {
    for committed in [false, true] {
        let mut c = config();
        let extra = super::ground_skill_tests::config();
        c.schema_version = 4;
        c.mechanism_version = 29;
        c.asset_contract = extra.asset_contract;
        c.result_contract = extra.result_contract;
        c.skill = extra.skill;
        c.skill.windup_ms = if committed { 0 } else { 5000 };
        c.skill.warning_ms = 5000;
        c.additional_skills = extra.additional_skills;
        c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
        c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
        let mut w = world(&c);
        w.action(
            &Action::CastGround {
                skill_id: c.skill.id.clone(),
                position: [600., 400.],
            },
            "ground-reset",
            &c,
            100,
        )
        .unwrap();
        if committed {
            w.pve_tick(&c, 120, vec![]);
            assert_eq!(w.pve.as_ref().unwrap().player_areas.len(), 1);
        }
        reset(&mut w);
        let e = w.pve_tick(&c, 1230, vec![]);
        assert!(w.pending.is_none());
        assert!(w.pve.as_ref().unwrap().player_areas.is_empty());
        assert_eq!(w.spirit, if committed { 88 } else { 100 });
        assert!(e.iter().any(|e| if committed {
            matches!(
                e,
                Event::PlayerAreaCancelled {
                    reason: PlayerAreaCancelReason::EncounterReset,
                    ..
                }
            )
        } else {
            matches!(e, Event::ActionInterrupted)
        }));
        reenter(&mut w);
        let hp: Vec<_> = w
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .map(|m| m.hp)
            .collect();
        w.pve_tick(&c, 6000, vec![]);
        assert_eq!(
            w.pve
                .as_ref()
                .unwrap()
                .monsters
                .iter()
                .map(|m| m.hp)
                .collect::<Vec<_>>(),
            hp
        );
    }
}

#[test]
fn field_skill_actual_reset_cancels_preparing_and_committed_plans_without_new_round_damage() {
    for committed in [false, true] {
        let mut c = config();
        let extra = super::field_skill_tests::config();
        c.schema_version = 4;
        c.mechanism_version = 30;
        c.asset_contract = extra.asset_contract;
        c.result_contract = extra.result_contract;
        c.skill = extra.skill;
        c.skill.windup_ms = if committed { 0 } else { 5000 };
        c.skill.warning_ms = 5000;
        c.additional_skills = extra.additional_skills;
        c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
        c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
        let mut w = world(&c);
        w.action(
            &Action::CastGround {
                skill_id: c.skill.id.clone(),
                position: [600., 400.],
            },
            "ground-reset",
            &c,
            100,
        )
        .unwrap();
        if committed {
            w.pve_tick(&c, 120, vec![]);
            assert_eq!(w.pve.as_ref().unwrap().player_fields.len(), 1);
        }
        reset(&mut w);
        let e = w.pve_tick(&c, 1230, vec![]);
        assert!(w.pending.is_none());
        assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
        assert_eq!(w.spirit, if committed { 88 } else { 100 });
        assert!(e.iter().any(|e| if committed {
            matches!(
                e,
                Event::PlayerFieldEnded {
                    reason: PlayerFieldEndReason::EncounterReset,
                    ..
                }
            )
        } else {
            matches!(e, Event::ActionInterrupted)
        }));
        reenter(&mut w);
        let hp: Vec<_> = w
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .map(|m| m.hp)
            .collect();
        w.pve_tick(&c, 6000, vec![]);
        assert_eq!(
            w.pve
                .as_ref()
                .unwrap()
                .monsters
                .iter()
                .map(|m| m.hp)
                .collect::<Vec<_>>(),
            hp
        );
    }
}

#[test]
fn contact_policy_actual_reset_cancels_preparing_and_committed_plans_without_new_round_damage() {
    for committed in [false, true] {
        let mut c = config();
        let extra = super::contact_policy_tests::config();
        c.schema_version = 4;
        c.mechanism_version = 31;
        c.asset_contract = extra.asset_contract;
        c.result_contract = extra.result_contract;
        c.skill = extra.skill;
        c.skill.windup_ms = if committed { 0 } else { 5000 };
        c.skill.warning_ms = 5000;
        c.additional_skills = extra.additional_skills;
        c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
        c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
        let mut w = world(&c);
        w.action(
            &Action::CastGround {
                skill_id: c.skill.id.clone(),
                position: [600., 400.],
            },
            "ground-reset",
            &c,
            100,
        )
        .unwrap();
        if committed {
            w.pve_tick(&c, 120, vec![]);
            assert_eq!(w.pve.as_ref().unwrap().player_fields.len(), 1);
        }
        reset(&mut w);
        let e = w.pve_tick(&c, 1230, vec![]);
        assert!(w.pending.is_none());
        assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
        assert_eq!(w.spirit, if committed { 88 } else { 100 });
        assert!(e.iter().any(|e| if committed {
            matches!(
                e,
                Event::PlayerFieldEnded {
                    reason: PlayerFieldEndReason::EncounterReset,
                    ..
                }
            )
        } else {
            matches!(e, Event::ActionInterrupted)
        }));
        reenter(&mut w);
        let hp: Vec<_> = w
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .map(|m| m.hp)
            .collect();
        w.pve_tick(&c, 6000, vec![]);
        assert_eq!(
            w.pve
                .as_ref()
                .unwrap()
                .monsters
                .iter()
                .map(|m| m.hp)
                .collect::<Vec<_>>(),
            hp
        );
    }
}

#[test]
fn piercing_actual_reset_cancels_preparing_and_committed_plans_without_new_round_damage() {
    for committed in [false, true] {
        let mut c = config();
        let extra = crate::ConfigCatalog::load(
            std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                .join("examples/piercing-host/catalog.json"),
        )
        .unwrap()
        .default_config()
        .as_ref()
        .clone();
        c.schema_version = 4;
        c.mechanism_version = 32;
        c.asset_contract = extra.asset_contract;
        c.result_contract = extra.result_contract;
        c.skill = extra.skill;
        c.skill.windup_ms = if committed { 0 } else { 5000 };
        c.skill.speed = 0.1;
        c.skill.projectile_ttl_ms = 60000;
        c.additional_skills = extra.additional_skills;
        c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
        c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
        let mut w = world(&c);
        w.action(
            &Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.],
            },
            "ground-reset",
            &c,
            100,
        )
        .unwrap();
        if committed {
            w.pve_tick(&c, 120, vec![]);
            assert_eq!(w.pve.as_ref().unwrap().piercing_projectiles.len(), 1);
        }
        reset(&mut w);
        let e = w.pve_tick(&c, 1230, vec![]);
        assert!(w.pending.is_none());
        assert!(w.pve.as_ref().unwrap().piercing_projectiles.is_empty());
        assert_eq!(w.spirit, if committed { 88 } else { 100 });
        assert!(e.iter().any(|e| if committed {
            matches!(
                e,
                Event::PiercingEnded {
                    reason: PiercingEndReason::EncounterReset,
                    ..
                }
            )
        } else {
            matches!(e, Event::ActionInterrupted)
        }));
        reenter(&mut w);
        let hp: Vec<_> = w
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .map(|m| m.hp)
            .collect();
        w.pve_tick(&c, 6000, vec![]);
        assert_eq!(
            w.pve
                .as_ref()
                .unwrap()
                .monsters
                .iter()
                .map(|m| m.hp)
                .collect::<Vec<_>>(),
            hp
        );
    }
}

#[test]
fn projectile_repeat_actual_reset_cancels_preparing_and_committed_plans_without_new_round_damage() {
    for committed in [false, true] {
        let mut c = config();
        let extra = crate::ConfigCatalog::load(
            std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                .join("examples/repeat-host/catalog.json"),
        )
        .unwrap()
        .default_config()
        .as_ref()
        .clone();
        c.schema_version = 4;
        c.mechanism_version = 33;
        c.asset_contract = extra.asset_contract;
        c.result_contract = extra.result_contract;
        c.skill = extra.skill;
        c.skill.windup_ms = if committed { 0 } else { 5000 };
        c.skill.speed = 0.1;
        c.skill.projectile_ttl_ms = 60000;
        c.additional_skills = extra.additional_skills;
        c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
        c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
        let mut w = world(&c);
        w.action(
            &Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.],
            },
            "ground-reset",
            &c,
            100,
        )
        .unwrap();
        if committed {
            w.pve_tick(&c, 120, vec![]);
            assert_eq!(w.pve.as_ref().unwrap().piercing_projectiles.len(), 1);
        }
        reset(&mut w);
        let e = w.pve_tick(&c, 1230, vec![]);
        assert!(w.pending.is_none());
        assert!(w.pve.as_ref().unwrap().piercing_projectiles.is_empty());
        assert_eq!(w.spirit, if committed { 88 } else { 100 });
        assert!(e.iter().any(|e| if committed {
            matches!(
                e,
                Event::PiercingEnded {
                    reason: PiercingEndReason::EncounterReset,
                    ..
                }
            )
        } else {
            matches!(e, Event::ActionInterrupted)
        }));
        reenter(&mut w);
        let hp: Vec<_> = w
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .map(|m| m.hp)
            .collect();
        w.pve_tick(&c, 6000, vec![]);
        assert_eq!(
            w.pve
                .as_ref()
                .unwrap()
                .monsters
                .iter()
                .map(|m| m.hp)
                .collect::<Vec<_>>(),
            hp
        );
    }
}

#[test]
fn line_field_actual_reset_cancels_preparing_and_committed_plans_without_new_round_damage() {
    for committed in [false, true] {
        let mut c = config();
        let extra = crate::ConfigCatalog::load(
            std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                .join("examples/line-field-host/catalog.json"),
        )
        .unwrap()
        .default_config()
        .as_ref()
        .clone();
        c.schema_version = 4;
        c.mechanism_version = 34;
        c.asset_contract = extra.asset_contract;
        c.result_contract = extra.result_contract;
        c.skill = extra.skill;
        c.skill.windup_ms = if committed { 0 } else { 5000 };
        c.skill.duration_ms = 60000;
        c.skill.contact_policy.as_mut().unwrap().max_total_hits = 128;

        c.additional_skills = extra.additional_skills;
        c.pve_mut().status_definitions = extra.pve.as_ref().unwrap().status_definitions.clone();
        c.pve_mut().status_rules = extra.pve.as_ref().unwrap().status_rules.clone();
        let mut w = world(&c);
        w.action(
            &Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.],
            },
            "ground-reset",
            &c,
            100,
        )
        .unwrap();
        if committed {
            w.pve_tick(&c, 120, vec![]);
            assert_eq!(w.pve.as_ref().unwrap().player_fields.len(), 1);
        }
        reset(&mut w);
        let e = w.pve_tick(&c, 1230, vec![]);
        assert!(w.pending.is_none());
        assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
        assert_eq!(w.spirit, if committed { 88 } else { 100 });
        assert!(e.iter().any(|e| if committed {
            matches!(
                e,
                Event::PlayerFieldEnded {
                    reason: PlayerFieldEndReason::EncounterReset,
                    ..
                }
            )
        } else {
            matches!(e, Event::ActionInterrupted)
        }));
        reenter(&mut w);
        let hp: Vec<_> = w
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .map(|m| m.hp)
            .collect();
        w.pve_tick(&c, 6000, vec![]);
        assert_eq!(
            w.pve
                .as_ref()
                .unwrap()
                .monsters
                .iter()
                .map(|m| m.hp)
                .collect::<Vec<_>>(),
            hp
        );
    }
}
