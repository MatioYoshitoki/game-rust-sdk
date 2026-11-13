use super::status_tests::{active_engine_with_config, advance, bind, cast, world};
use super::*;
fn config() -> Config {
    let mut c = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/heal-modifier-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone();
    c.pve_mut().stats.insert("hp".into(), 1000);
    c.pve_mut().status_definitions[0].heal_bps = Some(1000);
    c.pve_mut().status_definitions[1].duration_ms = 1200;
    c
}
fn attach(mut c: Config) -> Config {
    let s = config();
    c.schema_version = 4;
    c.mechanism_version = 25;
    c.asset_contract = s.asset_contract;
    c.result_contract = s.result_contract;
    c.pve_mut().status_definitions = s.pve.as_ref().unwrap().status_definitions.clone();
    c.pve_mut().status_rules = s.pve.as_ref().unwrap().status_rules.clone();
    c
}
fn grant(w: &mut World, ids: &[&str], source: StatusEntity, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.commit_statuses(
        w,
        &ids.iter().map(|v| (*v).into()).collect::<Vec<_>>(),
        &source,
        now,
        &mut e,
    );
    w.pve = Some(a);
    e
}
fn care(w: &mut World, now: u64) {
    grant(w, &["care"], status::participant(), now);
}
fn pulse(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.prepare_statuses(w, now, &mut e);
    a.advance_periodic(w, c, now, &mut e);
    w.pve = Some(a);
    e
}
fn resolve(w: &mut World, c: &Config, raw: u32, now: u64) -> (u32, Vec<Event>) {
    let a = w.pve.as_ref().unwrap();
    let mut e = vec![];
    let n = a.resolve_heal(
        w,
        c,
        (&status::participant(), &status::participant()),
        (HealKind::Hot, raw),
        now,
        &mut e,
    );
    w.hp += n;
    (n, e)
}
fn amounts(e: &[Event]) -> Vec<(u32, u32, u64, u32)> {
    e.iter()
        .filter_map(|e| {
            if let Event::HealResolved {
                raw_heal,
                multiplier_bps,
                adjusted_heal,
                hp_restored,
                ..
            } = e
            {
                Some((*raw_heal, *multiplier_bps, *adjusted_heal, *hp_restored))
            } else {
                None
            }
        })
        .collect()
}
fn extra(c: &mut Config, id: &str, bps: i32) {
    let mut d = c.pve.as_ref().unwrap().status_definitions[1].clone();
    d.id = id.into();
    d.modifier_bps = bps;
    c.pve_mut().status_definitions.push(d);
}
#[test]
fn received_exact_weakening_amplification_and_aggregation() {
    for (bps, want) in [(-5000, 50), (5000, 150), (-2500, 75)] {
        let mut c = config();
        c.pve_mut().status_definitions[1].modifier_bps = bps;
        let mut w = world(&c);
        w.hp = 500;
        care(&mut w, 0);
        let (n, e) = resolve(&mut w, &c, 100, 30);
        assert_eq!(n, want);
        assert_eq!(amounts(&e)[0].2, u64::from(want));
    }
}
#[test]
fn received_multiple_definitions_add_before_one_floor() {
    let mut c = config();
    extra(&mut c, "blessing", 2500);
    let mut w = world(&c);
    w.hp = 500;
    care(&mut w, 0);
    grant(&mut w, &["blessing"], status::participant(), 0);
    assert_eq!(resolve(&mut w, &c, 100, 30).0, 75);
}
#[test]
fn received_negative_sum_floors_at_zero_without_damage_minimum() {
    let mut c = config();
    extra(&mut c, "second", -10000);
    c.params.minimum_damage = 999;
    let mut w = world(&c);
    w.hp = 500;
    care(&mut w, 0);
    grant(&mut w, &["second"], status::participant(), 0);
    let (n, e) = resolve(&mut w, &c, 100, 30);
    assert_eq!(n, 0);
    assert_eq!(amounts(&e)[0], (100, 0, 0, 0));
}
#[test]
fn received_positive_sum_has_no_attack_or_defense_gameplay_cap() {
    let mut c = config();
    c.pve_mut().status_definitions[1].modifier_bps = 100000;
    extra(&mut c, "second", 100000);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .attack_bonus_cap_bps = Some(0);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .defense_bonus_cap_bps = 0;
    let mut w = world(&c);
    w.facts.hp_points = 10000;
    w.hp = 1;
    care(&mut w, 0);
    grant(&mut w, &["second"], status::participant(), 0);
    assert_eq!(resolve(&mut w, &c, 100, 30).0, 2100);
    assert_eq!(w.snapshot(&c, 30).healing_received_bps, Some(210000));
}
#[test]
fn received_source_scoping_and_latest_owner_cleanup_are_preserved() {
    for scoped in [true, false] {
        let mut c = config();
        c.pve_mut().status_definitions[1].source_scoped = scoped;
        c.pve_mut().monster_positions.push([500., 400.]);
        let mut w = world(&c);
        let first = w.pve.as_ref().unwrap().status_entity(0);
        let second = w.pve.as_ref().unwrap().status_entity(1);
        bind(&mut w, "care", first, status::participant(), 0);
        bind(&mut w, "care", second.clone(), status::participant(), 30);
        assert_eq!(
            w.snapshot(&c, 30).healing_received_bps,
            Some(if scoped { 0 } else { 5000 })
        );
        w.pve.as_mut().unwrap().monsters[0].hp = 0;
        pulse(&mut w, &c, 60);
        let s = w.snapshot(&c, 60);
        assert_eq!(s.healing_received_bps, Some(5000));
        assert_eq!(s.statuses.len(), 1);
        assert_eq!(s.statuses[0].source, second);
    }
}
#[test]
fn received_refresh_and_ignore_keep_their_identity_contract() {
    for ignore in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[1].duplicate_policy = if ignore {
            crate::config::DuplicatePolicy::Ignore
        } else {
            crate::config::DuplicatePolicy::Refresh
        };
        let mut w = world(&c);
        care(&mut w, 0);
        let first = w.snapshot(&c, 0).statuses[0].clone();
        care(&mut w, 600);
        let after = w.snapshot(&c, 600).statuses[0].clone();
        assert_eq!(after.instance_id, first.instance_id);
        assert_eq!(after.expires_at_ms, if ignore { 1200 } else { 1800 });
    }
}
#[test]
fn received_reading_does_not_modify_facts_attack_defense_or_shield() {
    let c = config();
    let mut w = world(&c);
    care(&mut w, 0);
    for _ in 0..10 {
        let s = w.snapshot(&c, 30);
        assert_eq!(s.healing_received_bps, Some(5000));
        assert_eq!(s.effective_atk, Some(100));
        assert_eq!(s.effective_def, Some(100));
        assert_eq!(s.shield_total, Some(0));
    }
    assert_eq!(w.facts.hp_points, 1000);
    assert_eq!(w.hp, 1000);
    assert_eq!(w.spirit, 200);
}
#[test]
fn received_capacity_failure_does_not_refresh_or_charge() {
    let mut c = config();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut w = world(&c);
    care(&mut w, 0);
    let before = w.snapshot(&c, 600).statuses;
    cast(&mut w, &c, 600);
    let e = advance(&mut w, &c, 600, 720);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(w.snapshot(&c, 720).statuses, before);
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready_at, 0);
}
#[test]
fn received_tiny_zero_and_full_heals_use_one_floor_and_clip() {
    let c = config();
    let mut w = world(&c);
    care(&mut w, 0);
    w.hp = 500;
    assert_eq!(resolve(&mut w, &c, 1, 30).0, 0);
    assert_eq!(resolve(&mut w, &c, 0, 30).0, 0);
    w.hp = 990;
    let (n, e) = resolve(&mut w, &c, 100, 30);
    assert_eq!(n, 10);
    assert_eq!(amounts(&e)[0], (100, 5000, 50, 10));
    assert_eq!(resolve(&mut w, &c, 100, 30).0, 0);
    w.hp = 1001;
    assert_eq!(resolve(&mut w, &c, 100, 30).0, 0);
    assert_eq!(w.hp, 1001);
}
#[test]
fn received_dead_or_invalid_source_never_heals_or_emits_success() {
    let c = config();
    let mut w = world(&c);
    w.hp = 0;
    assert_eq!(resolve(&mut w, &c, 100, 30), (0, vec![]));
    let mut w = world(&c);
    w.hp = 500;
    let mut bad = status::participant();
    bad.generation = 2;
    let mut e = vec![];
    assert_eq!(
        w.pve.as_ref().unwrap().resolve_heal(
            &w,
            &c,
            (&bad, &status::participant()),
            (HealKind::Hot, 100),
            30,
            &mut e
        ),
        0
    );
    assert!(e.is_empty());
}
#[test]
fn received_wide_math_keeps_adjusted_amount_before_hp_clamp() {
    let mut c = config();
    let mut template = c.pve.as_ref().unwrap().status_definitions[1].clone();
    template.modifier_bps = 100000;
    c.pve_mut().status_definitions = (0..16)
        .map(|i| {
            let mut d = template.clone();
            d.id = format!("m{i}");
            d
        })
        .collect();
    c.skill.on_commit_self_statuses = vec!["m0".into()];
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 32;
    let mut w = world(&c);
    w.hp = 1;
    let monster = w.pve.as_ref().unwrap().status_entity(0);
    for i in 0..16 {
        for source in [status::participant(), monster.clone()] {
            bind(&mut w, &format!("m{i}"), source, status::participant(), 0);
        }
    }
    let (n, e) = resolve(&mut w, &c, u32::MAX, 30);
    assert_eq!(n, 999);
    let a = amounts(&e)[0];
    assert_eq!(a.1, 3210000);
    assert_eq!(a.2, u64::from(u32::MAX) * 321);
}
#[test]
fn received_hot_reads_live_modifier_and_expiry_before_same_due_tick() {
    let c = config();
    let mut w = world(&c);
    w.hp = 500;
    grant(&mut w, &["renewal", "care"], status::participant(), 0);
    assert_eq!(amounts(&pulse(&mut w, &c, 600))[0], (100, 5000, 50, 50));
    let e = pulse(&mut w, &c, 1200);
    assert_eq!(amounts(&e)[0], (100, 10000, 100, 100));
    let s = w.snapshot(&c, 1200);
    let p = s.statuses[0].periodic.as_ref().unwrap();
    assert_eq!(p.raw_heal, Some(100));
    assert_eq!(p.ticks_done, 2);
    assert_eq!(p.next_tick_at_ms, Some(1800));
}
#[test]
fn received_modifier_application_does_not_restart_hot_clock() {
    let c = config();
    let mut w = world(&c);
    w.hp = 500;
    grant(&mut w, &["renewal"], status::participant(), 0);
    pulse(&mut w, &c, 600);
    care(&mut w, 700);
    let e = pulse(&mut w, &c, 1200);
    assert_eq!(amounts(&e)[0], (100, 5000, 50, 50));
    assert!(e.iter().any(|e| matches!(
        e,
        Event::PeriodicHealTick {
            scheduled_at_ms: 1200,
            ..
        }
    )));
}
#[test]
fn received_real_commit_and_target_hit_bindings_work() {
    let mut c = config();
    let mut w = world(&c);
    w.hp = 500;
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 720);
    assert_eq!(amounts(&e)[0].1, 5000);
    assert_eq!(w.spirit, 190);
    c.skill.on_commit_self_statuses = vec![];
    c.skill.on_hit_target_statuses = vec!["care".into()];
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    advance(&mut w, &c, 0, 300);
    let m = &w.snapshot(&c, 300).pve.unwrap().monsters[0];
    assert_eq!(m.healing_received_bps, Some(5000));
    assert_eq!(m.effective_def, Some(100));
}
#[test]
fn received_precommit_cancel_never_grants_modifier() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    w.action(&Action::Background {}, "bg", &c, 30).unwrap();
    advance(&mut w, &c, 0, 600);
    assert!(w.snapshot(&c, 600).statuses.is_empty());
    assert_eq!(w.spirit, 200);
}
fn self_channel(duration: u64) -> (Config, World) {
    let mut c = config();
    c.pve_mut().status_definitions[1].duration_ms = duration;
    c.pve_mut().combat.heal.heal_bps = 1000;
    c.pve_mut().combat.heal.ticks = 3;
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "care", status::participant(), m, 0);
    let m = &mut w.pve.as_mut().unwrap().monsters[0];
    m.hp = 200;
    m.casting = Some((1, 90));
    (c, w)
}
#[test]
fn received_real_self_channel_partitions_before_scaling_once() {
    let (c, mut w) = self_channel(4000);
    let e = advance(&mut w, &c, 0, 2100);
    let values = amounts(&e);
    assert_eq!(
        values,
        vec![(34, 5000, 17, 17), (33, 5000, 16, 16), (33, 5000, 16, 16)]
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 249);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 175);
    let old: Vec<_> = e
        .iter()
        .filter_map(|e| {
            if let Event::MonsterHealed { amount, .. } = e {
                Some(*amount)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(old, vec![17, 16, 16]);
}
#[test]
fn received_self_channel_expiry_changes_future_shares_only() {
    let (c, mut w) = self_channel(1200);
    let e = advance(&mut w, &c, 0, 2100);
    assert_eq!(
        amounts(&e).iter().map(|v| v.3).collect::<Vec<_>>(),
        vec![17, 33, 33]
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 283);
}
fn support(duration: u64) -> (Config, World) {
    let mut c = attach(super::support_tests::config());
    c.pve_mut().status_definitions[1].duration_ms = duration;
    c.pve_mut()
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap()
        .heal_bps = 3334;
    let mut w = super::support_tests::world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "care", status::participant(), m, 0);
    super::support_tests::start(&mut w, &c, 100);
    (c, w)
}
fn support_pulse(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.prepare_statuses(w, now, &mut e);
    a.advance_supports(w, c, now, &mut e);
    w.pve = Some(a);
    e
}
#[test]
fn received_support_partitions_before_scaling_once_and_preserves_cost() {
    let (c, mut w) = support(4000);
    let before = w.pve.as_ref().unwrap().monsters[1].spirit;
    let cost = w.pve.as_ref().unwrap().monsters[1]
        .profile
        .support
        .as_ref()
        .unwrap()
        .cost;
    let mut e = vec![];
    for t in [400, 500, 600] {
        e.extend(support_pulse(&mut w, &c, t));
    }
    assert_eq!(
        amounts(&e),
        vec![(34, 5000, 17, 17), (33, 5000, 16, 16), (33, 5000, 16, 16)]
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 169);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].spirit, before - cost);
    let old: Vec<_> = e
        .iter()
        .filter_map(|e| {
            if let Event::SupportPulse { amount, .. } = e {
                Some(*amount)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(old, vec![17, 16, 16]);
}
#[test]
fn received_support_uses_target_not_caster_and_live_expiry() {
    let (mut c, _) = support(500);
    extra(&mut c, "caster", 5000);
    let mut w = super::support_tests::world(&c);
    let target = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "care", status::participant(), target, 0);
    let caster = w.pve.as_ref().unwrap().status_entity(1);
    grant(&mut w, &["caster"], caster, 0);
    super::support_tests::start(&mut w, &c, 100);
    let mut e = vec![];
    for t in [400, 500, 600] {
        e.extend(support_pulse(&mut w, &c, t));
    }
    assert_eq!(
        amounts(&e).iter().map(|v| v.3).collect::<Vec<_>>(),
        vec![17, 33, 33]
    );
}

#[test]
fn received_natural_regen_is_outside_the_healing_multiplier() {
    let mut c = config();
    c.pve_mut().status_definitions[1].modifier_bps = -10000;
    c.params.minimum_damage = 0;
    c.pve_mut().stats.insert("atk".into(), 0);
    c.params.regen.near_ratio = 0.01;
    c.params.regen.far_ratio = 0.01;
    let mut w = world(&c);
    w.hp = 500;
    w.next_regen = 1000;
    care(&mut w, 0);
    let e = advance(&mut w, &c, 0, 1020);
    assert_eq!(w.hp, 510);
    assert!(amounts(&e).is_empty());
}
#[test]
fn received_no_rng_contact_or_extraction_interrupt() {
    let c = config();
    let mut w = world(&c);
    w.hp = 500;
    grant(&mut w, &["renewal", "care"], status::participant(), 0);
    w.extraction_end = Some(5000);
    let seed = w.rng;
    let e = pulse(&mut w, &c, 600);
    assert_eq!(w.rng, seed);
    assert_eq!(w.extraction_end, Some(5000));
    assert_eq!(w.spirit, 200);
    assert!(!e.iter().any(|e| matches!(
        e,
        Event::Damage { .. } | Event::CounterTriggered { .. } | Event::ShieldAbsorbed { .. }
    )));
}
#[test]
fn received_background_expires_modifier_without_replaying_hot() {
    let c = config();
    let mut w = world(&c);
    w.hp = 500;
    grant(&mut w, &["renewal", "care"], status::participant(), 0);
    w.action(&Action::Background {}, "bg", &c, 30).unwrap();
    assert!(amounts(&pulse(&mut w, &c, 600)).is_empty());
    assert_eq!(w.snapshot(&c, 700).healing_received_bps, Some(5000));
    pulse(&mut w, &c, 1200);
    assert_eq!(w.snapshot(&c, 1200).healing_received_bps, Some(10000));
    w.foreground = true;
    w.ready = true;
    w.protected = false;
    assert_eq!(amounts(&pulse(&mut w, &c, 1800))[0].3, 100);
    assert_eq!(w.hp, 600);
}
#[test]
fn received_hundred_closes_release_modifier_and_hot() {
    let c = config();
    for _ in 0..100 {
        let mut w = world(&c);
        grant(&mut w, &["renewal", "care"], status::participant(), 0);
        w.close_container_interaction(300);
        assert!(pulse(&mut w, &c, 600).is_empty());
        assert_eq!(w.snapshot(&c, 600).healing_received_bps, Some(10000));
    }
}
#[test]
fn received_target_generation_and_source_death_clean_up() {
    let c = config();
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "care", m, status::participant(), 0);
    w.pve.as_mut().unwrap().monsters[0].status_generation += 1;
    pulse(&mut w, &c, 30);
    assert_eq!(w.snapshot(&c, 30).healing_received_bps, Some(10000));
}
#[test]
fn received_public_retries_and_reconnect_do_not_double_apply() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine_with_config(config());
    e.world.as_mut().unwrap().hp = 500;
    let c = s::create(&e.config);
    let input = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: "received-cast".into(),
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
    let before = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    e.input(input).unwrap();
    e.input(Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "received-resume".into(),
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
    assert_eq!(after.statuses, before.statuses);
    assert_eq!(after.hp, before.hp);
    assert_eq!(after.healing_received_bps, Some(5000));
}
#[test]
fn received_legal_output_burst_releases_world_and_frozen_settlement() {
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
        let m = w.pve.as_ref().unwrap().status_entity(i);
        grant(w, &["renewal", "care"], m, now);
    }
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
    let r = crate::asset_contract::tests::requests(&o).remove(0);
    let receipt = crate::asset_contract::tests::receipt(
        &e.config,
        &r,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "received-peak".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
}
#[test]
fn received_old_views_and_events_remain_unchanged() {
    for name in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
        "hot-host",
    ] {
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
        let e = advance(&mut w, &c, 0, 900);
        assert!(amounts(&e).is_empty());
        let v = serde_json::to_value(w.snapshot(&c, 900)).unwrap();
        assert!(v.get("healing_received_bps").is_none());
        assert!(
            v["pve"]["monsters"][0]
                .get("healing_received_bps")
                .is_none()
        );
    }
}

