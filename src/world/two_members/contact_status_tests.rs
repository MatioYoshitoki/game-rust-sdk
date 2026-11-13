use super::*;
fn mark() -> Value {
    json!({"id":"contact-mark","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":0,"duration_ms":6000,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true})
}
fn condition() -> Value {
    json!({"subject":"other","definition_id":"contact-mark","at_least":1,"consume_max":3,"damage_bonus_bps_per_stack":2500,"armor_penetration_bps_per_stack":500})
}
fn attack() -> AttackRule {
    serde_json::from_value(json!({"damage_kind":"physical","fixed":0,"cultivation_bps":10000,"armor_penetration_bps":0,"armor_penetration_cap_bps":5000,"life_steal_bps":0,"unavoidable":true,"ignore_armor":false,"can_crit":false,"contact_status":condition()})).unwrap()
}
fn fixture() -> (Config, World) {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(mark()).unwrap());
    c.combat_rules.as_mut().unwrap().triggers.clear();
    for p in c.combat_rules.as_mut().unwrap().profiles.values_mut() {
        p.tier = 0;
        p.power_points_bps = 0;
        p.dodge_bps = 0;
        p.critical_chance_bps = 0;
        p.defense_baseline_attack = 100;
    }
    c.params.defense_scale = 1.;
    c.defense_cap = 0.9;
    c.combat_rules.as_mut().unwrap().basic_attack = attack();
    let p = Arc::make_mut(&mut w.npcs[0].profile);
    p.stats.insert("hp".into(), 10000);
    p.stats.insert("def".into(), 100);
    w.npcs[0].hp = 10000;
    (c, w)
}
fn give(w: &mut World, c: &Config, source: Owner, target: Owner, layers: u32, now: u64) {
    for _ in 0..layers {
        let e = w.owner_entity(target);
        w.apply_owned_statuses(
            source,
            (e, 10000),
            &["contact-mark".into()],
            c,
            now,
            &mut vec![],
        );
    }
}
fn strike(
    w: &mut World,
    c: &Config,
    r: &AttackRule,
    actors: (Owner, Owner),
    raw: u32,
    now: u64,
    bindings: &[String],
) -> Events {
    let (source, target) = actors;
    let e = w.owner_entity(source);
    let mut events = vec![];
    w.direct_contact(
        source,
        target,
        DirectContact {
            origin: None,
            action_id: Some("contact-test"),
            rule: Some(r),
            ignore_armor: false,
            unavoidable: r.unavoidable,
            dead_source: false,
            source: &e,
            raw,
            metal: true,
            hit_statuses: bindings,
        },
        c,
        now,
        &mut events,
    );
    events
}
fn hit(w: &mut World, c: &Config, r: &AttackRule, raw: u32, now: u64) -> Events {
    strike(w, c, r, (Owner::Member(0), Owner::Npc(0)), raw, now, &[])
}
fn damage(events: &Events) -> Vec<u32> {
    events
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::Damage { amount, .. } = e {
                Some(*amount)
            } else {
                None
            }
        })
        .collect()
}
fn spent(events: &Events) -> Vec<u32> {
    events
        .iter()
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
fn existing(w: &World) -> Option<&StatusView> {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "contact-mark")
}
#[test]
fn contact_status_loader_roundtrips_samples_and_rejects_invalid_caps_refs_carriers_and_bounds() {
    let p = Package::new();
    for txt in [
        crate::test_data::fixture!("examples/combat-host/contact-status-a.json"),
        crate::test_data::fixture!("examples/combat-host/contact-status-b.json"),
    ] {
        let v: Value = serde_json::from_str(txt).unwrap();
        p.write(&v);
        assert!(p.load().is_ok());
    }
    let mut v = document();
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(mark());
    v["combat_rules"]["basic_attack"] = serde_json::to_value(attack()).unwrap();
    p.write(&v);
    assert!(p.load().is_ok());
    for case in 0..17 {
        let mut b = v.clone();
        let a = &mut b["combat_rules"]["basic_attack"];
        match case {
            0 => a["contact_status"] = Value::Null,
            1 => a["armor_penetration_cap_bps"] = Value::Null,
            2 => a["armor_penetration_cap_bps"] = json!(10001),
            3 => a["contact_status"]["definition_id"] = json!("missing"),
            4 => a["contact_status"]["consume_max"] = json!(4),
            5 => a["contact_status"]["at_least"] = json!(0),
            6 => a["contact_status"]["at_least"] = json!(4),
            7 => a["contact_status"]["subject"] = json!("any"),
            8 => a["contact_status"]["damage_bonus_bps_per_stack"] = json!(100001),
            9 => a["contact_status"]["armor_penetration_bps_per_stack"] = json!(5000),
            10 => {
                a.as_object_mut()
                    .unwrap()
                    .remove("armor_penetration_cap_bps");
            }
            11 => a["contact_status"]["definition_id"] = json!("barrier"),
            12 => {
                a["contact_status"]["damage_bonus_bps_per_stack"] = json!(0);
                a["contact_status"]["armor_penetration_bps_per_stack"] = json!(0);
            }
            13 => a["contact_status"]["unknown"] = json!(1),
            14 => b["mechanism_version"] = json!(104),
            15 => {
                let d = b["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap();
                d["source_scoped"] = json!(false);
            }
            _ => {
                b["combat_rules"]["periodic_damage"]["wound"] =
                    b["combat_rules"]["basic_attack"].clone();
            }
        }
        p.write(&b);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn contact_status_original_sword_coefficients_bind_to_real_native_damage() {
    let vectors: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_contact_vectors.json"
    ))
    .unwrap();
    assert_eq!(
        vectors["sourceSha256"],
        "318480dc7cd726c7aea49cbace50e8c196de95d2914c9df033e6bd0f3a73e322"
    );
    for v in vectors["vectors"].as_array().unwrap() {
        let (c, mut w) = fixture();
        give(
            &mut w,
            &c,
            Owner::Member(0),
            Owner::Npc(0),
            v["stacks"].as_u64().unwrap() as u32,
            0,
        );
        let r = attack();
        let events = hit(&mut w, &c, &r, 100, 30);
        assert_eq!(damage(&events), vec![v["damage"].as_u64().unwrap() as u32]);
        assert_eq!(spent(&events), vec![v["stacks"].as_u64().unwrap() as u32]);
        assert!(existing(&w).is_none());
    }
}
#[test]
fn contact_status_partial_consumption_keeps_original_identity_and_deadline() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 3, 0);
    let before = existing(&w).unwrap().clone();
    let mut r = attack();
    r.contact_status.as_mut().unwrap().consume_max = 2;
    let e = hit(&mut w, &c, &r, 100, 100);
    assert_eq!(damage(&e), vec![78]);
    assert_eq!(spent(&e), vec![2]);
    assert_eq!(
        existing(&w),
        Some(&StatusView {
            retained_source: None,
            base_modifiers: vec![],
            stack_counter: None,
            stacks: Some(1),
            ..before
        })
    );
    assert_eq!(damage(&hit(&mut w, &c, &r, 100, 200)), vec![64]);
    assert_eq!(damage(&hit(&mut w, &c, &r, 100, 300)), vec![50]);
}
#[test]
fn contact_status_threshold_failure_uses_base_attack_without_consumption() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 0);
    let mut r = attack();
    r.contact_status.as_mut().unwrap().at_least = 2;
    let e = hit(&mut w, &c, &r, 100, 30);
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
    assert_eq!(existing(&w).unwrap().stacks, Some(1));
}
#[test]
fn contact_status_never_borrows_another_sources_marks() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(1), Owner::Npc(0), 3, 0);
    let e = hit(&mut w, &c, &attack(), 100, 30);
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
    assert_eq!(existing(&w).unwrap().stacks, Some(3));
}
#[test]
fn contact_status_owner_subject_consumes_only_the_attackers_own_status() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Member(0), 2, 0);
    let mut r = attack();
    r.contact_status.as_mut().unwrap().subject = crate::config::TriggerSubject::Owner;
    assert_eq!(damage(&hit(&mut w, &c, &r, 100, 30)), vec![78]);
    assert!(existing(&w).is_none());
}
#[test]
fn contact_status_expired_marks_and_invalid_generation_never_grant_damage() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 3, 0);
    let e = hit(&mut w, &c, &attack(), 100, 6000);
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 0);
    w.npcs[0].entity.generation += 1;
    let e = hit(&mut w, &c, &attack(), 100, 30);
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
}
#[test]
fn contact_status_immune_or_returning_contact_never_consumes_marks() {
    for immune in [false, true] {
        let (mut c, mut w) = fixture();
        give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 2, 0);
        if immune {
            c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"contact-immune","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
            let e = w.npcs[0].entity.clone();
            w.apply_owned_statuses(
                Owner::Npc(0),
                (e, 10000),
                &["contact-immune".into()],
                &c,
                0,
                &mut vec![],
            );
        } else {
            w.npcs[0].returning = true;
        }
        let e = hit(&mut w, &c, &attack(), 100, 30);
        assert!(damage(&e).is_empty());
        assert!(spent(&e).is_empty());
        assert_eq!(existing(&w).unwrap().stacks, Some(2));
    }
}
#[test]
fn contact_status_dodge_precedes_consumption_and_draws_only_existing_rng() {
    let (mut c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 3, 0);
    let r = c.combat_rules.as_mut().unwrap();
    r.dodge_cap_bps = Some(10000);
    r.profiles.get_mut("npc").unwrap().dodge_bps = 7000;
    r.profiles.get_mut("member").unwrap().hit_bps = 0;
    let mut a = attack();
    a.unavoidable = false;
    w.rng = 1;
    let e = hit(&mut w, &c, &a, 100, 30);
    assert!(damage(&e).is_empty());
    assert!(spent(&e).is_empty());
    assert_eq!(existing(&w).unwrap().stacks, Some(3));
    assert_eq!(w.rng, 270369);
}
#[test]
fn contact_status_full_shield_absorption_still_consumes_before_shield_and_hp() {
    let (mut c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 0);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"contact-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let n = w.npcs[0].entity.clone();
    w.apply_owned_statuses(
        Owner::Npc(0),
        (n, 10000),
        &["contact-shield".into()],
        &c,
        0,
        &mut vec![],
    );
    let e = hit(&mut w, &c, &attack(), 100, 30);
    assert_eq!(damage(&e), vec![0]);
    assert_eq!(spent(&e), vec![1]);
    let used = e
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
    let absorbed = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::ShieldAbsorbed { .. }
                }
            )
        })
        .unwrap();
    let hp = e
        .iter()
        .position(|(_, e)| matches!(e, EventV4::Damage { .. }))
        .unwrap();
    assert!(used < absorbed && absorbed < hp);
}
#[test]
fn contact_status_wide_sum_rounds_once_and_saturates_only_final_raw() {
    for (raw, expected) in [(3, 5), (u32::MAX, u32::MAX), (0, 0)] {
        let (c, mut w) = fixture();
        give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 3, 0);
        let mut r = attack();
        r.damage_kind = crate::config::DamageKind::TrueDamage;
        let result = w
            .contact_status_modifier(
                (Owner::Member(0), Owner::Npc(0), None),
                raw,
                &r,
                &c,
                30,
                &mut vec![],
            )
            .unwrap()
            .raw;
        assert_eq!(result, expected);
    }
}
#[test]
fn contact_status_final_cap_combines_static_mark_and_live_attribute_penetration() {
    let (mut c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 3, 0);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"live-penetration","mechanism":"timed_attribute_modifier","attribute":"armor_penetration","modifier_bps":2000,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let s = w.members[0].reference.entity.clone();
    w.apply_owned_statuses(
        Owner::Member(0),
        (s, 1000),
        &["live-penetration".into()],
        &c,
        0,
        &mut vec![],
    );
    let mut r = attack();
    r.armor_penetration_bps = 2000;
    assert_eq!(damage(&hit(&mut w, &c, &r, 100, 30)), vec![116]);
}
#[test]
fn contact_status_does_not_modify_frozen_config_or_the_next_unmarked_contact() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 0);
    let a = attack();
    let before = serde_json::to_value(&a).unwrap();
    assert_eq!(damage(&hit(&mut w, &c, &a, 100, 30)), vec![64]);
    assert_eq!(damage(&hit(&mut w, &c, &a, 100, 60)), vec![50]);
    assert_eq!(serde_json::to_value(a).unwrap(), before);
}
#[test]
fn contact_status_marks_granted_after_launch_are_sampled_at_actual_impact() {
    let (mut c, mut w) = fixture();
    c.skill = serde_json::from_value(skill("unit_projectile")).unwrap();
    c.combat_rules
        .as_mut()
        .unwrap()
        .skills
        .insert(c.skill.id.clone(), attack());
    commit(&mut w, &c, "unit_projectile");
    assert_eq!(w.shots[0].raw, 100);
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 45);
    let mut e = vec![];
    w.advance_shots(&c, 60, 1., &mut e);
    assert_eq!(damage(&e), vec![64]);
    assert_eq!(spent(&e), vec![1]);
}
#[test]
fn contact_status_expiring_in_flight_is_not_preconsumed_or_cached_into_the_shot() {
    let (mut c, mut w) = fixture();
    c.pve_mut()
        .status_definitions
        .last_mut()
        .unwrap()
        .duration_ms = 45;
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 0);
    c.skill = serde_json::from_value(skill("unit_projectile")).unwrap();
    c.combat_rules
        .as_mut()
        .unwrap()
        .skills
        .insert(c.skill.id.clone(), attack());
    commit(&mut w, &c, "unit_projectile");
    assert_eq!(existing(&w).unwrap().stacks, Some(1));
    let mut e = vec![];
    w.advance_shots(&c, 60, 1., &mut e);
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
}
#[test]
fn contact_status_two_in_flight_and_immediate_contacts_cannot_spend_one_mark_twice() {
    let (mut c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 0);
    c.skill = serde_json::from_value(skill("unit_projectile")).unwrap();
    c.combat_rules
        .as_mut()
        .unwrap()
        .skills
        .insert(c.skill.id.clone(), attack());
    commit(&mut w, &c, "unit_projectile");
    assert_eq!(damage(&hit(&mut w, &c, &attack(), 100, 45)), vec![64]);
    let mut e = vec![];
    w.advance_shots(&c, 60, 1., &mut e);
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
}
#[test]
fn contact_status_basic_and_six_native_plan_families_share_contact_consumption() {
    for kind in [
        "basic",
        "projectile",
        "unit_projectile",
        "piercing_projectile",
        "ground_circle",
        "ground_field",
        "line_field",
    ] {
        let (mut c, mut w) = fixture();
        give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 0);
        c.skill = serde_json::from_value(skill(if kind == "basic" { "projectile" } else { kind }))
            .unwrap();
        c.combat_rules
            .as_mut()
            .unwrap()
            .skills
            .insert(c.skill.id.clone(), attack());
        commit(&mut w, &c, kind);
        let mut e = vec![];
        if matches!(kind, "ground_circle" | "ground_field" | "line_field") {
            w.advance_areas(&c, 150, &mut e);
        } else {
            w.advance_shots(&c, 60, 1., &mut e);
        }
        assert_eq!(damage(&e), vec![64], "{kind}");
        assert_eq!(spent(&e), vec![1], "{kind}");
        if matches!(kind, "ground_field" | "line_field") {
            let mut next = vec![];
            w.advance_areas(&c, 270, &mut next);
            assert_eq!(damage(&next), vec![50], "{kind}");
            assert!(spent(&next).is_empty());
        }
    }
}
#[test]
fn contact_status_npc_attacker_preserves_native_source_and_target_attribution() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Npc(0), Owner::Member(0), 1, 0);
    let e = strike(
        &mut w,
        &c,
        &attack(),
        (Owner::Npc(0), Owner::Member(0)),
        100,
        30,
        &[],
    );
    assert_eq!(damage(&e), vec![64]);
    assert_eq!(spent(&e), vec![1]);
    assert!(e.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::StatusConsumed{source,target,..}} if source.kind==StatusEntityKind::Monster&&target.id=="P00")));
}
#[test]
fn contact_status_on_hit_new_mark_cannot_retroactively_boost_the_same_hit() {
    let (c, mut w) = fixture();
    let e = strike(
        &mut w,
        &c,
        &attack(),
        (Owner::Member(0), Owner::Npc(0)),
        100,
        30,
        &["contact-mark".into()],
    );
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
    assert_eq!(existing(&w).unwrap().stacks, Some(1));
}
#[test]
fn contact_status_periodic_and_derived_damage_do_not_consume_or_amplify_marks() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 1, 0);
    let mut e = vec![];
    w.resolve_damage_with_rule(intent(&w, 100, false), Some(&attack()), &c, 30, &mut e);
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
    w.triggers.executing = true;
    let e = hit(&mut w, &c, &attack(), 100, 60);
    assert_eq!(damage(&e), vec![50]);
    assert!(spent(&e).is_empty());
    assert_eq!(existing(&w).unwrap().stacks, Some(1));
}
#[test]
fn contact_status_consumed_own_maximum_clamps_current_resources_before_the_damage_chain() {
    let (mut c, mut w) = fixture();
    let d = c.pve_mut().status_definitions.last_mut().unwrap();
    d.attribute = Some(StatusAttribute::MaxHp);
    d.modifier_bps = 10000;
    give(&mut w, &c, Owner::Member(0), Owner::Member(0), 1, 0);
    w.members[0].hp = 2000;
    let mut a = attack();
    a.contact_status.as_mut().unwrap().subject = crate::config::TriggerSubject::Owner;
    hit(&mut w, &c, &a, 100, 30);
    assert_eq!(w.members[0].hp, 1000);
    assert!(existing(&w).is_none());
}
#[test]
fn contact_status_lifesteal_and_damage_events_use_only_final_actual_hp_damage() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 3, 0);
    w.npcs[0].hp = 20;
    w.members[0].hp = 100;
    let mut r = attack();
    r.life_steal_bps = 5000;
    let e = hit(&mut w, &c, &r, 100, 30);
    assert_eq!(damage(&e), vec![20]);
    assert_eq!(w.members[0].hp, 110);
    assert_eq!(spent(&e), vec![3]);
}

#[test]
fn contact_status_minimum_threshold_and_consumption_limit_are_independent() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), Owner::Npc(0), 3, 0);
    let mut r = attack();
    let condition = r.contact_status.as_mut().unwrap();
    condition.at_least = 3;
    condition.consume_max = 1;
    let first = hit(&mut w, &c, &r, 100, 30);
    assert_eq!(damage(&first), vec![64]);
    assert_eq!(spent(&first), vec![1]);
    assert_eq!(existing(&w).unwrap().stacks, Some(2));
    let next = hit(&mut w, &c, &r, 100, 60);
    assert_eq!(damage(&next), vec![50]);
    assert!(spent(&next).is_empty());
}
