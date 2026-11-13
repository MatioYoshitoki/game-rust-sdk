use super::*;
fn add(c: &mut Config, id: &str, attr: &str) {
    let lock = attr == "spirit_recovery_lock";
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":if lock{"timed_attribute_modifier"}else{"timed_control"},"attribute":attr,"modifier_bps":if lock{-10000}else{0},"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
}
fn group(c: &mut Config, id: &str, flags: Value, mods: Value) {
    let mut d = json!({"id":id,"mechanism":"timed_compound_modifier","duration_ms":900,"duplicate_policy":"refresh","source_scoped":true,"flags":flags});
    if !mods.as_array().unwrap().is_empty() {
        d["modifiers"] = mods;
    }
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
}
fn setup_control(attr: &str) -> (Config, World) {
    let (mut c, w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    add(&mut c, "control-gift", attr);
    operation_policy(
        &mut c,
        "control-gift",
        if ["silence", "root", "stun", "spirit_recovery_lock"].contains(&attr) {
            "harmful"
        } else {
            "beneficial"
        },
        true,
    );
    (c, w)
}
fn effect(benefit: bool) -> crate::config::TriggerEffect {
    serde_json::from_value(json!({"kind":"transfer_statuses","from":"other","disposition":if benefit{"beneficial"}else{"harmful"},"max_count":2})).unwrap()
}
fn bind_to(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str) {
    let target = w.owner_entity(target);
    bind(w, c, source, target, id, 0);
}
fn transfer(w: &mut World, c: &Config, benefit: bool, now: u64) -> Events {
    let mut e = vec![];
    w.execute_status_operation(
        (Owner::Member(0), Owner::Member(1)),
        &effect(benefit),
        c,
        now,
        &mut e,
    );
    e
}
fn gift(w: &World) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "control-gift")
        .unwrap()
        .clone()
}
fn prepare(w: &mut World, c: &mut Config) {
    c.skill = serde_json::from_value(skill("projectile")).unwrap();
    c.additional_skills.clear();
    w.action(
        0,
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "preparing",
        c,
        0,
    )
    .unwrap();
}
#[test]
fn control_transfer_loader_accepts_scalar_compound_and_lock_but_requires_valid_permissions() {
    let p = Package::new();
    for attr in [
        "silence",
        "root",
        "stun",
        "control_immunity",
        "first_control_immunity",
        "damage_immunity",
        "spirit_recovery_lock",
    ] {
        let mut v = document();
        v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({"id":"control-gift","mechanism":if attr=="spirit_recovery_lock"{"timed_attribute_modifier"}else{"timed_control"},"attribute":attr,"modifier_bps":if attr=="spirit_recovery_lock"{-10000}else{0},"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}));
        if attr != "spirit_recovery_lock" {
            v["pve"]["status_definitions"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove("modifier_bps");
        }
        v["combat_rules"]["status_policies"] = json!({"control-gift":{"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}});
        p.write(&v);
        let c = p.load().unwrap();
        assert_eq!(
            serde_json::to_value(c.default_config()).unwrap()["combat_rules"]["status_policies"],
            v["combat_rules"]["status_policies"]
        );
        v["combat_rules"]["status_policies"]["control-gift"]["dispellable"] = json!(false);
        p.write(&v);
        assert!(p.load().is_err());
    }
    let mut v = document();
    v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({"id":"control-gift","mechanism":"timed_compound_modifier","flags":["root","silence"],"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}));
    v["combat_rules"]["status_policies"] = json!({"control-gift":{"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}});
    p.write(&v);
    p.load().unwrap();
    v["mechanism_version"] = json!(104);
    p.write(&v);
    assert!(p.load().is_err());
}
#[test]
fn control_transfer_keeps_full_identity_remaining_duration_and_does_not_apply_target_shortening_twice()
 {
    let (mut c, mut w) = setup_control("root");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    let before = gift(&w);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"shorten","mechanism":"timed_attribute_modifier","attribute":"control_duration_reduction","modifier_bps":4000,"duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    bind_to(&mut w, &c, Owner::Member(0), Owner::Member(0), "shorten");
    transfer(&mut w, &c, false, 600);
    let mut expected = before;
    expected.target = status_entity(&w.members[0].reference.entity);
    assert_eq!(gift(&w), expected);
    w.prepare_periodic(900, &mut vec![]);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "control-gift")
    );
}
#[test]
fn control_transfer_silence_interrupts_new_preparation_after_transfer_fact_without_paying() {
    let (mut c, mut w) = setup_control("silence");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    prepare(&mut w, &mut c);
    let before = (
        w.members[0].hp,
        w.members[0].spirit,
        w.members[0].cooldowns.clone(),
    );
    let e = transfer(&mut w, &c, false, 30);
    assert!(w.members[0].pending.is_none());
    assert_eq!(
        (
            w.members[0].hp,
            w.members[0].spirit,
            w.members[0].cooldowns.clone()
        ),
        before
    );
    let moved = e
        .iter()
        .position(|(i, e)| {
            *i == Some(0)
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::StatusTransferred { .. }
                    }
                )
        })
        .unwrap();
    let interrupted = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::ActionInterrupted
                }
            )
        })
        .unwrap();
    assert!(moved < interrupted);
}
#[test]
fn control_transfer_root_stops_movement_but_keeps_a_nonmovement_skill_preparation() {
    let (mut c, mut w) = setup_control("root");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    prepare(&mut w, &mut c);
    let id = w.members[0].pending.as_ref().unwrap().id.clone();
    w.members[0].movement = [1., 0.];
    w.members[0].path.push([500., 500.]);
    transfer(&mut w, &c, false, 30);
    assert_eq!(w.members[0].movement, [0., 0.]);
    assert!(w.members[0].path.is_empty());
    assert_eq!(w.members[0].pending.as_ref().unwrap().id, id);
}
#[test]
fn control_transfer_stun_cancels_preparation_movement_and_does_not_resurrect_old_action() {
    let (mut c, mut w) = setup_control("stun");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    prepare(&mut w, &mut c);
    w.members[0].movement = [1., 0.];
    transfer(&mut w, &c, false, 30);
    assert!(w.members[0].pending.is_none());
    assert_eq!(w.members[0].movement, [0., 0.]);
    w.statuses.remove(
        StatusRemovalReason::Cleansed,
        |s| s.definition_id == "control-gift",
        &mut vec![],
    );
    assert!(w.members[0].pending.is_none());
}
#[test]
fn control_transfer_silence_ends_channel_but_preserves_paid_inflight_pulse() {
    let (mut c, mut w) = setup_control("silence");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    c.skill = serde_json::from_value(skill("projectile")).unwrap();
    c.skill.channel =
        serde_json::from_value(json!({"duration_ms":360,"interval_ms":90,"spirit_per_pulse":3}))
            .unwrap();
    commit(&mut w, &c, "projectile");
    w.tick(&c, 120);
    assert!(w.members[0].channel.is_some());
    let shots = w.shots.len();
    let spirit = w.members[0].spirit;
    let e = transfer(&mut w, &c, false, 150);
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.shots.len(), shots);
    assert_eq!(w.members[0].spirit, spirit);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ChannelEnded {
                reason: ChannelEndReason::Interrupted,
                ..
            }
        }
    )));
}
#[test]
fn control_transfer_npc_silence_and_stun_clear_casting_while_root_keeps_casting() {
    for attr in ["silence", "stun", "root"] {
        let (c, _) = setup_control(attr);
        let (_, mut w) = super::super::combat_tests::setup(2);
        bind_to(&mut w, &c, Owner::Member(0), Owner::Npc(1), "control-gift");
        w.npcs[0].casting = Some(super::npc_skills::NpcCast::legacy(150, [1., 0.]));
        w.execute_status_operation(
            (Owner::Npc(0), Owner::Npc(1)),
            &effect(false),
            &c,
            30,
            &mut vec![],
        );
        assert_eq!(w.npcs[0].casting.is_none(), attr != "root");
        if attr == "root" {
            assert_eq!(w.npc_movement_factor(0, &c, 30), 0.);
        }
    }
}
#[test]
fn control_transfer_normal_or_first_immunity_rejects_without_consuming_or_interrupting() {
    for attr in ["control_immunity", "first_control_immunity"] {
        let (mut c, mut w) = setup_control("silence");
        add(&mut c, "guard", attr);
        bind_to(&mut w, &c, Owner::Member(0), Owner::Member(0), "guard");
        bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
        prepare(&mut w, &mut c);
        let before = w.statuses.entries().to_vec();
        let e = transfer(&mut w, &c, false, 30);
        assert_eq!(w.statuses.entries(), before);
        assert!(w.members[0].pending.is_some());
        assert!(e.is_empty());
        assert!(matches!(
            w.plan_status_operation(
                &w.statuses,
                (Owner::Member(0), Owner::Member(1)),
                &effect(false),
                &c,
                30
            ),
            Err(Error::Immune)
        ));
    }
}
#[test]
fn control_transfer_same_batch_immunity_is_checked_in_the_final_preview() {
    let (mut c, mut w) = setup_control("stun");
    add(&mut c, "guard", "first_control_immunity");
    operation_policy(&mut c, "guard", "harmful", true);
    bind_to(&mut w, &c, Owner::Member(1), Owner::Member(1), "guard");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    let before = w.statuses.entries().to_vec();
    transfer(&mut w, &c, false, 30);
    assert_eq!(w.statuses.entries(), before);
}
#[test]
fn control_transfer_explicit_condition_consumption_can_release_first_immunity_before_preflight() {
    let (mut c, mut w) = setup_control("silence");
    add(&mut c, "guard", "first_control_immunity");
    bind_to(&mut w, &c, Owner::Member(0), Owner::Member(0), "guard");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    operation_trigger(
        &mut c,
        serde_json::to_value(effect(false)).unwrap(),
        Some(json!({"subject":"owner","definition_id":"guard","at_least":1,"consume":1})),
    );
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DamageDealt,
        (Owner::Member(0), Owner::Member(1)),
        (10, false),
        &c,
        30,
        &mut e,
    );
    assert_eq!(
        gift(&w).target,
        status_entity(&w.members[0].reference.entity)
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "guard")
    );
}
#[test]
fn control_transfer_compound_first_immunity_is_consumed_once_by_a_later_real_control() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    group(
        &mut c,
        "guard",
        json!(["first_control_immunity"]),
        json!([{"attribute":"max_hp","modifier_bps":10000}]),
    );
    operation_policy(&mut c, "guard", "beneficial", true);
    add(&mut c, "control-gift", "stun");
    bind_to(&mut w, &c, Owner::Member(1), Owner::Member(1), "guard");
    transfer(&mut w, &c, true, 30);
    assert_eq!(
        w.entity_max_hp(&w.members[0].reference.entity, &c, 30),
        Some(2000)
    );
    let target = w.members[0].reference.entity.clone();
    let e = bind(&mut w, &c, Owner::Npc(0), target, "control-gift", 60);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "guard")
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "control-gift")
    );
    assert_eq!(
        w.entity_max_hp(&w.members[0].reference.entity, &c, 60),
        Some(1000)
    );
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusRejected {
                reason: Error::Immune,
                ..
            }
        }
    )));
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), own, "control-gift", 90);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "control-gift")
    );
}
#[test]
fn control_transfer_compound_normal_immunity_precedes_first_without_consuming_the_instance() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    group(
        &mut c,
        "guard",
        json!(["control_immunity", "first_control_immunity"]),
        json!([]),
    );
    operation_policy(&mut c, "guard", "beneficial", true);
    add(&mut c, "control-gift", "stun");
    bind_to(&mut w, &c, Owner::Member(1), Owner::Member(1), "guard");
    transfer(&mut w, &c, true, 30);
    let before = w.statuses.entries().to_vec();
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), own, "control-gift", 60);
    assert_eq!(w.statuses.entries(), before);
}
#[test]
fn control_transfer_scalar_immunity_does_not_interrupt_and_can_cross_faction_explicitly() {
    let (mut c, mut w) = setup_control("damage_immunity");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "control-gift");
    prepare(&mut w, &mut c);
    let mut e = vec![];
    w.execute_status_operation(
        (Owner::Member(0), Owner::Npc(0)),
        &effect(true),
        &c,
        30,
        &mut e,
    );
    assert!(w.members[0].pending.is_some());
    assert!(w.entity_has_status(Owner::Member(0), StatusAttribute::DamageImmunity, 30));
    assert!(!w.entity_has_status(Owner::Npc(0), StatusAttribute::DamageImmunity, 30));
    assert!(e.iter().all(|(_, e)| !matches!(
        e,
        EventV4::Private {
            event: Event::ActionInterrupted
        }
    )));
}
#[test]
fn control_transfer_hard_controls_cannot_move_to_original_source_or_its_friends() {
    let (c, mut w) = setup_control("stun");
    bind_to(&mut w, &c, Owner::Member(0), Owner::Npc(0), "control-gift");
    assert!(matches!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Member(1), Owner::Npc(0)),
            &effect(false),
            &c,
            30
        ),
        Err(Error::InvalidTarget)
    ));
    assert!(matches!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(0)),
            &effect(false),
            &c,
            30
        ),
        Err(Error::InvalidTarget)
    ));
}
#[test]
fn control_transfer_spirit_lock_keeps_member_target_rule_without_using_control_immunity() {
    let (mut c, mut w) = setup_control("spirit_recovery_lock");
    add(&mut c, "guard", "control_immunity");
    bind_to(&mut w, &c, Owner::Member(0), Owner::Member(0), "guard");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    transfer(&mut w, &c, false, 30);
    w.members[0].spirit = 200;
    w.members[1].spirit = 200;
    assert_eq!(w.members[0].spirit_recovery_gap(&w.statuses, 30, 300), 0);
    assert_eq!(w.members[1].spirit_recovery_gap(&w.statuses, 30, 300), 100);
    let (c, mut w) = setup_control("spirit_recovery_lock");
    bind_to(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Member(0),
        "control-gift",
    );
    let op = serde_json::from_value(
        json!({"kind":"transfer_statuses","from":"owner","disposition":"harmful","max_count":1}),
    )
    .unwrap();
    assert!(matches!(
        w.plan_status_operation(&w.statuses, (Owner::Member(0), Owner::Npc(0)), &op, &c, 30),
        Err(Error::InvalidTarget)
    ));
}
#[test]
fn control_transfer_capacity_failure_keeps_partial_condition_stack_and_ready_icd_for_retry() {
    let (mut c, mut w) = setup_control("silence");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    charge_definition(&mut c, "charge");
    for _ in 0..2 {
        bind_to(&mut w, &c, Owner::Member(0), Owner::Member(0), "charge");
    }
    operation_trigger(
        &mut c,
        serde_json::to_value(effect(false)).unwrap(),
        Some(json!({"subject":"owner","definition_id":"charge","at_least":2,"consume":1})),
    );
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let before = w.statuses.entries().to_vec();
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DamageDealt,
        (Owner::Member(0), Owner::Member(1)),
        (10, false),
        &c,
        30,
        &mut e,
    );
    assert_eq!(w.statuses.entries(), before);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 2;
    w.combat_fact(
        crate::config::CombatFact::DamageDealt,
        (Owner::Member(0), Owner::Member(1)),
        (10, false),
        &c,
        60,
        &mut e,
    );
    assert_eq!(
        gift(&w).target,
        status_entity(&w.members[0].reference.entity)
    );
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "charge")
            .unwrap()
            .stacks,
        Some(1)
    );
}
#[test]
fn control_transfer_protection_or_stale_origin_rejects_without_touching_pending_action() {
    for case in 0..3 {
        let (mut c, mut w) = setup_control("stun");
        bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
        prepare(&mut w, &mut c);
        if case == 1 {
            w.npcs[0].entity.generation += 1;
        } else if case == 2 {
            w.npcs[0].returning = true;
        } else {
            w.members[0].protected = true;
        }
        let before = w.statuses.entries().to_vec();
        transfer(&mut w, &c, false, 30);
        assert_eq!(w.statuses.entries(), before);
        assert!(w.members[0].pending.is_some());
    }
}
#[test]
fn control_transfer_compound_markers_keep_one_identity_and_clamp_caps_then_interrupt_once() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    group(
        &mut c,
        "control-gift",
        json!(["silence", "root", "stun"]),
        json!([{"attribute":"max_hp","modifier_bps":-5000}]),
    );
    operation_policy(&mut c, "control-gift", "harmful", true);
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    prepare(&mut w, &mut c);
    let before = gift(&w);
    let e = transfer(&mut w, &c, false, 30);
    assert_eq!(w.members[0].hp, 500);
    assert_eq!(gift(&w).flags, before.flags);
    assert_eq!(gift(&w).instance_id, before.instance_id);
    assert_eq!(
        e.iter()
            .filter(|(i, e)| *i == Some(0)
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionInterrupted
                    }
                ))
            .count(),
        1
    );
}
#[test]
fn control_transfer_old_holder_exit_preserves_nonowned_instance_then_source_exit_cleans_it() {
    let (c, mut w) = setup_control("root");
    bind_to(&mut w, &c, Owner::Npc(0), Owner::Member(1), "control-gift");
    transfer(&mut w, &c, false, 30);
    let old = w.members[1].reference.entity.clone();
    w.clear_owned_statuses(&old, StatusRemovalReason::TargetInvalid, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    let source = w.npcs[0].entity.clone();
    w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(w.statuses.entries().is_empty());
}
#[test]
fn control_transfer_existing_numeric_movement_contract_is_not_reclassified_as_hard_control() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"numeric-slow","mechanism":"timed_attribute_modifier","attribute":"movement_slow","modifier_bps":2000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    operation_policy(&mut c, "numeric-slow", "harmful", true);
    bind_to(&mut w, &c, Owner::Member(0), Owner::Npc(0), "numeric-slow");
    w.execute_status_operation(
        (Owner::Member(0), Owner::Npc(0)),
        &effect(false),
        &c,
        30,
        &mut vec![],
    );
    assert_eq!(
        w.statuses.entries()[0].target,
        status_entity(&w.members[0].reference.entity)
    );
}
