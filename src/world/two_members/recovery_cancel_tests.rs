use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut config, mut world) = super::hp_cost_tests::setup(2);
    config.mechanism_version = 52;
    config.skill = config.additional_skills[1].clone();
    config.skill.id = "opener".into();
    config.skill.windup_ms = 30;
    config.skill.recovery_ms = 300;
    config.skill.recovery_cancel = serde_json::from_value(json!({"after_ms":210})).unwrap();
    config.skill.cast_movement = serde_json::from_value(json!({
        "preparation":{"kind":"free"}, "recovery":{"kind":"locked"}
    }))
    .unwrap();
    let mut next = config.skill.clone();
    next.id = "next".into();
    next.windup_ms = 60;
    next.cost = 7;
    next.recovery_cancel = None;
    config.additional_skills = vec![next];
    world.npcs.clear();
    (config, world)
}

fn cast(world: &mut World, config: &Config, skill_id: &str, id: &str, now: u64) -> Result<Events> {
    world.action(
        0,
        &Action::Cast {
            skill_id: skill_id.into(),
            direction: [0., 1.],
        },
        id,
        config,
        now,
    )
}

fn open(world: &mut World, config: &Config) {
    cast(world, config, "opener", "opener-input", 0).unwrap();
    world.tick(config, 30);
    assert_eq!(world.members[0].recovery_until, 330);
}

