use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_overheal_shield_configuration_requires_explicit_hp_and_refresh_shield() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-overheal-shield-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    assert!(load(&doc).is_ok());
    let mut old = doc.clone();
    old["mechanism_version"] = json!(74);
    assert!(load(&old).is_err());
    old["member_item_use"]["overflow-tonic"]
        .as_object_mut()
        .unwrap()
        .remove("overheal_shield");
    assert!(load(&old).is_ok());
    let mut dual = doc.clone();
    dual["member_item_use"]["overflow-tonic"]["restore_spirit"] = json!(30);
    assert!(load(&dual).is_ok());

    for value in [
        json!(null),
        json!({"status_id":"overflow-shield","conversion_bps":0}),
        json!({"status_id":"overflow-shield","conversion_bps":10001}),
        json!({"status_id":"demo-guard","conversion_bps":5000}),
        json!({"status_id":"missing-shield","conversion_bps":5000}),
    ] {
        let mut invalid = doc.clone();
        invalid["member_item_use"]["overflow-tonic"]["overheal_shield"] = value;
        assert!(load(&invalid).is_err());
    }
    let mut extra = doc.clone();
    extra["member_item_use"]["overflow-tonic"]["on_use_self_statuses"] = json!(["demo-guard"]);
    assert!(load(&extra).is_err());
}

fn setup(definition: Value) -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(2);
    c.mechanism_version = 75;
    let pve = c.pve_mut();
    pve.status_definitions.push(
        serde_json::from_value(json!({
            "id":"overflow","mechanism":"absorption_shield","shield_bps":1000,
            "duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    pve.status_rules.as_mut().unwrap().shield_cap_bps = Some(1500);
    c.member_item_use = serde_json::from_value(json!({"tonic":definition})).unwrap();
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 950;
    w.members[0].inventory.insert("tonic".into(), 3);
    w.members[0].looted.insert("tonic".into(), 3);
    (c, w)
}

fn use_tonic(w: &mut World, c: &Config, now: u64) -> Result<Events> {
    w.action(
        0,
        &Action::UseItem {
            item_id: "tonic".into(),
            expected_revision: w.members[0].revision,
        },
        &format!("use-tonic-{now}"),
        c,
        now,
    )
}

fn assert_grant(events: &Events, expected: (u32, u32, u32)) {
    let actual = events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event:
                    Event::ShieldGranted {
                        definition_id,
                        requested,
                        granted,
                        remaining,
                        ..
                    },
            } if definition_id == "overflow" => {
                Some((*recipient, (*requested, *granted, *remaining)))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, vec![(Some(0), expected)]);
}

#[test]
fn member_overheal_shield_uses_adjusted_excess_and_preserves_balance_at_total_cap() {
    let (mut c, mut w) = setup(json!({
        "heal_hp":100,"quality_multiplier_bps":12000,"apply_healing_received":true,"cooldown_ms":900,
        "overheal_shield":{"status_id":"overflow","conversion_bps":5000}
    }));
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({
        "id":"care","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":2000,
        "duration_ms":6000,"duplicate_policy":"refresh","source_scoped":true
    })).unwrap());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "care", 0);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "ward", 0);
    let events = use_tonic(&mut w, &c, 0).unwrap();
    assert_grant(&events, (47, 47, 47));
    let snapshot = w.snapshot(0, &c, Phase::Active, 0).own;
    assert_eq!((snapshot.hp, snapshot.shield_total), (1000, Some(147)));
    let instance = snapshot
        .statuses
        .iter()
        .find(|row| row.definition_id == "overflow")
        .unwrap();

    let events = use_tonic(&mut w, &c, 900).unwrap();
    assert_grant(&events, (72, 3, 50));
    let capped = w.snapshot(0, &c, Phase::Active, 900).own;
    let refreshed = capped
        .statuses
        .iter()
        .find(|row| row.definition_id == "overflow")
        .unwrap();
    assert_eq!(refreshed.instance_id, instance.instance_id);
    assert_eq!(refreshed.expires_at_ms, 3900);
    assert_eq!(capped.shield_total, Some(150));
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 900).own.shield_total,
        Some(0)
    );
    let before_retry = w.snapshot(0, &c, Phase::Active, 1800).own;
    assert_eq!(use_tonic(&mut w, &c, 1800), Err(Error::InvalidState));
    assert_eq!(w.snapshot(0, &c, Phase::Active, 1800).own, before_retry);
    assert_eq!(w.members[0].looted["tonic"], 1);
    let source = w.npcs[0].entity.clone();
    assert_eq!(
        w.absorb_damage(&target, (&source.id, false), 20, 1830, &mut vec![]),
        (0, 20)
    );
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 1830).own.shield_total,
        Some(130)
    );
}

#[test]
fn member_overheal_shield_capacity_rejection_leaves_item_and_hp_until_slot_expires() {
    let (mut c, mut w) = setup(json!({
        "heal_hp":200,"cooldown_ms":900,
        "overheal_shield":{"status_id":"overflow","conversion_bps":10000}
    }));
    let pve = c.pve_mut();
    pve.status_rules.as_mut().unwrap().max_active_per_entity = 1;
    pve.status_definitions
        .iter_mut()
        .find(|row| row.id == "ward")
        .unwrap()
        .duration_ms = 1000;
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "ward", 0);
    let before = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(use_tonic(&mut w, &c, 30), Err(Error::Capacity));
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own, before);
    assert_eq!(w.members[0].looted["tonic"], 3);
    let events = use_tonic(&mut w, &c, 1000).unwrap();
    assert_grant(&events, (150, 100, 100));
    assert_eq!((w.members[0].hp, w.members[0].looted["tonic"]), (1000, 2));
    assert_eq!(w.members[0].item_cooldowns["tonic"], 1900);
}
