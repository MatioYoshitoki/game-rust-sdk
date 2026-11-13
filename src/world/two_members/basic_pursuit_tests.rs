use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::hp_cost_tests::setup(2);
    c.mechanism_version = 55;
    c.basic_pursuit = serde_json::from_value(json!({"max_range":300,"duration_ms":1500})).unwrap();
    c.basic.range = 70.;
    c.basic.windup_ms = 60;
    c.basic.interval_ms = 240;
    w.npcs.truncate(1);
    let [x, y] = w.members[0].position;
    w.npcs[0].position = [x + 135., y];
    (c, w)
}

fn start(w: &mut World, c: &Config) -> Result<Events> {
    let action = Action::AttackUnit {
        target: World::unit_wire(&w.npcs[0].entity),
    };
    w.action(0, &action, "pursuit", c, 0)
}

fn rejected(events: &Events, expected: Error) {
    assert!(
        events.iter().any(|(recipient, event)| *recipient == Some(0)
            && matches!(event,
        EventV4::Private {event:Event::ActionRejected {action_id,reason}}
        if action_id=="pursuit" && *reason==expected)),
        "{events:?}"
    );
}

#[test]
fn basic_pursuit_moves_to_same_target_and_commits_once_after_locked_windup() {
    let (c, mut w) = setup();
    let origin = w.members[0].position;
    let target = w.npcs[0].entity.clone();
    let hp = w.members[0].hp;
    let spirit = w.members[0].spirit;
    start(&mut w, &c).unwrap();
    assert!(w.members[0].pending.is_none());
    assert_eq!(w.members[0].basic_ready, 0);
    w.action(
        0,
        &Action::Move {
            direction: [0., -1.],
        },
        "move",
        &c,
        0,
    )
    .unwrap();
    let mut now = 30;
    while w.members[0].pending.is_none() && now < 1500 {
        let mut events = vec![];
        w.advance_basic_pursuit(0, &c, now, 0.03, &mut events);
        assert!(events.is_empty());
        now += 30;
    }
    let entered = w.members[0].position;
    assert!(entered[0] > origin[0]);
    assert_eq!(entered[1], origin[1]);
    assert!(distance(entered, w.npcs[0].position) <= c.basic.range);
    let pending = w.members[0].pending.as_ref().unwrap();
    assert_eq!(pending.id, "pursuit");
    assert_eq!(pending.target.as_ref(), Some(&target));
    assert_eq!(pending.commits, now + 30);
    assert!(pending.pursuit_windup);
    assert!(w.shots.is_empty());
    let deadline = pending.commits;
    w.npcs[0].next_attack = 100_000;
    w.npcs[0].next_cast = 100_000;
    w.tick(&c, now);
    assert_eq!(w.members[0].position, entered);
    let events = w.tick(&c, deadline);
    assert_eq!(w.members[0].position, entered);
    assert!(events.iter().any(|(recipient,event)|*recipient==Some(0) && matches!(event,
        EventV4::Private {event:Event::ActionCommitted{action_id,unit_target:Some(t),skill:false,spirit_cost:0,hp_cost:None,..}}
        if action_id=="pursuit" && t==&World::unit_wire(&target))));
    assert_eq!(w.members[0].basic_ready, deadline + c.basic.interval_ms);
    assert_eq!(w.members[0].hp, hp);
    assert_eq!(w.members[0].spirit, spirit);
    assert_eq!(w.shots.len(), 1);
    let before = w.npcs[0].hp;
    w.advance_shots(&c, deadline + 30, 1., &mut vec![]);
    assert!(w.npcs[0].hp < before);
    w.tick(&c, deadline + 30);
    assert!(w.members[0].position[1] < entered[1]);
}

