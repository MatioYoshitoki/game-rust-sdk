use super::*;

fn setup_record() -> (Config, World) {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    record_shield(&mut c, "overflow-record", 2000, 900, record_rule());
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
    charge_definition(&mut c, "filler");
    w.members[1].facts.hp_points = 1000;
    w.members[1].hp = 1000;
    (c, w)
}
fn conversion() -> crate::config::OverhealShield {
    serde_json::from_value(json!({"status_id":"overflow-record","conversion_bps":5000})).unwrap()
}
fn grant(w: &mut World, c: &Config, source: usize, target: usize, excess: u64, now: u64) -> Events {
    let mut events = vec![];
    w.convert_resolved_overheal(
        c,
        &conversion(),
        (
            &status_entity(&w.members[source].reference.entity),
            &status_entity(&w.members[target].reference.entity),
            1000,
        ),
        now,
        excess,
        &mut events,
    );
    events
}
fn view(w: &World) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "overflow-record")
        .unwrap()
        .clone()
}
fn instant(w: &mut World, c: &Config, source: usize, target: usize, now: u64) -> Events {
    let heal = serde_json::from_value(json!({"amount":101,"overheal_shield":{"status_id":"overflow-record","conversion_bps":5000}})).unwrap();
    let mut events = vec![];
    w.resolve_skill_heal((source, target), &heal, "probe", c, now, &mut events);
    events
}
fn tonic(c: &mut Config, w: &mut World) {
    c.member_item_use=serde_json::from_value(json!({"tonic":{"heal_hp":101,"apply_healing_received":true,"quality_multiplier_bps":15000,"cooldown_ms":600,"overheal_shield":{"status_id":"overflow-record","conversion_bps":5000}}})).unwrap();
    w.members[0].inventory.insert("tonic".into(), 3);
    w.members[0].looted.insert("tonic".into(), 3);
}
fn use_tonic(w: &mut World, c: &Config, now: u64) -> Result<(u32, Option<u32>)> {
    w.use_item(0, "tonic", w.members[0].revision, c, now, &mut vec![])
}
#[test]
fn overheal_record_config_accepts_all_recovery_routes_and_rejects_invalid_shields() {
    let package = Package::new();
    let mut good = document();
    good["combat_rules"]["shield_records"] = json!({"overflow-shield":record_rule()});
    // All routes use the identical native shield definition.
    good["member_item_use"] = json!({good["item_id"].as_str().unwrap():{"heal_hp":101,"overheal_shield":{"status_id":"overflow-shield","conversion_bps":5000}}});
    *good["additional_skills"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["id"] == "focus-power")
        .unwrap() = json!({"id":"record-heal","mechanism":"self_status","cost":1,"windup_ms":0,"recovery_ms":0,"cooldown_ms":300,"self_heal":{"amount":101,"overheal_shield":{"status_id":"overflow-shield","conversion_bps":5000}}});
    package.write(&good);
    let loaded = package.load().unwrap();
    assert_eq!(
        serde_json::to_value(loaded.default_config()).unwrap()["combat_rules"]["shield_records"],
        serde_json::to_value(
            serde_json::from_value::<
                std::collections::BTreeMap<String, crate::config::ShieldRecordRule>,
            >(good["combat_rules"]["shield_records"].clone())
            .unwrap()
        )
        .unwrap()
    );
    for case in 0..6 {
        let mut bad = good.clone();
        let shield = bad["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|d| d["id"] == "overflow-shield")
            .unwrap();
        match case {
            0 => shield["duplicate_policy"] = json!("ignore"),
            1 => shield["source_scoped"] = json!(false),
            2 => bad["mechanism_version"] = json!(104),
            3 => {
                bad["combat_rules"]["shield_records"]["overflow-shield"]["cap_fixed"] =
                    json!(1000001)
            }
            4 => bad["combat_rules"]["shield_records"]["missing"] = record_rule(),
            _ => bad["combat_rules"]["shield_records"]["overflow-shield"]["counter"] = Value::Null,
        }
        package.write(&bad);
        assert!(package.load().is_err(), "case {case}");
    }
}
#[test]
fn overheal_record_instant_self_and_ally_use_actual_excess_and_private_ownership() {
    for target in [0, 1] {
        let (c, mut w) = setup_record();
        w.members[target].hp = 990;
        let events = instant(&mut w, &c, 0, target, 30);
        let shield = view(&w);
        assert_eq!(w.members[target].hp, 1000);
        assert_eq!(shield.shield.as_ref().unwrap().remaining, 45);
        assert_eq!(
            shield
                .shield
                .as_ref()
                .unwrap()
                .record
                .as_ref()
                .unwrap()
                .recorded,
            0
        );
        assert_eq!(shield.source, status_entity(&w.members[0].reference.entity));
        assert_eq!(
            shield.target,
            status_entity(&w.members[target].reference.entity)
        );
        assert_eq!(shield.modifiers[0].modifier_bps, 2000);
        assert!(
            events
                .iter()
                .all(|(i, _)| *i == Some(0) || target == 1 && *i == Some(1))
        );
        assert_eq!(w.members[0].spirit, 300);
    }
}
#[test]
fn overheal_record_healing_received_applies_once_before_conversion() {
    let (mut c, mut w) = setup_record();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"care","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":5000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), target, "care", 0);
    w.members[1].hp = 990;
    instant(&mut w, &c, 0, 1, 30);
    assert_eq!(view(&w).shield.unwrap().remaining, 70); // floor((floor(101*1.5)-10)*0.5)
}
#[test]
fn overheal_record_hot_uses_spirit_partition_then_records_only_later_incoming() {
    let (mut c, mut w) = setup_record();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"record-hot","mechanism":"periodic_heal","heal_bps":1000,"duration_ms":900,"tick_interval_ms":300,"duplicate_policy":"refresh","source_scoped":true,"overheal_shield":{"status_id":"overflow-record","conversion_bps":5000}})).unwrap());
    let rules = c.combat_rules.as_mut().unwrap();
    rules.healing.insert(
        "record-hot".into(),
        serde_json::from_value(json!({"fixed":101,"source_attack_bps":0,"target_max_hp_bps":0}))
            .unwrap(),
    );
    rules.periodic_recovery.insert("record-hot".into(),serde_json::from_value(json!({"spirit":{"fixed":1,"source_attack_bps":0,"target_max_spirit_bps":0},"overheal_spirit":{"bps":5000,"when":"any_excess"}})).unwrap());
    w.members[0].spirit = 296;
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "record-hot", 0);
    let mut events = vec![];
    w.advance_periodic(&c, 300, &mut events);
    assert_eq!(w.members[0].spirit, 300);
    assert_eq!(view(&w).shield.unwrap().remaining, 47); // (101-6)/2
    assert_eq!(recorded(&w, "overflow-record").recorded, 0);
    record_hit(&mut w, &c, 10, 330);
    assert_eq!(recorded(&w, "overflow-record").recorded, 10);
    w.advance_periodic(&c, 600, &mut events);
    assert_eq!(recorded(&w, "overflow-record").recorded, 0); // reset on positive refill
    assert_eq!(view(&w).shield.unwrap().remaining, 87); // 47-10+50
}
#[test]
fn overheal_record_positive_refresh_samples_effective_source_power_and_keeps_identity_balance() {
    for preserve in [false, true] {
        let (mut c, mut w) = setup_record();
        let r = c
            .combat_rules
            .as_mut()
            .unwrap()
            .shield_records
            .get_mut("overflow-record")
            .unwrap();
        r.cap_fixed = 0;
        r.cap_power_bps = 10000;
        r.refresh = if preserve {
            crate::config::RecordRefresh::Preserve
        } else {
            crate::config::RecordRefresh::Reset
        };
        grant(&mut w, &c, 0, 0, 200, 0);
        let first = view(&w);
        assert_eq!(recorded(&w, "overflow-record").cap, 125);
        record_hit(&mut w, &c, 80, 30);
        w.members[0].facts.atk_points = 40;
        assert_eq!(recorded(&w, "overflow-record").cap, 125);
        grant(&mut w, &c, 0, 0, 20, 60);
        let after = view(&w);
        assert_eq!(after.instance_id, first.instance_id);
        assert_eq!(after.expires_at_ms, 960);
        assert_eq!(after.shield.unwrap().remaining, 30);
        assert_eq!(
            (
                recorded(&w, "overflow-record").cap,
                recorded(&w, "overflow-record").recorded
            ),
            (50, if preserve { 50 } else { 0 })
        );
    }
}
#[test]
fn overheal_record_no_increase_does_not_refresh_expiry_reduction_or_ledger() {
    for full_pool in [false, true] {
        let (mut c, mut w) = setup_record();
        grant(&mut w, &c, 0, 0, 400, 0);
        let target = status_entity(&w.members[0].reference.entity);
        w.statuses.record_incoming(&target, 77, 30);
        if full_pool {
            c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(1000);
        }
        w.members[0].facts.atk_points = 1;
        let before = w.statuses.entries().to_vec();
        let events = grant(&mut w, &c, 0, 0, if full_pool { 100 } else { 0 }, 60);
        assert_eq!(w.statuses.entries(), before);
        assert!(events.is_empty());
    }
}
#[test]
fn overheal_record_own_and_shared_caps_and_multi_source_balances_are_independent() {
    let (mut c, mut w) = setup_record();
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(3000);
    grant(&mut w, &c, 0, 0, 1000, 0);
    grant(&mut w, &c, 1, 0, 1000, 30);
    let rows = w
        .statuses
        .views(&status_entity(&w.members[0].reference.entity), 30);
    let rows = rows
        .iter()
        .filter(|s| s.definition_id == "overflow-record")
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].shield.as_ref().unwrap().remaining, 200);
    assert_eq!(rows[1].shield.as_ref().unwrap().remaining, 100);
    assert!(
        rows.iter()
            .all(|s| s.shield.as_ref().unwrap().record.as_ref().unwrap().recorded == 0)
    );
    let before = w.statuses.entries().to_vec();
    assert!(grant(&mut w, &c, 1, 0, 1000, 60).is_empty());
    assert_eq!(before, w.statuses.entries());
}
#[test]
fn overheal_record_full_status_capacity_rejects_only_new_shield_without_undoing_healing() {
    let (mut c, mut w) = setup_record();
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "filler", 0);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    w.members[0].hp = 990;
    let events = instant(&mut w, &c, 0, 0, 30);
    assert_eq!(w.members[0].hp, 1000);
    assert!(!w.statuses.entries().iter().any(|s| s.shield.is_some()));
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusRejected {
                reason: Error::Capacity,
                ..
            }
        }
    )));
}
#[test]
fn overheal_record_item_quality_recovery_and_refill_commit_inventory_revision_and_cooldown_once() {
    let (mut c, mut w) = setup_record();
    tonic(&mut c, &mut w);
    w.members[0].hp = 990;
    let revision = w.members[0].revision;
    let spirit = w.members[0].spirit;
    assert_eq!(use_tonic(&mut w, &c, 30).unwrap(), (10, None));
    assert_eq!(view(&w).shield.unwrap().remaining, 70);
    assert_eq!(recorded(&w, "overflow-record").cap, 150);
    assert_eq!(w.members[0].inventory["tonic"], 2);
    assert_eq!(w.members[0].revision, revision + 1);
    assert_eq!(w.members[0].spirit, spirit);
    let before = w.statuses.entries().to_vec();
    assert_eq!(use_tonic(&mut w, &c, 60).unwrap_err(), Error::InvalidState);
    assert_eq!(w.statuses.entries(), before);
    assert_eq!(w.members[0].inventory["tonic"], 2);
    use_tonic(&mut w, &c, 630).unwrap();
    assert_eq!(w.members[0].inventory["tonic"], 1);
    assert_eq!(view(&w).shield.unwrap().remaining, 145);
}
#[test]
fn overheal_record_item_capacity_and_blocker_failures_leave_all_business_state_unchanged() {
    for blocked in [false, true] {
        let (mut c, mut w) = setup_record();
        tonic(&mut c, &mut w);
        w.members[0].hp = 990;
        let target = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), target, "filler", 0);
        if blocked {
            operation_policy(&mut c, "overflow-record", "beneficial", false);
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
        let hp = w.members[0].hp;
        let revision = w.members[0].revision;
        let before = w.statuses.entries().to_vec();
        assert_eq!(
            use_tonic(&mut w, &c, 30).unwrap_err(),
            if blocked {
                Error::Immune
            } else {
                Error::Capacity
            }
        );
        assert_eq!(w.members[0].hp, hp);
        assert_eq!(w.members[0].revision, revision);
        assert_eq!(w.members[0].inventory["tonic"], 3);
        assert!(w.members[0].item_cooldowns.is_empty());
        assert_eq!(w.statuses.entries(), before);
    }
}
#[test]
fn overheal_record_no_item_gain_does_not_consume_or_reset_a_full_recorded_shield() {
    let (mut c, mut w) = setup_record();
    tonic(&mut c, &mut w);
    grant(&mut w, &c, 0, 0, 400, 0);
    let target = status_entity(&w.members[0].reference.entity);
    w.statuses.record_incoming(&target, 80, 30);
    let before = w.statuses.entries().to_vec();
    assert_eq!(use_tonic(&mut w, &c, 60).unwrap_err(), Error::InvalidState);
    assert_eq!(w.statuses.entries(), before);
    assert_eq!(w.members[0].inventory["tonic"], 3);
}
#[test]
fn overheal_record_depletion_records_appropriate_basis_before_hp_and_releases_once() {
    for own_absorbed in [false, true] {
        let (mut c, mut w) = setup_record();
        if own_absorbed {
            c.combat_rules
                .as_mut()
                .unwrap()
                .shield_records
                .get_mut("overflow-record")
                .unwrap()
                .basis = ShieldRecordBasis::OwnAbsorbed;
        }
        grant(&mut w, &c, 0, 0, 200, 0);
        let hp = w.members[0].hp;
        let (result, mut events) = record_hit(&mut w, &c, 200, 30);
        assert_eq!((result.absorbed, result.hp_damage), (100, 100));
        assert_eq!(w.members[0].hp, hp - 100);
        w.finish_shield_records(&c, 30, &mut events);
        w.finish_shield_records(&c, 30, &mut events);
        let ends = events
            .iter()
            .filter_map(|(_, e)| match e {
                EventV4::Private {
                    event:
                        Event::ShieldRecordEnded {
                            recorded,
                            raw_counter,
                            targets_hit,
                            ..
                        },
                } => Some((*recorded, *raw_counter, *targets_hit)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ends,
            vec![(
                if own_absorbed { 100 } else { 150 },
                if own_absorbed { 60 } else { 90 },
                1
            )]
        );
    }
}
#[test]
fn overheal_record_cleanup_reasons_never_release_and_consumption_or_expiry_only_once() {
    for reason in [
        StatusRemovalReason::Cleansed,
        StatusRemovalReason::SourceInvalid,
        StatusRemovalReason::TargetInvalid,
        StatusRemovalReason::Reset,
        StatusRemovalReason::Closed,
        StatusRemovalReason::Expired,
        StatusRemovalReason::Consumed,
    ] {
        let (c, mut w) = setup_record();
        grant(&mut w, &c, 0, 0, 200, 0);
        let target = status_entity(&w.members[0].reference.entity);
        w.statuses.record_incoming(&target, 100, 30);
        w.statuses.remove(
            reason,
            |s| s.definition_id == "overflow-record",
            &mut vec![],
        );
        let mut events = vec![];
        w.finish_shield_records(&c, 60, &mut events);
        w.finish_shield_records(&c, 60, &mut events);
        assert_eq!(
            w.npcs[0].hp,
            if matches!(
                reason,
                StatusRemovalReason::Expired | StatusRemovalReason::Consumed
            ) {
                940
            } else {
                1000
            },
            "{reason:?}"
        );
    }
}
#[test]
fn overheal_record_source_exit_generation_and_protected_holder_cannot_release_benefits() {
    for case in 0..3 {
        let (c, mut w) = setup_record();
        grant(&mut w, &c, 0, 1, 200, 0);
        let target = status_entity(&w.members[1].reference.entity);
        w.statuses.record_incoming(&target, 100, 30);
        match case {
            0 => w.members[0].hp = 0,
            1 => w.members[0].reference.entity.generation += 1,
            _ => w.members[1].protected = true,
        }
        w.statuses.remove(
            StatusRemovalReason::Expired,
            |s| s.definition_id == "overflow-record",
            &mut vec![],
        );
        w.finish_shield_records(&c, 900, &mut vec![]);
        assert_eq!(w.npcs[0].hp, 1000);
    }
}
#[test]
fn overheal_record_derived_counter_does_not_feed_another_record_or_recurse() {
    let (c, mut w) = setup_record();
    grant(&mut w, &c, 0, 0, 200, 0);
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc, "overflow-record", 0);
    let own = status_entity(&w.members[0].reference.entity);
    w.statuses.record_incoming(&own, 100, 30);
    w.statuses.remove(
        StatusRemovalReason::Consumed,
        |s| s.target == own,
        &mut vec![],
    );
    w.finish_shield_records(&c, 60, &mut vec![]);
    let npc_shield = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.source.kind == StatusEntityKind::Monster)
        .unwrap()
        .shield
        .as_ref()
        .unwrap();
    assert_eq!(npc_shield.record.as_ref().unwrap().recorded, 0);
    assert!(npc_shield.remaining < 200);
}
#[test]
fn overheal_record_invalid_source_is_rejected_before_any_new_grant() {
    let (c, w) = setup_record();
    let mut source = status_entity(&w.members[0].reference.entity);
    source.generation += 1;
    let target = status_entity(&w.members[1].reference.entity);
    assert!(matches!(
        w.plan_overheal_conversion(&c, &conversion(), (&source, &target, 1000), 30, 100),
        Err(Error::InvalidTarget)
    ));
    assert!(w.statuses.entries().is_empty());
}

