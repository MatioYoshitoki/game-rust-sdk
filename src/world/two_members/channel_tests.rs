use super::*;
use serde_json::json;

pub(super) fn setup(keep_targets: bool) -> (Config, World) {
    let (mut config, mut world) = super::hp_cost_tests::setup(2);
    config.mechanism_version = 49;
    config.skill = config.additional_skills[1].clone();
    config.skill.id = "channel".into();
    config.skill.windup_ms = 60;
    config.skill.recovery_ms = 120;
    config.skill.damage_bps = 10_000;
    config.skill.channel = serde_json::from_value(json!({
        "duration_ms":360, "interval_ms":90, "spirit_per_pulse":3
    }))
    .unwrap();
    config.skill.cast_movement = serde_json::from_value(json!({
        "preparation":{"kind":"free"}, "recovery":{"kind":"locked"}
    }))
    .unwrap();
    if !keep_targets {
        world.npcs.clear();
    }
    (config, world)
}

pub(super) fn start(world: &mut World, config: &Config, member: usize) -> String {
    world
        .action(
            member,
            &Action::Cast {
                skill_id: config.skill.id.clone(),
                direction: [1., 0.],
            },
            "channel-input",
            config,
            0,
        )
        .unwrap();
    world.members[member].pending.as_ref().unwrap().id.clone()
}

fn pulse(events: &Events, member: usize, id: &str, number: u32) -> String {
    events
        .iter()
        .find_map(|(recipient, event)| match event {
            EventV4::Private {
                event:
                    Event::ChannelPulse {
                        action_id,
                        pulse,
                        spirit_cost: 3,
                        projectile_id,
                    },
            } if *recipient == Some(member) && action_id == id && *pulse == number => {
                Some(projectile_id.clone())
            }
            _ => None,
        })
        .expect("private paid pulse")
}

fn ended(events: &Events, member: usize, id: &str, expected: ChannelEndReason) {
    assert!(
        events
            .iter()
            .any(|(recipient, event)| *recipient == Some(member)
                && matches!(event,EventV4::Private{event:Event::ChannelEnded{action_id,reason}}
            if action_id==id && *reason==expected))
    );
}

#[test]
fn channel_clock_freezes_attack_and_applies_locked_motion_then_recovery() {
    let (config, mut world) = setup(false);
    let hp = world.members[0].hp;
    let spirit = world.members[0].spirit;
    let id = start(&mut world, &config, 0);
    assert!(world.members[0].channel.is_none());
    let committed = world.tick(&config, 60);
    assert!(committed.iter().any(|(_,event)|matches!(event,
        EventV4::Private{event:Event::ActionCommitted{action_id,hp_cost:Some(3),..}} if action_id==&id)));
    assert_eq!(world.members[0].hp, hp - 3);
    assert_eq!(world.members[0].spirit, spirit - config.skill.cost);
    assert!(world.shots.is_empty());
    let position = world.members[0].position;
    let raw = world.members[0].channel.as_ref().unwrap().raw;
    world.members[0].facts.atk_points += 100;
    for member in [0, 1] {
        world
            .action(
                member,
                &Action::Move {
                    direction: [1., 0.],
                },
                "move",
                &config,
                60,
            )
            .unwrap();
    }
    let peer = world.members[1].position;
    assert_eq!(
        world.action(0, &Action::ExtractStart {}, "extract", &config, 61),
        Err(Error::InvalidState)
    );
    assert_eq!(
        world.action(
            0,
            &Action::Cast {
                skill_id: config.skill.id.clone(),
                direction: [1., 0.]
            },
            "second-cast",
            &config,
            61
        ),
        Err(Error::InvalidState)
    );
    world.tick(&config, 149);
    assert!(world.shots.is_empty());
    for (number, time) in [(1, 150), (2, 240), (3, 330)] {
        let events = world.tick(&config, time);
        let shot = pulse(&events, 0, &id, number);
        let projectile = world
            .shots
            .iter()
            .find(|candidate| candidate.view.id == shot)
            .unwrap();
        assert_eq!(projectile.raw, raw);
        assert_eq!(projectile.direction, [1., 0.]);
        assert_eq!(projectile.view.position, position);
    }
    let view = world.snapshot(0, &config, Phase::Active, 330);
    assert_eq!(view.own.active_channel.unwrap().next_pulse_at_ms, None);
    assert_eq!(world.members[0].position, position);
    assert!(world.members[1].position[0] > peer[0]);
    let events = world.tick(&config, 420);
    ended(&events, 0, &id, ChannelEndReason::Completed);
    assert!(world.members[0].channel.is_none());
    assert_eq!(world.members[0].spirit, spirit - config.skill.cost - 9);
    assert_eq!(world.members[0].recovery_until, 540);
    assert_eq!(
        world.members[0].cooldowns[&config.skill.id],
        60 + config.skill.cooldown_ms
    );
    assert_eq!(world.members[0].position, position);
    world.tick(&config, 540);
    assert!(world.members[0].position[0] > position[0]);
}

