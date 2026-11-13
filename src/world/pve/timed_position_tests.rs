use super::*;

fn fixture() -> (Config, World) {
    fixture_for("b3-15")
}

pub(super) fn fixture_for(scenario: &str) -> (Config, World) {
    let config = crate::config::default_catalog()
        .active(scenario)
        .unwrap()
        .as_ref()
        .clone();
    let mut world = World::new(
        &config,
        ParticipantFacts {
            principal: "player".into(),
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
    world.phase = Phase::Active;
    world.active_at = Some(0);
    world.ready = true;
    world.foreground = true;
    world.joined = true;
    world.protected = false;
    let mut arena = world.pve.take().unwrap();
    let center = arena.grid.config.encounters[2].trigger.center;
    world.position = [center[0], center[1] + 140.0];
    arena.prepare_encounters(&world, 100, &mut vec![]);
    assert_eq!(arena.encounters[2].phase, EncounterPhase::Active);
    world.pve = Some(arena);
    (config, world)
}

pub(super) fn advance(world: &mut World, config: &Config, time: u64) -> Vec<Event> {
    let mut arena = world.pve.take().unwrap();
    arena.now = time;
    let mut events = vec![];
    arena.advance_timed_position(world, config, time, &mut events);
    world.pve = Some(arena);
    events
}

fn view(world: &World) -> TimedPositionView {
    world.pve.as_ref().unwrap().timed_position_view().unwrap()
}

pub(super) fn start_warning(world: &mut World, config: &Config) -> TimedPositionWarningView {
    assert!(advance(world, config, 100).is_empty());
    let interval = config
        .pve
        .as_ref()
        .unwrap()
        .timed_position
        .as_ref()
        .unwrap()
        .interval_ms;
    let events = advance(world, config, 100 + interval);
    assert!(matches!(
        events.as_slice(),
        [Event::TimedPositionStarted { .. }]
    ));
    view(world).warning.unwrap()
}

pub(super) fn complete_side(world: &mut World, config: &Config, index: usize) {
    let mut arena = world.pve.take().unwrap();
    arena.encounters[index].phase = EncounterPhase::Active;
    for slot in arena.encounters[index].slots.clone() {
        arena.monsters[slot].enabled = true;
        arena.monsters[slot].hp = arena.monsters[slot].profile.stats["hp"];
        arena.hurt_monster_contact(world, config, slot, (1000, false, None), &mut vec![]);
    }
    arena.advance_encounters(world, &mut vec![]);
    assert_eq!(arena.encounters[index].phase, EncounterPhase::Completed);
    world.pve = Some(arena);
}

#[test]
fn timed_position_side_effects_are_independent() {
    for (west, east, duration, backlash) in [
        (false, false, 3000, 42),
        (true, false, 4500, 42),
        (false, true, 3000, 23),
        (true, true, 4500, 23),
    ] {
        let (config, mut world) = fixture();
        if west {
            complete_side(&mut world, &config, 0);
        }
        if east {
            complete_side(&mut world, &config, 1);
        }
        let warning = start_warning(&mut world, &config);
        assert_eq!(warning.resolves_at_ms - warning.started_at_ms, duration);
        assert_eq!(warning.failure_raw_damage, backlash);
        assert_eq!(view(&world).warning_bonus_active, west);
        assert_eq!(view(&world).failure_scale_active, east);
        assert!(advance(&mut world, &config, warning.resolves_at_ms - 1).is_empty());
        let events = advance(&mut world, &config, warning.resolves_at_ms);
        assert!(events.iter().any(|event| matches!(event,
            Event::TimedPositionResolved { target_id: None, damage, .. } if *damage == backlash
        )));
        assert_eq!(world.hp, 1000 - backlash);
        assert!(advance(&mut world, &config, warning.resolves_at_ms + 1).is_empty());
    }
}

#[test]
fn timed_position_side_completion_during_warning_updates_only_the_next_action() {
    let (config, mut world) = fixture();
    let warning = start_warning(&mut world, &config);
    complete_side(&mut world, &config, 0);
    complete_side(&mut world, &config, 1);
    assert_eq!(view(&world).warning, Some(warning.clone()));
    advance(&mut world, &config, warning.resolves_at_ms);
    assert_eq!(world.hp, 958);
    advance(&mut world, &config, 11100);
    let next = view(&world).warning.unwrap();
    assert_eq!(next.cycle, warning.cycle + 1);
    assert_eq!(next.resolves_at_ms - next.started_at_ms, 4500);
    assert_eq!(next.failure_raw_damage, 23);
}

#[test]
fn timed_position_uses_stable_tie_order_and_exact_boundary_through_death_chain() {
    let (config, mut world) = fixture();
    let center = config.pve.as_ref().unwrap().encounters[2].trigger.center;
    let warning = start_warning(&mut world, &config);
    world.position = [center[0] - 48.0, center[1]];
    let events = advance(&mut world, &config, warning.resolves_at_ms);
    assert!(events.iter().any(|event| matches!(event,
        Event::TimedPositionResolved { target_id: Some(id), damage: 300, .. } if id == "sanctum-1"
    )));
    assert_eq!(view(&world).successes, 1);

    advance(&mut world, &config, 11100);
    let next = view(&world).warning.unwrap();
    world.position = [center[0] - 96.0, center[1] + 90.0];
    let events = advance(&mut world, &config, next.resolves_at_ms);
    assert!(events.iter().any(|event| matches!(event,
        Event::TimedPositionResolved { target_id: Some(id), damage: 82, .. } if id == "sanctum-2"
    )));
    assert!(events.iter().any(|event| matches!(event,
        Event::KillScored { monster_id, score: 10, .. } if monster_id == "sanctum-2"
    )));
    assert_eq!(world.kills, 1);
    advance(&mut world, &config, 16600);
    let next = view(&world).warning.unwrap();
    let events = advance(&mut world, &config, next.resolves_at_ms);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::TimedPositionResolved {
            target_id: None,
            damage: 42,
            ..
        }
    )));
    assert_eq!(world.kills, 1);
}

