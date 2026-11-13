use super::status_tests::{active_engine_with_config, advance, bind, world};
use super::*;
fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/line-field-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone()
}
fn short() -> Config {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.warning_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.tick_interval_ms = 60;
    c.skill.duration_ms = 360;
    c.skill.damage_bps = 1000;
    c.skill.on_hit_target_statuses.clear();
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 32;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 120;
    c
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
fn cast(w: &mut World, c: &Config, _p: [f64; 2], now: u64) -> Result<Vec<Event>> {
    w.action(
        &Action::Cast {
            skill_id: c.player_skills().find(|s| s.is_line()).unwrap().id.clone(),
            direction: [1., 0.],
        },
        &format!("line-{now}"),
        c,
        now,
    )
}
fn ticks(e: &[Event]) -> Vec<(u64, Vec<UnitTarget>, u32)> {
    e.iter()
        .filter_map(|e| {
            if let Event::PlayerFieldTick {
                scheduled_at_ms,
                hit_targets: Some(t),
                remaining_hits: Some(r),
                ..
            } = e
            {
                Some((*scheduled_at_ms, t.clone(), *r))
            } else {
                None
            }
        })
        .collect()
}
fn view(w: &World, c: &Config, now: u64) -> PlayerFieldView {
    w.snapshot(c, now).pve.unwrap().player_fields.unwrap()[0].clone()
}
fn only_one(w: &mut World) {
    for (i, m) in w.pve.as_mut().unwrap().monsters.iter_mut().enumerate() {
        m.enabled = i == 0;
    }
}
#[test]
fn line_field_capsule_footprint_endcaps_side_edges_and_degenerate() {
    let c = config();
    let w = make(&c);
    let g = &w.pve.as_ref().unwrap().grid;
    let line = LineFieldView {
        start: [400., 400.],
        end: [600., 400.],
        width: 40.,
    };
    for p in [[500., 434.], [366., 400.], [634., 400.], [600., 434.]] {
        assert!(g.line_field_contact(&line, p, 14.), "{p:?}");
    }
    for p in [
        [500., 434.001],
        [365.999, 400.],
        [634.001, 400.],
        [634., 434.],
    ] {
        assert!(!g.line_field_contact(&line, p, 14.), "{p:?}");
    }
    let line = LineFieldView {
        end: line.start,
        ..line
    };
    assert!(g.line_field_contact(&line, [400., 434.], 14.));
    assert!(!g.line_field_contact(&line, [434., 434.], 14.));
}
#[test]
fn line_field_diagonal_geometry_uses_world_distance() {
    let c = config();
    let w = make(&c);
    let g = &w.pve.as_ref().unwrap().grid;
    let line = LineFieldView {
        start: [400., 400.],
        end: [600., 600.],
        width: 40.,
    };
    let x = 34. / 2f64.sqrt();
    assert!(g.line_field_contact(&line, [500. - x + 0.001, 500. + x - 0.001], 14.));
    assert!(!g.line_field_contact(&line, [500. - x - 0.001, 500. + x + 0.001], 14.));
    assert!(!g.line_field_contact(&line, [400., 600.], 14.));
}
fn wall(w: &mut World, x: usize, y: usize) {
    let a = w.pve.as_mut().unwrap();
    let mut tile = a.grid.config.palette[0].clone();
    tile.blocks_projectiles = true;
    let index = a.grid.config.palette.len();
    Arc::make_mut(&mut a.grid.config).palette.push(tile);
    Arc::make_mut(&mut a.grid.cells)[y * 32 + x] = index;
}
#[test]
fn line_field_wall_and_map_clip_publish_actual_endpoint_and_block_contact() {
    let c = short();
    let mut w = make(&c);
    wall(&mut w, 13, 10);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 30);
    let l = view(&w, &c, 30).line.unwrap();
    assert_eq!(l.start, [400., 400.]);
    assert_eq!(l.end, [496., 400.]);
    assert_eq!(l.width, 48.);
    let e = advance(&mut w, &c, 30, 90);
    assert_eq!(
        ticks(&e)[0]
            .1
            .iter()
            .map(|t| t.id.as_str())
            .collect::<Vec<_>>(),
        ["status-target-1"]
    );
    let mut c = short();
    c.skill.range = 10000.;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 30);
    assert_eq!(view(&w, &c, 30).line.unwrap().end, [1256., 400.]);
}
#[test]
fn line_field_nearest_point_visibility_prevents_side_wall_leak() {
    let c = config();
    let mut w = make(&c);
    wall(&mut w, 12, 10);
    let a = w.pve.as_ref().unwrap();
    let l = LineFieldView {
        start: [400., 390.],
        end: [650., 390.],
        width: 80.,
    };
    assert!(!a.grid.line_field_contact(&l, [500., 430.], 14.));
    assert!(a.grid.line_field_contact(&l, [440., 430.], 14.));
}
#[test]
fn line_field_direction_freezes_at_accept_origin_at_commit_and_geometry_afterwards() {
    let mut c = short();
    c.skill.windup_ms = 60;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    w.position = [420., 420.];
    advance(&mut w, &c, 0, 60);
    let first = view(&w, &c, 60);
    assert_eq!(first.line.as_ref().unwrap().start, [420., 420.]);
    assert_eq!(first.line.as_ref().unwrap().end, [680., 420.]);
    w.position = [800., 800.];
    c.skill.range = 1.;
    c.skill.radius = 1.;
    advance(&mut w, &c, 60, 90);
    assert_eq!(view(&w, &c, 90).line, first.line);
    assert_eq!(w.spirit, 188);
}
#[test]
fn line_field_origin_accept_and_commit_revalidation_are_atomic() {
    let c = config();
    let mut w = make(&c);
    w.position = [1., 1.];
    assert_eq!(cast(&mut w, &c, both(), 0), Err(Error::InvalidTarget));
    assert!(w.pending.is_none());
    let mut c = config();
    c.skill.on_commit_self_statuses = vec!["ward".into()];
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    w.position = [1., 1.];
    let e = advance(&mut w, &c, 0, 120);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::InvalidTarget,
            ..
        }
    )));
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready_at, 0);
    assert!(w.snapshot(&c, 120).statuses.is_empty());
    assert_eq!(w.pve.as_ref().unwrap().ground_plan_count(), 0);
}
#[test]
fn line_field_wrong_intents_directions_and_in_memory_old_versions_reject() {
    for version in 1..34 {
        let mut c = short();
        c.mechanism_version = version;
        let mut w = make(&c);
        assert_eq!(cast(&mut w, &c, both(), 0), Err(Error::Unsupported));
    }
    let c = short();
    for direction in [[0., 0.], [f64::NAN, 0.], [f64::INFINITY, 1.]] {
        let mut w = make(&c);
        assert_eq!(
            w.action(
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
            position: both(),
        },
        Action::CastSelf {
            skill_id: c.skill.id.clone(),
        },
    ] {
        assert_eq!(
            make(&c).action(&action, "bad", &c, 0),
            Err(Error::InvalidMessage)
        );
    }
}
#[test]
fn line_field_schedule_first_tick_end_exclusive_and_nonintegral_repeat() {
    let mut c = short();
    c.skill.warning_ms = 31;
    c.skill.tick_interval_ms = 61;
    c.skill.duration_ms = 244;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 122;
    let mut w = make(&c);
    only_one(&mut w);
    cast(&mut w, &c, both(), 0).unwrap();
    assert!(ticks(&advance(&mut w, &c, 0, 120)).is_empty());
    let e = advance(&mut w, &c, 120, 330);
    assert_eq!(
        ticks(&e)
            .iter()
            .map(|t| (t.0, t.1.len()))
            .collect::<Vec<_>>(),
        [(122, 1), (183, 0), (244, 1)]
    );
    assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
}
#[test]
fn line_field_once_reentry_and_new_generation_identity() {
    let mut c = short();
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    let mut w = make(&c);
    only_one(&mut w);
    cast(&mut w, &c, both(), 0).unwrap();
    assert_eq!(ticks(&advance(&mut w, &c, 0, 90))[0].1.len(), 1);
    w.pve.as_mut().unwrap().monsters[0].position = [500., 900.];
    assert_eq!(ticks(&advance(&mut w, &c, 90, 150))[0].1.len(), 0);
    w.pve.as_mut().unwrap().monsters[0].position = both();
    assert_eq!(ticks(&advance(&mut w, &c, 150, 210))[0].1.len(), 0);
    w.pve.as_mut().unwrap().monsters[0].status_generation += 1;
    assert_eq!(ticks(&advance(&mut w, &c, 210, 270))[0].1[0].generation, 2);
    assert_eq!(view(&w, &c, 270).contact_policy.unwrap().contacts.len(), 2);
}
#[test]
fn line_field_empty_cycles_advance_without_catchup_damage_on_reentry() {
    let c = short();
    let mut w = make(&c);
    only_one(&mut w);
    w.pve.as_mut().unwrap().monsters[0].position = [500., 900.];
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 210);
    assert!(ticks(&e).iter().all(|t| t.1.is_empty()));
    w.pve.as_mut().unwrap().monsters[0].position = both();
    let e = advance(&mut w, &c, 210, 270);
    assert_eq!(ticks(&e).len(), 1);
    assert_eq!(ticks(&e)[0].1.len(), 1);
    assert_eq!(view(&w, &c, 270).contact_policy.unwrap().used_hits, 1);
}
#[test]
fn line_field_late_processing_is_bounded_and_uses_scheduled_times() {
    let c = short();
    let mut w = make(&c);
    only_one(&mut w);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 30);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_player_fields(&mut w, &c, 900, &mut e);
    assert_eq!(
        ticks(&e)
            .iter()
            .map(|t| (t.0, t.1.len()))
            .collect::<Vec<_>>(),
        [(90, 1), (150, 0), (210, 1), (270, 0), (330, 1)]
    );
    assert!(a.player_fields.is_empty());
}
#[test]
fn line_field_path_progress_then_distance_then_id_order_and_budget() {
    let mut c = short();
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 2;
    let mut w = make(&c);
    let a = w.pve.as_mut().unwrap();
    a.monsters[0].position = [550., 400.];
    a.monsters[0].id = "a".into();
    a.monsters[1].position = [450., 410.];
    a.monsters[1].id = "b".into();
    a.monsters[2].position = [450., 400.];
    a.monsters[2].id = "z".into();
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 90);
    assert_eq!(
        ticks(&e)[0]
            .1
            .iter()
            .map(|t| t.id.as_str())
            .collect::<Vec<_>>(),
        ["z", "b"]
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
    assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
}
#[test]
fn line_field_dodge_does_not_spend_slot_or_clock_shield_contact_does() {
    let mut c = short();
    c.skill
        .contact_policy
        .as_mut()
        .unwrap()
        .max_targets_per_tick = 1;
    let mut w = make(&c);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
        .stats
        .insert("dodge".into(), 100);
    let target = w.pve.as_ref().unwrap().status_entity(1);
    bind(&mut w, "ward", target.clone(), target, 0);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 90);
    assert_eq!(ticks(&e)[0].1[0].id, "status-target-2");
    let p = view(&w, &c, 90).contact_policy.unwrap();
    assert_eq!(p.used_hits, 1);
    assert_eq!(p.contacts.len(), 1);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 180);
}
#[test]
fn line_field_direct_attack_frozen_current_defense_and_shield_live() {
    let c = short();
    let mut w = make(&c);
    only_one(&mut w);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 90);
    let first = w.pve.as_ref().unwrap().monsters[0].hp;
    w.facts.atk_points = 10000;
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
        .stats
        .insert("def".into(), 0);
    advance(&mut w, &c, 90, 210);
    assert_eq!(first - w.pve.as_ref().unwrap().monsters[0].hp, 10);
}
#[test]
fn line_field_shared_ground_capacity_is_atomic_and_other_skill_works() {
    let mut c = short();
    c.skill.duration_ms = 6000;
    let mut w = make(&c);
    for now in [0, 30] {
        cast(&mut w, &c, both(), now).unwrap();
        advance(&mut w, &c, now, now + 30);
    }
    let before = w.spirit;
    cast(&mut w, &c, both(), 60).unwrap();
    let e = advance(&mut w, &c, 60, 90);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(w.spirit, before);
    let own = c.additional_skills[0].id.clone();
    w.action(&Action::CastSelf { skill_id: own }, "own", &c, 90)
        .unwrap();
    advance(&mut w, &c, 90, 210);
    assert_eq!(w.spirit, before - 8);
    assert_eq!(w.pve.as_ref().unwrap().ground_plan_count(), 2);
    // Circle plans occupy the same capacity and retain their own geometry.
    let mut c = short();
    c.skill.duration_ms = 6000;
    let mut circle = super::contact_policy_tests::config().skill;
    circle.cooldown_ms = 0;
    circle.recovery_ms = 0;
    circle.windup_ms = 0;
    circle.duration_ms = 6000;
    c.additional_skills = vec![circle.clone()];
    let mut w = make(&c);
    w.action(
        &Action::CastGround {
            skill_id: circle.id,
            position: both(),
        },
        "circle",
        &c,
        0,
    )
    .unwrap();
    advance(&mut w, &c, 0, 30);
    cast(&mut w, &c, both(), 30).unwrap();
    advance(&mut w, &c, 30, 60);
    assert!(view(&w, &c, 60).line.is_none());
    cast(&mut w, &c, both(), 60).unwrap();
    assert!(advance(&mut w, &c, 60, 90).iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
}
#[test]
fn line_field_kill_and_drop_once_budget_stops_future_ticks() {
    let mut c = short();
    c.skill.damage_bps = 100000;
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 1;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 900);
    assert_eq!(ticks(&e).len(), 1);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        1
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 180);
}
#[test]
fn line_field_legacy_circle_view_does_not_add_geometry() {
    let c = super::contact_policy_tests::config();
    let mut w = make(&c);
    w.action(
        &Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: both(),
        },
        "circle",
        &c,
        0,
    )
    .unwrap();
    advance(&mut w, &c, 0, 120);
    let value = serde_json::to_value(view(&w, &c, 120)).unwrap();
    assert!(value.get("line").is_none());
}

