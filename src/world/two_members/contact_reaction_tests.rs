use super::*;
use crate::config::{ContactReactionRule, ContactReactionWhen, ReactionAttacker, TriggerEffect};
fn fixture() -> (Config, World) {
    let p = Package::new();
    p.write(
        &serde_json::from_str::<Value>(crate::test_data::fixture!(
            "examples/combat-host/reaction-a.json"
        ))
        .unwrap(),
    );
    let mut c = p.load().unwrap().default_config().as_ref().clone();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    let seats = (0..4)
        .map(|i| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            combat_build: None,
            skills: None,
            participant_id: format!("P{i:02}"),
            authenticated_principal: format!("principal-P{i:02}"),
            connection: Connection {
                id: format!("c{i}"),
                generation: 1,
            },
        })
        .collect::<Vec<_>>();
    let facts = seats
        .iter()
        .map(|s| ParticipantFacts {
            principal: s.authenticated_principal.clone(),
            revision: 0,
            hp_points: 1000,
            spirit_points: 300,
            atk_points: 100,
            def_points: 100,
            spd_points: 8,
            warehouse_quantity: 100,
        })
        .collect();
    let mut w = World::new(&c, &seats, facts, 77);
    for m in &mut w.members {
        m.joined = true;
        m.ready = true;
    }
    w.activate(&c, 0, &mut vec![]);
    (c, w)
}
fn npc(w: &World, id: &str) -> usize {
    w.npcs.iter().position(|n| n.entity.id == id).unwrap()
}
fn add(c: &mut Config, d: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
}
fn give(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str, now: u64) {
    let t = w.owner_entity(target);
    w.apply_owned_statuses(source, (t, 10000), &[id.into()], c, now, &mut vec![]);
}
fn freeze(w: &mut World, c: &Config, source: usize) {
    give(
        w,
        c,
        Owner::Member(source),
        Owner::Npc(npc(w, "capped-target")),
        "compound-lock",
        0,
    );
}
fn rule(c: &mut Config) -> &mut ContactReactionRule {
    c.combat_rules
        .as_mut()
        .unwrap()
        .contact_reactions
        .get_mut("compound-lock")
        .unwrap()
}
fn strike(
    w: &mut World,
    c: &Config,
    source: Owner,
    target: Owner,
    raw: u32,
    now: u64,
    ids: &[String],
) -> Events {
    let mut events = vec![];
    let e = w.owner_entity(source);
    w.direct_contact(
        source,
        target,
        DirectContact {
            origin: None,
            action_id: Some("reaction-strike"),
            dead_source: false,
            source: &e,
            raw,
            rule: Some(&AttackRule {
                damage_kind: crate::config::DamageKind::TrueDamage,
                unavoidable: true,
                can_crit: false,
                ..Default::default()
            }),
            ignore_armor: false,
            unavoidable: true,
            metal: true,
            hit_statuses: ids,
        },
        c,
        now,
        &mut events,
    );
    events
}
fn hit(w: &mut World, c: &Config, raw: u32, now: u64) -> Events {
    strike(
        w,
        c,
        Owner::Member(0),
        Owner::Npc(npc(w, "capped-target")),
        raw,
        now,
        &[],
    )
}
fn results(e: &Events) -> Vec<(StatusEntity, StatusEntity, StatusEntity, u32, u32, u32)> {
    e.iter()
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event:
                    Event::ContactReactionResolved {
                        source,
                        target,
                        provoker,
                        raw_damage,
                        hp_damage,
                        absorbed,
                        ..
                    },
            } => Some((
                source.clone(),
                target.clone(),
                provoker.clone(),
                *raw_damage,
                *hp_damage,
                *absorbed,
            )),
            _ => None,
        })
        .collect()
}
fn held(w: &World) -> usize {
    w.statuses
        .entries()
        .iter()
        .filter(|s| s.definition_id == "compound-lock")
        .count()
}
fn shield(c: &mut Config) {
    add(
        c,
        json!({"id":"full-shield","mechanism":"absorption_shield","shield_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
}
#[test]
fn reaction_configs_roundtrip_and_reject_invalid_carriers_formulas_budgets() {
    for input in [
        crate::test_data::fixture!("examples/combat-host/reaction-a.json"),
        crate::test_data::fixture!("examples/combat-host/reaction-b.json"),
    ] {
        let p = Package::new();
        p.write(&serde_json::from_str::<Value>(input).unwrap());
        let c = p.load().unwrap();
        p.write(&serde_json::to_value(c.default_config().as_ref()).unwrap());
        assert!(p.load().is_ok());
    }
    for case in 0..22 {
        let p = Package::new();
        let mut v: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/reaction-a.json"
        ))
        .unwrap();
        match case {
            0 => {
                v["combat_rules"]["contact_reactions"]["unknown"] =
                    v["combat_rules"]["contact_reactions"]["compound-lock"].clone()
            }
            1 => v["combat_rules"]["contact_reactions"]["compound-lock"] = Value::Null,
            2 => v["combat_rules"]["contact_reactions"]["compound-lock"]["typo"] = json!(true),
            3 => v["combat_rules"]["contact_reactions"]["compound-lock"]["when"] = json!("unknown"),
            4 => v["combat_rules"]["contact_reactions"]["compound-lock"]["attacker"] = Value::Null,
            5 => {
                v["combat_rules"]["contact_reactions"]["compound-lock"]["source_power_bps"] =
                    json!(0)
            }
            6 => {
                v["combat_rules"]["contact_reactions"]["compound-lock"]["source_power_bps"] =
                    json!(100001)
            }
            7 => v["combat_rules"]["contact_reactions"]["compound-lock"]["fixed"] = json!(1000001),
            8 => {
                v["combat_rules"]["contact_reactions"]["compound-lock"]["armor_penetration_cap_bps"] =
                    json!(10001)
            }
            9 => {
                v["combat_rules"]["contact_reactions"]["compound-lock"]["element"] = json!("water")
            }
            10 => {
                v["combat_rules"]["contact_reactions"]["compound-lock"]["damage_kind"] =
                    json!("elemental")
            }
            11 => v["combat_rules"]["max_contact_reactions_per_hit"] = json!(0),
            12 => v["combat_rules"]["max_contact_reactions_per_hit"] = json!(5),
            13 => v["combat_rules"]["max_contact_reactions_per_step"] = json!(0),
            14 => v["combat_rules"]["max_contact_reactions_per_step"] = json!(129),
            15 => v["combat_rules"]["max_contact_reactions_per_step"] = Value::Null,
            16 => {
                v["combat_rules"]["status_policies"]["compound-lock"]["disposition"] =
                    json!("beneficial")
            }
            17 => {
                v["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|d| d["id"] == "compound-lock")
                    .unwrap()["source_scoped"] = json!(false)
            }
            18 => {
                v["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|d| d["id"] == "compound-lock")
                    .unwrap()["flags"] = json!(["damage_immunity"])
            }
            19 => {
                v["combat_rules"]["contact_reactions"]["threshold-mark"] =
                    v["combat_rules"]["contact_reactions"]["compound-lock"].clone()
            }
            20 => {
                v["combat_rules"]["skills"]["mark-bolt"]["contact_status"] = json!({"subject":"other","definition_id":"compound-lock","at_least":1,"consume_max":1,"damage_bonus_bps_per_stack":100,"armor_penetration_bps_per_stack":0})
            }
            _ => v["mechanism_version"] = json!(104),
        };
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn third_party_direct_damage_consumes_once_and_credits_original_source() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    let j = npc(&w, "capped-target");
    let hp = w.npcs[j].hp;
    let rng = w.rng;
    let e = hit(&mut w, &c, 100, 100);
    let v = results(&e);
    assert_eq!(v.len(), 1);
    assert_eq!(
        (&v[0].0.id, &v[0].2.id),
        (&"P01".to_owned(), &"P00".to_owned())
    );
    assert_eq!((v[0].3, v[0].4), (25, 25));
    assert_eq!(w.npcs[j].hp, hp - 125);
    assert_eq!(held(&w), 0);
    assert!(results(&hit(&mut w, &c, 100, 100)).is_empty());
    assert_eq!(w.rng, rng);
}
#[test]
fn original_25_percent_coefficient_drives_native_source_power() {
    let (c, mut w) = fixture();
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_reaction_vectors.json"
    ))
    .unwrap();
    freeze(&mut w, &c, 0);
    let e = hit(&mut w, &c, 1, 100);
    assert_eq!(json!(results(&e)[0].3), v["rawDamage"]);
}
#[test]
fn source_only_policy_does_not_borrow_teammate_consumption() {
    let (mut c, mut w) = fixture();
    rule(&mut c).attacker = ReactionAttacker::SourceOnly;
    freeze(&mut w, &c, 1);
    assert!(results(&hit(&mut w, &c, 1, 100)).is_empty());
    assert_eq!(held(&w), 1);
    let j = npc(&w, "capped-target");
    assert_eq!(
        results(&strike(
            &mut w,
            &c,
            Owner::Member(1),
            Owner::Npc(j),
            1,
            100,
            &[]
        ))
        .len(),
        1
    );
}
#[test]
fn newly_bound_control_survives_the_same_direct_hit() {
    let (c, mut w) = fixture();
    let j = npc(&w, "capped-target");
    let e = strike(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Npc(j),
        100,
        100,
        &["compound-lock".into()],
    );
    assert!(results(&e).is_empty());
    assert_eq!(held(&w), 1);
    assert_eq!(results(&hit(&mut w, &c, 100, 100)).len(), 1);
}
#[test]
fn existing_reaction_ends_before_this_contacts_new_binding() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    let j = npc(&w, "capped-target");
    let e = strike(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Npc(j),
        100,
        100,
        &["compound-lock".into()],
    );
    assert_eq!(results(&e).len(), 1);
    assert_eq!(held(&w), 1);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "compound-lock")
            .unwrap()
            .source
            .id,
        "P00"
    );
}
#[test]
fn hp_damage_mode_preserves_state_when_shield_absorbs_all_primary_damage() {
    let (mut c, mut w) = fixture();
    shield(&mut c);
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "full-shield", 0);
    freeze(&mut w, &c, 1);
    let e = hit(&mut w, &c, 100, 100);
    assert!(results(&e).is_empty());
    assert_eq!(held(&w), 1);
    assert_eq!(w.npcs[j].hp, 10000);
}
#[test]
fn direct_contact_mode_consumes_even_with_full_primary_absorption() {
    let (mut c, mut w) = fixture();
    rule(&mut c).when = ContactReactionWhen::DirectContact;
    shield(&mut c);
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "full-shield", 0);
    freeze(&mut w, &c, 1);
    let e = hit(&mut w, &c, 100, 100);
    let r = results(&e);
    assert_eq!((r[0].3, r[0].4, r[0].5), (25, 0, 25));
    assert_eq!(held(&w), 0);
}
#[test]
fn live_original_power_includes_attack_skill_bonus_and_profile_points_once() {
    let (mut c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    w.members[0].facts.atk_points = 999;
    w.members[1].facts.atk_points = 200;
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("member")
        .unwrap()
        .power_points_bps = 5000;
    add(
        &mut c,
        json!({"id":"power-test","mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","modifier_bps":10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    give(
        &mut w,
        &c,
        Owner::Member(1),
        Owner::Member(1),
        "power-test",
        0,
    );
    let e = hit(&mut w, &c, 1, 100);
    assert_eq!(results(&e)[0].3, 150);
}
#[test]
fn fixed_and_power_formula_round_once_and_saturate_u32() {
    let (mut c, _) = fixture();
    let r = rule(&mut c);
    r.fixed = 7;
    r.source_power_bps = 3333;
    assert_eq!(r.amount(101), 40);
    r.fixed = 1000000;
    r.source_power_bps = 100000;
    assert_eq!(r.amount(u32::MAX), u32::MAX);
}
#[test]
fn zero_rounded_budget_preserves_state_for_later_real_hit() {
    let (mut c, mut w) = fixture();
    rule(&mut c).source_power_bps = 1;
    freeze(&mut w, &c, 1);
    assert!(results(&hit(&mut w, &c, 1, 100)).is_empty());
    assert_eq!(held(&w), 1);
    w.members[1].facts.atk_points = 20000;
    assert_eq!(results(&hit(&mut w, &c, 1, 101))[0].3, 2);
}
#[test]
fn per_hit_cap_keeps_other_source_instance_for_next_contact() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_contact_reactions_per_hit = 1;
    freeze(&mut w, &c, 0);
    freeze(&mut w, &c, 1);
    let first = hit(&mut w, &c, 1, 100);
    assert_eq!(results(&first).len(), 1);
    assert_eq!(held(&w), 1);
    assert_eq!(results(&hit(&mut w, &c, 1, 100)).len(), 1);
    assert_eq!(held(&w), 0);
}
#[test]
fn step_budget_is_shared_and_does_not_schedule_unconsumed_states() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_contact_reactions_per_step = 1;
    freeze(&mut w, &c, 0);
    freeze(&mut w, &c, 1);
    assert_eq!(results(&hit(&mut w, &c, 1, 100)).len(), 1);
    assert_eq!(held(&w), 1);
    assert!(results(&hit(&mut w, &c, 1, 100)).is_empty());
    assert_eq!(held(&w), 1);
    assert_eq!(results(&hit(&mut w, &c, 1, 101)).len(), 1);
}
#[test]
fn stable_expiry_then_application_then_instance_order_determines_prefix() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_contact_reactions_per_hit = 1;
    let j = npc(&w, "capped-target");
    give(
        &mut w,
        &c,
        Owner::Member(1),
        Owner::Npc(j),
        "compound-lock",
        20,
    );
    give(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Npc(j),
        "compound-lock",
        0,
    );
    assert_eq!(results(&hit(&mut w, &c, 1, 100))[0].0.id, "P00");
}
#[test]
fn expired_state_and_stale_source_generation_cannot_release_damage() {
    for stale in [false, true] {
        let (c, mut w) = fixture();
        freeze(&mut w, &c, 1);
        if stale {
            w.members[1].reference.entity.generation += 1;
        }
        assert!(results(&hit(&mut w, &c, 1, if stale { 100 } else { 2000 })).is_empty());
    }
}
#[test]
fn protected_original_source_preserves_state_until_eligible_again() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    w.members[1].protected = true;
    assert!(results(&hit(&mut w, &c, 1, 100)).is_empty());
    assert_eq!(held(&w), 1);
    w.members[1].protected = false;
    assert_eq!(results(&hit(&mut w, &c, 1, 101)).len(), 1);
}
#[test]
fn primary_lethal_cleanup_does_not_release_a_second_damage() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 1;
    assert!(results(&hit(&mut w, &c, 100, 100)).is_empty());
    assert_eq!(held(&w), 0);
}
#[test]
fn primary_damage_trigger_can_remove_captured_state_before_reaction() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers.push(serde_json::from_value(json!({"id":"remove-first","profiles":["member"],"when":"damage_dealt","include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"dispel_statuses","subject":"other","disposition":"harmful","max_count":1}})).unwrap());
    freeze(&mut w, &c, 1);
    assert!(results(&hit(&mut w, &c, 1, 100)).is_empty());
    assert_eq!(held(&w), 0);
}
#[test]
fn reaction_uses_native_physical_defense_after_consuming_carrier() {
    let (mut c, mut w) = fixture();
    rule(&mut c).damage_kind = crate::config::DamageKind::Physical;
    let j = npc(&w, "capped-target");
    Arc::make_mut(&mut w.npcs[j].profile)
        .stats
        .insert("def".into(), 100);
    freeze(&mut w, &c, 1);
    assert_eq!(results(&hit(&mut w, &c, 1, 100))[0].4, 12);
}
#[test]
fn reaction_uses_elemental_resistance_and_not_primary_attacker_stats() {
    let (mut c, mut w) = fixture();
    rule(&mut c).damage_kind = crate::config::DamageKind::Elemental;
    rule(&mut c).element = Some("water".into());
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("ordinary-control")
        .unwrap()
        .element_resistance = Some(100);
    freeze(&mut w, &c, 1);
    assert_eq!(results(&hit(&mut w, &c, 1, 100))[0].4, 12);
}
#[test]
fn reduction_then_shield_then_hp_is_shared_by_derived_reaction() {
    let (mut c, mut w) = fixture();
    rule(&mut c).damage_kind = crate::config::DamageKind::Physical;
    rule(&mut c).ignore_armor = true;
    add(
        &mut c,
        json!({"id":"reduce","mechanism":"timed_attribute_modifier","attribute":"damage_reduction","modifier_bps":5000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "reduce", 0);
    freeze(&mut w, &c, 1);
    let e = hit(&mut w, &c, 100, 100);
    assert_eq!(results(&e)[0].4, 12);
    assert_eq!(w.npcs[j].hp, 9888);
}
#[test]
fn damage_immunity_stops_primary_and_preserves_reaction_state() {
    let (mut c, mut w) = fixture();
    add(
        &mut c,
        json!({"id":"immune","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let j = npc(&w, "capped-target");
    freeze(&mut w, &c, 1);
    give(&mut w, &c, Owner::Npc(j), Owner::Npc(j), "immune", 0);
    assert!(results(&hit(&mut w, &c, 100, 100)).is_empty());
    assert_eq!(held(&w), 1);
}
#[test]
fn derived_contacts_do_not_recursively_consume_states_or_generate_triggers() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    w.triggers.executing = true;
    assert!(results(&hit(&mut w, &c, 100, 100)).is_empty());
    assert_eq!(held(&w), 1);
}
#[test]
fn periodic_damage_does_not_enter_the_reaction_capture_path() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    let source = w.owner_entity(Owner::Member(0));
    let target = w.owner_entity(Owner::Npc(npc(&w, "capped-target")));
    let mut e = vec![];
    w.resolve_damage(
        DamageIntent {
            source,
            target,
            raw: 100,
            direct: false,
            ignore_armor: true,
        },
        &c,
        100,
        &mut e,
    );
    assert!(results(&e).is_empty());
    assert_eq!(held(&w), 1);
}
#[test]
fn ordinary_dispel_and_expiration_remove_without_release() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    let mut e = vec![];
    let j = npc(&w, "capped-target");
    w.execute_status_operation(
        (Owner::Member(0), Owner::Npc(j)),
        &TriggerEffect::DispelStatuses {
            subject: crate::config::TriggerSubject::Other,
            disposition: crate::config::StatusDisposition::Harmful,
            max_count: 1,
        },
        &c,
        100,
        &mut e,
    );
    assert!(results(&e).is_empty());
    assert_eq!(held(&w), 0);
    freeze(&mut w, &c, 1);
    w.statuses.expire(2000, &mut vec![]);
    assert!(results(&hit(&mut w, &c, 1, 2001)).is_empty());
}
#[test]
fn transferred_control_reacts_only_on_current_holder_and_preserves_source() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    let old = npc(&w, "capped-target");
    let new = npc(&w, "reaction-spare");
    let before = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "compound-lock")
        .unwrap()
        .clone();
    let mut e = vec![];
    w.execute_status_operation(
        (Owner::Npc(old), Owner::Npc(new)),
        &TriggerEffect::TransferStatuses {
            from: crate::config::TriggerSubject::Owner,
            disposition: crate::config::StatusDisposition::Harmful,
            max_count: 1,
        },
        &c,
        100,
        &mut e,
    );
    let after = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.instance_id == before.instance_id)
        .unwrap();
    assert_eq!(after.source, before.source);
    assert_eq!(after.expires_at_ms, before.expires_at_ms);
    assert_eq!(after.target.id, "reaction-spare");
    assert!(results(&hit(&mut w, &c, 1, 101)).is_empty());
    assert_eq!(
        results(&strike(
            &mut w,
            &c,
            Owner::Member(0),
            Owner::Npc(new),
            1,
            101,
            &[]
        ))[0]
            .1
            .id,
        "reaction-spare"
    );
}
#[test]
fn target_profile_replacement_does_not_inherit_original_control_reaction() {
    let (c, mut w) = fixture();
    let j = npc(&w, "replacement-target");
    give(
        &mut w,
        &c,
        Owner::Member(1),
        Owner::Npc(j),
        "compound-lock",
        0,
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.target.id == "replacement-target" && s.definition_id == "control-softened")
    );
    assert!(
        results(&strike(
            &mut w,
            &c,
            Owner::Member(0),
            Owner::Npc(j),
            1,
            100,
            &[]
        ))
        .is_empty()
    );
}
#[test]
fn third_party_does_not_receive_original_private_reaction_or_consumption() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    let e = hit(&mut w, &c, 1, 100);
    let recipients=e.iter().filter_map(|(i,e)|if matches!(e,EventV4::Private{event:Event::ContactReactionResolved{definition_id,..}|Event::StatusConsumed{definition_id,..}} if definition_id=="compound-lock"){Some(*i)}else{None}).collect::<Vec<_>>();
    assert_eq!(recipients, vec![Some(1), Some(1)]);
}

