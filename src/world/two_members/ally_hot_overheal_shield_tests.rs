use super::ally_hot_tests::cast;
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_ally_hot_overheal_shield_configuration_requires_scoped_refresh_shield() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-ally-hot-overheal-shield-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    assert!(load(&doc).is_ok());
    let hot = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    let mut old = doc.clone();
    old["mechanism_version"] = json!(81);
    assert!(load(&old).is_err());
    old["pve"]["status_definitions"][hot]
        .as_object_mut()
        .unwrap()
        .remove("overheal_shield");
    assert!(load(&old).is_ok());
    for invalid in [
        json!(null),
        json!({"status_id":"overflow-shield","conversion_bps":0}),
        json!({"status_id":"overflow-shield","conversion_bps":10001}),
        json!({"status_id":"demo-guard","conversion_bps":5000}),
    ] {
        let mut candidate = doc.clone();
        candidate["pve"]["status_definitions"][hot]["overheal_shield"] = invalid;
        assert!(load(&candidate).is_err());
    }
    for (field, value) in [
        ("source_scoped", json!(false)),
        ("duplicate_policy", json!("ignore")),
    ] {
        let mut candidate = doc.clone();
        let definitions = candidate["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap();
        definitions
            .iter_mut()
            .find(|row| row["id"] == "overflow-shield")
            .unwrap()[field] = value;
        assert!(load(&candidate).is_err());
    }
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::ally_hot_tests::setup();
    c.mechanism_version = 82;
    let pve = c.pve_mut();
    pve.status_definitions
        .iter_mut()
        .find(|row| row.id == "ally-renewal")
        .unwrap()
        .overheal_shield =
        serde_json::from_value(json!({"status_id":"overflow","conversion_bps":5000})).unwrap();
    pve.status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"absorption_shield","id":"overflow","shield_bps":500,
            "duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    pve.status_rules.as_mut().unwrap().shield_cap_bps = Some(600);
    w.members[1].hp = 977;
    (c, w)
}

fn outcomes(events: &Events) -> Vec<(usize, &Event)> {
    events
        .iter()
        .filter_map(|(recipient, row)| match row {
            EventV4::Private {
                event:
                    event @ (Event::PeriodicHealTick { .. }
                    | Event::ShieldGranted { .. }
                    | Event::StatusRejected { .. }),
            } => Some((recipient.unwrap(), event)),
            _ => None,
        })
        .collect()
}

fn grant(events: &Events, caster: usize, actual_hp: u32, expected: (u32, u32, u32)) {
    let rows = outcomes(events);
    assert!(matches!(rows.as_slice(), [
        (from, Event::PeriodicHealTick {hp_restored,..}), (1, Event::PeriodicHealTick {..}),
        (from_grant, Event::ShieldGranted {requested,granted,remaining,..}),
        (1, Event::ShieldGranted {..}),
    ] if *from==caster && *from_grant==caster && *hp_restored==actual_hp
        && (*requested,*granted,*remaining)==expected));
    assert_eq!(rows[0].1, rows[1].1);
    assert_eq!(rows[2].1, rows[3].1);
}

