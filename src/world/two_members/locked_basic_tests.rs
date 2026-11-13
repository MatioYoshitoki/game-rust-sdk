use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::hp_cost_tests::setup(2);
    c.mechanism_version = 54;
    c.basic_followup = serde_json::from_value(json!({"grace_ms":150})).unwrap();
    c.basic.windup_ms = 30;
    c.basic.interval_ms = 180;
    c.basic.range = 240.;
    w.npcs.truncate(2);
    let [x, y] = w.members[0].position;
    w.npcs[0].position = [x + 45., y];
    w.npcs[1].position = [x, y + 90.];
    (c, w)
}

fn aim(w: &World, target: usize) -> Action {
    Action::AttackUnit {
        target: World::unit_wire(&w.npcs[target].entity),
    }
}

fn input(w: &mut World, c: &Config, action: Action, id: &str, now: u64) -> Result<Events> {
    w.action(0, &action, id, c, now)
}

fn commit(w: &mut World, c: &Config, now: u64) -> Events {
    let pending = w.members[0].pending.take().unwrap();
    let mut events = vec![];
    w.commit(0, pending, c, now, &mut events);
    events
}

#[test]
fn locked_basic_uses_selected_non_nearest_enemy_and_actual_commit_aim() {
    let (c, mut w) = setup();
    let action = aim(&w, 1);
    let target = World::unit_wire(&w.npcs[1].entity);
    input(&mut w, &c, action, "chosen", 0).unwrap();
    let projected = w
        .snapshot(0, &c, Phase::Active, 0)
        .own
        .pending_action
        .unwrap();
    assert_eq!(projected.unit_target.as_ref(), Some(&target));
    assert!(projected.skill_id.is_none());
    let [x, y] = w.members[0].position;
    w.npcs[1].position = [x, y + 65.];
    let hps = [w.npcs[0].hp, w.npcs[1].hp];
    let events = commit(&mut w, &c, 30);
    assert!(events.iter().any(|(recipient,event)| *recipient == Some(0) && matches!(event,
        EventV4::Private { event: Event::ActionCommitted { action_id, unit_target: Some(t), skill: false, .. } }
        if action_id == "chosen" && t == &target)));
    assert_eq!(w.members[0].basic_ready, 210);
    assert_eq!(w.shots.len(), 1);
    assert_eq!(w.shots[0].direction, [0., 1.]);
    assert!(w.shots[0].view.unit_target.is_none());
    w.advance_shots(&c, 60, 1., &mut vec![]);
    assert_eq!(w.npcs[0].hp, hps[0]);
    assert!(w.npcs[1].hp < hps[1]);
}

#[test]
fn locked_basic_drops_stale_pending_projection_and_rejects_without_fallback() {
    let (c, mut w) = setup();
    let action = aim(&w, 1);
    input(&mut w, &c, action.clone(), "chosen", 0).unwrap();
    w.npcs[1].entity.generation += 1;
    assert!(
        w.snapshot(0, &c, Phase::Active, 0)
            .own
            .pending_action
            .unwrap()
            .unit_target
            .is_none()
    );
    let events = commit(&mut w, &c, 30);
    assert!(events.iter().any(|(recipient,event)| *recipient == Some(0) && matches!(event,
        EventV4::Private { event: Event::ActionRejected { action_id, reason: Error::InvalidTarget } }
        if action_id == "chosen")));
    assert!(w.shots.is_empty());
    assert_eq!(w.members[0].basic_ready, 0);
    assert_eq!(
        input(&mut w, &c, action, "stale", 30),
        Err(Error::InvalidTarget)
    );
}

#[test]
fn locked_basic_queue_replaces_with_latest_valid_target_and_keeps_old_on_rejection() {
    let (c, mut w) = setup();
    input(&mut w, &c, Action::Attack {}, "opening", 0).unwrap();
    commit(&mut w, &c, 30);
    let near = aim(&w, 0);
    let far = aim(&w, 1);
    input(&mut w, &c, near, "near", 30).unwrap();
    input(&mut w, &c, far.clone(), "far", 60).unwrap();
    let queue = w.members[0].queued_basic.clone().unwrap();
    assert_eq!(queue.action_id, "far");
    let out = w.npcs[1].position;
    w.npcs[1].position[1] += 300.;
    assert_eq!(
        input(&mut w, &c, far, "unreachable", 60),
        Err(Error::InvalidTarget)
    );
    assert_eq!(w.members[0].queued_basic.as_ref(), Some(&queue));
    w.npcs[1].position = out;
    w.promote_basic(0, &c, 210, &mut vec![]);
    assert_eq!(
        w.members[0]
            .pending
            .as_ref()
            .unwrap()
            .target
            .as_ref()
            .unwrap(),
        &w.npcs[1].entity
    );
    commit(&mut w, &c, 240);
    assert_eq!(w.shots.len(), 2);
    assert_eq!(w.shots[1].direction, [0., 1.]);
    assert_eq!(w.members[0].basic_ready, 420);
}
