use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/attribute-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn attribute_support_and_generic_selectors_are_explicit() {
    let p = Package::new();
    p.write(&source());
    assert_eq!(p.load().unwrap().default_config().protocol_version(), 3);
    let info = crate::sdk_info();
    assert!(info["mechanismVersions"]["max"].as_u64().unwrap() >= 21);
    assert_eq!(info["timedStatus"]["attributes"], json!(["atk", "def"]));
    assert_eq!(info["timedStatus"]["attributeMinimumMechanismVersion"], 21);
}
#[test]
fn attribute_invalid_versions_selectors_caps_and_extra_fields_reject() {
    let p = Package::new();
    for case in 0..23 {
        let mut v = source();
        match case {
            0 => {
                v["pve"]["status_definitions"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("attribute");
            }
            1 => v["pve"]["status_definitions"][0]["attribute"] = json!(null),
            2 => v["pve"]["status_definitions"][0]["attribute"] = json!("spd"),
            3 => v["pve"]["status_definitions"][0]["attribute"] = json!("ATK"),
            4 => v["pve"]["status_definitions"][0]["attribute"] = json!(1),
            5 => v["mechanism_version"] = json!(20),
            6 => v["mechanism_version"] = json!(19),
            7 => v["schema_version"] = json!(3),
            8 => {
                v.as_object_mut().unwrap().remove("pve");
            }
            9 => v["pve"]["status_definitions"][0]["mechanism"] = json!("timed_defense_modifier"),
            10 => {
                v["pve"]["status_definitions"][0]["mechanism"] = json!("timed_defense_modifier");
                v["pve"]["status_definitions"][0]["attribute"] = json!(null);
            }
            11 => {
                v["pve"]["status_rules"]
                    .as_object_mut()
                    .unwrap()
                    .remove("attack_bonus_cap_bps");
            }
            12 => v["pve"]["status_rules"]["attack_bonus_cap_bps"] = json!(null),
            13 => v["pve"]["status_rules"]["attack_bonus_cap_bps"] = json!(-1),
            14 => v["pve"]["status_rules"]["attack_bonus_cap_bps"] = json!(100001),
            15 => v["pve"]["status_rules"]["attack_bonus_cap_bps"] = json!(1.5),
            16 => v["pve"]["status_rules"]["attack_bonus_cap_bps"] = json!("100"),
            17 => v["pve"]["status_definitions"][0]["effective_atk"] = json!(200),
            18 => v["pve"]["status_definitions"][0]["capture"] = json!("hit"),
            19 => v["pve"]["status_definitions"][0]["modifier_bps"] = json!(100001),
            20 => v["pve"]["status_definitions"][0]["modifier_bps"] = json!(-10001),
            21 | 22 => {
                v = serde_json::from_str(crate::test_data::fixture!(
                    "examples/stack-host/arena.json"
                ))
                .unwrap();
                if case == 21 {
                    v["pve"]["status_definitions"][0]["attribute"] = json!(null);
                } else {
                    v["pve"]["status_rules"]["attack_bonus_cap_bps"] = json!(null);
                }
            }
            _ => unreachable!(),
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn attribute_legacy_19_and_20_definitions_keep_their_wire_form() {
    let p = Package::new();
    for raw in [
        crate::test_data::fixture!("examples/status-host/arena.json"),
        crate::test_data::fixture!("examples/stack-host/arena.json"),
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
#[test]
fn attribute_caps_are_tuning_and_direct_writes_remain_unavailable() {
    let p = Package::new();
    for cap in [0, 100000] {
        let mut v = source();
        v["pve"]["status_rules"]["attack_bonus_cap_bps"] = json!(cap);
        p.write(&v);
        assert!(p.load().is_ok());
    }
    for v in [
        json!({"kind":"set_atk","value":500}),
        json!({"kind":"cast","skill_id":"x","direction":[1,0],"effective_atk":500}),
    ] {
        assert!(serde_json::from_value::<crate::protocol::Action>(v).is_err());
    }
}