#[test]
fn basic_pursuit_rejects_blockage_stale_identity_range_and_deadline_without_commit() {
    for (mode, reason) in [
        (0, Error::InvalidTarget),
        (1, Error::InvalidTarget),
        (2, Error::InvalidTarget),
        (3, Error::OutOfRange),
        (4, Error::InvalidState),
    ] {
        let (c, mut w) = setup();
        let original = w.members[0].position;
        start(&mut w, &c).unwrap();
        match mode {
            0 => w.npcs[0].entity.generation += 1,
            1 => Arc::make_mut(&mut w.grid.config).palette[0].walkable = false,
            2 => Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true,
            3 => w.npcs[0].position[0] += 400.,
            _ => w.npcs[0].position = [original[0] + 50., original[1]],
        }
        let mut events = vec![];
        w.advance_basic_pursuit(0, &c, if mode == 4 { 1500 } else { 30 }, 0.03, &mut events);
        rejected(&events, reason);
        assert!(w.members[0].basic_pursuit.is_none() && w.members[0].pending.is_none());
        assert_eq!(w.members[0].position, original);
        assert_eq!(w.members[0].basic_ready, 0);
        assert!(w.shots.is_empty());
    }
    let (c, mut w) = setup();
    Arc::make_mut(&mut w.grid.config).palette[0].walkable = false;
    assert_eq!(start(&mut w, &c), Err(Error::InvalidTarget));
    assert!(w.members[0].basic_pursuit.is_none());
}

#[test]
fn basic_pursuit_keeps_stop_and_cancellation_separate_and_blocks_competing_actions() {
    let (c, mut w) = setup();
    assert_eq!(
        w.action(0, &Action::Attack {}, "auto", &c, 0),
        Err(Error::InvalidTarget)
    );
    w.members[0].basic_ready = 30;
    assert_eq!(start(&mut w, &c), Err(Error::InvalidState));
    w.members[0].basic_ready = 0;
    start(&mut w, &c).unwrap();
    assert_eq!(start(&mut w, &c), Err(Error::InvalidState));
    assert_eq!(
        w.action(0, &Action::ExtractStart {}, "extract", &c, 0),
        Err(Error::InvalidState)
    );
    w.action(0, &Action::Stop {}, "stop", &c, 0).unwrap();
    assert!(w.members[0].basic_pursuit.is_some());
    let events = w
        .action(0, &Action::CancelAction {}, "cancel", &c, 0)
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
    assert!(w.members[0].basic_pursuit.is_none());
    assert!(w.shots.is_empty());
    assert_eq!(w.members[0].basic_ready, 0);
}

fn auto_setup() -> (Config, World) {
    let (mut c, mut w) = super::hp_cost_tests::setup(2);
    c.mechanism_version = 56;
    c.basic.range = 70.;
    c.basic.windup_ms = 60;
    c.basic.interval_ms = 240;
    c.basic_pursuit =
        serde_json::from_value(json!({"max_range":300,"auto_max_range":175,"duration_ms":1500}))
            .unwrap();
    c.basic_followup = serde_json::from_value(json!({"grace_ms":150})).unwrap();
    w.npcs.truncate(2);
    let [x, y] = w.members[0].position;
    w.npcs[0].position = [x + 135., y];
    w.npcs[1].position = [x, y + 135.];
    (c, w)
}

fn auto_attack(w: &mut World, c: &Config, now: u64) -> Result<Events> {
    w.action(0, &Action::Attack {}, "pursuit", c, now)
}

#[test]
fn auto_basic_pursuit_preserves_in_range_and_cooldown_queue() {
    let (c, mut w) = auto_setup();
    let [x, y] = w.members[0].position;
    w.npcs[1].position = [x, y + c.basic.range];
    auto_attack(&mut w, &c, 0).unwrap();
    assert!(w.members[0].basic_pursuit.is_none());
    assert_eq!(w.members[0].pending.as_ref().unwrap().direction, [0., 1.]);
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 60, &mut vec![]);
    auto_attack(&mut w, &c, 90).unwrap();
    let queued = w.members[0].queued_basic.clone().unwrap();
    assert_eq!(queued.target, World::unit_wire(&w.npcs[1].entity));
    w.npcs[1].position[1] += 50.;
    assert_eq!(auto_attack(&mut w, &c, 120), Err(Error::InvalidState));
    assert_eq!(w.members[0].queued_basic.as_ref(), Some(&queued));
    assert!(w.members[0].basic_pursuit.is_none());
}

