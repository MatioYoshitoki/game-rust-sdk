use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/ground-skill-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn ground_skill_config_roundtrips_both_slots_and_support() {
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
        assert_eq!(s["mechanism"], "ground_circle");
        assert!(s.get("speed").is_none());
        p.write(&n);
        assert!(p.load().is_ok());
    }
    assert_eq!(
        crate::sdk_info()["playerSkills"]["groundCircle"]["maxPendingAreas"],
        2
    );
}
#[test]
fn ground_skill_config_requires_capability_schema_and_pve() {
    let p = Package::new();
    for m in 1..29 {
        let mut v = source();
        v["mechanism_version"] = json!(m);
        p.write(&v);
        assert!(p.load().is_err());
    }
    for key in ["pve", "schema_version"] {
        let mut v = source();
        v[key] = json!(null);
        p.write(&v);
        assert!(p.load().is_err());
    }
}
#[test]
fn ground_skill_config_rejects_bounds_mixed_and_unknown_fields() {
    let p = Package::new();
    for (k, x) in [
        ("radius", json!(0)),
        ("radius", json!(129)),
        ("range", json!(0)),
        ("range", json!(10001)),
        ("warning_ms", json!(60001)),
        ("warning_ms", json!(-1)),
        ("warning_ms", json!(1.5)),
        ("cost", json!(1000001)),
        ("damage_bps", json!(100001)),
        ("windup_ms", json!(3600001)),
        ("speed", json!(null)),
        ("target", json!(null)),
        ("position", json!([0, 0])),
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
fn ground_skill_config_required_fields_and_zero_time_boundaries() {
    let p = Package::new();
    for k in ["radius", "range", "damage_bps", "warning_ms"] {
        let mut v = source();
        v["skill"].as_object_mut().unwrap().remove(k);
        p.write(&v);
        assert!(p.load().is_err());
    }
    let mut v = source();
    for k in [
        "cost",
        "windup_ms",
        "recovery_ms",
        "cooldown_ms",
        "warning_ms",
        "damage_bps",
    ] {
        v["skill"][k] = json!(0);
    }
    p.write(&v);
    assert!(p.load().is_ok());
}
#[test]
fn ground_skill_wire_strict_point_and_extra_fields() {
    use crate::protocol::Action;
    let v = json!({"kind":"cast_ground","skill_id":"ground-burst","position":[400,400]});
    assert!(serde_json::from_value::<Action>(v.clone()).is_ok());
    for x in [
        json!(null),
        json!([]),
        json!([1]),
        json!([1, 2, 3]),
        json!(["x", 2]),
        json!({"x":1,"y":2}),
    ] {
        let mut v = v.clone();
        v["position"] = x;
        assert!(serde_json::from_value::<Action>(v).is_err());
    }
    for k in ["direction", "target", "cost", "damage"] {
        let mut v = v.clone();
        v[k] = json!(null);
        assert!(serde_json::from_value::<Action>(v).is_err());
    }
}
#[test]
fn ground_skill_old_19_to_28_normalized_shapes_keep_no_warning_fields() {
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
        assert!(n["skill"].get("warning_ms").is_none());
        assert_eq!(n["skill"]["mechanism"], v["skill"]["mechanism"]);
    }
}
