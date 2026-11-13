use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!("examples/hot-host/arena.json")).unwrap()
}
#[test]
fn hot_support_normalization_and_reload_are_explicit() {
    let p = Package::new();
    p.write(&source());
    let c = p.load().unwrap();
    let info = crate::sdk_info();
    assert!(info["mechanismVersions"]["max"].as_u64().unwrap() >= 24);
    assert_eq!(info["periodicEffects"]["healBasis"], "base_max_hp");
    assert_eq!(
        info["periodicEffects"]["kinds"],
        json!(["physical_damage", "max_hp_heal"])
    );
    let v = c.describe()["versions"][0]["config"].clone();
    assert_eq!(v["pve"]["status_definitions"][0]["heal_bps"], 1000);
    for key in [
        "damage_bps",
        "modifier_bps",
        "shield_bps",
        "attribute",
        "max_stacks",
    ] {
        assert!(v["pve"]["status_definitions"][0].get(key).is_none());
    }
    p.write(&v);
    assert!(p.load().is_ok());
}
#[test]
fn hot_invalid_fields_versions_timing_and_hit_bindings_reject() {
    let p = Package::new();
    for case in 0..28 {
        let mut v = source();
        match case {
            0 => {
                v["pve"]["status_definitions"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("heal_bps");
            }
            1 => v["pve"]["status_definitions"][0]["heal_bps"] = json!(null),
            2 => v["pve"]["status_definitions"][0]["heal_bps"] = json!(-1),
            3 => v["pve"]["status_definitions"][0]["heal_bps"] = json!(10001),
            4 => v["pve"]["status_definitions"][0]["heal_bps"] = json!("1000"),
            5 => {
                v["pve"]["status_definitions"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("tick_interval_ms");
            }
            6 => v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(null),
            7 => v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(0),
            8 => v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(29),
            9 => v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(2200),
            10 => v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(30.1),
            11 => {
                v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(30);
                v["pve"]["status_definitions"][0]["duration_ms"] = json!(3871);
            }
            12..=16 => {
                v["pve"]["status_definitions"][0][[
                    "damage_bps",
                    "modifier_bps",
                    "shield_bps",
                    "attribute",
                    "max_stacks",
                ][case - 12]] = json!(null)
            }
            17 => v["pve"]["status_definitions"][0]["duplicate_policy"] = json!("stack"),
            18 => v["skill"]["on_hit_target_statuses"] = json!(["renewal"]),
            19 => v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!(["renewal"]),
            20 => v["mechanism_version"] = json!(23),
            21 => v["schema_version"] = json!(3),
            22 => v["pve"]["status_definitions"][0]["formula"] = json!("attack"),
            23..=26 => {
                v = serde_json::from_str(crate::test_data::fixture!(
                    "examples/dot-host/arena.json"
                ))
                .unwrap();
                if case >= 25 {
                    v["mechanism_version"] = json!(24);
                };
                v["pve"]["status_definitions"][if case % 2 == 0 { 0 } else { 1 }]["heal_bps"] =
                    json!(null);
            }
            27 => v["pve"]["status_definitions"][0]["duration_ms"] = json!(0),
            _ => unreachable!(),
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn hot_zero_maximum_budget_and_self_monster_binding_are_valid() {
    let p = Package::new();
    let mut v = source();
    v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(30);
    v["pve"]["status_definitions"][0]["duration_ms"] = json!(3841);
    v["pve"]["combat"]["projectile"]["on_commit_self_statuses"] = json!(["renewal"]);
    for amount in [0, 10000] {
        v["pve"]["status_definitions"][0]["heal_bps"] = json!(amount);
        p.write(&v);
        assert!(p.load().is_ok());
    }
    for action in [
        json!({"kind":"heal","amount":100}),
        json!({"kind":"tick_periodic_heal"}),
        json!({"kind":"cast","skill_id":"x","direction":[1,0],"heal_bps":10000}),
    ] {
        assert!(serde_json::from_value::<crate::protocol::Action>(action).is_err());
    }
}
#[test]
fn hot_legacy_19_to_23_normalized_definitions_gain_no_heal_fields() {
    let p = Package::new();
    for name in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
    ] {
        let raw: Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                    .join(format!("examples/{name}/arena.json")),
            )
            .unwrap(),
        )
        .unwrap();
        p.write(&raw);
        let normalized = p.load().unwrap().describe()["versions"][0]["config"].clone();
        assert_eq!(
            normalized["pve"]["status_definitions"],
            raw["pve"]["status_definitions"]
        );
    }
}
