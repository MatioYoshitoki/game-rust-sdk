use super::status_tests::{active_engine_with_config, advance, bind, world};
use super::*;
pub(super) fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/ground-skill-host/catalog.json"),
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
fn cast(w: &mut World, c: &Config, p: [f64; 2], now: u64) -> Result<Vec<Event>> {
    let s = c.player_skills().find(|s| s.is_ground()).unwrap();
    w.action(
        &Action::CastGround {
            skill_id: s.id.clone(),
            position: p,
        },
        &format!("ground-{now}"),
        c,
        now,
    )
}
fn resolved(e: &[Event]) -> Vec<u32> {
    e.iter()
        .filter_map(|e| {
            if let Event::PlayerAreaResolved { hits, .. } = e {
                Some(*hits)
            } else {
                None
            }
        })
        .collect()
}
fn both() -> [f64; 2] {
    [450., 400.]
}
#[test]
fn ground_skill_one_pulse_hits_multiple_enemies_once_and_binds_status() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 1020);
    assert_eq!(resolved(&e), vec![2]);
    let a = w.pve.as_ref().unwrap();
    assert!(a.monsters.iter().all(|m| m.hp == 114));
    assert_eq!(w.spirit, 188);
    assert!(
        w.snapshot(&c, 1020)
            .pve
            .unwrap()
            .monsters
            .iter()
            .all(|m| m.statuses.len() == 1)
    );
    let e = advance(&mut w, &c, 1020, 1500);
    assert!(resolved(&e).is_empty());
    assert!(w.pve.as_ref().unwrap().player_areas.is_empty());
}
#[test]
fn ground_skill_strict_half_open_bounds_walkable_and_finite_without_clamping() {
    let c = config();
    for (p, error) in [
        ([-0.1, 400.], Error::InvalidTarget),
        ([1280., 400.], Error::InvalidTarget),
        ([400., 1280.], Error::InvalidTarget),
        ([f64::NAN, 400.], Error::InvalidMessage),
        ([400., f64::INFINITY], Error::InvalidMessage),
    ] {
        let mut w = make(&c);
        assert_eq!(cast(&mut w, &c, p, 0), Err(error));
        assert!(w.pending.is_none());
        assert_eq!(w.spirit, 200);
    }
    let mut w = make(&c);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0].walkable = false;
    assert_eq!(cast(&mut w, &c, both(), 0), Err(Error::InvalidTarget));
}
#[test]
fn ground_skill_accept_range_inclusive_self_point_and_vision() {
    let mut c = config();
    c.skill.range = 50.;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let mut w = make(&c);
    assert_eq!(cast(&mut w, &c, [450.001, 400.], 0), Err(Error::OutOfRange));
    assert!(w.pve.as_ref().unwrap().player_path.is_empty());
    let mut w = make(&c);
    cast(&mut w, &c, [400., 400.], 0).unwrap();
    let mut w = make(&c);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0].blocks_vision = true;
    assert_eq!(cast(&mut w, &c, both(), 0), Err(Error::OutOfRange));
}
#[test]
fn ground_skill_commit_revalidates_position_range_terrain_and_vision_atomically() {
    for case in 0..3 {
        let mut c = config();
        c.skill.on_commit_self_statuses = vec!["ward".into()];
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        match case {
            0 => w.position = [1100., 1100.],
            1 => {
                Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0].walkable = false
            }
            _ => {
                Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0].blocks_vision =
                    true
            }
        };
        let e = advance(&mut w, &c, 0, 120);
        assert!(e.iter().any(|e| matches!(e, Event::ActionRejected { .. })));
        assert_eq!(w.spirit, 200);
        assert_eq!(w.skill_ready_at, 0);
        assert!(w.snapshot(&c, 120).statuses.is_empty());
        assert!(w.pve.as_ref().unwrap().player_areas.is_empty());
    }
}
#[test]
fn ground_skill_pending_and_warning_freeze_point_and_parameters() {
    let mut c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    assert_eq!(
        w.snapshot(&c, 0).pending_action.unwrap().ground_position,
        Some(both())
    );
    c.skill.radius = 1.;
    c.skill.damage_bps = 1;
    c.skill.warning_ms = 0;
    advance(&mut w, &c, 0, 120);
    let p = &w.pve.as_ref().unwrap().player_areas[0];
    assert_eq!(p.warning.center, both());
    assert_eq!(p.warning.radius, 48.);
    assert_eq!(p.warning.resolves_at_ms, 1020);
    assert_eq!(p.raw, 100);
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
}
#[test]
fn ground_skill_zero_warning_cannot_resolve_same_fixed_step() {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.warning_ms = 0;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 30);
    assert!(resolved(&e).is_empty());
    assert_eq!(w.pve.as_ref().unwrap().player_areas.len(), 1);
    assert_eq!(resolved(&advance(&mut w, &c, 30, 60)), vec![2]);
}
#[test]
fn ground_skill_current_positions_allow_exit_and_entry() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 120);
    let a = w.pve.as_mut().unwrap();
    a.monsters[0].position = [700., 360.];
    a.monsters[1].position = both();
    assert_eq!(resolved(&advance(&mut w, &c, 120, 1020)), vec![1]);
    let a = w.pve.as_ref().unwrap();
    assert_eq!(a.monsters[0].hp, 180);
    assert_eq!(a.monsters[1].hp, 114);
}
#[test]
fn ground_skill_empty_circle_resolves_once_without_spurious_damage() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, [700., 400.], 0).unwrap();
    assert_eq!(resolved(&advance(&mut w, &c, 0, 1020)), vec![0]);
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| m.hp == 180));
    assert!(resolved(&advance(&mut w, &c, 1020, 1500)).is_empty());
}
#[test]
fn ground_skill_radius_edge_is_inclusive_and_order_stable() {
    let mut c = config();
    c.skill.radius = 26.;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 1020);
    let ids: Vec<_> = e
        .iter()
        .filter_map(|e| match e {
            Event::Damage { source, target, .. } if source == "participant" => {
                Some(target.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(ids, vec!["status-target-1", "status-target-2"]);
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[0].position[1] -= 0.01;
    cast(&mut w, &c, both(), 0).unwrap();
    assert_eq!(resolved(&advance(&mut w, &c, 0, 1020)), vec![1]);
}
#[test]
fn ground_skill_resolution_respects_projectile_occlusion() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 120);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0].blocks_projectiles = true;
    assert_eq!(resolved(&advance(&mut w, &c, 120, 1020)), vec![0]);
}
#[test]
fn ground_skill_skips_dead_disabled_returning_and_dodging_enemies() {
    let c = config();
    for case in 0..4 {
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 990);
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        match case {
            0 => m.hp = 0,
            1 => m.enabled = false,
            2 => {
                m.returning = true;
                m.home = [700., 360.];
            }
            _ => {
                Arc::make_mut(&mut m.profile)
                    .stats
                    .insert("dodge".into(), 100);
            }
        }
        assert_eq!(resolved(&advance(&mut w, &c, 990, 1020)), vec![1]);
    }
}
#[test]
fn ground_skill_raw_attack_frozen_defense_and_shield_live() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 120);
    w.facts.atk_points = 1000;
    let target = w.pve.as_ref().unwrap().status_entity(0);
    bind(
        &mut w,
        "exposed",
        status::participant(),
        target.clone(),
        150,
    );
    bind(&mut w, "ward", target.clone(), target, 150);
    let e = advance(&mut w, &c, 120, 1020);
    assert_eq!(resolved(&e), vec![2]);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 118);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 114);
}
#[test]
fn ground_skill_zero_damage_contact_can_apply_status_through_full_shield() {
    let mut c = config();
    c.skill.damage_bps = 0;
    let mut w = make(&c);
    let t = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "ward", t.clone(), t, 0);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 1020);
    assert_eq!(resolved(&e), vec![2]);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
    assert!(
        w.snapshot(&c, 1020).pve.unwrap().monsters[0]
            .statuses
            .iter()
            .any(|s| s.definition_id == "exposed")
    );
}
#[test]
fn ground_skill_commit_resource_and_self_capacity_fail_without_partial_effects() {
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
        cast(&mut w, &c, both(), 0).unwrap();
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
        assert_eq!(w.skill_ready_at, 0);
        assert!(w.snapshot(&c, 120).statuses.is_empty());
        assert!(w.pve.as_ref().unwrap().player_areas.is_empty());
    }
}
#[test]
fn ground_skill_two_plan_capacity_rejects_third_before_self_and_payment() {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.on_commit_self_statuses = vec!["ward".into()];
    let mut w = make(&c);
    for now in [0, 30] {
        cast(&mut w, &c, both(), now).unwrap();
        advance(&mut w, &c, now, now + 30);
    }
    let before = w.snapshot(&c, 60);
    cast(&mut w, &c, both(), 60).unwrap();
    let e = advance(&mut w, &c, 60, 90);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    let after = w.snapshot(&c, 90);
    assert_eq!(before.spirit, after.spirit);
    assert_eq!(before.player_skills, after.player_skills);
    assert_eq!(before.statuses, after.statuses);
    assert_eq!(w.pve.as_ref().unwrap().player_areas.len(), 2);
}
#[test]
fn ground_skill_prepare_cancel_background_leave_never_spend() {
    let c = config();
    for a in [
        Action::CancelAction {},
        Action::Background {},
        Action::Leave {},
    ] {
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        w.action(&a, "cancel", &c, 30).unwrap();
        advance(&mut w, &c, 30, 150);
        assert_eq!(w.spirit, 200);
        assert_eq!(w.skill_ready_at, 0);
        assert!(w.pve.as_ref().unwrap().player_areas.is_empty());
    }
}
#[test]
fn ground_skill_committed_cancel_move_and_background_preserve_plan_and_cooldown() {
    let c = config();
    for a in [
        Action::CancelAction {},
        Action::Move {
            direction: [0., 1.],
        },
        Action::Background {},
    ] {
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 120);
        w.action(&a, "after", &c, 150).unwrap();
        assert_eq!(w.skill_ready_at, 1320);
        assert_eq!(w.recovery_until, 420);
        assert_eq!(w.spirit, 188);
        assert_eq!(w.pve.as_ref().unwrap().player_areas.len(), 1);
        assert_eq!(resolved(&advance(&mut w, &c, 150, 1020)), vec![2]);
    }
}
#[test]
fn ground_skill_death_terminal_close_clear_committed_plan_without_refund() {
    let c = config();
    for case in 0..3 {
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 120);
        let e = match case {
            0 => w.action(&Action::Leave {}, "leave", &c, 150).unwrap(),
            1 => {
                w.extraction_end = Some(150);
                advance(&mut w, &c, 120, 150)
            }
            _ => w.close_container_interaction(150),
        };
        assert!(
            e.iter()
                .any(|e| matches!(e, Event::PlayerAreaCancelled { .. }))
        );
        assert!(w.pve.as_ref().unwrap().player_areas.is_empty());
        assert_eq!(w.spirit, 188);
        assert!(resolved(&advance(&mut w, &c, 150, 1200)).is_empty());
    }
}
#[test]
fn ground_skill_shared_recovery_and_pending_cannot_be_bypassed() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    assert_eq!(cast(&mut w, &c, both(), 30), Err(Error::InvalidState));
    advance(&mut w, &c, 0, 120);
    w.action(&Action::CancelAction {}, "cancel", &c, 150)
        .unwrap();
    assert_eq!(
        w.action(
            &Action::CastSelf {
                skill_id: c.additional_skills[0].id.clone()
            },
            "self",
            &c,
            150
        ),
        Err(Error::InvalidState)
    );
}
#[test]
fn ground_skill_primary_secondary_cooldowns_alias_and_warning_views() {
    for swap in [false, true] {
        let mut c = config();
        if swap {
            std::mem::swap(&mut c.skill, &mut c.additional_skills[0]);
        }
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        let e = advance(&mut w, &c, 0, 120);
        let s = w.snapshot(&c, 120);
        assert_eq!(s.skill_ready_at_ms, if swap { 0 } else { 1320 });
        assert_eq!(
            s.player_skills.unwrap()[usize::from(swap)].ready_at_ms,
            1320
        );
        assert_eq!(s.pve.unwrap().player_area_warnings.unwrap().len(), 1);
        assert!(
            e.iter().any(
                |e| matches!(e,Event::ActionCommitted{ground_position:Some(p),..} if p==&both())
            )
        );
    }
}
#[test]
fn ground_skill_mismatched_intents_old_versions_and_unknown_skill() {
    let c = config();
    for v in 1..29 {
        let mut c = c.clone();
        c.mechanism_version = v;
        let mut w = make(&c);
        assert_eq!(cast(&mut w, &c, both(), 0), Err(Error::Unsupported));
    }
    let mut w = make(&c);
    for a in [
        Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        Action::CastSelf {
            skill_id: c.skill.id.clone(),
        },
        Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: w.pve.as_ref().unwrap().unit_reference(0).unwrap(),
        },
        Action::CastGround {
            skill_id: c.additional_skills[0].id.clone(),
            position: both(),
        },
    ] {
        assert_eq!(w.action(&a, "bad", &c, 0), Err(Error::InvalidMessage));
    }
    assert_eq!(
        w.action(
            &Action::CastGround {
                skill_id: "missing".into(),
                position: both()
            },
            "missing",
            &c,
            0
        ),
        Err(Error::Unsupported)
    );
}
#[test]
fn ground_skill_old_snapshots_pending_events_omit_ground_fields() {
    for v in 19..29 {
        let mut c = super::unit_skill_tests::config();
        c.mechanism_version = v;
        let w = make(&c);
        let s = serde_json::to_value(w.snapshot(&c, 0)).unwrap();
        assert!(s["pve"].get("player_area_warnings").is_none());
    }
    let c = super::unit_skill_tests::config();
    let mut w = make(&c);
    let t = w.pve.as_ref().unwrap().unit_reference(0).unwrap();
    w.action(
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: t,
        },
        "old",
        &c,
        0,
    )
    .unwrap();
    assert!(
        serde_json::to_value(w.snapshot(&c, 0)).unwrap()["pending_action"]
            .get("ground_position")
            .is_none()
    );
    let e = advance(&mut w, &c, 0, 120);
    assert!(e.iter().all(|e| {
        serde_json::to_value(e)
            .unwrap()
            .get("ground_position")
            .is_none()
    }));
}

