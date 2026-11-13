use super::*;
use crate::config::{CombatFact, FinishPolicy, FinishRule, ThresholdComparison, TriggerEffect};
fn fixture() -> (Config, World) {
    let p = Package::new();
    p.write(
        &serde_json::from_str::<Value>(crate::test_data::fixture!(
            "examples/combat-host/finisher-a.json"
        ))
        .unwrap(),
    );
    let mut c = p.load().unwrap().default_config().as_ref().clone();
    c.combat_rules.as_mut().unwrap().triggers[0].conditions = Default::default();
    let seats = (0..4)
        .map(|i| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            combat_build: None,
            skills: None,
            participant_id: format!("P{i:02}"),
            authenticated_principal: format!("principal-P{i:02}"),
            connection: Connection {
                id: format!("c{i}"),
                generation: 1,
            },
        })
        .collect::<Vec<_>>();
    let facts = seats
        .iter()
        .map(|s| ParticipantFacts {
            principal: s.authenticated_principal.clone(),
            revision: 0,
            hp_points: 1000,
            spirit_points: 300,
            atk_points: 100,
            def_points: 100,
            spd_points: 8,
            warehouse_quantity: 100,
        })
        .collect();
    let mut w = World::new(&c, &seats, facts, 77);
    for m in &mut w.members {
        m.joined = true;
        m.ready = true;
    }
    w.activate(&c, 0, &mut vec![]);
    (c, w)
}
fn npc(w: &World, id: &str) -> usize {
    w.npcs.iter().position(|n| n.entity.id == id).unwrap()
}
fn rule(c: &mut Config) -> &mut FinishRule {
    let TriggerEffect::FinishOther { rule } =
        &mut c.combat_rules.as_mut().unwrap().triggers[0].effect
    else {
        panic!("fixture")
    };
    rule
}
fn give(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str) {
    let e = w.owner_entity(target);
    w.apply_owned_statuses(source, (e, 1000), &[id.into()], c, 0, &mut vec![]);
}
fn add(c: &mut Config, d: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
}
fn fire(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let mut e = vec![];
    w.combat_fact(
        CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(npc(w, id))),
        (1, false),
        c,
        now,
        &mut e,
    );
    e
}
fn result(e: &Events) -> Option<(FinisherMode, u32, u32, u32, bool)> {
    e.iter().find_map(|(_, e)| match e {
        EventV4::Private {
            event:
                Event::FinisherResolved {
                    mode,
                    raw_damage,
                    hp_damage,
                    absorbed,
                    lethal,
                    ..
                },
        } => Some((*mode, *raw_damage, *hp_damage, *absorbed, *lethal)),
        _ => None,
    })
}
fn mark_condition(c: &mut Config, id: &str, subject: &str) {
    c.combat_rules.as_mut().unwrap().triggers[0]
        .conditions
        .status = Some(
        serde_json::from_value(
            json!({"subject":subject,"definition_id":id,"at_least":1,"consume":1}),
        )
        .unwrap(),
    );
}
#[test]
fn finisher_config_roundtrip_and_strict_rejections() {
    for input in [
        crate::test_data::fixture!("examples/combat-host/finisher-a.json"),
        crate::test_data::fixture!("examples/combat-host/finisher-b.json"),
    ] {
        let p = Package::new();
        p.write(&serde_json::from_str::<Value>(input).unwrap());
        let c = p.load().unwrap();
        p.write(&serde_json::to_value(c.default_config().as_ref()).unwrap());
        assert!(p.load().is_ok());
    }
    for case in 0..24 {
        let p = Package::new();
        let mut v: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/finisher-a.json"
        ))
        .unwrap();
        let t = &mut v["combat_rules"]["triggers"][0];
        match case {
            0 => t["effect"]["rule"]["hp_threshold_bps"] = json!(0),
            1 => t["effect"]["rule"]["hp_threshold_bps"] = json!(10001),
            2 => t["effect"]["rule"]["hp_threshold_bps"] = Value::Null,
            3 => t["effect"]["rule"]["comparison"] = json!("unknown"),
            4 => t["effect"]["rule"]["default"] = Value::Null,
            5 => t["effect"]["rule"]["default"]["mode"] = json!("unknown"),
            6 => t["effect"]["rule"]["default"]["fixed"] = json!(1),
            7 => t["effect"]["rule"]["typo"] = json!(1),
            8 => t["effect"]["rule"]["profile_overrides"] = Value::Null,
            9 => t["effect"]["rule"]["profile_overrides"]["unknown"] = json!({"mode":"execute"}),
            10 => {
                t["effect"]["rule"]["profile_overrides"]["control-resistant"]["fixed"] =
                    json!(1000001)
            }
            11 => {
                t["effect"]["rule"]["profile_overrides"]["control-resistant"]["power_bps"] =
                    json!(100001)
            }
            12 => {
                t["effect"]["rule"]["profile_overrides"]["control-resistant"]["power_bps"] =
                    json!(0)
            }
            13 => {
                t["effect"]["rule"]["profile_overrides"]["control-resistant"]["cap_power_bps"] =
                    json!(0)
            }
            14 => {
                t["effect"]["rule"]["profile_overrides"]["control-resistant"]["cap_power_bps"] =
                    json!(100001)
            }
            15 => t["include_periodic"] = json!(true),
            16 => t["when"] = json!("kill"),
            17 => t["when"] = json!("skill_committed"),
            18 => t["when"] = json!("ally_skill_committed"),
            19 => t["counting"] = json!("action_total"),
            20 => {
                t["effect"]["rule"]
                    .as_object_mut()
                    .unwrap()
                    .remove("default");
            }
            21 => {
                t["effect"]["rule"]["profile_overrides"]["control-resistant"]["mode"] =
                    json!("missing")
            }
            22 => {
                t["effect"]["rule"]["profile_overrides"]["control-resistant"]["typo"] = json!(true)
            }
            _ => v["mechanism_version"] = json!(104),
        };
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn threshold_below_and_at_most_respect_exact_boundaries() {
    for inclusive in [false, true] {
        for hp in [499, 500, 501] {
            let (mut c, mut w) = fixture();
            if inclusive {
                rule(&mut c).comparison = ThresholdComparison::AtMost;
            }
            let j = npc(&w, "capped-target");
            w.npcs[j].hp = hp;
            assert_eq!(
                result(&fire(&mut w, &c, "capped-target", 100)).is_some(),
                hp < 500 || inclusive && hp == 500
            );
        }
    }
}
#[test]
fn full_health_requires_explicit_inclusive_hundred_percent_policy() {
    let (mut c, mut w) = fixture();
    rule(&mut c).hp_threshold_bps = 10000;
    assert!(result(&fire(&mut w, &c, "capped-target", 100)).is_none());
    rule(&mut c).comparison = ThresholdComparison::AtMost;
    assert!(result(&fire(&mut w, &c, "capped-target", 101)).unwrap().4);
}
#[test]
fn execute_budget_covers_current_hp_and_entire_shield_balance() {
    let (mut c, mut w) = fixture();
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
    add(
        &mut c,
        json!({"id":"shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 400;
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "shield");
    assert_eq!(
        result(&fire(&mut w, &c, "capped-target", 100)),
        Some((FinisherMode::Execute, 1400, 400, 1000, true))
    );
}
#[test]
fn expired_shield_is_excluded_from_execute_budget() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"old-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":30,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "old-shield");
    w.npcs[j].hp = 400;
    assert_eq!(
        result(&fire(&mut w, &c, "capped-target", 100)).unwrap().1,
        400
    );
}
#[test]
fn execute_over_u32_budget_is_rejected_before_mark_and_chance() {
    let (mut c, mut w) = fixture();
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
    add(
        &mut c,
        json!({"id":"large-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    Arc::make_mut(&mut w.npcs[j].profile)
        .stats
        .insert("hp".into(), u32::MAX);
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "large-shield");
    w.npcs[j].hp = 1;
    mark_condition(&mut c, "threshold-mark", "other");
    give(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Npc(j),
        "threshold-mark",
    );
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(5000);
    let rng = w.rng;
    assert!(result(&fire(&mut w, &c, "capped-target", 100)).is_none());
    assert_eq!(w.rng, rng);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "threshold-mark")
    );
}
#[test]
fn profile_override_returns_capped_damage_instead_of_execute() {
    let (c, mut w) = fixture();
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    assert_eq!(
        result(&fire(&mut w, &c, "replacement-target", 100)),
        Some((FinisherMode::PowerDamage, 100, 100, 0, false))
    );
    assert_eq!(w.npcs[j].hp, 300);
}
#[test]
fn replacement_fixed_and_ratio_are_bounded_by_source_power_cap() {
    let (mut c, mut w) = fixture();
    rule(&mut c).profile_overrides.insert(
        "control-resistant".into(),
        FinishPolicy::PowerDamage {
            fixed: 1000000,
            power_bps: 100000,
            cap_power_bps: 15000,
        },
    );
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    assert_eq!(
        result(&fire(&mut w, &c, "replacement-target", 100))
            .unwrap()
            .1,
        150
    );
}
#[test]
fn source_power_replacement_rounds_and_saturates_without_overflow() {
    let p = FinishPolicy::PowerDamage {
        fixed: 7,
        power_bps: 3333,
        cap_power_bps: 10000,
    };
    assert_eq!(p.amount(101), Some(40));
    let p = FinishPolicy::PowerDamage {
        fixed: 1000000,
        power_bps: 100000,
        cap_power_bps: 100000,
    };
    assert_eq!(p.amount(u32::MAX), Some(u32::MAX));
}
#[test]
fn original_threshold_and_power_coefficients_use_native_policies() {
    let (mut c, mut w) = fixture();
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_finisher_vectors.json"
    ))
    .unwrap();
    for n in 0..2 {
        rule(&mut c).hp_threshold_bps = v["thresholdsBps"][n].as_u64().unwrap() as u32;
        let j = npc(&w, "replacement-target");
        w.npcs[j].hp = v["eligibleHpInputs"][n].as_u64().unwrap() as u32;
        let bps = v["replacementBps"][n].as_u64().unwrap() as u32;
        rule(&mut c).profile_overrides.insert(
            "control-resistant".into(),
            FinishPolicy::PowerDamage {
                fixed: 0,
                power_bps: bps,
                cap_power_bps: bps,
            },
        );
        assert_eq!(
            json!(
                result(&fire(&mut w, &c, "replacement-target", 100 + n as u64))
                    .unwrap()
                    .1
            ),
            v["replacementDamage"][n]
        );
    }
}
#[test]
fn source_attack_power_bonus_and_profile_points_are_sampled_once() {
    let (mut c, mut w) = fixture();
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    w.members[0].facts.atk_points = 200;
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("member")
        .unwrap()
        .power_points_bps = 5000;
    add(
        &mut c,
        json!({"id":"power","mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","modifier_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    give(&mut w, &c, Owner::Member(0), Owner::Member(0), "power");
    assert_eq!(
        result(&fire(&mut w, &c, "replacement-target", 100))
            .unwrap()
            .1,
        600
    );
}
#[test]
fn source_condition_consumption_is_reflected_in_power_preview() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"attack","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    give(&mut w, &c, Owner::Member(0), Owner::Member(0), "attack");
    mark_condition(&mut c, "attack", "owner");
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    assert_eq!(
        result(&fire(&mut w, &c, "replacement-target", 100))
            .unwrap()
            .1,
        100
    );
}
#[test]
fn final_max_hp_preview_rejects_then_allows_atomic_condition_retry() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"maximum","mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Member(0), Owner::Npc(j), "maximum");
    mark_condition(&mut c, "maximum", "other");
    w.npcs[j].hp = 600;
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(5000);
    let rng = w.rng;
    assert!(result(&fire(&mut w, &c, "capped-target", 100)).is_none());
    assert_eq!(w.rng, rng);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "maximum")
    );
    w.npcs[j].hp = 400;
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(10000);
    assert!(result(&fire(&mut w, &c, "capped-target", 101)).unwrap().4);
}
#[test]
fn consuming_negative_maximum_uses_restored_maximum_without_gifting_hp() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"maximum","mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":-5000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Member(0), Owner::Npc(j), "maximum");
    mark_condition(&mut c, "maximum", "other");
    w.npcs[j].hp = 300;
    assert_eq!(
        result(&fire(&mut w, &c, "capped-target", 100)).unwrap().1,
        300
    );
}
#[test]
fn zero_rounded_replacement_does_not_spend_condition_or_probability() {
    let (mut c, mut w) = fixture();
    rule(&mut c).profile_overrides.insert(
        "control-resistant".into(),
        FinishPolicy::PowerDamage {
            fixed: 0,
            power_bps: 1,
            cap_power_bps: 1,
        },
    );
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    give(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Npc(j),
        "threshold-mark",
    );
    mark_condition(&mut c, "threshold-mark", "other");
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(5000);
    let rng = w.rng;
    assert!(result(&fire(&mut w, &c, "replacement-target", 100)).is_none());
    assert_eq!(w.rng, rng);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "threshold-mark")
    );
}
#[test]
fn target_immunity_refuses_finisher_without_consuming_condition() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"immune","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 400;
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "immune");
    give(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Npc(j),
        "threshold-mark",
    );
    mark_condition(&mut c, "threshold-mark", "other");
    assert!(result(&fire(&mut w, &c, "capped-target", 100)).is_none());
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "threshold-mark")
    );
}
#[test]
fn consuming_immunity_in_the_final_preview_does_not_use_stale_immunity() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"immune","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 400;
    give(&mut w, &c, Owner::Member(0), Owner::Npc(j), "immune");
    mark_condition(&mut c, "immune", "other");
    assert!(result(&fire(&mut w, &c, "capped-target", 100)).unwrap().4);
}
#[test]
fn native_death_guard_can_survive_an_execute_attempt() {
    let (c, mut w) = fixture();
    let j = npc(&w, "guard-target");
    w.npcs[j].hp = 400;
    assert_eq!(
        result(&fire(&mut w, &c, "guard-target", 100)),
        Some((FinisherMode::Execute, 400, 399, 0, false))
    );
    assert_eq!(w.npcs[j].hp, 1);
    assert!(result(&fire(&mut w, &c, "guard-target", 101)).unwrap().4);
}
#[test]
fn protected_returning_dead_or_friendly_targets_refuse_before_effect() {
    for case in 0..3 {
        let (c, mut w) = fixture();
        let j = npc(&w, "capped-target");
        w.npcs[j].hp = 400;
        match case {
            0 => w.npcs[j].returning = true,
            1 => w.npcs[j].hp = 0,
            _ => w.members[0].protected = true,
        };
        assert!(result(&fire(&mut w, &c, "capped-target", 100)).is_none());
    }
    let (c, w) = fixture();

    let r = c.combat_rules.as_ref().unwrap().triggers[0].effect.clone();
    let TriggerEffect::FinishOther { rule } = r else {
        panic!()
    };
    assert!(
        w.plan_finish_other(
            &w.statuses,
            (Owner::Member(0), Owner::Member(1)),
            &rule,
            &c,
            100,
            None
        )
        .is_err()
    );
}
#[test]
fn dead_target_cannot_be_finished_twice() {
    let (c, mut w) = fixture();
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 400;
    assert!(result(&fire(&mut w, &c, "capped-target", 100)).is_some());
    assert!(result(&fire(&mut w, &c, "capped-target", 100)).is_none());
}
#[test]
fn per_action_counter_prevents_double_finish_on_one_target() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers[0].counting = crate::config::TriggerCounting::Action;
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    w.triggers.action = Some("same".into());
    assert!(result(&fire(&mut w, &c, "replacement-target", 100)).is_some());
    assert!(result(&fire(&mut w, &c, "replacement-target", 100)).is_none());
    w.triggers.action = Some("next".into());
    assert!(result(&fire(&mut w, &c, "replacement-target", 100)).is_some());
}
#[test]
fn shared_step_trigger_budget_blocks_other_target_until_next_real_fact() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().max_triggers_per_step = 1;
    for id in ["replacement-target", "guard-target"] {
        let j = npc(&w, id);
        w.npcs[j].hp = 400;
    }
    assert!(result(&fire(&mut w, &c, "replacement-target", 100)).is_some());
    assert!(result(&fire(&mut w, &c, "guard-target", 100)).is_none());
    assert!(result(&fire(&mut w, &c, "guard-target", 101)).is_some());
}
#[test]
fn npc_counter_finisher_publishes_only_to_the_affected_member() {
    let (mut c, mut w) = fixture();
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.when = CombatFact::DamageTaken;
    t.profiles = vec!["ordinary-control".into()];
    w.members[0].hp = 400;
    let j = npc(&w, "capped-target");
    let mut e = vec![];
    w.combat_fact(
        CombatFact::DamageTaken,
        (Owner::Npc(j), Owner::Member(0)),
        (1, false),
        &c,
        100,
        &mut e,
    );
    assert!(result(&e).unwrap().4);
    let recipients = e
        .iter()
        .filter_map(|(i, e)| {
            if matches!(
                e,
                EventV4::Private {
                    event: Event::FinisherResolved { .. }
                }
            ) {
                Some(*i)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(recipients, vec![Some(0)]);
    assert_eq!(w.members[1].hp, 1000);
}
#[test]
fn finisher_does_not_recurse_or_grant_derived_damage_rewards() {
    let (mut c, mut w) = fixture();
    let t=serde_json::from_value(json!({"id":"derived-reward","profiles":["member"],"when":"damage_dealt","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"restore_spirit","amount":99}})).unwrap();
    c.combat_rules.as_mut().unwrap().triggers.push(t);
    w.members[0].spirit = 0;
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    let before = w.rng;
    assert!(result(&fire(&mut w, &c, "replacement-target", 100)).is_some());
    assert_eq!(w.members[0].spirit, 0);
    assert_eq!(w.rng, before);
}
#[test]
fn probability_failure_keeps_condition_and_allows_independent_retry() {
    let (mut c, mut w) = fixture();
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    give(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Npc(j),
        "threshold-mark",
    );
    mark_condition(&mut c, "threshold-mark", "other");
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(0);
    assert!(result(&fire(&mut w, &c, "replacement-target", 100)).is_none());
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "threshold-mark")
    );
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(10000);
    assert!(result(&fire(&mut w, &c, "replacement-target", 101)).is_some());
}
#[test]
fn replacement_can_lethally_damage_a_target_below_the_cap() {
    let (c, mut w) = fixture();
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 50;
    assert!(
        result(&fire(&mut w, &c, "replacement-target", 100))
            .unwrap()
            .4
    );
}

#[test]
fn wide_threshold_comparison_preserves_sub_unit_fraction() {
    for hp in [2147483647, 2147483648] {
        let (c, mut w) = fixture();
        let j = npc(&w, "capped-target");
        Arc::make_mut(&mut w.npcs[j].profile)
            .stats
            .insert("hp".into(), u32::MAX);
        w.npcs[j].hp = hp;
        assert_eq!(
            result(&fire(&mut w, &c, "capped-target", 100)).is_some(),
            hp == 2147483647
        );
    }
}
#[test]
fn failed_threshold_preserves_primary_damage_and_newly_applied_mark() {
    let (mut c, mut w) = fixture();
    mark_condition(&mut c, "threshold-mark", "other");
    let j = npc(&w, "capped-target");
    let source = w.owner_entity(Owner::Member(0));
    let mut e = vec![];
    w.direct_contact(
        Owner::Member(0),
        Owner::Npc(j),
        DirectContact {
            origin: None,
            action_id: Some("small"),
            dead_source: false,
            source: &source,
            raw: 100,
            rule: Some(&AttackRule {
                damage_kind: crate::config::DamageKind::TrueDamage,
                can_crit: false,
                ..Default::default()
            }),
            ignore_armor: false,
            unavoidable: true,
            metal: true,
            hit_statuses: &["threshold-mark".into()],
        },
        &c,
        100,
        &mut e,
    );
    assert!(result(&e).is_none());
    assert_eq!(w.npcs[j].hp, 900);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "threshold-mark" && s.stacks == Some(1))
    );
}
#[test]
fn finisher_death_credits_only_the_native_source_once() {
    let (c, mut w) = fixture();
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 400;
    let e = fire(&mut w, &c, "capped-target", 100);
    assert!(e.iter().any(
        |(_, e)| matches!(e,EventV4::MonsterDefeated{credited_to,..} if credited_to.id=="P00")
    ));
    assert_eq!(w.members[0].tally.get("capped-target"), Some(&1));
    assert!(w.members[1].tally.is_empty());
    fire(&mut w, &c, "capped-target", 101);
    assert_eq!(w.members[0].tally.get("capped-target"), Some(&1));
}
#[test]
fn a_consumed_shield_condition_changes_the_final_execute_pool() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Member(0), Owner::Npc(j), "shield");
    mark_condition(&mut c, "shield", "other");
    w.npcs[j].hp = 400;
    assert_eq!(
        result(&fire(&mut w, &c, "capped-target", 100)),
        Some((FinisherMode::Execute, 400, 400, 0, true))
    );
}
