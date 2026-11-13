use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-6")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [200.0, 400.0];
    c.extraction_point = c.spawn;
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
    e.waves[1].delay_ms = 300;
    for (i, m) in e.waves.iter_mut().flat_map(|w| &mut w.monsters).enumerate() {
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
fn advance(w: &mut World, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.advance_encounters(w, &mut e);
    w.pve = Some(a);
    e
}
fn view(w: &World) -> EncounterView {
    w.pve.as_ref().unwrap().encounter_views()[0].clone()
}
fn wave(w: &World) -> EncounterWaveView {
    view(w).wave.unwrap()
}
fn kill_id(w: &mut World, c: &Config, id: &str) {
    let mut a = w.pve.take().unwrap();
    let i = a.monsters.iter().position(|m| m.id == id).unwrap();
    a.hurt_monster(w, c, i, 10000, false, &mut vec![]);
    w.pve = Some(a);
}
fn clear_wave(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    for id in wave(w).required_monster_ids {
        kill_id(w, c, &id);
    }
    advance(w, now)
}
fn trigger(w: &mut World) {
    w.position = [450.0, 400.0];
    prepare(w, 100);
    assert_eq!(view(w).phase, EncounterPhase::Active);
}
fn finish(w: &mut World, c: &Config) {
    trigger(w);
    clear_wave(w, c, 500);
    prepare(w, 800);
    clear_wave(w, c, 900);
    assert_eq!(view(w).phase, EncounterPhase::Completed);
}
fn open(g: u32) -> Action {
    Action::OpenChest {
        chest_id: "wave-cache-1".into(),
        expected_generation: g,
    }
}
#[test]
fn all_waves_are_reserved_but_invisible_before_trigger() {
    let c = config();
    let w = world(&c);
    let s = w.snapshot(&c, 0);
    assert!(s.pve.unwrap().monsters.is_empty());
    assert_eq!(w.pve.as_ref().unwrap().monsters.len(), 3);
    assert_eq!(wave(&w).total, 2);
    assert_eq!(wave(&w).index, 1);
    assert_eq!(wave(&w).completed_waves, 0);
    assert!(wave(&w).ready_at_ms.is_none());
}
#[test]
fn zero_initial_delay_only_enables_first_wave_and_events_once() {
    let c = config();
    let mut w = world(&c);
    w.position = [450.0, 400.0];
    let e = prepare(&mut w, 100);
    assert_eq!(wave(&w).monster_ids.len(), 2);
    assert_eq!(view(&w).monster_ids.len(), 2);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::EncounterTriggered { .. }))
            .count(),
        1
    );
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::EncounterWaveScheduled { .. }))
            .count(),
        1
    );
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::EncounterWaveStarted { wave_index: 1, .. }))
            .count(),
        1
    );
    assert!(prepare(&mut w, 130).is_empty());
}
#[test]
fn initial_positive_delay_has_an_inclusive_authoritative_deadline() {
    let mut c = config();
    c.pve_mut().encounters[0].waves[0].delay_ms = 300;
    let mut w = world(&c);
    w.position = [450.0, 400.0];
    prepare(&mut w, 100);
    assert_eq!(view(&w).phase, EncounterPhase::Waiting);
    assert_eq!(wave(&w).ready_at_ms, Some(400));
    prepare(&mut w, 399);
    assert!(view(&w).monster_ids.is_empty());
    prepare(&mut w, 400);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
    assert_eq!(wave(&w).monster_ids.len(), 2);
}
#[test]
fn partial_required_deaths_and_global_score_cannot_advance_wave() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    kill_id(&mut w, &c, "forest-wave-1-wolf-1");
    w.kills = 999;
    assert!(advance(&mut w, 500).is_empty());
    assert_eq!(wave(&w).index, 1);
    assert_eq!(wave(&w).defeated_required, 1);
    assert_eq!(wave(&w).completed_waves, 0);
}
#[test]
fn last_required_death_schedules_next_wave_without_granting_reward() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    let e = clear_wave(&mut w, &c, 500);
    assert_eq!(view(&w).phase, EncounterPhase::Waiting);
    assert_eq!(wave(&w).index, 2);
    assert_eq!(wave(&w).completed_waves, 1);
    assert_eq!(wave(&w).ready_at_ms, Some(800));
    assert!(wave(&w).monster_ids.is_empty());
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::EncounterWaveCompleted { .. }))
            .count(),
        1
    );
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::EncounterCompleted { .. }))
    );
    assert!(
        !w.pve
            .as_ref()
            .unwrap()
            .encounter_gate("wave-cache-1")
            .unwrap()
            .completed
    );
}
#[test]
fn later_wave_starts_at_deadline_not_before() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    prepare(&mut w, 799);
    assert_eq!(view(&w).phase, EncounterPhase::Waiting);
    prepare(&mut w, 800);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
    assert_eq!(wave(&w).index, 2);
    assert_eq!(wave(&w).monster_ids, vec!["forest-wave-2-wolf-1"]);
}
#[test]
fn zero_later_delay_does_not_recursively_spawn_on_completion_step() {
    let mut c = config();
    c.pve_mut().encounters[0].waves[1].delay_ms = 0;
    let mut w = world(&c);
    trigger(&mut w);
    let e = clear_wave(&mut w, &c, 500);
    assert_eq!(view(&w).phase, EncounterPhase::Waiting);
    assert!(wave(&w).monster_ids.is_empty());
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::EncounterWaveStarted { wave_index: 2, .. }))
    );
    prepare(&mut w, 530);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
}
#[test]
fn repeated_steps_never_reschedule_or_reenable_a_completed_wave() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    for now in [510, 600, 799] {
        assert!(advance(&mut w, now).is_empty());
        assert!(prepare(&mut w, now).is_empty());
        assert_eq!(wave(&w).ready_at_ms, Some(800));
    }
    let e = prepare(&mut w, 800);
    assert_eq!(e.len(), 1);
    assert!(prepare(&mut w, 900).is_empty());
    assert_eq!(w.pve.as_ref().unwrap().monsters.len(), 3);
}
#[test]
fn later_wave_blocked_by_player_is_atomic_and_retries_without_rng() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    w.position = [800.0, 400.0];
    let rng = w.rng;
    let e = prepare(&mut w, 800);
    assert_eq!(view(&w).phase, EncounterPhase::Spawning);
    assert!(wave(&w).monster_ids.is_empty());
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::EncounterWaveSpawnBlocked { wave_index: 2, .. }))
    );
    assert!(prepare(&mut w, 1000).is_empty());
    w.position = [200.0, 700.0];
    prepare(&mut w, 1200);
    assert_eq!(wave(&w).monster_ids.len(), 1);
    assert_eq!(w.rng, rng);
}
#[test]
fn blocked_notification_budget_resets_for_each_wave() {
    let c = config();
    let mut w = world(&c);
    w.position = [600.0, 400.0];
    let first = prepare(&mut w, 100);
    assert!(
        first
            .iter()
            .any(|e| matches!(e, Event::EncounterWaveSpawnBlocked { wave_index: 1, .. }))
    );
    w.position = [450.0, 400.0];
    prepare(&mut w, 300);
    clear_wave(&mut w, &c, 500);
    w.position = [800.0, 400.0];
    let second = prepare(&mut w, 800);
    assert!(
        second
            .iter()
            .any(|e| matches!(e, Event::EncounterWaveSpawnBlocked { wave_index: 2, .. }))
    );
    assert!(prepare(&mut w, 1000).is_empty());
}
#[test]
fn shared_coordinates_keep_distinct_entities_and_old_death_cannot_complete_new_wave() {
    let mut c = config();
    c.pve_mut().encounters[0].waves[1].monsters[0].position = [600.0, 400.0];
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    prepare(&mut w, 800);
    let rng = w.rng;
    kill_id(&mut w, &c, "forest-wave-1-wolf-1");
    assert_eq!(w.rng, rng);
    assert!(advance(&mut w, 900).is_empty());
    assert_eq!(wave(&w).defeated_required, 0);
    assert_eq!(view(&w).monster_ids.len(), 3);
    assert_eq!(
        w.snapshot(&c, 900)
            .pve
            .unwrap()
            .monsters
            .iter()
            .filter(|m| m.hp > 0)
            .count(),
        1
    );
}
#[test]
fn nonrequired_old_enemy_remains_and_can_block_new_wave() {
    let mut c = config();
    let e = &mut c.pve_mut().encounters[0];
    e.waves[0].required_monster_ids.truncate(1);
    e.waves[1].monsters[0].position = [700.0, 400.0];
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    prepare(&mut w, 800);
    assert_eq!(view(&w).phase, EncounterPhase::Spawning);
    kill_id(&mut w, &c, "forest-wave-1-wolf-2");
    assert!(advance(&mut w, 900).is_empty());
    prepare(&mut w, 1000);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
    assert_eq!(wave(&w).index, 2);
    assert_eq!(wave(&w).defeated_required, 0);
}
#[test]
fn static_deaths_and_old_wave_deaths_do_not_contribute_to_current_targets() {
    let mut c = config();
    c.pve_mut().monster_positions = vec![[200.0, 200.0]];
    let mut w = world(&c);
    trigger(&mut w);
    kill_id(&mut w, &c, "yao-wolf-1");
    advance(&mut w, 300);
    assert_eq!(wave(&w).defeated_required, 0);
    clear_wave(&mut w, &c, 500);
    prepare(&mut w, 800);
    kill_id(&mut w, &c, "forest-wave-1-wolf-2");
    advance(&mut w, 900);
    assert_eq!(wave(&w).defeated_required, 0);
    assert_eq!(wave(&w).index, 2);
}
#[test]
fn noncredit_targets_in_all_waves_complete_without_score_inference() {
    let mut c = config();
    let p = c.pve_mut();
    let mut t = p.build_profiles()[&p.template_id].as_ref().clone();
    t.kill_credit = false;
    p.additional_templates.insert("free".into(), t);
    for m in p.encounters[0]
        .waves
        .iter_mut()
        .flat_map(|w| &mut w.monsters)
    {
        m.template_id = "free".into();
    }
    let mut w = world(&c);
    finish(&mut w, &c);
    assert_eq!(w.kills, 0);
    assert_eq!(wave(&w).completed_waves, 2);
    assert!(!w.resources.is_empty());
}
#[test]
fn future_wave_uses_its_own_template_stats_and_identity() {
    let mut c = config();
    let p = c.pve_mut();
    let mut t = p.build_profiles()[&p.template_id].as_ref().clone();
    t.stats.insert("hp".into(), 777);
    t.combat.radius = 20.0;
    p.additional_templates.insert("later".into(), t);
    p.encounters[0].waves[1].monsters[0].template_id = "later".into();
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    prepare(&mut w, 800);
    let s = w.snapshot(&c, 800).pve.unwrap();
    let m = s.monsters.last().unwrap();
    assert_eq!(m.max_hp, 777);
    assert_eq!(m.radius, Some(20.0));
    assert_eq!(m.wave_id.as_deref(), Some("forest-wave-2"));
}
#[test]
fn different_encounters_advance_independently() {
    let mut c = config();
    let p = c.pve_mut();
    let mut e = p.encounters[0].clone();
    e.id = "other".into();
    for wave in &mut e.waves {
        for m in &mut wave.monsters {
            m.id = format!("other-{}", m.id);
            m.position[1] += 250.0;
        }
        for id in &mut wave.required_monster_ids {
            *id = format!("other-{id}");
        }
    }
    e.reward_chest_ids = vec!["wave-cache-2".into()];
    p.encounters[0].reward_chest_ids = vec!["wave-cache-1".into()];
    p.encounters.push(e);
    let mut w = world(&c);
    finish(&mut w, &c);
    let views = w.pve.as_ref().unwrap().encounter_views();
    assert_eq!(views[0].phase, EncounterPhase::Completed);
    assert_eq!(views[1].wave.as_ref().unwrap().index, 1);
    assert_eq!(views[1].defeated_required, 0);
    assert!(
        !w.pve
            .as_ref()
            .unwrap()
            .encounter_gate("wave-cache-2")
            .unwrap()
            .completed
    );
}
#[test]
fn final_completion_is_once_and_does_not_open_or_grant_container_loot() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    prepare(&mut w, 800);
    for id in wave(&w).required_monster_ids {
        kill_id(&mut w, &c, &id);
    }
    let state = (w.rng, w.inventory, w.resources.len());
    let e = advance(&mut w, 900);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::EncounterCompleted { .. }))
            .count(),
        1
    );
    assert_eq!(wave(&w).completed_waves, 2);
    assert_eq!((w.rng, w.inventory, w.resources.len()), state);
    assert!(advance(&mut w, 1000).is_empty());
    assert!(!w.pve.as_ref().unwrap().chest_views()[0].opened);
}
#[test]
fn waiting_reward_is_locked_and_final_reward_requires_seal_then_explicit_claim() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    w.position = [900.0, 400.0];
    assert_eq!(
        w.action(&open(1), "waiting-open", &c, 600),
        Err(Error::EncounterLocked)
    );
    prepare(&mut w, 800);
    clear_wave(&mut w, &c, 900);
    w.action(&open(1), "unseal", &c, 900).unwrap();
    w.tick(&c, 3300);
    assert!(!w.pve.as_ref().unwrap().chest_views()[0].opened);
    w.action(&open(1), "claim", &c, 3300).unwrap();
    assert!(w.pve.as_ref().unwrap().chest_views()[0].opened);
}
#[test]
fn protected_wait_preserves_deadline_and_resumes_once_after_it() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    w.protected = true;
    prepare(&mut w, 900);
    assert_eq!(view(&w).phase, EncounterPhase::Waiting);
    assert_eq!(wave(&w).ready_at_ms, Some(800));
    w.protected = false;
    w.ready = false;
    prepare(&mut w, 1100);
    assert_eq!(view(&w).phase, EncounterPhase::Waiting);
    w.ready = true;
    prepare(&mut w, 1200);
    assert_eq!(view(&w).phase, EncounterPhase::Active);
    assert_eq!(wave(&w).index, 2);
    assert!(prepare(&mut w, 1300).is_empty());
}
#[test]
fn repeated_region_entries_and_active_background_preserve_wave_progress() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    kill_id(&mut w, &c, "forest-wave-1-wolf-1");
    let before = wave(&w);
    w.action(&Action::Background {}, "background", &c, 300)
        .unwrap();
    prepare(&mut w, 500);
    assert_eq!(wave(&w), before);
    w.action(&Action::Foreground {}, "foreground", &c, 600)
        .unwrap();
    w.action(
        &Action::CombatReady {
            publication_id: "unit-view".into(),
        },
        "ready",
        &c,
        600,
    )
    .unwrap();
    for p in [[200.0, 700.0], [450.0, 400.0]] {
        w.position = p;
        assert!(prepare(&mut w, 700).is_empty());
        assert_eq!(wave(&w), before);
    }
}
#[test]
fn death_close_and_terminal_deadline_never_spawn_following_wave() {
    let c = config();
    for mode in 0..4 {
        let mut w = world(&c);
        trigger(&mut w);
        clear_wave(&mut w, &c, 500);
        match mode {
            0 => w.hp = 0,
            1 => w.phase = Phase::Closed,
            2 => w.desired_outcome = Some(Outcome::Death),
            _ => w.foreground = false,
        }
        assert!(prepare(&mut w, 900).is_empty());
        assert_eq!(view(&w).phase, EncounterPhase::Waiting);
    }
    let mut c = config();
    c.terminal_ms = 600;
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 300);
    w.tick(&c, 600);
    assert_eq!(w.desired_outcome, Some(Outcome::Death));
    assert_eq!(wave(&w).index, 2);
    assert!(wave(&w).monster_ids.is_empty());
}
#[test]
fn earlier_ground_drops_remain_and_cross_wave_resource_ids_are_unique() {
    let c = config();
    let mut w = world(&c);
    trigger(&mut w);
    clear_wave(&mut w, &c, 500);
    let before: Vec<_> = w.resources.iter().map(|r| r.view.id.clone()).collect();
    prepare(&mut w, 800);
    clear_wave(&mut w, &c, 900);
    let after: std::collections::BTreeSet<_> =
        w.resources.iter().map(|r| r.view.id.clone()).collect();
    assert!(before.iter().all(|id| after.contains(id)));
    assert_eq!(after.len(), w.resources.len());
    assert!(after.iter().any(|id| id.contains("wave-1")));
    assert!(after.iter().any(|id| id.contains("wave-2")));
}
#[test]
fn recovery_population_and_loot_upper_bound_include_every_future_wave() {
    let mut c = config();
    let p = c.pve_mut();
    p.drop_table = crate::config::LootTable {
        guaranteed: vec![crate::config::LootRule {
            item_id: "xuan-tie".into(),
            chance: None,
            quantity_min: 2,
            quantity_max: 2,
        }],
        random: vec![],
    };
    p.treasure_rules.as_mut().unwrap().loot = crate::config::LootTable {
        guaranteed: vec![],
        random: vec![],
    };
    assert_eq!(c.max_kills(0), 3);
    assert_eq!(c.pve_loot_bound("xuan-tie", 3, c.terminal_ms), 6);
}
#[test]
fn one_and_three_wave_data_use_the_same_sequential_mechanism() {
    for count in [1, 3] {
        let mut c = config();
        let e = &mut c.pve_mut().encounters[0];
        if count == 1 {
            e.waves.truncate(1);
        } else {
            let mut next = e.waves[1].clone();
            next.id = "third".into();
            next.monsters[0].id = "third-target".into();
            next.monsters[0].position = [900.0, 600.0];
            next.required_monster_ids = vec!["third-target".into()];
            next.delay_ms = 600;
            e.waves.push(next);
        }
        let mut w = world(&c);
        trigger(&mut w);
        let mut now = 500;
        for index in 1..=count {
            assert_eq!(wave(&w).index, index);
            clear_wave(&mut w, &c, now);
            if index < count {
                now = wave(&w).ready_at_ms.unwrap();
                prepare(&mut w, now);
                now += 100;
            }
        }
        assert_eq!(view(&w).phase, EncounterPhase::Completed);
        assert_eq!(wave(&w).completed_waves, count);
    }
}
