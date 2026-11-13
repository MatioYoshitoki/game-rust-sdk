use super::status_tests::{active_engine_with_config, advance, bind, world};
use super::*;
pub(super) fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/contact-host/catalog.json"),
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
    w.action(
        &Action::CastGround {
            skill_id: c
                .player_skills()
                .find(|s| s.contact_policy.is_some())
                .unwrap()
                .id
                .clone(),
            position: p,
        },
        &format!("contact-{now}"),
        c,
        now,
    )
}
fn short() -> Config {
    let mut c = config();
    c.skill.warning_ms = 0;
    c.skill.tick_interval_ms = 60;
    c.skill.duration_ms = 300;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 120;
    c
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
fn policy(w: &World, c: &Config, now: u64) -> FieldContactPolicyView {
    w.snapshot(c, now).pve.unwrap().player_fields.unwrap()[0]
        .contact_policy
        .clone()
        .unwrap()
}
#[test]
fn contact_policy_once_per_identity_advances_empty_cycles_then_expires() {
    let mut c = short();
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 420);
    let t = ticks(&e);
    assert_eq!(
        t.iter().map(|t| t.1.len()).collect::<Vec<_>>(),
        vec![1, 1, 0, 0]
    );
    assert_eq!(t.iter().map(|t| t.2).collect::<Vec<_>>(), vec![2, 1, 1, 1]);
    assert_eq!(t[0].1[0].id, "status-target-1");
    assert_eq!(t[1].1[0].id, "status-target-2");
    assert!(e.iter().any(|e| matches!(
        e,
        Event::PlayerFieldEnded {
            reason: PlayerFieldEndReason::Expired,
            ..
        }
    )));
    assert_eq!(w.spirit, 188);
}
#[test]
fn contact_policy_positive_repeat_equal_boundary_and_budget_end() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 420);
    let t = ticks(&e);
    assert_eq!(
        t.iter().map(|t| t.0).collect::<Vec<_>>(),
        vec![180, 240, 300]
    );
    assert_eq!(
        t.iter().map(|t| t.1[0].id.as_str()).collect::<Vec<_>>(),
        vec!["status-target-1", "status-target-2", "status-target-1"]
    );
    assert_eq!(t.last().unwrap().2, 0);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 70);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 130);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::PlayerFieldEnded {
            reason: PlayerFieldEndReason::HitBudgetExhausted,
            ..
        }
    )));
    assert_eq!(w.pve.as_ref().unwrap().ground_plan_count(), 0);
    assert_eq!(w.skill_ready_at, 1320);
    assert_eq!(w.spirit, 188);
}
#[test]
fn contact_policy_repeat_is_independent_of_scan_and_late_processing() {
    let mut c = short();
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 121;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 120);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_player_fields(&mut w, &c, 900, &mut e);
    let t = ticks(&e);
    assert_eq!(
        t.iter().map(|t| (t.0, t.1.len())).collect::<Vec<_>>(),
        vec![(180, 1), (240, 1), (300, 0), (360, 1)]
    );
    assert!(a.player_fields.is_empty());
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::PlayerFieldEnded { .. }))
            .count(),
        1
    );
}
#[test]
fn contact_policy_nearest_then_id_order_overrides_storage_index() {
    for tie in [false, true] {
        let c = short();
        let mut w = make(&c);
        let a = w.pve.as_mut().unwrap();
        if tie {
            a.monsters[0].id = "z".into();
            a.monsters[1].id = "a".into();
        } else {
            a.monsters[0].position = [500., 400.];
            a.monsters[1].position = both();
        }
        cast(&mut w, &c, both(), 0).unwrap();
        let t = ticks(&advance(&mut w, &c, 0, 180));
        assert_eq!(t[0].1[0].id, if tie { "a" } else { "status-target-2" });
    }
}
#[test]
fn contact_policy_cycle_and_total_caps_stop_before_extra_damage_and_status() {
    let mut c = short();
    let p = c.skill.contact_policy.as_mut().unwrap();
    p.max_targets_per_tick = 2;
    p.max_total_hits = 1;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 420);
    assert_eq!(ticks(&e).len(), 1);
    assert_eq!(ticks(&e)[0].1.len(), 1);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 180);
    assert!(
        w.snapshot(&c, 420).pve.unwrap().monsters[1]
            .statuses
            .is_empty()
    );
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::PlayerFieldEnded { .. }))
            .count(),
        1
    );
    assert!(ticks(&advance(&mut w, &c, 420, 900)).is_empty());
}
#[test]
fn contact_policy_dodge_does_not_spend_slot_budget_or_history() {
    let mut c = short();
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    let mut w = make(&c);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
        .stats
        .insert("dodge".into(), 100);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(ticks(&e)[0].1[0].id, "status-target-2");
    let p = policy(&w, &c, 180);
    assert_eq!(p.used_hits, 1);
    assert_eq!(p.contacts.len(), 1);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
        .stats
        .insert("dodge".into(), 0);
    let e = advance(&mut w, &c, 180, 240);
    assert_eq!(ticks(&e)[0].1[0].id, "status-target-1");
}
#[test]
fn contact_policy_shield_absorption_counts_zero_hp_loss_contact() {
    let mut c = short();
    c.skill.damage_bps = 1000;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    let mut w = make(&c);
    let t = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "ward", t.clone(), t, 0);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(ticks(&e)[0].1[0].id, "status-target-1");
    assert!(
        e.iter()
            .any(|e| matches!(e,Event::Damage{target,amount:0,..} if target=="status-target-1"))
    );
    assert_eq!(policy(&w, &c, 180).used_hits, 1);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
    assert!(
        w.snapshot(&c, 180).pve.unwrap().monsters[0]
            .statuses
            .iter()
            .any(|s| s.definition_id == "exposed")
    );
}
#[test]
fn contact_policy_zero_damage_without_shield_still_consumes_contact() {
    let mut c = short();
    c.skill.damage_bps = 0;
    c.params.minimum_damage = 0;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 180);
    assert_eq!(policy(&w, &c, 180).used_hits, 1);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
}
#[test]
fn contact_policy_blocked_and_ineligible_targets_never_spend() {
    let c = short();
    for block in [false, true] {
        let mut w = make(&c);
        cast(&mut w, &c, both(), 0).unwrap();
        advance(&mut w, &c, 0, 150);
        if block {
            Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0].blocks_projectiles =
                true;
        } else {
            for m in &mut w.pve.as_mut().unwrap().monsters {
                m.enabled = false;
            }
        }
        let e = advance(&mut w, &c, 150, 180);
        assert!(ticks(&e)[0].1.is_empty());
        assert_eq!(policy(&w, &c, 180).used_hits, 0);
    }
}
#[test]
fn contact_policy_exit_reentry_keeps_old_history_but_new_generation_is_distinct() {
    let mut c = short();
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[1].enabled = false;
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 180);
    w.pve.as_mut().unwrap().monsters[0].position = [700., 400.];
    assert!(ticks(&advance(&mut w, &c, 180, 240))[0].1.is_empty());
    w.pve.as_mut().unwrap().monsters[0].position = [450., 360.];
    assert!(ticks(&advance(&mut w, &c, 240, 300))[0].1.is_empty());
    w.pve.as_mut().unwrap().monsters[0].status_generation += 1;
    let t = ticks(&advance(&mut w, &c, 300, 360));
    assert_eq!(t[0].1[0].generation, 2);
    let p = policy(&w, &c, 360);
    assert_eq!(
        p.contacts
            .iter()
            .map(|h| h.target.generation)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}
