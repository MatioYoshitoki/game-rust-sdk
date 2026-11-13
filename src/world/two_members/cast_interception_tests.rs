use super::periodic_tests::bind;
use super::*;
use serde_json::{Value, json};
fn defs() -> Vec<Value> {
    vec![
        json!({"id":"cast-mark","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":0,"harmful":true,"duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"cast-seal","mechanism":"timed_control","attribute":"silence","duration_ms":700,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"cast-delay","mechanism":"timed_attribute_modifier","attribute":"movement_slow","modifier_bps":-2500,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"full-immunity","mechanism":"timed_control","attribute":"control_immunity","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"first-immunity","mechanism":"timed_control","attribute":"first_control_immunity","duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
    ]
}
fn harmful() -> Value {
    json!({"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]})
}
fn rules(skill: &str) -> Value {
    json!({"cast-mark":{"action_filter":{"basic":false,"skills":{"mode":"only","ids":[skill]}},"default":{"kind":"interrupt","status_id":"cast-seal"}}})
}
fn setup() -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(false, false);
    w.statuses = Default::default();
    c.skill.hp_cost =
        serde_json::from_value(json!({"mode":"fixed","value":5,"min_hp_after_cost":1})).unwrap();
    c.skill.resource_commit=serde_json::from_value(json!({"reserve_at":"accept","consume_at":"commit","cooldown_starts_at":"commit","refund_before_commit":{"kind":"full"}})).unwrap();
    c.pve_mut().status_definitions.extend(
        defs()
            .into_iter()
            .map(|v| serde_json::from_value(v).unwrap()),
    );
    let r = c.combat_rules.as_mut().unwrap();
    r.cast_interceptions = serde_json::from_value(rules(&c.skill.id)).unwrap();
    for id in ["cast-mark", "cast-seal", "cast-delay"] {
        r.status_policies
            .insert(id.into(), serde_json::from_value(harmful()).unwrap());
    }
    (c, w)
}
fn mark(w: &mut World, c: &Config, source: Owner, target: Owner, now: u64) {
    let e = w.owner_entity(target);
    bind(w, c, source, e, "cast-mark", now);
}
fn count(w: &World, id: &str) -> usize {
    w.statuses
        .entries()
        .iter()
        .filter(|s| s.definition_id == id)
        .count()
}
fn action(c: &Config, w: &World) -> Action {
    Action::CastUnit {
        skill_id: c.skill.id.clone(),
        target: World::unit_wire(&w.npcs[0].entity),
    }
}
fn outcomes(e: &Events) -> Vec<CastInterceptionOutcome> {
    e.iter()
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event: Event::CastInterceptionResolved { outcome, .. },
            } => Some(*outcome),
            _ => None,
        })
        .collect()
}
fn commit(c: &Config, w: &mut World, now: u64) -> Events {
    let id = if now <= 60 {
        "protected-skill".to_owned()
    } else {
        format!("protected-skill-{now}")
    };
    w.action(0, &action(c, w), &id, c, now).unwrap();
    let p = w.members[0].pending.take().unwrap();
    let mut e = vec![];
    w.commit(0, p, c, now + 60, &mut e);
    e
}
#[test]
fn cast_interception_final_member_commit_has_no_fees_or_primary_effect_and_real_silence() {
    let (c, mut w) = setup();
    mark(&mut w, &c, Owner::Npc(1), Owner::Member(0), 0);
    let e = commit(&c, &mut w, 0);
    assert_eq!(outcomes(&e), vec![CastInterceptionOutcome::Interrupted]);
    assert_eq!(
        (w.members[0].hp, w.members[0].spirit, w.npcs[0].hp),
        (50, 50, 1000)
    );
    assert!(w.members[0].cooldowns.is_empty());
    assert!(w.shots.is_empty() && w.areas.is_empty() && w.members[0].channel.is_none());
    assert_eq!(count(&w, "cast-mark"), 0);
    let s = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "cast-seal")
        .unwrap();
    assert_eq!(s.source, status_entity(&w.npcs[1].entity));
    assert_eq!((s.applied_at_ms, s.expires_at_ms), (60, 760));
    assert!(w.members[0].spirit_recovery_gap(&w.statuses, 60, 100) > 0);
    assert!(w.action(0, &action(&c, &w), "silenced", &c, 759).is_err());
    let e = commit(&c, &mut w, 760);
    assert!(outcomes(&e).is_empty());
    assert_eq!((w.members[0].hp, w.members[0].spirit), (45, 48));
    assert!(w.npcs[0].hp < 1000);
}
#[test]
fn cast_interception_permanent_and_first_control_immunity_consume_lock_and_allow() {
    for mode in 0..3 {
        let (c, mut w) = setup();
        let own = w.members[0].reference.entity.clone();
        if mode != 0 {
            bind(
                &mut w,
                &c,
                Owner::Member(0),
                own.clone(),
                "full-immunity",
                0,
            );
        }
        if mode != 1 {
            bind(&mut w, &c, Owner::Member(0), own, "first-immunity", 0);
        }
        let full = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "full-immunity")
            .cloned();
        mark(&mut w, &c, Owner::Npc(1), Owner::Member(0), 0);
        let e = commit(&c, &mut w, 0);
        assert_eq!(outcomes(&e), vec![CastInterceptionOutcome::ImmuneAllowed]);
        assert_eq!((w.members[0].hp, w.members[0].spirit), (45, 48));
        assert_eq!(
            (
                count(&w, "cast-mark"),
                count(&w, "cast-seal"),
                count(&w, "first-immunity")
            ),
            (0, 0, usize::from(mode == 2))
        );
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .find(|s| s.definition_id == "full-immunity")
                .cloned(),
            full
        );
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::CastInterceptionResolved {
                    followup_status_id: None,
                    ..
                }
            }
        )));
    }
}
#[test]
fn cast_interception_unfunded_or_lost_target_cannot_spend_marks() {
    for mode in 0..2 {
        let (c, mut w) = setup();
        mark(&mut w, &c, Owner::Npc(1), Owner::Member(0), 0);
        w.action(0, &action(&c, &w), "candidate", &c, 0).unwrap();
        let p = w.members[0].pending.take().unwrap();
        if mode == 0 {
            w.members[0].spirit = 1;
        } else {
            w.npcs[0].entity.generation += 1;
        }
        let mut e = vec![];
        w.commit(0, p, &c, 60, &mut e);
        assert!(outcomes(&e).is_empty());
        assert_eq!(count(&w, "cast-mark"), 1);
        assert_eq!(count(&w, "cast-seal"), 0);
        assert_eq!(w.members[0].hp, 50);
        assert!(w.members[0].cooldowns.is_empty());
    }
}
#[test]
fn cast_interception_cancel_excluded_skill_expired_and_dead_source_do_not_consume() {
    for mode in 0..4 {
        let (mut c, mut w) = setup();
        mark(&mut w, &c, Owner::Npc(1), Owner::Member(0), 0);
        if mode == 0 {
            w.action(0, &action(&c, &w), "uncommitted", &c, 0).unwrap();
            let e = w
                .action(0, &Action::CancelAction {}, "cancel", &c, 30)
                .unwrap();
            assert!(outcomes(&e).is_empty());
            assert_eq!(w.members[0].spirit, 50);
            assert_eq!(count(&w, "cast-mark"), 1);
            continue;
        }
        if mode == 1 {
            c.combat_rules.as_mut().unwrap().cast_interceptions =
                serde_json::from_value(rules("excluded")).unwrap();
        } else if mode == 2 {
            w.npcs[1].hp = 0;
        }
        let e = commit(&c, &mut w, if mode == 3 { 5000 } else { 0 });
        assert!(outcomes(&e).is_empty());
        assert_eq!(w.members[0].spirit, 48);
        assert_eq!(count(&w, "cast-seal"), 0);
        if mode != 3 {
            assert_eq!(count(&w, "cast-mark"), 1);
        }
    }
}
#[test]
fn cast_interception_uses_only_earliest_live_source_mark_and_private_result() {
    let (c, mut w) = setup();
    mark(&mut w, &c, Owner::Npc(1), Owner::Member(0), 0);
    mark(&mut w, &c, Owner::Npc(0), Owner::Member(0), 30);
    let original = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "cast-mark" && s.source.id == w.npcs[1].entity.id)
        .unwrap()
        .instance_id
        .clone();
    let e = commit(&c, &mut w, 60);
    assert_eq!(outcomes(&e), vec![CastInterceptionOutcome::Interrupted]);
    assert_eq!(count(&w, "cast-mark"), 1);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "cast-mark" && s.source.id == w.npcs[0].entity.id)
    );
    assert!(e.iter().any(|(recipient,e)|*recipient==Some(0)&&matches!(e,EventV4::Private{event:Event::CastInterceptionResolved{mark_instance_id,source,target,action_id:Some(action_id),..}}if mark_instance_id==&original && source==&w.npcs[1].entity && target==&w.members[0].reference.entity && action_id=="protected-skill")));
}
#[test]
fn cast_interception_previews_policy_and_capacity_after_consuming_its_own_mark() {
    for blocked_by_other in [false, true] {
        let (mut c, mut w) = setup();
        c.pve_mut()
            .status_rules
            .as_mut()
            .unwrap()
            .max_active_per_entity = if blocked_by_other { 2 } else { 1 };
        c.pve_mut().status_definitions.push(
            serde_json::from_value(json!({"id":"gate-policy-ward","mechanism":"timed_attribute_modifier",
                "attribute":"def","modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh",
                "source_scoped":true})).unwrap(),
        );
        mark(&mut w, &c, Owner::Npc(1), Owner::Member(0), 0);
        let policy = c.combat_rules.as_mut().unwrap();
        if blocked_by_other {
            policy.status_policies.insert("gate-policy-ward".into(), serde_json::from_value(
                json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":["harmful"]})
            ).unwrap());
            let holder = w.members[0].reference.entity.clone();
            bind(&mut w, &c, Owner::Member(0), holder, "gate-policy-ward", 0);
        } else {
            policy.status_policies.get_mut("cast-mark").unwrap().blocks =
                vec![crate::config::StatusDisposition::Harmful];
        }
        let first = commit(&c, &mut w, 0);
        if blocked_by_other {
            assert!(outcomes(&first).is_empty());
            assert_eq!((count(&w, "cast-mark"), count(&w, "cast-seal")), (1, 0));
            assert_eq!((w.members[0].hp, w.members[0].spirit), (45, 48));
            w.prepare_periodic(1000, &mut vec![]);
            assert_eq!(
                (count(&w, "cast-mark"), count(&w, "gate-policy-ward")),
                (1, 0)
            );
            let second = commit(&c, &mut w, 1000);
            assert_eq!(
                outcomes(&second),
                vec![CastInterceptionOutcome::Interrupted],
                "{second:?}; statuses: {:?}",
                w.statuses.entries()
            );
            assert_eq!((w.members[0].hp, w.members[0].spirit), (45, 48));
        } else {
            assert_eq!(outcomes(&first), vec![CastInterceptionOutcome::Interrupted]);
            assert_eq!((w.members[0].hp, w.members[0].spirit), (50, 50));
        }
        assert_eq!((count(&w, "cast-mark"), count(&w, "cast-seal")), (0, 1));
    }
}

