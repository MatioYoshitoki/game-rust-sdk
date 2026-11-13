use super::*;
fn rule(base: Option<u32>, bps: Option<u32>, when: &str) -> Value {
    let mut v = json!({});
    if let Some(n) = base {
        v["spirit"] = json!({"fixed":n,"source_attack_bps":0,"target_max_spirit_bps":0});
    }
    if let Some(n) = bps {
        v["overheal_spirit"] = json!({"bps":n,"when":when});
    }
    v
}
fn add(c: &mut Config, v: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(v).unwrap());
}
fn energy(c: &mut Config, r: Value, shield: bool) {
    add(
        c,
        json!({"id":"energy","mechanism":"periodic_heal","heal_bps":1000,"tick_interval_ms":300,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}),
    );
    c.combat_rules
        .as_mut()
        .unwrap()
        .periodic_recovery
        .insert("energy".into(), serde_json::from_value(r).unwrap());
    c.combat_rules.as_mut().unwrap().healing.insert(
        "energy".into(),
        serde_json::from_value(json!({"fixed":101,"source_attack_bps":0,"target_max_hp_bps":0}))
            .unwrap(),
    );
    if shield {
        add(
            c,
            json!({"id":"energy-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true}),
        );
        c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
        c.pve_mut()
            .status_definitions
            .iter_mut()
            .find(|d| d.id == "energy")
            .unwrap()
            .overheal_shield = Some(
            serde_json::from_value(json!({"status_id":"energy-shield","conversion_bps":10000}))
                .unwrap(),
        );
    }
}
fn own(w: &World) -> EntityRef {
    w.members[0].reference.entity.clone()
}
fn apply(w: &mut World, c: &Config) {
    let target = own(w);
    bind(w, c, Owner::Member(0), target, "energy", 0);
}
fn view(w: &World, id: &str) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == id)
        .unwrap()
        .clone()
}
fn rows(e: &Events) -> Vec<Event> {
    e.iter()
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event: e @ Event::PeriodicSpiritTick { .. },
            } => Some(e.clone()),
            _ => None,
        })
        .collect()
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
fn periodic_resources_config_roundtrips_and_rejects_invalid_formulas_routes_and_references() {
    let p = Package::new();
    let mut good = document();
    good["combat_rules"]["periodic_recovery"] =
        json!({"ally-renewal":rule(Some(5),Some(5000),"full_before_tick")});
    p.write(&good);
    assert_eq!(
        serde_json::to_value(p.load().unwrap().default_config()).unwrap()["combat_rules"]["periodic_recovery"],
        good["combat_rules"]["periodic_recovery"]
    );
    for case in 0..17 {
        let mut bad = good.clone();
        let r = &mut bad["combat_rules"]["periodic_recovery"]["ally-renewal"];
        match case {
            0 => *r = json!({}),
            1 => r["spirit"] = Value::Null,
            2 => r["overheal_spirit"] = Value::Null,
            3 => r["spirit"]["fixed"] = json!(1000001),
            4 => r["spirit"]["source_attack_bps"] = json!(100001),
            5 => r["spirit"]["target_max_spirit_bps"] = json!(10001),
            6 => r["spirit"]["fixed"] = json!(0),
            7 => r["overheal_spirit"]["bps"] = json!(0),
            8 => r["overheal_spirit"]["bps"] = json!(10001),
            9 => r["overheal_spirit"]["when"] = json!("unknown"),
            10 => r["spirit"]["target_max_hp_bps"] = json!(100),
            11 => r["unknown"] = json!(true),
            12 => bad["combat_rules"]["periodic_recovery"] = json!({"wound":rule(Some(1),None,"")}),
            13 => bad["combat_rules"]["periodic_recovery"] = json!({"ward":rule(Some(1),None,"")}),
            14 => {
                bad["combat_rules"]["periodic_recovery"] = json!({"missing":rule(Some(1),None,"")})
            }
            15 => bad["mechanism_version"] = json!(104),
            _ => {
                let map = bad["combat_rules"]["periodic_recovery"]
                    .as_object_mut()
                    .unwrap();
                for i in 0..16 {
                    map.insert(format!("missing-{i}"), rule(Some(1), None, ""));
                }
            }
        }
        p.write(&bad);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn periodic_resources_hp_base_spirit_conversion_and_shield_partition_one_budget() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(10), Some(5000), "full_before_tick"), true);
    apply(&mut w, &c);
    w.members[0].spirit = 200;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 260);
    assert_eq!(view(&w, "energy-shield").shield.unwrap().granted, 1);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            raw_spirit: 10,
            base_restored: 10,
            hp_excess: 101,
            converted_spirit: 50,
            spent_heal: 100,
            remaining_excess: 1,
            ..
        }]
    ));
    let heal = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::PeriodicHealTick { .. }
                }
            )
        })
        .unwrap();
    let spirit = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::PeriodicSpiritTick { .. }
                }
            )
        })
        .unwrap();
    let shield = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::ShieldGranted { .. }
                }
            )
        })
        .unwrap();
    assert!(heal < spirit && spirit < shield);
}
#[test]
fn periodic_resources_base_recovery_has_priority_and_unused_conversion_budget_reaches_shield() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(10), Some(5000), "full_before_tick"), true);
    apply(&mut w, &c);
    w.members[0].spirit = 299;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 300);
    assert_eq!(view(&w, "energy-shield").shield.unwrap().granted, 101);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            base_restored: 1,
            converted_spirit: 0,
            spent_heal: 0,
            remaining_excess: 101,
            ..
        }]
    ));
}
#[test]
fn periodic_resources_full_before_tick_and_any_excess_differ_after_partial_hp_restore() {
    for full in [false, true] {
        let (mut c, mut w) = setup();
        energy(
            &mut c,
            rule(
                None,
                Some(5000),
                if full {
                    "full_before_tick"
                } else {
                    "any_excess"
                },
            ),
            true,
        );
        apply(&mut w, &c);
        w.members[0].hp = 990;
        w.members[0].spirit = 200;
        let mut e = vec![];
        w.advance_periodic(&c, 300, &mut e);
        assert_eq!(w.members[0].hp, 1000);
        assert_eq!(w.members[0].spirit, if full { 200 } else { 245 });
        assert_eq!(
            view(&w, "energy-shield").shield.unwrap().granted,
            if full { 91 } else { 1 }
        );
        assert!(
            matches!(rows(&e).as_slice(),[Event::PeriodicSpiritTick{conversion_eligible,..}] if *conversion_eligible!=full)
        );
    }
}
#[test]
fn periodic_resources_conversion_rounding_preserves_budget_at_tiny_ratios_large_inputs_and_small_gaps()
 {
    for amount in [1, 2, 3, 10, 101, u64::from(u32::MAX) * 100] {
        for bps in [1, 3333, 5000, 10000] {
            let (mut c, mut w) = setup();
            energy(&mut c, rule(None, Some(bps), "any_excess"), false);
            apply(&mut w, &c);
            let (status, _) = w.statuses.take_periodic_due(300).unwrap();
            w.members[0].spirit = 299;
            let (remaining, e) = w.resolve_periodic_spirit(
                &status,
                (0, amount, true, true),
                (300, 300, PeriodicTickOutcome::Applied),
                &c,
            );
            if let Event::PeriodicSpiritTick {
                converted_spirit,
                spent_heal,
                ..
            } = e
            {
                assert_eq!(remaining + spent_heal, amount);
                assert!(spent_heal <= amount);
                assert!(converted_spirit <= 1);
                if converted_spirit > 0 {
                    assert!(
                        u128::from(spent_heal) * u128::from(bps) / 10000
                            >= u128::from(converted_spirit)
                    );
                    assert!(
                        u128::from(spent_heal - 1) * u128::from(bps) / 10000
                            < u128::from(converted_spirit)
                    );
                }
            } else {
                panic!("resource fact");
            }
        }
    }
}
#[test]
fn periodic_resources_four_sampling_modes_choose_attack_owner_maximum_and_time() {
    for (sampling, target, live) in [
        ("source_snapshot", false, false),
        ("target_snapshot", true, false),
        ("source_live", false, true),
        ("target_live", true, true),
    ] {
        let (mut c, mut w) = setup();
        energy(
            &mut c,
            json!({"spirit":{"fixed":1,"source_attack_bps":5000,"target_max_spirit_bps":1000}}),
            false,
        );
        c.combat_rules.as_mut().unwrap().periodic_sampling.insert(
            "energy".into(),
            serde_json::from_value(json!(sampling)).unwrap(),
        );
        w.members[0].facts.atk_points = 100;
        w.members[1].facts.atk_points = 200;
        w.members[1].facts.spirit_points = 300;
        let holder = w.members[1].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), holder, "energy", 0);
        assert_eq!(
            view(&w, "energy").periodic.unwrap().raw_spirit,
            Some(if target { 131 } else { 81 })
        );
        w.members[0].facts.atk_points = 300;
        w.members[1].facts.atk_points = 400;
        w.members[1].facts.spirit_points = 600;
        w.members[1].spirit = 0;
        let mut e = vec![];
        w.advance_periodic(&c, 300, &mut e);
        let expected = if live {
            if target { 261 } else { 211 }
        } else if target {
            131
        } else {
            81
        };
        assert_eq!(w.members[1].spirit, expected);
        assert_eq!(
            view(&w, "energy").periodic.unwrap().raw_spirit,
            Some(expected)
        );
        assert_eq!(rows(&e).len(), 2);
    }
}
#[test]
fn periodic_resources_snapshot_refresh_and_ignore_preserve_native_identity_and_cadence() {
    for ignore in [false, true] {
        let (mut c, mut w) = setup();
        energy(
            &mut c,
            json!({"spirit":{"fixed":0,"source_attack_bps":10000,"target_max_spirit_bps":0}}),
            false,
        );
        if ignore {
            c.pve_mut()
                .status_definitions
                .iter_mut()
                .find(|d| d.id == "energy")
                .unwrap()
                .duplicate_policy = crate::config::DuplicatePolicy::Ignore;
        }
        apply(&mut w, &c);
        let first = view(&w, "energy");
        w.members[0].facts.atk_points = 200;
        let target = own(&w);
        bind(&mut w, &c, Owner::Member(0), target, "energy", 150);
        let next = view(&w, "energy");
        assert_eq!(first.instance_id, next.instance_id);
        assert_eq!(
            next.periodic.as_ref().unwrap().next_tick_at_ms,
            Some(if ignore { 300 } else { 450 })
        );
        assert_eq!(
            next.periodic.unwrap().raw_spirit,
            Some(if ignore { 100 } else { 200 })
        );
    }
}
#[test]
fn periodic_resources_large_base_formula_saturates_after_combined_wide_integer_math() {
    let (mut c, mut w) = setup();
    energy(
        &mut c,
        json!({"spirit":{"fixed":1000000,"source_attack_bps":100000,"target_max_spirit_bps":10000}}),
        false,
    );
    w.members[0].facts.atk_points = u32::MAX;
    w.members[0].facts.spirit_points = u32::MAX;
    w.members[0].spirit = 0;
    apply(&mut w, &c);
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, u32::MAX);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            raw_spirit: u32::MAX,
            base_restored: u32::MAX,
            ..
        }]
    ));
}
#[test]
fn periodic_resources_combines_formula_terms_before_rounding() {
    let (mut c, mut w) = setup();
    energy(
        &mut c,
        json!({"spirit":{"fixed":1,"source_attack_bps":3333,"target_max_spirit_bps":3333}}),
        false,
    );
    w.members[0].facts.atk_points = 2;
    w.members[0].facts.spirit_points = 2;
    w.members[0].spirit = 0;
    apply(&mut w, &c);
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 2);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick { raw_spirit: 2, .. }]
    ));
}
#[test]
fn periodic_resources_spirit_lock_blocks_both_paths_without_consuming_shield_budget() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(10), Some(5000), "full_before_tick"), true);
    add(
        &mut c,
        json!({"id":"lock","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock","modifier_bps":-10000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}),
    );
    apply(&mut w, &c);
    let target = own(&w);
    bind(&mut w, &c, Owner::Npc(0), target, "lock", 0);
    w.members[0].spirit = 200;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 200);
    assert_eq!(view(&w, "energy-shield").shield.unwrap().granted, 101);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            base_restored: 0,
            converted_spirit: 0,
            spent_heal: 0,
            remaining_excess: 101,
            ..
        }]
    ));
}
#[test]
fn periodic_resources_zero_healing_received_does_not_block_independent_base_spirit() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(10), Some(5000), "any_excess"), true);
    add(
        &mut c,
        json!({"id":"no-heal","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":-10000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}),
    );
    apply(&mut w, &c);
    let target = own(&w);
    bind(&mut w, &c, Owner::Npc(0), target, "no-heal", 0);
    w.members[0].spirit = 200;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 210);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            base_restored: 10,
            hp_excess: 0,
            converted_spirit: 0,
            spent_heal: 0,
            ..
        }]
    ));
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "energy-shield")
    );
}
#[test]
fn periodic_resources_healing_received_is_applied_once_before_conversion() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(None, Some(5000), "any_excess"), true);
    add(
        &mut c,
        json!({"id":"boost","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":5000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}),
    );
    apply(&mut w, &c);
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target, "boost", 0);
    w.members[0].spirit = 0;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 75);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            hp_excess: 151,
            converted_spirit: 75,
            spent_heal: 150,
            remaining_excess: 1,
            ..
        }]
    ));
}
#[test]
fn periodic_resources_cleanse_happens_after_locked_spirit_and_shield_then_unlocks_next_tick() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(10), Some(5000), "any_excess"), true);
    add(
        &mut c,
        json!({"id":"lock","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock","modifier_bps":-10000,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}),
    );
    c.combat_rules.as_mut().unwrap().status_policies.insert(
        "lock".into(),
        serde_json::from_value(
            json!({"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]}),
        )
        .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().status_cleanses.insert(
        "energy".into(),
        serde_json::from_value(
            json!({"on_tick":{"disposition":"harmful","max_count":1,"scope":"all"}}),
        )
        .unwrap(),
    );
    apply(&mut w, &c);
    let target = own(&w);
    bind(&mut w, &c, Owner::Npc(0), target, "lock", 0);
    w.members[0].spirit = 200;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 200);
    let spirit = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::PeriodicSpiritTick { .. }
                }
            )
        })
        .unwrap();
    let shield = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::ShieldGranted { .. }
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
    assert!(spirit < shield && shield < clean);
    e.clear();
    w.advance_periodic(&c, 600, &mut e);
    assert_eq!(w.members[0].spirit, 260);
}
#[test]
fn periodic_resources_zero_spirit_maximum_and_expired_temporary_maxima_never_grant_above_live_cap()
{
    for zero in [false, true] {
        let (mut c, mut w) = setup();
        quiet(&mut w);
        energy(&mut c, rule(Some(10), Some(5000), "any_excess"), true);
        if zero {
            w.members[0].facts.spirit_points = 0;
            w.members[0].spirit = 7;
        } else {
            add(
                &mut c,
                json!({"id":"maximum","mechanism":"timed_attribute_modifier","attribute":"max_spirit","modifier_bps":10000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
            );
            let target = own(&w);
            bind(&mut w, &c, Owner::Member(0), target, "maximum", 0);
            w.members[0].spirit = 599;
        }
        apply(&mut w, &c);
        let e = w.tick(&c, 300);
        assert_eq!(w.members[0].spirit, if zero { 0 } else { 300 });
        assert!(matches!(
            rows(&e).as_slice(),
            [Event::PeriodicSpiritTick {
                base_restored: 0,
                converted_spirit: 0,
                spent_heal: 0,
                ..
            }]
        ));
    }
}
#[test]
fn periodic_resources_protected_tick_is_consumed_and_reconnect_does_not_backfill() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(5), None, ""), false);
    apply(&mut w, &c);
    w.members[0].spirit = 200;
    let before = view(&w, "energy");
    w.members[0].protected = true;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 200);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            tick_index: 1,
            outcome: PeriodicTickOutcome::Protected,
            base_restored: 0,
            ..
        }]
    ));
    w.members[0].protected = false;
    w.members[0].ready = true;
    assert_eq!(view(&w, "energy").instance_id, before.instance_id);
    e.clear();
    w.advance_periodic(&c, 600, &mut e);
    assert_eq!(w.members[0].spirit, 205);
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            tick_index: 2,
            base_restored: 5,
            ..
        }]
    ));
}
#[test]
fn periodic_resources_ally_attribution_and_generation_cleanup_keep_benefits_on_holder() {
    for source in [false, true] {
        let (mut c, mut w) = setup();
        energy(&mut c, rule(Some(5), None, ""), false);
        let holder = w.members[1].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), holder, "energy", 0);
        w.members[1].spirit = 200;
        let source_spirit = w.members[0].spirit;
        let mut e = vec![];
        w.advance_periodic(&c, 300, &mut e);
        assert_eq!(w.members[1].spirit, 205);
        assert_eq!(w.members[0].spirit, source_spirit);
        let r = rows(&e);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0], r[1]);
        assert!(
            matches!(&r[0],Event::PeriodicSpiritTick{source,target,..} if source.id==w.members[0].reference.entity.id&&target.id==w.members[1].reference.entity.id)
        );
        w.members[usize::from(!source)].reference.entity.generation += 1;
        w.prepare_periodic(330, &mut vec![]);
        e.clear();
        w.advance_periodic(&c, 600, &mut e);
        assert!(rows(&e).is_empty());
    }
}
#[test]
fn periodic_resources_npc_self_recovery_and_returning_use_original_native_qualification() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(5), None, ""), false);
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc, "energy", 0);
    w.npcs[0].spirit = 0;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.npcs[0].spirit, 5);
    assert_eq!(rows(&e).len(), w.members.len());
    w.npcs[0].returning = true;
    e.clear();
    w.advance_periodic(&c, 600, &mut e);
    assert_eq!(w.npcs[0].spirit, 5);
    assert!(rows(&e).iter().all(|e| matches!(
        e,
        Event::PeriodicSpiritTick {
            outcome: PeriodicTickOutcome::Returning,
            base_restored: 0,
            ..
        }
    )));
}
#[test]
fn periodic_resources_multiple_sources_share_the_live_gap_without_duplicate_gains() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(3), None, ""), false);
    let holder = own(&w);
    bind(&mut w, &c, Owner::Member(0), holder.clone(), "energy", 0);
    bind(&mut w, &c, Owner::Member(1), holder, "energy", 0);
    w.members[0].spirit = 295;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 300);
    let first = rows(&e);
    assert_eq!(first.len(), 3);
    assert!(matches!(
        &first[0],
        Event::PeriodicSpiritTick {
            base_restored: 3,
            ..
        }
    ));
    assert!(first[1..].iter().all(|e| matches!(
        e,
        Event::PeriodicSpiritTick {
            base_restored: 2,
            ..
        }
    )));
}
#[test]
fn periodic_resources_final_tick_window_end_and_readonly_views_do_not_duplicate_recovery() {
    let (mut c, mut w) = setup();
    quiet(&mut w);
    energy(&mut c, rule(Some(5), None, ""), false);
    c.combat_rules.as_mut().unwrap().status_windows.insert("energy".into(),serde_json::from_value(json!({"basis":"none","include_periodic":false,"record_cap":0,"refresh":"reset","maximum_sampling":"snapshot","on_expired":true,"on_consumed":false,"spirit":{"fixed":10,"recorded_bps":0,"max_resource_bps":0,"cap_max_resource_bps":10000}})).unwrap());
    apply(&mut w, &c);
    w.members[0].spirit = 292;
    let before = view(&w, "energy");
    for _ in 0..3 {
        w.snapshot(0, &c, Phase::Active, 0);
    }
    assert_eq!(view(&w, "energy"), before);
    let mut e = vec![];
    for now in [300, 600, 900, 930] {
        e.extend(w.tick(&c, now));
    }
    assert_eq!(w.members[0].spirit, 300);
    assert_eq!(rows(&e).len(), 2);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::WindowRecordEnded {
                raw_spirit: 10,
                spirit_restored: 0,
                ..
            }
        }
    )));
}
#[test]
fn periodic_resources_exit_reset_close_and_lethal_cleanup_cannot_pay_later_ticks() {
    for reason in [
        StatusRemovalReason::TargetInvalid,
        StatusRemovalReason::SourceInvalid,
        StatusRemovalReason::Reset,
        StatusRemovalReason::Closed,
    ] {
        let (mut c, mut w) = setup();
        energy(&mut c, rule(Some(5), Some(5000), "any_excess"), false);
        apply(&mut w, &c);
        w.members[0].spirit = 200;
        let target = own(&w);
        w.clear_owned_statuses(&target, reason, &mut vec![]);
        let mut e = vec![];
        w.advance_periodic(&c, 300, &mut e);
        assert_eq!(w.members[0].spirit, 200);
        assert!(rows(&e).is_empty());
    }
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(5), None, ""), false);
    apply(&mut w, &c);
    w.members[0].hp = 5;
    record_hit(&mut w, &c, 100, 30);
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert!(!w.members[0].living());
    assert!(rows(&e).is_empty());
}

