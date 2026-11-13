use super::status_tests::{active_engine_with_config, advance, bind, world};
use super::*;
fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/repeat-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone()
}
fn short() -> Config {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.speed = 0.1;
    c.skill.projectile_repeat_interval_ms = Some(60);
    c.skill.max_hits = 8;
    c.skill.damage_bps = 1000;
    c
}
fn make(c: &Config) -> World {
    let mut w = world(c);
    for (i, m) in w.pve.as_mut().unwrap().monsters.iter_mut().enumerate() {
        m.aggro = true;
        m.position = [400., 400.];
        m.enabled = i == 0;
    }
    w
}
fn cast(w: &mut World, c: &Config, now: u64) -> Result<Vec<Event>> {
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        &format!("repeat-{now}"),
        c,
        now,
    )
}
fn hits(e: &[Event]) -> Vec<(UnitTarget, u64, u32)> {
    e.iter()
        .filter_map(|e| {
            if let Event::PiercingHit {
                target,
                hit_at_ms: Some(t),
                remaining_hits,
                ..
            } = e
            {
                Some((target.clone(), *t, *remaining_hits))
            } else {
                None
            }
        })
        .collect()
}
fn view(w: &World, c: &Config, now: u64) -> PiercingProjectileView {
    w.snapshot(c, now)
        .pve
        .unwrap()
        .piercing_projectiles
        .unwrap()[0]
        .clone()
}
#[test]
fn projectile_repeat_zero_hits_once_for_the_full_flight() {
    let mut c = short();
    c.skill.projectile_repeat_interval_ms = Some(0);
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!(hits(&e).len(), 1);
    let v = view(&w, &c, 600);
    assert_eq!(v.repeat_interval_ms, Some(0));
    assert_eq!(v.used_hits, 1);
    assert_eq!(v.contacts[0].next_eligible_at_ms, None);
}
#[test]
fn projectile_repeat_equal_interval_updates_latest_time_and_shared_total() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(
        hits(&e).iter().map(|h| h.1).collect::<Vec<_>>(),
        [60, 120, 180]
    );
    let v = view(&w, &c, 180);
    assert_eq!(v.used_hits, 3);
    assert_eq!(v.remaining_hits, 5);
    assert_eq!(v.contacts.len(), 1);
    assert_eq!(v.contacts[0].hit_at_ms, 180);
    assert_eq!(v.contacts[0].next_eligible_at_ms, Some(240));
}
#[test]
fn projectile_repeat_misaligned_interval_uses_next_contacting_step() {
    let mut c = short();
    c.skill.projectile_repeat_interval_ms = Some(61);
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    assert_eq!(
        hits(&advance(&mut w, &c, 0, 240))
            .iter()
            .map(|h| h.1)
            .collect::<Vec<_>>(),
        [60, 150, 240]
    );
}
#[test]
fn projectile_repeat_smaller_than_step_never_hits_twice_in_one_step() {
    let mut c = short();
    c.skill.projectile_repeat_interval_ms = Some(1);
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let h = hits(&advance(&mut w, &c, 0, 150));
    assert_eq!(
        h.iter().map(|h| h.1).collect::<Vec<_>>(),
        [60, 90, 120, 150]
    );
    assert_eq!(view(&w, &c, 150).contacts.len(), 1);
}
#[test]
fn projectile_repeat_no_contact_does_not_schedule_damage_or_catch_up() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 60);
    w.pve.as_mut().unwrap().monsters[0].position = [900., 900.];
    let e = advance(&mut w, &c, 60, 600);
    assert!(hits(&e).is_empty());
    assert_eq!(view(&w, &c, 600).contacts[0].hit_at_ms, 60);
    w.pve.as_mut().unwrap().monsters[0].position = [400., 400.];
    assert_eq!(hits(&advance(&mut w, &c, 600, 630)).len(), 1);
    assert_eq!(view(&w, &c, 630).used_hits, 2);
}
#[test]
fn projectile_repeat_exit_reentry_does_not_reset_interval() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 60);
    w.pve.as_mut().unwrap().monsters[0].position = [900., 900.];
    advance(&mut w, &c, 60, 90);
    w.pve.as_mut().unwrap().monsters[0].position = [400., 400.];
    assert_eq!(hits(&advance(&mut w, &c, 90, 120))[0].1, 120);
}
#[test]
fn projectile_repeat_new_generation_has_independent_eligibility() {
    let mut c = short();
    c.skill.projectile_repeat_interval_ms = Some(1000);
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 60);
    w.pve.as_mut().unwrap().monsters[0].status_generation = 2;
    assert_eq!(hits(&advance(&mut w, &c, 60, 90))[0].0.generation, 2);
    let v = view(&w, &c, 90);
    assert_eq!(v.contacts.len(), 2);
    assert_eq!(v.used_hits, 2);
}
#[test]
fn projectile_repeat_dodge_does_not_consume_or_refresh_success_clock() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 60);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
        .stats
        .insert("dodge".into(), 100);
    assert!(hits(&advance(&mut w, &c, 60, 180)).is_empty());
    let v = view(&w, &c, 180);
    assert_eq!(v.used_hits, 1);
    assert_eq!(v.contacts[0].hit_at_ms, 60);
    assert_eq!(v.contacts[0].next_eligible_at_ms, Some(120));
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
        .stats
        .insert("dodge".into(), 0);
    assert_eq!(hits(&advance(&mut w, &c, 180, 210))[0].1, 210);
    assert_eq!(view(&w, &c, 210).contacts[0].next_eligible_at_ms, Some(270));
}
#[test]
fn projectile_repeat_zero_hp_loss_shield_counts_and_hit_status_reapplies() {
    let mut c = short();
    c.skill.damage_bps = 500;
    let mut w = make(&c);
    let t = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "ward", t.clone(), t, 0);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(hits(&e).len(), 3);
    assert_eq!(e.iter().filter(|e|matches!(e,Event::Damage{source,target,amount:0}if source=="participant"&&target=="status-target-1")).count(),3);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
    assert!(
        w.snapshot(&c, 180).pve.unwrap().monsters[0]
            .statuses
            .iter()
            .any(|s| s.definition_id == "exposed")
    );
}
#[test]
fn projectile_repeat_attack_frozen_defense_read_on_each_success() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 60);
    let hp = w.pve.as_ref().unwrap().monsters[0].hp;
    w.facts.atk_points = 1000;
    Arc::make_mut(&mut w.pve.as_mut().unwrap().monsters[0].profile)
        .stats
        .insert("def".into(), 10000);
    let e = advance(&mut w, &c, 60, 120);
    assert_eq!(hits(&e).len(), 1);
    assert!(w.pve.as_ref().unwrap().monsters[0].hp > hp - 10);
}
#[test]
fn projectile_repeat_interval_and_budget_frozen_at_acceptance() {
    let mut c = short();
    c.skill.windup_ms = 120;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    c.skill.projectile_repeat_interval_ms = Some(60000);
    c.skill.max_hits = 1;
    advance(&mut w, &c, 0, 120);
    let v = view(&w, &c, 120);
    assert_eq!(v.repeat_interval_ms, Some(60));
    assert_eq!(v.max_hits, 8);
    assert_eq!(hits(&advance(&mut w, &c, 120, 240)).len(), 2);
}
#[test]
fn projectile_repeat_flights_have_independent_history_and_charge_only_on_commit() {
    let c = short();
    let mut w = make(&c);
    for now in [0, 30] {
        cast(&mut w, &c, now).unwrap();
        advance(&mut w, &c, now, now + 30);
    }
    advance(&mut w, &c, 60, 180);
    let flights = w
        .snapshot(&c, 180)
        .pve
        .unwrap()
        .piercing_projectiles
        .unwrap();
    assert_eq!(flights.len(), 2);
    assert!(
        flights
            .iter()
            .all(|p| p.used_hits >= 2 && p.contacts.len() == 1)
    );
    assert_eq!(w.spirit, 176);
}
#[test]
fn projectile_repeat_budget_stops_before_extra_target_or_status() {
    let mut c = short();
    c.skill.max_hits = 3;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 150);
    w.pve.as_mut().unwrap().monsters[1].enabled = true;
    let e = advance(&mut w, &c, 150, 180);
    assert_eq!(hits(&e).len(), 1);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 180);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::PiercingEnded {
            reason: PiercingEndReason::HitBudgetExhausted,
            ..
        }
    )));
    assert!(!w.pve.as_ref().unwrap().has_piercing());
    assert!(hits(&advance(&mut w, &c, 180, 600)).is_empty());
}
#[test]
fn projectile_repeat_kill_and_drop_once_with_no_post_death_tick() {
    let mut c = short();
    c.skill.damage_bps = 7500;
    c.skill.max_hits = 8;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!(hits(&e).len(), 4);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        1
    );
    assert_eq!(
        w.resources
            .iter()
            .filter(|r| !r.view.orb)
            .map(|r| r.view.quantity)
            .sum::<u32>(),
        2
    );
}
#[test]
fn projectile_repeat_ttl_excludes_due_repeat_and_keeps_end_reason() {
    let mut c = short();
    c.skill.projectile_ttl_ms = 90;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 240);
    assert_eq!(hits(&e).len(), 1);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::PiercingEnded {
            reason: PiercingEndReason::Expired,
            ..
        }
    )));
}
#[test]
fn projectile_repeat_wall_blocks_before_repeat_even_if_due() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 60);
    Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0].blocks_projectiles = true;
    let e = advance(&mut w, &c, 60, 180);
    assert!(hits(&e).is_empty());
    assert!(e.iter().any(|e| matches!(
        e,
        Event::PiercingEnded {
            reason: PiercingEndReason::Wall,
            ..
        }
    )));
}
#[test]
fn projectile_repeat_old_and_omitted_shapes_have_no_new_fields() {
    for m in [32, 33] {
        let mut c = short();
        c.mechanism_version = m;
        c.skill.projectile_repeat_interval_ms = None;
        let mut w = make(&c);
        cast(&mut w, &c, 0).unwrap();
        let e = advance(&mut w, &c, 0, 180);
        let v = serde_json::to_value(view(&w, &c, 180)).unwrap();
        assert!(v.get("repeat_interval_ms").is_none());
        assert!(v["contacts"][0].get("next_eligible_at_ms").is_none());
        let h = e
            .iter()
            .filter(|e| matches!(e, Event::PiercingHit { .. }))
            .collect::<Vec<_>>();
        assert_eq!(h.len(), 1);
        assert!(
            serde_json::to_value(h[0])
                .unwrap()
                .get("hit_at_ms")
                .is_none()
        );
    }
}
#[test]
fn projectile_repeat_rejects_old_version_in_memory_gate() {
    let mut c = short();
    c.mechanism_version = 32;
    let mut w = make(&c);
    assert_eq!(cast(&mut w, &c, 0), Err(Error::Unsupported));
    assert_eq!(w.spirit, 200);
}
#[test]
fn projectile_repeat_128_successes_use_one_history_slot_and_release() {
    let mut c = short();
    c.skill.projectile_repeat_interval_ms = Some(1);
    c.skill.max_hits = 128;
    c.skill.damage_bps = 0;
    c.params.minimum_damage = 0;
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 30);
    for n in 1..=128 {
        let now = 30 + n * 30;
        let e = advance(&mut w, &c, now - 30, now);
        assert_eq!(hits(&e).len(), 1);
        if n == 127 {
            let v = view(&w, &c, now);
            assert_eq!(v.used_hits, 127);
            assert_eq!(v.contacts.len(), 1);
            assert!(serde_json::to_vec(&v).unwrap().len() < crate::JSONL_LINE_LIMIT);
        }
        if n == 128 {
            assert!(!w.pve.as_ref().unwrap().has_piercing());
        }
    }
}
#[test]
fn projectile_repeat_terminal_death_close_clear_history_without_refund() {
    for case in 0..4 {
        let c = short();
        let mut w = make(&c);
        cast(&mut w, &c, 0).unwrap();
        advance(&mut w, &c, 0, 180);
        assert_eq!(view(&w, &c, 180).used_hits, 3);
        let e = match case {
            0 => w.action(&Action::Leave {}, "leave", &c, 210).unwrap(),
            1 => {
                w.extraction_end = Some(210);
                advance(&mut w, &c, 180, 210)
            }
            2 => w.close_container_interaction(210),
            _ => {
                let mut a = w.pve.take().unwrap();
                let mut e = vec![];
                a.hurt_player(&mut w, &c, 0, (100000, false), 210, &mut e);
                w.pve = Some(a);
                e
            }
        };
        assert_eq!(
            e.iter()
                .filter(|e| matches!(e, Event::PiercingEnded { .. }))
                .count(),
            1
        );
        assert!(!w.pve.as_ref().unwrap().has_piercing());
        assert_eq!(w.spirit, 188);
        assert!(hits(&advance(&mut w, &c, 210, 600)).is_empty());
    }
}
#[test]
fn projectile_repeat_hundred_closes_release_repeated_plans_and_assets() {
    use crate::asset_contract::tests as s;
    for commit in [false, true] {
        for _ in 0..50 {
            let c = short();
            let mut e = active_engine_with_config(c);
            let now = e.status().logical_ms;
            let c = e.config.clone();
            e.world.as_mut().unwrap().pve.as_mut().unwrap().monsters[0].position = [400., 400.];
            cast(e.world.as_mut().unwrap(), &c, now).unwrap();
            if commit {
                for _ in 0..6 {
                    e.tick();
                    e.drain();
                }
                assert!(view(e.world.as_ref().unwrap(), &c, e.status().logical_ms).used_hits >= 2);
            }
            e.close(Close {
                key: s::create(&e.config).key,
                request_id: "repeat-close".into(),
            })
            .unwrap();
            assert_eq!(e.live_worlds(), 0);
            let r = s::requests(&e.drain()).remove(0);
            e.business(s::receipt(
                &e.config,
                &r,
                ReceiptOutcome::Committed {
                    revision: 2,
                    commit_id: "repeat-assets".into(),
                },
            ))
            .unwrap();
            assert_eq!(e.pending_count(), 0);
        }
    }
}

