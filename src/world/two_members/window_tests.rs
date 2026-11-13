use super::*;

fn recovery(recorded: u32, maximum: u32, cap: u32) -> Value {
    json!({"fixed":0,"recorded_bps":recorded,"max_resource_bps":maximum,"cap_max_resource_bps":cap})
}
fn window_rule() -> Value {
    json!({"basis":"damage_dealt","include_periodic":false,"record_cap":10000,"refresh":"reset","maximum_sampling":"snapshot","on_expired":true,"on_consumed":false,"hp":recovery(1000,0,2000)})
}
fn window(c: &mut Config, id: &str, rule: Value) {
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_windows
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
fn hit(
    w: &mut World,
    c: &Config,
    actors: (Owner, Owner),
    amount: (u32, bool),
    now: u64,
) -> (super::super::periodic::DamageResult, Events) {
    let mut e = vec![];
    let damage = w.resolve_damage_with_rule(
        DamageIntent {
            source: w.owner_entity(actors.0),
            target: w.owner_entity(actors.1),
            raw: amount.0,
            direct: amount.1,
            ignore_armor: true,
        },
        Some(&AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            can_crit: false,
            ..Default::default()
        }),
        c,
        now,
        &mut e,
    );
    (damage, e)
}
fn end(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.prepare_periodic(now, &mut e);
    w.finish_window_records(c, now, &mut e);
    e
}
fn ended(events: &Events) -> Vec<Event> {
    events
        .iter()
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event: event @ Event::WindowRecordEnded { .. },
            } => Some(event.clone()),
            _ => None,
        })
        .collect()
}
#[test]
fn window_configuration_roundtrips_and_rejects_unsupported_carriers_refs_modes_and_nulls() {
    let p = Package::new();
    let mut v = document();
    v["combat_rules"]["status_windows"] = json!({"demo-guard":window_rule()});
    p.write(&v);
    let c = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(c.default_config()).unwrap()["combat_rules"]["status_windows"],
        v["combat_rules"]["status_windows"]
    );
    for case in 0..17 {
        let mut bad = v.clone();
        let r = &mut bad["combat_rules"]["status_windows"]["demo-guard"];
        match case {
            0 => r["record_cap"] = json!(0),
            1 => r["record_cap"] = json!(9007199254740992_u64),
            2 => r["hp"] = Value::Null,
            3 => r["hp"]["cap_max_resource_bps"] = json!(0),
            4 => r["hp"]["recorded_bps"] = json!(100001),
            5 => r["maximum_sampling"] = json!("unknown"),
            6 => r["basis"] = json!("none"),
            7 => {
                r["basis"] = json!("none");
                r["record_cap"] = json!(0);
            }
            8 => {
                r.as_object_mut().unwrap().remove("hp");
            }
            9 => r["refresh"] = Value::Null,
            10 => bad["combat_rules"]["status_windows"] = json!({"ward":window_rule()}),
            11 => bad["combat_rules"]["status_windows"] = json!({"wound":window_rule()}),
            12 => bad["combat_rules"]["status_windows"] = json!({"unknown":window_rule()}),
            13 => bad["combat_rules"]["max_window_releases_per_step"] = json!(0),
            14 => bad["combat_rules"]["max_window_releases_per_step"] = json!(129),
            15 => {
                bad["combat_rules"]["status_policies"] = json!({"demo-guard":{"disposition":"beneficial","dispellable":false,"transferable":true,"blocks":[]}})
            }
            _ => {
                bad["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|d| d["id"] == "demo-guard")
                    .unwrap()["source_scoped"] = json!(false)
            }
        }
        p.write(&bad);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn window_counts_holder_actual_damage_caps_record_and_recovers_once_with_original_source() {
    let (mut c, mut w) = setup();
    let mut rule = window_rule();
    rule["record_cap"] = json!(200);
    rule["spirit"] = recovery(200, 0, 2000);
    window(&mut c, "window", rule);
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), own.clone(), "window", 0);
    w.members[0].hp = 500;
    w.members[0].spirit = 100;
    hit(
        &mut w,
        &c,
        (Owner::Member(0), Owner::Npc(0)),
        (250, true),
        30,
    );
    hit(
        &mut w,
        &c,
        (Owner::Member(0), Owner::Npc(0)),
        (100, true),
        60,
    );
    let before = view(&w, "window");
    assert_eq!(before.window.as_ref().unwrap().recorded, 200);
    assert_eq!(before.window.as_ref().unwrap().contacts, 2);
    let e = end(&mut w, &c, 300);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (520, 104));
    assert_eq!(ended(&e).len(), 2);
    assert!(ended(&e).iter().all(|e|matches!(e,Event::WindowRecordEnded{source,target,recorded:200,raw_hp:20,hp_restored:20,raw_spirit:4,spirit_restored:4,..} if source==&status_entity(&w.members[1].reference.entity)&&target==&status_entity(&own))));
    w.finish_window_records(&c, 330, &mut vec![]);
    assert_eq!(w.members[0].hp, 520);
}
#[test]
fn window_damage_taken_shield_absorption_and_periodic_selection_use_separate_actual_facts() {
    for basis in ["damage_taken", "shield_absorbed"] {
        for periodic in [false, true] {
            let (mut c, mut w) = setup();
            let mut rule = window_rule();
            rule["basis"] = json!(basis);
            rule["include_periodic"] = json!(periodic);
            window(&mut c, "window", rule);
            let own = w.members[0].reference.entity.clone();
            bind(&mut w, &c, Owner::Member(0), own.clone(), "window", 0);
            bind(&mut w, &c, Owner::Member(0), own, "ward", 0);
            let first = hit(
                &mut w,
                &c,
                (Owner::Npc(0), Owner::Member(0)),
                (50, false),
                30,
            )
            .0;
            assert!(first.absorbed > 0);
            let expected = if basis == "shield_absorbed" {
                first.absorbed
            } else {
                first.hp_damage
            };
            assert_eq!(
                view(&w, "window").window.unwrap().recorded,
                if periodic { u64::from(expected) } else { 0 }
            );
            let second = hit(
                &mut w,
                &c,
                (Owner::Npc(0), Owner::Member(0)),
                (200, true),
                60,
            )
            .0;
            let amount = if basis == "shield_absorbed" {
                second.absorbed
            } else {
                second.hp_damage
            };
            assert_eq!(
                view(&w, "window").window.unwrap().recorded,
                if periodic {
                    u64::from(expected + amount)
                } else {
                    u64::from(amount)
                }
            );
        }
    }
}
#[test]
fn window_refresh_reset_preserve_and_ignore_keep_identity_and_snapshot_policy() {
    for mode in ["reset", "preserve", "ignore"] {
        let (mut c, mut w) = setup();
        let mut rule = window_rule();
        rule["refresh"] = json!(if mode == "ignore" { "reset" } else { mode });
        window(&mut c, "window", rule);
        if mode == "ignore" {
            c.pve_mut()
                .status_definitions
                .last_mut()
                .unwrap()
                .duplicate_policy = crate::config::DuplicatePolicy::Ignore;
        }
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own.clone(), "window", 0);
        hit(
            &mut w,
            &c,
            (Owner::Member(0), Owner::Npc(0)),
            (100, true),
            30,
        );
        let first = view(&w, "window");
        w.members[0].facts.hp_points = 2000;
        bind(&mut w, &c, Owner::Member(0), own, "window", 60);
        let second = view(&w, "window");
        assert_eq!(first.instance_id, second.instance_id);
        assert_eq!(
            second.window.as_ref().unwrap().recorded,
            if mode == "reset" { 0 } else { 100 }
        );
        assert_eq!(
            second.window.as_ref().unwrap().max_hp_at_apply,
            if mode == "reset" { 2000 } else { 1000 }
        );
        assert_eq!(
            second.expires_at_ms,
            if mode == "ignore" { 300 } else { 360 }
        );
    }
}
#[test]
fn window_snapshot_live_maxima_and_combined_formula_round_only_once() {
    for live in [false, true] {
        let (mut c, mut w) = setup();
        let mut r = window_rule();
        r["maximum_sampling"] = json!(if live { "live" } else { "snapshot" });
        r["hp"] = recovery(5000, 5000, 10000);
        r["spirit"] = recovery(5000, 5000, 10000);
        window(&mut c, "window", r);
        w.members[0].facts.hp_points = 1001;
        w.members[0].facts.spirit_points = 101;
        w.members[0].hp = 100;
        w.members[0].spirit = 10;
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own, "window", 0);
        hit(&mut w, &c, (Owner::Member(0), Owner::Npc(0)), (1, true), 30);
        w.members[0].facts.hp_points = 2001;
        w.members[0].facts.spirit_points = 201;
        let e = end(&mut w, &c, 300);
        assert!(ended(&e).iter().all(|e|matches!(e,Event::WindowRecordEnded{raw_hp,raw_spirit,..} if (*raw_hp,*raw_spirit)==if live{(1001,101)}else{(501,51)})));
        assert_eq!(
            (w.members[0].hp, w.members[0].spirit),
            if live { (1101, 111) } else { (601, 61) }
        );
    }
}
#[test]
fn window_large_record_and_recovery_do_not_clip_to_u32_before_ratio() {
    let (mut c, mut w) = setup();
    let mut r = window_rule();
    r["record_cap"] = json!(9007199254740991_u64);
    r["hp"] = recovery(1, 0, 10000);
    window(&mut c, "window", r);
    w.members[0].facts.hp_points = 1000000;
    w.members[0].hp = 1000;
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own.clone(), "window", 0);
    for _ in 0..2 {
        w.statuses.record_windows(
            (&status_entity(&own), &status_entity(&w.npcs[0].entity)),
            (u32::MAX, 0, false),
            30,
            &c.combat_rules.as_ref().unwrap().status_windows,
        );
    }
    assert_eq!(
        view(&w, "window").window.unwrap().recorded,
        2 * u64::from(u32::MAX)
    );
    let e = end(&mut w, &c, 300);
    assert!(
        ended(&e)
            .iter()
            .all(|e| matches!(e, Event::WindowRecordEnded { raw_hp: 858993, .. }))
    );
    assert_eq!(w.members[0].hp, 859993);
}
#[test]
fn window_expired_consumed_and_cleanup_reasons_have_independent_once_only_rewards() {
    for reason in [
        StatusRemovalReason::Expired,
        StatusRemovalReason::Consumed,
        StatusRemovalReason::Cleansed,
        StatusRemovalReason::Depleted,
        StatusRemovalReason::SourceInvalid,
        StatusRemovalReason::TargetInvalid,
        StatusRemovalReason::Reset,
        StatusRemovalReason::Closed,
    ] {
        for consume in [false, true] {
            let (mut c, mut w) = setup();
            let mut r = window_rule();
            r["basis"] = json!("none");
            r["record_cap"] = json!(0);
            r["on_consumed"] = json!(consume);
            r["hp"] = recovery(0, 1000, 10000);
            window(&mut c, "window", r);
            let own = w.members[0].reference.entity.clone();
            bind(&mut w, &c, Owner::Member(0), own, "window", 0);
            w.members[0].hp = 100;
            w.statuses
                .remove(reason, |s| s.definition_id == "window", &mut vec![]);
            let mut e = vec![];
            w.finish_window_records(&c, 30, &mut e);
            let permitted = reason == StatusRemovalReason::Expired
                || reason == StatusRemovalReason::Consumed && consume;
            assert_eq!(
                w.members[0].hp,
                if permitted { 200 } else { 100 },
                "{reason:?}/{consume}"
            );
            w.finish_window_records(&c, 60, &mut e);
            assert_eq!(ended(&e).len(), 1);
        }
    }
}
#[test]
fn window_none_end_recovers_hp_and_spirit_with_current_gaps_healing_and_lock() {
    let (mut c, mut w) = setup();
    w.members[0].facts.spirit_points = 200;
    let mut r = window_rule();
    r["basis"] = json!("none");
    r["record_cap"] = json!(0);
    r["hp"] = recovery(0, 1000, 10000);
    r["spirit"] = recovery(0, 1000, 10000);
    window(&mut c, "window", r);
    for (id, attr, bps) in [
        ("no-heal", "healing_received", -10000),
        ("spirit-lock", "spirit_recovery_lock", -10000),
    ] {
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":attr,"modifier_bps":bps,"duration_ms":600,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    }
    let own = w.members[0].reference.entity.clone();
    for id in ["window", "no-heal", "spirit-lock"] {
        bind(&mut w, &c, Owner::Member(0), own.clone(), id, 0);
    }
    w.members[0].hp = 500;
    w.members[0].spirit = 100;
    let e = end(&mut w, &c, 300);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (500, 100));
    assert!(ended(&e).iter().all(|e| matches!(
        e,
        Event::WindowRecordEnded {
            raw_hp: 100,
            hp_restored: 0,
            raw_spirit: 20,
            spirit_restored: 0,
            ..
        }
    )));
    end(&mut w, &c, 600);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (500, 100));
}
#[test]
fn window_multiple_casters_observe_holder_independently_and_source_exit_removes_only_own() {
    let (mut c, mut w) = setup();
    window(&mut c, "window", window_rule());
    let target = w.members[1].reference.entity.clone();
    for caster in [Owner::Member(0), Owner::Member(1)] {
        bind(&mut w, &c, caster, target.clone(), "window", 0);
    }
    w.members[1].hp = 100;
    hit(
        &mut w,
        &c,
        (Owner::Member(1), Owner::Npc(0)),
        (100, true),
        30,
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .filter_map(|s| s.window.as_ref())
            .all(|v| v.recorded == 100)
    );
    w.action(0, &Action::Leave {}, "exit", &c, 60).unwrap();
    let e = end(&mut w, &c, 300);
    assert_eq!(w.members[1].hp, 110);
    assert!(ended(&e).iter().all(|e|matches!(e,Event::WindowRecordEnded{source,hp_restored:10,..}if source==&status_entity(&target))));
}
#[test]
fn window_reconnect_keeps_record_deadline_and_protected_end_is_not_retried() {
    for early in [false, true] {
        let (mut c, mut w) = setup();
        window(&mut c, "window", window_rule());
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own, "window", 0);
        w.members[0].hp = 500;
        hit(
            &mut w,
            &c,
            (Owner::Member(0), Owner::Npc(0)),
            (100, true),
            30,
        );
        let before = view(&w, "window");
        w.protect(0, 60, &mut vec![]);
        if early {
            w.members[0].protected = false;
            w.members[0].ready = true;
            w.members[0].foreground = true;
        }
        assert_eq!(view(&w, "window"), before);
        let e = end(&mut w, &c, 300);
        assert_eq!(w.members[0].hp, if early { 510 } else { 500 });
        if !early {
            assert!(ended(&e).iter().all(|e| matches!(
                e,
                Event::WindowRecordEnded {
                    suppressed: Some(Error::InvalidState),
                    ..
                }
            )));
        }
        w.members[0].protected = false;
        w.members[0].ready = true;
        w.members[0].foreground = true;
        w.finish_window_records(&c, 330, &mut vec![]);
        assert_eq!(w.members[0].hp, if early { 510 } else { 500 });
    }
}
#[test]
fn window_generation_and_fatal_cleanup_cannot_rebind_or_resurrect() {
    for generation in [false, true] {
        let (mut c, mut w) = setup();
        let mut r = window_rule();
        r["basis"] = json!("damage_taken");
        window(&mut c, "window", r);
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own, "window", 0);
        w.members[0].hp = 20;
        if generation {
            w.members[0].reference.entity.generation += 1;
            let e = end(&mut w, &c, 300);
            assert!(ended(&e).iter().all(|e| matches!(
                e,
                Event::WindowRecordEnded {
                    suppressed: Some(Error::InvalidTarget),
                    ..
                }
            )));
        } else {
            let (_, e) = hit(
                &mut w,
                &c,
                (Owner::Npc(0), Owner::Member(0)),
                (100, true),
                30,
            );
            assert!(!w.members[0].living());
            assert!(ended(&e).iter().all(|e| matches!(
                e,
                Event::WindowRecordEnded {
                    raw_hp: 0,
                    hp_restored: 0,
                    ..
                }
            )));
        }
    }
}
#[test]
fn window_end_budget_consumes_suppressed_ends_without_delayed_recovery() {
    let (mut c, mut w) = setup();
    let mut r = window_rule();
    r["basis"] = json!("none");
    r["record_cap"] = json!(0);
    r["hp"] = recovery(0, 1000, 10000);
    window(&mut c, "window", r);
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_window_releases_per_step = 1;
    for i in 0..2 {
        let own = w.members[i].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(i), own, "window", 0);
        w.members[i].hp = 100;
    }
    let e = end(&mut w, &c, 300);
    assert_eq!((w.members[0].hp, w.members[1].hp), (200, 100));
    assert!(ended(&e).iter().any(|e| matches!(
        e,
        Event::WindowRecordEnded {
            suppressed: Some(Error::Capacity),
            ..
        }
    )));
    w.finish_window_records(&c, 330, &mut vec![]);
    assert_eq!(w.members[1].hp, 100);
}
#[test]
fn window_hot_carrier_preserves_periodic_identity_schedule_and_ally_ownership() {
    let (mut c, mut w) = setup();
    w.members[0].facts.spirit_points = 200;
    let mut r = window_rule();
    r["basis"] = json!("none");
    r["record_cap"] = json!(0);
    r["hp"] = recovery(0, 1000, 10000);
    r["spirit"] = recovery(0, 1000, 10000);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_windows
        .insert("renewal".into(), serde_json::from_value(r).unwrap());
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), own.clone(), "renewal", 0);
    let first = view(&w, "renewal");
    assert!(first.window.is_some() && first.periodic.is_some());
    let periodic = first.periodic.as_ref().unwrap();
    w.members[0].hp = 100;
    w.members[0].spirit = 0;
    let mut e = vec![];
    w.advance_periodic(&c, periodic.next_tick_at_ms.unwrap(), &mut e);
    assert!(ended(&e).is_empty());
    let at = first.expires_at_ms;
    let before = w.members[0].hp;
    let e = end(&mut w, &c, at);
    assert_eq!(w.members[0].hp, before + 100);
    assert_eq!(w.members[0].spirit, 20);
    assert!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::WindowRecordEnded { .. }
                }
            ))
            .all(|(i, _)| matches!(i, Some(0) | Some(1)))
    );
}
#[test]
fn window_nested_reflection_cannot_take_outer_pending_end_or_record_derived_damage() {
    let (mut c, mut w) = setup();
    let mut r = window_rule();
    r["on_consumed"] = json!(true);
    window(&mut c, "window", r);
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "window", 0);
    w.members[0].hp = 500;
    hit(
        &mut w,
        &c,
        (Owner::Member(0), Owner::Npc(0)),
        (100, true),
        30,
    );
    let id = view(&w, "window").instance_id;
    w.statuses.consume(&id, 1, &mut vec![]);
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"reflect","profiles":["npc"],"when":"damage_taken","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":10000,"damage_kind":"true_damage"}}),
    );
    let (_, mut e) = hit(
        &mut w,
        &c,
        (Owner::Member(0), Owner::Npc(0)),
        (100, true),
        60,
    );
    assert!(ended(&e).is_empty());
    assert_eq!(w.members[0].hp, 400);
    w.finish_window_records(&c, 60, &mut e);
    assert_eq!(w.members[0].hp, 410);
    assert!(ended(&e).iter().all(|e| matches!(
        e,
        Event::WindowRecordEnded {
            recorded: 100,
            raw_hp: 10,
            ..
        }
    )));
}
#[test]
fn window_recovery_reads_live_caps_after_temporary_carrier_is_removed_and_never_overflows() {
    let (mut c, mut w) = setup();
    w.members[0].facts.spirit_points = 200;
    let mut r = window_rule();
    r["basis"] = json!("none");
    r["record_cap"] = json!(0);
    r["maximum_sampling"] = json!("live");
    r["hp"] = recovery(0, 1000, 10000);
    r["spirit"] = recovery(0, 1000, 10000);
    window(&mut c, "window", r);
    let d = c.pve_mut().status_definitions.last_mut().unwrap();
    d.mechanism = crate::config::StatusMechanism::TimedCompoundModifier;
    d.attribute = None;
    d.modifier_bps = 0;
    d.modifiers=serde_json::from_value(json!([{"attribute":"max_hp","modifier_bps":10000},{"attribute":"max_spirit","modifier_bps":10000}])).unwrap();
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "window", 0);
    w.members[0].hp = 1900;
    w.members[0].spirit = 350;
    let e = end(&mut w, &c, 300);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (1000, 200));
    assert!(ended(&e).iter().all(|e| matches!(
        e,
        Event::WindowRecordEnded {
            raw_hp: 100,
            hp_restored: 0,
            raw_spirit: 20,
            spirit_restored: 0,
            ..
        }
    )));
}