#[test]
fn member_ally_hot_overheal_shield_heals_first_and_preserves_refresh_at_cap() {
    let (c, mut w) = setup();
    let target = w.members[1].reference.entity.clone();
    let source = status_entity(&w.members[0].reference.entity);
    cast(&mut w, &c, 0, 0);
    bind(&mut w, &c, Owner::Member(1), target.clone(), "care", 600);
    let first = pulse(&mut w, &c, 720);
    grant(&first, 0, 23, (26, 26, 26));
    assert!(
        matches!(outcomes(&first)[2].1,Event::ShieldGranted {source:s,target:t,..}
        if *s==source && *t==status_entity(&target))
    );
    assert_eq!(w.members[1].hp, 1000);
    let id = w
        .statuses
        .entries()
        .iter()
        .find(|row| row.definition_id == "overflow")
        .unwrap()
        .instance_id
        .clone();
    grant(&pulse(&mut w, &c, 1320), 0, 0, (38, 24, 50));
    cast(&mut w, &c, 2, 1300);
    let capped = pulse(&mut w, &c, 1920);
    assert!(matches!(
        outcomes(&capped).as_slice(),
        [
            (0, Event::PeriodicHealTick { hp_restored: 0, .. }),
            (1, Event::PeriodicHealTick { hp_restored: 0, .. }),
        ]
    ));
    grant(&pulse(&mut w, &c, 2020), 2, 0, (38, 10, 10));
    let old = w
        .statuses
        .entries()
        .iter()
        .find(|row| row.instance_id == id)
        .unwrap();
    assert_eq!(old.expires_at_ms, 4320);
    for (i, expected) in [(0, 0), (1, 60), (2, 0), (3, 0)] {
        assert_eq!(
            w.snapshot(i, &c, Phase::Active, 2020).own.shield_total,
            Some(expected)
        );
    }
}

#[test]
fn member_ally_hot_overheal_shield_capacity_keeps_healing_and_protection_skips() {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 2;
    cast(&mut w, &c, 0, 0);
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), target, "care", 600);
    let first = pulse(&mut w, &c, 720);
    assert!(matches!(outcomes(&first).as_slice(), [
        (0, Event::PeriodicHealTick {hp_restored:23,..}), (1,Event::PeriodicHealTick {hp_restored:23,..}),
        (0, Event::StatusRejected {source,target,reason:Error::Capacity,..}),
        (1, Event::StatusRejected {reason:Error::Capacity,..}),
    ] if *source==status_entity(&w.members[0].reference.entity) && *target==status_entity(&w.members[1].reference.entity)));
    assert_eq!(w.members[1].hp, 1000);
    w.protect(1, 1000, &mut vec![]);
    assert!(matches!(
        outcomes(&pulse(&mut w, &c, 1320)).as_slice(),
        [
            (
                0,
                Event::PeriodicHealTick {
                    hp_restored: 0,
                    outcome: PeriodicTickOutcome::Protected,
                    ..
                }
            ),
            (
                1,
                Event::PeriodicHealTick {
                    hp_restored: 0,
                    outcome: PeriodicTickOutcome::Protected,
                    ..
                }
            ),
        ]
    ));
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 1320).own.shield_total,
        Some(0)
    );
}

#[test]
fn member_ally_hot_overheal_shield_exit_clears_hot_and_shield_for_that_source() {
    let (c, mut w) = setup();
    cast(&mut w, &c, 0, 0);
    cast(&mut w, &c, 2, 30);
    grant(&pulse(&mut w, &c, 720), 0, 23, (14, 14, 14));
    grant(&pulse(&mut w, &c, 750), 2, 0, (25, 25, 25));
    let instances = |w: &World, i: usize| {
        let source = status_entity(&w.members[i].reference.entity);
        let mut ids = w
            .statuses
            .entries()
            .iter()
            .filter(|row| row.source == source)
            .map(|row| row.instance_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };
    let first = instances(&w, 0);
    let second = instances(&w, 2);
    let sees = |events: &Events, observer, ids: &[String], expected| {
        events.iter().any(|(recipient, row)| {
            *recipient == Some(observer)
                && matches!(row,EventV4::Private {
            event:Event::StatusRemoved {instance_ids,reason}
        } if *reason==expected && instance_ids==ids)
        })
    };
    let left = w.action(0, &Action::Leave {}, "leave", &c, 900).unwrap();
    assert!(sees(&left, 1, &first, StatusRemovalReason::SourceInvalid));
    assert_eq!(instances(&w, 2), second);
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 900).own.shield_total,
        Some(25)
    );
    let left = w.action(1, &Action::Leave {}, "leave", &c, 1000).unwrap();
    assert!(sees(&left, 2, &second, StatusRemovalReason::TargetInvalid));
    assert!(w.statuses.is_empty());
    assert!(outcomes(&pulse(&mut w, &c, 1800)).is_empty());
}
