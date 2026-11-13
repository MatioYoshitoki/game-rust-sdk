use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_ally_overheal_shield_config_requires_version_and_source_scoped_refresh() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-ally-overheal-shield-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let slot = doc["additional_skills"].as_array().unwrap().len() - 1;
    let catalog = load(&doc).unwrap();
    let config = catalog.default_config();
    assert_eq!(
        serde_json::to_value(config.player_skill("ally-restore").unwrap()).unwrap(),
        doc["additional_skills"][slot]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(80);
    assert!(load(&old).is_err());
    old["additional_skills"][slot]["heal"]
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
        candidate["additional_skills"][slot]["heal"]["overheal_shield"] = invalid;
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
    let (mut c, mut w) = super::combat_tests::setup(4);
    c.mechanism_version = 81;
    c.additional_skills.push(
        serde_json::from_value(json!({
            "mechanism":"ally_heal","id":"ally-restore","range":100,"cost":2,
            "windup_ms":120,"recovery_ms":0,"cooldown_ms":100,
            "heal":{"amount":51,"overheal_shield":{"status_id":"overflow","conversion_bps":5000}}
        }))
        .unwrap(),
    );
    c.pve_mut().status_definitions.extend([
        serde_json::from_value(json!({
            "mechanism":"absorption_shield","id":"overflow","shield_bps":500,
            "duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
        serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","id":"care","attribute":"healing_received",
            "modifier_bps":5000,"duration_ms":6000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    ]);
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(600);
    w.members[0].position = [400., 400.];
    w.members[1].position = [400., 440.];
    w.members[2].position = [420., 440.];
    w.members[1].facts.hp_points = 1000;
    w.members[1].hp = 977;
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), target, "care", 0);
    (c, w)
}

fn cast(w: &mut World, c: &Config, i: usize, now: u64) -> Events {
    let target = World::unit_wire(&w.members[1].reference.entity);
    w.action(
        i,
        &Action::CastUnit {
            skill_id: "ally-restore".into(),
            target,
        },
        "heal",
        c,
        now,
    )
    .unwrap();
    let pending = w.members[i].pending.take().unwrap();
    let mut events = vec![];
    w.commit(i, pending, c, now + 120, &mut events);
    events
}

fn outcomes(events: &Events) -> Vec<(usize, &Event)> {
    events
        .iter()
        .filter_map(|(recipient, row)| match row {
            EventV4::Private {
                event:
                    event @ (Event::HealResolved { .. }
                    | Event::ShieldGranted { .. }
                    | Event::StatusRejected { .. }),
            } => Some((recipient.unwrap(), event)),
            _ => None,
        })
        .collect()
}

fn grant(events: &Events, caster: usize, expected: (u32, u32, u32)) {
    let rows = outcomes(events);
    assert!(matches!(rows.as_slice(), [
        (from, Event::HealResolved {..}), (1, Event::HealResolved {..}),
        (from_grant, Event::ShieldGranted {requested,granted,remaining,..}),
        (1, Event::ShieldGranted {..}),
    ] if *from==caster && *from_grant==caster && (*requested,*granted,*remaining)==expected));
    assert_eq!(rows[0].1, rows[1].1);
    assert_eq!(rows[2].1, rows[3].1);
}

#[test]
fn member_ally_overheal_shield_heals_first_and_caps_without_refreshing_zero_grants() {
    let (c, mut w) = setup();
    let source = status_entity(&w.members[0].reference.entity);
    let target = status_entity(&w.members[1].reference.entity);
    let first = cast(&mut w, &c, 0, 0);
    grant(&first, 0, (26, 26, 26));
    let results = outcomes(&first);
    assert_eq!(
        *results[0].1,
        Event::HealResolved {
            source: source.clone(),
            target: target.clone(),
            heal_kind: HealKind::AllySkill,
            raw_heal: 51,
            multiplier_bps: 15000,
            adjusted_heal: 76,
            hp_restored: 23,
        }
    );
    assert!(
        matches!(results[2].1, Event::ShieldGranted {source:s,target:t,..} if *s==source && *t==target)
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
    grant(&cast(&mut w, &c, 0, 300), 0, (38, 24, 50));
    grant(&cast(&mut w, &c, 2, 350), 2, (38, 10, 10));
    let before = w.statuses.entries().to_vec();
    assert!(
        before
            .iter()
            .any(|row| row.instance_id == id && row.expires_at_ms == 3420)
    );
    let capped = cast(&mut w, &c, 0, 600);
    assert!(matches!(
        outcomes(&capped).as_slice(),
        [
            (0, Event::HealResolved { hp_restored: 0, .. }),
            (1, Event::HealResolved { hp_restored: 0, .. }),
        ]
    ));
    assert_eq!(w.statuses.entries(), before);
    for (i, expected) in [(0, 0), (1, 60), (2, 0), (3, 0)] {
        assert_eq!(
            w.snapshot(i, &c, Phase::Active, 720).own.shield_total,
            Some(expected)
        );
    }
}

#[test]
fn member_ally_overheal_shield_slot_failure_keeps_committed_heal_and_cost() {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let spirit = w.members[0].spirit;
    let e = cast(&mut w, &c, 0, 0);
    let rows = outcomes(&e);
    assert!(matches!(rows.as_slice(), [
        (0, Event::HealResolved {hp_restored:23,..}), (1,Event::HealResolved {hp_restored:23,..}),
        (0, Event::StatusRejected {source,target,reason:Error::Capacity,..}),
        (1, Event::StatusRejected {reason:Error::Capacity,..}),
    ] if *source==status_entity(&w.members[0].reference.entity) && *target==status_entity(&w.members[1].reference.entity)));
    assert_eq!(w.members[1].hp, 1000);
    assert_eq!(w.members[0].spirit, spirit - 2);
    assert_eq!(w.members[0].cooldowns["ally-restore"], 220);
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 120).own.shield_total,
        Some(0)
    );
}

#[test]
fn member_ally_overheal_shield_exit_preserves_other_source_then_clears_target() {
    let (c, mut w) = setup();
    cast(&mut w, &c, 0, 0);
    cast(&mut w, &c, 2, 30);
    let shields = w
        .statuses
        .entries()
        .iter()
        .filter(|row| row.definition_id == "overflow")
        .map(|row| row.instance_id.clone())
        .collect::<Vec<_>>();
    let sees = |events: &Events, observer, id: &str, expected| {
        events.iter().any(|(recipient, row)| {
            *recipient == Some(observer)
                && matches!(row,EventV4::Private {
            event:Event::StatusRemoved {instance_ids,reason}
        } if *reason==expected && instance_ids.contains(&id.to_owned()))
        })
    };
    let first = w.action(0, &Action::Leave {}, "leave", &c, 200).unwrap();
    assert!(sees(
        &first,
        1,
        &shields[0],
        StatusRemovalReason::SourceInvalid
    ));
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 200).own.shield_total,
        Some(34)
    );
    let second = w.action(1, &Action::Leave {}, "leave", &c, 250).unwrap();
    assert!(sees(
        &second,
        2,
        &shields[1],
        StatusRemovalReason::TargetInvalid
    ));
    assert!(w.statuses.is_empty());
}
