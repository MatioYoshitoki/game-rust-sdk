use super::*;
use crate::config::CombatFact;

fn rule(effect: Value) -> Value {
    json!({"id":"ally-transaction","profiles":["member"],"when":"ally_skill_committed","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"effect":effect})
}
fn prepare_kind(kind: &str) -> (Config, World) {
    let (mut c, _) = setup();
    let (_, mut w) = super::super::combat_tests::setup(3);
    c.additional_skills.clear();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"token","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let mut s = json!({"mechanism":"ally_heal","id":"aid","cost":3,"windup_ms":30,"recovery_ms":0,"cooldown_ms":0,"range":150});
    match kind {
        "hot" => s["hot_status"] = json!("renewal"),
        "shield" => {
            s["mechanism"] = json!("ally_shield");
            s["shield_status"] = json!("ward");
        }
        _ => s["heal"] = json!({"amount":7}),
    }
    c.skill = serde_json::from_value(s).unwrap();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(&mut c, rule(json!({"kind":"restore_spirit","amount":5})));
    for (i, m) in w.members.iter_mut().enumerate() {
        m.position = [400., 400. + 40. * i as f64];
        m.spirit = 100;
        m.hp = m.facts.hp_points;
    }
    (c, w)
}
fn prepare(w: &mut World, c: &Config, target: usize, now: u64) {
    let target = World::unit_wire(&w.members[target].reference.entity);
    w.action(
        0,
        &Action::CastUnit {
            skill_id: "aid".into(),
            target,
        },
        &format!("aid-{now}"),
        c,
        now,
    )
    .unwrap();
}
fn finish(w: &mut World, c: &Config, now: u64) -> Events {
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, c, now, &mut e);
    e
}
fn cast(w: &mut World, c: &Config, target: usize, now: u64) -> Events {
    prepare(w, c, target, now);
    finish(w, c, now + 30)
}
fn fired(e: &Events) -> usize {
    e.iter().filter(|(_,e)| matches!(e,EventV4::Private{event:Event::CombatTriggerFired{trigger_id,..}} if trigger_id=="ally-transaction")).count()
}
fn control(c: &mut Config, w: &mut World, attr: &str) -> StatusView {
    let d = if attr == "compound" {
        json!({"id":"gift","mechanism":"timed_compound_modifier","flags":["silence","root"],"modifiers":[{"attribute":"def","modifier_bps":-2000}],"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})
    } else {
        json!({"id":"gift","mechanism":"timed_control","attribute":attr,"duration_ms":900,"duplicate_policy":"refresh","source_scoped":true})
    };
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
    operation_policy(c, "gift", "harmful", true);
    let target = w.members[1].reference.entity.clone();
    bind(w, c, Owner::Npc(0), target, "gift", 0);
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "gift")
        .unwrap()
        .clone()
}
fn transfer_rule(c: &mut Config) {
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        c,
        rule(
            json!({"kind":"transfer_statuses","from":"other","disposition":"harmful","max_count":2}),
        ),
    );
}
#[test]
fn ally_commit_loader_accepts_scopes_counting_and_rejects_unsupported_fact_policies() {
    let p = Package::new();
    for scope in ["owner", "target"] {
        for counting in ["contact", "action"] {
            let mut v = document();
            let mut r = rule(json!({"kind":"restore_spirit","amount":5}));
            r["counter_scope"] = json!(scope);
            r["counting"] = json!(counting);
            v["combat_rules"]["triggers"] = json!([r.clone()]);
            p.write(&v);
            let loaded = p.load().unwrap();
            assert_eq!(
                serde_json::to_value(
                    &loaded
                        .default_config()
                        .combat_rules
                        .as_ref()
                        .unwrap()
                        .triggers[0]
                )
                .unwrap()["when"],
                json!("ally_skill_committed")
            );
            for bad in 0..6 {
                let mut x = v.clone();
                match bad {
                    0 => x["combat_rules"]["triggers"][0]["include_periodic"] = json!(true),
                    1 => x["combat_rules"]["triggers"][0]["counting"] = json!("action_total"),
                    2 => {
                        x["combat_rules"]["triggers"][0]["effect"] =
                            json!({"kind":"heal_from_damage","bps":1000})
                    }
                    3 => {
                        x["combat_rules"]["triggers"][0]["effect"] =
                            json!({"kind":"reflect_damage","bps":1000,"damage_kind":"physical"})
                    }
                    4 => x["combat_rules"]["triggers"][0]["when"] = json!("ally_skill_tick"),
                    _ => x["mechanism_version"] = json!(104),
                }
                p.write(&x);
                assert!(p.load().is_err(), "{scope}/{counting}/{bad}");
            }
        }
    }
}
#[test]
fn ally_commit_all_three_primary_effects_run_after_cost_and_committed_fact_once() {
    for kind in ["instant", "hot", "shield"] {
        let (mut c, mut w) = prepare_kind(kind);
        w.members[1].hp -= 20;
        let mut old = rule(json!({"kind":"restore_spirit","amount":2}));
        old["id"] = json!("old-self-commit");
        old["when"] = json!("skill_committed");
        trigger(&mut c, old);
        let e = cast(&mut w, &c, 1, 0);
        assert_eq!(fired(&e), 1);
        assert_eq!(w.members[0].spirit, 104);
        assert_eq!(w.members[1].spirit, 100);
        let committed = e
            .iter()
            .position(|(_, e)| {
                matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionCommitted { .. }
                    }
                )
            })
            .unwrap();
        let reward=e.iter().position(|(_,e)|matches!(e,EventV4::Private{event:Event::CombatTriggerFired{trigger_id,..}} if trigger_id=="ally-transaction")).unwrap();
        assert!(committed < reward);
        let primary = e
            .iter()
            .position(|(_, e)| {
                matches!(
                    e,
                    EventV4::Private {
                        event: Event::HealResolved { .. }
                            | Event::StatusApplied { .. }
                            | Event::ShieldGranted { .. }
                    }
                )
            })
            .unwrap();
        assert!(primary < reward);
    }
}
#[test]
fn ally_commit_full_hp_zero_actual_heal_still_commits_and_rewards() {
    let (c, mut w) = prepare_kind("instant");
    let hp = w.members[1].hp;
    let e = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&e), 1);
    assert_eq!(w.members[1].hp, hp);
    assert_eq!(w.members[0].spirit, 102);
}
#[test]
fn ally_commit_hot_ticks_and_refresh_do_not_emit_extra_commit_rewards() {
    let (c, mut w) = prepare_kind("hot");
    let first = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&first), 1);
    let id = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "renewal")
        .unwrap()
        .instance_id
        .clone();
    let second = cast(&mut w, &c, 1, 60);
    assert_eq!(fired(&second), 1);
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "renewal")
            .unwrap()
            .instance_id,
        id
    );
    let mut e = vec![];
    w.advance_periodic(&c, 1200, &mut e);
    assert_eq!(fired(&e), 0);
    assert_eq!(w.members[0].spirit, 104);
}
#[test]
fn ally_commit_basic_self_and_hostile_casts_do_not_emit_support_fact() {
    for kind in ["basic", "self", "hostile"] {
        let (mut c, mut w) = prepare_kind("instant");
        let action = match kind {
            "basic" => Action::Attack {},
            "self" => {
                c.skill=serde_json::from_value(json!({"id":"self","mechanism":"self_status","cost":3,"windup_ms":30,"recovery_ms":0,"cooldown_ms":0,"on_commit_self_statuses":["token"]})).unwrap();
                Action::CastSelf {
                    skill_id: "self".into(),
                }
            }
            _ => {
                c.skill = serde_json::from_value(skill("projectile")).unwrap();
                Action::Cast {
                    skill_id: c.skill.id.clone(),
                    direction: [1., 0.],
                }
            }
        };
        w.action(0, &action, "non-support", &c, 0).unwrap();
        let e = finish(&mut w, &c, 120);
        assert_eq!(fired(&e), 0);
    }
}
#[test]
fn ally_commit_rechecks_target_lifecycle_generation_and_source_target_range_without_spending() {
    for invalid in 0..5 {
        let (c, mut w) = prepare_kind("instant");
        prepare(&mut w, &c, 1, 0);
        match invalid {
            0 => w.members[1].ready = false,
            1 => w.members[1].hp = 0,
            2 => w.members[1].reference.entity.generation += 1,
            3 => w.members[1].position = [900., 900.],
            _ => {
                w.members[0].position = [900., 400.];
            }
        }
        let e = finish(&mut w, &c, 30);
        assert_eq!(fired(&e), 0);
        assert_eq!(w.members[0].spirit, 100);
        assert!(!w.members[0].cooldowns.contains_key("aid"));
    }
}
#[test]
fn ally_commit_source_resource_failure_after_preparation_has_no_trigger_or_cooldown() {
    let (c, mut w) = prepare_kind("instant");
    prepare(&mut w, &c, 1, 0);
    w.members[0].spirit = 2;
    let e = finish(&mut w, &c, 30);
    assert_eq!(fired(&e), 0);
    assert_eq!(w.members[0].spirit, 2);
    assert!(!w.members[0].cooldowns.contains_key("aid"));
}
#[test]
fn ally_commit_primary_status_capacity_failure_keeps_cost_and_counter_untouched() {
    let (mut c, mut w) = prepare_kind("shield");
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), target, "token", 0);
    let e = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&e), 0);
    assert_eq!(w.members[0].spirit, 100);
    assert!(!w.members[0].cooldowns.contains_key("aid"));
}
#[test]
fn ally_commit_owner_and_target_counts_are_separate_with_independent_cooldowns() {
    for target_scoped in [false, true] {
        let (mut c, mut w) = prepare_kind("instant");
        let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
        t.every = 2;
        t.cooldown_ms = 1000;
        t.counter_scope = if target_scoped {
            crate::config::TriggerScope::Target
        } else {
            crate::config::TriggerScope::Owner
        };
        let counts = [1, 2, 1, 2].map(|j| j);
        let mut fires = vec![];
        for (i, j) in counts.into_iter().enumerate() {
            fires.push(fired(&cast(&mut w, &c, j, i as u64 * 60)));
        }
        assert_eq!(
            fires,
            if target_scoped {
                vec![0, 0, 1, 1]
            } else {
                vec![0, 1, 0, 0]
            }
        );
    }
}
#[test]
fn ally_commit_action_counting_uses_original_action_identity_and_deduplicates() {
    let (mut c, mut w) = prepare_kind("instant");
    c.combat_rules.as_mut().unwrap().triggers[0].counting = crate::config::TriggerCounting::Action;
    w.triggers.action = Some("same-support".into());
    let mut e = vec![];
    for _ in 0..2 {
        w.combat_fact(
            CombatFact::AllySkillCommitted,
            (Owner::Member(0), Owner::Member(1)),
            (0, false),
            &c,
            30,
            &mut e,
        );
    }
    assert_eq!(fired(&e), 1);
    assert_eq!(w.members[0].spirit, 105);
}
#[test]
fn ally_commit_zero_fact_amount_and_probability_refusal_preserve_effect_budget() {
    for zero_chance in [false, true] {
        let (mut c, mut w) = prepare_kind("instant");
        let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
        if zero_chance {
            t.chance_bps = Some(0)
        } else {
            t.conditions.minimum_amount = 1
        };
        let e = cast(&mut w, &c, 1, 0);
        assert_eq!(fired(&e), 0);
        assert_eq!(w.members[0].spirit, 97);
    }
}
#[test]
fn ally_commit_transfers_npc_compound_control_after_heal_with_original_deadline_and_privacy() {
    let (mut c, mut w) = prepare_kind("instant");
    let old = control(&mut c, &mut w, "compound");
    transfer_rule(&mut c);
    w.members[1].hp -= 20;
    let e = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&e), 1);
    assert_eq!(w.members[1].hp, w.members[1].facts.hp_points - 13);
    let moved = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.instance_id == old.instance_id)
        .unwrap();
    let mut expected = old;
    expected.target = status_entity(&w.members[0].reference.entity);
    assert_eq!(
        serde_json::to_value(moved).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    let recipients = e
        .iter()
        .filter_map(|(i, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::StatusTransferred { .. }
                }
            )
            .then_some(*i)
        })
        .collect::<Vec<_>>();
    assert_eq!(recipients, vec![Some(0), Some(1)]);
    assert!(w.skill_silenced(0, 30));
    assert!(!w.skill_silenced(1, 30));
    assert!(
        w.action(
            0,
            &Action::CastUnit {
                skill_id: "aid".into(),
                target: World::unit_wire(&w.members[1].reference.entity)
            },
            "blocked",
            &c,
            60
        )
        .is_err()
    );
    assert_eq!(w.members[0].spirit, 97);
}
#[test]
fn ally_commit_transfer_refusal_keeps_condition_and_icd_but_retains_primary_cost_and_heal() {
    let (mut c, mut w) = prepare_kind("instant");
    control(&mut c, &mut w, "stun");
    transfer_rule(&mut c);
    let immunity = json!({"id":"immune","mechanism":"timed_control","attribute":"first_control_immunity","duration_ms":900,"duplicate_policy":"refresh","source_scoped":true});
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(immunity).unwrap());
    let source = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), source.clone(), "token", 0);
    bind(&mut w, &c, Owner::Member(0), source.clone(), "immune", 0);
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.cooldown_ms = 1000;
    t.conditions = serde_json::from_value(
        json!({"status":{"subject":"owner","definition_id":"token","at_least":1,"consume":1}}),
    )
    .unwrap();
    w.members[1].hp -= 30;
    let e = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&e), 0);
    assert_eq!(w.members[0].spirit, 97);
    assert_eq!(w.members[1].hp, w.members[1].facts.hp_points - 23);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "token")
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "immune")
    );
    let before = w.statuses.entries().to_vec();
    let ids = before
        .iter()
        .filter(|s| s.definition_id == "immune")
        .map(|s| s.instance_id.clone())
        .collect::<Vec<_>>();
    let mut raw = vec![];
    w.statuses.remove(
        StatusRemovalReason::Cleansed,
        |s| ids.contains(&s.instance_id),
        &mut raw,
    );
    let e = cast(&mut w, &c, 1, 60);
    assert_eq!(fired(&e), 1);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "token")
    );
    assert_eq!(w.members[0].spirit, 94);
}
#[test]
fn ally_commit_hostile_control_transfer_preserves_primary_hot_and_shared_lifetime() {
    let (mut c, mut w) = prepare_kind("hot");
    control(&mut c, &mut w, "silence");
    transfer_rule(&mut c);
    let e = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&e), 1);
    let h = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "renewal")
        .unwrap();
    assert_eq!(h.target, status_entity(&w.members[1].reference.entity));
    assert_eq!(h.source, status_entity(&w.members[0].reference.entity));
    let mut e = vec![];
    w.advance_periodic(&c, 900, &mut e);
    assert!(!w.skill_silenced(0, 900));
    assert_eq!(fired(&e), 0);
}
#[test]
fn ally_commit_shared_step_budget_applies_across_old_and_new_commit_facts() {
    let (mut c, mut w) = prepare_kind("instant");
    c.combat_rules.as_mut().unwrap().max_triggers_per_step = 1;
    let mut t = rule(json!({"kind":"restore_spirit","amount":2}));
    t["when"] = json!("skill_committed");
    t["id"] = json!("old");
    trigger(&mut c, t);
    let e = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&e), 0);
    assert_eq!(w.members[0].spirit, 99);
}
#[test]
fn ally_commit_other_disposition_can_cleanse_npc_control_without_retargeting_or_new_damage() {
    let (mut c, mut w) = prepare_kind("instant");
    control(&mut c, &mut w, "root");
    c.combat_rules.as_mut().unwrap().triggers.clear();
    trigger(
        &mut c,
        rule(
            json!({"kind":"dispel_statuses","subject":"other","disposition":"harmful","max_count":2}),
        ),
    );
    let e = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&e), 1);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "gift")
    );
    assert!(!e.iter().any(|(_, e)| matches!(e, EventV4::Damage { .. })));
}
#[test]
fn ally_commit_target_generation_replacement_does_not_inherit_target_progress() {
    let (mut c, mut w) = prepare_kind("instant");
    let t = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    t.every = 2;
    t.counter_scope = crate::config::TriggerScope::Target;
    assert_eq!(fired(&cast(&mut w, &c, 1, 0)), 0);
    w.members[1].reference.entity.generation += 1;
    assert_eq!(fired(&cast(&mut w, &c, 1, 60)), 0);
    assert_eq!(fired(&cast(&mut w, &c, 1, 120)), 1);
}
#[test]
fn ally_commit_profile_filter_does_not_allow_other_members_to_run_the_rule() {
    let (mut c, mut w) = prepare_kind("instant");
    let profile = c.combat_rules.as_ref().unwrap().profiles["member"].clone();
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .insert("other".into(), profile);
    c.combat_rules
        .as_mut()
        .unwrap()
        .member_profiles
        .insert("P00".into(), "other".into());
    let e = cast(&mut w, &c, 1, 0);
    assert_eq!(fired(&e), 0);
    assert_eq!(w.members[0].spirit, 97);
}
