use super::*;
use serde_json::{Value, json};
fn setup() -> (Config, World) {
    let (mut c, w) = super::conditional_activation_tests::setup(false);
    c.combat_rules.as_mut().unwrap().status_policies.insert(
        "marks".into(),
        serde_json::from_value(
            json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]}),
        )
        .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"renew","profiles":["member"],"when":"kill","include_periodic":false,"every":1,"cooldown_ms":1000,"priority":1,"effect":{"kind":"refresh_status_duration","subject":"owner","status_id":"marks","duration_ms":1000}}])).unwrap();
    (c, w)
}
fn state(w: &World, id: &str) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == id)
        .unwrap()
        .clone()
}
fn fact(c: &Config, w: &mut World, now: u64) -> Events {
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::Kill,
        (Owner::Member(0), Owner::Npc(0)),
        (1, false),
        c,
        now,
        &mut e,
    );
    e
}
#[test]
fn conditional_activation_clock_keeps_layers_identity_origin_and_application_time() {
    let (c, mut w) = setup();
    w.statuses = Default::default();
    let own = w.members[0].reference.entity.clone();
    w.triggers.origin = Some(crate::origin::ActionOrigin::Skill(c.skill.id.clone()));
    w.triggers.action_source = Some(own.clone());
    for _ in 0..3 {
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "marks", 0);
    }
    let old = state(&w, "marks");
    let origin = w.statuses.action_origin(&old.instance_id).cloned();
    assert!(origin.is_some());
    w.triggers.origin = Some(crate::origin::ActionOrigin::Basic);
    let e = fact(&c, &mut w, 100);
    let new = state(&w, "marks");
    assert_eq!(new.instance_id, old.instance_id);
    assert_eq!(new.stacks, old.stacks);
    assert_eq!(new.source, old.source);
    assert_eq!(new.applied_at_ms, 0);
    assert_eq!(new.expires_at_ms, 1100);
    assert_eq!(w.statuses.action_origin(&new.instance_id), origin.as_ref());
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusRefreshed { .. }
                }
            ))
            .count(),
        1
    );
}
#[test]
fn conditional_activation_clock_consumption_preview_keeps_full_stack_and_refreshes_partial_without_rebuild()
 {
    for amount in [2, 3] {
        let (mut c, mut w) = setup();
        c.combat_rules.as_mut().unwrap().triggers[0].conditions=serde_json::from_value(json!({"status":{"subject":"owner","definition_id":"marks","at_least":3,"consume":amount}})).unwrap();
        let old = state(&w, "marks");
        let e = fact(&c, &mut w, 100);
        let new = state(&w, "marks");
        assert_eq!(new.instance_id, old.instance_id);
        assert_eq!(new.stacks, Some(if amount == 2 { 1 } else { 3 }));
        assert_eq!(new.expires_at_ms, if amount == 2 { 1100 } else { 10000 });
        assert_eq!(
            e.iter()
                .filter(|(_, e)| matches!(
                    e,
                    EventV4::Private {
                        event: Event::CombatTriggerFired { .. }
                    }
                ))
                .count(),
            usize::from(amount == 2)
        );
    }
}
#[test]
fn conditional_activation_clock_missing_expired_dead_and_other_dead_holder_never_create_or_spend_icd()
 {
    use crate::config::TriggerSubject;
    for mode in 0..3 {
        let (c, mut w) = setup();
        match mode {
            0 => w.statuses = Default::default(),
            1 => {}
            _ => w.members[0].hp = 0,
        };
        let now = if mode == 1 { 10000 } else { 100 };
        assert!(
            w.plan_status_refresh(
                &w.statuses,
                (Owner::Member(0), Owner::Npc(0)),
                (TriggerSubject::Owner, "marks", Some(1000)),
                &c,
                now
            )
            .is_err()
        );
        fact(&c, &mut w, now);
        assert!(!w.statuses.entries().iter().any(|s| s.applied_at_ms == now));
    }
    let (c, mut w) = setup();
    w.statuses = Default::default();
    fact(&c, &mut w, 100);
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own, "marks", 200);
    fact(&c, &mut w, 200);
    assert_eq!(state(&w, "marks").expires_at_ms, 1200);
    let (c, mut w) = setup();
    w.npcs[0].hp = 0;
    assert!(
        w.plan_status_refresh(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(0)),
            (TriggerSubject::Owner, "marks", Some(1000)),
            &c,
            100
        )
        .is_ok()
    );
    assert!(
        w.plan_status_refresh(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(0)),
            (TriggerSubject::Other, "marks", Some(1000)),
            &c,
            100
        )
        .is_err()
    );
}
#[test]
fn conditional_activation_clock_zero_counter_keeps_window_record_and_only_settles_new_deadline_once()
 {
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"clock-window","mechanism":"timed_compound_modifier","base_modifiers":[{"attribute":"def","modifier_bps":1}],"stack_counter":{"initial":0,"maximum":3},"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    c.combat_rules.as_mut().unwrap().status_policies.insert(
        "clock-window".into(),
        serde_json::from_value(
            json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]}),
        )
        .unwrap(),
    );
    c.combat_rules.as_mut().unwrap().status_windows.insert("clock-window".into(),serde_json::from_value(json!({"basis":"damage_dealt","include_periodic":false,"record_cap":1000,"refresh":"reset","maximum_sampling":"snapshot","on_expired":true,"on_consumed":false,"hp":{"fixed":0,"recorded_bps":1000,"max_resource_bps":0,"cap_max_resource_bps":2000}})).unwrap());
    c.combat_rules.as_mut().unwrap().triggers[0].effect=serde_json::from_value(json!({"kind":"refresh_status_duration","subject":"owner","status_id":"clock-window","duration_ms":1000})).unwrap();
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "clock-window", 0);
    w.resolve_damage_with_rule(
        combat::DamageIntent {
            source: own,
            target: w.npcs[0].entity.clone(),
            raw: 100,
            direct: true,
            ignore_armor: true,
        },
        Some(&crate::config::AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            ..Default::default()
        }),
        &c,
        30,
        &mut vec![],
    );
    let old = state(&w, "clock-window");
    fact(&c, &mut w, 100);
    let new = state(&w, "clock-window");
    assert_eq!(new.window, old.window);
    assert_eq!(new.stacks, Some(0));
    assert_eq!(new.applied_at_ms, 0);
    assert_eq!(new.instance_id, old.instance_id);
    assert_eq!(new.expires_at_ms, 1100);
    w.prepare_periodic(300, &mut vec![]);
    w.finish_window_records(&c, 300, &mut vec![]);
    assert_eq!(w.members[0].hp, 50);
    let mut e = vec![];
    w.prepare_periodic(1100, &mut e);
    w.finish_window_records(&c, 1100, &mut e);
    assert_eq!(w.members[0].hp, 60);
    w.finish_window_records(&c, 1130, &mut e);
    assert_eq!(w.members[0].hp, 60);
}
#[test]
fn conditional_activation_clock_resamples_harmful_reduction_and_post_consumption_preview() {
    for consume in [false, true] {
        let (mut c, mut w) = setup();
        w.statuses = Default::default();
        let def = c
            .pve_mut()
            .status_definitions
            .iter_mut()
            .find(|d| d.id == "marks")
            .unwrap();
        def.attribute = Some(StatusAttribute::Atk);
        def.modifier_bps = -1000;
        c.combat_rules
            .as_mut()
            .unwrap()
            .status_policies
            .get_mut("marks")
            .unwrap()
            .disposition = crate::config::StatusDisposition::Harmful;
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"shortener","mechanism":"timed_attribute_modifier","attribute":"harmful_duration_reduction","modifier_bps":5000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
        let own = w.members[0].reference.entity.clone();
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "marks", 0);
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own, "shortener", 0);
        if consume {
            c.combat_rules.as_mut().unwrap().triggers[0].conditions=serde_json::from_value(json!({"status":{"subject":"owner","definition_id":"shortener","at_least":1,"consume":1}})).unwrap();
        }
        fact(&c, &mut w, 100);
        assert_eq!(
            state(&w, "marks").expires_at_ms,
            if consume { 1100 } else { 600 }
        );
    }
}
#[test]
fn conditional_activation_clock_attack_speed_work_crosses_original_expiry_without_free_credit() {
    let (mut c, mut w) = setup();
    w.statuses = Default::default();
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "marks")
        .unwrap();
    d.attribute = Some(StatusAttribute::AttackSpeed);
    d.modifier_bps = -5000;
    d.duration_ms = 300;
    d.max_stacks = None;
    d.duplicate_policy = crate::config::DuplicatePolicy::Refresh;
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own, "marks", 0);
    w.start_attack_clock(Owner::Member(0), 1000, &c, 0);
    c.combat_rules.as_mut().unwrap().triggers[0].effect=serde_json::from_value(json!({"kind":"refresh_status_duration","subject":"owner","status_id":"marks","duration_ms":500})).unwrap();
    fact(&c, &mut w, 100);
    assert_eq!(state(&w, "marks").expires_at_ms, 600);
    w.advance_attack_clocks(&c, 600);
    assert_eq!(w.members[0].basic_ready, 1300);
    w.advance_attack_clocks(&c, 1300);
    assert_eq!(w.members[0].basic_ready, 1300);
}
#[test]
fn conditional_activation_clock_default_and_ordered_same_time_refresh_are_absolute_not_additive() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers[0].effect = serde_json::from_value(
        json!({"kind":"refresh_status_duration","subject":"owner","status_id":"marks"}),
    )
    .unwrap();
    fact(&c, &mut w, 100);
    assert_eq!(state(&w, "marks").expires_at_ms, 10100);
    let (mut c, mut w) = setup();
    let mut second = c.combat_rules.as_ref().unwrap().triggers[0].clone();
    second.id = "longer".into();
    second.priority = 2;
    second.cooldown_ms = 0;
    let first = &mut c.combat_rules.as_mut().unwrap().triggers[0];
    first.cooldown_ms = 0;
    first.effect=serde_json::from_value(json!({"kind":"refresh_status_duration","subject":"owner","status_id":"marks","duration_ms":500})).unwrap();
    c.combat_rules.as_mut().unwrap().triggers.push(second);
    let e = fact(&c, &mut w, 100);
    assert_eq!(state(&w, "marks").expires_at_ms, 1100);
    assert_eq!(state(&w, "marks").stacks, Some(3));
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusRefreshed { .. }
                }
            ))
            .count(),
        2
    );
}
#[test]
fn conditional_activation_clock_policy_failure_keeps_deadline_and_icd_ready() {
    let (mut c, mut w) = setup();
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_policies
        .get_mut("source-power")
        .unwrap()
        .blocks = vec![crate::config::StatusDisposition::Beneficial];
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(1), own, "source-power", 0);
    fact(&c, &mut w, 100);
    assert_eq!(state(&w, "marks").expires_at_ms, 10000);
    fact(&c, &mut w, 5000);
    assert_eq!(state(&w, "marks").expires_at_ms, 6000);
}
#[test]
fn conditional_activation_clock_cold_rejects_non_numeric_shapes_and_invalid_explicit_durations() {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/activation-a.json"
    ))
    .unwrap();
    d["combat_rules"]["triggers"] = json!([{"id":"clock","profiles":["member"],"when":"kill","include_periodic":false,"every":1,"cooldown_ms":0,"priority":1,"effect":{"kind":"refresh_status_duration","subject":"owner","status_id":"marks","duration_ms":10000}}]);
    for def in [
        json!({"id":"periodic-clock","mechanism":"periodic_heal","heal_bps":100,"tick_interval_ms":300,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"shield-clock","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"root-clock","mechanism":"timed_control","attribute":"root","duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"flag-clock","mechanism":"timed_compound_modifier","modifiers":[{"attribute":"def","modifier_bps":100}],"flags":["control_immunity"],"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true}),
    ] {
        let id = def["id"].as_str().unwrap().to_owned();
        d["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .push(def);
        d["combat_rules"]["status_policies"][&id] =
            json!({"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]});
    }
    p.write(&d);
    p.load().unwrap();
    for case in 0..14 {
        let mut bad = d.clone();
        let e = &mut bad["combat_rules"]["triggers"][0]["effect"];
        match case {
            0 => e["duration_ms"] = Value::Null,
            1 => e["duration_ms"] = json!(0),
            2 => e["duration_ms"] = json!(29),
            3 => e["duration_ms"] = json!(60001),
            4 => e["duration_ms"] = json!(3600001),
            5 => e["duration_ms"] = json!(1.5),
            6 => e["status_id"] = json!("missing"),
            7 => e["status_id"] = json!("periodic-clock"),
            8 => e["status_id"] = json!("shield-clock"),
            9 => e["status_id"] = json!("root-clock"),
            10 => e["status_id"] = json!("flag-clock"),
            11 => e["extra"] = json!(true),
            12 => {
                bad["combat_rules"]["status_policies"]
                    .as_object_mut()
                    .unwrap()
                    .remove("marks");
            }
            _ => bad["combat_rules"]["triggers"][0]["counting"] = json!("action_total"),
        };
        p.write(&bad);
        assert!(p.load().is_err(), "clock case {case}");
    }
}
#[test]
fn conditional_activation_clock_original_stage_four_and_five_real_kills_renew_ten_seconds_without_layer_gain()
 {
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/conditional_activation_vectors.json"
    ))
    .unwrap();
    let catalog = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/combat-host/activation-catalog.json"),
    )
    .unwrap();
    for index in [2, 3, 4] {
        let mut c = catalog.default_config().as_ref().clone();
        let (_, mut w) = super::combat_tests::setup(4);
        w.statuses = Default::default();
        let build=serde_json::from_value(json!({"power_points":0,"skill_power_points":0,"element_resistance_points":0,"powers":{"drain":{"model_id":"phase","rank":0,"quality":0,"level":0,"mastery":index}}})).unwrap();
        assert!(c.valid_combat_build(&build));
        w.members[0].combat_build = Some(build);
        let source = w.members[0].reference.entity.clone();
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), source.clone(), "marks", 0);
        let old = state(&w, "marks");
        w.npcs[1].hp = 1;
        w.resolve_damage_with_rule(
            combat::DamageIntent {
                source,
                target: w.npcs[1].entity.clone(),
                raw: 1,
                direct: true,
                ignore_armor: true,
            },
            Some(&crate::config::AttackRule {
                damage_kind: crate::config::DamageKind::TrueDamage,
                ..Default::default()
            }),
            &c,
            1000,
            &mut vec![],
        );
        assert_eq!(w.npcs[1].hp, 0);
        let new = state(&w, "marks");
        assert_eq!(new.instance_id, old.instance_id);
        assert_eq!(new.stacks, Some(1));
        assert_eq!(new.applied_at_ms, 0);
        assert_eq!(
            new.expires_at_ms,
            if index >= v["killMinimumStageIndex"].as_u64().unwrap() as usize {
                1000 + v["killRefreshMs"].as_u64().unwrap()
            } else {
                10000
            }
        );
        c.params.basic_spirit_on_hit = 0;
    }
}
