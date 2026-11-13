use super::*;
fn definition(heal: bool, policy: &str) -> Value {
    let mut d = json!({"id":"composed","mechanism":if heal{"periodic_heal"}else{"periodic_damage"},"duration_ms":1800,"tick_interval_ms":500,"duplicate_policy":"stack","max_stacks":10,"source_scoped":true,"periodic_stack":{"tick_policy":policy},"modifiers":[{"attribute":"healing_received","modifier_bps":-300}]});
    d[if heal { "heal_bps" } else { "damage_bps" }] = json!(if heal { 70 } else { 700 });
    d
}
fn fixture(heal: bool, policy: &str) -> (Config, World) {
    let (mut c, w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(definition(heal, policy)).unwrap());
    (c, w)
}
fn apply(w: &mut World, c: &Config, source: Owner, target: Owner, now: u64) -> Events {
    let target = w.owner_entity(target);
    bind(w, c, source, target, "composed", now)
}
fn view(w: &World) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "composed")
        .unwrap()
        .clone()
}
fn amount(w: &mut World, c: &Config, now: u64, heal: bool) -> u32 {
    let mut e = vec![];
    w.advance_periodic(c, now, &mut e);
    e.iter()
        .find_map(|(_, e)| match e {
            EventV4::Private {
                event: Event::PeriodicTick { raw_damage, .. },
            } if !heal => Some(*raw_damage),
            EventV4::Private {
                event: Event::PeriodicHealTick { raw_heal, .. },
            } if heal => Some(*raw_heal),
            _ => None,
        })
        .unwrap()
}
#[test]
fn layered_periodic_loader_roundtrips_and_rejects_invalid_clock_flags_and_versions() {
    let p = Package::new();
    let mut v = document();
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(definition(false, "preserve"));
    p.write(&v);
    let catalog = p.load().unwrap();
    assert_eq!(
        serde_json::to_value(catalog.default_config()).unwrap()["pve"]["status_definitions"]
            .as_array()
            .unwrap()
            .last(),
        v["pve"]["status_definitions"].as_array().unwrap().last()
    );
    for case in 0..18 {
        let mut bad = v.clone();
        let ds = bad["pve"]["status_definitions"].as_array_mut().unwrap();
        let d = ds.last_mut().unwrap();
        match case {
            0 => d["periodic_stack"] = Value::Null,
            1 => d["periodic_stack"] = json!({}),
            2 => d["periodic_stack"] = json!({"tick_policy":"unknown"}),
            3 => d["periodic_stack"] = json!({"tick_policy":{"preserve":null}}),
            4 => d["source_scoped"] = json!(false),
            5 => d["duplicate_policy"] = json!("refresh"),
            6 => d["max_stacks"] = json!(33),
            7 => d["max_stacks"] = json!(0),
            8 => d["duration_ms"] = json!(64001),
            9 => d["flags"] = json!(["control_immunity"]),
            10 => d["flags"] = json!(["root"]),
            11 => d["modifiers"] = json!([{"attribute":"movement_haste","modifier_bps":-1}]),
            12 => d["modifiers"] = json!([{"attribute":"root","modifier_bps":0}]),
            13 => {
                d["modifiers"] = json!([{"attribute":"atk","modifier_bps":1},{"attribute":"atk","modifier_bps":2}])
            }
            14 => {
                d.as_object_mut().unwrap().remove("periodic_stack");
            }
            15 => bad["mechanism_version"] = json!(104),
            17 => d["modifiers"] = json!([]),
            _ => d["periodic_stack"] = json!({"tick_policy":"preserve","other":true}),
        }
        p.write(&bad);
        assert!(p.load().is_err(), "case {case}");
    }
}
#[test]
fn composed_hot_flags_allow_only_nonstack_immunity_and_keep_legacy_wire() {
    let p = Package::new();
    let mut v = document();
    let mut d = definition(true, "preserve");
    for k in ["periodic_stack", "max_stacks"] {
        d.as_object_mut().unwrap().remove(k);
    }
    d["duplicate_policy"] = json!("refresh");
    d["flags"] = json!(["first_control_immunity"]);
    d["modifiers"] = json!([{"attribute":"atk","modifier_bps":2000}]);
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(d);
    p.write(&v);
    assert!(p.load().is_ok());
    for flags in [
        json!([]),
        json!(["root"]),
        json!(["control_immunity", "root"]),
        json!(["control_immunity", "control_immunity"]),
    ] {
        let mut bad = v.clone();
        bad["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()["flags"] = flags;
        p.write(&bad);
        assert!(p.load().is_err());
    }
    let (c, mut w) = setup();
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "renewal", 0);
    let wire = serde_json::to_value(
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "renewal")
            .unwrap(),
    )
    .unwrap();
    assert!(wire["periodic"].get("per_stack").is_none());
    assert!(wire["periodic"].get("first_tick_at_ms").is_none());
}
#[test]
fn cauldron_two_layers_floor_after_aggregation_and_keep_one_clock_and_received_modifier() {
    let (c, mut w) = fixture(false, "preserve");
    w.members[0].facts.atk_points = 99;
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), 0);
    let id = view(&w).instance_id;
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), 300);
    assert_eq!(view(&w).instance_id, id);
    assert_eq!(view(&w).stacks, Some(2));
    assert_eq!(
        view(&w).periodic.as_ref().unwrap().next_tick_at_ms,
        Some(500)
    );
    assert_eq!(
        w.statuses.healing_received_bps(
            &status_entity(&w.npcs[0].entity),
            300,
            c.pve.as_ref().unwrap().status_rules.as_ref()
        ),
        9400
    );
    assert_eq!(amount(&mut w, &c, 500, false), 13);
    assert_eq!(
        view(&w).periodic.unwrap().raw_damage,
        6,
        "wire advertises the unmultiplied base; tick is13"
    );
}
#[test]
fn preserved_continuous_stacks_do_not_starve_ticks_but_restart_is_explicit() {
    for (policy, due) in [("preserve", 500), ("restart", 950)] {
        let (c, mut w) = fixture(false, policy);
        for now in [0, 150, 300, 450] {
            apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), now);
        }
        let s = view(&w);
        assert_eq!(s.stacks, Some(4));
        assert_eq!(s.periodic.unwrap().next_tick_at_ms, Some(due));
        assert_eq!(amount(&mut w, &c, due, false), 28);
    }
}
#[test]
fn periodic_stack_cap_refresh_and_partial_consume_keep_private_inputs_and_schedule() {
    let (c, mut w) = fixture(false, "preserve");
    w.members[0].facts.atk_points = 99;
    for now in 0..12 {
        apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), now * 30);
    }
    let before = view(&w);
    assert_eq!(before.stacks, Some(10));
    assert!(w.statuses.consume(&before.instance_id, 8, &mut vec![]));
    let after = view(&w);
    assert_eq!(after.stacks, Some(2));
    assert_eq!(after.periodic, before.periodic);
    assert!(w.statuses.periodic_inputs(&before.instance_id).is_some());
    assert_eq!(amount(&mut w, &c, 500, false), 13);
    assert!(w.statuses.consume(&before.instance_id, 2, &mut vec![]));
    assert!(w.statuses.periodic_inputs(&before.instance_id).is_none());
    let mut e = vec![];
    w.advance_periodic(&c, 1000, &mut e);
    assert!(e.is_empty());
}
#[test]
fn layered_transfer_preserves_snapshot_or_uses_new_live_holder_and_old_exit_cannot_clear() {
    for (sampling, expected) in [
        (PeriodicSampling::TargetSnapshot, 13),
        (PeriodicSampling::TargetLive, 28),
    ] {
        let (mut c, mut w) = fixture(false, "preserve");
        c.combat_rules
            .as_mut()
            .unwrap()
            .periodic_sampling
            .insert("composed".into(), sampling);
        w.members[0].facts.atk_points = 99;
        w.members[1].facts.atk_points = 200;
        apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), 0);
        apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), 300);
        let s = view(&w);
        let target = status_entity(&w.members[1].reference.entity);
        w.statuses
            .retarget(std::slice::from_ref(&s.instance_id), &target, &mut vec![]);
        let mut expected_view = s.clone();
        expected_view.target = target;
        assert_eq!(view(&w), expected_view);
        w.clear_owned_statuses(
            &w.members[0].reference.entity.clone(),
            StatusRemovalReason::TargetInvalid,
            &mut vec![],
        );
        assert_eq!(amount(&mut w, &c, 500, false), expected);
        w.clear_owned_statuses(
            &w.npcs[0].entity.clone(),
            StatusRemovalReason::SourceInvalid,
            &mut vec![],
        );
        assert!(w.statuses.periodic_inputs(&s.instance_id).is_none());
        let mut e = vec![];
        w.advance_periodic(&c, 1000, &mut e);
        assert!(e.is_empty());
    }
}
#[test]
fn layered_hot_snapshots_formula_inputs_and_floors_hp_and_spirit_once() {
    let (mut c, mut w) = fixture(true, "preserve");
    c.combat_rules.as_mut().unwrap().healing.insert(
        "composed".into(),
        serde_json::from_value(json!({"fixed":1,"source_attack_bps":700,"target_max_hp_bps":3}))
            .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().periodic_recovery.insert(
        "composed".into(),
        serde_json::from_value(
            json!({"spirit":{"fixed":1,"source_attack_bps":700,"target_max_spirit_bps":3}}),
        )
        .unwrap(),
    );
    w.members[0].facts.atk_points = 99;
    w.members[1].hp = 1;
    w.members[1].spirit = 0;
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), 0);
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), 300);
    w.members[0].facts.atk_points = 1000;
    // rawHP=2+floor((99*700+1100*3)*2/10000)=16; received94%=15.
    assert_eq!(amount(&mut w, &c, 500, true), 16);
    assert_eq!(w.members[1].hp, 16);
    // rawSP=2+floor((99*700+300*3)*2/10000)=16; no receiving penalty on SP.
    assert_eq!(w.members[1].spirit, 16);
}
#[test]
fn first_immunity_consumes_the_entire_hot_with_its_attributes_and_next_tick() {
    let (mut c, mut w) = fixture(true, "preserve");
    let d = c.pve_mut().status_definitions.last_mut().unwrap();
    d.periodic_stack = None;
    d.max_stacks = None;
    d.duplicate_policy = crate::config::DuplicatePolicy::Refresh;
    d.flags = vec![StatusAttribute::FirstControlImmunity];
    d.modifiers = vec![StatusModifier {
        attribute: StatusAttribute::Atk,
        modifier_bps: 2000,
    }];
    apply(&mut w, &c, Owner::Member(0), Owner::Member(0), 0);
    assert_eq!(w.stats(0, &c, 0).0, 120);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"root-attempt","mechanism":"timed_control","attribute":"root","duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target, "root-attempt", 300);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "composed" || s.definition_id == "root-attempt")
    );
    assert_eq!(w.stats(0, &c, 300).0, 100);
    let mut e = vec![];
    w.advance_periodic(&c, 500, &mut e);
    assert!(e.is_empty());
}
#[test]
fn layered_compound_movement_uses_strongest_slow_and_haste_without_agility_or_max_hp() {
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"composed","mechanism":"timed_compound_modifier","duration_ms":1800,"duplicate_policy":"stack","max_stacks":3,"source_scoped":true,"modifiers":[{"attribute":"attack_speed","modifier_bps":-3000},{"attribute":"movement_slow","modifier_bps":-500}]})).unwrap());
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), 0);
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), 30);
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"tribulation","mechanism":"timed_compound_modifier","duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true,"modifiers":[{"attribute":"atk","modifier_bps":1200},{"attribute":"movement_haste","modifier_bps":1000}]})).unwrap());
    let target = w.members[0].reference.entity.clone();
    bind(
        &mut w,
        &c,
        Owner::Member(0),
        target.clone(),
        "tribulation",
        60,
    );
    assert_eq!(
        w.statuses.movement_factor_bps(&status_entity(&target), 60),
        10000
    );
    assert_eq!(w.stats(0, &c, 60).0, 112);
    assert_eq!(w.entity_max_hp(&target, &c, 60), Some(1000));
    assert_eq!(
        w.statuses.attribute(
            &status_entity(&target),
            10000,
            60,
            c.pve.as_ref().unwrap().status_rules.as_ref(),
            StatusAttribute::AttackSpeed
        ),
        4000
    );
}
#[test]
fn shortened_harmful_periodic_can_legally_expire_before_first_tick_without_a_late_reward() {
    let (mut c, mut w) = fixture(false, "preserve");
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"resist","mechanism":"timed_attribute_modifier","attribute":"harmful_duration_reduction","modifier_bps":9000,"duration_ms":1800,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "resist", 0);
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), 0);
    let s = view(&w);
    assert_eq!(s.expires_at_ms, 180);
    assert_eq!(s.periodic.as_ref().unwrap().total_ticks, 0);
    assert_eq!(s.periodic.as_ref().unwrap().next_tick_at_ms, None);
    let mut e = vec![];
    w.prepare_periodic(180, &mut e);
    w.advance_periodic(&c, 500, &mut e);
    assert!(w.statuses.periodic_inputs(&s.instance_id).is_none());
    assert_eq!(w.members[0].hp, 1000);
}
#[test]
fn approved_six_status_projections_use_native_root_speed_and_original_compound_numbers() {
    let rows: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_composed_status_vectors.json"
    ))
    .unwrap();
    for (i, row) in rows["statuses"].as_array().unwrap().iter().enumerate() {
        let (mut c, mut w) = setup();
        let d: crate::config::StatusDefinition =
            serde_json::from_value(row["definition"].clone()).unwrap();
        let beneficial = matches!(i, 2 | 4);
        c.pve_mut().status_definitions.push(d);
        let source = if beneficial {
            Owner::Member(0)
        } else {
            Owner::Npc(0)
        };
        let target = w.members[0].reference.entity.clone();
        bind(
            &mut w,
            &c,
            source,
            target.clone(),
            row["id"].as_str().unwrap(),
            0,
        );
        let factor = w.statuses.movement_factor_bps(&status_entity(&target), 0);
        assert_eq!(
            factor,
            [10000, 4000, 7000, 7500, 11000, 10000, 10000][i],
            "{}",
            row["id"]
        );
        assert_eq!(w.entity_max_hp(&target, &c, 0), Some(1000));
        if i == 0 {
            assert!(w.member_has_status(0, StatusAttribute::Root, 0));
            assert!(!w.member_has_status(0, StatusAttribute::Stun, 0));
        }
        if i == 2 {
            assert_eq!(
                w.statuses.damage_reduction_bps(
                    &status_entity(&target),
                    0,
                    c.pve.as_ref().unwrap().status_rules.as_ref()
                ),
                2000
            );
            assert!(
                w.statuses
                    .entries()
                    .iter()
                    .any(|s| s.has_attribute(StatusAttribute::FirstControlImmunity))
            );
        }
        if i == 4 {
            assert_eq!(w.stats(0, &c, 0).0, 112);
        }
        if i == 5 {
            let d: crate::config::StatusDefinition =
                serde_json::from_value(row["conditionalMovement"]["definition"].clone()).unwrap();
            c.pve_mut().status_definitions.push(d);
            bind(
                &mut w,
                &c,
                source,
                target.clone(),
                "dragon-movement-qualified",
                0,
            );
            assert_eq!(
                w.statuses.movement_factor_bps(&status_entity(&target), 0),
                9500
            );
        }
    }
}
#[test]
fn different_periodic_sources_have_independent_slots_layers_and_private_snapshot_inputs() {
    let (c, mut w) = fixture(false, "preserve");
    w.members[0].facts.atk_points = 99;
    w.members[1].facts.atk_points = 200;
    for src in [Owner::Member(0), Owner::Member(1)] {
        apply(&mut w, &c, src, Owner::Npc(0), 0);
        apply(&mut w, &c, src, Owner::Npc(0), 300);
    }
    let entries: Vec<_> = w
        .statuses
        .entries()
        .iter()
        .filter(|s| s.definition_id == "composed")
        .cloned()
        .collect();
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0].instance_id, entries[1].instance_id);
    w.clear_owned_statuses(
        &w.members[0].reference.entity.clone(),
        StatusRemovalReason::SourceInvalid,
        &mut vec![],
    );
    assert!(
        w.statuses
            .periodic_inputs(&entries[0].instance_id)
            .is_none()
    );
    assert!(
        w.statuses
            .periodic_inputs(&entries[1].instance_id)
            .is_some()
    );
    assert_eq!(amount(&mut w, &c, 500, false), 28);
}
#[test]
fn rust_replica_rejects_invalid_periodic_clocks_atomically_and_preserves_valid_full_delta() {
    use crate::replication::{DecodeError, ReplicaV6};
    let (c, mut w) = fixture(false, "preserve");
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), 0);
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), 300);
    let key = MatchKey {
        id: "composed".into(),
        incarnation: 1,
    };
    let snapshot = w.snapshot(0, &c, Phase::Active, 300);
    let f = PublicationEnvelopeV6 {
        key: key.clone(),
        publication_id: "full-1".into(),
        recipient_id: "P00".into(),
        connection: Connection {
            id: "c".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            snapshot: Box::new(snapshot.clone()),
            reason: FullReason::Initial,
        },
    };
    let mut r = ReplicaV6::new(key, "P00".into());
    assert_eq!(r.apply(&f).unwrap(), &snapshot);
    for case in 0..8 {
        let mut n = snapshot.clone();
        let status = n
            .own
            .statuses
            .iter_mut()
            .find(|s| s.definition_id == "composed")
            .unwrap();
        let p = status.periodic.as_mut().unwrap();
        match case {
            0 => p.per_stack = Some(false),
            1 => p.first_tick_at_ms = None,
            2 => p.next_tick_at_ms = Some(1000),
            3 => p.total_ticks -= 1,
            4 => status.stacks = None,
            5 => status.stacks = Some(11),
            6 => status.flags = vec![StatusAttribute::ControlImmunity],
            _ => p.raw_heal = Some(0),
        }
        let bad = PublicationEnvelopeV6 {
            publication_id: "delta-2".into(),
            revision: 2,
            payload: PublicationPayloadV6::Delta {
                base_publication_id: "full-1".into(),
                base_revision: 1,
                delta: Box::new(crate::replication::diff(&snapshot, &n)),
            },
            ..f.clone()
        };
        assert_eq!(r.apply(&bad), Err(DecodeError::InvalidFrame), "case {case}");
        assert_eq!(r.snapshot(), Some(&snapshot));
    }
    w.advance_periodic(&c, 500, &mut vec![]);
    let next = w.snapshot(0, &c, Phase::Active, 510);
    let d = PublicationEnvelopeV6 {
        publication_id: "delta-2".into(),
        revision: 2,
        payload: PublicationPayloadV6::Delta {
            base_publication_id: "full-1".into(),
            base_revision: 1,
            delta: Box::new(crate::replication::diff(&snapshot, &next)),
        },
        ..f
    };
    assert_eq!(r.apply(&d).unwrap(), &next);
}
#[test]
fn periodic_composition_quality_overrides_share_validation_and_item_capacity_failure_is_atomic() {
    let p = Package::new();
    let mut v = document();
    let mut d = definition(true, "preserve");
    d.as_object_mut().unwrap().remove("periodic_stack");
    d.as_object_mut().unwrap().remove("max_stacks");
    d["duplicate_policy"] = json!("refresh");
    d["modifiers"] = json!([{"attribute":"atk","modifier_bps":2000}]);
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(d.clone());
    let item = json!({"grant_statuses_without_recovery":true,"cooldown_ms":600,"on_use_self_statuses":["composed"],"on_use_self_status_overrides":{"composed":{"modifier_bps":0,"duration_ms":1200,"modifiers":[{"attribute":"atk","modifier_bps":3000}]}}});
    v["member_item_use"] = json!({"xuan-tie":item});
    p.write(&v);
    p.load().unwrap();
    for case in 0..3 {
        let mut bad = v.clone();
        let q = &mut bad["member_item_use"]["xuan-tie"]["on_use_self_status_overrides"]["composed"];
        match case {
            0 => q["duration_ms"] = json!(300),
            1 => q["modifier_bps"] = json!(1),
            _ => q["modifiers"][0]["attribute"] = json!("root"),
        }
        p.write(&bad);
        assert!(p.load().is_err(), "case {case}");
    }
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
    c.member_item_use = Some(
        serde_json::from_value(json!({"pill_healing":v["member_item_use"]["xuan-tie"]})).unwrap(),
    );
    w.members[0].inventory.insert("pill_healing".into(), 2);
    w.members[0].looted.insert("pill_healing".into(), 2);
    let revision = w.members[0].revision;
    let action = Action::UseItem {
        item_id: "pill_healing".into(),
        expected_revision: revision,
    };
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "renewal", 0);
    let before = w.snapshot(0, &c, Phase::Active, 0).own;
    assert_eq!(
        w.action(0, &action, "full-hot", &c, 0),
        Err(Error::Capacity)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 0).own, before);
    w.statuses
        .remove(StatusRemovalReason::Cleansed, |_| true, &mut vec![]);
    w.action(0, &action, "use-hot", &c, 0).unwrap();
    assert_eq!(w.members[0].inventory["pill_healing"], 1);
    assert_eq!(w.stats(0, &c, 0).0, 130);
    let s = view(&w);
    assert_eq!(s.expires_at_ms, 1200);
    assert_eq!(s.periodic.unwrap().next_tick_at_ms, Some(500));
}
#[test]
fn layered_transfer_full_capacity_rejects_without_losing_snapshot_and_actual_transfer_keeps_it() {
    let (mut c, mut w) = fixture(false, "preserve");
    c.combat_rules.as_mut().unwrap().status_policies.insert(
        "composed".into(),
        serde_json::from_value(
            json!({"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}),
        )
        .unwrap(),
    );
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(1), 0);
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(1), 300);
    let old = view(&w);
    let input = w.statuses.periodic_inputs(&old.instance_id).unwrap().attack;
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "renewal", 300);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let effect = serde_json::from_value(
        json!({"kind":"transfer_statuses","from":"other","disposition":"harmful","max_count":1}),
    )
    .unwrap();
    assert_eq!(
        w.plan_status_operation(
            &w.statuses,
            (Owner::Member(0), Owner::Member(1)),
            &effect,
            &c,
            300
        ),
        Err(Error::Capacity)
    );
    assert_eq!(view(&w), old);
    assert_eq!(
        w.statuses.periodic_inputs(&old.instance_id).unwrap().attack,
        input
    );
    w.statuses.remove(
        StatusRemovalReason::Cleansed,
        |s| s.definition_id == "renewal",
        &mut vec![],
    );
    w.execute_status_operation(
        (Owner::Member(0), Owner::Member(1)),
        &effect,
        &c,
        300,
        &mut vec![],
    );
    let moved = view(&w);
    assert_eq!(moved.target.id, "P00");
    assert_eq!(moved.periodic, old.periodic);
    assert_eq!(moved.stacks, old.stacks);
    assert_eq!(
        w.statuses.periodic_inputs(&old.instance_id).unwrap().attack,
        input
    );
}
#[test]
fn periodic_wire_amount_shape_rejects_missing_null_unknown_and_opposite_budget_fields() {
    let (c, mut w) = fixture(false, "preserve");
    apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), 0);
    let base = serde_json::to_value(view(&w)).unwrap();
    for case in 0..7 {
        let mut bad = base.clone();
        let p = &mut bad["periodic"];
        match case {
            0 => {
                p.as_object_mut().unwrap().remove("raw_damage");
            }
            1 => p["raw_damage"] = Value::Null,
            2 => p["raw_heal"] = json!(0),
            3 => p["raw_spirit"] = Value::Null,
            4 => {
                p.as_object_mut().unwrap().remove("next_tick_at_ms");
            }
            5 => p["power_sampling"] = Value::Null,
            _ => p["attack"] = json!(100),
        }
        assert!(
            serde_json::from_value::<StatusView>(bad).is_err(),
            "case {case}"
        );
    }
    assert_eq!(
        serde_json::from_value::<StatusView>(base).unwrap(),
        view(&w)
    );
}
#[test]
fn periodic_wire_requires_string_enums_and_cannot_hide_nonfinite_clocks_as_zero_ticks() {
    let (c, mut w) = fixture(false, "preserve");
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), 0);
    let base = serde_json::to_value(view(&w)).unwrap();
    for case in 0..3 {
        let mut bad = base.clone();
        match case {
            0 => bad["periodic"]["kind"] = json!({"physical_damage":null}),
            1 => bad["source"]["kind"] = json!({"monster":null}),
            _ => bad["periodic"]["power_sampling"] = json!({"target_live":null}),
        }
        assert!(serde_json::from_value::<StatusView>(bad).is_err());
    }
    let key = MatchKey {
        id: "clock".into(),
        incarnation: 1,
    };
    let mut snapshot = w.snapshot(0, &c, Phase::Active, 300);
    let s = &mut snapshot.own.statuses[0];
    let p = s.periodic.as_mut().unwrap();
    p.interval_ms = (1u64 << 53) - 1;
    p.first_tick_at_ms = Some(1u64 << 53);
    s.applied_at_ms = 300;
    p.total_ticks = 0;
    p.next_tick_at_ms = None;
    let f = PublicationEnvelopeV6 {
        key: key.clone(),
        publication_id: "full".into(),
        recipient_id: "P00".into(),
        connection: Connection {
            id: "c".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            snapshot: Box::new(snapshot),
            reason: FullReason::Initial,
        },
    };
    assert_eq!(
        crate::replication::ReplicaV6::new(key, "P00".into()).apply(&f),
        Err(crate::replication::DecodeError::InvalidFrame)
    );
}
#[test]
fn layered_hot_large_budget_saturates_once_then_clips_actual_hp_and_spirit_to_current_gaps() {
    let (mut c, mut w) = fixture(true, "preserve");
    c.resource_limits = Some(
        serde_json::from_value(
            json!({"max_hp":1000000000,"max_spirit":1000000000,"max_spirit_cost":8000000}),
        )
        .unwrap(),
    );
    let d = c.pve_mut().status_definitions.last_mut().unwrap();
    d.modifiers.clear();
    d.max_stacks = Some(32);
    c.combat_rules.as_mut().unwrap().healing.insert(
        "composed".into(),
        serde_json::from_value(
            json!({"fixed":1000000,"source_attack_bps":100000,"target_max_hp_bps":10000}),
        )
        .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().periodic_recovery.insert("composed".into(),serde_json::from_value(json!({"spirit":{"fixed":1000000,"source_attack_bps":100000,"target_max_spirit_bps":10000}})).unwrap());
    w.members[0].facts.atk_points = 1000000;
    w.members[1].facts.hp_points = 1000000000;
    w.members[1].facts.spirit_points = 1000000000;
    w.members[1].hp = 1;
    w.members[1].spirit = 0;
    for _ in 0..32 {
        apply(&mut w, &c, Owner::Member(0), Owner::Member(1), 0);
    }
    let mut e = vec![];
    w.advance_periodic(&c, 500, &mut e);
    assert!(e.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::PeriodicHealTick{raw_heal,hp_restored,stacks:Some(32),..}} if *raw_heal==u32::MAX && *hp_restored==999999999)));
    assert!(e.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::PeriodicSpiritTick{raw_spirit,base_restored,stacks:Some(32),..}} if *raw_spirit==u32::MAX && *base_restored==1000000000)));
    assert_eq!(w.members[1].hp, 1000000000);
    assert_eq!(w.members[1].spirit, 1000000000);
    let s = view(&w);
    let p = s.periodic.unwrap();
    assert_eq!(p.raw_heal, Some(1011000000));
    assert_eq!(p.raw_spirit, Some(1011000000));
}
#[test]
fn layered_dot_wide_product_caps_raw_once_and_cannot_award_more_than_actual_target_hp() {
    let (mut c, mut w) = fixture(false, "preserve");
    let d = c.pve_mut().status_definitions.last_mut().unwrap();
    d.damage_bps = Some(100000);
    d.max_stacks = Some(32);
    d.modifiers.clear();
    let mut r = rule(&c);
    r.damage_kind = crate::config::DamageKind::TrueDamage;
    r.base_power = Some(1000000);
    r.cultivation_bps = 1000000;
    c.combat_rules
        .as_mut()
        .unwrap()
        .periodic_damage
        .insert("composed".into(), r);
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("hp".into(), 120000000);
    w.npcs[0].hp = 120000000;
    for _ in 0..32 {
        apply(&mut w, &c, Owner::Member(0), Owner::Npc(0), 0);
    }
    let id = view(&w).instance_id;
    let mut e = vec![];
    w.advance_periodic(&c, 500, &mut e);
    assert!(e.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::PeriodicTick{raw_damage,hp_damage,stacks:Some(32),..}} if *raw_damage==u32::MAX && *hp_damage==120000000)));
    assert_eq!(w.npcs[0].hp, 0);
    w.prepare_periodic(500, &mut vec![]);
    assert!(w.statuses.periodic_inputs(&id).is_none());
    let mut late = vec![];
    w.advance_periodic(&c, 1000, &mut late);
    assert!(late.is_empty());
}
