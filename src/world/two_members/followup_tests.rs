use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut config, mut world) = super::hp_cost_tests::setup(2);
    config.mechanism_version = 51;
    config.skill_followup = serde_json::from_value(json!({"window_ms":150})).unwrap();
    config.skill = config.additional_skills[1].clone();
    config.skill.id = "opener".into();
    config.skill.windup_ms = 30;
    config.skill.recovery_ms = 275;
    let mut first = config.skill.clone();
    first.id = "first".into();
    first.windup_ms = 60;
    first.cost = 4;
    first.cast_movement = serde_json::from_value(json!({
        "preparation":{"kind":"locked"}, "recovery":{"kind":"free"}
    }))
    .unwrap();
    let mut latest = first.clone();
    latest.id = "latest".into();
    latest.cost = 7;
    config.additional_skills = vec![first, latest];
    world.npcs.clear();
    (config, world)
}

fn cast(
    world: &mut World,
    config: &Config,
    skill_id: &str,
    id: &str,
    direction: [f64; 2],
    now: u64,
) -> Result<Events> {
    world.action(
        0,
        &Action::Cast {
            skill_id: skill_id.into(),
            direction,
        },
        id,
        config,
        now,
    )
}

fn open(world: &mut World, config: &Config) {
    cast(world, config, "opener", "open", [1., 0.], 0).unwrap();
    world.tick(config, 30);
    assert_eq!(world.members[0].recovery_until, 305);
}

#[test]
fn followup_latest_valid_at_window_boundary_promotes_then_commits_once() {
    let (config, mut world) = setup();
    open(&mut world, &config);
    let hp = world.members[0].hp;
    let spirit = world.members[0].spirit;
    let cooldown = world.members[0].cooldowns["opener"];
    assert_eq!(
        cast(&mut world, &config, "first", "early", [1., 0.], 150),
        Err(Error::InvalidState)
    );
    assert_eq!(
        cast(&mut world, &config, "opener", "same", [1., 0.], 180),
        Err(Error::InvalidState)
    );
    cast(&mut world, &config, "first", "first-valid", [1., 0.], 180).unwrap();
    assert_eq!(
        world.members[0].queued_cast.as_ref().unwrap().expires_at_ms,
        330
    );
    world.members[0].cooldowns.insert("latest".into(), 331);
    assert_eq!(
        cast(&mut world, &config, "latest", "cooling", [0., 1.], 180),
        Err(Error::InvalidState)
    );
    assert_eq!(
        world.members[0].queued_cast.as_ref().unwrap().skill_id,
        "first"
    );
    world.members[0].cooldowns.insert("latest".into(), 330);
    cast(&mut world, &config, "latest", "latest-valid", [0., 2.], 180).unwrap();
    assert_eq!(
        cast(&mut world, &config, "first", "bad-direction", [0., 0.], 180),
        Err(Error::InvalidMessage)
    );
    assert_eq!(
        world.members[0].queued_cast.as_ref().unwrap().direction,
        [0., 1.]
    );
    assert_eq!(world.members[0].cooldowns["latest"], 330);
    assert_eq!(world.members[0].hp, hp);
    assert_eq!(world.members[0].spirit, spirit);
    assert_eq!(world.shots.len(), 1);
    world.tick(&config, 300);
    assert!(world.members[0].pending.is_none());
    let position = world.members[0].position;
    world
        .action(
            0,
            &Action::Move {
                direction: [1., 0.],
            },
            "move",
            &config,
            300,
        )
        .unwrap();
    world.tick(&config, 330);
    let pending = world.members[0].pending.as_ref().unwrap();
    assert_eq!(
        (pending.id.as_str(), pending.commits),
        ("latest-valid", 390)
    );
    assert_eq!(world.members[0].position, position);
    assert_eq!(world.members[0].hp, hp);
    assert_eq!(world.members[0].spirit, spirit);
    assert!(world.members[0].queued_cast.is_none());
    let events = world.tick(&config, 390);
    assert!(events.iter().any(|(_, event)| matches!(event, EventV4::Private { event:
        Event::ActionCommitted { action_id, skill_id: Some(skill_id), spirit_cost: 7, hp_cost: Some(3), .. }
    } if action_id == "latest-valid" && skill_id == "latest")));
    assert_eq!(world.members[0].spirit, spirit - 7);
    assert_eq!(world.members[0].hp, hp - 3);
    assert_eq!(world.members[0].cooldowns["opener"], cooldown);
    assert_eq!(
        world.members[0].cooldowns["latest"],
        390 + config.additional_skills[1].cooldown_ms
    );
    assert_eq!(world.shots.last().unwrap().direction, [0., 1.]);
    assert_eq!(world.shots.last().unwrap().view.position, position);
    assert!(!world.members[0].cooldowns.contains_key("first"));
}

