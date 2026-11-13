use super::combat::{DamageIntent, DirectContact};
use super::member_unavoidable_tests::{commit, skill};
use super::periodic_tests::bind;
use super::*;
use crate::{asset_contract::tests::Package, config::AttackRule};
use serde_json::{Value, json};

fn document() -> Value {
    let mut v: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/arena-a.json"
    ))
    .unwrap();
    // Preserve the original core numeric fixture's free definition slots. The
    // complete current a/b samples are separately loaded in the public tests.
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .retain(|s| {
            !["offer", "operation-seal", "life-window"].contains(&s["id"].as_str().unwrap())
        });
    for s in v["additional_skills"].as_array_mut().unwrap() {
        match s["id"].as_str().unwrap() {
            "focus-target" | "small-pulse" => {
                s["on_commit_self_statuses"] = json!([]);
                s["on_hit_target_statuses"] = json!([]);
            }
            "next-bolt" => s["on_commit_self_statuses"] = json!(["ward"]),
            _ => {}
        }
    }
    let pulse = v["additional_skills"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["id"] == "small-pulse")
        .unwrap();
    pulse["mechanism"] = json!("ground_circle");
    pulse["radius"] = json!(12);
    pulse["damage_bps"] = json!(100);
    for key in [
        "duration_ms",
        "tick_interval_ms",
        "contact_policy",
        "unavoidable",
    ] {
        pulse.as_object_mut().unwrap().remove(key);
    }
    for d in v["pve"]["status_definitions"].as_array_mut().unwrap() {
        if d["id"] == "control-softened" {
            *d = json!({"id":"barrier","mechanism":"absorption_shield","shield_bps":100,"duration_ms":2400,"duplicate_policy":"refresh","source_scoped":false});
        }
        d.as_object_mut().unwrap().remove("flags");
    }
    v["pve"]["additional_templates"]["control-source"]["combat"]["projectile"]["on_hit_target_statuses"] =
        json!([]);
    v["combat_rules"]["control_policies"] = json!({});
    v["combat_rules"]["status_policies"] = json!({});
    v["combat_rules"]["death_guards"] = json!({});
    v["combat_rules"]["status_windows"] = json!({});
    v["combat_rules"]["status_cleanses"] = json!({});
    v["combat_rules"]["shield_records"]
        .as_object_mut()
        .unwrap()
        .remove("overflow-shield");
    v["combat_rules"]["periodic_recovery"] = json!({});
    v["combat_rules"]["periodic_sampling"]
        .as_object_mut()
        .unwrap()
        .remove("ally-renewal");
    v["combat_rules"]["periodic_sampling"]
        .as_object_mut()
        .unwrap()
        .remove("renewal");
    v["combat_rules"]["triggers"]
        .as_array_mut()
        .unwrap()
        .retain(|t| {
            ![
                "carry-lifecycle",
                "ally-carry-control",
                "take-offer",
                "prepare-periodic-gift",
                "prepare-immunity-gift",
                "cleanse-tempo",
                "action-recovery",
                "target-progress",
            ]
            .contains(&t["id"].as_str().unwrap())
        });
    v
}
fn setup() -> (Config, World) {
    let (mut c, w) = super::member_unavoidable_tests::setup();
    c.mechanism_version = 105;
    c.combat_rules = Some(serde_json::from_value(document()["combat_rules"].clone()).unwrap());
    c.combat_rules.as_mut().unwrap().status_policies.clear();
    c.combat_rules.as_mut().unwrap().death_guards.clear();
    (c, w)
}
fn intent(w: &World, raw: u32, direct: bool) -> DamageIntent {
    DamageIntent {
        ignore_armor: false,
        source: w.members[0].reference.entity.clone(),
        target: w.npcs[0].entity.clone(),
        raw,
        direct,
    }
}
fn rule(c: &Config) -> AttackRule {
    c.combat_rules
        .as_ref()
        .unwrap()
        .skills
        .values()
        .next()
        .unwrap()
        .clone()
}

#[test]
fn combat_tables_load_two_configs_and_reject_invalid_references_versions_and_bounds() {
    let p = Package::new();
    for text in [
        crate::test_data::fixture!("examples/combat-host/arena-a.json"),
        crate::test_data::fixture!("examples/combat-host/arena-b.json"),
    ] {
        let v: Value = serde_json::from_str(text).unwrap();
        p.write(&v);
        let loaded = p.load().unwrap();
        assert_eq!(
            serde_json::to_value(loaded.default_config()).unwrap()["combat_rules"],
            v["combat_rules"]
        );
    }
    for case in 0..16 {
        let mut v = document();
        match case {
            0 => v["mechanism_version"] = json!(104),
            1 => v["combat_rules"] = Value::Null,
            2 => v["combat_rules"]["default_member_profile"] = json!("missing"),
            3 => v["combat_rules"]["profiles"]["member"]["tier"] = json!(32),
            4 => v["combat_rules"]["profiles"]["member"]["element"] = json!("unknown"),
            5 => v["combat_rules"]["profiles"]["member"]["critical_multiplier_bps"] = json!(30001),
            6 => {
                v["combat_rules"]["tier_multipliers"]
                    .as_object_mut()
                    .unwrap()
                    .remove("2");
            }
            7 => v["combat_rules"]["skills"]["missing"] = v["combat_rules"]["basic_attack"].clone(),
            8 => v["combat_rules"]["npc_profiles"]["missing"] = json!("npc"),
            9 => {
                v["combat_rules"]["periodic_damage"]["missing"] =
                    v["combat_rules"]["basic_attack"].clone()
            }
            10 => v["combat_rules"]["basic_attack"]["life_steal_bps"] = json!(10001),
            11 => v["combat_rules"]["basic_attack"]["damage_kind"] = json!("true_damage"),
            12 => {
                v["combat_rules"]["healing"]["missing"] =
                    json!({"fixed":1,"source_attack_bps":0,"target_max_hp_bps":0})
            }
            13 => v["combat_rules"]["element_multipliers_bps"]["metal"]["unknown"] = json!(13000),
            14 => v["combat_rules"]["basic_attack"]["unexpected"] = json!(true),
            _ => v["combat_rules"]["tier_multipliers"]["-02"] = json!(2500),
        }
        p.write(&v);
        assert!(p.load().is_err(), "invalid case {case}");
    }
}

#[test]
fn combat_source_realm_defense_and_dodge_vectors_use_runtime_arithmetic() {
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_combat_vectors.json"
    ))
    .unwrap();
    let c = setup().0;
    let mut count = 0;
    for row in v["vectors"].as_array().unwrap() {
        let x = &row["input"];
        let actual = match row["op"].as_str().unwrap() {
            "realm_modifier" => c.combat_rules.as_ref().unwrap().tier_multipliers
                [&(x["levelDifference"].as_i64().unwrap() as i16)]
                .factor(),
            "defense_damage" => f64::from(crate::combat_math::defense_damage(
                x["raw"].as_f64().unwrap(),
                x["armor"].as_f64().unwrap(),
                crate::combat_math::DefenseContext {
                    baseline: x["defenderRealmBaseAtk"].as_u64().unwrap() as u32,
                    pierce: x["pierceBps"].as_u64().unwrap() as u32,
                    ignore: x["irreducible"].as_bool().unwrap(),
                    scale: 2.,
                    cap: 0.7,
                    critical: x["critMultiplier"].as_f64().unwrap(),
                    minimum: 1,
                },
            )),
            "dodge_percent" => {
                combat::dodge_probability(
                    (x["dodge"].as_u64().unwrap()) * 100,
                    x["penalty"].as_u64().unwrap() as u32 * 100,
                    x["hit"].as_u64().unwrap() * 100,
                ) * 100.
            }
            _ => continue,
        };
        assert!(
            (actual - row["expected"].as_f64().unwrap()).abs() < 1e-9,
            "{}: {actual}",
            row["id"]
        );
        count += 1;
    }
    assert_eq!(count, 13);
}

#[test]
fn combat_pipeline_applies_tier_element_synergy_resistance_and_actual_hp_lifesteal() {
    let (c, mut w) = setup();
    w.members[0].hp = 700;
    let mut events = vec![];
    let result =
        w.resolve_damage_with_rule(intent(&w, 125, true), Some(&rule(&c)), &c, 30, &mut events);
    assert_eq!(
        (result.hp_damage, w.npcs[0].hp, w.members[0].hp),
        (102, 898, 751)
    );
    let heal = events.iter().find_map(|(_, e)| match e {
        EventV4::Private {
            event:
                Event::HealResolved {
                    heal_kind: HealKind::LifeSteal,
                    raw_heal,
                    hp_restored,
                    ..
                },
        } => Some((*raw_heal, *hp_restored)),
        _ => None,
    });
    assert_eq!(heal, Some((51, 51)));
    // Basic physical contacts use DEF and do not gain the skill-only power-points multiplier.
    let basic = c.combat_rules.as_ref().unwrap().basic_attack.clone();
    let result =
        w.resolve_damage_with_rule(intent(&w, 100, true), Some(&basic), &c, 60, &mut vec![]);
    assert_eq!(result.hp_damage, 80);
    let mut weakened = rule(&c);
    weakened.element = Some("metal".into());
    weakened.weapon_element = None;
    assert_eq!(
        w.resolve_damage_with_rule(intent(&w, 100, true), Some(&weakened), &c, 90, &mut vec![])
            .hp_damage,
        48
    );
}

#[test]
fn combat_unbounded_penetration_true_damage_zero_and_resistance_fallback() {
    let (mut c, mut w) = setup();
    let mut r = rule(&c);
    r.armor_penetration_bps = 20000;
    assert_eq!(
        w.resolve_damage_with_rule(intent(&w, 100, false), Some(&r), &c, 30, &mut vec![])
            .hp_damage,
        163
    );
    r.damage_kind = crate::config::DamageKind::TrueDamage;
    r.can_crit = false;
    r.element = None;
    r.weapon_element = None;
    assert_eq!(
        w.resolve_damage_with_rule(intent(&w, 0, false), Some(&r), &c, 60, &mut vec![])
            .hp_damage,
        0
    );
    assert_eq!(
        w.resolve_damage_with_rule(intent(&w, 100, false), Some(&r), &c, 90, &mut vec![])
            .hp_damage,
        100
    );
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("npc")
        .unwrap()
        .element_resistance = None;
    assert_eq!(
        w.resolve_damage_with_rule(
            intent(&w, 100, false),
            Some(&rule(&c)),
            &c,
            120,
            &mut vec![]
        )
        .hp_damage,
        109
    );
}

#[test]
fn combat_all_six_skill_plans_and_channel_use_frozen_generic_power() {
    for kind in [
        "projectile",
        "unit_projectile",
        "piercing_projectile",
        "ground_circle",
        "ground_field",
        "line_field",
    ] {
        let (mut c, mut w) = setup();
        c.skill = serde_json::from_value(skill(kind)).unwrap();
        let r = rule(&c);
        c.combat_rules
            .as_mut()
            .unwrap()
            .skills
            .insert(c.skill.id.clone(), r);
        commit(&mut w, &c, kind);
        // Post-commit ATK cannot change an already materialized budget.
        w.members[0].facts.atk_points = 1000;
        if c.skill.is_ground() || c.skill.is_line() {
            w.advance_areas(&c, 150, &mut vec![]);
            assert_eq!(w.npcs[0].hp, 898, "{kind}");
            if c.skill.is_field() {
                w.advance_areas(&c, 330, &mut vec![]);
                assert_eq!(w.npcs[0].hp, 796);
            }
        } else {
            w.advance_shots(&c, 330, 1., &mut vec![]);
            assert_eq!(w.npcs[0].hp, 898, "{kind}");
        }
    }
    let (mut c, mut w) = setup();
    let mut v = skill("projectile");
    v["channel"] = json!({"duration_ms":390,"interval_ms":120,"spirit_per_pulse":1});
    c.skill = serde_json::from_value(v).unwrap();
    let r = rule(&c);
    c.combat_rules
        .as_mut()
        .unwrap()
        .skills
        .insert(c.skill.id.clone(), r);
    commit(&mut w, &c, "projectile");
    assert_eq!(w.members[0].channel.as_ref().unwrap().raw, 125);
    w.members[0].facts.atk_points = 1000;
    w.advance_channel(0, &c, 150, &mut vec![]);
    w.advance_shots(&c, 180, 1., &mut vec![]);
    assert_eq!(w.npcs[0].hp, 898);
}

#[test]
fn combat_periodic_damage_freezes_source_budget_and_uses_the_same_target_pipeline() {
    let (mut c, mut w) = setup();
    let d:crate::config::StatusDefinition=serde_json::from_value(json!({"id":"elemental-dot","mechanism":"periodic_damage","damage_bps":10000,"duration_ms":390,"tick_interval_ms":120,"duplicate_policy":"refresh","source_scoped":true})).unwrap();
    c.pve_mut().status_definitions.push(d);
    let r = rule(&c);
    c.combat_rules
        .as_mut()
        .unwrap()
        .periodic_damage
        .insert("elemental-dot".into(), r);
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), npc, "elemental-dot", 0);
    let status = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "elemental-dot")
        .unwrap();
    assert_eq!(
        status.periodic.as_ref().unwrap().kind,
        PeriodicEffectKind::ElementalDamage
    );
    w.members[0].facts.atk_points = 1000;
    w.rng = 7;
    w.advance_periodic(&c, 120, &mut vec![]);
    assert_eq!(w.npcs[0].hp, 919);
    assert_eq!(w.rng, 7);
    w.advance_periodic(&c, 240, &mut vec![]);
    assert_eq!(w.npcs[0].hp, 838);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    w.advance_periodic(&c, 360, &mut vec![]);
    assert_eq!(w.npcs[0].hp, 838);
}

#[test]
fn combat_healing_formula_combines_fixed_source_attack_and_target_maximum_once() {
    let (mut c, mut w) = setup();
    c.combat_rules
        .as_mut()
        .unwrap()
        .periodic_sampling
        .remove("renewal");
    c.combat_rules.as_mut().unwrap().healing.insert(
        "test-heal".into(),
        serde_json::from_value(json!({"fixed":5,"source_attack_bps":5000,"target_max_hp_bps":100}))
            .unwrap(),
    );
    w.members[1].hp = 500;
    let heal: crate::config::InstantHeal = serde_json::from_value(json!({"amount":1})).unwrap();
    w.resolve_skill_heal((0, 1), &heal, "test-heal", &c, 30, &mut vec![]);
    assert_eq!(w.members[1].hp, 566);
    c.combat_rules.as_mut().unwrap().healing.insert(
        "renewal".into(),
        serde_json::from_value(json!({"fixed":5,"source_attack_bps":5000,"target_max_hp_bps":100}))
            .unwrap(),
    );
    let self_ref = w.members[0].reference.entity.clone();
    w.members[0].hp = 500;
    bind(&mut w, &c, Owner::Member(0), self_ref, "renewal", 0);
    w.members[0].facts.atk_points = 1000;
    let due = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "renewal")
        .unwrap()
        .periodic
        .as_ref()
        .unwrap()
        .next_tick_at_ms
        .unwrap();
    w.advance_periodic(&c, due, &mut vec![]);
    assert_eq!(w.members[0].hp, 565);
}

#[test]
fn combat_member_profiles_are_isolated_and_immunity_precedes_dodge_and_shields() {
    let (mut c, mut w) = setup();
    let mut isolated = c.combat_rules.as_ref().unwrap().profiles["member"].clone();
    isolated.tier = 0;
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .insert("isolated".into(), isolated);
    c.combat_rules
        .as_mut()
        .unwrap()
        .member_profiles
        .insert("P01".into(), "isolated".into());
    let p = w.members[1].reference.entity.clone();
    let raw = DamageIntent {
        ignore_armor: false,
        source: p,
        target: w.npcs[0].entity.clone(),
        raw: 100,
        direct: true,
    };
    let plain = AttackRule::default();
    assert_eq!(
        w.resolve_damage_with_rule(raw, Some(&plain), &c, 30, &mut vec![])
            .hp_damage,
        66
    );
    let mut d = c.combat_rules.as_ref().unwrap().profiles["npc"].clone();
    d.dodge_bps = 100000000;
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .insert("npc".into(), d);
    let source = w.members[0].reference.entity.clone();
    let r = rule(&c);
    assert!(w.direct_contact(
        Owner::Member(0),
        Owner::Npc(0),
        DirectContact {
            origin: None,
            action_id: None,
            rule: Some(&r),
            ignore_armor: false,
            unavoidable: false,
            dead_source: false,
            source: &source,
            raw: 100,
            metal: true,
            hit_statuses: &[]
        },
        &c,
        60,
        &mut vec![]
    ));
}

