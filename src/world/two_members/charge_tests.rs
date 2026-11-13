use super::*;
use serde_json::json;

pub(super) fn setup(piercing: bool) -> (Config, World) {
    let (mut config, mut world) = super::hp_cost_tests::setup(2);
    config.mechanism_version = 47;
    if !piercing {
        config.skill = config.additional_skills[1].clone();
    }
    config.skill.windup_ms = 0;
    config.skill.damage_bps = 10_000;
    config.skill.charge = serde_json::from_value(json!({
        "min_ms": 90, "max_ms": 275, "min_damage_bps": 4000
    }))
    .unwrap();
    world.npcs.clear();
    (config, world)
}

pub(super) fn start(world: &mut World, config: &Config, member: usize, now: u64) -> String {
    world
        .action(
            member,
            &Action::StartCharge {
                skill_id: config.skill.id.clone(),
                direction: [1., 0.],
            },
            "same-request-id",
            config,
            now,
        )
        .unwrap();
    world.members[member].pending.as_ref().unwrap().id.clone()
}

fn release(
    world: &mut World,
    config: &Config,
    member: usize,
    id: &str,
    now: u64,
) -> Result<Events> {
    world.action(
        member,
        &Action::ReleaseCharge {
            action_id: id.into(),
            direction: None,
        },
        "release",
        config,
        now,
    )
}

pub(super) fn result(events: &Events, member: usize) -> (&str, ChargeCommitView) {
    events
        .iter()
        .find_map(|(recipient, event)| match event {
            EventV4::Private {
                event:
                    Event::ActionCommitted {
                        action_id,
                        charge: Some(charge),
                        ..
                    },
            } if *recipient == Some(member) => Some((action_id.as_str(), *charge)),
            _ => None,
        })
        .expect("one private charge commit")
}

#[test]
fn early_release_queue_locks_aim_and_commits_once_at_minimum_with_frozen_curve() {
    let (mut c, mut w) = setup(true);
    c.mechanism_version = 105;
    c.skill.charge.as_mut().unwrap().early_release =
        Some(serde_json::from_value(json!({"kind":"queue_until_min"})).unwrap());
    let id = start(&mut w, &c, 0, 0);
    let before = (w.members[0].hp, w.members[0].spirit);
    assert!(release(&mut w, &c, 0, &id, 20).unwrap().is_empty());
    assert_eq!(w.members[0].pending.as_ref().unwrap().commits, 90);
    assert_eq!(release(&mut w, &c, 0, &id, 21), Err(Error::InvalidState));
    assert_eq!(
        w.action(
            0,
            &Action::UpdateChargeAim {
                action_id: id.clone(),
                direction: [0., 1.]
            },
            "aim",
            &c,
            30
        ),
        Err(Error::InvalidState)
    );
    w.tick(&c, 60);
    assert_eq!((w.members[0].hp, w.members[0].spirit), before);
    assert!(w.shots.is_empty());
    let e = w.tick(&c, 90);
    let (_, q) = result(&e, 0);
    assert_eq!(q.held_ms, 90);
    assert_eq!(q.damage_factor_bps, 4000);
    assert_eq!(w.members[0].spirit, before.1 - c.skill.cost);
    assert_eq!(w.shots.len(), 1);
    assert!(w.tick(&c, 120).iter().all(|(_, e)| !matches!(
        e,
        EventV4::Private {
            event: Event::ActionCommitted { .. }
        }
    )));
    assert_eq!(w.members[0].spirit, before.1 - c.skill.cost);
}

#[test]
fn queued_early_release_cancel_and_commit_rejection_are_free() {
    for cancel in [false, true] {
        let (mut c, mut w) = setup(true);
        c.mechanism_version = 105;
        c.skill.charge.as_mut().unwrap().early_release =
            Some(serde_json::from_value(json!({"kind":"queue_until_min"})).unwrap());
        let id = start(&mut w, &c, 0, 0);
        release(&mut w, &c, 0, &id, 20).unwrap();
        if cancel {
            w.action(0, &Action::CancelAction {}, "cancel", &c, 30)
                .unwrap();
        } else {
            w.members[0].spirit = 0;
        }
        let before = (w.members[0].hp, w.members[0].spirit);
        let e = w.tick(&c, 90);
        assert_eq!((w.members[0].hp, w.members[0].spirit), before);
        assert!(w.members[0].pending.is_none());
        assert!(w.members[0].cooldowns.is_empty());
        assert!(w.shots.is_empty());
        if !cancel {
            assert!(e.iter().any(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::ActionRejected {
                        reason: Error::NoResource,
                        ..
                    }
                }
            )));
        }
    }
}