#[test]
fn periodic_resources_shield_capacity_drops_unused_remainder_without_replaying_spirit_conversion() {
    let (mut c, mut w) = setup();
    energy(&mut c, rule(Some(10), Some(5000), "any_excess"), true);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "energy-shield")
        .unwrap()
        .shield_bps = Some(1000);
    apply(&mut w, &c);
    w.members[0].spirit = 299;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.members[0].spirit, 300);
    assert_eq!(view(&w, "energy-shield").shield.unwrap().granted, 100);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ShieldGranted {
                requested: 101,
                granted: 100,
                ..
            }
        }
    )));
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            base_restored: 1,
            converted_spirit: 0,
            spent_heal: 0,
            remaining_excess: 101,
            ..
        }]
    ));
}

#[test]
fn periodic_resources_expired_hp_maximum_is_clamped_without_becoming_conversion_budget() {
    let (mut c, mut w) = setup();
    quiet(&mut w);
    energy(&mut c, rule(None, Some(5000), "full_before_tick"), true);
    c.combat_rules.as_mut().unwrap().healing.insert(
        "energy".into(),
        serde_json::from_value(json!({"fixed":0,"source_attack_bps":0,"target_max_hp_bps":1000}))
            .unwrap(),
    );
    add(
        &mut c,
        json!({"id":"hp-maximum","mechanism":"timed_attribute_modifier","attribute":"max_hp","modifier_bps":10000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let target = own(&w);
    bind(&mut w, &c, Owner::Member(0), target, "hp-maximum", 0);
    apply(&mut w, &c);
    w.members[0].hp = 1500;
    w.members[0].spirit = 0;
    let e = w.tick(&c, 300);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (1000, 100));
    assert!(matches!(
        rows(&e).as_slice(),
        [Event::PeriodicSpiritTick {
            hp_excess: 200,
            converted_spirit: 100,
            spent_heal: 200,
            remaining_excess: 0,
            ..
        }]
    ));
}

#[test]
fn periodic_resources_configured_npc_self_heal_uses_receiving_multiplier_before_spirit_conversion()
{
    let (mut c, mut w) = setup();
    energy(&mut c, rule(None, Some(5000), "full_before_tick"), false);
    add(
        &mut c,
        json!({"id":"npc-receiving","mechanism":"timed_compound_modifier","flags":["control_immunity"],"modifiers":[{"attribute":"healing_received","modifier_bps":5000}],"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), npc.clone(), "npc-receiving", 0);
    bind(&mut w, &c, Owner::Npc(0), npc, "energy", 0);
    w.npcs[0].spirit = 0;
    let member_spirit = w.members[0].spirit;
    let mut e = vec![];
    w.advance_periodic(&c, 300, &mut e);
    assert_eq!(w.npcs[0].spirit, 75);
    assert_eq!(w.members[0].spirit, member_spirit);
    assert!(rows(&e).iter().all(|e| matches!(
        e,
        Event::PeriodicSpiritTick {
            hp_excess: 151,
            converted_spirit: 75,
            spent_heal: 150,
            remaining_excess: 1,
            ..
        }
    )));
}
