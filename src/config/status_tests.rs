use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/status-host/arena.json"
    ))
    .unwrap()
}

#[test]
fn status_schema_protocol_and_support_matrix_match_registered_capability() {
    let p = Package::new();
    p.write(&source());
    let c = p.load().unwrap();
    assert_eq!(c.default_config().protocol_version(), 3);
    assert!(
        crate::sdk_info()["mechanismVersions"]["max"]
            .as_u64()
            .unwrap()
            >= 19
    );
}
#[test]
fn status_invalid_fields_references_policies_and_ranges_fail_before_start() {
    let p = Package::new();
    for case in 0..25 {
        let mut v = source();
        match case {
            0 => v["mechanism_version"] = json!(18),
            1 => v["schema_version"] = json!(3),
            2 => {
                v.as_object_mut().unwrap().remove("pve");
            }
            3 => {
                v["pve"].as_object_mut().unwrap().remove("status_rules");
            }
            4 => v["pve"]["status_definitions"][0]["duplicate_policy"] = json!("stack"),
            5 => v["pve"]["status_definitions"][0]["mechanism"] = json!("move_speed_modifier"),
            6 => v["pve"]["status_definitions"][0]["id"] = json!(""),
            7 => v["pve"]["status_definitions"][0]["id"] = json!("x".repeat(65)),
            8 => v["pve"]["status_definitions"][0]["id"] = json!("bad\n"),
            9 => v["pve"]["status_definitions"][1]["id"] = json!("guard"),
            10 => v["pve"]["status_definitions"][0]["duration_ms"] = json!(0),
            11 => v["pve"]["status_definitions"][0]["duration_ms"] = json!(60001),
            12 => v["pve"]["status_definitions"][0]["modifier_bps"] = json!(-10001),
            13 => v["pve"]["status_definitions"][0]["modifier_bps"] = json!(100001),
            14 => v["pve"]["status_definitions"][0]["source_scoped"] = json!("true"),
            15 => v["pve"]["status_definitions"][0]["attribute"] = json!("hp"),
            16 => v["pve"]["status_rules"]["max_active_per_entity"] = json!(0),
            17 => v["pve"]["status_rules"]["defense_bonus_cap_bps"] = json!(-1),
            18 => v["skill"]["on_hit_target_statuses"] = json!(["absent"]),
            19 => v["skill"]["on_hit_target_statuses"] = json!(["exposed", "exposed"]),
            20 => v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!(["absent"]),
            21 => v["skill"]["on_timer_statuses"] = json!(["guard"]),
            22 => {
                v["mechanism_version"] = json!(18);
                v["skill"]["on_commit_self_statuses"] = json!([]);
                v["skill"]["on_hit_target_statuses"] = json!([]);
                v["pve"]
                    .as_object_mut()
                    .unwrap()
                    .remove("status_definitions");
                v["pve"].as_object_mut().unwrap().remove("status_rules");
            }
            23 => v["pve"]["status_definitions"] = json!([]),
            _ => v["pve"]["status_rules"]["max_active_per_entity"] = json!(33),
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn status_publication_budget_is_checked_for_all_reserved_entities() {
    let p = Package::new();
    let mut v = source();
    v["pve"]["status_rules"]["max_active_per_entity"] = json!(32);
    v["pve"]["monster_positions"] = json!([[600., 400.], [720., 400.], [840., 400.], [960., 400.]]);
    p.write(&v);
    assert!(
        p.load()
            .err()
            .unwrap()
            .to_string()
            .contains("CONFIG_STATUS_PUBLICATION_BUDGET")
    );
    v["pve"]["monster_positions"] = json!([[600., 400.], [720., 400.], [840., 400.]]);
    p.write(&v);
    assert!(p.load().is_ok());
}
#[test]
fn status_public_actions_have_no_direct_state_mutation_or_clock_input() {
    for value in [
        json!({"kind":"apply_status","id":"guard"}),
        json!({"kind":"cast","skill_id":"x","direction":[1,0],"modifier_bps":10000}),
        json!({"kind":"expire_status","instance_id":"status-1"}),
        json!({"kind":"move","direction":[1,0],"def":999}),
    ] {
        assert!(serde_json::from_value::<crate::protocol::Action>(value).is_err());
    }
}