#[test]
fn timed_position_cancels_outside_or_protected_and_restarts_after_reset() {
    let (config, mut world) = fixture();
    complete_side(&mut world, &config, 0);
    complete_side(&mut world, &config, 1);
    let warning = start_warning(&mut world, &config);
    let position = world.position;
    world.position[1] += 250.0;
    let events = advance(&mut world, &config, warning.resolves_at_ms);
    assert!(matches!(
        events.as_slice(),
        [Event::TimedPositionCancelled { .. }]
    ));
    assert_eq!(world.hp, 1000);
    world.position = position;
    advance(&mut world, &config, 11000);
    advance(&mut world, &config, 16500);
    assert_eq!(view(&world).warning.unwrap().cycle, 2);
    world.protected = true;
    let events = advance(&mut world, &config, 16530);
    assert!(matches!(
        events.as_slice(),
        [Event::TimedPositionCancelled { .. }]
    ));
    assert!(view(&world).warning.is_none());
    world.protected = false;
    world.pve.as_mut().unwrap().encounters[2].round = 2;
    advance(&mut world, &config, 17000);
    advance(&mut world, &config, 22500);
    let warning = view(&world).warning.unwrap();
    assert_eq!((warning.round, warning.cycle), (2, 1));
    assert_eq!(warning.failure_raw_damage, 23);
    assert_eq!(warning.resolves_at_ms - warning.started_at_ms, 4500);
}

#[test]
fn timed_position_lethal_backlash_uses_existing_death_cleanup() {
    let (config, mut world) = fixture();
    world.hp = 20;
    let warning = start_warning(&mut world, &config);
    let events = advance(&mut world, &config, warning.resolves_at_ms);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::TimedPositionResolved {
            target_id: None,
            damage: 20,
            ..
        }
    )));
    assert_eq!(world.desired_outcome, Some(Outcome::Death));
    assert!(view(&world).warning.is_none());
    assert!(advance(&mut world, &config, warning.resolves_at_ms + 6000).is_empty());
}