#[test]
fn cast_interception_uses_native_shortening_and_source_or_holder_exit_clears_mark() {
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "id":"shorten","mechanism":"timed_compound_modifier","duration_ms":2000,
            "duplicate_policy":"refresh","source_scoped":true,"modifiers":[
                {"attribute":"harmful_duration_reduction","modifier_bps":5000},
                {"attribute":"control_duration_reduction","modifier_bps":4000}
            ]
        }))
        .unwrap(),
    );
    let holder = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), holder, "shorten", 0);
    mark(&mut w, &c, Owner::Npc(1), Owner::Member(0), 0);
    assert_eq!(
        outcomes(&commit(&c, &mut w, 0)),
        vec![CastInterceptionOutcome::Interrupted]
    );
    let seal = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "cast-seal")
        .unwrap();
    assert_eq!((seal.applied_at_ms, seal.expires_at_ms), (60, 270));
    mark(&mut w, &c, Owner::Npc(1), Owner::Member(0), 90);
    mark(&mut w, &c, Owner::Member(0), Owner::Npc(0), 90);
    assert_eq!(count(&w, "cast-mark"), 2);
    let exit = w.action(0, &Action::Leave {}, "leave", &c, 120).unwrap();
    assert_eq!((count(&w, "cast-mark"), count(&w, "cast-seal")), (0, 0));
    assert!(outcomes(&exit).is_empty());
}

