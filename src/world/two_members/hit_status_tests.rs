use super::*;
fn mark() -> Value {
    json!({"id":"hit-mark","mechanism":"timed_attribute_modifier","attribute":"movement_slow","modifier_bps":-800,"duration_ms":6000,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true})
}
fn lock() -> Value {
    json!({"id":"hit-lock","mechanism":"timed_control","attribute":"stun","duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true})
}
fn trigger() -> Value {
    json!({"id":"transform","profiles":["member"],"when":"direct_hit","include_periodic":false,"counter_scope":"target","every":1,"cooldown_ms":0,"priority":0,"conditions":{"status":{"subject":"other","definition_id":"hit-mark","at_least":3,"consume":3}},"effect":{"kind":"apply_other_status","status_id":"hit-lock"}})
}
fn add(c: &mut Config, d: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
}
fn fixture() -> (Config, World) {
    let (mut c, mut w) = setup();
    add(&mut c, mark());
    add(&mut c, lock());
    operation_policy(&mut c, "hit-mark", "harmful", true);
    operation_policy(&mut c, "hit-lock", "harmful", true);
    c.combat_rules.as_mut().unwrap().triggers = vec![serde_json::from_value(trigger()).unwrap()];
    w.npcs[0].hp = 1000;
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("hp".into(), 1000);
    (c, w)
}
fn strike(w: &mut World, c: &Config, now: u64, raw: u32) -> Events {
    let source = w.members[0].reference.entity.clone();
    let attack = AttackRule {
        damage_kind: crate::config::DamageKind::TrueDamage,
        unavoidable: true,
        can_crit: false,
        ..Default::default()
    };
    let mut e = vec![];
    w.direct_contact(
        Owner::Member(0),
        Owner::Npc(0),
        DirectContact {
            origin: None,
            action_id: Some(&format!("strike-{now}")),
            rule: Some(&attack),
            ignore_armor: false,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw,
            metal: true,
            hit_statuses: &["hit-mark".into()],
        },
        c,
        now,
        &mut e,
    );
    e
}
fn status<'a>(w: &'a World, id: &str) -> Option<&'a StatusView> {
    w.statuses.entries().iter().find(|s| s.definition_id == id)
}
fn give(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str, now: u64) {
    let target = w.owner_entity(target);
    w.apply_owned_statuses(source, (target, 1000), &[id.into()], c, now, &mut vec![]);
}
fn fired(e: &Events) -> usize {
    e.iter()
        .filter(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::CombatTriggerFired { .. }
                }
            )
        })
        .count()
}

