use super::status_tests::{advance, bind, cast, damage, world};
use super::*;
fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/attribute-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone()
}
fn attach(mut c: Config) -> Config {
    let sample = config();
    c.schema_version = 4;
    c.mechanism_version = 21;
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
fn self_bind(w: &mut World, id: &str, now: u64) {
    bind(w, id, status::participant(), status::participant(), now);
}
fn monster_bind(w: &mut World, id: &str, now: u64) {
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(w, id, m.clone(), m, now);
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
fn attribute_self_commit_changes_this_shot_and_expiry_restores_basic_damage() {
    let mut c = config();
    c.skill.on_hit_target_statuses.clear();
    let mut w = world(&c);
    let mut events = vec![];
    let mut now = 0;
    for at in [0, 900, 1800, 2700] {
        events.extend(advance(&mut w, &c, now, at));
        cast(&mut w, &c, at);
        events.extend(advance(&mut w, &c, at, at + 600));
        now = at + 600;
    }
    assert_eq!(damage(&events), vec![83, 100, 116, 116]);
    let s = w.snapshot(&c, now);
    assert_eq!((s.effective_atk, s.effective_def), (Some(175), Some(100)));
    assert_eq!(s.statuses[0].attribute, Some(StatusAttribute::Atk));
    assert_eq!(w.facts.atk_points, 100);
    advance(&mut w, &c, now, 6000);
    assert_eq!(w.snapshot(&c, 6000).effective_atk, Some(100));
    w.action(&Action::Attack {}, "basic", &c, 6000).unwrap();
    let e = advance(&mut w, &c, 6000, 6600);
    assert_eq!(damage(&e), vec![66]);
}
#[test]
fn attribute_attack_and_defense_combine_without_double_scaling() {
    let c = config();
    let mut w = world(&c);
    let mut e = vec![];
    let mut now = 0;
    for at in [0, 900, 1800, 2700] {
        e.extend(advance(&mut w, &c, now, at));
        cast(&mut w, &c, at);
        e.extend(advance(&mut w, &c, at, at + 600));
        now = at + 600;
    }
    assert_eq!(damage(&e), vec![83, 109, 140, 155]);
    let s = w.snapshot(&c, now);
    assert_eq!(s.effective_atk, Some(175));
    assert_eq!(s.pve.unwrap().monsters[0].effective_def, Some(25));
}
#[test]
fn attribute_aggregation_is_separate_clamped_wide_and_rounds_once() {
    let mut c = config();
    let mut w = world(&c);
    w.facts.atk_points = 7;
    for t in [0, 30, 60] {
        self_bind(&mut w, "guard", t);
        self_bind(&mut w, "exposed", t);
    }
    let s = w.snapshot(&c, 60);
    assert_eq!((s.effective_atk, s.effective_def), (Some(12), Some(25)));
    assert_eq!((w.facts.atk_points, w.facts.def_points), (7, 100));
    let p = c.pve_mut();
    p.status_definitions[0].modifier_bps = 100000;
    p.status_definitions[0].max_stacks = Some(128);
    p.status_rules.as_mut().unwrap().attack_bonus_cap_bps = Some(100000);
    let mut w = world(&c);
    w.facts.atk_points = 1000000;
    for _ in 0..128 {
        self_bind(&mut w, "guard", 0);
    }
    assert_eq!(w.snapshot(&c, 0).effective_atk, Some(11000000));
    let mut c = config();
    c.pve_mut().status_definitions[0].modifier_bps = -10000;
    let mut w = world(&c);
    for _ in 0..3 {
        self_bind(&mut w, "guard", 0);
    }
    assert_eq!(w.snapshot(&c, 0).effective_atk, Some(0));
    w.facts.atk_points = 0;
    assert_eq!(w.snapshot(&c, 0).effective_atk, Some(0));
}
#[test]
fn attribute_zero_attack_keeps_configured_minimum_damage() {
    for minimum in [0, 1] {
        let mut c = config();
        c.params.minimum_damage = minimum;
        c.pve_mut().status_definitions[0].modifier_bps = -10000;
        c.skill.on_hit_target_statuses.clear();
        let mut w = world(&c);
        cast(&mut w, &c, 0);
        let e = advance(&mut w, &c, 0, 600);
        assert_eq!(damage(&e), vec![minimum]);
        assert_eq!(w.snapshot(&c, 600).effective_atk, Some(0));
    }
}
#[test]
fn attribute_preparation_uses_commit_state_and_exact_expiry_precedes_commit() {
    for (applied, expected) in [(0, 66), (30, 83)] {
        let mut c = config();
        c.skill.on_commit_self_statuses.clear();
        c.skill.on_hit_target_statuses.clear();
        c.pve_mut().status_definitions[0].duration_ms = 90;
        let mut w = world(&c);
        cast(&mut w, &c, 0);
        advance(&mut w, &c, 0, applied);
        self_bind(&mut w, "guard", applied);
        let e = advance(&mut w, &c, applied, 600);
        assert_eq!(damage(&e), vec![expected]);
    }
}
#[test]
fn attribute_flight_freezes_attack_but_reads_defense_at_contact() {
    let mut c = config();
    c.skill.speed = 100.;
    c.skill.on_commit_self_statuses.clear();
    c.skill.on_hit_target_statuses.clear();
    c.pve_mut().status_definitions[0].duration_ms = 500;
    c.pve_mut().status_definitions[1].duration_ms = 3000;
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    advance(&mut w, &c, 0, 60);
    self_bind(&mut w, "guard", 60);
    advance(&mut w, &c, 60, 120);
    assert_eq!(w.pve.as_ref().unwrap().shots[0].raw, 125);
    advance(&mut w, &c, 120, 600);
    assert_eq!(w.snapshot(&c, 600).effective_atk, Some(100));
    self_bind(&mut w, "guard", 600);
    self_bind(&mut w, "guard", 600);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    for _ in 0..2 {
        bind(&mut w, "exposed", status::participant(), m.clone(), 600);
    }
    assert_eq!(w.pve.as_ref().unwrap().shots[0].raw, 125);
    let e = advance(&mut w, &c, 600, 2400);
    assert_eq!(damage(&e), vec![100]);
}
#[test]
fn attribute_player_basic_reads_effective_attack_once() {
    let c = config();
    let mut w = world(&c);
    self_bind(&mut w, "guard", 0);
    w.action(&Action::Attack {}, "basic", &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!(damage(&e), vec![83]);
}
#[test]
fn attribute_monster_projectile_self_buff_freezes_even_after_source_death() {
    let mut c = config();
    let p = c.pve_mut();
    p.stats.insert("atk".into(), 100);
    p.combat.projectile.on_commit_self_statuses = vec!["guard".into()];
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[0].casting = Some((0, 90));
    advance(&mut w, &c, 0, 120);
    assert_eq!(w.pve.as_ref().unwrap().shots[0].raw, 125);
    w.pve.as_mut().unwrap().monsters[0].hp = 0;
    let e = advance(&mut w, &c, 120, 600);
    assert_eq!(received(&e), vec![83]);
    assert!(w.pve.as_ref().unwrap().statuses.is_empty());
}
#[test]
fn attribute_hit_debuff_changes_subsequent_melee_not_defense_constant() {
    let mut c = config();
    let p = c.pve_mut();
    p.stats.insert("atk".into(), 100);
    p.status_definitions[1].attribute = Some(StatusAttribute::Atk);
    p.status_definitions[1].modifier_bps = -5000;
    p.combat.melee_range = 300.;
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[0].next_attack = 600;
    w.pve.as_mut().unwrap().monsters[0].aggro = true;
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 700);
    assert_eq!(received(&e), vec![33]);
    assert_eq!(
        w.snapshot(&c, 700).pve.unwrap().monsters[0].effective_atk,
        Some(50)
    );
}
#[test]
fn attribute_area_reads_attack_at_release() {
    let c = attach(super::area_tests::config());
    let mut w = super::area_tests::world(&c);
    monster_bind(&mut w, "guard", 0);
    super::area_tests::start(&mut w, &c, 0, 100);
    monster_bind(&mut w, "guard", 200);
    monster_bind(&mut w, "guard", 200);
    let e = super::area_tests::resolve(&mut w, &c, 0, 400);
    assert_eq!(received(&e), vec![175]);
}
#[test]
fn attribute_heavy_plan_freezes_at_warning_creation() {
    let c = attach(super::heavy_tests::config());
    let mut w = super::heavy_tests::world(&c);
    monster_bind(&mut w, "guard", 0);
    super::heavy_tests::start(&mut w, &c, 0, 100);
    monster_bind(&mut w, "guard", 200);
    super::heavy_tests::advance(&mut w, &c, 400);
    let e = super::heavy_tests::advance(&mut w, &c, 1000);
    assert_eq!(received(&e), vec![125]);
}
#[test]
fn attribute_counter_window_freezes_attack_before_later_contact() {
    let c = attach(super::heavy_tests::config());
    let mut w = super::heavy_tests::world(&c);
    self_bind(&mut w, "guard", 0);
    super::heavy_tests::start(&mut w, &c, 0, 100);
    super::heavy_tests::advance(&mut w, &c, 400);
    w.position = [400., 500.];
    super::heavy_tests::advance(&mut w, &c, 1000);
    assert_eq!(super::heavy_tests::window(&w).unwrap().raw_damage, 150);
    self_bind(&mut w, "guard", 1100);
    assert_eq!(super::heavy_tests::window(&w).unwrap().raw_damage, 150);
}
#[test]
fn attribute_timed_position_freezes_failure_and_success_budgets() {
    for success in [false, true] {
        let (old, old_world) = super::timed_position_tests::fixture_for("b3-15");
        let mut c = attach(old);
        let p = c.pve_mut();
        p.stats.insert("def".into(), 0);
        p.stats.insert("hp".into(), 10000);
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
        bind(&mut w, "guard", m.clone(), m.clone(), 0);
        self_bind(&mut w, "guard", 0);
        let warning = super::timed_position_tests::start_warning(&mut w, &c);
        let a = w.pve.as_ref().unwrap();
        let check = a.grid.config.timed_position.as_ref().unwrap();
        assert_eq!(
            warning.failure_raw_damage,
            scaled_amount(
                a.monster_attack(source, warning.started_at_ms),
                check.failure_damage_bps.unwrap(),
                10000
            )
        );
        let success_budget = scaled_amount(125, check.success_damage_bps.unwrap(), 10000);
        self_bind(&mut w, "guard", warning.started_at_ms + 1);
        bind(&mut w, "guard", m.clone(), m, warning.started_at_ms + 1);
        if success {
            w.position = w.pve.as_ref().unwrap().monsters[source].position;
        }
        let e = super::timed_position_tests::advance(&mut w, &c, warning.resolves_at_ms);
        if success {
            assert_eq!(damage(&e), vec![success_budget]);
        } else {
            assert_eq!(received(&e), vec![warning.failure_raw_damage]);
        }
    }
}
#[test]
fn attribute_capacity_rejection_and_source_cleanup_are_atomic() {
    let mut c = config();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    c.skill.on_commit_self_statuses = vec!["guard".into(), "exposed".into()];
    let mut w = world(&c);
    self_bind(&mut w, "guard", 0);
    let before = w.snapshot(&c, 0).statuses;
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 600);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready_at, 0);
    assert_eq!(w.snapshot(&c, 600).statuses, before);
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "guard", m, status::participant(), 0);
    w.pve.as_mut().unwrap().monsters[0].hp = 0;
    advance(&mut w, &c, 0, 30);
    assert!(w.snapshot(&c, 30).statuses.is_empty());
}
#[test]
fn attribute_duplicate_policies_and_hundred_closes_share_lifecycle() {
    use crate::config::DuplicatePolicy;
    for policy in [
        DuplicatePolicy::Refresh,
        DuplicatePolicy::Ignore,
        DuplicatePolicy::Stack,
    ] {
        let mut c = config();
        let d = &mut c.pve_mut().status_definitions[0];
        d.duplicate_policy = policy;
        if !matches!(policy, DuplicatePolicy::Stack) {
            d.max_stacks = None;
        }
        for _ in 0..100 {
            let mut w = world(&c);
            self_bind(&mut w, "guard", 0);
            self_bind(&mut w, "guard", 30);
            let s = w.snapshot(&c, 30);
            assert_eq!(
                s.effective_atk,
                Some(if matches!(policy, DuplicatePolicy::Stack) {
                    150
                } else {
                    125
                })
            );
            assert_eq!(
                s.statuses[0].applied_at_ms,
                if matches!(policy, DuplicatePolicy::Ignore) {
                    0
                } else {
                    30
                }
            );
            w.close_container_interaction(30);
            assert!(w.pve.as_ref().unwrap().statuses.is_empty());
        }
    }
}
#[test]
fn attribute_old_19_20_snapshots_omit_new_fields_and_match_damage() {
    for name in ["status-host", "stack-host"] {
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
        let e = advance(&mut w, &c, 0, 600);
        assert_eq!(damage(&e), vec![66]);
        let v = serde_json::to_value(w.snapshot(&c, 600)).unwrap();
        assert!(v.get("effective_atk").is_none());
        assert!(v["statuses"][0].get("attribute").is_none());
        assert!(v["pve"]["monsters"][0].get("effective_atk").is_none());
    }
}

