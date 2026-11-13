use super::periodic_tests::bind;
use super::*;
use serde_json::{Value, json};
fn carrier(remove: bool) -> Value {
    json!({"id":"charges","mechanism":"timed_compound_modifier","base_modifiers":[{"attribute":"def","modifier_bps":0}],"stack_counter":{"initial":3,"maximum":3,"zero_policy":if remove{"remove"}else{"keep"}},"duration_ms":8000,"duplicate_policy":"refresh","source_scoped":true})
}
fn setup(channel: bool) -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(channel, false);
    w.statuses = Default::default();
    c.skill.cost = 10;
    c.skill.cooldown_ms = 60;
    c.skill.recovery_ms = 0;
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(carrier(true)).unwrap());
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"exhausted","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock","modifier_bps":-10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let r = c.combat_rules.as_mut().unwrap();
    r.triggers=serde_json::from_value(json!([{"id":"paid-return","profiles":["member"],"when":"skill_cost_committed","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"conditions":{"status":{"subject":"owner","definition_id":"charges","at_least":1,"consume":1}},"effect":{"kind":"restore_paid_spirit","bps":3000}}])).unwrap();
    r.status_ends.insert(
        "charges".into(),
        serde_json::from_value(json!({"on":["consumed"],"target_statuses":["exhausted"]})).unwrap(),
    );
    w.members[0].spirit = 100;
    let e = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), e, "charges", 0);
    (c, w)
}
fn remaining(w: &World) -> Option<u32> {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "charges")
        .map(|s| s.stacks.unwrap())
}
fn paid(e: &Events) -> Vec<(u32, u32, u32, u32)> {
    e.iter()
        .filter_map(|(r, e)| match e {
            EventV4::Private {
                event:
                    Event::PaidSpiritRestored {
                        paid_spirit,
                        requested,
                        restored,
                        excess,
                        ..
                    },
            } => {
                assert_eq!(*r, Some(0));
                Some((*paid_spirit, *requested, *restored, *excess))
            }
            _ => None,
        })
        .collect()
}
fn commit(c: &Config, w: &mut World, now: u64) -> Events {
    let target = w.npcs[0].entity.clone();
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&target),
        },
        &format!("cost-{now}"),
        c,
        now,
    )
    .unwrap();
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, c, now + 60, &mut e);
    e
}
#[test]
fn paid_resource_three_real_debits_return_before_once_only_exhaustion_lock() {
    let (c, mut w) = setup(false);
    let mut all = vec![];
    for (i, spirit) in [93, 86, 79].into_iter().enumerate() {
        let now = i as u64 * 120;
        let mut e = commit(&c, &mut w, now);
        assert_eq!(paid(&e), vec![(10, 3, 3, 0)]);
        assert_eq!(w.members[0].spirit, spirit);
        assert_eq!(
            remaining(&w),
            if i == 2 { None } else { Some(2 - i as u32) }
        );
        assert!(!w.entity_has_status(
            Owner::Member(0),
            StatusAttribute::SpiritRecoveryLock,
            now + 60
        ));
        w.finish_status_ends(&c, now + 60, &mut e);
        all.extend(e);
    }
    let lock = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "exhausted")
        .unwrap();
    assert_eq!(lock.expires_at_ms, 3300);
    let refund = all
        .iter()
        .rposition(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::PaidSpiritRestored { .. }
                }
            )
        })
        .unwrap();
    let locked=all.iter().position(|(_,e)|matches!(e,EventV4::Private{event:Event::StatusApplied{status}}if status.definition_id=="exhausted")).unwrap();
    assert!(refund < locked);
    let e = commit(&c, &mut w, 360);
    assert!(paid(&e).is_empty());
    assert_eq!(w.members[0].spirit, 69);
    assert!(!w.entity_has_status(Owner::Member(0), StatusAttribute::SpiritRecoveryLock, 3300));
    let mut e = vec![];
    w.finish_status_ends(&c, 600, &mut e);
    assert!(e.is_empty());
}
#[test]
fn paid_resource_cannot_borrow_future_return_and_commit_failure_keeps_all_charges() {
    let (c, mut w) = setup(false);
    w.members[0].spirit = 9;
    let target = w.npcs[0].entity.clone();
    assert_eq!(
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: World::unit_wire(&target)
            },
            "unfunded",
            &c,
            0
        ),
        Err(Error::NoResource)
    );
    assert_eq!(remaining(&w), Some(3));
    w.members[0].spirit = 100;
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&target),
        },
        "accepted",
        &c,
        0,
    )
    .unwrap();
    w.members[0].spirit = 9;
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, &c, 60, &mut e);
    assert!(paid(&e).is_empty());
    assert_eq!(w.members[0].spirit, 9);
    assert_eq!(remaining(&w), Some(3));
    assert!(w.members[0].cooldowns.is_empty());
}
#[test]
fn paid_resource_zero_small_and_full_rate_have_single_wide_floor() {
    for (cost, bps, expected) in [
        (0, 3000, 0),
        (1, 3000, 0),
        (3, 3000, 0),
        (7, 3000, 2),
        (99, 3000, 29),
        (99, 10000, 99),
    ] {
        let (mut c, mut w) = setup(false);
        c.skill.cost = cost;
        c.combat_rules.as_mut().unwrap().triggers[0].effect =
            serde_json::from_value(json!({"kind":"restore_paid_spirit","bps":bps})).unwrap();
        let e = commit(&c, &mut w, 0);
        assert_eq!(paid(&e), vec![(cost, expected, expected, 0)]);
        assert_eq!(w.members[0].spirit, 100 - cost + expected);
        assert_eq!(remaining(&w), Some(2));
    }
}
#[test]
fn paid_resource_existing_recovery_lock_blocks_only_return_and_reports_excess() {
    let (c, mut w) = setup(false);
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "exhausted", 0);
    let e = commit(&c, &mut w, 0);
    assert_eq!(paid(&e), vec![(10, 3, 0, 3)]);
    assert_eq!(w.members[0].spirit, 90);
    assert_eq!(remaining(&w), Some(2));
}
#[test]
fn paid_resource_channel_initial_cost_does_not_reward_pulse_spending() {
    let (c, mut w) = setup(true);
    let mut e = commit(&c, &mut w, 0);
    assert_eq!(paid(&e), vec![(10, 3, 3, 0)]);
    for now in [120, 180, 240] {
        w.advance_channel(0, &c, now, &mut e);
    }
    assert_eq!(paid(&e).len(), 1);
    assert_eq!(remaining(&w), Some(2));
    assert_eq!(w.members[0].spirit, 90);
}
#[test]
fn paid_resource_condition_chance_icd_and_shared_budget_leave_original_cost_owned() {
    for mode in 0..3 {
        let (mut c, mut w) = setup(false);
        match mode {
            0 => c.combat_rules.as_mut().unwrap().triggers[0].chance_bps = Some(0),
            1 => {
                c.combat_rules.as_mut().unwrap().max_triggers_per_step = 1;
                let mut filler = c.combat_rules.as_ref().unwrap().triggers[0].clone();
                filler.id = "fill-budget".into();
                filler.when = crate::config::CombatFact::SkillCommitted;
                filler.conditions = Default::default();
                filler.effect = serde_json::from_value(
                    json!({"kind":"apply_self_status","status_id":"charges"}),
                )
                .unwrap();
                c.combat_rules.as_mut().unwrap().triggers.push(filler);
            }
            _ => {
                c.combat_rules.as_mut().unwrap().triggers[0]
                    .conditions
                    .minimum_amount = 11
            }
        };
        let e = commit(&c, &mut w, 0);
        assert!(paid(&e).is_empty());
        assert_eq!(remaining(&w), Some(3));
        assert_eq!(w.members[0].spirit, 90);
    }
    let (mut c, mut w) = setup(false);
    c.combat_rules.as_mut().unwrap().triggers[0].cooldown_ms = 1000;
    assert_eq!(paid(&commit(&c, &mut w, 0)).len(), 1);
    assert!(paid(&commit(&c, &mut w, 120)).is_empty());
    assert_eq!(remaining(&w), Some(2));
    assert_eq!(paid(&commit(&c, &mut w, 1080)).len(), 1);
}
#[test]
fn paid_resource_remove_policy_is_cloned_for_consumption_preview_and_keep_stays_zero() {
    for remove in [false, true] {
        let (mut c, mut w) = setup(false);
        w.statuses = Default::default();
        let d = c
            .pve_mut()
            .status_definitions
            .iter_mut()
            .find(|d| d.id == "charges")
            .unwrap();
        *d = serde_json::from_value(carrier(remove)).unwrap();
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own, "charges", 0);
        let id = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "charges")
            .unwrap()
            .instance_id
            .clone();
        let mut preview = w.statuses.clone();
        assert!(preview.consume(&id, 3, &mut vec![]));
        assert_eq!(
            preview
                .entries()
                .iter()
                .find(|s| s.instance_id == id)
                .map(|s| s.stacks.unwrap()),
            if remove { None } else { Some(0) }
        );
        assert_eq!(remaining(&w), Some(3));
        assert!(w.statuses.consume(&id, 3, &mut vec![]));
        w.finish_status_ends(&c, 60, &mut vec![]);
        assert_eq!(
            w.entity_has_status(Owner::Member(0), StatusAttribute::SpiritRecoveryLock, 60),
            remove
        );
    }
}
#[test]
fn paid_resource_expiration_and_disposal_never_improvise_exhaustion() {
    for expired in [false, true] {
        let (c, mut w) = setup(false);
        let mut e = vec![];
        if expired {
            w.statuses.expire(8000, &mut vec![]);
        } else {
            w.statuses
                .remove(StatusRemovalReason::Cleansed, |_| true, &mut vec![]);
        }
        w.finish_status_ends(&c, 8000, &mut e);
        assert!(!w.entity_has_status(Owner::Member(0), StatusAttribute::SpiritRecoveryLock, 8000));
        assert_eq!(remaining(&w), None);
        assert!(paid(&e).is_empty());
    }
}
#[test]
fn paid_resource_private_zero_policy_and_amount_context_cannot_be_injected() {
    let (c, w) = setup(false);
    let s = serde_json::to_value(w.statuses.entries()[0].clone()).unwrap();
    for field in ["zero_policy", "paid_spirit", "refund", "qualification"] {
        let mut bad = s.clone();
        bad[field] = json!(1);
        assert!(serde_json::from_value::<StatusView>(bad).is_err());
    }
    let mut action = serde_json::to_value(Action::CastUnit {
        skill_id: c.skill.id.clone(),
        target: World::unit_wire(&w.npcs[0].entity),
    })
    .unwrap();
    action["paid_spirit"] = json!(1000);
    assert!(serde_json::from_value::<Action>(action).is_err());
}
#[test]
fn paid_resource_cold_policy_effect_and_facts_are_strict_and_versioned() {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/activation-a.json"
    ))
    .unwrap();
    let (c, _) = setup(false);
    d["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(carrier(true));
    d["pve"]["status_definitions"].as_array_mut().unwrap().push(
        serde_json::to_value(
            c.pve
                .as_ref()
                .unwrap()
                .status_definitions
                .iter()
                .find(|s| s.id == "exhausted")
                .unwrap(),
        )
        .unwrap(),
    );
    d["combat_rules"]["triggers"] =
        serde_json::to_value(&c.combat_rules.as_ref().unwrap().triggers).unwrap();
    d["combat_rules"]["status_ends"] =
        serde_json::to_value(&c.combat_rules.as_ref().unwrap().status_ends).unwrap();
    p.write(&d);
    p.load().unwrap();
    let at = d["pve"]["status_definitions"].as_array().unwrap().len() - 2;
    for mode in 0..20 {
        let mut b = d.clone();
        match mode {
            0 => b["combat_rules"]["triggers"][0]["effect"]["bps"] = json!(0),
            1 => b["combat_rules"]["triggers"][0]["effect"]["bps"] = json!(10001),
            2 => b["combat_rules"]["triggers"][0]["effect"]["bps"] = Value::Null,
            3 => b["combat_rules"]["triggers"][0]["effect"]["cost"] = json!(20),
            4 => b["combat_rules"]["triggers"][0]["when"] = json!("skill_committed"),
            5 => {
                b["combat_rules"]["triggers"][0]["effect"] =
                    json!({"kind":"restore_spirit","amount":1})
            }
            6 => b["combat_rules"]["triggers"][0]["include_periodic"] = json!(true),
            7 => b["combat_rules"]["triggers"][0]["counting"] = json!("action_total"),
            8 => b["combat_rules"]["triggers"][0]["counter_scope"] = json!("target"),
            9 => {
                b["combat_rules"]["triggers"][0]["conditions"]["status"]["subject"] = json!("other")
            }
            10 => {
                b["combat_rules"]["triggers"][0]["conditions"]["other_hp_at_most_bps"] = json!(5000)
            }
            11 => b["pve"]["status_definitions"][at]["stack_counter"]["zero_policy"] = Value::Null,
            12 => {
                b["pve"]["status_definitions"][at]["stack_counter"]["zero_policy"] = json!("expire")
            }
            13 => b["pve"]["status_definitions"][at]["stack_counter"]["initial"] = json!(0),
            14 => b["pve"]["status_definitions"][at]["stack_counter"]["maximum"] = json!(129),
            15 => b["pve"]["status_definitions"][at]["stack_counter"]["extra"] = json!(true),
            16 => b["pve"]["status_definitions"][at]["duplicate_policy"] = json!("stack"),
            17 => b["schema_version"] = json!(6),
            18 => b["mechanism_version"] = json!(104),
            _ => b["combat_rules"]["triggers"][0]["effect"]["bps"] = json!(1.5),
        };
        p.write(&b);
        assert!(p.load().is_err(), "cold mode {mode}");
    }
}
#[test]
fn paid_resource_replay_target_loss_and_cancellation_never_double_return() {
    let (c, mut w) = setup(false);
    let target = w.npcs[0].entity.clone();
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&target),
        },
        "once",
        &c,
        0,
    )
    .unwrap();
    let repeated = Action::CastUnit {
        skill_id: c.skill.id.clone(),
        target: World::unit_wire(&target),
    };
    assert!(w.action(0, &repeated, "once", &c, 0).is_err());
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, &c, 60, &mut e);
    assert!(w.action(0, &repeated, "once", &c, 60).is_err());
    assert_eq!(paid(&e).len(), 1);
    assert_eq!(remaining(&w), Some(2));
    assert_eq!(w.members[0].spirit, 93);
    let (c, mut w) = setup(false);
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        },
        "lost",
        &c,
        0,
    )
    .unwrap();
    let p = w.members[0].pending.take().unwrap();
    w.npcs[0].hp = 0;
    let mut e = vec![];
    w.commit(0, p, &c, 60, &mut e);
    assert!(paid(&e).is_empty());
    assert_eq!(remaining(&w), Some(3));
    assert_eq!(w.members[0].spirit, 100);
    let (c, mut w) = setup(false);
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        },
        "cancel",
        &c,
        0,
    )
    .unwrap();
    w.action(0, &Action::CancelAction {}, "cancelled", &c, 30)
        .unwrap();
    assert!(w.members[0].pending.is_none());
    assert_eq!(remaining(&w), Some(3));
    assert_eq!(w.members[0].spirit, 100);
}
#[test]
fn paid_resource_action_selection_and_original_holder_ownership_stay_native() {
    let (mut c, mut w) = setup(false);
    c.combat_rules.as_mut().unwrap().triggers[0].action_filter = Some(
        serde_json::from_value(
            json!({"basic":false,"skills":{"mode":"only","ids":["different-skill"]}}),
        )
        .unwrap(),
    );
    let e = commit(&c, &mut w, 0);
    assert!(paid(&e).is_empty());
    assert_eq!(remaining(&w), Some(3));
    assert_eq!(w.members[0].spirit, 90);
    let (c, mut w) = setup(false);
    let id = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "charges")
        .unwrap()
        .instance_id
        .clone();
    let target = status_entity(&w.members[1].reference.entity);
    w.statuses.retarget(&[id], &target, &mut vec![]);
    let e = commit(&c, &mut w, 0);
    assert!(paid(&e).is_empty());
    assert_eq!(remaining(&w), Some(3));
    assert_eq!(w.members[0].spirit, 90);
}
#[test]
fn paid_resource_declared_eight_million_cost_returns_actual_two_point_four_million() {
    let (mut c, mut w) = setup(false);
    c.resource_limits = Some(
        serde_json::from_value(
            json!({"max_hp":1000000000,"max_spirit":1000000000,"max_spirit_cost":8000000}),
        )
        .unwrap(),
    );
    c.skill.cost = 8_000_000;
    w.members[0].facts.spirit_points = 8_000_000;
    w.members[0].spirit = 8_000_000;
    let e = commit(&c, &mut w, 0);
    assert_eq!(paid(&e), vec![(8_000_000, 2_400_000, 2_400_000, 0)]);
    assert_eq!(w.members[0].spirit, 2_400_000);
    assert_eq!(remaining(&w), Some(2));
}
#[test]
fn paid_resource_existing_seven_skill_carriers_share_one_initial_cost_owner() {
    let sample = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/skill-hp-cost-host/catalog.json"),
    )
    .unwrap();
    for s in sample.active("eight").unwrap().player_skills() {
        let (mut c, mut w) = setup(false);
        c.members.as_mut().unwrap().area_capacity = Some(8);
        c.skill = s.clone();
        c.skill.cost = 10;
        c.skill.hp_cost = None;
        c.skill.on_commit_self_statuses.clear();
        if c.skill.is_self() {
            c.skill.on_commit_self_statuses = vec!["charges".into()];
        }
        c.combat_rules.as_mut().unwrap().skills.clear();
        let action = if s.is_self() {
            Action::CastSelf {
                skill_id: s.id.clone(),
            }
        } else if s.is_unit() {
            Action::CastUnit {
                skill_id: s.id.clone(),
                target: World::unit_wire(&w.npcs[0].entity),
            }
        } else if s.is_ground() {
            Action::CastGround {
                skill_id: s.id.clone(),
                position: w.npcs[0].position,
            }
        } else {
            Action::Cast {
                skill_id: s.id.clone(),
                direction: [1., 0.],
            }
        };
        w.action(0, &action, "each-plan", &c, 0).unwrap();
        let p = w.members[0].pending.take().unwrap();
        let mut e = vec![];
        w.commit(0, p, &c, s.windup_ms, &mut e);
        assert_eq!(paid(&e), vec![(10, 3, 3, 0)], "{}", s.id);
        assert_eq!(w.members[0].spirit, 93);
        assert_eq!(remaining(&w), Some(2));
    }
}
#[test]
fn paid_resource_consumption_preview_releases_a_slot_only_for_remove_policy() {
    for remove in [false, true] {
        let (mut c, mut w) = super::conditional_activation_tests::setup(false);
        w.statuses = Default::default();
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(carrier(remove)).unwrap());
        c.pve_mut()
            .status_rules
            .as_mut()
            .unwrap()
            .max_active_per_entity = 1;
        c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"replace-charge","profiles":["member"],"when":"unit_skill_committed","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"conditions":{"status":{"subject":"owner","definition_id":"charges","at_least":3,"consume":3}},"effect":{"kind":"apply_status_bundle","owner_statuses":["source-power"],"other_statuses":["target-drained"]}}])).unwrap();
        let own = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), own, "charges", 0);
        let e = super::unit_effect_tests::start(&c, &mut w, false);
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "source-power"),
            remove
        );
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "target-drained"),
            remove
        );
        assert_eq!(remaining(&w), if remove { None } else { Some(3) });
        assert!(paid(&e).is_empty());
    }
}
#[test]
fn paid_resource_original_source_parameters_drive_the_three_charge_native_cycle() {
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/paid_resource_vectors.json"
    ))
    .unwrap();
    let (mut c, mut w) = setup(false);
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|s| s.id == "charges")
        .unwrap();
    let mut data = carrier(true);
    data["duration_ms"] = v["carrierMs"].clone();
    data["stack_counter"]["initial"] = v["initialCharges"].clone();
    *d = serde_json::from_value(data).unwrap();
    c.combat_rules.as_mut().unwrap().triggers[0].effect =
        serde_json::from_value(json!({"kind":"restore_paid_spirit","bps":v["paidSpiritBps"]}))
            .unwrap();
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|s| s.id == "exhausted")
        .unwrap()
        .duration_ms = v["exhaustionLockMs"].as_u64().unwrap();
    w.statuses = Default::default();
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "charges", 0);
    for n in 0..3 {
        let mut e = commit(&c, &mut w, n * 120);
        assert_eq!(paid(&e), vec![(10, 3, 3, 0)]);
        w.finish_status_ends(&c, n * 120 + 60, &mut e);
    }
    assert_eq!(remaining(&w), None);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "exhausted")
            .unwrap()
            .expires_at_ms,
        3300
    );
    assert!(!v["fullOriginalRealmTechniqueAccepted"].as_bool().unwrap());
}
#[test]
fn paid_resource_reserved_charge_release_and_auto_use_only_the_committed_quote() {
    for auto in [false, true] {
        let (mut c, mut w) = setup(false);
        let mut s = serde_json::to_value(&c.skill).unwrap();
        s["mechanism"] = json!("projectile");
        s["radius"] = json!(6);
        s["speed"] = json!(100);
        s["resource_commit"] = json!({"reserve_at":"accept","consume_at":"commit","cooldown_starts_at":"commit","refund_before_commit":{"kind":"full"}});
        s["charge"] = json!({"min_ms":60,"max_ms":180,"min_damage_bps":10000,"early_release":{"kind":"queue_until_min"},"release_windup_ms":30});
        c.skill = serde_json::from_value(s).unwrap();
        w.action(
            0,
            &Action::StartCharge {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.],
            },
            "charged-payment",
            &c,
            0,
        )
        .unwrap();
        let id = w.members[0].pending.as_ref().unwrap().id.clone();
        assert_eq!(w.members[0].spirit, 100);
        assert_eq!(remaining(&w), Some(3));
        if !auto {
            w.action(
                0,
                &Action::ReleaseCharge {
                    action_id: id,
                    direction: None,
                },
                "released",
                &c,
                30,
            )
            .unwrap();
        }
        let p = w.members[0].pending.take().unwrap();
        let at = p.commits;
        let mut e = vec![];
        w.commit(0, p, &c, at, &mut e);
        assert_eq!(paid(&e), vec![(10, 3, 3, 0)]);
        assert_eq!(remaining(&w), Some(2));
        assert_eq!(w.members[0].spirit, 93);
    }
}
#[test]
fn paid_resource_source_exit_discards_pending_exhaustion_and_cannot_restore_late() {
    let (c, mut w) = setup(false);
    for n in 0..3 {
        commit(&c, &mut w, n * 120);
    }
    assert_eq!(w.members[0].spirit, 79);
    let mut e = vec![];
    w.terminal_intent(0, Outcome::Death, &mut e);
    w.finish_status_ends(&c, 300, &mut e);
    assert!(paid(&e).is_empty());
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "exhausted" || s.definition_id == "charges")
    );
    let prior = w.members[0].spirit;
    w.combat_fact(
        crate::config::CombatFact::SkillCostCommitted,
        (Owner::Member(0), Owner::Member(0)),
        (10, false),
        &c,
        330,
        &mut e,
    );
    assert_eq!(w.members[0].spirit, prior);
    assert!(paid(&e).is_empty());
}