fn trigger(c: &mut Config, value: Value) {
    c.combat_rules
        .as_mut()
        .unwrap()
        .triggers
        .push(serde_json::from_value(value).unwrap());
}
#[test]
fn combat_triggers_have_per_owner_counters_cooldowns_and_no_periodic_leakage() {
    let (mut c, mut w) = setup();
    w.members[0].hp = 500;
    trigger(
        &mut c,
        json!({"id":"leech","profiles":["member"],"when":"damage_dealt","include_periodic":false,"every":2,"cooldown_ms":1000,"priority":0,"effect":{"kind":"heal_from_damage","bps":5000}}),
    );
    let r = AttackRule::default();
    for now in [30, 60, 90, 120] {
        w.resolve_damage_with_rule(intent(&w, 100, true), Some(&r), &c, now, &mut vec![]);
    }
    assert_eq!(w.members[0].hp, 540);
    w.resolve_damage_with_rule(intent(&w, 100, false), Some(&r), &c, 1100, &mut vec![]);
    assert_eq!(w.members[0].hp, 540);
    w.resolve_damage_with_rule(intent(&w, 100, true), Some(&r), &c, 1130, &mut vec![]);
    assert_eq!(w.members[0].hp, 580);
    let peer = DamageIntent {
        source: w.members[1].reference.entity.clone(),
        ..intent(&w, 100, true)
    };
    w.members[1].hp = 500;
    w.resolve_damage_with_rule(peer, Some(&r), &c, 1160, &mut vec![]);
    assert_eq!(w.members[1].hp, 500);
    w.clear_owned_statuses(
        &w.members[0].reference.entity.clone(),
        StatusRemovalReason::Reset,
        &mut vec![],
    );
    w.resolve_damage_with_rule(intent(&w, 100, true), Some(&r), &c, 1200, &mut vec![]);
    assert_eq!(w.members[0].hp, 580);
}
#[test]
fn combat_reflection_cannot_recursively_trigger_more_reflection_or_resurrect_the_dead() {
    let (mut c, mut w) = setup();
    for profile in ["member", "npc"] {
        trigger(
            &mut c,
            json!({"id":format!("reflect-{profile}"),"profiles":[profile],"when":"damage_taken","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":5000,"damage_kind":"true_damage"}}),
        );
    }
    w.members[0].hp = 500;
    let r = AttackRule::default();
    w.resolve_damage_with_rule(intent(&w, 100, true), Some(&r), &c, 30, &mut vec![]);
    assert_eq!((w.members[0].hp, w.npcs[0].hp), (460, 920));
    w.npcs[0].hp = 1;
    w.resolve_damage_with_rule(intent(&w, 100, true), Some(&r), &c, 60, &mut vec![]);
    assert_eq!((w.members[0].hp, w.npcs[0].hp), (460, 0));
}
#[test]
fn combat_trigger_budget_and_priority_are_bounded_and_skill_commit_runs_once() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().max_triggers_per_step = 1;
    for (id, priority, amount) in [("later", 9, 7), ("first", 0, 1)] {
        trigger(
            &mut c,
            json!({"id":id,"profiles":["member"],"when":"skill_committed","include_periodic":false,"every":1,"cooldown_ms":0,"priority":priority,"effect":{"kind":"restore_spirit","amount":amount}}),
        );
    }
    w.members[0].spirit = 100;
    c.skill = serde_json::from_value(skill("projectile")).unwrap();
    commit(&mut w, &c, "projectile");
    assert_eq!(w.members[0].spirit, 99);
    w.advance_shots(&c, 330, 1., &mut vec![]);
    assert_eq!(w.members[0].spirit, 99);
}

#[test]
fn combat_compound_status_is_one_identity_with_atomic_stack_expiry_and_source_cleanup() {
    let (mut c, mut w) = setup();
    let d:crate::config::StatusDefinition=serde_json::from_value(json!({"id":"compound","mechanism":"timed_compound_modifier","duration_ms":300,"duplicate_policy":"stack","source_scoped":true,"max_stacks":3,"modifiers":[{"attribute":"atk","modifier_bps":2000},{"attribute":"def","modifier_bps":-2000},{"attribute":"critical_chance","modifier_bps":5000}]})).unwrap();
    c.pve_mut().status_definitions.push(d);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "compound", 0);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "compound", 30);
    assert_eq!(w.stats(0, &c, 30), (140, 60));
    assert_eq!(w.stats(0, &c, 30), (140, 60));
    let one = w.statuses.views(&status_entity(&target), 30);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].modifiers.len(), 3);
    assert_eq!(one[0].stacks, Some(2));
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "compound", 60);
    assert_eq!(w.stats(0, &c, 60), (160, 40));
    w.clear_owned_statuses(
        &w.npcs[0].entity.clone(),
        StatusRemovalReason::SourceInvalid,
        &mut vec![],
    );
    assert_eq!(w.stats(0, &c, 60), (140, 60));
    assert_eq!(w.stats(0, &c, 330), (100, 100));
}
#[test]
fn combat_compound_configuration_roundtrips_and_rejects_unsupported_or_duplicate_attributes() {
    let p = Package::new();
    let mut v = document();
    let def = json!({"id":"compound","mechanism":"timed_compound_modifier","duration_ms":300,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"atk","modifier_bps":2000},{"attribute":"def","modifier_bps":-2000}]});
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(def.clone());
    let last = v["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    p.write(&v);
    let loaded = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(loaded.default_config()).unwrap()["pve"]["status_definitions"][last]["modifiers"],
        def["modifiers"]
    );
    for a in [json!("silence"), json!("movement_haste"), json!("atk")] {
        let mut bad = v.clone();
        bad["pve"]["status_definitions"][last]["modifiers"][1]["attribute"] = a;
        p.write(&bad);
        assert!(p.load().is_err());
    }
}

#[test]
fn combat_ground_clamp_is_explicit_freezes_the_accepted_point_and_revalidates_before_cost() {
    let (mut c, mut w) = setup();
    c.skill = serde_json::from_value(skill("ground_circle")).unwrap();
    c.skill.range = 60.;
    let origin = w.members[0].position;
    let requested = [origin[0] + 1000., origin[1]];
    assert!(
        w.configured_ground_target(0, &c.skill, requested, &c)
            .is_err()
    );
    c.combat_rules
        .as_mut()
        .unwrap()
        .ground_target_policies
        .insert(
            c.skill.id.clone(),
            crate::config::GroundTargetPolicy::ClampToRange,
        );
    let corrected = w
        .configured_ground_target(0, &c.skill, requested, &c)
        .unwrap();
    assert_eq!(corrected, [origin[0] + 60., origin[1]]);
    let action = Action::CastGround {
        skill_id: c.skill.id.clone(),
        position: requested,
    };
    w.action(0, &action, "clamp", &c, 0).unwrap();
    assert_eq!(
        w.members[0].pending.as_ref().unwrap().ground,
        Some(corrected)
    );
    let hp = w.members[0].hp;
    let spirit = w.members[0].spirit;
    w.members[0].position = [origin[0] - 100., origin[1]];
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 30, &mut vec![]);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (hp, spirit));
    assert!(w.areas.is_empty());
    assert_eq!(
        w.configured_ground_target(0, &c.skill, [f64::NAN, 0.], &c),
        Err(Error::InvalidMessage)
    );
}

#[test]
fn combat_npc_melee_uses_configured_dodge_and_critical_exactly_once() {
    let (mut c, mut w) = setup();
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("npc")
        .unwrap()
        .critical_chance_bps = 10000;
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("member")
        .unwrap()
        .dodge_bps = 100000000;
    c.combat_rules.as_mut().unwrap().npc_attacks.insert(
        w.npcs[0].template_id.clone(),
        AttackRule {
            unavoidable: true,
            ..Default::default()
        },
    );
    w.rng = 1;
    let profile = Arc::make_mut(&mut w.npcs[0].profile);
    profile.stats.insert("atk".into(), 100);
    profile.stats.insert("crit".into(), 100);
    profile.combat.melee_range = 100.;
    profile.combat.cast_chance = 0.;
    w.npcs[0].next_attack = 0;
    w.npcs[0].casting = None;
    w.advance_npcs(&c, 30, 0.03, &mut vec![]);
    assert_eq!(w.members[0].hp, 975);
}
#[test]
fn combat_member_control_interrupts_npcs_and_expires_without_source_or_peer_leakage() {
    for attribute in ["silence", "root", "stun", "movement_slow"] {
        let (mut c, mut w) = setup();
        let control = attribute != "movement_slow";
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"npc-control","mechanism":if control {"timed_control"}else{"timed_attribute_modifier"},"attribute":attribute,"modifier_bps":if control {0}else{-5000},"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
        w.npcs[0].casting = Some(super::npc_skills::NpcCast::legacy(150, [1., 0.]));
        let target = w.npcs[0].entity.clone();
        let events = bind(&mut w, &c, Owner::Member(0), target, "npc-control", 0);
        assert!(events.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::StatusApplied{status}} if status.definition_id=="npc-control")),"{attribute}");
        if attribute == "silence" || attribute == "stun" {
            assert!(w.npcs[0].casting.is_none());
        }
        if attribute == "root" {
            assert_eq!(w.npc_movement_factor(0, &c, 30), 0.);
        }
        if attribute == "movement_slow" {
            assert_eq!(w.npc_movement_factor(0, &c, 30), 0.5);
            c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"weaker-slow","mechanism":"timed_attribute_modifier","attribute":"movement_slow","modifier_bps":-2000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
            let npc = w.npcs[0].entity.clone();
            bind(&mut w, &c, Owner::Member(1), npc, "weaker-slow", 0);
            assert_eq!(w.npc_movement_factor(0, &c, 30), 0.5);
        }
        assert_eq!(w.npc_movement_factor(0, &c, 300), 1.);
        assert!(
            w.statuses
                .views(&status_entity(&w.members[1].reference.entity), 30)
                .is_empty()
        );
    }
}
#[test]
fn combat_elemental_resistance_status_is_live_and_immunity_still_precedes_the_pipeline() {
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"resist","mechanism":"timed_attribute_modifier","attribute":"element_resistance","modifier_bps":10000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc.clone(), "resist", 0);
    assert_eq!(
        w.resolve_damage_with_rule(intent(&w, 100, false), Some(&rule(&c)), &c, 30, &mut vec![])
            .hp_damage,
        54
    );
    assert_eq!(
        w.resolve_damage_with_rule(
            intent(&w, 100, false),
            Some(&rule(&c)),
            &c,
            300,
            &mut vec![]
        )
        .hp_damage,
        81
    );
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"immune","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    bind(&mut w, &c, Owner::Npc(0), npc, "immune", 300);
    let true_rule = AttackRule {
        damage_kind: crate::config::DamageKind::TrueDamage,
        can_crit: false,
        ..Default::default()
    };
    assert_eq!(
        w.resolve_damage_with_rule(
            intent(&w, 100, false),
            Some(&true_rule),
            &c,
            330,
            &mut vec![]
        )
        .hp_damage,
        0
    );
}

#[test]
fn combat_mitigation_shield_hp_and_lifesteal_conserve_actual_damage_in_order() {
    let (mut c, mut w) = setup();
    for d in [
        json!({"id":"guard-chain","mechanism":"timed_compound_modifier","duration_ms":300,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"damage_reduction","modifier_bps":5000}]}),
        json!({"id":"chain-shield","mechanism":"absorption_shield","shield_bps":300,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    ] {
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(d).unwrap());
    }
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc.clone(), "guard-chain", 0);
    bind(&mut w, &c, Owner::Npc(0), npc, "chain-shield", 0);
    w.members[0].hp = 800;
    let result =
        w.resolve_damage_with_rule(intent(&w, 100, false), Some(&rule(&c)), &c, 30, &mut vec![]);
    assert_eq!(
        (
            result.absorbed,
            result.hp_damage,
            w.npcs[0].hp,
            w.members[0].hp
        ),
        (30, 10, 990, 805)
    );
}
#[test]
fn combat_trigger_loading_rejects_unknown_facts_invalid_effects_and_unbounded_budgets() {
    let package = Package::new();
    for case in 0..9 {
        let mut v = document();
        match case {
            0 => v["combat_rules"]["max_triggers_per_step"] = json!(0),
            1 => v["combat_rules"]["triggers"][0]["every"] = json!(0),
            2 => v["combat_rules"]["triggers"][0]["when"] = json!("unknown_fact"),
            3 => v["combat_rules"]["triggers"][0]["profiles"] = json!(["missing"]),
            4 => {
                v["combat_rules"]["triggers"][0]["effect"] =
                    json!({"kind":"reflect_damage","bps":5000,"damage_kind":"true_damage"})
            }
            5 => {
                v["combat_rules"]["triggers"][0]["effect"] =
                    json!({"kind":"apply_self_status","status_id":"missing"})
            }
            _ => {
                v["combat_rules"]["triggers"][0]["effect"] =
                    json!({"kind":"restore_spirit","amount":0})
            }
        }
        if case >= 7 {
            v["combat_rules"]["triggers"][0]["when"] = json!("damage_taken");
            v["combat_rules"]["triggers"][0]["effect"] = if case == 7 {
                json!({"kind":"reflect_damage","bps":5000,"damage_kind":"elemental"})
            } else {
                json!({"kind":"reflect_damage","bps":5000,"damage_kind":"elemental","element":"missing"})
            };
        }
        package.write(&v);
        assert!(package.load().is_err(), "case {case}");
    }
}

#[test]
fn combat_exact_tier_ratios_preserve_low_tier_rounding_and_reject_invalid_denominators() {
    let (mut c, mut w) = setup();
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("member")
        .unwrap()
        .tier = 0;
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("npc")
        .unwrap()
        .tier = 8;
    let r = AttackRule {
        ignore_armor: true,
        can_crit: false,
        ..Default::default()
    };
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("hp".into(), 1000000);
    w.npcs[0].hp = 1000000;
    assert_eq!(
        w.resolve_damage_with_rule(intent(&w, 1000000, false), Some(&r), &c, 30, &mut vec![])
            .hp_damage,
        3906
    );
    let package = Package::new();
    for ratio in [
        json!({"numerator":1,"denominator":0}),
        json!({"numerator":1,"denominator":256,"unknown":1}),
        json!({"numerator":1000001,"denominator":1}),
    ] {
        let mut bad = document();
        bad["combat_rules"]["tier_multipliers"]["-8"] = ratio;
        package.write(&bad);
        assert!(package.load().is_err());
    }
}

#[test]
fn combat_temporary_resource_maxima_and_spd_are_derived_without_changing_permanent_facts() {
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"vitality","mechanism":"timed_compound_modifier","duration_ms":300,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"max_hp","modifier_bps":5000},{"attribute":"max_spirit","modifier_bps":5000},{"attribute":"spd","modifier_bps":10000}]})).unwrap());
    let target = w.members[0].reference.entity.clone();
    let facts = w.members[0].facts.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "vitality", 0);
    assert_eq!(w.entity_max_hp(&target, &c, 30), Some(1500));
    assert_eq!(w.member_max_spirit(0, &c, 30), 450);
    assert_eq!(w.effective_spd(Owner::Member(0), &c, 30), 16);
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.members[0].facts, facts);
    assert_eq!(w.restore_hp(&target, 1000, &c, 30), 500);
    w.members[0].spirit = 450;
    w.prepare_periodic(300, &mut vec![]);
    w.reconcile_caps(&c, 300);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (1000, 300));
    assert_eq!(w.effective_spd(Owner::Member(0), &c, 300), 8);
    assert_eq!(w.members[0].facts, facts);
}
#[test]
fn combat_maximum_debuff_cannot_kill_or_revive_and_source_exit_updates_views() {
    let (mut c, mut w) = setup();
    for (id, bps) in [("capacity", 5000), ("capacity-down", -10000)] {
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":bps,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    }
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "capacity", 0);
    assert_eq!(w.restore_hp(&target, 1000, &c, 30), 550);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    let snapshot = w.snapshot(1, &c, Phase::Active, 30);
    assert_eq!((snapshot.own.hp, snapshot.own.max_hp), (1100, 1100));
    w.reconcile_caps(&c, 30);
    assert_eq!(w.members[1].hp, 1100);
    bind(
        &mut w,
        &c,
        Owner::Member(1),
        target.clone(),
        "capacity-down",
        60,
    );
    assert_eq!(
        (w.members[1].hp, w.entity_max_hp(&target, &c, 60)),
        (1, Some(1))
    );
    w.members[1].hp = 0;
    assert_eq!(w.restore_hp(&target, 1000, &c, 90), 0);
}

#[test]
fn combat_attribute_caps_are_configured_separately_from_individual_status_strength() {
    let package = Package::new();
    let mut v = document();
    v["pve"]["status_rules"]["attribute_caps_bps"] =
        json!({"max_hp":2500,"spd":0,"damage_reduction":8000,"critical_damage_bonus":40000});
    v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({"id":"capacity","mechanism":"timed_compound_modifier","duration_ms":300,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"max_hp","modifier_bps":10000},{"attribute":"spd","modifier_bps":10000},{"attribute":"damage_reduction","modifier_bps":9000}]}));
    v["combat_rules"]["dodge_cap_bps"] = json!(7500);
    v["combat_rules"]["critical_multiplier_cap_bps"] = json!(50000);
    package.write(&v);
    let loaded = package.load().unwrap();
    assert_eq!(
        serde_json::to_value(loaded.default_config()).unwrap()["pve"]["status_rules"]["attribute_caps_bps"],
        v["pve"]["status_rules"]["attribute_caps_bps"]
    );
    let (mut c, mut w) = setup();
    c.pve_mut().status_rules = loaded
        .default_config()
        .pve
        .as_ref()
        .unwrap()
        .status_rules
        .clone();
    c.pve_mut().status_definitions.push(
        serde_json::from_value(
            v["pve"]["status_definitions"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .clone(),
        )
        .unwrap(),
    );
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "capacity", 0);
    assert_eq!(w.entity_max_hp(&target, &c, 0), Some(1250));
    assert_eq!(w.effective_spd(Owner::Member(0), &c, 0), 8);
    assert_eq!(
        w.statuses.damage_reduction_bps(
            &status_entity(&target),
            0,
            c.pve.as_ref().unwrap().status_rules.as_ref()
        ),
        8000
    );
    for (field, value) in [
        ("damage_reduction", 10001),
        ("max_hp", 1000001),
        ("silence", 0),
    ] {
        let mut bad = v.clone();
        bad["pve"]["status_rules"]["attribute_caps_bps"][field] = json!(value);
        package.write(&bad);
        assert!(package.load().is_err());
    }
    for (field, value) in [
        ("dodge_cap_bps", json!(10001)),
        ("critical_multiplier_cap_bps", json!(9999)),
        ("dodge_cap_bps", Value::Null),
    ] {
        let mut bad = v.clone();
        bad["combat_rules"][field] = value;
        package.write(&bad);
        assert!(package.load().is_err());
    }
    let mut legacy = v;
    legacy["mechanism_version"] = json!(104);
    legacy.as_object_mut().unwrap().remove("combat_rules");
    package.write(&legacy);
    assert!(package.load().is_err());
}

#[test]
fn combat_configured_dodge_and_critical_caps_control_the_actual_resolution() {
    let (mut c, mut w) = setup();
    let rules = c.combat_rules.as_mut().unwrap();
    rules.dodge_cap_bps = Some(10000);
    rules.profiles.get_mut("npc").unwrap().dodge_bps = 100000000;
    assert!(w.configured_dodge(Owner::Member(0), Owner::Npc(0), &c, 0));
    c.combat_rules.as_mut().unwrap().dodge_cap_bps = Some(0);
    assert!(!w.configured_dodge(Owner::Member(0), Owner::Npc(0), &c, 0));
    let rules = c.combat_rules.as_mut().unwrap();
    rules.critical_multiplier_cap_bps = Some(50000);
    let p = rules.profiles.get_mut("member").unwrap();
    p.critical_multiplier_bps = 50000;
    p.critical_chance_bps = 10000;
    let r = AttackRule {
        ignore_armor: true,
        ..Default::default()
    };
    let hit = intent(&w, 100, true);
    assert_eq!(
        w.configured_incoming(
            &hit,
            &r,
            (Owner::Member(0), Owner::Npc(0)),
            hit.direct,
            &c,
            0
        ),
        (600, true)
    );
    c.combat_rules.as_mut().unwrap().critical_multiplier_cap_bps = Some(30000);
    assert_eq!(
        w.configured_incoming(
            &hit,
            &r,
            (Owner::Member(0), Owner::Npc(0)),
            hit.direct,
            &c,
            0
        ),
        (360, true)
    );
}

