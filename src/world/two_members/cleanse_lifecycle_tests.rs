use super::*;
fn action(scope: &str, max: u32) -> Value {
    json!({"disposition":"harmful","max_count":max,"scope":scope})
}
fn add(c: &mut Config, d: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
}
fn plain(id: &str, duration: u64) -> Value {
    json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":-2000,"duration_ms":duration,"duplicate_policy":"refresh","source_scoped":true})
}
fn policy(c: &mut Config, id: &str, dispellable: bool) {
    c.combat_rules.as_mut().unwrap().status_policies.insert(id.into(),serde_json::from_value(json!({"disposition":"harmful","dispellable":dispellable,"transferable":false,"blocks":[]})).unwrap());
}
fn carrier(c: &mut Config, id: &str, hot: bool, scope: &str, expiry: bool) {
    add(
        c,
        if hot {
            json!({"id":id,"mechanism":"periodic_heal","heal_bps":1000,"tick_interval_ms":300,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})
        } else {
            json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})
        },
    );
    let mut rule = json!({});
    if hot {
        rule["on_tick"] = action(scope, 1);
    }
    if expiry {
        rule["on_expired"] = action(scope, 16);
    }
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_cleanses
        .insert(id.into(), serde_json::from_value(rule).unwrap());
}
fn view(w: &World, id: &str) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == id)
        .unwrap()
        .clone()
}
fn own(w: &World) -> EntityRef {
    w.members[0].reference.entity.clone()
}
fn facts(e: &Events) -> Vec<Event> {
    e.iter()
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event: e @ Event::StatusCleanseResolved { .. },
            } => Some(e.clone()),
            _ => None,
        })
        .collect()
}
fn debuff(c: &mut Config, id: &str, duration: u64) {
    add(c, plain(id, duration));
    policy(c, id, true);
}
fn quiet(w: &mut World) {
    for n in &mut w.npcs {
        n.position = [100., 100.];
        n.alerted = false;
        let p = Arc::make_mut(&mut n.profile);
        p.combat.aggro_range = 0.;
        p.memory_range = 0.;
        p.combat.cast_chance = 0.;
    }
}
#[test]
fn cleanse_lifecycle_config_roundtrips_and_rejects_invalid_rules_and_carriers() {
    let p = Package::new();
    let mut good = document();
    good["combat_rules"]["status_cleanses"] = json!({"ally-renewal":{"on_tick":action("all",1),"on_expired":action("since_application",16)},"demo-guard":{"on_expired":action("all",1)}});
    p.write(&good);
    assert_eq!(
        serde_json::to_value(p.load().unwrap().default_config()).unwrap()["combat_rules"]["status_cleanses"],
        good["combat_rules"]["status_cleanses"]
    );
    for case in 0..19 {
        let mut v = good.clone();
        let rule = &mut v["combat_rules"]["status_cleanses"]["ally-renewal"];
        match case {
            0 => *rule = Value::Null,
            1 => *rule = json!({}),
            2 => rule["on_tick"] = Value::Null,
            3 => rule["on_tick"]["max_count"] = json!(0),
            4 => rule["on_expired"]["max_count"] = json!(17),
            5 => rule["on_tick"]["scope"] = json!("unknown"),
            6 => rule["on_tick"]["disposition"] = json!("neutral"),
            7 => rule["unknown"] = json!(1),
            8 => rule["on_tick"]["subject"] = json!("source"),
            9 => {
                v["combat_rules"]["status_cleanses"] =
                    json!({"missing":{"on_expired":action("all",1)}})
            }
            10 => {
                v["combat_rules"]["status_cleanses"] =
                    json!({"ward":{"on_expired":action("all",1)}})
            }
            11 => {
                v["combat_rules"]["status_cleanses"] =
                    json!({"wound":{"on_expired":action("all",1)}})
            }
            12 => {
                v["combat_rules"]["status_cleanses"] =
                    json!({"demo-guard":{"on_tick":action("all",1)}})
            }
            13 => {
                v["combat_rules"]["status_policies"] = json!({"demo-guard":{"disposition":"beneficial","dispellable":false,"transferable":true,"blocks":[]}})
            }
            14 => v["combat_rules"]["max_cleanse_operations_per_step"] = json!(0),
            15 => v["combat_rules"]["max_cleanse_operations_per_step"] = json!(129),
            16 => v["mechanism_version"] = json!(104),
            17 => {
                let d = v["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|d| d["id"] == "demo-guard")
                    .unwrap();
                d["duplicate_policy"] = json!("stack");
                d["max_stacks"] = json!(3);
            }
            _ => {
                v["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|d| d["id"] == "demo-guard")
                    .unwrap()["source_scoped"] = json!(false)
            }
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn cleanse_lifecycle_selection_is_stable_bounded_and_respects_application_scope() {
    for scope in ["all", "since_application"] {
        let (mut c, mut w) = setup();
        carrier(&mut c, "clean", true, scope, false);
        for id in ["old", "same", "new", "safe"] {
            debuff(&mut c, id, 1800);
        }
        policy(&mut c, "safe", false);
        policy(&mut c, "clean", true);
        let target = own(&w);
        bind(&mut w, &c, Owner::Npc(0), target.clone(), "old", 0);
        bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 30);
        bind(&mut w, &c, Owner::Npc(0), target.clone(), "same", 30);
        bind(&mut w, &c, Owner::Npc(0), target.clone(), "new", 60);
        bind(&mut w, &c, Owner::Npc(0), target, "safe", 90);
        let expected = view(&w, if scope == "all" { "old" } else { "same" }).instance_id;
        let mut e = vec![];
        w.advance_periodic(&c, 330, &mut e);
        assert!(
            !w.statuses
                .entries()
                .iter()
                .any(|s| s.instance_id == expected)
        );
        assert!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "safe")
        );
        assert!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "clean")
        );
        assert!(matches!(
            facts(&e).as_slice(),
            [Event::StatusCleanseResolved {
                removed_count: 1,
                tick_index: Some(1),
                scheduled_at_ms: 330,
                ..
            }]
        ));
    }
}
#[test]
fn cleanse_lifecycle_excludes_expired_stale_sources_wrong_categories_and_itself() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "clean", true, "all", false);
    for id in ["expired", "stale", "benefit"] {
        debuff(&mut c, id, 300);
    }
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("benefit")
        .unwrap()
        .disposition = crate::config::StatusDisposition::Beneficial;
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "stale")
        .unwrap()
        .duration_ms = 1800;
    let target = own(&w);
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "expired", 0);
    bind(&mut w, &c, Owner::Member(1), target.clone(), "stale", 0);
    w.members[1].reference.entity.generation += 1;
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "benefit", 60);
    bind(&mut w, &c, Owner::Member(0), target, "clean", 0);
    policy(&mut c, "clean", true);
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert!(matches!(
        facts(&e).as_slice(),
        [Event::StatusCleanseResolved {
            removed_count: 0,
            ..
        }]
    ));
    assert_eq!(w.statuses.entries().len(), 4);
}
#[test]
fn cleanse_lifecycle_refresh_and_ignore_share_original_hot_identity_and_cadence() {
    for ignore in [false, true] {
        let (mut c, mut w) = setup();
        carrier(&mut c, "clean", true, "since_application", false);
        debuff(&mut c, "debuff", 1800);
        if ignore {
            c.pve_mut()
                .status_definitions
                .iter_mut()
                .find(|d| d.id == "clean")
                .unwrap()
                .duplicate_policy = crate::config::DuplicatePolicy::Ignore;
        }
        let target = own(&w);
        bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 0);
        let first = view(&w, "clean");
        bind(&mut w, &c, Owner::Npc(0), target.clone(), "debuff", 90);
        bind(&mut w, &c, Owner::Member(0), target, "clean", 150);
        let next = view(&w, "clean");
        assert_eq!(first.instance_id, next.instance_id);
        assert_eq!(
            next.periodic.as_ref().unwrap().next_tick_at_ms,
            Some(if ignore { 300 } else { 450 })
        );
        let mut e = vec![];
        w.advance_periodic(&c, if ignore { 300 } else { 450 }, &mut e);
        assert!(
            matches!(facts(&e).as_slice(),[Event::StatusCleanseResolved{removed_count,..}] if *removed_count==u32::from(ignore))
        );
    }
}
#[test]
fn cleanse_lifecycle_hot_heal_and_overflow_finish_before_cleanse_even_at_full_hp() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "clean", true, "all", false);
    debuff(&mut c, "debuff", 1800);
    add(
        &mut c,
        json!({"id":"clean-overflow","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true}),
    );
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "clean")
        .unwrap()
        .overheal_shield = Some(
        serde_json::from_value(json!({"status_id":"clean-overflow","conversion_bps":10000}))
            .unwrap(),
    );
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 0);
    bind(&mut w, &c, Owner::Npc(0), target, "debuff", 30);
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    let heal = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::PeriodicHealTick { hp_restored: 0, .. }
                }
            )
        })
        .unwrap();
    let clean = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::StatusCleanseResolved {
                        removed_count: 1,
                        ..
                    }
                }
            )
        })
        .unwrap();
    assert!(heal < clean);
    assert!(e.iter().any(|(_, event)| matches!(
        event,
        EventV4::Private {
            event: Event::ShieldGranted { .. }
        }
    )));
    assert_eq!(view(&w, "clean-overflow").shield.unwrap().granted, 100);
    for (index, (_, event)) in e.iter().enumerate() {
        if matches!(
            event,
            EventV4::Private {
                event: Event::ShieldGranted { .. }
            }
        ) {
            assert!(heal < index && index < clean);
        }
    }
}
#[test]
fn cleanse_lifecycle_native_last_tick_and_expiry_have_no_duplicate_terminal_tick() {
    let (mut c, mut w) = setup();
    quiet(&mut w);
    carrier(&mut c, "clean", true, "all", true);
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target, "clean", 0);
    let mut e = vec![];
    for now in [300, 600, 900, 930] {
        e.extend(w.tick(&c, now));
    }
    let f = facts(&e);
    assert_eq!(f.len(), 3);
    assert!(matches!(
        &f[0],
        Event::StatusCleanseResolved {
            tick_index: Some(1),
            scheduled_at_ms: 300,
            ..
        }
    ));
    assert!(matches!(
        &f[1],
        Event::StatusCleanseResolved {
            tick_index: Some(2),
            scheduled_at_ms: 600,
            ..
        }
    ));
    assert!(matches!(
        &f[2],
        Event::StatusCleanseResolved {
            phase: StatusCleansePhase::Expired,
            tick_index: None,
            scheduled_at_ms: 900,
            ..
        }
    ));
}
#[test]
fn cleanse_lifecycle_expired_before_late_execution_drops_all_missed_native_ticks() {
    let (mut c, mut w) = setup();
    quiet(&mut w);
    carrier(&mut c, "clean", true, "all", false);
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target, "clean", 0);
    assert!(facts(&w.tick(&c, 1200)).is_empty());
}
#[test]
fn cleanse_lifecycle_expiry_cleanses_before_window_recovery_and_same_step_dot() {
    let (mut c, mut w) = setup();
    quiet(&mut w);
    carrier(&mut c, "clean", false, "all", true);
    add(
        &mut c,
        json!({"id":"no-heal","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":-10000,"duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true}),
    );
    policy(&mut c, "no-heal", true);
    c.combat_rules.as_mut().unwrap().status_windows.insert("clean".into(),serde_json::from_value(json!({"basis":"none","include_periodic":false,"record_cap":0,"refresh":"reset","maximum_sampling":"live","on_expired":true,"on_consumed":false,"hp":{"fixed":100,"recorded_bps":0,"max_resource_bps":0,"cap_max_resource_bps":10000}})).unwrap());
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "wound")
        .unwrap();
    d.duration_ms = 1800;
    d.tick_interval_ms = Some(900);
    d.damage_bps = Some(50000);
    c.combat_rules.as_mut().unwrap().periodic_damage.insert(
        "wound".into(),
        AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            can_crit: false,
            ..Default::default()
        },
    );
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 0);
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "no-heal", 0);
    bind(&mut w, &c, Owner::Npc(0), target, "wound", 0);
    w.members[0].hp = 500;
    let e = w.tick(&c, 900);
    assert_eq!(w.members[0].hp, 500);
    let a = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::StatusCleanseResolved {
                        removed_count: 1,
                        ..
                    }
                }
            )
        })
        .unwrap();
    let b = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::WindowRecordEnded {
                        hp_restored: 100,
                        ..
                    }
                }
            )
        })
        .unwrap();
    let d = e
        .iter()
        .position(|(_, e)| matches!(e, EventV4::Damage { amount: 100, .. }))
        .unwrap();
    assert!(a < b && b < d);
}
#[test]
fn cleanse_lifecycle_dispelling_recorded_shields_windows_and_carriers_never_releases_them() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "clean", false, "all", true);
    carrier(&mut c, "other", false, "all", true);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "other")
        .unwrap()
        .duration_ms = 1800;
    policy(&mut c, "other", true);
    record_shield(&mut c, "guard", 2000, 1800, record_rule());
    policy(&mut c, "guard", true);
    add(&mut c, plain("window", 1800));
    policy(&mut c, "window", true);
    c.combat_rules.as_mut().unwrap().status_windows.insert("window".into(),serde_json::from_value(json!({"basis":"none","include_periodic":false,"record_cap":0,"refresh":"reset","maximum_sampling":"snapshot","on_expired":true,"on_consumed":true,"hp":{"fixed":100,"recorded_bps":0,"max_resource_bps":0,"cap_max_resource_bps":10000}})).unwrap());
    let target = own(&w);
    for id in ["clean", "other", "guard", "window"] {
        bind(&mut w, &c, Owner::Member(0), target.clone(), id, 0);
    }
    w.members[0].hp = 500;
    let mut e = vec![];
    w.prepare_periodic(900, &mut e);
    w.finish_status_cleanses(&c, 900, &mut e);
    w.finish_shield_records(&c, 900, &mut e);
    w.finish_window_records(&c, 900, &mut e);
    w.finish_status_cleanses(&c, 930, &mut e);
    assert!(w.statuses.entries().is_empty());
    assert_eq!(w.members[0].hp, 500);
    assert_eq!(facts(&e).len(), 1);
    assert!(matches!(
        facts(&e).as_slice(),
        [Event::StatusCleanseResolved {
            removed_count: 3,
            ..
        }]
    ));
    assert!(!e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Damage { .. }
            | EventV4::Private {
                event: Event::HealResolved { .. }
            }
    )));
}
#[test]
fn cleanse_lifecycle_end_queues_are_isolated_from_nested_reflection() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "clean", false, "all", true);
    debuff(&mut c, "debuff", 1800);
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 0);
    bind(&mut w, &c, Owner::Npc(0), target, "debuff", 0);
    trigger(
        &mut c,
        json!({"id":"reflect","profiles":["member"],"when":"damage_taken","include_periodic":true,"every":1,"cooldown_ms":0,"priority":1,"effect":{"kind":"reflect_damage","bps":5000,"damage_kind":"true_damage"}}),
    );
    w.statuses.expire(900, &mut vec![]);
    let (_, mut e) = record_hit(&mut w, &c, 40, 900);
    assert!(facts(&e).is_empty());
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "debuff")
    );
    w.finish_status_cleanses(&c, 900, &mut e);
    assert_eq!(facts(&e).len(), 1);
}
#[test]
fn cleanse_lifecycle_empty_end_operations_spend_budget_before_same_step_hot_catchup() {
    let (mut c, mut w) = setup();
    quiet(&mut w);
    carrier(&mut c, "first", false, "all", true);
    carrier(&mut c, "second", false, "all", true);
    carrier(&mut c, "hot", true, "all", false);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "hot")
        .unwrap()
        .duration_ms = 1500;
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_cleanse_operations_per_step = 1;
    let target = own(&w);
    for id in ["first", "second", "hot"] {
        bind(&mut w, &c, Owner::Member(0), target.clone(), id, 0);
    }
    let e = w.tick(&c, 900);
    let f = facts(&e);
    assert_eq!(f.len(), 5);
    assert!(matches!(
        &f[0],
        Event::StatusCleanseResolved {
            phase: StatusCleansePhase::Expired,
            suppressed: None,
            removed_count: 0,
            ..
        }
    ));
    assert!(f[1..].iter().all(|e| matches!(
        e,
        Event::StatusCleanseResolved {
            suppressed: Some(Error::Capacity),
            ..
        }
    )));
}
#[test]
fn cleanse_lifecycle_protection_consumes_tick_without_retry_and_reconnect_keeps_cadence() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "clean", true, "all", false);
    debuff(&mut c, "debuff", 1800);
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 0);
    bind(&mut w, &c, Owner::Npc(0), target, "debuff", 0);
    let before = view(&w, "clean");
    w.members[0].protected = true;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert!(matches!(
        facts(&e).as_slice(),
        [Event::StatusCleanseResolved {
            tick_index: Some(1),
            suppressed: Some(Error::InvalidState),
            ..
        }]
    ));
    w.members[0].protected = false;
    w.members[0].ready = true;
    assert_eq!(view(&w, "clean").instance_id, before.instance_id);
    e.clear();
    w.advance_periodic(&c, 600, &mut e);
    assert!(matches!(
        facts(&e).as_slice(),
        [Event::StatusCleanseResolved {
            tick_index: Some(2),
            scheduled_at_ms: 600,
            removed_count: 1,
            ..
        }]
    ));
}
#[test]
fn cleanse_lifecycle_expiry_protection_and_stale_generation_drop_once() {
    for stale in [false, true] {
        let (mut c, mut w) = setup();
        carrier(&mut c, "clean", false, "all", true);
        debuff(&mut c, "debuff", 1800);
        let target = own(&w);
        bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 0);
        bind(&mut w, &c, Owner::Npc(0), target, "debuff", 0);
        w.statuses.expire(900, &mut vec![]);
        if stale {
            w.members[0].reference.entity.generation += 1;
        } else {
            w.members[0].protected = true;
        }
        let mut e = vec![];
        w.finish_status_cleanses(&c, 900, &mut e);
        assert!(facts(&e).iter().all(|e| matches!(
            e,
            Event::StatusCleanseResolved {
                suppressed: Some(_),
                removed_count: 0,
                ..
            }
        )));
        w.members[0].protected = false;
        let count = facts(&e).len();
        w.finish_status_cleanses(&c, 930, &mut e);
        assert_eq!(facts(&e).len(), count);
        assert!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "debuff")
        );
    }
}
#[test]
fn cleanse_lifecycle_source_and_holder_are_distinct_and_results_only_expose_count() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "clean", true, "all", true);
    debuff(&mut c, "debuff", 1800);
    let holder = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), holder.clone(), "clean", 0);
    bind(&mut w, &c, Owner::Npc(0), holder, "debuff", 0);
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    let f = facts(&e);
    assert_eq!(f.len(), 2);
    assert_eq!(f[0], f[1]);
    assert!(
        matches!(&f[0],Event::StatusCleanseResolved{source,target,removed_count:1,..} if source.id==w.members[0].reference.entity.id&&target.id==w.members[1].reference.entity.id)
    );
    let v = serde_json::to_value(&f[0]).unwrap();
    assert!(v.get("removed_ids").is_none() && v.get("instance_ids").is_none());
    assert!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusRemoved {
                        reason: StatusRemovalReason::Cleansed,
                        ..
                    }
                }
            ))
            .all(|(i, _)| *i == Some(1))
    );
}
#[test]
fn cleanse_lifecycle_source_or_holder_exit_and_reset_remove_without_end_reward() {
    for source in [false, true] {
        for reason in [
            StatusRemovalReason::SourceInvalid,
            StatusRemovalReason::TargetInvalid,
            StatusRemovalReason::Reset,
            StatusRemovalReason::Closed,
        ] {
            let (mut c, mut w) = setup();
            carrier(&mut c, "clean", true, "all", true);
            let holder = w.members[1].reference.entity.clone();
            bind(&mut w, &c, Owner::Member(0), holder, "clean", 0);
            let entity = w.members[usize::from(!source)].reference.entity.clone();
            let mut e = vec![];
            w.clear_owned_statuses(&entity, reason, &mut e);
            w.finish_status_cleanses(&c, 900, &mut e);
            w.advance_periodic(&c, 900, &mut e);
            assert!(facts(&e).is_empty());
            assert!(w.statuses.entries().is_empty());
        }
    }
}
#[test]
fn cleanse_lifecycle_consumed_or_dispelled_carrier_never_runs_its_expiry_hook() {
    for consumed in [false, true] {
        let (mut c, mut w) = setup();
        carrier(&mut c, "clean", false, "all", true);
        debuff(&mut c, "debuff", 1800);
        let target = own(&w);
        bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 0);
        bind(&mut w, &c, Owner::Npc(0), target, "debuff", 0);
        let id = view(&w, "clean").instance_id;
        if consumed {
            w.statuses.consume(&id, 1, &mut vec![]);
        } else {
            w.statuses.remove(
                StatusRemovalReason::Cleansed,
                |s| s.instance_id == id,
                &mut vec![],
            );
        }
        let mut e = vec![];
        w.finish_status_cleanses(&c, 900, &mut e);
        assert!(facts(&e).is_empty());
        assert_eq!(w.statuses.entries().len(), 1);
    }
}
#[test]
fn cleanse_lifecycle_source_returning_skips_hot_operation_without_pausing_tick() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "clean", true, "all", false);
    debuff(&mut c, "debuff", 1800);
    let target = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "clean", 0);
    bind(&mut w, &c, Owner::Member(0), target, "debuff", 0);
    w.npcs[0].returning = true;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(facts(&e).len(), w.members.len());
    assert!(facts(&e).iter().all(|event| matches!(
        event,
        Event::StatusCleanseResolved {
            suppressed: Some(Error::InvalidState),
            tick_index: Some(1),
            ..
        }
    )));
    assert_eq!(
        view(&w, "clean").periodic.unwrap().next_tick_at_ms,
        Some(600)
    );
}