#[test]
fn overheal_record_physical_reduction_precedes_record_shield_and_hp_without_reducing_true_damage() {
    let (mut c, mut w) = setup_record();
    grant(&mut w, &c, 0, 0, 200, 0);
    w.members[0].facts.def_points = 0;
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("npc")
        .unwrap()
        .tier = 2;
    let source = w.npcs[0].entity.clone();
    let target = w.members[0].reference.entity.clone();
    let result = w.resolve_damage_with_rule(
        DamageIntent {
            source,
            target,
            raw: 200,
            direct: false,
            ignore_armor: true,
        },
        Some(&AttackRule {
            can_crit: false,
            ..Default::default()
        }),
        &c,
        30,
        &mut vec![],
    );
    assert_eq!((result.absorbed, result.hp_damage), (100, 60));
    assert_eq!(w.members[0].hp, 940);
}

#[test]
fn overheal_record_preflight_freezes_effective_source_power_before_any_later_resource_write() {
    let (mut c, mut w) = setup_record();
    let rule = c
        .combat_rules
        .as_mut()
        .unwrap()
        .shield_records
        .get_mut("overflow-record")
        .unwrap();
    rule.cap_fixed = 0;
    rule.cap_power_bps = 10000;
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"power-test","mechanism":"timed_compound_modifier","duration_ms":900,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"atk","modifier_bps":2000},{"attribute":"skill_power_bonus","modifier_bps":5000}]})).unwrap());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "power-test", 0);
    let source = status_entity(&w.members[0].reference.entity);
    let planned = w
        .plan_overheal_conversion(&c, &conversion(), (&source, &source, 1000), 30, 100)
        .unwrap()
        .unwrap();
    w.members[0].facts.atk_points = 1;
    w.apply_overheal_conversion(planned, &source, &source, 30, &mut vec![]);
    assert_eq!(recorded(&w, "overflow-record").cap, 225);
}

