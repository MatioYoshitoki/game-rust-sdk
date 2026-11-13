use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!("examples/stack-host/arena.json")).unwrap()
}
#[test]
fn stack_loads_version20_and_declares_finite_support() {
    let p = Package::new();
    p.write(&source());
    let c = p.load().unwrap();
    assert_eq!(c.default_config().protocol_version(), 3);
    let info = crate::sdk_info();
    assert!(info["mechanismVersions"]["max"].as_u64().unwrap() >= 20);
    assert_eq!(info["timedStatus"]["maxStacks"], 128);
}
#[test]
fn stack_invalid_caps_policies_versions_and_extra_fields_reject_startup() {
    let p = Package::new();
    for case in 0..14 {
        let mut v = source();
        match case {
            0 => {
                v["pve"]["status_definitions"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("max_stacks");
            }
            1 => v["pve"]["status_definitions"][0]["max_stacks"] = json!(null),
            2 => v["pve"]["status_definitions"][0]["max_stacks"] = json!(0),
            3 => v["pve"]["status_definitions"][0]["max_stacks"] = json!(129),
            4 => v["pve"]["status_definitions"][0]["max_stacks"] = json!(1.5),
            5 => v["pve"]["status_definitions"][0]["max_stacks"] = json!("3"),
            6 => v["mechanism_version"] = json!(19),
            7 => v["pve"]["status_definitions"][0]["duplicate_policy"] = json!("refresh"),
            8 => v["pve"]["status_definitions"][0]["duplicate_policy"] = json!("ignore"),
            9 => v["pve"]["status_definitions"][0]["duplicate_policy"] = json!("independent"),
            10 => v["pve"]["status_definitions"][0]["stack_expiry"] = json!("per_layer"),
            11 => v["schema_version"] = json!(3),
            12 => {
                v["pve"]["status_definitions"][0]["duplicate_policy"] = json!("refresh");
                v["pve"]["status_definitions"][0]["max_stacks"] = json!(null);
            }
            _ => {
                v["mechanism_version"] = json!(19);
                v["pve"]["status_definitions"][0]["max_stacks"] = json!(null);
            }
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn stack_accepts_one_and_engineering_cap_and_preserves_legacy19_definitions() {
    let p = Package::new();
    for cap in [1, 128] {
        let mut v = source();
        for d in v["pve"]["status_definitions"].as_array_mut().unwrap() {
            d["max_stacks"] = json!(cap);
        }
        p.write(&v);
        assert!(p.load().is_ok());
    }
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/status-host/arena.json"
    ))
    .unwrap();
    p.write(&v);
    let c = p.load().unwrap();
    assert_eq!(c.default_config().protocol_version(), 3);
    assert_eq!(
        c.describe()["versions"][0]["config"]["pve"]["status_definitions"],
        v["pve"]["status_definitions"]
    );
}
#[test]
fn stack_count_cannot_be_submitted_as_player_intent_or_external_fact() {
    for v in [
        json!({"kind":"set_stacks","count":3}),
        json!({"kind":"cast","skill_id":"x","direction":[1,0],"stacks":3}),
        json!({"kind":"cast","skill_id":"x","direction":[1,0],"max_stacks":128}),
    ] {
        assert!(serde_json::from_value::<crate::protocol::Action>(v).is_err());
    }
}
