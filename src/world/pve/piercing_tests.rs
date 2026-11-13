use super::status_tests::{active_engine_with_config, advance, bind, world};
use super::*;
fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/piercing-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone()
}
fn make(c: &Config) -> World {
    let mut w = world(c);
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.aggro = true;
    }
    w
}
fn cast(w: &mut World, c: &Config, now: u64) -> Result<Vec<Event>> {
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        &format!("pierce-{now}"),
        c,
        now,
    )
}
fn fast() -> Config {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.speed = 10000.;
    c.skill.recovery_ms = 0;
    c.skill.cooldown_ms = 0;
    c
}
fn hits(e: &[Event]) -> Vec<String> {
    e.iter()
        .filter_map(|e| {
            if let Event::PiercingHit { target, .. } = e {
                Some(target.id.clone())
            } else {
                None
            }
        })
        .collect()
}
fn ended(e: &[Event], reason: PiercingEndReason) -> usize {
    e.iter()
        .filter(|e| matches!(e,Event::PiercingEnded{reason:r,..}if*r==reason))
        .count()
}
fn view(w: &World, c: &Config, t: u64) -> PiercingProjectileView {
    w.snapshot(c, t).pve.unwrap().piercing_projectiles.unwrap()[0].clone()
}
#[test]
fn piercing_high_speed_sweep_hits_three_in_path_order_and_budget_ends() {
    let c = fast();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 60);
    assert_eq!(
        hits(&e),
        ["status-target-1", "status-target-2", "status-target-3"]
    );
    assert_eq!(ended(&e, PiercingEndReason::HitBudgetExhausted), 1);
    assert!(!w.pve.as_ref().unwrap().has_piercing());
}
#[test]
fn piercing_total_budget_two_stops_before_third() {
    let mut c = fast();
    c.skill.max_hits = 2;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 90);
    assert_eq!(hits(&e).len(), 2);
    assert_eq!(w.pve.as_ref().unwrap().monsters[2].hp, 180);
    assert_eq!(ended(&e, PiercingEndReason::HitBudgetExhausted), 1);
}
#[test]
fn piercing_tie_uses_id_not_storage_order() {
    let mut c = fast();
    c.skill.max_hits = 2;
    let mut w = make(&c);
    for (i, m) in w.pve.as_mut().unwrap().monsters.iter_mut().enumerate() {
        m.position = [450., 400.];
        m.id = ["z", "b", "a"][i].into();
    }
    cast(&mut w, &c, 0).unwrap();
    assert_eq!(hits(&advance(&mut w, &c, 0, 60)), ["a", "b"]);
}
#[test]
fn piercing_overlap_once_and_new_generation_is_new_identity() {
    let mut c = fast();
    c.skill.speed = 0.1;
    c.skill.max_hits = 5;
    let mut w = make(&c);
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.position = [400., 400.];
    }
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 90);
    assert_eq!(hits(&e).len(), 3);
    assert_eq!(view(&w, &c, 90).used_hits, 3);
    w.pve.as_mut().unwrap().monsters[0].status_generation += 1;
    let e = advance(&mut w, &c, 90, 120);
    assert_eq!(hits(&e), ["status-target-1"]);
    assert_eq!(view(&w, &c, 120).contacts.len(), 4);
}
#[test]
fn piercing_dodge_spends_nothing_and_retries_next_step_only() {
    let mut c = fast();
    c.skill.speed = 0.1;
    let mut w = make(&c);
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.position = [400., 400.];
        Arc::make_mut(&mut m.profile)
            .stats
            .insert("dodge".into(), 100);
    }
    cast(&mut w, &c, 0).unwrap();
    assert!(hits(&advance(&mut w, &c, 0, 90)).is_empty());
    assert_eq!(view(&w, &c, 90).used_hits, 0);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
        .stats
        .insert("dodge".into(), 0);
    assert_eq!(hits(&advance(&mut w, &c, 90, 120)), ["status-target-1"]);
}
#[test]
fn piercing_full_shield_and_zero_hp_damage_count_and_apply_status() {
    let mut c = fast();
    c.skill.damage_bps = 1000;
    let mut w = make(&c);
    let t = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "ward", t.clone(), t, 0);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 60);
    assert_eq!(hits(&e).len(), 3);
    assert!(
        e.iter()
            .any(|e| matches!(e,Event::Damage{target,amount:0,..}if target=="status-target-1"))
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
    assert!(
        w.snapshot(&c, 60).pve.unwrap().monsters[0]
            .statuses
            .iter()
            .any(|s| s.definition_id == "exposed")
    );
}
#[test]
fn piercing_disabled_returning_and_reset_targets_do_not_spend() {
    let c = fast();
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[0].enabled = false;
    w.pve.as_mut().unwrap().monsters[1].returning = true;
    w.pve.as_mut().unwrap().monsters[1].home = [1000., 1000.];
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 60);
    assert_eq!(hits(&e), ["status-target-3"]);
    assert_eq!(view(&w, &c, 60).remaining_hits, 2);
}
#[test]
fn piercing_attack_frozen_after_self_commit_and_current_defense_used() {
    let mut c = fast();
    c.skill.speed = 600.;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 30);
    let raw = w.pve.as_ref().unwrap().piercing_projectiles[0].view();
    w.facts.atk_points = 1000;
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[1].profile)
        .stats
        .insert("def".into(), 10000);
    advance(&mut w, &c, 30, 390);
    let a = w.pve.as_ref().unwrap();
    assert!(a.monsters[0].hp > 0 && a.monsters[1].hp > a.monsters[0].hp);
    assert_eq!(raw.used_hits, 0);
}
#[test]
fn piercing_parameters_direction_frozen_origin_is_commit_position() {
    let mut c = fast();
    c.skill.windup_ms = 120;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    c.skill.speed = 20.;
    c.skill.max_hits = 1;
    c.skill.projectile_ttl_ms = 60;
    w.position = [410., 410.];
    advance(&mut w, &c, 0, 120);
    let p = view(&w, &c, 120);
    assert_eq!(p.position, [410., 410.]);
    assert_eq!(p.direction, [1., 0.]);
    assert_eq!(p.speed, 10000.);
    assert_eq!(p.max_hits, 3);
    assert_eq!(p.expires_at_ms, 3120);
}
#[test]
fn piercing_zero_windup_commits_next_step_and_does_not_move_creation_step() {
    let c = fast();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    assert!(!w.pve.as_ref().unwrap().has_piercing());
    assert!(hits(&advance(&mut w, &c, 0, 30)).is_empty());
    assert_eq!(view(&w, &c, 30).position, [400., 400.]);
}
#[test]
fn piercing_range_endpoint_counts_before_range_end() {
    let mut c = fast();
    c.skill.range = 30.;
    c.skill.max_hits = 5;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 60);
    assert_eq!(hits(&e), ["status-target-1"]);
    assert_eq!(ended(&e, PiercingEndReason::RangeExhausted), 1);
}
#[test]
fn piercing_budget_at_endpoint_has_priority_over_range_end() {
    let mut c = fast();
    c.skill.range = 30.;
    c.skill.max_hits = 1;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 60);
    assert_eq!(hits(&e).len(), 1);
    assert_eq!(ended(&e, PiercingEndReason::HitBudgetExhausted), 1);
    assert_eq!(ended(&e, PiercingEndReason::RangeExhausted), 0);
}
#[test]
fn piercing_ttl_is_exclusive_before_movement() {
    let mut c = fast();
    c.skill.projectile_ttl_ms = 60;
    c.skill.speed = 600.;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 90);
    assert!(hits(&e).is_empty());
    assert_eq!(ended(&e, PiercingEndReason::Expired), 1);
}
fn wall(w: &mut World, x: usize) {
    let a = w.pve.as_mut().unwrap();
    let mut tile = a.grid.config.palette[0].clone();
    tile.blocks_projectiles = true;
    let cfg = Arc::make_mut(&mut a.grid.config);
    cfg.palette.push(tile);
    let k = cfg.palette.len() - 1;
    let width = cfg.width;
    let cells = Arc::make_mut(&mut a.grid.cells);
    cells[10 * width + x] = k;
}
#[test]
fn piercing_wall_truncates_before_later_enemy() {
    let c = fast();
    let mut w = make(&c);
    wall(&mut w, 12);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 90);
    assert_eq!(hits(&e), ["status-target-1"]);
    assert_eq!(ended(&e, PiercingEndReason::Wall), 1);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 180);
}
#[test]
fn piercing_exact_wall_unit_tie_wall_wins() {
    let c = fast();
    let mut w = make(&c);
    wall(&mut w, 12);
    w.pve.as_mut().unwrap().monsters[0].position = [494., 400.];
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 60);
    assert!(hits(&e).is_empty());
    assert_eq!(ended(&e, PiercingEndReason::Wall), 1);
}
#[test]
fn piercing_wall_corner_disk_tangent_and_boundary() {
    let c = fast();
    let mut w = make(&c);
    wall(&mut w, 12);
    let grid = &w.pve.as_ref().unwrap().grid;
    assert!(grid.piercing_wall([400., 394.], [700., 394.], 6.).is_some());
    assert!(grid.piercing_wall([400., 393.], [700., 393.], 6.).is_none());
    assert_eq!(
        grid.piercing_wall([1200., 300.], [1300., 300.], 0.),
        Some(0.8)
    );
}
#[test]
fn basic_wave_controls_swept_movement_uses_radius_and_map_edge() {
    let c = fast();
    let mut w = make(&c);
    wall(&mut w, 12);
    let grid = &mut w.pve.as_mut().unwrap().grid;
    Arc::make_mut(&mut grid.config)
        .palette
        .last_mut()
        .unwrap()
        .walkable = false;
    let end = grid.walk_until_blocked([400., 394.], [700., 394.], 6.);
    assert!((end[0] - 480.).abs() < 1e-5);
    assert_eq!(
        grid.walk_until_blocked([400., 393.], [700., 393.], 6.),
        [700., 393.]
    );
    assert_eq!(
        grid.walk_until_blocked([474., 420.], [400., 420.], 6.),
        [400., 420.]
    );
    let edge = grid.walk_until_blocked([1200., 300.], [1400., 300.], 14.);
    assert!((edge[0] - 1266.).abs() < 1e-5);
    assert!(grid.clear(edge, 14., 0));
}
#[test]
fn piercing_circle_tangent_hits_and_outside_misses() {
    for delta in [20., 20.01] {
        let c = fast();
        let mut w = make(&c);
        for m in &mut w.pve.as_mut().unwrap().monsters {
            m.position[1] = 400. + delta;
        }
        cast(&mut w, &c, 0).unwrap();
        let e = advance(&mut w, &c, 0, 60);
        assert_eq!(hits(&e).len(), if delta == 20. { 3 } else { 0 });
    }
}
#[test]
fn piercing_kills_drop_once_and_no_future_contacts() {
    let mut c = fast();
    c.skill.damage_bps = 100000;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 300);
    assert_eq!(hits(&e).len(), 3);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        3
    );
    assert_eq!(
        w.resources
            .iter()
            .filter(|r| !r.view.orb)
            .map(|r| r.view.quantity)
            .sum::<u32>(),
        6
    );
}
#[test]
fn piercing_two_flights_have_independent_histories_and_shared_recovery() {
    let mut c = fast();
    c.skill.speed = 0.1;
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[0].position = [400., 400.];
    for now in [0, 30] {
        cast(&mut w, &c, now).unwrap();
        advance(&mut w, &c, now, now + 30);
    }
    advance(&mut w, &c, 60, 90);
    let a = w.pve.as_ref().unwrap();
    assert_eq!(a.piercing_projectiles.len(), 2);
    assert!(
        a.piercing_projectiles
            .iter()
            .all(|p| p.view().used_hits == 1)
    );
    assert_eq!(w.spirit, 176);
}
#[test]
fn piercing_capacity_failure_is_atomic_and_self_skill_unaffected() {
    let mut c = fast();
    c.skill.speed = 0.1;
    c.skill.on_commit_self_statuses = vec!["ward".into()];
    let mut w = make(&c);
    for now in [0, 30, 60] {
        cast(&mut w, &c, now).unwrap();
        let e = advance(&mut w, &c, now, now + 30);
        if now == 60 {
            assert!(e.iter().any(|e| matches!(
                e,
                Event::ActionRejected {
                    reason: Error::Capacity,
                    ..
                }
            )));
            assert!(!e.iter().any(|e| matches!(e, Event::StatusApplied { .. })));
        }
    }
    assert_eq!(w.spirit, 176);
    w.action(
        &Action::CastSelf {
            skill_id: c.additional_skills[0].id.clone(),
        },
        "self",
        &c,
        90,
    )
    .unwrap();
    let e = advance(&mut w, &c, 90, 240);
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::ActionCommitted { skill: true, .. }))
    );
}
#[test]
fn piercing_resource_and_self_group_capacity_fail_atomically() {
    for cap in [false, true] {
        let mut c = config();
        c.skill.on_commit_self_statuses = vec!["ward".into(), "renewal".into()];
        if cap {
            c.pve_mut()
                .status_rules
                .as_mut()
                .unwrap()
                .max_active_per_entity = 1;
        }
        let mut w = make(&c);
        cast(&mut w, &c, 0).unwrap();
        if !cap {
            w.spirit = 0;
        }
        let e = advance(&mut w, &c, 0, 120);
        assert!(e.iter().any(|e| matches!(
            e,
            Event::ActionRejected {
                reason: Error::Capacity | Error::NoResource,
                ..
            }
        )));
        assert!(!w.pve.as_ref().unwrap().has_piercing());
        assert_eq!(w.skill_ready_at, 0);
        assert!(w.snapshot(&c, 120).statuses.is_empty());
    }
}
#[test]
fn piercing_cancel_and_background_before_commit_are_free() {
    for action in [Action::CancelAction {}, Action::Background {}] {
        let c = config();
        let mut w = make(&c);
        cast(&mut w, &c, 0).unwrap();
        w.action(&action, "cancel", &c, 30).unwrap();
        advance(&mut w, &c, 30, 300);
        assert_eq!(w.spirit, 200);
        assert!(!w.pve.as_ref().unwrap().has_piercing());
    }
}
#[test]
fn piercing_committed_cancel_move_background_keep_flight() {
    let mut c = fast();
    c.skill.speed = 0.1;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 30);
    for action in [
        Action::Move {
            direction: [0., 1.],
        },
        Action::CancelAction {},
        Action::Background {},
    ] {
        w.action(&action, "keep", &c, 30).unwrap();
        assert!(w.pve.as_ref().unwrap().has_piercing());
    }
    assert_eq!(w.spirit, 188);
}
#[test]
fn piercing_death_terminal_close_end_once_and_do_not_refund() {
    for case in 0..4 {
        let mut c = fast();
        c.skill.speed = 0.1;
        let mut w = make(&c);
        cast(&mut w, &c, 0).unwrap();
        advance(&mut w, &c, 0, 30);
        let e = match case {
            0 => w.action(&Action::Leave {}, "leave", &c, 60).unwrap(),
            1 => {
                w.extraction_end = Some(60);
                advance(&mut w, &c, 30, 60)
            }
            2 => w.close_container_interaction(60),
            _ => {
                let mut a = w.pve.take().unwrap();
                let mut e = vec![];
                a.hurt_player(&mut w, &c, 0, (100000, false), 60, &mut e);
                w.pve = Some(a);
                e
            }
        };
        let reason = match case {
            3 => PiercingEndReason::OwnerDied,
            0 => PiercingEndReason::Terminal,
            1 => PiercingEndReason::Terminal,
            _ => PiercingEndReason::Closed,
        };
        assert_eq!(ended(&e, reason), 1);
        assert!(!w.pve.as_ref().unwrap().has_piercing());
        assert_eq!(w.spirit, 188);
        assert!(hits(&advance(&mut w, &c, 60, 300)).is_empty());
    }
}
#[test]
fn piercing_old_views_omit_new_field() {
    let c = super::contact_policy_tests::config();
    let w = make(&c);
    let v = serde_json::to_value(w.snapshot(&c, 0)).unwrap();
    assert!(v["pve"].get("piercing_projectiles").is_none());
}
#[test]
fn piercing_hundred_closes_release_world_and_frozen_assets() {
    use crate::asset_contract::tests as s;
    for committed in [false, true] {
        for _ in 0..50 {
            let mut c = fast();
            c.skill.speed = 0.1;
            let mut e = active_engine_with_config(c);
            let now = e.status().logical_ms;
            let c = e.config.clone();
            if committed {
                e.world.as_mut().unwrap().pve.as_mut().unwrap().monsters[0].position = [400., 400.];
            }
            cast(e.world.as_mut().unwrap(), &c, now).unwrap();
            if committed {
                for _ in 0..2 {
                    e.tick();
                    e.drain();
                }
                assert_eq!(
                    view(e.world.as_ref().unwrap(), &c, e.status().logical_ms).used_hits,
                    1
                );
            }
            e.close(Close {
                key: s::create(&e.config).key,
                request_id: "pierce-close".into(),
            })
            .unwrap();
            assert_eq!(e.live_worlds(), 0);
            let r = s::requests(&e.drain()).remove(0);
            e.business(s::receipt(
                &e.config,
                &r,
                ReceiptOutcome::Committed {
                    revision: 2,
                    commit_id: "pierce-assets".into(),
                },
            ))
            .unwrap();
            assert_eq!(e.pending_count(), 0);
        }
    }
}
#[test]
fn piercing_public_reconnect_dedup_keeps_real_history() {
    use crate::asset_contract::tests as s;
    let mut c = fast();
    c.skill.speed = 0.1;
    let mut e = active_engine_with_config(c);
    let c = s::create(&e.config);
    e.world.as_mut().unwrap().pve.as_mut().unwrap().monsters[0].position = [400., 400.];
    let input = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: "pierce-cast".into(),
        sequence: 4,
        action: Action::Cast {
            skill_id: e.config.skill.id.clone(),
            direction: [1., 0.],
        },
    };
    for direction in [[0., 0.], [f64::NAN, 0.], [f64::INFINITY, 0.]] {
        let mut bad = input.clone();
        bad.request_id = "bad".into();
        bad.sequence = 3;
        bad.action = Action::Cast {
            skill_id: e.config.skill.id.clone(),
            direction,
        };
        assert_eq!(e.input(bad), Err(Error::InvalidMessage));
    }
    let accepted = e.input(input.clone());
    assert!(accepted.is_ok(), "{accepted:?}");
    assert_eq!(accepted, e.input(input.clone()));
    for _ in 0..10 {
        e.tick();
        e.drain();
    }
    let before = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    assert_eq!(before.spirit, 88);
    assert_eq!(
        before
            .pve
            .as_ref()
            .unwrap()
            .piercing_projectiles
            .as_ref()
            .unwrap()[0]
            .used_hits,
        1
    );
    e.input(Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "pierce-resume".into(),
            generation: 2,
        },
        request_id: "resume".into(),
        sequence: 5,
        action: Action::Reconnect {
            previous_connection: c.connection,
            authenticated_principal: c.authenticated_principal,
        },
    })
    .unwrap();
    let after = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    assert_eq!(
        after.pve.unwrap().piercing_projectiles,
        before.pve.unwrap().piercing_projectiles
    );
    assert_eq!(after.player_skills, before.player_skills);
    assert_eq!(after.spirit, before.spirit);
    assert_eq!(e.input(input), Err(Error::StaleConnection));
}
#[test]
fn piercing_128_identity_history_is_bounded_then_released() {
    let mut c = fast();
    c.skill.speed = 0.1;
    c.skill.max_hits = 128;
    c.skill.projectile_ttl_ms = 60000;
    c.skill.damage_bps = 0;
    c.params.minimum_damage = 0;
    let mut w = make(&c);
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.enabled = false;
    }
    let m = &mut w.pve.as_mut().unwrap().monsters[0];
    m.enabled = true;
    m.position = [400., 400.];
    m.id = "x".repeat(128);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 30);
    for n in 1..=128 {
        w.pve.as_mut().unwrap().monsters[0].status_generation = n;
        let now = 30 + u64::from(n) * 30;
        let e = advance(&mut w, &c, now - 30, now);
        assert_eq!(hits(&e).len(), 1);
        if n == 127 {
            assert_eq!(view(&w, &c, now).contacts.len(), 127);
            assert!(
                serde_json::to_vec(&w.snapshot(&c, now)).unwrap().len() < crate::JSONL_LINE_LIMIT
            );
        }
        if n == 128 {
            assert_eq!(ended(&e, PiercingEndReason::HitBudgetExhausted), 1);
        }
    }
    assert!(!w.pve.as_ref().unwrap().has_piercing());
}
#[test]
fn piercing_peak_output_fault_releases_world_preserves_assets() {
    use crate::asset_contract::tests as s;
    let mut c = fast();
    c.output_capacity = 32;
    c.skill.max_hits = 128;
    c.skill.damage_bps = 100000;
    c.pve_mut().monster_positions = (0..32).map(|i| [450. + f64::from(i), 400.]).collect();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 3;
    let mut e = active_engine_with_config(c);
    let now = e.status().logical_ms;
    let c = e.config.clone();
    cast(e.world.as_mut().unwrap(), &c, now).unwrap();
    e.tick();
    e.drain();
    e.tick();
    let o = e.drain();
    assert_eq!(e.live_worlds(), 0);
    assert!(o.len() <= 32);
    assert!(o.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::Fault {
                code: Error::QueueFull
            }
        }
    )));
    let r = s::requests(&o).remove(0);
    e.business(s::receipt(
        &e.config,
        &r,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "pierce-peak".into(),
        },
    ))
    .unwrap();
    assert_eq!(e.pending_count(), 0);
}
