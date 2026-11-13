use super::*;

fn definition(id: &str, flags: Value) -> Value {
    json!({"id":id,"mechanism":"timed_compound_modifier","duration_ms":600,"duplicate_policy":"refresh","source_scoped":true,"flags":flags,"modifiers":[{"attribute":"atk","modifier_bps":-2000}]})
}
fn add(c: &mut Config, id: &str, flags: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(definition(id, flags)).unwrap());
}
fn current(w: &World, id: &str) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == id)
        .unwrap()
        .clone()
}
fn target(w: &World) -> EntityRef {
    w.members[0].reference.entity.clone()
}
fn prepare(w: &mut World, c: &mut Config) {
    c.skill = serde_json::from_value(skill("projectile")).unwrap();
    c.additional_skills.clear();
    w.action(
        0,
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "preparing",
        c,
        0,
    )
    .unwrap();
}
#[test]
fn compound_flags_config_roundtrips_numeric_and_flag_only_groups() {
    let p = Package::new();
    for flags in [
        json!(["silence", "root", "stun"]),
        json!([
            "control_immunity",
            "damage_immunity",
            "first_control_immunity"
        ]),
    ] {
        for numeric in [false, true] {
            let mut v = document();
            let mut d = definition("group", flags.clone());
            if !numeric {
                d.as_object_mut().unwrap().remove("modifiers");
            }
            v["pve"]["status_definitions"]
                .as_array_mut()
                .unwrap()
                .push(d.clone());
            p.write(&v);
            let c = p.load().unwrap().default_config();
            let def = c.pve.as_ref().unwrap().status_definitions.last().unwrap();
            assert_eq!(serde_json::to_value(def).unwrap(), d);
        }
    }
}
#[test]
fn compound_flags_reject_invalid_presence_carriers_groups_and_lifetime_modes() {
    let p = Package::new();
    for case in 0..17 {
        let mut v = document();
        let mut d = definition("group", json!(["root"]));
        match case {
            0 => d["flags"] = Value::Null,
            1 => d["flags"] = json!([]),
            2 => d["flags"] = json!(["root", "root"]),
            3 => d["flags"] = json!(["root", "damage_immunity"]),
            4 => d["flags"] = json!(["atk"]),
            5 => d["flags"] = json!(["spirit_recovery_lock"]),
            6 => d["flags"] = json!(["unknown"]),
            7 => d["flags"] = json!(["silence", "root", "stun", "control_immunity"]),
            8 => d["source_scoped"] = json!(false),
            9 => d["duplicate_policy"] = json!("ignore"),
            10 => {
                d["duplicate_policy"] = json!("stack");
                d["max_stacks"] = json!(3);
            }
            11 => d["attribute"] = json!("atk"),
            12 => d["modifier_bps"] = json!(1),
            13 => {
                d["mechanism"] = json!("timed_attribute_modifier");
                d["attribute"] = json!("atk");
                d["modifier_bps"] = json!(1000);
                d.as_object_mut().unwrap().remove("modifiers");
            }
            14 => {
                d["mechanism"] = json!("periodic_heal");
                d["heal_bps"] = json!(500);
                d["tick_interval_ms"] = json!(100);
                d.as_object_mut().unwrap().remove("modifiers");
            }
            15 => d["modifiers"][0]["attribute"] = json!("silence"),
            _ => v["mechanism_version"] = json!(104),
        }
        v["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .push(d);
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn compound_flags_cannot_attach_a_window_or_hostile_self_binding_and_transfer_needs_dispel_permission()
 {
    let p = Package::new();
    for case in 0..4 {
        let mut v = document();
        v["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .push(definition("group", json!(["root"])));
        match case {
            0 => {
                v["combat_rules"]["status_policies"] = json!({"group":{"disposition":"harmful","dispellable":false,"transferable":true,"blocks":[]}})
            }
            1 => {
                v["combat_rules"]["status_windows"] = json!({"group":{"basis":"none","record_cap":0,"include_periodic":false,"refresh":"reset","maximum_sampling":"snapshot","on_expired":true,"on_consumed":false,"hp":{"fixed":1,"recorded_bps":0,"max_resource_bps":0,"cap_max_resource_bps":10000}}})
            }
            2 => v["skill"]["on_commit_self_statuses"] = json!(["group"]),
            _ => {
                v["combat_rules"]["death_guards"] =
                    json!({"member":{"retain_hp":1,"heal_max_hp_bps":0,"self_statuses":["group"]}})
            }
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn compound_flags_share_identity_source_slot_and_numeric_refresh() {
    let (mut c, mut w) = setup();
    add(&mut c, "group", json!(["root", "silence"]));
    let npc = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), npc.clone(), "group", 0);
    let first = current(&w, "group");
    assert_eq!(first.flags.len(), 2);
    assert_eq!(w.statuses.entries().len(), 1);
    bind(&mut w, &c, Owner::Member(0), npc.clone(), "group", 120);
    let second = current(&w, "group");
    assert_eq!(first.instance_id, second.instance_id);
    assert_eq!(second.expires_at_ms, 720);
    assert_eq!(second.modifiers, first.modifiers);
    bind(&mut w, &c, Owner::Member(1), npc, "group", 150);
    assert_eq!(w.statuses.entries().len(), 2);
    let source = w.members[0].reference.entity.clone();
    w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(
        current(&w, "group").source.id,
        w.members[1].reference.entity.id
    );
}
#[test]
fn compound_flags_capacity_refusal_does_not_apply_numeric_or_interrupt_preparation() {
    let (mut c, mut w) = setup();
    add(&mut c, "full", json!(["damage_immunity"]));
    add(&mut c, "group", json!(["root", "silence"]));
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let own = target(&w);
    bind(&mut w, &c, Owner::Member(0), own.clone(), "full", 0);
    prepare(&mut w, &mut c);
    let before = w.stats(0, &c, 30);
    let e = bind(&mut w, &c, Owner::Npc(0), own, "group", 30);
    assert!(w.members[0].pending.is_some());
    assert_eq!(w.stats(0, &c, 30), before);
    assert_eq!(w.statuses.entries().len(), 1);
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
fn compound_root_and_silence_interrupt_members_and_restore_both_gates_at_expiry() {
    let (mut c, mut w) = setup();
    add(&mut c, "group", json!(["root", "silence"]));
    w.action(
        0,
        &Action::Navigate {
            destination: [440., 400.],
        },
        "path",
        &c,
        0,
    )
    .unwrap();
    prepare(&mut w, &mut c);
    w.members[0].queued_cast = Some(QueuedCastView {
        action_id: "queued".into(),
        skill_id: c.skill.id.clone(),
        direction: [1., 0.],
        expires_at_ms: 600,
    });
    let before = w.stats(0, &c, 0).0;
    let spirit = w.members[0].spirit;
    let own = target(&w);
    bind(&mut w, &c, Owner::Npc(0), own, "group", 30);
    assert!(
        w.members[0].pending.is_none()
            && w.members[0].path.is_empty()
            && w.members[0].queued_cast.is_none()
    );
    assert_eq!(w.members[0].movement, [0., 0.]);
    assert_eq!(w.members[0].spirit, spirit);
    assert_eq!(w.stats(0, &c, 60).0, before * 8 / 10);
    for a in [
        Action::Move {
            direction: [1., 0.],
        },
        Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
    ] {
        assert_eq!(w.action(0, &a, "blocked", &c, 60), Err(Error::InvalidState));
    }
    w.prepare_periodic(630, &mut vec![]);
    assert_eq!(w.stats(0, &c, 630).0, before);
    assert!(
        w.action(
            0,
            &Action::Move {
                direction: [1., 0.]
            },
            "free",
            &c,
            630
        )
        .is_ok()
    );
    assert!(
        w.action(
            0,
            &Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.]
            },
            "free-cast",
            &c,
            630
        )
        .is_ok()
    );
}
#[test]
fn compound_stun_cancels_uncommitted_actions_but_keeps_committed_projectiles() {
    let (mut c, mut w) = setup();
    add(&mut c, "group", json!(["stun"]));
    c.skill = serde_json::from_value(skill("projectile")).unwrap();
    commit(&mut w, &c, "projectile");
    assert_eq!(w.shots.len(), 1);
    let shots = w.shots[0].view.id.clone();
    let spirit = w.members[0].spirit;
    let own = target(&w);
    bind(&mut w, &c, Owner::Npc(0), own, "group", 60);
    assert_eq!(w.shots[0].view.id, shots);
    assert_eq!(w.members[0].spirit, spirit);
    for a in [
        Action::Attack {},
        Action::Move {
            direction: [1., 0.],
        },
        Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
    ] {
        assert_eq!(w.action(0, &a, "blocked", &c, 90), Err(Error::InvalidState));
    }
}
#[test]
fn compound_control_cancels_npc_cast_and_stops_rooted_movement_or_stunned_ticks() {
    for flags in [json!(["silence", "root"]), json!(["stun"])] {
        let (mut c, mut w) = setup();
        add(&mut c, "group", flags.clone());
        w.npcs[0].casting = Some(super::npc_skills::NpcCast::legacy(150, [1., 0.]));
        let npc = w.npcs[0].entity.clone();
        bind(&mut w, &c, Owner::Member(0), npc, "group", 0);
        assert!(w.npcs[0].casting.is_none());
        if flags == json!(["silence", "root"]) {
            assert_eq!(w.npc_movement_factor(0, &c, 30), 0.);
        }
        let before = w.npcs[0].position;
        w.tick(&c, 30);
        assert_eq!(w.npcs[0].position, before);
        assert!(!w.entity_has_status(Owner::Npc(0), StatusAttribute::Stun, 600));
    }
}
#[test]
fn compound_control_immunity_rejects_whole_definition_without_numeric_or_interruption() {
    let (mut c, mut w) = setup();
    add(&mut c, "guard", json!(["control_immunity"]));
    add(&mut c, "group", json!(["silence", "root", "stun"]));
    let own = target(&w);
    bind(&mut w, &c, Owner::Member(0), own.clone(), "guard", 0);
    prepare(&mut w, &mut c);
    let atk = w.stats(0, &c, 30).0;
    let e = bind(&mut w, &c, Owner::Npc(0), own, "group", 30);
    assert!(w.members[0].pending.is_some());
    assert_eq!(w.stats(0, &c, 30).0, atk);
    assert_eq!(w.statuses.entries().len(), 1);
    assert!(e.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::StatusRejected{reason:Error::Immune,definition_ids,..}} if definition_ids==&vec!["group".to_owned()])));
}
#[test]
fn compound_first_control_consumes_entire_guard_and_numeric_once_for_all_flags() {
    let (mut c, mut w) = setup();
    add(&mut c, "guard", json!(["first_control_immunity"]));
    add(&mut c, "group", json!(["silence", "root", "stun"]));
    let own = target(&w);
    let base = w.stats(0, &c, 0).0;
    bind(&mut w, &c, Owner::Member(0), own.clone(), "guard", 0);
    let guard = current(&w, "guard");
    assert_eq!(w.stats(0, &c, 0).0, base * 8 / 10);
    let e = bind(&mut w, &c, Owner::Npc(0), own.clone(), "group", 30);
    assert!(w.statuses.entries().is_empty());
    assert_eq!(w.stats(0, &c, 30).0, base);
    assert_eq!(e.iter().filter(|(_,e)|matches!(e,EventV4::Private{event:Event::StatusRemoved{reason:StatusRemovalReason::Depleted,instance_ids}} if instance_ids==&vec![guard.instance_id.clone()])).count(),1);
    bind(&mut w, &c, Owner::Npc(0), own, "group", 60);
    assert_eq!(w.statuses.entries().len(), 1);
    assert!(w.member_has_status(0, StatusAttribute::Stun, 60));
}
#[test]
fn compound_normal_immunity_precedes_first_immunity_without_consumption() {
    let (mut c, mut w) = setup();
    add(&mut c, "first", json!(["first_control_immunity"]));
    add(&mut c, "guard", json!(["control_immunity"]));
    add(&mut c, "group", json!(["root"]));
    let own = target(&w);
    for id in ["first", "guard"] {
        bind(&mut w, &c, Owner::Member(0), own.clone(), id, 0);
    }
    let first = current(&w, "first");
    bind(&mut w, &c, Owner::Npc(0), own, "group", 30);
    assert_eq!(current(&w, "first"), first);
    assert_eq!(w.statuses.entries().len(), 2);
}
#[test]
fn compound_harmful_and_control_shortening_multiply_once_and_refresh_from_definition() {
    let (mut c, mut w) = setup();
    add(&mut c, "group", json!(["silence", "root", "stun"]));
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"resilience","mechanism":"timed_compound_modifier","duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"harmful_duration_reduction","modifier_bps":5000},{"attribute":"control_duration_reduction","modifier_bps":4000}]})).unwrap());
    let own = target(&w);
    bind(&mut w, &c, Owner::Member(0), own.clone(), "resilience", 0);
    bind(&mut w, &c, Owner::Npc(0), own.clone(), "group", 30);
    let first = current(&w, "group");
    assert_eq!(first.expires_at_ms, 210);
    bind(&mut w, &c, Owner::Npc(0), own, "group", 90);
    let refreshed = current(&w, "group");
    assert_eq!(first.instance_id, refreshed.instance_id);
    assert_eq!(refreshed.expires_at_ms, 270);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "resilience")
        .unwrap()
        .modifiers[0]
        .modifier_bps = 10000;
    let own = target(&w);
    bind(&mut w, &c, Owner::Member(0), own.clone(), "resilience", 120);
    bind(&mut w, &c, Owner::Npc(0), own, "group", 130);
    let shortest = current(&w, "group");
    assert_eq!(shortest.expires_at_ms - shortest.applied_at_ms, 1);
}
#[test]
fn compound_damage_immunity_covers_direct_periodic_and_derived_but_control_immunity_does_not() {
    for immune in [false, true] {
        for direct in [false, true] {
            for derived in [false, true] {
                let (mut c, mut w) = setup();
                add(
                    &mut c,
                    "guard",
                    json!([if immune {
                        "damage_immunity"
                    } else {
                        "control_immunity"
                    }]),
                );
                let own = target(&w);
                bind(&mut w, &c, Owner::Member(0), own, "guard", 0);
                w.triggers.executing = derived;
                let before = w.members[0].hp;
                let (damage, _) = record_hit(&mut w, &c, 100, 30);
                if direct {
                    w.triggers.executing = false;
                    let source = w.npcs[0].entity.clone();
                    let target = target(&w);
                    w.resolve_damage_with_rule(
                        DamageIntent {
                            source,
                            target,
                            raw: 100,
                            direct,
                            ignore_armor: true,
                        },
                        Some(&AttackRule {
                            damage_kind: crate::config::DamageKind::TrueDamage,
                            can_crit: false,
                            ..Default::default()
                        }),
                        &c,
                        60,
                        &mut vec![],
                    );
                }
                assert_eq!(damage.hp_damage, if immune { 0 } else { 100 });
                assert_eq!(
                    w.members[0].hp,
                    if immune {
                        before
                    } else {
                        before - 100 - u32::from(direct) * 100
                    }
                );
            }
        }
    }
}
#[test]
fn compound_flags_enforce_relations_and_source_generation_identity() {
    let (mut c, mut w) = setup();
    add(&mut c, "control", json!(["stun"]));
    add(&mut c, "guard", json!(["damage_immunity"]));
    let own = target(&w);
    assert!(bind(&mut w, &c, Owner::Member(1), own.clone(), "control", 0).is_empty());
    assert!(bind(&mut w, &c, Owner::Npc(0), own.clone(), "guard", 0).is_empty());
    bind(&mut w, &c, Owner::Member(1), own, "guard", 0);
    assert_eq!(
        current(&w, "guard").source.id,
        w.members[1].reference.entity.id
    );
    w.members[1].reference.entity.generation += 1;
    w.prepare_periodic(30, &mut vec![]);
    assert!(w.statuses.entries().is_empty());
}
#[test]
fn compound_cleanse_item_removes_all_flags_and_numeric_and_preserves_other_sources() {
    let (mut c, mut w) = setup();
    add(&mut c, "group", json!(["root", "silence"]));
    c.member_item_use = serde_json::from_value(
        json!({"tonic":{"cleanse_self_statuses":["group"],"cooldown_ms":300}}),
    )
    .unwrap();
    w.members[0].inventory.insert("tonic".into(), 1);
    w.members[0].looted.insert("tonic".into(), 1);
    let own = target(&w);
    let peer = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), own, "group", 0);
    bind(&mut w, &c, Owner::Npc(0), peer, "group", 0);
    let base = w.stats(0, &c, 600).0;
    let revision = w.members[0].revision;
    w.action(
        0,
        &Action::UseItem {
            item_id: "tonic".into(),
            expected_revision: revision,
        },
        "cleanse",
        &c,
        30,
    )
    .unwrap();
    assert_eq!(w.stats(0, &c, 30).0, base);
    assert!(!w.member_has_status(0, StatusAttribute::Root, 30));
    assert!(w.member_has_status(1, StatusAttribute::Root, 30));
}
#[test]
fn compound_cleanup_reasons_and_reconnect_preserve_or_remove_complete_instances() {
    for reason in [
        StatusRemovalReason::Cleansed,
        StatusRemovalReason::Consumed,
        StatusRemovalReason::TargetInvalid,
        StatusRemovalReason::SourceInvalid,
        StatusRemovalReason::Reset,
        StatusRemovalReason::Closed,
    ] {
        let (mut c, mut w) = setup();
        add(&mut c, "group", json!(["root", "silence"]));
        let own = target(&w);
        let base = w.stats(0, &c, 0).0;
        bind(&mut w, &c, Owner::Npc(0), own, "group", 0);
        let s = current(&w, "group");
        w.members[0].protected = true;
        w.members[0].ready = false;
        assert_eq!(current(&w, "group"), s);
        w.members[0].protected = false;
        w.members[0].ready = true;
        if reason == StatusRemovalReason::Consumed {
            assert!(w.statuses.consume(&s.instance_id, 1, &mut vec![]));
        } else {
            w.statuses
                .remove(reason, |r| r.instance_id == s.instance_id, &mut vec![]);
        }
        assert!(w.statuses.entries().is_empty());
        assert_eq!(w.stats(0, &c, 30).0, base);
    }
}

