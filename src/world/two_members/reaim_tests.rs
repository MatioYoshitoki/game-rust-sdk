use super::*;
use serde_json::json;

fn setup(piercing: bool) -> (Config, World) {
    let (mut config, world) = super::charge_tests::setup(piercing);
    config.mechanism_version = 48;
    config.skill.charge = serde_json::from_value(json!({
        "min_ms": 90, "max_ms": 500, "min_damage_bps": 4000,
        "aim_update": {"min_interval_ms":100, "max_turn_deg_per_sec":720}
    }))
    .unwrap();
    (config, world)
}

fn direction(degrees: f64) -> [f64; 2] {
    let angle = degrees.to_radians();
    [angle.cos(), angle.sin()]
}

fn update(
    world: &mut World,
    config: &Config,
    id: &str,
    direction: [f64; 2],
    now: u64,
) -> Result<Events> {
    world.action(
        0,
        &Action::UpdateChargeAim {
            action_id: id.into(),
            direction,
        },
        "aim",
        config,
        now,
    )
}

fn release(
    world: &mut World,
    config: &Config,
    id: &str,
    direction: [f64; 2],
    now: u64,
) -> Result<Events> {
    world.action(
        0,
        &Action::ReleaseCharge {
            action_id: id.into(),
            direction: Some(direction),
        },
        "release-with-aim",
        config,
        now,
    )
}

fn aim(world: &World) -> ([f64; 2], Option<u64>) {
    let pending = world.members[0].pending.as_ref().unwrap();
    (pending.direction, pending.aim_updated_at_ms)
}

#[test]
fn reaim_interval_angle_boundaries_and_same_direction_do_not_spend_budget() {
    let (mut config, mut world) = setup(true);
    let id = super::charge_tests::start(&mut world, &config, 0, 0);
    let original = aim(&world);
    assert_eq!(
        update(&mut world, &config, &id, [5., 0.], 99).unwrap(),
        vec![]
    );
    assert_eq!(
        update(&mut world, &config, &id, direction(1.), 99),
        Err(Error::InvalidState)
    );
    assert_eq!(
        update(&mut world, &config, &id, direction(73.), 100),
        Err(Error::OutOfRange)
    );
    assert_eq!(
        update(&mut world, &config, &id, [0., 0.], 100),
        Err(Error::InvalidMessage)
    );
    assert_eq!(aim(&world), original);
    // The accepted action keeps its original rate and turn configuration.
    config.skill.charge.as_mut().unwrap().aim_update = None;
    update(&mut world, &config, &id, direction(72.), 100).unwrap();
    let accepted = aim(&world);
    assert_eq!(accepted.1, Some(100));
    update(
        &mut world,
        &config,
        &id,
        accepted.0.map(|value| value * 5.),
        199,
    )
    .unwrap();
    assert_eq!(aim(&world), accepted);
    update(&mut world, &config, &id, direction(144.), 200).unwrap();
    update(&mut world, &config, &id, direction(-144.), 300).unwrap();
    assert_eq!(aim(&world).1, Some(300)); // The shortest turn across ±180° is 72°.
    assert_eq!(world.members[0].pending.as_ref().unwrap().commits, 500);
    assert!(world.shots.is_empty() && world.members[0].cooldowns.is_empty());
}

#[test]
fn reaim_manual_final_direction_and_elapsed_freeze_with_atomic_cost() {
    for piercing in [false, true] {
        let (config, mut world) = setup(piercing);
        let id = super::charge_tests::start(&mut world, &config, 0, 0);
        update(&mut world, &config, &id, direction(45.), 100).unwrap();
        let accepted = aim(&world);
        let hp = world.members[0].hp;
        let spirit = world.members[0].spirit;
        assert_eq!(
            release(&mut world, &config, &id, [-1., 0.], 200),
            Err(Error::OutOfRange)
        );
        assert_eq!(aim(&world), accepted);
        assert_eq!(world.members[0].pending.as_ref().unwrap().commits, 500);
        world
            .action(
                0,
                &Action::Move {
                    direction: [1., 0.],
                },
                "move",
                &config,
                200,
            )
            .unwrap();
        release(&mut world, &config, &id, [0., 5.], 250).unwrap();
        assert_eq!(
            update(&mut world, &config, &id, [1., 0.], 250),
            Err(Error::InvalidState)
        );
        assert_eq!((world.members[0].hp, world.members[0].spirit), (hp, spirit));
        let before = world.members[0].position;
        let events = world.tick(&config, 270);
        let (_, fact) = super::charge_tests::result(&events, 0);
        assert_eq!(
            fact,
            ChargeCommitView {
                held_ms: 250,
                damage_factor_bps: 6341,
                direction: Some([0., 1.])
            }
        );
        assert!(world.members[0].position[0] > before[0]);
        assert_eq!(world.members[0].hp, hp - 3);
        assert_eq!(world.members[0].spirit, spirit - config.skill.cost);
        assert_eq!(world.shots[0].direction, [0., 1.]);
        assert_eq!(world.shots[0].contact.is_some(), piercing);
        assert!(events.iter().any(
            |(_, event)| matches!(event,EventV4::ProjectileStarted{projectile}
            if projectile.position==world.members[0].position)
        ));
    }
}

#[test]
fn reaim_automatic_commit_preserves_last_accepted_direction_and_max_power() {
    let (config, mut world) = setup(true);
    let id = super::charge_tests::start(&mut world, &config, 0, 0);
    update(&mut world, &config, &id, [1., 1.], 100).unwrap();
    let last = aim(&world).0;
    assert_eq!(
        update(&mut world, &config, &id, [0., -1.], 200),
        Err(Error::OutOfRange)
    );
    assert_eq!(
        update(&mut world, &config, &id, [-1., 0.], 500),
        Err(Error::InvalidState)
    );
    let events = world.tick(&config, 510);
    let (_, fact) = super::charge_tests::result(&events, 0);
    assert_eq!(
        fact,
        ChargeCommitView {
            held_ms: 500,
            damage_factor_bps: 10_000,
            direction: Some(last)
        }
    );
    assert_eq!(world.shots[0].direction, last);
    assert_eq!(
        update(&mut world, &config, &id, [0., 1.], 510),
        Err(Error::InvalidTarget)
    );
}

#[test]
fn reaim_locked_charge_rejects_final_override_and_early_release_remains_free() {
    let (mut config, mut world) = setup(true);
    let id = super::charge_tests::start(&mut world, &config, 0, 0);
    let hp = world.members[0].hp;
    let spirit = world.members[0].spirit;
    assert_eq!(
        release(&mut world, &config, &id, [0., 0.], 20),
        Err(Error::InvalidMessage)
    );
    assert!(world.members[0].pending.is_some());
    release(&mut world, &config, &id, [0., 1.], 21).unwrap();
    assert!(world.members[0].pending.is_none());
    config.skill.charge.as_mut().unwrap().aim_update = None;
    let locked = super::charge_tests::start(&mut world, &config, 0, 30);
    assert_eq!(
        update(&mut world, &config, &locked, [1., 0.], 150),
        Err(Error::Unsupported)
    );
    assert_eq!(
        release(&mut world, &config, &locked, [0., 1.], 150),
        Err(Error::Unsupported)
    );
    assert_eq!(aim(&world), ([1., 0.], None));
    assert_eq!((world.members[0].hp, world.members[0].spirit), (hp, spirit));
}
