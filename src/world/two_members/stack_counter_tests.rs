use super::combat::DamageIntent;
use super::*;
use serde_json::{Value, json};
fn setup() -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(false, false);
    w.statuses = Default::default();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"counter","mechanism":"timed_compound_modifier","modifiers":[{"attribute":"life_steal","modifier_bps":300}],"base_modifiers":[{"attribute":"life_steal","modifier_bps":1500},{"attribute":"damage_reduction","modifier_bps":2000}],"stack_counter":{"initial":0,"maximum":3},"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let target = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), target, "counter", 0);
    (c, w)
}
fn view(w: &World) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "counter")
        .unwrap()
        .clone()
}
fn value(c: &Config, w: &World, axis: StatusAttribute, now: u64) -> u32 {
    w.statuses.attribute(
        &status_entity(&w.members[0].reference.entity),
        10000,
        now,
        c.pve.as_ref().unwrap().status_rules.as_ref(),
        axis,
    )
}
#[test]
fn stack_counter_base_plus_layered_values_are_one_instance_and_real_zero() {
    let (c, mut w) = setup();
    let original = view(&w);
    assert_eq!(
        (original.stack_counter, original.stacks, original.max_stacks),
        (Some(true), Some(0), Some(3))
    );
    for i in 0..4 {
        assert_eq!(
            value(&c, &w, StatusAttribute::LifeSteal, 100),
            11500 + 300 * i.min(3)
        );
        assert_eq!(value(&c, &w, StatusAttribute::DamageReduction, 100), 12000);
        w.statuses
            .adjust_counter(&original.instance_id, 1, 100, None, &mut vec![])
            .unwrap();
    }
    assert_eq!(w.statuses.entries().len(), 1);
    let after = view(&w);
    assert_eq!(after.instance_id, original.instance_id);
    assert_eq!((after.applied_at_ms, after.expires_at_ms), (0, 1200));
}
#[test]
fn stack_counter_consume_to_zero_keeps_fixed_portion_and_absolute_lifetime() {
    let (c, mut w) = setup();
    let original = view(&w);
    w.statuses
        .adjust_counter(&original.instance_id, 3, 100, None, &mut vec![])
        .unwrap();
    let mut e = vec![];
    assert!(w.statuses.consume(&original.instance_id, 3, &mut e));
    assert!(!e.iter().any(|e| matches!(e, Event::StatusRemoved { .. })));
    assert_eq!(view(&w).stacks, Some(0));
    assert_eq!(value(&c, &w, StatusAttribute::LifeSteal, 100), 11500);
    assert_eq!(view(&w).expires_at_ms, 1200);
    w.statuses.expire(1200, &mut vec![]);
    assert!(w.statuses.entries().is_empty());
    assert_eq!(value(&c, &w, StatusAttribute::LifeSteal, 1200), 10000);
}
#[test]
fn stack_counter_refresh_resets_initial_ignore_preserves_and_adjust_can_refresh_only_clock() {
    let (mut c, mut w) = setup();
    let original = view(&w);
    w.statuses
        .adjust_counter(&original.instance_id, 2, 100, None, &mut vec![])
        .unwrap();
    let target = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), target.clone(), "counter", 300);
    assert_eq!(view(&w).stacks, Some(0));
    assert_eq!(view(&w).expires_at_ms, 1500);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "counter")
        .unwrap()
        .duplicate_policy = crate::config::DuplicatePolicy::Ignore;
    w.statuses
        .adjust_counter(&original.instance_id, 1, 400, None, &mut vec![])
        .unwrap();
    let before = view(&w);
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), target, "counter", 500);
    assert_eq!(view(&w), before);
    w.statuses
        .adjust_counter(&original.instance_id, 99, 600, Some(1200), &mut vec![])
        .unwrap();
    assert_eq!(
        (
            view(&w).stacks,
            view(&w).applied_at_ms,
            view(&w).expires_at_ms
        ),
        (Some(3), 600, 1800)
    );
}
#[test]
fn stack_counter_fixed_and_layered_same_axis_compose_before_global_cap_and_movement_selection() {
    let (mut c, mut w) = setup();
    let original = view(&w);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .attribute_caps_bps
        .insert(StatusAttribute::LifeSteal, 2000);
    w.statuses
        .adjust_counter(&original.instance_id, 3, 100, None, &mut vec![])
        .unwrap();
    assert_eq!(value(&c, &w, StatusAttribute::LifeSteal, 100), 12000);
    let mut d: Value = serde_json::to_value(
        c.pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .find(|d| d.id == "counter")
            .unwrap(),
    )
    .unwrap();
    d["id"] = json!("slow-counter");
    d["base_modifiers"] = json!([{"attribute":"movement_slow","modifier_bps":-1000}]);
    d["modifiers"] = json!([{"attribute":"movement_slow","modifier_bps":-500}]);
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
    let target = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), target, "slow-counter", 0);
    let id = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "slow-counter")
        .unwrap()
        .instance_id
        .clone();
    w.statuses
        .adjust_counter(&id, 3, 100, None, &mut vec![])
        .unwrap();
    assert_eq!(
        w.statuses
            .movement_factor_bps(&status_entity(&w.members[0].reference.entity), 100),
        7500
    );
}
fn policy(c: &mut Config, id: &str, harmful: bool) {
    c.combat_rules.as_mut().unwrap().status_policies.insert(id.into(),serde_json::from_value(json!({"disposition":if harmful{"harmful"}else{"beneficial"},"dispellable":true,"transferable":false,"blocks":[]})).unwrap());
}
fn triggers(c: &mut Config, leaf: Value) {
    policy(c, "counter", false);
    c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([
 {"id":"gain","profiles":["member"],"when":"damage_dealt","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"effect":{"kind":"add_status_stacks","subject":"owner","status_id":"counter","amount":1}},
 {"id":"threshold","profiles":["member"],"when":"status_stacks_reached","stack_change":{"definition_id":"counter","at_least":3},"include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"effect":leaf}
 ])).unwrap();
}
fn damage(w: &mut World, c: &Config, source: usize, now: u64) -> Events {
    let mut e = vec![];
    w.resolve_damage_with_rule(
        DamageIntent {
            source: w.members[source].reference.entity.clone(),
            target: w.npcs[0].entity.clone(),
            raw: 100,
            direct: true,
            ignore_armor: true,
        },
        Some(&crate::config::AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            can_crit: false,
            ..Default::default()
        }),
        c,
        now,
        &mut e,
    );
    e
}
fn fired(e: &Events, id: &str) -> usize {
    e.iter().filter(|(_,e)|matches!(e,EventV4::Private{event:Event::CombatTriggerFired{trigger_id,..}} if trigger_id==id)).count()
}
#[test]
fn stack_counter_real_damage_uses_previous_lifesteal_before_gain_and_threshold_fires_once() {
    let (mut c, mut w) = setup();
    triggers(&mut c, json!({"kind":"restore_spirit","amount":2}));
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 1;
    let mut heals = vec![];
    for i in 0..5 {
        let before = w.members[0].hp;
        let e = damage(&mut w, &c, 0, 30 * (i + 1));
        heals.push(w.members[0].hp - before);
        assert_eq!(fired(&e, "threshold"), usize::from(i == 2));
    }
    assert_eq!(heals, [15, 18, 21, 24, 24]);
    assert_eq!(view(&w).stacks, Some(3));
    assert_eq!(w.members[0].spirit, 52);
    let id = view(&w).instance_id;
    w.statuses.consume(&id, 3, &mut vec![]);
    for i in 0..3 {
        let e = damage(&mut w, &c, 0, 210 + i * 30);
        assert_eq!(fired(&e, "threshold"), usize::from(i == 2));
    }
    assert_eq!(w.members[0].spirit, 54);
}
#[test]
fn stack_counter_window_records_survive_adjustment_and_zero_consumption_then_expire_once() {
    let (mut c, mut w) = setup();
    triggers(&mut c, json!({"kind":"restore_spirit","amount":1}));
    c.combat_rules.as_mut().unwrap().status_windows.insert("counter".into(),serde_json::from_value(json!({"basis":"damage_dealt","include_periodic":false,"record_cap":10000,"refresh":"reset","maximum_sampling":"snapshot","on_expired":true,"on_consumed":true,"hp":{"fixed":0,"recorded_bps":1000,"max_resource_bps":0,"cap_max_resource_bps":2000}})).unwrap());
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own, "counter", 0);
    w.members[0].hp = 1;
    let original = view(&w);
    for i in 0..4 {
        damage(&mut w, &c, 0, 30 * (i + 1));
    }
    let full = view(&w);
    assert_eq!(
        (
            full.window.as_ref().unwrap().recorded,
            full.window.as_ref().unwrap().contacts
        ),
        (400, 4)
    );
    let mut e = vec![];
    w.statuses.consume(&full.instance_id, 3, &mut e);
    assert_eq!(view(&w).stacks, Some(0));
    assert_eq!(view(&w).window, full.window);
    assert_eq!(view(&w).expires_at_ms, original.expires_at_ms);
    let hp = w.members[0].hp;
    let mut e = vec![];
    w.prepare_periodic(1200, &mut e);
    w.finish_window_records(&c, 1200, &mut e);
    assert_eq!(w.members[0].hp, hp + 20);
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::WindowRecordEnded {
                        recorded: 400,
                        raw_hp: 20,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
    w.finish_window_records(&c, 1230, &mut e);
    assert_eq!(w.members[0].hp, hp + 20);
}
#[test]
fn stack_counter_threshold_cleanse_obeys_crossing_and_does_not_end_derive() {
    let (mut c, mut w) = setup();
    triggers(
        &mut c,
        json!({"kind":"dispel_statuses","subject":"owner","disposition":"harmful","max_count":1}),
    );
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"debuff","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":-1000,"duration_ms":1100,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    policy(&mut c, "debuff", true);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"reward","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules.as_mut().unwrap().status_ends.insert(
        "debuff".into(),
        serde_json::from_value(json!({"on":["cleansed"],"target_statuses":["reward"]})).unwrap(),
    );
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Npc(0), own.clone(), "debuff", 0);
    for i in 0..3 {
        damage(&mut w, &c, 0, (i + 1) * 30);
    }
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "debuff")
    );
    w.finish_status_ends(&c, 90, &mut vec![]);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "reward")
    );
    super::periodic_tests::bind(&mut w, &c, Owner::Npc(0), own, "debuff", 120);
    damage(&mut w, &c, 0, 150);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "debuff")
    );
}
#[test]
fn stack_counter_failed_threshold_budget_and_chance_do_not_undo_damage_or_counter_or_replay_at_max()
{
    for chance in [true, false] {
        let (mut c, mut w) = setup();
        triggers(&mut c, json!({"kind":"restore_spirit","amount":2}));
        if chance {
            c.combat_rules.as_mut().unwrap().triggers[1].chance_bps = Some(0);
        } else {
            c.combat_rules.as_mut().unwrap().max_triggers_per_step = 1;
        }
        for i in 0..4 {
            let e = damage(&mut w, &c, 0, 30 * (i + 1));
            assert_eq!(fired(&e, "threshold"), 0);
        }
        assert_eq!(view(&w).stacks, Some(3));
        assert_eq!(w.members[0].spirit, 50);
        assert_eq!(w.npcs[0].hp, 600);
    }
}
#[test]
fn stack_counter_real_native_source_and_holder_isolation_missing_expired_and_dead_fail_closed() {
    use crate::config::TriggerSubject::Owner as Own;
    let (mut c, mut w) = setup();
    policy(&mut c, "counter", false);
    assert!(
        w.add_status_stacks(
            (Owner::Member(1), Owner::Npc(0)),
            (Own, "counter", 1, false),
            &c,
            30,
            &mut vec![]
        )
        .is_none()
    );
    assert!(
        w.add_status_stacks(
            (Owner::Member(0), Owner::Npc(0)),
            (Own, "counter", 1, false),
            &c,
            1200,
            &mut vec![]
        )
        .is_none()
    );
    w.members[0].hp = 0;
    assert!(
        w.add_status_stacks(
            (Owner::Member(0), Owner::Npc(0)),
            (Own, "counter", 1, false),
            &c,
            30,
            &mut vec![]
        )
        .is_none()
    );
    assert_eq!(view(&w).stacks, Some(0));
}
#[test]
fn stack_counter_policy_blocked_adjustment_does_not_consume_condition_or_cooldown() {
    let (mut c, mut w) = setup();
    triggers(&mut c, json!({"kind":"restore_spirit","amount":1}));
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"block","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1,"duration_ms":90,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules.as_mut().unwrap().status_policies.insert("block".into(),serde_json::from_value(json!({"disposition":"harmful","dispellable":true,"transferable":false,"blocks":["beneficial"]})).unwrap());
    c.combat_rules.as_mut().unwrap().triggers[0].cooldown_ms = 1000;
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Npc(0), own, "block", 0);
    let e = damage(&mut w, &c, 0, 30);
    assert_eq!(fired(&e, "gain"), 0);
    assert_eq!(view(&w).stacks, Some(0));
    let e = damage(&mut w, &c, 0, 90);
    assert_eq!(fired(&e, "gain"), 1);
    assert_eq!(view(&w).stacks, Some(1));
}
#[test]
fn stack_counter_config_cold_loads_window_and_conditions_and_rejects_null_conflicts_and_recursive_leaves()
 {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/power-a.json"
    ))
    .unwrap();
    let (c, _) = setup();
    let def = c
        .pve
        .as_ref()
        .unwrap()
        .status_definitions
        .iter()
        .find(|s| s.id == "counter")
        .unwrap();
    d["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::to_value(def).unwrap());
    let (mut c, _) = setup();
    triggers(&mut c, json!({"kind":"restore_spirit","amount":2}));
    d["combat_rules"]["triggers"] =
        serde_json::to_value(&c.combat_rules.as_ref().unwrap().triggers).unwrap();
    d["combat_rules"]["status_policies"]["counter"] =
        json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]});
    d["combat_rules"]["triggers"][1]["conditions"] =
        json!({"status":{"subject":"owner","definition_id":"counter","at_least":3,"consume":3}});
    p.write(&d);
    p.load().unwrap();
    let pos = d["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    for i in 0..25 {
        let mut b = d.clone();
        let v = &mut b["pve"]["status_definitions"][pos];
        match i {
            0 => v["stack_counter"] = Value::Null,
            1 => v["stack_counter"]["initial"] = json!(4),
            2 => v["stack_counter"]["maximum"] = json!(0),
            3 => v["stack_counter"]["maximum"] = json!(129),
            4 => v["stack_counter"]["extra"] = json!(true),
            5 => v["source_scoped"] = json!(false),
            6 => v["duplicate_policy"] = json!("stack"),
            7 => v["max_stacks"] = json!(3),
            8 => v["base_modifiers"] = json!([]),
            9 => v["base_modifiers"] = Value::Null,
            10 => v["base_modifiers"][1] = v["base_modifiers"][0].clone(),
            11 => v["flags"] = json!(["root"]),
            12 => v["mechanism"] = json!("periodic_heal"),
            13 => b["mechanism_version"] = json!(104),
            14 => b["combat_rules"]["triggers"][0]["effect"]["amount"] = json!(0),
            15 => b["combat_rules"]["triggers"][0]["effect"]["status_id"] = json!("demo-guard"),
            16 => b["combat_rules"]["triggers"][0]["effect"]["refresh_duration"] = Value::Null,
            17 => b["combat_rules"]["triggers"][1]["stack_change"] = Value::Null,
            18 => b["combat_rules"]["triggers"][1]["stack_change"]["at_least"] = json!(4),
            19 => {
                b["combat_rules"]["triggers"][1]["effect"] = json!({"kind":"add_status_stacks","subject":"owner","status_id":"counter","amount":1})
            }
            20 => b["combat_rules"]["triggers"][1]["counting"] = json!("action"),
            21 => b["combat_rules"]["triggers"][1]["include_periodic"] = json!(true),
            22 => {
                b["combat_rules"]["triggers"][0]["stack_change"] =
                    json!({"definition_id":"counter","at_least":1})
            }
            23 => {
                b["combat_rules"]["triggers"][1]["effect"] =
                    json!({"kind":"apply_self_status","status_id":"counter"})
            }
            _ => {
                b["combat_rules"]["status_policies"]
                    .as_object_mut()
                    .unwrap()
                    .remove("counter");
            }
        }
        p.write(&b);
        assert!(p.load().is_err(), "invalid {i}");
    }
}
#[test]
fn stack_counter_transfer_preserves_zero_fixed_context_and_can_only_be_adjusted_by_original_source()
{
    use crate::config::TriggerSubject::Other;
    let (mut c, mut w) = setup();
    policy(&mut c, "counter", false);
    let before = view(&w);
    let new_holder = w.members[1].reference.entity.clone();
    w.statuses.retarget(
        std::slice::from_ref(&before.instance_id),
        &status_entity(&new_holder),
        &mut vec![],
    );
    let after = view(&w);
    assert_eq!(after.stacks, Some(0));
    assert_eq!(after.base_modifiers, before.base_modifiers);
    assert_eq!(after.expires_at_ms, 1200);
    assert!(
        w.add_status_stacks(
            (Owner::Member(1), Owner::Member(1)),
            (Other, "counter", 1, false),
            &c,
            30,
            &mut vec![]
        )
        .is_none()
    );
    assert!(
        w.add_status_stacks(
            (Owner::Member(0), Owner::Member(1)),
            (Other, "counter", 1, false),
            &c,
            30,
            &mut vec![]
        )
        .is_some()
    );
    assert_eq!(view(&w).stacks, Some(1));
    let source = w.members[0].reference.entity.clone();
    w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(w.statuses.entries().is_empty());
}
#[test]
fn stack_counter_multimember_same_definition_has_private_counts_and_sources() {
    let (mut c, mut w) = setup();
    triggers(&mut c, json!({"kind":"restore_spirit","amount":2}));
    let own = w.members[1].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(1), own, "counter", 0);
    damage(&mut w, &c, 0, 30);
    damage(&mut w, &c, 0, 60);
    damage(&mut w, &c, 1, 90);
    let values = w
        .statuses
        .entries()
        .iter()
        .filter(|s| s.definition_id == "counter")
        .map(|s| {
            (
                s.source.id.as_str(),
                s.target.id.as_str(),
                s.stacks.unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(values, [("P00", "P00", 2), ("P01", "P01", 1)]);
    assert_eq!(w.members[0].spirit, 50);
    assert_eq!(w.members[1].spirit, 50);
}
#[test]
fn stack_counter_threshold_can_consume_real_full_counter_without_removing_fixed_carrier() {
    let (mut c, mut w) = setup();
    triggers(&mut c, json!({"kind":"restore_spirit","amount":1}));
    c.combat_rules.as_mut().unwrap().triggers[1].conditions = serde_json::from_value(
        json!({"status":{"subject":"owner","definition_id":"counter","at_least":3,"consume":3}}),
    )
    .unwrap();
    for i in 0..6 {
        damage(&mut w, &c, 0, (i + 1) * 30);
    }
    assert_eq!(view(&w).stacks, Some(0));
    assert_eq!(w.members[0].spirit, 52);
    assert_eq!(value(&c, &w, StatusAttribute::LifeSteal, 180), 11500);
    assert_eq!(view(&w).expires_at_ms, 1200);
}
#[test]
fn stack_counter_threshold_owner_skill_and_trigger_origin_remain_native_and_leaf_status_is_derived()
{
    use crate::origin::ActionOrigin;
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"leaf","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    triggers(
        &mut c,
        json!({"kind":"apply_self_status","status_id":"leaf"}),
    );
    c.combat_rules.as_mut().unwrap().triggers[1].action_filter = Some(
        serde_json::from_value(json!({"basic":true,"skills":{"mode":"only","ids":[]}})).unwrap(),
    );
    w.triggers.origin = Some(ActionOrigin::Skill(c.skill.id.clone()));
    for i in 0..3 {
        damage(&mut w, &c, 0, (i + 1) * 30);
    }
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "leaf")
    );
    let id = view(&w).instance_id;
    w.statuses.consume(&id, 1, &mut vec![]);
    w.triggers.origin = Some(ActionOrigin::Basic);
    damage(&mut w, &c, 0, 120);
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "leaf")
        .unwrap();
    assert!(w.statuses.is_derived(&s.instance_id));
    assert!(w.statuses.action_origin(&s.instance_id).is_none());
}
#[test]
fn stack_counter_rust_full_delta_decode_real_zero_and_reject_malformed_atomically() {
    use crate::replication::{DecodeError, ReplicaV6};
    let (c, w) = setup();
    let before = w.snapshot(0, &c, Phase::Active, 30);
    let f = PublicationEnvelopeV6 {
        key: MatchKey {
            id: "counter".into(),
            incarnation: 1,
        },
        publication_id: "counter-1".into(),
        recipient_id: "P00".into(),
        connection: Connection {
            id: "c0".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            snapshot: Box::new(before.clone()),
            reason: FullReason::Initial,
        },
    };
    let mut r = ReplicaV6::new(f.key.clone(), "P00".into());
    r.apply(&f).unwrap();
    for i in 0..7 {
        let mut after = before.clone();
        let s = &mut after.own.statuses[0];
        match i {
            0 => s.stack_counter = None,
            1 => s.stack_counter = Some(false),
            2 => s.stacks = Some(4),
            3 => s.max_stacks = None,
            4 => s.attribute = Some(StatusAttribute::LifeSteal),
            5 => s.flags = vec![StatusAttribute::Root],
            _ => s.base_modifiers.push(s.base_modifiers[0].clone()),
        };
        let d = PublicationEnvelopeV6 {
            publication_id: "counter-2".into(),
            revision: 2,
            payload: PublicationPayloadV6::Delta {
                base_publication_id: "counter-1".into(),
                base_revision: 1,
                delta: Box::new(crate::replication::diff(&before, &after)),
            },
            ..f.clone()
        };
        assert_eq!(r.apply(&d), Err(DecodeError::InvalidFrame), "case {i}");
        assert_eq!(r.snapshot(), Some(&before));
    }
    let mut after = before.clone();
    after.own.statuses[0].stacks = Some(3);
    let d = PublicationEnvelopeV6 {
        publication_id: "counter-2".into(),
        revision: 2,
        payload: PublicationPayloadV6::Delta {
            base_publication_id: "counter-1".into(),
            base_revision: 1,
            delta: Box::new(crate::replication::diff(&before, &after)),
        },
        ..f
    };
    assert_eq!(r.apply(&d).unwrap().own.statuses[0].stacks, Some(3));
}
#[test]
fn stack_counter_fixed_reduction_is_applied_before_shield_and_hp_at_zero_and_full_layers() {
    for stacks in [0, 3] {
        let (mut c, mut w) = setup();
        for profile in c.combat_rules.as_mut().unwrap().profiles.values_mut() {
            profile.tier = 0;
        }
        let original = view(&w);
        w.statuses
            .adjust_counter(&original.instance_id, stacks, 30, None, &mut vec![]);
        let own = w.members[0].reference.entity.clone();
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "ward", 0);
        w.members[0].hp = 50;
        let result = w.resolve_damage_with_rule(
            DamageIntent {
                source: w.npcs[0].entity.clone(),
                target: own,
                raw: 40,
                direct: true,
                ignore_armor: true,
            },
            Some(&crate::config::AttackRule {
                damage_kind: crate::config::DamageKind::Physical,
                unavoidable: true,
                can_crit: false,
                ignore_armor: true,
                ..Default::default()
            }),
            &c,
            30,
            &mut vec![],
        );
        assert_eq!(result.hp_damage + result.absorbed, 32);
        assert!(result.absorbed > 0);
        assert_eq!(w.members[0].hp, 50 - result.hp_damage);
        // A second cap is runtime aggregation, not fixed values multiplied by the layers.
        c.combat_rules.as_mut().unwrap().life_steal_cap_bps = Some(1800);
        assert_eq!(value(&c, &w, StatusAttribute::DamageReduction, 30), 12000);
    }
}
#[test]
fn stack_counter_original_seed_vector_matches_native_instances_and_window_formula() {
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/stack_window_vectors.json"
    ))
    .unwrap();
    assert_eq!(v["fullOriginalStageBindingsAccepted"], false);
    let (mut c, mut w) = setup();
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|s| s.id == "counter")
        .unwrap();
    d.duration_ms = v["baseDurationMs"].as_u64().unwrap();
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "counter", 0);
    let zero = view(&w);
    assert_eq!(zero.expires_at_ms, 12000);
    assert_eq!(zero.stacks, Some(v["initial"].as_u64().unwrap() as u32));
    assert_eq!(
        value(&c, &w, StatusAttribute::LifeSteal, 30),
        10000 + v["baseLifeStealBps"].as_u64().unwrap() as u32
    );
    w.statuses
        .adjust_counter(&zero.instance_id, 3, 30, None, &mut vec![]);
    assert_eq!(value(&c, &w, StatusAttribute::LifeSteal, 30), 12400);
    assert_eq!(value(&c, &w, StatusAttribute::DamageReduction, 30), 12000);
    let rule:crate::config::StatusWindowRule=serde_json::from_value(json!({"basis":"damage_dealt","include_periodic":false,"record_cap":100000,"refresh":"reset","maximum_sampling":"snapshot","on_expired":true,"on_consumed":false,"hp":{"fixed":0,"recorded_bps":v["expiryRecordedBps"],"max_resource_bps":0,"cap_max_resource_bps":v["expiryMaxHpCapBps"]}})).unwrap();
    let formula = rule.hp.unwrap();
    assert_eq!(formula.amount(700, 1000), 70);
    assert_eq!(formula.amount(100000, 1000), 200);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|s| s.id == "counter")
        .unwrap()
        .duration_ms = v["masteredDurationMs"].as_u64().unwrap();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own, "counter", 300);
    assert_eq!((view(&w).stacks, view(&w).expires_at_ms), (Some(0), 15300));
}
#[test]
fn stack_counter_zero_carrier_expiry_cleanse_uses_original_lifetime_and_valid_private_projection() {
    let (mut c, mut w) = setup();
    policy(&mut c, "counter", false);
    c.combat_rules.as_mut().unwrap().status_cleanses.insert(
        "counter".into(),
        serde_json::from_value(
            json!({"on_expired":{"disposition":"harmful","max_count":1,"scope":"all"}}),
        )
        .unwrap(),
    );
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "counter", 0);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"long-debuff","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":-1000,"duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    policy(&mut c, "long-debuff", true);
    super::periodic_tests::bind(&mut w, &c, Owner::Npc(0), own, "long-debuff", 30);
    let before = w.snapshot(0, &c, Phase::Active, 30);
    let frame = PublicationEnvelopeV6 {
        key: MatchKey {
            id: "zero-expiry".into(),
            incarnation: 1,
        },
        publication_id: "zero-1".into(),
        recipient_id: "P00".into(),
        connection: Connection {
            id: "c0".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            snapshot: Box::new(before),
            reason: FullReason::Initial,
        },
    };
    let mut replica = crate::replication::ReplicaV6::new(frame.key.clone(), "P00".into());
    replica.apply(&frame).unwrap();
    assert_eq!(view(&w).stacks, Some(0));
    let mut events = vec![];
    w.prepare_periodic(1200, &mut events);
    w.finish_status_cleanses(&c, 1200, &mut events);
    assert!(w.statuses.entries().is_empty());
    assert_eq!(
        events
            .iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusCleanseResolved {
                        removed_count: 1,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
}
