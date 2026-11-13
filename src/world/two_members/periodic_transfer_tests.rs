use super::*;
fn setup_periodic(heal: bool) -> (Config, World) {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    let mut d = json!({"id":"periodic-gift","mechanism":if heal{"periodic_heal"}else{"periodic_damage"},"duration_ms":1200,"tick_interval_ms":300,"duplicate_policy":"refresh","source_scoped":true});
    d[if heal { "heal_bps" } else { "damage_bps" }] = json!(1000);
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
    operation_policy(
        &mut c,
        "periodic-gift",
        if heal { "beneficial" } else { "harmful" },
        true,
    );
    w.members[1].facts.hp_points = 1000;
    w.members[1].hp = 1000;
    (c, w)
}
fn gift(w: &World) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "periodic-gift")
        .unwrap()
        .clone()
}
fn bind_gift(w: &mut World, c: &Config, source: Owner, target: Owner) {
    let target = w.owner_entity(target);
    bind(w, c, source, target, "periodic-gift", 0);
}
fn effect(heal: bool) -> crate::config::TriggerEffect {
    serde_json::from_value(json!({"kind":"transfer_statuses","from":"other","disposition":if heal{"beneficial"}else{"harmful"},"max_count":2})).unwrap()
}
fn move_from(w: &mut World, c: &Config, old: Owner, heal: bool, now: u64) -> Events {
    let mut e = vec![];
    w.execute_status_operation((Owner::Member(0), old), &effect(heal), c, now, &mut e);
    e
}
fn tick(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.advance_periodic(c, now, &mut e);
    e
}
fn raw_amount(e: &Events) -> u32 {
    e.iter()
        .find_map(|(_, e)| match e {
            EventV4::Private {
                event: Event::PeriodicTick { raw_damage, .. },
            } => Some(*raw_damage),
            EventV4::Private {
                event: Event::PeriodicHealTick { raw_heal, .. },
            } => Some(*raw_heal),
            _ => None,
        })
        .unwrap()
}
#[test]
fn periodic_transfer_loader_accepts_native_lifecycle_rules_and_rejects_invalid_permissions() {
    let p = Package::new();
    let mut good = document();
    good["combat_rules"]["status_policies"] = json!({"wound":{"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]},"renewal":{"disposition":"beneficial","dispellable":true,"transferable":true,"blocks":[]}});
    p.write(&good);
    let c = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(c.default_config()).unwrap()["combat_rules"]["status_policies"],
        good["combat_rules"]["status_policies"]
    );
    for case in 0..5 {
        let mut bad = good.clone();
        match case {
            0 => bad["mechanism_version"] = json!(104),
            1 => bad["combat_rules"]["status_policies"]["renewal"]["dispellable"] = json!(false),
            2 => {
                bad["combat_rules"]["status_cleanses"] = json!({"renewal":{"on_tick":{"disposition":"harmful","max_count":1,"scope":"all"}}})
            }
            3 => {
                bad["combat_rules"]["status_policies"]["missing"] =
                    good["combat_rules"]["status_policies"]["renewal"].clone()
            }
            _ => {
                bad["combat_rules"]["status_windows"] = json!({"renewal":{"basis":"none","include_periodic":false,"record_cap":0,"refresh":"reset","maximum_sampling":"snapshot","on_expired":true,"on_consumed":false,"hp":{"fixed":1,"recorded_bps":0,"max_resource_bps":0,"cap_max_resource_bps":10000}}})
            }
        }
        p.write(&bad);
        assert_eq!(p.load().is_ok(), matches!(case, 2 | 4), "{case}");
    }
}
#[test]
fn periodic_transfer_dot_preserves_history_and_next_tick_changes_only_the_holder() {
    let (c, mut w) = setup_periodic(false);
    bind_gift(&mut w, &c, Owner::Npc(0), Owner::Member(1));
    tick(&mut w, &c, 300);
    let before = gift(&w);
    let hp = w.members[1].hp;
    let e = move_from(&mut w, &c, Owner::Member(1), false, 450);
    let mut expected = before.clone();
    expected.target = status_entity(&w.members[0].reference.entity);
    assert_eq!(gift(&w), expected);
    assert!(e.iter().all(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusTransferred { .. }
        }
    )));
    tick(&mut w, &c, 600);
    assert_eq!(w.members[1].hp, hp);
    assert!(w.members[0].hp < 1000);
    assert_eq!(gift(&w).periodic.unwrap().ticks_done, 2);
}
#[test]
fn periodic_transfer_stolen_hot_uses_explicit_policy_while_direct_enemy_heal_still_rejects() {
    let (mut c, mut w) = setup_periodic(true);
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), own, "periodic-gift", 0);
    assert!(w.statuses.entries().is_empty());
    bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
    w.members[0].hp = 900;
    move_from(&mut w, &c, Owner::Npc(0), true, 150);
    tick(&mut w, &c, 300);
    assert_eq!(w.members[0].hp, 1000);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("periodic-gift")
        .unwrap()
        .transferable = false;
    w.members[0].hp = 900;
    tick(&mut w, &c, 600);
    assert_eq!(w.members[0].hp, 900);
}
#[test]
fn periodic_transfer_four_dot_sampling_modes_keep_origin_or_read_new_holder_at_original_tick() {
    for mode in [
        "source_snapshot",
        "target_snapshot",
        "source_live",
        "target_live",
    ] {
        let (mut c, mut w) = setup_periodic(false);
        c.combat_rules.as_mut().unwrap().periodic_sampling.insert(
            "periodic-gift".into(),
            serde_json::from_value(json!(mode)).unwrap(),
        );
        let source_at = w
            .entity_attack(&status_entity(&w.npcs[0].entity), &c, 0)
            .unwrap();
        let old_at = w.members[1].facts.atk_points;
        bind_gift(&mut w, &c, Owner::Npc(0), Owner::Member(1));
        Arc::make_mut(&mut w.npcs[0].profile)
            .stats
            .insert("atk".into(), 500);
        w.members[0].facts.atk_points = 300;
        move_from(&mut w, &c, Owner::Member(1), false, 150);
        let events = tick(&mut w, &c, 300);
        let attack = match mode {
            "source_snapshot" => source_at,
            "target_snapshot" => old_at,
            "source_live" => 500,
            _ => 300,
        };
        assert_eq!(raw_amount(&events), attack / 10, "{mode}");
    }
}
#[test]
fn periodic_transfer_hot_snapshot_keeps_original_maximum_and_live_reads_new_maximum_attack() {
    for mode in [
        "source_snapshot",
        "target_snapshot",
        "source_live",
        "target_live",
    ] {
        let (mut c, mut w) = setup_periodic(true);
        let r = c.combat_rules.as_mut().unwrap();
        r.periodic_sampling.insert(
            "periodic-gift".into(),
            serde_json::from_value(json!(mode)).unwrap(),
        );
        r.healing.insert(
            "periodic-gift".into(),
            serde_json::from_value(
                json!({"fixed":0,"source_attack_bps":10000,"target_max_hp_bps":1000}),
            )
            .unwrap(),
        );
        let source_at = w
            .entity_attack(&status_entity(&w.npcs[0].entity), &c, 0)
            .unwrap();
        let old_at = source_at;
        bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
        w.members[0].facts.hp_points = 2000;
        w.members[0].hp = 1000;
        w.members[0].facts.atk_points = 300;
        Arc::make_mut(&mut w.npcs[0].profile)
            .stats
            .insert("atk".into(), 500);
        move_from(&mut w, &c, Owner::Npc(0), true, 150);
        let events = tick(&mut w, &c, 300);
        let expected = match mode {
            "source_snapshot" => source_at + 100,
            "target_snapshot" => old_at + 100,
            "source_live" => 700,
            _ => 500,
        };
        assert_eq!(raw_amount(&events), expected, "{mode}");
    }
}
#[test]
fn periodic_transfer_dot_invalid_relation_rejects_the_entire_mixed_batch() {
    let (mut c, mut w) = setup_periodic(false);
    bind_gift(&mut w, &c, Owner::Member(0), Owner::Npc(0));
    charge_definition(&mut c, "numeric-gift");
    operation_policy(&mut c, "numeric-gift", "harmful", true);
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), npc, "numeric-gift", 0);
    let before = w.statuses.entries().to_vec();
    assert!(matches!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Member(1), Owner::Npc(0)),
            &effect(false),
            &c,
            150
        ),
        Err(Error::InvalidTarget)
    ));
    w.execute_status_operation(
        (Owner::Member(1), Owner::Npc(0)),
        &effect(false),
        &c,
        150,
        &mut vec![],
    );
    assert_eq!(w.statuses.entries(), before);
}
#[test]
fn periodic_transfer_cannot_reflect_dot_onto_its_original_source() {
    let (c, mut w) = setup_periodic(false);
    bind_gift(&mut w, &c, Owner::Npc(0), Owner::Member(0));
    let e = serde_json::from_value(
        json!({"kind":"transfer_statuses","from":"owner","disposition":"harmful","max_count":2}),
    )
    .unwrap();
    assert!(matches!(
        w.plan_status_operation(&w.statuses, (Owner::Member(0), Owner::Npc(0)), &e, &c, 150),
        Err(Error::InvalidTarget)
    ));
}
#[test]
fn periodic_transfer_dual_resources_and_remaining_shield_belong_to_new_holder_only() {
    let (mut c, mut w) = setup_periodic(true);
    let r = c.combat_rules.as_mut().unwrap();
    r.healing.insert(
        "periodic-gift".into(),
        serde_json::from_value(json!({"fixed":101,"source_attack_bps":0,"target_max_hp_bps":0}))
            .unwrap(),
    );
    r.periodic_recovery.insert("periodic-gift".into(),serde_json::from_value(json!({"spirit":{"fixed":1,"source_attack_bps":0,"target_max_spirit_bps":0},"overheal_spirit":{"bps":5000,"when":"any_excess"}})).unwrap());
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"moved-shield","mechanism":"absorption_shield","shield_bps":2000,"duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "periodic-gift")
        .unwrap()
        .overheal_shield = Some(
        serde_json::from_value(json!({"status_id":"moved-shield","conversion_bps":5000})).unwrap(),
    );
    bind_gift(&mut w, &c, Owner::Member(0), Owner::Member(1));
    tick(&mut w, &c, 300);
    let old_shield = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "moved-shield")
        .unwrap()
        .clone();
    w.members[0].spirit = 296;
    move_from(&mut w, &c, Owner::Member(1), true, 450);
    let events = tick(&mut w, &c, 600);
    assert_eq!(w.members[0].spirit, 300);
    assert_eq!(w.members[1].spirit, 300);
    assert!(w.statuses.entries().contains(&old_shield));
    let new = w
        .statuses
        .entries()
        .iter()
        .find(|s| {
            s.definition_id == "moved-shield"
                && s.target == status_entity(&w.members[0].reference.entity)
        })
        .unwrap();
    assert_eq!(new.shield.as_ref().unwrap().remaining, 47);
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::PeriodicSpiritTick {
                spent_heal: 6,
                remaining_excess: 95,
                ..
            }
        }
    )));
}
#[test]
fn periodic_transfer_collision_failure_keeps_charge_and_icd_until_next_legal_action() {
    let (mut c, mut w) = setup_periodic(true);
    bind_gift(&mut w, &c, Owner::Member(0), Owner::Member(1));
    bind_gift(&mut w, &c, Owner::Member(0), Owner::Member(0));
    charge_definition(&mut c, "charge");
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "charge", 0);
    operation_trigger(
        &mut c,
        transfer_effect(),
        Some(json!({"subject":"owner","definition_id":"charge","at_least":1,"consume":1})),
    );
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
    let own = status_entity(&w.members[0].reference.entity);
    w.statuses.remove(
        StatusRemovalReason::Cleansed,
        |s| s.target == own && s.definition_id == "periodic-gift",
        &mut vec![],
    );
    w.combat_fact(
        crate::config::CombatFact::DamageDealt,
        (Owner::Member(0), Owner::Member(1)),
        (10, false),
        &c,
        60,
        &mut e,
    );
    assert_eq!(gift(&w).target, own);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "charge")
    );
}
#[test]
fn periodic_transfer_protection_skips_without_replaying_or_delaying_original_schedule() {
    let (c, mut w) = setup_periodic(true);
    bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
    move_from(&mut w, &c, Owner::Npc(0), true, 150);
    w.members[0].hp = 800;
    w.members[0].protected = true;
    tick(&mut w, &c, 300);
    assert_eq!(w.members[0].hp, 800);
    assert_eq!(gift(&w).periodic.as_ref().unwrap().ticks_done, 1);
    w.members[0].protected = false;
    tick(&mut w, &c, 600);
    assert_eq!(w.members[0].hp, 900);
    tick(&mut w, &c, 600);
    assert_eq!(w.members[0].hp, 900);
    assert_eq!(gift(&w).periodic.unwrap().next_tick_at_ms, Some(900));
}
#[test]
fn periodic_transfer_original_source_returning_still_skips_a_stolen_hot_tick() {
    let (c, mut w) = setup_periodic(true);
    bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
    move_from(&mut w, &c, Owner::Npc(0), true, 150);
    w.members[0].hp = 800;
    w.npcs[0].returning = true;
    tick(&mut w, &c, 300);
    assert_eq!(w.members[0].hp, 800);
    assert_eq!(gift(&w).periodic.unwrap().ticks_done, 1);
}
#[test]
fn periodic_transfer_source_exit_cleans_new_holder_but_former_unrelated_holder_exit_does_not() {
    let (c, mut w) = setup_periodic(true);
    bind_gift(&mut w, &c, Owner::Member(0), Owner::Member(1));
    move_from(&mut w, &c, Owner::Member(1), true, 150);
    let old = w.members[1].reference.entity.clone();
    w.clear_owned_statuses(&old, StatusRemovalReason::TargetInvalid, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    let source = w.members[0].reference.entity.clone();
    w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(w.statuses.entries().is_empty());
}
#[test]
fn periodic_transfer_new_holder_death_and_source_generation_never_rebind_or_resurrect() {
    for stale in [false, true] {
        let (c, mut w) = setup_periodic(true);
        bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
        move_from(&mut w, &c, Owner::Npc(0), true, 150);
        if stale {
            w.npcs[0].entity.generation += 1;
            w.members[0].hp = 800;
        } else {
            w.members[0].hp = 0;
        }
        tick(&mut w, &c, 300);
        assert_eq!(w.members[0].hp, if stale { 800 } else { 0 });
    }
}
#[test]
fn periodic_transfer_expired_and_late_schedules_do_not_grant_catchup_after_deadline() {
    let (c, mut w) = setup_periodic(true);
    bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
    move_from(&mut w, &c, Owner::Npc(0), true, 1050);
    w.members[0].hp = 500;
    tick(&mut w, &c, 1200);
    assert_eq!(w.members[0].hp, 500);
    assert!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Npc(0), Owner::Member(0)),
            &effect(true),
            &c,
            1200
        )
        .is_err()
    );
}
#[test]
fn periodic_transfer_read_views_and_return_trip_do_not_advance_or_resample() {
    let (c, mut w) = setup_periodic(true);
    bind_gift(&mut w, &c, Owner::Member(0), Owner::Member(1));
    tick(&mut w, &c, 300);
    let before = gift(&w);
    move_from(&mut w, &c, Owner::Member(1), true, 450);
    for _ in 0..5 {
        w.statuses
            .views(&status_entity(&w.members[0].reference.entity), 450);
    }
    w.execute_status_operation(
        (Owner::Member(1), Owner::Member(0)),
        &effect(true),
        &c,
        450,
        &mut vec![],
    );
    assert_eq!(gift(&w), before);
}
#[test]
fn periodic_transfer_new_tick_has_current_holder_observers_and_old_npc_bystanders_only_remove() {
    let (c, mut w) = setup_periodic(true);
    bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
    let e = move_from(&mut w, &c, Owner::Npc(0), true, 150);
    assert!(
        e.iter()
            .filter(|(i, _)| *i != Some(0))
            .all(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusRemoved {
                        reason: StatusRemovalReason::Transferred,
                        ..
                    }
                }
            ))
    );
    let e = tick(&mut w, &c, 300);
    assert!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::PeriodicHealTick { .. }
                }
            ))
            .all(|(i, _)| *i == Some(0))
    );
}
#[test]
fn periodic_transfer_cleanup_reasons_end_without_delayed_ticks_or_new_statuses() {
    for reason in [
        StatusRemovalReason::Cleansed,
        StatusRemovalReason::Closed,
        StatusRemovalReason::Reset,
        StatusRemovalReason::TargetInvalid,
        StatusRemovalReason::SourceInvalid,
    ] {
        let (c, mut w) = setup_periodic(true);
        bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
        move_from(&mut w, &c, Owner::Npc(0), true, 150);
        w.statuses.remove(reason, |_| true, &mut vec![]);
        let hp = w.members[0].hp;
        tick(&mut w, &c, 900);
        assert_eq!(w.members[0].hp, hp);
        assert!(w.statuses.entries().is_empty());
    }
}
#[test]
fn periodic_transfer_capacity_and_blocker_failures_leave_original_progress_untouched() {
    for blocked in [false, true] {
        let (mut c, mut w) = setup_periodic(true);
        bind_gift(&mut w, &c, Owner::Member(0), Owner::Member(1));
        tick(&mut w, &c, 300);
        charge_definition(&mut c, "filler");
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own, "filler", 0);
        if blocked {
            operation_policy(&mut c, "filler", "beneficial", false);
            c.combat_rules
                .as_mut()
                .unwrap()
                .status_policies
                .get_mut("filler")
                .unwrap()
                .blocks = vec![crate::config::StatusDisposition::Beneficial];
        } else {
            c.pve_mut()
                .status_rules
                .as_mut()
                .unwrap()
                .max_active_per_entity = 1;
        }
        let before = w.statuses.entries().to_vec();
        move_from(&mut w, &c, Owner::Member(1), true, 450);
        assert_eq!(w.statuses.entries(), before);
    }
}