#[test]
fn cleanse_lifecycle_since_application_includes_later_refresh_of_an_older_identity() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "clean", true, "since_application", false);
    debuff(&mut c, "old", 1800);
    let target = own(&w);
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "old", 0);
    let old = view(&w, "old");
    bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 30);
    bind(&mut w, &c, Owner::Npc(0), target, "old", 60);
    assert_eq!(view(&w, "old").instance_id, old.instance_id);
    let mut e = vec![];
    w.advance_periodic(&c, 330, &mut e);
    assert!(matches!(
        facts(&e).as_slice(),
        [Event::StatusCleanseResolved {
            removed_count: 1,
            ..
        }]
    ));
}
#[test]
fn cleanse_lifecycle_lethal_dot_precedes_hot_and_prevents_tick_or_expiry_cleanup_rewards() {
    let (mut c, mut w) = setup();
    quiet(&mut w);
    carrier(&mut c, "clean", true, "all", true);
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "clean", 0);
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "wound")
        .unwrap();
    d.tick_interval_ms = Some(300);
    d.damage_bps = Some(50000);
    d.duration_ms = 1200;
    c.combat_rules.as_mut().unwrap().periodic_damage.insert(
        "wound".into(),
        AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            can_crit: false,
            ..Default::default()
        },
    );
    bind(&mut w, &c, Owner::Npc(0), target, "wound", 0);
    w.members[0].hp = 5;
    let e = w.tick(&c, 300);
    assert!(!w.members[0].living());
    assert!(facts(&e).is_empty());
    let e = w.tick(&c, 900);
    assert!(facts(&e).is_empty());
}
#[test]
fn cleanse_lifecycle_offensive_expiry_removes_registered_benefit_and_routes_original_observers() {
    let (mut c, mut w) = setup();
    carrier(&mut c, "seal", false, "all", true);
    add(
        &mut c,
        json!({"id":"gift","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":2000,"duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true}),
    );
    policy(&mut c, "gift", true);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("gift")
        .unwrap()
        .disposition = crate::config::StatusDisposition::Beneficial;
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_cleanses
        .get_mut("seal")
        .unwrap()
        .on_expired
        .as_mut()
        .unwrap()
        .disposition = crate::config::StatusDisposition::Beneficial;
    let target = w.npcs[0].entity.clone();
    let base = w.entity_attack(&status_entity(&target), &c, 0).unwrap();
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "gift", 0);
    bind(&mut w, &c, Owner::Member(0), target.clone(), "seal", 0);
    assert_eq!(
        w.entity_attack(&status_entity(&target), &c, 0).unwrap(),
        base * 12 / 10
    );
    let mut e = vec![];
    w.prepare_periodic(900, &mut e);
    w.finish_status_cleanses(&c, 900, &mut e);
    assert_eq!(
        w.entity_attack(&status_entity(&target), &c, 900).unwrap(),
        base
    );
    assert!(matches!(
        facts(&e).as_slice(),
        [Event::StatusCleanseResolved {
            removed_count: 1,
            ..
        }]
    ));
    assert!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusCleanseResolved { .. }
                }
            ))
            .all(|(recipient, _)| *recipient == Some(0))
    );
}