#[test]
fn compound_flags_load_npc_contact_self_and_item_bindings_without_silent_noops() {
    let p = Package::new();
    let mut v = document();
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(definition("group", json!(["root", "silence"])));
    v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!(["group"]);
    p.write(&v);
    assert!(p.load().is_ok());
    v["member_item_use"] = json!({"xuan-tie":{"cleanse_self_statuses":["group"]}});
    p.write(&v);
    assert!(p.load().is_ok());
    v["member_item_use"] = json!({"xuan-tie":{"on_use_self_statuses":["group"]}});
    p.write(&v);
    assert!(p.load().is_err());
    v.as_object_mut().unwrap().remove("member_item_use");
    v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!([]);
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["flags"] = json!(["damage_immunity"]);
    v["pve"]["combat"]["projectile"]["on_commit_self_statuses"] = json!(["group"]);
    p.write(&v);
    assert!(p.load().is_ok());
    v["skill"]["on_hit_target_statuses"] = json!(["group"]);
    p.write(&v);
    assert!(p.load().is_err());
}
#[test]
fn compound_silence_interrupts_channel_and_retains_already_paid_pulse() {
    let (mut c, mut w) = setup();
    add(&mut c, "group", json!(["silence", "root"]));
    c.skill = serde_json::from_value(skill("projectile")).unwrap();
    c.skill.channel =
        serde_json::from_value(json!({"duration_ms":360,"interval_ms":90,"spirit_per_pulse":3}))
            .unwrap();
    commit(&mut w, &c, "projectile");
    w.tick(&c, 120);
    assert!(w.members[0].channel.is_some());
    assert_eq!(w.shots.len(), 1);
    let spirit = w.members[0].spirit;
    let own = target(&w);
    let e = bind(&mut w, &c, Owner::Npc(0), own, "group", 150);
    assert!(w.members[0].channel.is_none());
    assert_eq!(w.shots.len(), 1);
    assert_eq!(w.members[0].spirit, spirit);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ChannelEnded {
                reason: ChannelEndReason::Interrupted,
                ..
            }
        }
    )));
}

