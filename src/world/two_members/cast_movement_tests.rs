use super::*;
use serde_json::{Value, json};

fn setup(preparation: Value, recovery: Value) -> (Config, World) {
    let (mut config, mut world) = super::hp_cost_tests::setup(2);
    config.mechanism_version = 46;
    config.skill.windup_ms = config.step_ms * 2;
    config.skill.recovery_ms = config.step_ms * 2;
    config.skill.cast_movement = serde_json::from_value(json!({
        "preparation": preparation, "recovery": recovery
    }))
    .unwrap();
    world.npcs.clear();
    (config, world)
}

fn input(world: &mut World, config: &Config, member: usize, action: Action, now: u64) -> Events {
    world
        .action(member, &action, "movement-test", config, now)
        .unwrap()
}

fn cast(world: &mut World, config: &Config) {
    input(
        world,
        config,
        0,
        Action::Cast {
            skill_id: config.skill.id.clone(),
            direction: [1., 0.],
        },
        0,
    );
}

fn locked() -> Value {
    json!({"kind":"locked"})
}
fn free() -> Value {
    json!({"kind":"free"})
}
fn slow() -> Value {
    json!({"kind":"slow","speed_bps":2500})
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "got {actual}, expected {expected}"
    );
}

fn breaks_on_move() -> Value {
    json!({"kind":"interrupt_on_new_move","min_turn_degrees":15,"min_destination_distance":4.0})
}

#[test]
fn new_move_ignores_preheld_keepalive_stop_and_noise_but_not_accumulated_turn() {
    let (mut c, mut w) = setup(breaks_on_move(), free());
    c.mechanism_version = 105;
    c.skill.windup_ms = 600;
    input(
        &mut w,
        &c,
        0,
        Action::Move {
            direction: [1., 0.],
        },
        0,
    );
    cast(&mut w, &c);
    let balance = (w.members[0].hp, w.members[0].spirit);
    for (n, degrees) in [0_f64, 5., 10.].into_iter().enumerate() {
        let a = degrees.to_radians();
        let e = input(
            &mut w,
            &c,
            0,
            Action::Move {
                direction: [a.cos(), a.sin()],
            },
            n as u64,
        );
        assert!(e.is_empty());
        assert!(w.members[0].pending.is_some());
    }
    input(&mut w, &c, 0, Action::Stop {}, 3);
    assert!(w.members[0].pending.is_some());
    input(
        &mut w,
        &c,
        0,
        Action::Move {
            direction: [0., 0.],
        },
        4,
    );
    assert!(w.members[0].pending.is_some());
    let e = input(
        &mut w,
        &c,
        0,
        Action::Move {
            direction: [0., 1.],
        },
        5,
    );
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::ActionInterrupted
                }
            ))
            .count(),
        1
    );
    assert!(w.members[0].pending.is_none());
    w.tick(&c, c.step_ms);
    assert_eq!((w.members[0].hp, w.members[0].spirit), balance);
    assert!(w.members[0].cooldowns.is_empty());
    assert!(w.shots.is_empty());
    assert!(w.members[0].position[1] > c.spawn[1]);
}

#[test]
fn new_move_navigation_uses_phase_destination_and_invalid_requests_are_atomic() {
    let (mut c, mut w) = setup(breaks_on_move(), free());
    c.mechanism_version = 105;
    let p = w.members[0].position;
    let destination = [p[0] + 20., p[1]];
    input(&mut w, &c, 0, Action::Navigate { destination }, 0);
    cast(&mut w, &c);
    for invalid in [
        Action::Move {
            direction: [f64::NAN, 0.],
        },
        Action::Navigate {
            destination: [-1., -1.],
        },
    ] {
        assert!(w.action(0, &invalid, "invalid", &c, 1).is_err());
        assert!(w.members[0].pending.is_some());
    }
    input(
        &mut w,
        &c,
        0,
        Action::Navigate {
            destination: [destination[0] + 2., destination[1]],
        },
        2,
    );
    assert!(w.members[0].pending.is_some());
    input(
        &mut w,
        &c,
        0,
        Action::Navigate {
            destination: [destination[0] + 5., destination[1]],
        },
        3,
    );
    assert!(w.members[0].pending.is_none());
    assert_eq!(
        w.members[0].path.last(),
        Some(&[destination[0] + 5., destination[1]])
    );
}

