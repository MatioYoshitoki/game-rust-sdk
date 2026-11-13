use super::combat::{DamageIntent, Relation};
use super::*;
pub(super) fn setup(n: usize) -> (Config, World) {
    let (old, w) = super::periodic_tests::setup(n);
    let cat = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/multiplayer-combat-host/catalog.json"),
    )
    .unwrap();
    let mut c = cat.default_config().as_ref().clone();
    c.members = old.members;
    let seats = w
        .members
        .iter()
        .enumerate()
        .map(|(i, m)| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            combat_build: None,
            skills: None,
            participant_id: m.reference.entity.id.clone(),
            authenticated_principal: m.facts.principal.clone(),
            connection: Connection {
                id: format!("c{i}"),
                generation: 1,
            },
        })
        .collect::<Vec<_>>();
    let facts = w.members.iter().map(|m| m.facts.clone()).collect();
    let mut w = World::new(&c, &seats, facts, 77);
    for m in &mut w.members {
        m.joined = true;
        m.ready = true;
    }
    w.activate(&c, 0, &mut vec![]);
    (c, w)
}
fn prepare(w: &mut World, c: &Config, i: usize, j: usize) -> Events {
    w.action(
        i,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[j].entity),
        },
        "unit",
        c,
        0,
    )
    .unwrap()
}
fn commit(w: &mut World, c: &Config, i: usize, now: u64) -> Events {
    let p = w.members[i].pending.take().unwrap();
    let mut e = vec![];
    w.commit(i, p, c, now, &mut e);
    e
}
fn shot(w: &mut World, c: &Config, i: usize, j: usize) {
    prepare(w, c, i, j);
    commit(w, c, i, 120);
}
#[test]
fn unified_entities_relations_one_two_four_eight_and_full_identity() {
    for n in [1, 2, 4, 8] {
        let (_, w) = setup(n);
        for i in 0..n {
            let a = &w.members[i].reference.entity;
            assert_eq!(w.resolve_entity(a), Some(Owner::Member(i)));
            assert_eq!(w.relation(a, a), Some(Relation::SelfTarget));
            for j in 0..n {
                if i != j {
                    assert_eq!(
                        w.relation(a, &w.members[j].reference.entity),
                        Some(Relation::Friendly)
                    );
                }
            }
            assert_eq!(w.relation(a, &w.npcs[0].entity), Some(Relation::Hostile));
            let mut stale = a.clone();
            stale.generation += 1;
            assert_eq!(w.resolve_entity(&stale), None);
            stale.generation = 1;
            stale.kind = StatusEntityKind::Monster;
            assert_eq!(w.resolve_entity(&stale), None);
        }
        assert_eq!(
            w.relation(&w.npcs[0].entity, &w.npcs[1].entity),
            Some(Relation::Friendly)
        );
    }
}
#[test]
fn unified_damage_rejects_friendly_and_stale_or_dead_sources() {
    let (c, mut w) = setup(2);
    let source = w.members[0].reference.entity.clone();
    let target = w.members[1].reference.entity.clone();
    assert_eq!(
        w.resolve_damage(
            DamageIntent {
                ignore_armor: false,
                source: source.clone(),
                target,
                raw: 1000,
                direct: true
            },
            &c,
            0,
            &mut vec![]
        )
        .hp_damage,
        0
    );
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    let hp = w.npcs[0].hp;
    w.resolve_damage(
        DamageIntent {
            ignore_armor: false,
            source,
            target: w.npcs[0].entity.clone(),
            raw: 1000,
            direct: true,
        },
        &c,
        30,
        &mut vec![],
    );
    assert_eq!(w.npcs[0].hp, hp);
}
#[test]
fn unified_damage_and_hot_share_actual_entity_base_and_preserve_formula() {
    let (c, mut w) = setup(2);
    let target = w.members[1].reference.entity.clone();
    let r = w.resolve_damage(
        DamageIntent {
            ignore_armor: false,
            source: w.npcs[0].entity.clone(),
            target: target.clone(),
            raw: 300,
            direct: true,
        },
        &c,
        30,
        &mut vec![],
    );
    assert_eq!(r.hp_damage, 200);
    assert_eq!(w.entity_max_hp(&target, &c, 0), Some(1100));
    assert_eq!(w.restore_hp(&target, 5000, &c, 0), 200);
    assert_eq!(w.members[1].hp, 1100);
}

