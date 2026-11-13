use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn definition(policy: &str) -> crate::config::ResourceSteal {
    serde_json::from_value(
        json!({"hp_current_bps":1000,"spirit_current_bps":1000,"hp_policy":policy}),
    )
    .unwrap()
}
fn setup(channel: bool, policy: &str) -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(channel, false);
    c.skill.resource_steal = Some(definition(policy));
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("hp".into(), 1000);
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("spirit".into(), 1000);
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("def".into(), 0);
    w.npcs[0].hp = 1000;
    w.npcs[0].spirit = 123;
    (c, w)
}
fn start(c: &Config, w: &mut World) {
    super::unit_effect_tests::start(c, w, false);
}
fn rows(e: &Events) -> Vec<(u32, u32, u32, u32, u32, u32)> {
    e.iter()
        .filter_map(|(r, e)| {
            if let EventV4::Private {
                event:
                    Event::ResourceStolen {
                        requested_hp,
                        requested_spirit,
                        hp_removed,
                        spirit_removed,
                        hp_restored,
                        spirit_restored,
                        ..
                    },
            } = e
            {
                assert_eq!(*r, Some(0));
                Some((
                    *requested_hp,
                    *requested_spirit,
                    *hp_removed,
                    *spirit_removed,
                    *hp_restored,
                    *spirit_restored,
                ))
            } else {
                None
            }
        })
        .collect()
}
fn bind(w: &mut World, c: &mut Config, target: Owner, d: Value) {
    let id = d["id"].as_str().unwrap().to_string();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
    let t = w.owner_entity(target);
    super::periodic_tests::bind(w, c, target, t, &id, 0);
}
#[test]
fn resource_steal_config_strict_shape_policy_combinations_and_roundtrip() {
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/units-a.json"
    ))
    .unwrap();
    d["skill"]["resource_steal"] = serde_json::to_value(definition("direct_transfer")).unwrap();
    p.write(&d);
    let c = p.load().unwrap();
    let normalized: SkillConfig = serde_json::from_value(d["skill"].clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&c.default_config().skill).unwrap(),
        serde_json::to_value(normalized).unwrap()
    );
    for case in 0..10 {
        let mut bad = d.clone();
        match case {
            0 => bad["mechanism_version"] = json!(104),
            1 => bad["skill"]["resource_steal"] = Value::Null,
            2 => bad["skill"]["resource_steal"]["hp_current_bps"] = json!(10001),
            3 => bad["skill"]["resource_steal"]["spirit_current_bps"] = json!(-1),
            4 => bad["skill"]["resource_steal"]["hp_policy"] = json!({"damage":null}),
            5 => bad["skill"]["resource_steal"]["hp_policy"] = Value::Null,
            6 => {
                bad["skill"]["resource_steal"]
                    .as_object_mut()
                    .unwrap()
                    .remove("hp_policy");
            }
            7 => {
                bad["skill"]["resource_steal"]["hp_current_bps"] = json!(0);
            }
            8 => {
                bad["skill"]["resource_steal"] = json!({"hp_current_bps":0,"spirit_current_bps":0});
            }
            _ => bad["skill"]["resource_steal"]["evil"] = json!(1),
        };
        p.write(&bad);
        assert!(p.load().is_err(), "{case}");
    }
    d["skill"]["resource_steal"] = json!({"hp_current_bps":0,"spirit_current_bps":1000});
    p.write(&d);
    assert!(p.load().is_ok());
}
#[test]
fn resource_steal_channel_freezes_current_resources_and_total_final_shares() {
    let (c, mut w) = setup(true, "direct_transfer");
    start(&c, &mut w);
    assert_eq!(
        w.members[0].channel.as_ref().unwrap().steal_budget,
        Some([100, 12])
    );
    let mut result = vec![];
    for now in [120, 180, 240] {
        let mut e = vec![];
        w.advance_channel(0, &c, now, &mut e);
        result.extend(rows(&e));
    }
    assert_eq!(
        result,
        vec![
            (33, 4, 33, 4, 33, 4),
            (33, 4, 33, 4, 17, 4),
            (34, 4, 34, 4, 0, 4)
        ]
    );
    assert_eq!((w.npcs[0].hp, w.npcs[0].spirit), (892, 111));
    assert_eq!((w.members[0].hp, w.members[0].spirit), (100, 57));
    assert!(w.members[0].channel.is_none());
}
#[test]
fn resource_steal_later_resource_changes_do_not_requote_committed_budget() {
    let (c, mut w) = setup(true, "direct_transfer");
    start(&c, &mut w);
    w.advance_channel(0, &c, 120, &mut vec![]);
    w.npcs[0].spirit = 500;
    w.members[0].facts.atk_points = 10000;
    let mut e = vec![];
    w.advance_channel(0, &c, 180, &mut e);
    assert_eq!(rows(&e)[0].0, 33);
    assert_eq!(rows(&e)[0].1, 4);
    assert_eq!(w.npcs[0].spirit, 496);
}
#[test]
fn resource_steal_direct_transfer_bypasses_damage_reduction_and_shield_but_damage_policy_uses_them()
{
    for policy in ["direct_transfer", "damage"] {
        let (mut c, mut w) = setup(false, policy);
        bind(
            &mut w,
            &mut c,
            Owner::Npc(0),
            json!({"id":"steal-shield","mechanism":"absorption_shield","shield_bps":500,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
        );
        bind(
            &mut w,
            &mut c,
            Owner::Npc(0),
            json!({"id":"steal-dr","mechanism":"timed_attribute_modifier","attribute":"damage_reduction","modifier_bps":2000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
        );
        let target = w.npcs[0].entity.clone();
        let mut e = vec![];
        w.steal_resources(
            (Owner::Member(0), &target, Some("settle")),
            &definition(policy),
            [100, 0],
            &c,
            30,
            &mut e,
        );
        // Existing member tier2 vs NPC tier0 gives 1.2; then 20% DR, then shield50.
        let taken = if policy == "direct_transfer" { 100 } else { 46 };
        assert_eq!(w.npcs[0].hp, 1000 - taken);
        assert_eq!(rows(&e)[0].2, taken);
        let shields = w
            .statuses
            .entries()
            .iter()
            .filter_map(|s| s.shield.as_ref())
            .map(|s| s.remaining)
            .sum::<u32>();
        assert_eq!(shields, if policy == "direct_transfer" { 50 } else { 0 });
    }
}
#[test]
fn resource_steal_damage_immunity_blocks_hp_but_spirit_is_independent() {
    for policy in ["direct_transfer", "damage"] {
        let (mut c, mut w) = setup(false, policy);
        bind(
            &mut w,
            &mut c,
            Owner::Npc(0),
            json!({"id":"steal-immune","mechanism":"timed_control","attribute":"damage_immunity","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
        );
        let target = w.npcs[0].entity.clone();
        let mut e = vec![];
        w.steal_resources(
            (Owner::Member(0), &target, Some("immune")),
            &definition(policy),
            [100, 12],
            &c,
            30,
            &mut e,
        );
        assert_eq!(rows(&e), vec![(100, 12, 0, 12, 0, 12)]);
        assert_eq!((w.npcs[0].hp, w.npcs[0].spirit), (1000, 111));
        assert!(!e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::HealResolved {
                    heal_kind: HealKind::ResourceSteal,
                    ..
                }
            }
        )));
    }
}
#[test]
fn resource_steal_source_receiving_reduction_and_spirit_lock_apply_to_actual_loss() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    bind(
        &mut w,
        &mut c,
        Owner::Member(0),
        json!({"id":"steal-half","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":-5000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    bind(
        &mut w,
        &mut c,
        Owner::Member(0),
        json!({"id":"steal-lock","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock","modifier_bps":-10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let target = w.npcs[0].entity.clone();
    let mut e = vec![];
    w.steal_resources(
        (Owner::Member(0), &target, Some("receive")),
        &definition("direct_transfer"),
        [100, 12],
        &c,
        30,
        &mut e,
    );
    assert_eq!(rows(&e), vec![(100, 12, 100, 12, 50, 0)]);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (100, 50));
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ResourceStolen {
                hp_excess: 0,
                spirit_excess: 12,
                ..
            }
        }
    )));
}
#[test]
fn resource_steal_zero_budget_shares_do_not_invent_minimum_damage() {
    let (mut c, mut w) = setup(true, "direct_transfer");
    c.skill.resource_steal = Some(
        serde_json::from_value(
            json!({"hp_current_bps":10,"spirit_current_bps":100,"hp_policy":"direct_transfer"}),
        )
        .unwrap(),
    );
    start(&c, &mut w);
    let mut result = vec![];
    for now in [120, 180, 240] {
        let mut e = vec![];
        w.advance_channel(0, &c, now, &mut e);
        result.extend(rows(&e));
    }
    assert_eq!(
        result.iter().map(|r| (r.0, r.1)).collect::<Vec<_>>(),
        vec![(0, 0), (0, 0), (1, 1)]
    );
    assert_eq!(w.npcs[0].hp, 991);
    assert_eq!(w.npcs[0].spirit, 122);
    assert_eq!(w.members[0].hp, 51);
}
#[test]
fn resource_steal_cancellation_keeps_only_committed_prefix() {
    let (c, mut w) = setup(true, "direct_transfer");
    start(&c, &mut w);
    w.advance_channel(0, &c, 120, &mut vec![]);
    let old = (
        w.npcs[0].hp,
        w.npcs[0].spirit,
        w.members[0].hp,
        w.members[0].spirit,
    );
    w.action(0, &Action::CancelAction {}, "cancel", &c, 150)
        .unwrap();
    for now in [180, 240, 300] {
        let mut e = vec![];
        w.advance_channel(0, &c, now, &mut e);
        assert!(rows(&e).is_empty());
    }
    assert_eq!(
        (
            w.npcs[0].hp,
            w.npcs[0].spirit,
            w.members[0].hp,
            w.members[0].spirit
        ),
        old
    );
}
#[test]
fn resource_steal_lethal_withdrawal_clears_future_channel_and_never_repeats_death() {
    let (mut c, mut w) = setup(true, "direct_transfer");
    c.skill.resource_steal.as_mut().unwrap().hp_current_bps = 10000;
    start(&c, &mut w);
    w.npcs[0].hp = 10;
    let mut e = vec![];
    w.advance_channel(0, &c, 120, &mut e);
    assert_eq!(w.npcs[0].hp, 0);
    assert_eq!(rows(&e)[0].0, 333);
    assert_eq!(rows(&e)[0].2, 8);
    assert!(w.members[0].channel.is_none());
    let mut later = vec![];
    w.advance_channel(0, &c, 180, &mut later);
    assert!(rows(&later).is_empty());
}
#[test]
fn resource_steal_reflected_primary_kill_does_not_resurrect_caster_or_restore_channel() {
    let (mut c, mut w) = setup(true, "direct_transfer");
    w.members[0].hp = 1;
    c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"reflect-during-pulse","profiles":["npc"],"when":"damage_taken","include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":10000,"damage_kind":"true_damage"}}])).unwrap();
    start(&c, &mut w);
    let mut e = vec![];
    w.advance_channel(0, &c, 120, &mut e);
    assert_eq!(w.members[0].hp, 0);
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.npcs[0].spirit, 123);
    assert_eq!(rows(&e)[0].2, 0);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ChannelEnded {
                reason: ChannelEndReason::SourceInvalid,
                ..
            }
        }
    )));
}
fn trigger(when: &str, counting: &str) -> Value {
    json!({"id":"steal-passive","profiles":["member"],"when":when,"include_periodic":false,"counting":counting,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"steal_other","steal":{"hp_current_bps":1000,"spirit_current_bps":1000,"hp_policy":"direct_transfer"}}})
}
fn contact(w: &mut World, c: &Config, metal: bool, id: &str) -> Events {
    let source = w.members[0].reference.entity.clone();
    let rule = crate::config::AttackRule {
        damage_kind: crate::config::DamageKind::TrueDamage,
        ignore_armor: true,
        unavoidable: true,
        can_crit: false,
        ..Default::default()
    };
    let mut e = vec![];
    assert!(w.direct_contact(
        Owner::Member(0),
        Owner::Npc(0),
        combat::DirectContact {
            origin: None,
            action_id: Some(id),
            rule: Some(&rule),
            ignore_armor: true,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw: 1,
            metal,
            hit_statuses: &[]
        },
        c,
        30,
        &mut e
    ));
    e
}
#[test]
fn resource_steal_basic_fact_filters_skills_without_triggering_itself() {
    for skill in [false, true] {
        let (mut c, mut w) = setup(false, "direct_transfer");
        c.skill.resource_steal = None;
        c.combat_rules.as_mut().unwrap().triggers =
            serde_json::from_value(json!([trigger("basic_hit", "contact")])).unwrap();
        let e = contact(&mut w, &c, skill, "hit");
        if skill {
            assert!(rows(&e).is_empty());
            assert_eq!(w.npcs[0].hp, 999);
        } else {
            assert_eq!(rows(&e).len(), 1);
            assert_eq!(rows(&e)[0].0, 99);
            assert_eq!(w.npcs[0].hp, 900);
            assert_eq!(w.npcs[0].spirit, 111);
        }
    }
}
#[test]
fn resource_steal_direct_hit_trigger_action_count_is_once_and_cannot_recurse() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    c.skill.resource_steal = None;
    c.combat_rules.as_mut().unwrap().triggers =
        serde_json::from_value(json!([trigger("direct_hit", "action")])).unwrap();
    let a = contact(&mut w, &c, true, "same-action");
    let b = contact(&mut w, &c, true, "same-action");
    assert_eq!(rows(&a).len(), 1);
    assert!(rows(&b).is_empty());
    assert_eq!(w.npcs[0].hp, 899);
    assert_eq!(w.npcs[0].spirit, 111);
}
#[test]
fn resource_steal_transfer_is_not_damage_but_damage_policy_can_trigger_reflection() {
    for policy in ["direct_transfer", "damage"] {
        let (mut c, mut w) = setup(false, policy);
        c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"reflection","profiles":["npc"],"when":"damage_taken","include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"reflect_damage","bps":10000,"damage_kind":"true_damage"}}])).unwrap();
        let target = w.npcs[0].entity.clone();
        let mut e = vec![];
        w.steal_resources(
            (Owner::Member(0), &target, Some("only-steal")),
            &definition(policy),
            [100, 0],
            &c,
            30,
            &mut e,
        );
        if policy == "direct_transfer" {
            assert_eq!(w.members[0].hp, 100);
            assert!(!e.iter().any(|(_, e)| matches!(e, EventV4::Damage { .. })));
        } else {
            assert_eq!(w.members[0].hp, 0);
            assert_eq!(rows(&e)[0].4, 0);
        }
    }
}
#[test]
fn resource_steal_external_resource_loss_invalidates_reserved_cast_without_partial_payment() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    c.skill.resource_steal = None;
    c.skill.resource_commit=serde_json::from_value(json!({"reserve_at":"accept","consume_at":"commit","cooldown_starts_at":"commit","refund_before_commit":{"kind":"full"}})).unwrap();
    c.skill.hp_cost = serde_json::from_value(
        json!({"mode":"current_hp_ratio","value":0.5,"min_hp_after_cost":1}),
    )
    .unwrap();
    c.skill.cost = 10;
    w.members[0].hp = 100;
    w.members[0].spirit = 10;
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        },
        "reserved",
        &c,
        0,
    )
    .unwrap();
    let target = w.members[0].reference.entity.clone();
    w.steal_resources(
        (Owner::Npc(0), &target, Some("incoming")),
        &definition("direct_transfer"),
        [80, 5],
        &c,
        30,
        &mut vec![],
    );
    assert_eq!((w.members[0].hp, w.members[0].spirit), (20, 5));
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, &c, 60, &mut e);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (20, 5));
    assert!(w.members[0].cooldowns.is_empty());
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ActionRejected {
                reason: Error::NoResource,
                ..
            }
        }
    )));
}
#[test]
fn resource_steal_billion_unit_resources_keep_integer_caps_and_excess_conservation() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    c.resource_limits = serde_json::from_value(
        json!({"max_hp":1000000000,"max_spirit":1000000000,"max_spirit_cost":8000000}),
    )
    .unwrap();
    for name in ["hp", "spirit"] {
        Arc::make_mut(&mut w.npcs[0].profile)
            .stats
            .insert(name.into(), 1000000000);
    }
    w.npcs[0].hp = 1000000000;
    w.npcs[0].spirit = 1000000000;
    w.members[0].facts.hp_points = 1000000000;
    w.members[0].facts.spirit_points = 1000000000;
    w.members[0].hp = 10;
    w.members[0].spirit = 10;
    let target = w.npcs[0].entity.clone();
    let mut e = vec![];
    w.steal_resources(
        (Owner::Member(0), &target, Some("large")),
        &definition("direct_transfer"),
        [1000000000, 1000000000],
        &c,
        30,
        &mut e,
    );
    assert_eq!((w.npcs[0].hp, w.npcs[0].spirit), (0, 0));
    assert_eq!(
        (w.members[0].hp, w.members[0].spirit),
        (1000000000, 1000000000)
    );
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ResourceStolen {
                hp_excess: 10,
                spirit_excess: 10,
                ..
            }
        }
    )));
}
#[test]
fn resource_steal_instant_snapshots_before_primary_damage_and_pays_only_once() {
    let (c, mut w) = setup(false, "direct_transfer");
    let e = super::unit_effect_tests::start(&c, &mut w, false);
    assert_eq!(rows(&e), vec![(100, 12, 100, 12, 50, 12)]);
    assert_eq!((w.npcs[0].hp, w.npcs[0].spirit), (891, 111));
    assert_eq!(w.members[0].spirit, 60);
    assert!(w.shots.is_empty());
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::ActionCommitted { .. }
                }
            ))
            .count(),
        1
    );
}
#[test]
fn resource_steal_guarded_lethal_uses_actual_loss_and_once_per_target_lifecycle() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    c.combat_rules.as_mut().unwrap().death_guards.insert(
        "npc".into(),
        serde_json::from_value(json!({"retain_hp":1,"heal_max_hp_bps":0,"self_statuses":[]}))
            .unwrap(),
    );
    w.npcs[0].hp = 100;
    let target = w.npcs[0].entity.clone();
    let mut e = vec![];
    w.steal_resources(
        (Owner::Member(0), &target, Some("guarded")),
        &definition("direct_transfer"),
        [1000, 0],
        &c,
        30,
        &mut e,
    );
    assert_eq!(w.npcs[0].hp, 1);
    assert_eq!(rows(&e)[0].2, 99);
    assert_eq!(rows(&e)[0].4, 50);
    assert_eq!(
        e.iter()
            .filter(|(recipient, e)| *recipient == Some(0)
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::DeathGuardActivated { .. }
                    }
                ))
            .count(),
        1
    );
    let mut second = vec![];
    w.steal_resources(
        (Owner::Member(0), &target, Some("after-guard")),
        &definition("direct_transfer"),
        [1, 0],
        &c,
        60,
        &mut second,
    );
    assert_eq!(w.npcs[0].hp, 0);
    assert_eq!(rows(&second)[0].2, 1);
    let mut later = vec![];
    w.steal_resources(
        (Owner::Member(0), &target, Some("dead")),
        &definition("direct_transfer"),
        [1, 1],
        &c,
        90,
        &mut later,
    );
    assert_eq!(rows(&later)[0].2, 0);
    assert_eq!(rows(&later)[0].3, 0);
}
#[test]
fn resource_steal_trigger_budget_does_not_take_extra_resources_after_limit() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    c.skill.resource_steal = None;
    let r = c.combat_rules.as_mut().unwrap();
    r.triggers = serde_json::from_value(json!([trigger("basic_hit", "contact")])).unwrap();
    r.max_triggers_per_step = 1;
    let a = contact(&mut w, &c, false, "first");
    let b = contact(&mut w, &c, false, "second");
    assert_eq!(rows(&a).len(), 1);
    assert!(rows(&b).is_empty());
    assert_eq!((w.npcs[0].hp, w.npcs[0].spirit), (899, 111));
}
#[test]
fn resource_steal_trigger_loader_rejects_recursive_facts_and_accepts_basic_hit() {
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/units-a.json"
    ))
    .unwrap();
    d["combat_rules"]["triggers"] = json!([trigger("basic_hit", "contact")]);
    p.write(&d);
    assert!(p.load().is_ok());
    for fact in [
        "damage_dealt",
        "damage_taken",
        "shield_absorbed",
        "kill",
        "skill_committed",
    ] {
        let mut bad = d.clone();
        bad["combat_rules"]["triggers"][0]["when"] = json!(fact);
        p.write(&bad);
        assert!(p.load().is_err(), "{fact}");
    }
}
fn vamp(w: &mut World, c: &mut Config, id: &str, bps: i32, stack: bool) {
    let mut d = json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"life_steal","modifier_bps":bps,"duration_ms":300,"duplicate_policy":if stack{"stack"}else{"refresh"},"source_scoped":true});
    if stack {
        d["max_stacks"] = json!(3);
    }
    bind(w, c, Owner::Member(0), d);
}
fn hit_for_vamp(w: &mut World, c: &Config, direct: bool, static_bps: u32, now: u64) {
    let source = w.members[0].reference.entity.clone();
    let target = w.npcs[0].entity.clone();
    let rule = crate::config::AttackRule {
        damage_kind: crate::config::DamageKind::TrueDamage,
        can_crit: false,
        life_steal_bps: static_bps,
        ..Default::default()
    };
    w.resolve_damage_with_rule(
        combat::DamageIntent {
            source,
            target,
            raw: 99,
            direct,
            ignore_armor: true,
        },
        Some(&rule),
        c,
        now,
        &mut vec![],
    );
}
#[test]
fn resource_steal_life_steal_status_merges_layers_before_one_floor_with_live_cap_and_expiry() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    vamp(&mut w, &mut c, "vamp", 1000, true);
    let target = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), target, "vamp", 0);
    hit_for_vamp(&mut w, &c, true, 0, 30);
    assert_eq!(w.members[0].hp, 69);
    w.members[0].hp = 50;
    c.combat_rules.as_mut().unwrap().life_steal_cap_bps = Some(1500);
    hit_for_vamp(&mut w, &c, true, 0, 60);
    assert_eq!(w.members[0].hp, 64);
    w.members[0].hp = 50;
    hit_for_vamp(&mut w, &c, true, 0, 330);
    assert_eq!(w.members[0].hp, 50);
}
#[test]
fn resource_steal_life_steal_dynamic_bonus_excludes_dot_and_counter_but_static_rules_remain() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    vamp(&mut w, &mut c, "vamp", 2000, false);
    hit_for_vamp(&mut w, &c, false, 0, 30);
    assert_eq!(w.members[0].hp, 50);
    hit_for_vamp(&mut w, &c, false, 500, 60);
    assert_eq!(w.members[0].hp, 54);
}
#[test]
fn resource_steal_life_steal_can_be_reduced_or_disabled_without_underflow() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    vamp(&mut w, &mut c, "vamp-loss", -1000, false);
    hit_for_vamp(&mut w, &c, true, 2000, 30);
    assert_eq!(w.members[0].hp, 59);
    w.members[0].hp = 50;
    hit_for_vamp(&mut w, &c, true, 500, 60);
    assert_eq!(w.members[0].hp, 50);
    c.combat_rules.as_mut().unwrap().life_steal_cap_bps = Some(0);
    hit_for_vamp(&mut w, &c, true, 10000, 90);
    assert_eq!(w.members[0].hp, 50);
}
#[test]
fn resource_steal_independent_transfer_does_not_double_return_with_life_steal_buff() {
    let (mut c, mut w) = setup(false, "direct_transfer");
    vamp(&mut w, &mut c, "vamp", 5000, false);
    let target = w.npcs[0].entity.clone();
    let mut e = vec![];
    w.steal_resources(
        (Owner::Member(0), &target, Some("separate")),
        &definition("direct_transfer"),
        [10, 0],
        &c,
        30,
        &mut e,
    );
    assert_eq!(w.members[0].hp, 60);
    assert_eq!(rows(&e)[0].4, 10);
    assert!(!e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::HealResolved {
                heal_kind: HealKind::LifeSteal,
                ..
            }
        }
    )));
}
#[test]
fn resource_steal_life_steal_modifier_and_cap_load_strictly_in_both_status_forms() {
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/units-a.json"
    ))
    .unwrap();
    d["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({"id":"configured-vamp","mechanism":"timed_attribute_modifier","attribute":"life_steal","modifier_bps":1000,"duration_ms":1200,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true}));
    d["skill"]["on_commit_self_statuses"] = json!(["configured-vamp"]);
    d["combat_rules"]["life_steal_cap_bps"] = json!(1500);
    p.write(&d);
    assert!(p.load().is_ok());
    for invalid in [Value::Null, json!(-1), json!(1000001)] {
        let mut bad = d.clone();
        bad["combat_rules"]["life_steal_cap_bps"] = invalid;
        p.write(&bad);
        assert!(p.load().is_err());
    }
    let last = d["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    d["pve"]["status_definitions"][last] = json!({"id":"configured-vamp","mechanism":"timed_compound_modifier","modifiers":[{"attribute":"life_steal","modifier_bps":1000},{"attribute":"atk","modifier_bps":500}],"duration_ms":1200,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true});
    p.write(&d);
    assert!(p.load().is_ok());
}

#[test]
fn resource_steal_original_three_tick_budget_and_actual_devoured_status_projection() {
    let fixture: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_resource_steal_vectors.json"
    ))
    .unwrap();
    assert_eq!(fixture["originalMappingAccepted"], false);
    assert_eq!(
        fixture["sourceBudget"]["snapshot"],
        "activation_before_primary_effects"
    );
    let status = &fixture["sourceStatus"];
    assert_eq!(
        status["definition"]["attachedFactors"][0]["params"][0],
        1000
    );
    for policy in ["direct_transfer", "damage"] {
        let (mut c, mut w) = setup(true, policy);
        let mut s = serde_json::to_value(&c.skill).unwrap();
        s["windup_ms"] = json!(180);
        s["damage_bps"] = json!(30000);
        s["channel"]["duration_ms"] = json!(1200);
        s["channel"]["interval_ms"] = fixture["sourceBudget"]["intervalMs"].clone();
        s["channel"]["on_success"] = json!([{"after_pulse":2,"self_statuses":["real-devoured"]}]);
        c.skill = serde_json::from_value(s).unwrap();
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"real-devoured","mechanism":"timed_attribute_modifier","attribute":"life_steal","modifier_bps":status["lifeStealBpsPerLayer"],"duration_ms":status["durationMs"],"duplicate_policy":"stack","max_stacks":status["maxStacks"],"source_scoped":true})).unwrap());
        let target = w.npcs[0].entity.clone();
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: World::unit_wire(&target),
            },
            "source-projection",
            &c,
            0,
        )
        .unwrap();
        let p = w.members[0].pending.take().unwrap();
        let mut e = vec![];
        w.commit(0, p, &c, 180, &mut e);
        assert_eq!(
            w.members[0].channel.as_ref().unwrap().steal_budget,
            Some([100, 12])
        );
        w.advance_channel(0, &c, 580, &mut e);
        assert!(
            !w.statuses
                .entries()
                .iter()
                .any(|v| v.definition_id == "real-devoured")
        );
        w.advance_channel(0, &c, 980, &mut e);
        let v = w
            .statuses
            .entries()
            .iter()
            .find(|v| v.definition_id == "real-devoured")
            .unwrap();
        assert_eq!(v.stacks, Some(1));
        assert_eq!(v.attribute, Some(StatusAttribute::LifeSteal));
        assert_eq!((v.modifier_bps, v.expires_at_ms), (1000, 12980));
        w.advance_channel(0, &c, 1380, &mut e);
        assert!(w.members[0].channel.is_none());
        let transfers = rows(&e);
        assert_eq!(
            transfers.iter().map(|r| r.0).collect::<Vec<_>>(),
            vec![33, 33, 34]
        );
        assert_eq!(transfers.iter().map(|r| r.1).sum::<u32>(), 12);
        assert_eq!(
            transfers.iter().map(|r| r.2).sum::<u32>(),
            if policy == "damage" { 118 } else { 100 }
        );
        assert_eq!(w.npcs[0].hp, if policy == "damage" { 855 } else { 873 });
    }
}

#[test]
fn resource_steal_public_configs_cold_load_with_native_npc_numeric_self_statuses() {
    let p = Package::new();
    for raw in [
        crate::test_data::fixture!("examples/combat-host/steal-a.json"),
        crate::test_data::fixture!("examples/combat-host/steal-b.json"),
    ] {
        let d: Value = serde_json::from_str(raw).unwrap();
        p.write(&d);
        assert!(p.load().is_ok());
    }
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/steal-b.json"
    ))
    .unwrap();
    d["pve"]["status_definitions"][2] = json!({"id":"reduce","mechanism":"timed_control","attribute":"stun","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true});
    p.write(&d);
    assert!(p.load().is_err());
}