#[test]
fn auto_basic_pursuit_orders_ties_and_never_retargets_before_hit() {
    let (c, mut w) = auto_setup();
    let [x, y] = w.members[0].position;
    w.npcs[0].entity.id = "z".into();
    w.npcs[1].entity.id = "a".into();
    let target = w.npcs[1].entity.clone();
    auto_attack(&mut w, &c, 0).unwrap();
    assert_eq!(
        w.members[0].basic_pursuit.as_ref().unwrap().view.target,
        World::unit_wire(&target)
    );
    w.npcs[0].position = [x + 85., y];
    let mut events = vec![];
    let mut now = 30;
    while w.members[0].basic_pursuit.is_some() && now < 1500 {
        w.advance_basic_pursuit(0, &c, now, 0.03, &mut events);
        now += 30;
    }
    assert!(events.is_empty());
    let pending = w.members[0].pending.take().unwrap();
    assert_eq!(pending.id, "pursuit");
    assert_eq!(pending.target.as_ref(), Some(&target));
    let committed = pending.commits;
    w.commit(0, pending, &c, committed, &mut events);
    assert_eq!(w.shots.len(), 1);
    assert_eq!(w.members[0].basic_ready, committed + c.basic.interval_ms);
    let before = [w.npcs[0].hp, w.npcs[1].hp];
    w.advance_shots(&c, committed + 30, 1., &mut events);
    assert_eq!(w.npcs[0].hp, before[0]);
    assert!(w.npcs[1].hp < before[1]);
}

#[test]
fn auto_basic_pursuit_uses_its_own_boundary_and_shared_failure_cleanup() {
    let (c, mut w) = auto_setup();
    let [x, y] = w.members[0].position;
    w.npcs[0].position = [x + 175., y];
    w.npcs[1].hp = 0;
    auto_attack(&mut w, &c, 0).unwrap();
    w.npcs[0].position[0] += 1.;
    let mut events = vec![];
    w.advance_basic_pursuit(0, &c, 30, 0.03, &mut events);
    rejected(&events, Error::OutOfRange);
    assert_eq!(auto_attack(&mut w, &c, 30), Err(Error::InvalidTarget));
    start(&mut w, &c).unwrap();
    w.advance_basic_pursuit(0, &c, 30, 0.03, &mut vec![]);
    assert!(w.members[0].basic_pursuit.is_some());

    for (mode, reason) in [
        (0, Error::InvalidTarget),
        (1, Error::InvalidTarget),
        (2, Error::InvalidTarget),
        (3, Error::InvalidState),
    ] {
        let (c, mut w) = auto_setup();
        let origin = w.members[0].position;
        let hp = w.members[0].hp;
        let spirit = w.members[0].spirit;
        auto_attack(&mut w, &c, 0).unwrap();
        let target = w.members[0]
            .basic_pursuit
            .as_ref()
            .unwrap()
            .view
            .target
            .clone();
        let npc = w
            .npcs
            .iter_mut()
            .find(|n| n.entity.id == target.id)
            .unwrap();
        match mode {
            0 => npc.entity.generation += 1,
            1 => Arc::make_mut(&mut w.grid.config).palette[0].walkable = false,
            2 => Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true,
            _ => {}
        }
        let mut events = vec![];
        w.advance_basic_pursuit(0, &c, if mode == 3 { 1500 } else { 30 }, 0.03, &mut events);
        rejected(&events, reason);
        let member = &w.members[0];
        assert!(member.basic_pursuit.is_none() && member.pending.is_none());
        assert_eq!(member.position, origin);
        assert_eq!(
            (member.hp, member.spirit, member.basic_ready),
            (hp, spirit, 0)
        );
        assert!(w.shots.is_empty());
    }
}
