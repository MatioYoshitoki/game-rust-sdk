use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_ally_heal_config_checks_version_range_and_effect_fields() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-ally-heal-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    let skill = doc["additional_skills"].as_array().unwrap().last().unwrap();
    let catalog = load(&doc).unwrap();
    let config = catalog.default_config();
    assert_eq!(
        serde_json::to_value(config.player_skill("ally-restore").unwrap()).unwrap(),
        *skill
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(77);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("range", json!(0)),
        ("range", json!(10001)),
        ("heal", json!({"amount":0})),
        ("heal", json!({"max_hp_bps":10001})),
        ("heal", json!({"amount":1,"max_hp_bps":1})),
        (
            "heal",
            json!({"amount":1,"overheal_shield":{"status_id":"overflow-shield","conversion_bps":5000}}),
        ),
        ("on_commit_self_statuses", json!(["demo-guard"])),
        ("on_hit_target_statuses", json!([])),
    ] {
        let mut invalid = doc.clone();
        invalid["additional_skills"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()[field] = value;
        assert!(load(&invalid).is_err(), "{field}");
    }
    let mut primary = doc;
    let slot = primary["additional_skills"]
        .as_array_mut()
        .unwrap()
        .pop()
        .unwrap();
    primary["skill"] = slot;
    primary["skill"]["heal"] = json!({"max_hp_bps":3000});
    assert!(load(&primary).is_ok());
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(3);
    c.mechanism_version = 78;
    c.additional_skills.push(
        serde_json::from_value(json!({
            "mechanism":"ally_heal","id":"ally-restore","cost":2,"windup_ms":120,
            "recovery_ms":240,"cooldown_ms":2000,"range":100,
            "hp_cost":{"mode":"fixed","value":10,"min_hp_after_cost":1},
            "heal":{"max_hp_bps":301}
        }))
        .unwrap(),
    );
    w.members[0].position = [400., 400.];
    w.members[1].position = [400., 440.];
    w.members[0].facts.hp_points = 500;
    w.members[0].hp = 500;
    w.members[1].facts.hp_points = 1700;
    w.members[1].hp = 1600;
    (c, w)
}

fn cast(w: &mut World, c: &Config, target: UnitTarget) -> Result<Events> {
    w.action(
        0,
        &Action::CastUnit {
            skill_id: "ally-restore".into(),
            target,
        },
        "heal",
        c,
        0,
    )
}

fn prepare(w: &mut World, c: &Config) {
    let target = World::unit_wire(&w.members[1].reference.entity);
    assert!(cast(w, c, target).unwrap().is_empty());
}

fn commit(w: &mut World, c: &Config) -> Events {
    let pending = w.members[0].pending.take().unwrap();
    let mut events = vec![];
    w.commit(0, pending, c, 120, &mut events);
    events
}

#[test]
fn member_ally_heal_accepts_only_an_explicit_eligible_friendly_identity() {
    let (c, mut w) = setup();
    let target = World::unit_wire(&w.members[1].reference.entity);
    let stale = UnitTarget {
        generation: target.generation + 1,
        ..target.clone()
    };
    for invalid in [
        stale,
        World::unit_wire(&w.members[0].reference.entity),
        World::unit_wire(&w.npcs[0].entity),
    ] {
        assert_eq!(cast(&mut w, &c, invalid), Err(Error::InvalidTarget));
    }
    for action in [
        Action::CastSelf {
            skill_id: "ally-restore".into(),
        },
        Action::Cast {
            skill_id: "ally-restore".into(),
            direction: [1., 0.],
        },
    ] {
        assert_eq!(
            w.action(0, &action, "invalid", &c, 0),
            Err(Error::InvalidTarget)
        );
    }
    w.members[1].ready = false;
    assert_eq!(cast(&mut w, &c, target.clone()), Err(Error::InvalidTarget));
    w.members[1].ready = true;
    w.members[1].position = [400., 550.];
    assert_eq!(cast(&mut w, &c, target.clone()), Err(Error::OutOfRange));
    w.members[1].position = [400., 440.];
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
    assert_eq!(cast(&mut w, &c, target), Err(Error::OutOfRange));
    assert!(w.members[0].pending.is_none());
}

#[test]
fn member_ally_heal_commit_revalidates_target_without_cost_or_retargeting() {
    for case in 0..5 {
        let (c, mut w) = setup();
        prepare(&mut w, &c);
        let spirit = w.members[0].spirit;
        let reason = match case {
            0 => {
                w.members[1].reference.entity.generation += 1;
                Error::InvalidTarget
            }
            1 => {
                w.protect(1, 100, &mut vec![]);
                Error::InvalidTarget
            }
            2 => {
                w.terminal_intent(1, Outcome::Death, &mut vec![]);
                Error::InvalidTarget
            }
            3 => {
                w.members[1].position = [400., 550.];
                Error::OutOfRange
            }
            _ => {
                Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
                Error::OutOfRange
            }
        };
        let peer_hp = w.members[2].hp;
        assert_eq!(
            commit(&mut w, &c),
            vec![(
                Some(0),
                EventV4::Private {
                    event: Event::ActionRejected {
                        action_id: "heal".into(),
                        reason
                    },
                }
            )]
        );
        assert_eq!(w.members[0].spirit, spirit);
        assert_eq!(w.members[0].hp, 500);
        assert_eq!(w.members[2].hp, peer_hp);
        assert!(w.members[0].cooldowns.is_empty());
        assert!(w.shots.is_empty());
    }
}

#[test]
fn member_ally_heal_uses_target_max_and_live_multiplier_with_private_results() {
    for (hp, restored) in [(1600, 76), (1680, 20), (1700, 0)] {
        let (mut c, mut w) = setup();
        let spirit = w.members[0].spirit;
        w.members[1].hp = hp;
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({
            "id":"care","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":5000,
            "duration_ms":6000,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
        let target = w.members[1].reference.entity.clone();
        prepare(&mut w, &c);
        let pending = w
            .snapshot(0, &c, Phase::Active, 0)
            .own
            .pending_action
            .unwrap();
        assert_eq!(pending.ally_target, Some(World::unit_wire(&target)));
        assert_eq!(pending.unit_target, None);
        bind(&mut w, &c, Owner::Member(1), target.clone(), "care", 60);
        let events = commit(&mut w, &c);
        assert!(matches!(events.as_slice(), [
            (Some(0), EventV4::Private {event: Event::ActionCommitted {
                unit_target: Some(unit), spirit_cost:2, hp_cost:Some(10), ..
            }}),
            (Some(0), EventV4::Private {event: first}),
            (Some(1), EventV4::Private {event: second}),
        ] if *unit == World::unit_wire(&target) && first == second
            && *first == (Event::HealResolved {
                source:status_entity(&w.members[0].reference.entity), target:status_entity(&target),
                heal_kind:HealKind::AllySkill, raw_heal:51,multiplier_bps:15000,adjusted_heal:76,hp_restored:restored,
            })));
        assert_eq!(w.members[0].spirit, spirit - 2);
        assert_eq!(w.members[0].hp, 490);
        assert_eq!(w.members[1].hp, hp + restored);
        assert_eq!(w.members[0].cooldowns["ally-restore"], 2120);
        assert!(w.members[1].cooldowns.is_empty());
        assert!(w.shots.is_empty());
    }
}
