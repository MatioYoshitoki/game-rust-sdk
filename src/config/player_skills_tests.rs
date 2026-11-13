use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/dual-skill-host/arena.json"
    ))
    .unwrap()
}
#[test]
fn multi_config_normalizes_and_reloads_both_skills() {
    let p = Package::new();
    p.write(&source());
    let c = p.load().unwrap();
    let v = c.describe()["versions"][0]["config"].clone();
    assert_eq!(v["additional_skills"][0]["id"], "renewal-bolt");
    assert_eq!(
        v["additional_skills"][0]["on_commit_self_statuses"],
        json!(["renewal"])
    );
    p.write(&v);
    assert!(p.load().is_ok());
    assert_eq!(crate::sdk_info()["playerSkills"]["maxSkills"], 2);
}
#[test]
fn multi_config_rejects_presence_count_version_schema_and_no_pve() {
    let p = Package::new();
    for rows in [
        json!(null),
        json!([]),
        json!({}),
        json!([null]),
        json!([{}, {}]),
    ] {
        let mut v = source();
        v["additional_skills"] = rows;
        p.write(&v);
        assert!(p.load().is_err());
    }
    for case in 0..5 {
        let mut v = source();
        match case {
            0 => v["mechanism_version"] = json!(25),
            1 => v["schema_version"] = json!(3),
            2 => v["pve"] = json!(null),
            3 => {
                v.as_object_mut().unwrap().remove("additional_skills");
                v["pve"] = json!(null);
            }
            _ => {
                v.as_object_mut().unwrap().remove("additional_skills");
                v["schema_version"] = json!(2);
            }
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn multi_config_validates_each_definition_and_unique_ids() {
    let p = Package::new();
    for (field, value) in [
        ("id", json!("ward-bolt")),
        ("id", json!("")),
        ("id", json!("x".repeat(65))),
        ("id", json!("a\nb")),
        ("cost", json!(1000001)),
        ("windup_ms", json!(3600001)),
        ("recovery_ms", json!(3600001)),
        ("cooldown_ms", json!(3600001)),
        ("damage_bps", json!(100001)),
        ("range", json!(0)),
        ("speed", json!(10001)),
        ("radius", json!(129)),
        ("mechanism", json!("instant_heal")),
        ("unknown", json!(null)),
        ("cost", json!(-1)),
        ("cost", json!(1.5)),
    ] {
        let mut v = source();
        v["additional_skills"][0][field] = value;
        p.write(&v);
        assert!(p.load().is_err(), "{field}");
    }
}
#[test]
fn multi_config_secondary_bindings_obey_scope_references_and_capacity() {
    let p = Package::new();
    for (field, value) in [
        ("on_hit_target_statuses", json!(["renewal"])),
        ("on_commit_self_statuses", json!(["missing"])),
        ("on_commit_self_statuses", json!(["renewal", "renewal"])),
        ("on_commit_self_statuses", json!(null)),
    ] {
        let mut v = source();
        v["additional_skills"][0][field] = value;
        p.write(&v);
        assert!(p.load().is_err(), "{field}");
    }
    let mut v = source();
    v["pve"]["status_rules"]["max_active_per_entity"] = json!(1);
    v["additional_skills"][0]["on_commit_self_statuses"] = json!(["renewal", "ward"]);
    p.write(&v);
    assert!(p.load().is_ok()); // Runtime atomically rejects a grant that cannot fit.
}
#[test]
fn multi_config_zero_timings_cost_and_single_skill_are_valid() {
    let p = Package::new();
    let mut v = source();
    for field in [
        "windup_ms",
        "recovery_ms",
        "cooldown_ms",
        "cost",
        "damage_bps",
        "radius",
    ] {
        v["additional_skills"][0][field] = json!(0);
    }
    p.write(&v);
    assert!(p.load().is_ok());
    v.as_object_mut().unwrap().remove("additional_skills");
    p.write(&v);
    assert!(p.load().is_ok());
}
#[test]
fn multi_config_old_19_to_25_omit_new_field_and_reject_opt_in() {
    let p = Package::new();
    for name in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
        "hot-host",
        "heal-modifier-host",
    ] {
        let mut v: Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                    .join(format!("examples/{name}/arena.json")),
            )
            .unwrap(),
        )
        .unwrap();
        p.write(&v);
        assert!(
            p.load().unwrap().describe()["versions"][0]["config"]
                .get("additional_skills")
                .is_none()
        );
        v["additional_skills"] = source()["additional_skills"].clone();
        p.write(&v);
        assert!(p.load().is_err());
    }
}