#[test]
fn timed_position_reserves_one_cancel_event_and_stops_when_core_dies() {
    let (config, mut world) = fixture();
    let warning = start_warning(&mut world, &config);
    assert_eq!(world.close_interaction_event_budget(), 1);
    let closed = world.close_container_interaction(5700);
    assert!(matches!(
        closed.as_slice(),
        [Event::TimedPositionCancelled { cycle: 1, .. }]
    ));
    assert_eq!(world.close_interaction_event_budget(), 0);

    let (config, mut world) = fixture();
    start_warning(&mut world, &config);
    let mut arena = world.pve.take().unwrap();
    let core = arena
        .monsters
        .iter()
        .position(|m| m.id == "sanctum-1")
        .unwrap();
    arena.hurt_monster_contact(&mut world, &config, core, (1000, false, None), &mut vec![]);
    world.pve = Some(arena);
    let events = advance(&mut world, &config, warning.resolves_at_ms);
    assert!(matches!(
        events.as_slice(),
        [Event::TimedPositionCancelled { cycle: 1, .. }]
    ));
    assert_eq!(world.hp, 1000);
    assert!(advance(&mut world, &config, warning.resolves_at_ms + 6000).is_empty());
}

#[test]
fn rotating_sites_walk_order_wraps_and_wrong_circles_cause_one_backlash() {
    let (config, mut world) = fixture_for("b3-16");
    let sites = config
        .pve
        .as_ref()
        .unwrap()
        .timed_position
        .as_ref()
        .unwrap()
        .sites();
    advance(&mut world, &config, 100);
    for (step, expected) in ["west-scar", "east-shard", "core-rim", "west-scar"]
        .into_iter()
        .enumerate()
    {
        advance(&mut world, &config, 6100 + step as u64 * 6000);
        let warning = view(&world).warning.unwrap();
        assert_eq!(warning.site.as_ref().unwrap().id, expected);
        world.position = sites[(step + 1) % sites.len()].position;
        let events = advance(&mut world, &config, warning.resolves_at_ms);
        assert!(matches!(
            events.last(),
            Some(Event::TimedPositionResolved { site_id: Some(id), target_id: None, damage: 40, .. })
                if id == expected
        ));
        assert!(advance(&mut world, &config, warning.resolves_at_ms + 1).is_empty());
    }
    assert_eq!(world.hp, 840);
    assert_eq!(view(&world).successes, 0);
}

#[test]
fn rotating_sites_includes_boundary_and_targets_from_site_center_then_skips_dead() {
    let (config, mut world) = fixture_for("b3-16");
    let warning = start_warning(&mut world, &config);
    let center = warning.site.as_ref().unwrap().position;
    let arena = world.pve.as_mut().unwrap();
    for (index, offset) in arena.encounters[2].slots.iter().zip([160.0, 40.0, -70.0]) {
        arena.monsters[*index].position = [center[0] + offset, center[1]];
    }
    // The player stands at the left boundary, closest to sanctum-3. The site is closest to sanctum-2.
    world.position = [center[0] - 82.0, center[1]];
    let events = advance(&mut world, &config, warning.resolves_at_ms);
    assert!(matches!(
        events.last(),
        Some(Event::TimedPositionResolved { target_id: Some(id), damage: 82, .. }) if id == "sanctum-2"
    ));
    assert_eq!(world.kills, 1);
    for time in [12100, 18100] {
        advance(&mut world, &config, time);
        let warning = view(&world).warning.unwrap();
        advance(&mut world, &config, warning.resolves_at_ms);
    }
    advance(&mut world, &config, 24100);
    let warning = view(&world).warning.unwrap();
    let events = advance(&mut world, &config, warning.resolves_at_ms);
    assert!(matches!(
        events.last(),
        Some(Event::TimedPositionResolved { target_id: Some(id), damage: 82, .. }) if id == "sanctum-3"
    ));
    assert_eq!(world.kills, 2);
}

