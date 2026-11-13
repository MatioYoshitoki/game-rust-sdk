use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut config, mut world) = super::hp_cost_tests::setup(2);
    config.mechanism_version = 53;
    config.basic_followup = serde_json::from_value(json!({"grace_ms":150})).unwrap();
    config.basic.interval_ms = 275;
    config.basic.windup_ms = 30;
    config.basic.range = 240.;
    world.npcs.truncate(1);
    let [x, y] = world.members[0].position;
    world.npcs[0].position = [x + 60., y];
    (config, world)
}

fn attack(world: &mut World, config: &Config, id: &str, now: u64) -> Result<Events> {
    world.action(0, &Action::Attack {}, id, config, now)
}

fn commit(world: &mut World, config: &Config, now: u64) -> Events {
    let pending = world.members[0].pending.take().unwrap();
    let mut events = vec![];
    world.commit(0, pending, config, now, &mut events);
    events
}

fn open(world: &mut World, config: &Config) {
    attack(world, config, "open", 0).unwrap();
    assert_eq!(
        attack(world, config, "too-early", 0),
        Err(Error::InvalidState)
    );
    commit(world, config, 30);
    assert_eq!(world.members[0].basic_ready, 305);
    assert_eq!(world.shots.len(), 1);
}

fn assert_rejected(events: &Events, expected: Error) {
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
fn basic_followup_latest_at_cooldown_start_promotes_on_rounded_step_and_aims_again_at_commit() {
    let (mut config, mut world) = setup();
    config.basic_followup.as_mut().unwrap().grace_ms = 0;
    open(&mut world, &config);
    let hp = world.members[0].hp;
    let spirit = world.members[0].spirit;
    attack(&mut world, &config, "queued", 30).unwrap();
    attack(&mut world, &config, "latest", 60).unwrap();
    let queue = world.members[0].queued_basic.clone().unwrap();
    assert_eq!(
        (
            queue.action_id.as_str(),
            queue.promotes_at_ms,
            queue.expires_at_ms
        ),
        ("latest", 330, 330)
    );
    assert_eq!(world.members[0].basic_ready, 305);
    assert_eq!(world.shots.len(), 1);
    world.promote_basic(0, &config, 300, &mut vec![]);
    assert!(world.members[0].pending.is_none());
    world.promote_basic(0, &config, 330, &mut vec![]);
    assert_eq!(world.members[0].pending.as_ref().unwrap().commits, 360);
    assert!(world.members[0].queued_basic.is_none());
    let [x, y] = world.members[0].position;
    world.npcs[0].position = [x, y + 60.];
    let before = world.npcs[0].hp;
    let events = commit(&mut world, &config, 360);
    assert!(events.iter().any(|(recipient, event)| *recipient == Some(0) && matches!(event,
        EventV4::Private { event: Event::ActionCommitted { action_id, skill: false, hp_cost: None, spirit_cost: 0, .. } }
        if action_id == "latest")));
    assert_eq!(world.members[0].basic_ready, 635);
    assert_eq!(world.members[0].hp, hp);
    assert_eq!(world.members[0].spirit, spirit);
    assert_eq!(world.shots[1].direction, [0., 1.]);
    assert!(world.shots[1].target.is_none());
    world.advance_shots(&config, 390, 1., &mut vec![]);
    assert!(world.npcs[0].hp < before);
}

#[test]
fn basic_followup_rejects_stale_target_range_and_visibility_at_promotion_and_commit() {
    for at_commit in [false, true] {
        for case in 0..4 {
            let (config, mut world) = setup();
            open(&mut world, &config);
            attack(&mut world, &config, "queued", 60).unwrap();
            if at_commit {
                world.promote_basic(0, &config, 330, &mut vec![]);
                assert!(world.members[0].pending.is_some());
            }
            match case {
                0 => world.npcs[0].entity.generation += 1,
                1 => world.npcs[0].position[0] += 300.,
                2 => Arc::make_mut(&mut world.grid.config).palette[0].blocks_vision = true,
                _ => world.npcs[0].hp = 0,
            }
            let events = if at_commit {
                commit(&mut world, &config, 360)
            } else {
                let mut events = vec![];
                world.promote_basic(0, &config, 330, &mut events);
                events
            };
            assert_rejected(&events, Error::InvalidTarget);
            assert_eq!(world.members[0].basic_ready, 305);
            assert_eq!(world.shots.len(), 1);
            assert!(world.members[0].queued_basic.is_none() && world.members[0].pending.is_none());
        }
    }
}

#[test]
fn basic_followup_stop_preserves_queue_skill_can_take_over_and_late_queue_expires() {
    let (config, mut world) = setup();
    open(&mut world, &config);
    attack(&mut world, &config, "queued", 60).unwrap();
    let queue = world.members[0].queued_basic.clone();
    world
        .action(0, &Action::Stop {}, "stop", &config, 60)
        .unwrap();
    let invalid = Action::Cast {
        skill_id: "missing".into(),
        direction: [1., 0.],
    };
    assert_eq!(
        world.action(0, &invalid, "bad", &config, 60),
        Err(Error::InvalidTarget)
    );
    assert_eq!(world.members[0].queued_basic, queue);
    assert!(matches!(
        world
            .action(0, &Action::CancelAction {}, "cancel", &config, 60)
            .unwrap()
            .as_slice(),
        [(
            Some(0),
            EventV4::Private {
                event: Event::ActionInterrupted
            }
        )]
    ));
    assert!(world.members[0].queued_basic.is_none());
    attack(&mut world, &config, "queued", 90).unwrap();
    let mut events = vec![];
    world.promote_basic(0, &config, 510, &mut events);
    assert_rejected(&events, Error::InvalidState);
    assert_eq!(world.members[0].basic_ready, 305);

    attack(&mut world, &config, "queued", 90).unwrap();
    let skill = Action::Cast {
        skill_id: config.skill.id.clone(),
        direction: [1., 0.],
    };
    world.action(0, &skill, "takeover", &config, 90).unwrap();
    assert!(world.members[0].queued_basic.is_none());
    assert_eq!(world.members[0].pending.as_ref().unwrap().id, "takeover");
    world.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert!(world.members[0].pending.is_none() && world.members[0].queued_basic.is_none());
}
