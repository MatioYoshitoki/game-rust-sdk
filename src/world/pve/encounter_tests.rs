use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-5")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [200.0, 400.0];
    c.extraction_point = [200.0, 400.0];
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.combat.cast_chance = 0.0;
    for key in ["atk", "spd", "crit", "dodge"] {
        p.stats.insert(key.into(), 0);
    }
    p.stats.insert("hp".into(), 60);
    let e = &mut p.encounters[0];
    e.trigger.center = [500.0, 400.0];
    e.trigger.radius = 100.0;
    for (i, m) in e.monsters.iter_mut().enumerate() {
        m.position = [600.0 + i as f64 * 100.0, 400.0];
    }
    p.chest_spawns[0].position = [900.0, 400.0];
    p.chest_spawns[1].position = [900.0, 600.0];
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
            def_points: 10,
            spd_points: 8,
            warehouse_quantity: 95,
        },
        7,
    );
    w.phase = Phase::Active;
    w.ready = true;
    w.foreground = true;
    w.joined = true;
    w.protected = false;
    w.active_at = Some(0);
    w
}
fn prepare(w: &mut World, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.prepare_encounters(w, now, &mut e);
    w.pve = Some(a);
    e
}
fn advance(w: &mut World) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_encounters(w, &mut e);
    w.pve = Some(a);
    e
}
fn view(w: &World) -> EncounterView {
    w.pve.as_ref().unwrap().encounter_views()[0].clone()
}
fn trigger(w: &mut World) {
    w.position = [450.0, 400.0];
    prepare(w, 30);
    assert_eq!(view(w).phase, EncounterPhase::Active);
}
fn kill(w: &mut World, c: &Config, i: usize) {
    let mut a = w.pve.take().unwrap();
    a.hurt_monster(w, c, i, 10000, false, &mut vec![]);
    w.pve = Some(a);
}
fn clear(w: &mut World, c: &Config) {
    for i in 0..3 {
        kill(w, c, i);
    }
    advance(w);
}
fn open() -> Action {
    Action::OpenChest {
        chest_id: "encounter-cache-1".into(),
        expected_generation: 1,
    }
}
#[test]
fn reserved_enemies_are_invisible_and_cannot_fight_before_trigger() {
    let c = config();
    let mut w = world(&c);
    let rng = w.rng;
    w.tick(&c, 30);
    let s = w.snapshot(&c, 30);
    assert!(s.pve.unwrap().monsters.is_empty());
    assert!(s.target.is_none());
    assert_eq!(w.rng, rng);
    assert_eq!(view(&w).defeated_required, 0);
    assert_eq!(w.pve.as_ref().unwrap().monsters.len(), 3);
}
#[test]
fn outside_and_inclusive_boundary_are_sampled_after_movement() {
    let c = config();
    let mut w = world(&c);
    w.position = [399.0, 400.0];
    assert!(prepare(&mut w, 0).is_empty());
    w.movement = [1.0, 0.0];
    w.tick(&c, 30);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
    let mut b = world(&c);
    b.position = [400.0, 400.0];
    prepare(&mut b, 30);
    assert_eq!(view(&b).phase, EncounterPhase::Active);
}
#[test]
fn spawn_inside_region_triggers_on_first_eligible_step() {
    let mut c = config();
    c.spawn = [450.0, 400.0];
    let mut w = world(&c);
    assert_eq!(view(&w).phase, EncounterPhase::Dormant);
    w.tick(&c, 30);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
}
#[test]
fn lifecycle_and_readiness_gate_activation() {
    let c = config();
    for mode in 0..7 {
        let mut w = world(&c);
        w.position = [450.0, 400.0];
        match mode {
            0 => w.protected = true,
            1 => w.ready = false,
            2 => w.foreground = false,
            3 => w.hp = 0,
            4 => w.phase = Phase::Draining,
            5 => w.desired_outcome = Some(Outcome::Death),
            _ => w.extraction_end = Some(2000),
        }
        assert!(prepare(&mut w, 30).is_empty());
        assert_eq!(view(&w).phase, EncounterPhase::Dormant);
    }
}
#[test]
fn repeated_entries_do_not_recreate_entities_or_events() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    let ids = view(&w).monster_ids;
    for p in [[200.0, 200.0], [450.0, 400.0], [200.0, 400.0]] {
        w.position = p;
        assert!(prepare(&mut w, 1000).is_empty());
        assert_eq!(view(&w).monster_ids, ids);
    }
    assert_eq!(w.pve.as_ref().unwrap().monsters.len(), 3);
}
#[test]
fn player_blocked_group_is_atomic_and_recovers_outside_region() {
    let c = config();
    let mut w = world(&c);
    w.position = [600.0, 400.0];
    let e = prepare(&mut w, 30);
    assert_eq!(view(&w).phase, EncounterPhase::Spawning);
    assert!(view(&w).monster_ids.is_empty());
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::EncounterSpawnBlocked { .. }))
    );
    assert!(prepare(&mut w, 230).is_empty());
    w.position = [200.0, 700.0];
    let e = prepare(&mut w, 430);
    assert_eq!(view(&w).monster_ids.len(), 3);
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::EncounterStarted { .. }))
    );
}
#[test]
fn living_static_enemy_blocks_spawn_until_actually_removed() {
    let mut c = config();
    c.pve_mut().monster_positions = vec![[600.0, 400.0]];
    let mut w = world(&c);
    w.position = [450.0, 400.0];
    prepare(&mut w, 30);
    assert_eq!(view(&w).phase, EncounterPhase::Spawning);
    kill(&mut w, &c, 0);
    prepare(&mut w, 230);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
}
#[test]
fn retry_uses_no_rng_and_background_preserves_pending_plan() {
    let c = config();
    let mut w = world(&c);
    w.position = [600.0, 400.0];
    let rng = w.rng;
    prepare(&mut w, 30);
    for now in [230, 430, 630] {
        prepare(&mut w, now);
    }
    assert_eq!(w.rng, rng);
    w.protected = true;
    w.position = [200.0, 700.0];
    assert!(prepare(&mut w, 830).is_empty());
    assert_eq!(view(&w).phase, EncounterPhase::Spawning);
    w.protected = false;
    prepare(&mut w, 1030);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
    assert_eq!(w.rng, rng);
}
#[test]
fn mixed_templates_have_independent_stats_and_bound_identity() {
    let mut c = config();
    let p = c.pve_mut();
    let mut t = p.build_profiles()[&p.template_id].as_ref().clone();
    t.name = "独立目标".into();
    t.stats.insert("hp".into(), 777);
    t.combat.radius = 20.0;
    p.additional_templates.insert("mixed".into(), t);
    p.encounters[0].monsters[1].template_id = "mixed".into();
    let mut w = world(&c);
    trigger(&mut w);
    let s = w.snapshot(&c, 30).pve.unwrap();
    assert_eq!(s.monsters[0].max_hp, 60);
    assert_eq!(s.monsters[1].max_hp, 777);
    assert_eq!(s.monsters[1].radius, Some(20.0));
    assert!(
        s.monsters
            .iter()
            .all(|m| m.encounter_id.as_deref() == Some("forest-clearing"))
    );
}
#[test]
fn partial_deaths_do_not_unlock_reward_or_advance_via_global_kills() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    kill(&mut w, &c, 0);
    w.kills = 999;
    advance(&mut w);
    assert_eq!(view(&w).defeated_required, 1);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
    w.position = [900.0, 400.0];
    assert_eq!(
        w.action(&open(), "locked", &c, 30),
        Err(Error::EncounterLocked)
    );
}
#[test]
fn non_encounter_death_does_not_satisfy_required_targets() {
    let mut c = config();
    c.pve_mut().monster_positions = vec![[200.0, 200.0]];
    let mut w = world(&c);
    trigger(&mut w);
    kill(&mut w, &c, 0);
    advance(&mut w);
    assert_eq!(view(&w).defeated_required, 0);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
}
#[test]
fn returning_targets_are_alive_and_leaving_region_does_not_reset() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    w.pve.as_mut().unwrap().monsters[0].returning = true;
    w.position = [200.0, 700.0];
    assert!(advance(&mut w).is_empty());
    assert_eq!(view(&w).phase, EncounterPhase::Active);
    assert_eq!(view(&w).defeated_required, 0);
}
#[test]
fn independent_encounters_only_release_their_own_chests() {
    let mut c = config();
    let p = c.pve_mut();
    let mut second = p.encounters[0].clone();
    second.id = "second".into();
    for m in &mut second.monsters {
        m.id = format!("second-{}", m.id);
        m.position[1] += 200.0;
    }
    second.required_monster_ids = second.monsters.iter().map(|m| m.id.clone()).collect();
    second.reward_chest_ids = vec!["encounter-cache-2".into()];
    p.encounters[0].reward_chest_ids = vec!["encounter-cache-1".into()];
    p.encounters.push(second);
    let mut w = world(&c);
    trigger(&mut w);
    clear(&mut w, &c);
    let a = w.pve.as_ref().unwrap();
    assert!(a.encounter_gate("encounter-cache-1").unwrap().completed);
    assert!(!a.encounter_gate("encounter-cache-2").unwrap().completed);
    assert_eq!(a.encounter_views()[1].defeated_required, 0);
}
#[test]
fn required_subset_completes_without_deleting_other_living_enemies() {
    let mut c = config();
    c.pve_mut().encounters[0].required_monster_ids.truncate(1);
    let mut w = world(&c);
    trigger(&mut w);
    kill(&mut w, &c, 0);
    advance(&mut w);
    assert_eq!(view(&w).phase, EncounterPhase::Completed);
    assert!(w.pve.as_ref().unwrap().monsters[1].hp > 0);
}
#[test]
fn noncredit_target_deaths_complete_and_keep_configured_loot() {
    let mut c = config();
    let p = c.pve_mut();
    let mut t = p.build_profiles()[&p.template_id].as_ref().clone();
    t.kill_credit = false;
    p.additional_templates.insert("free".into(), t);
    for m in &mut p.encounters[0].monsters {
        m.template_id = "free".into();
    }
    let mut w = world(&c);
    trigger(&mut w);
    clear(&mut w, &c);
    assert_eq!(w.kills, 0);
    assert_eq!(view(&w).phase, EncounterPhase::Completed);
    assert!(!w.resources.is_empty());
}
#[test]
fn completion_is_once_and_does_not_open_or_grant_container_loot() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    for i in 0..3 {
        kill(&mut w, &c, i);
    }
    let (rng, n, bag) = (w.rng, w.resources.len(), w.inventory);
    assert_eq!(
        advance(&mut w)
            .iter()
            .filter(|e| matches!(e, Event::EncounterCompleted { .. }))
            .count(),
        1
    );
    kill(&mut w, &c, 2);
    assert!(advance(&mut w).is_empty());
    assert_eq!((w.rng, w.resources.len(), w.inventory), (rng, n, bag));
    assert!(!w.pve.as_ref().unwrap().chest_views()[0].opened);
    assert_eq!(w.kills, 3);
}
#[test]
fn dormant_lock_and_completed_gate_reuse_explicit_seal_and_claim() {
    let c = config();
    let mut w = world(&c);
    w.position = [900.0, 400.0];
    assert_eq!(
        w.action(&open(), "dormant", &c, 0),
        Err(Error::EncounterLocked)
    );
    trigger(&mut w);
    clear(&mut w, &c);
    w.position = [900.0, 400.0];
    w.action(&open(), "unseal", &c, 30).unwrap();
    w.tick(&c, 2430);
    assert!(!w.pve.as_ref().unwrap().chest_views()[0].opened);
    w.action(&open(), "claim", &c, 2430).unwrap();
    assert!(w.pve.as_ref().unwrap().chest_views()[0].opened);
}
#[test]
fn container_respawn_preserves_completed_encounter_and_finite_storage() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    clear(&mut w, &c);
    w.position = [900.0, 400.0];
    w.action(&open(), "unseal", &c, 30).unwrap();
    w.tick(&c, 2430);
    w.action(&open(), "claim", &c, 2430).unwrap();
    w.tick(&c, 602430);
    assert_eq!(view(&w).phase, EncounterPhase::Completed);
    assert!(
        w.pve.as_ref().unwrap().chest_views()[0]
            .encounter
            .as_ref()
            .unwrap()
            .completed
    );
    assert_eq!(w.pve.as_ref().unwrap().chest_views()[0].generation, 2);
    assert_eq!(w.pve.as_ref().unwrap().monsters.len(), 3);
}
#[test]
fn death_and_close_do_not_complete_or_generate_pending_encounters() {
    let c = config();
    for mode in 0..6 {
        let mut w = world(&c);
        trigger(&mut w);
        for i in 0..3 {
            kill(&mut w, &c, i);
        }
        match mode {
            0 => w.hp = 0,
            1 => w.protected = true,
            2 => w.ready = false,
            3 => w.foreground = false,
            4 => w.phase = Phase::Draining,
            _ => w.desired_outcome = Some(Outcome::Death),
        }
        assert!(advance(&mut w).is_empty());
        assert_eq!(view(&w).phase, EncounterPhase::Active);
    }
    let mut b = world(&c);
    b.position = [450.0, 400.0];
    b.phase = Phase::Closed;
    assert!(prepare(&mut b, 100).is_empty());
    assert_eq!(view(&b).phase, EncounterPhase::Dormant);
}
#[test]
fn encounter_population_and_early_exit_assets_are_bounded() {
    let c = config();
    assert_eq!(c.max_kills(c.terminal_ms), 3);
    assert!(c.pve_loot_bound("xuan-tie", 3, c.terminal_ms) >= 3);
    let mut w = world(&c);
    w.action(&Action::Leave {}, "leave", &c, 30).unwrap();
    let b = w.pve_settlement(&c);
    if let Business::CommitSettlement {
        looted,
        items,
        cultivation,
        ..
    } = b
    {
        assert_eq!(looted, 0);
        assert!(items.is_empty());
        assert_eq!(cultivation, 0);
    } else {
        panic!("settlement")
    }
}