#[test]
fn combat_periodic_sampling_selects_source_or_target_and_snapshot_or_live_attack() {
    for (sampling, expected) in [
        (PeriodicSampling::SourceSnapshot, 60),
        (PeriodicSampling::TargetSnapshot, 240),
        (PeriodicSampling::SourceLive, 120),
        (PeriodicSampling::TargetLive, 480),
    ] {
        let (mut c, mut w) = setup();
        c.combat_rules
            .as_mut()
            .unwrap()
            .periodic_sampling
            .insert("wound".into(), sampling);
        Arc::make_mut(&mut w.npcs[0].profile)
            .stats
            .insert("atk".into(), 400);
        let target = w.npcs[0].entity.clone();
        bind(&mut w, &c, Owner::Member(0), target, "wound", 0);
        w.members[0].facts.atk_points = 200;
        Arc::make_mut(&mut w.npcs[0].profile)
            .stats
            .insert("atk".into(), 800);
        let due = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "wound")
            .unwrap()
            .periodic
            .as_ref()
            .unwrap()
            .next_tick_at_ms
            .unwrap();
        let mut events = vec![];
        w.advance_periodic(&c, due, &mut events);
        assert!(events.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::PeriodicTick{raw_damage,..}} if *raw_damage==expected)),"{sampling:?}: {events:?}");
        let view = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "wound")
            .unwrap()
            .periodic
            .as_ref()
            .unwrap();
        assert_eq!(view.raw_damage, expected);
        assert_eq!(view.power_sampling, Some(sampling));
        w.clear_owned_statuses(
            &w.members[0].reference.entity.clone(),
            StatusRemovalReason::SourceInvalid,
            &mut vec![],
        );
        let mut events = vec![];
        w.advance_periodic(&c, due + 600, &mut events);
        assert!(events.is_empty());
    }
}

#[test]
fn combat_hot_sampling_preserves_source_attribution_and_uses_live_target_maximum_only_when_requested()
 {
    for (sampling, expected) in [
        (PeriodicSampling::SourceSnapshot, 115),
        (PeriodicSampling::TargetSnapshot, 165),
        (PeriodicSampling::SourceLive, 242),
        (PeriodicSampling::TargetLive, 342),
    ] {
        let (mut c, mut w) = setup();
        let combat = c.combat_rules.as_mut().unwrap();
        combat.periodic_sampling.insert("renewal".into(), sampling);
        combat.healing.insert(
            "renewal".into(),
            serde_json::from_value(
                json!({"fixed":10,"source_attack_bps":5000,"target_max_hp_bps":500}),
            )
            .unwrap(),
        );
        let target = w.members[1].reference.entity.clone();
        w.members[1].hp = 100;
        bind(&mut w, &c, Owner::Member(0), target.clone(), "renewal", 0);
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"capacity","mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":5000,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
        bind(&mut w, &c, Owner::Member(1), target, "capacity", 30);
        w.members[0].facts.atk_points = 300;
        w.members[1].facts.atk_points = 500;
        let due = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "renewal")
            .unwrap()
            .periodic
            .as_ref()
            .unwrap()
            .next_tick_at_ms
            .unwrap();
        let mut events = vec![];
        w.advance_periodic(&c, due, &mut events);
        assert!(events.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::PeriodicHealTick{raw_heal,source,target,..}} if *raw_heal==expected && source.id==w.members[0].reference.entity.id && target.id==w.members[1].reference.entity.id)),"{sampling:?}: {events:?}");
        assert_eq!(w.members[1].hp, 100 + expected);
    }
}

#[test]
fn combat_sampling_configuration_rejects_unknown_non_periodic_and_unsupported_policies() {
    let package = Package::new();
    let mut v = document();
    v["combat_rules"]["periodic_sampling"] =
        json!({"wound":"target_live","renewal":"source_snapshot"});
    package.write(&v);
    assert_eq!(
        serde_json::to_value(package.load().unwrap().default_config()).unwrap()["combat_rules"]["periodic_sampling"],
        v["combat_rules"]["periodic_sampling"]
    );
    for policy in [
        json!({"ward":"source_live"}),
        json!({"missing":"source_live"}),
        json!({"wound":"unknown"}),
        json!({"wound":null}),
    ] {
        v["combat_rules"]["periodic_sampling"] = policy;
        package.write(&v);
        assert!(package.load().is_err());
    }
}

#[test]
fn combat_harmful_duration_scales_debuffs_and_controls_once_and_leaves_benefits_intact() {
    let (mut c, mut w) = setup();
    for def in [
        json!({"id":"resilience","mechanism":"timed_compound_modifier","duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"harmful_duration_reduction","modifier_bps":5000},{"attribute":"control_duration_reduction","modifier_bps":4000}]}),
        json!({"id":"slow","mechanism":"timed_attribute_modifier","attribute":"movement_slow","modifier_bps":-3000,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"weakness","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":-2000,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"benefit","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":2000,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"tagged","harmful":true,"mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":0,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true}),
    ] {
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(def).unwrap());
    }
    let target = w.members[0].reference.entity.clone();
    bind(
        &mut w,
        &c,
        Owner::Member(0),
        target.clone(),
        "resilience",
        0,
    );
    for (id, duration) in [
        ("slow", 300),
        ("weakness", 500),
        ("benefit", 1000),
        ("tagged", 500),
        ("wound", 2250),
    ] {
        bind(&mut w, &c, Owner::Npc(0), target.clone(), id, 30);
        let s = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == id)
            .unwrap();
        assert_eq!(s.expires_at_ms - s.applied_at_ms, duration, "{id}");
    }
    // Refresh samples the current protection, without shortening the stored
    // definition or multiplying a prior shortened instance a second time.
    bind(&mut w, &c, Owner::Npc(0), target, "weakness", 60);
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "weakness")
        .unwrap();
    assert_eq!(s.expires_at_ms, 560);
    assert_eq!(
        c.pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .find(|d| d.id == "weakness")
            .unwrap()
            .duration_ms,
        1000
    );
}

#[test]
fn combat_status_only_items_work_at_full_resources_and_keep_inventory_and_capacity_atomic() {
    let (mut c, mut w) = setup();
    let def = json!({"grant_statuses_without_recovery":true,"cooldown_ms":600,"on_use_self_statuses":["battle-pose"],"on_use_self_status_overrides":{"battle-pose":{"modifier_bps":0,"duration_ms":900,"modifiers":[{"attribute":"atk","modifier_bps":3000},{"attribute":"def","modifier_bps":-1000}]}}});
    c.pve_mut().status_definitions.push(
        serde_json::from_value(
            document()["pve"]["status_definitions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["id"] == "battle-pose")
                .unwrap()
                .clone(),
        )
        .unwrap(),
    );
    c.member_item_use = Some(serde_json::from_value(json!({"pill_healing":def})).unwrap());
    w.members[0].inventory.insert("pill_healing".into(), 2);
    w.members[0].looted.insert("pill_healing".into(), 2);
    let revision = w.members[0].revision;
    let action = |revision| Action::UseItem {
        item_id: "pill_healing".into(),
        expected_revision: revision,
    };
    w.action(0, &action(revision), "buff", &c, 0).unwrap();
    assert_eq!(w.stats(0, &c, 0), (130, 90));
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.members[0].inventory["pill_healing"], 1);
    let before = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        w.action(0, &action(revision), "stale", &c, 30),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        w.action(0, &action(revision + 1), "cooldown", &c, 30),
        Err(Error::InvalidState)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own, before);
    let peer = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), peer, "battle-pose", 30);
    assert_eq!(w.stats(1, &c, 30), (240, 80));
    let max = w
        .statuses
        .entries()
        .iter()
        .filter(|s| s.target.id == w.members[0].reference.entity.id)
        .count();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = max;
    c.member_item_use
        .as_mut()
        .unwrap()
        .get_mut("pill_healing")
        .unwrap()
        .on_use_self_statuses = Some(vec!["renewal".into()]);
    let before = w.snapshot(0, &c, Phase::Active, 600).own;
    assert_eq!(
        w.action(0, &action(revision + 1), "full", &c, 600),
        Err(Error::Capacity)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 600).own, before);
}

#[test]
fn combat_status_only_item_and_compound_quality_configuration_is_validated_as_status_definitions() {
    let package = Package::new();
    let mut v = document();
    let base = json!({"grant_statuses_without_recovery":true,"on_use_self_statuses":["battle-pose"],"on_use_self_status_overrides":{"battle-pose":{"modifier_bps":0,"duration_ms":900,"modifiers":[{"attribute":"atk","modifier_bps":3000},{"attribute":"def","modifier_bps":-1000}]}}});
    v["pve"]["items"]["pill_healing"] = json!({"name":"sample buff","stackLimit":99});
    v["member_item_use"]["pill_healing"] = base;
    package.write(&v);
    package.load().unwrap();
    for case in 0..5 {
        let mut bad = v.clone();
        match case {
            0 => {
                bad["member_item_use"]["pill_healing"]
                    .as_object_mut()
                    .unwrap()
                    .remove("on_use_self_statuses");
            }
            1 => {
                bad["member_item_use"]["pill_healing"]["on_use_self_status_overrides"]["battle-pose"]
                    ["modifiers"][0]["attribute"] = json!("silence")
            }
            2 => {
                bad["member_item_use"]["pill_healing"]["on_use_self_status_overrides"]["battle-pose"]
                    ["modifiers"][0]["modifier_bps"] = json!(100001)
            }
            3 => {
                bad["member_item_use"]["pill_healing"]["on_use_self_status_overrides"]["battle-pose"]
                    ["duration_ms"] = json!(0)
            }
            _ => {
                bad["mechanism_version"] = json!(104);
                bad.as_object_mut().unwrap().remove("combat_rules");
            }
        };
        package.write(&bad);
        assert!(package.load().is_err(), "{case}");
    }
}

#[test]
fn combat_action_counting_groups_field_ticks_and_channel_pulses_without_losing_the_parent_action() {
    for kind in ["ground_field", "line_field", "channel"] {
        let (mut c, mut w) = setup();
        let mut v = skill(if kind == "channel" {
            "projectile"
        } else {
            kind
        });
        if kind == "channel" {
            v["channel"] = json!({"duration_ms":390,"interval_ms":120,"spirit_per_pulse":1});
        }
        c.skill = serde_json::from_value(v).unwrap();
        c.combat_rules.as_mut().unwrap().triggers.clear();
        trigger(
            &mut c,
            json!({"id":"once-per-cast","profiles":["member"],"when":"damage_dealt","counting":"action","include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"restore_spirit","amount":7}}),
        );
        w.members[0].spirit = 100;
        commit(
            &mut w,
            &c,
            if kind == "channel" {
                "projectile"
            } else {
                kind
            },
        );
        let mut events = vec![];
        if kind == "channel" {
            w.advance_channel(0, &c, 150, &mut events);
            w.advance_shots(&c, 180, 1., &mut events);
            w.prune_trigger_actions();
            w.advance_channel(0, &c, 270, &mut events);
            w.advance_shots(&c, 300, 1., &mut events);
        } else {
            w.advance_areas(&c, 150, &mut events);
            w.prune_trigger_actions();
            w.advance_areas(&c, 270, &mut events);
        }
        let fired=events.iter().filter(|(_,e)| matches!(e,EventV4::Private{event:Event::CombatTriggerFired{trigger_id,..}} if trigger_id=="once-per-cast")).count();
        assert_eq!(fired, 1, "{kind}: {events:?}");
        assert_eq!(
            w.members[0].spirit,
            if kind == "channel" { 103 } else { 105 },
            "{kind}"
        );
    }
}

#[test]
fn combat_action_counting_is_per_owner_counts_once_during_cooldown_and_cleans_up_on_source_exit() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"two-actions","profiles":["member"],"when":"damage_dealt","counting":"action","include_periodic":false,"every":2,"cooldown_ms":300,"priority":0,"effect":{"kind":"restore_spirit","amount":7}}),
    );
    w.members[0].spirit = 100;
    w.members[1].spirit = 100;
    let r = AttackRule {
        unavoidable: true,
        can_crit: false,
        ..Default::default()
    };
    let contact = |w: &mut World, i: usize, action: &str, now: u64| {
        let source = w.members[i].reference.entity.clone();
        w.direct_contact(
            Owner::Member(i),
            Owner::Npc(0),
            DirectContact {
                origin: None,
                action_id: Some(action),
                rule: Some(&r),
                ignore_armor: true,
                unavoidable: true,
                dead_source: false,
                source: &source,
                raw: 1,
                metal: true,
                hit_statuses: &[],
            },
            &c,
            now,
            &mut vec![],
        );
    };
    contact(&mut w, 0, "a", 30);
    contact(&mut w, 0, "a", 60);
    assert_eq!(w.members[0].spirit, 100);
    contact(&mut w, 1, "a", 60);
    assert_eq!(w.members[1].spirit, 100);
    contact(&mut w, 0, "b", 90);
    assert_eq!(w.members[0].spirit, 107);
    contact(&mut w, 0, "c", 120);
    contact(&mut w, 0, "d", 150);
    contact(&mut w, 0, "d", 450);
    assert_eq!(w.members[0].spirit, 107);
    contact(&mut w, 0, "e", 480);
    assert_eq!(w.members[0].spirit, 114);
    w.clear_owned_statuses(
        &w.members[0].reference.entity.clone(),
        StatusRemovalReason::SourceInvalid,
        &mut vec![],
    );
    contact(&mut w, 0, "a", 510);
    assert_eq!(w.members[0].spirit, 114);
}

#[test]
fn combat_action_counting_rejects_ambiguous_periodic_or_unknown_counting_rules() {
    let package = Package::new();
    let mut v = document();
    v["combat_rules"]["triggers"][0]["counting"] = json!("action");
    package.write(&v);
    package.load().unwrap();
    for (key, value) in [
        ("include_periodic", json!(true)),
        ("counting", json!("arbitrary")),
    ] {
        let mut bad = v.clone();
        bad["combat_rules"]["triggers"][0][key] = value;
        package.write(&bad);
        assert!(package.load().is_err());
    }
}

#[test]
fn combat_d03_source_mapping_changes_movement_and_keeps_maximum_hp_unchanged() {
    let fixture: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_status_mappings.json"
    ))
    .unwrap();
    for row in fixture["statuses"].as_array().unwrap() {
        let (mut c, mut w) = setup();
        let definition = |modifier, duration| json!({"id":row["id"],"mechanism":"timed_attribute_modifier","attribute":row["attribute"],"modifier_bps":modifier,"duration_ms":duration,"duplicate_policy":"refresh","source_scoped":true});
        let target = w.members[0].reference.entity.clone();
        c.pve_mut().status_definitions.push(
            serde_json::from_value(definition(row["modifier_bps"].as_i64().unwrap(), 300)).unwrap(),
        );
        let source = if row["attribute"] == "movement_haste" {
            Owner::Member(0)
        } else {
            Owner::Npc(0)
        };
        bind(
            &mut w,
            &c,
            source,
            target.clone(),
            row["id"].as_str().unwrap(),
            0,
        );
        assert_eq!(
            w.statuses.movement_factor_bps(&status_entity(&target), 0),
            (10000 + row["modifier_bps"].as_i64().unwrap()) as u32
        );
        assert_eq!(w.entity_max_hp(&target, &c, 0), Some(1000));
        if let Some(qualities) = row["quality"].as_array() {
            for quality in qualities {
                let d = serde_json::from_value(definition(
                    quality["modifier_bps"].as_i64().unwrap(),
                    quality["duration_ms"].as_u64().unwrap(),
                ))
                .unwrap();
                w.apply_owned_status_definitions(source, target.clone(), &[d], &c, 30, &mut vec![]);
                assert_eq!(
                    w.statuses.movement_factor_bps(&status_entity(&target), 30),
                    10000 + quality["modifier_bps"].as_u64().unwrap() as u32
                );
                let s = w
                    .statuses
                    .entries()
                    .iter()
                    .find(|s| s.definition_id == row["id"])
                    .unwrap();
                assert_eq!(
                    s.expires_at_ms - 30,
                    quality["duration_ms"].as_u64().unwrap()
                );
                assert_eq!(w.entity_max_hp(&target, &c, 30), Some(1000));
            }
        }
    }
}

fn attack_speed_definition(c: &mut Config, id: &str, bps: i32, duration: u64, stack: bool) {
    let mut d = json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"attack_speed","modifier_bps":bps,"duration_ms":duration,"duplicate_policy":if stack {"stack"} else {"refresh"},"source_scoped":true});
    if stack {
        d["max_stacks"] = json!(3);
    }
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
}

#[test]
fn combat_attack_speed_mapping_changes_progress_not_attack_movement_or_skill_clocks() {
    let (mut c, mut w) = setup();
    let fixture: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_status_mappings.json"
    ))
    .unwrap();
    let mapping = &fixture["attackSpeed"];
    attack_speed_definition(
        &mut c,
        mapping["id"].as_str().unwrap(),
        mapping["modifier_bps"].as_i64().unwrap() as i32,
        mapping["duration_ms"].as_u64().unwrap(),
        true,
    );
    assert_eq!(mapping["max_stacks"], 3);
    let target = w.members[0].reference.entity.clone();
    let facts = w.members[0].facts.clone();
    w.members[0].cooldowns.insert(c.skill.id.clone(), 1500);
    bind(
        &mut w,
        &c,
        Owner::Npc(0),
        target.clone(),
        "effect_zhenlong_suppress",
        0,
    );
    w.start_attack_clock(Owner::Member(0), 1000, &c, 0);
    assert_eq!(w.members[0].basic_ready, 1429);
    assert_eq!(w.stats(0, &c, 0), (100, 100));
    assert_eq!(w.effective_spd(Owner::Member(0), &c, 0), 8);
    assert_eq!(
        w.statuses.movement_factor_bps(&status_entity(&target), 0),
        10000
    );
    assert_eq!(w.members[0].cooldowns[&c.skill.id], 1500);
    assert_eq!(w.members[0].facts, facts);
    let a = w.snapshot(0, &c, Phase::Active, 30);
    let b = w.snapshot(0, &c, Phase::Active, 30);
    assert_eq!(a, b);
    assert_eq!(a.own.basic_ready_at_ms, 1429);
}

