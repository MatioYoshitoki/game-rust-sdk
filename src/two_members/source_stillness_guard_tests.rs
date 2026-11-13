use super::*;

const SKILL: &str = "lin-mi";
const BASE: &str = "effect_lin_secret";
const PERFECT: &str = "effect_lin_secret_stage_5";
const COST: u32 = 1_800_000;
const SPIRIT: u32 = 2_000_000;

fn catalog(change: impl FnOnce(&mut Value)) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    catalog_with(|v| {
        *v = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/mastery-stages.json"
        ))
        .unwrap();
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/source-lin-mi.json"
        ))
        .unwrap();
        v["additional_skills"]
            .as_array_mut()
            .unwrap()
            .push(sample["skill"].clone());
        v["pve"]["status_rules"]["attribute_caps_bps"]["damage_reduction"] =
            sample["damage_reduction_cap_bps"].clone();
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
        change(v);
    })
}

fn host(stages: [Option<usize>; 2], change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let cat = catalog(change).unwrap();
    let mut request = create(&cat.default_config());
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
        store.hp = 1000;
        store.spirit = SPIRIT;
    }
    h.active();
    h
}

fn cast(h: &mut DeltaHarness, i: usize) {
    h.action(
        i,
        Action::CastSelf {
            skill_id: SKILL.into(),
        },
    )
    .unwrap();
    h.pump();
}

#[test]
fn source_stillness_guard_commits_five_stages_with_matching_deadlines() {
    for stages in [[Some(0), Some(1)], [Some(2), Some(3)], [Some(4), None]] {
        let mut h = host(stages, |_| {});
        for (i, stage) in stages.into_iter().enumerate() {
            cast(&mut h, i);
            let stage = stage.unwrap_or(0);
            let id = if stage == 0 {
                BASE.into()
            } else {
                format!("{BASE}_stage_{}", stage + 1)
            };
            let own = &h.snapshot(i).own;
            let s = own.statuses.iter().find(|s| s.definition_id == id).unwrap();
            assert_eq!(s.source, s.target);
            assert_eq!(s.expires_at_ms, if stage == 0 { 5000 } else { 6000 });
            assert!(s.has_attribute(StatusAttribute::ControlImmunity));
            assert_eq!(s.modifiers[0].modifier_bps, 7000);
            assert_eq!(own.spirit, SPIRIT - COST);
            assert_eq!(
                own.player_skills
                    .iter()
                    .find(|s| s.skill_id == SKILL)
                    .unwrap()
                    .ready_at_ms,
                if stage == 0 { 40000 } else { 36000 }
            );
        }
        h.ticks(200);
        for (i, stage) in stages.into_iter().enumerate() {
            let lingering = h
                .snapshot(i)
                .own
                .statuses
                .iter()
                .find(|s| s.definition_id == "effect_lin_linger");
            assert_eq!(
                lingering.map(|s| (s.modifier_bps, s.expires_at_ms)),
                stage.filter(|m| *m >= 3).map(|_| (2000, 8000))
            );
        }
    }
}

#[test]
fn source_stillness_guard_catalog_validates_cooldown_rows_and_guard_leaves() {
    catalog(|_| {}).unwrap();
    for rows in [
        json!([]),
        json!([{"minimum_mastery":1,"reduction_bps":10001}]),
        json!([{"minimum_mastery":5,"reduction_bps":1000}]),
        json!([{"minimum_mastery":1,"reduction_bps":1000},{"minimum_mastery":1,"reduction_bps":2000}]),
    ] {
        assert!(
            catalog(|v| v["combat_rules"]["skill_cooldowns_by_mastery"][SKILL] = rows).is_err()
        );
    }
    for (key, value) in [
        ("cooldown_ms", json!(119999)),
        ("cooldown_group", json!("bad group")),
        (
            "guard",
            json!({"retain_hp":1,"heal_max_hp_bps":0,"self_statuses":[PERFECT]}),
        ),
    ] {
        assert!(
            catalog(|v| v["combat_rules"]["status_death_guards"][PERFECT][key] = value).is_err(),
            "{key}"
        );
    }
    assert!(
        catalog(|v| v["combat_rules"]["status_policies"][PERFECT]["transferable"] = json!(true))
            .is_err()
    );
}

#[test]
fn source_stillness_guard_combines_mastery_and_live_cooldown_multiplicatively() {
    let mut h = host([Some(4), None], |v| {
        v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({
            "id":"test-cd","mechanism":"timed_attribute_modifier","attribute":"cooldown_reduction",
            "modifier_bps":2000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true
        }));
        let tag = v["additional_skills"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|s| s["id"] == "tag-debuff")
            .unwrap();
        tag["on_commit_self_statuses"] = json!(["test-cd"]);
    });
    h.action(
        0,
        Action::CastSelf {
            skill_id: "tag-debuff".into(),
        },
    )
    .unwrap();
    h.pump();
    h.ticks(4);
    cast(&mut h, 0);
    let deadline = h
        .snapshot(0)
        .own
        .player_skills
        .iter()
        .find(|s| s.skill_id == SKILL)
        .unwrap()
        .ready_at_ms;
    assert_eq!(deadline, h.engine.now + 28800);
    h.ticks(30);
    assert_eq!(
        h.snapshot(0)
            .own
            .player_skills
            .iter()
            .find(|s| s.skill_id == SKILL)
            .unwrap()
            .ready_at_ms,
        deadline
    );
}
