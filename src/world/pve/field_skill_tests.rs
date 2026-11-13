use super::status_tests::{active_engine_with_config, advance, bind, world};
use super::*;
pub(super) fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/field-host/catalog.json"),
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
fn both() -> [f64; 2] {
    [450., 400.]
}
fn cast(w: &mut World, c: &Config, p: [f64; 2], now: u64) -> Result<Vec<Event>> {
    let s = c.player_skills().find(|s| s.is_field()).unwrap();
    w.action(
        &Action::CastGround {
            skill_id: s.id.clone(),
            position: p,
        },
        &format!("field-{now}"),
        c,
        now,
    )
}
fn ticks(e: &[Event]) -> Vec<(u32, u64, u32)> {
    e.iter()
        .filter_map(|e| {
            if let Event::PlayerFieldTick {
                tick_index,
                scheduled_at_ms,
                hits,
                ..
            } = e
            {
                Some((*tick_index, *scheduled_at_ms, *hits))
            } else {
                None
            }
        })
        .collect()
}
fn short() -> Config {
    let mut c = config();
    c.skill.warning_ms = 0;
    c.skill.tick_interval_ms = 60;
    c.skill.duration_ms = 180;
    c
}
#[test]
fn field_skill_three_pulses_current_targets_and_hit_statuses_pay_once() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 3120);
    assert_eq!(ticks(&e), vec![(1, 1320, 2), (2, 1920, 2), (3, 2520, 2)]);
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| m.hp == 112));
    assert_eq!(w.spirit, 188);
    assert_eq!(w.skill_ready_at, 1320);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::ActionCommitted { .. }))
            .count(),
        1
    );
    assert_eq!(
        e.iter()
            .filter(|e| matches!(
                e,
                Event::PlayerFieldEnded {
                    reason: PlayerFieldEndReason::Expired,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
}
#[test]
fn field_skill_first_tick_waits_interval_and_end_is_exclusive() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 150);
    assert!(ticks(&e).is_empty());
    assert_eq!(
        w.snapshot(&c, 150).pve.unwrap().player_fields.unwrap()[0].next_tick_at_ms,
        Some(180)
    );
    let e = advance(&mut w, &c, 150, 300);
    assert_eq!(ticks(&e), vec![(1, 180, 2), (2, 240, 2)]);
    assert!(
        w.snapshot(&c, 300)
            .pve
            .unwrap()
            .player_fields
            .unwrap()
            .is_empty()
    );
}
#[test]
fn field_skill_zero_windup_warning_still_wait_one_interval() {
    let mut c = short();
    c.skill.windup_ms = 0;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    assert!(ticks(&advance(&mut w, &c, 0, 30)).is_empty());
    assert!(ticks(&advance(&mut w, &c, 30, 60)).is_empty());
    assert_eq!(ticks(&advance(&mut w, &c, 60, 90)), vec![(1, 90, 2)]);
}
#[test]
fn field_skill_non_step_timing_uses_scheduled_times_without_drift() {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.warning_ms = 5;
    c.skill.tick_interval_ms = 50;
    c.skill.duration_ms = 151;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let mut actual = vec![];
    for n in 1..=7 {
        let now = n * 30;
        for t in ticks(&advance(&mut w, &c, now - 30, now)) {
            actual.push((t.1, now));
        }
    }
    assert_eq!(actual, vec![(85, 90), (135, 150), (185, 210)]);
    assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
}
#[test]
fn field_skill_late_processing_is_bounded_and_never_adds_expiry_pulse() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 120);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_player_fields(&mut w, &c, 900, &mut e);
    assert_eq!(ticks(&e), vec![(1, 180, 2), (2, 240, 2)]);
    a.advance_player_fields(&mut w, &c, 930, &mut e);
    assert_eq!(ticks(&e).len(), 2);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::PlayerFieldEnded { .. }))
            .count(),
        1
    );
}
#[test]
fn field_skill_128_empty_ticks_advance_once_each_then_end() {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.warning_ms = 0;
    c.skill.duration_ms = 3870;
    c.skill.tick_interval_ms = 30;
    let mut w = make(&c);
    cast(&mut w, &c, [700., 400.], 0).unwrap();
    let e = advance(&mut w, &c, 0, 3900);
    let t = ticks(&e);
    assert_eq!(t.len(), 128);
    assert_eq!(t.first(), Some(&(1, 60, 0)));
    assert_eq!(t.last(), Some(&(128, 3870, 0)));
    assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
    assert_eq!(w.spirit, 188);
}
#[test]
fn field_skill_exit_and_reentry_only_affect_future_scheduled_contacts() {
    let mut c = short();
    c.skill.duration_ms = 300;
    c.skill.radius = 20.;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    assert_eq!(ticks(&advance(&mut w, &c, 0, 180)), vec![(1, 180, 0)]);
    w.pve.as_mut().unwrap().monsters[0].position = both();
    assert_eq!(ticks(&advance(&mut w, &c, 180, 240)), vec![(2, 240, 1)]);
    w.pve.as_mut().unwrap().monsters[0].position = [450., 360.];
    assert_eq!(ticks(&advance(&mut w, &c, 240, 300)), vec![(3, 300, 0)]);
    w.pve.as_mut().unwrap().monsters[0].position = both();
    assert_eq!(ticks(&advance(&mut w, &c, 300, 420)), vec![(4, 360, 1)]);
    assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
}
#[test]
fn field_skill_overlaps_are_independent_and_ties_keep_creation_order() {
    let mut c = short();
    c.skill.windup_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.tick_interval_ms = 90;
    c.skill.duration_ms = 301;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 30);
    c.skill.tick_interval_ms = 60;
    cast(&mut w, &c, both(), 30).unwrap();
    advance(&mut w, &c, 30, 60);
    let e = advance(&mut w, &c, 60, 210);
    let ids: Vec<_> = e
        .iter()
        .filter_map(|e| {
            if let Event::PlayerFieldTick {
                action_id,
                scheduled_at_ms,
                ..
            } = e
            {
                Some((action_id.as_str(), *scheduled_at_ms))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        ids,
        vec![
            ("field-0", 120),
            ("field-30", 120),
            ("field-30", 180),
            ("field-0", 210)
        ]
    );
    assert_eq!(w.spirit, 176);
}
#[test]
fn field_skill_freezes_attack_then_uses_current_defense_and_shields() {
    let c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 1320);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 160);
    w.facts.atk_points = 1000;
    let t = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "ward", t.clone(), t, 1350);
    advance(&mut w, &c, 1320, 2520);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 130);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 112);
}
#[test]
fn field_skill_real_contact_order_boundary_obstruction_and_dodge() {
    for case in 0..3 {
        let mut c = short();
        c.skill.radius = 26.;
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 150);
        match case {
            0 => {}
            1 => {
                Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0]
                    .blocks_projectiles = true
            }
            _ => {
                Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
                    .stats
                    .insert("dodge".into(), 100);
            }
        }
        let e = advance(&mut w, &c, 150, 180);
        assert_eq!(
            ticks(&e)[0].2,
            match case {
                0 => 2,
                1 => 0,
                _ => 1,
            }
        );
        if case == 0 {
            let ids: Vec<_> = e
                .iter()
                .filter_map(|e| {
                    if let Event::Damage { source, target, .. } = e {
                        (source == "participant").then_some(target.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(ids, vec!["status-target-1", "status-target-2"]);
        }
    }
}
#[test]
fn field_skill_skips_dead_disabled_returning_targets_each_pulse() {
    let c = short();
    for case in 0..3 {
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 150);
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        match case {
            0 => m.hp = 0,
            1 => m.enabled = false,
            _ => {
                m.returning = true;
                m.home = [700., 360.];
            }
        }
        assert_eq!(
            ticks(&advance(&mut w, &c, 150, 240)),
            vec![(1, 180, 1), (2, 240, 1)]
        );
    }
}
#[test]
fn field_skill_zero_hp_damage_shield_contact_can_apply_hit_status() {
    let mut c = short();
    c.skill.damage_bps = 0;
    let mut w = make(&c);
    let t = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "ward", t.clone(), t, 0);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(ticks(&e), vec![(1, 180, 2)]);
    let s = w.snapshot(&c, 180);
    assert_eq!(s.pve.as_ref().unwrap().monsters[0].hp, 180);
    assert!(
        s.pve.unwrap().monsters[0]
            .statuses
            .iter()
            .any(|s| s.definition_id == "exposed")
    );
}
#[test]
fn field_skill_mixed_ground_capacity_is_atomic_and_basic_unaffected() {
    let mut c = short();
    c.skill.windup_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.on_commit_self_statuses = vec!["ward".into()];
    c.skill.warning_ms = 60000;
    c.additional_skills[0] = super::ground_skill_tests::config().skill;
    c.additional_skills[0].windup_ms = 0;
    c.additional_skills[0].recovery_ms = 0;
    c.additional_skills[0].cooldown_ms = 0;
    c.additional_skills[0].warning_ms = 60000;
    c.basic.windup_ms = 0;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 30);
    w.action(
        &Action::CastGround {
            skill_id: c.additional_skills[0].id.clone(),
            position: both(),
        },
        "circle",
        &c,
        30,
    )
    .unwrap();
    advance(&mut w, &c, 30, 60);
    assert_eq!(w.pve.as_ref().unwrap().ground_plan_count(), 2);
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
    assert_eq!(before.statuses, after.statuses);
    assert_eq!(before.player_skills, after.player_skills);
    w.action(&Action::Attack {}, "basic", &c, 90).unwrap();
    assert!(
        advance(&mut w, &c, 90, 120)
            .iter()
            .any(|e| matches!(e, Event::ActionCommitted { skill: false, .. }))
    );
}
#[test]
fn field_skill_full_capacity_still_allows_self_skill() {
    let mut c = short();
    c.skill.windup_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.warning_ms = 60000;
    let mut w = make(&c);
    for n in [0, 30] {
        cast(&mut w, &c, both(), n).unwrap();
        advance(&mut w, &c, n, n + 30);
    }
    w.action(
        &Action::CastSelf {
            skill_id: c.additional_skills[0].id.clone(),
        },
        "self-full",
        &c,
        60,
    )
    .unwrap();
    let e = advance(&mut w, &c, 60, 180);
    assert!(e.iter().any(|e|matches!(e,Event::ActionCommitted{skill_id:Some(id),..} if id==&c.additional_skills[0].id)));
    assert_eq!(w.spirit, 168);
    assert_eq!(w.pve.as_ref().unwrap().ground_plan_count(), 2);
}
#[test]
fn field_skill_views_parameters_and_progress_stay_frozen() {
    let mut c = config();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    c.skill.radius = 1.;
    c.skill.duration_ms = 60000;
    c.skill.tick_interval_ms = 3000;
    advance(&mut w, &c, 0, 1320);
    let v = w
        .snapshot(&c, 1320)
        .pve
        .unwrap()
        .player_fields
        .unwrap()
        .remove(0);
    assert_eq!(v.radius, 80.);
    assert_eq!(v.center, both());
    assert_eq!(v.activates_at_ms, 720);
    assert_eq!(v.expires_at_ms, 3120);
    assert_eq!(v.total_ticks, 3);
    assert_eq!(v.completed_ticks, 1);
    assert_eq!(v.next_tick_at_ms, Some(1920));
    advance(&mut w, &c, 1320, 2520);
    assert_eq!(
        w.snapshot(&c, 2520).pve.unwrap().player_fields.unwrap()[0].next_tick_at_ms,
        None
    );
}
#[test]
fn field_skill_death_terminal_and_close_end_once_and_never_refund() {
    let c = config();
    for case in 0..4 {
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 1320);
        let before = w.pve.as_ref().unwrap().monsters[0].hp;
        let e = match case {
            0 => w.action(&Action::Leave {}, "leave", &c, 1350).unwrap(),
            1 => {
                w.extraction_end = Some(1350);
                advance(&mut w, &c, 1320, 1350)
            }
            2 => w.close_container_interaction(1350),
            _ => {
                let mut a = w.pve.take().unwrap();
                let mut e = vec![];
                a.hurt_player(&mut w, &c, 0, (100000, false), 1350, &mut e);
                w.pve = Some(a);
                e
            }
        };
        assert_eq!(
            e.iter()
                .filter(|e| matches!(e, Event::PlayerFieldEnded { .. }))
                .count(),
            1
        );
        assert_eq!(w.spirit, 188);
        assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
        let e = advance(&mut w, &c, 1350, 3300);
        assert!(ticks(&e).is_empty());
        assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, before);
        assert!(
            !w.close_container_interaction(3330)
                .iter()
                .any(|e| matches!(e, Event::PlayerFieldEnded { .. }))
        );
    }
}
#[test]
fn field_skill_background_cancel_and_move_keep_committed_cycles() {
    let c = config();
    for action in [
        Action::Background {},
        Action::CancelAction {},
        Action::Move {
            direction: [0., 1.],
        },
    ] {
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 120);
        w.action(&action, "after", &c, 150).unwrap();
        let e = advance(&mut w, &c, 150, 3120);
        assert_eq!(ticks(&e).len(), 3);
        assert_eq!(w.spirit, 188);
        assert_eq!(w.skill_ready_at, 1320);
    }
}
#[test]
fn field_skill_expiry_frees_shared_capacity_before_next_commit() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 300);
    assert_eq!(w.pve.as_ref().unwrap().ground_plan_count(), 0);
    assert!(
        w.close_container_interaction(330)
            .iter()
            .all(|e| !matches!(e, Event::PlayerFieldEnded { .. }))
    );
}
#[test]
fn field_skill_old_snapshots_omit_field_views_and_events() {
    for version in 19..30 {
        let mut c = super::ground_skill_tests::config();
        c.mechanism_version = version;
        let w = make(&c);
        let s = serde_json::to_value(w.snapshot(&c, 0)).unwrap();
        assert!(s["pve"].get("player_fields").is_none());
    }
    let c = super::ground_skill_tests::config();
    let mut w = make(&c);
    w.action(
        &Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: both(),
        },
        "old-circle",
        &c,
        0,
    )
    .unwrap();
    let e = advance(&mut w, &c, 0, 1020);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::PlayerAreaResolved { .. }))
            .count(),
        1
    );
    assert!(e.iter().all(|e| !matches!(
        e,
        Event::PlayerFieldTick { .. }
            | Event::PlayerFieldStarted { .. }
            | Event::PlayerFieldEnded { .. }
    )));
}