#[test]
fn projectile_repeat_public_reconnect_dedup_keeps_real_history() {
    use crate::asset_contract::tests as s;
    let mut c = short();
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
        5
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
fn projectile_repeat_peak_output_fault_releases_world_preserves_assets() {
    use crate::asset_contract::tests as s;
    let mut c = short();
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
#[test]
fn projectile_repeat_intervals_are_per_identity_and_order_stays_stable() {
    let mut c = short();
    c.skill.max_hits = 4;
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[1].enabled = true;
    cast(&mut w, &c, 0).unwrap();
    let h = hits(&advance(&mut w, &c, 0, 180));
    assert_eq!(
        h.iter().map(|h| h.0.id.as_str()).collect::<Vec<_>>(),
        [
            "status-target-1",
            "status-target-2",
            "status-target-1",
            "status-target-2"
        ]
    );
    assert_eq!(
        h.iter().map(|h| h.1).collect::<Vec<_>>(),
        [60, 60, 120, 120]
    );
}
#[test]
fn projectile_repeat_disabled_target_does_not_refresh_elapsed_clock() {
    let c = short();
    let mut w = make(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 60);
    w.pve.as_mut().unwrap().monsters[0].enabled = false;
    assert!(hits(&advance(&mut w, &c, 60, 180)).is_empty());
    assert_eq!(view(&w, &c, 180).contacts[0].hit_at_ms, 60);
    w.pve.as_mut().unwrap().monsters[0].enabled = true;
    assert_eq!(hits(&advance(&mut w, &c, 180, 210))[0].1, 210);
}