#[test]
fn rotating_sites_freezes_active_warning_and_applies_each_side_independently() {
    for (side, duration, backlash) in [(0, 4700, 40), (1, 3200, 22)] {
        let (config, mut world) = fixture_for("b3-16");
        let warning = start_warning(&mut world, &config);
        complete_side(&mut world, &config, side);
        assert_eq!(view(&world).warning, Some(warning.clone()));
        advance(&mut world, &config, warning.resolves_at_ms);
        assert_eq!(world.hp, 960);
        advance(&mut world, &config, 12100);
        let next = view(&world).warning.unwrap();
        assert_eq!(next.site.unwrap().id, "east-shard");
        assert_eq!(next.resolves_at_ms - next.started_at_ms, duration);
        assert_eq!(next.failure_raw_damage, backlash);
    }
}

#[test]
fn rotating_sites_round_reset_clears_old_warning_and_returns_to_first_site() {
    let (config, mut world) = fixture_for("b3-16");
    complete_side(&mut world, &config, 0);
    complete_side(&mut world, &config, 1);
    let first = start_warning(&mut world, &config);
    advance(&mut world, &config, first.resolves_at_ms);
    advance(&mut world, &config, 12100);
    let second = view(&world).warning.unwrap();
    assert_eq!(second.site.unwrap().id, "east-shard");
    let inside = world.position;
    world.position[0] -= 750.0;
    let mut arena = world.pve.take().unwrap();
    let mut events = vec![];
    arena.advance_encounter_resets(&world, 12200, &mut events);
    arena.advance_timed_position(&mut world, &config, 12200, &mut events);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::TimedPositionCancelled { cycle: 2, .. }))
    );
    arena.advance_encounter_resets(&world, 17200, &mut events);
    assert_eq!(arena.encounters[2].round, 2);
    world.position = inside;
    arena.prepare_encounters(&world, 17300, &mut events);
    world.pve = Some(arena);
    advance(&mut world, &config, 17300);
    advance(&mut world, &config, 23300);
    let next = view(&world).warning.unwrap();
    assert_eq!((next.round, next.cycle), (2, 1));
    assert_eq!(next.site.unwrap().id, "west-scar");
    assert_eq!(next.resolves_at_ms - next.started_at_ms, 4700);
    assert_eq!(next.failure_raw_damage, 22);
}

#[test]
fn channel_guard_restores_once_at_boundary_caps_at_max_and_pulses_on_miss() {
    let (config, mut world) = fixture_for("b3-17");
    let mut warning = start_warning(&mut world, &config);
    let mut expected_successes = 0;
    for (step, expected_site, spirit, restored) in [
        (0, "furnace", 75, Some(12)),
        (1, "crystal", 96, Some(4)),
        (2, "river", 100, Some(0)),
        (3, "furnace", 75, None),
    ] {
        if step > 0 {
            advance(&mut world, &config, 6600 + step * 6500);
            warning = view(&world).warning.unwrap();
        }
        let site = warning.site.as_ref().unwrap();
        assert_eq!(site.id, expected_site);
        world.position = [
            site.position[0] - if restored.is_some() { 80.0 } else { 81.0 },
            site.position[1],
        ];
        world.spirit = spirit;
        let hp = world.hp;
        let damage = if restored.is_some() { 0 } else { 24 };
        let events = advance(&mut world, &config, warning.resolves_at_ms);
        assert!(matches!(events.last(), Some(Event::TimedPositionResolved {
            target_id: None, spirit_restored, damage: actual, ..
        }) if *spirit_restored == restored && *actual == damage));
        expected_successes += u32::from(restored.is_some());
        assert_eq!(view(&world).successes, expected_successes);
        assert_eq!(world.spirit, spirit + restored.unwrap_or(0));
        assert_eq!(world.hp, hp - damage);
        assert_eq!(world.kills, 0);
        assert!(advance(&mut world, &config, warning.resolves_at_ms + 1).is_empty());
    }
}