#[test]
fn hit_status_loader_accepts_samples_and_rejects_illegal_fact_effect_carrier_and_old_capability() {
    let p = Package::new();
    for text in [
        crate::test_data::fixture!("examples/combat-host/hit-status-a.json"),
        crate::test_data::fixture!("examples/combat-host/hit-status-b.json"),
    ] {
        let v: Value = serde_json::from_str(text).unwrap();
        p.write(&v);
        assert!(p.load().is_ok());
    }
    let mut v = document();
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .extend([mark(), lock()]);
    v["combat_rules"]["triggers"] = json!([trigger()]);
    v["combat_rules"]["status_policies"] = json!({"hit-lock":{"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}});
    p.write(&v);
    assert!(p.load().is_ok());
    for case in 0..11 {
        let mut b = v.clone();
        match case {
            0 => b["mechanism_version"] = json!(104),
            1 => b["combat_rules"]["triggers"][0]["include_periodic"] = json!(true),
            2 => b["combat_rules"]["triggers"][0]["counting"] = json!("action_total"),
            3 => b["combat_rules"]["triggers"][0]["when"] = json!("ally_skill_committed"),
            4 => b["combat_rules"]["triggers"][0]["when"] = json!("kill"),
            5 => b["combat_rules"]["triggers"][0]["effect"]["status_id"] = json!("missing"),
            6 => b["combat_rules"]["triggers"][0]["effect"]["status_id"] = json!("barrier"),
            7 => {
                b["combat_rules"]["status_policies"]["hit-lock"]["disposition"] =
                    json!("beneficial")
            }
            8 => {
                b["combat_rules"]["status_policies"] = json!({});
            }
            9 => b["combat_rules"]["triggers"][0]["effect"]["status_id"] = Value::Null,
            _ => b["combat_rules"]["triggers"][0]["when"] = json!("unknown_hit"),
        }
        p.write(&b);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn hit_status_third_contact_observes_native_binding_and_consumes_exactly_one_owned_stack_group() {
    let (c, mut w) = fixture();
    for now in [0, 100] {
        let e = strike(&mut w, &c, now, 10);
        assert_eq!(fired(&e), 0);
    }
    assert_eq!(status(&w, "hit-mark").unwrap().stacks, Some(2));
    let e = strike(&mut w, &c, 200, 10);
    assert_eq!(fired(&e), 1);
    assert!(status(&w, "hit-mark").is_none());
    assert_eq!(status(&w, "hit-lock").unwrap().expires_at_ms, 1400);
    let stacked=e.iter().position(|(_,e)|matches!(e,EventV4::Private{event:Event::StatusStacked{status,..}} if status.stacks==Some(3))).unwrap();
    let consumed = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::StatusConsumed { .. }
                }
            )
        })
        .unwrap();
    let applied=e.iter().position(|(_,e)|matches!(e,EventV4::Private{event:Event::StatusApplied{status}} if status.definition_id=="hit-lock")).unwrap();
    assert!(stacked < consumed && consumed < applied);
}
#[test]
fn hit_status_zero_hp_contact_and_fully_absorbed_contact_still_transform() {
    for shielded in [false, true] {
        let (mut c, mut w) = fixture();
        if shielded {
            add(
                &mut c,
                json!({"id":"hit-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true}),
            );
            give(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "hit-shield", 0);
        }
        let hp = w.npcs[0].hp;
        for n in 0..3 {
            strike(&mut w, &c, n * 100, if shielded { 10 } else { 0 });
        }
        assert_eq!(w.npcs[0].hp, hp);
        assert!(status(&w, "hit-lock").is_some());
        assert!(status(&w, "hit-mark").is_none());
    }
}
#[test]
fn hit_status_damage_immunity_and_invalid_contact_do_not_gain_marks_or_fire() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"immune","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    give(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "immune", 0);
    assert_eq!(fired(&strike(&mut w, &c, 0, 10)), 0);
    assert!(status(&w, "hit-mark").is_none());
    w.npcs[0].hp = 0;
    strike(&mut w, &c, 100, 10);
    assert!(status(&w, "hit-mark").is_none());
}
#[test]
fn hit_status_periodic_and_trigger_derived_damage_never_emit_direct_hit() {
    let (c, mut w) = fixture();
    for now in 0..3 {
        w.resolve_damage(intent(&w, 10, false), &c, now * 100, &mut vec![]);
    }
    assert!(status(&w, "hit-mark").is_none());
    assert!(status(&w, "hit-lock").is_none());
}
#[test]
fn hit_status_stack_movement_is_per_instance_clamped_and_uses_strongest_source() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 0);
        assert_eq!(
            w.statuses
                .movement_factor_bps(&status_entity(&w.npcs[0].entity), n * 100),
            10000 - 800 * (n as u32 + 1)
        );
    }
    let s = status(&w, "hit-mark").unwrap().clone();
    w.statuses.consume(&s.instance_id, 1, &mut vec![]);
    assert_eq!(w.statuses.movement_factor_bps(&s.target, 300), 8400);
    assert_eq!(
        status(&w, "hit-mark").unwrap().expires_at_ms,
        s.expires_at_ms
    );
    give(&mut w, &c, Owner::Member(1), Owner::Npc(0), "hit-mark", 300);
    assert_eq!(w.statuses.movement_factor_bps(&s.target, 300), 8400);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "hit-mark")
        .unwrap()
        .modifier_bps = -9000;
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hit-mark", 400);
    assert_eq!(w.statuses.movement_factor_bps(&s.target, 400), 1);
    assert_eq!(w.statuses.movement_factor_bps(&s.target, 6400), 10000);
}
#[test]
fn hit_status_target_policy_resolves_replacement_using_original_control_duration() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"soft-lock","mechanism":"timed_compound_modifier","modifiers":[{"attribute":"movement_slow","modifier_bps":-5000},{"attribute":"basic_interval_delay","modifier_bps":1500}],"duration_ms":90,"duplicate_policy":"refresh","source_scoped":true}),
    );
    operation_policy(&mut c, "soft-lock", "harmful", true);
    let r = c.combat_rules.as_mut().unwrap();
    r.control_policies.insert(
        "npc".into(),
        serde_json::from_value(json!({"replacements":{"hit-lock":"soft-lock"}})).unwrap(),
    );
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 0);
    }
    assert!(status(&w, "hit-lock").is_none());
    let s = status(&w, "soft-lock").unwrap();
    assert_eq!(s.expires_at_ms - s.applied_at_ms, 1200);
    assert_eq!(s.source.id, "P00");
    assert_eq!(w.statuses.movement_factor_bps(&s.target, 200), 5000);
}
#[test]
fn hit_status_capacity_failure_keeps_marks_and_allows_next_independent_contact_to_retry() {
    let (mut c, mut w) = fixture();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    c.combat_rules.as_mut().unwrap().triggers[0]
        .conditions
        .status
        .as_mut()
        .unwrap()
        .consume = 2;
    for n in 0..3 {
        assert_eq!(fired(&strike(&mut w, &c, n * 100, 0)), 0);
    }
    assert_eq!(status(&w, "hit-mark").unwrap().stacks, Some(3));
    assert!(status(&w, "hit-lock").is_none());
    c.combat_rules.as_mut().unwrap().triggers[0]
        .conditions
        .status
        .as_mut()
        .unwrap()
        .consume = 3;
    assert_eq!(fired(&strike(&mut w, &c, 300, 0)), 1);
    assert!(status(&w, "hit-mark").is_none());
    assert!(status(&w, "hit-lock").is_some());
}
#[test]
fn hit_status_control_immunities_reject_atomically_without_consuming_first_opportunity_or_marks() {
    for attribute in ["control_immunity", "first_control_immunity"] {
        let (mut c, mut w) = fixture();
        // Populate the marks before immunity so native movement control rejection
        // does not obscure the conditional transaction being tested.
        c.combat_rules.as_mut().unwrap().triggers.clear();
        for n in 0..3 {
            strike(&mut w, &c, n * 100, 0);
        }
        add(
            &mut c,
            json!({"id":"control-guard","mechanism":"timed_control","attribute":attribute,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true}),
        );
        give(
            &mut w,
            &c,
            Owner::Npc(0),
            Owner::Npc(0),
            "control-guard",
            200,
        );
        c.combat_rules.as_mut().unwrap().triggers =
            vec![serde_json::from_value(trigger()).unwrap()];
        give(
            &mut w,
            &c,
            Owner::Npc(0),
            Owner::Npc(0),
            "control-guard",
            400,
        );
        let before = status(&w, "control-guard").unwrap().clone();
        let mut e = vec![];
        w.combat_fact(
            crate::config::CombatFact::DirectHit,
            (Owner::Member(0), Owner::Npc(0)),
            (0, false),
            &c,
            400,
            &mut e,
        );
        assert_eq!(fired(&e), 0);
        assert_eq!(status(&w, "control-guard"), Some(&before));
        assert_eq!(status(&w, "hit-mark").unwrap().stacks, Some(3));
    }
}
#[test]
fn hit_status_condition_only_matches_trigger_owners_marks_not_another_member() {
    let (c, mut w) = fixture();
    for n in 0..3 {
        give(
            &mut w,
            &c,
            Owner::Member(1),
            Owner::Npc(0),
            "hit-mark",
            n * 100,
        );
    }
    assert_eq!(fired(&strike(&mut w, &c, 300, 0)), 0);
    assert!(status(&w, "hit-lock").is_none());
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .filter(|s| s.definition_id == "hit-mark")
            .count(),
        2
    );
}
#[test]
fn hit_status_probability_failure_preserves_stack_threshold_until_new_contact() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(0);
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 0);
    }
    assert_eq!(status(&w, "hit-mark").unwrap().stacks, Some(3));
    assert!(status(&w, "hit-lock").is_none());
    c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(10000);
    assert_eq!(fired(&strike(&mut w, &c, 300, 0)), 1);
}
#[test]
fn hit_status_source_exit_cleans_native_conversion_and_partial_marks() {
    let (c, mut w) = fixture();
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 0);
    }
    w.action(0, &Action::Leave {}, "leave", &c, 300).unwrap();
    assert!(status(&w, "hit-lock").is_none());
    assert!(status(&w, "hit-mark").is_none());
}

