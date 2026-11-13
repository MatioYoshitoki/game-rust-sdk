use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_instant_self_heal_config_requires_self_skill_and_valid_amount() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-instant-self-heal-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    let mut config = doc.clone();
    let catalog = load(&config).unwrap();
    let skill = catalog
        .default_config()
        .player_skill("self-resolve")
        .unwrap()
        .clone();
    assert_eq!(
        serde_json::to_value(skill).unwrap(),
        config["additional_skills"][0]
    );

    config["mechanism_version"] = json!(76);
    assert!(load(&config).is_err());
    config = doc.clone();
    config["skill"]["self_heal"] = json!({"amount":100});
    assert!(load(&config).is_err());
    config = doc.clone();
    config["additional_skills"][0]["on_commit_self_statuses"] = json!(["demo-guard"]);
    assert!(load(&config).is_err());
    for invalid in [
        json!(null),
        json!({}),
        json!({"amount": 100, "max_hp_bps": 100}),
        json!({"amount": 0}),
        json!({"amount": null, "max_hp_bps": 100}),
        json!({"amount": 1000001}),
        json!({"max_hp_bps": 10001}),
        json!({"amount": 100, "overheal_shield":{"status_id":"demo-guard","conversion_bps":5000}}),
        json!({"amount": 100, "overheal_shield":{"status_id":"overflow-shield","conversion_bps":0}}),
    ] {
        config = doc.clone();
        config["additional_skills"][0]["self_heal"] = invalid;
        assert!(load(&config).is_err());
    }
    config = doc;
    config["additional_skills"][0]["self_heal"] = json!({"max_hp_bps": 2500});
    assert!(load(&config).is_ok());
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(2);
    c.mechanism_version = 77;
    c.additional_skills.push(serde_json::from_value(json!({
        "mechanism":"self_status", "id":"heal-now", "cost":2,
        "windup_ms":120, "recovery_ms":200, "cooldown_ms":5000,
        "hp_cost":{"mode":"fixed","value":23,"min_hp_after_cost":1},
        "self_heal":{"amount":51,"overheal_shield":{"status_id":"overflow","conversion_bps":5000}}
    })).unwrap());
    c.pve_mut().status_definitions.extend([
        serde_json::from_value(json!({
            "id":"overflow","mechanism":"absorption_shield","shield_bps":1000,
            "duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
        serde_json::from_value(json!({
            "id":"care","mechanism":"timed_attribute_modifier","attribute":"healing_received",
            "modifier_bps":5000,"duration_ms":6000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    ]);
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 1000;
    (c, w)
}

fn prepare(w: &mut World, c: &Config, now: u64) -> Result<Events> {
    w.action(
        0,
        &Action::CastSelf {
            skill_id: "heal-now".into(),
        },
        "cast",
        c,
        now,
    )
}

fn commit(w: &mut World, c: &Config) -> Events {
    let pending = w.members[0].pending.take().unwrap();
    let mut events = vec![];
    w.commit(0, pending, c, 120, &mut events);
    events
}

fn relevant(events: &Events) -> Vec<(Option<usize>, &Event)> {
    events
        .iter()
        .filter_map(|(recipient, envelope)| match envelope {
            EventV4::Private {
                event:
                    event @ (Event::ActionCommitted { .. }
                    | Event::ActionRejected { .. }
                    | Event::HealResolved { .. }
                    | Event::ShieldGranted { .. }
                    | Event::StatusRejected { .. }),
            } => Some((*recipient, event)),
            _ => None,
        })
        .collect()
}

#[test]
fn member_instant_self_heal_cost_precedes_healing_and_only_excess_becomes_shield() {
    let (c, mut w) = setup();
    let spirit = w.members[0].spirit;
    assert!(relevant(&prepare(&mut w, &c, 0).unwrap()).is_empty());
    assert_eq!(w.members[0].spirit, spirit);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "care", 60);
    let events = commit(&mut w, &c);
    assert!(matches!(relevant(&events).as_slice(), [
        (Some(0), Event::ActionCommitted { hp_cost: Some(23), spirit_cost: 2, .. }),
        (Some(0), Event::HealResolved {
            heal_kind: HealKind::SelfSkill, source, target: healed,
            raw_heal: 51, multiplier_bps: 15000, adjusted_heal: 76, hp_restored: 23,
        }),
        (Some(0), Event::ShieldGranted { requested: 26, granted: 26, remaining: 26, .. }),
    ] if *source == status_entity(&target) && source == healed));
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.members[0].spirit, spirit - 2);
    assert_eq!(w.members[0].cooldowns["heal-now"], 5120);
    assert!(prepare(&mut w, &c, 400).is_err());
    assert!(w.shots.is_empty());
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 120).own.shield_total,
        Some(0)
    );
}

#[test]
fn member_instant_self_heal_no_shield_or_failed_commit_has_no_extra_effect() {
    let (mut c, mut w) = setup();
    let skill = c
        .additional_skills
        .iter_mut()
        .find(|s| s.id == "heal-now")
        .unwrap();
    skill.hp_cost = None;
    skill.instant_heal = serde_json::from_value(json!({"max_hp_bps":2511})).unwrap();
    prepare(&mut w, &c, 0).unwrap();
    w.action(0, &Action::CancelAction {}, "cancel", &c, 1)
        .unwrap();
    assert!(w.members[0].pending.is_none());
    assert!(w.members[0].cooldowns.is_empty());
    prepare(&mut w, &c, 5).unwrap();
    w.members[0].spirit = 0;
    assert!(matches!(
        relevant(&commit(&mut w, &c)).as_slice(),
        [(
            Some(0),
            Event::ActionRejected {
                reason: Error::NoResource,
                ..
            }
        ),]
    ));
    assert!(w.members[0].cooldowns.is_empty());

    w.members[0].spirit = 10;
    prepare(&mut w, &c, 10).unwrap();
    w.protect(0, 50, &mut vec![]);
    assert!(w.members[0].pending.is_none());
    assert!(w.members[0].cooldowns.is_empty());

    let (_, mut leaving) = setup();
    prepare(&mut leaving, &c, 0).unwrap();
    let mut events = vec![];
    leaving.terminal_intent(0, Outcome::Death, &mut events);
    assert!(relevant(&events).is_empty());
    assert!(leaving.members[0].pending.is_none());
    assert!(leaving.members[0].cooldowns.is_empty());

    let (_, mut w) = setup();
    prepare(&mut w, &c, 0).unwrap();
    assert!(matches!(
        relevant(&commit(&mut w, &c)).as_slice(),
        [
            (Some(0), Event::ActionCommitted { .. }),
            (
                Some(0),
                Event::HealResolved {
                    raw_heal: 251,
                    hp_restored: 0,
                    ..
                }
            ),
        ]
    ));
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 120).own.shield_total,
        Some(0)
    );
}

#[test]
fn member_instant_self_heal_capacity_decline_keeps_health_and_skill_commit() {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "care", 0);
    prepare(&mut w, &c, 0).unwrap();
    assert!(matches!(
        relevant(&commit(&mut w, &c)).as_slice(),
        [
            (
                Some(0),
                Event::ActionCommitted {
                    hp_cost: Some(23),
                    ..
                }
            ),
            (
                Some(0),
                Event::HealResolved {
                    hp_restored: 23,
                    ..
                }
            ),
            (
                Some(0),
                Event::StatusRejected {
                    reason: Error::Capacity,
                    ..
                }
            ),
        ]
    ));
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.members[0].cooldowns["heal-now"], 5120);
}