#[test]
fn ground_skill_hundred_closes_release_ground_plans_and_frozen_assets() {
    use crate::asset_contract::tests as s;
    for commit in [false, true] {
        for _ in 0..50 {
            let mut e = active_engine_with_config(config());
            let now = e.status().logical_ms;
            let c = e.config.clone();
            cast(e.world.as_mut().unwrap(), &c, both(), now).unwrap();
            if commit {
                for _ in 0..5 {
                    e.tick();
                    e.drain();
                }
            }
            let key = s::create(&e.config).key;
            e.close(Close {
                key,
                request_id: "unit-close".into(),
            })
            .unwrap();
            assert_eq!(e.live_worlds(), 0);
            let r = s::requests(&e.drain()).remove(0);
            let receipt = s::receipt(
                &e.config,
                &r,
                ReceiptOutcome::Committed {
                    revision: 2,
                    commit_id: "unit-close-assets".into(),
                },
            );
            e.business(receipt).unwrap();
            assert_eq!(e.pending_count(), 0);
        }
    }
}
#[test]
fn ground_skill_legal_output_peak_keeps_ground_views_and_frozen_asset_responsibility_bounded() {
    use crate::asset_contract::tests as s;
    let mut c = config();
    c.output_capacity = 32;
    c.skill.windup_ms = 0;
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 3;
    c.pve_mut().status_definitions[0].tick_interval_ms = Some(30);
    c.pve_mut().monster_positions = (0..32)
        .map(|i| {
            [
                100. + f64::from(i % 16) * 40.,
                600. + f64::from(i / 16) * 80.,
            ]
        })
        .collect();
    let mut e = active_engine_with_config(c);
    let now = e.status().logical_ms;
    let w = e.world.as_mut().unwrap();
    let mut a = w.pve.take().unwrap();
    for i in 0..32 {
        let m = a.status_entity(i);
        a.commit_statuses(w, &["renewal".into(), "ward".into()], &m, now, &mut vec![]);
    }
    w.pve = Some(a);
    let c = e.config.clone();
    cast(e.world.as_mut().unwrap(), &c, both(), now).unwrap();
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
    let receipt = s::receipt(
        &e.config,
        &r,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "unit-peak-assets".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
}

#[test]
fn ground_skill_public_finite_validation_reconnect_and_dedup_preserve_warning() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine_with_config(config());
    let c = s::create(&e.config);
    let input = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: "unit-cast".into(),
        sequence: 3,
        action: Action::CastGround {
            skill_id: e.config.skill.id.clone(),
            position: both(),
        },
    };
    let mut bad = input.clone();
    bad.request_id = "bad-zero".into();
    bad.action = Action::CastGround {
        skill_id: e.config.skill.id.clone(),
        position: [f64::NAN, 400.],
    };
    assert_eq!(e.input(bad), Err(Error::InvalidMessage));
    assert_eq!(e.input(input.clone()), e.input(input.clone()));
    for _ in 0..5 {
        e.tick();
        e.drain();
    }
    let before = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    assert_eq!(before.spirit, 88);
    e.input(input.clone()).unwrap();
    e.input(Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "unit-resume".into(),
            generation: 2,
        },
        request_id: "resume".into(),
        sequence: 4,
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
    assert_eq!(after.player_skills, before.player_skills);
    assert_eq!(after.recovery_until_ms, before.recovery_until_ms);
    assert_eq!(
        after.pve.unwrap().player_area_warnings,
        before.pve.unwrap().player_area_warnings
    );
    assert_eq!(after.spirit, before.spirit);
    assert_eq!(e.input(input), Err(Error::StaleConnection));
}
#[test]
fn ground_skill_movement_preserves_preparation_and_submission_rechecks_current_caster() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    w.action(
        &Action::Move {
            direction: [0., 1.],
        },
        "move",
        &c,
        30,
    )
    .unwrap();
    assert!(w.pending.is_some());
    advance(&mut w, &c, 30, 120);
    assert_eq!(w.pve.as_ref().unwrap().player_areas.len(), 1);
    assert_eq!(w.spirit, 188);
}

#[test]
fn ground_skill_full_plan_capacity_does_not_reject_basic_attack() {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.recovery_ms = 0;
    c.basic.windup_ms = 0;
    let mut w = make(&c);
    for now in [0, 30] {
        cast(&mut w, &c, both(), now).unwrap();
        advance(&mut w, &c, now, now + 30);
    }
    w.action(&Action::Attack {}, "basic-full", &c, 60).unwrap();
    let e = advance(&mut w, &c, 60, 90);
    assert!(e.iter().any(
        |e| matches!(e,Event::ActionCommitted{action_id,skill:false,..} if action_id=="basic-full")
    ));
    assert_eq!(w.pve.as_ref().unwrap().player_areas.len(), 2);
    assert!(w.pve.as_ref().unwrap().shots.iter().any(|s| !s.metal));
}