#[test]
fn hit_status_taken_damage_counter_applies_to_actual_attacker_with_defender_attribution() {
    let (mut c, mut w) = fixture();
    let mut t = trigger();
    t["when"] = json!("damage_taken");
    t["profiles"] = json!(["npc"]);
    t["conditions"] = json!({});
    t["effect"]["status_id"] = json!("hit-mark");
    c.combat_rules.as_mut().unwrap().triggers = vec![serde_json::from_value(t).unwrap()];
    strike(&mut w, &c, 0, 10);
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "hit-mark" && s.target.id == "P00")
        .unwrap();
    assert_eq!(s.source, status_entity(&w.npcs[0].entity));
    assert_eq!(s.stacks, Some(1));
}
#[test]
fn hit_status_absorption_counter_works_without_actual_hp_damage() {
    let (mut c, mut w) = fixture();
    let mut t = trigger();
    t["when"] = json!("shield_absorbed");
    t["profiles"] = json!(["npc"]);
    t["conditions"] = json!({});
    t["effect"]["status_id"] = json!("hit-mark");
    c.combat_rules.as_mut().unwrap().triggers = vec![serde_json::from_value(t).unwrap()];
    add(
        &mut c,
        json!({"id":"hit-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    give(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "hit-shield", 0);
    let hp = w.npcs[0].hp;
    strike(&mut w, &c, 100, 10);
    assert_eq!(w.npcs[0].hp, hp);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "hit-mark"
                && s.target.id == "P00"
                && s.source == status_entity(&w.npcs[0].entity))
    );
}
#[test]
fn hit_status_preview_uses_final_condition_consumption_to_remove_category_blocker() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 0);
    }
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("hit-mark")
        .unwrap()
        .blocks = vec![crate::config::StatusDisposition::Harmful];
    let mut t = trigger();
    t["conditions"]["status"]["consume"] = json!(2);
    c.combat_rules.as_mut().unwrap().triggers = vec![serde_json::from_value(t).unwrap()];
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        300,
        &mut e,
    );
    assert_eq!(fired(&e), 0);
    assert_eq!(status(&w, "hit-mark").unwrap().stacks, Some(3));
    c.combat_rules.as_mut().unwrap().triggers[0]
        .conditions
        .status
        .as_mut()
        .unwrap()
        .consume = 3;
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        400,
        &mut e,
    );
    assert_eq!(fired(&e), 1);
    assert!(status(&w, "hit-mark").is_none());
    assert!(status(&w, "hit-lock").is_some());
}
#[test]
fn hit_status_target_replacement_is_resolved_before_conditional_immunity_preflight() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 0);
    }
    add(
        &mut c,
        json!({"id":"first","mechanism":"timed_control","attribute":"first_control_immunity","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    give(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "first", 200);
    add(
        &mut c,
        json!({"id":"numeric-lock","mechanism":"timed_attribute_modifier","attribute":"basic_interval_delay","modifier_bps":1500,"duration_ms":90,"duplicate_policy":"refresh","source_scoped":true}),
    );
    operation_policy(&mut c, "numeric-lock", "harmful", true);
    let r = c.combat_rules.as_mut().unwrap();
    r.control_policies.insert(
        "npc".into(),
        serde_json::from_value(json!({"replacements":{"hit-lock":"numeric-lock"}})).unwrap(),
    );
    r.triggers = vec![serde_json::from_value(trigger()).unwrap()];
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        300,
        &mut e,
    );
    assert_eq!(fired(&e), 1);
    assert!(status(&w, "first").is_some());
    assert!(status(&w, "hit-mark").is_none());
    assert!(status(&w, "numeric-lock").is_some());
}
#[test]
fn hit_status_action_dedup_does_not_retry_failed_capacity_within_the_same_action() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 0);
    }
    let mut t = trigger();
    t["counting"] = json!("action");
    t["conditions"]["status"]["consume"] = json!(2);
    c.combat_rules.as_mut().unwrap().triggers = vec![serde_json::from_value(t).unwrap()];
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    w.triggers.action = Some("same".into());
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        300,
        &mut e,
    );
    c.combat_rules.as_mut().unwrap().triggers[0]
        .conditions
        .status
        .as_mut()
        .unwrap()
        .consume = 3;
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        400,
        &mut e,
    );
    assert_eq!(fired(&e), 0);
    assert_eq!(status(&w, "hit-mark").unwrap().stacks, Some(3));
    w.triggers.action = Some("next".into());
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        500,
        &mut e,
    );
    assert_eq!(fired(&e), 1);
}
#[test]
fn hit_status_amount_condition_uses_hp_damage_and_excludes_absorption() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers[0]
        .conditions
        .minimum_amount = 1;
    add(
        &mut c,
        json!({"id":"hit-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    give(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "hit-shield", 0);
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 10);
    }
    assert_eq!(status(&w, "hit-mark").unwrap().stacks, Some(3));
    assert!(status(&w, "hit-lock").is_none());
    w.statuses.remove(
        StatusRemovalReason::Cleansed,
        |s| s.definition_id == "hit-shield",
        &mut vec![],
    );
    assert_eq!(fired(&strike(&mut w, &c, 300, 10)), 1);
}
#[test]
fn hit_status_lethal_contact_allows_owner_reward_but_never_installs_a_dead_target_status() {
    let (mut c, mut w) = fixture();
    let mut t = trigger();
    t["conditions"] = json!({});
    t["effect"] = json!({"kind":"restore_spirit","amount":1});
    c.combat_rules.as_mut().unwrap().triggers = vec![serde_json::from_value(t).unwrap()];
    w.members[0].spirit = 1;
    w.npcs[0].hp = 1;
    assert_eq!(fired(&strike(&mut w, &c, 0, 1000)), 1);
    assert_eq!(w.members[0].spirit, 2);
    assert!(status(&w, "hit-mark").is_none());
    assert!(status(&w, "hit-lock").is_none());
}
#[test]
fn hit_status_invalid_friendly_other_and_shared_step_budget_do_not_partially_grant() {
    let (mut c, mut w) = fixture();
    let mut t = trigger();
    t["conditions"] = json!({});
    c.combat_rules.as_mut().unwrap().triggers = vec![serde_json::from_value(t).unwrap()];
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Member(1)),
        (0, false),
        &c,
        0,
        &mut e,
    );
    assert_eq!(fired(&e), 0);
    assert!(status(&w, "hit-lock").is_none());
    let r = c.combat_rules.as_mut().unwrap();
    r.max_triggers_per_step = 1;
    r.triggers.push(r.triggers[0].clone());
    r.triggers[1].id = "second".into();
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (0, false),
        &c,
        100,
        &mut e,
    );
    assert_eq!(fired(&e), 1);
    assert!(status(&w, "hit-lock").is_some());
}