#[test]
fn compound_first_immunity_removal_reconciles_caps_before_remaining_hit_status_sampling() {
    let (mut c, mut w) = setup();
    add(&mut c, "guard", json!(["first_control_immunity"]));
    add(&mut c, "group", json!(["root"]));
    c.pve_mut().status_definitions.iter_mut().find(|d|d.id=="guard").unwrap().modifiers=serde_json::from_value(json!([{"attribute":"max_hp","modifier_bps":10000},{"attribute":"max_spirit","modifier_bps":10000}])).unwrap();
    let own = target(&w);
    let base_hp = w.entity_max_hp(&own, &c, 0).unwrap();
    let base_spirit = w.member_max_spirit(0, &c, 0);
    bind(&mut w, &c, Owner::Member(0), own.clone(), "guard", 0);
    w.members[0].hp = base_hp * 2;
    w.members[0].spirit = base_spirit * 2;
    w.apply_owned_statuses(
        Owner::Npc(0),
        (own, 0),
        &["group".into(), "ward".into()],
        &c,
        30,
        &mut vec![],
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "guard" || s.definition_id == "group")
    );
    let bps = c
        .pve
        .as_ref()
        .unwrap()
        .status_definitions
        .iter()
        .find(|d| d.id == "ward")
        .unwrap()
        .shield_bps
        .unwrap();
    assert_eq!(
        current(&w, "ward").shield.unwrap().granted,
        base_hp * bps / 10000
    );
    assert_eq!(
        (w.members[0].hp, w.members[0].spirit),
        (base_hp, base_spirit)
    );
}