#[test]
fn window_capacity_rejection_keeps_other_source_identity_record_and_deadline() {
    let (mut c, mut w) = setup();
    window(&mut c, "window", window_rule());
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own.clone(), "window", 0);
    hit(
        &mut w,
        &c,
        (Owner::Member(0), Owner::Npc(0)),
        (100, true),
        30,
    );
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let before = view(&w, "window");
    let e = bind(&mut w, &c, Owner::Member(1), own, "window", 60);
    assert_eq!(view(&w, "window"), before);
    assert!(e.iter().any(|(_, e)| matches!(
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
fn window_excludes_reflection_and_records_actual_guarded_loss_without_counting_guard_heal() {
    let (mut c, mut w) = setup();
    let mut r = window_rule();
    r["include_periodic"] = json!(true);
    r["basis"] = json!("damage_taken");
    window(&mut c, "taken", r.clone());
    r["basis"] = json!("damage_dealt");
    window(&mut c, "dealt", r);
    let own = w.members[0].reference.entity.clone();
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "taken", 0);
    bind(&mut w, &c, Owner::Npc(0), npc, "dealt", 0);
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        json!({"id":"reflect","profiles":["npc"],"when":"damage_taken","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":10000,"damage_kind":"true_damage"}}),
    );
    hit(
        &mut w,
        &c,
        (Owner::Member(0), Owner::Npc(0)),
        (100, true),
        30,
    );
    assert_eq!(view(&w, "taken").window.unwrap().recorded, 0);
    assert_eq!(view(&w, "dealt").window.unwrap().recorded, 0);
    guard_config(&mut c, &[], 1500);
    w.members[0].hp = 20;
    hit(
        &mut w,
        &c,
        (Owner::Npc(0), Owner::Member(0)),
        (100, true),
        60,
    );
    assert_eq!(view(&w, "taken").window.unwrap().recorded, 19);
    assert_eq!(view(&w, "dealt").window.unwrap().recorded, 19);
    assert_eq!(w.members[0].hp, 151);
}
#[test]
fn window_natural_end_precedes_same_step_periodic_damage_and_expiry_switch_is_independent() {
    let (mut c, mut w) = setup();
    let mut r = window_rule();
    r["basis"] = json!("none");
    r["record_cap"] = json!(0);
    r["hp"] = recovery(0, 1000, 10000);
    window(&mut c, "window", r);
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "wound")
        .unwrap();
    d.duration_ms = 600;
    d.tick_interval_ms = Some(300);
    d.damage_bps = Some(10000);
    c.combat_rules.as_mut().unwrap().periodic_damage.insert(
        "wound".into(),
        AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            can_crit: false,
            ..Default::default()
        },
    );
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own.clone(), "window", 0);
    bind(&mut w, &c, Owner::Npc(0), own, "wound", 0);
    w.members[0].hp = 5;
    let mut e = end(&mut w, &c, 300);
    w.advance_periodic(&c, 300, &mut e);
    // The NPC contributes 20 damage: expiry must restore 100 before that tick.
    assert_eq!(w.members[0].hp, 85);
    assert!(ended(&e).iter().any(|e| matches!(
        e,
        Event::WindowRecordEnded {
            raw_hp: 100,
            hp_restored: 100,
            ..
        }
    )));
    assert!(w.members[0].living());
    for consumed in [false, true] {
        let (mut c, mut w) = setup();
        let mut r = window_rule();
        r["on_expired"] = json!(false);
        r["on_consumed"] = json!(true);
        r["hp"] = recovery(0, 1000, 10000);
        window(&mut c, "window", r);
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own, "window", 0);
        w.members[0].hp = 100;
        if consumed {
            let id = view(&w, "window").instance_id;
            w.statuses.consume(&id, 1, &mut vec![]);
            w.finish_window_records(&c, 30, &mut vec![]);
        } else {
            end(&mut w, &c, 300);
        }
        assert_eq!(w.members[0].hp, if consumed { 200 } else { 100 });
    }
}
