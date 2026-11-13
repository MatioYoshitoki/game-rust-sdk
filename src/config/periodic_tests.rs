use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!("examples/dot-host/arena.json")).unwrap()
}
#[test]
fn periodic_support_and_normalized_config_are_explicit_and_reloadable() {
    let p = Package::new();
    p.write(&source());
    let c = p.load().unwrap();
    let info = crate::sdk_info();
    assert!(info["mechanismVersions"]["max"].as_u64().unwrap() >= 23);
    assert!(
        info["periodicEffects"]["kinds"]
            .as_array()
            .unwrap()
            .contains(&json!("physical_damage"))
    );
    assert_eq!(info["periodicEffects"]["maxTicks"], 128);
    let normalized = c.describe()["versions"][0]["config"].clone();
    assert!(
        normalized["pve"]["status_definitions"][1]
            .get("modifier_bps")
            .is_none()
    );
    p.write(&normalized);
    assert!(p.load().is_ok());
}
#[test]
fn periodic_invalid_fields_timing_versions_and_bindings_reject_startup() {
    let p = Package::new();
    for case in 0..24 {
        let mut v = source();
        match case {
            0 => {
                v["pve"]["status_definitions"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("tick_interval_ms");
            }
            1 => v["pve"]["status_definitions"][1]["tick_interval_ms"] = json!(null),
            2 => v["pve"]["status_definitions"][1]["tick_interval_ms"] = json!(0),
            3 => v["pve"]["status_definitions"][1]["tick_interval_ms"] = json!(29),
            4 => v["pve"]["status_definitions"][1]["tick_interval_ms"] = json!(2200),
            5 => v["pve"]["status_definitions"][1]["tick_interval_ms"] = json!(30.5),
            6 => {
                v["pve"]["status_definitions"][1]["tick_interval_ms"] = json!(30);
                v["pve"]["status_definitions"][1]["duration_ms"] = json!(3871);
            }
            7 => {
                v["pve"]["status_definitions"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("damage_bps");
            }
            8 => v["pve"]["status_definitions"][1]["damage_bps"] = json!(null),
            9 => v["pve"]["status_definitions"][1]["damage_bps"] = json!(-1),
            10 => v["pve"]["status_definitions"][1]["damage_bps"] = json!(100001),
            11 => v["pve"]["status_definitions"][1]["damage_bps"] = json!("5000"),
            12 => v["pve"]["status_definitions"][1]["modifier_bps"] = json!(null),
            13 => v["pve"]["status_definitions"][1]["attribute"] = json!(null),
            14 => v["pve"]["status_definitions"][1]["shield_bps"] = json!(null),
            15 => v["pve"]["status_definitions"][1]["max_stacks"] = json!(null),
            16 => v["pve"]["status_definitions"][1]["duplicate_policy"] = json!("stack"),
            17 => v["skill"]["on_commit_self_statuses"] = json!(["wound"]),
            18 => v["pve"]["combat"]["projectile"]["on_commit_self_statuses"] = json!(["wound"]),
            19 => v["mechanism_version"] = json!(22),
            20 => v["schema_version"] = json!(3),
            21 => v["pve"]["status_definitions"][1]["formula"] = json!("target_hp"),
            22 | 23 => {
                v = serde_json::from_str(crate::test_data::fixture!(
                    "examples/shield-host/arena.json"
                ))
                .unwrap();
                if case == 23 {
                    v["mechanism_version"] = json!(23);
                }
                v["pve"]["status_definitions"][0]["tick_interval_ms"] = json!(null);
            }
            _ => unreachable!(),
        };
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn periodic_engineering_limit_and_zero_damage_are_configurable_but_not_inputs() {
    let p = Package::new();
    let mut v = source();
    v["pve"]["status_definitions"][1]["tick_interval_ms"] = json!(30);
    v["pve"]["status_definitions"][1]["duration_ms"] = json!(3841);
    v["pve"]["status_definitions"][1]["damage_bps"] = json!(0);
    p.write(&v);
    assert!(p.load().is_ok());
    for value in [
        json!({"kind":"tick_periodic"}),
        json!({"kind":"cast","skill_id":"x","direction":[1,0],"ticks_done":3}),
    ] {
        assert!(serde_json::from_value::<crate::protocol::Action>(value).is_err());
    }
}
#[test]
fn periodic_legacy_definitions_19_to_22_keep_their_normalized_shape() {
    let p = Package::new();
    for raw in [
        crate::test_data::fixture!("examples/status-host/arena.json"),
        crate::test_data::fixture!("examples/stack-host/arena.json"),
        crate::test_data::fixture!("examples/attribute-host/arena.json"),
        crate::test_data::fixture!("examples/shield-host/arena.json"),
    ] {
        let v: Value = serde_json::from_str(raw).unwrap();
        p.write(&v);
        let c = p.load().unwrap();
        assert_eq!(
            c.describe()["versions"][0]["config"]["pve"]["status_definitions"],
            v["pve"]["status_definitions"]
        );
    }
}