#[test]
fn periodic_transfer_multiple_hot_sources_keep_independent_identity_and_source_cleanup() {
    let (c, mut w) = setup_periodic(true);
    bind_gift(&mut w, &c, Owner::Member(0), Owner::Member(1));
    bind_gift(&mut w, &c, Owner::Member(1), Owner::Member(1));
    move_from(&mut w, &c, Owner::Member(1), true, 150);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .filter(|s| s.target == status_entity(&w.members[0].reference.entity))
            .count(),
        2
    );
    let old = w.members[1].reference.entity.clone();
    w.clear_owned_statuses(&old, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(
        gift(&w).source,
        status_entity(&w.members[0].reference.entity)
    );
}
#[test]
fn periodic_transfer_zero_received_heal_and_spirit_lock_use_new_holders_live_state() {
    let (mut c, mut w) = setup_periodic(true);
    c.combat_rules.as_mut().unwrap().periodic_recovery.insert(
        "periodic-gift".into(),
        serde_json::from_value(
            json!({"spirit":{"fixed":10,"source_attack_bps":0,"target_max_spirit_bps":0}}),
        )
        .unwrap(),
    );
    for d in [
        json!({"id":"no-heal","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":-10000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"lock","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock","modifier_bps":-10000,"duration_ms":600,"duplicate_policy":"refresh","source_scoped":true}),
    ] {
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(d).unwrap());
    }
    bind_gift(&mut w, &c, Owner::Npc(0), Owner::Npc(0));
    move_from(&mut w, &c, Owner::Npc(0), true, 150);
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own.clone(), "no-heal", 150);
    bind(&mut w, &c, Owner::Member(0), own, "lock", 0);
    w.members[0].hp = 800;
    w.members[0].spirit = 200;
    tick(&mut w, &c, 300);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (800, 200));
    w.prepare_periodic(600, &mut vec![]);
    tick(&mut w, &c, 600);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (800, 210));
}
