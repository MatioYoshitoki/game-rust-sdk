use super::*;
use crate::config::{CombatFact, TriggerEffect};
fn fixture() -> (Config, World) {
    let p = Package::new();
    p.write(
        &serde_json::from_str::<Value>(crate::test_data::fixture!(
            "examples/combat-host/post-consumption-a.json"
        ))
        .unwrap(),
    );
    let c = p.load().unwrap().default_config().as_ref().clone();
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
    w.members[0].position = [500., 300.];
    w.members[0].cooldowns.insert("break-bolt".into(), 3000);
    (c, w)
}
fn npc(w: &World, id: &str) -> usize {
    w.npcs.iter().position(|n| n.entity.id == id).unwrap()
}
fn grant(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str) {
    let e = w.owner_entity(target);
    w.apply_owned_statuses(source, (e, 10000), &[id.into()], c, 0, &mut vec![]);
}
fn mark(w: &mut World, c: &Config, n: u32, source: usize) {
    let target = Owner::Npc(npc(w, "capped-target"));
    for _ in 0..n {
        grant(w, c, Owner::Member(source), target, "threshold-mark");
    }
}
fn hit(w: &mut World, c: &Config, now: u64, raw: u32) -> Events {
    let mut events = vec![];
    let action = w.triggers.action.clone();
    let source = w.owner_entity(Owner::Member(0));
    w.direct_contact(
        Owner::Member(0),
        Owner::Npc(npc(w, "capped-target")),
        combat::DirectContact {
            origin: None,
            action_id: action.as_deref(),
            rule: Some(&c.combat_rules.as_ref().unwrap().skills["break-bolt"]),
            ignore_armor: false,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw,
            metal: true,
            hit_statuses: &[],
        },
        c,
        now,
        &mut events,
    );
    events
}
fn chains(events: &Events) -> Vec<ChainHopView> {
    events
        .iter()
        .find_map(|(_, e)| match e {
            EventV4::Private {
                event: Event::ChainDamageResolved { hops, .. },
            } => Some(hops.clone()),
            _ => None,
        })
        .unwrap_or_default()
}
fn returns(events: &Events) -> Vec<SkillCooldownChange> {
    events
        .iter()
        .find_map(|(_, e)| match e {
            EventV4::Private {
                event: Event::SkillCooldownsReduced { changes, .. },
            } => Some(changes.clone()),
            _ => None,
        })
        .unwrap_or_default()
}
fn private_kinds(events: &Events) -> Vec<&Event> {
    events
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::Private { event } = e {
                Some(event)
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn post_consumption_configs_roundtrip_and_strict_rejections() {
    for input in [
        crate::test_data::fixture!("examples/combat-host/post-consumption-a.json"),
        crate::test_data::fixture!("examples/combat-host/post-consumption-b.json"),
    ] {
        let p = Package::new();
        let v: Value = serde_json::from_str(input).unwrap();
        p.write(&v);
        let c = p.load().unwrap();
        p.write(&serde_json::to_value(c.default_config().as_ref()).unwrap());
        assert!(p.load().is_ok());
    }
    for case in 0..22 {
        let p = Package::new();
        let mut v: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/post-consumption-a.json"
        ))
        .unwrap();
        let t = &mut v["combat_rules"]["triggers"][0];
        match case {
            0 => {
                t.as_object_mut().unwrap().remove("contact_consumption");
            }
            1 => t["contact_consumption"] = Value::Null,
            2 => t["contact_consumption"]["at_least"] = json!(0),
            3 => t["contact_consumption"]["at_least"] = json!(4),
            4 => t["contact_consumption"]["definition_id"] = json!("unknown"),
            5 => t["contact_consumption"]["typo"] = json!(true),
            6 => t["when"] = json!("direct_hit"),
            7 => t["include_periodic"] = json!(true),
            8 => t["counting"] = json!("action_total"),
            9 => t["effect"]["rule"]["direction"] = Value::Null,
            10 => t["effect"]["rule"]["direction"] = json!("backward"),
            11 => v["combat_rules"]["triggers"][1]["effect"]["skill_ids"] = json!([]),
            12 => v["combat_rules"]["triggers"][1]["effect"]["skill_ids"] = json!(["unknown"]),
            13 => {
                v["combat_rules"]["triggers"][1]["effect"]["skill_ids"] =
                    json!(["break-bolt", "break-bolt"])
            }
            14 => v["combat_rules"]["triggers"][1]["effect"]["remaining_bps"] = json!(10001),
            15 => v["combat_rules"]["triggers"][1]["effect"]["fixed_ms"] = json!(3600001),
            16 => {
                v["combat_rules"]["triggers"][1]["effect"]["remaining_bps"] = json!(0);
                v["combat_rules"]["triggers"][1]["effect"]["fixed_ms"] = json!(0);
            }
            17 => v["combat_rules"]["triggers"][1]["effect"]["remaining_bps"] = Value::Null,
            18 => v["combat_rules"]["triggers"][1]["effect"]["typo"] = json!(true),
            19 => v["combat_rules"]["triggers"][1]["effect"]["skill_ids"] = Value::Null,
            20 => {
                v["combat_rules"]["triggers"][1]["effect"]["skill_ids"] =
                    json!(vec!["break-bolt"; 9])
            }
            _ => v["mechanism_version"] = json!(104),
        };
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn consumed_three_layers_inherits_enhanced_raw_and_returns_current_remaining() {
    let vector: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_post_consumption_vectors.json"
    ))
    .unwrap();
    let (c, mut w) = fixture();
    mark(&mut w, &c, 3, 0);
    let before = w.rng;
    let e = hit(&mut w, &c, 100, 100);
    assert_eq!(chains(&e)[0].raw_damage, 105);
    assert_eq!(json!(chains(&e)[0].raw_damage), vector["pursuitRaw"]);
    assert_eq!(json!(returns(&e)[0].reduced_ms), vector["reducedMs"]);
    assert_eq!(chains(&e)[0].target.id, "chain-a");
    assert_eq!(
        returns(&e)[0],
        SkillCooldownChange {
            skill_id: "break-bolt".into(),
            previous_ready_at_ms: 3000,
            ready_at_ms: 2130,
            reduced_ms: 870
        }
    );
    assert_eq!(w.npcs[npc(&w, "capped-target")].hp, 9825);
    assert_eq!(w.npcs[npc(&w, "chain-a")].hp, 9895);
    assert_eq!(w.npcs[npc(&w, "near-behind")].hp, 10000);
    assert_eq!(w.rng, before);
}
#[test]
fn fewer_layers_are_consumed_without_fabricating_threshold() {
    for n in [0, 1, 2] {
        let (c, mut w) = fixture();
        mark(&mut w, &c, n, 0);
        let e = hit(&mut w, &c, 100, 100);
        assert!(chains(&e).is_empty() && returns(&e).is_empty());
        assert_eq!(w.members[0].cooldowns["break-bolt"], 3000);
    }
}
#[test]
fn partial_consumption_predicate_uses_actual_consumed_count() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .skills
        .get_mut("break-bolt")
        .unwrap()
        .contact_status
        .as_mut()
        .unwrap()
        .consume_max = 2;
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    assert!(chains(&e).is_empty() && returns(&e).is_empty());
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "threshold-mark")
            .unwrap()
            .stacks,
        Some(1)
    );
}
#[test]
fn marks_from_another_source_do_not_publish_consumption_fact() {
    let (c, mut w) = fixture();
    mark(&mut w, &c, 3, 1);
    let e = hit(&mut w, &c, 100, 100);
    assert!(chains(&e).is_empty() && returns(&e).is_empty());
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "threshold-mark")
            .unwrap()
            .stacks,
        Some(3)
    );
}
#[test]
fn consumption_definition_filter_cannot_borrow_another_mark() {
    let (mut c, mut w) = fixture();
    for t in &mut c.combat_rules.as_mut().unwrap().triggers {
        t.contact_consumption.as_mut().unwrap().definition_id = "different".into();
    }
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    assert!(chains(&e).is_empty() && returns(&e).is_empty());
}
#[test]
fn fully_absorbed_primary_still_inherits_pre_defense_raw() {
    let (mut c, mut w) = fixture();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"full-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let j = npc(&w, "capped-target");
    grant(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "full-shield");
    mark(&mut w, &c, 3, 0);
    let hp = w.npcs[j].hp;
    let e = hit(&mut w, &c, 100, 100);
    assert_eq!(w.npcs[j].hp, hp);
    assert_eq!(chains(&e)[0].raw_damage, 105);
    assert_eq!(returns(&e).len(), 1);
}