#[test]
fn channel_projectile_keeps_real_damage_after_explicit_cancel() {
    let (config, mut world) = setup(true);
    world.npcs.truncate(1);
    let position = world.members[0].position;
    world.npcs[0].position = [position[0] + 45., position[1]];
    let before = world.npcs[0].hp;
    let id = start(&mut world, &config, 0);
    let pending = world.members[0].pending.take().unwrap();
    world.commit(0, pending, &config, 60, &mut vec![]);
    let mut events = vec![];
    world.advance_channel(0, &config, 150, &mut events);
    let shot = pulse(&events, 0, &id, 1);
    let events = world
        .action(0, &Action::CancelAction {}, "cancel", &config, 151)
        .unwrap();
    ended(&events, 0, &id, ChannelEndReason::Cancelled);
    assert_eq!(world.shots[0].view.id, shot);
    world.advance_shots(&config, 180, 1., &mut vec![]);
    assert!(world.npcs[0].hp < before);
    let spirit = world.members[0].spirit;
    assert!(
        world
            .action(0, &Action::CancelAction {}, "duplicate", &config, 200)
            .unwrap()
            .is_empty()
    );
    world.advance_channel(0, &config, 240, &mut vec![]);
    assert_eq!(world.members[0].spirit, spirit);
    assert_eq!(world.members[0].recovery_until, 271);
}

#[test]
fn channel_pulse_failures_leave_cost_and_projectiles_atomic() {
    for reason in [
        ChannelEndReason::NoResource,
        ChannelEndReason::InvalidOrigin,
        ChannelEndReason::Capacity,
    ] {
        let (mut config, mut world) = setup(false);
        let member = if reason == ChannelEndReason::Capacity {
            config.members.as_mut().unwrap().projectile_capacity = 1;
            start(&mut world, &config, 0);
            1
        } else {
            0
        };
        let id = start(&mut world, &config, member);
        world.tick(&config, 60);
        if reason == ChannelEndReason::NoResource {
            world.members[member].spirit = 2;
        }
        if reason == ChannelEndReason::InvalidOrigin {
            world.members[member].position = [-1., -1.];
        }
        let spirit = world.members[member].spirit;
        let events = world.tick(&config, 150);
        ended(&events, member, &id, reason);
        assert_eq!(world.members[member].spirit, spirit);
        assert_eq!(
            world.shots.len(),
            usize::from(reason == ChannelEndReason::Capacity)
        );
        assert!(world.members[member].channel.is_none());
        assert_eq!(world.members[member].recovery_until, 270);
    }
}

#[test]
fn channel_protection_clears_movement_and_preserves_launched_projectile() {
    let (config, mut world) = setup(false);
    let id = start(&mut world, &config, 0);
    world.tick(&config, 60);
    world
        .action(
            0,
            &Action::Move {
                direction: [0., 1.],
            },
            "move",
            &config,
            70,
        )
        .unwrap();
    let shot = pulse(&world.tick(&config, 150), 0, &id, 1);
    let mut events = vec![];
    world.protect(0, 151, &mut events);
    ended(&events, 0, &id, ChannelEndReason::Interrupted);
    assert_eq!(world.members[0].movement, [0., 0.]);
    assert_eq!(world.members[0].recovery_until, 271);
    let spirit = world.members[0].spirit;
    world.tick(&config, 240);
    assert_eq!(world.members[0].spirit, spirit);
    assert!(
        world
            .shots
            .iter()
            .any(|projectile| projectile.view.id == shot)
    );
}

#[test]
fn channel_terminal_cleanup_only_retires_its_own_action_and_shots() {
    let (config, mut world) = setup(false);
    let actor = start(&mut world, &config, 0);
    let peer = start(&mut world, &config, 1);
    assert_ne!(actor, peer);
    world.tick(&config, 60);
    let events = world.tick(&config, 150);
    let first = pulse(&events, 0, &actor, 1);
    let other = pulse(&events, 1, &peer, 1);
    let mut events = vec![];
    world.terminal_intent(0, Outcome::Death, &mut events);
    ended(&events, 0, &actor, ChannelEndReason::SourceInvalid);
    assert!(
        world
            .shots
            .iter()
            .all(|projectile| projectile.view.id != first)
    );
    assert!(
        world
            .shots
            .iter()
            .any(|projectile| projectile.view.id == other)
    );
    let events = world.tick(&config, 240);
    pulse(&events, 1, &peer, 2);
    assert!(world.members[0].channel.is_none());
}