#[test]
fn combat_attack_speed_accounts_for_exact_expiry_between_ticks_and_zero_rate() {
    for (bps, expected) in [(-3000, 1090), (-10000, 1300), (10000, 700)] {
        let (mut c, mut w) = setup();
        attack_speed_definition(&mut c, "tempo", bps, 300, false);
        let target = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Npc(0), target, "tempo", 0);
        w.start_attack_clock(Owner::Member(0), 1000, &c, 0);
        assert_eq!(w.members[0].basic_ready, expected);
        for now in [90, 210, 450, 600, expected] {
            w.reconcile_caps(&c, now);
            assert_eq!(w.members[0].basic_ready, expected);
        }
        assert!(w.members[0].attack_clock.is_none());
    }
}

#[test]
fn combat_attack_speed_mid_cooldown_refresh_and_source_exit_preserve_completed_work() {
    let (mut c, mut w) = setup();
    attack_speed_definition(&mut c, "tempo", -3000, 2000, false);
    w.start_attack_clock(Owner::Member(0), 1000, &c, 0);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), target.clone(), "tempo", 300);
    assert_eq!(w.members[0].basic_ready, 1300);
    bind(&mut w, &c, Owner::Member(1), target, "tempo", 600);
    assert_eq!(w.members[0].basic_ready, 1300);
    w.action(1, &Action::Leave {}, "source-exit", &c, 600)
        .unwrap();
    assert_eq!(w.members[0].basic_ready, 1090);
    assert_eq!(w.members[0].facts.atk_points, 100);
}

#[test]
fn combat_attack_speed_stack_and_multiple_sources_use_finite_zero_rate_and_configured_positive_cap()
{
    let (mut c, mut w) = setup();
    attack_speed_definition(&mut c, "tempo", -3000, 5000, true);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "tempo", 0);
    w.start_attack_clock(Owner::Member(0), 1000, &c, 0);
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "tempo", 400);
    assert_eq!(w.members[0].basic_ready, 2200);
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "tempo", 800);
    assert_eq!(w.members[0].basic_ready, 5860);
    bind(&mut w, &c, Owner::Member(1), target, "tempo", 800);
    assert_eq!(w.members[0].basic_ready, 6360);
    let (mut c, mut w) = setup();
    attack_speed_definition(&mut c, "tempo", 10000, 5000, false);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .attribute_caps_bps
        .insert(StatusAttribute::AttackSpeed, 5000);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "tempo", 0);
    w.start_attack_clock(Owner::Member(0), 1000, &c, 0);
    assert_eq!(w.members[0].basic_ready, 667);
}

#[test]
fn combat_attack_speed_updates_buffered_basic_promotion_once_and_keeps_target_identity() {
    let (mut c, mut w) = setup();
    attack_speed_definition(&mut c, "tempo", -3000, 2000, false);
    c.basic_followup = Some(serde_json::from_value(json!({"grace_ms":150})).unwrap());
    c.basic.windup_ms = 30;
    c.basic.range = 240.;
    w.start_attack_clock(Owner::Member(0), 1000, &c, 0);
    w.action(0, &Action::Attack {}, "queued", &c, 30).unwrap();
    let before = w.members[0].queued_basic.clone().unwrap();
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), target, "tempo", 300);
    let queued = w.members[0].queued_basic.as_ref().unwrap();
    assert_eq!((queued.promotes_at_ms, queued.expires_at_ms), (1320, 1470));
    assert_eq!(
        (&queued.action_id, &queued.target),
        (&before.action_id, &before.target)
    );
    w.action(1, &Action::Leave {}, "source-exit", &c, 600)
        .unwrap();
    let queued = w.members[0].queued_basic.as_ref().unwrap();
    assert_eq!((queued.promotes_at_ms, queued.expires_at_ms), (1110, 1260));
    w.reconcile_caps(&c, 1110);
    w.promote_basic(0, &c, 1110, &mut vec![]);
    assert!(w.members[0].queued_basic.is_none());
    assert_eq!(w.members[0].pending.as_ref().unwrap().id, "queued");
    w.promote_basic(0, &c, 1110, &mut vec![]);
    let p = w.members[0].pending.take().unwrap();
    w.commit(0, p, &c, 1140, &mut vec![]);
    assert_eq!(w.shots.len(), 1);
}

#[test]
fn combat_attack_speed_npc_melee_uses_same_clock_while_skill_timing_is_unchanged() {
    let (mut c, mut w) = setup();
    attack_speed_definition(&mut c, "tempo", -3000, 5000, false);
    let profile = Arc::make_mut(&mut w.npcs[0].profile);
    profile.combat.attack_interval_ms = 1000;
    profile.combat.melee_range = 100.;
    profile.combat.cast_chance = 0.;
    w.npcs[0].next_attack = 0;
    w.npcs[0].casting = None;
    let target = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "tempo", 0);
    w.advance_npcs(&c, 30, 0.03, &mut vec![]);
    assert_eq!(w.npcs[0].next_attack, 1459);
    let hp = w.members[0].hp;
    w.reconcile_caps(&c, 1440);
    w.advance_npcs(&c, 1440, 0.03, &mut vec![]);
    assert_eq!(w.members[0].hp, hp);
    w.reconcile_caps(&c, 1470);
    w.advance_npcs(&c, 1470, 0.03, &mut vec![]);
    assert!(w.members[0].hp < hp);
    assert_eq!(w.npcs[0].next_attack, 2899);
}

#[test]
fn combat_attack_speed_scalar_compound_and_probability_caps_reject_unsupported_configs() {
    let p = Package::new();
    let mut v = document();
    let d = json!({"id":"tempo","mechanism":"timed_attribute_modifier","attribute":"attack_speed","modifier_bps":-3000,"duration_ms":5000,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true});
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(d);
    p.write(&v);
    p.load().unwrap();
    let i = v["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    for case in 0..4 {
        let mut bad = v.clone();
        match case {
            0 => {
                bad["mechanism_version"] = json!(104);
                bad.as_object_mut().unwrap().remove("combat_rules");
            }
            1 => bad["pve"]["status_definitions"][i]["modifier_bps"] = json!(-10001),
            2 => bad["pve"]["status_rules"]["attribute_caps_bps"]["attack_speed"] = json!(1000001),
            _ => {
                bad["pve"]["status_rules"]["attribute_caps_bps"]["harmful_duration_reduction"] =
                    json!(10001)
            }
        };
        p.write(&bad);
        assert!(p.load().is_err(), "{case}");
    }
    v["pve"]["status_definitions"][i] = json!({"id":"tempo","mechanism":"timed_compound_modifier","duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"attack_speed","modifier_bps":-3000},{"attribute":"def","modifier_bps":1000}]});
    p.write(&v);
    p.load().unwrap();
    v["member_item_use"] = json!({v["item_id"].as_str().unwrap():{"grant_statuses_without_recovery":true,"on_use_self_statuses":["tempo"],"on_use_self_status_overrides":{"tempo":{"modifier_bps":0,"duration_ms":900,"modifiers":[{"attribute":"attack_speed","modifier_bps":-2000},{"attribute":"def","modifier_bps":1000}]}}}});
    p.write(&v);
    p.load().unwrap();
}

fn record_rule() -> Value {
    json!({"basis":"incoming","cap_fixed":150,"cap_power_bps":0,"damage_reduction_bps":2000,"refresh":"reset","on_depleted":true,"on_expired":true,"on_consumed":true,"counter":{"ratio_bps":6000,"damage_kind":"true_damage","ignore_armor":false,"radius":300,"max_targets":8,"on_hit_target_statuses":[]},"after_end_target_statuses":[]})
}
fn record_shield(c: &mut Config, id: &str, bps: u32, duration: u64, rule: Value) {
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"absorption_shield","shield_bps":bps,"duration_ms":duration,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules
        .as_mut()
        .unwrap()
        .shield_records
        .insert(id.into(), serde_json::from_value(rule).unwrap());
}
fn record_hit(
    w: &mut World,
    c: &Config,
    raw: u32,
    now: u64,
) -> (super::periodic::DamageResult, Events) {
    let mut events = vec![];
    let result = w.resolve_damage_with_rule(
        DamageIntent {
            ignore_armor: false,
            source: w.npcs[0].entity.clone(),
            target: w.members[0].reference.entity.clone(),
            raw,
            direct: false,
        },
        Some(&AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            can_crit: false,
            ..Default::default()
        }),
        c,
        now,
        &mut events,
    );
    (result, events)
}
fn recorded(w: &World, id: &str) -> ShieldRecordView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == id)
        .unwrap()
        .shield
        .as_ref()
        .unwrap()
        .record
        .as_ref()
        .unwrap()
        .clone()
}

#[test]
fn combat_record_shield_conserves_incoming_shield_hp_and_releases_actual_capped_ledger_once() {
    let (mut c, mut w) = setup();
    record_shield(&mut c, "guard", 2000, 600, record_rule());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "guard", 0);
    let (a, _) = record_hit(&mut w, &c, 100, 30);
    assert_eq!((a.absorbed, a.hp_damage), (100, 0));
    assert_eq!(recorded(&w, "guard").recorded, 100);
    let (b, events) = record_hit(&mut w, &c, 150, 60);
    assert_eq!((b.absorbed, b.hp_damage), (100, 50));
    assert_eq!(w.members[0].hp, 950);
    assert_eq!(w.npcs[0].hp, 910);
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.definition_id != "guard")
    );
    assert_eq!(
        events
            .iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::ShieldRecordEnded {
                        recorded: 150,
                        raw_counter: 90,
                        targets_hit: 1,
                        suppressed: None,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
    let hp = w.npcs[0].hp;
    w.finish_shield_records(&c, 60, &mut vec![]);
    assert_eq!(w.npcs[0].hp, hp);
}

#[test]
fn combat_record_shield_reduction_precedes_incoming_record_but_own_absorbed_uses_pool_allocation() {
    for basis in ["incoming", "own_absorbed"] {
        let (mut c, mut w) = setup();
        let mut rule = record_rule();
        rule["basis"] = json!(basis);
        rule["cap_fixed"] = json!(1000);
        rule["counter"] = Value::Null;
        // Optional rules reject explicit null at load; native fixture omits it.
        rule.as_object_mut().unwrap().remove("counter");
        record_shield(&mut c, "guard", 2000, 600, rule);
        c.pve_mut()
            .status_definitions
            .iter_mut()
            .find(|d| d.id == "barrier")
            .unwrap()
            .duration_ms = 300;
        let target = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), target.clone(), "barrier", 0);
        bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
        w.members[0].facts.def_points = 0;
        c.combat_rules
            .as_mut()
            .unwrap()
            .profiles
            .get_mut("npc")
            .unwrap()
            .tier = 2;
        let mut events = vec![];
        let hit = DamageIntent {
            ignore_armor: true,
            source: w.npcs[0].entity.clone(),
            target,
            raw: 100,
            direct: false,
        };
        let result = w.resolve_damage_with_rule(
            hit,
            Some(&AttackRule {
                can_crit: false,
                ..Default::default()
            }),
            &c,
            30,
            &mut events,
        );
        assert_eq!((result.absorbed, result.hp_damage), (80, 0));
        // The earlier barrier absorbs 10 first; only the guard's own allocation is 70.
        assert_eq!(
            recorded(&w, "guard").recorded,
            if basis == "incoming" { 80 } else { 70 },
            "{basis}"
        );
        assert_eq!(
            w.statuses
                .shield_total(&status_entity(&w.members[0].reference.entity), 30),
            130
        );
    }
}

#[test]
fn combat_record_shield_refresh_freezes_power_resets_or_preserves_and_clamps_to_new_cap() {
    let (mut c, mut w) = setup();
    let mut rule = record_rule();
    rule["cap_fixed"] = json!(0);
    rule["cap_power_bps"] = json!(10000);
    record_shield(&mut c, "guard", 2000, 600, rule);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
    let original = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "guard")
        .unwrap()
        .instance_id
        .clone();
    record_hit(&mut w, &c, 80, 30);
    assert_eq!(recorded(&w, "guard").cap, 125);
    w.members[0].facts.atk_points = 1000;
    assert_eq!(recorded(&w, "guard").cap, 125);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 60);
    assert_eq!(
        (recorded(&w, "guard").recorded, recorded(&w, "guard").cap),
        (0, 1250)
    );
    record_hit(&mut w, &c, 80, 90);
    c.combat_rules
        .as_mut()
        .unwrap()
        .shield_records
        .get_mut("guard")
        .unwrap()
        .refresh = crate::config::RecordRefresh::Preserve;
    w.members[0].facts.atk_points = 40;
    bind(&mut w, &c, Owner::Member(0), target, "guard", 120);
    assert_eq!(
        (recorded(&w, "guard").recorded, recorded(&w, "guard").cap),
        (50, 50)
    );
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "guard")
            .unwrap()
            .instance_id,
        original
    );
}

#[test]
fn combat_record_shield_end_causes_are_explicit_and_cleanup_never_releases() {
    for reason in [
        StatusRemovalReason::Expired,
        StatusRemovalReason::Depleted,
        StatusRemovalReason::Consumed,
        StatusRemovalReason::Cleansed,
        StatusRemovalReason::SourceInvalid,
        StatusRemovalReason::TargetInvalid,
        StatusRemovalReason::Reset,
        StatusRemovalReason::Closed,
    ] {
        let (mut c, mut w) = setup();
        record_shield(&mut c, "guard", 2000, 600, record_rule());
        let target = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
        w.statuses.record_incoming(&status_entity(&target), 100, 30);
        w.statuses
            .remove(reason, |s| s.definition_id == "guard", &mut vec![]);
        let mut events = vec![];
        w.finish_shield_records(&c, 60, &mut events);
        w.finish_shield_records(&c, 60, &mut events);
        let enabled = matches!(
            reason,
            StatusRemovalReason::Expired
                | StatusRemovalReason::Depleted
                | StatusRemovalReason::Consumed
        );
        assert_eq!(w.npcs[0].hp, if enabled { 940 } else { 1000 }, "{reason:?}");
        assert_eq!(
            events
                .iter()
                .filter(|(_, e)| matches!(
                    e,
                    EventV4::Private {
                        event: Event::ShieldRecordEnded { .. }
                    }
                ))
                .count(),
            1
        );
    }
    let (mut c, mut w) = setup();
    let mut rule = record_rule();
    rule["on_expired"] = json!(false);
    record_shield(&mut c, "guard", 2000, 300, rule);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
    w.statuses.record_incoming(&status_entity(&target), 100, 30);
    w.prepare_periodic(300, &mut vec![]);
    w.finish_shield_records(&c, 300, &mut vec![]);
    assert_eq!(w.npcs[0].hp, 1000);
}

#[test]
fn combat_record_shield_fatal_damage_and_protection_cannot_release_or_apply_aftereffects() {
    for fatal in [true, false] {
        let (mut c, mut w) = setup();
        record_shield(&mut c, "guard", 2000, 600, record_rule());
        let target = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
        if fatal {
            record_hit(&mut w, &c, 2000, 30);
            assert_eq!(w.members[0].hp, 0);
        } else {
            w.statuses.record_incoming(&status_entity(&target), 100, 30);
            w.members[0].protected = true;
            w.prepare_periodic(600, &mut vec![]);
            w.finish_shield_records(&c, 600, &mut vec![]);
        }
        assert_eq!(w.npcs[0].hp, 1000);
        assert!(
            w.statuses
                .entries()
                .iter()
                .all(|s| s.definition_id != "guard")
        );
    }
}

#[test]
fn combat_record_shield_derived_release_cannot_record_retrigger_or_counter_back() {
    let (mut c, mut w) = setup();
    record_shield(&mut c, "guard", 2000, 600, record_rule());
    record_shield(&mut c, "npc-guard", 500, 600, record_rule());
    let member = w.members[0].reference.entity.clone();
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), member.clone(), "guard", 0);
    bind(&mut w, &c, Owner::Npc(0), npc.clone(), "npc-guard", 0);
    w.statuses.record_incoming(&status_entity(&npc), 100, 0);
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"must-not-proc","profiles":["npc"],"when":"damage_taken","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"restore_spirit","amount":7}}),
    );
    let (result, events) = record_hit(&mut w, &c, 200, 30);
    assert_eq!((result.absorbed, result.hp_damage), (200, 0));
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.npcs[0].hp, 960);
    assert!(!events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::CombatTriggerFired { .. }
        }
    )));
    assert!(events.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::ShieldRecordEnded{definition_id,suppressed:Some(Error::InvalidState),recorded:100,..}} if definition_id=="npc-guard")));
}

#[test]
fn combat_record_shield_ally_source_exit_and_private_ledger_are_isolated() {
    let (mut c, mut w) = setup();
    record_shield(&mut c, "guard", 2000, 600, record_rule());
    let target = w.members[1].reference.entity.clone();
    let applied = bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
    assert!(applied.iter().all(|(i, _)| matches!(i, Some(0) | Some(1))));
    w.statuses.record_incoming(&status_entity(&target), 100, 30);
    w.action(0, &Action::Leave {}, "caster-exit", &c, 60)
        .unwrap();
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.definition_id != "guard")
    );
    assert_eq!(w.members[1].hp, 1100);
    assert_eq!(w.npcs[0].hp, 1000);
}

#[test]
fn combat_record_shield_release_budget_is_bounded_and_spent_records_do_not_retry() {
    let (mut c, mut w) = setup();
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_shield_releases_per_step = 1;
    record_shield(&mut c, "guard", 2000, 300, record_rule());
    for i in 0..2 {
        let target = w.members[i].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(i), target.clone(), "guard", 0);
        w.statuses.record_incoming(&status_entity(&target), 100, 30);
    }
    w.prepare_periodic(300, &mut vec![]);
    let mut events = vec![];
    w.finish_shield_records(&c, 300, &mut events);
    assert_eq!(w.npcs[0].hp, 940);
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ShieldRecordEnded {
                suppressed: Some(Error::Capacity),
                ..
            }
        }
    )));
    w.finish_shield_records(&c, 330, &mut events);
    assert_eq!(w.npcs[0].hp, 940);
}

