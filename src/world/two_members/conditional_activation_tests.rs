use super::*;
use serde_json::json;
pub(super) fn setup(channel: bool) -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(channel, false);
    w.statuses = Default::default();
    for d in [
        json!({"id":"marks","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1,"duration_ms":10000,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true}),
        json!({"id":"source-power","mechanism":"timed_compound_modifier","modifiers":[{"attribute":"atk","modifier_bps":500},{"attribute":"def","modifier_bps":500}],"duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"target-drained","mechanism":"timed_compound_modifier","modifiers":[{"attribute":"atk","modifier_bps":-500},{"attribute":"def","modifier_bps":-500}],"duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true}),
    ] {
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(d).unwrap());
    }
    for (id, harmful) in [("source-power", false), ("target-drained", true)] {
        c.combat_rules.as_mut().unwrap().status_policies.insert(id.into(),serde_json::from_value(json!({"disposition":if harmful{"harmful"}else{"beneficial"},"dispellable":true,"transferable":false,"blocks":[]})).unwrap());
    }
    let own = w.members[0].reference.entity.clone();
    for _ in 0..3 {
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "marks", 0);
    }
    (c, w)
}
fn bundle(c: &mut Config) {
    c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"paired","profiles":["member"],"when":"unit_skill_committed","include_periodic":false,"every":1,"cooldown_ms":1000,"priority":1,"conditions":{"status":{"subject":"owner","definition_id":"marks","at_least":3,"consume":3}},"effect":{"kind":"apply_status_bundle","owner_statuses":["source-power"],"other_statuses":["target-drained"]}}])).unwrap();
}
fn marked(w: &World) -> bool {
    w.statuses
        .entries()
        .iter()
        .any(|s| s.definition_id == "marks")
}
fn steal() -> crate::config::ResourceSteal {
    serde_json::from_value(json!({"hp_current_bps":1000,"spirit_current_bps":1000,"hp_policy":"direct_transfer","status_bonus":{"mode":"multiply_base","subject":"owner","definition_id":"marks","per_stack_bps":1000,"maximum_bonus_bps":3000}})).unwrap()
}
#[test]
fn conditional_activation_native_unit_target_and_one_consumption_grant_both_sides() {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    let target = w.npcs[1].entity.clone();
    let before = w.members[0].spirit;
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&target),
        },
        "strict-second",
        &c,
        0,
    )
    .unwrap();
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, &c, 60, &mut e);
    assert!(!marked(&w));
    assert_eq!(w.members[0].spirit, before - 2);
    let a = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "source-power")
        .unwrap();
    let b = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "target-drained")
        .unwrap();
    assert_eq!(a.target.id, "P00");
    assert_eq!(b.target.id, target.id);
    assert_eq!(b.source.id, "P00");
    assert_eq!((a.expires_at_ms, b.expires_at_ms), (5060, 5060));
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "target-drained" && s.target.id == w.npcs[0].entity.id)
    );
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusConsumed {
                        consumed_stacks: 3,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
}
#[test]
fn conditional_activation_target_capacity_failure_keeps_source_condition_and_icd_for_retry() {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let target = w.npcs[0].entity.clone();
    super::periodic_tests::bind(
        &mut w,
        &c,
        Owner::Member(1),
        target.clone(),
        "source-power",
        0,
    );
    w.combat_fact(
        crate::config::CombatFact::UnitSkillCommitted,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        30,
        &mut vec![],
    );
    assert!(marked(&w));
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "source-power" && s.target.id == "P00")
    );
    let old = w.members[1].reference.entity.clone();
    w.clear_owned_statuses(&old, StatusRemovalReason::SourceInvalid, &mut vec![]);
    w.combat_fact(
        crate::config::CombatFact::UnitSkillCommitted,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        60,
        &mut vec![],
    );
    assert!(!marked(&w));
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "target-drained")
    );
}
#[test]
fn conditional_activation_old_self_commit_does_not_improvise_a_strict_unit_target() {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    w.combat_fact(
        crate::config::CombatFact::SkillCommitted,
        (Owner::Member(0), Owner::Member(0)),
        (0, false),
        &c,
        30,
        &mut vec![],
    );
    assert!(marked(&w));
    assert_eq!(w.statuses.entries().len(), 1);
}
#[test]
fn conditional_activation_resource_status_bonus_is_source_scoped_bounded_and_once_floored() {
    let (mut c, mut w) = setup(false);
    let d = steal();
    w.npcs[0].hp = 1005;
    w.npcs[0].spirit = 123;
    let source = w.members[0].reference.entity.clone();
    let target = w.npcs[0].entity.clone();
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 30),
        Some([130, 15])
    );
    let peer = w.members[1].reference.entity.clone();
    assert_eq!(
        w.resource_budget(&peer, &target, &d, &c, 30),
        Some([100, 12])
    );
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 10000),
        Some([100, 12])
    );
    let id = w.statuses.entries()[0].instance_id.clone();
    w.statuses.consume(&id, 2, &mut vec![]);
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 30),
        Some([110, 13])
    );
    c.skill.resource_steal = Some(d);
}
#[test]
fn conditional_activation_resource_budget_freezes_before_unit_commit_consumes_three_marks() {
    let (mut c, mut w) = setup(true);
    bundle(&mut c);
    c.skill.resource_steal = Some(steal());
    w.npcs[0].hp = 1005;
    w.npcs[0].spirit = 123;
    super::unit_effect_tests::start(&c, &mut w, false);
    assert!(!marked(&w));
    assert_eq!(
        w.members[0].channel.as_ref().unwrap().steal_budget,
        Some([130, 15])
    );
}
#[test]
fn conditional_activation_hit_trigger_freezes_resource_bonus_before_its_own_consumption() {
    let (mut c, mut w) = setup(false);
    w.npcs[0].hp = 1005;
    w.npcs[0].spirit = 123;
    c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"drain","profiles":["member"],"when":"direct_hit","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"conditions":{"status":{"subject":"owner","definition_id":"marks","at_least":3,"consume":3}},"effect":{"kind":"steal_other","steal":serde_json::to_value(steal()).unwrap()}}])).unwrap();
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        30,
        &mut vec![],
    );
    assert!(!marked(&w));
    assert_eq!(w.npcs[0].hp, 875);
    assert_eq!(w.npcs[0].spirit, 108);
}
#[test]
fn conditional_activation_mastery_closed_range_upper_only_and_missing_record() {
    let (mut c, mut w) = setup(false);
    let id = c.skill.id.clone();
    let mut r: crate::config::SkillRequirement =
        serde_json::from_value(json!({"skill_id":id,"minimum_mastery":1,"maximum_mastery":3}))
            .unwrap();
    for stage in 0..5 {
        w.members[0].combat_build=Some(serde_json::from_value(json!({"power_points":0,"skill_power_points":0,"element_resistance_points":0,"powers":{id.clone():{"model_id":"phase","rank":0,"quality":0,"level":0,"mastery":stage}}})).unwrap());
        assert_eq!(
            w.meets_skill_requirement(Owner::Member(0), &r, &c),
            (1..=3).contains(&stage)
        );
    }
    r.minimum_mastery = None;
    r.maximum_mastery = Some(0);
    w.members[0].combat_build = None;
    assert!(!w.meets_skill_requirement(Owner::Member(0), &r, &c));
    c.params.basic_spirit_on_hit = 0;
}
#[test]
fn conditional_activation_cold_ranges_bundles_and_resource_bonus_are_strict_and_source_owned() {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut d: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/power-a.json"
    ))
    .unwrap();
    let (mut c, _) = setup(false);
    bundle(&mut c);
    for def in c
        .pve
        .as_ref()
        .unwrap()
        .status_definitions
        .iter()
        .filter(|d| ["marks", "source-power", "target-drained"].contains(&d.id.as_str()))
    {
        d["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::to_value(def).unwrap());
    }
    d["skill"]["resource_steal"] = serde_json::to_value(steal()).unwrap();
    d["combat_rules"]["triggers"] =
        serde_json::to_value(&c.combat_rules.as_ref().unwrap().triggers).unwrap();
    for id in ["source-power", "target-drained"] {
        d["combat_rules"]["status_policies"][id] =
            serde_json::to_value(&c.combat_rules.as_ref().unwrap().status_policies[id]).unwrap();
    }
    for model in d["combat_rules"]["build_rules"]["power_models"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        model["mastery_add_bps"] = json!([0, 0, 0, 0, 0]);
    }
    d["combat_rules"]["triggers"][0]["owner_skill"] =
        json!({"skill_id":"growth-strike","minimum_mastery":1,"maximum_mastery":3});
    p.write(&d);
    p.load().unwrap();
    for case in 0..35 {
        let mut bad = d.clone();
        match case {
            0 => {
                bad["combat_rules"]["triggers"][0]["owner_skill"]["maximum_mastery"] =
                    serde_json::Value::Null
            }
            1 => bad["combat_rules"]["triggers"][0]["owner_skill"]["maximum_mastery"] = json!(0),
            2 => bad["combat_rules"]["triggers"][0]["owner_skill"]["maximum_mastery"] = json!(5),
            3 => bad["combat_rules"]["triggers"][0]["owner_skill"]["maximum_mastery"] = json!(128),
            4 => {
                bad["combat_rules"]["triggers"][0]["effect"]["owner_statuses"] =
                    json!(["source-power", "source-power"])
            }
            5 => {
                bad["combat_rules"]["triggers"][0]["effect"]["other_statuses"] =
                    serde_json::Value::Null
            }
            6 => {
                bad["combat_rules"]["triggers"][0]["effect"]["other_statuses"] =
                    json!(["source-power"])
            }
            7 => {
                bad["combat_rules"]["triggers"][0]["effect"]["owner_statuses"] =
                    json!(["target-drained"])
            }
            8 => {
                bad["combat_rules"]["triggers"][0]["effect"]["owner_statuses"] = json!(["missing"])
            }
            9 => {
                bad["combat_rules"]["triggers"][0]["effect"]["owner_statuses"] =
                    json!(["source-power", "source-power", "source-power"])
            }
            10 => {
                bad["combat_rules"]["triggers"][0]["effect"]["owner_statuses"] = json!([]);
                bad["combat_rules"]["triggers"][0]["effect"]["other_statuses"] = json!([]);
            }
            11 => bad["combat_rules"]["triggers"][0]["effect"]["extra"] = json!(true),
            12 => bad["combat_rules"]["triggers"][0]["when"] = json!("status_stacks_reached"),
            13 => bad["combat_rules"]["triggers"][0]["include_periodic"] = json!(true),
            14 => bad["combat_rules"]["triggers"][0]["counting"] = json!("action_total"),
            15 => bad["skill"]["resource_steal"]["status_bonus"] = serde_json::Value::Null,
            16 => bad["skill"]["resource_steal"]["status_bonus"]["per_stack_bps"] = json!(0),
            17 => bad["skill"]["resource_steal"]["status_bonus"]["maximum_bonus_bps"] = json!(0),
            18 => bad["skill"]["resource_steal"]["status_bonus"]["per_stack_bps"] = json!(100001),
            19 => {
                bad["skill"]["resource_steal"]["status_bonus"]["maximum_bonus_bps"] = json!(100001)
            }
            20 => {
                bad["skill"]["resource_steal"]["status_bonus"]["definition_id"] = json!("missing")
            }
            21 => bad["skill"]["resource_steal"]["hp_current_bps"] = json!(10000),
            22 => bad["skill"]["resource_steal"]["status_bonus"]["subject"] = json!("unknown"),
            23 => bad["mechanism_version"] = json!(104),
            24 => {
                bad["skill"]["resource_steal"]["status_bonus"]
                    .as_object_mut()
                    .unwrap()
                    .remove("mode");
            }
            25 => bad["skill"]["resource_steal"]["status_bonus"]["mode"] = serde_json::Value::Null,
            26 => bad["skill"]["resource_steal"]["status_bonus"]["mode"] = json!("unknown"),
            27 => {
                bad["skill"]["resource_steal"]["status_bonus"]["qualification"] =
                    serde_json::Value::Null
            }
            28 => {
                bad["skill"]["resource_steal"]["status_bonus"]["qualification"] =
                    json!({"maximum_mastery":3})
            }
            29 => {
                bad["skill"]["resource_steal"]["status_bonus"]["qualification"] =
                    json!({"skill_id":"growth-strike","minimum_mastery":3,"maximum_mastery":1})
            }
            30 => {
                bad["skill"]["resource_steal"]["status_bonus"]["qualification"] =
                    json!({"skill_id":"growth-strike","maximum_mastery":5})
            }
            31 => {
                bad["skill"]["resource_steal"]["status_bonus"]["qualification"] =
                    json!({"skill_id":"unknown"})
            }
            32 => {
                bad["skill"]["resource_steal"]["status_bonus"]["qualification"] =
                    json!({"skill_id":"growth-strike","equipped_any_of":[]})
            }
            33 => {
                bad["skill"]["resource_steal"]["status_bonus"]["qualification"] =
                    json!({"skill_id":"growth-strike","maximum_mastery":null})
            }
            _ => {
                bad["skill"]["resource_steal"]["status_bonus"]["qualification"] =
                    json!({"skill_id":"growth-strike","private_budget":1})
            }
        }
        p.write(&bad);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn conditional_activation_additive_mode_is_explicit_and_never_enables_disabled_hp_or_spirit() {
    let (c, mut w) = setup(false);
    w.npcs[0].hp = 1005;
    w.npcs[0].spirit = 123;
    let source = w.members[0].reference.entity.clone();
    let target = w.npcs[0].entity.clone();
    let mut d = steal();
    d.status_bonus.as_mut().unwrap().mode = crate::config::ResourceBonusMode::AddCurrentBps;
    assert!(d.valid(&c));
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 30),
        Some([402, 49])
    );
    d.hp_current_bps = 0;
    d.hp_policy = None;
    assert!(d.valid(&c));
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 30),
        Some([0, 49])
    );
    d.hp_current_bps = 1000;
    d.hp_policy = Some(ResourceHpPolicy::DirectTransfer);
    d.spirit_current_bps = 0;
    assert!(d.valid(&c));
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 30),
        Some([402, 0])
    );
}
#[test]
fn conditional_activation_resource_bonus_qualification_reads_trusted_stage_and_live_equipment() {
    let (mut c, mut w) = setup(false);
    let mut d = steal();
    let id = c.skill.id.clone();
    d.status_bonus.as_mut().unwrap().qualification = Some(
        serde_json::from_value(json!({"skill_id":id,"minimum_mastery":1,"maximum_mastery":3}))
            .unwrap(),
    );
    w.npcs[0].hp = 1005;
    w.npcs[0].spirit = 123;
    let source = w.members[0].reference.entity.clone();
    let target = w.npcs[0].entity.clone();
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 30),
        Some([100, 12])
    );
    for stage in 0..5 {
        w.members[0].combat_build=Some(serde_json::from_value(json!({"power_points":0,"skill_power_points":0,"element_resistance_points":0,"powers":{id.clone():{"model_id":"phase","rank":0,"quality":0,"level":0,"mastery":stage}}})).unwrap());
        assert_eq!(
            w.resource_budget(&source, &target, &d, &c, 30),
            Some(if (1..=3).contains(&stage) {
                [130, 15]
            } else {
                [100, 12]
            })
        );
    }
    w.members[0]
        .combat_build
        .as_mut()
        .unwrap()
        .powers
        .get_mut(&id)
        .unwrap()
        .mastery = 1;
    w.members[0].skills.declared = true;
    w.members[0].skills.loadout.equipped_skill_ids.clear();
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 30),
        Some([100, 12])
    );
    w.members[0].skills.loadout.equipped_skill_ids.push(id);
    assert_eq!(
        w.resource_budget(&source, &target, &d, &c, 30),
        Some([130, 15])
    );
    c.params.basic_spirit_on_hit = 0;
}
#[test]
fn conditional_activation_combined_owner_capacity_and_partial_consumption_are_atomic() {
    for consume in [2, 3] {
        let (mut c, mut w) = setup(false);
        bundle(&mut c);
        c.combat_rules.as_mut().unwrap().triggers[0]
            .conditions
            .status
            .as_mut()
            .unwrap()
            .consume = consume;
        c.pve_mut()
            .status_rules
            .as_mut()
            .unwrap()
            .max_active_per_entity = 1;
        w.combat_fact(
            crate::config::CombatFact::UnitSkillCommitted,
            (Owner::Member(0), Owner::Npc(0)),
            (0, false),
            &c,
            30,
            &mut vec![],
        );
        assert_eq!(marked(&w), consume == 2);
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "source-power"),
            consume == 3
        );
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "target-drained"),
            consume == 3
        );
    }
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"extra-power","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1,"duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules.as_mut().unwrap().status_policies.insert(
        "extra-power".into(),
        serde_json::from_value(
            json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]}),
        )
        .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().triggers[0].effect=serde_json::from_value(json!({"kind":"apply_status_bundle","owner_statuses":["source-power","extra-power"],"other_statuses":["target-drained"]})).unwrap();
    w.combat_fact(
        crate::config::CombatFact::UnitSkillCommitted,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        30,
        &mut vec![],
    );
    assert!(marked(&w));
    assert_eq!(w.statuses.entries().len(), 1);
}
#[test]
fn conditional_activation_blocked_target_probability_and_shared_budget_do_not_consume_or_grant() {
    for mode in 0..3 {
        let (mut c, mut w) = setup(false);
        bundle(&mut c);
        match mode {
            0 => {
                c.combat_rules
                    .as_mut()
                    .unwrap()
                    .status_policies
                    .get_mut("source-power")
                    .unwrap()
                    .blocks = vec![crate::config::StatusDisposition::Harmful];
                let target = w.npcs[0].entity.clone();
                super::periodic_tests::bind(
                    &mut w,
                    &c,
                    Owner::Member(1),
                    target,
                    "source-power",
                    0,
                );
            }
            1 => c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(0),
            _ => {
                c.combat_rules.as_mut().unwrap().max_triggers_per_step = 1;
                let rule=serde_json::from_value(json!({"id":"first","profiles":["member"],"when":"skill_committed","include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"restore_spirit","amount":1}})).unwrap();
                c.combat_rules.as_mut().unwrap().triggers.push(rule);
            }
        }
        super::unit_effect_tests::start(&c, &mut w, false);
        assert!(marked(&w));
        assert!(
            !w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "source-power" && s.target.id == "P00")
        );
        assert!(
            !w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "target-drained")
        );
        assert!(w.npcs[0].hp < 1000);
    }
}
#[test]
fn conditional_activation_source_exit_cleans_both_bundle_ends_and_does_not_reassign_ownership() {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    super::unit_effect_tests::start(&c, &mut w, false);
    let source = w.members[0].reference.entity.clone();
    w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(w.statuses.entries().iter().all(|s| s.source.id != "P00"));
}
#[test]
fn conditional_activation_first_control_immunity_blocks_entire_pair_without_spending_it() {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"freeze","mechanism":"timed_control","attribute":"root","duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"first-immunity","mechanism":"timed_control","attribute":"first_control_immunity","duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules.as_mut().unwrap().status_policies.insert(
        "freeze".into(),
        serde_json::from_value(
            json!({"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]}),
        )
        .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().triggers[0].effect=serde_json::from_value(json!({"kind":"apply_status_bundle","owner_statuses":["source-power"],"other_statuses":["freeze"]})).unwrap();
    let target = w.npcs[0].entity.clone();
    super::periodic_tests::bind(
        &mut w,
        &c,
        Owner::Npc(0),
        target.clone(),
        "first-immunity",
        0,
    );
    let original = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "first-immunity")
        .unwrap()
        .clone();
    w.combat_fact(
        crate::config::CombatFact::UnitSkillCommitted,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        30,
        &mut vec![],
    );
    assert!(marked(&w));
    assert!(w.statuses.entries().iter().any(|s| s == &original));
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "source-power")
    );
    w.clear_owned_statuses(&target, StatusRemovalReason::SourceInvalid, &mut vec![]);
    w.combat_fact(
        crate::config::CombatFact::UnitSkillCommitted,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        60,
        &mut vec![],
    );
    assert!(!marked(&w));
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "freeze")
    );
}
#[test]
fn conditional_activation_combined_control_replacements_deduplicate_before_capacity() {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    for (id, axis) in [("freeze", "root"), ("daze", "stun")] {
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"timed_control","attribute":axis,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
        c.combat_rules.as_mut().unwrap().status_policies.insert(id.into(),serde_json::from_value(json!({"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]})).unwrap());
    }
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"softened","mechanism":"timed_compound_modifier","modifiers":[{"attribute":"movement_slow","modifier_bps":-5000},{"attribute":"basic_interval_delay","modifier_bps":1500}],"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules.as_mut().unwrap().control_policies.insert(
        "npc".into(),
        serde_json::from_value(
            json!({"max_duration_ms":1800,"replacements":{"freeze":"softened","daze":"softened"}}),
        )
        .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().triggers[0].effect=serde_json::from_value(json!({"kind":"apply_status_bundle","owner_statuses":["source-power"],"other_statuses":["freeze","daze"]})).unwrap();
    w.combat_fact(
        crate::config::CombatFact::UnitSkillCommitted,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        30,
        &mut vec![],
    );
    assert!(!marked(&w));
    let target = &status_entity(&w.npcs[0].entity);
    let statuses = w
        .statuses
        .entries()
        .iter()
        .filter(|s| s.target == *target)
        .collect::<Vec<_>>();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].definition_id, "softened");
}
#[test]
fn conditional_activation_npc_trigger_bundle_uses_its_own_source_and_actual_member_target() {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    let own = w.npcs[0].entity.clone();
    for _ in 0..3 {
        super::periodic_tests::bind(&mut w, &c, Owner::Npc(0), own.clone(), "marks", 0);
    }
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.when = crate::config::CombatFact::DirectHit;
    t.profiles = vec!["npc".into()];
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Npc(0), Owner::Member(1)),
        (0, false),
        &c,
        30,
        &mut vec![],
    );
    assert!(marked(&w));
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "marks" && s.source.id == own.id)
    );
    let target = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "target-drained")
        .unwrap();
    assert_eq!(target.source.id, own.id);
    assert_eq!(target.target.id, "P01");
    let self_buff = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "source-power")
        .unwrap();
    assert_eq!(self_buff.target.id, own.id);
}
#[test]
fn conditional_activation_committed_unit_projectile_gets_pair_before_its_native_flight_and_cancel_is_free()
 {
    for cancel in [true, false] {
        let (mut c, mut w) = setup(false);
        bundle(&mut c);
        c.skill=serde_json::from_value(json!({"id":"projectile-unit","mechanism":"unit_projectile","cost":2,"windup_ms":60,"recovery_ms":30,"cooldown_ms":600,"range":600,"speed":100,"radius":6,"damage_bps":10000})).unwrap();
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: World::unit_wire(&w.npcs[0].entity),
            },
            "prepare",
            &c,
            0,
        )
        .unwrap();
        if cancel {
            w.action(0, &Action::CancelAction {}, "cancel", &c, 30)
                .unwrap();
            assert!(marked(&w));
            assert!(w.shots.is_empty());
            assert_eq!(w.members[0].spirit, 50);
        } else {
            let p = w.members[0].pending.take().unwrap();
            w.commit(0, p, &c, 60, &mut vec![]);
            assert!(!marked(&w));
            assert_eq!(w.shots.len(), 1);
            assert_eq!(w.members[0].spirit, 48);
            assert!(
                w.statuses
                    .entries()
                    .iter()
                    .any(|s| s.definition_id == "target-drained")
            );
        }
    }
}
#[test]
fn conditional_activation_original_profiles_use_trusted_stage_and_atomic_five_second_parameters() {
    let v: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/conditional_activation_vectors.json"
    ))
    .unwrap();
    assert_eq!(v["originalHpPolicyAccepted"], false);
    assert_eq!(v["originalStackModeAccepted"], true);
    assert_eq!(v["approvedSourceMode"], "multiply_base");
    for mode in v["modeCases"].as_array().unwrap() {
        for stage in v["stages"].as_array().unwrap() {
            let (mut c, mut w) = setup(true);
            let id = c.skill.id.clone();
            let index = stage["index"].as_u64().unwrap() as usize;
            c.combat_rules.as_mut().unwrap().build_rules=Some(serde_json::from_value(json!({"conversion":{"hp_bps":10000,"spirit_bps":10000,"atk_bps":10000,"def_bps":10000,"spd_bps":10000,"element_resistance_bps":10000},"maximum_input_points":10000,"power_models":{"phase":{"rank_base":[100],"quality_add_bps":[0],"level_add_bps":[0],"mastery_add_bps":[0,0,0,0,0],"external_bps":10000,"maximum_power":1000000000}}})).unwrap());
            c.combat_rules
                .as_mut()
                .unwrap()
                .skills
                .get_mut(&id)
                .unwrap()
                .base_power = None;
            let build=serde_json::from_value(json!({"power_points":0,"skill_power_points":0,"element_resistance_points":0,"powers":{id.clone():{"model_id":"phase","rank":0,"quality":0,"level":0,"mastery":index}}})).unwrap();
            assert!(c.valid_combat_build(&build));
            w.members[0].combat_build = Some(build);
            if let Some(source_status) = stage["sourceStatus"].as_str() {
                if source_status == "mastered-power" {
                    let source = c
                        .pve
                        .as_ref()
                        .unwrap()
                        .status_definitions
                        .iter()
                        .find(|d| d.id == "source-power")
                        .unwrap();
                    let mut data = serde_json::to_value(source).unwrap();
                    data["id"] = json!(source_status);
                    data["modifiers"].as_array_mut().unwrap().push(
                        json!({"attribute":"life_steal","modifier_bps":v["masteredLifeStealBps"]}),
                    );
                    c.pve_mut()
                        .status_definitions
                        .push(serde_json::from_value(data).unwrap());
                    c.combat_rules.as_mut().unwrap().status_policies.insert(source_status.into(),serde_json::from_value(json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]})).unwrap());
                }
                bundle(&mut c);
                let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
                t.owner_skill = Some(
                    serde_json::from_value(
                        json!({"skill_id":id,"minimum_mastery":index,"maximum_mastery":index}),
                    )
                    .unwrap(),
                );
                t.effect=serde_json::from_value(json!({"kind":"apply_status_bundle","owner_statuses":[source_status],"other_statuses":["target-drained"]})).unwrap();
            }
            let mut resource = steal();
            resource.status_bonus.as_mut().unwrap().mode =
                serde_json::from_value(mode["mode"].clone()).unwrap();
            c.skill.resource_steal = Some(resource);
            w.npcs[0].hp = 1005;
            w.npcs[0].spirit = 123;
            super::unit_effect_tests::start(&c, &mut w, false);
            assert_eq!(
                w.members[0].channel.as_ref().unwrap().steal_budget,
                Some([
                    mode["hpBudget"].as_u64().unwrap() as u32,
                    mode["spiritBudget"].as_u64().unwrap() as u32
                ])
            );
            assert_eq!(marked(&w), stage["consume"] == 0);
            if let Some(source_status) = stage["sourceStatus"].as_str() {
                let s = w
                    .statuses
                    .entries()
                    .iter()
                    .find(|s| s.definition_id == source_status)
                    .unwrap();
                assert_eq!(
                    s.expires_at_ms - s.applied_at_ms,
                    v["durationMs"].as_u64().unwrap()
                );
                assert_eq!(
                    s.modifiers[0].modifier_bps,
                    v["sourceAttackBps"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    s.modifiers[1].modifier_bps,
                    v["sourceDefenseBps"].as_i64().unwrap() as i32
                );
                let target = w
                    .statuses
                    .entries()
                    .iter()
                    .find(|s| s.definition_id == "target-drained")
                    .unwrap();
                assert_eq!(
                    target.modifiers[0].modifier_bps,
                    v["targetAttackBps"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    target.modifiers[1].modifier_bps,
                    v["targetDefenseBps"].as_i64().unwrap() as i32
                );
            }
        }
    }
}
#[test]
fn conditional_activation_paired_attack_and_defense_modifiers_change_a_real_basic_hit_together() {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    for p in c.combat_rules.as_mut().unwrap().profiles.values_mut() {
        p.tier = 0;
        p.hit_bps = 10000;
        p.critical_chance_bps = 0;
        p.dodge_bps = 0;
    }
    w.members[0].facts.atk_points = 800;
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("def".into(), 200);
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("hp".into(), 10000);
    w.npcs[0].hp = 10000;
    super::unit_effect_tests::start(&c, &mut w, false);
    assert_eq!(
        w.entity_attack(&status_entity(&w.members[0].reference.entity), &c, 200),
        Some(840)
    );
    let target = w.npcs[0].entity.clone();
    w.action(
        0,
        &Action::AttackUnit {
            target: World::unit_wire(&target),
        },
        "basic-after-pair",
        &c,
        200,
    )
    .unwrap();
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, &c, 400, &mut e);
    let before = w.npcs[0].hp;
    w.advance_shots(&c, 430, 1., &mut e);
    assert_eq!(before - w.npcs[0].hp, 430);
}
#[test]
fn conditional_activation_private_eligibility_and_resource_context_cannot_enter_player_actions_or_views()
 {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    super::unit_effect_tests::start(&c, &mut w, false);
    let status = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "source-power")
        .unwrap();
    for field in [
        "minimum_mastery",
        "maximum_mastery",
        "qualification",
        "status_bonus",
        "budget",
        "action_origin",
        "derived",
    ] {
        let mut data = serde_json::to_value(status).unwrap();
        data[field] = json!({"trusted":true});
        assert!(serde_json::from_value::<StatusView>(data).is_err());
        let mut action = json!({"kind":"cast_unit","skill_id":c.skill.id,"target":{"id":w.npcs[0].entity.id,"generation":1}});
        action[field] = json!({"trusted":true});
        assert!(serde_json::from_value::<Action>(action).is_err());
    }
}
#[test]
fn conditional_activation_mislabeled_self_control_cannot_cold_load_or_partially_grant_the_other_end()
 {
    let (mut c, mut w) = setup(false);
    bundle(&mut c);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"self-root","mechanism":"timed_control","attribute":"root","harmful":false,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules.as_mut().unwrap().status_policies.insert(
        "self-root".into(),
        serde_json::from_value(
            json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]}),
        )
        .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().triggers[0].effect=serde_json::from_value(json!({"kind":"apply_status_bundle","owner_statuses":["self-root"],"other_statuses":["target-drained"]})).unwrap();
    super::unit_effect_tests::start(&c, &mut w, false);
    assert!(marked(&w));
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "target-drained")
    );
    assert_eq!(w.members[0].spirit, 48);
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut data: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/activation-a.json"
    ))
    .unwrap();
    data["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({"id":"self-root","mechanism":"timed_control","attribute":"root","harmful":false,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true}));
    data["combat_rules"]["status_policies"]["self-root"] =
        json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]});
    data["combat_rules"]["triggers"][0]["effect"]["owner_statuses"] = json!(["self-root"]);
    p.write(&data);
    assert!(p.load().is_err());
}
#[test]
fn conditional_activation_confirmed_source_mode_has_exact_zero_to_three_layer_budgets() {
    let v: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/conditional_activation_vectors.json"
    ))
    .unwrap();
    assert_eq!(v["approvedLayerRatiosBps"], json!([1000, 1100, 1200, 1300]));
    assert!(!v["originalHpPolicyAccepted"].as_bool().unwrap());
    for (i, budget) in v["approvedLayerBudgets"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let (c, mut w) = setup(false);
        w.statuses = Default::default();
        w.npcs[0].hp = 1005;
        w.npcs[0].spirit = 123;
        let source = w.members[0].reference.entity.clone();
        let target = w.npcs[0].entity.clone();
        for _ in 0..i {
            super::periodic_tests::bind(&mut w, &c, Owner::Member(0), source.clone(), "marks", 0);
        }
        for hp_policy in ["direct_transfer", "damage"] {
            let mut d = serde_json::to_value(steal()).unwrap();
            d["hp_policy"] = json!(hp_policy);
            d["status_bonus"]["mode"] = v["approvedSourceMode"].clone();
            let d = serde_json::from_value(d).unwrap();
            assert_eq!(
                json!(w.resource_budget(&source, &target, &d, &c, 30).unwrap()),
                *budget
            );
        }
    }
}