#[test]
fn channel_guard_side_completion_filters_future_sites_and_keeps_current_warning() {
    let (config, mut world) = fixture_for("b3-17");
    let first = start_warning(&mut world, &config);
    advance(&mut world, &config, first.resolves_at_ms);
    advance(&mut world, &config, 13100);
    let crystal = view(&world).warning.unwrap();
    assert_eq!(crystal.site.as_ref().unwrap().id, "crystal");
    complete_side(&mut world, &config, 0);
    assert_eq!(view(&world).stabilized_sites, Some(1));
    assert_eq!(view(&world).warning, Some(crystal.clone()));
    world.position = crystal.site.unwrap().position;
    world.spirit = 50;
    advance(&mut world, &config, crystal.resolves_at_ms);
    assert_eq!(world.spirit, 62);

    for (step, expected_site) in ["river", "furnace", "river", "furnace", "furnace"]
        .into_iter()
        .enumerate()
    {
        advance(&mut world, &config, 19600 + step as u64 * 6500);
        let warning = view(&world).warning.unwrap();
        let site = warning.site.as_ref().unwrap();
        assert_eq!(site.id, expected_site);
        if step == 2 {
            complete_side(&mut world, &config, 1);
            assert_eq!(view(&world).stabilized_sites, Some(2));
            assert_eq!(view(&world).warning, Some(warning.clone()));
        }
        world.position = site.position;
        advance(&mut world, &config, warning.resolves_at_ms);
    }
    assert_eq!(view(&world).successes, 6);
}

#[test]
fn channel_guard_reset_preserves_side_completion_and_ignores_old_warning() {
    let (config, mut world) = fixture_for("b3-17");
    complete_side(&mut world, &config, 1);
    let first = start_warning(&mut world, &config);
    advance(&mut world, &config, first.resolves_at_ms);
    advance(&mut world, &config, 13100);
    let pending = view(&world).warning.unwrap();
    assert_eq!(pending.site.unwrap().id, "crystal");

    let inside = world.position;
    world.position[0] -= 750.0;
    let mut arena = world.pve.take().unwrap();
    let mut events = vec![];
    arena.advance_encounter_resets(&world, 13200, &mut events);
    arena.advance_timed_position(&mut world, &config, 13200, &mut events);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::TimedPositionCancelled { cycle: 2, .. }))
    );
    arena.advance_encounter_resets(&world, 18200, &mut events);
    world.position = inside;
    arena.prepare_encounters(&world, 18300, &mut events);
    world.pve = Some(arena);
    advance(&mut world, &config, 18300);
    advance(&mut world, &config, 24800);
    let check = view(&world);
    assert_eq!(check.stabilized_sites, Some(1));
    let restarted = check.warning.unwrap();
    assert_eq!((restarted.round, restarted.cycle), (2, 1));
    assert_eq!(restarted.site.unwrap().id, "furnace");
}

fn glacier_core(world: &World) -> usize {
    world.pve.as_ref().unwrap().encounters[2].slots[0]
}

fn glacier_contact(
    world: &mut World,
    config: &Config,
    target: usize,
    accepted: Option<u64>,
    at: u64,
) -> Vec<Event> {
    let mut arena = world.pve.take().unwrap();
    arena.now = at;
    let mut events = vec![];
    arena.hurt_monster_contact(world, config, target, (100, false, accepted), &mut events);
    world.pve = Some(arena);
    events
}