#[test]
fn contact_policy_configuration_and_attack_are_frozen_at_acceptance() {
    let mut c = short();
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 1;
    advance(&mut w, &c, 0, 120);
    w.facts.atk_points = 1000;
    assert_eq!(policy(&w, &c, 120).max_total_hits, 3);
    let e = advance(&mut w, &c, 120, 420);
    assert_eq!(ticks(&e).len(), 3);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 70);
}
#[test]
fn contact_policy_multiple_regions_have_independent_history_and_budget() {
    let mut c = short();
    c.skill.windup_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    let mut w = make(&c);
    for now in [0, 30] {
        cast(&mut w, &c, both(), now).unwrap();
        advance(&mut w, &c, now, now + 30);
    }
    let e = advance(&mut w, &c, 60, 120);
    let t = ticks(&e);
    assert_eq!(t.len(), 2);
    assert!(t.iter().all(|t| t.1[0].id == "status-target-1" && t.2 == 2));
    let v = w.snapshot(&c, 120).pve.unwrap().player_fields.unwrap();
    assert!(
        v.iter()
            .all(|f| f.contact_policy.as_ref().unwrap().contacts.len() == 1)
    );
    assert_eq!(w.spirit, 176);
}
#[test]
fn contact_policy_budget_has_priority_over_expiry_after_last_legal_pulse() {
    let mut c = short();
    c.skill.duration_ms = 61;
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 1;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 120);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_player_fields(&mut w, &c, 900, &mut e);
    assert_eq!(ticks(&e).len(), 1);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(
                e,
                Event::PlayerFieldEnded {
                    reason: PlayerFieldEndReason::HitBudgetExhausted,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(!e.iter().any(|e| matches!(
        e,
        Event::PlayerFieldEnded {
            reason: PlayerFieldEndReason::Expired,
            ..
        }
    )));
}
#[test]
fn contact_policy_kill_consumes_once_and_never_repeats_drop() {
    let mut c = short();
    c.skill.damage_bps = 100000;
    let mut w = make(&c);
    cast(&mut w, &c, both(), 0).unwrap();
    let e = advance(&mut w, &c, 0, 420);
    assert_eq!(
        ticks(&e).iter().map(|t| t.1.len()).collect::<Vec<_>>(),
        vec![1, 1, 0, 0]
    );
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        2
    );
    assert_eq!(
        w.resources
            .iter()
            .filter(|r| !r.view.orb)
            .map(|r| r.view.quantity)
            .sum::<u32>(),
        4
    );
}
#[test]
fn contact_policy_old_and_omitted_views_keep_no_new_fields() {
    let mut c = super::field_skill_tests::config();
    for version in [30, 31] {
        c.mechanism_version = version;
        let mut w = make(&c);
        w.action(
            &Action::CastGround {
                skill_id: c.skill.id.clone(),
                position: both(),
            },
            "old",
            &c,
            0,
        )
        .unwrap();
        let e = advance(&mut w, &c, 0, 1320);
        let v = serde_json::to_value(w.snapshot(&c, 1320)).unwrap();
        assert!(v["pve"]["player_fields"][0].get("contact_policy").is_none());
        for e in e {
            if matches!(e, Event::PlayerFieldTick { .. }) {
                let e = serde_json::to_value(e).unwrap();
                assert!(e.get("remaining_hits").is_none() && e.get("hit_targets").is_none());
            }
        }
    }
}
#[test]
fn contact_policy_128_history_is_bounded_published_and_released() {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.warning_ms = 0;
    c.skill.tick_interval_ms = 30;
    c.skill.duration_ms = 3870;
    c.skill.contact_policy = Some(crate::config::ContactPolicyConfig {
        max_targets_per_tick: 1,
        max_total_hits: 128,
        repeat_interval_ms: 0,
    });
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[1].enabled = false;
    w.pve.as_mut().unwrap().monsters[0].id = "x".repeat(128);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 30);
    for n in 1..=128 {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.status_generation = n;
        m.hp = 180;
        let now = 30 + n as u64 * 30;
        let e = advance(&mut w, &c, now - 30, now);
        assert_eq!(ticks(&e)[0].1.len(), 1);
        if n == 127 {
            let p = policy(&w, &c, now);
            assert_eq!(p.contacts.len(), 127);
            assert_eq!(p.remaining_hits, 1);
            assert!(
                serde_json::to_vec(&w.snapshot(&c, now)).unwrap().len() < crate::JSONL_LINE_LIMIT
            );
        }
        if n == 128 {
            assert!(e.iter().any(|e| matches!(
                e,
                Event::PlayerFieldEnded {
                    reason: PlayerFieldEndReason::HitBudgetExhausted,
                    ..
                }
            )));
        }
    }
    assert!(w.pve.as_ref().unwrap().player_fields.is_empty());
}