#[test]
fn attribute_source_groups_and_latest_owner_cover_capped_reapplication() {
    for scoped in [true, false] {
        let mut c = config();
        c.pve_mut().status_definitions[0].source_scoped = scoped;
        let mut w = world(&c);
        for t in [0, 30, 60] {
            self_bind(&mut w, "guard", t);
        }
        let m = w.pve.as_ref().unwrap().status_entity(0);
        bind(&mut w, "guard", m.clone(), status::participant(), 90);
        let s = w.snapshot(&c, 90);
        assert_eq!(s.statuses.len(), if scoped { 2 } else { 1 });
        if !scoped {
            assert_eq!(s.statuses[0].source, m);
            assert_eq!(s.statuses[0].stacks, Some(3));
        }
        w.pve.as_mut().unwrap().monsters[0].hp = 0;
        advance(&mut w, &c, 90, 120);
        assert_eq!(
            w.snapshot(&c, 120).effective_atk,
            Some(if scoped { 175 } else { 100 })
        );
    }
}
#[test]
fn attribute_positive_and_negative_instances_cancel_without_touching_defense() {
    let mut c = config();
    let mut weak = c.pve.as_ref().unwrap().status_definitions[0].clone();
    weak.id = "weak".into();
    weak.modifier_bps = -2500;
    c.pve_mut().status_definitions.push(weak);
    let mut w = world(&c);
    for t in [0, 30, 60] {
        self_bind(&mut w, "guard", t);
    }
    for (t, value) in [(90, 150), (120, 125), (150, 100)] {
        self_bind(&mut w, "weak", t);
        let s = w.snapshot(&c, t);
        assert_eq!(s.effective_atk, Some(value));
        assert_eq!(s.effective_def, Some(100));
    }
}
