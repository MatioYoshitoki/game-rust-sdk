use super::heavy_tests::{advance, start, window, world};
use super::timed_position_tests::complete_side;
use super::*;

fn fixture() -> (Config, World, usize) {
    let config = crate::config::default_catalog()
        .active("b3-19")
        .unwrap()
        .as_ref()
        .clone();
    let mut player = world(&config);
    let mut arena = player.pve.take().unwrap();
    let core = arena.encounters[2].slots[0];
    let center = arena.monsters[core].home;
    player.position = [center[0], center[1] - 180.0];
    arena.prepare_encounters(&player, 100, &mut vec![]);
    assert_eq!(arena.encounters[2].phase, EncounterPhase::Active);
    arena.monsters[core].aggro = true;
    player.pve = Some(arena);
    (config, player, core)
}

fn dodge_sword(player: &mut World, config: &Config, started: u64) -> CounterView {
    player.position[0] += 90.0;
    advance(player, config, started + 1600);
    advance(player, config, started + 2300);
    window(player).unwrap()
}

#[test]
fn sword_trial_side_conditions_apply_independently_when_the_window_opens() {
    for (west, east, duration, damage) in [
        (false, false, 2400, 200),
        (true, false, 3800, 200),
        (false, true, 2400, 270),
        (true, true, 3800, 270),
    ] {
        let (config, mut player, core) = fixture();
        assert!(matches!(
            start(&mut player, &config, core, 100).as_slice(),
            [Event::HeavyStarted { .. }]
        ));
        if west {
            complete_side(&mut player, &config, 0);
        }
        if east {
            complete_side(&mut player, &config, 1);
        }
        let view = dodge_sword(&mut player, &config, 100);
        assert_eq!(view.ends_at_ms - view.started_at_ms, duration);
        assert_eq!(view.raw_damage, damage);
    }
}

#[test]
fn sword_trial_open_window_and_damage_stay_frozen_after_side_completion() {
    let (config, mut player, core) = fixture();
    let start_position = player.position;
    start(&mut player, &config, core, 100);
    let base = dodge_sword(&mut player, &config, 100);
    complete_side(&mut player, &config, 0);
    complete_side(&mut player, &config, 1);
    assert_eq!(window(&player), Some(base.clone()));

    let mut arena = player.pve.take().unwrap();
    arena.now = base.started_at_ms + 30;
    let mut events = vec![];
    arena.hurt_monster_contact(
        &mut player,
        &config,
        core,
        (100, false, Some(base.started_at_ms)),
        &mut events,
    );
    assert!(matches!(
        events.last(),
        Some(Event::CounterTriggered { damage: 200, .. })
    ));
    player.pve = Some(arena);
    assert!(window(&player).is_none());

    player.position = start_position;
    start(&mut player, &config, core, 9000);
    let prepared = dodge_sword(&mut player, &config, 9000);
    assert_eq!(
        (
            prepared.ends_at_ms - prepared.started_at_ms,
            prepared.raw_damage
        ),
        (3800, 270)
    );
}

#[test]
fn sword_trial_reset_cancels_old_window_and_preserves_completed_sides() {
    let (config, mut player, core) = fixture();
    let inside = player.position;
    complete_side(&mut player, &config, 0);
    complete_side(&mut player, &config, 1);
    start(&mut player, &config, core, 100);
    let old = dodge_sword(&mut player, &config, 100);

    player.position[1] -= 800.0;
    let mut arena = player.pve.take().unwrap();
    let mut events = vec![];
    arena.advance_encounter_resets(&player, 2500, &mut events);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::HeavyCancelled {
            counter: Some(_),
            ..
        }
    )));
    assert!(arena.counter.is_none());
    arena.advance_encounter_resets(&player, 7500, &mut events);
    assert_eq!(arena.encounters[2].round, 2);
    player.position = inside;
    arena.prepare_encounters(&player, 7600, &mut events);
    arena.monsters[core].aggro = true;
    player.pve = Some(arena);

    start(&mut player, &config, core, 7600);
    let fresh = dodge_sword(&mut player, &config, 7600);
    assert_eq!(
        (fresh.ends_at_ms - fresh.started_at_ms, fresh.raw_damage),
        (3800, 270)
    );
    let arena = player.pve.as_mut().unwrap();
    assert!(
        arena
            .take_counter(core, Some(old.started_at_ms), fresh.started_at_ms)
            .is_none()
    );
    assert_eq!(window(&player), Some(fresh));
}

#[test]
fn sword_trial_old_projectile_lands_after_open_but_new_hit_consumes_once() {
    let (config, mut player, core) = fixture();
    start(&mut player, &config, core, 100);
    player.position[0] += 90.0;
    advance(&mut player, &config, 1700);
    player
        .action(&Action::Attack {}, "old-shot", &config, 2170)
        .unwrap();
    let trace: Vec<_> = (2200..=2800)
        .step_by(30)
        .flat_map(|now| player.tick(&config, now))
        .collect();
    let launch = trace
        .iter()
        .position(
            |e| matches!(e, Event::ActionCommitted { action_id, .. } if action_id == "old-shot"),
        )
        .unwrap();
    let opened = trace
        .iter()
        .position(|e| matches!(e, Event::CounterOpened { .. }))
        .unwrap();
    let impact = trace
        .iter()
        .position(
            |e| matches!(e, Event::Damage { target, amount: 100, .. } if target == "sanctum-1"),
        )
        .unwrap();
    assert!(launch < opened && opened < impact);
    assert!(
        !trace
            .iter()
            .any(|e| matches!(e, Event::CounterTriggered { .. }))
    );
    assert!(window(&player).is_some());

    for (now, id) in [(3000, "new-shot"), (3840, "repeat-shot")] {
        player.action(&Action::Attack {}, id, &config, now).unwrap();
        let events: Vec<_> = (now + 30..=now + 750)
            .step_by(30)
            .flat_map(|at| player.tick(&config, at))
            .collect();
        assert!(events.iter().any(
            |e| matches!(e, Event::Damage { target, amount: 100, .. } if target == "sanctum-1")
        ));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::CounterTriggered { damage: 200, .. }))
                .count(),
            usize::from(id == "new-shot")
        );
        assert!(window(&player).is_none());
    }
}
