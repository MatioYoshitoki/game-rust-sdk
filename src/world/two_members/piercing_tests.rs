use super::*;
fn setup(n: usize, repeat: bool) -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(n);
    let cat = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/member-piercing-host/catalog.json"),
    )
    .unwrap();
    c.mechanism_version = 43;
    c.members.as_mut().unwrap().area_capacity = Some(8);
    c.members.as_mut().unwrap().projectile_capacity = 16;
    c.skill = if repeat {
        cat.active("eight").unwrap().skill.clone()
    } else {
        cat.default_config().skill.clone()
    };
    if repeat {
        w.npcs[0].position = [410., 400.];
    }
    w.npcs[1].position = [1000., 1000.];
    (c, w)
}
fn cast(w: &mut World, c: &Config, i: usize, direction: [f64; 2]) -> Result<Events> {
    w.action(
        i,
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction,
        },
        "private-input",
        c,
        0,
    )
}
fn commit(w: &mut World, c: &Config, i: usize) -> Events {
    let p = w.members[i].pending.take().unwrap();
    let mut e = vec![];
    w.commit(i, p, c, 120, &mut e);
    e
}
fn shot(w: &mut World, c: &Config, i: usize) {
    cast(w, c, i, [1., 0.]).unwrap();
    commit(w, c, i);
}
fn advance(w: &mut World, c: &Config, now: u64, dt: f64) -> Events {
    let mut e = vec![];
    w.advance_shots(c, now, dt, &mut e);
    e
}
fn contacts(e: &Events) -> Vec<String> {
    e.iter()
        .filter_map(|(_, e)| {
            if let EventV4::ProjectileContact { target, .. } = e {
                Some(target.id.clone())
            } else {
                None
            }
        })
        .collect()
}
fn ended(e: &Events, why: ProjectileEndReasonV4) -> usize {
    e.iter()
        .filter(|(_, e)| matches!(e,EventV4::ProjectileEnded{reason:Some(r),..}if *r==why))
        .count()
}
fn wall(w: &mut World, x: usize, y: usize) {
    let p = Arc::make_mut(&mut w.grid.config);
    let mut tile = p.palette[0].clone();
    tile.blocks_projectiles = true;
    p.palette.push(tile);
    Arc::make_mut(&mut p.cells)[y * p.width + x] = p.palette.len() - 1;
    w.grid = super::super::pve::Grid::new(&w.grid.config);
}
#[test]
fn piercing_one_two_four_eight_actual_sources_and_independent_costs() {
    for n in [1, 2, 4, 8] {
        let (c, mut w) = setup(n, false);
        let before = w.members.iter().map(|m| m.spirit).collect::<Vec<_>>();
        for i in 0..n {
            shot(&mut w, &c, i);
        }
        let s = w.snapshot(0, &c, Phase::Active, 120);
        assert_eq!(s.projectiles.len(), n);
        for (i, p) in s.projectiles.iter().enumerate() {
            assert_eq!(p.source, w.members[i].reference.entity);
            assert!(p.piercing.is_some() && p.unit_target.is_none());
            assert_eq!(w.members[i].spirit, before[i] - c.skill.cost);
        }
    }
}
#[test]
fn piercing_accept_direction_kind_and_origin_are_strict() {
    let (c, mut w) = setup(1, false);
    for d in [[0., 0.], [f64::NAN, 0.], [f64::INFINITY, 1.]] {
        assert_eq!(cast(&mut w, &c, 0, d), Err(Error::InvalidMessage));
    }
    for a in [
        Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: [460., 400.],
        },
        Action::CastSelf {
            skill_id: c.skill.id.clone(),
        },
        Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        },
    ] {
        assert_eq!(w.action(0, &a, "bad", &c, 0), Err(Error::InvalidTarget));
    }
    cast(&mut w, &c, 0, [100., 0.]).unwrap();
    assert_eq!(w.members[0].pending.as_ref().unwrap().direction, [1., 0.]);
    w.members[0].pending = None;
    w.members[0].position = [1., 400.];
    assert_eq!(cast(&mut w, &c, 0, [1., 0.]), Err(Error::InvalidTarget));
    assert!(w.shots.is_empty() && w.members[0].cooldowns.is_empty());
}
#[test]
fn piercing_commit_origin_resource_status_and_shared_capacity_fail_atomically() {
    for mode in 0..5 {
        let (mut c, mut w) = setup(1, false);
        cast(&mut w, &c, 0, [1., 0.]).unwrap();
        match mode {
            0 => w.members[0].position = [1., 400.],
            1 => Arc::make_mut(&mut w.grid.config).palette[0].blocks_projectiles = true,
            2 => w.members[0].spirit = 0,
            3 => c.members.as_mut().unwrap().projectile_capacity = 0,
            _ => {
                c.pve_mut()
                    .status_rules
                    .as_mut()
                    .unwrap()
                    .max_active_per_entity = 0
            }
        };
        let before = w.members[0].spirit;
        let e = commit(&mut w, &c, 0);
        assert_eq!(w.members[0].spirit, before);
        assert!(w.shots.is_empty() && w.statuses.is_empty() && w.members[0].cooldowns.is_empty());
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected { .. }
            }
        )));
    }
}
#[test]
fn piercing_shared_slot_race_and_self_or_area_skills_are_independent() {
    let (mut c, mut w) = setup(2, false);
    c.members.as_mut().unwrap().projectile_capacity = 1;
    cast(&mut w, &c, 0, [1., 0.]).unwrap();
    cast(&mut w, &c, 1, [1., 0.]).unwrap();
    commit(&mut w, &c, 0);
    let before = w.members[1].spirit;
    commit(&mut w, &c, 1);
    assert_eq!(w.shots.len(), 1);
    assert_eq!(w.members[1].spirit, before);
    let id = c.additional_skills[0].id.clone();
    w.action(1, &Action::CastSelf { skill_id: id }, "self", &c, 0)
        .unwrap();
    commit(&mut w, &c, 1);
    assert_eq!(w.shots.len(), 1);
    let cat = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/member-area-host/catalog.json"),
    )
    .unwrap();
    c.skill = cat.default_config().skill.clone();
    w.members[1].recovery_until = 0;
    w.action(
        1,
        &Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: [460., 400.],
        },
        "circle",
        &c,
        0,
    )
    .unwrap();
    commit(&mut w, &c, 1);
    assert_eq!(w.areas.len(), 1);
    assert_eq!(w.shots.len(), 1);
}
#[test]
fn piercing_commit_step_never_moves_or_contacts() {
    let (c, mut w) = setup(1, true);
    shot(&mut w, &c, 0);
    let p = w.shots[0].view();
    assert!(advance(&mut w, &c, 120, 1.).is_empty());
    assert_eq!(w.shots[0].view(), p);
}
#[test]
fn piercing_high_speed_sweep_hits_multiple_enemies_without_tunneling() {
    let (mut c, mut w) = setup(1, false);
    c.skill.speed = 10000.;
    w.npcs[1].position = [520., 400.];
    shot(&mut w, &c, 0);
    let e = advance(&mut w, &c, 150, 0.03);
    assert_eq!(
        contacts(&e),
        w.npcs
            .iter()
            .map(|n| n.entity.id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(w.shots[0].view().piercing.unwrap().used_hits, 2);
    assert!(w.npcs.iter().all(|n| n.hp < 1800));
}
#[test]
fn piercing_contacts_order_by_swept_progress_then_stable_identity() {
    for tie in [false, true] {
        let (c, mut w) = setup(1, false);
        w.npcs[0].position = if tie { [460., 400.] } else { [500., 400.] };
        w.npcs[1].position = [460., 400.];
        shot(&mut w, &c, 0);
        let e = advance(&mut w, &c, 150, 0.5);
        let ids = contacts(&e);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], w.npcs[usize::from(!tie)].entity.id);
    }
}
#[test]
fn piercing_wall_and_unit_tie_is_wall_first_but_front_target_can_contact() {
    for front in [false, true] {
        let (mut c, mut w) = setup(1, false);
        c.skill.radius = 6.;
        let size = w.grid.config.cell_size;
        let x = (512. / size) as usize;
        let y = (400. / size) as usize;
        let face = x as f64 * size;
        wall(&mut w, x, y);
        w.npcs[0].position = [
            face + w.npcs[0].profile.combat.radius - if front { 0.01 } else { 0. },
            400.,
        ];
        shot(&mut w, &c, 0);
        let e = advance(&mut w, &c, 150, 1.);
        assert_eq!(contacts(&e).len(), usize::from(front));
        assert_eq!(ended(&e, ProjectileEndReasonV4::Wall), 1);
        assert!(w.shots.is_empty());
    }
}
#[test]
fn piercing_diagonal_sweep_and_map_boundary_stop_without_far_side_hits() {
    let (mut c, mut w) = setup(1, false);
    c.skill.range = 2000.;
    c.skill.speed = 10000.;
    w.npcs[0].position = [460., 460.];
    w.npcs[1].position = [900., 1000.];
    cast(&mut w, &c, 0, [1., 1.]).unwrap();
    commit(&mut w, &c, 0);
    let e = advance(&mut w, &c, 150, 1.);
    assert_eq!(contacts(&e).len(), 1);
    assert_eq!(ended(&e, ProjectileEndReasonV4::Wall), 1);
    assert!(w.shots.is_empty());
}
#[test]
fn piercing_once_history_survives_exit_reentry_and_allows_new_generation() {
    let (mut c, mut w) = setup(1, true);
    c.skill.speed = 0.1;
    c.skill.projectile_repeat_interval_ms = Some(0);
    shot(&mut w, &c, 0);
    advance(&mut w, &c, 150, 0.03);
    let hp = w.npcs[0].hp;
    w.npcs[0].position = [900., 900.];
    advance(&mut w, &c, 750, 0.03);
    w.npcs[0].position = [410., 400.];
    assert!(contacts(&advance(&mut w, &c, 1350, 0.03)).is_empty());
    assert_eq!(w.npcs[0].hp, hp);
    w.npcs[0].entity.generation += 1;
    assert_eq!(contacts(&advance(&mut w, &c, 1950, 0.03)).len(), 1);
    assert_eq!(w.shots[0].view().piercing.unwrap().contacts.len(), 2);
}
#[test]
fn piercing_repeat_inclusive_clock_once_per_step_and_no_catchup() {
    let (mut c, mut w) = setup(1, true);
    c.skill.speed = 0.1;
    shot(&mut w, &c, 0);
    assert_eq!(contacts(&advance(&mut w, &c, 150, 0.03)).len(), 1);
    assert!(contacts(&advance(&mut w, &c, 749, 0.03)).is_empty());
    assert_eq!(contacts(&advance(&mut w, &c, 750, 0.03)).len(), 1);
    assert_eq!(contacts(&advance(&mut w, &c, 5000, 1.)).len(), 1);
    assert!(contacts(&advance(&mut w, &c, 5000, 1.)).is_empty());
    let p = w.shots[0].view().piercing.unwrap();
    assert_eq!(p.used_hits, 3);
    assert_eq!(p.contacts[0].next_eligible_at_ms, Some(5600));
}
#[test]
fn piercing_elapsed_repeat_without_current_swept_contact_never_hits() {
    let (c, mut w) = setup(1, true);
    shot(&mut w, &c, 0);
    advance(&mut w, &c, 150, 0.03);
    let hp = w.npcs[0].hp;
    w.npcs[0].position = [900., 900.];
    assert!(contacts(&advance(&mut w, &c, 5000, 0.03)).is_empty());
    assert_eq!(w.npcs[0].hp, hp);
    assert_eq!(w.shots[0].view().piercing.unwrap().used_hits, 1);
}
#[test]
fn piercing_dodge_does_not_consume_or_bind_full_shield_does() {
    for dodge in [false, true] {
        let (mut c, mut w) = setup(1, true);
        c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
        c.pve_mut().status_definitions[2].shield_bps = Some(10000);
        if dodge {
            Arc::make_mut(&mut w.npcs[0].profile)
                .stats
                .insert("dodge".into(), 100);
        }
        let t = w.npcs[0].entity.clone();
        w.apply_owned_statuses(
            Owner::Npc(0),
            (t, 1800),
            &["ward".into()],
            &c,
            0,
            &mut vec![],
        );
        shot(&mut w, &c, 0);
        let hp = w.npcs[0].hp;
        advance(&mut w, &c, 150, 0.03);
        assert_eq!(w.npcs[0].hp, hp);
        assert_eq!(
            w.shots[0].view().piercing.unwrap().used_hits,
            u32::from(!dodge)
        );
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .any(|s| s.periodic.is_some() && s.target == status_entity(&w.npcs[0].entity)),
            !dodge
        );
    }
}
#[test]
fn piercing_attack_frozen_and_current_defense_shield_resolved() {
    let (mut c, mut w) = setup(1, true);
    c.pve_mut().status_definitions[2].shield_bps = Some(2000);
    shot(&mut w, &c, 0);
    w.members[0].facts.atk_points = 99999;
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("def".into(), 0);
    let e = advance(&mut w, &c, 150, 0.03);
    assert!(
        e.iter()
            .any(|(_, e)| matches!(e,EventV4::Damage{amount,..}if *amount<1000))
    );
    let t = w.npcs[0].entity.clone();
    w.apply_owned_statuses(
        Owner::Npc(0),
        (t, 1800),
        &["ward".into()],
        &c,
        200,
        &mut vec![],
    );
    let hp = w.npcs[0].hp;
    advance(&mut w, &c, 750, 0.03);
    assert_eq!(w.npcs[0].hp, hp);
}
#[test]
fn piercing_multiple_lethal_flights_credit_and_drop_once() {
    let (c, mut w) = setup(2, false);
    w.npcs[0].hp = 1;
    for i in 0..2 {
        w.members[i].position = [400., 400.];
        shot(&mut w, &c, i);
    }
    let e = advance(&mut w, &c, 150, 0.5);
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
}
#[test]
fn piercing_ttl_deadline_precedes_contact_and_ends_once() {
    let (mut c, mut w) = setup(1, true);
    c.skill.projectile_ttl_ms = 1000;
    shot(&mut w, &c, 0);
    let hp = w.npcs[0].hp;
    let e = advance(&mut w, &c, 1120, 1.);
    assert_eq!(w.npcs[0].hp, hp);
    assert!(contacts(&e).is_empty());
    assert_eq!(ended(&e, ProjectileEndReasonV4::Expired), 1);
    assert!(advance(&mut w, &c, 1150, 1.).is_empty());
}
#[test]
fn piercing_range_exhaustion_and_zero_remaining_never_publish() {
    let (mut c, mut w) = setup(1, false);
    c.skill.range = 10.;
    shot(&mut w, &c, 0);
    let e = advance(&mut w, &c, 150, 1.);
    assert_eq!(ended(&e, ProjectileEndReasonV4::RangeExhausted), 1);
    assert!(w.shots.is_empty());
    assert!(advance(&mut w, &c, 180, 1.).is_empty());
}
#[test]
fn piercing_budget_exhaustion_stops_later_targets_and_ends_once() {
    let (mut c, mut w) = setup(1, false);
    c.skill.max_hits = 1;
    w.npcs[1].position = [520., 400.];
    shot(&mut w, &c, 0);
    let e = advance(&mut w, &c, 150, 1.);
    assert_eq!(contacts(&e), vec![w.npcs[0].entity.id.clone()]);
    assert_eq!(ended(&e, ProjectileEndReasonV4::HitBudgetExhausted), 1);
    assert!(w.shots.is_empty());
    assert_eq!(w.npcs[1].hp, 1800);
}
#[test]
fn piercing_friendly_dead_and_returning_targets_do_not_block() {
    let (c, mut w) = setup(2, false);
    w.members[1].position = [420., 400.];
    w.npcs[0].hp = 0;
    w.npcs[1].position = [480., 400.];
    w.npcs[1].returning = true;
    let own = w.members[1].hp;
    shot(&mut w, &c, 0);
    let e = advance(&mut w, &c, 150, 1.);
    assert!(contacts(&e).is_empty());
    assert_eq!(w.members[1].hp, own);
}
#[test]
fn piercing_background_and_move_keep_flight_budget_and_absolute_deadline() {
    let (c, mut w) = setup(1, true);
    shot(&mut w, &c, 0);
    advance(&mut w, &c, 150, 0.03);
    let before = w.shots[0].view().piercing.unwrap();
    w.protect(0, 0, &mut vec![]);
    w.members[0].position = [600., 600.];
    advance(&mut w, &c, 750, 0.03);
    let after = w.shots[0].view().piercing.unwrap();
    assert_eq!(after.direction, before.direction);
    assert_eq!(after.speed, before.speed);
    assert_eq!(after.expires_at_ms, before.expires_at_ms);
    assert_eq!(after.created_at_ms, before.created_at_ms);
    assert!(after.remaining_range < before.remaining_range && after.used_hits > before.used_hits);
}
#[test]
fn piercing_prepare_cancel_and_source_exit_clean_only_owner() {
    let (c, mut w) = setup(2, true);
    cast(&mut w, &c, 0, [1., 0.]).unwrap();
    let before = w.members[0].spirit;
    w.action(0, &Action::CancelAction {}, "cancel", &c, 0)
        .unwrap();
    assert_eq!(w.members[0].spirit, before);
    assert!(w.shots.is_empty());
    for i in 0..2 {
        shot(&mut w, &c, i);
    }
    let mut e = vec![];
    w.terminal_intent(0, Outcome::Death, &mut e);
    assert_eq!(w.shots.len(), 1);
    assert_eq!(w.shots[0].view.source.id, "P01");
    assert_eq!(ended(&e, ProjectileEndReasonV4::SourceInvalid), 1);
    w.terminal_intent(0, Outcome::Death, &mut e);
    assert_eq!(ended(&e, ProjectileEndReasonV4::SourceInvalid), 1);
}
#[test]
fn piercing_absolute_terminal_clears_all_without_late_contacts() {
    let (c, mut w) = setup(8, true);
    for i in 0..8 {
        shot(&mut w, &c, i);
    }
    let e = w.tick(&c, c.terminal_ms);
    assert!(w.shots.is_empty());
    assert!(contacts(&e).is_empty());
    assert_eq!(ended(&e, ProjectileEndReasonV4::SourceInvalid), 8);
}
#[test]
fn piercing_public_plan_excludes_private_input_attack_cost_and_bindings() {
    let (c, mut w) = setup(1, false);
    shot(&mut w, &c, 0);
    let v = serde_json::to_value(w.snapshot(0, &c, Phase::Active, 120)).unwrap();
    let p = &v["projectiles"][0];
    let text = p.to_string();
    assert!(!text.contains("private-input"));
    for k in ["action_id", "raw", "hit_statuses", "cost"] {
        assert!(p.get(k).is_none() && p["piercing"].get(k).is_none());
    }
    assert_eq!(p["source"]["id"], "P00");
    assert!(p.get("unit_target").is_none());
}
#[test]
fn piercing_each_owner_and_flight_has_independent_history() {
    let (c, mut w) = setup(2, true);
    w.members[1].position = w.members[0].position;
    for i in 0..2 {
        shot(&mut w, &c, i);
    }
    let e = advance(&mut w, &c, 150, 0.03);
    assert_eq!(contacts(&e).len(), 2);
    assert!(
        w.shots
            .iter()
            .all(|s| s.view().piercing.unwrap().used_hits == 1)
    );
    w.members[0].cooldowns.clear();
    w.members[0].recovery_until = 0;
    shot(&mut w, &c, 0);
    let e = advance(&mut w, &c, 180, 0.03);
    assert_eq!(contacts(&e).len(), 1);
    assert!(
        w.shots
            .iter()
            .all(|s| s.view().piercing.unwrap().used_hits == 1)
    );
}
