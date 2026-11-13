use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!("examples/field-host/arena.json")).unwrap()
}
#[test]
fn field_skill_config_both_slots_roundtrip_and_support() {
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
        assert_eq!(s["mechanism"], "ground_field");
        assert_eq!(s["duration_ms"], 2400);
        assert!(s.get("speed").is_none());
        p.write(&n);
        assert!(p.load().is_ok());
    }
    assert_eq!(
        crate::sdk_info()["playerSkills"]["groundField"]["sourceInvalidPolicy"],
        "end"
    );
}
#[test]
fn field_skill_config_requires_mechanism_30_schema_and_pve() {
    let p = Package::new();
    for m in 1..30 {
        let mut v = source();
        v["mechanism_version"] = json!(m);
        p.write(&v);
        assert!(p.load().is_err());
    }
    for k in ["pve", "schema_version"] {
        let mut v = source();
        v[k] = json!(null);
        p.write(&v);
        assert!(p.load().is_err());
    }
}
#[test]
fn field_skill_config_strict_numbers_ownership_and_fields() {
    let p = Package::new();
    for (k, x) in [
        ("duration_ms", json!(0)),
        ("duration_ms", json!(60001)),
        ("duration_ms", json!(-1)),
        ("duration_ms", json!(1.5)),
        ("duration_ms", json!(600)),
        ("tick_interval_ms", json!(0)),
        ("tick_interval_ms", json!(29)),
        ("tick_interval_ms", json!(60001)),
        ("radius", json!(0)),
        ("range", json!(0)),
        ("cost", json!(1000001)),
        ("damage_bps", json!(100001)),
        ("warning_ms", json!(60001)),
        ("speed", json!(null)),
        ("end_on_source_invalid", json!(false)),
        ("pulse_cost", json!(0)),
        ("unknown", json!(null)),
        ("on_hit_target_statuses", json!(["renewal"])),
        ("on_commit_self_statuses", json!(["missing"])),
    ] {
        let mut v = source();
        v["skill"][k] = x;
        p.write(&v);
        assert!(p.load().is_err(), "{k}");
    }
}
#[test]
fn field_skill_config_requires_duration_interval_and_128_tick_cap() {
    let p = Package::new();
    for k in ["duration_ms", "tick_interval_ms", "warning_ms"] {
        let mut v = source();
        v["skill"].as_object_mut().unwrap().remove(k);
        p.write(&v);
        assert!(p.load().is_err());
    }
    let mut v = source();
    v["skill"]["tick_interval_ms"] = json!(30);
    v["skill"]["duration_ms"] = json!(3870);
    v["skill"]["warning_ms"] = json!(0);
    p.write(&v);
    assert!(p.load().is_ok());
    v["skill"]["duration_ms"] = json!(3871);
    p.write(&v);
    assert!(p.load().is_err());
}
#[test]
fn field_skill_old_19_to_29_normalized_shapes_omit_periodic_fields() {
    let p = Package::new();
    for sample in [
        "status",
        "stack",
        "attribute",
        "shield",
        "dot",
        "hot",
        "heal-modifier",
        "dual-skill",
        "self-skill",
        "unit-skill",
        "ground-skill",
    ] {
        let v: Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                    .join(format!("examples/{sample}-host/arena.json")),
            )
            .unwrap(),
        )
        .unwrap();
        p.write(&v);
        let n = p.load().unwrap().describe()["versions"][0]["config"].clone();
        for k in ["duration_ms", "tick_interval_ms"] {
            assert!(n["skill"].get(k).is_none());
        }
        assert_eq!(n["skill"]["mechanism"], v["skill"]["mechanism"]);
    }
}

#[test]
fn field_skill_zero_step_and_interval_reject_without_division_or_panic() {
    let p = Package::new();
    let mut v = source();
    v["step_ms"] = json!(0);
    v["skill"]["tick_interval_ms"] = json!(0);
    p.write(&v);
    assert!(p.load().is_err());
}