#[test]
fn member_item_damage_reduction_runs_after_armor_before_shields_with_seeded_rounding() {
    let (mut c, mut w) = setup(2);
    c.mechanism_version = 67;
    let add = |id, bps| {
        serde_json::from_value(serde_json::json!({
            "id":id,"mechanism":"timed_attribute_modifier","attribute":"damage_reduction",
            "modifier_bps":bps,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true,
        }))
        .unwrap()
    };
    c.pve_mut()
        .status_definitions
        .extend([add("guard-a", 2500), add("guard-b", 3200)]);
    let target = w.members[0].reference.entity.clone();
    let max = w.members[0].facts.hp_points;
    let source = w.npcs[0].entity.clone();
    let apply = |w: &mut World, ids: &[&str]| {
        w.apply_statuses(
            0,
            (target.clone(), max),
            &ids.iter().map(|id| (*id).into()).collect::<Vec<_>>(),
            &c,
            0,
            &mut vec![],
        );
    };
    let hit = |w: &mut World, raw, direct, now| {
        w.resolve_damage(
            DamageIntent {
                ignore_armor: false,
                source: source.clone(),
                target: target.clone(),
                raw,
                direct,
            },
            &c,
            now,
            &mut vec![],
        )
    };
    apply(&mut w, &["guard-a", "guard-b", "ward"]);
    let shield = w.statuses.shield_total(&status_entity(&target), 0);
    let rng = w.rng;
    let direct = hit(&mut w, 150, true, 30);
    assert_eq!((direct.hp_damage, direct.absorbed), (0, 50));
    assert_eq!(
        w.statuses.shield_total(&status_entity(&target), 30),
        shield - 50
    );
    assert_eq!(w.rng, rng);
    let periodic = hit(&mut w, 150, false, 60);
    assert_eq!(periodic.hp_damage + periodic.absorbed, 100);
    let expired = hit(&mut w, 150, true, 300);
    assert_eq!(expired.hp_damage + expired.absorbed, 100);
    assert_eq!(w.rng, rng);

    for (seed, expected) in [(1, 76), (12345, 75)] {
        let (_, mut w) = setup(2);
        apply(&mut w, &["guard-a"]);
        w.rng = seed;
        let rolled = hit(&mut w, 152, true, 30);
        assert_eq!((rolled.hp_damage, rolled.absorbed), (expected, 0));
        assert_ne!(w.rng, seed);
    }
}
#[test]
fn unit_accepts_published_npc_and_exposes_only_own_pending_target() {
    let (c, mut w) = setup(2);
    prepare(&mut w, &c, 0, 0);
    let a = w.snapshot(0, &c, Phase::Active, 0);
    let b = w.snapshot(1, &c, Phase::Active, 0);
    assert_eq!(
        a.own.pending_action.unwrap().unit_target,
        Some(World::unit_wire(&w.npcs[0].entity))
    );
    assert!(b.own.pending_action.is_none());
    assert!(
        serde_json::to_value(a.members)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m.get("pending_action").is_none())
    );
}
#[test]
fn unit_accept_rejects_unknown_zero_stale_friendly_dead_and_returning() {
    let (c, mut w) = setup(2);
    for target in [
        UnitTarget {
            id: "missing".into(),
            generation: 1,
        },
        UnitTarget {
            id: w.npcs[0].entity.id.clone(),
            generation: 0,
        },
        UnitTarget {
            id: w.npcs[0].entity.id.clone(),
            generation: 2,
        },
        UnitTarget {
            id: "P01".into(),
            generation: 1,
        },
    ] {
        assert!(
            w.action(
                0,
                &Action::CastUnit {
                    skill_id: c.skill.id.clone(),
                    target
                },
                "bad",
                &c,
                0
            )
            .is_err()
        );
    }
    w.npcs[0].returning = true;
    let t = World::unit_wire(&w.npcs[0].entity);
    assert_eq!(
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: t.clone()
            },
            "bad",
            &c,
            0
        ),
        Err(Error::InvalidTarget)
    );
    w.npcs[0].returning = false;
    w.npcs[0].hp = 0;
    assert_eq!(
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: t
            },
            "bad",
            &c,
            0
        ),
        Err(Error::InvalidTarget)
    );
    assert!(w.members[0].pending.is_none());
    assert!(w.statuses.is_empty());
}
#[test]
fn unit_wrong_input_kind_and_wrong_skill_do_not_create_plan() {
    let (c, mut w) = setup(1);
    assert_eq!(
        w.action(
            0,
            &Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.]
            },
            "bad",
            &c,
            0
        ),
        Err(Error::InvalidTarget)
    );
    let t = World::unit_wire(&w.npcs[0].entity);
    assert_eq!(
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.additional_skills[0].id.clone(),
                target: t
            },
            "bad",
            &c,
            0
        ),
        Err(Error::InvalidTarget)
    );
}
#[test]
fn unit_range_and_overlap_rejected_before_prepare() {
    let (c, mut w) = setup(1);
    w.npcs[0].position = [1200., 1200.];
    let t = World::unit_wire(&w.npcs[0].entity);
    assert_eq!(
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: t.clone()
            },
            "bad",
            &c,
            0
        ),
        Err(Error::OutOfRange)
    );
    w.npcs[0].position = w.members[0].position;
    assert_eq!(
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: t
            },
            "bad",
            &c,
            0
        ),
        Err(Error::OutOfRange)
    );
}
#[test]
fn unit_commit_revalidates_no_fee_cooldown_self_status_or_shot() {
    for mode in 0..4 {
        let (c, mut w) = setup(1);
        prepare(&mut w, &c, 0, 0);
        let spirit = w.members[0].spirit;
        match mode {
            0 => w.npcs[0].hp = 0,
            1 => w.npcs[0].returning = true,
            2 => w.npcs[0].entity.generation += 1,
            _ => w.npcs[0].position = [1200., 1200.],
        };
        let e = commit(&mut w, &c, 0, 120);
        assert_eq!(w.members[0].spirit, spirit);
        assert!(w.members[0].cooldowns.is_empty());
        assert!(w.statuses.is_empty());
        assert!(w.shots.is_empty());
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected { .. }
            }
        )));
    }
}
#[test]
fn unit_commit_aims_at_current_target_then_freezes_direction_and_attack() {
    let (c, mut w) = setup(1);
    prepare(&mut w, &c, 0, 0);
    w.npcs[0].position = [460., 440.];
    let e = commit(&mut w, &c, 0, 120);
    let direction = w.shots[0].direction;
    assert_eq!(direction, normalize([60., 40.]));
    let raw = w.shots[0].raw;
    w.members[0].facts.atk_points = 9999;
    w.npcs[0].position = [460., 500.];
    w.advance_shots(&c, 150, 0.03, &mut vec![]);
    assert_eq!(w.shots[0].direction, direction);
    assert_eq!(w.shots[0].raw, raw);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ActionCommitted {
                unit_target: Some(_),
                spirit_cost: 12,
                ..
            }
        }
    )));
}
#[test]
fn unit_intervening_npc_blocks_without_damage_rng_status_or_spirit() {
    let (c, mut w) = setup(1);
    w.npcs[0].position = [600., 400.];
    w.npcs[1].position = [500., 400.];
    shot(&mut w, &c, 0, 0);
    let before = (
        w.npcs[0].hp,
        w.npcs[1].hp,
        w.rng,
        w.members[0].spirit,
        w.statuses.entries().to_vec(),
    );
    let mut e = vec![];
    w.advance_shots(&c, 150, 1., &mut e);
    assert!(w.shots.is_empty());
    assert_eq!(
        (
            w.npcs[0].hp,
            w.npcs[1].hp,
            w.rng,
            w.members[0].spirit,
            w.statuses.entries().to_vec()
        ),
        before
    );
    assert!(e.iter().all(|(_, e)| !matches!(e, EventV4::Damage { .. })));
}
#[test]
fn unit_hits_only_selected_npc_and_applies_existing_dot_shield_bindings() {
    let (c, mut w) = setup(1);
    shot(&mut w, &c, 0, 0);
    let other = w.npcs[1].hp;
    w.advance_shots(&c, 150, 1., &mut vec![]);
    assert!(w.npcs[0].hp < w.npcs[0].profile.stats["hp"]);
    assert_eq!(w.npcs[1].hp, other);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.target == status_entity(&w.npcs[0].entity) && s.periodic.is_some())
    );
}
#[test]
fn unit_full_absorption_still_applies_hit_bindings() {
    let (mut c, mut w) = setup(1);
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
    c.pve_mut().status_definitions[2].shield_bps = Some(10000);
    let t = w.npcs[0].entity.clone();
    w.apply_owned_statuses(
        Owner::Npc(0),
        (t, 1800),
        &["ward".into()],
        &c,
        0,
        &mut vec![],
    );
    shot(&mut w, &c, 0, 0);
    let before = w.npcs[0].hp;
    let mut e = vec![];
    w.advance_shots(&c, 150, 1., &mut e);
    assert_eq!(w.npcs[0].hp, before);
    assert!(
        e.iter()
            .any(|(_, e)| matches!(e, EventV4::Damage { amount: 0, .. }))
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.target == status_entity(&w.npcs[0].entity) && s.periodic.is_some())
    );
}
#[test]
fn unit_dead_returning_and_new_generation_target_end_inflight_without_retarget() {
    for mode in 0..3 {
        let (c, mut w) = setup(1);
        shot(&mut w, &c, 0, 0);
        match mode {
            0 => w.npcs[0].hp = 0,
            1 => w.npcs[0].returning = true,
            _ => w.npcs[0].entity.generation += 1,
        };
        let before = w.members[0].spirit;
        w.advance_shots(&c, 150, 1., &mut vec![]);
        assert!(w.shots.is_empty());
        assert_eq!(w.members[0].spirit, before);
    }
}
#[test]
fn unit_target_can_move_off_frozen_path_without_retargeting() {
    let (c, mut w) = setup(1);
    shot(&mut w, &c, 0, 0);
    w.npcs[0].position = [460., 700.];
    let before = w.npcs[0].hp;
    w.advance_shots(&c, 150, 1., &mut vec![]);
    assert_eq!(w.npcs[0].hp, before);
    assert!(w.shots.is_empty());
}
#[test]
fn unit_prepare_cancel_and_protection_do_not_pay_or_spawn() {
    for protect in [false, true] {
        let (c, mut w) = setup(2);
        prepare(&mut w, &c, 0, 0);
        let before = w.members[0].spirit;
        if protect {
            w.protect(0, 0, &mut vec![])
        } else {
            w.action(0, &Action::CancelAction {}, "cancel", &c, 30)
                .unwrap();
        }
        assert!(w.members[0].pending.is_none());
        assert_eq!(w.members[0].spirit, before);
        assert!(w.shots.is_empty());
        assert!(w.members[1].eligible());
    }
}
#[test]
fn unit_protection_preserves_committed_plan_while_source_exit_clears_it() {
    let (c, mut w) = setup(2);
    shot(&mut w, &c, 0, 0);
    let id = w.shots[0].view.id.clone();
    w.protect(0, 0, &mut vec![]);
    assert_eq!(w.shots[0].view.id, id);
    w.advance_shots(&c, 150, 0.001, &mut vec![]);
    assert_eq!(w.shots[0].view.id, id);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert!(w.shots.is_empty());
    assert!(w.members[1].living());
}
#[test]
fn unit_ttl_distance_close_and_absolute_terminal_release_plans() {
    for mode in 0..3 {
        let (c, mut w) = setup(1);
        shot(&mut w, &c, 0, 0);
        if mode == 0 {
            let end = w.shots[0].expires;
            w.advance_shots(&c, end, 0.03, &mut vec![]);
        } else if mode == 1 {
            w.npcs[0].position = [460., 1000.];
            w.shots[0].remaining = 1.;
            w.advance_shots(&c, 150, 0.03, &mut vec![]);
        } else {
            w.tick(&c, c.terminal_ms);
        }
        assert!(w.shots.is_empty());
    }
}
#[test]
fn unit_npc_reset_invalidates_frozen_target_without_using_new_identity() {
    let (c, mut w) = setup(1);
    shot(&mut w, &c, 0, 0);
    let old = w.npcs[0].entity.clone();
    w.npcs[0].returning = true;
    w.npcs[0].position = w.npcs[0].home;
    w.advance_npcs(&c, 150, 0.03, &mut vec![]);
    assert_ne!(w.npcs[0].entity, old);
    w.advance_shots(&c, 180, 0.03, &mut vec![]);
    assert!(w.shots.iter().all(|s| s.target.is_none()));
}
#[test]
fn unit_repeated_hits_cannot_double_defeat_drop_or_credit() {
    let (c, mut w) = setup(2);
    w.npcs[0].hp = 1;
    shot(&mut w, &c, 0, 0);
    shot(&mut w, &c, 1, 0);
    let mut e = vec![];
    w.advance_shots(&c, 150, 1., &mut e);
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(e, EventV4::MonsterDefeated { .. }))
            .count(),
        1
    );
    assert_eq!(
        w.members.iter().flat_map(|m| m.tally.values()).sum::<u32>(),
        1
    );
    assert!(w.shots.is_empty());
}
#[test]
fn unit_capacity_and_missing_spirit_at_commit_leave_no_partial_effect() {
    for capacity in [false, true] {
        let (mut c, mut w) = setup(1);
        prepare(&mut w, &c, 0, 0);
        if capacity {
            c.members.as_mut().unwrap().projectile_capacity = 0;
        } else {
            w.members[0].spirit = 0;
        }
        let before = w.members[0].spirit;
        commit(&mut w, &c, 0, 120);
        assert_eq!(w.members[0].spirit, before);
        assert!(w.members[0].cooldowns.is_empty());
        assert!(w.statuses.is_empty());
        assert!(w.shots.is_empty());
    }
}
#[test]
fn unit_projectile_view_names_target_ordinary_projectile_view_omits_it() {
    let (c, mut w) = setup(1);
    shot(&mut w, &c, 0, 0);
    let v = serde_json::to_value(w.snapshot(0, &c, Phase::Active, 120)).unwrap();
    assert_eq!(
        v["projectiles"][0]["unit_target"],
        serde_json::to_value(World::unit_wire(&w.npcs[0].entity)).unwrap()
    );
    let mut bad = v.clone();
    bad["projectiles"][0]["unit_target"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SnapshotV4>(bad).is_err());
    let mut ordinary = w.shots[0].view.clone();
    ordinary.unit_target = None;
    assert!(
        serde_json::to_value(ordinary)
            .unwrap()
            .get("unit_target")
            .is_none()
    );
}
#[test]
fn unit_vision_accept_and_commit_revalidation_are_atomic() {
    for at_commit in [false, true] {
        let (c, mut w) = setup(1);
        if at_commit {
            prepare(&mut w, &c, 0, 0);
        }
        Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
        if at_commit {
            let before = w.members[0].spirit;
            let e = commit(&mut w, &c, 0, 120);
            assert!(e.iter().any(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::ActionRejected {
                        reason: Error::OutOfRange,
                        ..
                    }
                }
            )));
            assert_eq!(w.members[0].spirit, before);
            assert!(w.shots.is_empty());
            assert!(w.statuses.is_empty());
        } else {
            let target = w.npcs[0].entity.clone();
            assert_eq!(w.unit_aim(0, &c.skill, &target), Err(Error::OutOfRange));
        }
    }
}
#[test]
fn unit_wall_wins_simultaneous_contact_without_random_or_hit_side_effects() {
    let (c, mut w) = setup(1);
    shot(&mut w, &c, 0, 0);
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_projectiles = true;
    w.npcs[0].position = w.members[0].position;
    let before = (w.npcs[0].hp, w.rng, w.statuses.entries().to_vec());
    let mut e = vec![];
    w.advance_shots(&c, 150, 1., &mut e);
    assert_eq!((w.npcs[0].hp, w.rng, w.statuses.entries().to_vec()), before);
    assert!(w.shots.is_empty());
    assert!(e.iter().all(|(_, e)| !matches!(e, EventV4::Damage { .. })));
}
