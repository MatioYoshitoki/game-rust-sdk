use super::status_tests::{advance, bind, cast, damage, world};
use super::*;
fn config() -> Config {
    let mut c = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/shield-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone();
    c.pve_mut().status_definitions[0].shield_bps = Some(600);
    c.skill.damage_bps = 10000;
    c
}
fn attach(mut c: Config) -> Config {
    let sample = config();
    c.schema_version = 4;
    c.mechanism_version = 22;
    c.asset_contract = sample.asset_contract;
    c.result_contract = sample.result_contract;
    let p = c.pve_mut();
    p.status_definitions = sample.pve.as_ref().unwrap().status_definitions.clone();
    p.status_rules = sample.pve.as_ref().unwrap().status_rules.clone();
    for d in &mut p.status_definitions {
        d.duration_ms = 60000;
    }
    c
}
fn grant(w: &mut World, id: &str, now: u64) {
    bind(w, id, status::participant(), status::participant(), now);
}
fn grant_monster(w: &mut World, id: &str, now: u64) {
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(w, id, status::participant(), m, now);
}
fn hit_player(w: &mut World, c: &Config, raw: u32, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.hurt_player(w, c, 0, (raw, false), now, &mut e);
    w.pve = Some(a);
    e
}
fn hit_monster(w: &mut World, c: &Config, raw: u32, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.hurt_monster(w, c, 0, raw, false, &mut e);
    w.pve = Some(a);
    e
}
fn absorbed(e: &[Event]) -> u32 {
    e.iter()
        .filter_map(|e| match e {
            Event::ShieldAbsorbed { amount, .. } => Some(*amount),
            _ => None,
        })
        .sum()
}
fn received(e: &[Event]) -> Vec<u32> {
    e.iter()
        .filter_map(|e| match e {
            Event::Damage { target, amount, .. } if target == "participant" => Some(*amount),
            _ => None,
        })
        .collect()
}
#[test]
fn shield_absorbs_before_hp_and_depleted_groups_do_not_return() {
    let c = config();
    let mut w = world(&c);
    w.facts.def_points = 0;
    grant(&mut w, "ward", 0);
    let e = hit_player(&mut w, &c, 100, 30);
    assert_eq!((absorbed(&e), received(&e)), (60, vec![40]));
    assert_eq!(w.snapshot(&c, 30).shield_total, Some(0));
    let e = hit_player(&mut w, &c, 100, 60);
    assert_eq!((absorbed(&e), received(&e)), (0, vec![100]));
    assert_eq!(w.hp, 860);
}
#[test]
fn shield_partial_absorption_and_remaining_damage_are_conserved() {
    let c = config();
    let mut w = world(&c);
    w.facts.def_points = 0;
    grant(&mut w, "ward", 0);
    let e = hit_player(&mut w, &c, 40, 30);
    assert_eq!((absorbed(&e), received(&e)), (40, vec![0]));
    assert_eq!(w.snapshot(&c, 30).shield_total, Some(20));
    let e = hit_player(&mut w, &c, 100, 60);
    assert_eq!((absorbed(&e), received(&e)), (20, vec![80]));
    assert_eq!(w.hp, 920);
}
#[test]
fn shield_applies_after_defense_and_before_overkill_hp_clamp() {
    let c = config();
    let mut w = world(&c);
    grant(&mut w, "ward", 0);
    let e = hit_player(&mut w, &c, 100, 30);
    assert_eq!((absorbed(&e), received(&e)), (60, vec![6]));
    let mut w = world(&c);
    w.facts.def_points = 0;
    w.hp = 10;
    grant(&mut w, "ward", 0);
    let e = hit_player(&mut w, &c, 100, 30);
    assert_eq!((absorbed(&e), received(&e)), (60, vec![10]));
    assert_eq!(w.hp, 0);
    assert!(w.pve.as_ref().unwrap().statuses.is_empty());
}
#[test]
fn shield_zero_damage_and_minimum_damage_do_not_double_apply() {
    for minimum in [0, 1] {
        let mut c = config();
        c.params.minimum_damage = minimum;
        let mut w = world(&c);
        grant(&mut w, "ward", 0);
        let e = hit_player(&mut w, &c, 0, 30);
        assert_eq!(absorbed(&e), minimum);
        assert_eq!(received(&e), vec![0]);
        assert_eq!(w.snapshot(&c, 30).shield_total, Some(60 - minimum));
    }
}
#[test]
fn shield_refresh_refills_only_consumed_budget_and_ignore_keeps_everything() {
    use crate::config::DuplicatePolicy;
    for ignore in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[0].duplicate_policy = if ignore {
            DuplicatePolicy::Ignore
        } else {
            DuplicatePolicy::Refresh
        };
        let mut w = world(&c);
        w.facts.def_points = 0;
        grant(&mut w, "ward", 0);
        let id = w.snapshot(&c, 0).statuses[0].instance_id.clone();
        hit_player(&mut w, &c, 20, 30);
        grant(&mut w, "ward", 60);
        let s = w.snapshot(&c, 60).statuses.remove(0);
        assert_eq!(s.instance_id, id);
        assert_eq!(s.shield.unwrap().remaining, if ignore { 40 } else { 60 });
        assert_eq!(s.applied_at_ms, if ignore { 0 } else { 60 });
    }
}
#[test]
fn shield_aggregate_cap_clips_only_new_group_and_zero_grant_has_no_slot() {
    let mut c = config();
    let p = c.pve_mut();
    for d in &mut p.status_definitions {
        d.shield_bps = Some(2000);
    }
    let mut extra = p.status_definitions[0].clone();
    extra.id = "extra".into();
    p.status_definitions.push(extra);
    let mut w = world(&c);
    grant(&mut w, "ward", 0);
    grant(&mut w, "barrier", 0);
    let before = w.snapshot(&c, 0).statuses;
    let e = bind(
        &mut w,
        "extra",
        status::participant(),
        status::participant(),
        30,
    );
    assert_eq!(w.snapshot(&c, 30).statuses, before);
    assert_eq!(w.snapshot(&c, 30).shield_total, Some(300));
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ShieldGranted {
            instance_id: None,
            granted: 0,
            ..
        }
    )));
    grant(&mut w, "ward", 60);
    assert_eq!(
        w.snapshot(&c, 60)
            .statuses
            .iter()
            .find(|s| s.definition_id == "barrier")
            .unwrap()
            .shield
            .as_ref()
            .unwrap()
            .remaining,
        100
    );
}
#[test]
fn shield_cap_zero_tiny_hp_rounding_and_large_integer_are_bounded() {
    for (hp, bps, cap, expected) in [
        (1, 1, 3000, 0),
        (1000, 600, 0, 0),
        (1000000, 10000, 10000, 1000000),
    ] {
        let mut c = config();
        c.pve_mut().status_definitions[0].shield_bps = Some(bps);
        c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(cap);
        let mut w = world(&c);
        w.facts.hp_points = hp;
        grant(&mut w, "ward", 0);
        assert_eq!(w.snapshot(&c, 0).shield_total, Some(expected));
        assert_eq!(w.snapshot(&c, 0).statuses.is_empty(), expected == 0);
        assert_eq!(w.facts.hp_points, hp);
    }
}
#[test]
fn shield_orders_by_deadline_then_applied_time() {
    for same_deadline in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[1].duration_ms = if same_deadline { 3970 } else { 1000 };
        let mut w = world(&c);
        w.facts.def_points = 0;
        grant(&mut w, "ward", 0);
        grant(&mut w, "barrier", 30);
        let first = if same_deadline { "ward" } else { "barrier" };
        let expected = w
            .snapshot(&c, 30)
            .statuses
            .iter()
            .find(|s| s.definition_id == first)
            .unwrap()
            .instance_id
            .clone();
        let e = hit_player(&mut w, &c, 100, 60);
        let rows = e
            .iter()
            .find_map(|e| match e {
                Event::ShieldAbsorbed { consumptions, .. } => Some(consumptions),
                _ => None,
            })
            .unwrap();
        assert_eq!(rows[0].instance_id, expected);
        assert_eq!(
            rows.iter().map(|s| s.absorbed).collect::<Vec<_>>(),
            vec![60, 40]
        );
    }
}
#[test]
fn shield_stable_id_breaks_exact_ties_without_double_consumption() {
    let mut c = config();
    let p = c.pve_mut();
    p.status_definitions.clear();
    p.status_rules.as_mut().unwrap().shield_cap_bps = Some(10000);
    p.status_rules.as_mut().unwrap().max_active_per_entity = 16;
    let mut definition = config().pve.as_ref().unwrap().status_definitions[0].clone();
    definition.shield_bps = Some(10);
    for i in 0..12 {
        let mut d = definition.clone();
        d.id = format!("shield-{i}");
        p.status_definitions.push(d);
    }
    let mut w = world(&c);
    w.facts.def_points = 0;
    for i in 0..12 {
        grant(&mut w, &format!("shield-{i}"), 0);
    }
    let e = hit_player(&mut w, &c, 12, 30);
    let ids = e
        .iter()
        .find_map(|e| match e {
            Event::ShieldAbsorbed { consumptions, .. } => Some(
                consumptions
                    .iter()
                    .map(|v| v.instance_id.clone())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
    assert_eq!(absorbed(&e), 12);
    assert_eq!(w.snapshot(&c, 30).shield_total, Some(0));
}
#[test]
fn shield_expiry_precedes_same_tick_hit_and_regrant_uses_new_id() {
    let mut c = config();
    c.pve_mut().status_definitions[0].duration_ms = 101;
    let mut w = world(&c);
    w.facts.def_points = 0;
    grant(&mut w, "ward", 0);
    let old = w.snapshot(&c, 0).statuses[0].instance_id.clone();
    let e = hit_player(&mut w, &c, 100, 101);
    assert_eq!(absorbed(&e), 0);
    assert_eq!(received(&e), vec![100]);
    grant(&mut w, "ward", 101);
    assert_ne!(w.snapshot(&c, 101).statuses[0].instance_id, old);
    hit_player(&mut w, &c, 60, 102);
    grant(&mut w, "ward", 103);
    assert_eq!(w.snapshot(&c, 103).statuses[0].instance_id, "status-3");
}
#[test]
fn shield_source_scoping_and_latest_owner_cleanup_remain_consistent() {
    for scoped in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[0].source_scoped = scoped;
        let mut w = world(&c);
        grant(&mut w, "ward", 0);
        let m = w.pve.as_ref().unwrap().status_entity(0);
        bind(&mut w, "ward", m.clone(), status::participant(), 30);
        assert_eq!(
            w.snapshot(&c, 30).shield_total,
            Some(if scoped { 120 } else { 60 })
        );
        w.pve.as_mut().unwrap().monsters[0].hp = 0;
        advance(&mut w, &c, 30, 60);
        assert_eq!(
            w.snapshot(&c, 60).shield_total,
            Some(if scoped { 60 } else { 0 })
        );
    }
}
#[test]
fn shield_full_absorption_preserves_contact_but_not_hp_lifesteal() {
    let mut c = config();
    c.pve_mut().combat.lifesteal_ratio = 0.5;
    let mut w = world(&c);
    w.facts.def_points = 0;
    w.extraction_end = Some(5000);
    w.pve.as_mut().unwrap().monsters[0].hp = 500;
    grant(&mut w, "ward", 0);
    let e = hit_player(&mut w, &c, 40, 30);
    assert_eq!(received(&e), vec![0]);
    assert!(w.extraction_end.is_none());
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 500);
    hit_player(&mut w, &c, 100, 60);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 540);
}
#[test]
fn shield_real_player_skill_applies_target_shield_after_current_hit() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!(damage(&e), vec![66]);
    assert_eq!(
        w.snapshot(&c, 600).pve.unwrap().monsters[0].shield_total,
        Some(60)
    );
    cast(&mut w, &c, 900);
    let e = advance(&mut w, &c, 600, 1500);
    assert_eq!(damage(&e), vec![6]);
    assert_eq!(
        e.iter()
            .filter_map(|e| match e {
                Event::ShieldAbsorbed { target, amount, .. }
                    if target.kind == StatusEntityKind::Monster =>
                    Some(*amount),
                _ => None,
            })
            .sum::<u32>(),
        60
    );
}
#[test]
fn shield_basic_and_monster_projectile_both_use_the_same_absorption_chain() {
    let c = config();
    let mut w = world(&c);
    grant_monster(&mut w, "barrier", 0);
    w.action(&Action::Attack {}, "basic", &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!((absorbed(&e), damage(&e)), (60, vec![6]));
    let mut w = world(&c);
    grant(&mut w, "ward", 0);
    w.pve.as_mut().unwrap().monsters[0].casting = Some((0, 90));
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!((absorbed(&e), received(&e)), (60, vec![6]));
}
#[test]
fn shield_dodge_protection_and_zero_contact_do_not_consume() {
    let mut c = config();
    c.pve_mut().stats.insert("dodge".into(), 100);
    let mut w = world(&c);
    grant_monster(&mut w, "barrier", 0);
    let e = hit_monster(&mut w, &c, 100, 30);
    assert_eq!(absorbed(&e), 0);
    assert_eq!(
        w.snapshot(&c, 30).pve.unwrap().monsters[0].shield_total,
        Some(60)
    );
    let mut w = world(&c);
    grant(&mut w, "ward", 0);
    w.protected = true;
    let e = hit_player(&mut w, &c, 100, 30);
    assert_eq!(absorbed(&e), 0);
    assert_eq!(w.snapshot(&c, 30).shield_total, Some(60));
}
#[test]
fn shield_mixed_binding_capacity_rejects_atomically_without_cost() {
    let mut c = config();
    c.skill.on_commit_self_statuses = vec!["ward".into(), "barrier".into()];
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut w = world(&c);
    grant(&mut w, "ward", 0);
    w.facts.def_points = 0;
    hit_player(&mut w, &c, 20, 30);
    let before = w.snapshot(&c, 30).statuses;
    cast(&mut w, &c, 30);
    let e = advance(&mut w, &c, 30, 600);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(w.snapshot(&c, 600).statuses, before);
    assert_eq!((w.spirit, w.skill_ready_at), (200, 0));
}
#[test]
fn shield_area_and_heavy_attacks_consume_shields() {
    let c = attach(super::area_tests::config());
    let mut w = super::area_tests::world(&c);
    grant(&mut w, "ward", 0);
    super::area_tests::start(&mut w, &c, 0, 100);
    let e = super::area_tests::resolve(&mut w, &c, 0, 400);
    assert_eq!((absorbed(&e), received(&e)), (60, vec![40]));
    let c = attach(super::heavy_tests::config());
    let mut w = super::heavy_tests::world(&c);
    grant(&mut w, "ward", 0);
    super::heavy_tests::start(&mut w, &c, 0, 100);
    super::heavy_tests::advance(&mut w, &c, 400);
    let e = super::heavy_tests::advance(&mut w, &c, 1000);
    assert_eq!((absorbed(&e), received(&e)), (60, vec![40]));
}
#[test]
fn shield_counter_after_fully_absorbed_contact_consumes_remaining_shield_once() {
    let mut c = attach(super::heavy_tests::config());
    c.pve_mut().status_definitions[1].shield_bps = Some(1200);
    let mut w = super::heavy_tests::world(&c);
    super::heavy_tests::start(&mut w, &c, 0, 100);
    super::heavy_tests::advance(&mut w, &c, 400);
    w.position = [400., 500.];
    super::heavy_tests::advance(&mut w, &c, 1000);
    grant_monster(&mut w, "barrier", 1000);
    let mut a = w.pve.take().unwrap();
    a.now = 1100;
    let mut e = vec![];
    a.hurt_monster_contact(&mut w, &c, 0, (10, false, Some(1100)), &mut e);
    w.pve = Some(a);
    assert_eq!(absorbed(&e), 60);
    assert_eq!(damage(&e), vec![0]);
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::CounterTriggered { damage: 70, .. }))
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 430);
}
#[test]
fn shield_hundred_closes_and_background_preserve_or_clear_remaining_budget() {
    let c = config();
    for _ in 0..100 {
        let mut w = world(&c);
        w.facts.def_points = 0;
        grant(&mut w, "ward", 0);
        hit_player(&mut w, &c, 20, 30);
        w.action(&Action::Background {}, "bg", &c, 30).unwrap();
        advance(&mut w, &c, 30, 1000);
        assert_eq!(w.snapshot(&c, 1000).shield_total, Some(40));
        w.close_container_interaction(1000);
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    }
}
#[test]
fn shield_legacy_snapshots_and_damage_do_not_gain_shield_fields() {
    for fixture in ["status-host", "stack-host", "attribute-host"] {
        let c = crate::ConfigCatalog::load(
            std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                .join(format!("examples/{fixture}/catalog.json")),
        )
        .unwrap()
        .default_config()
        .as_ref()
        .clone();
        let mut w = world(&c);
        cast(&mut w, &c, 0);
        let e = advance(&mut w, &c, 0, 600);
        assert_eq!(absorbed(&e), 0);
        let v = serde_json::to_value(w.snapshot(&c, 600)).unwrap();
        assert!(v.get("shield_total").is_none());
        assert!(v["statuses"][0].get("shield").is_none());
    }
}

