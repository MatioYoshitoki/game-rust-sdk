use super::*;
use serde_json::{Value, json};
pub(super) fn setup(n: usize) -> (Config, World) {
    let (mut c, w) = super::combat_tests::setup(n);
    let cat = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/skill-hp-cost-host/catalog.json"),
    )
    .unwrap();
    let sample = cat.active("eight").unwrap();
    c.mechanism_version = 45;
    c.skill = sample.skill.clone();
    c.additional_skills = sample.additional_skills.clone();
    c.pve_mut().status_definitions.push(
        sample
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .last()
            .unwrap()
            .clone(),
    );
    c.members.as_mut().unwrap().area_capacity = Some(8);
    c.members.as_mut().unwrap().projectile_capacity = 16;
    (c, w)
}
fn definition(mode: &str, value: f64, minimum: u32) -> Value {
    json!({"mode":mode,"value":value,"min_hp_after_cost":minimum})
}
fn configure(c: &mut Config, slot: usize, value: Value) {
    let s = if slot == 0 {
        &mut c.skill
    } else {
        &mut c.additional_skills[slot - 1]
    };
    s.hp_cost = serde_json::from_value(value).unwrap();
}
fn prepare(w: &mut World, c: &Config, i: usize, slot: usize, now: u64) -> Result<Events> {
    let s = c.player_skills().nth(slot).unwrap();
    let a = if s.is_self() {
        Action::CastSelf {
            skill_id: s.id.clone(),
        }
    } else if s.is_unit() {
        Action::CastUnit {
            skill_id: s.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        }
    } else if s.is_ground() {
        Action::CastGround {
            skill_id: s.id.clone(),
            position: w.npcs[0].position,
        }
    } else {
        Action::Cast {
            skill_id: s.id.clone(),
            direction: [1., 0.],
        }
    };
    w.action(i, &a, "hp-private", c, now)
}
fn commit(w: &mut World, c: &Config, i: usize, now: u64) -> Events {
    let p = w.members[i].pending.take().unwrap();
    let mut events = vec![];
    w.commit(i, p, c, now, &mut events);
    events
}
fn actual(events: &Events, i: usize) -> u32 {
    let rows = events
        .iter()
        .filter_map(|(recipient, e)| match e {
            EventV4::Private {
                event:
                    Event::ActionCommitted {
                        hp_cost: Some(cost),
                        ..
                    },
            } => {
                assert_eq!(*recipient, Some(i));
                Some(*cost)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]
}
#[test]
fn hp_cost_original_three_vectors_execute_real_commits() {
    let vectors: Vec<Value> = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_hp_cost_vectors.json"
    ))
    .unwrap();
    assert_eq!(
        vectors
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["hp-ratio", "hp-min", "hp-clamped"]
    );
    for v in vectors {
        let (mut c, mut w) = setup(1);
        let input = &v["input"];
        let hp = input["hp"].as_u64().unwrap() as u32;
        configure(
            &mut c,
            0,
            definition(
                input["mode"].as_str().unwrap(),
                input["cost"].as_f64().unwrap(),
                input["minimum"].as_u64().unwrap() as u32,
            ),
        );
        w.members[0].hp = hp;
        prepare(&mut w, &c, 0, 0, 0).unwrap();
        assert_eq!(w.members[0].hp, hp);
        let events = commit(&mut w, &c, 0, c.skill.windup_ms);
        let expected = v["expected"].as_u64().unwrap() as u32;
        assert_eq!(actual(&events, 0), expected, "{}", v["id"]);
        assert_eq!(w.members[0].hp, hp - expected);
        assert!(w.members[0].hp >= 1);
    }
}
#[test]
fn hp_cost_floor_ratio_extremes_minimum_and_maximum_hp() {
    for (hp, mode, value, min, expected) in [
        (99, "fixed", 4.99, 1, 4),
        (99, "current_hp_ratio", 0., 1, 0),
        (99, "current_hp_ratio", 1., 1, 98),
        (1, "fixed", 100., 2, 0),
        (1000000, "fixed", 1000000., 1, 999999),
        (u32::MAX, "current_hp_ratio", 1., 1, u32::MAX - 1),
    ] {
        let (mut c, mut w) = setup(1);
        configure(&mut c, 0, definition(mode, value, min));
        w.members[0].hp = hp;
        prepare(&mut w, &c, 0, 0, 0).unwrap();
        let events = commit(&mut w, &c, 0, c.skill.windup_ms);
        assert_eq!(actual(&events, 0), expected);
        assert_eq!(w.members[0].hp, hp - expected);
    }
}
#[test]
fn hp_cost_all_seven_mechanisms_and_member_capacities_have_one_owner() {
    for n in [1, 2, 4, 8] {
        for slot in 0..8 {
            let (c, mut w) = setup(n);
            for i in 0..n {
                let hp = w.members[i].hp;
                let spirit = w.members[i].spirit;
                let s = c.player_skills().nth(slot).unwrap();
                prepare(&mut w, &c, i, slot, 0).unwrap();
                assert_eq!(w.members[i].hp, hp);
                assert_eq!(w.members[i].spirit, spirit);
                let e = commit(&mut w, &c, i, s.windup_ms);
                let cost = actual(&e, i);
                assert!(cost > 0);
                assert_eq!(w.members[i].hp, hp - cost);
                assert_eq!(w.members[i].spirit, spirit - s.cost);
                assert_eq!(w.members[i].cooldowns[&s.id], s.windup_ms + s.cooldown_ms);
                for j in i + 1..n {
                    assert_eq!(w.members[j].hp, w.members[j].facts.hp_points);
                    assert!(w.members[j].cooldowns.is_empty());
                }
            }
        }
    }
}
#[test]
fn hp_cost_ratio_uses_commit_hp_after_damage_or_heal() {
    for after in [10, 99, 1000] {
        let (mut c, mut w) = setup(1);
        configure(&mut c, 0, definition("current_hp_ratio", 0.05, 1));
        w.members[0].hp = 100;
        prepare(&mut w, &c, 0, 0, 0).unwrap();
        w.members[0].hp = after;
        let e = commit(&mut w, &c, 0, c.skill.windup_ms);
        assert_eq!(actual(&e, 0), (f64::from(after) * 0.05).floor() as u32);
    }
}
#[test]
fn hp_cost_definition_is_frozen_at_acceptance() {
    let (mut c, mut w) = setup(1);
    configure(&mut c, 0, definition("fixed", 7., 1));
    let hp = w.members[0].hp;
    prepare(&mut w, &c, 0, 0, 0).unwrap();
    configure(&mut c, 0, definition("fixed", 999999., 1));
    let e = commit(&mut w, &c, 0, c.skill.windup_ms);
    assert_eq!(actual(&e, 0), 7);
    assert_eq!(w.members[0].hp, hp - 7);
}
#[test]
fn hp_cost_cancel_protect_and_owner_exit_before_commit_are_free() {
    for mode in 0..3 {
        let (c, mut w) = setup(2);
        let hp = w.members[0].hp;
        let spirit = w.members[0].spirit;
        prepare(&mut w, &c, 0, 0, 0).unwrap();
        match mode {
            0 => {
                w.action(0, &Action::CancelAction {}, "cancel", &c, 1)
                    .unwrap();
            }
            1 => w.protect(0, 0, &mut vec![]),
            _ => w.terminal_intent(0, Outcome::Death, &mut vec![]),
        }
        assert!(w.members[0].pending.is_none());
        assert_eq!(w.members[0].hp, hp);
        assert_eq!(w.members[0].spirit, spirit);
        assert!(w.members[0].cooldowns.is_empty());
        assert!(w.shots.is_empty());
        assert!(w.members[1].eligible());
    }
}
#[test]
fn hp_cost_rejected_unit_targets_and_spirit_are_atomic() {
    for failure in 0..4 {
        let (c, mut w) = setup(1);
        let hp = w.members[0].hp;
        prepare(&mut w, &c, 0, 3, 0).unwrap();
        match failure {
            0 => w.npcs[0].hp = 0,
            1 => w.npcs[0].entity.generation += 1,
            2 => w.npcs[0].position = [10000., 10000.],
            _ => w.members[0].spirit = 0,
        }
        let spirit = w.members[0].spirit;
        let e = commit(&mut w, &c, 0, 100);
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected { .. }
            }
        )));
        assert_eq!(w.members[0].hp, hp);
        assert_eq!(w.members[0].spirit, spirit);
        assert!(w.members[0].cooldowns.is_empty());
        assert!(w.shots.is_empty());
        assert!(w.snapshot(0, &c, Phase::Active, 0).own.statuses.is_empty());
    }
}
#[test]
fn hp_cost_plan_capacity_failure_does_not_charge_or_grant_status() {
    let (mut c, mut w) = setup(2);
    c.members.as_mut().unwrap().projectile_capacity = 1;
    prepare(&mut w, &c, 0, 0, 0).unwrap();
    prepare(&mut w, &c, 1, 0, 0).unwrap();
    commit(&mut w, &c, 0, c.skill.windup_ms);
    let before = (w.members[1].hp, w.members[1].spirit);
    let e = commit(&mut w, &c, 1, c.skill.windup_ms);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ActionRejected {
                reason: Error::Capacity,
                ..
            }
        }
    )));
    assert_eq!((w.members[1].hp, w.members[1].spirit), before);
    assert!(w.members[1].cooldowns.is_empty());
    assert!(w.snapshot(1, &c, Phase::Active, 0).own.statuses.is_empty());
}
#[test]
fn hp_cost_ground_origin_and_area_capacity_failures_are_atomic() {
    for mode in 0..3 {
        let (mut c, mut w) = setup(2);
        c.members.as_mut().unwrap().area_capacity = Some(1);
        let slot = if mode == 1 { 0 } else { 4 };
        prepare(&mut w, &c, 1, slot, 0).unwrap();
        if mode == 0 {
            w.members[1].position = [10000., 10000.];
        } else if mode == 1 {
            w.members[1].position = [-1., -1.];
        } else {
            prepare(&mut w, &c, 0, 4, 0).unwrap();
            commit(&mut w, &c, 0, 100);
        }
        let before = (w.members[1].hp, w.members[1].spirit);
        let e = commit(&mut w, &c, 1, 100);
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected { .. }
            }
        )));
        assert_eq!((w.members[1].hp, w.members[1].spirit), before);
        assert!(w.members[1].cooldowns.is_empty());
    }
}
#[test]
fn hp_cost_bypasses_shields_damage_and_credit_but_grants_self_status() {
    let (mut c, mut w) = setup(1);
    configure(&mut c, 1, definition("fixed", 50., 1));
    let shield = c.additional_skills[0]
        .on_commit_self_statuses
        .iter()
        .find(|id| {
            c.pve
                .as_ref()
                .unwrap()
                .status_definitions
                .iter()
                .any(|d| d.id == **id && d.is_shield())
        })
        .unwrap()
        .clone();
    w.apply_statuses(
        0,
        (
            w.members[0].reference.entity.clone(),
            w.members[0].facts.hp_points,
        ),
        &[shield],
        &c,
        0,
        &mut vec![],
    );
    let old = w.snapshot(0, &c, Phase::Active, 0).own.statuses.clone();
    let hp = w.members[0].hp;
    prepare(&mut w, &c, 0, 1, 0).unwrap();
    let e = commit(&mut w, &c, 0, 100);
    assert_eq!(actual(&e, 0), 50);
    assert_eq!(w.members[0].hp, hp - 50);
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 0)
            .own
            .statuses
            .iter()
            .filter_map(|s| s.shield.as_ref().map(|v| v.remaining))
            .sum::<u32>(),
        old.iter()
            .filter_map(|s| s.shield.as_ref().map(|v| v.remaining))
            .sum::<u32>()
    );
    assert!(!e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::Damage { .. } | Event::ShieldAbsorbed { .. } | Event::PeriodicTick { .. }
        }
    )));
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusApplied { .. } | Event::StatusRefreshed { .. }
        }
    )));
}
#[test]
fn hp_cost_committed_flights_protection_and_cancel_never_refund_or_recharge() {
    let (c, mut w) = setup(2);
    let hp = w.members[0].hp;
    prepare(&mut w, &c, 0, 0, 0).unwrap();
    let e = commit(&mut w, &c, 0, c.skill.windup_ms);
    let cost = actual(&e, 0);
    w.action(0, &Action::CancelAction {}, "cancel", &c, 150)
        .unwrap();
    w.protect(0, 0, &mut vec![]);
    assert_eq!(w.members[0].hp, hp - cost);
    w.tick(&c, 180);
    assert!(w.members[0].hp <= hp - cost);
    assert_eq!(w.members[1].cooldowns.len(), 0);
}
#[test]
fn hp_cost_old_skills_and_basic_omit_cost_field() {
    let (mut c, mut w) = setup(1);
    c.skill.hp_cost = None;
    let hp = w.members[0].hp;
    prepare(&mut w, &c, 0, 0, 0).unwrap();
    let e = commit(&mut w, &c, 0, c.skill.windup_ms);
    let event = e
        .iter()
        .find_map(|(_, e)| match e {
            EventV4::Private {
                event: e @ Event::ActionCommitted { .. },
            } => Some(e),
            _ => None,
        })
        .unwrap();
    assert!(
        serde_json::to_value(event)
            .unwrap()
            .get("hp_cost")
            .is_none()
    );
    assert_eq!(w.members[0].hp, hp);
    w.members[0].recovery_until = 0;
    w.action(0, &Action::Attack {}, "basic", &c, 1000).unwrap();
    let e = commit(&mut w, &c, 0, 1100);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ActionCommitted {
                hp_cost: None,
                skill: false,
                ..
            }
        }
    )));
    assert_eq!(w.members[0].hp, hp);
}
#[test]
fn hp_cost_status_group_capacity_failure_is_atomic() {
    let (mut c, mut w) = setup(1);
    prepare(&mut w, &c, 0, 7, 0).unwrap();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 0;
    let before = (w.members[0].hp, w.members[0].spirit);
    let e = commit(&mut w, &c, 0, 120);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ActionRejected {
                reason: Error::Capacity,
                ..
            }
        }
    )));
    assert_eq!((w.members[0].hp, w.members[0].spirit), before);
    assert!(w.statuses.is_empty());
    assert!(w.members[0].cooldowns.is_empty());
}