#[test]
fn contact_policy_commit_revalidates_position_range_terrain_and_vision_atomically() {
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
fn contact_policy_commit_resource_and_self_capacity_fail_without_partial_effects() {
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
fn contact_policy_death_terminal_and_close_end_once_and_never_refund() {
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
fn contact_policy_hundred_closes_release_ground_plans_and_frozen_assets() {
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
fn contact_policy_direct_pulse_output_peak_releases_world_and_frozen_assets() {
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
fn contact_policy_public_finite_validation_reconnect_and_dedup_preserve_warning() {
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
fn contact_policy_old_versions_cannot_prepare_explicit_policy() {
    let c = config();
    for version in 1..31 {
        let mut c = c.clone();
        c.mechanism_version = version;
        let mut w = make(&c);
        assert_eq!(cast(&mut w, &c, both(), 0), Err(Error::Unsupported));
    }
}

#[test]
fn contact_policy_budget_stays_spent_when_hit_status_capacity_rejects() {
    let mut c = short();
    c.skill.damage_bps = 1000;
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut w = make(&c);
    let t = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "ward", t.clone(), t, 0);
    cast(&mut w, &c, both(), 0).unwrap();
    advance(&mut w, &c, 0, 180);
    assert_eq!(policy(&w, &c, 180).used_hits, 1);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
    assert!(
        !w.snapshot(&c, 180).pve.unwrap().monsters[0]
            .statuses
            .iter()
            .any(|s| s.definition_id == "exposed")
    );
}
#[test]
fn contact_policy_budget_end_releases_slot_and_new_region_has_fresh_history() {
    let mut c = short();
    c.skill.windup_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.tick_interval_ms = 30;
    c.skill.duration_ms = 90;
    c.skill.contact_policy.as_mut().unwrap().max_total_hits = 1;
    c.skill.contact_policy.as_mut().unwrap().repeat_interval_ms = 0;
    let mut w = make(&c);
    for now in [0, 60] {
        cast(&mut w, &c, both(), now).unwrap();
        let e = advance(&mut w, &c, now, now + 60);
        assert_eq!(ticks(&e).len(), 1);
        assert_eq!(ticks(&e)[0].1[0].id, "status-target-1");
        assert_eq!(w.pve.as_ref().unwrap().ground_plan_count(), 0);
    }
    assert_eq!(w.spirit, 176);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 70);
}
#[test]
fn contact_policy_omitted_field_keeps_original_index_contact_order() {
    let mut c = super::field_skill_tests::config();
    c.mechanism_version = 31;
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[1].position = both();
    w.action(
        &Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: both(),
        },
        "old-order",
        &c,
        0,
    )
    .unwrap();
    let e = advance(&mut w, &c, 0, 1320);
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