#[test]
fn shield_timed_position_success_and_failure_use_the_common_chain() {
    for success in [false, true] {
        let (old, old_world) = super::timed_position_tests::fixture_for("b3-15");
        let mut c = attach(old);
        let p = c.pve_mut();
        p.stats.insert("def".into(), 0);
        p.stats.insert("hp".into(), 10000);
        p.status_definitions[1].shield_bps = Some(60);
        for t in p.additional_templates.values_mut() {
            t.stats.insert("def".into(), 0);
            t.stats.insert("hp".into(), 10000);
        }
        let mut w = world(&c);
        w.facts.def_points = 0;
        w.position = old_world.position;
        let mut a = w.pve.take().unwrap();
        a.prepare_encounters(&w, 100, &mut vec![]);
        w.pve = Some(a);
        let source = w
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .position(|m| {
                m.id == c
                    .pve
                    .as_ref()
                    .unwrap()
                    .timed_position
                    .as_ref()
                    .unwrap()
                    .source_id
            })
            .unwrap();
        let m = w.pve.as_ref().unwrap().status_entity(source);
        bind(&mut w, "barrier", status::participant(), m, 0);
        grant(&mut w, "ward", 0);
        let warning = super::timed_position_tests::start_warning(&mut w, &c);
        let raw = if success {
            scaled_amount(
                100,
                c.pve
                    .as_ref()
                    .unwrap()
                    .timed_position
                    .as_ref()
                    .unwrap()
                    .success_damage_bps
                    .unwrap(),
                10000,
            )
        } else {
            warning.failure_raw_damage
        };
        if success {
            w.position = w.pve.as_ref().unwrap().monsters[source].position;
        }
        let e = super::timed_position_tests::advance(&mut w, &c, warning.resolves_at_ms);
        assert_eq!(absorbed(&e), raw.min(60));
        if success {
            assert_eq!(damage(&e), vec![raw.saturating_sub(60)]);
        } else {
            assert_eq!(received(&e), vec![raw.saturating_sub(60)]);
        }
    }
}
#[test]
fn shield_encounter_reset_clears_budget_and_rejects_old_round_source() {
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
    let a = w.pve.as_ref().unwrap();
    let i = a
        .monsters
        .iter()
        .position(|m| m.enabled && m.encounter_index == Some(0))
        .unwrap();
    let old = a.status_entity(i);
    bind(&mut w, "ward", old.clone(), status::participant(), 30);
    assert_eq!(w.snapshot(&c, 30).shield_total, Some(60));
    w.position = [0., 0.];
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_encounter_resets(&w, 60, &mut e);
    a.advance_encounter_resets(&w, 60 + delay, &mut e);
    assert!(a.statuses.is_empty());
    a.hit_statuses(
        &w,
        &["ward".into()],
        &old,
        &status::participant(),
        60 + delay,
        &mut e,
    );
    assert!(a.statuses.is_empty());
}
#[test]
fn shield_public_input_and_transport_retries_never_refill_partial_shield() {
    use crate::asset_contract::tests as support;
    let mut c = config();
    c.pve_mut().status_definitions[0].shield_bps = Some(1000);
    let mut e = super::status_tests::active_engine_with_config(c);
    let create = support::create(&e.config);
    let input = Input {
        key: create.key.clone(),
        participant_id: create.participant_id.clone(),
        entity_generation: 1,
        connection: create.connection.clone(),
        request_id: "shield-input".into(),
        sequence: 3,
        action: Action::Cast {
            skill_id: e.config.skill.id.clone(),
            direction: [1., 0.],
        },
    };
    e.input(input.clone()).unwrap();
    e.input(input.clone()).unwrap();
    let mut outputs = vec![];
    for _ in 0..60 {
        e.tick();
        outputs.extend(e.drain());
    }
    let before = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    let remaining = before.shield_total.unwrap();
    assert!(remaining > 0 && remaining < 100);
    e.input(input).unwrap();
    let output = outputs
        .iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap();
    let receipt = TransportReceipt {
        key: create.key,
        recipient_id: create.participant_id,
        connection: create.connection,
        output_id: output.id.clone(),
        outcome: TransportOutcome::Sent,
    };
    e.transport(receipt.clone()).unwrap();
    e.transport(receipt).unwrap();
    let after = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    assert_eq!(before.statuses, after.statuses);
    assert_eq!(before.spirit, after.spirit);
    assert_eq!(after.shield_total, Some(remaining));
    assert!(!e.drain().iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::ShieldGranted { .. }
        }
    )));
}
#[test]
fn shield_full_absorption_does_not_advance_boss_hp_phase_and_walls_do_not_consume() {
    let mut c = attach(super::heavy_tests::config());
    c.pve_mut().status_definitions[1].shield_bps = Some(1200);
    let mut w = super::heavy_tests::world(&c);
    grant_monster(&mut w, "barrier", 0);
    let e = hit_monster(&mut w, &c, 40, 30);
    assert_eq!(absorbed(&e), 40);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].boss_phase, 0);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 500);
    hit_monster(&mut w, &c, 420, 60);
    assert!(w.pve.as_ref().unwrap().monsters[0].boss_phase > 0);
    let mut c = config();
    let p = c.pve_mut();
    p.monster_positions = vec![[600., 400.]];
    let mut wall = p.palette[0].clone();
    wall.blocks_projectiles = true;
    p.palette.push(wall);
    p.cells_rle = vec![(0, 332), (1, 1), (0, 691)];
    let mut w = world(&c);
    grant_monster(&mut w, "barrier", 0);
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!(absorbed(&e), 0);
    assert_eq!(
        w.snapshot(&c, 600).pve.unwrap().monsters[0].shield_total,
        Some(60)
    );
}
