use super::*;
fn setup_shield(record: bool) -> (Config, World) {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    record_shield(&mut c, "gift-shield", 2000, 900, record_rule());
    if !record {
        c.combat_rules
            .as_mut()
            .unwrap()
            .shield_records
            .remove("gift-shield");
    }
    operation_policy(&mut c, "gift-shield", "beneficial", true);
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(3000);
    w.members[1].facts.hp_points = 1000;
    w.members[1].hp = 1000;
    (c, w)
}
fn bind_gift(w: &mut World, c: &Config, source: Owner) {
    let target = w.members[1].reference.entity.clone();
    bind(w, c, source, target, "gift-shield", 0);
}
fn gift(w: &World) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "gift-shield")
        .unwrap()
        .clone()
}
fn effect() -> crate::config::TriggerEffect {
    serde_json::from_value(transfer_effect()).unwrap()
}
fn plan(w: &World, c: &Config, now: u64) -> Result<(Vec<String>, Option<StatusEntity>)> {
    w.plan_status_operation(
        &w.statuses,
        (Owner::Member(0), Owner::Member(1)),
        &effect(),
        c,
        now,
    )
}
fn transfer(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.execute_status_operation(
        (Owner::Member(0), Owner::Member(1)),
        &effect(),
        c,
        now,
        &mut e,
    );
    e
}
fn trigger_fact(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DamageDealt,
        (Owner::Member(0), Owner::Member(1)),
        (10, false),
        c,
        now,
        &mut e,
    );
    e
}
#[test]
fn shield_transfer_loader_accepts_native_and_overheal_recorded_shields_but_refuses_other_carriers()
{
    let p = Package::new();
    let mut v = document();
    v["combat_rules"]["status_policies"] = json!({"ward":{"disposition":"beneficial","dispellable":true,"transferable":true,"blocks":[]},"overflow-shield":{"disposition":"beneficial","dispellable":true,"transferable":true,"blocks":[]}});
    v["combat_rules"]["shield_records"] = json!({"overflow-shield":record_rule()});
    p.write(&v);
    let loaded = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(loaded.default_config()).unwrap()["combat_rules"]["status_policies"],
        v["combat_rules"]["status_policies"]
    );
    for case in 0..5 {
        let mut bad = v.clone();
        match case {
            0 => bad["mechanism_version"] = json!(104),
            1 => bad["combat_rules"]["status_policies"]["ward"]["dispellable"] = json!(false),
            2 => {
                bad["combat_rules"]["status_policies"]["wound"] =
                    v["combat_rules"]["status_policies"]["ward"].clone();
                bad["combat_rules"]["status_policies"]["wound"]["dispellable"] = json!(false)
            }
            3 => {
                bad["combat_rules"]["status_policies"]["missing"] =
                    v["combat_rules"]["status_policies"]["ward"].clone()
            }
            _ => {
                bad["combat_rules"]["status_policies"]["renewal"] =
                    v["combat_rules"]["status_policies"]["ward"].clone();
                bad["combat_rules"]["status_policies"]["renewal"]["dispellable"] = json!(false)
            }
        }
        p.write(&bad);
        assert!(p.load().is_err(), "{case}");
    }
}
#[test]
fn shield_transfer_keeps_partial_balance_identity_source_deadline_without_any_grant_or_end() {
    let (c, mut w) = setup_shield(false);
    bind_gift(&mut w, &c, Owner::Member(0));
    let old = status_entity(&w.members[1].reference.entity);
    w.statuses.absorb(&old, 50, 30, "test", &mut vec![]);
    let before = gift(&w);
    let e = transfer(&mut w, &c, 60);
    let mut expected = before;
    expected.target = status_entity(&w.members[0].reference.entity);
    assert_eq!(gift(&w), expected);
    assert_eq!(w.statuses.shield_total(&old, 60), 0);
    assert_eq!(w.statuses.entries().len(), 1);
    assert!(e.iter().all(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusTransferred { .. }
        }
    )));
    w.finish_shield_records(&c, 60, &mut vec![]);
    assert_eq!(w.npcs[0].hp, 1000);
}
#[test]
fn shield_transfer_preserves_recorded_amount_cap_reduction_and_never_resamples_source_power() {
    let (c, mut w) = setup_shield(true);
    bind_gift(&mut w, &c, Owner::Member(0));
    let target = status_entity(&w.members[1].reference.entity);
    w.statuses.record_incoming(&target, 80, 30);
    let before = gift(&w);
    w.members[0].facts.atk_points = 1;
    transfer(&mut w, &c, 60);
    let after = gift(&w);
    assert_eq!(after.shield, before.shield);
    assert_eq!(after.modifiers, before.modifiers);
    assert_eq!(after.expires_at_ms, 900);
}
#[test]
fn shield_transfer_accepts_an_overheal_created_record_without_regranting_its_balance() {
    let (c, mut w) = setup_shield(true);
    let source = status_entity(&w.members[0].reference.entity);
    let target = status_entity(&w.members[1].reference.entity);
    let cv =
        serde_json::from_value(json!({"status_id":"gift-shield","conversion_bps":5000})).unwrap();
    w.convert_resolved_overheal(&c, &cv, (&source, &target, 1000), 0, 101, &mut vec![]);
    let before = gift(&w);
    assert_eq!(before.shield.as_ref().unwrap().remaining, 50);
    transfer(&mut w, &c, 60);
    assert_eq!(gift(&w).shield, before.shield);
}
#[test]
fn shield_transfer_final_numeric_modifier_can_expand_destination_pool_for_the_same_atomic_batch() {
    let (mut c, mut w) = setup_shield(false);
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(1000);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"maximum","mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":10000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    operation_policy(&mut c, "maximum", "beneficial", true);
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "maximum", 0);
    bind_gift(&mut w, &c, Owner::Member(0));
    assert_eq!(gift(&w).shield.as_ref().unwrap().remaining, 200);
    assert!(plan(&w, &c, 30).is_ok());
    transfer(&mut w, &c, 30);
    assert_eq!(w.member_max_spirit(0, &c, 30), 300);
    assert_eq!(
        w.statuses
            .shield_total(&status_entity(&w.members[0].reference.entity), 30),
        200
    );
}
#[test]
fn shield_transfer_final_negative_modifier_rejects_the_whole_batch_instead_of_clipping() {
    let (mut c, mut w) = setup_shield(false);
    bind_gift(&mut w, &c, Owner::Member(0));
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"maximum","mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":-5000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    operation_policy(&mut c, "maximum", "beneficial", true);
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "maximum", 0);
    let before = w.statuses.entries().to_vec();
    assert!(matches!(plan(&w, &c, 30), Err(Error::Capacity)));
    transfer(&mut w, &c, 30);
    assert_eq!(w.statuses.entries(), before);
}
#[test]
fn shield_transfer_consumed_maximum_is_removed_in_preview_before_pool_capacity_is_checked() {
    let (mut c, mut w) = setup_shield(false);
    bind_gift(&mut w, &c, Owner::Member(0));
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(1000);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"charge","mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":10000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "charge", 0);
    operation_trigger(
        &mut c,
        transfer_effect(),
        Some(json!({"subject":"owner","definition_id":"charge","at_least":1,"consume":1})),
    );
    let before = w.statuses.entries().to_vec();
    trigger_fact(&mut w, &c, 30);
    assert_eq!(w.statuses.entries(), before);
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(2000);
    trigger_fact(&mut w, &c, 60);
    assert_eq!(
        gift(&w).target,
        status_entity(&w.members[0].reference.entity)
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "charge")
    );
}
#[test]
fn shield_transfer_whole_shield_consumption_can_free_both_slot_and_pool_before_commit() {
    let (mut c, mut w) = setup_shield(false);
    bind_gift(&mut w, &c, Owner::Member(0));
    record_shield(&mut c, "charge", 3000, 900, record_rule());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "charge", 0);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    operation_trigger(
        &mut c,
        transfer_effect(),
        Some(json!({"subject":"owner","definition_id":"charge","at_least":1,"consume":1})),
    );
    trigger_fact(&mut w, &c, 30);
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(
        gift(&w).target,
        status_entity(&w.members[0].reference.entity)
    );
}
#[test]
fn shield_transfer_multiple_sources_require_the_complete_sum_and_keep_balances_distinct() {
    let (mut c, mut w) = setup_shield(false);
    bind_gift(&mut w, &c, Owner::Member(0));
    bind_gift(&mut w, &c, Owner::Npc(0));
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(2000);
    let before = w.statuses.entries().to_vec();
    assert!(matches!(plan(&w, &c, 30), Err(Error::Capacity)));
    assert_eq!(w.statuses.entries(), before);
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(3000);
    transfer(&mut w, &c, 30);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .filter(|s| s.target == status_entity(&w.members[0].reference.entity))
            .count(),
        2
    );
}
#[test]
fn shield_transfer_collision_and_category_blocking_do_not_consume_or_start_icd() {
    for blocked in [false, true] {
        let (mut c, mut w) = setup_shield(false);
        bind_gift(&mut w, &c, Owner::Member(0));
        charge_definition(&mut c, "charge");
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own.clone(), "charge", 0);
        if blocked {
            operation_policy(&mut c, "charge", "beneficial", false);
            c.combat_rules
                .as_mut()
                .unwrap()
                .status_policies
                .get_mut("charge")
                .unwrap()
                .blocks = vec![crate::config::StatusDisposition::Beneficial];
        } else {
            bind(&mut w, &c, Owner::Member(0), own, "gift-shield", 0);
        }
        operation_trigger(
            &mut c,
            transfer_effect(),
            Some(json!({"subject":"owner","definition_id":"charge","at_least":1,"consume":0})),
        );
        let before = w.statuses.entries().to_vec();
        trigger_fact(&mut w, &c, 30);
        assert_eq!(w.statuses.entries(), before);
    }
}
#[test]
fn shield_transfer_rejects_protected_old_or_new_holder_returning_and_stale_source() {
    for case in 0..4 {
        let (c, mut w) = setup_shield(true);
        bind_gift(&mut w, &c, Owner::Npc(0));
        match case {
            0 => w.members[1].protected = true,
            1 => w.members[0].protected = true,
            2 => w.npcs[0].entity.generation += 1,
            _ => w.members[1].hp = 0,
        };
        assert!(plan(&w, &c, 30).is_err());
    }
    let (c, mut w) = setup_shield(false);
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), npc, "gift-shield", 0);
    w.npcs[0].returning = true;
    assert!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(0)),
            &effect(),
            &c,
            30
        )
        .is_err()
    );
}
#[test]
fn shield_transfer_new_holder_alone_absorbs_and_accumulates_each_record_basis() {
    for basis in ["incoming", "own_absorbed"] {
        let (mut c, mut w) = setup_shield(true);
        c.combat_rules
            .as_mut()
            .unwrap()
            .shield_records
            .get_mut("gift-shield")
            .unwrap()
            .basis = serde_json::from_value(json!(basis)).unwrap();
        bind_gift(&mut w, &c, Owner::Member(0));
        transfer(&mut w, &c, 30);
        record_hit(&mut w, &c, 80, 60);
        assert_eq!(recorded(&w, "gift-shield").recorded, 80);
        assert_eq!(gift(&w).shield.unwrap().remaining, 120);
        let old = status_entity(&w.members[1].reference.entity);
        assert_eq!(w.statuses.absorb(&old, 20, 60, "test", &mut vec![]), 20);
        assert_eq!(recorded(&w, "gift-shield").recorded, 80);
    }
}
#[test]
fn shield_transfer_end_uses_new_holder_location_and_original_source_without_releasing_on_move() {
    let (mut c, mut w) = setup_shield(true);
    c.combat_rules
        .as_mut()
        .unwrap()
        .shield_records
        .get_mut("gift-shield")
        .unwrap()
        .counter
        .as_mut()
        .unwrap()
        .radius = 40.;
    w.members[1].position = [100., 100.];
    w.members[0].position = [500., 500.];
    w.npcs[0].position = [510., 500.];
    bind_gift(&mut w, &c, Owner::Member(0));
    let target = status_entity(&w.members[1].reference.entity);
    w.statuses.record_incoming(&target, 100, 0);
    transfer(&mut w, &c, 30);
    w.finish_shield_records(&c, 30, &mut vec![]);
    assert_eq!(w.npcs[0].hp, 1000);
    w.statuses.expire(900, &mut vec![]);
    let mut e = vec![];
    w.finish_shield_records(&c, 900, &mut e);
    w.finish_shield_records(&c, 900, &mut e);
    assert_eq!(w.npcs[0].hp, 940);
    assert!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::ShieldRecordEnded { .. }
                }
            ))
            .all(|(i, _)| *i == Some(0))
    );
}
#[test]
fn shield_transfer_former_holder_exit_preserves_instance_but_original_source_exit_cleans_it() {
    let (c, mut w) = setup_shield(true);
    bind_gift(&mut w, &c, Owner::Npc(0));
    transfer(&mut w, &c, 30);
    let old = w.members[1].reference.entity.clone();
    w.clear_owned_statuses(&old, StatusRemovalReason::TargetInvalid, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    let source = w.npcs[0].entity.clone();
    w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(w.statuses.entries().is_empty());
    w.finish_shield_records(&c, 900, &mut vec![]);
    assert_eq!(w.members[0].hp, 1000);
}
#[test]
fn shield_transfer_cleanup_and_enabled_ends_keep_original_once_only_semantics() {
    for reason in [
        StatusRemovalReason::Expired,
        StatusRemovalReason::Consumed,
        StatusRemovalReason::Cleansed,
        StatusRemovalReason::Closed,
        StatusRemovalReason::Reset,
        StatusRemovalReason::TargetInvalid,
        StatusRemovalReason::SourceInvalid,
    ] {
        let (c, mut w) = setup_shield(true);
        bind_gift(&mut w, &c, Owner::Member(0));
        let target = status_entity(&w.members[1].reference.entity);
        w.statuses.record_incoming(&target, 100, 0);
        transfer(&mut w, &c, 30);
        w.statuses.remove(reason, |_| true, &mut vec![]);
        w.finish_shield_records(&c, 60, &mut vec![]);
        w.finish_shield_records(&c, 60, &mut vec![]);
        assert_eq!(
            w.npcs[0].hp,
            if matches!(
                reason,
                StatusRemovalReason::Expired | StatusRemovalReason::Consumed
            ) {
                940
            } else {
                1000
            }
        );
    }
}
#[test]
fn shield_transfer_return_trip_keeps_frozen_single_grant_and_does_not_extend_lifetime() {
    let (c, mut w) = setup_shield(true);
    bind_gift(&mut w, &c, Owner::Member(0));
    let before = gift(&w);
    transfer(&mut w, &c, 30);
    w.execute_status_operation(
        (Owner::Member(1), Owner::Member(0)),
        &effect(),
        &c,
        60,
        &mut vec![],
    );
    assert_eq!(gift(&w), before);
}
#[test]
fn shield_transfer_budget_sums_wide_integers_before_comparing_destination_pool() {
    let (mut c, mut w) = setup_shield(false);
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "gift-shield")
        .unwrap()
        .shield_bps = Some(5000);
    w.members[1].facts.hp_points = u32::MAX;
    w.members[1].hp = u32::MAX;
    w.members[0].facts.hp_points = u32::MAX;
    w.members[0].hp = u32::MAX;
    bind_gift(&mut w, &c, Owner::Member(0));
    bind_gift(&mut w, &c, Owner::Npc(0));
    record_shield(&mut c, "tiny", 1, 900, record_rule());
    let source = status_entity(&w.members[0].reference.entity);
    let cv = serde_json::from_value(json!({"status_id":"tiny","conversion_bps":10000})).unwrap();
    w.convert_resolved_overheal(&c, &cv, (&source, &source, u32::MAX), 0, 2, &mut vec![]);
    assert!(matches!(plan(&w, &c, 30), Err(Error::Capacity)));
}
#[test]
fn shield_transfer_npc_bystanders_only_lose_old_visible_record_and_never_learn_new_private_balance()
{
    let (c, mut w) = setup_shield(true);
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc, "gift-shield", 0);
    let mut e = vec![];
    w.execute_status_operation((Owner::Member(0), Owner::Npc(0)), &effect(), &c, 30, &mut e);
    assert!(e.iter().any(|(i, e)| *i == Some(0)
        && matches!(
            e,
            EventV4::Private {
                event: Event::StatusTransferred { .. }
            }
        )));
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
}

