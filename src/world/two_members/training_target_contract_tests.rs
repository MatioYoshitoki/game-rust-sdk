use super::*;

fn training_input(v: &mut Value) {
    summon_input(v, 900);
    v["pve"]["additional_templates"]["ally"]["combat"]["melee_range"] = json!(2);
    v["pve"]["monster_positions"] = json!([[52, 20], [52, 40], [80, 20]]);
    v["pve"]["drop_table"]["guaranteed"] = json!([{
        "itemId":"item","quantityMin":1,"quantityMax":1
    }]);
    v["params"]["entry_years"] = json!(0);
    v["carried_quantity"] = json!(0);
    v["members"]["training_targets"] = json!([
        {"initial_preset":"first","presets":{"first":["npc-1"],"other":["npc-3"]}},
        {"initial_preset":"second","presets":{"second":["npc-2"]}}
    ]);
    v["pve"]["status_rules"] = input()["pve"]["status_rules"].clone();
    v["pve"]["status_definitions"] = json!([{
        "id":"ward","mechanism":"absorption_shield","shield_bps":2000,
        "duration_ms":900,"duplicate_policy":"refresh","source_scoped":true
    }]);
    v["additional_skills"] = json!([
        {"id":"bolt","mechanism":"unit_projectile","cost":0,"range":80,"radius":1,"speed":60,
         "windup_ms":30,"recovery_ms":0,"cooldown_ms":30,"damage_bps":10000},
        {"id":"field","mechanism":"ground_field","cost":0,"range":80,"radius":7,"damage_bps":5000,
         "windup_ms":30,"cooldown_ms":30,"recovery_ms":0,"warning_ms":0,"duration_ms":300,
         "tick_interval_ms":60,"contact_policy":{"max_targets_per_tick":1,"max_total_hits":4,"repeat_interval_ms":60}}
    ]);
}

fn bolt(target: &EntityRef) -> Action {
    Action::CastUnit {
        skill_id: "bolt".into(),
        target: World::unit_wire(target),
    }
}

