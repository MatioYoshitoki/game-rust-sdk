use super::*;
fn setup(n: usize) -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(n);
    let cat = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/member-line-host/catalog.json"),
    )
    .unwrap();
    let sample = cat.default_config();
    c.mechanism_version = 42;
    c.members.as_mut().unwrap().area_capacity = Some(8);
    c.skill = sample.skill.clone();
    w.npcs[1].position = [1000., 1000.];
    (c, w)
}
fn cast(w: &mut World, c: &Config, i: usize, p: [f64; 2]) -> Result<Events> {
    w.action(
        i,
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [
                p[0] - w.members[i].position[0],
                p[1] - w.members[i].position[1],
            ],
        },
        "private-input",
        c,
        0,
    )
}
fn commit(w: &mut World, c: &Config, i: usize, now: u64) -> Events {
    let p = w.members[i].pending.take().unwrap();
    let mut e = vec![];
    w.commit(i, p, c, now, &mut e);
    e
}
fn plan(w: &mut World, c: &Config, i: usize, p: [f64; 2]) {
    cast(w, c, i, p).unwrap();
    commit(w, c, i, 120);
}
fn advance(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.advance_areas(c, now, &mut e);
    e
}
#[test]
fn line_one_two_four_eight_share_world_capacity_and_actual_sources() {
    for n in [1, 2, 4, 8] {
        let (c, mut w) = setup(n);
        let p = w.npcs[0].position;
        for i in 0..n {
            plan(&mut w, &c, i, p);
        }
        let v = w.snapshot(0, &c, Phase::Active, 120).areas.unwrap();
        assert_eq!(v.len(), n);
        for i in 0..n {
            assert!(v.iter().any(|a| a.source.id == format!("P{i:02}")));
        }
        assert_eq!(
            w.members.iter().filter(|m| m.cooldowns.len() == 1).count(),
            n
        );
    }
}
#[test]
fn line_field_first_tick_end_exclusive_and_non_aligned_schedule() {
    let (mut c, mut w) = setup(1);
    c.skill.warning_ms = 11;
    c.skill.tick_interval_ms = 601;
    c.skill.duration_ms = 1400;
    let p = w.npcs[0].position;
    plan(&mut w, &c, 0, p);
    assert!(advance(&mut w, &c, 731).is_empty());
    let e = advance(&mut w, &c, 750);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::AreaTick {
            scheduled_at_ms: 732,
            tick_index: 1,
            ..
        }
    )));
    let e = advance(&mut w, &c, 2000);
    let scheduled = e
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::AreaTick {
                scheduled_at_ms, ..
            } = e
            {
                Some(*scheduled_at_ms)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(scheduled, vec![1333]);
    assert!(w.areas.is_empty());
}
#[test]
fn line_same_due_creation_order_and_catchup_keep_scheduled_timestamps() {
    let (mut c, mut w) = setup(2);
    c.skill.warning_ms = 0;
    c.skill.duration_ms = 1900;
    c.skill.tick_interval_ms = 600;
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 128;
    let p = w.npcs[0].position;
    plan(&mut w, &c, 0, p);
    plan(&mut w, &c, 1, p);
    let ids = w
        .snapshot(0, &c, Phase::Active, 120)
        .areas
        .unwrap()
        .iter()
        .map(|p| p.id.clone())
        .collect::<Vec<_>>();
    let e = advance(&mut w, &c, 2000);
    let ticks = e
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::AreaTick {
                area_id,
                scheduled_at_ms,
                ..
            } = e
            {
                Some((area_id.clone(), *scheduled_at_ms))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ticks,
        vec![
            (ids[0].clone(), 720),
            (ids[1].clone(), 720),
            (ids[0].clone(), 1320),
            (ids[1].clone(), 1320),
            (ids[0].clone(), 1920),
            (ids[1].clone(), 1920)
        ]
    );
}
#[test]
fn line_attack_frozen_but_current_defense_and_shield_are_resolved_per_contact() {
    let (mut c, mut w) = setup(1);
    c.pve_mut().status_definitions[2].shield_bps = Some(2000);
    c.skill.warning_ms = 0;
    let center = w.npcs[0].position;
    plan(&mut w, &c, 0, center);
    w.members[0].facts.atk_points = 99999;
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("def".into(), 0);
    let e = advance(&mut w, &c, 720);
    assert!(
        e.iter()
            .any(|(_, e)| matches!(e,EventV4::Damage{amount,..}if *amount<1000))
    );
    let target = w.npcs[0].entity.clone();
    w.apply_owned_statuses(
        Owner::Npc(0),
        (target, 1800),
        &["ward".into()],
        &c,
        800,
        &mut vec![],
    );
    let hp = w.npcs[0].hp;
    advance(&mut w, &c, 1320);
    assert_eq!(w.npcs[0].hp, hp);
}
#[test]
fn line_zero_repeat_allows_new_generation_but_never_reuses_old_identity() {
    let (mut c, mut w) = setup(1);
    c.skill.warning_ms = 0;
    c.skill.radius = 10.;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    let center = w.npcs[0].position;
    plan(&mut w, &c, 0, center);
    advance(&mut w, &c, 720);
    let hp = w.npcs[0].hp;
    advance(&mut w, &c, 1320);
    assert_eq!(w.npcs[0].hp, hp);
    w.npcs[0].entity.generation += 1;
    advance(&mut w, &c, 1920);
    assert!(w.npcs[0].hp < hp);
    let v = w.snapshot(0, &c, Phase::Active, 1920).areas.unwrap();
    assert_eq!(v[0].contact_policy.as_ref().unwrap().contacts.len(), 2);
}
#[test]
fn line_positive_repeat_uses_scheduled_inclusive_time_not_execution_time() {
    let (mut c, mut w) = setup(1);
    c.skill.warning_ms = 0;
    c.skill.radius = 10.;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 1200;
    let p = w.npcs[0].position;
    plan(&mut w, &c, 0, p);
    let e = advance(&mut w, &c, 2000);
    let counts = e
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::AreaTick { targets, .. } = e {
                Some(targets.len())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(counts, vec![1, 0, 1]);
    let v = w.snapshot(0, &c, Phase::Active, 2000).areas.unwrap();
    assert_eq!(
        v[0].contact_policy.as_ref().unwrap().contacts[0].last_hit_at_ms,
        1920
    );
}
#[test]
fn line_dodge_does_not_consume_budget_full_absorption_does_and_keeps_bindings() {
    for dodge in [false, true] {
        let (mut c, mut w) = setup(1);
        c.skill.warning_ms = 0;
        c.skill.radius = 10.;
        c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
        c.pve_mut().status_definitions[2].shield_bps = Some(10000);
        if dodge {
            Arc::make_mut(&mut w.npcs[0].profile)
                .stats
                .insert("dodge".into(), 100);
        }
        let target = w.npcs[0].entity.clone();
        w.apply_owned_statuses(
            Owner::Npc(0),
            (target, 1800),
            &["ward".into()],
            &c,
            0,
            &mut vec![],
        );
        let p = w.npcs[0].position;
        plan(&mut w, &c, 0, p);
        let hp = w.npcs[0].hp;
        advance(&mut w, &c, 720);
        assert_eq!(w.npcs[0].hp, hp);
        let v = w.snapshot(0, &c, Phase::Active, 720).areas.unwrap();
        assert_eq!(
            v[0].contact_policy.as_ref().unwrap().used_hits,
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
fn line_hit_budget_exhaustion_and_expiry_end_exactly_once() {
    for cap in [1, 128] {
        let (mut c, mut w) = setup(1);
        c.skill.warning_ms = 0;
        c.skill.radius = 10.;
        c.skill.contact_policy.as_mut().unwrap().max_total_hits = cap;
        let p = w.npcs[0].position;
        plan(&mut w, &c, 0, p);
        let mut e = advance(&mut w, &c, 720);
        e.extend(advance(&mut w, &c, 5000));
        assert_eq!(
            e.iter()
                .filter(|(_, e)| matches!(e, EventV4::AreaEnded { .. }))
                .count(),
            1
        );
        assert!(w.areas.is_empty());
        assert!(advance(&mut w, &c, 10000).is_empty());
    }
}
#[test]
fn line_prepare_cancel_and_protect_leave_cost_and_plan_unchanged() {
    for protect in [false, true] {
        let (c, mut w) = setup(1);
        cast(&mut w, &c, 0, [460., 400.]).unwrap();
        let before = w.members[0].spirit;
        if protect {
            w.protect(0, 0, &mut vec![])
        } else {
            w.action(0, &Action::CancelAction {}, "cancel", &c, 30)
                .unwrap();
        }
        assert!(w.members[0].pending.is_none());
        assert!(w.areas.is_empty());
        assert_eq!(w.members[0].spirit, before);
    }
}
#[test]
fn line_source_exit_cleans_only_owner_without_waiting_for_finance() {
    let (c, mut w) = setup(8);
    let p = w.npcs[0].position;
    for i in 0..8 {
        plan(&mut w, &c, i, p);
    }
    let mut e = vec![];
    w.terminal_intent(0, Outcome::Death, &mut e);
    assert_eq!(w.areas.len(), 7);
    assert!(
        w.snapshot(1, &c, Phase::Active, 120)
            .areas
            .unwrap()
            .iter()
            .all(|a| a.source.id != "P00")
    );
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::AreaEnded {
                    reason: AreaEndReason::SourceInvalid,
                    ..
                }
            ))
            .count(),
        1
    );
}
#[test]
fn line_absolute_terminal_clears_all_without_late_contacts() {
    let (c, mut w) = setup(2);
    let p = w.npcs[0].position;
    plan(&mut w, &c, 0, p);
    plan(&mut w, &c, 1, p);
    let e = w.tick(&c, c.terminal_ms);
    assert!(w.areas.is_empty());
    assert!(
        e.iter()
            .all(|(_, e)| !matches!(e, EventV4::AreaTick { .. }))
    );
}
#[test]
fn line_public_plan_uses_kernel_id_and_never_exposes_private_request_or_stats() {
    let (c, mut w) = setup(2);
    plan(&mut w, &c, 0, [460., 400.]);
    let v = serde_json::to_value(w.snapshot(1, &c, Phase::Active, 120)).unwrap();
    let text = serde_json::to_string(&v["areas"]).unwrap();
    assert!(!text.contains("private-input"));
    for key in ["raw", "hit_statuses", "spirit", "cooldowns", "action_id"] {
        assert!(v["areas"][0].get(key).is_none());
    }
    assert_eq!(v["areas"][0]["source"]["id"], "P00");
    assert!(v["self"]["pending_action"].is_null());
}

#[test]
fn line_accept_direction_and_origin_are_strict_without_ground_or_vision_gate() {
    let (c, mut w) = setup(1);
    for direction in [[0., 0.], [f64::NAN, 1.], [f64::INFINITY, 0.]] {
        assert_eq!(
            w.action(
                0,
                &Action::Cast {
                    skill_id: c.skill.id.clone(),
                    direction
                },
                "bad",
                &c,
                0
            ),
            Err(Error::InvalidMessage)
        );
    }
    for action in [
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
        assert_eq!(
            w.action(0, &action, "wrong", &c, 0),
            Err(Error::InvalidTarget)
        );
    }
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
    cast(&mut w, &c, 0, [460., 400.]).unwrap();
    assert_eq!(w.members[0].pending.as_ref().unwrap().direction, [1., 0.]);
    w.members[0].pending = None;
    w.members[0].position = [1., 400.];
    assert_eq!(cast(&mut w, &c, 0, [460., 400.]), Err(Error::InvalidTarget));
    assert!(w.members[0].cooldowns.is_empty());
}
#[test]
fn line_commit_revalidates_origin_cost_status_and_capacity_atomically() {
    for mode in 0..5 {
        let (mut c, mut w) = setup(1);
        cast(&mut w, &c, 0, [460., 400.]).unwrap();
        match mode {
            0 => w.members[0].position = [1., 400.],
            1 => Arc::make_mut(&mut w.grid.config).palette[0].blocks_projectiles = true,
            2 => w.members[0].spirit = 0,
            3 => c.members.as_mut().unwrap().area_capacity = Some(0),
            _ => {
                c.pve_mut()
                    .status_rules
                    .as_mut()
                    .unwrap()
                    .max_active_per_entity = 0
            }
        };
        let spirit = w.members[0].spirit;
        let e = commit(&mut w, &c, 0, 120);
        assert!(
            w.areas.is_empty()
                && w.shots.is_empty()
                && w.statuses.is_empty()
                && w.members[0].cooldowns.is_empty()
        );
        assert_eq!(w.members[0].spirit, spirit);
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected { .. }
            }
        )));
    }
}
#[test]
fn line_commit_freezes_current_origin_direction_and_background_geometry() {
    let (c, mut w) = setup(1);
    cast(&mut w, &c, 0, [460., 400.]).unwrap();
    w.members[0].position = [420., 420.];
    commit(&mut w, &c, 0, 120);
    let before = w.areas[0].view();
    let line = before.line.as_ref().unwrap();
    assert_eq!(line.start, [420., 420.]);
    assert_eq!(line.end, [420. + c.skill.range, 420.]);
    w.members[0].position = [500., 500.];
    w.protect(0, 0, &mut vec![]);
    advance(&mut w, &c, 1000);
    let after = w.areas[0].view();
    assert_eq!(after.line, before.line);
    assert_eq!(after.expires_at_ms, before.expires_at_ms);
    assert!(after.completed_ticks > 0);
}
#[test]
fn line_capsule_swept_wall_boundary_diagonal_and_zero_length_are_shared() {
    let (mut c, mut w) = setup(1);
    c.skill.range = 2000.;
    c.skill.radius = 10.;
    let l = w.line_plan(0, &c.skill, [1., 0.]).unwrap();
    assert_eq!(l.end, [1270., 400.]);
    let d = normalize([1., 1.]);
    let l = w.line_plan(0, &c.skill, d).unwrap();
    assert!((l.end[0] - 1270.).abs() < 1e-8 && (l.end[1] - 1270.).abs() < 1e-8);
    let size = w.grid.config.cell_size;
    let x = (500. / size) as usize;
    let y = (400. / size) as usize;
    let wall = y * w.grid.config.width + x;
    let palette = Arc::make_mut(&mut w.grid.config);
    let mut tile = palette.palette[0].clone();
    tile.blocks_projectiles = true;
    palette.palette.push(tile);
    Arc::make_mut(&mut palette.cells)[wall] = palette.palette.len() - 1;
    w.grid = super::super::pve::Grid::new(&w.grid.config);
    let l = w.line_plan(0, &c.skill, [1., 0.]).unwrap();
    assert!((l.end[0] - (x as f64 * size - 10.)).abs() < 1e-8);
    w.members[0].position = [x as f64 * size - 10., 400.];
    let l = w.line_plan(0, &c.skill, [1., 0.]).unwrap();
    assert_eq!(l.start, l.end);
}
#[test]
fn line_caps_and_sides_include_footprint_and_respect_axis_to_target_walls() {
    let (_, mut w) = setup(1);
    let l = LineFieldView {
        start: [400., 400.],
        end: [500., 400.],
        width: 20.,
    };
    for p in [[390. - 14., 400.], [510. + 14., 400.], [450., 424.]] {
        assert!(w.grid.line_field_contact(&l, p, 14.));
    }
    assert!(!w.grid.line_field_contact(&l, [525., 400.], 14.));
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_projectiles = true;
    assert!(!w.grid.line_field_contact(&l, [450., 420.], 14.));
}
#[test]
fn line_order_is_path_progress_then_axis_distance_then_identity() {
    let (mut c, mut w) = setup(1);
    c.skill.warning_ms = 0;
    c.skill.radius = 50.;
    c.skill
        .contact_policy
        .as_mut()
        .unwrap()
        .max_targets_per_tick = 1;
    for (a, b, winner) in [
        ([450., 440.], [460., 400.], 0),
        ([450., 430.], [450., 400.], 1),
        ([450., 400.], [450., 400.], 0),
    ] {
        w.npcs[0].position = a;
        w.npcs[1].position = b;
        w.areas.clear();
        w.members[0].cooldowns.clear();
        w.members[0].recovery_until = 0;
        w.members[0].spirit = 100;
        plan(&mut w, &c, 0, [500., 400.]);
        let e = advance(&mut w, &c, 720);
        let t = e
            .iter()
            .find_map(|(_, e)| {
                if let EventV4::AreaTick { targets, .. } = e {
                    Some(targets)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(t, &vec![w.npcs[winner].entity.clone()]);
    }
}
#[test]
fn line_targets_enter_leave_return_reset_and_friendly_are_live() {
    let (mut c, mut w) = setup(2);
    c.skill.warning_ms = 0;
    c.skill.radius = 10.;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    w.npcs[1].returning = true;
    w.npcs[0].position = [450., 600.];
    plan(&mut w, &c, 0, [500., 400.]);
    let own = w.members[1].hp;
    advance(&mut w, &c, 720);
    let hp = w.npcs[0].hp;
    w.npcs[0].position = [450., 400.];
    advance(&mut w, &c, 1320);
    assert!(w.npcs[0].hp < hp);
    let hp = w.npcs[0].hp;
    w.npcs[0].position = [450., 600.];
    advance(&mut w, &c, 1920);
    assert_eq!(w.npcs[0].hp, hp);
    w.npcs[0].position = [450., 400.];
    advance(&mut w, &c, 2520);
    assert_eq!(w.npcs[0].hp, hp);
    w.npcs[0].entity.generation += 1;
    advance(&mut w, &c, 3120);
    assert!(w.npcs[0].hp < hp);
    assert_eq!(w.members[1].hp, own);
}
#[test]
fn line_and_circle_share_slots_while_projectiles_and_self_do_not() {
    let (mut c, mut w) = setup(2);
    c.members.as_mut().unwrap().area_capacity = Some(1);
    let p = w.npcs[0].position;
    cast(&mut w, &c, 0, p).unwrap();
    cast(&mut w, &c, 1, p).unwrap();
    commit(&mut w, &c, 0, 120);
    let spirit = w.members[1].spirit;
    commit(&mut w, &c, 1, 120);
    assert_eq!(w.areas.len(), 1);
    assert_eq!(w.members[1].spirit, spirit);
    assert!(w.members[1].cooldowns.is_empty() && w.shots.is_empty());
    let cat = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/member-area-host/catalog.json"),
    )
    .unwrap();
    c.skill = cat.default_config().skill.clone();
    w.action(
        1,
        &Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: p,
        },
        "circle",
        &c,
        0,
    )
    .unwrap();
    commit(&mut w, &c, 1, 120);
    assert_eq!(w.areas.len(), 1);
    assert_eq!(w.members[1].spirit, spirit);
    c.skill = super::combat_tests::setup(2).0.skill;
    w.action(
        1,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        },
        "unit",
        &c,
        0,
    )
    .unwrap();
    commit(&mut w, &c, 1, 120);
    assert_eq!(w.shots.len(), 1);
    assert_eq!(w.areas.len(), 1);
}
#[test]
fn line_overlapping_lethal_contacts_credit_and_drop_once() {
    let (mut c, mut w) = setup(2);
    c.skill.warning_ms = 0;
    w.npcs[0].hp = 1;
    let p = w.npcs[0].position;
    plan(&mut w, &c, 0, p);
    plan(&mut w, &c, 1, p);
    let e = advance(&mut w, &c, 720);
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
fn line_zero_warning_still_waits_interval_and_never_hits_commit_step() {
    let (mut c, mut w) = setup(1);
    c.skill.warning_ms = 0;
    let p = w.npcs[0].position;
    plan(&mut w, &c, 0, p);
    let hp = w.npcs[0].hp;
    assert!(advance(&mut w, &c, 120).is_empty());
    assert!(advance(&mut w, &c, 719).is_empty());
    assert_eq!(w.npcs[0].hp, hp);
    let e = advance(&mut w, &c, 720);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::AreaTick {
            scheduled_at_ms: 720,
            ..
        }
    )));
    assert!(w.npcs[0].hp < hp);
}