#[test]
fn shield_transfer_frozen_grant_can_exceed_new_single_ratio_when_complete_shared_pool_fits() {
    let (mut c, mut w) = setup_shield(false);
    w.members[1].facts.hp_points = 2000;
    w.members[1].hp = 2000;
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(5000);
    bind_gift(&mut w, &c, Owner::Member(0));
    let before = gift(&w);
    assert_eq!(before.shield.as_ref().unwrap().remaining, 400);
    transfer(&mut w, &c, 30);
    assert_eq!(gift(&w).shield, before.shield);
    assert_eq!(
        gift(&w).target,
        status_entity(&w.members[0].reference.entity)
    );
}

#[test]
fn shield_transfer_new_holder_death_cleanup_never_releases_original_sources_record() {
    let (c, mut w) = setup_shield(true);
    bind_gift(&mut w, &c, Owner::Member(1));
    let old = status_entity(&w.members[1].reference.entity);
    w.statuses.record_incoming(&old, 100, 0);
    transfer(&mut w, &c, 30);
    w.members[0].hp = 0;
    let holder = w.members[0].reference.entity.clone();
    w.clear_owned_statuses(&holder, StatusRemovalReason::TargetInvalid, &mut vec![]);
    assert!(w.statuses.entries().is_empty());
    w.finish_shield_records(&c, 60, &mut vec![]);
    assert_eq!(w.npcs[0].hp, 1000);
}
