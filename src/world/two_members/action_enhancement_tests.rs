use super::*;
use crate::config::{CombatFact, CombatTrigger, ContactSampling, TriggerEffect};
fn document() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/enhancement-a.json"
    ))
    .unwrap()
}
fn fixture() -> (Config, World) {
    let p = Package::new();
    p.write(&document());
    let mut c = p.load().unwrap().default_config().as_ref().clone();
    c.combat_rules.as_mut().unwrap().triggers[0].conditions = Default::default();
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
            spirit_points: 200,
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
    for m in &mut w.members {
        m.spirit = 50;
    }
    w.members[0].cooldowns.insert("surge-line".into(), 4100);
    (c, w)
}
fn npc(w: &World, id: &str) -> usize {
    w.npcs.iter().position(|n| n.entity.id == id).unwrap()
}
fn trigger<'a>(c: &'a mut Config, id: &str) -> &'a mut CombatTrigger {
    c.combat_rules
        .as_mut()
        .unwrap()
        .triggers
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
}
fn give(w: &mut World, c: &Config, owner: Owner, id: &str, now: u64) {
    let e = w.owner_entity(owner);
    w.apply_owned_statuses(owner, (e, 1000), &[id.into()], c, now, &mut vec![]);
}
fn state(w: &World) -> Option<&StatusView> {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "afterglow")
}
fn attack(c: &Config) -> AttackRule {
    c.combat_rules.as_ref().unwrap().skills["surge-line"].clone()
}
fn strike(w: &mut World, c: &Config, id: &str, action: Option<&str>, raw: u32, now: u64) -> Events {
    let source = w.owner_entity(Owner::Member(0));
    let rule = attack(c);
    let mut e = vec![];
    w.direct_contact(
        Owner::Member(0),
        Owner::Npc(npc(w, id)),
        DirectContact {
            origin: None,
            action_id: action,
            rule: Some(&rule),
            ignore_armor: false,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw,
            metal: true,
            hit_statuses: &["threshold-mark".into()],
        },
        c,
        now,
        &mut e,
    );
    e
}
fn consumed(e: &Events) -> u32 {
    e.iter()
        .filter_map(|(_, e)| {
            if let EventV4::Private {
                event:
                    Event::StatusConsumed {
                        definition_id,
                        consumed_stacks,
                        ..
                    },
            } = e
            {
                (definition_id == "afterglow").then_some(*consumed_stacks)
            } else {
                None
            }
        })
        .sum()
}
fn fired(e: &Events, id: &str) -> usize {
    e.iter().filter(|(_,e)|matches!(e,EventV4::Private{event:Event::CombatTriggerFired{trigger_id,..}} if trigger_id==id)).count()
}
fn finishes(e: &Events) -> usize {
    e.iter()
        .filter(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::FinisherResolved { .. }
                }
            )
        })
        .count()
}
fn no_rewards(c: &mut Config) {
    c.combat_rules.as_mut().unwrap().triggers.truncate(1);
}
#[test]
fn action_enhancement_samples_roundtrip_and_reject_malformed_references_and_policies() {
    for input in [
        crate::test_data::fixture!("examples/combat-host/enhancement-a.json"),
        crate::test_data::fixture!("examples/combat-host/enhancement-b.json"),
    ] {
        let p = Package::new();
        p.write(&serde_json::from_str::<Value>(input).unwrap());
        let c = p.load().unwrap();
        p.write(&serde_json::to_value(c.default_config().as_ref()).unwrap());
        assert!(p.load().is_ok());
    }
    for n in 0..31 {
        let p = Package::new();
        let mut v = document();
        match n {
            0 => v["combat_rules"]["max_contact_action_snapshots"] = json!(0),
            1 => v["combat_rules"]["max_contact_action_snapshots"] = json!(1025),
            2 => v["combat_rules"]["max_contact_action_snapshots"] = Value::Null,
            3 => {
                v["combat_rules"]["skills"]["surge-line"]["contact_status"]["sampling"] =
                    json!("unknown")
            }
            4 => {
                v["combat_rules"]["skills"]["surge-line"]["contact_status"]["sampling"] =
                    Value::Null
            }
            5 => {
                v["combat_rules"]["skills"]["surge-line"]["contact_status"]["sampling"] =
                    json!({"action":null})
            }
            6 => {
                v["combat_rules"]["skills"]["surge-line"]["contact_status"]["subject"] =
                    json!("other")
            }
            7 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]["consumed_status_bonus"] =
                    Value::Null
            }
            8 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]["consumed_status_bonus"]["definition_id"] =
                    json!("missing")
            }
            9 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]["consumed_status_bonus"]["hp_threshold_bps_per_stack"] =
                    json!(0)
            }
            10 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]["consumed_status_bonus"]["hp_threshold_bps_per_stack"] =
                    json!(10001)
            }
            11 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]["consumed_status_bonus"]["typo"] =
                    json!(1)
            }
            12 => v["combat_rules"]["triggers"][0]["when"] = json!("damage_dealt"),
            13 => v["combat_rules"]["triggers"][0]["when"] = json!("contact_status_consumed"),
            14 => v["combat_rules"]["triggers"][1]["finisher"]["enhanced"] = Value::Null,
            15 => v["combat_rules"]["triggers"][1]["finisher"]["enhanced"] = json!(0),
            16 => v["combat_rules"]["triggers"][1]["finisher"]["enhanced"] = json!("false"),
            17 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]
                    .as_object_mut()
                    .unwrap()
                    .remove("consumed_status_bonus");
            }
            18 => {
                for id in ["threshold-bolt", "surge-line"] {
                    v["combat_rules"]["skills"][id]["contact_status"]["sampling"] =
                        json!("contact");
                    v["combat_rules"]["skills"][id]["contact_status"]["damage_bonus_bps_per_stack"] =
                        json!(1);
                }
            }
            19 => {
                v["combat_rules"]["skills"]["surge-line"]["contact_status"]["consume_max"] =
                    json!(0)
            }
            20 => {
                v["combat_rules"]["skills"]["surge-line"]["contact_status"]["consume_max"] =
                    json!(2)
            }
            21 => v["combat_rules"]["triggers"][0]["include_periodic"] = json!(true),
            22 => v["combat_rules"]["triggers"][0]["counting"] = json!("action_total"),
            23 => {
                v["combat_rules"]["skills"]["surge-line"]["contact_status"]["sampling"] = json!(1)
            }
            24 => v["mechanism_version"] = json!(104),
            25 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]["consumed_status_bonus"]["hp_threshold_bps_per_stack"] =
                    Value::Null
            }
            26 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]["consumed_status_bonus"]
                    .as_object_mut()
                    .unwrap()
                    .remove("hp_threshold_bps_per_stack");
            }
            27 => {
                v["combat_rules"]["triggers"][0]["effect"]["rule"]["consumed_status_bonus"]["hp_threshold_bps_per_stack"] =
                    json!(1.2)
            }
            28 => v["combat_rules"]["max_contact_action_snapshots"] = json!(-1),
            29 => v["combat_rules"]["triggers"][1]["finisher"]["enhanced"] = json!({"false":null}),
            _ => {
                v["combat_rules"]["skills"]["surge-line"]["contact_status"]["sampling"] =
                    json!("contact")
            }
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {n}");
    }
}
#[test]
fn default_contact_and_optional_new_fields_preserve_old_wire_output() {
    let v = json!({"subject":"owner","definition_id":"x","at_least":1,"consume_max":1,"damage_bonus_bps_per_stack":500,"armor_penetration_bps_per_stack":0});
    let r: crate::config::ContactStatusRule = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(r.sampling, ContactSampling::Contact);
    assert_eq!(serde_json::to_value(r).unwrap(), v);
    let p = Package::new();
    p.write(
        &serde_json::from_str::<Value>(crate::test_data::fixture!(
            "examples/combat-host/followup-a.json"
        ))
        .unwrap(),
    );
    let c = p.load().unwrap();
    let out = serde_json::to_value(c.default_config().as_ref()).unwrap();
    assert!(
        out["combat_rules"]
            .get("max_contact_action_snapshots")
            .is_none()
    );
    assert!(
        out["combat_rules"]["triggers"][0]["effect"]["rule"]
            .get("consumed_status_bonus")
            .is_none()
    );
    assert!(
        out["combat_rules"]["triggers"][1]["finisher"]
            .get("enhanced")
            .is_none()
    );
}
#[test]
fn first_absent_snapshot_protects_new_reward_from_same_action() {
    let (c, mut w) = fixture();
    let first = strike(&mut w, &c, "capped-target", Some("a"), 600, 100);
    let original = state(&w).unwrap().clone();
    assert_eq!(fired(&first, "enhance-next"), 1);
    let second = strike(&mut w, &c, "line-a", Some("a"), 450, 101);
    assert_eq!(consumed(&second), 0);
    assert_eq!(finishes(&second), 0);
    assert_eq!(w.npcs[npc(&w, "line-a")].hp, 550);
    assert_eq!(state(&w), Some(&original));
}
#[test]
fn new_action_consumes_once_and_every_target_uses_frozen_threshold() {
    let (c, mut w) = fixture();
    strike(&mut w, &c, "capped-target", Some("a"), 600, 100);
    let a = strike(&mut w, &c, "line-a", Some("b"), 450, 200);
    let b = strike(&mut w, &c, "line-b", Some("b"), 450, 230);
    assert_eq!((consumed(&a), consumed(&b)), (1, 0));
    assert_eq!((finishes(&a), finishes(&b)), (1, 1));
    assert!(state(&w).is_none());
    assert_eq!(fired(&a, "enhance-next") + fired(&b, "enhance-next"), 0);
    assert_eq!(fired(&a, "enhance-spirit") + fired(&b, "enhance-spirit"), 1);
    assert_eq!(w.members[0].spirit, 66);
}
#[test]
fn renewed_buff_does_not_change_cached_positive_or_get_consumed_again() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let a = strike(&mut w, &c, "line-a", Some("b"), 450, 100);
    give(&mut w, &c, Owner::Member(0), "afterglow", 110);
    let renewed = state(&w).unwrap().clone();
    let b = strike(&mut w, &c, "line-b", Some("b"), 450, 120);
    assert_eq!((consumed(&a), consumed(&b)), (1, 0));
    assert_eq!(finishes(&b), 1);
    assert_eq!(state(&w), Some(&renewed));
}
#[test]
fn unenhanced_multikill_grants_only_once_per_action() {
    let (c, mut w) = fixture();
    let a = strike(&mut w, &c, "line-a", Some("a"), 600, 100);
    let first = state(&w).unwrap().clone();
    let b = strike(&mut w, &c, "line-b", Some("a"), 600, 130);
    assert_eq!(finishes(&a) + finishes(&b), 2);
    assert_eq!(fired(&a, "enhance-next") + fired(&b, "enhance-next"), 1);
    assert_eq!(state(&w), Some(&first));
}
#[test]
fn missing_snapshot_identity_does_not_spend_buff_or_classify_unenhanced_reward() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let e = strike(&mut w, &c, "line-a", None, 600, 100);
    assert_eq!(consumed(&e), 0);
    assert_eq!(finishes(&e), 0);
    assert_eq!(fired(&e, "enhance-next"), 0);
    assert!(state(&w).is_some());
    assert_eq!(w.npcs[npc(&w, "line-a")].hp, 400);
}
#[test]
fn threshold_below_and_inclusive_use_exact_frozen_bonus() {
    for inclusive in [false, true] {
        for hp in [549, 550, 551] {
            let (mut c, mut w) = fixture();
            no_rewards(&mut c);
            if let TriggerEffect::FinishOther { rule } =
                &mut trigger(&mut c, "threshold-finish").effect
            {
                rule.comparison = if inclusive {
                    crate::config::ThresholdComparison::AtMost
                } else {
                    crate::config::ThresholdComparison::Below
                };
            }
            let j = npc(&w, "line-a");
            w.npcs[j].hp = hp + 1;
            give(&mut w, &c, Owner::Member(0), "afterglow", 0);
            let e = strike(&mut w, &c, "line-a", Some("b"), 1, 100);
            assert_eq!(finishes(&e) > 0, hp < 550 || inclusive && hp == 550);
            assert_eq!(consumed(&e), 1);
        }
    }
}
#[test]
fn consumption_is_not_refunded_when_threshold_is_not_met() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let e = strike(&mut w, &c, "line-a", Some("a"), 100, 100);
    assert_eq!(consumed(&e), 1);
    assert_eq!(finishes(&e), 0);
    assert!(state(&w).is_none());
    assert_eq!(w.npcs[npc(&w, "line-a")].hp, 900);
}
#[test]
fn primary_damage_kill_spends_enhancement_without_fake_success() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let e = strike(&mut w, &c, "line-a", Some("a"), 2000, 100);
    assert_eq!(consumed(&e), 1);
    assert_eq!(finishes(&e), 0);
    assert!(state(&w).is_none());
    assert_eq!(fired(&e, "enhance-spirit"), 0);
}
#[test]
fn death_guard_spends_enhancement_but_never_rewards() {
    let (mut c, mut w) = fixture();
    let id = w.npcs[npc(&w, "line-a")].template_id.clone();
    c.combat_rules
        .as_mut()
        .unwrap()
        .npc_profiles
        .insert(id, "guarded".into());
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let e = strike(&mut w, &c, "line-a", Some("a"), 450, 100);
    assert_eq!(consumed(&e), 1);
    assert!(state(&w).is_none());
    assert_eq!(w.npcs[npc(&w, "line-a")].hp, 1);
    assert_eq!(fired(&e, "enhance-spirit"), 0);
}
#[test]
fn capped_replacement_is_still_nonlethal_and_enhanced_mode_filter_is_explicit() {
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .npc_profiles
        .insert("line-a".into(), "control-resistant".into());
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let e = strike(&mut w, &c, "line-a", Some("a"), 450, 100);
    assert_eq!(consumed(&e), 1);
    assert_eq!(w.npcs[npc(&w, "line-a")].hp, 450);
    assert_eq!(fired(&e, "enhance-spirit"), 0);
    assert!(state(&w).is_none());
}
#[test]
fn probability_failure_and_trigger_budget_do_not_undo_contact_consumption() {
    for n in 0..2 {
        let (mut c, mut w) = fixture();
        if n == 0 {
            trigger(&mut c, "threshold-finish").chance_bps = Some(0);
        } else {
            w.trigger_step(100);
            let mut dummy = trigger(&mut c, "threshold-finish").clone();
            dummy.id = "spend".into();
            dummy.when = CombatFact::Kill;
            dummy.effect = TriggerEffect::RestoreSpirit {
                amount: 1,
                max_spirit_bps: 0,
            };
            dummy.conditions = Default::default();
            c.combat_rules.as_mut().unwrap().triggers.push(dummy);
            c.combat_rules.as_mut().unwrap().max_triggers_per_step = 1;
            w.combat_fact(
                CombatFact::Kill,
                (Owner::Member(0), Owner::Npc(npc(&w, "line-b"))),
                (1, false),
                &c,
                100,
                &mut vec![],
            );
        }
        give(&mut w, &c, Owner::Member(0), "afterglow", 0);
        let e = strike(&mut w, &c, "line-a", Some("a"), 450, 100);
        assert_eq!(consumed(&e), 1);
        assert_eq!(finishes(&e), 0);
        assert!(state(&w).is_none());
    }
}
#[test]
fn independent_actions_and_owners_do_not_share_snapshot_counts() {
    let (c, mut w) = fixture();
    strike(&mut w, &c, "line-a", Some("a"), 100, 100);
    give(&mut w, &c, Owner::Member(0), "afterglow", 110);
    let e = strike(&mut w, &c, "line-b", Some("b"), 450, 120);
    assert_eq!(consumed(&e), 1);
    assert_eq!(finishes(&e), 1);
    let source = w.owner_entity(Owner::Member(1));
    give(&mut w, &c, Owner::Member(1), "afterglow", 130);
    let mut e = vec![];
    let j = npc(&w, "renew-target");
    w.direct_contact(
        Owner::Member(1),
        Owner::Npc(j),
        DirectContact {
            origin: None,
            action_id: Some("b"),
            rule: Some(&attack(&c)),
            ignore_armor: false,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw: 450,
            metal: true,
            hit_statuses: &[],
        },
        &c,
        140,
        &mut e,
    );
    assert_eq!(consumed(&e), 1);
    assert_eq!(finishes(&e), 1);
}
#[test]
fn source_generation_and_definition_are_both_part_of_snapshot_identity() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    strike(&mut w, &c, "line-a", Some("a"), 100, 100);
    w.members[0].reference.entity.generation += 1;
    give(&mut w, &c, Owner::Member(0), "afterglow", 110);
    let e = strike(&mut w, &c, "line-b", Some("a"), 450, 120);
    assert_eq!(consumed(&e), 1);
    assert_eq!(finishes(&e), 1);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"charges","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":0,"duration_ms":3000,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true})).unwrap());
    give(&mut w, &c, Owner::Member(0), "charges", 130);
    let mut a = attack(&c);
    a.contact_status.as_mut().unwrap().definition_id = "charges".into();
    let m = w
        .contact_status_modifier(
            (Owner::Member(0), Owner::Npc(0), Some("a")),
            100,
            &a,
            &c,
            140,
            &mut vec![],
        )
        .unwrap();
    assert_eq!(m.consumed, 1);
    assert_eq!(m.augment.unwrap().stacks, 1);
}
#[test]
fn cached_damage_bonus_and_penetration_apply_each_contact_with_one_consumption_fact() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let r = c
        .combat_rules
        .as_mut()
        .unwrap()
        .skills
        .get_mut("surge-line")
        .unwrap();
    let s = r.contact_status.as_mut().unwrap();
    s.damage_bonus_bps_per_stack = 2500;
    s.armor_penetration_bps_per_stack = 500;
    r.armor_penetration_cap_bps = Some(5000);
    for id in ["line-a", "line-b"] {
        let e = strike(&mut w, &c, id, Some("a"), 100, 100);
        assert_eq!(w.npcs[npc(&w, id)].hp, 875);
        assert_eq!(consumed(&e), if id == "line-a" { 1 } else { 0 });
    }
}
#[test]
fn explicit_contact_sampling_keeps_live_per_contact_semantics() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    let r = c
        .combat_rules
        .as_mut()
        .unwrap()
        .skills
        .get_mut("surge-line")
        .unwrap();
    r.contact_status.as_mut().unwrap().sampling = ContactSampling::Contact;
    r.contact_status
        .as_mut()
        .unwrap()
        .damage_bonus_bps_per_stack = 1000;
    for id in ["line-a", "line-b"] {
        give(&mut w, &c, Owner::Member(0), "afterglow", 0);
        let e = strike(&mut w, &c, id, Some("a"), 100, 100);
        assert_eq!(consumed(&e), 1);
    }
    assert!(w.triggers.contacts.empty());
}
#[test]
fn protected_or_immune_targets_do_not_reserve_or_consume() {
    for n in 0..2 {
        let (mut c, mut w) = fixture();
        give(&mut w, &c, Owner::Member(0), "afterglow", 0);
        let source = w.owner_entity(Owner::Member(0));
        let mut e = vec![];
        let target = if n == 0 {
            w.members[1].protected = true;
            Owner::Member(1)
        } else {
            c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"immune","mechanism":"timed_attribute_modifier","attribute":"damage_immunity","modifier_bps":0,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
            let j = npc(&w, "line-a");
            give(&mut w, &c, Owner::Npc(j), "immune", 0);
            Owner::Npc(j)
        };
        w.direct_contact(
            Owner::Member(0),
            target,
            DirectContact {
                origin: None,
                action_id: Some("a"),
                rule: Some(&attack(&c)),
                ignore_armor: false,
                unavoidable: true,
                dead_source: false,
                source: &source,
                raw: 450,
                metal: true,
                hit_statuses: &[],
            },
            &c,
            100,
            &mut e,
        );
        assert_eq!(consumed(&e), 0);
        assert!(w.triggers.contacts.empty());
        assert!(state(&w).is_some());
    }
}
#[test]
fn dodge_defers_first_snapshot_and_preserves_the_whole_buff() {
    let mut found = false;
    for seed in 1..64 {
        let (mut c, mut w) = fixture();
        no_rewards(&mut c);
        give(&mut w, &c, Owner::Member(0), "afterglow", 0);
        c.combat_rules
            .as_mut()
            .unwrap()
            .profiles
            .get_mut("ordinary-control")
            .unwrap()
            .dodge_bps = 5000;
        w.rng = seed;
        let source = w.owner_entity(Owner::Member(0));
        let mut r = attack(&c);
        r.unavoidable = false;
        let mut e = vec![];
        let j = npc(&w, "line-a");
        let hit = w.direct_contact(
            Owner::Member(0),
            Owner::Npc(j),
            DirectContact {
                origin: None,
                action_id: Some("a"),
                rule: Some(&r),
                ignore_armor: false,
                unavoidable: false,
                dead_source: false,
                source: &source,
                raw: 450,
                metal: true,
                hit_statuses: &[],
            },
            &c,
            100,
            &mut e,
        );
        if !hit {
            assert!(state(&w).is_some());
            assert!(w.triggers.contacts.empty());
            assert_eq!(consumed(&e), 0);
            found = true;
            break;
        }
    }
    assert!(found);
}
#[test]
fn expired_buff_freezes_zero_and_later_grant_waits_for_next_action() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let e = strike(&mut w, &c, "line-a", Some("a"), 100, 5000);
    assert_eq!(consumed(&e), 0);
    give(&mut w, &c, Owner::Member(0), "afterglow", 5010);
    let e = strike(&mut w, &c, "line-b", Some("a"), 450, 5020);
    assert_eq!(consumed(&e), 0);
    assert_eq!(finishes(&e), 0);
    assert!(state(&w).is_some());
}
#[test]
fn derived_contact_does_not_create_or_spend_action_snapshots() {
    let (c, mut w) = fixture();
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    w.triggers.executing = true;
    let e = strike(&mut w, &c, "line-a", Some("a"), 450, 100);
    assert_eq!(consumed(&e), 0);
    assert_eq!(finishes(&e), 0);
    assert!(w.triggers.contacts.empty());
    assert!(state(&w).is_some());
}
#[test]
fn capacity_pause_never_turns_unknown_into_ordinary_reward_or_retries_active_action() {
    use super::super::triggers::ActionKey;
    let (mut c, mut w) = fixture();
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_contact_action_snapshots = 1;
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    strike(&mut w, &c, "line-a", Some("a"), 100, 100);
    give(&mut w, &c, Owner::Member(0), "afterglow", 110);
    let e = strike(&mut w, &c, "line-b", Some("b"), 600, 120);
    assert_eq!(consumed(&e), 0);
    assert_eq!(finishes(&e), 0);
    assert_eq!(fired(&e, "enhance-next"), 0);
    assert!(state(&w).is_some());
    assert!(w.triggers.contacts.paused);
    let source = w.owner_entity(Owner::Member(0));
    let b = ActionKey::new(&source, "b");
    w.triggers
        .contacts
        .prune(&std::collections::BTreeSet::from([b]));
    assert!(w.triggers.contacts.paused);
    let e = strike(&mut w, &c, "renew-target", Some("b"), 600, 130);
    assert_eq!(finishes(&e), 0);
    assert_eq!(consumed(&e), 0);
    w.triggers.contacts.prune(&Default::default());
    assert!(!w.triggers.contacts.paused);
    let e = strike(&mut w, &c, "capped-target", Some("c"), 450, 140);
    assert_eq!(consumed(&e), 1);
    assert_eq!(finishes(&e), 1);
}
#[test]
fn existing_positive_snapshot_continues_while_new_admissions_are_paused() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_contact_action_snapshots = 1;
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    strike(&mut w, &c, "line-a", Some("a"), 100, 100);
    strike(&mut w, &c, "line-b", Some("b"), 100, 110);
    assert!(w.triggers.contacts.paused);
    let e = strike(&mut w, &c, "renew-target", Some("a"), 450, 120);
    assert_eq!(finishes(&e), 1);
    assert_eq!(consumed(&e), 0);
}
#[test]
fn source_cleanup_and_action_drain_release_snapshot_storage() {
    let (c, mut w) = fixture();
    strike(&mut w, &c, "line-a", Some("a"), 100, 100);
    assert_eq!(w.triggers.contacts.entries.len(), 1);
    let source = w.owner_entity(Owner::Member(0));
    w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(w.triggers.contacts.empty());
    strike(&mut w, &c, "line-b", Some("b"), 100, 110);
    w.prune_trigger_actions();
    assert!(w.triggers.contacts.empty());
}
#[test]
fn reconnect_readiness_does_not_reset_cached_classification() {
    let (c, mut w) = fixture();
    strike(&mut w, &c, "capped-target", Some("a"), 600, 100);
    w.members[0].ready = false;
    w.members[0].protected = true;
    w.members[0].ready = true;
    w.members[0].protected = false;
    let e = strike(&mut w, &c, "line-a", Some("a"), 450, 120);
    assert_eq!(finishes(&e), 0);
    assert_eq!(consumed(&e), 0);
    assert!(state(&w).is_some());
}
#[test]
fn enhanced_selector_and_mode_filters_compose_without_cross_awards() {
    for required in [false, true] {
        let (mut c, mut w) = fixture();
        trigger(&mut c, "enhance-spirit")
            .finisher
            .as_mut()
            .unwrap()
            .enhanced = Some(required);
        give(&mut w, &c, Owner::Member(0), "afterglow", 0);
        let e = strike(&mut w, &c, "line-a", Some("a"), 450, 100);
        assert_eq!(fired(&e, "enhance-spirit"), usize::from(required));
        assert_eq!(fired(&e, "enhance-next"), 0);
    }
}
#[test]
fn unmatched_status_never_increases_threshold() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    if let TriggerEffect::FinishOther { rule } = &mut trigger(&mut c, "threshold-finish").effect {
        rule.consumed_status_bonus.as_mut().unwrap().definition_id = "different".into();
    }
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    let e = strike(&mut w, &c, "line-a", Some("a"), 450, 100);
    assert_eq!(finishes(&e), 0);
    assert_eq!(consumed(&e), 1);
}
#[test]
fn bonus_clamps_at_full_health_and_comparison_remains_explicit() {
    use super::super::contact_status::ContactAugment;
    let (mut c, w) = fixture();
    let j = npc(&w, "line-a");
    let mut rule = if let TriggerEffect::FinishOther { rule } =
        trigger(&mut c, "threshold-finish").effect.clone()
    {
        rule
    } else {
        panic!()
    };
    rule.hp_threshold_bps = 9999;
    rule.consumed_status_bonus
        .as_mut()
        .unwrap()
        .hp_threshold_bps_per_stack = 10000;
    let context = ContactAugment {
        definition_id: "afterglow".into(),
        stacks: u32::MAX,
        failure: None,
    };
    assert!(
        w.plan_finish_other(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(j)),
            &rule,
            &c,
            100,
            Some(&context)
        )
        .is_ok()
    );
    rule.comparison = crate::config::ThresholdComparison::Below;
    assert!(
        w.plan_finish_other(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(j)),
            &rule,
            &c,
            100,
            Some(&context)
        )
        .is_err()
    );
}
#[test]
fn original_five_percent_bonus_attribute_and_lifetime_are_local_native_vectors() {
    let f: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_action_enhancement_vectors.json"
    ))
    .unwrap();
    let (mut c, mut w) = fixture();
    let duration = f["durationMs"].as_u64().unwrap();
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "afterglow")
        .unwrap();
    d.duration_ms = duration;
    strike(&mut w, &c, "capped-target", Some("a"), 600, 100);
    let s = state(&w).unwrap();
    assert_eq!(s.expires_at_ms - s.applied_at_ms, duration);
    assert_eq!(w.member_max_spirit(0, &c, 100), 250);
    assert_eq!(w.stats(0, &c, 100).0, 125);
    if let TriggerEffect::FinishOther { rule } = &trigger(&mut c, "threshold-finish").effect {
        assert_eq!(
            json!(
                rule.consumed_status_bonus
                    .as_ref()
                    .unwrap()
                    .hp_threshold_bps_per_stack
            ),
            f["bonusBps"]
        );
    }
    let e = strike(&mut w, &c, "line-a", Some("b"), 450, 200);
    assert_eq!(consumed(&e), 1);
    assert_eq!(w.npcs[npc(&w, "line-a")].hp, 0);
    assert_eq!(w.stats(0, &c, 200).0, 100);
}
#[test]
fn partial_stack_consumption_freezes_effect_count_and_preserves_remaining_layers() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"charges","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":0,"duration_ms":3000,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true})).unwrap());
    let r = c
        .combat_rules
        .as_mut()
        .unwrap()
        .skills
        .get_mut("surge-line")
        .unwrap();
    let s = r.contact_status.as_mut().unwrap();
    s.definition_id = "charges".into();
    s.consume_max = 2;
    s.damage_bonus_bps_per_stack = 2500;
    for _ in 0..3 {
        give(&mut w, &c, Owner::Member(0), "charges", 0);
    }
    let a = strike(&mut w, &c, "line-a", Some("a"), 100, 100);
    let b = strike(&mut w, &c, "line-b", Some("a"), 100, 120);
    assert_eq!(w.npcs[npc(&w, "line-a")].hp, 850);
    assert_eq!(w.npcs[npc(&w, "line-b")].hp, 850);
    let counts = |e: &Events| {
        e.iter()
            .filter_map(|(_, e)| {
                if let EventV4::Private {
                    event:
                        Event::StatusConsumed {
                            definition_id,
                            consumed_stacks,
                            ..
                        },
                } = e
                {
                    (definition_id == "charges").then_some(*consumed_stacks)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(counts(&a), [2]);
    assert!(counts(&b).is_empty());
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "charges")
            .unwrap()
            .stacks,
        Some(1)
    );
    let e = strike(&mut w, &c, "renew-target", Some("b"), 100, 130);
    assert_eq!(counts(&e), [1]);
    assert_eq!(w.npcs[npc(&w, "renew-target")].hp, 875);
}
#[test]
fn native_npc_action_uses_its_own_buff_and_complete_source_identity() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    trigger(&mut c, "threshold-finish").profiles = vec!["ordinary-control".into()];
    let j = npc(&w, "capped-target");
    give(&mut w, &c, Owner::Npc(j), "afterglow", 0);
    let source = w.owner_entity(Owner::Npc(j));
    let mut rule = attack(&c);
    rule.damage_kind = crate::config::DamageKind::TrueDamage;
    for i in [0, 1] {
        w.members[i].hp = 551;
        let mut e = vec![];
        w.direct_contact(
            Owner::Npc(j),
            Owner::Member(i),
            DirectContact {
                origin: None,
                action_id: Some("npc-parent"),
                rule: Some(&rule),
                ignore_armor: true,
                unavoidable: true,
                dead_source: false,
                source: &source,
                raw: 1,
                metal: true,
                hit_statuses: &[],
            },
            &c,
            100 + i as u64,
            &mut e,
        );
        assert_eq!(w.members[i].hp, 0);
        // An NPC self-status is observed by all four living members at consumption.
        assert_eq!(consumed(&e), 4 * u32::from(i == 0));
        assert_eq!(finishes(&e), 1);
    }
    assert!(
        w.triggers
            .contacts
            .entries
            .keys()
            .all(|(a, _)| a.source.0 == 1 && a.source.1 == source.id)
    );
}
#[test]
fn real_field_ticks_share_parent_snapshot_and_release_it_at_end() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    let mut v = skill("ground_field");
    v["contact_policy"] =
        json!({"max_targets_per_tick":1,"max_total_hits":2,"repeat_interval_ms":120});
    c.skill = serde_json::from_value(v).unwrap();
    let mut r = attack(&c);
    r.contact_status
        .as_mut()
        .unwrap()
        .damage_bonus_bps_per_stack = 2500;
    c.combat_rules
        .as_mut()
        .unwrap()
        .skills
        .insert(c.skill.id.clone(), r);
    w.members[0].position = [500., 240.];
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    commit(&mut w, &c, "ground_field");
    let mut e = vec![];
    for now in [30, 150, 270] {
        w.prune_trigger_actions();
        w.advance_areas(&c, now, &mut e);
    }
    assert_eq!(consumed(&e), 1);
    let hits=e.iter().filter(|(_,e)|matches!(e,EventV4::Damage{source,target,..} if source.id=="P00"&&target.id=="capped-target")).count();
    assert_eq!(hits, 2);
    w.advance_areas(&c, 420, &mut e);
    w.prune_trigger_actions();
    assert!(w.triggers.contacts.empty());
}
#[test]
fn real_channel_child_shots_keep_snapshot_between_pulses() {
    let (mut c, mut w) = fixture();
    no_rewards(&mut c);
    let mut v = skill("projectile");
    v["channel"] = json!({"duration_ms":480,"interval_ms":120,"spirit_per_pulse":1});
    c.skill = serde_json::from_value(v).unwrap();
    let r = attack(&c);
    c.combat_rules
        .as_mut()
        .unwrap()
        .skills
        .insert(c.skill.id.clone(), r);
    w.members[0].position = [500., 240.];
    give(&mut w, &c, Owner::Member(0), "afterglow", 0);
    commit(&mut w, &c, "projectile");
    let mut e = vec![];
    for now in (30..=600).step_by(30) {
        e.extend(w.tick(&c, now));
    }
    assert_eq!(consumed(&e), 1);
    let amounts = e
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::Damage {
                source,
                target,
                amount,
                ..
            } = e
            {
                (source.id == "P00" && target.id == "capped-target").then_some(*amount)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(amounts.len(), 3);
    assert!(amounts.iter().all(|a| *a == amounts[0]));
    assert!(w.triggers.contacts.empty());
}
