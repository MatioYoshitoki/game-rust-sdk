use super::status_tests::{active_engine_with_config, advance, bind, cast, world};
use super::*;
fn config() -> Config {
    let mut c = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/hot-host/catalog.json"),
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
    c.mechanism_version = 24;
    c.asset_contract = sample.asset_contract;
    c.result_contract = sample.result_contract;
    c.pve_mut().status_definitions = sample.pve.as_ref().unwrap().status_definitions.clone();
    c.pve_mut().status_rules = sample.pve.as_ref().unwrap().status_rules.clone();
    c
}
fn grant(w: &mut World, id: &str, entity: StatusEntity, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.commit_statuses(w, &[id.into()], &entity, now, &mut e);
    w.pve = Some(a);
    e
}
fn apply(w: &mut World, now: u64) -> Vec<Event> {
    grant(w, "renewal", status::participant(), now)
}
fn pulse(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.prepare_statuses(w, now, &mut vec![]);
    let mut e = vec![];
    a.advance_periodic(w, c, now, &mut e);
    w.pve = Some(a);
    e
}
fn view(w: &World, c: &Config, now: u64) -> StatusView {
    w.snapshot(c, now)
        .statuses
        .into_iter()
        .find(|s| {
            s.periodic
                .as_ref()
                .is_some_and(|p| p.kind == PeriodicEffectKind::MaxHpHeal)
        })
        .unwrap()
}
fn ticks(e: &[Event]) -> Vec<(u64, u32, PeriodicTickOutcome)> {
    e.iter()
        .filter_map(|e| {
            if let Event::PeriodicHealTick {
                scheduled_at_ms,
                hp_restored,
                outcome,
                ..
            } = e
            {
                Some((*scheduled_at_ms, *hp_restored, *outcome))
            } else {
                None
            }
        })
        .collect()
}
fn add_dot(c: &mut Config) {
    let v: serde_json::Value =
        serde_json::from_str(crate::test_data::fixture!("examples/dot-host/arena.json")).unwrap();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(v["pve"]["status_definitions"][1].clone()).unwrap());
    c.pve_mut().stats.insert("atk".into(), 100);
}
#[test]
fn hot_first_last_and_half_open_deadline_are_exact() {
    let c = config();
    let mut w = world(&c);
    w.hp = 500;
    apply(&mut w, 0);
    assert!(pulse(&mut w, &c, 599).is_empty());
    let mut e = vec![];
    for t in [600, 1200, 1800] {
        e.extend(pulse(&mut w, &c, t));
    }
    assert_eq!(
        ticks(&e),
        vec![
            (600, 100, PeriodicTickOutcome::Applied),
            (1200, 100, PeriodicTickOutcome::Applied),
            (1800, 100, PeriodicTickOutcome::Applied)
        ]
    );
    assert_eq!(w.hp, 800);
    assert_eq!(view(&w, &c, 1800).periodic.unwrap().next_tick_at_ms, None);
    assert!(pulse(&mut w, &c, 2200).is_empty());
    assert!(w.snapshot(&c, 2200).statuses.is_empty());
}
#[test]
fn hot_non_aligned_clock_does_not_drift_and_expiry_wins() {
    let mut c = config();
    c.pve_mut().status_definitions[0].tick_interval_ms = Some(101);
    c.pve_mut().status_definitions[0].duration_ms = 305;
    let mut w = world(&c);
    w.hp = 500;
    apply(&mut w, 0);
    let e = advance(&mut w, &c, 0, 330);
    let times: Vec<_> = e
        .iter()
        .filter_map(|e| {
            if let Event::PeriodicHealTick {
                scheduled_at_ms,
                executed_at_ms,
                ..
            } = e
            {
                Some((*scheduled_at_ms, *executed_at_ms))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(times, vec![(101, 120), (202, 210)]);
}
#[test]
fn hot_full_hp_clamps_actual_heal_and_consumes_zero_tick() {
    let c = config();
    let mut w = world(&c);
    w.hp = 850;
    apply(&mut w, 0);
    let seed = w.rng;
    let mut amounts = vec![];
    for t in [600, 1200, 1800] {
        amounts.push(ticks(&pulse(&mut w, &c, t))[0].1);
    }
    assert_eq!(amounts, vec![100, 50, 0]);
    assert_eq!(w.hp, 1000);
    assert_eq!(w.rng, seed);
    assert_eq!(view(&w, &c, 1800).periodic.unwrap().ticks_done, 3);
}
#[test]
fn hot_zero_and_tiny_heal_do_not_use_minimum_damage() {
    for bps in [0, 1] {
        let mut c = config();
        c.params.minimum_damage = 100;
        c.pve_mut().status_definitions[0].heal_bps = Some(bps);
        let mut w = world(&c);
        w.hp = 500;
        apply(&mut w, 0);
        assert_eq!(view(&w, &c, 0).periodic.unwrap().raw_heal, Some(0));
        assert_eq!(ticks(&pulse(&mut w, &c, 600))[0].1, 0);
        assert_eq!(w.hp, 500);
    }
}
#[test]
fn hot_refresh_recaptures_max_hp_and_ignore_keeps_plan() {
    for ignore in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[0].duplicate_policy = if ignore {
            crate::config::DuplicatePolicy::Ignore
        } else {
            crate::config::DuplicatePolicy::Refresh
        };
        let mut w = world(&c);
        w.hp = 500;
        apply(&mut w, 0);
        let initial = view(&w, &c, 0);
        pulse(&mut w, &c, 600);
        w.facts.hp_points = 2000;
        apply(&mut w, 700);
        let latest = view(&w, &c, 700);
        assert_eq!(latest.instance_id, initial.instance_id);
        let p = latest.periodic.unwrap();
        assert_eq!(p.raw_heal, Some(if ignore { 100 } else { 200 }));
        assert_eq!(p.ticks_done, if ignore { 1 } else { 0 });
        assert_eq!(p.next_tick_at_ms, Some(if ignore { 1200 } else { 1300 }));
    }
}
#[test]
fn hot_attack_defense_shields_and_permanent_facts_are_unchanged() {
    let mut c = config();
    let shield: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/shield-host/arena.json"
    ))
    .unwrap();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(shield["pve"]["status_definitions"][0].clone()).unwrap());
    let mut w = world(&c);
    w.hp = 500;
    grant(&mut w, "ward", status::participant(), 0);
    apply(&mut w, 0);
    let shield = w.snapshot(&c, 0).shield_total;
    let facts = w.facts.clone();
    w.facts.atk_points = 1000000;
    w.facts.def_points = 1000000;
    let e = pulse(&mut w, &c, 600);
    assert_eq!(ticks(&e)[0].1, 100);
    assert_eq!(w.snapshot(&c, 600).shield_total, shield);
    assert_eq!(w.facts.hp_points, facts.hp_points);
    assert_eq!(w.spirit, 200);
    assert!(!e.iter().any(|e| matches!(
        e,
        Event::Damage { .. }
            | Event::ShieldAbsorbed { .. }
            | Event::StatusApplied { .. }
            | Event::CounterTriggered { .. }
    )));
}
#[test]
fn hot_self_entities_keep_independent_groups_and_refresh_identity() {
    for scoped in [true, false] {
        let mut c = config();
        c.pve_mut().status_definitions[0].source_scoped = scoped;
        c.pve_mut().monster_positions.push([500., 400.]);
        let mut w = world(&c);
        let targets: Vec<_> = (0..2)
            .map(|i| w.pve.as_ref().unwrap().status_entity(i))
            .collect();
        for target in &targets {
            grant(&mut w, "renewal", target.clone(), 0);
        }
        let initial = w.snapshot(&c, 0).pve.unwrap().monsters[0].statuses[0]
            .instance_id
            .clone();
        grant(&mut w, "renewal", targets[0].clone(), 60);
        let s = w.snapshot(&c, 60).pve.unwrap();
        assert_eq!(s.monsters[0].statuses[0].instance_id, initial);
        assert_ne!(s.monsters[1].statuses[0].instance_id, initial);
        assert_eq!(s.monsters[1].statuses[0].applied_at_ms, 0);
    }
}
#[test]
fn hot_hit_target_and_mixed_group_reject_atomically() {
    let c = config();
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    for target in [m, status::participant()] {
        let e = bind(&mut w, "renewal", status::participant(), target, 0);
        assert!(matches!(
            e[0],
            Event::StatusRejected {
                reason: Error::InvalidState,
                ..
            }
        ));
    }
    assert!(w.snapshot(&c, 0).statuses.is_empty());
}
#[test]
fn hot_capacity_failure_does_not_refresh_or_charge() {
    let mut c = config();
    let shield: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/shield-host/arena.json"
    ))
    .unwrap();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(shield["pve"]["status_definitions"][0].clone()).unwrap());
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    c.skill.on_commit_self_statuses = vec!["renewal".into(), "ward".into()];
    let mut w = world(&c);
    apply(&mut w, 0);
    pulse(&mut w, &c, 600);
    let before = view(&w, &c, 700);
    let spirit = w.spirit;
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "full",
        &c,
        700,
    )
    .unwrap();
    let e = advance(&mut w, &c, 700, 810);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(view(&w, &c, 700), before);
    assert_eq!(w.spirit, spirit);
    assert_eq!(w.skill_ready_at, 0);
}
#[test]
fn hot_real_player_commit_delays_first_heal_and_pays_once() {
    let c = config();
    let mut w = world(&c);
    w.hp = 500;
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 600);
    assert!(ticks(&e).is_empty());
    let p = view(&w, &c, 600).periodic.unwrap();
    assert_eq!(p.next_tick_at_ms, Some(690));
    assert_eq!(w.spirit, 190);
    assert_eq!(ticks(&advance(&mut w, &c, 600, 900)).len(), 1);
}
#[test]
fn hot_precommit_cancellation_never_grants_a_plan() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    w.action(&Action::Background {}, "bg", &c, 30).unwrap();
    let e = advance(&mut w, &c, 0, 900);
    assert!(ticks(&e).is_empty());
    assert!(w.snapshot(&c, 900).statuses.is_empty());
    assert_eq!(w.spirit, 200);
}
#[test]
fn hot_real_monster_projectile_commit_uses_the_same_self_effect() {
    let mut c = config();
    c.pve_mut().combat.projectile.on_commit_self_statuses = vec!["renewal".into()];
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[0].hp = 500;
    w.pve.as_mut().unwrap().monsters[0].casting = Some((0, 90));
    advance(&mut w, &c, 0, 120);
    let s = w.snapshot(&c, 120).pve.unwrap().monsters[0].statuses[0].clone();
    assert_eq!(s.source, s.target);
    assert_eq!(s.periodic.unwrap().raw_heal, Some(100));
    let e = pulse(&mut w, &c, 690);
    assert_eq!(ticks(&e)[0].1, 100);
}
#[test]
fn hot_full_and_partial_heals_do_not_roll_rng_or_trigger_contact() {
    let c = config();
    let mut w = world(&c);
    w.hp = 950;
    apply(&mut w, 0);
    let seed = w.rng;
    let e = pulse(&mut w, &c, 600);
    assert_eq!(ticks(&e)[0].1, 50);
    assert_eq!(w.rng, seed);
    assert!(
        e.iter()
            .all(|e| matches!(e, Event::PeriodicHealTick { .. }))
    );
    assert_eq!(w.pve.as_ref().unwrap().marks, 0);
}
#[test]
fn hot_protected_background_and_not_ready_ticks_are_consumed() {
    for mode in 0..3 {
        let c = config();
        let mut w = world(&c);
        w.hp = 500;
        apply(&mut w, 0);
        match mode {
            0 => w.protected = true,
            1 => w.foreground = false,
            _ => w.ready = false,
        };
        assert_eq!(
            ticks(&pulse(&mut w, &c, 600)),
            vec![(600, 0, PeriodicTickOutcome::Protected)]
        );
        w.protected = false;
        w.foreground = true;
        w.ready = true;
        assert!(pulse(&mut w, &c, 700).is_empty());
        assert_eq!(ticks(&pulse(&mut w, &c, 1200))[0].1, 100);
        assert_eq!(w.hp, 600);
    }
}
#[test]
fn hot_returning_skips_without_refilling_hp_or_extending_plan() {
    let c = config();
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    grant(&mut w, "renewal", m, 0);
    let monster = &mut w.pve.as_mut().unwrap().monsters[0];
    monster.hp = 500;
    monster.returning = true;
    assert_eq!(
        ticks(&pulse(&mut w, &c, 600)),
        vec![(600, 0, PeriodicTickOutcome::Returning)]
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 500);
}
#[test]
fn hot_death_or_generation_change_clears_and_never_revives() {
    for dead in [false, true] {
        let c = config();
        let mut w = world(&c);
        let m = w.pve.as_ref().unwrap().status_entity(0);
        grant(&mut w, "renewal", m, 0);
        if dead {
            w.pve.as_mut().unwrap().monsters[0].hp = 0;
        } else {
            w.pve.as_mut().unwrap().monsters[0].status_generation += 1;
        }
        assert!(pulse(&mut w, &c, 600).is_empty());
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    }
    let c = config();
    let mut w = world(&c);
    apply(&mut w, 0);
    w.hp = 0;
    assert!(pulse(&mut w, &c, 600).is_empty());
    assert_eq!(w.hp, 0);
    assert!(
        apply(&mut w, 600)
            .iter()
            .any(|e| matches!(e, Event::StatusRejected { .. }))
    );
}
#[test]
fn hot_hundred_closes_and_absolute_terminal_leave_no_late_heal() {
    let mut c = config();
    for _ in 0..100 {
        let mut w = world(&c);
        w.hp = 500;
        apply(&mut w, 0);
        w.close_container_interaction(300);
        assert!(pulse(&mut w, &c, 600).is_empty());
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    }
    c.terminal_ms = 600;
    let mut w = world(&c);
    w.hp = 500;
    apply(&mut w, 0);
    assert!(ticks(&w.tick(&c, 600)).is_empty());
    assert_eq!(w.desired_outcome, Some(Outcome::Death));
}
#[test]
fn hot_same_due_dot_kills_first_even_when_hot_has_older_id() {
    for player_target in [true, false] {
        let mut c = config();
        add_dot(&mut c);
        let mut w = world(&c);
        let m = w.pve.as_ref().unwrap().status_entity(0);
        let (source, target) = if player_target {
            w.hp = 10;
            (m, status::participant())
        } else {
            w.pve.as_mut().unwrap().monsters[0].hp = 10;
            (status::participant(), m)
        };
        grant(&mut w, "renewal", target.clone(), 0);
        bind(&mut w, "wound", source, target, 0);
        let e = pulse(&mut w, &c, 600);
        assert!(ticks(&e).is_empty());
        assert!(
            e.iter()
                .any(|e| matches!(e, Event::PeriodicTick { hp_damage: 10, .. }))
        );
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    }
}
#[test]
fn hot_same_due_nonlethal_dot_precedes_heal() {
    let mut c = config();
    add_dot(&mut c);
    let mut w = world(&c);
    w.hp = 50;
    apply(&mut w, 0);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "wound", m, status::participant(), 0);
    let e = pulse(&mut w, &c, 600);
    assert_eq!(w.hp, 117);
    let damage = e
        .iter()
        .position(|e| matches!(e, Event::PeriodicTick { .. }))
        .unwrap();
    let heal = e
        .iter()
        .position(|e| matches!(e, Event::PeriodicHealTick { .. }))
        .unwrap();
    assert!(damage < heal);
}
#[test]
fn hot_earlier_scheduled_heal_precedes_later_dot_during_catchup() {
    let mut c = config();
    add_dot(&mut c);
    let mut w = world(&c);
    w.hp = 10;
    apply(&mut w, 0);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "wound", m, status::participant(), 1);
    let e = pulse(&mut w, &c, 610);
    assert_eq!(w.hp, 77);
    assert_eq!(ticks(&e)[0].0, 600);
    assert!(matches!(e[0], Event::PeriodicHealTick { .. }));
}
#[test]
fn hot_same_family_order_is_stable_and_reads_never_advance() {
    let mut c = config();
    let mut extra = c.pve.as_ref().unwrap().status_definitions[0].clone();
    extra.id = "extra".into();
    c.pve_mut().status_definitions.push(extra);
    let mut w = world(&c);
    w.hp = 500;
    apply(&mut w, 0);
    grant(&mut w, "extra", status::participant(), 0);
    for _ in 0..10 {
        assert_eq!(view(&w, &c, 600).periodic.unwrap().ticks_done, 0);
    }
    let e = pulse(&mut w, &c, 600);
    let ids: Vec<_> = e
        .iter()
        .filter_map(|e| {
            if let Event::PeriodicHealTick { instance_id, .. } = e {
                Some(instance_id.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ids, vec!["status-1", "status-2"]);
    assert!(pulse(&mut w, &c, 600).is_empty());
    assert_eq!(w.hp, 700);
}
#[test]
fn hot_does_not_interrupt_extraction_or_active_support() {
    let c = config();
    let mut w = world(&c);
    w.hp = 500;
    apply(&mut w, 0);
    w.extraction_end = Some(5000);
    pulse(&mut w, &c, 600);
    assert_eq!(w.extraction_end, Some(5000));
    use super::support_tests as s;
    let c = attach(s::config());
    let mut w = s::world(&c);
    s::start(&mut w, &c, 100);
    let m = w.pve.as_ref().unwrap().status_entity(1);
    grant(&mut w, "renewal", m, 0);
    pulse(&mut w, &c, 600);
    assert!(w.pve.as_ref().unwrap().monsters[1].support_cast.is_some());
    assert_eq!(w.pve.as_ref().unwrap().support_interrupt_ready, 0);
}
#[test]
fn hot_public_retry_and_reconnect_preserve_progress() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine_with_config(config());
    e.world.as_mut().unwrap().hp = 500;
    let c = s::create(&e.config);
    let input = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: "hot-cast".into(),
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
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "hot-replacement".into(),
            generation: 2,
        },
        request_id: "hot-reconnect".into(),
        sequence: 4,
        action: Action::Reconnect {
            previous_connection: c.connection,
            authenticated_principal: c.authenticated_principal,
        },
    })
    .unwrap();
    assert_eq!(
        view(e.world.as_ref().unwrap(), &e.config, e.status().logical_ms),
        before
    );
}
#[test]
fn hot_lethal_dot_naturally_settles_without_resurrection() {
    let mut c = config();
    add_dot(&mut c);
    c.pve_mut().combat.melee_range = 1.;
    let mut e = active_engine_with_config(c);
    let now = e.status().logical_ms;
    let w = e.world.as_mut().unwrap();
    w.hp = 10;
    apply(w, now);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(w, "wound", m, status::participant(), now);
    let mut outputs = vec![];
    for _ in 0..20 {
        e.tick();
        outputs.extend(e.drain());
    }
    assert_eq!(e.status().phase, Phase::Draining);
    assert_eq!(e.world.as_ref().unwrap().hp, 0);
    assert!(!outputs.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::PeriodicHealTick { .. }
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
            commit_id: "hot-natural-death".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
    assert_eq!(e.live_worlds(), 0);
}
#[test]
fn hot_event_burst_with_legal_capacity_releases_world_and_settles() {
    let mut c = config();
    c.output_capacity = 32;
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
    for i in 0..32 {
        let target = w.pve.as_ref().unwrap().status_entity(i);
        grant(w, "renewal", target, now);
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
    let receipt = crate::asset_contract::tests::receipt(
        &e.config,
        &request,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "hot-peak".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
}
#[test]
fn hot_legacy_views_keep_zero_raw_damage_and_omit_heal_fields() {
    for name in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
    ] {
        let mut c = crate::ConfigCatalog::load(
            std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                .join(format!("examples/{name}/catalog.json")),
        )
        .unwrap()
        .default_config()
        .as_ref()
        .clone();
        if name == "dot-host" {
            c.pve_mut().status_definitions[1].damage_bps = Some(0);
        }
        let mut w = world(&c);
        cast(&mut w, &c, 0);
        advance(&mut w, &c, 0, 600);
        let v = serde_json::to_value(w.snapshot(&c, 600)).unwrap();
        for status in v["statuses"].as_array().into_iter().flatten().chain(
            v["pve"]["monsters"][0]["statuses"]
                .as_array()
                .into_iter()
                .flatten(),
        ) {
            if let Some(p) = status.get("periodic") {
                assert!(p.get("raw_heal").is_none());
                assert_eq!(p["raw_damage"], 0);
            }
        }
    }
}
#[test]
fn hot_maximum_ratio_uses_wide_math_and_never_reduces_overcap_hp() {
    let mut c = config();
    c.pve_mut().status_definitions[0].heal_bps = Some(10000);
    let mut w = world(&c);
    w.facts.hp_points = 1000000;
    w.hp = 1;
    apply(&mut w, 0);
    assert_eq!(view(&w, &c, 0).periodic.unwrap().raw_heal, Some(1000000));
    assert_eq!(ticks(&pulse(&mut w, &c, 600))[0].1, 999999);
    w.hp = 1000001;
    assert_eq!(ticks(&pulse(&mut w, &c, 1200))[0].1, 0);
    assert_eq!(w.hp, 1000001);
}
#[test]
fn hot_encounter_reset_clears_plan_and_rejects_old_round() {
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
    grant(&mut w, "renewal", old.clone(), 30);
    w.position = [0., 0.];
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_encounter_resets(&w, 60, &mut e);
    a.prepare_statuses(&w, 630, &mut e);
    a.advance_periodic(&mut w, &c, 630, &mut e);
    assert_eq!(ticks(&e)[0].2, PeriodicTickOutcome::Returning);
    a.advance_encounter_resets(&w, 60 + delay, &mut e);
    assert!(a.statuses.is_empty());
    assert!(!a.commit_statuses(&w, &["renewal".into()], &old, 60 + delay, &mut e));
    assert!(a.statuses.is_empty());
}
#[test]
fn hot_healing_does_not_rewind_boss_phase_or_gain_aggro() {
    let c = attach(super::heavy_tests::config());
    let mut w = super::heavy_tests::world(&c);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hurt_monster(&mut w, &c, 0, 420, false, &mut e);
    assert!(a.monsters[0].boss_phase > 0);
    let phase = a.monsters[0].boss_phase;
    a.monsters[0].aggro = false;
    let m = a.status_entity(0);
    w.pve = Some(a);
    grant(&mut w, "renewal", m, 0);
    pulse(&mut w, &c, 600);
    let monster = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!(monster.boss_phase, phase);
    assert!(!monster.aggro);
    assert_eq!(monster.hp, 130);
}