#[test]
fn reaction_damage_respects_the_shared_once_only_death_guard() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().death_guards.insert(
        "ordinary-control".into(),
        serde_json::from_value(json!({"retain_hp":1,"heal_max_hp_bps":0,"self_statuses":[]}))
            .unwrap(),
    );
    let j = npc(&w, "capped-target");
    w.npcs[j].hp = 10;
    freeze(&mut w, &c, 1);
    let e = hit(&mut w, &c, 1, 100);
    assert_eq!(results(&e)[0].4, 8);
    assert_eq!(w.npcs[j].hp, 1);
    assert_eq!(w.triggers.death_used.len(), 1);
}
#[test]
fn original_source_exit_cleans_the_carrier_without_delayed_damage() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    w.action(1, &Action::Leave {}, "leave-reaction", &c, 30)
        .unwrap();
    w.tick(&c, 60);
    assert_eq!(held(&w), 0);
    assert!(results(&hit(&mut w, &c, 1, 90)).is_empty());
}
#[test]
fn npc_origin_reaction_uses_original_npc_instead_of_actual_provoker() {
    let (c, mut w) = fixture();
    let original = npc(&w, "reaction-spare");
    let provoker = npc(&w, "capped-target");
    give(
        &mut w,
        &c,
        Owner::Npc(original),
        Owner::Member(0),
        "compound-lock",
        0,
    );
    let expected = c.combat_rules.as_ref().unwrap().contact_reactions["compound-lock"]
        .amount(w.record_power(Owner::Npc(original), &c, 100));
    let e = strike(
        &mut w,
        &c,
        Owner::Npc(provoker),
        Owner::Member(0),
        1,
        100,
        &[],
    );
    let r = results(&e);
    assert_eq!(r[0].0.id, "reaction-spare");
    assert_eq!(r[0].2.id, "capped-target");
    assert_eq!(r[0].3, expected);
}
#[test]
fn zero_primary_raw_only_qualifies_for_explicit_contact_policy() {
    for contact in [false, true] {
        let (mut c, mut w) = fixture();
        if contact {
            rule(&mut c).when = ContactReactionWhen::DirectContact;
        }
        freeze(&mut w, &c, 1);
        let e = hit(&mut w, &c, 0, 100);
        assert_eq!(results(&e).len(), usize::from(contact));
    }
}