#[test]
fn overheal_record_live_maximum_drop_preserves_the_old_balance_without_free_refresh() {
    let (c, mut w) = setup_record();
    grant(&mut w, &c, 0, 0, 400, 0);
    let target = status_entity(&w.members[0].reference.entity);
    w.statuses.record_incoming(&target, 80, 30);
    let before = view(&w);
    w.members[0].facts.hp_points = 150;
    w.members[0].hp = 150;
    instant(&mut w, &c, 0, 0, 60);
    assert_eq!(view(&w), before);
    assert_eq!(w.members[0].hp, 150);
    assert_eq!(w.statuses.shield_total(&target, 60), 200);
}

#[test]
fn overheal_record_blocked_instant_conversion_keeps_completed_heal_and_never_queues_a_release() {
    let (mut c, mut w) = setup_record();
    operation_policy(&mut c, "overflow-record", "beneficial", false);
    operation_policy(&mut c, "filler", "beneficial", false);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("filler")
        .unwrap()
        .blocks = vec![crate::config::StatusDisposition::Beneficial];
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "filler", 0);
    w.members[0].hp = 990;
    let mut events = instant(&mut w, &c, 0, 0, 30);
    assert_eq!(w.members[0].hp, 1000);
    assert!(!w.statuses.entries().iter().any(|s| s.shield.is_some()));
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusRejected {
                reason: Error::Immune,
                ..
            }
        }
    )));
    w.finish_shield_records(&c, 30, &mut events);
    assert!(!events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ShieldRecordEnded { .. }
        }
    )));
}