#[test]
fn combat_record_shield_loading_rejects_invalid_references_and_bounds() {
    let package = Package::new();
    let mut v = document();
    v["combat_rules"]["shield_records"] = json!({"ward":record_rule()});
    package.write(&v);
    package.load().unwrap();
    for case in 0..8 {
        let mut bad = v.clone();
        match case {
            0 => bad["combat_rules"]["shield_records"] = json!({"renewal":record_rule()}),
            1 => bad["combat_rules"]["shield_records"]["ward"]["cap_power_bps"] = json!(100001),
            2 => bad["combat_rules"]["shield_records"]["ward"]["counter"]["radius"] = json!(0),
            3 => {
                bad["combat_rules"]["shield_records"]["ward"]["counter"]["max_targets"] = json!(33)
            }
            4 => {
                bad["combat_rules"]["shield_records"]["ward"]["counter"]["damage_kind"] =
                    json!("elemental")
            }
            5 => {
                bad["combat_rules"]["shield_records"]["ward"]["after_end_target_statuses"] =
                    json!(["missing"])
            }
            6 => bad["combat_rules"]["max_shield_releases_per_step"] = json!(0),
            _ => bad["combat_rules"]["shield_records"]["ward"]["counter"] = Value::Null,
        };
        package.write(&bad);
        assert!(package.load().is_err(), "{case}");
    }
}

fn charge_definition(c: &mut Config, id: &str) {
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":1000,"duration_ms":1500,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true})).unwrap());
}
fn fact(w: &mut World, c: &Config, damage: u32, now: u64) -> Events {
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DamageDealt,
        (Owner::Member(0), Owner::Npc(0)),
        (damage, false),
        c,
        now,
        &mut e,
    );
    e
}
#[test]
fn combat_trigger_conditions_and_partial_consumption_preserve_expiry_source_and_icd() {
    let (mut c, mut w) = setup();
    charge_definition(&mut c, "charge");
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"spend","profiles":["member"],"when":"damage_dealt","include_periodic":false,"every":2,"cooldown_ms":600,"priority":0,"chance_bps":10000,"conditions":{"minimum_amount":20,"owner_hp_at_most_bps":5000,"status":{"subject":"owner","definition_id":"charge","at_least":2,"consume":1}},"effect":{"kind":"restore_spirit","amount":7}}),
    );
    w.members[0].hp = 600;
    w.members[0].spirit = 100;
    fact(&mut w, &c, 100, 30);
    w.members[0].hp = 500;
    fact(&mut w, &c, 10, 60);
    fact(&mut w, &c, 100, 90);
    let target = w.members[0].reference.entity.clone();
    for _ in 0..2 {
        bind(&mut w, &c, Owner::Member(1), target.clone(), "charge", 90);
    }
    fact(&mut w, &c, 100, 120);
    assert_eq!(w.members[0].spirit, 100);
    for _ in 0..2 {
        bind(&mut w, &c, Owner::Member(0), target.clone(), "charge", 120);
    }
    fact(&mut w, &c, 100, 150);
    assert_eq!(w.members[0].spirit, 100);
    let e = fact(&mut w, &c, 100, 180);
    assert_eq!(w.members[0].spirit, 107);
    let own = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "charge" && s.source == status_entity(&target))
        .unwrap();
    assert_eq!((own.stacks, own.expires_at_ms), (Some(1), 1620));
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusConsumed {
                consumed_stacks: 1,
                remaining_stacks: 1,
                ..
            }
        }
    )));
    bind(&mut w, &c, Owner::Member(0), target, "charge", 210);
    fact(&mut w, &c, 100, 240);
    fact(&mut w, &c, 100, 270);
    assert_eq!(w.members[0].spirit, 107);
    fact(&mut w, &c, 100, 800);
    assert_eq!(w.members[0].spirit, 114);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(
                |s| s.source == status_entity(&w.members[1].reference.entity)
                    && s.definition_id == "charge"
            )
            .unwrap()
            .stacks,
        Some(2)
    );
}

#[test]
fn combat_trigger_other_subject_consumes_only_the_attackers_own_marks() {
    let (mut c, mut w) = setup();
    charge_definition(&mut c, "mark");
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"spend-mark","profiles":["member"],"when":"damage_dealt","include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"conditions":{"status":{"subject":"other","definition_id":"mark","at_least":2,"consume":2}},"effect":{"kind":"restore_spirit","amount":7}}),
    );
    let target = w.npcs[0].entity.clone();
    for owner in [Owner::Member(0), Owner::Member(1)] {
        for _ in 0..2 {
            bind(&mut w, &c, owner, target.clone(), "mark", 0);
        }
    }
    w.members[0].spirit = 100;
    let events = fact(&mut w, &c, 100, 30);
    assert_eq!(w.members[0].spirit, 107);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .filter(|s| s.definition_id == "mark")
            .count(),
        1
    );
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "mark")
        .unwrap();
    assert_eq!(s.source, status_entity(&w.members[1].reference.entity));
    assert_eq!(s.stacks, Some(2));
    assert!(events.iter().all(|(recipient, _)| *recipient == Some(0)));
}

#[test]
fn combat_trigger_capacity_preflight_accounts_for_a_consumed_slot_and_does_not_start_icd_on_failure()
 {
    for consume in [1, 2] {
        let (mut c, mut w) = setup();
        charge_definition(&mut c, "charge");
        c.combat_rules.as_mut().unwrap().triggers.clear();
        trigger(
            &mut c,
            json!({"id":"convert","profiles":["member"],"when":"damage_dealt","include_periodic":false,"every":1,"cooldown_ms":1000,"priority":0,"conditions":{"status":{"subject":"owner","definition_id":"charge","at_least":2,"consume":consume}},"effect":{"kind":"apply_self_status","status_id":"renewal"}}),
        );
        let target = w.members[0].reference.entity.clone();
        for _ in 0..2 {
            bind(&mut w, &c, Owner::Member(0), target.clone(), "charge", 0);
        }
        c.pve_mut()
            .status_rules
            .as_mut()
            .unwrap()
            .max_active_per_entity = 1;
        let e = fact(&mut w, &c, 100, 30);
        if consume == 1 {
            assert_eq!(
                w.statuses
                    .entries()
                    .iter()
                    .find(|s| s.definition_id == "charge")
                    .unwrap()
                    .stacks,
                Some(2)
            );
            assert!(!e.iter().any(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::CombatTriggerFired { .. } | Event::StatusConsumed { .. }
                }
            )));
            c.pve_mut()
                .status_rules
                .as_mut()
                .unwrap()
                .max_active_per_entity = 2;
            fact(&mut w, &c, 100, 60);
            assert_eq!(
                w.statuses
                    .entries()
                    .iter()
                    .find(|s| s.definition_id == "charge")
                    .unwrap()
                    .stacks,
                Some(1)
            );
        } else {
            assert!(
                w.statuses
                    .entries()
                    .iter()
                    .all(|s| s.definition_id != "charge")
            );
        }
        assert!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "renewal")
        );
    }
}

#[test]
fn combat_trigger_probability_and_action_dedup_do_not_consume_twice_or_use_rng_at_endpoints() {
    let (mut c, mut w) = setup();
    charge_definition(&mut c, "charge");
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"spend","profiles":["member"],"when":"damage_dealt","include_periodic":false,"counting":"action","every":1,"cooldown_ms":0,"chance_bps":0,"priority":0,"conditions":{"status":{"subject":"owner","definition_id":"charge","at_least":1,"consume":1}},"effect":{"kind":"restore_spirit","amount":7}}),
    );
    let target = w.members[0].reference.entity.clone();
    for _ in 0..3 {
        bind(&mut w, &c, Owner::Member(0), target.clone(), "charge", 0);
    }
    let rng = w.rng;
    w.members[0].spirit = 100;
    w.triggers.action = Some("miss".into());
    fact(&mut w, &c, 100, 30);
    assert_eq!(w.rng, rng);
    assert_eq!(w.members[0].spirit, 100);
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(10000);
    w.triggers.action = Some("field".into());
    fact(&mut w, &c, 100, 60);
    fact(&mut w, &c, 100, 90);
    assert_eq!(w.members[0].spirit, 107);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "charge")
            .unwrap()
            .stacks,
        Some(2)
    );
    assert_eq!(w.rng, rng);
    w.triggers.action = Some("next".into());
    fact(&mut w, &c, 100, 120);
    assert_eq!(w.members[0].spirit, 114);
}

#[test]
fn combat_trigger_full_shield_consumption_uses_consumed_end_policy_once() {
    let (mut c, mut w) = setup();
    record_shield(&mut c, "guard", 2000, 600, record_rule());
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"release","profiles":["member"],"when":"skill_committed","include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"conditions":{"status":{"subject":"owner","definition_id":"guard","at_least":1,"consume":1}},"effect":{"kind":"restore_spirit","amount":7}}),
    );
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
    w.statuses.record_incoming(&status_entity(&target), 100, 0);
    w.members[0].spirit = 100;
    let mut events = vec![];
    w.combat_fact(
        crate::config::CombatFact::SkillCommitted,
        (Owner::Member(0), Owner::Member(0)),
        (0, false),
        &c,
        30,
        &mut events,
    );
    w.finish_shield_records(&c, 30, &mut events);
    assert_eq!(w.npcs[0].hp, 940);
    assert_eq!(w.members[0].spirit, 107);
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.definition_id != "guard")
    );
    w.finish_shield_records(&c, 60, &mut events);
    assert_eq!(w.npcs[0].hp, 940);
}

#[test]
fn combat_trigger_condition_loader_rejects_missing_refs_unknown_subject_and_invalid_thresholds() {
    let package = Package::new();
    let mut v = document();
    v["combat_rules"]["triggers"][0]["conditions"] = json!({"status":{"subject":"owner","definition_id":"battle-pose","at_least":1,"consume":1},"owner_hp_at_most_bps":5000});
    v["combat_rules"]["triggers"][0]["chance_bps"] = json!(5000);
    package.write(&v);
    package.load().unwrap();
    for case in 0..7 {
        let mut bad = v.clone();
        match case {
            0 => {
                bad["combat_rules"]["triggers"][0]["conditions"]["status"]["definition_id"] =
                    json!("missing")
            }
            1 => {
                bad["combat_rules"]["triggers"][0]["conditions"]["status"]["subject"] =
                    json!("client_selected")
            }
            2 => bad["combat_rules"]["triggers"][0]["conditions"]["status"]["at_least"] = json!(0),
            3 => bad["combat_rules"]["triggers"][0]["conditions"]["status"]["consume"] = json!(2),
            4 => {
                bad["combat_rules"]["triggers"][0]["conditions"]["owner_hp_at_most_bps"] =
                    json!(10001)
            }
            5 => bad["combat_rules"]["triggers"][0]["chance_bps"] = json!(10001),
            _ => bad["combat_rules"]["triggers"][0]["chance_bps"] = Value::Null,
        };
        package.write(&bad);
        assert!(package.load().is_err(), "{case}");
    }
}

#[test]
fn combat_record_shield_end_effects_share_the_original_source_and_do_not_persist_guard_reduction() {
    let (mut c, mut w) = setup();
    for d in [
        json!({"id":"linger","mechanism":"timed_attribute_modifier","attribute":"damage_reduction","modifier_bps":1500,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"counter-slow","mechanism":"timed_attribute_modifier","attribute":"movement_slow","modifier_bps":-3000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    ] {
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(d).unwrap());
    }
    let mut rule = record_rule();
    rule["after_end_target_statuses"] = json!(["linger"]);
    rule["counter"]["on_hit_target_statuses"] = json!(["counter-slow"]);
    record_shield(&mut c, "guard", 2000, 300, rule);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
    w.statuses.record_incoming(&status_entity(&target), 100, 30);
    w.prepare_periodic(300, &mut vec![]);
    w.finish_shield_records(&c, 300, &mut vec![]);
    assert_eq!(
        w.statuses.damage_reduction_bps(
            &status_entity(&target),
            300,
            c.pve.as_ref().unwrap().status_rules.as_ref()
        ),
        1500
    );
    assert_eq!(w.npc_movement_factor(0, &c, 300), 0.7);
    assert!(
        w.statuses
            .entries()
            .iter()
            .filter(|s| s.definition_id == "linger" || s.definition_id == "counter-slow")
            .all(|s| s.source == status_entity(&target))
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.definition_id != "guard")
    );
}

#[test]
fn combat_record_shield_own_absorption_and_multiple_sources_do_not_share_balances() {
    let (mut c, mut w) = setup();
    let mut rule = record_rule();
    rule["basis"] = json!("own_absorbed");
    rule["cap_fixed"] = json!(1000);
    rule.as_object_mut().unwrap().remove("counter");
    record_shield(&mut c, "guard", 2000, 600, rule);
    let target = w.members[0].reference.entity.clone();
    for source in [Owner::Member(0), Owner::Member(1)] {
        bind(&mut w, &c, source, target.clone(), "guard", 0);
    }
    let (hit, events) = record_hit(&mut w, &c, 250, 30);
    assert_eq!((hit.absorbed, hit.hp_damage), (250, 0));
    let remaining = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "guard")
        .unwrap();
    assert_eq!(
        remaining.source,
        status_entity(&w.members[1].reference.entity)
    );
    assert_eq!(
        remaining
            .shield
            .as_ref()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .recorded,
        50
    );
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ShieldRecordEnded { recorded: 200, .. }
        }
    )));
}

#[test]
fn combat_record_shield_source_guard_vectors_bind_real_pool_reduction_and_frozen_power_cap() {
    let fixture: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_guard_vectors.json"
    ))
    .unwrap();
    for row in fixture["vectors"].as_array().unwrap() {
        let (mut c, mut w) = setup();
        let p = &row["sourceFactor"]["params"];
        c.combat_rules
            .as_mut()
            .unwrap()
            .profiles
            .get_mut("member")
            .unwrap()
            .power_points_bps = 0;
        c.combat_rules
            .as_mut()
            .unwrap()
            .profiles
            .get_mut("npc")
            .unwrap()
            .tier = 2;
        w.members[0].facts.def_points = 0;
        let mut rule = record_rule();
        rule["cap_fixed"] = json!(0);
        rule["cap_power_bps"] = p[3].clone();
        rule["damage_reduction_bps"] = p[1].clone();
        record_shield(
            &mut c,
            "source-guard",
            p[0].as_u64().unwrap() as u32,
            p[2].as_u64().unwrap(),
            rule,
        );
        let target = w.members[0].reference.entity.clone();
        bind(
            &mut w,
            &c,
            Owner::Member(0),
            target.clone(),
            "source-guard",
            0,
        );
        let s = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "source-guard")
            .unwrap();
        assert_eq!(
            s.shield.as_ref().unwrap().granted,
            row["expected_shield"].as_u64().unwrap() as u32
        );
        assert_eq!(
            recorded(&w, "source-guard").cap,
            row["expected_record_cap"].as_u64().unwrap() as u32
        );
        let result = w.resolve_damage_with_rule(
            DamageIntent {
                source: w.npcs[0].entity.clone(),
                target,
                raw: 100,
                direct: false,
                ignore_armor: true,
            },
            Some(&AttackRule {
                can_crit: false,
                ..Default::default()
            }),
            &c,
            30,
            &mut vec![],
        );
        assert_eq!(
            result.absorbed,
            row["expected_incoming_from_100"].as_u64().unwrap() as u32
        );
        assert_eq!(recorded(&w, "source-guard").recorded, result.absorbed);
    }
}

#[test]
fn combat_record_shield_counter_target_order_and_attempt_budget_include_immune_contacts() {
    for immune in [false, true] {
        let (mut c, mut w) = setup();
        let mut rule = record_rule();
        rule["counter"]["max_targets"] = json!(if immune { 2 } else { 1 });
        c.combat_rules
            .as_mut()
            .unwrap()
            .max_shield_counter_hits_per_step = 1;
        record_shield(&mut c, "guard", 2000, 300, rule);
        let at = [w.members[0].position[0] + 30., w.members[0].position[1]];
        w.npcs[0].entity.id = "z".into();
        w.npcs[0].position = at;
        let first = &w.npcs[0];
        w.npcs.push(Npc {
            training_owner: None,
            training_active: false,
            training_reverse: false,
            entity: EntityRef {
                kind: StatusEntityKind::Monster,
                id: "a".into(),
                generation: 1,
            },
            hp: 1000,
            spirit: 100,
            profile: first.profile.clone(),
            template_id: first.template_id.clone(),
            position: at,
            home: at,
            target: None,
            alerted: false,
            returning: false,
            attack_clock: None,
            next_attack: 0,
            next_cast: 0,
            casting: None,
            recovery: 0,
            path: vec![],
            repath: 0,
            ai: Default::default(),
            boss_phase: 0,
            pursuit: None,
        });
        if immune {
            c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"immune","mechanism":"timed_control","attribute":"damage_immunity","modifier_bps":0,"duration_ms":600,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
            let target = w.npcs[1].entity.clone();
            bind(&mut w, &c, Owner::Npc(1), target, "immune", 0);
        }
        let target = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
        w.statuses.record_incoming(&status_entity(&target), 100, 30);
        w.prepare_periodic(300, &mut vec![]);
        let mut events = vec![];
        w.finish_shield_records(&c, 300, &mut events);
        assert_eq!(w.npcs[0].hp, 1000);
        assert_eq!(w.npcs[1].hp, if immune { 1000 } else { 940 });
        if immune {
            assert!(events.iter().any(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::ShieldRecordEnded {
                        targets_hit: 0,
                        suppressed: Some(Error::Capacity),
                        ..
                    }
                }
            )));
        }
    }
}

#[test]
fn combat_record_shield_zero_grant_refresh_cannot_keep_reduction_or_release_as_damage_depletion() {
    let (mut c, mut w) = setup();
    record_shield(&mut c, "guard", 2000, 600, record_rule());
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "barrier")
        .unwrap()
        .shield_bps = Some(3000);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"low-capacity","mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":-9000,"duration_ms":600,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 0);
    w.statuses.record_incoming(&status_entity(&target), 100, 30);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "barrier", 30);
    bind(
        &mut w,
        &c,
        Owner::Member(1),
        target.clone(),
        "low-capacity",
        60,
    );
    bind(&mut w, &c, Owner::Member(0), target.clone(), "guard", 90);
    w.finish_shield_records(&c, 90, &mut vec![]);
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.definition_id != "guard")
    );
    assert_eq!(
        w.statuses.damage_reduction_bps(
            &status_entity(&target),
            90,
            c.pve.as_ref().unwrap().status_rules.as_ref()
        ),
        0
    );
    assert_eq!(w.npcs[0].hp, 1000);
}