#[test]
fn line_field_commit_resource_and_self_capacity_fail_without_partial_effects() {
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
fn line_field_death_terminal_and_close_end_once_and_never_refund() {
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
fn line_field_hundred_closes_release_ground_plans_and_frozen_assets() {
    use crate::asset_contract::tests as s;
    for commit in [false, true] {
        for _ in 0..50 {
            let mut e = active_engine_with_config(config());
            let now = e.status().logical_ms;
            let c = e.config.clone();
            cast(e.world.as_mut().unwrap(), &c, both(), now).unwrap();
            if commit {
                for _ in 0..45 {
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
fn line_field_direct_pulse_output_peak_releases_world_and_frozen_assets() {
    use crate::asset_contract::tests as s;
    let mut c = short();
    c.output_capacity = 32;
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 128;
    c.skill
        .contact_policy
        .as_mut()
        .unwrap()
        .max_targets_per_tick = 32;
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

#[test]
fn line_field_public_finite_validation_reconnect_and_dedup_preserve_warning() {
    use crate::asset_contract::tests as s;
    let mut configured = config();
    configured
        .skill
        .contact_policy
        .as_mut()
        .unwrap()
        .max_targets_per_tick = 1;
    let mut e = active_engine_with_config(configured);
    let c = s::create(&e.config);
    let input = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: "unit-cast".into(),
        sequence: 3,
        action: Action::Cast {
            skill_id: e.config.skill.id.clone(),
            direction: [1., 0.],
        },
    };
    let mut bad = input.clone();
    bad.request_id = "bad-zero".into();
    bad.action = Action::Cast {
        skill_id: e.config.skill.id.clone(),
        direction: [f64::NAN, 0.],
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
    assert_eq!(
        before.pve.as_ref().unwrap().player_fields.as_ref().unwrap()[0]
            .contact_policy
            .as_ref()
            .unwrap()
            .used_hits,
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
fn line_field_background_cancel_and_move_keep_committed_cycles() {
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
fn line_field_prepare_cancel_background_leave_never_spend() {
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
fn line_field_128_successes_keep_one_identity_and_release_budget() {
    let mut c = short();
    c.skill.duration_ms = 3870;
    c.skill.tick_interval_ms = 30;
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 128;
    c.skill
        .contact_policy
        .as_mut()
        .unwrap()
        .max_targets_per_tick = 1;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 30;
    c.skill.damage_bps = 0;
    let mut w = make(&c);
    only_one(&mut w);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 3840);
    let p = view(&w, &c, 3840).contact_policy.unwrap();
    assert_eq!(p.used_hits, 127);
    assert_eq!(p.contacts.len(), 1);
    assert_eq!(p.contacts[0].last_hit_at_ms, 3840);
    let e = advance(&mut w, &c, 3840, 3870);
    assert_eq!(ticks(&e)[0].2, 0);
    assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
}
