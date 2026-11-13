use super::*;

const SKILL: &str = "xianwang-jiufeng";
const BASE: &str = "effect_xianwang_seal";
const COST: u32 = 4_000_000;
const SPIRIT: u32 = 6_000_000;

fn catalog(change: impl FnOnce(&mut Value)) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    catalog_with(|v| {
        *v = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/mastery-stages.json"
        ))
        .unwrap();
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/source-xianwang-jiufeng.json"
        ))
        .unwrap();
        v["additional_skills"]
            .as_array_mut()
            .unwrap()
            .push(sample["skill"].clone());
        v["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .extend(
                sample["status_definitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
        for (key, entries) in sample["combat_rules"].as_object().unwrap() {
            let table = &mut v["combat_rules"][key];
            if table.is_null() {
                *table = json!({});
            }
            table
                .as_object_mut()
                .unwrap()
                .extend(entries.as_object().unwrap().clone());
        }
        v["resource_limits"]["max_spirit"] = json!(SPIRIT);
        v["resource_limits"]["max_spirit_cost"] = json!(COST);
        change(v);
    })
}

#[test]
fn source_retaliation_five_stages_share_skill_identity_and_cost() {
    for stages in [[Some(0), Some(1)], [Some(2), Some(3)], [Some(4), None]] {
        let cat = catalog(|_| {}).unwrap();
        let config = cat.default_config();
        let mut request = create(&config);
        request.protocol_version = 6;
        for (seat, stage) in request.participants.iter_mut().zip(stages) {
            seat.combat_build = stage.map(|mastery| CombatBuild {
                power_points: 0,
                skill_power_points: 0,
                element_resistance_points: 0,
                powers: BTreeMap::from([(
                    SKILL.into(),
                    PowerGrowth {
                        model_id: "mastery-demo".into(),
                        rank: 0,
                        quality: 0,
                        level: 0,
                        mastery,
                    },
                )]),
            });
        }
        let mut h = DeltaHarness::from_request(cat, request);
        for store in &mut h.stores {
            store.spirit = SPIRIT;
        }
        h.active();
        for (i, stage) in stages.into_iter().enumerate() {
            h.action(
                i,
                Action::CastSelf {
                    skill_id: SKILL.into(),
                },
            )
            .unwrap();
            h.pump();
            let stage = stage.unwrap_or(0);
            let id = if stage == 0 {
                BASE.into()
            } else {
                format!("{BASE}_stage_{}", stage + 1)
            };
            let own = &h.snapshot(i).own;
            let carrier = own.statuses.iter().find(|s| s.definition_id == id).unwrap();
            assert_eq!(carrier.source, carrier.target);
            assert_eq!(carrier.source.id, own.participant.entity.id);
            assert_eq!(carrier.expires_at_ms, 8000);
            assert_eq!(carrier.modifier_bps, if stage >= 3 { 2500 } else { 0 });
            assert_eq!(own.spirit, SPIRIT - COST);
            assert_eq!(
                own.player_skills
                    .iter()
                    .find(|s| s.skill_id == SKILL)
                    .unwrap()
                    .ready_at_ms,
                60000
            );
            let rule = &config.combat_rules.as_ref().unwrap().status_retaliations[&id];
            assert_eq!(rule.chance_bps, 4000);
            assert_eq!(
                rule.source_cooldown_ms,
                if stage == 0 { 5000 } else { 4000 }
            );
            assert_eq!(rule.dispel_beneficial, usize::from(stage >= 2));
            assert_eq!(rule.on_expired_status_id.is_some(), stage == 4);
        }
    }
}

#[test]
fn source_retaliation_validates_native_limits_and_dependencies() {
    assert!(catalog(|_| {}).is_ok());
    for (key, value) in [
        ("chance_bps", json!(10001)),
        ("source_cooldown_ms", json!(29)),
        ("max_sources", json!(0)),
        ("max_sources", json!(33)),
        ("dispel_beneficial", json!(17)),
        ("reply_status_id", json!("missing")),
        ("on_expired_status_id", json!(BASE)),
        ("on_expired_status_id", Value::Null),
        ("unknown", json!(true)),
    ] {
        assert!(
            catalog(|v| v["combat_rules"]["status_retaliations"][BASE][key] = value).is_err(),
            "{key}"
        );
    }
    assert!(
        catalog(|v| {
            v["combat_rules"]["status_policies"][BASE]["transferable"] = json!(true);
        })
        .is_err()
    );
}