fn rejected(events: Events, expected: Error) {
    assert!(
        events.iter().any(|(recipient, event)| *recipient == Some(0)
            && matches!(event,
                EventV4::Private { event: Event::ActionRejected { action_id, reason } }
                if action_id == "queued" && *reason == expected
            )),
        "{events:?}"
    );
}

#[test]
fn followup_expiry_promotion_and_commit_failures_are_unpaid() {
    for (failure_time, expected) in [
        (360, Error::InvalidState),
        (330, Error::NoResource),
        (390, Error::NoResource),
    ] {
        let (config, mut world) = setup();
        open(&mut world, &config);
        cast(&mut world, &config, "latest", "queued", [1., 0.], 180).unwrap();
        if failure_time == 390 {
            world.tick(&config, 330);
            assert!(world.members[0].pending.is_some());
        }
        let hp = world.members[0].hp;
        world.members[0].spirit = 0;
        rejected(world.tick(&config, failure_time), expected);
        assert_eq!(world.members[0].hp, hp);
        assert_eq!(world.members[0].spirit, 0);
        assert!(!world.members[0].cooldowns.contains_key("latest"));
        assert!(world.members[0].pending.is_none() && world.members[0].queued_cast.is_none());
    }
    for fail_after_promotion in [false, true] {
        let (config, mut world) = setup();
        open(&mut world, &config);
        cast(&mut world, &config, "latest", "queued", [1., 0.], 180).unwrap();
        if fail_after_promotion {
            world.tick(&config, 330);
        }
        let spirit = world.members[0].spirit;
        world.members[0].position = [-1., -1.];
        rejected(
            world.tick(&config, if fail_after_promotion { 390 } else { 330 }),
            Error::InvalidTarget,
        );
        assert_eq!(world.members[0].spirit, spirit);
        assert!(!world.members[0].cooldowns.contains_key("latest"));
    }
}

#[test]
fn followup_stop_preserves_intent_cancel_and_terminal_clear_it() {
    let (config, mut world) = setup();
    open(&mut world, &config);
    cast(&mut world, &config, "latest", "queued", [1., 0.], 180).unwrap();
    world
        .action(0, &Action::Stop {}, "stop", &config, 180)
        .unwrap();
    assert!(world.members[0].queued_cast.is_some());
    assert_eq!(
        world.action(0, &Action::ExtractStart {}, "extract", &config, 180),
        Err(Error::InvalidState)
    );
    let events = world
        .action(0, &Action::CancelAction {}, "cancel", &config, 180)
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [(
            Some(0),
            EventV4::Private {
                event: Event::ActionInterrupted
            }
        )]
    ));
    assert!(world.members[0].queued_cast.is_none());
    assert_eq!(world.members[0].recovery_until, 305);
    assert_eq!(world.shots.len(), 1);
    cast(&mut world, &config, "latest", "requeued", [1., 0.], 180).unwrap();
    world.terminal_intent(0, Outcome::Death, &mut vec![]);
    world.tick(&config, 390);
    assert!(world.members[0].queued_cast.is_none() && world.members[0].pending.is_none());
    assert!(!world.members[0].cooldowns.contains_key("latest"));
}