#[test]
fn clearing_one_training_seat_keeps_the_other_targets_and_effects_live() {
    let c = catalog(input(), training_input).unwrap().default_config();
    let mut w = world(&c);
    let original = w.npcs[0].entity.clone();
    let other = w.npcs[1].entity.clone();
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 0)
            .monsters
            .iter()
            .map(|n| (n.entity.id.as_str(), n.training_owner_id.as_deref()))
            .collect::<Vec<_>>(),
        [("npc-1", Some("a")), ("npc-2", Some("b"))]
    );
    assert_eq!(
        w.action(0, &bolt(&other), "foreign-target", &c, 0),
        Err(Error::InvalidTarget)
    );
    for i in 0..2 {
        cast_summon(&mut w, &c, i, 0, "summon");
    }
    w.tick(&c, 30);
    for (i, position) in [(0, [52., 20.]), (1, [52., 40.])] {
        let entity = w.members[i].reference.entity.clone();
        w.apply_owned_statuses(
            Owner::Member(i),
            (entity, 100),
            &["ward".into()],
            &c,
            30,
            &mut vec![],
        );
        w.action(
            i,
            &Action::CastGround {
                skill_id: "field".into(),
                position,
            },
            "field",
            &c,
            30,
        )
        .unwrap();
    }
    w.tick(&c, 60);
    for (i, target) in [(0, &original), (1, &other)] {
        w.action(i, &bolt(target), "bolt", &c, 60).unwrap();
    }
    w.tick(&c, 90);
    let b = w.members[1].reference.entity.clone();
    let b_shot = w
        .shots
        .iter()
        .find(|s| s.view.source == b)
        .unwrap()
        .view
        .clone();
    let b_area = w
        .areas
        .iter()
        .find(|s| s.view().source == b)
        .unwrap()
        .view();
    let b_view = w.snapshot(1, &c, Phase::Active, 90).own;
    assert_eq!(b_view.shield_total, Some(20));
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 90).own.shield_total,
        Some(20)
    );
    assert_eq!(w.summons.len(), 2);
    let b_hp = w.npcs[1].hp;
    let a_hp = w.members[0].hp;
    let a_spirit = w.members[0].spirit;
    assert_eq!(
        w.action(
            0,
            &Action::SetOwnedTargets {
                preset_id: "second".into()
            },
            "foreign-group",
            &c,
            90
        ),
        Err(Error::InvalidTarget)
    );
    assert_eq!((w.shots.len(), w.areas.len()), (2, 2));

    w.action(0, &Action::ClearOwnedTargets {}, "clear", &c, 90)
        .unwrap();
    assert!(w.members[0].eligible());
    assert_eq!((w.members[0].hp, w.members[0].spirit), (a_hp, a_spirit));
    assert_eq!(w.snapshot(1, &c, Phase::Active, 90).own, b_view);
    assert_eq!(w.npcs[1].entity, other);
    assert_eq!(w.npcs[1].hp, b_hp);
    assert_eq!(w.shots.len(), 1);
    assert_eq!(w.shots[0].view, b_shot);
    assert_eq!(w.areas.len(), 1);
    assert_eq!(w.areas[0].view(), b_area);
    let summons = w
        .snapshot(1, &c, Phase::Active, 90)
        .monsters
        .iter()
        .filter_map(|n| n.summon.as_ref().map(|s| s.owner.clone()))
        .collect::<Vec<_>>();
    assert_eq!(summons.as_slice(), std::slice::from_ref(&b));
    assert!(w.snapshot(0, &c, Phase::Active, 90).own.statuses.is_empty());
    assert!(
        !w.snapshot(0, &c, Phase::Active, 90)
            .monsters
            .iter()
            .any(|n| n.entity.id == "npc-1")
    );

    w.action(
        0,
        &Action::SetOwnedTargets {
            preset_id: "other".into(),
        },
        "switch",
        &c,
        90,
    )
    .unwrap();
    assert!(
        w.snapshot(0, &c, Phase::Active, 90)
            .monsters
            .iter()
            .any(|n| n.entity.id == "npc-3")
    );
    w.action(
        0,
        &Action::SetOwnedTargets {
            preset_id: "first".into(),
        },
        "restore",
        &c,
        90,
    )
    .unwrap();
    assert!(w.npcs[0].entity.generation > original.generation);
    assert_eq!(
        w.action(0, &bolt(&original), "old-generation", &c, 90),
        Err(Error::InvalidTarget)
    );
    let mut b_contact = false;
    for now in (120..=660).step_by(30) {
        b_contact |= w.tick(&c, now).iter().any(|(_, e)| {
            matches!(e,
            EventV4::Damage{source,target,amount:10,..} if *source==b && *target==other)
        });
    }
    assert!(b_contact);
    assert!(w.npcs[1].hp < b_hp);
    assert!(!w.shots.iter().any(|s| s.view.id == b_shot.id));
    let entity = w.members[1].reference.entity.clone();
    let hit = w.resolve_damage(
        combat::DamageIntent {
            source: entity,
            target: other,
            raw: 1000,
            direct: true,
            ignore_armor: true,
        },
        &c,
        660,
        &mut vec![],
    );
    assert!(hit.lethal);
    assert!(w.resources.is_empty());
    assert!(
        w.members
            .iter()
            .all(|m| m.tally.is_empty() && m.desired.is_none())
    );
}

#[test]
fn training_catalog_rejects_shared_targets_unassigned_targets_and_paid_entry() {
    for (path, value, error) in [
        (
            "/members/training_targets/1/presets/second",
            json!(["npc-1", "npc-2"]),
            "CONFIG_TRAINING_TARGETS",
        ),
        (
            "/members/training_targets/0/presets/other",
            json!(["npc-1"]),
            "CONFIG_TRAINING_TARGETS",
        ),
        (
            "/params/entry_years",
            json!(1),
            "CONFIG_TRAINING_CAPABILITY",
        ),
    ] {
        let result = catalog(input(), |v| {
            training_input(v);
            *v.pointer_mut(path).unwrap() = value;
        });
        assert!(result.err().unwrap().to_string().contains(error));
    }
}

fn training_runtime_input(v: &mut Value) {
    training_input(v);
    v["combat_rules"]
        .as_object_mut()
        .unwrap()
        .remove("member_summons");
    v["members"]["spawns"][1] = json!([50, 30]);
    v["members"]["training_targets"][0]["presets"]["first"] = json!(["npc-1", "npc-3"]);
    v["members"]["training_targets"][0]["behaviors"] = json!({"npc-1":{"kind":"combat"}});
    v["members"]["training_targets"][1]["behaviors"] = json!({
        "npc-2":{"kind":"dummy","reset_on_defeat":true,"motion":{"end":[58,40],"speed":100}}
    });
    v["members"]["training_runtime"] = json!({
        "respawn":{"delay_ms":95,"points":[[18,68],[18,88]]},
        "metrics":{"window_ms":180,"publish_interval_ms":60}
    });
    v["pve"]["stats"]["atk"] = json!(400);
    v["pve"]["combat"]["cast_chance"] = json!(1);
    v["pve"]["combat"]["projectile"]["speed"] = json!(600);
    v["pve"]["combat"]["projectile"]["windup_ms"] = json!(90);
    v["combat_rules"]["profiles"]["test"]["critical_chance_bps"] = json!(10000);
    v["combat_rules"]["profiles"]["test"]["critical_multiplier_bps"] = json!(15000);
    v["skill"] = json!({
        "id":"mark","mechanism":"unit_effect","range":80,"cost":15,"windup_ms":30,
        "recovery_ms":0,"cooldown_ms":600,"damage_bps":10000,"unavoidable":true
    });
}

