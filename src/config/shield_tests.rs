use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/shield-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn shield_support_and_normalized_definitions_are_explicit_and_reloadable() {
    let p = Package::new();
    p.write(&source());
    let catalog = p.load().unwrap();
    assert_eq!(catalog.default_config().protocol_version(), 3);
    let info = crate::sdk_info();
    assert!(info["mechanismVersions"]["max"].as_u64().unwrap() >= 22);
    assert_eq!(
        info["absorptionShield"]["duplicatePolicies"],
        json!(["refresh", "ignore"])
    );
    let normalized = catalog.describe()["versions"][0]["config"].clone();
    assert!(
        normalized["pve"]["status_definitions"][0]
            .get("modifier_bps")
            .is_none()
    );
    p.write(&normalized);
    assert!(p.load().is_ok());
}
#[test]
fn shield_invalid_fields_caps_policies_and_versions_reject() {
    let p = Package::new();
    for case in 0..25 {
        let mut v = source();
        match case {
            0 => {
                v["pve"]["status_definitions"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("shield_bps");
            }
            1 => v["pve"]["status_definitions"][0]["shield_bps"] = json!(null),
            2 => v["pve"]["status_definitions"][0]["shield_bps"] = json!(0),
            3 => v["pve"]["status_definitions"][0]["shield_bps"] = json!(10001),
            4 => v["pve"]["status_definitions"][0]["shield_bps"] = json!(1.5),
            5 => v["pve"]["status_definitions"][0]["shield_bps"] = json!("600"),
            6 => {
                v["pve"]["status_rules"]
                    .as_object_mut()
                    .unwrap()
                    .remove("shield_cap_bps");
            }
            7 => v["pve"]["status_rules"]["shield_cap_bps"] = json!(null),
            8 => v["pve"]["status_rules"]["shield_cap_bps"] = json!(-1),
            9 => v["pve"]["status_rules"]["shield_cap_bps"] = json!(10001),
            10 => v["pve"]["status_rules"]["shield_cap_bps"] = json!(1.5),
            11 => v["pve"]["status_definitions"][0]["attribute"] = json!(null),
            12 => v["pve"]["status_definitions"][0]["modifier_bps"] = json!(0),
            13 => v["pve"]["status_definitions"][0]["modifier_bps"] = json!(null),
            14 => v["pve"]["status_definitions"][0]["duplicate_policy"] = json!("stack"),
            15 => v["pve"]["status_definitions"][0]["max_stacks"] = json!(null),
            16 => v["pve"]["status_definitions"][0]["formula"] = json!("atk"),
            17 => v["mechanism_version"] = json!(21),
            18 => v["schema_version"] = json!(3),
            19 => {
                v.as_object_mut().unwrap().remove("pve");
            }
            20 => v["pve"]["status_definitions"][0]["duplicate_policy"] = json!("independent"),
            21..=24 => {
                v = serde_json::from_str(crate::test_data::fixture!(
                    "examples/attribute-host/arena.json"
                ))
                .unwrap();
                match case {
                    21 => v["pve"]["status_definitions"][0]["shield_bps"] = json!(null),
                    22 => v["pve"]["status_rules"]["shield_cap_bps"] = json!(null),
                    23 => {
                        v["mechanism_version"] = json!(22);
                        v["pve"]["status_definitions"][0]["shield_bps"] = json!(600);
                    }
                    _ => {
                        v["pve"]["status_definitions"][0]
                            .as_object_mut()
                            .unwrap()
                            .remove("modifier_bps");
                    }
                }
            }
            _ => unreachable!(),
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn shield_caps_are_configuration_and_direct_writes_are_not_inputs() {
    let p = Package::new();
    for cap in [0, 10000] {
        let mut v = source();
        v["pve"]["status_rules"]["shield_cap_bps"] = json!(cap);
        p.write(&v);
        assert!(p.load().is_ok());
    }
    for v in [
        json!({"kind":"set_shield","value":500}),
        json!({"kind":"cast","skill_id":"x","direction":[1,0],"shield":500}),
    ] {
        assert!(serde_json::from_value::<crate::protocol::Action>(v).is_err());
    }
}
#[test]
fn shield_legacy_19_20_21_normalized_json_remains_unchanged() {
    let p = Package::new();
    for raw in [
        crate::test_data::fixture!("examples/status-host/arena.json"),
        crate::test_data::fixture!("examples/stack-host/arena.json"),
        crate::test_data::fixture!("examples/attribute-host/arena.json"),
    ] {
        let v: Value = serde_json::from_str(raw).unwrap();
        p.write(&v);
        let c = p.load().unwrap();
        assert_eq!(
            c.describe()["versions"][0]["config"]["pve"]["status_definitions"],
            v["pve"]["status_definitions"]
        );
        assert_eq!(
            c.describe()["versions"][0]["config"]["pve"]["status_rules"],
            v["pve"]["status_rules"]
        );
    }
}