#[test]
fn immunity_prevents_both_mark_consumption_and_linked_effects() {
    let (mut c, mut w) = fixture();
    let d = json!({"id":"immune-test","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true});
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
    let j = npc(&w, "capped-target");
    grant(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "immune-test");
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    assert!(chains(&e).is_empty() && returns(&e).is_empty());
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "threshold-mark")
            .unwrap()
            .stacks,
        Some(3)
    );
}
#[test]
fn no_forward_direction_suppresses_pursuit_but_preserves_independent_refund() {
    let (c, mut w) = fixture();
    w.members[0].position = w.npcs[npc(&w, "capped-target")].position;
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    assert!(chains(&e).is_empty());
    assert_eq!(returns(&e).len(), 1);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "threshold-mark")
    );
}
#[test]
fn forward_boundary_excludes_perpendicular_and_coincident_candidates() {
    let (c, mut w) = fixture();
    let p = w.npcs[npc(&w, "capped-target")].position;
    let j = npc(&w, "near-behind");
    w.npcs[j].position = [p[0] + 10., p[1] + 10.];
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    assert_eq!(chains(&e)[0].target.id, "chain-a");
}
#[test]
fn killed_primary_can_anchor_forward_pursuit() {
    let (c, mut w) = fixture();
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 1;
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    assert_eq!(w.npcs[j].hp, 0);
    assert_eq!(chains(&e)[0].raw_damage, 105);
    assert_eq!(returns(&e).len(), 1);
}
#[test]
fn empty_remaining_cooldown_does_not_reopen_or_report_deadline() {
    let (c, mut w) = fixture();
    w.members[0].cooldowns.insert("break-bolt".into(), 100);
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    assert_eq!(chains(&e).len(), 1);
    assert!(returns(&e).is_empty());
    assert_eq!(w.members[0].cooldowns["break-bolt"], 100);
}
#[test]
fn reductions_floor_each_ratio_then_add_fixed_and_saturate_remaining() {
    let (c, mut w) = fixture();
    w.members[0].cooldowns.insert("mark-bolt".into(), 111);
    w.members[0].cooldowns.insert("break-bolt".into(), 101);
    let ids = vec!["mark-bolt".into(), "break-bolt".into(), "absent".into()];
    let p = w.plan_cooldown_reduction(Owner::Member(0), &ids, 3000, 2, 100);
    assert_eq!(
        p.iter().map(|r| r.reduced_ms).collect::<Vec<_>>(),
        vec![5, 1]
    );
    let p = w.plan_cooldown_reduction(Owner::Member(0), &ids, 10000, 3600000, 100);
    assert_eq!(
        p.iter().map(|r| r.ready_at_ms).collect::<Vec<_>>(),
        vec![100, 100]
    );
    assert!(
        w.plan_cooldown_reduction(Owner::Npc(0), &ids, 3000, 2, 100)
            .is_empty()
    );
    drop(c);
}
#[test]
fn cooldown_plan_uses_wide_arithmetic_near_u64_limit() {
    let (_, mut w) = fixture();
    w.members[0].cooldowns.insert("break-bolt".into(), u64::MAX);
    let p = w.plan_cooldown_reduction(Owner::Member(0), &["break-bolt".into()], 10000, 3600000, 1);
    assert_eq!(p[0].reduced_ms, u64::MAX - 1);
    assert_eq!(p[0].ready_at_ms, 1);
}
#[test]
fn refund_does_not_edit_other_members_or_items_or_work_clocks() {
    let (c, mut w) = fixture();
    w.members[1].cooldowns.insert("break-bolt".into(), 3000);
    w.members[0].item_cooldowns.insert("item".into(), 3000);
    w.members[0].basic_ready = 3000;
    w.members[0].recovery_until = 3000;
    mark(&mut w, &c, 3, 0);
    let _ = hit(&mut w, &c, 100, 100);
    assert_eq!(w.members[1].cooldowns["break-bolt"], 3000);
    assert_eq!(w.members[0].item_cooldowns["item"], 3000);
    assert_eq!(w.members[0].basic_ready, 3000);
    assert_eq!(w.members[0].recovery_until, 3000);
}
#[test]
fn two_same_tick_refunds_observe_current_deadline_in_priority_order() {
    let (mut c, mut w) = fixture();
    let mut t = c.combat_rules.as_ref().unwrap().triggers[1].clone();
    t.id = "second-refund".into();
    t.priority = 2;
    c.combat_rules.as_mut().unwrap().triggers.push(t);
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    let changes = private_kinds(&e)
        .iter()
        .filter_map(|e| {
            if let Event::SkillCooldownsReduced { changes, .. } = e {
                Some(changes[0].clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        changes.iter().map(|c| c.reduced_ms).collect::<Vec<_>>(),
        vec![870, 609]
    );
    assert_eq!(w.members[0].cooldowns["break-bolt"], 1521);
}
#[test]
fn original_damage_direct_hit_then_consumption_link_order_is_stable() {
    let (mut c, mut w) = fixture();
    let mut t = c.combat_rules.as_ref().unwrap().triggers[1].clone();
    t.id = "direct-before-consumed".into();
    t.when = CombatFact::DirectHit;
    t.contact_consumption = None;
    t.effect = TriggerEffect::RestoreSpirit {
        amount: 1,
        max_spirit_bps: 0,
    };
    c.combat_rules.as_mut().unwrap().triggers.push(t);
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    let ids = private_kinds(&e)
        .iter()
        .filter_map(|e| {
            if let Event::CombatTriggerFired { trigger_id, .. } = e {
                Some(trigger_id.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "direct-before-consumed",
            "consumed-pursuit",
            "consumed-refund"
        ]
    );
}
#[test]
fn zero_rounded_refund_is_preflighted_before_chance_and_condition_consumption() {
    let (mut c, mut w) = fixture();
    let r = c.combat_rules.as_mut().unwrap();
    r.triggers.remove(0);
    r.triggers[0].effect = TriggerEffect::ReduceSkillCooldowns {
        skill_ids: vec!["break-bolt".into()],
        remaining_bps: 1,
        fixed_ms: 0,
    };
    r.triggers[0].chance_bps = Some(5000);
    r.triggers[0].conditions.status = Some(
        serde_json::from_value(
            json!({"subject":"owner","definition_id":"threshold-mark","at_least":1,"consume":1}),
        )
        .unwrap(),
    );
    w.members[0].cooldowns.insert("break-bolt".into(), 101);
    grant(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Member(0),
        "threshold-mark",
    );
    mark(&mut w, &c, 3, 0);
    let rng = w.rng;
    let e = hit(&mut w, &c, 100, 100);
    assert!(returns(&e).is_empty());
    assert_eq!(w.rng, rng);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.target.id == "P00" && s.definition_id == "threshold-mark")
    );
}
#[test]
fn consumption_links_do_not_run_during_derived_execution() {
    let (c, mut w) = fixture();
    mark(&mut w, &c, 3, 0);
    w.triggers.executing = true;
    let e = hit(&mut w, &c, 100, 100);
    assert!(chains(&e).is_empty() && returns(&e).is_empty());
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "threshold-mark")
            .unwrap()
            .stacks,
        Some(3)
    );
}
#[test]
fn private_cooldown_results_are_only_delivered_to_owner() {
    let (c, mut w) = fixture();
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    let recipients = e
        .iter()
        .filter_map(|(i, e)| {
            if matches!(
                e,
                EventV4::Private {
                    event: Event::SkillCooldownsReduced { .. }
                }
            ) {
                Some(*i)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(recipients, vec![Some(0)]);
}

#[test]
fn newly_applied_marks_do_not_retroactively_satisfy_consumed_threshold() {
    let (c, mut w) = fixture();
    mark(&mut w, &c, 2, 0);
    let source = w.owner_entity(Owner::Member(0));
    let mut e = vec![];
    w.direct_contact(
        Owner::Member(0),
        Owner::Npc(npc(&w, "capped-target")),
        combat::DirectContact {
            origin: None,
            action_id: None,
            rule: Some(&c.combat_rules.as_ref().unwrap().skills["break-bolt"]),
            ignore_armor: false,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw: 100,
            metal: true,
            hit_statuses: &["threshold-mark".into()],
        },
        &c,
        100,
        &mut e,
    );
    assert!(chains(&e).is_empty() && returns(&e).is_empty());
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "threshold-mark")
            .unwrap()
            .stacks,
        Some(1)
    );
}
#[test]
fn linked_action_counter_deduplicates_multiple_consumptions_of_one_action() {
    let (mut c, mut w) = fixture();
    for t in &mut c.combat_rules.as_mut().unwrap().triggers {
        t.counting = crate::config::TriggerCounting::Action;
    }
    w.triggers.action = Some("same".into());
    mark(&mut w, &c, 3, 0);
    let first = hit(&mut w, &c, 100, 100);
    mark(&mut w, &c, 3, 0);
    let next = hit(&mut w, &c, 100, 100);
    assert_eq!(returns(&first).len(), 1);
    assert!(chains(&next).is_empty() && returns(&next).is_empty());
    w.triggers.action = Some("next".into());
    mark(&mut w, &c, 3, 0);
    assert_eq!(returns(&hit(&mut w, &c, 100, 100)).len(), 1);
}
#[test]
fn expired_marks_cannot_publish_consumption_or_refund() {
    let (c, mut w) = fixture();
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 20000, 100);
    assert!(chains(&e).is_empty() && returns(&e).is_empty());
}
#[test]
fn forward_chain_turns_with_each_actual_segment() {
    let (mut c, mut w) = fixture();
    if let TriggerEffect::ChainDamage { rule } =
        &mut c.combat_rules.as_mut().unwrap().triggers[0].effect
    {
        rule.max_hops = 3;
        rule.radius = 150.;
    }
    let b = npc(&w, "chain-b");
    let z = npc(&w, "chain-c");
    w.npcs[b].position = [680., 200.];
    w.npcs[z].position = [640., 120.];
    mark(&mut w, &c, 3, 0);
    let e = hit(&mut w, &c, 100, 100);
    assert_eq!(
        chains(&e)
            .iter()
            .map(|h| h.target.id.as_str())
            .collect::<Vec<_>>(),
        vec!["chain-a", "chain-b", "chain-c"]
    );
}
#[test]
fn generic_consumption_has_no_post_contact_reward_entry() {
    let (c, mut w) = fixture();
    mark(&mut w, &c, 3, 0);
    let id = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "threshold-mark")
        .unwrap()
        .instance_id
        .clone();
    let mut ignored = vec![];
    assert!(w.statuses.consume(&id, 3, &mut ignored));
    let e = hit(&mut w, &c, 100, 100);
    assert!(chains(&e).is_empty() && returns(&e).is_empty());
}