#[test]
fn thaw_window_rejects_a_real_old_projectile_and_applies_one_new_contact() {
    let (config, mut world) = fixture_for("b3-18");
    let target = glacier_core(&world);
    let point = world.pve.as_ref().unwrap().monsters[target].position;
    world.position = [point[0] - 110.0, point[1]];
    advance(&mut world, &config, 100);
    world
        .action(&Action::Attack {}, "old", &config, 5860)
        .unwrap();
    let trace: Vec<_> = (5890..=6340)
        .step_by(30)
        .flat_map(|time| world.tick(&config, time))
        .collect();
    let start = trace
        .iter()
        .position(|event| matches!(event, Event::TimedPositionStarted { .. }))
        .unwrap();
    assert!(trace[..start].iter().any(
        |event| matches!(event, Event::ActionCommitted { action_id, .. } if action_id == "old")
    ));
    assert!(trace[start + 1..].iter().any(
        |event| matches!(event, Event::Damage { target, amount: 100, .. } if target == "sanctum-1")
    ));
    assert_eq!(view(&world).warning.unwrap().hit_confirmed, Some(false));

    world
        .action(
            &Action::Cast {
                skill_id: config.skill.id.clone(),
                direction: [1.0, 0.0],
            },
            "new",
            &config,
            6340,
        )
        .unwrap();
    let cast: Vec<_> = (6370..=6940)
        .step_by(30)
        .flat_map(|time| world.tick(&config, time))
        .collect();
    assert!(cast.iter().any(|event| matches!(
        event,
        Event::TimedHitTriggered {
            damage: 160,
            cycle: 1,
            ..
        }
    )));
    assert_eq!(view(&world).warning.unwrap().hit_confirmed, Some(true));
    world
        .action(&Action::Attack {}, "repeat", &config, 7000)
        .unwrap();
    let trace: Vec<_> = (7030..=8320)
        .step_by(30)
        .flat_map(|time| world.tick(&config, time))
        .collect();
    assert!(trace.iter().any(
        |event| matches!(event, Event::Damage { target, amount: 100, .. } if target == "sanctum-1")
    ));
    assert!(!trace.iter().any(|event| matches!(
        event,
        Event::TimedHitTriggered { .. } | Event::TimedHitMissed { .. }
    )));
    assert_eq!(view(&world).successes, 1);
    assert!(view(&world).warning.is_none());
}

#[test]
fn thaw_window_requires_live_direct_contact_and_closes_at_the_deadline() {
    let (config, mut world) = fixture_for("b3-18");
    let target = glacier_core(&world);
    let other = world.pve.as_ref().unwrap().encounters[2].slots[1];
    let warning = start_warning(&mut world, &config);
    let start = warning.started_at_ms;
    let end = warning.resolves_at_ms;
    glacier_contact(&mut world, &config, other, Some(start), start + 30);
    glacier_contact(&mut world, &config, target, None, start + 60);
    let profile = &mut world.pve.as_mut().unwrap().monsters[target].profile;
    Arc::make_mut(profile).stats.insert("dodge".into(), 100);
    let dodged = glacier_contact(&mut world, &config, target, Some(start), start + 90);
    assert!(dodged.is_empty());
    let profile = &mut world.pve.as_mut().unwrap().monsters[target].profile;
    Arc::make_mut(profile).stats.insert("dodge".into(), 0);
    glacier_contact(&mut world, &config, target, Some(start), end);
    assert_eq!(view(&world).warning.unwrap().hit_confirmed, Some(false));
    world.pve.as_mut().unwrap().monsters[target].hp = 970;
    let miss = advance(&mut world, &config, end);
    assert!(matches!(
        miss.as_slice(),
        [Event::TimedHitMissed { healed: 30, .. }]
    ));
    assert_eq!(world.pve.as_ref().unwrap().monsters[target].hp, 1000);
    assert!(advance(&mut world, &config, end + 30).is_empty());

    advance(&mut world, &config, start + 6000);
    world.pve.as_mut().unwrap().monsters[target].hp = 50;
    let events = glacier_contact(
        &mut world,
        &config,
        target,
        Some(start + 6000),
        start + 6030,
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::TargetKilled))
            .count(),
        1
    );
    assert!(matches!(
        events.last(),
        Some(Event::TimedHitTriggered {
            damage: 0,
            cycle: 2,
            ..
        })
    ));
    let cancellation = advance(&mut world, &config, start + 6060);
    assert!(matches!(
        cancellation.as_slice(),
        [Event::TimedPositionCancelled { cycle: 2, .. }]
    ));
    assert_eq!(world.pve.as_ref().unwrap().monsters[target].hp, 0);
}

