use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut config, world) = super::hp_cost_tests::setup(2);
    config.mechanism_version = 50;
    config.skill = config.additional_skills[1].clone();
    config.skill.id = "redirect".into();
    config.skill.windup_ms = 30;
    config.skill.recovery_ms = 120;
    config.skill.cooldown_ms = 90;
    config.skill.damage_bps = 10_000;
    config.skill.recast = serde_json::from_value(json!({"window_ms":360,"spirit_cost":7})).unwrap();
    (config, world)
}

fn cast(world: &mut World, config: &Config, member: usize, now: u64) {
    world
        .action(
            member,
            &Action::Cast {
                skill_id: config.skill.id.clone(),
                direction: [1., 0.],
            },
            "cast",
            config,
            now,
        )
        .unwrap();
    let pending = world.members[member].pending.take().unwrap();
    world.commit(member, pending, config, now + 30, &mut vec![]);
}

fn attempt(world: &mut World, config: &Config, action_id: &str, now: u64) -> Result<Events> {
    world.action(
        0,
        &Action::RecastProjectile {
            action_id: action_id.into(),
            recast_index: 1,
            direction: [0., 4.],
        },
        "redirect",
        config,
        now,
    )
}

#[test]
fn recast_redirects_existing_shot_and_hits_from_its_current_position() {
    let (config, mut world) = setup();
    world.npcs.truncate(1);
    let [x, y] = world.members[0].position;
    world.npcs[0].position = [x + 45., y + 60.];
    let target_hp = world.npcs[0].hp;
    let hp = world.members[0].hp;
    cast(&mut world, &config, 0, 0);
    let recast = world.members[0].recast.as_ref().unwrap().view.clone();
    world.advance_shots(&config, 60, 45. / config.skill.speed, &mut vec![]);
    let shot = &world.shots[0];
    let before = (
        shot.view.position,
        shot.remaining,
        shot.expires,
        shot.created,
        shot.raw,
    );
    assert_eq!(before.0, [x + 45., y]);
    let spirit = world.members[0].spirit;
    let cd = world.members[0].cooldowns[&config.skill.id];
    let events = attempt(&mut world, &config, &recast.action_id, 60).unwrap();
    assert!(matches!(events.as_slice(), [(Some(0), EventV4::Private {
        event: Event::ProjectileRecast { action_id, projectile_id, recast_index: 1, direction, spirit_cost: 7 }
    })] if action_id == &recast.action_id && projectile_id == &recast.projectile_id && *direction == [0., 1.]));
    let shot = &world.shots[0];
    assert_eq!(world.shots.len(), 1);
    assert_eq!(shot.view.id, recast.projectile_id);
    assert_eq!(
        before,
        (
            shot.view.position,
            shot.remaining,
            shot.expires,
            shot.created,
            shot.raw
        )
    );
    assert_eq!(world.members[0].hp, hp - 3);
    assert_eq!(world.members[0].spirit, spirit - 7);
    assert_eq!(world.members[0].cooldowns[&config.skill.id], cd);
    assert_eq!(world.members[0].recovery_until, 150);
    assert_eq!(
        attempt(&mut world, &config, &recast.action_id, 61),
        Err(Error::InvalidTarget)
    );
    world.advance_shots(&config, 90, 80. / config.skill.speed, &mut vec![]);
    assert!(world.npcs[0].hp < target_hp);
}

#[test]
fn recast_failures_preserve_opportunity_and_window_end_is_exclusive() {
    let (config, mut world) = setup();
    world.npcs.clear();
    cast(&mut world, &config, 0, 0);
    let recast = world.members[0].recast.as_ref().unwrap().view.clone();
    world.members[0].spirit = 6;
    assert_eq!(
        attempt(&mut world, &config, &recast.action_id, 60),
        Err(Error::NoResource)
    );
    assert_eq!(world.shots[0].direction, [1., 0.]);
    assert_eq!(world.members[0].spirit, 6);
    assert_eq!(world.members[0].recast.as_ref().unwrap().view, recast);
    assert_eq!(
        world.action(
            0,
            &Action::RecastProjectile {
                action_id: recast.action_id.clone(),
                recast_index: 1,
                direction: [0., 0.]
            },
            "zero-direction",
            &config,
            60
        ),
        Err(Error::InvalidMessage)
    );
    world.members[0].spirit = 7;
    assert_eq!(
        attempt(&mut world, &config, &recast.action_id, recast.expires_at_ms),
        Err(Error::InvalidTarget)
    );
    assert!(
        attempt(
            &mut world,
            &config,
            &recast.action_id,
            recast.expires_at_ms - 1
        )
        .is_ok()
    );
    assert_eq!(world.members[0].spirit, 0);
}

#[test]
fn recast_replacement_cancel_projectile_end_and_source_end_are_scoped() {
    let (config, mut world) = setup();
    world.npcs.clear();
    cast(&mut world, &config, 0, 0);
    cast(&mut world, &config, 1, 0);
    let first = world.members[0].recast.as_ref().unwrap().view.clone();
    let peer = world.members[1].recast.as_ref().unwrap().view.clone();
    cast(&mut world, &config, 0, 150);
    let second = world.members[0].recast.as_ref().unwrap().view.clone();
    assert_ne!(first.action_id, second.action_id);
    assert!(
        world
            .shots
            .iter()
            .any(|shot| shot.view.id == first.projectile_id)
    );
    assert_eq!(
        attempt(&mut world, &config, &first.action_id, 181),
        Err(Error::InvalidTarget)
    );
    world
        .action(0, &Action::CancelAction {}, "cancel", &config, 181)
        .unwrap();
    assert!(world.members[0].recast.is_none());
    assert!(
        world
            .shots
            .iter()
            .any(|shot| shot.view.id == second.projectile_id)
    );
    cast(&mut world, &config, 0, 300);
    let shot = world.shots.last_mut().unwrap();
    shot.remaining = 0.01;
    world.tick(&config, 360);
    assert!(world.members[0].recast.is_none());
    assert_eq!(world.members[1].recast.as_ref().unwrap().view, peer);
    world.terminal_intent(1, Outcome::Death, &mut vec![]);
    assert!(world.members[1].recast.is_none());
    assert!(
        world
            .shots
            .iter()
            .all(|shot| shot.view.id != peer.projectile_id)
    );
}