fn mark(w: &mut World, i: usize, j: usize, c: &Config) {
    let target = World::unit_wire(&w.npcs[j].entity);
    w.action(
        i,
        &Action::CastUnit {
            skill_id: "mark".into(),
            target,
        },
        "mark",
        c,
        0,
    )
    .unwrap();
}

#[test]
fn training_runtime_revives_only_the_defeated_seat_and_keeps_its_skill_clock_and_metrics() {
    let c = catalog(input(), training_runtime_input)
        .unwrap()
        .default_config();
    let mut w = world(&c);
    let a = w.members[0].reference.entity.clone();
    let b = w.members[1].reference.entity.clone();
    mark(&mut w, 0, 2, &c);
    mark(&mut w, 1, 1, &c);
    w.tick(&c, 30);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&a));
    assert_eq!(w.npcs[1].position, [55., 40.]);
    let a_metrics = w.training_view(0).unwrap().metrics.unwrap();
    assert_eq!(
        (
            a_metrics.damage,
            a_metrics.hits,
            a_metrics.critical_hits,
            a_metrics.highest_hit
        ),
        (15, 1, 1, 15)
    );
    assert_eq!(
        (
            a_metrics.rolling_dps,
            a_metrics.average_dps,
            a_metrics.by_source.skill
        ),
        (83, 250, 15)
    );
    for (i, position) in [(0, [24., 28.]), (1, [55., 40.])] {
        w.action(
            i,
            &Action::CastGround {
                skill_id: "field".into(),
                position,
            },
            "field",
            &c,
            30,
        )
        .unwrap();
    }
    w.tick(&c, 60);
    assert_eq!(w.npcs[1].position, [58., 40.]);
    w.tick(&c, 90);
    assert_eq!(w.npcs[1].position, [55., 40.]);
    let field = w
        .areas
        .iter()
        .find(|p| p.view().source == b)
        .unwrap()
        .view()
        .id;
    let before = w.snapshot(0, &c, Phase::Active, 90);
    let key = MatchKey {
        id: "training".into(),
        incarnation: 1,
    };
    let mut replica = crate::replication::ReplicaV6::new(key.clone(), "a".into());
    let full = PublicationEnvelopeV6 {
        key,
        publication_id: "full".into(),
        recipient_id: "a".into(),
        connection: Connection {
            id: "a".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            snapshot: Box::new(before.clone()),
            reason: FullReason::Initial,
        },
    };
    replica.apply(&full).unwrap();
    let mut killed_at = None;
    for now in (120..=210).step_by(30) {
        w.tick(&c, now);
        if w.members[0].hp == 0 {
            killed_at = Some(now);
            break;
        }
    }
    let killed_at = killed_at.unwrap();
    let due = killed_at + 95;
    assert_eq!(w.training_respawn_at(0), Some(due));
    assert!(w.members[0].desired.is_none());
    assert!(w.members[1].eligible() && w.members[1].hp == 100);
    assert_eq!(w.areas.len(), 1);
    assert_eq!(w.areas[0].view().id, field);
    let down = w.snapshot(0, &c, Phase::Active, killed_at);
    let delta = PublicationEnvelopeV6 {
        publication_id: "down".into(),
        revision: 2,
        payload: PublicationPayloadV6::Delta {
            base_publication_id: full.publication_id.clone(),
            base_revision: 1,
            delta: Box::new(crate::replication::diff(&before, &down)),
        },
        ..full
    };
    assert_eq!(
        replica.apply(&delta).unwrap().own.training,
        down.own.training
    );
    assert_eq!(
        w.action(
            0,
            &Action::Move {
                direction: [1., 0.]
            },
            "down",
            &c,
            killed_at
        ),
        Err(Error::NotReady)
    );
    w.protect(0, killed_at, &mut vec![]);
    let b_target = w.npcs[1].entity.clone();
    let revived_at = due.div_ceil(c.step_ms) * c.step_ms;
    for now in (killed_at + 30..revived_at).step_by(30) {
        w.tick(&c, now);
        assert_eq!(w.training_respawn_at(0), Some(due));
    }
    let events = w.tick(&c, revived_at);
    let restored = &w.members[0];
    assert_eq!(restored.reference.entity.generation, a.generation + 1);
    assert_eq!(
        (restored.position, restored.hp, restored.spirit),
        ([18., 68.], 100, 100)
    );
    assert_eq!(restored.cooldowns["mark"], 630);
    assert!(restored.protected && !restored.ready && !restored.eligible());
    assert!(events.iter().any(|(_, e)| matches!(e, EventV4::ModeRespawned { entity } if *entity == restored.reference.entity)));
    assert_eq!(w.training_respawn_at(0), None);
    assert_eq!(w.members[1].reference.entity, b);
    assert_eq!(w.npcs[1].entity, b_target);
    let retained = w.training_view(0).unwrap().metrics.unwrap();
    assert_eq!((retained.damage, retained.rolling_dps), (15, 0));
    assert!(w.training_view(1).unwrap().metrics.unwrap().damage > 15);
    w.action(1, &Action::ResetTrainingMetrics {}, "reset", &c, revived_at)
        .unwrap();
    assert_eq!(w.training_view(1).unwrap().metrics.unwrap().damage, 0);
    assert_eq!(w.training_view(0).unwrap().metrics.unwrap(), retained);
    assert!(w.resources.is_empty());
}

