use super::periodic_tests::{bind, setup};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_item_healing_received_config_requires_supported_hp_item_and_bool() {
    let mut doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-item-healing-received-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let write_load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    assert!(write_load(&doc).is_ok());

    for value in [json!(null), json!(1), json!("true")] {
        doc["member_item_use"]["pill_renewal"]["apply_healing_received"] = value;
        assert!(write_load(&doc).is_err());
    }
    doc["member_item_use"]["pill_renewal"] =
        json!({"restore_spirit": 40, "apply_healing_received": false});
    assert!(write_load(&doc).is_err());
    doc["member_item_use"]["pill_renewal"]["heal_max_hp_bps"] = json!(430);
    assert!(write_load(&doc).is_ok());
    doc["mechanism_version"] = json!(69);
    assert!(write_load(&doc).is_err());
    for entry in doc["member_item_use"].as_object_mut().unwrap().values_mut() {
        entry
            .as_object_mut()
            .unwrap()
            .remove("apply_healing_received");
    }
    assert!(write_load(&doc).is_ok());
}

fn add_care(c: &mut Config, id: &str, modifier_bps: i32) {
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "id": id, "mechanism": "timed_attribute_modifier", "attribute": "healing_received",
            "modifier_bps": modifier_bps, "duration_ms": 1200,
            "duplicate_policy": "refresh", "source_scoped": true,
        }))
        .unwrap(),
    );
}

fn dose(w: &mut World, c: &Config, now: u64) -> Result<Events> {
    w.action(
        0,
        &Action::UseItem {
            item_id: c.item_id.clone(),
            expected_revision: w.members[0].revision,
        },
        &format!("dose-{now}"),
        c,
        now,
    )
}

fn assert_used(events: &Events, hp: u32, spirit: Option<u32>) {
    assert_eq!(
        events
            .iter()
            .filter_map(|(receiver, e)| match e {
                EventV4::Private {
                    event:
                        Event::ItemUsed {
                            hp_restored,
                            spirit_restored,
                            ..
                        },
                } => Some((*receiver, *hp_restored, *spirit_restored)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![(Some(0), hp, spirit)]
    );
}

#[test]
fn member_item_healing_received_applies_quality_then_modifier_then_hp_gap() {
    for (mut definition, max_hp, modifier, active, expired) in [
        (json!({"heal_hp":43}), 1000, 5000, 79, 53),
        (json!({"heal_max_hp_bps":430}), 1000, -5000, 26, 53),
        (json!({"heal_max_hp_bps":10000}), 1000, -8750, 500, 999),
        (
            json!({"heal_max_hp_bps":10000}),
            u32::MAX,
            100000,
            u32::MAX - 1,
            u32::MAX - 1,
        ),
        (
            json!({"heal_hp":43, "apply_healing_received":false}),
            1000,
            5000,
            53,
            53,
        ),
    ] {
        let (mut c, mut w) = setup(2);
        c.mechanism_version = 70;
        add_care(&mut c, "care", modifier);
        let high_quality = definition["heal_max_hp_bps"] == 10000;
        definition["quality_multiplier_bps"] = json!(if high_quality { 40000 } else { 12500 });
        definition["cooldown_ms"] = json!(900);
        definition
            .as_object_mut()
            .unwrap()
            .entry("apply_healing_received")
            .or_insert(json!(true));
        c.member_item_use = serde_json::from_value(json!({c.item_id.clone(): definition})).unwrap();
        w.members[0].facts.hp_points = max_hp;
        w.members[0].inventory.insert(c.item_id.clone(), 3);
        w.members[0].looted.insert(c.item_id.clone(), 3);
        let target = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), target, "care", 0);
        let other_hp = w.members[1].hp;
        for (now, amount) in [(0, active), (1200, expired)] {
            w.members[0].hp = 1;
            assert_used(&dose(&mut w, &c, now).unwrap(), amount, None);
            assert_eq!(w.members[0].hp, 1 + amount);
            assert_eq!(w.members[1].hp, other_hp);
        }
        assert_eq!(w.members[0].looted[&c.item_id], 1);
    }
}

#[test]
fn member_item_healing_received_zero_rejects_atomically_and_spirit_can_commit() {
    let (mut c, mut w) = setup(2);
    c.mechanism_version = 70;
    add_care(&mut c, "blocked-care", -10000);
    add_care(&mut c, "new-care", 10000);
    let mut definition = json!({
        "heal_hp": 40, "apply_healing_received": true, "cooldown_ms": 900,
        "on_use_self_statuses": ["new-care"],
    });
    c.member_item_use = serde_json::from_value(json!({c.item_id.clone(): definition})).unwrap();
    w.members[0].hp -= 100;
    w.members[0].spirit -= 100;
    w.members[0].inventory.insert(c.item_id.clone(), 2);
    w.members[0].looted.insert(c.item_id.clone(), 2);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "blocked-care", 0);

    let before = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(dose(&mut w, &c, 30), Err(Error::InvalidState));
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own, before);
    assert_eq!(w.members[0].looted[&c.item_id], 2);

    definition["restore_spirit"] = json!(40);
    c.member_item_use = serde_json::from_value(json!({c.item_id.clone(): definition})).unwrap();
    assert_used(&dose(&mut w, &c, 30).unwrap(), 0, Some(40));
    let after = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(after.hp, before.hp);
    assert_eq!(after.spirit, before.spirit + 40);
    assert_eq!(after.inventory_revision, before.inventory_revision + 1);
    assert_eq!(w.members[0].looted[&c.item_id], 1);
    assert_eq!(w.members[0].item_cooldowns[&c.item_id], 930);
    assert!(after.statuses.iter().any(|s| s.definition_id == "new-care"));

    assert_used(&dose(&mut w, &c, 930).unwrap(), 40, Some(40));
}
