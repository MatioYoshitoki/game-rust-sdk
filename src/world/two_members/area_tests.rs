use super::*;
fn setup(n: usize, field: bool) -> (Config, World) {
    let (mut c, w) = super::combat_tests::setup(n);
    let cat = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/member-area-host/catalog.json"),
    )
    .unwrap();
    let sample = cat.default_config();
    c.mechanism_version = 41;
    c.members.as_mut().unwrap().area_capacity = Some(8);
    c.skill = sample.skill.clone();
    if field {
        let b = cat.active("eight").unwrap();
        c.skill = b.skill.clone();
    }
    (c, w)
}
fn cast(w: &mut World, c: &Config, i: usize, p: [f64; 2]) -> Result<Events> {
    w.action(
        i,
        &Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: p,
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
fn area_one_two_four_eight_share_world_capacity_and_actual_sources() {
    for n in [1, 2, 4, 8] {
        let (c, mut w) = setup(n, false);
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
fn area_accept_rejects_nonfinite_outside_unwalkable_range_and_vision() {
    let (c, mut w) = setup(1, false);
    for p in [[f64::NAN, 400.], [-1., 400.], [1280., 400.], [1200., 1200.]] {
        assert!(cast(&mut w, &c, 0, p).is_err());
    }
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
    assert_eq!(cast(&mut w, &c, 0, [460., 400.]), Err(Error::OutOfRange));
    Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = false;
    Arc::make_mut(&mut w.grid.config).palette[0].walkable = false;
    assert_eq!(cast(&mut w, &c, 0, [460., 400.]), Err(Error::InvalidTarget));
    assert!(w.members[0].pending.is_none());
}
#[test]
fn area_commit_revalidates_position_resource_status_and_slot_atomically() {
    for mode in 0..5 {
        let (mut c, mut w) = setup(1, false);
        cast(&mut w, &c, 0, [460., 400.]).unwrap();
        match mode {
            0 => w.members[0].position = [1200., 1200.],
            1 => Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true,
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
        assert!(w.areas.is_empty());
        assert!(w.shots.is_empty());
        assert!(w.statuses.is_empty());
        assert!(w.members[0].cooldowns.is_empty());
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
fn area_global_slot_race_does_not_charge_loser_or_block_self_and_projectiles() {
    let (mut c, mut w) = setup(2, false);
    c.members.as_mut().unwrap().area_capacity = Some(1);
    let p = w.npcs[0].position;
    cast(&mut w, &c, 0, p).unwrap();
    cast(&mut w, &c, 1, p).unwrap();
    commit(&mut w, &c, 0, 120);
    let before = w.members[1].spirit;
    commit(&mut w, &c, 1, 120);
    assert_eq!(w.areas.len(), 1);
    assert_eq!(w.members[1].spirit, before);
    assert!(w.members[1].cooldowns.is_empty());
    w.action(
        1,
        &Action::CastSelf {
            skill_id: c.additional_skills[0].id.clone(),
        },
        "self",
        &c,
        150,
    )
    .unwrap();
    commit(&mut w, &c, 1, 270);
    assert_eq!(w.areas.len(), 1);
    assert!(
        w.members[1]
            .cooldowns
            .contains_key(&c.additional_skills[0].id)
    );
}
#[test]
fn area_circle_warning_and_zero_warning_resolve_once_after_commit_step() {
    for warning in [0, 701] {
        let (mut c, mut w) = setup(1, false);
        c.skill.warning_ms = warning;
        let p = w.npcs[0].position;
        plan(&mut w, &c, 0, p);
        let hp = w.npcs[0].hp;
        assert!(advance(&mut w, &c, 120).is_empty());
        assert_eq!(w.npcs[0].hp, hp);
        let at = if warning == 0 { 150 } else { 821 };
        let e = advance(&mut w, &c, at);
        assert!(w.npcs[0].hp < hp);
        assert_eq!(
            e.iter()
                .filter(|(_, e)| matches!(e, EventV4::AreaTick { .. }))
                .count(),
            1
        );
        assert!(w.areas.is_empty());
        assert!(advance(&mut w, &c, at + 1000).is_empty());
    }
}
#[test]
fn area_field_first_tick_end_exclusive_and_non_aligned_schedule() {
    let (mut c, mut w) = setup(1, true);
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
fn area_same_due_creation_order_and_catchup_keep_scheduled_timestamps() {
    let (mut c, mut w) = setup(2, true);
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
fn area_empty_and_moving_targets_are_read_at_each_pulse() {
    let (mut c, mut w) = setup(1, true);
    c.skill.radius = 10.;
    c.skill.warning_ms = 0;
    let center = [500., 500.];
    plan(&mut w, &c, 0, center);
    let e = advance(&mut w, &c, 720);
    assert!(
        e.iter()
            .any(|(_, e)| matches!(e,EventV4::AreaTick{targets,..}if targets.is_empty()))
    );
    w.npcs[0].position = center;
    let hp = w.npcs[0].hp;
    advance(&mut w, &c, 1320);
    assert!(w.npcs[0].hp < hp);
    w.npcs[0].position = [700., 700.];
    let hp = w.npcs[0].hp;
    advance(&mut w, &c, 1920);
    assert_eq!(w.npcs[0].hp, hp);
}
#[test]
fn area_circle_edge_footprint_and_projectile_wall_are_shared_geometry() {
    for blocked in [false, true] {
        let (mut c, mut w) = setup(1, false);
        c.skill.warning_ms = 0;
        c.skill.radius = 10.;
        let radius = w.npcs[0].profile.combat.radius;
        w.npcs[0].position = [500. + 10. + radius, 400.];
        plan(&mut w, &c, 0, [500., 400.]);
        if blocked {
            Arc::make_mut(&mut w.grid.config).palette[0].blocks_projectiles = true;
        }
        let hp = w.npcs[0].hp;
        advance(&mut w, &c, 150);
        assert_eq!(w.npcs[0].hp < hp, !blocked);
    }
}
#[test]
fn area_targets_sort_by_center_distance_then_identity_and_obey_per_tick_limit() {
    let (mut c, mut w) = setup(1, true);
    c.skill.warning_ms = 0;
    c.skill.radius = 128.;
    c.skill
        .contact_policy
        .as_mut()
        .unwrap()
        .max_targets_per_tick = 1;
    w.npcs[0].position = [500., 420.];
    w.npcs[1].position = [500., 400.];
    plan(&mut w, &c, 0, [500., 400.]);
    let e = advance(&mut w, &c, 720);
    let targets = e
        .iter()
        .find_map(|(_, e)| {
            if let EventV4::AreaTick { targets, .. } = e {
                Some(targets)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], w.npcs[1].entity);
}
#[test]
fn area_attack_frozen_but_current_defense_and_shield_are_resolved_per_contact() {
    let (mut c, mut w) = setup(1, true);
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
fn area_zero_repeat_allows_new_generation_but_never_reuses_old_identity() {
    let (mut c, mut w) = setup(1, true);
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
fn area_positive_repeat_uses_scheduled_inclusive_time_not_execution_time() {
    let (mut c, mut w) = setup(1, true);
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
fn area_dodge_does_not_consume_budget_full_absorption_does_and_keeps_bindings() {
    for dodge in [false, true] {
        let (mut c, mut w) = setup(1, true);
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
fn area_hit_budget_exhaustion_and_expiry_end_exactly_once() {
    for cap in [1, 128] {
        let (mut c, mut w) = setup(1, true);
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
fn area_returning_dead_and_friendly_targets_are_never_contacted() {
    let (mut c, mut w) = setup(2, false);
    c.skill.radius = 128.;
    c.skill.warning_ms = 0;
    w.npcs[0].hp = 0;
    w.npcs[1].returning = true;
    let center = w.members[1].position;
    plan(&mut w, &c, 0, center);
    let hp = w.members[1].hp;
    let e = advance(&mut w, &c, 150);
    assert_eq!(w.members[1].hp, hp);
    assert!(e.iter().all(|(_, e)| !matches!(e, EventV4::Damage { .. })));
}
#[test]
fn area_background_move_cancel_keep_committed_center_and_budget() {
    let (mut c, mut w) = setup(1, true);
    c.skill.warning_ms = 0;
    let center = w.npcs[0].position;
    plan(&mut w, &c, 0, center);
    let before = w.snapshot(0, &c, Phase::Active, 120).areas.unwrap()[0].clone();
    w.protect(0, 0, &mut vec![]);
    w.members[0].position = [400., 500.];
    advance(&mut w, &c, 720);
    let after = w.snapshot(0, &c, Phase::Active, 720).areas.unwrap()[0].clone();
    assert_eq!(after.center, before.center);
    assert_eq!(after.expires_at_ms, before.expires_at_ms);
    assert_eq!(after.completed_ticks, 1);
    assert!(after.contact_policy.unwrap().used_hits > 0);
}
#[test]
fn area_prepare_cancel_and_protect_leave_cost_and_plan_unchanged() {
    for protect in [false, true] {
        let (c, mut w) = setup(1, false);
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
fn area_source_exit_cleans_only_owner_without_waiting_for_finance() {
    let (c, mut w) = setup(8, true);
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
fn area_absolute_terminal_clears_all_without_late_contacts() {
    let (c, mut w) = setup(2, true);
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
fn area_overlapping_lethal_contacts_credit_and_drop_only_once() {
    let (mut c, mut w) = setup(2, false);
    c.skill.warning_ms = 0;
    w.npcs[0].hp = 1;
    let p = w.npcs[0].position;
    plan(&mut w, &c, 0, p);
    plan(&mut w, &c, 1, p);
    let e = advance(&mut w, &c, 150);
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
fn area_public_plan_uses_kernel_id_and_never_exposes_private_request_or_stats() {
    let (c, mut w) = setup(2, false);
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
