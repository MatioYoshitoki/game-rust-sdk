use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/piercing-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn piercing_config_both_slots_normalize_and_capabilities() {
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
        assert_eq!(s["max_hits"], 3);
        assert_eq!(s["ttl_ms"], 3000);
        assert!(s.get("contact_policy").is_none());
        p.write(&n);
        assert!(p.load().is_ok());
    }
    assert_eq!(
        crate::sdk_info()["playerSkills"]["piercingProjectile"]["minimumMechanismVersion"],
        32
    );
}
#[test]
fn piercing_config_version_schema_pve_gates() {
    let p = Package::new();
    for m in 1..32 {
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
fn piercing_config_strict_bounds_types_required_and_no_dormant_fields() {
    let p = Package::new();
    for (k, x) in [
        ("ttl_ms", json!(0)),
        ("ttl_ms", json!(59)),
        ("ttl_ms", json!(60001)),
        ("ttl_ms", json!(60.5)),
        ("max_hits", json!(0)),
        ("max_hits", json!(129)),
        ("max_hits", json!(-1)),
        ("max_hits", json!(1.5)),
        ("radius", json!(129)),
        ("speed", json!(0)),
        ("range", json!(0)),
        ("contact_policy", json!(null)),
        ("warning_ms", json!(0)),
        ("duration_ms", json!(0)),
        ("target", json!({})),
        ("unknown", json!(null)),
    ] {
        let mut v = source();
        v["skill"][k] = x;
        p.write(&v);
        assert!(p.load().is_err(), "{k}");
    }
    for k in [
        "ttl_ms", "max_hits", "speed", "range", "radius", "cost", "id",
    ] {
        for null in [false, true] {
            let mut v = source();
            if null {
                v["skill"][k] = json!(null);
            } else {
                v["skill"].as_object_mut().unwrap().remove(k);
            }
            p.write(&v);
            assert!(p.load().is_err(), "{k}");
        }
    }
}
#[test]
fn piercing_config_inclusive_minima_and_maxima() {
    let p = Package::new();
    for (t, h) in [(60, 1), (60000, 128)] {
        let mut v = source();
        v["skill"]["ttl_ms"] = json!(t);
        v["skill"]["max_hits"] = json!(h);
        p.write(&v);
        assert!(p.load().is_ok());
    }
}
#[test]
fn piercing_config_bindings_and_duplicate_ids_remain_strict() {
    let p = Package::new();
    for (k, x) in [
        ("on_commit_self_statuses", json!(["missing"])),
        ("on_hit_target_statuses", json!(["renewal"])),
        ("id", source()["additional_skills"][0]["id"].clone()),
    ] {
        let mut v = source();
        v["skill"][k] = x;
        p.write(&v);
        assert!(p.load().is_err());
    }
}