#[test]
fn field_skill_commit_revalidates_position_range_terrain_and_vision_atomically() {
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
        assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
    }
}
#[test]
fn field_skill_commit_resource_and_self_capacity_fail_without_partial_effects() {
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
        assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
    }
}
#[test]
fn field_skill_prepare_cancel_background_leave_never_spend() {
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
        assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
    }
}
#[test]
fn field_skill_shared_recovery_and_pending_cannot_be_bypassed() {
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
fn field_skill_hundred_closes_release_ground_plans_and_frozen_assets() {
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
fn field_skill_legal_output_peak_keeps_ground_views_and_frozen_asset_responsibility_bounded() {
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
fn field_skill_public_finite_validation_reconnect_and_dedup_preserve_warning() {
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
    for _ in 0..45 {
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
        before.pve.as_ref().unwrap().player_fields.as_ref().unwrap()[0].completed_ticks,
        1
    );
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
        after.pve.unwrap().player_fields,
        before.pve.unwrap().player_fields
    );
    assert_eq!(after.spirit, before.spirit);
    assert_eq!(e.input(input), Err(Error::StaleConnection));
}
#[test]
fn field_skill_rejects_old_capabilities_and_mismatched_intents() {
    let c = config();
    for version in 1..30 {
        let mut c = c.clone();
        c.mechanism_version = version;
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
        Action::CastGround {
            skill_id: c.additional_skills[0].id.clone(),
            position: both(),
        },
    ] {
        assert_eq!(w.action(&a, "wrong", &c, 0), Err(Error::InvalidMessage));
    }
}
#[test]
fn field_skill_primary_secondary_cd_aliases_remain_independent() {
    for swap in [false, true] {
        let mut c = config();
        if swap {
            std::mem::swap(&mut c.skill, &mut c.additional_skills[0]);
        }
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 120);
        let s = w.snapshot(&c, 120);
        assert_eq!(s.skill_ready_at_ms, if swap { 0 } else { 1320 });
        assert_eq!(
            s.player_skills.unwrap()[usize::from(swap)].ready_at_ms,
            1320
        );
        assert_eq!(s.pve.unwrap().player_fields.unwrap().len(), 1);
    }
}

#[test]
fn field_skill_kills_and_drops_once_despite_later_pulses() {
    let mut c = config();
    c.skill.damage_bps = 100000;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 3120);
    assert_eq!(ticks(&e), vec![(1, 1320, 2), (2, 1920, 0), (3, 2520, 0)]);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        2
    );
    assert_eq!(w.kills, 2);
    assert_eq!(
        w.resources
            .iter()
            .filter(|r| !r.view.orb)
            .map(|r| r.view.quantity)
            .sum::<u32>(),
        4
    );
    assert_eq!(w.spirit, 188);
}
#[test]
fn field_skill_direct_pulse_output_peak_releases_world_and_frozen_assets() {
    use crate::asset_contract::tests as s;
    let mut c = short();
    c.output_capacity = 32;
    c.skill.windup_ms = 0;
    c.skill.tick_interval_ms = 30;
    c.skill.damage_bps = 100000;
    c.skill.radius = 128.;
    c.pve_mut().monster_positions = (0..32)
        .map(|i| [450. + f64::from(i % 8) * 10., 350. + f64::from(i / 8) * 10.])
        .collect();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 3;
    let mut e = active_engine_with_config(c);
    let now = e.status().logical_ms;
    let c = e.config.clone();
    cast(e.world.as_mut().unwrap(), &c, [480., 380.], now).unwrap();
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
            commit_id: "field-peak-assets".into(),
        },
    ))
    .unwrap();
    assert_eq!(e.pending_count(), 0);
}
