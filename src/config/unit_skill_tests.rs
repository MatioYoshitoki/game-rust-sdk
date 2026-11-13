use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/unit-skill-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn unit_skill_config_roundtrips_both_slots_and_support() {
    let p = Package::new();
    for swap in [false, true] {
        let mut v = source();
        if swap {
            let s = v["skill"].take();
            v["skill"] = v["additional_skills"][0].take();
            v["additional_skills"][0] = s;
        }
        p.write(&v);
        let c = p.load().unwrap();
        let n = c.describe()["versions"][0]["config"].clone();
        assert_eq!(
            if swap {
                &n["additional_skills"][0]["mechanism"]
            } else {
                &n["skill"]["mechanism"]
            },
            "unit_projectile"
        );
        p.write(&n);
        assert!(p.load().is_ok());
    }
    assert_eq!(
        crate::sdk_info()["playerSkills"]["unitProjectile"]["minimumMechanismVersion"],
        28
    );
}
#[test]
fn unit_skill_config_requires_capability_schema_and_pve() {
    let p = Package::new();
    for version in 1..28 {
        let mut v = source();
        v["mechanism_version"] = json!(version);
        p.write(&v);
        assert!(p.load().is_err());
    }
    for case in 0..2 {
        let mut v = source();
        if case == 0 {
            v["pve"] = json!(null);
        } else {
            v["schema_version"] = json!(3);
        }
        p.write(&v);
        assert!(p.load().is_err());
    }
}
#[test]
fn unit_skill_config_rejects_bad_bounds_mixed_fields_and_references() {
    let p = Package::new();
    for (k, v) in [
        ("cost", json!(1000001)),
        ("cost", json!(-1)),
        ("cost", json!(1.5)),
        ("range", json!(0)),
        ("range", json!(10001)),
        ("speed", json!(0)),
        ("radius", json!(129)),
        ("damage_bps", json!(100001)),
        ("windup_ms", json!(3600001)),
        ("id", json!("")),
        ("id", json!("renewing-ward")),
        ("id", json!("x".repeat(65))),
        ("on_hit_target_statuses", json!(["missing"])),
        ("on_hit_target_statuses", json!(["renewal"])),
        ("target", json!(null)),
        ("target_policy", json!("homing")),
        ("retarget", json!(true)),
        ("unknown", json!(null)),
    ] {
        let mut cfg = source();
        cfg["skill"][k] = v;
        p.write(&cfg);
        assert!(p.load().is_err(), "{k}");
    }
}
#[test]
fn unit_skill_config_projectile_fields_are_required_and_zero_timing_valid() {
    let p = Package::new();
    for k in ["range", "speed", "radius", "damage_bps"] {
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
        "damage_bps",
        "radius",
    ] {
        v["skill"][k] = json!(0);
    }
    p.write(&v);
    assert!(p.load().is_ok());
}
#[test]
fn unit_skill_wire_rejects_missing_unknown_and_wrong_target_types() {
    use crate::protocol::Action;
    assert!(
        serde_json::from_value::<Action>(
            json!({"kind":"cast_unit","skill_id":"marked-bolt","target":{"id":"a","generation":1}})
        )
        .is_ok()
    );
    for target in [
        json!(null),
        json!({}),
        json!({"id":"a"}),
        json!({"id":"a","generation":1,"position":[0,0]}),
        json!({"id":"a","generation":-1}),
        json!({"id":"a","generation":1.5}),
        json!({"id":"a","generation":4294967296u64}),
    ] {
        assert!(
            serde_json::from_value::<Action>(
                json!({"kind":"cast_unit","skill_id":"marked-bolt","target":target})
            )
            .is_err()
        );
    }
    for k in ["direction", "position", "cost", "damage", "relation"] {
        let mut v =
            json!({"kind":"cast_unit","skill_id":"marked-bolt","target":{"id":"a","generation":1}});
        v[k] = json!(null);
        assert!(serde_json::from_value::<Action>(v).is_err());
    }
}
#[test]
fn unit_skill_config_old_19_to_27_keep_their_shapes() {
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
        assert_eq!(n["skill"]["mechanism"], v["skill"]["mechanism"]);
        assert!(n["skill"].get("target_policy").is_none());
    }
}