#[test]
fn hit_status_slow_stack_transfer_preserves_group_identity_expiry_and_derived_strength() {
    let (c, mut w) = fixture();
    for n in 0..2 {
        give(
            &mut w,
            &c,
            Owner::Npc(0),
            Owner::Member(1),
            "hit-mark",
            n * 100,
        );
    }
    let before = status(&w, "hit-mark").unwrap().clone();
    let effect = serde_json::from_value(
        json!({"kind":"transfer_statuses","from":"other","disposition":"harmful","max_count":1}),
    )
    .unwrap();
    w.execute_status_operation(
        (Owner::Member(0), Owner::Member(1)),
        &effect,
        &c,
        200,
        &mut vec![],
    );
    let after = status(&w, "hit-mark").unwrap();
    assert_eq!(
        after,
        &StatusView {
            retained_source: None,
            base_modifiers: vec![],
            stack_counter: None,
            target: status_entity(&w.members[0].reference.entity),
            ..before.clone()
        }
    );
    assert_eq!(w.statuses.movement_factor_bps(&after.target, 200), 8400);
    assert_eq!(w.statuses.movement_factor_bps(&before.target, 200), 10000);
}
#[test]
fn hit_status_successful_conversion_starts_icd_and_does_not_consume_next_threshold_early() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers[0].cooldown_ms = 1000;
    for n in 0..3 {
        strike(&mut w, &c, n * 100, 0);
    }
    let first = status(&w, "hit-lock").unwrap().clone();
    for n in 3..6 {
        assert_eq!(fired(&strike(&mut w, &c, n * 100, 0)), 0);
    }
    assert_eq!(status(&w, "hit-mark").unwrap().stacks, Some(3));
    assert_eq!(status(&w, "hit-lock"), Some(&first));
    assert_eq!(fired(&strike(&mut w, &c, 1200, 0)), 1);
    assert!(status(&w, "hit-mark").is_none());
    assert_eq!(status(&w, "hit-lock").unwrap().applied_at_ms, 1200);
}