#[test]
fn mobile_channel_is_frozen_moves_at_configured_speed_and_does_not_reprice() {
    for active in [free(), slow(), locked()] {
        let (mut c, mut w) = super::channel_tests::setup(false);
        c.mechanism_version = 105;
        c.skill.cast_movement =
            serde_json::from_value(json!({"preparation":free(),"recovery":free(),"active":active}))
                .unwrap();
        super::channel_tests::start(&mut w, &c, 0);
        w.tick(&c, 60);
        let budget = w.members[0].channel.as_ref().unwrap().raw;
        let before = w.members[0].position;
        let expected = w.members[0].phase_movement(60).speed_factor();
        let full = c.params.movement.speed(w.members[0].facts.spd_points)
            * w.grid.speed(before)
            * c.step_ms as f64
            / 1000.;
        c.skill.cast_movement = None;
        w.members[0].facts.atk_points += 100;
        input(
            &mut w,
            &c,
            0,
            Action::Move {
                direction: [1., 0.],
            },
            60,
        );
        w.tick(&c, 90);
        near(w.members[0].position[0] - before[0], full * expected);
        assert_eq!(w.members[0].channel.as_ref().unwrap().raw, budget);
        let mut e = vec![];
        w.advance_channel(0, &c, 150, &mut e);
        assert_eq!(w.shots.len(), 1);
        assert_eq!(w.shots[0].raw, budget);
    }
}

#[test]
fn channel_new_move_stops_only_future_pulses_and_keeps_peer_and_committed_shot() {
    let (mut c, mut w) = super::channel_tests::setup(false);
    c.mechanism_version = 105;
    c.skill.cast_movement = serde_json::from_value(
        json!({"preparation":free(),"recovery":free(),"active":breaks_on_move()}),
    )
    .unwrap();
    input(
        &mut w,
        &c,
        0,
        Action::Move {
            direction: [1., 0.],
        },
        0,
    );
    for i in 0..2 {
        super::channel_tests::start(&mut w, &c, i);
    }
    w.tick(&c, 60);
    w.advance_channel(0, &c, 150, &mut vec![]);
    let shot = w.shots[0].view.id.clone();
    let spirit = w.members[0].spirit;
    input(
        &mut w,
        &c,
        0,
        Action::Move {
            direction: [1., 0.],
        },
        151,
    );
    assert!(w.members[0].channel.is_some());
    let e = input(
        &mut w,
        &c,
        0,
        Action::Move {
            direction: [0., 1.],
        },
        152,
    );
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ChannelEnded {
                reason: ChannelEndReason::Interrupted,
                ..
            }
        }
    )));
    assert!(w.members[0].channel.is_none());
    assert!(w.members[1].channel.is_some());
    assert_eq!(w.shots[0].view.id, shot);
    w.advance_channel(0, &c, 240, &mut vec![]);
    assert_eq!(w.members[0].spirit, spirit);
    assert!(w.members[0].cooldowns.contains_key(&c.skill.id));
}

#[test]
fn cast_movement_locked_preparation_keeps_latest_direction_and_peer_moves() {
    let (config, mut world) = setup(locked(), free());
    let step = config.step_ms;
    let origin = world.members[0].position;
    let peer = world.members[1].position;
    cast(&mut world, &config);
    input(
        &mut world,
        &config,
        0,
        Action::Move {
            direction: [1., 0.],
        },
        0,
    );
    input(
        &mut world,
        &config,
        1,
        Action::Move {
            direction: [1., 0.],
        },
        0,
    );
    world.tick(&config, step);
    assert_eq!(world.members[0].position, origin);
    assert!(world.members[1].position[0] > peer[0]);
    input(
        &mut world,
        &config,
        0,
        Action::Move {
            direction: [0., 1.],
        },
        step,
    );
    world.tick(&config, step * 2);
    assert_eq!(world.members[0].position, origin);
    assert_eq!(world.shots[0].view.position, origin);
    world.tick(&config, step * 3);
    assert_eq!(world.members[0].position[0], origin[0]);
    assert!(world.members[0].position[1] > origin[1]);
}

#[test]
fn cast_movement_slow_preparation_applies_on_commit_step_then_locked_recovery() {
    let (config, mut world) = setup(slow(), locked());
    let step = config.step_ms;
    let origin = world.members[0].position;
    let full = config
        .params
        .movement
        .speed(world.members[0].facts.spd_points)
        * world.grid.speed(origin)
        * step as f64
        / 1000.;
    let hp = world.members[0].hp;
    let spirit = world.members[0].spirit;
    cast(&mut world, &config);
    input(
        &mut world,
        &config,
        0,
        Action::Move {
            direction: [1., 0.],
        },
        0,
    );
    world.tick(&config, step);
    near(world.members[0].position[0], origin[0] + full * 0.25);
    assert_eq!((world.members[0].hp, world.members[0].spirit), (hp, spirit));
    let events = world.tick(&config, step * 2);
    let committed = world.members[0].position;
    near(committed[0], origin[0] + full * 0.5);
    assert_eq!(world.shots[0].view.position, committed);
    assert!(events.iter().any(|(recipient, event)| *recipient == Some(0)
        && matches!(
            event,
            EventV4::Private {
                event: Event::ActionCommitted {
                    hp_cost: Some(3),
                    ..
                }
            }
        )));
    assert_eq!(world.members[0].spirit, spirit - config.skill.cost);
    input(&mut world, &config, 0, Action::CancelAction {}, step * 2);
    world.tick(&config, step * 3);
    assert_eq!(world.members[0].position, committed);
    world.tick(&config, step * 4);
    near(world.members[0].position[0], committed[0] + full);
}