#[test]
fn thaw_window_freezes_current_duration_and_heal_with_independent_side_rules() {
    for (side, duration, healed) in [(0, 3700, 80), (1, 2200, 28)] {
        let (config, mut world) = fixture_for("b3-18");
        let target = glacier_core(&world);
        let first = start_warning(&mut world, &config);
        assert_eq!(first.miss_heal, Some(80));
        assert_eq!(first.resolves_at_ms - first.started_at_ms, 2200);
        complete_side(&mut world, &config, side);
        assert_eq!(view(&world).warning, Some(first.clone()));
        world.pve.as_mut().unwrap().monsters[target].hp = 800;
        let events = advance(&mut world, &config, first.resolves_at_ms);
        assert!(matches!(
            events.as_slice(),
            [Event::TimedHitMissed { healed: 80, .. }]
        ));
        advance(&mut world, &config, first.started_at_ms + 6000);
        let next = view(&world).warning.unwrap();
        assert_eq!(next.resolves_at_ms - next.started_at_ms, duration);
        assert_eq!(next.miss_heal, Some(healed));
        let events = advance(&mut world, &config, next.resolves_at_ms);
        assert!(
            matches!(events.as_slice(), [Event::TimedHitMissed { healed: amount, .. }] if *amount == healed)
        );
    }
}

#[test]
fn thaw_window_cancels_on_protection_and_resets_with_side_benefits() {
    let (config, mut world) = fixture_for("b3-18");
    let target = glacier_core(&world);
    complete_side(&mut world, &config, 0);
    complete_side(&mut world, &config, 1);
    let first = start_warning(&mut world, &config);
    world.pve.as_mut().unwrap().monsters[target].hp = 900;
    world.protected = true;
    let events = advance(&mut world, &config, first.resolves_at_ms);
    assert!(matches!(
        events.as_slice(),
        [Event::TimedPositionCancelled { .. }]
    ));
    assert_eq!(world.pve.as_ref().unwrap().monsters[target].hp, 900);
    world.protected = false;
    advance(&mut world, &config, 10400);
    advance(&mut world, &config, 16400);
    let pending = view(&world).warning.unwrap();
    let inside = world.position;
    world.position[0] -= 750.0;
    let mut arena = world.pve.take().unwrap();
    let mut events = vec![];
    arena.advance_encounter_resets(&world, 16500, &mut events);
    arena.advance_timed_position(&mut world, &config, 16500, &mut events);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::TimedPositionCancelled { cycle: 2, .. }))
    );
    arena.advance_encounter_resets(&world, 21500, &mut events);
    world.position = inside;
    arena.prepare_encounters(&world, 21600, &mut events);
    world.pve = Some(arena);
    advance(&mut world, &config, 21600);
    advance(&mut world, &config, 27600);
    let restarted = view(&world).warning.unwrap();
    assert_eq!((restarted.round, restarted.cycle), (2, 1));
    assert_eq!(
        (
            restarted.resolves_at_ms - restarted.started_at_ms,
            restarted.miss_heal
        ),
        (3700, Some(28))
    );
    glacier_contact(
        &mut world,
        &config,
        target,
        Some(pending.started_at_ms),
        27630,
    );
    assert_eq!(view(&world).warning.unwrap().hit_confirmed, Some(false));
}
