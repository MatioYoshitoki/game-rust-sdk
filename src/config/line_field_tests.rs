use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/line-field-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn line_field_config_both_slots_normalize_public_geometry_and_policy() {
    let p = Package::new();
    for swap in [false, true] {
        let mut v = source();
        if swap {
            let a = v["skill"].take();
            v["skill"] = v["additional_skills"][0].take();
            v["additional_skills"][0] = a;
        }
        p.write(&v);
        let n = p.load().unwrap().describe()["versions"][0]["config"].clone();
        let s = if swap {
            &n["additional_skills"][0]
        } else {
            &n["skill"]
        };
        assert_eq!(s["length"], 260.);
        assert_eq!(s["width"], 48.);
        assert_eq!(s["contact_policy"], source()["skill"]["contact_policy"]);
        for k in ["speed", "range", "radius"] {
            assert!(s.get(k).is_none());
        }
        p.write(&n);
        assert!(p.load().is_ok());
    }
    assert_eq!(
        crate::sdk_info()["playerSkills"]["lineField"]["geometry"],
        "fixed_capsule"
    );
}
#[test]
fn line_field_config_old_versions_schema_and_pve_rejected() {
    let p = Package::new();
    for m in 1..34 {
        let mut v = source();
        v["mechanism_version"] = json!(m);
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
fn line_field_config_finite_geometry_inclusive_bounds_and_strict_types() {
    let p = Package::new();
    for (k, x) in [
        ("length", json!(0.09)),
        ("length", json!(10000.1)),
        ("width", json!(0.19)),
        ("width", json!(256.1)),
        ("width", json!(-1)),
        ("length", json!("260")),
        ("length", json!(null)),
        ("width", json!(true)),
        ("contact_policy", json!(null)),
    ] {
        let mut v = source();
        v["skill"][k] = x;
        p.write(&v);
        assert!(p.load().is_err(), "{k}");
    }
    for (length, width) in [(0.1, 0.2), (10000., 256.)] {
        let mut v = source();
        v["skill"]["length"] = json!(length);
        v["skill"]["width"] = json!(width);
        p.write(&v);
        assert!(p.load().is_ok());
    }
    for raw in ["NaN", "1e999"] {
        let mut s = source().to_string();
        s = s.replace("\"length\":260", &format!("\"length\":{raw}"));
        assert!(serde_json::from_str::<Value>(&s).is_err());
    }
}
#[test]
fn line_field_config_required_and_incompatible_fields_reject() {
    let p = Package::new();
    for k in [
        "length",
        "width",
        "warning_ms",
        "duration_ms",
        "tick_interval_ms",
        "contact_policy",
    ] {
        let mut v = source();
        v["skill"].as_object_mut().unwrap().remove(k);
        p.write(&v);
        assert!(p.load().is_err(), "{k}");
    }
    for k in [
        "range",
        "radius",
        "speed",
        "repeat_interval_ms",
        "target",
        "end_on_source_invalid",
        "unknown",
    ] {
        let mut v = source();
        v["skill"][k] = json!(1);
        p.write(&v);
        assert!(p.load().is_err(), "{k}");
    }
}
#[test]
fn line_field_config_shared_tick_budget_and_binding_limits() {
    let p = Package::new();
    for (k, x) in [
        ("duration_ms", json!(600)),
        ("duration_ms", json!(60001)),
        ("tick_interval_ms", json!(29)),
        ("warning_ms", json!(60001)),
        ("on_hit_target_statuses", json!(["renewal"])),
        ("on_commit_self_statuses", json!(["missing"])),
    ] {
        let mut v = source();
        v["skill"][k] = x;
        p.write(&v);
        assert!(p.load().is_err(), "{k}");
    }
    for (k, x) in [
        ("max_targets_per_tick", 33),
        ("max_targets_per_tick", 0),
        ("max_total_hits", 129),
        ("repeat_interval_ms", 60001),
    ] {
        let mut v = source();
        v["skill"]["contact_policy"][k] = json!(x);
        p.write(&v);
        assert!(p.load().is_err());
    }
    let mut v = source();
    v["skill"]["tick_interval_ms"] = json!(30);
    v["skill"]["duration_ms"] = json!(3870);
    p.write(&v);
    assert!(p.load().is_ok());
    v["skill"]["duration_ms"] = json!(3871);
    p.write(&v);
    assert!(p.load().is_err());
}