#[test]
fn charge_early_release_is_free_and_late_id_cannot_release_next_charge() {
    let (config, mut world) = setup(true);
    let hp = world.members[0].hp;
    let spirit = world.members[0].spirit;
    let first = start(&mut world, &config, 0, 0);
    assert!(
        release(&mut world, &config, 0, &first, 89)
            .unwrap()
            .iter()
            .any(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::ActionInterrupted
                }
            ))
    );
    assert!(world.members[0].pending.is_none());
    let second = start(&mut world, &config, 0, 90);
    assert_ne!(first, second);
    assert_eq!(
        release(&mut world, &config, 0, &first, 180),
        Err(Error::InvalidTarget)
    );
    world
        .action(0, &Action::CancelAction {}, "cancel", &config, 200)
        .unwrap();
    world.tick(&config, 390);
    assert_eq!((world.members[0].hp, world.members[0].spirit), (hp, spirit));
    assert!(world.members[0].cooldowns.is_empty() && world.shots.is_empty());
}

#[test]
fn charge_manual_release_freezes_elapsed_direction_and_reuses_both_projectile_plans() {
    for (piercing, held_ms, factor) in [(false, 90, 4000), (true, 180, 6918)] {
        let (mut config, mut world) = setup(piercing);
        config.skill.cast_movement = serde_json::from_value(json!({
            "preparation":{"kind":"slow","speed_bps":2500}, "recovery":{"kind":"locked"}
        }))
        .unwrap();
        let id = start(&mut world, &config, 0, 0);
        let origin = world.members[0].position;
        world
            .action(
                0,
                &Action::Move {
                    direction: [0., 1.],
                },
                "move",
                &config,
                0,
            )
            .unwrap();
        world.tick(&config, 30);
        assert_eq!(world.members[0].position[0], origin[0]);
        assert!(world.members[0].position[1] > origin[1]);
        let spirit = world.members[0].spirit;
        config.skill.charge = None; // The accepted definition belongs to the pending action.
        release(&mut world, &config, 0, &id, held_ms).unwrap();
        assert_eq!(
            release(&mut world, &config, 0, &id, held_ms),
            Err(Error::InvalidState)
        );
        assert_eq!(world.members[0].spirit, spirit);
        let events = world.tick(&config, held_ms + 30);
        assert_eq!(
            result(&events, 0),
            (
                id.as_str(),
                ChargeCommitView {
                    held_ms,
                    damage_factor_bps: factor,
                    direction: None,
                }
            )
        );
        assert_eq!(world.members[0].spirit, spirit - config.skill.cost);
        assert_eq!(world.shots.len(), 1);
        assert_eq!(world.shots[0].direction, [1., 0.]);
        assert_eq!(world.shots[0].contact.is_some(), piercing);
        let raw = u64::from(world.stats(0, &config, held_ms + 30).0) * u64::from(factor) / 10_000;
        assert_eq!(u64::from(world.shots[0].raw), raw);
        assert!(events.iter().any(
            |(_, event)| matches!(event, EventV4::ProjectileStarted { projectile }
            if projectile.position == world.members[0].position)
        ));
        let position = world.members[0].position;
        world.tick(&config, held_ms + 60);
        assert_eq!(world.members[0].position, position);
        assert_eq!(
            release(&mut world, &config, 0, &id, held_ms + 60),
            Err(Error::InvalidTarget)
        );
    }
}

#[test]
fn charge_automatic_commit_uses_exact_max_between_fixed_steps() {
    let (config, mut world) = setup(true);
    let id = start(&mut world, &config, 0, 0);
    world.tick(&config, 270);
    assert!(world.shots.is_empty());
    let events = world.tick(&config, 300);
    assert_eq!(
        result(&events, 0),
        (
            id.as_str(),
            ChargeCommitView {
                held_ms: 275,
                damage_factor_bps: 10_000,
                direction: None,
            }
        )
    );
    assert!(world.members[0].pending.is_none());
}

