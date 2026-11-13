use super::status_tests::{advance, bind, cast, world};
use super::*;
fn config() -> Config {
    let mut c = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/dot-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone();
    c.pve_mut().stats.insert("hp".into(), 1000);
    c
}
fn attach(mut c: Config) -> Config {
    let sample = config();
    c.schema_version = 4;
    c.mechanism_version = 23;
    c.asset_contract = sample.asset_contract;
    c.result_contract = sample.result_contract;
    let p = c.pve_mut();
    p.status_definitions = sample.pve.as_ref().unwrap().status_definitions.clone();
    p.status_rules = sample.pve.as_ref().unwrap().status_rules.clone();
    c
}
fn apply(w: &mut World, now: u64) -> Vec<Event> {
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(w, "wound", status::participant(), m, now)
}
fn pulse(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.prepare_statuses(w, now, &mut vec![]);
    let mut e = vec![];
    a.advance_periodic(w, c, now, &mut e);
    w.pve = Some(a);
    e
}
fn ticks(e: &[Event]) -> Vec<(u64, u32, u32, PeriodicTickOutcome)> {
    e.iter()
        .filter_map(|e| match e {
            Event::PeriodicTick {
                scheduled_at_ms,
                absorbed,
                hp_damage,
                outcome,
                ..
            } => Some((*scheduled_at_ms, *absorbed, *hp_damage, *outcome)),
            _ => None,
        })
        .collect()
}
fn view(w: &World, c: &Config, now: u64) -> PeriodicView {
    w.snapshot(c, now).pve.unwrap().monsters[0]
        .statuses
        .iter()
        .find_map(|s| s.periodic.clone())
        .unwrap()
}
#[test]
fn periodic_first_last_and_half_open_deadline_are_exact() {
    let c = config();
    let mut w = world(&c);
    apply(&mut w, 0);
    assert_eq!(view(&w, &c, 0).total_ticks, 3);
    assert!(pulse(&mut w, &c, 0).is_empty());
    assert!(pulse(&mut w, &c, 599).is_empty());
    let mut all = vec![];
    for t in [600, 1200, 1800] {
        all.extend(pulse(&mut w, &c, t));
    }
    assert_eq!(
        ticks(&all),
        vec![
            (600, 0, 33, PeriodicTickOutcome::Applied),
            (1200, 0, 33, PeriodicTickOutcome::Applied),
            (1800, 0, 33, PeriodicTickOutcome::Applied)
        ]
    );
    assert!(view(&w, &c, 1800).next_tick_at_ms.is_none());
    assert!(pulse(&mut w, &c, 2200).is_empty());
    assert!(
        w.snapshot(&c, 2200).pve.unwrap().monsters[0]
            .statuses
            .is_empty()
    );
}
#[test]
fn periodic_non_aligned_clock_does_not_drift_and_expiry_wins_last_step() {
    let mut c = config();
    c.pve_mut().status_definitions[1].tick_interval_ms = Some(101);
    c.pve_mut().status_definitions[1].duration_ms = 1000;
    let mut w = world(&c);
    apply(&mut w, 0);
    let e = advance(&mut w, &c, 0, 1000);
    let times: Vec<_> = e
        .iter()
        .filter_map(|e| match e {
            Event::PeriodicTick {
                scheduled_at_ms,
                executed_at_ms,
                ..
            } => Some((*scheduled_at_ms, *executed_at_ms)),
            _ => None,
        })
        .collect();
    assert_eq!(
        times,
        (1_u64..=9)
            .map(|i| (i * 101, (i * 101).div_ceil(30) * 30))
            .collect::<Vec<_>>()
    );
    c.pve_mut().status_definitions[1].duration_ms = 305;
    let mut w = world(&c);
    apply(&mut w, 0);
    let e = advance(&mut w, &c, 0, 330);
    assert_eq!(ticks(&e).len(), 2);
    assert!(
        w.snapshot(&c, 330).pve.unwrap().monsters[0]
            .statuses
            .is_empty()
    );
}
#[test]
fn periodic_late_valid_jobs_are_ordered_but_expired_jobs_are_not_replayed() {
    let c = config();
    let mut w = world(&c);
    apply(&mut w, 0);
    assert_eq!(
        ticks(&pulse(&mut w, &c, 1900))
            .iter()
            .map(|t| t.0)
            .collect::<Vec<_>>(),
        vec![600, 1200, 1800]
    );
    assert!(pulse(&mut w, &c, 1900).is_empty());
    let mut w = world(&c);
    apply(&mut w, 0);
    assert!(pulse(&mut w, &c, 2200).is_empty());
}
#[test]
fn periodic_refresh_recaptures_source_attack_and_ignore_keeps_original_plan() {
    use crate::config::DuplicatePolicy;
    for ignore in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[1].duplicate_policy = if ignore {
            DuplicatePolicy::Ignore
        } else {
            DuplicatePolicy::Refresh
        };
        let mut w = world(&c);
        apply(&mut w, 0);
        pulse(&mut w, &c, 600);
        w.facts.atk_points = 200;
        apply(&mut w, 700);
        let p = view(&w, &c, 700);
        assert_eq!(
            (p.raw_damage, p.ticks_done, p.next_tick_at_ms),
            (
                if ignore { 50 } else { 100 },
                if ignore { 1 } else { 0 },
                Some(if ignore { 1200 } else { 1300 })
            )
        );
    }
}
#[test]
fn periodic_uses_effective_source_attack_but_later_changes_do_not_recalculate() {
    let mut c = config();
    let power = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/attribute-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .pve
    .as_ref()
    .unwrap()
    .status_definitions[0]
        .clone();
    c.pve_mut().status_definitions.push(power);
    let mut w = world(&c);
    bind(
        &mut w,
        "guard",
        status::participant(),
        status::participant(),
        0,
    );
    apply(&mut w, 0);
    assert_eq!(view(&w, &c, 0).raw_damage, 62);
    w.facts.atk_points = 200;
    let e = pulse(&mut w, &c, 600);
    assert_eq!(ticks(&e)[0].2, 41);
}
#[test]
fn periodic_current_defense_and_shield_are_read_each_tick() {
    let mut c = config();
    let mut shield = c.pve.as_ref().unwrap().status_definitions[0].clone();
    shield.id = "barrier".into();
    shield.shield_bps = Some(600);
    c.pve_mut().status_definitions.push(shield);
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "barrier", status::participant(), m, 0);
    apply(&mut w, 0);
    let mut e = vec![];
    for t in [600, 1200, 1800] {
        e.extend(pulse(&mut w, &c, t));
    }
    assert_eq!(
        ticks(&e),
        vec![
            (600, 33, 0, PeriodicTickOutcome::Applied),
            (1200, 27, 6, PeriodicTickOutcome::Applied),
            (1800, 0, 33, PeriodicTickOutcome::Applied)
        ]
    );
}
#[test]
fn periodic_defense_change_after_application_changes_next_damage() {
    let mut c = config();
    let d = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/attribute-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .pve
    .as_ref()
    .unwrap()
    .status_definitions[1]
        .clone();
    c.pve_mut().status_definitions.push(d);
    let mut w = world(&c);
    apply(&mut w, 0);
    assert_eq!(ticks(&pulse(&mut w, &c, 600))[0].2, 33);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    for _ in 0..2 {
        bind(&mut w, "exposed", status::participant(), m.clone(), 700);
    }
    assert_eq!(ticks(&pulse(&mut w, &c, 1200))[0].2, 40);
}
#[test]
fn periodic_zero_raw_damage_obeys_minimum_without_contact_rng() {
    for minimum in [0, 1] {
        let mut c = config();
        c.params.minimum_damage = minimum;
        c.pve_mut().status_definitions[1].damage_bps = Some(0);
        let mut w = world(&c);
        apply(&mut w, 0);
        let seed = w.rng;
        let e = pulse(&mut w, &c, 600);
        assert_eq!(ticks(&e)[0].2, minimum);
        assert_eq!(w.rng, seed);
    }
}
#[test]
fn periodic_does_not_roll_dodge_or_crit_or_repeat_hit_bindings() {
    let mut c = config();
    c.pve_mut().stats.insert("dodge".into(), 100);
    c.pve_mut().combat.projectile.on_hit_target_statuses = vec!["wound".into()];
    let mut w = world(&c);
    apply(&mut w, 0);
    let seed = w.rng;
    let e = pulse(&mut w, &c, 600);
    assert_eq!(ticks(&e)[0].2, 33);
    assert_eq!(w.rng, seed);
    assert!(!e.iter().any(|e| matches!(
        e,
        Event::StatusApplied { .. } | Event::StatusRefreshed { .. }
    )));
}
#[test]
fn periodic_protected_ticks_are_consumed_without_damage_or_later_replay() {
    let c = config();
    let mut w = world(&c);
    apply(&mut w, 0);
    w.action(&Action::Background {}, "bg", &c, 0).unwrap();
    assert_eq!(
        ticks(&pulse(&mut w, &c, 600)),
        vec![(600, 0, 0, PeriodicTickOutcome::Protected)]
    );
    w.foreground = true;
    w.ready = true;
    w.protected = false;
    assert_eq!(view(&w, &c, 700).next_tick_at_ms, Some(1200));
    assert!(pulse(&mut w, &c, 700).is_empty());
    assert_eq!(ticks(&pulse(&mut w, &c, 1200))[0].2, 33);
}
#[test]
fn periodic_returning_ticks_skip_without_cancelling_return_or_extending_status() {
    let c = config();
    let mut w = world(&c);
    apply(&mut w, 0);
    w.pve.as_mut().unwrap().monsters[0].returning = true;
    assert_eq!(
        ticks(&pulse(&mut w, &c, 600))[0].3,
        PeriodicTickOutcome::Returning
    );
    assert!(w.pve.as_ref().unwrap().monsters[0].returning);
    assert_eq!(view(&w, &c, 600).next_tick_at_ms, Some(1200));
}
#[test]
fn periodic_self_target_and_mixed_group_are_rejected_atomically() {
    let c = config();
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hit_statuses(
        &w,
        &["ward".into(), "wound".into()],
        &status::participant(),
        &status::participant(),
        0,
        &mut e,
    );
    assert!(a.statuses.is_empty());
    assert!(matches!(
        e[0],
        Event::StatusRejected {
            reason: Error::InvalidState,
            ..
        }
    ));
}
#[test]
fn periodic_capacity_failure_does_not_refresh_existing_plan() {
    let mut c = config();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut w = world(&c);
    apply(&mut w, 0);
    pulse(&mut w, &c, 600);
    let before = view(&w, &c, 700);
    let target = w.pve.as_ref().unwrap().status_entity(0);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hit_statuses(
        &w,
        &["wound".into(), "ward".into()],
        &status::participant(),
        &target,
        700,
        &mut e,
    );
    w.pve = Some(a);
    assert_eq!(view(&w, &c, 700), before);
    assert!(matches!(
        e[0],
        Event::StatusRejected {
            reason: Error::Capacity,
            ..
        }
    ));
}
#[test]
fn periodic_source_scoped_groups_or_latest_owner_preserve_identity() {
    for scoped in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[1].source_scoped = scoped;
        c.pve_mut().monster_positions.push([500., 400.]);
        let mut w = world(&c);
        let first = w.pve.as_ref().unwrap().status_entity(0);
        let second = w.pve.as_ref().unwrap().status_entity(1);
        bind(&mut w, "wound", first.clone(), status::participant(), 0);
        let before = w.snapshot(&c, 0).statuses[0].clone();
        bind(&mut w, "wound", second.clone(), status::participant(), 60);
        let after = w.snapshot(&c, 60).statuses;
        assert_eq!(after.len(), if scoped { 2 } else { 1 });
        let latest = after.iter().find(|s| s.source == second).unwrap();
        if scoped {
            assert_ne!(before.instance_id, latest.instance_id);
            assert_eq!(after.iter().find(|s| s.source == first).unwrap(), &before);
        } else {
            assert_eq!(before.instance_id, latest.instance_id);
        }
        assert_eq!(latest.applied_at_ms, 60);
        assert_eq!(latest.periodic.as_ref().unwrap().next_tick_at_ms, Some(660));
        w.pve.as_mut().unwrap().monsters[0].hp = 0;
        pulse(&mut w, &c, 600);
        let remaining = w.snapshot(&c, 600).statuses;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].source, second);
    }
}
#[test]
fn periodic_actual_projectile_applies_after_hit_and_first_tick_is_delayed() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 600);
    assert!(ticks(&e).is_empty());
    let p = view(&w, &c, 600);
    assert!(p.next_tick_at_ms.unwrap() > 600);
    let e = advance(&mut w, &c, 600, 900);
    assert_eq!(ticks(&e)[0].2, 33);
}
#[test]
fn periodic_can_kill_and_drop_once_without_another_direct_attack() {
    let mut c = config();
    c.pve_mut().stats.insert("hp".into(), 80);
    let mut w = world(&c);
    apply(&mut w, 0);
    let mut e = vec![];
    for t in [600, 1200, 1800, 2100] {
        e.extend(pulse(&mut w, &c, t));
    }
    assert_eq!(w.kills, 1);
    assert_eq!(w.resources.len(), 2);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        1
    );
    assert_eq!(
        ticks(&e).iter().map(|t| t.2).collect::<Vec<_>>(),
        vec![33, 33, 14]
    );
    assert!(w.pve.as_ref().unwrap().statuses.is_empty());
}
#[test]
fn periodic_monster_source_can_cause_natural_death_without_lifesteal_or_marks() {
    let mut c = config();
    let p = c.pve_mut();
    p.stats.insert("atk".into(), 100);
    p.combat.lifesteal_ratio = 0.5;
    p.combat.mark_max = 3;
    let mut w = world(&c);
    w.hp = 50;
    w.pve.as_mut().unwrap().monsters[0].hp = 500;
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "wound", m, status::participant(), 0);
    let seed = w.rng;
    pulse(&mut w, &c, 600);
    let e = pulse(&mut w, &c, 1200);
    assert_eq!(w.desired_outcome, Some(Outcome::Death));
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 500);
    assert_eq!(w.pve.as_ref().unwrap().marks, 0);
    assert_eq!(w.rng, seed);
    assert_eq!(ticks(&e)[0].2, 17);
    assert!(w.pve.as_ref().unwrap().statuses.is_empty());
}
#[test]
fn periodic_full_absorption_does_not_interrupt_extraction_but_hp_damage_does() {
    let mut c = config();
    c.pve_mut().stats.insert("atk".into(), 100);
    let mut w = world(&c);
    bind(
        &mut w,
        "ward",
        status::participant(),
        status::participant(),
        0,
    );
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "wound", m, status::participant(), 0);
    w.extraction_end = Some(5000);
    pulse(&mut w, &c, 600);
    assert_eq!(w.extraction_end, Some(5000));
    pulse(&mut w, &c, 1200);
    pulse(&mut w, &c, 1800);
    assert_eq!(w.extraction_end, Some(5000));
    c.pve_mut().status_definitions[1].duration_ms = 4000;
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "wound", m, status::participant(), 0);
    w.extraction_end = Some(5000);
    pulse(&mut w, &c, 600);
    assert!(w.extraction_end.is_none());
}
#[test]
fn periodic_source_death_or_generation_change_stops_future_damage() {
    let c = config();
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "wound", m, status::participant(), 0);
    w.pve.as_mut().unwrap().monsters[0].status_generation += 1;
    assert!(pulse(&mut w, &c, 600).is_empty());
    assert!(w.snapshot(&c, 600).statuses.is_empty());
}
#[test]
fn periodic_hundred_closes_and_absolute_terminal_never_emit_late_ticks() {
    let c = config();
    for _ in 0..100 {
        let mut w = world(&c);
        apply(&mut w, 0);
        pulse(&mut w, &c, 600);
        w.close_container_interaction(700);
        assert!(pulse(&mut w, &c, 1200).is_empty());
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    }
    let mut c = c;
    c.terminal_ms = 600;
    let mut w = world(&c);
    apply(&mut w, 0);
    let e = w.tick(&c, 600);
    assert!(ticks(&e).is_empty());
    assert_eq!(w.desired_outcome, Some(Outcome::Death));
}
#[test]
fn periodic_simultaneous_jobs_are_stably_ordered_and_dead_targets_cancel_rest() {
    let mut c = config();
    let mut extra = c.pve.as_ref().unwrap().status_definitions[1].clone();
    extra.id = "extra".into();
    c.pve_mut().status_definitions.push(extra);
    c.pve_mut().stats.insert("hp".into(), 40);
    let mut w = world(&c);
    apply(&mut w, 0);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "extra", status::participant(), m, 0);
    let e = pulse(&mut w, &c, 600);
    let ids: Vec<_> = e
        .iter()
        .filter_map(|e| match e {
            Event::PeriodicTick { instance_id, .. } => Some(instance_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, vec!["status-1", "status-2"]);
    assert_eq!(w.kills, 1);
    assert!(pulse(&mut w, &c, 1200).is_empty());
}
#[test]
fn periodic_encounter_reset_invalidates_old_round_and_pending_jobs() {
    let c = attach(
        crate::config::default_catalog()
            .active("b3-13")
            .unwrap()
            .as_ref()
            .clone(),
    );
    let center = c.pve.as_ref().unwrap().encounters[0].trigger.center;
    let delay = c.pve.as_ref().unwrap().encounters[0]
        .reset
        .as_ref()
        .unwrap()
        .delay_ms;
    let mut w = world(&c);
    w.position = center;
    advance(&mut w, &c, 0, 30);
    let i = w
        .pve
        .as_ref()
        .unwrap()
        .monsters
        .iter()
        .position(|m| m.enabled && m.encounter_index == Some(0))
        .unwrap();
    let old = w.pve.as_ref().unwrap().status_entity(i);
    bind(&mut w, "wound", status::participant(), old.clone(), 30);
    w.position = [0., 0.];
    let mut a = w.pve.take().unwrap();
    a.advance_encounter_resets(&w, 60, &mut vec![]);
    a.now = 630;
    let mut e = vec![];
    a.advance_periodic(&mut w, &c, 630, &mut e);
    assert_eq!(ticks(&e)[0].3, PeriodicTickOutcome::Returning);
    a.advance_encounter_resets(&w, 60 + delay, &mut e);
    assert!(a.statuses.is_empty());
    a.hit_statuses(
        &w,
        &["wound".into()],
        &status::participant(),
        &old,
        60 + delay,
        &mut e,
    );
    assert!(a.statuses.is_empty());
}
#[test]
fn periodic_old_views_do_not_gain_periodic_fields() {
    for name in ["status-host", "stack-host", "attribute-host", "shield-host"] {
        let c = crate::ConfigCatalog::load(
            std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                .join(format!("examples/{name}/catalog.json")),
        )
        .unwrap()
        .default_config()
        .as_ref()
        .clone();
        let mut w = world(&c);
        cast(&mut w, &c, 0);
        advance(&mut w, &c, 0, 600);
        let value = serde_json::to_value(w.snapshot(&c, 600)).unwrap();
        assert!(value["statuses"][0].get("periodic").is_none());
    }
}
#[test]
fn periodic_large_attack_and_damage_ratio_are_bounded_by_existing_math() {
    let mut c = config();
    c.pve_mut().status_definitions[1].damage_bps = Some(100000);
    let mut w = world(&c);
    w.facts.atk_points = 11000000;
    apply(&mut w, 0);
    assert_eq!(view(&w, &c, 0).raw_damage, 110000000);
    let e = pulse(&mut w, &c, 600);
    assert_eq!(ticks(&e)[0].2, 1000);
    assert_eq!(w.kills, 1);
}

#[test]
fn periodic_pulses_do_not_consume_counter_or_interrupt_support_casts() {
    use super::heavy_tests as h;
    let c = attach(h::config());
    let mut w = h::world(&c);
    h::start(&mut w, &c, 0, 100);
    h::advance(&mut w, &c, 400);
    w.position = [400., 500.];
    h::advance(&mut w, &c, 1000);
    let before = h::window(&w).unwrap();
    apply(&mut w, 500);
    let e = pulse(&mut w, &c, 1100);
    assert_eq!(h::window(&w).unwrap(), before);
    assert!(!ticks(&e).is_empty());
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::CounterTriggered { .. }))
    );

    use super::support_tests as s;
    let c = attach(s::config());
    let mut w = s::world(&c);
    s::start(&mut w, &c, 100);
    let target = w.pve.as_ref().unwrap().status_entity(1);
    bind(&mut w, "wound", status::participant(), target, 0);
    let e = pulse(&mut w, &c, 600);
    assert!(!ticks(&e).is_empty());
    assert!(w.pve.as_ref().unwrap().monsters[1].support_cast.is_some());
    assert_eq!(w.pve.as_ref().unwrap().support_interrupt_ready, 0);
}
#[test]
fn periodic_public_input_retry_and_reconnect_preserve_next_tick() {
    use crate::asset_contract::tests as s;
    let mut e = super::status_tests::active_engine_with_config(config());
    let create = s::create(&e.config);
    let input = Input {
        key: create.key.clone(),
        participant_id: create.participant_id.clone(),
        entity_generation: 1,
        connection: create.connection.clone(),
        request_id: "periodic-cast".into(),
        sequence: 3,
        action: Action::Cast {
            skill_id: e.config.skill.id.clone(),
            direction: [1., 0.],
        },
    };
    assert_eq!(e.input(input.clone()), e.input(input.clone()));
    for _ in 0..25 {
        e.tick();
        e.drain();
    }
    let before = view(e.world.as_ref().unwrap(), &e.config, e.status().logical_ms);
    e.input(input).unwrap();
    assert_eq!(
        view(e.world.as_ref().unwrap(), &e.config, e.status().logical_ms),
        before
    );
    e.input(Input {
        key: create.key,
        participant_id: create.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "periodic-replacement".into(),
            generation: 2,
        },
        request_id: "periodic-reconnect".into(),
        sequence: 4,
        action: Action::Reconnect {
            previous_connection: create.connection,
            authenticated_principal: create.authenticated_principal,
        },
    })
    .unwrap();
    assert_eq!(
        view(e.world.as_ref().unwrap(), &e.config, e.status().logical_ms),
        before
    );
}
#[test]
fn periodic_event_burst_releases_world_and_retains_frozen_settlement() {
    let mut c = config();
    c.output_capacity = 32;
    c.pve_mut().status_definitions[1].tick_interval_ms = Some(30);
    c.pve_mut().monster_positions = (0..32).map(|i| [500. + f64::from(i) * 10., 600.]).collect();
    let mut e = super::status_tests::active_engine_with_config(c);
    let now = e.status().logical_ms;
    let w = e.world.as_mut().unwrap();
    let targets: Vec<_> = (0..32)
        .map(|i| w.pve.as_ref().unwrap().status_entity(i))
        .collect();
    for target in targets {
        bind(w, "wound", status::participant(), target, now);
    }
    e.tick();
    let outputs = e.drain();
    assert_eq!(e.live_worlds(), 0);
    assert!(outputs.len() <= 32);
    assert!(outputs.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::Fault {
                code: Error::QueueFull
            }
        }
    )));
    let request = crate::asset_contract::tests::requests(&outputs).remove(0);
    assert!(matches!(
        request.body,
        Business::CommitAssets {
            transaction: AssetTransaction::ResultSettlement { .. }
        }
    ));
    let receipt = crate::asset_contract::tests::receipt(
        &e.config,
        &request,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "periodic-burst-result".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
}
