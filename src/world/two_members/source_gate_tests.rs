use super::*;
use crate::origin::ActionOrigin;
use serde_json::{Value, json};
fn setup() -> (Config, World) {
    let (mut c, w) = super::unit_effect_tests::setup(false, false);
    c.params.basic_spirit_on_hit = 0;
    let ids = vec!["member"];
    c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"eligible-reward","profiles":ids,"when":"damage_dealt","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"effect":{"kind":"restore_spirit","amount":1}}])).unwrap();
    (c, w)
}
fn hit(
    w: &mut World,
    c: &Config,
    source: usize,
    target: usize,
    id: Option<&str>,
    origin: Option<ActionOrigin>,
    now: u64,
) -> Events {
    let mut e = vec![];
    let owner = Owner::Member(source);
    let source = w.members[source].reference.entity.clone();
    let metal = !matches!(origin.as_ref(), Some(ActionOrigin::Basic));
    w.direct_contact(
        owner,
        Owner::Npc(target),
        combat::DirectContact {
            origin,
            action_id: id,
            rule: Some(&crate::config::AttackRule {
                ignore_armor: true,
                can_crit: false,
                ..Default::default()
            }),
            ignore_armor: true,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw: 1,
            metal,
            hit_statuses: &[],
        },
        c,
        now,
        &mut e,
    );
    e
}
fn skill(c: &Config) -> Option<ActionOrigin> {
    Some(ActionOrigin::Skill(c.skill.id.clone()))
}
fn filter(c: &mut Config, value: Value) {
    c.combat_rules.as_mut().unwrap().triggers[0].action_filter =
        Some(serde_json::from_value(value).unwrap());
}
#[test]
fn source_gate_filter_distinguishes_basic_only_except_and_unknown_without_inference() {
    for mode in ["any", "only", "except"] {
        let (mut c, mut w) = setup();
        let choice = if mode == "any" {
            json!({"mode":"any"})
        } else {
            json!({"mode":mode,"ids":[c.skill.id]})
        };
        filter(&mut c, json!({"basic":true,"skills":choice}));
        let selected = skill(&c);
        hit(
            &mut w,
            &c,
            0,
            0,
            Some("basic"),
            Some(ActionOrigin::Basic),
            0,
        );
        hit(&mut w, &c, 0, 0, Some("skill"), selected, 0);
        hit(&mut w, &c, 0, 0, Some("unknown"), None, 0);
        assert_eq!(w.members[0].spirit, if mode == "except" { 51 } else { 52 });
    }
}
#[test]
fn source_gate_owner_skill_equipment_and_explicit_growth_mastery_are_native_inputs() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers[0].owner_skill =
        Some(serde_json::from_value(json!({"skill_id":c.skill.id})).unwrap());
    let origin = skill(&c);
    hit(&mut w, &c, 0, 0, Some("a"), origin.clone(), 0);
    assert_eq!(w.members[0].spirit, 51);
    w.members[0].skills.declared = true;
    w.members[0].skills.loadout.equipped_skill_ids.clear();
    hit(&mut w, &c, 0, 0, Some("b"), origin.clone(), 0);
    assert_eq!(w.members[0].spirit, 51);
    w.members[0].skills.loadout.equipped_skill_ids = vec![c.skill.id.clone()];
    c.combat_rules.as_mut().unwrap().triggers[0]
        .owner_skill
        .as_mut()
        .unwrap()
        .minimum_mastery = Some(1);
    hit(&mut w, &c, 0, 0, Some("c"), origin.clone(), 0);
    assert_eq!(w.members[0].spirit, 51);
    build(&mut c, &mut w, 0, 1);
    let profile = c
        .combat_rules
        .as_ref()
        .unwrap()
        .default_member_profile
        .clone();
    let profile_build = w.members[0].combat_build.take();
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut(&profile)
        .unwrap()
        .build = profile_build;
    hit(
        &mut w,
        &c,
        0,
        0,
        Some("profile-is-not-member-input"),
        origin.clone(),
        0,
    );
    assert_eq!(w.members[0].spirit, 51);
    build(&mut c, &mut w, 0, 0);
    hit(&mut w, &c, 0, 0, Some("d"), origin.clone(), 0);
    assert_eq!(w.members[0].spirit, 51);
    w.members[0]
        .combat_build
        .as_mut()
        .unwrap()
        .powers
        .get_mut(&c.skill.id)
        .unwrap()
        .mastery = 1;
    hit(&mut w, &c, 0, 0, Some("e"), origin, 0);
    assert_eq!(w.members[0].spirit, 52);
}
#[test]
fn source_gate_action_limits_one_two_three_eight_owner_and_target_are_independent() {
    for limit in [1, 2, 3, 8] {
        for target_scope in [true, false] {
            let (mut c, mut w) = setup();
            let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
            t.counting = crate::config::TriggerCounting::Action;
            t.maximum_per_action = limit;
            t.counter_scope = if target_scope {
                crate::config::TriggerScope::Target
            } else {
                crate::config::TriggerScope::Owner
            };
            for n in 0..12 {
                let origin = skill(&c);
                hit(&mut w, &c, 0, n % 2, Some("same"), origin, 0);
            }
            assert_eq!(
                w.members[0].spirit,
                50 + if target_scope {
                    limit.min(6) * 2
                } else {
                    limit
                }
            );
            let origin = skill(&c);
            hit(&mut w, &c, 0, 0, Some("next"), origin, 0);
            assert_eq!(
                w.members[0].spirit,
                51 + if target_scope {
                    limit.min(6) * 2
                } else {
                    limit
                }
            );
        }
    }
}
#[test]
fn source_gate_explicit_multiple_attempts_require_a_real_action_identity() {
    let (mut c, mut w) = setup();
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.counting = crate::config::TriggerCounting::Action;
    t.maximum_per_action = 3;
    let origin = skill(&c);
    for _ in 0..5 {
        hit(&mut w, &c, 0, 0, None, origin.clone(), 0);
    }
    assert_eq!(w.members[0].spirit, 50);
}
#[test]
fn source_gate_unit_commit_and_periodic_inherit_the_actual_native_skill() {
    let (mut c, mut w) = setup();
    let id = c.skill.id.clone();
    filter(
        &mut c,
        json!({"basic":false,"skills":{"mode":"only","ids":[id]}}),
    );
    c.combat_rules.as_mut().unwrap().triggers[0].include_periodic = true;
    c.skill.on_hit_target_statuses = vec!["wound".into()];
    super::unit_effect_tests::start(&c, &mut w, false);
    assert_eq!(w.members[0].spirit, 49); // cost2 then one true direct damage reward
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "wound")
        .unwrap();
    assert_eq!(w.statuses.action_origin(&s.instance_id), skill(&c).as_ref());
    super::periodic_tests::pulse(&mut w, &c, 660);
    assert_eq!(w.members[0].spirit, 50);
}
fn build(c: &mut Config, w: &mut World, i: usize, mastery: usize) {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/power-a.json"
    ))
    .unwrap();
    c.combat_rules.as_mut().unwrap().build_rules =
        Some(serde_json::from_value(doc["combat_rules"]["build_rules"].clone()).unwrap());
    c.combat_rules
        .as_mut()
        .unwrap()
        .skills
        .get_mut(&c.skill.id)
        .unwrap()
        .base_power = None;
    let input:CombatBuild=serde_json::from_value(json!({"power_points":0,"skill_power_points":0,"element_resistance_points":0,"powers":{c.skill.id.clone():{"model_id":"growth","rank":0,"quality":0,"level":0,"mastery":mastery}}})).unwrap();
    assert!(c.valid_combat_build(&input));
    w.members[i].combat_build = Some(input);
}
#[test]
fn source_gate_current_variant_equipment_is_separate_from_canonical_growth_record() {
    let (mut c, mut w) = setup();
    build(&mut c, &mut w, 0, 1);
    let variant = c.additional_skills[0].id.clone();
    c.combat_rules.as_mut().unwrap().triggers[0].owner_skill = Some(
        serde_json::from_value(
            json!({"skill_id":c.skill.id,"equipped_any_of":[variant],"minimum_mastery":1}),
        )
        .unwrap(),
    );
    w.members[0].skills.declared = true;
    w.members[0].skills.loadout.equipped_skill_ids = vec![variant];
    let origin = skill(&c);
    hit(&mut w, &c, 0, 0, Some("committed-base"), origin.clone(), 0);
    assert_eq!(w.members[0].spirit, 51);
    w.members[0].skills.loadout.equipped_skill_ids = vec![c.skill.id.clone()];
    hit(&mut w, &c, 0, 0, Some("other-equipment"), origin, 0);
    assert_eq!(w.members[0].spirit, 51);
}
#[test]
fn source_gate_native_skill_committed_and_ally_commit_select_their_exact_skill() {
    for ally in [true, false] {
        let (mut c, mut w) = super::unit_effect_tests::setup(false, ally);
        let kind = if ally {
            "ally_skill_committed"
        } else {
            "skill_committed"
        };
        c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"cast-only","profiles":["member"],"when":kind,"include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"action_filter":{"basic":false,"skills":{"mode":"only","ids":[c.skill.id]}},"effect":{"kind":"restore_spirit","amount":3}}])).unwrap();
        super::unit_effect_tests::start(&c, &mut w, ally);
        assert_eq!(w.members[0].spirit, 51);
        assert_eq!(w.members[1].spirit, 50);
    }
}
#[test]
fn source_gate_periodic_transfer_ignore_and_refresh_have_precise_classification() {
    let (mut c, mut w) = setup();
    filter(&mut c, json!({"basic":true,"skills":{"mode":"any"}}));
    c.combat_rules.as_mut().unwrap().triggers[0].include_periodic = true;
    c.skill.on_hit_target_statuses = vec!["wound".into()];
    super::unit_effect_tests::start(&c, &mut w, false);
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "wound")
        .unwrap()
        .clone();
    let target = status_entity(&w.npcs[1].entity);
    w.statuses
        .retarget(std::slice::from_ref(&s.instance_id), &target, &mut vec![]);
    assert_eq!(w.statuses.action_origin(&s.instance_id), skill(&c).as_ref());
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|s| s.id == "wound")
        .unwrap()
        .duplicate_policy = crate::config::DuplicatePolicy::Ignore;
    let t = w.npcs[1].entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), t.clone(), "wound", 90);
    assert_eq!(w.statuses.action_origin(&s.instance_id), skill(&c).as_ref());
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|s| s.id == "wound")
        .unwrap()
        .duplicate_policy = crate::config::DuplicatePolicy::Refresh;
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), t, "wound", 90);
    assert!(w.statuses.action_origin(&s.instance_id).is_none());
    let before = w.members[0].spirit;
    super::periodic_tests::pulse(&mut w, &c, 690);
    assert_eq!(w.members[0].spirit, before);
}
#[test]
fn source_gate_original_source_qualification_controls_third_party_reaction_without_consuming() {
    let (mut c, mut w) = setup();
    build(&mut c, &mut w, 0, 1);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"gated-control","mechanism":"timed_control","attribute":"root","duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let r = c.combat_rules.as_mut().unwrap();
    r.triggers.clear();
    r.status_policies.insert(
        "gated-control".into(),
        serde_json::from_value(
            json!({"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}),
        )
        .unwrap(),
    );
    r.contact_reactions.insert("gated-control".into(),serde_json::from_value(json!({"when":"direct_contact","attacker":"any_hostile","fixed":0,"source_power_bps":2500,"damage_kind":"physical","ignore_armor":true,"armor_penetration_cap_bps":10000,"source_requirement":{"skill_id":c.skill.id,"minimum_mastery":1}})).unwrap());
    r.status_power_sources
        .insert("gated-control".into(), c.skill.id.clone());
    let target = w.npcs[0].entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(1), target, "gated-control", 0);
    let origin = skill(&c);
    hit(
        &mut w,
        &c,
        0,
        0,
        Some("provoker-qualified"),
        origin.clone(),
        0,
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gated-control")
    );
    build(&mut c, &mut w, 1, 1);
    let expected = w.selected_power_with(&w.statuses, Owner::Member(1), &c.skill.id, &c, 0) / 4;
    let e = hit(&mut w, &c, 0, 0, Some("original-now-qualified"), origin, 0);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gated-control")
    );
    assert!(e.iter().any(|(i,e)|*i==Some(1)&&matches!(e,EventV4::Private{event:Event::ContactReactionResolved{source,raw_damage,..}}if source.id=="P01"&&*raw_damage==expected)));
}
#[test]
fn source_gate_cold_loader_rejects_unknown_conflicting_refs_and_indices() {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/power-a.json"
    ))
    .unwrap();
    d["combat_rules"]["triggers"] = json!([{"id":"source-filter","profiles":["member"],"when":"damage_dealt","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"counting":"action","maximum_per_action":3,"action_filter":{"basic":true,"skills":{"mode":"except","ids":["growth-strike"]}},"owner_skill":{"skill_id":"growth-strike","equipped_any_of":["growth-strike","growth-heal"],"minimum_mastery":1},"effect":{"kind":"restore_spirit","amount":3}}]);
    p.write(&d);
    p.load().unwrap();
    for n in 0..20 {
        let mut x = d.clone();
        let t = &mut x["combat_rules"]["triggers"][0];
        match n {
            0 => *t.get_mut("action_filter").unwrap() = Value::Null,
            1 => t["action_filter"]["basic"] = json!(1),
            2 => t["action_filter"]["skills"] = json!({"mode":"all"}),
            3 => t["action_filter"]["skills"]["ids"] = json!(["missing"]),
            4 => t["action_filter"]["skills"]["ids"] = json!(["growth-strike", "growth-strike"]),
            5 => t["action_filter"]["skills"] = json!({"mode":"any","ids":["growth-strike"]}),
            6 => t["owner_skill"] = Value::Null,
            7 => t["owner_skill"]["skill_id"] = json!("missing"),
            8 => t["owner_skill"]["minimum_mastery"] = Value::Null,
            9 => t["owner_skill"]["minimum_mastery"] = json!(128),
            10 => t["owner_skill"]["equipped_any_of"] = json!([]),
            11 => t["owner_skill"]["equipped_any_of"] = json!(["missing"]),
            12 => t["owner_skill"]["equipped_any_of"] = json!(["growth-heal", "growth-heal"]),
            13 => t["maximum_per_action"] = json!(0),
            14 => t["maximum_per_action"] = json!(9),
            15 => t["counting"] = json!("contact"),
            16 => t["maximum_per_action"] = Value::Null,
            17 => x["mechanism_version"] = json!(104),
            18 => t["owner_skill"]["minimum_mastery"] = json!(127),
            19 => t["action_filter"] = json!({"basic":false,"skills":{"mode":"only","ids":[]}}),
            _ => unreachable!(),
        };
        p.write(&x);
        assert!(p.load().is_err(), "{n}");
    }
}
#[test]
fn source_gate_completed_action_total_uses_frozen_origin_instead_of_later_global_context() {
    let (mut c, mut w) = setup();
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.counting = crate::config::TriggerCounting::ActionTotal;
    filter(&mut c, json!({"basic":false,"skills":{"mode":"any"}}));
    let origin = skill(&c);
    hit(&mut w, &c, 0, 0, Some("completed-skill"), origin, 0);
    w.triggers.origin = Some(ActionOrigin::Basic);
    w.finish_trigger_actions(&c, 0, &mut vec![]);
    assert_eq!(w.members[0].spirit, 51);
    w.finish_trigger_actions(&c, 0, &mut vec![]);
    assert_eq!(w.members[0].spirit, 51);
}
#[test]
fn source_gate_mixed_classification_for_one_action_cannot_steal_an_existing_total() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers[0].counting =
        crate::config::TriggerCounting::ActionTotal;
    filter(&mut c, json!({"basic":true,"skills":{"mode":"any"}}));
    let origin = skill(&c);
    hit(&mut w, &c, 0, 0, Some("conflict"), origin, 0);
    hit(
        &mut w,
        &c,
        0,
        0,
        Some("conflict"),
        Some(ActionOrigin::Basic),
        0,
    );
    w.finish_trigger_actions(&c, 0, &mut vec![]);
    assert_eq!(w.members[0].spirit, 50);
}
#[test]
fn source_gate_same_action_text_from_different_members_has_independent_quota() {
    let (mut c, mut w) = setup();
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.counting = crate::config::TriggerCounting::Action;
    t.maximum_per_action = 2;
    for i in 0..2 {
        for _ in 0..3 {
            let origin = skill(&c);
            hit(&mut w, &c, i, 0, Some("shared-text"), origin, 0);
        }
    }
    assert_eq!((w.members[0].spirit, w.members[1].spirit), (52, 52));
}
#[test]
fn source_gate_capacity_failure_spends_only_attempt_quota_and_preserves_layers_and_icd() {
    let (mut c, mut w) = setup();
    for (id, stacks) in [("gate-charge", true), ("gate-gift", false)] {
        let mut d = json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":100,"duration_ms":6000,"duplicate_policy":if stacks{"stack"}else{"refresh"},"source_scoped":true});
        if stacks {
            d["max_stacks"] = json!(2)
        };
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(d).unwrap());
    }
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.counting = crate::config::TriggerCounting::Action;
    t.maximum_per_action = 2;
    t.cooldown_ms = 6000;
    t.conditions=serde_json::from_value(json!({"status":{"subject":"owner","definition_id":"gate-charge","at_least":2,"consume":1}})).unwrap();
    t.effect = serde_json::from_value(json!({"kind":"apply_self_status","status_id":"gate-gift"}))
        .unwrap();
    let entity = w.members[0].reference.entity.clone();
    for _ in 0..2 {
        super::periodic_tests::bind(
            &mut w,
            &c,
            Owner::Member(0),
            entity.clone(),
            "gate-charge",
            0,
        );
    }
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    for _ in 0..2 {
        let origin = skill(&c);
        hit(&mut w, &c, 0, 0, Some("capacity-failed"), origin, 0);
    }
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 2;
    let origin = skill(&c);
    hit(&mut w, &c, 0, 0, Some("capacity-failed"), origin.clone(), 0);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gate-charge" && s.stacks == Some(2))
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gate-gift")
    );
    hit(&mut w, &c, 0, 0, Some("fresh"), origin, 0);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gate-charge" && s.stacks == Some(1))
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gate-gift")
    );
}
#[test]
fn source_gate_npc_installed_skill_and_profile_growth_are_distinct_from_member_builds() {
    for mastery in [0, 1] {
        let (mut c, mut w) = setup();
        let id = w.npcs[0].profile.combat.projectile.id.clone();
        let profile = c.combat_rules.as_ref().unwrap().default_npc_profile.clone();
        let doc: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/power-a.json"
        ))
        .unwrap();
        c.combat_rules.as_mut().unwrap().build_rules =
            Some(serde_json::from_value(doc["combat_rules"]["build_rules"].clone()).unwrap());
        c.combat_rules.as_mut().unwrap().profiles.get_mut(&profile).unwrap().build=Some(serde_json::from_value(json!({"power_points":0,"skill_power_points":0,"element_resistance_points":0,"powers":{id.clone():{"model_id":"growth","rank":0,"quality":0,"level":0,"mastery":mastery}}})).unwrap());
        let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
        t.profiles = vec![profile];
        t.owner_skill =
            Some(serde_json::from_value(json!({"skill_id":id,"minimum_mastery":1})).unwrap());
        t.action_filter = Some(
            serde_json::from_value(json!({"basic":true,"skills":{"mode":"only","ids":[]}}))
                .unwrap(),
        );
        w.npcs[0].spirit = 0;
        let source = w.npcs[0].entity.clone();
        w.hurt_member(0, source, 1, &c, 0, &mut vec![]);
        assert_eq!(w.npcs[0].spirit, mastery as u32);
    }
}
fn commit_kind(c: &mut Config, w: &mut World, kind: &str) {
    c.skill = serde_json::from_value(super::member_unavoidable_tests::skill(kind)).unwrap();
    if kind == "ground_field" {
        c.members.as_mut().unwrap().area_capacity = Some(8);
    }
    c.combat_rules.as_mut().unwrap().skills.insert(
        c.skill.id.clone(),
        crate::config::AttackRule {
            base_power: Some(8),
            can_crit: false,
            ignore_armor: true,
            ..Default::default()
        },
    );
    let id = c.skill.id.clone();
    filter(
        c,
        json!({"basic":false,"skills":{"mode":"only","ids":[id]}}),
    );
    let action = if kind == "ground_field" {
        Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: w.npcs[0].position,
        }
    } else {
        Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        }
    };
    w.action(0, &action, "native-source-cast", c, 0).unwrap();
    let p = w.members[0].pending.take().unwrap();
    w.commit(0, p, c, 30, &mut vec![]);
}
#[test]
fn source_gate_committed_projectile_and_piercing_keep_classification_after_slot_revoke() {
    for kind in ["projectile", "piercing_projectile"] {
        for live_gate in [true, false] {
            let (mut c, mut w) = setup();
            commit_kind(&mut c, &mut w, kind);
            if live_gate {
                c.combat_rules.as_mut().unwrap().triggers[0].owner_skill =
                    Some(serde_json::from_value(json!({"skill_id":c.skill.id})).unwrap());
            }
            let count = if kind == "piercing_projectile" { 2 } else { 1 };
            assert_eq!(w.shots[0].origin, ActionOrigin::Skill(c.skill.id.clone()));
            w.replace_skill_loadout(
                0,
                SkillLoadoutState {
                    declared: true,
                    revision: 1,
                    loadout: SkillLoadout {
                        owned_skill_ids: vec![c.skill.id.clone()],
                        equipped_skill_ids: vec![],
                    },
                },
                30,
            );
            let hp = w.npcs.iter().map(|n| n.hp).sum::<u32>();
            w.advance_shots(&c, 120, 0.2, &mut vec![]);
            assert!(w.npcs.iter().map(|n| n.hp).sum::<u32>() < hp);
            assert_eq!(w.members[0].spirit, 48 + if live_gate { 0 } else { count });
        }
    }
}
#[test]
fn source_gate_field_ticks_keep_original_source_and_multiple_attempt_allowance() {
    let (mut c, mut w) = setup();
    commit_kind(&mut c, &mut w, "ground_field");
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.counting = crate::config::TriggerCounting::Action;
    t.maximum_per_action = 2;
    w.replace_skill_loadout(
        0,
        SkillLoadoutState {
            declared: true,
            revision: 1,
            loadout: SkillLoadout {
                owned_skill_ids: vec![c.skill.id.clone()],
                equipped_skill_ids: vec![],
            },
        },
        30,
    );
    w.advance_areas(&c, 150, &mut vec![]);
    w.advance_areas(&c, 270, &mut vec![]);
    w.advance_areas(&c, 390, &mut vec![]);
    assert_eq!(w.members[0].spirit, 50);
}
#[test]
fn source_gate_channel_pulses_and_nth_status_share_parent_classification() {
    let (mut c, mut w) = super::unit_effect_tests::setup(true, false);
    c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"two-pulses","profiles":["member"],"when":"damage_dealt","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"counting":"action","maximum_per_action":2,"action_filter":{"basic":false,"skills":{"mode":"only","ids":[c.skill.id]}},"effect":{"kind":"restore_spirit","amount":1}}])).unwrap();
    c.skill.channel.as_mut().unwrap().on_success =
        serde_json::from_value(json!([{"after_pulse":2,"target_statuses":["wound"]}])).unwrap();
    super::unit_effect_tests::start(&c, &mut w, false);
    for now in [120, 180, 240] {
        w.advance_channel(0, &c, now, &mut vec![]);
    }
    assert_eq!(w.members[0].spirit, 47);
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "wound")
        .unwrap();
    assert_eq!(w.statuses.action_origin(&s.instance_id), skill(&c).as_ref());
    assert!(w.triggers.origin.is_none());
    assert!(w.triggers.action_source.is_none());
}
#[test]
fn source_gate_contact_requirement_is_strictly_loaded_against_source_models_and_abilities() {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/power-a.json"
    ))
    .unwrap();
    d["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({"id":"cold-root","mechanism":"timed_control","attribute":"root","duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true}));
    d["combat_rules"]["status_policies"] = json!({"cold-root":{"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}});
    d["combat_rules"]["contact_reactions"] = json!({"cold-root":{"when":"direct_contact","attacker":"any_hostile","fixed":0,"source_power_bps":2500,"damage_kind":"physical","ignore_armor":true,"armor_penetration_cap_bps":10000,"source_requirement":{"skill_id":"growth-strike","minimum_mastery":1,"equipped_any_of":["growth-strike","growth-heal"]}}});
    p.write(&d);
    p.load().unwrap();
    for n in 0..7 {
        let mut bad = d.clone();
        let r = &mut bad["combat_rules"]["contact_reactions"]["cold-root"];
        match n {
            0 => r["source_requirement"] = Value::Null,
            1 => r["source_requirement"]["skill_id"] = json!("missing"),
            2 => r["source_requirement"]["minimum_mastery"] = json!(128),
            3 => r["source_requirement"]["equipped_any_of"] = json!([]),
            4 => r["source_requirement"]["equipped_any_of"] = Value::Null,
            5 => r["source_requirement"]["extra"] = json!(true),
            6 => {
                bad["combat_rules"]
                    .as_object_mut()
                    .unwrap()
                    .remove("build_rules");
            }
            _ => unreachable!(),
        };
        p.write(&bad);
        assert!(p.load().is_err(), "{n}");
    }
}
#[test]
fn source_gate_original_sword_exclusion_and_ice_per_cast_caps_use_native_facts() {
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_gate_vectors.json"
    ))
    .unwrap();
    assert_eq!(v["fullOriginalStageBindingsAccepted"], false);
    let (mut c, mut w) = setup();
    let sword = &v["sword"];
    c.skill.id = sword["skillId"].as_str().unwrap().into();
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.counting = crate::config::TriggerCounting::Action;
    t.maximum_per_action = sword["perActionLimit"].as_u64().unwrap() as u32;
    t.action_filter = Some(
        serde_json::from_value(
            json!({"basic":true,"skills":{"mode":"except","ids":sword["excludedSkillIds"]}}),
        )
        .unwrap(),
    );
    let origins = [
        Some(ActionOrigin::Basic),
        Some(ActionOrigin::Skill(
            sword["eligibleSkillId"].as_str().unwrap().into(),
        )),
        Some(ActionOrigin::Skill(
            sword["excludedSkillIds"][0].as_str().unwrap().into(),
        )),
        Some(ActionOrigin::Skill(
            sword["excludedSkillIds"][1].as_str().unwrap().into(),
        )),
        None,
    ];
    for (i, origin) in origins.into_iter().enumerate() {
        hit(&mut w, &c, 0, 0, Some(&format!("source-{i}")), origin, 0);
    }
    assert_eq!(
        w.members[0].spirit,
        50 + sword["expectedRewards"].as_u64().unwrap() as u32
    );
    let (mut c, mut w) = setup();
    let quota = v["ice"]["perTargetActionLimit"].as_u64().unwrap() as u32;
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.counting = crate::config::TriggerCounting::Action;
    t.counter_scope = crate::config::TriggerScope::Target;
    t.maximum_per_action = quota;
    for _ in 0..quota + 1 {
        let origin = Some(ActionOrigin::Skill(
            v["ice"]["skillId"].as_str().unwrap().into(),
        ));
        hit(&mut w, &c, 0, 0, Some("one-original-cast"), origin, 0);
    }
    assert_eq!(w.members[0].spirit, 50 + quota);
}
#[test]
fn source_gate_original_shatter_stage_input_uses_original_owner_and_exact_damage_ratio() {
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_gate_vectors.json"
    ))
    .unwrap();
    let ice = &v["ice"];
    let minimum = ice["qualifiedStageInputIndex"].as_u64().unwrap() as usize;
    assert_eq!(
        (
            ice["sourceFactorThreshold"].as_u64(),
            ice["qualifiedStageSourceMinimum"].as_u64()
        ),
        (Some(800), Some(801))
    );
    assert_eq!(v["originalRawProficiencyDerivationAccepted"], false);
    for mastery in [minimum - 1, minimum] {
        let (mut c, mut w) = setup();
        c.skill.id = ice["skillId"].as_str().unwrap().into();
        let id = c.skill.id.clone();
        let r = c.combat_rules.as_mut().unwrap();
        r.triggers.clear();
        r.skills.insert(id.clone(), Default::default());
        r.build_rules=Some(serde_json::from_value(json!({"conversion":{"hp_bps":10000,"spirit_bps":10000,"atk_bps":10000,"def_bps":10000,"spd_bps":10000,"element_resistance_bps":10000},"maximum_input_points":10000,"power_models":{"stages":{"rank_base":[100],"quality_add_bps":[0],"level_add_bps":[0],"mastery_add_bps":[0,0,0,0,0],"external_bps":10000,"maximum_power":1000000000}}})).unwrap());
        w.members[1].combat_build=Some(serde_json::from_value(json!({"power_points":0,"skill_power_points":0,"element_resistance_points":0,"powers":{id.clone():{"model_id":"stages","rank":0,"quality":0,"level":0,"mastery":mastery}}})).unwrap());
        assert!(c.valid_combat_build(w.members[1].combat_build.as_ref().unwrap()));
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"original-freeze","mechanism":"timed_control","attribute":"root","duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
        let r = c.combat_rules.as_mut().unwrap();
        r.status_power_sources
            .insert("original-freeze".into(), id.clone());
        r.status_policies.insert(
            "original-freeze".into(),
            serde_json::from_value(
                json!({"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}),
            )
            .unwrap(),
        );
        r.contact_reactions.insert("original-freeze".into(),serde_json::from_value(json!({"when":"direct_contact","attacker":"any_hostile","fixed":0,"source_power_bps":ice["shatterDamageBps"],"damage_kind":"physical","ignore_armor":true,"armor_penetration_cap_bps":10000,"source_requirement":{"skill_id":id,"minimum_mastery":minimum}})).unwrap());
        let target = w.npcs[0].entity.clone();
        super::periodic_tests::bind(&mut w, &c, Owner::Member(1), target, "original-freeze", 0);
        let e = hit(
            &mut w,
            &c,
            0,
            0,
            Some("third-party"),
            Some(ActionOrigin::Basic),
            0,
        );
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "original-freeze"),
            mastery < minimum
        );
        assert_eq!(e.iter().filter(|(_,e)|matches!(e,EventV4::Private{event:Event::ContactReactionResolved{source,raw_damage,..}}if source.id=="P01"&&*raw_damage==ice["expectedRaw"].as_u64().unwrap()as u32)).count(),usize::from(mastery==minimum));
    }
}
#[test]
fn source_gate_private_action_classification_cannot_be_injected_and_cleans_with_its_status() {
    let (mut c, mut w) = setup();
    c.skill.on_hit_target_statuses = vec!["wound".into()];
    super::unit_effect_tests::start(&c, &mut w, false);
    let status = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "wound")
        .unwrap()
        .clone();
    let mut bad = serde_json::to_value(&status).unwrap();
    bad["action_origin"] = json!({"skill":"borrowed-skill"});
    assert!(serde_json::from_value::<StatusView>(bad).is_err());
    let mut action = json!({"kind":"cast_unit","skill_id":c.skill.id,"target":{"id":w.npcs[0].entity.id,"generation":1}});
    action["action_origin"] = json!("basic");
    assert!(serde_json::from_value::<Action>(action).is_err());
    let source = w.members[0].reference.entity.clone();
    w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(w.statuses.action_origin(&status.instance_id).is_none());
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.instance_id == status.instance_id)
    );
}
#[test]
fn source_gate_incoming_fact_uses_attacker_origin_and_defender_own_qualification() {
    for shielded in [true, false] {
        let (mut c, mut w) = setup();
        build(&mut c, &mut w, 0, 1);
        let skill_id = w.npcs[0].profile.combat.projectile.id.clone();
        let canonical = c.skill.id.clone();
        let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
        t.when = if shielded {
            crate::config::CombatFact::ShieldAbsorbed
        } else {
            crate::config::CombatFact::DamageTaken
        };
        t.owner_skill = Some(
            serde_json::from_value(json!({"skill_id":canonical,"minimum_mastery":1})).unwrap(),
        );
        t.action_filter = Some(
            serde_json::from_value(
                json!({"basic":false,"skills":{"mode":"only","ids":[skill_id.clone()]}}),
            )
            .unwrap(),
        );
        if shielded {
            let target = w.members[0].reference.entity.clone();
            super::periodic_tests::bind(&mut w, &c, Owner::Member(0), target, "ward", 0);
        }
        let hp = w.members[0].hp;
        let source = w.npcs[0].entity.clone();
        w.direct_contact(
            Owner::Npc(0),
            Owner::Member(0),
            combat::DirectContact {
                origin: Some(ActionOrigin::Skill(skill_id)),
                action_id: Some("npc-ability"),
                rule: Some(&crate::config::AttackRule {
                    can_crit: false,
                    ignore_armor: true,
                    ..Default::default()
                }),
                ignore_armor: true,
                unavoidable: true,
                dead_source: false,
                source: &source,
                raw: 1,
                metal: true,
                hit_statuses: &[],
            },
            &c,
            0,
            &mut vec![],
        );
        assert_eq!(w.members[0].spirit, 51);
        assert_eq!(w.members[0].hp, if shielded { hp } else { hp - 1 });
        assert!(w.triggers.origin.is_none());
    }
}