#[test]
fn combat_record_shield_nested_reflection_cannot_take_the_outer_hits_pending_release() {
    let (mut c, mut w) = setup();
    record_shield(&mut c, "guard", 2000, 600, record_rule());
    record_shield(&mut c, "npc-guard", 500, 600, record_rule());
    let member = w.members[0].reference.entity.clone();
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), member, "guard", 0);
    bind(&mut w, &c, Owner::Npc(0), npc.clone(), "npc-guard", 0);
    w.statuses.record_incoming(&status_entity(&npc), 100, 0);
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"reflect","profiles":["member"],"when":"shield_absorbed","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":5000,"damage_kind":"true_damage"}}),
    );
    let (_, events) = record_hit(&mut w, &c, 200, 30);
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.npcs[0].hp, 860);
    assert!(events.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::ShieldRecordEnded{definition_id,suppressed:None,targets_hit:1,..}} if definition_id=="guard")));
    assert!(events.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::ShieldRecordEnded{definition_id,recorded:100,suppressed:Some(Error::InvalidState),..}} if definition_id=="npc-guard")));
}

fn operation_policy(c: &mut Config, id: &str, disposition: &str, transferable: bool) {
    c.combat_rules.as_mut().unwrap().status_policies.insert(
        id.into(),
        serde_json::from_value(json!({
            "disposition":disposition,"dispellable":true,"transferable":transferable,"blocks":[]
        }))
        .unwrap(),
    );
}
fn operation_trigger(c: &mut Config, effect: Value, consume: Option<Value>) {
    c.combat_rules.as_mut().unwrap().triggers.clear();
    let mut t = json!({"id":"operation","profiles":["member"],"when":"damage_dealt","include_periodic":false,
        "every":1,"cooldown_ms":600,"priority":0,"effect":effect});
    if let Some(requirement) = consume {
        t["conditions"] = json!({"status":requirement});
    }
    trigger(c, t);
}
fn transfer_effect() -> Value {
    json!({"kind":"transfer_statuses","from":"other","disposition":"beneficial","max_count":2})
}
fn guard_config(c: &mut Config, statuses: &[&str], heal: u32) {
    c.combat_rules.as_mut().unwrap().triggers.clear();
    c.combat_rules.as_mut().unwrap().death_guards.insert(
        "member".into(),
        serde_json::from_value(json!({
            "retain_hp":1,"heal_max_hp_bps":heal,"self_statuses":statuses
        }))
        .unwrap(),
    );
}
fn operation_immunity(c: &mut Config) {
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"life-window","mechanism":"timed_control",
        "attribute":"damage_immunity","duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
}
#[test]
fn combat_status_operation_loader_rejects_unknown_policies_unsupported_transfers_and_limits() {
    let package = Package::new();
    let mut good = document();
    good["combat_rules"]["status_policies"] = json!({"demo-guard":{"disposition":"beneficial","dispellable":true,"transferable":true,"blocks":[]}});
    good["combat_rules"]["death_guards"] =
        json!({"member":{"retain_hp":1,"heal_max_hp_bps":1500,"self_statuses":["demo-guard"]}});
    package.write(&good);
    package.load().unwrap();
    for case in 0..14 {
        let mut bad = good.clone();
        match case {
            0 => {
                bad["combat_rules"]["status_policies"]["unknown"] =
                    good["combat_rules"]["status_policies"]["demo-guard"].clone()
            }
            1 => {
                bad["combat_rules"]["status_policies"]["demo-guard"]["transferable"] = json!("true")
            }
            2 => {
                bad["combat_rules"]["status_policies"]["wound"] =
                    good["combat_rules"]["status_policies"]["demo-guard"].clone();
                bad["combat_rules"]["status_policies"]["wound"]["dispellable"] = json!(false);
            }
            3 => bad["combat_rules"]["status_policies"]["demo-guard"]["dispellable"] = json!(false),
            4 => {
                bad["combat_rules"]["status_policies"]["demo-guard"]["blocks"] =
                    json!(["harmful", "harmful"])
            }
            5 => bad["combat_rules"]["status_policies"] = Value::Null,
            6 => bad["combat_rules"]["death_guards"]["member"]["retain_hp"] = json!(0),
            7 => bad["combat_rules"]["death_guards"]["member"]["heal_max_hp_bps"] = json!(10001),
            8 => {
                bad["combat_rules"]["death_guards"]["member"]["self_statuses"] = json!(["missing"])
            }
            9 => bad["combat_rules"]["death_guards"]["member"]["self_statuses"] = json!(["wound"]),
            10 => {
                bad["combat_rules"]["death_guards"]["unknown"] =
                    good["combat_rules"]["death_guards"]["member"].clone()
            }
            11 => bad["combat_rules"]["death_guards"] = Value::Null,
            12 => {
                bad["combat_rules"]["triggers"][0]["effect"] = json!({"kind":"dispel_statuses","subject":"owner","disposition":"harmful","max_count":17})
            }
            _ => {
                bad["combat_rules"]["triggers"][0]["effect"] = json!({"kind":"transfer_statuses","from":"owner","disposition":"harmful","max_count":0})
            }
        }
        package.write(&bad);
        assert!(package.load().is_err(), "case {case}");
    }
}
#[test]
fn combat_dispel_selects_stable_permitted_instances_without_removing_unregistered_or_expired() {
    let (mut c, mut w) = setup();
    charge_definition(&mut c, "late");
    charge_definition(&mut c, "early");
    charge_definition(&mut c, "private");
    for id in ["late", "early"] {
        operation_policy(&mut c, id, "harmful", true);
    }
    operation_trigger(
        &mut c,
        json!({"kind":"dispel_statuses","subject":"owner","disposition":"harmful","max_count":1}),
        None,
    );
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), own.clone(), "late", 60);
    let earlier = bind(&mut w, &c, Owner::Npc(0), own.clone(), "early", 0);
    bind(&mut w, &c, Owner::Npc(0), own, "private", 0);
    let events = fact(&mut w, &c, 10, 90);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "late")
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "private")
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "early")
    );
    assert!(events.iter().all(|(recipient, _)| *recipient == Some(0)));
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusRemoved {
                reason: StatusRemovalReason::Cleansed,
                ..
            }
        }
    )));
    assert!(!earlier.is_empty());
    let before = w.statuses.entries().to_vec();
    fact(&mut w, &c, 10, 1800);
    assert_eq!(before, w.statuses.entries()); // expired candidates do not count as dispels
}
#[test]
fn combat_status_blocks_filter_categories_and_refresh_then_expire_without_deleting_existing() {
    let (mut c, mut w) = setup();
    for id in ["seal", "harm", "benefit"] {
        charge_definition(&mut c, id);
    }
    operation_policy(&mut c, "harm", "harmful", true);
    operation_policy(&mut c, "benefit", "beneficial", true);
    operation_policy(&mut c, "seal", "beneficial", false);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("seal")
        .unwrap()
        .blocks = vec![crate::config::StatusDisposition::Harmful];
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "harm", 0);
    let original = w.statuses.entries()[0].clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "seal", 30);
    let events = bind(&mut w, &c, Owner::Npc(0), target.clone(), "harm", 60);
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusRejected {
                reason: Error::Immune,
                ..
            }
        }
    )));
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.instance_id == original.instance_id)
            .unwrap(),
        &original
    );
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "benefit", 90);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "benefit")
    );
    bind(&mut w, &c, Owner::Npc(0), target, "harm", 1600);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "harm" && s.applied_at_ms == 1600)
    );
}
#[test]
fn combat_transfer_preserves_identity_source_layers_and_expiry_and_routes_only_entitled_members() {
    let (mut c, mut w) = setup();
    charge_definition(&mut c, "transferable");
    operation_policy(&mut c, "transferable", "beneficial", true);
    operation_trigger(&mut c, transfer_effect(), None);
    let old = w.members[1].reference.entity.clone();
    for _ in 0..2 {
        bind(
            &mut w,
            &c,
            Owner::Member(1),
            old.clone(),
            "transferable",
            30,
        );
    }
    let before = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "transferable")
        .unwrap()
        .clone();
    let mut events = vec![];
    w.combat_fact(
        crate::config::CombatFact::DamageDealt,
        (Owner::Member(0), Owner::Member(1)),
        (10, false),
        &c,
        60,
        &mut events,
    );
    let after = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.instance_id == before.instance_id)
        .unwrap();
    let mut expected = before.clone();
    expected.target = status_entity(&w.members[0].reference.entity);
    assert_eq!(after, &expected);
    let transfers = events
        .iter()
        .filter(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::StatusTransferred { .. }
                }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 2);
    assert_eq!(
        transfers.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![Some(0), Some(1)]
    );
    w.clear_owned_statuses(&old, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.instance_id == before.instance_id)
    );
}
#[test]
fn combat_transfer_capacity_and_collision_failures_do_not_consume_or_start_cooldown() {
    for collision in [false, true] {
        let (mut c, mut w) = setup();
        for id in ["gift", "charge", "filler"] {
            charge_definition(&mut c, id);
        }
        operation_policy(&mut c, "gift", "beneficial", true);
        operation_trigger(
            &mut c,
            transfer_effect(),
            Some(json!({"subject":"owner","definition_id":"charge","at_least":2,"consume":1})),
        );
        let own = w.members[0].reference.entity.clone();
        let npc = w.npcs[0].entity.clone();
        for _ in 0..2 {
            bind(&mut w, &c, Owner::Member(0), own.clone(), "charge", 0);
        }
        bind(&mut w, &c, Owner::Npc(0), npc.clone(), "gift", 0);
        bind(
            &mut w,
            &c,
            Owner::Npc(0),
            own.clone(),
            if collision { "gift" } else { "filler" },
            0,
        );
        c.pve_mut()
            .status_rules
            .as_mut()
            .unwrap()
            .max_active_per_entity = 2;
        let before = w.statuses.entries().to_vec();
        fact(&mut w, &c, 10, 30);
        assert_eq!(before, w.statuses.entries());
        let mut raw = vec![];
        w.statuses.remove(
            StatusRemovalReason::Cleansed,
            |s| {
                s.target == status_entity(&own)
                    && s.definition_id == if collision { "gift" } else { "filler" }
            },
            &mut raw,
        );
        let events = fact(&mut w, &c, 10, 60);
        assert!(events.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::StatusTransferred { .. }
            }
        )));
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .find(|s| s.definition_id == "charge")
                .unwrap()
                .stacks,
            Some(1)
        );
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .filter(|s| s.definition_id == "gift")
                .count(),
            1
        );
    }
}
#[test]
fn combat_transfer_full_consumption_frees_slot_and_empty_or_same_target_does_not_consume() {
    let (mut c, mut w) = setup();
    for id in ["gift", "charge"] {
        charge_definition(&mut c, id);
    }
    operation_policy(&mut c, "gift", "beneficial", true);
    operation_trigger(
        &mut c,
        transfer_effect(),
        Some(json!({"subject":"owner","definition_id":"charge","at_least":1,"consume":1})),
    );
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own.clone(), "charge", 0);
    let before = w.statuses.entries().to_vec();
    fact(&mut w, &c, 10, 30);
    assert_eq!(before, w.statuses.entries());
    let mut events = vec![];
    w.combat_fact(
        crate::config::CombatFact::DamageDealt,
        (Owner::Member(0), Owner::Member(0)),
        (10, false),
        &c,
        60,
        &mut events,
    );
    assert_eq!(before, w.statuses.entries());
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc, "gift", 60);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    fact(&mut w, &c, 10, 90);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "charge")
    );
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "gift")
            .unwrap()
            .target,
        status_entity(&own)
    );
}
#[test]
fn combat_transfer_blocked_destination_rejects_entire_batch_and_keeps_consumption() {
    let (mut c, mut w) = setup();
    for id in ["gift", "charge", "seal"] {
        charge_definition(&mut c, id);
    }
    for id in ["gift", "seal"] {
        operation_policy(&mut c, id, "beneficial", id == "gift");
    }
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("seal")
        .unwrap()
        .blocks = vec![crate::config::StatusDisposition::Beneficial];
    operation_trigger(
        &mut c,
        transfer_effect(),
        Some(json!({"subject":"owner","definition_id":"charge","at_least":1,"consume":1})),
    );
    let own = w.members[0].reference.entity.clone();
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc, "gift", 0);
    bind(&mut w, &c, Owner::Member(0), own.clone(), "charge", 0);
    bind(&mut w, &c, Owner::Member(0), own, "seal", 0);
    let before = w.statuses.entries().to_vec();
    let events = fact(&mut w, &c, 10, 30);
    assert_eq!(before, w.statuses.entries());
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::CombatTriggerSuppressed {
                reason: Error::Immune,
                ..
            }
        }
    )));
}
#[test]
fn combat_dispelling_recorded_shield_never_releases_counter_or_end_benefits() {
    let (mut c, mut w) = setup();
    record_shield(&mut c, "guard", 2000, 600, record_rule());
    operation_policy(&mut c, "guard", "beneficial", false);
    operation_trigger(
        &mut c,
        json!({"kind":"dispel_statuses","subject":"other","disposition":"beneficial","max_count":1}),
        None,
    );
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc.clone(), "guard", 0);
    w.statuses.record_incoming(&status_entity(&npc), 100, 0);
    let mut events = fact(&mut w, &c, 10, 30);
    w.finish_shield_records(&c, 30, &mut events);
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.npcs[0].hp, 1000);
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ShieldRecordEnded {
                reason: StatusRemovalReason::Cleansed,
                targets_hit: 0,
                raw_counter: 0,
                ..
            }
        }
    )));
}
#[test]
fn combat_death_guard_runs_after_reduction_shield_then_actual_loss_heal_and_immunity() {
    let (mut c, mut w) = setup();
    operation_immunity(&mut c);
    guard_config(&mut c, &["life-window"], 1500);
    record_shield(&mut c, "guard", 2000, 600, record_rule());
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "guard", 0);
    w.members[0].hp = 100;
    let (result, events) = record_hit(&mut w, &c, 500, 30);
    assert_eq!(
        (result.absorbed, result.hp_damage, w.members[0].hp),
        (200, 99, 151)
    );
    assert!(w.members[0].living());
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::DeathGuardActivated {
                prevented_hp_loss: 1,
                hp: 151,
                ..
            }
        }
    )));
    assert!(
        events
            .iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::DeathGuardActivated { .. }
                }
            ))
            .all(|(i, _)| *i == Some(0))
    );
    assert_eq!(record_hit(&mut w, &c, 10000, 60).0.hp_damage, 0);
    let (result, _) = record_hit(&mut w, &c, 10000, 360);
    assert_eq!(result.hp_damage, 151);
    assert!(!w.members[0].living());
}
#[test]
fn combat_death_guard_capacity_failure_does_not_use_guard_and_full_absorption_does_not_use_it() {
    let (mut c, mut w) = setup();
    operation_immunity(&mut c);
    guard_config(&mut c, &["life-window"], 0);
    charge_definition(&mut c, "filler");
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "filler", 0);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let guard = w.plan_death_guard(Owner::Member(0), 1000, &c, 30, &mut vec![]);
    assert!(guard.is_none());
    assert!(w.triggers.death_used.is_empty());
    w.statuses
        .remove(StatusRemovalReason::Cleansed, |_| true, &mut vec![]);
    record_shield(&mut c, "shield", 10000, 600, record_rule());
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "shield", 30);
    w.members[0].hp = 1;
    record_hit(&mut w, &c, 100, 60);
    assert!(w.triggers.death_used.is_empty());
    w.statuses
        .remove(StatusRemovalReason::Cleansed, |_| true, &mut vec![]);
    record_hit(&mut w, &c, 100, 90);
    assert_eq!(w.members[0].hp, 1);
    assert_eq!(w.triggers.death_used.len(), 1);
}
#[test]
fn combat_death_guard_zero_healing_received_and_same_step_second_lethal_are_exact() {
    let (mut c, mut w) = setup();
    guard_config(&mut c, &[], 1500);
    let mut d = json!({"id":"no-heal","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":-10000,"duration_ms":600,"duplicate_policy":"refresh","source_scoped":true});
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d.take()).unwrap());
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), own, "no-heal", 0);
    w.members[0].hp = 20;
    let first = record_hit(&mut w, &c, 100, 30);
    assert_eq!(first.0.hp_damage, 19);
    assert_eq!(w.members[0].hp, 1);
    assert!(first.1.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::HealResolved {
                raw_heal: 150,
                hp_restored: 0,
                ..
            }
        }
    )));
    assert_eq!(record_hit(&mut w, &c, 100, 30).0.hp_damage, 1);
    assert!(!w.members[0].living());
}
#[test]
fn combat_death_guard_is_per_entity_generation_and_not_reset_by_read_or_other_entity_clear() {
    let (mut c, mut w) = setup();
    guard_config(&mut c, &[], 0);
    w.members[0].hp = 1;
    w.members[1].hp = 1;
    record_hit(&mut w, &c, 100, 30);
    assert_eq!(w.triggers.death_used.len(), 1);
    let other = w.members[1].reference.entity.clone();
    w.triggers.clear(&other);
    assert_eq!(w.triggers.death_used.len(), 1);
    w.snapshot(0, &c, Phase::Active, 30);
    assert_eq!(w.triggers.death_used.len(), 1);
    w.members[0].reference.entity.generation += 1;
    record_hit(&mut w, &c, 100, 60);
    assert_eq!(w.members[0].hp, 1);
    assert_eq!(w.triggers.death_used.len(), 2);
}
#[test]
fn combat_death_guard_applies_to_npc_without_false_kill_reward_and_lifesteal_uses_actual_loss() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    c.combat_rules.as_mut().unwrap().death_guards.insert(
        "npc".into(),
        serde_json::from_value(json!({"retain_hp":1,"heal_max_hp_bps":1000,"self_statuses":[]}))
            .unwrap(),
    );
    w.members[0].hp = 500;
    w.npcs[0].hp = 20;
    let mut r = rule(&c);
    r.damage_kind = crate::config::DamageKind::TrueDamage;
    r.life_steal_bps = 5000;
    let mut events = vec![];
    let result = w.resolve_damage_with_rule(intent(&w, 1000, true), Some(&r), &c, 30, &mut events);
    assert_eq!(
        (result.hp_damage, w.members[0].hp, w.npcs[0].hp),
        (19, 509, 101)
    );
    assert!(
        !events
            .iter()
            .any(|(_, e)| matches!(e, EventV4::MonsterDefeated { .. }))
    );
}
#[test]
fn combat_death_guard_blocked_self_status_leaves_once_flag_unspent() {
    let (mut c, mut w) = setup();
    operation_immunity(&mut c);
    guard_config(&mut c, &["life-window"], 1500);
    charge_definition(&mut c, "seal");
    operation_policy(&mut c, "life-window", "beneficial", false);
    operation_policy(&mut c, "seal", "beneficial", false);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("seal")
        .unwrap()
        .blocks = vec![crate::config::StatusDisposition::Beneficial];
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "seal", 0);
    assert!(
        w.plan_death_guard(Owner::Member(0), 1000, &c, 30, &mut vec![])
            .is_none()
    );
    assert!(w.triggers.death_used.is_empty());
}