#[test]
fn cast_interception_source_recipe_and_client_ingress_stay_separate() {
    use crate::asset_contract::tests::Package;
    let package = Package::new();
    let fixture: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/cast_interception_vectors.json"
    ))
    .unwrap();
    let config: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/interception-a.json"
    ))
    .unwrap();
    let definition = |id: &str| {
        config["pve"]["status_definitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == id)
            .unwrap()
    };
    assert_eq!(definition("cast-mark")["duration_ms"], fixture["markMs"]);
    assert_eq!(definition("cast-seal")["duration_ms"], fixture["sealMs"]);
    assert_eq!(
        definition("cast-seal")["attribute"],
        fixture["sealAttribute"]
    );
    assert_eq!(
        definition("cast-delay")["duration_ms"],
        fixture["bossSlowMs"]
    );
    assert_eq!(
        definition("cast-delay")["modifier_bps"],
        fixture["bossSlowBps"]
    );
    assert_eq!(
        fixture["immunity"],
        json!({"consumeMark":true,"allowCommit":true,"applyFollowup":false})
    );
    assert_eq!(fixture["fullOriginalRealmTechniqueAccepted"], false);
    package.write(&config);
    package.load().unwrap();
    let mut no_mark_rule = config.clone();
    no_mark_rule["combat_rules"]
        .as_object_mut()
        .unwrap()
        .remove("cast_interceptions");
    package.write(&no_mark_rule);
    assert!(
        package.load().is_err(),
        "ordinary numeric NPC hit binding remains forbidden without the explicit mark"
    );
    let mut recursive = config.clone();
    recursive["combat_rules"]["control_policies"]["member"] =
        json!({"replacements":{"cast-seal":"cast-mark"}});
    package.write(&recursive);
    assert!(package.load().is_err());
    assert!(
        serde_json::from_value::<Action>(json!({
            "kind":"cast_unit","skill_id":"ordinary","target":{"id":"mark-a","generation":1},
            "cast_interception":{"source":{"id":"P01","generation":1}}
        }))
        .is_err()
    );
}

