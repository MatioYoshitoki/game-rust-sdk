use super::*;
use crate::config::{ChainDamageRule, CombatFact, TriggerEffect};
fn fixture() -> (Config, World) {
    let p = Package::new();
    p.write(
        &serde_json::from_str::<Value>(crate::test_data::fixture!(
            "examples/combat-host/chain-a.json"
        ))
        .unwrap(),
    );
    let c = p.load().unwrap().default_config().as_ref().clone();
    let n = c.members.as_ref().unwrap().capacity;
    let seats = (0..n)
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
    (c, w)
}
fn npc(w: &World, id: &str) -> usize {
    w.npcs.iter().position(|n| n.entity.id == id).unwrap()
}
fn rule(c: &mut Config) -> &mut ChainDamageRule {
    match &mut c.combat_rules.as_mut().unwrap().triggers[0].effect {
        TriggerEffect::ChainDamage { rule } => rule,
        _ => panic!("chain fixture"),
    }
}
fn add(c: &mut Config, d: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
}
fn grant(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str, now: u64) {
    let e = w.owner_entity(target);
    w.apply_owned_statuses(source, (e, 10000), &[id.into()], c, now, &mut vec![]);
}
fn marks(w: &mut World, c: &Config, source: usize) {
    let target = Owner::Npc(npc(w, "capped-target"));
    for _ in 0..3 {
        grant(w, c, Owner::Member(source), target, "threshold-mark", 0);
    }
}
fn fire(w: &mut World, c: &Config, now: u64, amount: u32) -> Events {
    let mut e = vec![];
    let j = npc(w, "capped-target");
    w.combat_fact(
        CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(j)),
        (amount, false),
        c,
        now,
        &mut e,
    );
    e
}
fn hops(e: &Events) -> Vec<ChainHopView> {
    e.iter()
        .find_map(|(_, e)| {
            if let EventV4::Private {
                event: Event::ChainDamageResolved { hops, .. },
            } = e
            {
                Some(hops.clone())
            } else {
                None
            }
        })
        .unwrap_or_default()
}
fn reason(e: &Events) -> Option<ChainEndReason> {
    e.iter().find_map(|(_, e)| {
        if let EventV4::Private {
            event: Event::ChainDamageResolved { reason, .. },
        } = e
        {
            Some(*reason)
        } else {
            None
        }
    })
}
fn initial(e: &Events) -> Option<u32> {
    e.iter().find_map(|(_, e)| {
        if let EventV4::Private {
            event: Event::ChainDamageResolved { initial_raw, .. },
        } = e
        {
            Some(*initial_raw)
        } else {
            None
        }
    })
}
fn spent(e: &Events) -> Vec<u32> {
    e.iter()
        .filter_map(|(_, e)| {
            if let EventV4::Private {
                event:
                    Event::StatusConsumed {
                        consumed_stacks, ..
                    },
            } = e
            {
                Some(*consumed_stacks)
            } else {
                None
            }
        })
        .collect()
}
fn mark_count(w: &World) -> u32 {
    w.statuses
        .entries()
        .iter()
        .filter(|s| s.definition_id == "threshold-mark")
        .map(|s| s.stacks.unwrap_or(1))
        .sum()
}
#[test]
fn chain_loader_roundtrips_samples_and_rejects_unbounded_invalid_or_unsupported_rules() {
    let p = Package::new();
    for txt in [
        crate::test_data::fixture!("examples/combat-host/chain-a.json"),
        crate::test_data::fixture!("examples/combat-host/chain-b.json"),
    ] {
        let v: Value = serde_json::from_str(txt).unwrap();
        p.write(&v);
        assert!(p.load().is_ok());
    }
    let base: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/chain-a.json"
    ))
    .unwrap();
    for case in 0..19 {
        let mut v = base.clone();
        let r = &mut v["combat_rules"]["triggers"][0]["effect"]["rule"];
        match case {
            0 => r["max_hops"] = json!(9),
            1 => r["max_hops"] = json!(0),
            2 => r["radius"] = json!(0),
            3 => r["retention_bps"] = json!(10001),
            4 => r["final_multiplier_bps"] = json!(0),
            5 => r["formula"] = json!({"fixed":0,"source_attack_bps":0,"fact_amount_bps":0}),
            6 => r["element"] = Value::Null,
            7 => r["element"] = json!("unknown"),
            8 => r["damage_kind"] = json!("elemental"),
            9 => r["on_hit_target_statuses"] = json!(["compound-lock", "compound-lock"]),
            10 => r["on_hit_target_statuses"] = json!(["missing"]),
            11 => r["unknown"] = json!(1),
            12 => v["combat_rules"]["max_chain_hits_per_step"] = json!(0),
            13 => v["combat_rules"]["max_chain_hits_per_step"] = json!(129),
            14 => v["combat_rules"]["triggers"][0]["when"] = json!("ally_skill_committed"),
            15 => v["combat_rules"]["triggers"][0]["counting"] = json!("action_total"),
            16 => v["mechanism_version"] = json!(104),
            17 => {
                v["combat_rules"]["status_policies"]["compound-lock"]["disposition"] =
                    json!("beneficial")
            }
            _ => r["formula"]["source_attack_bps"] = json!(100001),
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn chain_original_count_decay_and_final_coefficients_drive_native_contacts() {
    let (c, mut w) = fixture();
    marks(&mut w, &c, 0);
    let before = w.rng;
    let e = fire(&mut w, &c, 0, 100);
    let vector: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_chain_vectors.json"
    ))
    .unwrap();
    assert_eq!(
        vector["sourceSha256"],
        "318480dc7cd726c7aea49cbace50e8c196de95d2914c9df033e6bd0f3a73e322"
    );
    assert_eq!(
        hops(&e).iter().map(|h| h.raw_damage).collect::<Vec<_>>(),
        vec![80, 60, 54]
    );
    assert_eq!(
        hops(&e)
            .iter()
            .map(|h| h.target.id.as_str())
            .collect::<Vec<_>>(),
        vec!["chain-a", "chain-b", "chain-c"]
    );
    assert_eq!(spent(&e), vec![3]);
    assert_eq!(reason(&e), Some(ChainEndReason::Complete));
    assert_eq!(w.rng, before);
    for (id, damage) in [("chain-a", 80), ("chain-b", 60), ("chain-c", 54)] {
        assert_eq!(w.npcs[npc(&w, id)].hp, 10000 - damage);
    }
}
#[test]
fn chain_ties_use_identity_and_ignore_storage_order() {
    let (c, mut w) = fixture();
    let a = npc(&w, "chain-a");
    let b = npc(&w, "chain-b");
    w.npcs[b].position = w.npcs[a].position;
    w.npcs.swap(a, b);
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(
        hops(&e)
            .iter()
            .map(|h| h.target.id.as_str())
            .collect::<Vec<_>>(),
        vec!["chain-a", "chain-b"]
    );
    assert_eq!(reason(&e), Some(ChainEndReason::NoTarget));
}
#[test]
fn chain_radius_boundary_and_no_target_retry_do_not_spend_condition_early() {
    let (mut c, mut w) = fixture();
    rule(&mut c).radius = 79.999;
    marks(&mut w, &c, 0);
    let before = w.rng;
    let e = fire(&mut w, &c, 0, 100);
    assert!(hops(&e).is_empty());
    assert!(spent(&e).is_empty());
    assert_eq!(mark_count(&w), 3);
    assert_eq!(w.rng, before);
    rule(&mut c).radius = 80.;
    assert_eq!(hops(&fire(&mut w, &c, 100, 100)).len(), 3);
}
#[test]
fn chain_checks_line_of_sight_before_consumption() {
    let (c, mut w) = fixture();
    marks(&mut w, &c, 0);
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
    let e = fire(&mut w, &c, 0, 100);
    assert!(hops(&e).is_empty());
    assert_eq!(mark_count(&w), 3);
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = false;
    assert_eq!(hops(&fire(&mut w, &c, 100, 100)).len(), 3);
}
#[test]
fn chain_excludes_origin_source_and_every_attempted_entity() {
    let (mut c, mut w) = fixture();
    rule(&mut c).max_hops = 8;
    rule(&mut c).radius = 2000.;
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    let h = hops(&e);
    let targets = h
        .iter()
        .map(|h| (h.target.kind, h.target.id.clone(), h.target.generation))
        .collect::<Vec<_>>();
    assert!(
        !h.iter()
            .any(|h| h.target.id == "capped-target" || h.target.id == "P00")
    );
    for (i, t) in targets.iter().enumerate() {
        assert!(!targets[..i].contains(t));
    }
    assert_eq!(h.len(), w.npcs.len() - 1);
    assert_eq!(reason(&e), Some(ChainEndReason::NoTarget));
}
#[test]
fn chain_zero_rounded_formula_does_not_spend_marks_icd_or_rng() {
    let (mut c, mut w) = fixture();
    rule(&mut c).formula.source_attack_bps = 1;
    marks(&mut w, &c, 0);
    let before = w.rng;
    assert!(hops(&fire(&mut w, &c, 0, 0)).is_empty());
    assert_eq!(mark_count(&w), 3);
    assert_eq!(w.rng, before);
    rule(&mut c).formula.source_attack_bps = 8000;
    assert_eq!(hops(&fire(&mut w, &c, 100, 0)).len(), 3);
}
#[test]
fn chain_budget_ends_a_prefix_and_never_defers_the_last_hop() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().max_chain_hits_per_step = 2;
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(
        hops(&e).iter().map(|h| h.raw_damage).collect::<Vec<_>>(),
        vec![80, 60]
    );
    assert_eq!(reason(&e), Some(ChainEndReason::Capacity));
    assert_eq!(spent(&e), vec![3]);
    assert!(hops(&w.tick(&c, 30)).is_empty());
    assert_eq!(w.npcs[npc(&w, "chain-c")].hp, 10000);
}
#[test]
fn chain_shared_budget_applies_across_trigger_owners_in_one_step() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().max_chain_hits_per_step = 4;
    marks(&mut w, &c, 0);
    marks(&mut w, &c, 1);
    assert_eq!(hops(&fire(&mut w, &c, 0, 100)).len(), 3);
    let mut e = vec![];
    let j = npc(&w, "capped-target");
    w.combat_fact(
        CombatFact::DirectHit,
        (Owner::Member(1), Owner::Npc(j)),
        (100, false),
        &c,
        0,
        &mut e,
    );
    assert_eq!(hops(&e).len(), 1);
    assert_eq!(reason(&e), Some(ChainEndReason::Capacity));
    assert_eq!(w.triggers.chain_used, 4);
}
#[test]
fn chain_immune_target_costs_an_attempt_and_continues_without_status() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"immune-hop","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let a = Owner::Npc(npc(&w, "chain-a"));
    grant(&mut w, &c, a, a, "immune-hop", 0);
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(
        hops(&e).iter().map(|h| h.applied).collect::<Vec<_>>(),
        vec![false, true, true]
    );
    assert_eq!(w.npcs[npc(&w, "chain-a")].hp, 10000);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.target.id == "chain-a" && s.definition_id == "compound-lock")
    );
    assert_eq!(w.triggers.chain_used, 3);
}
#[test]
fn chain_fully_absorbed_contact_still_counts_and_applies_attached_control() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"hop-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let a = Owner::Npc(npc(&w, "chain-a"));
    grant(&mut w, &c, a, a, "hop-shield", 0);
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    assert!(hops(&e)[0].applied);
    assert_eq!(w.npcs[npc(&w, "chain-a")].hp, 10000);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.target.id == "chain-a" && s.definition_id == "compound-lock")
    );
}
#[test]
fn chain_physical_damage_uses_live_defense_reduction_and_shared_hp_pipeline() {
    let (mut c, mut w) = fixture();
    let r = rule(&mut c);
    r.damage_kind = crate::config::DamageKind::Physical;
    r.max_hops = 1;
    r.final_multiplier_bps = 10000;
    let a = npc(&w, "chain-a");
    Arc::make_mut(&mut w.npcs[a].profile)
        .stats
        .insert("def".into(), 100);
    add(
        &mut c,
        json!({"id":"hop-reduction","mechanism":"timed_attribute_modifier","attribute":"damage_reduction","modifier_bps":5000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    grant(&mut w, &c, Owner::Npc(a), Owner::Npc(a), "hop-reduction", 0);
    marks(&mut w, &c, 0);
    fire(&mut w, &c, 0, 100);
    assert_eq!(w.npcs[a].hp, 9980);
}
#[test]
fn chain_elemental_damage_uses_resistance_and_does_not_roll_critical_or_dodge() {
    let (mut c, mut w) = fixture();
    let r = rule(&mut c);
    r.damage_kind = crate::config::DamageKind::Elemental;
    r.element = Some("water".into());
    r.max_hops = 1;
    r.final_multiplier_bps = 10000;
    let p = c
        .combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("ordinary-control")
        .unwrap();
    p.element_resistance = Some(100);
    p.dodge_bps = 100000000;
    marks(&mut w, &c, 0);
    let rng = w.rng;
    fire(&mut w, &c, 0, 100);
    assert_eq!(w.npcs[npc(&w, "chain-a")].hp, 9960);
    assert_eq!(w.rng, rng);
}
#[test]
fn chain_damage_respects_native_once_only_lethal_guard() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().death_guards.insert(
        "ordinary-control".into(),
        serde_json::from_value(json!({"retain_hp":1,"heal_max_hp_bps":0,"self_statuses":[]}))
            .unwrap(),
    );
    let a = npc(&w, "chain-a");
    w.npcs[a].hp = 10;
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(w.npcs[a].hp, 1);
    assert!(hops(&e)[0].applied);
    assert_eq!(w.triggers.death_used.len(), 1);
}
#[test]
fn chain_budget_freezes_source_attack_even_when_a_hop_removes_its_source_buff() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"borrowed-power","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let a = npc(&w, "chain-a");
    grant(
        &mut w,
        &c,
        Owner::Npc(a),
        Owner::Member(0),
        "borrowed-power",
        0,
    );
    w.npcs[a].hp = 1;
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(initial(&e), Some(160));
    assert_eq!(
        hops(&e).iter().map(|h| h.raw_damage).collect::<Vec<_>>(),
        vec![160, 120, 108]
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "borrowed-power")
    );
}
#[test]
fn chain_previews_and_samples_after_its_own_condition_consumes_attack_stacks() {
    let (mut c, mut w) = fixture();
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "threshold-mark")
        .unwrap();
    d.attribute = Some(StatusAttribute::Atk);
    d.modifier_bps = 10000;
    c.combat_rules.as_mut().unwrap().triggers[0]
        .conditions
        .status
        .as_mut()
        .unwrap()
        .subject = crate::config::TriggerSubject::Owner;
    for _ in 0..3 {
        grant(
            &mut w,
            &c,
            Owner::Member(0),
            Owner::Member(0),
            "threshold-mark",
            0,
        );
    }
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(initial(&e), Some(80));
    assert_eq!(spent(&e), vec![3]);
}
#[test]
fn chain_derived_hits_do_not_feed_damage_triggers_or_new_marks() {
    let (mut c, mut w) = fixture();
    let mut t = serde_json::to_value(&c.combat_rules.as_ref().unwrap().triggers[0]).unwrap();
    t["id"] = json!("forbidden-derived-reward");
    t["when"] = json!("damage_dealt");
    t["conditions"] = json!({});
    t["effect"] = json!({"kind":"restore_spirit","amount":50});
    c.combat_rules
        .as_mut()
        .unwrap()
        .triggers
        .push(serde_json::from_value(t).unwrap());
    w.members[0].spirit = 1;
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(w.members[0].spirit, 1);
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::CombatTriggerFired { .. }
                }
            ))
            .count(),
        1
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "threshold-mark")
    );
}
#[test]
fn chain_attached_control_immunity_failure_does_not_undo_damage_or_other_hops() {
    for first in [false, true] {
        let (mut c, mut w) = fixture();
        add(
            &mut c,
            json!({"id":"hop-guard","mechanism":"timed_control","attribute":if first{"first_control_immunity"}else{"control_immunity"},"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
        );
        let a = Owner::Npc(npc(&w, "chain-a"));
        grant(&mut w, &c, a, a, "hop-guard", 0);
        marks(&mut w, &c, 0);
        let e = fire(&mut w, &c, 0, 100);
        assert_eq!(hops(&e).len(), 3);
        assert_eq!(w.npcs[npc(&w, "chain-a")].hp, 9920);
        assert!(
            !w.statuses
                .entries()
                .iter()
                .any(|s| s.target.id == "chain-a" && s.definition_id == "compound-lock")
        );
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "hop-guard"),
            !first
        );
    }
}
#[test]
fn chain_attached_state_uses_target_profile_and_original_duration() {
    let (c, mut w) = fixture();
    marks(&mut w, &c, 0);
    fire(&mut w, &c, 0, 100);
    for id in ["chain-a", "chain-b"] {
        let s = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.target.id == id && s.definition_id == "compound-lock")
            .unwrap();
        assert_eq!(s.source.id, "P00");
        assert_eq!(s.expires_at_ms - s.applied_at_ms, 1200);
    }
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.target.id == "chain-c" && s.definition_id == "control-softened")
        .unwrap();
    assert_eq!(s.expires_at_ms - s.applied_at_ms, 1200);
    assert!(s.flags.is_empty());
}
#[test]
fn chain_can_continue_from_a_lethal_origin_without_hitting_it_again() {
    let (c, mut w) = fixture();
    marks(&mut w, &c, 0);
    let origin = npc(&w, "capped-target");
    w.npcs[origin].hp = 0;
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(hops(&e).len(), 3);
    assert!(!hops(&e).iter().any(|h| h.target.id == "capped-target"));
}
#[test]
fn chain_source_exit_cleans_attached_instances_and_never_replays_a_completed_chain() {
    let (c, mut w) = fixture();
    marks(&mut w, &c, 0);
    fire(&mut w, &c, 0, 100);
    w.action(0, &Action::Leave {}, "leave-chain", &c, 30)
        .unwrap();
    assert!(!w.statuses.entries().iter().any(|s| s.source.id == "P00"));
    assert!(hops(&w.tick(&c, 60)).is_empty());
}
#[test]
fn chain_formula_combines_fractions_before_floor_and_uses_actual_fact_amount() {
    let (mut c, mut w) = fixture();
    w.members[0].facts.atk_points = 101;
    let r = rule(&mut c);
    r.formula.fixed = 1;
    r.formula.source_attack_bps = 3333;
    r.formula.fact_amount_bps = 5000;
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 3);
    assert_eq!(initial(&e), Some(36));
    assert_eq!(
        hops(&e).iter().map(|h| h.raw_damage).collect::<Vec<_>>(),
        vec![36, 27, 24]
    );
}
#[test]
fn chain_zero_decay_and_early_end_do_not_shift_final_hop_bonus() {
    let (mut c, mut w) = fixture();
    rule(&mut c).retention_bps = 0;
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, 100);
    assert_eq!(
        hops(&e).iter().map(|h| h.raw_damage).collect::<Vec<_>>(),
        vec![80]
    );
    assert_eq!(reason(&e), Some(ChainEndReason::ZeroBudget));
}
#[test]
fn chain_saturates_raw_budget_without_overflowing_or_claiming_overkill_damage() {
    let (mut c, mut w) = fixture();
    w.members[0].facts.atk_points = u32::MAX;
    rule(&mut c).formula.source_attack_bps = 100000;
    marks(&mut w, &c, 0);
    let e = fire(&mut w, &c, 0, u32::MAX);
    assert_eq!(initial(&e), Some(u32::MAX));
    assert!(
        e.iter()
            .filter_map(|(_, e)| if let EventV4::Damage { amount, .. } = e {
                Some(*amount)
            } else {
                None
            })
            .all(|amount| amount <= 10000)
    );
}
#[test]
fn chain_action_dedup_keeps_failed_attempt_atomic_until_a_new_action() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers[0].counting = crate::config::TriggerCounting::Action;
    rule(&mut c).radius = 1.;
    marks(&mut w, &c, 0);
    w.triggers.action = Some("same".into());
    assert!(hops(&fire(&mut w, &c, 0, 100)).is_empty());
    rule(&mut c).radius = 100.;
    assert!(hops(&fire(&mut w, &c, 100, 100)).is_empty());
    assert_eq!(mark_count(&w), 3);
    w.triggers.action = Some("next".into());
    assert_eq!(hops(&fire(&mut w, &c, 200, 100)).len(), 3);
}
#[test]
fn chain_probability_failure_does_not_consume_marks_or_start_icd() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(0);
    marks(&mut w, &c, 0);
    let before = w.rng;
    assert!(hops(&fire(&mut w, &c, 0, 100)).is_empty());
    assert_eq!(mark_count(&w), 3);
    assert_eq!(w.rng, before);
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(10000);
    assert_eq!(hops(&fire(&mut w, &c, 100, 100)).len(), 3);
}
#[test]
fn chain_defender_npc_owns_counter_damage_and_selects_new_hostile_members() {
    let (mut c, mut w) = fixture();
    let source = npc(&w, "capped-target");
    w.members[1].position = [580., 300.];
    w.members[2].position = [660., 300.];
    w.members[3].position = [740., 300.];
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.when = CombatFact::DamageTaken;
    t.profiles = vec!["ordinary-control".into()];
    t.conditions.status = None;
    let r = rule(&mut c);
    r.formula.fixed = 20;
    r.formula.source_attack_bps = 0;
    r.formula.fact_amount_bps = 0;
    r.on_hit_target_statuses.clear();
    let mut e = vec![];
    w.combat_fact(
        CombatFact::DamageTaken,
        (Owner::Npc(source), Owner::Member(0)),
        (100, false),
        &c,
        0,
        &mut e,
    );
    assert_eq!(
        hops(&e)
            .iter()
            .map(|h| h.target.id.as_str())
            .collect::<Vec<_>>(),
        vec!["P01", "P02", "P03"]
    );
    assert!(e.iter().any(|(_,e)|matches!(e,EventV4::Damage{source,target,..}if source.kind==StatusEntityKind::Monster&&target.id=="P01")));
}
