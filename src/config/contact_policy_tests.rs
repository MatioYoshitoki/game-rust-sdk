use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/contact-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn contact_policy_config_both_slots_normalize_and_report_capability() {
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
        let skill = if swap {
            &n["additional_skills"][0]
        } else {
            &n["skill"]
        };
        assert_eq!(skill["contact_policy"]["max_total_hits"], 3);
        p.write(&n);
        assert!(p.load().is_ok());
    }
    assert_eq!(
        crate::sdk_info()["playerSkills"]["contactPolicy"]["repeatClock"],
        "scheduled_at_ms"
    );
}
#[test]
fn contact_policy_config_requires_version_schema_pve_and_field() {
    let p = Package::new();
    for m in 1..31 {
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
    for mechanism in [
        "projectile",
        "unit_projectile",
        "self_status",
        "ground_circle",
    ] {
        let mut v = source();
        v["skill"]["mechanism"] = json!(mechanism);
        p.write(&v);
        assert!(p.load().is_err());
    }
}
#[test]
fn contact_policy_config_strict_bounds_types_and_required_fields() {
    let p = Package::new();
    for (k, x) in [
        ("max_targets_per_tick", json!(0)),
        ("max_targets_per_tick", json!(33)),
        ("max_targets_per_tick", json!(1.5)),
        ("max_total_hits", json!(0)),
        ("max_total_hits", json!(129)),
        ("max_total_hits", json!(-1)),
        ("repeat_interval_ms", json!(-1)),
        ("repeat_interval_ms", json!(60001)),
        ("repeat_interval_ms", json!(0.5)),
        ("unknown", json!(null)),
    ] {
        let mut v = source();
        v["skill"]["contact_policy"][k] = x;
        p.write(&v);
        assert!(p.load().is_err(), "{k}");
    }
    for k in [
        "max_targets_per_tick",
        "max_total_hits",
        "repeat_interval_ms",
    ] {
        let mut v = source();
        v["skill"]["contact_policy"]
            .as_object_mut()
            .unwrap()
            .remove(k);
        p.write(&v);
        assert!(p.load().is_err());
    }
    for x in [json!(null), json!([]), json!({}), json!(true)] {
        let mut v = source();
        v["skill"]["contact_policy"] = x;
        p.write(&v);
        assert!(p.load().is_err());
    }
}
#[test]
fn contact_policy_config_zero_repeat_and_maxima_are_legal() {
    let p = Package::new();
    for repeat in [0, 60000] {
        let mut v = source();
        v["skill"]["contact_policy"] =
            json!({"max_targets_per_tick":32,"max_total_hits":128,"repeat_interval_ms":repeat});
        p.write(&v);
        assert!(p.load().is_ok());
    }
}
#[test]
fn contact_policy_config_omission_preserves_old_field_and_normalized_shape() {
    let p = Package::new();
    let mut v = source();
    v["skill"].as_object_mut().unwrap().remove("contact_policy");
    for m in [30, 31] {
        v["mechanism_version"] = json!(m);
        p.write(&v);
        let n = p.load().unwrap().describe()["versions"][0]["config"].clone();
        assert!(n["skill"].get("contact_policy").is_none());
    }
}
#[test]
fn contact_policy_config_old_19_to_30_keep_shapes() {
    let p = Package::new();
    for sample in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
        "hot-host",
        "heal-modifier-host",
        "dual-skill-host",
        "self-skill-host",
        "unit-skill-host",
        "ground-skill-host",
        "field-host",
    ] {
        let v: Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                    .join(format!("examples/{sample}/arena.json")),
            )
            .unwrap(),
        )
        .unwrap();
        p.write(&v);
        let n = p.load().unwrap().describe()["versions"][0]["config"].clone();
        assert!(n["skill"].get("contact_policy").is_none());
        assert_eq!(n["skill"]["mechanism"], v["skill"]["mechanism"]);
    }
}

#[test]
fn contact_policy_does_not_bypass_skill_identity_or_status_reference_checks() {
    let p = Package::new();
    for (k, x) in [
        ("id", json!("")),
        ("id", json!("renewing-ward")),
        ("on_hit_target_statuses", json!(["missing"])),
        ("on_hit_target_statuses", json!(["renewal"])),
        ("on_commit_self_statuses", json!(["missing"])),
    ] {
        let mut v = source();
        v["skill"][k] = x;
        p.write(&v);
        assert!(p.load().is_err());
    }
}