#[test]
fn training_dummy_rebuild_invalidates_old_targets_and_training_exit_cancels_respawn() {
    let c = catalog(input(), training_runtime_input)
        .unwrap()
        .default_config();
    let mut w = world(&c);
    let b = w.members[1].reference.entity.clone();
    let dummy = w.npcs[1].entity.clone();
    mark(&mut w, 1, 1, &c);
    let mut events = vec![];
    let hit = w.resolve_damage(
        combat::DamageIntent {
            source: b,
            target: dummy.clone(),
            raw: 1000,
            direct: true,
            ignore_armor: true,
        },
        &c,
        0,
        &mut events,
    );
    assert_eq!(hit.hp_damage, 100);
    assert!(
        !events
            .iter()
            .any(|(_, e)| matches!(e, EventV4::MonsterDefeated { .. }))
    );
    w.tick(&c, 30);
    assert_eq!(w.npcs[1].entity.generation, dummy.generation + 1);
    assert_eq!(w.npcs[1].hp, 100);
    assert!(w.members[1].pending.is_none());
    assert_eq!(
        w.action(1, &bolt(&dummy), "stale", &c, 30),
        Err(Error::InvalidTarget)
    );
    assert_eq!(w.training_view(1).unwrap().metrics.unwrap().damage, 100);
    let down = (60..=240)
        .step_by(30)
        .find(|&now| {
            w.tick(&c, now);
            w.training_respawn_at(0).is_some()
        })
        .unwrap();
    w.action(0, &Action::Leave {}, "leave-down", &c, down)
        .unwrap();
    assert_eq!(w.training_respawn_at(0), None);
    assert_eq!(w.members[0].phase, MemberPhase::Settling);
    w.tick(&c, down + 120);
    assert_eq!(w.members[0].reference.entity.generation, 1);
    assert!(w.members[1].eligible());
    assert!(w.npcs[1].training_active);
    assert!(w.resources.is_empty());
}

#[test]
fn training_motion_obeys_dynamic_blockers_and_rejects_permanent_obstructions() {
    let geometry = |v: &mut Value| {
        training_runtime_input(v);
        v["pve"]["world_geometry"] = json!({
            "boundary":[[0,0],[128,0],[128,128],[0,128]],
            "blockers":[{"id":"gate","vertices":[[57.5,34],[62,34],[62,46],[57.5,46]],
                "blocks_movement":true,"blocks_projectiles":false,"blocks_vision":false,
                "active_window":{"start_ms":60,"end_ms":150}}]
        });
    };
    let c = catalog(input(), geometry).unwrap().default_config();
    let mut w = world(&c);
    for now in [30, 60, 90, 120] {
        w.tick(&c, now);
        assert_eq!(w.npcs[1].position, [55., 40.]);
    }
    w.tick(&c, 150);
    assert_eq!(w.npcs[1].position, [58., 40.]);
    let invalid = catalog(input(), |v| {
        geometry(v);
        v["pve"]["world_geometry"]["blockers"][0]
            .as_object_mut()
            .unwrap()
            .remove("active_window");
    });
    assert!(
        invalid
            .err()
            .unwrap()
            .to_string()
            .contains("CONFIG_TRAINING_MOTION_PATH")
    );
}
