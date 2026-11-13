use super::*;
use crate::config::{CombatFact, CombatTrigger, TriggerCounting, TriggerEffect};
fn document() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/followup-a.json"
    ))
    .unwrap()
}
fn fixture() -> (Config, World) {
    let p = Package::new();
    p.write(&document());
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
            spirit_points: 200,
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
    for m in &mut w.members {
        m.spirit = 50;
    }
    w.members[0].cooldowns.insert("threshold-bolt".into(), 1100);
    (c, w)
}
fn npc(w: &World, id: &str) -> usize {
    w.npcs.iter().position(|n| n.entity.id == id).unwrap()
}
fn trigger<'a>(c: &'a mut Config, id: &str) -> &'a mut CombatTrigger {
    c.combat_rules
        .as_mut()
        .unwrap()
        .triggers
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
}
fn only(c: &mut Config, id: &str) {
    c.combat_rules
        .as_mut()
        .unwrap()
        .triggers
        .retain(|t| t.id == "threshold-finish" || t.id == id);
}
fn fire(w: &mut World, c: &Config, id: &str, source: Owner, now: u64) -> Events {
    let mut e = vec![];
    w.combat_fact(
        CombatFact::DirectHit,
        (source, Owner::Npc(npc(w, id))),
        (1, false),
        c,
        now,
        &mut e,
    );
    e
}
fn kill(w: &mut World, c: &Config, now: u64) -> Events {
    let j = npc(w, "capped-target");
    w.npcs[j].hp = 400;
    fire(w, c, "capped-target", Owner::Member(0), now)
}
fn fired(e: &Events) -> Vec<String> {
    e.iter()
        .filter_map(|(_, e)| {
            if let EventV4::Private {
                event: Event::CombatTriggerFired { trigger_id, .. },
            } = e
            {
                Some(trigger_id.clone())
            } else {
                None
            }
        })
        .collect()
}
fn add(c: &mut Config, v: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(v).unwrap());
}
fn give(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str) {
    let e = w.owner_entity(target);
    w.apply_owned_statuses(source, (e, 1000), &[id.into()], c, 0, &mut vec![]);
}
fn simple_reward(c: &mut Config, amount: u32, bps: u32) {
    only(c, "finish-spirit");
    trigger(c, "finish-spirit").effect = TriggerEffect::RestoreSpirit {
        amount,
        max_spirit_bps: bps,
    };
}
#[test]
fn result_fact_samples_roundtrip_and_reject_invalid_policies() {
    for input in [
        crate::test_data::fixture!("examples/combat-host/followup-a.json"),
        crate::test_data::fixture!("examples/combat-host/followup-b.json"),
    ] {
        let p = Package::new();
        p.write(&serde_json::from_str::<Value>(input).unwrap());
        let c = p.load().unwrap();
        p.write(&serde_json::to_value(c.default_config().as_ref()).unwrap());
        assert!(p.load().is_ok());
    }
    for n in 0..35 {
        let p = Package::new();
        let mut v = document();
        let t = &mut v["combat_rules"]["triggers"][2];
        match n {
            0 => {
                t.as_object_mut().unwrap().remove("finisher");
            }
            1 => t["finisher"] = Value::Null,
            2 => t["finisher"]["trigger_id"] = json!("missing"),
            3 => t["finisher"]["trigger_id"] = json!("finish-state"),
            4 => t["finisher"]["mode"] = json!("unknown"),
            5 => t["finisher"]["mode"] = Value::Null,
            6 => {
                t["finisher"].as_object_mut().unwrap().remove("mode");
            }
            7 => t["finisher"]["typo"] = json!(1),
            8 => t["profiles"] = json!(["npc"]),
            9 => t["include_periodic"] = json!(true),
            10 => t["counting"] = json!("action_total"),
            11 => t["counter_scope"] = json!("target"),
            12 => t["conditions"] = json!({"other_hp_at_most_bps":10000}),
            13 => {
                t["conditions"] = json!({"status":{"subject":"other","definition_id":"threshold-mark","at_least":1,"consume":1}})
            }
            14 => {
                t["effect"] =
                    json!({"kind":"reflect_damage","bps":10000,"damage_kind":"true_damage"})
            }
            15 => t["effect"] = json!({"kind":"apply_other_status","status_id":"threshold-mark"}),
            16 => {
                t["effect"] = json!({"kind":"dispel_statuses","subject":"owner","disposition":"harmful","max_count":1})
            }
            17 => {
                t["effect"] = json!({"kind":"transfer_statuses","from":"other","disposition":"harmful","max_count":1})
            }
            18 => {
                t["effect"] = json!({"kind":"finish_other","rule":{"hp_threshold_bps":5000,"comparison":"below","default":{"mode":"execute"}}})
            }
            19 => t["when"] = json!("kill"),
            20 => t["when"] = json!("direct_hit"),
            21 => t["effect"]["max_spirit_bps"] = json!(10001),
            22 => t["effect"]["max_spirit_bps"] = Value::Null,
            23 => t["effect"]["max_spirit_bps"] = json!(-1),
            24 => t["effect"]["max_spirit_bps"] = json!(1.5),
            25 => t["effect"]["max_spirit_bps"] = json!(0),
            26 => t["effect"]["amount"] = json!(1000001),
            27 => {
                t["effect"].as_object_mut().unwrap().remove("amount");
            }
            28 => t["effect"]["typo"] = json!(1),
            29 => t["finisher"]["trigger_id"] = Value::Null,
            30 => {
                v["combat_rules"]["triggers"][0]["finisher"] =
                    json!({"trigger_id":"threshold-finish","mode":"any"})
            }
            31 => v["combat_rules"]["triggers"][0]["profiles"] = json!(["npc"]),
            32 => v["mechanism_version"] = json!(104),
            33 => t["contact_consumption"] = json!({"definition_id":"threshold-mark","at_least":1}),
            _ => t["finisher"]["mode"] = json!({"execute":null}),
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {n}");
    }
}
#[test]
fn old_fixed_spirit_output_omits_new_optional_fields() {
    let v = json!({"kind":"restore_spirit","amount":7});
    let e: TriggerEffect = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(e).unwrap(), v);
    let p = Package::new();
    let mut v = document();
    v["combat_rules"]["triggers"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    p.write(&v);
    let c = p.load().unwrap();
    let out = serde_json::to_value(&c.default_config().combat_rules.as_ref().unwrap().triggers[0])
        .unwrap();
    assert!(out.get("finisher").is_none());
}
#[test]
fn true_execute_kill_runs_four_leaves_in_declared_order() {
    let (c, mut w) = fixture();
    let e = kill(&mut w, &c, 100);
    assert_eq!(
        fired(&e),
        [
            "threshold-finish",
            "finish-state",
            "finish-spirit",
            "finish-refund",
            "finish-chain"
        ]
    );
    assert_eq!(w.members[0].spirit, 70);
    assert_eq!(w.member_max_spirit(0, &c, 100), 250);
    assert_eq!(w.members[0].cooldowns["threshold-bolt"], 800);
    assert_eq!(w.npcs[npc(&w, "follow-target")].hp, 680);
    assert!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::CombatTriggerFired { .. }
                        | Event::SkillCooldownsReduced { .. }
                        | Event::ChainDamageResolved { .. }
                        | Event::FinisherResolved { .. }
                }
            ))
            .all(|(i, _)| *i == Some(0))
    );
}
#[test]
fn death_guard_prevents_every_success_leaf() {
    let (c, mut w) = fixture();
    let j = npc(&w, "guard-target");
    w.npcs[j].hp = 400;
    let e = fire(&mut w, &c, "guard-target", Owner::Member(0), 100);
    assert_eq!(w.npcs[j].hp, 1);
    assert_eq!(fired(&e), ["threshold-finish"]);
    assert_eq!(w.members[0].spirit, 50);
}
#[test]
fn nonlethal_replacement_never_emits_success() {
    let (c, mut w) = fixture();
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 400;
    let e = fire(&mut w, &c, "replacement-target", Owner::Member(0), 100);
    assert_eq!(w.npcs[j].hp, 300);
    assert_eq!(fired(&e), ["threshold-finish"]);
}
#[test]
fn capped_damage_kill_matches_only_explicit_mode_or_any() {
    for mode in ["execute", "power_damage", "any"] {
        let (mut c, mut w) = fixture();
        simple_reward(&mut c, 9, 0);
        trigger(&mut c, "finish-spirit").finisher = Some(
            serde_json::from_value(json!({"trigger_id":"threshold-finish","mode":mode})).unwrap(),
        );
        let j = npc(&w, "replacement-target");
        w.npcs[j].hp = 50;
        let e = fire(&mut w, &c, "replacement-target", Owner::Member(0), 100);
        assert_eq!(w.npcs[j].hp, 0);
        assert_eq!(w.members[0].spirit, if mode == "execute" { 50 } else { 59 });
        assert_eq!(fired(&e).len(), if mode == "execute" { 1 } else { 2 });
    }
}
#[test]
fn ordinary_main_hit_kill_does_not_forge_finisher_result() {
    let (c, mut w) = fixture();
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 10;
    let source = w.owner_entity(Owner::Member(0));
    let mut e = vec![];
    w.direct_contact(
        Owner::Member(0),
        Owner::Npc(j),
        DirectContact {
            origin: None,
            action_id: Some("ordinary"),
            rule: None,
            ignore_armor: true,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw: 100,
            metal: false,
            hit_statuses: &[],
        },
        &c,
        100,
        &mut e,
    );
    assert_eq!(w.npcs[j].hp, 0);
    assert!(fired(&e).is_empty());
    assert_eq!(w.members[0].spirit, 52); // Ordinary basic-contact recovery remains authoritative.
}
#[test]
fn generic_fact_entry_cannot_forge_success() {
    let (c, mut w) = fixture();
    let mut e = vec![];
    w.combat_fact(
        CombatFact::FinisherKill,
        (Owner::Member(0), Owner::Npc(npc(&w, "capped-target"))),
        (400, false),
        &c,
        100,
        &mut e,
    );
    assert!(e.is_empty());
    assert_eq!(w.members[0].spirit, 50);
}
#[test]
fn primary_rules_complete_before_result_stage_and_leaf_priority_is_stable() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 9, 0);
    let mut root = trigger(&mut c, "finish-spirit").clone();
    root.id = "late-primary".into();
    root.when = CombatFact::DirectHit;
    root.finisher = None;
    root.priority = 20;
    root.effect = TriggerEffect::RestoreSpirit {
        amount: 11,
        max_spirit_bps: 0,
    };
    c.combat_rules.as_mut().unwrap().triggers.push(root);
    let mut second = trigger(&mut c, "finish-spirit").clone();
    second.id = "a-tie".into();
    second.priority = 1;
    c.combat_rules.as_mut().unwrap().triggers.push(second);
    let e = kill(&mut w, &c, 100);
    assert_eq!(
        fired(&e),
        ["threshold-finish", "late-primary", "a-tie", "finish-spirit"]
    );
    assert_eq!(w.members[0].spirit, 79);
}
#[test]
fn hp_amount_excludes_absorbed_shield_for_heal_and_chain() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"shield","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "shield");
    let mut heal = trigger(&mut c, "finish-spirit").clone();
    heal.id = "finish-heal".into();
    heal.effect = TriggerEffect::HealFromDamage { bps: 5000 };
    c.combat_rules.as_mut().unwrap().triggers.push(heal);
    w.members[0].hp = 500;
    let e = kill(&mut w, &c, 100);
    assert_eq!(w.members[0].hp, 700);
    assert_eq!(w.npcs[npc(&w, "follow-target")].hp, 680);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::FinisherResolved {
                raw_damage: 500,
                hp_damage: 400,
                absorbed: 100,
                lethal: true,
                ..
            }
        }
    )));
}
#[test]
fn original_action_identity_deduplicates_different_targets_and_keeps_creator() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 9, 0);
    trigger(&mut c, "finish-spirit").counting = TriggerCounting::Action;
    // The action's creator may differ from the finisher owner (a victim-side fact).
    w.triggers.action = Some("shared".into());
    w.triggers.action_source = Some(w.owner_entity(Owner::Member(1)));
    kill(&mut w, &c, 100);
    let j = npc(&w, "follow-target");
    w.npcs[j].hp = 400;
    fire(&mut w, &c, "follow-target", Owner::Member(0), 101);
    assert_eq!(w.members[0].spirit, 59);
    w.triggers.action_source = Some(w.owner_entity(Owner::Member(0)));
    let j = npc(&w, "replacement-target");
    w.npcs[j].hp = 50;
    trigger(&mut c, "finish-spirit").finisher = Some(
        serde_json::from_value(json!({"trigger_id":"threshold-finish","mode":"any"})).unwrap(),
    );
    fire(&mut w, &c, "replacement-target", Owner::Member(0), 102);
    assert_eq!(w.members[0].spirit, 68);
}
#[test]
fn step_budget_is_shared_and_never_replayed_after_capacity() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().max_triggers_per_step = 2;
    let e = kill(&mut w, &c, 100);
    assert_eq!(fired(&e), ["threshold-finish", "finish-state"]);
    assert_eq!(w.members[0].spirit, 50);
    let e = fire(&mut w, &c, "capped-target", Owner::Member(0), 101);
    assert!(fired(&e).is_empty());
    assert_eq!(w.members[0].spirit, 50);
}
#[test]
fn chain_budget_failure_does_not_undo_other_leaves() {
    let (c, mut w) = fixture();
    w.trigger_step(100);
    w.triggers.chain_used = c.combat_rules.as_ref().unwrap().max_chain_hits_per_step;
    let e = kill(&mut w, &c, 100);
    assert_eq!(
        fired(&e),
        [
            "threshold-finish",
            "finish-state",
            "finish-spirit",
            "finish-refund"
        ]
    );
    assert_eq!(w.members[0].spirit, 70);
    assert_eq!(w.npcs[npc(&w, "follow-target")].hp, 1000);
}
#[test]
fn failed_cooldown_leaf_does_not_rollback_kill_or_rewards() {
    let (c, mut w) = fixture();
    w.members[0].cooldowns.clear();
    let e = kill(&mut w, &c, 100);
    assert_eq!(
        fired(&e),
        [
            "threshold-finish",
            "finish-state",
            "finish-spirit",
            "finish-chain"
        ]
    );
    assert_eq!(w.members[0].spirit, 70);
    assert_eq!(w.npcs[npc(&w, "capped-target")].hp, 0);
}
#[test]
fn derived_chain_kill_does_not_recurse_into_kill_rewards_or_finisher() {
    let (mut c, mut w) = fixture();
    let j = npc(&w, "follow-target");
    w.npcs[j].hp = 100;
    let mut reward = trigger(&mut c, "finish-spirit").clone();
    reward.id = "ordinary-kill".into();
    reward.when = CombatFact::Kill;
    reward.finisher = None;
    reward.effect = TriggerEffect::RestoreSpirit {
        amount: 100,
        max_spirit_bps: 0,
    };
    c.combat_rules.as_mut().unwrap().triggers.push(reward);
    let e = kill(&mut w, &c, 100);
    assert_eq!(w.npcs[j].hp, 0);
    assert!(!fired(&e).contains(&"ordinary-kill".into()));
    assert_eq!(fired(&e).len(), 5);
    assert_eq!(w.members[0].spirit, 70);
}
#[test]
fn post_condition_and_internal_cooldown_are_owned_by_source() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 9, 0);
    trigger(&mut c, "finish-spirit").cooldown_ms = 500;
    trigger(&mut c, "finish-spirit").conditions.status = Some(
        serde_json::from_value(
            json!({"subject":"owner","definition_id":"threshold-mark","at_least":1,"consume":1}),
        )
        .unwrap(),
    );
    give(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Member(0),
        "threshold-mark",
    );
    let e = kill(&mut w, &c, 100);
    assert_eq!(w.members[0].spirit, 59);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusConsumed { .. }
        }
    )));
    give(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Member(0),
        "threshold-mark",
    );
    let j = npc(&w, "follow-target");
    w.npcs[j].hp = 400;
    fire(&mut w, &c, "follow-target", Owner::Member(0), 200);
    assert_eq!(w.members[0].spirit, 59);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "threshold-mark")
    );
}
#[test]
fn owner_counters_every_and_chance_are_independent_of_provider() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 9, 0);
    trigger(&mut c, "finish-spirit").every = 2;
    let e = kill(&mut w, &c, 100);
    assert_eq!(fired(&e), ["threshold-finish"]);
    let j = npc(&w, "follow-target");
    w.npcs[j].hp = 400;
    fire(&mut w, &c, "follow-target", Owner::Member(0), 101);
    assert_eq!(w.members[0].spirit, 59);
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 9, 0);
    trigger(&mut c, "finish-spirit").chance_bps = Some(0);
    let e = kill(&mut w, &c, 100);
    assert_eq!(fired(&e), ["threshold-finish"]);
    assert_eq!(w.members[0].spirit, 50);
    assert_eq!(w.npcs[npc(&w, "capped-target")].hp, 0);
}
#[test]
fn different_owners_and_provider_selectors_do_not_share_rewards() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 9, 0);
    let mut other = trigger(&mut c, "threshold-finish").clone();
    other.id = "other-finish".into();
    other.priority = 1;
    c.combat_rules.as_mut().unwrap().triggers.push(other);
    trigger(&mut c, "finish-spirit").finisher =
        Some(serde_json::from_value(json!({"trigger_id":"other-finish","mode":"any"})).unwrap());
    let e = kill(&mut w, &c, 100);
    assert_eq!(fired(&e), ["threshold-finish"]);
    assert_eq!(w.members[0].spirit, 50);
    trigger(&mut c, "finish-spirit").finisher = Some(
        serde_json::from_value(json!({"trigger_id":"threshold-finish","mode":"execute"})).unwrap(),
    );
    let j = npc(&w, "follow-target");
    w.npcs[j].hp = 400;
    fire(&mut w, &c, "follow-target", Owner::Member(1), 101);
    assert_eq!(w.members[0].spirit, 50);
    assert_eq!(w.members[1].spirit, 59);
}
#[test]
fn ratio_plus_fixed_rounds_once_and_clamps_to_gap() {
    for (initial, fixed, bps, expected) in [
        (50, 7, 3333, 123),
        (190, 7, 3333, 200),
        (200, 7, 3333, 200),
        (50, 0, 1, 50),
    ] {
        let (mut c, mut w) = fixture();
        simple_reward(&mut c, fixed, bps);
        w.members[0].spirit = initial;
        kill(&mut w, &c, 100);
        assert_eq!(w.members[0].spirit, expected);
    }
}
#[test]
fn ratio_wide_arithmetic_saturates_at_u32_and_never_wraps() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 1000000, 10000);
    w.members[0].facts.spirit_points = u32::MAX;
    w.members[0].spirit = 0;
    kill(&mut w, &c, 100);
    assert_eq!(w.members[0].spirit, u32::MAX);
}
#[test]
fn ratio_reads_prior_status_leaf_and_current_maximum_at_execution() {
    let (mut c, mut w) = fixture();
    only(&mut c, "finish-spirit");
    give(&mut w, &c, Owner::Member(0), Owner::Member(0), "afterglow");
    kill(&mut w, &c, 100);
    assert_eq!(w.members[0].spirit, 70);
    let (mut c, mut w) = fixture();
    trigger(&mut c, "finish-state").priority = 9;
    kill(&mut w, &c, 100);
    assert_eq!(w.members[0].spirit, 66);
    assert_eq!(w.member_max_spirit(0, &c, 100), 250);
}
#[test]
fn expired_maximum_bonus_is_not_included_in_ratio() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 0, 800);
    give(&mut w, &c, Owner::Member(0), Owner::Member(0), "afterglow");
    kill(&mut w, &c, 3000);
    assert_eq!(w.members[0].spirit, 66);
}
#[test]
fn locked_or_full_recovery_still_spends_only_its_own_condition() {
    for locked in [false, true] {
        let (mut c, mut w) = fixture();
        simple_reward(&mut c, 0, 800);
        trigger(&mut c,"finish-spirit").conditions.status=Some(serde_json::from_value(json!({"subject":"owner","definition_id":"threshold-mark","at_least":1,"consume":1})).unwrap());
        give(
            &mut w,
            &c,
            Owner::Member(0),
            Owner::Member(0),
            "threshold-mark",
        );
        if locked {
            add(
                &mut c,
                json!({"id":"lock","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock","modifier_bps":-10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
            );
            give(&mut w, &c, Owner::Member(0), Owner::Member(0), "lock");
        } else {
            w.members[0].spirit = 200;
        }
        let before = w.members[0].spirit;
        let e = kill(&mut w, &c, 100);
        assert_eq!(w.members[0].spirit, before);
        assert!(fired(&e).contains(&"finish-spirit".into()));
        assert!(
            !w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "threshold-mark")
        );
    }
}
#[test]
fn ratio_on_ordinary_fact_and_fixed_legacy_path_still_work() {
    for bps in [0, 800] {
        let (mut c, mut w) = fixture();
        simple_reward(&mut c, 7, bps);
        c.combat_rules.as_mut().unwrap().triggers.remove(0);
        let t = trigger(&mut c, "finish-spirit");
        t.when = CombatFact::Kill;
        t.finisher = None;
        let mut e = vec![];
        w.combat_fact(
            CombatFact::Kill,
            (Owner::Member(0), Owner::Npc(npc(&w, "capped-target"))),
            (400, false),
            &c,
            100,
            &mut e,
        );
        assert_eq!(w.members[0].spirit, 57 + if bps == 0 { 0 } else { 16 });
    }
}
#[test]
fn npc_source_can_receive_ratio_and_uses_its_own_maximum() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 7, 800);
    let t = trigger(&mut c, "finish-spirit");
    t.when = CombatFact::Kill;
    t.finisher = None;
    t.profiles = vec!["ordinary-control".into()];
    let j = npc(&w, "capped-target");
    w.npcs[j].spirit = 0;
    Arc::make_mut(&mut w.npcs[j].profile)
        .stats
        .insert("spirit".into(), 250);
    let mut e = vec![];
    w.combat_fact(
        CombatFact::Kill,
        (Owner::Npc(j), Owner::Member(0)),
        (400, false),
        &c,
        100,
        &mut e,
    );
    assert_eq!(w.npcs[j].spirit, 27);
}
#[test]
fn source_condition_consumption_updates_maximum_before_recovery() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 0, 800);
    give(&mut w, &c, Owner::Member(0), Owner::Member(0), "afterglow");
    trigger(&mut c, "finish-spirit").conditions.status = Some(
        serde_json::from_value(
            json!({"subject":"owner","definition_id":"afterglow","at_least":1,"consume":1}),
        )
        .unwrap(),
    );
    kill(&mut w, &c, 100);
    assert_eq!(w.members[0].spirit, 66);
    assert_eq!(w.member_max_spirit(0, &c, 100), 200);
}
fn captured(w: &World) -> super::super::finishers::FinishKillFact {
    super::super::finishers::FinishKillFact {
        origin: w.triggers.origin.clone(),
        finisher_id: "threshold-finish".into(),
        source: w.owner_entity(Owner::Member(0)),
        target: w.owner_entity(Owner::Npc(npc(w, "capped-target"))),
        mode: FinisherMode::Execute,
        hp_damage: 400,
        enhanced: false,
        action: None,
    }
}
#[test]
fn captured_invalid_source_or_replaced_target_generation_is_dropped() {
    for n in 0..4 {
        let (c, mut w) = fixture();
        let fact = captured(&w);
        match n {
            0 => w.members[0].hp = 0,
            1 => w.members[0].desired = Some(Outcome::Death),
            2 => w.members[0].reference.entity.generation += 1,
            _ => {
                let j = npc(&w, "capped-target");
                w.npcs[j].entity.generation += 1;
            }
        }
        let mut e = vec![];
        w.dispatch_finisher_kills(vec![fact], &c, 100, &mut e);
        assert!(fired(&e).is_empty());
        assert_eq!(w.members[0].spirit, 50);
    }
}
#[test]
fn dead_same_generation_origin_is_allowed_and_results_follow_generation_order() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 9, 0);
    trigger(&mut c, "finish-spirit").every = 2;
    let a = captured(&w);
    let mut b = captured(&w);
    b.target = w.owner_entity(Owner::Npc(npc(&w, "follow-target")));
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 0;
    let j = npc(&w, "follow-target");
    w.npcs[j].hp = 0;
    let mut e = vec![];
    w.trigger_step(100);
    w.dispatch_finisher_kills(vec![a, b], &c, 100, &mut e);
    assert_eq!(fired(&e), ["finish-spirit"]);
    assert_eq!(w.members[0].spirit, 59);
}
#[test]
fn runtime_leaf_gate_prevents_unvalidated_recursive_finisher() {
    let (mut c, mut w) = fixture();
    let effect = trigger(&mut c, "threshold-finish").effect.clone();
    trigger(&mut c, "finish-spirit").effect = effect;
    let e = kill(&mut w, &c, 100);
    assert!(!fired(&e).contains(&"finish-spirit".into()));
    assert_eq!(fired(&e).len(), 4);
}
#[test]
fn status_failure_is_independent_and_post_state_naturally_expires_once() {
    let (mut c, mut w) = fixture();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 0;
    let e = kill(&mut w, &c, 100);
    assert!(!fired(&e).contains(&"finish-state".into()));
    assert_eq!(w.members[0].spirit, 66);
    let (c, mut w) = fixture();
    kill(&mut w, &c, 100);
    let state = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "afterglow")
        .unwrap()
        .clone();
    let mut e = vec![];
    w.prepare_periodic(state.expires_at_ms, &mut e);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.instance_id == state.instance_id)
    );
    let n = e.len();
    w.prepare_periodic(state.expires_at_ms + 30, &mut e);
    assert_eq!(e.len(), n);
    assert_eq!(w.member_max_spirit(0, &c, state.expires_at_ms), 200);
    assert_eq!(w.members[0].spirit, 70);
}
#[test]
fn original_followup_coefficients_execute_native_leaves_only() {
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_finisher_followup_vectors.json"
    ))
    .unwrap();
    let (c, mut w) = fixture();
    let e = kill(&mut w, &c, 100);
    assert_eq!(json!(w.members[0].spirit - 50), v["spiritRestored"]);
    assert_eq!(
        json!(1100 - w.members[0].cooldowns["threshold-bolt"]),
        v["cooldownReducedMs"]
    );
    let TriggerEffect::ChainDamage { rule } =
        trigger(&mut c.clone(), "finish-chain").effect.clone()
    else {
        panic!("chain")
    };
    assert_eq!(json!(rule.formula.fact_amount_bps), v["chainBps"][0]);
    assert!(fired(&e).contains(&"finish-chain".into()));
    for n in 0..2 {
        let (mut c, mut w) = fixture();
        only(&mut c, "finish-chain");
        if let TriggerEffect::ChainDamage { rule } = &mut trigger(&mut c, "finish-chain").effect {
            rule.formula.source_attack_bps = v["chainBps"][n].as_u64().unwrap() as u32;
            rule.formula.fact_amount_bps = 0;
        }
        kill(&mut w, &c, 100);
        assert_eq!(
            json!(1000 - w.npcs[npc(&w, "follow-target")].hp),
            v["chainDamage"][n]
        );
    }
}
#[test]
fn npc_finisher_kill_preserves_owner_and_only_rewards_the_killer() {
    let (mut c, mut w) = fixture();
    simple_reward(&mut c, 7, 800);
    for t in &mut c.combat_rules.as_mut().unwrap().triggers {
        t.profiles = vec!["ordinary-control".into()];
    }
    let j = npc(&w, "capped-target");
    w.npcs[j].spirit = 0;
    Arc::make_mut(&mut w.npcs[j].profile)
        .stats
        .insert("spirit".into(), 250);
    w.members[1].hp = 400;
    let mut e = vec![];
    w.combat_fact(
        CombatFact::DirectHit,
        (Owner::Npc(j), Owner::Member(1)),
        (1, false),
        &c,
        100,
        &mut e,
    );
    assert_eq!(w.members[1].hp, 0);
    assert_eq!(w.npcs[j].spirit, 27);
    assert_eq!(w.members[0].spirit, 50);
    assert!(e.iter().any(|(i, e)| *i == Some(1)
        && matches!(
            e,
            EventV4::Private {
                event: Event::FinisherResolved { lethal: true, .. }
            }
        )));
}
#[test]
fn source_exit_cleans_reward_status_and_captured_terminal_fact_is_discarded() {
    let (c, mut w) = fixture();
    kill(&mut w, &c, 100);
    let fact = captured(&w);
    let mut e = vec![];
    w.terminal_intent(0, Outcome::Death, &mut e);
    w.prepare_periodic(101, &mut e);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "afterglow")
    );
    let before = w.members[0].spirit;
    let n = e.len();
    w.dispatch_finisher_kills(vec![fact], &c, 101, &mut e);
    assert_eq!(w.members[0].spirit, before);
    assert_eq!(e.len(), n);
}