#[test]
fn reaction_does_not_emit_passive_damage_facts_or_restore_source_hp() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers.push(serde_json::from_value(json!({"id":"derived-blocked","profiles":["member"],"when":"damage_dealt","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"conditions":{"minimum_amount":20},"effect":{"kind":"restore_spirit","amount":99}})).unwrap());
    w.members[0].spirit = 0;
    w.members[1].spirit = 0;
    w.members[1].hp = 50;
    freeze(&mut w, &c, 1);
    let e = hit(&mut w, &c, 1, 100);
    assert_eq!(results(&e)[0].3, 25);
    assert_eq!(w.members[1].hp, 50);
    assert_eq!(w.members[0].spirit, 0);
    assert_eq!(w.members[1].spirit, 0);
    assert!(!e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::CombatTriggerFired { .. }
        }
    )));
}
#[test]
fn provoker_death_from_primary_reflection_does_not_steal_original_ownership() {
    let (mut c, mut w) = fixture();
    c.combat_rules.as_mut().unwrap().triggers.push(serde_json::from_value(json!({"id":"primary-reflect","profiles":["ordinary-control"],"when":"damage_taken","include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":10000,"damage_kind":"true_damage"}})).unwrap());
    w.members[0].hp = 1;
    freeze(&mut w, &c, 1);
    let e = hit(&mut w, &c, 1, 100);
    assert_eq!(w.members[0].hp, 0);
    assert_eq!(results(&e)[0].0.id, "P01");
    assert_eq!(w.members[1].hp, 1000);
}
#[test]
fn returning_target_cannot_consume_or_release_existing_state() {
    let (c, mut w) = fixture();
    freeze(&mut w, &c, 1);
    let j = npc(&w, "capped-target");
    w.npcs[j].returning = true;
    assert!(results(&hit(&mut w, &c, 100, 100)).is_empty());
    assert_eq!(held(&w), 1);
}
