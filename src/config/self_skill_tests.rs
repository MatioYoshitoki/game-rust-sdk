use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/self-skill-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn self_skill_configuration_roundtrips_both_slot_orders_without_projectile_fields() {
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
        let normalized = c.describe()["versions"][0]["config"].clone();
        let s = if swap {
            &normalized["skill"]
        } else {
            &normalized["additional_skills"][0]
        };
        assert_eq!(s["mechanism"], "self_status");
        for field in [
            "range",
            "damage_bps",
            "speed",
            "radius",
            "on_hit_target_statuses",
        ] {
            assert!(s.get(field).is_none());
        }
        p.write(&normalized);
        assert!(p.load().is_ok());
    }
    assert_eq!(
        crate::sdk_info()["playerSkills"]["selfStatus"]["minimumMechanismVersion"],
        27
    );
}
#[test]
fn self_skill_configuration_rejects_every_projectile_and_unknown_field() {
    let p = Package::new();
    for field in [
        "range",
        "speed",
        "radius",
        "damage_bps",
        "on_hit_target_statuses",
        "direction",
        "target",
        "unknown",
    ] {
        for value in [json!(null), json!(0), json!([])] {
            let mut v = source();
            v["additional_skills"][0][field] = value;
            p.write(&v);
            assert!(p.load().is_err(), "{field}");
        }
    }
}
#[test]
fn self_skill_configuration_is_gated_by_version_schema_and_pve() {
    let p = Package::new();
    for version in 1..27 {
        let mut v = source();
        v["mechanism_version"] = json!(version);
        p.write(&v);
        assert!(p.load().is_err(), "{version}");
    }
    for case in 0..3 {
        let mut v = source();
        match case {
            0 => v["schema_version"] = json!(3),
            1 => v["pve"] = json!(null),
            _ => {
                v.as_object_mut().unwrap().remove("pve");
            }
        }
        p.write(&v);
        assert!(p.load().is_err());
    }
}
#[test]
fn self_skill_configuration_requires_nonempty_unique_resolved_legal_bindings() {
    let p = Package::new();
    for binding in [
        json!(null),
        json!([]),
        json!(["missing"]),
        json!(["ward", "ward"]),
        json!(["ward", "renewal", "ward"]),
        json!("ward"),
    ] {
        let mut v = source();
        v["additional_skills"][0]["on_commit_self_statuses"] = binding;
        p.write(&v);
        assert!(p.load().is_err());
    }
    let mut v = source();
    v["additional_skills"][0]
        .as_object_mut()
        .unwrap()
        .remove("on_commit_self_statuses");
    p.write(&v);
    assert!(p.load().is_err());
    let dot: Value =
        serde_json::from_str(crate::test_data::fixture!("examples/dot-host/arena.json")).unwrap();
    v = source();
    v["pve"]["status_definitions"] = dot["pve"]["status_definitions"].clone();
    v["additional_skills"][0]["on_commit_self_statuses"] = json!(["wound"]);
    p.write(&v);
    assert!(p.load().is_err());
}
#[test]
fn self_skill_configuration_bounds_counts_and_ids_remain_strict() {
    let p = Package::new();
    for (field, value) in [
        ("cost", json!(1000001)),
        ("cost", json!(-1)),
        ("cost", json!(1.5)),
        ("cost", json!(null)),
        ("windup_ms", json!(3600001)),
        ("recovery_ms", json!(3600001)),
        ("cooldown_ms", json!(3600001)),
        ("id", json!("arc-bolt")),
        ("id", json!("")),
        ("id", json!("x".repeat(65))),
        ("id", json!("a\nb")),
        ("mechanism", json!("self_unknown")),
    ] {
        let mut v = source();
        v["additional_skills"][0][field] = value;
        p.write(&v);
        assert!(p.load().is_err(), "{field}");
    }
    let mut v = source();
    let extra = v["skill"].clone();
    v["additional_skills"].as_array_mut().unwrap().push(extra);
    p.write(&v);
    assert!(p.load().is_err());
}
#[test]
fn self_skill_configuration_all_self_primary_only_and_zero_timing_are_valid() {
    let p = Package::new();
    let mut v = source();
    let mut self_skill = v["additional_skills"][0].clone();
    for field in ["cost", "windup_ms", "recovery_ms", "cooldown_ms"] {
        self_skill[field] = json!(0);
    }
    v["skill"] = self_skill;
    v["skill"]["id"] = json!("primary-self");
    p.write(&v);
    assert!(p.load().is_ok());
    v.as_object_mut().unwrap().remove("additional_skills");
    p.write(&v);
    assert!(p.load().is_ok());
}
#[test]
fn self_skill_wire_input_rejects_aim_targets_effects_and_amounts() {
    use crate::protocol::Action;
    assert!(matches!(
        serde_json::from_value::<Action>(json!({"kind":"cast_self","skill_id":"renewing-ward"}))
            .unwrap(),
        Action::CastSelf { .. }
    ));
    for field in [
        "direction",
        "target_id",
        "position",
        "hp",
        "cost",
        "statuses",
    ] {
        let mut v = json!({"kind":"cast_self","skill_id":"renewing-ward"});
        v[field] = json!(null);
        assert!(serde_json::from_value::<Action>(v).is_err());
    }
    for value in [json!(null), json!(1)] {
        assert!(
            serde_json::from_value::<Action>(json!({"kind":"cast_self","skill_id":value})).is_err()
        );
    }
}
#[test]
fn self_skill_old_projectile_shape_and_required_parameters_remain() {
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
    ] {
        let mut v: Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                    .join(format!("examples/{sample}/arena.json")),
            )
            .unwrap(),
        )
        .unwrap();
        p.write(&v);
        let c = p.load().unwrap();
        let n = c.describe()["versions"][0]["config"].clone();
        assert_eq!(n["skill"]["mechanism"], "projectile");
        assert!(n["skill"].get("range").is_some());
        assert!(n["skill"].get("target_type").is_none());
        v["skill"].as_object_mut().unwrap().remove("range");
        p.write(&v);
        assert!(p.load().is_err());
    }
}
