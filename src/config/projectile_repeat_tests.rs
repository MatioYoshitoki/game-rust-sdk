use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/repeat-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn projectile_repeat_config_both_slots_and_normalization() {
    let p = Package::new();
    for swap in [false, true] {
        let mut v = source();
        if swap {
            let s = v["skill"].take();
            v["skill"] = v["additional_skills"][0].take();
            v["additional_skills"][0] = s;
        }
        p.write(&v);
        let n = p.load().unwrap().describe()["versions"][0]["config"].clone();
        let s = if swap {
            &n["additional_skills"][0]
        } else {
            &n["skill"]
        };
        assert_eq!(s["repeat_interval_ms"], 600);
        p.write(&n);
        assert!(p.load().is_ok());
    }
    assert_eq!(
        crate::sdk_info()["playerSkills"]["projectileRepeat"]["clock"],
        "logical_fixed_step"
    );
}
#[test]
fn projectile_repeat_config_gates_all_old_versions_and_wrong_mechanisms() {
    let p = Package::new();
    for m in 1..33 {
        let mut v = source();
        v["mechanism_version"] = json!(m);
        p.write(&v);
        assert!(p.load().is_err());
    }
    for mechanism in [
        "projectile",
        "unit_projectile",
        "self_status",
        "ground_circle",
        "ground_field",
    ] {
        let mut v = source();
        v["skill"]["mechanism"] = json!(mechanism);
        p.write(&v);
        assert!(p.load().is_err());
    }
    for k in ["schema_version", "pve"] {
        let mut v = source();
        v[k] = json!(null);
        p.write(&v);
        assert!(p.load().is_err());
    }
}
#[test]
fn projectile_repeat_config_strict_types_and_inclusive_bounds() {
    let p = Package::new();
    for x in [
        json!(null),
        json!(-1),
        json!(0.1),
        json!(60001),
        json!("60"),
        json!({}),
        json!([]),
        json!(true),
    ] {
        let mut v = source();
        v["skill"]["repeat_interval_ms"] = x;
        p.write(&v);
        assert!(p.load().is_err());
    }
    for n in [0, 1, 29, 30, 31, 60000] {
        let mut v = source();
        v["skill"]["repeat_interval_ms"] = json!(n);
        p.write(&v);
        assert!(p.load().is_ok());
    }
}
#[test]
fn projectile_repeat_config_omission_retains_old_wire_and_once_semantics() {
    let p = Package::new();
    let mut v = source();
    v["skill"]
        .as_object_mut()
        .unwrap()
        .remove("repeat_interval_ms");
    for m in [32, 33] {
        v["mechanism_version"] = json!(m);
        p.write(&v);
        let c = p.load().unwrap();
        let n = c.describe()["versions"][0]["config"].clone();
        assert!(n["skill"].get("repeat_interval_ms").is_none());
    }
}
#[test]
fn projectile_repeat_config_preserves_reference_and_total_budget_validation() {
    let p = Package::new();
    for (k, x) in [
        ("max_hits", json!(129)),
        ("ttl_ms", json!(0)),
        ("on_hit_target_statuses", json!(["missing"])),
        ("on_hit_target_statuses", json!(["renewal"])),
        ("contact_policy", json!({"repeat_interval_ms":30})),
    ] {
        let mut v = source();
        v["skill"][k] = x;
        p.write(&v);
        assert!(p.load().is_err());
    }
}