#[test]
fn combat_transfer_batch_is_atomic_when_second_instance_conflicts() {
    let (mut c, mut w) = setup();
    for id in ["first", "second"] {
        charge_definition(&mut c, id);
        operation_policy(&mut c, id, "beneficial", true);
    }
    operation_trigger(&mut c, transfer_effect(), None);
    let own = w.members[0].reference.entity.clone();
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc.clone(), "first", 0);
    bind(&mut w, &c, Owner::Npc(0), npc, "second", 30);
    bind(&mut w, &c, Owner::Npc(0), own, "second", 60);
    let before = w.statuses.entries().to_vec();
    fact(&mut w, &c, 10, 90);
    assert_eq!(before, w.statuses.entries());
}
#[test]
fn combat_transfer_rejects_protected_returning_and_stale_entities() {
    let (mut c, mut w) = setup();
    charge_definition(&mut c, "gift");
    operation_policy(&mut c, "gift", "beneficial", true);
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc, "gift", 0);
    let effect: crate::config::TriggerEffect = serde_json::from_value(transfer_effect()).unwrap();
    w.members[0].protected = true;
    assert!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(0)),
            &effect,
            &c,
            30
        )
        .is_err()
    );
    w.members[0].protected = false;
    w.npcs[0].returning = true;
    assert!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(0)),
            &effect,
            &c,
            30
        )
        .is_err()
    );
    w.npcs[0].returning = false;
    w.npcs[0].entity.generation += 1;
    assert!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(0)),
            &effect,
            &c,
            30
        )
        .is_err()
    );
}
#[test]
fn combat_death_guard_expired_status_slot_is_available_and_retain_does_not_add_hp() {
    let (mut c, mut w) = setup();
    operation_immunity(&mut c);
    guard_config(&mut c, &["life-window"], 0);
    charge_definition(&mut c, "expired");
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "expired", 0);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    c.combat_rules
        .as_mut()
        .unwrap()
        .death_guards
        .get_mut("member")
        .unwrap()
        .retain_hp = 500;
    w.members[0].hp = 20;
    let result = record_hit(&mut w, &c, 1000, 1500);
    assert_eq!(result.0.hp_damage, 0);
    assert_eq!(w.members[0].hp, 20);
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(w.statuses.entries()[0].definition_id, "life-window");
}
#[test]
fn combat_forced_terminal_and_hp_cost_bypass_death_guard_without_spending_it() {
    let (mut c, mut w) = setup();
    guard_config(&mut c, &[], 1500);
    let mut s = skill("projectile");
    s["hp_cost"] = json!({"mode":"fixed","value":30,"min_hp_after_cost":1});
    c.skill = serde_json::from_value(s).unwrap();
    w.members[0].hp = 20;
    commit(&mut w, &c, "fee-only");
    assert_eq!(w.members[0].hp, 1);
    assert!(w.triggers.death_used.is_empty());
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert!(!w.members[0].living());
    assert!(w.triggers.death_used.is_empty());
}
#[test]
fn combat_death_guard_reflection_uses_same_gate_and_spends_once() {
    let (mut c, mut w) = setup();
    guard_config(&mut c, &[], 0);
    w.members[0].hp = 10;
    trigger(
        &mut c,
        json!({"id":"reflect","profiles":["npc"],"when":"damage_taken","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":10000,"damage_kind":"true_damage"}}),
    );
    let mut r = rule(&c);
    r.damage_kind = crate::config::DamageKind::TrueDamage;
    let mut e = vec![];
    w.resolve_damage_with_rule(intent(&w, 100, true), Some(&r), &c, 30, &mut e);
    assert_eq!(w.members[0].hp, 1);
    assert!(w.members[0].living());
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::DeathGuardActivated { .. }
                }
            ))
            .count(),
        1
    );
    w.resolve_damage_with_rule(intent(&w, 100, true), Some(&r), &c, 60, &mut e);
    assert!(!w.members[0].living());
}

#[test]
fn combat_death_guard_dot_uses_shared_lethal_stage_and_does_not_restart_periodic_schedule() {
    let (mut c, mut w) = setup();
    guard_config(&mut c, &[], 1500);
    c.combat_rules.as_mut().unwrap().periodic_damage.insert(
        "wound".into(),
        AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            can_crit: false,
            ..Default::default()
        },
    );
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), own, "wound", 0);
    let periodic = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "wound")
        .unwrap()
        .periodic
        .clone()
        .unwrap();
    w.members[0].hp = 1;
    let mut events = vec![];
    w.advance_periodic(&c, periodic.next_tick_at_ms.unwrap(), &mut events);
    assert!(w.members[0].hp > 1);
    assert_eq!(
        events
            .iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::DeathGuardActivated { .. }
                }
            ))
            .count(),
        1
    );
    let after = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "wound")
        .unwrap()
        .periodic
        .as_ref()
        .unwrap();
    assert_eq!(after.ticks_done, 1);
    assert_eq!(
        after.next_tick_at_ms,
        Some(periodic.next_tick_at_ms.unwrap() + periodic.interval_ms)
    );
}

#[test]
fn combat_transfer_from_public_npc_hides_private_new_holder_from_other_observers() {
    let (mut c, mut w) = setup();
    charge_definition(&mut c, "gift");
    operation_policy(&mut c, "gift", "beneficial", true);
    operation_trigger(&mut c, transfer_effect(), None);
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc, "gift", 0);
    let events = fact(&mut w, &c, 10, 30);
    assert!(events.iter().any(|(i, e)| *i == Some(0)
        && matches!(
            e,
            EventV4::Private {
                event: Event::StatusTransferred { .. }
            }
        )));
    assert!(!events.iter().any(|(i, e)| *i == Some(1)
        && matches!(
            e,
            EventV4::Private {
                event: Event::StatusTransferred { .. }
            }
        )));
    assert!(events.iter().any(|(i, e)| *i == Some(1)
        && matches!(
            e,
            EventV4::Private {
                event: Event::StatusRemoved {
                    reason: StatusRemovalReason::Transferred,
                    ..
                }
            }
        )));
    assert!(
        w.snapshot(1, &c, Phase::Active, 30)
            .monsters
            .iter()
            .all(|n| n
                .statuses
                .as_ref()
                .unwrap()
                .iter()
                .all(|s| s.definition_id != "gift"))
    );
}

fn total_trigger(c: &mut Config, scope: &str, effect: Value) {
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        c,
        json!({"id":"total","profiles":["member"],"when":"damage_dealt","counting":"action_total","counter_scope":scope,"include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"effect":effect}),
    );
}
fn add_total_npc(w: &mut World) {
    let n = &w.npcs[0];
    let at = [n.position[0], n.position[1] + 12.];
    w.npcs.push(Npc {
        training_owner: None,
        training_active: false,
        training_reverse: false,
        entity: EntityRef {
            kind: StatusEntityKind::Monster,
            id: "total-other".into(),
            generation: 1,
        },
        template_id: n.template_id.clone(),
        profile: n.profile.clone(),
        position: at,
        home: at,
        hp: 1000,
        spirit: 100,
        target: None,
        alerted: false,
        returning: false,
        attack_clock: None,
        next_attack: 0,
        next_cast: 0,
        casting: None,
        recovery: 0,
        path: vec![],
        repath: 0,
        ai: Default::default(),
        boss_phase: 0,
        pursuit: None,
    });
}
fn total_contact(
    w: &mut World,
    c: &Config,
    target: usize,
    action: &str,
    raw: u32,
    now: u64,
    events: &mut Events,
) {
    let source = w.members[0].reference.entity.clone();
    let rule = AttackRule {
        damage_kind: crate::config::DamageKind::TrueDamage,
        can_crit: false,
        ..Default::default()
    };
    w.direct_contact(
        Owner::Member(0),
        Owner::Npc(target),
        DirectContact {
            origin: None,
            action_id: Some(action),
            rule: Some(&rule),
            ignore_armor: true,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw,
            metal: true,
            hit_statuses: &[],
        },
        c,
        now,
        events,
    );
}
fn total_rows(events: &Events) -> Vec<(u64, u32, bool)> {
    events
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::Private {
                event:
                    Event::CombatActionTotal {
                        total_amount,
                        contacts,
                        fired,
                        ..
                    },
            } = e
            {
                Some((*total_amount, *contacts, *fired))
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn combat_action_total_and_target_scope_config_roundtrip_and_reject_ambiguous_forms() {
    let p = Package::new();
    let mut v = document();
    v["combat_rules"]["triggers"][0] = json!({"id":"total","profiles":["member"],"when":"damage_dealt","counting":"action_total","counter_scope":"target","include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"heal_from_damage","bps":5000}});
    p.write(&v);
    let loaded = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(loaded.default_config()).unwrap()["combat_rules"]["triggers"][0],
        v["combat_rules"]["triggers"][0]
    );
    for case in 0..12 {
        let mut bad = v.clone();
        let t = &mut bad["combat_rules"]["triggers"][0];
        match case {
            0 => t["counter_scope"] = json!("unknown"),
            1 => t["counter_scope"] = Value::Null,
            2 => t["counting"] = Value::Null,
            3 => t["include_periodic"] = json!(true),
            4 => t["when"] = json!("skill_committed"),
            5 => {
                t["effect"] =
                    json!({"kind":"reflect_damage","bps":1000,"damage_kind":"true_damage"})
            }
            6 => t["conditions"] = json!({"other_hp_at_most_bps":5000}),
            7 => {
                t["conditions"] = json!({"status":{"subject":"other","definition_id":"ward","at_least":1,"consume":1}})
            }
            8 => bad["combat_rules"]["max_trigger_targets_per_rule"] = json!(0),
            9 => bad["combat_rules"]["max_trigger_targets_per_rule"] = json!(129),
            10 => bad["combat_rules"]["max_pending_action_totals"] = json!(1025),
            _ => bad["combat_rules"]["max_pending_action_totals"] = Value::Null,
        }
        p.write(&bad);
        assert!(p.load().is_err(), "{case}");
    }
}
#[test]
fn combat_target_counters_cooldowns_owners_and_generations_do_not_mix() {
    let (mut c, mut w) = setup();
    add_total_npc(&mut w);
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"targets","profiles":["member"],"when":"damage_dealt","counter_scope":"target","include_periodic":false,"every":2,"cooldown_ms":300,"priority":0,"effect":{"kind":"restore_spirit","amount":7}}),
    );
    w.members[0].spirit = 100;
    w.members[1].spirit = 100;
    let mut events = vec![];
    let f = |w: &mut World, owner, target, now, events: &mut Events| {
        w.combat_fact(
            crate::config::CombatFact::DamageDealt,
            (Owner::Member(owner), Owner::Npc(target)),
            (1, false),
            &c,
            now,
            events,
        )
    };
    f(&mut w, 0, 0, 30, &mut events);
    f(&mut w, 0, 1, 60, &mut events);
    assert_eq!(w.members[0].spirit, 100);
    f(&mut w, 0, 0, 90, &mut events);
    assert_eq!(w.members[0].spirit, 107);
    f(&mut w, 1, 1, 90, &mut events);
    assert_eq!(w.members[1].spirit, 100);
    f(&mut w, 0, 1, 120, &mut events);
    assert_eq!(w.members[0].spirit, 114);
    f(&mut w, 0, 0, 150, &mut events);
    w.npcs[0].entity.generation += 1;
    f(&mut w, 0, 0, 180, &mut events);
    assert_eq!(w.members[0].spirit, 114);
    f(&mut w, 0, 0, 210, &mut events);
    assert_eq!(w.members[0].spirit, 121);
}
#[test]
fn combat_target_action_dedup_is_per_target_and_never_evicts_live_counter_for_capacity() {
    let (mut c, mut w) = setup();
    add_total_npc(&mut w);
    c.combat_rules.as_mut().unwrap().triggers.clear();
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_trigger_targets_per_rule = 1;
    trigger(
        &mut c,
        json!({"id":"targets","profiles":["member"],"when":"damage_dealt","counting":"action","counter_scope":"target","include_periodic":false,"every":2,"cooldown_ms":0,"priority":0,"effect":{"kind":"restore_spirit","amount":7}}),
    );
    w.members[0].spirit = 100;
    let mut e = vec![];
    total_contact(&mut w, &c, 0, "same", 1, 30, &mut e);
    total_contact(&mut w, &c, 0, "same", 1, 60, &mut e);
    total_contact(&mut w, &c, 1, "other", 1, 60, &mut e);
    assert_eq!(w.members[0].spirit, 100);
    total_contact(&mut w, &c, 0, "next", 1, 90, &mut e);
    assert_eq!(w.members[0].spirit, 107);
    w.npcs[0].hp = 0;
    total_contact(&mut w, &c, 1, "a", 1, 120, &mut e);
    total_contact(&mut w, &c, 1, "b", 1, 150, &mut e);
    assert_eq!(w.members[0].spirit, 114);
}
#[test]
fn combat_action_total_sums_actual_hp_loss_across_targets_and_rounds_healing_only_once() {
    let (mut c, mut w) = setup();
    add_total_npc(&mut w);
    total_trigger(
        &mut c,
        "owner",
        json!({"kind":"heal_from_damage","bps":5000}),
    );
    w.members[0].hp = 100;
    let mut e = vec![];
    total_contact(&mut w, &c, 0, "one", 3, 30, &mut e);
    total_contact(&mut w, &c, 1, "one", 3, 30, &mut e);
    assert_eq!(w.members[0].hp, 100);
    assert_eq!(w.triggers.totals.pending_len(), 1);
    w.finish_trigger_actions(&c, 30, &mut e);
    assert_eq!(w.members[0].hp, 103);
    assert_eq!(total_rows(&e), vec![(6, 2, true)]);
    w.finish_trigger_actions(&c, 60, &mut e);
    assert_eq!(w.members[0].hp, 103);
    assert_eq!(total_rows(&e).len(), 1);
    assert!(
        e.iter()
            .filter(|(_, x)| matches!(
                x,
                EventV4::Private {
                    event: Event::CombatActionTotal { .. }
                }
            ))
            .all(|(i, _)| *i == Some(0))
    );
}
#[test]
fn combat_action_total_target_scope_keeps_separate_sums_and_dead_target_progress_until_flush() {
    let (mut c, mut w) = setup();
    add_total_npc(&mut w);
    total_trigger(
        &mut c,
        "target",
        json!({"kind":"restore_spirit","amount":7}),
    );
    c.combat_rules.as_mut().unwrap().triggers[0].every = 2;
    w.members[0].spirit = 100;
    let mut e = vec![];
    for target in 0..2 {
        total_contact(&mut w, &c, target, "first", 10, 30, &mut e);
    }
    w.finish_trigger_actions(&c, 30, &mut e);
    assert_eq!(w.members[0].spirit, 100);
    w.npcs[0].hp = 5;
    for target in 0..2 {
        total_contact(&mut w, &c, target, "last", 10, 60, &mut e);
    }
    w.finish_trigger_actions(&c, 60, &mut e);
    assert_eq!(w.members[0].spirit, 114);
    assert!(total_rows(&e).contains(&(5, 1, true)));
    assert!(total_rows(&e).contains(&(10, 1, true)));
}
#[test]
fn combat_action_total_healing_does_not_clip_sum_to_u32_before_ratio_or_lose_zero_heal_events() {
    let (mut c, mut w) = setup();
    total_trigger(&mut c, "owner", json!({"kind":"heal_from_damage","bps":1}));
    let mut e = vec![];
    w.members[0].hp = 100;
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("hp".into(), u32::MAX);
    for _ in 0..2 {
        w.npcs[0].hp = u32::MAX;
        total_contact(&mut w, &c, 0, "large", u32::MAX, 30, &mut e);
    }
    w.finish_trigger_actions(&c, 30, &mut e);
    assert_eq!(total_rows(&e), vec![(2 * u64::from(u32::MAX), 2, true)]);
    assert!(e.iter().any(|(_, x)| matches!(
        x,
        EventV4::Private {
            event: Event::HealResolved {
                raw_heal: 858993,
                ..
            }
        }
    )));
    w.npcs[0].hp = 1000;
    total_contact(&mut w, &c, 0, "tiny", 1, 60, &mut e);
    w.finish_trigger_actions(&c, 60, &mut e);
    assert!(e.iter().any(|(_, x)| matches!(
        x,
        EventV4::Private {
            event: Event::HealResolved { raw_heal: 0, .. }
        }
    )));
    assert_eq!(
        super::action_totals::checked_total(super::action_totals::MAX_TOTAL_AMOUNT, 1),
        None
    );
}
#[test]
fn combat_action_total_waits_for_fields_and_all_channel_children_and_preserves_canceled_damage() {
    for kind in [
        "ground_field",
        "line_field",
        "channel",
        "projectile",
        "piercing_projectile",
    ] {
        let (mut c, mut w) = setup();
        let mut s = skill(if kind == "channel" {
            "projectile"
        } else {
            kind
        });
        if kind == "channel" {
            s["channel"] = json!({"duration_ms":390,"interval_ms":120,"spirit_per_pulse":1});
        }
        c.skill = serde_json::from_value(s).unwrap();
        total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
        w.members[0].spirit = 100;
        commit(
            &mut w,
            &c,
            if kind == "channel" {
                "projectile"
            } else {
                kind
            },
        );
        let mut e = vec![];
        if kind == "channel" {
            w.advance_channel(0, &c, 150, &mut e);
            w.advance_shots(&c, 180, 1., &mut e);
            w.finish_trigger_actions(&c, 180, &mut e);
            assert!(total_rows(&e).is_empty());
            w.advance_channel(0, &c, 270, &mut e);
            w.finish_channel(0, ChannelEndReason::Cancelled, 270, &mut e);
            w.finish_trigger_actions(&c, 270, &mut e);
            assert!(total_rows(&e).is_empty());
            w.advance_shots(&c, 300, 1., &mut e);
            w.finish_trigger_actions(&c, 300, &mut e);
        } else if kind.contains("field") {
            w.advance_areas(&c, 150, &mut e);
            w.finish_trigger_actions(&c, 150, &mut e);
            assert!(total_rows(&e).is_empty());
            w.advance_areas(&c, 270, &mut e);
            w.finish_trigger_actions(&c, 270, &mut e);
        } else {
            w.advance_shots(&c, 60, 1., &mut e);
            w.finish_trigger_actions(&c, 60, &mut e);
        }
        assert_eq!(total_rows(&e).len(), 1, "{kind}: {e:?}");
        assert!(total_rows(&e)[0].2);
        assert_eq!(w.triggers.totals.pending_len(), 0);
    }
}
#[test]
fn combat_action_total_minimum_conditions_consumption_and_icd_are_evaluated_at_completion() {
    let (mut c, mut w) = setup();
    charge_definition(&mut c, "charge");
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.cooldown_ms = 300;
    t.conditions=serde_json::from_value(json!({"minimum_amount":6,"owner_hp_at_most_bps":5000,"status":{"subject":"owner","definition_id":"charge","at_least":2,"consume":1}})).unwrap();
    let own = w.members[0].reference.entity.clone();
    for _ in 0..3 {
        bind(&mut w, &c, Owner::Member(0), own.clone(), "charge", 0);
    }
    w.members[0].spirit = 100;
    let mut e = vec![];
    total_contact(&mut w, &c, 0, "one", 3, 30, &mut e);
    total_contact(&mut w, &c, 0, "one", 3, 60, &mut e);
    w.members[0].hp = 400;
    w.finish_trigger_actions(&c, 60, &mut e);
    assert_eq!(w.members[0].spirit, 107);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "charge")
            .unwrap()
            .stacks,
        Some(2)
    );
    total_contact(&mut w, &c, 0, "two", 6, 90, &mut e);
    w.finish_trigger_actions(&c, 90, &mut e);
    assert_eq!(w.members[0].spirit, 107);
    w.finish_trigger_actions(&c, 400, &mut e);
    assert_eq!(w.members[0].spirit, 107);
    total_contact(&mut w, &c, 0, "three", 6, 420, &mut e);
    w.finish_trigger_actions(&c, 420, &mut e);
    assert_eq!(w.members[0].spirit, 114);
}
#[test]
fn combat_action_total_capacity_or_probability_failure_does_not_consume_and_cannot_retry_old_total()
{
    let (mut c, mut w) = setup();
    for id in ["charge", "bonus"] {
        charge_definition(&mut c, id);
    }
    total_trigger(
        &mut c,
        "owner",
        json!({"kind":"apply_self_status","status_id":"bonus"}),
    );
    c.combat_rules.as_mut().unwrap().triggers[0].conditions = serde_json::from_value(
        json!({"status":{"subject":"owner","definition_id":"charge","at_least":2,"consume":1}}),
    )
    .unwrap();
    let own = w.members[0].reference.entity.clone();
    for _ in 0..2 {
        bind(&mut w, &c, Owner::Member(0), own.clone(), "charge", 0);
    }
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut e = vec![];
    total_contact(&mut w, &c, 0, "full", 10, 30, &mut e);
    w.finish_trigger_actions(&c, 30, &mut e);
    assert_eq!(w.statuses.entries()[0].stacks, Some(2));
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 2;
    w.finish_trigger_actions(&c, 60, &mut e);
    assert_eq!(w.statuses.entries().len(), 1);
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(0);
    let rng = w.rng;
    total_contact(&mut w, &c, 0, "chance", 10, 90, &mut e);
    w.finish_trigger_actions(&c, 90, &mut e);
    assert_eq!(w.rng, rng);
    assert_eq!(w.statuses.entries()[0].stacks, Some(2));
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(10000);
    total_contact(&mut w, &c, 0, "new", 10, 120, &mut e);
    w.finish_trigger_actions(&c, 120, &mut e);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "bonus")
    );
}
#[test]
fn combat_action_total_budget_discards_entire_action_and_does_not_start_late_partial_reward() {
    let (mut c, mut w) = setup();
    add_total_npc(&mut w);
    total_trigger(
        &mut c,
        "target",
        json!({"kind":"restore_spirit","amount":7}),
    );
    c.combat_rules.as_mut().unwrap().max_pending_action_totals = 1;
    w.members[0].spirit = 100;
    let mut e = vec![];
    total_contact(&mut w, &c, 0, "large", 1, 30, &mut e);
    assert_eq!(w.triggers.totals.pending_len(), 1);
    total_contact(&mut w, &c, 1, "large", 1, 30, &mut e);
    assert_eq!(w.triggers.totals.pending_len(), 0);
    total_contact(&mut w, &c, 0, "large", 1, 60, &mut e);
    assert_eq!(w.triggers.totals.pending_len(), 0);
    w.finish_trigger_actions(&c, 60, &mut e);
    assert_eq!(w.members[0].spirit, 100);
    assert!(total_rows(&e).is_empty());
    total_contact(&mut w, &c, 0, "next", 1, 90, &mut e);
    w.finish_trigger_actions(&c, 90, &mut e);
    assert_eq!(w.members[0].spirit, 107);
}
#[test]
fn combat_action_total_source_exit_death_protection_and_periodic_facts_cannot_pay_late() {
    for mode in [0, 1, 2, 3] {
        let (mut c, mut w) = setup();
        total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
        w.members[0].spirit = 100;
        let mut e = vec![];
        if mode == 3 {
            w.triggers.action = Some("dot".into());
            w.combat_fact(
                crate::config::CombatFact::DamageDealt,
                (Owner::Member(0), Owner::Npc(0)),
                (10, true),
                &c,
                30,
                &mut e,
            );
        } else {
            total_contact(&mut w, &c, 0, "out", 10, 30, &mut e);
        }
        match mode {
            0 => {
                w.action(0, &Action::Leave {}, "exit", &c, 60).unwrap();
            }
            1 => w.members[0].hp = 0,
            2 => w.members[0].protected = true,
            _ => {}
        }
        w.finish_trigger_actions(&c, 60, &mut e);
        assert_eq!(w.members[0].spirit, 100);
        assert!(total_rows(&e).is_empty());
        assert_eq!(w.triggers.totals.pending_len(), 0);
    }
}