#[test]
fn charge_commit_rechecks_origin_spirit_and_shared_capacity_before_cost() {
    for (failure, error) in [
        (0, Error::InvalidTarget),
        (1, Error::NoResource),
        (2, Error::Capacity),
    ] {
        let (mut config, mut world) = setup(true);
        let actor = if failure == 2 {
            config.members.as_mut().unwrap().projectile_capacity = 1;
            start(&mut world, &config, 0, 0);
            1
        } else {
            0
        };
        let id = start(&mut world, &config, actor, 0);
        if failure == 0 {
            world.members[actor].position = [-1., -1.];
        }
        if failure == 1 {
            world.members[actor].spirit = 0;
        }
        let hp = world.members[actor].hp;
        let spirit = world.members[actor].spirit;
        let events = world.tick(&config, 300);
        assert!(events.iter().any(|(recipient, event)| *recipient == Some(actor) &&
            matches!(event, EventV4::Private { event: Event::ActionRejected { action_id, reason } }
                if action_id == &id && *reason == error)));
        assert_eq!(
            (world.members[actor].hp, world.members[actor].spirit),
            (hp, spirit)
        );
        assert_eq!(world.members[actor].recovery_until, 0);
        assert!(world.members[actor].cooldowns.is_empty());
        assert_eq!(world.shots.len(), usize::from(failure == 2));
    }
}

#[test]
fn source_charge_release_windup_has_distinct_clocks_and_never_increases_power() {
    let source: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_cast_lifecycle_vectors.json"
    ))
    .unwrap();
    let row = &source["vectors"][0];
    let p = &row["castLifecycle"]["preparation"];
    for release_at in [Some(30_u64), Some(750), None] {
        let (mut c, mut w) = setup(true);
        c.mechanism_version = 105;
        c.step_ms = 30;
        c.skill.hp_cost = None;
        c.skill.cost = row["spiritCost"].as_u64().unwrap() as u32;
        c.skill.charge=serde_json::from_value(json!({"min_ms":p["minDurationMs"],"max_ms":p["maxDurationMs"],"min_damage_bps":7500,"early_release":{"kind":"queue_until_min"},"release_windup_ms":p["releaseWindupMs"]})).unwrap();
        w.members[0].spirit = c.skill.cost;
        let id = start(&mut w, &c, 0, 0);
        let clock = w.members[0].pending.as_ref().unwrap().charge_clock.unwrap();
        assert_eq!(
            (clock.min_release, clock.max_release, clock.release_windup),
            (360, 1410, 120)
        );
        let (commit_at, held, bps) = match release_at {
            Some(30) => (480, 350, 7500),
            Some(750) => (870, 750, 8452),
            None => (1530, 1400, 10000),
            _ => unreachable!(),
        };
        let mut commits = vec![];
        for now in (30..=commit_at).step_by(30) {
            if Some(now) == release_at {
                release(&mut w, &c, 0, &id, now).unwrap();
            }
            if now == 1410 {
                assert_eq!(release(&mut w, &c, 0, &id, now), Err(Error::InvalidState));
                assert_eq!(w.members[0].spirit, c.skill.cost);
            }
            let events = w.tick(&c, now);
            if now < commit_at {
                assert_eq!(w.members[0].spirit, c.skill.cost);
            }
            commits.extend(events.into_iter().filter(|(_, e)| {
                matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionCommitted { .. }
                    }
                )
            }));
        }
        assert_eq!(commits.len(), 1);
        let (_, q) = result(&commits, 0);
        assert_eq!((q.held_ms, q.damage_factor_bps), (held, bps));
        assert_eq!(w.members[0].spirit, 0);
    }
}

#[test]
fn source_charge_release_windup_cancel_is_uncommitted_and_free() {
    let (mut c, mut w) = setup(true);
    c.mechanism_version = 105;
    c.skill.charge=serde_json::from_value(json!({"min_ms":350,"max_ms":1400,"min_damage_bps":7500,"early_release":{"kind":"queue_until_min"},"release_windup_ms":100})).unwrap();
    let id = start(&mut w, &c, 0, 0);
    release(&mut w, &c, 0, &id, 30).unwrap();
    let before = (w.members[0].hp, w.members[0].spirit);
    w.tick(&c, 360);
    w.action(
        0,
        &Action::CancelAction {},
        "cancel-release-windup",
        &c,
        390,
    )
    .unwrap();
    w.tick(&c, 480);
    assert_eq!((w.members[0].hp, w.members[0].spirit), before);
    assert!(w.members[0].cooldowns.is_empty());
    assert!(w.shots.is_empty());
}