#[test]
fn cast_interception_npc_real_projectile_commit_respects_ordinary_and_boss_profiles() {
    for boss in [false, true] {
        let (mut c, mut w) = setup();
        w.npcs.truncate(1);
        let s = {
            let p = Arc::make_mut(&mut w.npcs[0].profile);
            p.combat.aggro_range = 1000.;
            p.combat.cast_chance = 1.;
            p.combat.projectile.cost = 4;
            p.combat.projectile.windup_ms = 60;
            p.combat.projectile.cooldown_ms = 5000;
            p.combat.projectile.clone()
        };
        w.npcs[0].spirit = 50;
        let r = c.combat_rules.as_mut().unwrap();
        let mut config = rules(&s.id);
        let npc_profile = r.default_npc_profile.clone();
        if boss {
            config["cast-mark"]["profile_overrides"][npc_profile] =
                json!({"kind":"allow_and_apply","status_id":"cast-delay"});
        }
        r.cast_interceptions = serde_json::from_value(config).unwrap();
        mark(&mut w, &c, Owner::Member(0), Owner::Npc(0), 0);
        let mut e = vec![];
        w.advance_npcs(&c, 0, 0.03, &mut e);
        assert!(w.npcs[0].casting.is_some());
        assert_eq!(w.npcs[0].spirit, 50);
        w.advance_npcs(&c, 60, 0.03, &mut e);
        assert_eq!(count(&w, "cast-mark"), 0);
        assert_eq!(w.npcs[0].spirit, if boss { 46 } else { 50 });
        assert_eq!(w.npcs[0].next_cast, if boss { 5060 } else { 0 });
        assert_eq!(w.shots.len(), usize::from(boss));
        assert_eq!(
            outcomes(&e),
            vec![if boss {
                CastInterceptionOutcome::AllowedWithFollowup
            } else {
                CastInterceptionOutcome::Interrupted
            }]
        );
        if boss {
            assert_eq!(w.npc_movement_factor(0, &c, 60), 0.75);
            assert_eq!(w.npc_movement_factor(0, &c, 1260), 1.);
            let s = w
                .statuses
                .entries()
                .iter()
                .find(|s| s.definition_id == "cast-delay")
                .unwrap();
            assert_eq!(s.expires_at_ms, 1260);
            assert_eq!(s.source, status_entity(&w.members[0].reference.entity));
        } else {
            assert!(w.entity_has_status(Owner::Npc(0), StatusAttribute::Silence, 60));
        }
    }
}
#[test]
fn cast_interception_cold_marks_references_and_policies_are_bounded() {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut x: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/activation-a.json"
    ))
    .unwrap();
    let n = x["pve"]["status_definitions"].as_array().unwrap().len();
    x["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .extend(defs());
    for id in ["cast-mark", "cast-seal", "cast-delay"] {
        x["combat_rules"]["status_policies"][id] = harmful();
    }
    x["combat_rules"]["cast_interceptions"] = rules("drain");
    p.write(&x);
    p.load().unwrap();
    for ncase in 0..13 {
        let mut b = x.clone();
        match ncase {
            0 => b["combat_rules"]["cast_interceptions"] = Value::Null,
            1 => {
                b["combat_rules"]["cast_interceptions"]["cast-mark"]["action_filter"]["basic"] =
                    json!(true)
            }
            2 => {
                b["combat_rules"]["cast_interceptions"]["cast-mark"]["action_filter"]["skills"]["ids"] =
                    json!(["unknown"])
            }
            3 => {
                b["combat_rules"]["cast_interceptions"]["cast-mark"]["default"]["status_id"] =
                    json!("cast-mark")
            }
            4 => {
                b["combat_rules"]["cast_interceptions"]["cast-mark"]["default"]["status_id"] =
                    json!("full-immunity")
            }
            5 => {
                b["combat_rules"]["cast_interceptions"]["cast-mark"]["default"] =
                    json!({"kind":"allow_and_apply","status_id":"cast-seal"})
            }
            6 => {
                b["combat_rules"]["cast_interceptions"]["cast-mark"]["profile_overrides"] =
                    json!({"unknown":{"kind":"allow_and_apply","status_id":"cast-delay"}})
            }
            7 => b["pve"]["status_definitions"][n]["modifier_bps"] = json!(10),
            8 => b["pve"]["status_definitions"][n]["source_scoped"] = json!(false),
            9 => b["pve"]["status_definitions"][n]["harmful"] = json!(false),
            10 => {
                b["combat_rules"]["cast_interceptions"]["cast-mark"]["default"]["extra"] = json!(1)
            }
            11 => {
                b["combat_rules"]["status_ends"]["cast-mark"] =
                    json!({"on":["consumed"],"target_statuses":["cast-delay"]})
            }
            _ => b["mechanism_version"] = json!(104),
        };
        p.write(&b);
        assert!(p.load().is_err(), "case {ncase}");
    }
}