#[test]
fn combat_action_total_same_channel_id_from_other_member_cannot_delay_completion() {
    let (mut c, mut w) = setup();
    let mut s = skill("projectile");
    s["channel"] = json!({"duration_ms":390,"interval_ms":120,"spirit_per_pulse":1});
    c.skill = serde_json::from_value(s).unwrap();
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    w.members[0].spirit = 100;
    commit(&mut w, &c, "projectile");
    let id = w.members[0].channel.as_ref().unwrap().action_id.clone();
    w.members[1].channel = Some(ActiveChannel {
        steal_budget: None,
        target: None,
        movement_anchor: super::cast_movement::MovementAnchor::capture(&w.members[1]),
        recovery_budget: None,
        skill: c.skill.clone(),
        action_id: id,
        direction: [1., 0.],
        raw: 1,
        started_at_ms: 0,
        ends_at_ms: 900,
        next_pulse_at_ms: 900,
        pulses: 0,
    });
    let mut e = vec![];
    w.advance_channel(0, &c, 150, &mut e);
    w.advance_shots(&c, 180, 1., &mut e);
    w.finish_channel(0, ChannelEndReason::Cancelled, 180, &mut e);
    w.finish_trigger_actions(&c, 180, &mut e);
    assert_eq!(total_rows(&e).len(), 1);
    assert!(w.members[1].channel.is_some());
    assert_eq!(w.members[0].spirit, 104);
}
#[test]
fn combat_action_total_budget_poison_remains_until_inflight_field_ends() {
    let (mut c, mut w) = setup();
    add_total_npc(&mut w);
    c.skill = serde_json::from_value(skill("ground_field")).unwrap();
    c.skill.radius = 100.;
    c.skill
        .contact_policy
        .as_mut()
        .unwrap()
        .max_targets_per_tick = 2;
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 6;
    total_trigger(
        &mut c,
        "target",
        json!({"kind":"restore_spirit","amount":7}),
    );
    c.combat_rules.as_mut().unwrap().max_pending_action_totals = 1;
    w.members[0].spirit = 100;
    commit(&mut w, &c, "ground_field");
    let mut e = vec![];
    for now in [150, 270, 450] {
        w.advance_areas(&c, now, &mut e);
        w.finish_trigger_actions(&c, now, &mut e);
        assert_eq!(w.triggers.totals.pending_len(), 0);
        assert!(total_rows(&e).is_empty());
    }
    assert_eq!(w.members[0].spirit, 98);
}
#[test]
fn combat_action_total_suppression_uses_shared_priority_budget_and_does_not_retry_completed_totals()
{
    let (mut c, mut w) = setup();
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    let mut second = serde_json::to_value(&c.combat_rules.as_ref().unwrap().triggers[0]).unwrap();
    second["id"] = json!("later");
    second["priority"] = json!(10);
    trigger(&mut c, second);
    c.combat_rules.as_mut().unwrap().max_triggers_per_step = 1;
    w.members[0].spirit = 100;
    let mut e = vec![];
    total_contact(&mut w, &c, 0, "one", 10, 30, &mut e);
    w.finish_trigger_actions(&c, 30, &mut e);
    assert_eq!(w.members[0].spirit, 107);
    assert_eq!(total_rows(&e).iter().filter(|r| r.2).count(), 1);
    w.finish_trigger_actions(&c, 60, &mut e);
    assert_eq!(w.members[0].spirit, 107);
}
#[test]
fn combat_action_total_does_not_count_shield_absorption_as_hp_damage_or_trigger_reflection_again() {
    let (mut c, mut w) = setup();
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc, "ward", 0);
    let mut e = vec![];
    total_contact(&mut w, &c, 0, "shielded", 1, 30, &mut e);
    w.finish_trigger_actions(&c, 30, &mut e);
    assert!(total_rows(&e).is_empty());
    w.statuses
        .remove(StatusRemovalReason::Cleansed, |_| true, &mut vec![]);
    trigger(
        &mut c,
        json!({"id":"reflection","profiles":["member"],"when":"damage_taken","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":10000,"damage_kind":"true_damage"}}),
    );
    record_hit(&mut w, &c, 10, 60);
    w.finish_trigger_actions(&c, 60, &mut e);
    assert!(total_rows(&e).is_empty());
}

#[test]
fn combat_action_total_safe_integer_overflow_rejects_all_rows_and_fails_closed_with_bounded_memory()
{
    use super::triggers::{ActionKey, TriggerInput};
    let (mut c, mut w) = setup();
    add_total_npc(&mut w);
    total_trigger(
        &mut c,
        "target",
        json!({"kind":"restore_spirit","amount":7}),
    );
    c.combat_rules.as_mut().unwrap().max_pending_action_totals = 1;
    w.members[0].spirit = 100;
    let mut e = vec![];
    let source = w.members[0].reference.entity.clone();
    let definition = c.combat_rules.as_ref().unwrap().triggers[0].clone();
    let mut push = |w: &mut World, id: &str, target: usize, amount: u64| {
        w.collect_action_total(
            definition.clone(),
            TriggerInput {
                origin: None,
                actors: (Owner::Member(0), Owner::Npc(target)),
                other: w.npcs[target].entity.clone(),
                amount,
                action: Some(ActionKey::new(&source, id)),
                contact: None,
            },
            &source,
            &c,
            &mut e,
        );
    };
    push(&mut w, "overflow", 0, 1);
    push(
        &mut w,
        "overflow",
        0,
        super::action_totals::MAX_TOTAL_AMOUNT,
    );
    assert_eq!(w.triggers.totals.pending_len(), 0);
    for i in 0..257 {
        let id = format!("budget-{i}");
        push(&mut w, &id, 0, 1);
        push(&mut w, &id, 1, 1);
        assert!(w.triggers.totals.pending_len() <= 1);
    }
    push(&mut w, "paused", 0, 1);
    assert_eq!(w.triggers.totals.pending_len(), 0);
    w.finish_trigger_actions(&c, 30, &mut e);
    assert_eq!(w.members[0].spirit, 100);
    total_contact(&mut w, &c, 0, "after-drain", 1, 60, &mut e);
    w.finish_trigger_actions(&c, 60, &mut e);
    assert_eq!(w.members[0].spirit, 107);
}
#[test]
fn combat_action_total_shield_fact_uses_attacker_action_and_defender_private_ownership() {
    let (mut c, mut w) = setup();
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    c.combat_rules.as_mut().unwrap().triggers[0].when = crate::config::CombatFact::ShieldAbsorbed;
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "ward", 0);
    w.members[0].spirit = 100;
    let source = w.npcs[0].entity.clone();
    let mut e = vec![];
    for _ in 0..2 {
        w.direct_contact(
            Owner::Npc(0),
            Owner::Member(0),
            DirectContact {
                origin: None,
                action_id: Some("npc-action"),
                rule: Some(&AttackRule {
                    damage_kind: crate::config::DamageKind::TrueDamage,
                    can_crit: false,
                    ..Default::default()
                }),
                ignore_armor: true,
                unavoidable: true,
                dead_source: false,
                source: &source,
                raw: 10,
                metal: true,
                hit_statuses: &[],
            },
            &c,
            30,
            &mut e,
        );
    }
    w.finish_trigger_actions(&c, 30, &mut e);
    assert_eq!(total_rows(&e), vec![(20, 2, true)]);
    assert!(e.iter().any(|(_,event)|matches!(event,EventV4::Private{event:Event::CombatActionTotal{action_source,..}} if action_source==&status_entity(&source))));
    assert_eq!(w.members[0].spirit, 107);
    assert!(
        e.iter()
            .filter(|(_, x)| matches!(
                x,
                EventV4::Private {
                    event: Event::CombatActionTotal { .. }
                }
            ))
            .all(|(i, _)| *i == Some(0))
    );
}

#[test]
fn combat_action_total_reconnect_during_active_field_cannot_revive_canceled_or_partial_journal() {
    let (mut c, mut w) = setup();
    c.skill = serde_json::from_value(skill("ground_field")).unwrap();
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    w.members[0].spirit = 100;
    commit(&mut w, &c, "ground_field");
    let mut e = vec![];
    w.advance_areas(&c, 150, &mut e);
    assert_eq!(w.triggers.totals.pending_len(), 1);
    w.protect(0, 150, &mut e);
    assert_eq!(w.triggers.totals.pending_len(), 0);
    w.members[0].protected = false;
    w.members[0].ready = true;
    w.members[0].foreground = true;
    w.advance_areas(&c, 270, &mut e);
    w.finish_trigger_actions(&c, 270, &mut e);
    assert!(total_rows(&e).is_empty());
    assert_eq!(w.members[0].spirit, 98);
}

#[test]
fn combat_action_id_cannot_be_reused_for_pending_active_or_unsettled_total_before_completion() {
    let (mut c, mut w) = setup();
    c.skill = serde_json::from_value(skill("ground_field")).unwrap();
    c.skill.cooldown_ms = 0;
    c.skill.recovery_ms = 0;
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    let action = Action::CastGround {
        skill_id: c.skill.id.clone(),
        position: w.npcs[0].position,
    };
    w.action(0, &action, "reuse", &c, 0).unwrap();
    assert_eq!(
        w.action(0, &action, "reuse", &c, 0),
        Err(Error::InvalidState)
    );
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 30, &mut vec![]);
    assert_eq!(
        w.action(0, &action, "reuse", &c, 60),
        Err(Error::InvalidState)
    );
    let mut e = vec![];
    w.advance_areas(&c, 150, &mut e);
    assert_eq!(w.triggers.totals.pending_len(), 1);
    let source = w.members[0].reference.entity.clone();
    w.clear_areas(&source, &mut e);
    assert_eq!(
        w.action(0, &action, "reuse", &c, 150),
        Err(Error::InvalidState)
    );
    w.finish_trigger_actions(&c, 180, &mut e);
    assert!(w.action(0, &action, "reuse", &c, 210).is_ok());
}

#[path = "window_tests.rs"]
mod window_tests;

#[path = "action_life_steal_tests.rs"]
mod action_life_steal_tests;

#[path = "compound_flag_tests.rs"]
mod compound_flag_tests;

#[path = "cleanse_lifecycle_tests.rs"]
mod cleanse_lifecycle_tests;

#[path = "periodic_resource_tests.rs"]
mod periodic_resource_tests;

#[path = "overheal_record_tests.rs"]
mod overheal_record_tests;

#[path = "shield_transfer_tests.rs"]
mod shield_transfer_tests;

#[path = "periodic_transfer_tests.rs"]
mod periodic_transfer_tests;

#[path = "control_transfer_tests.rs"]
mod control_transfer_tests;

#[path = "ally_commit_tests.rs"]
mod ally_commit_tests;

#[path = "lifecycle_transfer_tests.rs"]
mod lifecycle_transfer_tests;

#[path = "control_policy_tests.rs"]
mod control_policy_tests;

#[path = "hit_status_tests.rs"]
mod hit_status_tests;

#[path = "contact_status_tests.rs"]
mod contact_status_tests;

#[path = "chain_damage_tests.rs"]
mod chain_damage_tests;

#[path = "post_consumption_tests.rs"]
mod post_consumption_tests;

#[path = "contact_reaction_tests.rs"]
mod contact_reaction_tests;

#[path = "finisher_tests.rs"]
mod finisher_tests;

#[path = "finisher_followup_tests.rs"]
mod finisher_followup_tests;

#[path = "action_enhancement_tests.rs"]
mod action_enhancement_tests;
#[path = "periodic_composition_tests.rs"]
mod periodic_composition_tests;
