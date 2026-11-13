use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_periodic_overheal_shield_configuration_requires_member_hot_and_refresh_shield() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-periodic-overheal-shield-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    assert!(load(&doc).is_ok());
    let index = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    let mut old = doc.clone();
    old["mechanism_version"] = json!(75);
    assert!(load(&old).is_err());
    old["pve"]["status_definitions"][index]
        .as_object_mut()
        .unwrap()
        .remove("overheal_shield");
    assert!(load(&old).is_ok());

    for value in [
        json!(null),
        json!({"status_id":"overflow-shield","conversion_bps":0}),
        json!({"status_id":"overflow-shield","conversion_bps":10001}),
        json!({"status_id":"demo-guard","conversion_bps":5000}),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][index]["overheal_shield"] = value;
        assert!(load(&invalid).is_err());
    }
    let mut non_hot = doc.clone();
    non_hot["pve"]["status_definitions"][0]["overheal_shield"] =
        doc["pve"]["status_definitions"][index]["overheal_shield"].clone();
    assert!(load(&non_hot).is_err());
    let mut npc = doc;
    npc["pve"]["combat"]["projectile"]["on_commit_self_statuses"] = json!(["overflow-hot"]);
    assert!(load(&npc).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(2);
    c.mechanism_version = 76;
    c.pve_mut().status_definitions.extend([
        serde_json::from_value(json!({
            "id":"overflow-hot","mechanism":"periodic_heal","heal_bps":510,
            "tick_interval_ms":600,"duration_ms":2100,"duplicate_policy":"refresh","source_scoped":true,
            "overheal_shield":{"status_id":"overflow","conversion_bps":5000}
        })).unwrap(),
        serde_json::from_value(json!({
            "id":"overflow","mechanism":"absorption_shield","shield_bps":500,
            "duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap(),
    ]);
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 977;
    (c, w)
}

fn own_outcomes(events: &Events) -> Vec<(usize, &Event)> {
    events
        .iter()
        .filter_map(|(recipient, e)| match e {
            EventV4::Private {
                event:
                    e @ (Event::PeriodicHealTick { .. }
                    | Event::ShieldGranted { .. }
                    | Event::StatusRejected { .. }),
            } => Some((recipient.unwrap(), e)),
            _ => None,
        })
        .collect()
}

#[test]
fn member_periodic_overheal_shield_heals_first_and_only_refreshes_when_adding_balance() {
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({
        "id":"care","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":5000,
        "duration_ms":6000,"duplicate_policy":"refresh","source_scoped":true
    })).unwrap());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "care", 0);
    bind(&mut w, &c, Owner::Member(0), target, "overflow-hot", 0);
    let first = pulse(&mut w, &c, 600);
    assert!(matches!(
        own_outcomes(&first).as_slice(),
        [
            (
                0,
                Event::PeriodicHealTick {
                    raw_heal: 51,
                    hp_restored: 23,
                    ..
                }
            ),
            (
                0,
                Event::ShieldGranted {
                    requested: 26,
                    granted: 26,
                    remaining: 26,
                    ..
                }
            ),
        ]
    ));
    let second = pulse(&mut w, &c, 1200);
    assert!(matches!(
        own_outcomes(&second).as_slice(),
        [
            (0, Event::PeriodicHealTick { hp_restored: 0, .. }),
            (
                0,
                Event::ShieldGranted {
                    requested: 38,
                    granted: 24,
                    remaining: 50,
                    ..
                }
            ),
        ]
    ));
    let snapshot = w.snapshot(0, &c, Phase::Active, 1200).own;
    let shield = snapshot
        .statuses
        .iter()
        .find(|s| s.definition_id == "overflow")
        .unwrap();
    assert_eq!(shield.expires_at_ms, 4200);
    let capped = pulse(&mut w, &c, 1800);
    assert!(matches!(
        own_outcomes(&capped).as_slice(),
        [(0, Event::PeriodicHealTick { hp_restored: 0, .. })]
    ));
    let snapshot = w.snapshot(0, &c, Phase::Active, 1800).own;
    assert_eq!(
        snapshot
            .statuses
            .iter()
            .find(|s| s.definition_id == "overflow"),
        Some(shield)
    );
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 1800).own.shield_total,
        Some(0)
    );
    pulse(&mut w, &c, 4200);
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 4200).own.shield_total,
        Some(0)
    );
}

#[test]
fn member_periodic_overheal_shield_slot_failure_keeps_healing_and_protection_skips_conversion() {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "overflow-hot", 0);
    let first = pulse(&mut w, &c, 600);
    assert!(matches!(
        own_outcomes(&first).as_slice(),
        [
            (
                0,
                Event::PeriodicHealTick {
                    hp_restored: 23,
                    outcome: PeriodicTickOutcome::Applied,
                    ..
                }
            ),
            (
                0,
                Event::StatusRejected {
                    reason: Error::Capacity,
                    ..
                }
            ),
        ]
    ));
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 600).own.shield_total,
        Some(0)
    );
    w.protect(0, 900, &mut vec![]);
    let protected = pulse(&mut w, &c, 1200);
    assert!(matches!(
        own_outcomes(&protected).as_slice(),
        [(
            0,
            Event::PeriodicHealTick {
                hp_restored: 0,
                outcome: PeriodicTickOutcome::Protected,
                ..
            }
        )]
    ));
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 1200).own.shield_total,
        Some(0)
    );
}