#[test]
fn received_dot_death_and_absolute_terminal_never_allow_late_healing() {
    let mut c = config();
    let v: serde_json::Value =
        serde_json::from_str(crate::test_data::fixture!("examples/dot-host/arena.json")).unwrap();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(v["pve"]["status_definitions"][1].clone()).unwrap());
    c.pve_mut().stats.insert("atk".into(), 100);
    let mut w = world(&c);
    w.hp = 10;
    grant(&mut w, &["renewal", "care"], status::participant(), 0);
    let source = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "wound", source, status::participant(), 0);
    assert!(amounts(&pulse(&mut w, &c, 600)).is_empty());
    assert_eq!(w.hp, 0);
    assert!(pulse(&mut w, &c, 1200).is_empty());
    c.terminal_ms = 600;
    let mut w = world(&c);
    grant(&mut w, &["renewal", "care"], status::participant(), 0);
    assert!(amounts(&w.tick(&c, 600)).is_empty());
    assert_eq!(w.desired_outcome, Some(Outcome::Death));
}
#[test]
fn received_encounter_reset_invalidates_old_round_modifier() {
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
    bind(&mut w, "care", status::participant(), old.clone(), 30);
    w.position = [0., 0.];
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.advance_encounter_resets(&w, 60, &mut e);
    a.advance_encounter_resets(&w, 60 + delay, &mut e);
    assert!(a.statuses.is_empty());
    a.hit_statuses(
        &w,
        &["care".into()],
        &status::participant(),
        &old,
        60 + delay,
        &mut e,
    );
    assert!(a.statuses.is_empty());
}
#[test]
fn received_orb_restores_hp_and_spirit_without_this_multiplier() {
    let mut c = config();
    c.pve_mut().status_definitions[1].modifier_bps = -10000;
    let mut w = world(&c);
    w.hp = 500;
    w.spirit = 100;
    care(&mut w, 0);
    let mut a = w.pve.take().unwrap();
    a.hurt_monster(&mut w, &c, 0, 5000, false, &mut vec![]);
    w.pve = Some(a);
    let orb = w
        .resources
        .iter()
        .find(|r| r.view.orb)
        .unwrap()
        .view
        .clone();
    w.position = orb.position;
    let e = w
        .action(
            &Action::Pickup {
                resource_id: orb.id,
                expected_revision: w.inventory_revision,
            },
            "orb",
            &c,
            30,
        )
        .unwrap();
    assert_eq!(w.hp, 530);
    assert_eq!(w.spirit, 130);
    assert!(amounts(&e).is_empty());
}
#[test]
fn received_healing_does_not_rewind_boss_or_gain_aggro() {
    let c = attach(super::heavy_tests::config());
    let mut w = super::heavy_tests::world(&c);
    let mut a = w.pve.take().unwrap();
    a.hurt_monster(&mut w, &c, 0, 420, false, &mut vec![]);
    let phase = a.monsters[0].boss_phase;
    assert!(phase > 0);
    a.monsters[0].aggro = false;
    let target = a.status_entity(0);
    w.pve = Some(a);
    grant(&mut w, &["renewal", "care"], target, 0);
    assert_eq!(amounts(&pulse(&mut w, &c, 600))[0].3, 25);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!(m.boss_phase, phase);
    assert!(!m.aggro);
}