#[test]
fn recovery_cancel_explicit_equal_gate_resumes_movement_and_keeps_committed_shot() {
    let (config, mut world) = setup();
    open(&mut world, &config);
    let previous = world.shots[0].view.id.clone();
    let hp = world.members[0].hp;
    let spirit = world.members[0].spirit;
    let cooldown = world.members[0].cooldowns["opener"];
    let position = world.members[0].position;
    world
        .action(
            0,
            &Action::Move {
                direction: [1., 0.],
            },
            "move",
            &config,
            180,
        )
        .unwrap();
    world.tick(&config, 210);
    assert_eq!(world.members[0].position, position);
    assert!(
        world
            .action(0, &Action::CancelAction {}, "early", &config, 239)
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.members[0].recovery_until, 330);
    let events = world
        .action(0, &Action::CancelAction {}, "on-gate", &config, 240)
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
    assert_eq!(world.members[0].recovery_until, 240);
    assert!(world.members[0].recovery_cancel_view(240).is_none());
    assert_eq!(world.shots[0].view.id, previous);
    assert_eq!(world.members[0].hp, hp);
    assert_eq!(world.members[0].spirit, spirit);
    assert_eq!(world.members[0].cooldowns["opener"], cooldown);
    world.tick(&config, 270);
    assert!(world.members[0].position[0] > position[0]);
    assert!(
        world
            .action(0, &Action::CancelAction {}, "later", &config, 270)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn recovery_cancel_replacement_only_clears_queue_after_successful_admission() {
    let (mut config, mut world) = setup();
    config.skill_followup = serde_json::from_value(json!({"window_ms":180})).unwrap();
    open(&mut world, &config);
    let spirit = world.members[0].spirit;
    let hp = world.members[0].hp;
    let cd = world.members[0].cooldowns["opener"];
    cast(&mut world, &config, "next", "queued", 150).unwrap();
    let queued = world.members[0].queued_cast.clone();
    assert!(queued.is_some());
    world.members[0].cooldowns.insert("next".into(), 241);
    assert_eq!(
        cast(&mut world, &config, "next", "blocked", 240),
        Err(Error::InvalidState)
    );
    assert_eq!(world.members[0].queued_cast, queued);
    assert_eq!(world.members[0].recovery_until, 330);
    world.members[0].cooldowns.insert("next".into(), 240);
    world.members[0].spirit = 6;
    assert_eq!(
        cast(&mut world, &config, "next", "unpaid", 240),
        Err(Error::NoResource)
    );
    assert_eq!(world.members[0].queued_cast, queued);
    assert_eq!(world.members[0].recovery_until, 330);
    assert_eq!(
        cast(&mut world, &config, "opener", "same", 240),
        Err(Error::InvalidState)
    );
    world.members[0].spirit = spirit;
    cast(&mut world, &config, "next", "replacement", 240).unwrap();
    assert_eq!(world.members[0].recovery_until, 240);
    assert!(world.members[0].queued_cast.is_none());
    let pending = world.members[0].pending.as_ref().unwrap();
    assert_eq!((pending.id.as_str(), pending.commits), ("replacement", 300));
    assert_eq!(world.members[0].spirit, spirit);
    assert_eq!(world.members[0].hp, hp);
    assert_eq!(world.shots.len(), 1);
    let events = world.tick(&config, 300);
    assert!(
        events
            .iter()
            .any(|(_, event)| matches!(event, EventV4::Private {
        event: Event::ActionCommitted { action_id, hp_cost: Some(3), spirit_cost: 7, .. }
    } if action_id == "replacement"))
    );
    assert_eq!(world.shots.len(), 2);
    assert_eq!(world.shots[1].direction, [0., 1.]);
    assert_eq!(world.members[0].cooldowns["opener"], cd);
    assert_eq!(world.members[0].spirit, spirit - 7);
    assert_eq!(world.members[0].hp, hp - 3);
}

#[test]
fn recovery_cancel_replacement_commit_revalidates_origin_and_natural_end_is_ordinary() {
    let (config, mut world) = setup();
    open(&mut world, &config);
    let spirit = world.members[0].spirit;
    assert_eq!(
        cast(&mut world, &config, "next", "early", 239),
        Err(Error::InvalidState)
    );
    cast(&mut world, &config, "next", "replacement", 240).unwrap();
    world.members[0].position = [-1., -1.];
    let events = world.tick(&config, 300);
    assert!(
        events
            .iter()
            .any(|(_, event)| matches!(event, EventV4::Private {
        event: Event::ActionRejected { action_id, reason: Error::InvalidTarget }
    } if action_id == "replacement"))
    );
    assert_eq!(world.members[0].spirit, spirit);
    assert_eq!(world.members[0].recovery_until, 240);
    assert!(!world.members[0].cooldowns.contains_key("next"));
    assert_eq!(world.shots.len(), 1);

    let (config, mut world) = setup();
    open(&mut world, &config);
    assert!(
        world
            .action(0, &Action::CancelAction {}, "boundary", &config, 330)
            .unwrap()
            .is_empty()
    );
    cast(&mut world, &config, "next", "ordinary", 330).unwrap();
    assert!(
        !world.members[0]
            .pending
            .as_ref()
            .unwrap()
            .recovery_replacement
    );
    world.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert!(world.members[0].recovery_cancel_at.is_none());
}

#[test]
fn recovery_policy_every_native_skill_preserves_its_commit_and_movement_permission() {
    let (mut c, base) = super::hp_cost_tests::setup(2);
    drop(base);
    c.mechanism_version = 105;
    let skills: Vec<_> = c.player_skills().cloned().collect();
    for mut skill in skills {
        let (_, mut w) = super::hp_cost_tests::setup(2);
        skill.windup_ms = 30;
        skill.recovery_ms = 300;
        skill.recovery_cancel =
            serde_json::from_value(json!({"after_ms":0,"by_move":true,"by_explicit_action":false}))
                .unwrap();
        c.skill = skill.clone();
        c.additional_skills.clear();
        let action = if skill.is_self() {
            Action::CastSelf {
                skill_id: skill.id.clone(),
            }
        } else if skill.is_unit() {
            Action::CastUnit {
                skill_id: skill.id.clone(),
                target: World::unit_wire(&w.npcs[0].entity),
            }
        } else if skill.is_ground() {
            Action::CastGround {
                skill_id: skill.id.clone(),
                position: w.npcs[0].position,
            }
        } else {
            Action::Cast {
                skill_id: skill.id.clone(),
                direction: [1., 0.],
            }
        };
        w.action(0, &action, "native-recovery", &c, 0).unwrap();
        let pending = w.members[0].pending.take().unwrap();
        w.commit(0, pending, &c, 30, &mut vec![]);
        assert_eq!(w.members[0].recovery_until, 330, "{}", skill.id);
        let before = (
            w.members[0].hp,
            w.members[0].spirit,
            w.shots.len(),
            w.areas.len(),
        );
        w.action(0, &Action::CancelAction {}, "no-explicit", &c, 31)
            .unwrap();
        assert_eq!(w.members[0].recovery_until, 330);
        w.action(0, &Action::Stop {}, "stop", &c, 32).unwrap();
        assert_eq!(w.members[0].recovery_until, 330);
        w.action(
            0,
            &Action::Move {
                direction: [0., 0.],
            },
            "zero",
            &c,
            33,
        )
        .unwrap();
        assert_eq!(w.members[0].recovery_until, 330);
        assert!(
            w.action(
                0,
                &Action::Navigate {
                    destination: [-1., -1.]
                },
                "invalid",
                &c,
                34
            )
            .is_err()
        );
        assert_eq!(w.members[0].recovery_until, 330);
        w.action(
            0,
            &Action::Move {
                direction: [0., 1.],
            },
            "valid",
            &c,
            35,
        )
        .unwrap();
        assert_eq!(w.members[0].recovery_until, 35);
        assert_eq!(
            (
                w.members[0].hp,
                w.members[0].spirit,
                w.shots.len(),
                w.areas.len()
            ),
            before
        );
    }
}
#[test]
fn recovery_policy_self_skill_takeover_is_atomic_and_keeps_original_flight() {
    let (mut c, mut w) = setup();
    c.mechanism_version = 105;
    c.skill.recovery_cancel =
        serde_json::from_value(json!({"after_ms":0,"by_move":false,"by_explicit_action":true}))
            .unwrap();
    let mut self_skill = super::hp_cost_tests::setup(2).0.additional_skills[0].clone();
    self_skill.id = "self-takeover".into();
    self_skill.windup_ms = 30;
    self_skill.cost = 1_000_000;
    c.additional_skills = vec![self_skill];
    open(&mut w, &c);
    let spirit = w.members[0].spirit;
    let flight = w.shots[0].view.id.clone();
    assert_eq!(
        w.action(
            0,
            &Action::CastSelf {
                skill_id: "self-takeover".into()
            },
            "too-expensive",
            &c,
            31
        ),
        Err(Error::NoResource)
    );
    assert_eq!(w.members[0].recovery_until, 330);
    c.additional_skills[0].cost = 1;
    w.action(
        0,
        &Action::CastSelf {
            skill_id: "self-takeover".into(),
        },
        "valid-self",
        &c,
        32,
    )
    .unwrap();
    assert_eq!(w.members[0].recovery_until, 32);
    assert_eq!(w.members[0].spirit, spirit);
    assert_eq!(w.shots[0].view.id, flight);
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 62, &mut vec![]);
    assert_eq!(w.members[0].spirit, spirit - 1);
    assert_eq!(w.shots[0].view.id, flight);
}
#[test]
fn recovery_policy_channel_opens_after_end_and_keeps_frozen_flags() {
    let (mut c, mut w) = super::channel_tests::setup(false);
    c.mechanism_version = 105;
    c.skill.recovery_ms = 300;
    c.skill.recovery_cancel =
        serde_json::from_value(json!({"after_ms":60,"by_move":true,"by_explicit_action":false}))
            .unwrap();
    super::channel_tests::start(&mut w, &c, 0);
    w.tick(&c, 60);
    c.skill.recovery_cancel = None;
    w.finish_channel(0, ChannelEndReason::Cancelled, 300, &mut vec![]);
    assert_eq!(w.members[0].recovery_until, 600);
    assert_eq!(w.members[0].recovery_cancel_at, Some(360));
    w.action(
        0,
        &Action::Move {
            direction: [1., 0.],
        },
        "too-soon",
        &c,
        330,
    )
    .unwrap();
    assert_eq!(w.members[0].recovery_until, 600);
    w.action(
        0,
        &Action::Move {
            direction: [1., 0.],
        },
        "open",
        &c,
        360,
    )
    .unwrap();
    assert_eq!(w.members[0].recovery_until, 360);
}

#[test]
fn recovery_policy_one_cancel_input_ends_active_channel_without_skipping_new_recovery() {
    let (mut c, mut w) = super::channel_tests::setup(false);
    c.mechanism_version = 105;
    c.skill.recovery_ms = 300;
    c.skill.recovery_cancel =
        serde_json::from_value(json!({"after_ms":0,"by_move":true,"by_explicit_action":true}))
            .unwrap();
    super::channel_tests::start(&mut w, &c, 0);
    w.tick(&c, 60);
    w.action(0, &Action::CancelAction {}, "cancel-channel", &c, 90)
        .unwrap();
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.members[0].recovery_until, 390);
    w.action(0, &Action::CancelAction {}, "cancel-recovery", &c, 91)
        .unwrap();
    assert_eq!(w.members[0].recovery_until, 91);
}
