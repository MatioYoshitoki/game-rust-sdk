use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/heal-modifier-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn received_support_and_normalized_configuration_are_explicit() {
    let p = Package::new();
    p.write(&source());
    let c = p.load().unwrap();
    let info = crate::sdk_info();
    assert!(info["mechanismVersions"]["max"].as_u64().unwrap() >= 25);
    assert_eq!(
        info["timedStatus"]["healingReceived"]["capture"],
        "per_heal"
    );
    assert!(info["timedStatus"]["healingReceived"]["positiveGameplayCap"].is_null());
    let v = c.describe()["versions"][0]["config"].clone();
    assert_eq!(
        v["pve"]["status_definitions"][1]["attribute"],
        "healing_received"
    );
    p.write(&v);
    assert!(p.load().is_ok());
}
#[test]
fn received_invalid_versions_fields_strategies_and_limits_reject() {
    let p = Package::new();
    for case in 0..20 {
        let mut v = source();
        match case {
            0 => v["mechanism_version"] = json!(24),
            1 => v["schema_version"] = json!(3),
            2 => v["pve"]["status_definitions"][1]["attribute"] = json!("healing_unknown"),
            3 => v["pve"]["status_definitions"][1]["attribute"] = json!(null),
            4 => v["pve"]["status_definitions"][1]["modifier_bps"] = json!(-10001),
            5 => v["pve"]["status_definitions"][1]["modifier_bps"] = json!(100001),
            6 => v["pve"]["status_definitions"][1]["modifier_bps"] = json!(null),
            7 => v["pve"]["status_definitions"][1]["duplicate_policy"] = json!("stack"),
            8 => {
                v["pve"]["status_definitions"][1]["duplicate_policy"] = json!("stack");
                v["pve"]["status_definitions"][1]["max_stacks"] = json!(2);
            }
            9 => v["pve"]["status_definitions"][1]["max_stacks"] = json!(null),
            10..=12 => {
                v["pve"]["status_definitions"][1]
                    [["heal_bps", "damage_bps", "shield_bps"][case - 10]] = json!(null)
            }
            13 => v["pve"]["status_definitions"][1]["healing_cap_bps"] = json!(10000),
            14 => v["pve"]["status_definitions"][1]["duration_ms"] = json!(0),
            15 => v["pve"]["status_definitions"][1]["mechanism"] = json!("timed_defense_modifier"),
            16 => v["skill"]["on_hit_target_statuses"] = json!(["missing"]),
            17 => v["pve"]["status_rules"]["max_active_per_entity"] = json!(65),
            18 => v["pve"]["status_definitions"][1]["modifier_bps"] = json!(1.5),
            19 => {
                v["pve"]["status_definitions"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("modifier_bps");
            }
            _ => unreachable!(),
        };
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn received_bounds_and_existing_self_hit_bindings_are_configurable() {
    let p = Package::new();
    for bps in [-10000, 0, 100000] {
        let mut v = source();
        v["pve"]["status_definitions"][1]["modifier_bps"] = json!(bps);
        v["skill"]["on_hit_target_statuses"] = json!(["care"]);
        v["pve"]["combat"]["projectile"]["on_commit_self_statuses"] = json!(["care"]);
        p.write(&v);
        assert!(p.load().is_ok());
    }
    for v in [
        json!({"kind":"heal","amount":100}),
        json!({"kind":"set_healing_received","bps":0}),
    ] {
        assert!(serde_json::from_value::<crate::protocol::Action>(v).is_err());
    }
}
#[test]
fn received_legacy_definitions_19_to_24_keep_their_shape() {
    let p = Package::new();
    for name in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
        "hot-host",
    ] {
        let v: Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                    .join(format!("examples/{name}/arena.json")),
            )
            .unwrap(),
        )
        .unwrap();
        p.write(&v);
        assert_eq!(
            p.load().unwrap().describe()["versions"][0]["config"]["pve"]["status_definitions"],
            v["pve"]["status_definitions"]
        );
    }
}