#[test]
fn cast_movement_cancel_releases_buffered_navigation_and_stop_clears_it() {
    for stop in [false, true] {
        let (config, mut world) = setup(locked(), locked());
        let step = config.step_ms;
        let origin = world.members[0].position;
        let hp = world.members[0].hp;
        let spirit = world.members[0].spirit;
        cast(&mut world, &config);
        input(
            &mut world,
            &config,
            0,
            Action::Navigate {
                destination: [origin[0] + 20., origin[1]],
            },
            0,
        );
        let path = world.members[0].path.clone();
        assert!(!path.is_empty());
        world.tick(&config, step);
        assert_eq!(world.members[0].position, origin);
        assert_eq!(world.members[0].path, path);
        if stop {
            input(&mut world, &config, 0, Action::Stop {}, step);
        }
        input(&mut world, &config, 0, Action::CancelAction {}, step);
        world.tick(&config, step * 2);
        assert_eq!(world.members[0].position == origin, stop);
        assert_eq!((world.members[0].hp, world.members[0].spirit), (hp, spirit));
        assert!(world.members[0].cooldowns.is_empty());
        assert!(world.shots.is_empty());
    }
}

#[test]
fn cast_movement_policies_are_frozen_and_failed_commit_never_locks_recovery() {
    for reject in [false, true] {
        let (mut config, mut world) = setup(locked(), slow());
        let step = config.step_ms;
        let origin = world.members[0].position;
        let full = config
            .params
            .movement
            .speed(world.members[0].facts.spd_points)
            * world.grid.speed(origin)
            * step as f64
            / 1000.;
        cast(&mut world, &config);
        config.skill.cast_movement = None;
        if reject {
            world.members[0].spirit = 0;
        }
        input(
            &mut world,
            &config,
            0,
            Action::Move {
                direction: [1., 0.],
            },
            0,
        );
        world.tick(&config, step);
        world.tick(&config, step * 2);
        assert_eq!(world.members[0].position, origin);
        assert_eq!(world.shots.is_empty(), reject);
        world.tick(&config, step * 3);
        near(
            world.members[0].position[0],
            origin[0] + full * if reject { 1. } else { 0.25 },
        );
    }
}

#[test]
fn cast_movement_slow_movement_respects_existing_terrain_collision() {
    let (mut config, mut world) = setup(slow(), free());
    let step = config.step_ms;
    config.skill.windup_ms = 60_000;
    let mut position = world.members[0].position;
    let direction = [-1., 0.];
    cast(&mut world, &config);
    input(&mut world, &config, 0, Action::Move { direction }, 0);
    let mut blocked = false;
    let mut moved = false;
    for i in 1..=1200 {
        let budget = config
            .params
            .movement
            .speed(world.members[0].facts.spd_points)
            * world.grid.speed(position)
            * step as f64
            / 1000.
            * 0.25;
        let next = [position[0] - budget, position[1]];
        let clear = world
            .grid
            .line(position, next, config.params.player_radius, 0);
        world.tick(&config, i * step);
        let actual = world.members[0].position;
        if !clear {
            assert!(moved);
            assert_eq!(actual, position);
            blocked = true;
            break;
        }
        near(actual[0], next[0]);
        moved = true;
        position = actual;
    }
    assert!(blocked);
}

#[test]
fn new_move_stopping_does_not_interrupt_but_a_fresh_same_direction_does() {
    for stop in [
        Action::Stop {},
        Action::Move {
            direction: [0., 0.],
        },
    ] {
        let (mut c, mut w) = setup(breaks_on_move(), free());
        c.mechanism_version = 105;
        input(
            &mut w,
            &c,
            0,
            Action::Move {
                direction: [1., 0.],
            },
            0,
        );
        cast(&mut w, &c);
        input(&mut w, &c, 0, stop, 1);
        assert!(w.members[0].pending.is_some());
        input(
            &mut w,
            &c,
            0,
            Action::Move {
                direction: [1., 0.],
            },
            2,
        );
        assert!(w.members[0].pending.is_none());
        assert!(w.members[0].cooldowns.is_empty());
    }
}

#[test]
fn new_move_large_finite_direction_remains_a_nonzero_unit_intent() {
    let (mut c, mut w) = setup(breaks_on_move(), free());
    c.mechanism_version = 105;
    cast(&mut w, &c);
    input(
        &mut w,
        &c,
        0,
        Action::Move {
            direction: [f64::MAX, f64::MAX],
        },
        1,
    );
    assert!(w.members[0].pending.is_none());
    let [x, y] = w.members[0].movement;
    near(x, 0.5_f64.sqrt());
    near(y, 0.5_f64.sqrt());
    assert!(
        w.action(
            0,
            &Action::Move {
                direction: [f64::INFINITY, 0.]
            },
            "infinite",
            &c,
            2
        )
        .is_err()
    );
    assert_eq!(w.members[0].movement, [x, y]);
}
