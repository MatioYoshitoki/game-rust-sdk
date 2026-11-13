use super::*;
fn add(c: &mut Config, v: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(v).unwrap());
}
fn original() -> Value {
    json!({"id":"hold","mechanism":"timed_compound_modifier","flags":["stun","root"],"modifiers":[{"attribute":"def","modifier_bps":-2000}],"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true})
}
fn softened() -> Value {
    json!({"id":"soft","mechanism":"timed_compound_modifier","modifiers":[{"attribute":"movement_slow","modifier_bps":-5000},{"attribute":"basic_interval_delay","modifier_bps":1500}],"duration_ms":90,"duplicate_policy":"refresh","source_scoped":true})
}
fn policy(c: &mut Config) {
    let r = c.combat_rules.as_mut().unwrap();
    r.profiles
        .insert("resistant".into(), r.profiles["npc"].clone());
    r.control_policies.insert(
        "resistant".into(),
        serde_json::from_value(json!({"max_duration_ms":1800,"replacements":{"hold":"soft"}}))
            .unwrap(),
    );
}
fn fixture() -> (Config, World) {
    let (mut c, w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    add(&mut c, original());
    add(&mut c, softened());
    operation_policy(&mut c, "hold", "harmful", true);
    operation_policy(&mut c, "soft", "harmful", true);
    policy(&mut c);
    c.combat_rules
        .as_mut()
        .unwrap()
        .npc_profiles
        .insert(w.npcs[0].template_id.clone(), "resistant".into());
    (c, w)
}
fn apply(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str, now: u64) -> Events {
    let target = w.owner_entity(target);
    bind(w, c, source, target, id, now)
}
fn view(w: &World, id: &str) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == id)
        .unwrap()
        .clone()
}
fn extra(c: &mut Config, id: &str, attribute: &str, bps: i32, duration: u64) {
    add(
        c,
        json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":attribute,"modifier_bps":bps,"duration_ms":duration,"duplicate_policy":"refresh","source_scoped":true}),
    );
}
#[test]
fn control_policy_loader_roundtrips_and_rejects_invalid_references_carriers_caps_and_versions() {
    let p = Package::new();
    let mut v = document();
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .extend([original(), softened()]);
    v["combat_rules"]["status_policies"] = json!({"hold":{"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]},"soft":{"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}});
    v["combat_rules"]["control_policies"] =
        json!({"npc":{"max_duration_ms":1800,"replacements":{"hold":"soft"}}});
    p.write(&v);
    let loaded = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(
            &loaded
                .default_config()
                .combat_rules
                .as_ref()
                .unwrap()
                .control_policies
        )
        .unwrap(),
        v["combat_rules"]["control_policies"]
    );
    for case in 0..10 {
        let mut b = v.clone();
        match case {
            0 => b["mechanism_version"] = json!(104),
            1 => {
                b["combat_rules"]["control_policies"] = json!({"missing":{"max_duration_ms":1800}})
            }
            2 => b["combat_rules"]["control_policies"]["npc"] = json!({}),
            3 => b["combat_rules"]["control_policies"]["npc"]["max_duration_ms"] = json!(0),
            4 => b["combat_rules"]["control_policies"]["npc"]["max_duration_ms"] = Value::Null,
            5 => {
                b["combat_rules"]["control_policies"]["npc"]["replacements"] =
                    json!({"missing":"soft"})
            }
            6 => {
                b["combat_rules"]["control_policies"]["npc"]["replacements"] =
                    json!({"soft":"hold"})
            }
            7 => {
                b["combat_rules"]["control_policies"]["npc"]["replacements"] =
                    json!({"hold":"ward"})
            }
            8 => b["combat_rules"]["status_policies"]["soft"]["disposition"] = json!("beneficial"),
            _ => {
                b["combat_rules"]["status_cleanses"] = json!({"soft":{"on_expired":{"disposition":"harmful","max_count":1,"scope":"all"}}})
            }
        }
        p.write(&b);
        assert!(p.load().is_err(), "{case}");
    }
}
#[test]
fn control_policy_replaces_complete_compound_identity_and_preserves_requested_duration() {
    let (c, mut w) = fixture();
    let def = w.npcs[0].profile.stats["def"];
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    let s = view(&w, "soft");
    assert!(s.flags.is_empty());
    assert_eq!((s.applied_at_ms, s.expires_at_ms), (0, 1800));
    assert_eq!(s.source.id, "P00");
    assert_eq!(
        w.statuses.defense(
            &s.target,
            def,
            0,
            c.pve.as_ref().unwrap().status_rules.as_ref()
        ),
        def
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "hold")
    );
    assert_eq!(w.npc_movement_factor(0, &c, 0), 0.5);
}
#[test]
fn control_policy_plain_profile_keeps_original_control_while_cap_only_limits_duration() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().npc_profiles.clear();
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    assert_eq!(view(&w, "hold").expires_at_ms, 3000);
    w.statuses = Statuses::default();
    c.combat_rules.as_mut().unwrap().control_policies.insert(
        "npc".into(),
        serde_json::from_value(json!({"max_duration_ms":1800})).unwrap(),
    );
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    assert_eq!(view(&w, "hold").expires_at_ms, 1800);
}
#[test]
fn control_policy_scalar_control_uses_the_same_mapping_without_business_id_branches() {
    let (mut c, mut w) = fixture();
    *c.pve_mut().status_definitions.iter_mut().find(|d|d.id=="hold").unwrap()=serde_json::from_value(json!({"id":"hold","mechanism":"timed_control","attribute":"silence","duration_ms":2400,"duplicate_policy":"refresh","source_scoped":true})).unwrap();
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    assert_eq!(view(&w, "soft").expires_at_ms, 1800);
}
#[test]
fn control_policy_numeric_replacement_ignores_first_immunity_without_spending_it() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"immune","mechanism":"timed_control","attribute":"first_control_immunity","duration_ms":4000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    apply(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "immune", 0);
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 30);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "immune")
    );
    assert_eq!(view(&w, "soft").applied_at_ms, 30);
}
#[test]
fn control_policy_normal_immunity_does_not_prevent_noncontrol_numeric_replacement() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"immune","mechanism":"timed_control","attribute":"control_immunity","duration_ms":4000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    apply(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "immune", 0);
    let e = apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 30);
    assert!(!e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusRejected { .. }
        }
    )));
    assert_eq!(view(&w, "soft").expires_at_ms, 1830);
}
#[test]
fn control_policy_member_targets_resolve_by_their_own_profile_and_keep_pending_skill() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .member_profiles
        .insert("P00".into(), "resistant".into());
    w.action(
        0,
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "pending",
        &c,
        0,
    )
    .unwrap();
    let commits = w.members[0].pending.as_ref().unwrap().commits;
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), "hold", 0);
    assert_eq!(w.members[0].pending.as_ref().unwrap().commits, commits);
    assert!(!w.skill_silenced(0, 0));
    assert_eq!(view(&w, "soft").target.id, "P00");
}
#[test]
fn control_policy_original_friendly_or_self_hard_control_cannot_be_laundered_into_a_debuff() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .member_profiles
        .insert("P01".into(), "resistant".into());
    for target in [Owner::Member(0), Owner::Member(1)] {
        apply(&mut w, &c, Owner::Member(0), target, "hold", 0);
    }
    assert!(w.statuses.entries().is_empty());
}
#[test]
fn control_policy_actual_category_block_is_checked_after_resolution() {
    let (mut c, mut w) = fixture();
    extra(&mut c, "seal", "def", 1, 4000);
    operation_policy(&mut c, "seal", "beneficial", false);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("seal")
        .unwrap()
        .blocks = vec![crate::config::StatusDisposition::Harmful];
    apply(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "seal", 0);
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 30);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "soft")
    );
}
#[test]
fn control_policy_capacity_and_refresh_preflight_use_resolved_identity() {
    let (mut c, mut w) = fixture();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    let first = view(&w, "soft");
    assert!(w.can_owned_statuses(Owner::Member(0), &w.npcs[0].entity, &["hold".into()], &c));
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 300);
    assert_eq!(view(&w, "soft").instance_id, first.instance_id);
    assert_eq!(view(&w, "soft").expires_at_ms, 2100);
}
#[test]
fn control_policy_capacity_failure_does_not_install_control_or_remove_immunity() {
    let (mut c, mut w) = fixture();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    apply(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "ward", 0);
    let before = w.statuses.entries().to_vec();
    assert!(!w.can_owned_statuses(Owner::Member(0), &w.npcs[0].entity, &["hold".into()], &c));
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 30);
    assert_eq!(w.statuses.entries(), before);
}
#[test]
fn control_policy_same_batch_resolution_merges_once_with_longest_request() {
    let (mut c, mut w) = fixture();
    let mut second = original();
    second["id"] = json!("hold-two");
    second["duration_ms"] = json!(1000);
    add(&mut c, second);
    c.combat_rules
        .as_mut()
        .unwrap()
        .control_policies
        .get_mut("resistant")
        .unwrap()
        .replacements
        .insert("hold-two".into(), "soft".into());
    let target = w.npcs[0].entity.clone();
    let mut e = vec![];
    w.apply_owned_statuses(
        Owner::Member(0),
        (target, 1000),
        &["hold".into(), "hold-two".into()],
        &c,
        0,
        &mut e,
    );
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(view(&w, "soft").expires_at_ms, 1800);
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusApplied { .. }
                }
            ))
            .count(),
        1
    );
}
#[test]
fn control_policy_actual_kind_duration_scaling_does_not_apply_control_reduction_to_numeric_replacement()
 {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"shorten","mechanism":"timed_compound_modifier","duration_ms":4000,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"control_duration_reduction","modifier_bps":4000},{"attribute":"harmful_duration_reduction","modifier_bps":2000}]}),
    );
    apply(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "shorten", 0);
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    assert_eq!(view(&w, "soft").expires_at_ms, 1440);
}
#[test]
fn control_policy_ignore_keeps_identity_and_original_deadline() {
    let (mut c, mut w) = fixture();
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "soft")
        .unwrap()
        .duplicate_policy = crate::config::DuplicatePolicy::Ignore;
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    let original = view(&w, "soft");
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 300);
    assert_eq!(view(&w, "soft"), original);
}
#[test]
fn control_policy_transfer_cannot_bypass_replacement_or_clip_remaining_duration() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .member_profiles
        .insert("P00".into(), "resistant".into());
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(1), "hold", 0);
    let old = view(&w, "hold");
    let effect = serde_json::from_value(
        json!({"kind":"transfer_statuses","from":"other","disposition":"harmful","max_count":1}),
    )
    .unwrap();
    let mut e = vec![];
    w.execute_status_operation(
        (Owner::Member(0), Owner::Member(1)),
        &effect,
        &c,
        300,
        &mut e,
    );
    assert!(e.is_empty());
    assert_eq!(view(&w, "hold"), old);
    c.combat_rules
        .as_mut()
        .unwrap()
        .control_policies
        .get_mut("resistant")
        .unwrap()
        .replacements
        .clear();
    w.execute_status_operation(
        (Owner::Member(0), Owner::Member(1)),
        &effect,
        &c,
        300,
        &mut e,
    );
    assert!(e.is_empty());
    w.execute_status_operation(
        (Owner::Member(0), Owner::Member(1)),
        &effect,
        &c,
        1500,
        &mut e,
    );
    assert_eq!(view(&w, "hold").target.id, "P00");
    assert_eq!(view(&w, "hold").expires_at_ms, 3000);
}
#[test]
fn control_policy_basic_interval_delay_is_exact_and_composes_with_attack_speed() {
    for accelerated in [false, true] {
        let (mut c, mut w) = fixture();
        apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
        if accelerated {
            extra(&mut c, "fast", "attack_speed", 10000, 4000);
            apply(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "fast", 0);
        }
        w.start_attack_clock(Owner::Npc(0), 1000, &c, 0);
        assert_eq!(w.npcs[0].next_attack, if accelerated { 575 } else { 1150 });
    }
}
#[test]
fn control_policy_interval_expiry_and_many_small_steps_preserve_completed_work() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .control_policies
        .get_mut("resistant")
        .unwrap()
        .max_duration_ms = Some(500);
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    w.start_attack_clock(Owner::Npc(0), 1000, &c, 0);
    assert_eq!(w.npcs[0].next_attack, 1066);
    for now in (30..=480).step_by(30) {
        w.advance_attack_clocks(&c, now);
        assert_eq!(w.npcs[0].next_attack, 1066);
    }
    w.reconcile_caps(&c, 500);
    w.prepare_periodic(500, &mut vec![]);
    w.advance_attack_clocks(&c, 500);
    assert_eq!(w.npcs[0].next_attack, 1066);
}
#[test]
fn control_policy_source_exit_cleans_numeric_replacement_and_recomputes_interval() {
    let (c, mut w) = fixture();
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), "hold", 0);
    w.start_attack_clock(Owner::Npc(0), 1000, &c, 0);
    w.action(0, &Action::Leave {}, "source-left", &c, 300)
        .unwrap();
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "soft")
    );
    w.advance_attack_clocks(&c, 300);
    assert_eq!(w.npcs[0].next_attack, 1040);
}
