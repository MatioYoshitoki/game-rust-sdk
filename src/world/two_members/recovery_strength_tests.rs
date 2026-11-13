use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_recovery_strength_configuration_accepts_only_supported_self_bindings() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-recovery-strength-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    assert!(load(&doc).is_ok());

    let mut item = doc.clone();
    item["member_item_use"] = json!({
        "xue-zhi": { "heal_hp": 10, "on_use_self_statuses": ["regen-strength"] }
    });
    assert!(load(&item).is_ok());

    let definition = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    for (path, value) in [
        ("/mechanism_version".to_owned(), json!(71)),
        (
            "/skill/on_hit_target_statuses".to_owned(),
            json!(["regen-strength"]),
        ),
        (
            "/pve/combat/projectile/on_commit_self_statuses".to_owned(),
            json!(["regen-strength"]),
        ),
        (
            format!("/pve/status_definitions/{definition}/duplicate_policy"),
            json!("stack"),
        ),
        (
            format!("/pve/status_definitions/{definition}/modifier_bps"),
            json!(100001),
        ),
        (
            format!("/pve/status_definitions/{definition}/modifier_bps"),
            json!(-10001),
        ),
    ] {
        let mut invalid = doc.clone();
        *invalid.pointer_mut(&path).unwrap() = value;
        assert!(load(&invalid).is_err(), "{path}");
    }
    let mut null_stacks = doc.clone();
    null_stacks["pve"]["status_definitions"][definition]["max_stacks"] = Value::Null;
    assert!(load(&null_stacks).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(2);
    c.mechanism_version = 72;
    c.params.regen.interval_ms = 1000;
    c.params.regen.near_radius = 200.0;
    c.params.regen.near_ratio = 0.005;
    c.params.regen.far_ratio = 0.015;
    w.next_regen = 1000;
    for m in &mut w.members {
        m.facts.hp_points = 1000;
        m.facts.spirit_points = 300;
        m.hp = 500;
        m.spirit = 100;
    }
    (c, w)
}

fn add_modifier(
    w: &mut World,
    c: &mut Config,
    id: &str,
    attribute: &str,
    bps: i32,
    duration_ms: u64,
) {
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "id": id, "mechanism": "timed_attribute_modifier", "attribute": attribute,
            "modifier_bps": bps, "duration_ms": duration_ms, "duplicate_policy": "refresh",
            "source_scoped": true,
        }))
        .unwrap(),
    );
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, 0);
}

#[test]
fn member_recovery_strength_combines_once_and_uses_each_members_live_enemy_distance() {
    let (mut c, mut w) = setup();
    w.members[0].position = [400.0, 400.0];
    w.members[1].position = [400.0, 480.0];
    w.npcs.truncate(1);
    w.npcs[0].position = [600.0, 400.0];
    w.npcs[0].home = w.npcs[0].position;
    Arc::make_mut(&mut w.npcs[0].profile).combat.aggro_range = 0.1;
    add_modifier(
        &mut w,
        &mut c,
        "strength-a",
        "recovery_strength",
        4000,
        2100,
    );
    add_modifier(
        &mut w,
        &mut c,
        "strength-b",
        "recovery_strength",
        6000,
        2100,
    );
    add_modifier(
        &mut w,
        &mut c,
        "healing-a",
        "healing_received",
        100000,
        2100,
    );

    w.tick(&c, 1000);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (510, 103));
    assert_eq!((w.members[1].hp, w.members[1].spirit), (515, 104));
    w.npcs[0].hp = 0;
    w.tick(&c, 2000);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (540, 112));
    assert_eq!((w.members[1].hp, w.members[1].spirit), (530, 108));
}

#[test]
fn member_recovery_strength_has_zero_floor_expiry_clipping_and_no_revival() {
    let (mut c, mut w) = setup();
    w.npcs.clear();
    add_modifier(
        &mut w,
        &mut c,
        "strength-a",
        "recovery_strength",
        -6000,
        2000,
    );
    add_modifier(
        &mut w,
        &mut c,
        "strength-b",
        "recovery_strength",
        -5000,
        2000,
    );
    w.protect(0, 0, &mut vec![]);
    w.tick(&c, 1000);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (500, 100));
    assert_eq!((w.members[1].hp, w.members[1].spirit), (515, 104));
    w.members[0].hp = 999;
    w.members[0].spirit = 299;
    w.tick(&c, 2000);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (1000, 300));
    assert!(w.members[0].protected);
    w.members[0].hp = 0;
    w.members[0].spirit = 0;
    w.tick(&c, 3000);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (0, 0));
}
