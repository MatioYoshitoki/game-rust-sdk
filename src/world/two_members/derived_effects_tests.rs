use super::periodic_tests::{bind, pulse};
use super::*;
use crate::config::AttackRule;
use serde_json::{Value, json};
fn add(c: &mut Config, d: Value) {
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
}
fn numeric(id: &str) -> Value {
    json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":0,"duration_ms":120,"duplicate_policy":"refresh","source_scoped":true})
}
fn setup() -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(false, false);
    w.statuses = Default::default();
    w.periodic_enabled = true;
    w.shields_enabled = true;
    for p in c.combat_rules.as_mut().unwrap().profiles.values_mut() {
        p.tier = 0;
    }
    c.combat_rules.as_mut().unwrap().status_windows.clear();
    c.combat_rules.as_mut().unwrap().shield_records.clear();
    c.combat_rules.as_mut().unwrap().status_cleanses.clear();
    for n in &mut w.npcs {
        n.hp = 500;
        std::sync::Arc::make_mut(&mut n.profile)
            .stats
            .insert("hp".into(), 1000);
        std::sync::Arc::make_mut(&mut n.profile)
            .stats
            .insert("def".into(), 0);
        std::sync::Arc::make_mut(&mut n.profile)
            .stats
            .insert("atk".into(), 100);
    }
    for m in &mut w.members {
        m.facts.atk_points = 100;
        m.facts.def_points = 0;
    }
    add(&mut c, numeric("end-carrier"));
    add(
        &mut c,
        json!({"id":"end-hot","mechanism":"periodic_heal","heal_bps":1000,"tick_interval_ms":60,"duration_ms":180,"duplicate_policy":"refresh","source_scoped":true}),
    );
    add(
        &mut c,
        json!({"id":"end-dot","mechanism":"periodic_damage","damage_bps":1000,"tick_interval_ms":60,"duration_ms":180,"duplicate_policy":"refresh","source_scoped":true}),
    );
    add(
        &mut c,
        json!({"id":"end-shield","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let r = c.combat_rules.as_mut().unwrap();
    r.status_ends.insert(
        "end-carrier".into(),
        serde_json::from_value(
            json!({"on":["expired"],"target_statuses":["end-hot","end-shield"]}),
        )
        .unwrap(),
    );
    r.periodic_damage.insert(
        "end-dot".into(),
        AttackRule {
            ignore_armor: true,
            can_crit: false,
            ..Default::default()
        },
    );
    (c, w)
}
fn finish(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.prepare_periodic(now, &mut e);
    w.finish_status_ends(c, now, &mut e);
    e
}
fn own(w: &mut World, c: &Config, id: &str) {
    let e = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), e, id, 0);
}
fn count(e: &Events) -> usize {
    e.iter()
        .filter(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::StatusEndResolved { .. }
                }
            )
        })
        .count()
}
fn dot_plan(c: &mut Config) {
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_ends
        .get_mut("end-carrier")
        .unwrap()
        .target_statuses = vec!["end-dot".into()];
}
#[test]
fn derived_end_expiry_once_then_native_hot_and_shield() {
    let (c, mut w) = setup();
    own(&mut w, &c, "end-carrier");
    assert_eq!(count(&finish(&mut w, &c, 120)), 1);
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| w.statuses.is_derived(&s.instance_id))
    );
    assert_eq!(count(&finish(&mut w, &c, 120)), 0);
    pulse(&mut w, &c, 180);
    pulse(&mut w, &c, 240);
    assert_eq!(w.members[0].hp, 70);
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 240).own.shield_total,
        Some(10)
    );
}
#[test]
fn derived_end_selected_reasons_consume_only_at_zero() {
    let (mut c, mut w) = setup();
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|s| s.id == "end-carrier")
        .unwrap();
    d.duplicate_policy = crate::config::DuplicatePolicy::Stack;
    d.max_stacks = Some(2);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_ends
        .get_mut("end-carrier")
        .unwrap()
        .on = vec![StatusRemovalReason::Consumed];
    own(&mut w, &c, "end-carrier");
    own(&mut w, &c, "end-carrier");
    let id = w.statuses.entries()[0].instance_id.clone();
    assert!(w.statuses.consume(&id, 1, &mut vec![]));
    assert_eq!(count(&finish(&mut w, &c, 60)), 0);
    assert!(w.statuses.consume(&id, 1, &mut vec![]));
    assert_eq!(count(&finish(&mut w, &c, 60)), 1);
    assert!(!w.statuses.consume(&id, 1, &mut vec![]));
}
#[test]
fn derived_end_cleanup_reasons_never_generate_benefits() {
    for reason in [
        StatusRemovalReason::SourceInvalid,
        StatusRemovalReason::TargetInvalid,
        StatusRemovalReason::Reset,
        StatusRemovalReason::Cleansed,
    ] {
        let (c, mut w) = setup();
        own(&mut w, &c, "end-carrier");
        w.statuses.remove(reason, |_| true, &mut vec![]);
        assert_eq!(count(&finish(&mut w, &c, 120)), 0);
        assert!(w.statuses.entries().is_empty());
    }
}
#[test]
fn derived_end_explicit_cleanse_is_separate_policy() {
    let (mut c, mut w) = setup();
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_ends
        .get_mut("end-carrier")
        .unwrap()
        .on = vec![StatusRemovalReason::Cleansed];
    own(&mut w, &c, "end-carrier");
    w.statuses
        .remove(StatusRemovalReason::Cleansed, |_| true, &mut vec![]);
    assert_eq!(count(&finish(&mut w, &c, 60)), 1);
}
#[test]
fn derived_end_transfer_preserves_identity_source_deadline_and_new_holder() {
    let (c, mut w) = setup();
    own(&mut w, &c, "end-carrier");
    let id = w.statuses.entries()[0].instance_id.clone();
    let holder = status_entity(&w.members[1].reference.entity);
    w.statuses.retarget(&[id], &holder, &mut vec![]);
    let e = finish(&mut w, &c, 120);
    assert_eq!(count(&e), 2);
    assert!(e.iter().all(|(i, _)| [Some(0), Some(1)].contains(i)));
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.target == holder && s.source.id == "P00")
    );
    pulse(&mut w, &c, 180);
    assert_eq!((w.members[0].hp, w.members[1].hp), (50, 60));
}
#[test]
fn derived_end_invalid_source_or_holder_does_not_apply() {
    for source in [true, false] {
        let (c, mut w) = setup();
        let target = w.members[1].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), target, "end-carrier", 0);
        w.members[if source { 0 } else { 1 }].hp = 0;
        let e = finish(&mut w, &c, 120);
        assert_eq!(count(&e), 2);
        assert!(w.statuses.entries().is_empty());
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::StatusEndResolved {
                    failure: Some(_),
                    ..
                }
            }
        )));
    }
}
#[test]
fn derived_end_capacity_failure_keeps_carrier_ended_and_no_partial_leaves() {
    let (mut c, mut w) = setup();
    own(&mut w, &c, "end-carrier");
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let e = finish(&mut w, &c, 120);
    assert_eq!(count(&e), 1);
    assert!(w.statuses.entries().is_empty());
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusEndResolved {
                failure: Some(Error::Capacity),
                ..
            }
        }
    )));
}
#[test]
fn derived_end_shared_step_budget_discards_excess_without_replay() {
    let (mut c, mut w) = setup();
    c.combat_rules
        .as_mut()
        .unwrap()
        .max_status_end_releases_per_step = 1;
    own(&mut w, &c, "end-carrier");
    let t = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(1), t, "end-carrier", 0);
    let e = finish(&mut w, &c, 120);
    assert_eq!(count(&e), 1);
    assert!(e.iter().any(|(i, e)| *i == Some(1)
        && matches!(
            e,
            EventV4::Private {
                event: Event::StatusEndsSuppressed { count: 1, .. }
            }
        )));
    assert_eq!(count(&finish(&mut w, &c, 180)), 0);
}
fn watch(c: &mut Config) {
    c.combat_rules.as_mut().unwrap().triggers=serde_json::from_value(json!([{"id":"damage-gain","profiles":["member"],"when":"damage_dealt","include_periodic":true,"every":1,"cooldown_ms":0,"priority":1,"effect":{"kind":"restore_spirit","amount":5}}])).unwrap();
}
#[test]
fn derived_dot_uses_native_damage_without_normal_damage_passives() {
    for derived in [true, false] {
        let (mut c, mut w) = setup();
        dot_plan(&mut c);
        watch(&mut c);
        let target = w.npcs[0].entity.clone();
        bind(
            &mut w,
            &c,
            Owner::Member(0),
            target,
            if derived { "end-carrier" } else { "end-dot" },
            0,
        );
        let now = if derived {
            finish(&mut w, &c, 120);
            180
        } else {
            60
        };
        pulse(&mut w, &c, now);
        assert_eq!(w.npcs[0].hp, 490);
        assert_eq!(w.members[0].spirit, if derived { 50 } else { 55 });
    }
}
#[test]
fn derived_damage_depleting_normal_end_shield_cannot_start_another_chain() {
    for derived in [true, false] {
        let (mut c, mut w) = setup();
        dot_plan(&mut c);
        c.combat_rules.as_mut().unwrap().status_ends.insert(
            "end-shield".into(),
            serde_json::from_value(json!({"on":["depleted"],"target_statuses":["end-dot"]}))
                .unwrap(),
        );
        let d = c
            .pve_mut()
            .status_definitions
            .iter_mut()
            .find(|s| s.id == "end-shield")
            .unwrap();
        d.shield_bps = Some(100);
        let target = w.npcs[0].entity.clone();
        bind(
            &mut w,
            &c,
            Owner::Member(1),
            target.clone(),
            "end-shield",
            0,
        );
        bind(
            &mut w,
            &c,
            Owner::Member(0),
            target,
            if derived { "end-carrier" } else { "end-dot" },
            0,
        );
        let now = if derived {
            finish(&mut w, &c, 120);
            180
        } else {
            60
        };
        pulse(&mut w, &c, now);
        let mut e = vec![];
        w.finish_status_ends(&c, now, &mut e);
        assert_eq!(count(&e), if derived { 0 } else { 1 });
        assert_eq!(w.npcs[0].hp, 500);
    }
}
#[test]
fn derived_lineage_survives_transfer_ignore_and_clears_on_real_refresh() {
    let (mut c, mut w) = setup();
    dot_plan(&mut c);
    let target = w.npcs[0].entity.clone();
    bind(
        &mut w,
        &c,
        Owner::Member(0),
        target.clone(),
        "end-carrier",
        0,
    );
    finish(&mut w, &c, 120);
    let id = w.statuses.entries()[0].instance_id.clone();
    let next = status_entity(&w.npcs[1].entity);
    w.statuses
        .retarget(std::slice::from_ref(&id), &next, &mut vec![]);
    assert!(w.statuses.is_derived(&id));
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "end-dot")
        .unwrap()
        .duplicate_policy = crate::config::DuplicatePolicy::Ignore;
    let t = w.npcs[1].entity.clone();
    bind(&mut w, &c, Owner::Member(0), t.clone(), "end-dot", 150);
    assert!(w.statuses.is_derived(&id));
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "end-dot")
        .unwrap()
        .duplicate_policy = crate::config::DuplicatePolicy::Refresh;
    bind(&mut w, &c, Owner::Member(0), t, "end-dot", 150);
    assert!(!w.statuses.is_derived(&id));
    assert_eq!(w.statuses.entries()[0].instance_id, id);
}
fn delayed(live: bool) -> (Config, World) {
    let (mut c, w) = setup();
    add(
        &mut c,
        json!({"id":"delayed","mechanism":"periodic_damage","damage_bps":0,"tick_interval_ms":120,"duration_ms":180,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let r = c.combat_rules.as_mut().unwrap();
    r.periodic_sampling.insert(
        "delayed".into(),
        if live {
            PeriodicSampling::SourceLive
        } else {
            PeriodicSampling::SourceSnapshot
        },
    );
    r.delayed_damage.insert("delayed".into(),serde_json::from_value(json!({"fixed":1,"source_power_bps":5000,"target_missing_hp_bps":2000,"caps_by_target_profile":{}})).unwrap());
    r.periodic_damage.insert(
        "delayed".into(),
        AttackRule {
            ignore_armor: true,
            can_crit: false,
            ..Default::default()
        },
    );
    (c, w)
}
fn delayed_tick(w: &mut World, c: &Config, now: u64) -> u32 {
    pulse(w, c, now)
        .into_iter()
        .find_map(|(_, e)| {
            if let EventV4::Private {
                event: Event::PeriodicTick { raw_damage, .. },
            } = e
            {
                Some(raw_damage)
            } else {
                None
            }
        })
        .unwrap()
}
#[test]
fn derived_delayed_source_snapshot_but_target_missing_hp_always_live() {
    let (c, mut w) = delayed(false);
    let t = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
    w.members[0].facts.atk_points = 200;
    w.npcs[0].hp = 750;
    assert_eq!(delayed_tick(&mut w, &c, 120), 101);
    assert_eq!(w.npcs[0].hp, 649);
}
#[test]
fn derived_delayed_live_source_and_target_profile_cap() {
    let (mut c, mut w) = delayed(true);
    c.combat_rules
        .as_mut()
        .unwrap()
        .delayed_damage
        .get_mut("delayed")
        .unwrap()
        .caps_by_target_profile
        .insert("npc".into(), 5000);
    let t = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
    w.members[0].facts.atk_points = 200;
    assert_eq!(delayed_tick(&mut w, &c, 120), 100);
    assert_eq!(w.npcs[0].hp, 400);
}
#[test]
fn derived_delayed_one_deadline_late_step_and_no_second_execution() {
    let (c, mut w) = delayed(false);
    let t = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
    assert!(pulse(&mut w, &c, 60).iter().all(|(_, e)| !matches!(
        e,
        EventV4::Private {
            event: Event::PeriodicTick { .. }
        }
    )));
    assert_eq!(delayed_tick(&mut w, &c, 150), 151);
    assert!(pulse(&mut w, &c, 150).is_empty());
    assert!(pulse(&mut w, &c, 180).iter().all(|(_, e)| !matches!(
        e,
        EventV4::Private {
            event: Event::PeriodicTick { .. }
        }
    )));
}
#[test]
fn derived_delayed_dispel_and_source_exit_cancel_without_replay() {
    for reason in [
        StatusRemovalReason::Cleansed,
        StatusRemovalReason::SourceInvalid,
    ] {
        let (c, mut w) = delayed(false);
        let t = w.npcs[0].entity.clone();
        bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
        w.statuses.remove(reason, |_| true, &mut vec![]);
        assert!(pulse(&mut w, &c, 120).is_empty());
        assert_eq!(w.npcs[0].hp, 500);
    }
}
#[test]
fn derived_delayed_fraction_floor_missing_maximum_and_large_values() {
    let (mut c, mut w) = delayed(false);
    let f = c
        .combat_rules
        .as_mut()
        .unwrap()
        .delayed_damage
        .get_mut("delayed")
        .unwrap();
    f.fixed = 0;
    f.source_power_bps = 5000;
    f.target_missing_hp_bps = 5000;
    assert_eq!(f.amount(1, 1, "npc"), 1);
    assert_eq!(f.amount(0, 0, "npc"), 0);
    f.source_power_bps = 100000;
    f.target_missing_hp_bps = 100000;
    assert_eq!(f.amount(u32::MAX, u32::MAX, "npc"), u32::MAX);
    f.source_power_bps = 0;
    f.target_missing_hp_bps = 10000;
    let t = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
    std::sync::Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("hp".into(), 400);
    assert_eq!(delayed_tick(&mut w, &c, 120), 0);
    assert_eq!(w.npcs[0].hp, 399); // Existing native minimum damage remains authoritative even at raw zero.
}
#[test]
fn derived_end_and_delayed_cold_loader_strict_refs_nonrecursive_leaves_and_clock() {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/power-a.json"
    ))
    .unwrap();
    doc["pve"]["status_definitions"].as_array_mut().unwrap().extend([
        numeric("end-carrier"),
        json!({"id":"end-dot","mechanism":"periodic_damage","damage_bps":0,"tick_interval_ms":120,"duration_ms":150,"duplicate_policy":"refresh","source_scoped":true}),
    ]);
    doc["combat_rules"]["status_ends"] =
        json!({"end-carrier":{"on":["expired","cleansed"],"target_statuses":["end-dot"]}});
    doc["combat_rules"]["delayed_damage"] = json!({"end-dot":{"fixed":0,"source_power_bps":0,"target_missing_hp_bps":1000,"caps_by_target_profile":{"npc":5000}}});
    doc["combat_rules"]["periodic_sampling"]["end-dot"] = json!("source_snapshot");
    doc["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|d| d["id"] == "end-carrier")
        .unwrap()["modifier_bps"] = json!(1000);
    let carrier = doc["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|d| d["id"] == "end-carrier")
        .unwrap();
    carrier["duplicate_policy"] = json!("stack");
    carrier["max_stacks"] = json!(2);
    doc["combat_rules"]["status_policies"]["end-dot"] =
        json!({"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]});
    doc["combat_rules"]["triggers"] = json!([{"id":"consume-to-delay","profiles":["member"],"when":"direct_hit","include_periodic":false,"every":1,"cooldown_ms":6000,"priority":1,"conditions":{"status":{"subject":"owner","definition_id":"end-carrier","at_least":2,"consume":2}},"effect":{"kind":"apply_other_status","status_id":"end-dot"}}]);
    p.write(&doc);
    p.load().unwrap();
    for case in 0..31 {
        let mut d = doc.clone();
        match case {
            0 => d["mechanism_version"] = json!(104),
            1 => d["combat_rules"]["status_ends"] = Value::Null,
            2 => d["combat_rules"]["status_ends"]["end-carrier"]["on"] = json!([]),
            3 => {
                d["combat_rules"]["status_ends"]["end-carrier"]["on"] =
                    json!(["expired", "expired"])
            }
            4 => d["combat_rules"]["status_ends"]["end-carrier"]["on"] = json!(["source_invalid"]),
            5 => {
                d["combat_rules"]["status_ends"]["end-carrier"]["target_statuses"] =
                    json!(["missing"])
            }
            6 => {
                d["combat_rules"]["status_ends"]["end-carrier"]["target_statuses"] =
                    json!(["end-carrier"])
            }
            7 => {
                d["combat_rules"]["status_ends"]["end-carrier"]["target_statuses"] =
                    json!(["ward", "ward"])
            }
            8 => {
                d["combat_rules"]["status_ends"]["end-carrier"]["target_statuses"] =
                    json!(["wound", "ward", "end-dot"])
            }
            9 => d["combat_rules"]["status_ends"]["end-carrier"]["extra"] = json!(true),
            10 => d["combat_rules"]["max_status_end_releases_per_step"] = json!(0),
            11 => d["combat_rules"]["max_status_end_releases_per_step"] = json!(129),
            12 => d["combat_rules"]["delayed_damage"] = Value::Null,
            13 => {
                d["combat_rules"]["delayed_damage"]["end-dot"]["target_missing_hp_bps"] =
                    json!(100001)
            }
            14 => {
                d["combat_rules"]["delayed_damage"]["end-dot"]["caps_by_target_profile"] =
                    json!({"missing":1})
            }
            15 => {
                d["combat_rules"]["delayed_damage"]["end-dot"]["caps_by_target_profile"] =
                    json!({"npc":100001})
            }
            16 => d["combat_rules"]["periodic_sampling"]["end-dot"] = json!("target_live"),
            17 => {
                d["combat_rules"]["periodic_sampling"]
                    .as_object_mut()
                    .unwrap()
                    .remove("end-dot");
            }
            18 => {
                d["combat_rules"]["delayed_damage"]["end-dot"]["target_missing_hp_bps"] = json!(0)
            }
            19 => {
                d["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()["duration_ms"] = json!(300)
            }
            20 => {
                d["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()["source_scoped"] = json!(false)
            }
            21 => {
                d["combat_rules"]["delayed_damage"]["ward"] =
                    d["combat_rules"]["delayed_damage"]["end-dot"].clone()
            }
            22 => {
                d["combat_rules"]["periodic_damage"]["end-dot"] = serde_json::to_value(AttackRule {
                    life_steal_bps: 1000,
                    ..Default::default()
                })
                .unwrap()
            }
            23 => {
                d["combat_rules"]["periodic_damage"]["end-dot"] = serde_json::to_value(AttackRule {
                    base_power: Some(1),
                    ..Default::default()
                })
                .unwrap()
            }
            24 => {
                d["combat_rules"]["status_ends"]["end-dot"] =
                    json!({"on":["expired"],"target_statuses":["ward"]})
            }
            25 => d["combat_rules"]["delayed_damage"]["end-dot"]["derived"] = json!(true),
            26 => {
                d["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()["derived"] = json!(true)
            }
            27 => {
                d["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()["end_effects"] = json!(1)
            }
            28 => d["combat_rules"]["delayed_damage"]["end-dot"]["fixed"] = Value::Null,
            29 => {
                d["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()["tick_interval_ms"] = json!(u64::MAX)
            }
            30 => d["combat_rules"]["status_ends"]["end-carrier"]["on"] = Value::Null,
            _ => unreachable!(),
        }
        p.write(&d);
        assert!(p.load().is_err(), "invalid case {case}");
    }
}
#[test]
fn derived_record_end_and_counter_create_native_hot_shield_and_dot_leaves() {
    let (mut c, mut w) = setup();
    watch(&mut c);
    c.combat_rules.as_mut().unwrap().shield_records.insert("ward".into(),serde_json::from_value(json!({"basis":"own_absorbed","cap_fixed":1000,"cap_power_bps":0,"damage_reduction_bps":0,"refresh":"reset","on_depleted":true,"on_expired":false,"on_consumed":false,"counter":{"ratio_bps":10000,"damage_kind":"physical","ignore_armor":true,"radius":120,"max_targets":1,"on_hit_target_statuses":["end-dot"]},"after_end_target_statuses":["end-hot","end-shield"]})).unwrap());
    own(&mut w, &c, "ward");
    let source = w.npcs[0].entity.clone();
    let target = w.members[0].reference.entity.clone();
    let mut e = vec![];
    w.resolve_damage_with_rule(
        combat::DamageIntent {
            source,
            target,
            raw: 10,
            ignore_armor: true,
            direct: false,
        },
        Some(&AttackRule {
            can_crit: false,
            ..Default::default()
        }),
        &c,
        30,
        &mut e,
    );
    w.finish_shield_records(&c, 30, &mut e);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ShieldRecordEnded {
                recorded: 10,
                targets_hit: 1,
                ..
            }
        }
    )));
    assert_eq!(w.statuses.entries().len(), 3);
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| w.statuses.is_derived(&s.instance_id))
    );
    let npc_hp = w.npcs[0].hp;
    pulse(&mut w, &c, 90);
    assert_eq!(w.npcs[0].hp, npc_hp - 10);
    assert_eq!(w.members[0].spirit, 50);
    assert_eq!(w.members[0].hp, 60);
}
#[test]
fn derived_dot_native_kill_keeps_original_source_credit() {
    let (mut c, mut w) = setup();
    dot_plan(&mut c);
    watch(&mut c);
    let t = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(1), t, "end-carrier", 0);
    finish(&mut w, &c, 120);
    w.npcs[0].hp = 5;
    let template = w.npcs[0].template_id.clone();
    let kills = w.members[1].tally.get(&template).copied().unwrap_or(0);
    pulse(&mut w, &c, 180);
    assert_eq!(w.npcs[0].hp, 0);
    assert_eq!(w.members[1].tally[&template], kills + 1);
    assert_eq!(w.members[0].spirit, 50);
    assert_eq!(w.members[1].spirit, 50);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "end-dot")
    );
}
#[test]
fn derived_delayed_native_immunity_shield_and_reduction_order() {
    for immune in [true, false] {
        let (mut c, mut w) = delayed(false);
        let mut d = numeric("delayed-dr");
        d["attribute"] = json!(if immune {
            "damage_immunity"
        } else {
            "damage_reduction"
        });
        d["modifier_bps"] = json!(if immune { 10000 } else { 2000 });
        d["duration_ms"] = json!(300);
        add(&mut c, d);
        let t = w.npcs[0].entity.clone();
        bind(&mut w, &c, Owner::Npc(0), t.clone(), "delayed-dr", 0);
        bind(&mut w, &c, Owner::Npc(0), t.clone(), "end-shield", 0);
        bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
        let e = pulse(&mut w, &c, 120);
        let row = e
            .iter()
            .find_map(|(_, e)| {
                if let EventV4::Private {
                    event:
                        Event::PeriodicTick {
                            raw_damage,
                            absorbed,
                            hp_damage,
                            ..
                        },
                } = e
                {
                    Some((*raw_damage, *absorbed, *hp_damage))
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(row, if immune { (151, 0, 0) } else { (151, 100, 20) });
        assert_eq!(w.npcs[0].hp, if immune { 500 } else { 480 });
    }
}
#[test]
fn derived_pending_queue_is_bounded_and_drained_in_one_frame() {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 2000;
    // Component stress deliberately exceeds World admission limits to test the hard cache bound.
    let base = c
        .pve
        .as_ref()
        .unwrap()
        .status_definitions
        .iter()
        .find(|d| d.id == "end-carrier")
        .unwrap()
        .clone();
    for i in 0..1100 {
        let mut d = base.clone();
        d.id = format!("stress-{i}");
        d.end_effects = 1;
        let e = status_entity(&w.members[0].reference.entity);
        assert!(w.statuses.apply(
            &[d],
            (&e, 100),
            (&e, 100),
            0,
            c.pve.as_ref().unwrap().status_rules.as_ref().unwrap(),
            &mut vec![]
        ));
    }
    w.statuses.expire(120, &mut vec![]);
    assert_eq!(w.statuses.take_status_ends().len(), 1024);
    let overflow = w.statuses.take_end_overflow();
    assert_eq!(overflow.len(), 1);
    assert_eq!(overflow[0].2, 76);
    assert!(w.statuses.take_status_ends().is_empty());
    assert!(w.statuses.take_end_overflow().is_empty());
}
#[test]
fn derived_delayed_selected_skill_power_is_not_replaced_by_holder_or_provoker() {
    let (mut c, mut w) = delayed(false);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_power_sources
        .insert("delayed".into(), c.skill.id.clone());
    w.members[1].facts.atk_points = 900;
    let t = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
    // The selected native unit skill is 8, rather than the source ATK 100 or peer ATK 900.
    assert_eq!(delayed_tick(&mut w, &c, 120), 105);
}
#[test]
fn derived_record_and_generic_end_share_one_attempt_budget() {
    let (mut c, mut w) = setup();
    let r = c.combat_rules.as_mut().unwrap();
    r.max_status_end_releases_per_step = 1;
    r.shield_records.insert("ward".into(),serde_json::from_value(json!({"basis":"incoming","cap_fixed":10,"cap_power_bps":0,"damage_reduction_bps":0,"refresh":"reset","on_depleted":false,"on_expired":false,"on_consumed":true,"after_end_target_statuses":["end-shield"]})).unwrap());
    own(&mut w, &c, "ward");
    own(&mut w, &c, "end-carrier");
    let id = w
        .statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "ward")
        .unwrap()
        .instance_id
        .clone();
    w.statuses.consume(&id, 1, &mut vec![]);
    let mut e = vec![];
    w.finish_shield_records(&c, 120, &mut e);
    w.prepare_periodic(120, &mut e);
    w.finish_status_ends(&c, 120, &mut e);
    assert_eq!(count(&e), 0);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusEndsSuppressed { count: 1, .. }
        }
    )));
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(w.statuses.entries()[0].definition_id, "end-shield");
}
#[test]
fn derived_exit_and_generation_invalidation_clear_pending_and_leaf_work() {
    for generation in [true, false] {
        let (mut c, mut w) = delayed(false);
        dot_plan(&mut c);
        let t = w.npcs[0].entity.clone();
        bind(&mut w, &c, Owner::Member(0), t.clone(), "end-carrier", 0);
        bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
        if generation {
            w.members[0].reference.entity.generation += 1;
        } else {
            let source = w.members[0].reference.entity.clone();
            w.clear_owned_statuses(&source, StatusRemovalReason::SourceInvalid, &mut vec![]);
        }
        let e = finish(&mut w, &c, 120);
        let p = pulse(&mut w, &c, 120);
        assert_eq!(w.npcs[0].hp, 500);
        assert!(p.iter().all(|(_, e)| !matches!(
            e,
            EventV4::Private {
                event: Event::PeriodicTick { .. }
            }
        )));
        assert!(e.iter().all(|(_, e)| !matches!(
            e,
            EventV4::Private {
                event: Event::StatusApplied { .. }
            }
        )));
    }
}
#[test]
fn derived_delayed_and_native_hot_same_frame_keep_damage_first_then_restoration() {
    let (mut c, mut w) = delayed(false);
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|s| s.id == "end-hot")
        .unwrap();
    d.tick_interval_ms = Some(120);
    let t = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), t.clone(), "delayed", 0);
    bind(&mut w, &c, Owner::Member(0), t, "end-hot", 0);
    let e = pulse(&mut w, &c, 120);
    // 1 + 50 + 20% * 50 = 61. Lethal damage removes the HOT, so it cannot revive.
    assert_eq!(w.members[0].hp, 0);
    assert!(!e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::PeriodicHealTick { .. }
        }
    )));
}
#[test]
fn derived_original_interface_contract_vectors_run_in_native_world() {
    let v: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_derived_vectors.json"
    ))
    .unwrap();
    assert_eq!(v["fullOriginalStageBindingsAccepted"], false);
    assert_eq!(
        v["delayedParameterNames"],
        json!([
            "stateId",
            "requiredStacks",
            "delayMs",
            "missingHpDamageBps",
            "bossPowerCapBps",
            "casterId",
            "sourcePower",
            "skillId",
            "cultivationId"
        ])
    );
    for row in v["declaredVectors"].as_array().unwrap() {
        let (mut c, mut w) = delayed(false);
        let f = c
            .combat_rules
            .as_mut()
            .unwrap()
            .delayed_damage
            .get_mut("delayed")
            .unwrap();
        f.fixed = row["fixed"].as_u64().unwrap() as u32;
        f.source_power_bps = row["sourcePowerBps"].as_u64().unwrap() as u32;
        f.target_missing_hp_bps = row["missingHpBps"].as_u64().unwrap() as u32;
        w.members[0].facts.atk_points = row["power"].as_u64().unwrap() as u32;
        w.npcs[0].hp = 1000 - row["missingHp"].as_u64().unwrap() as u32;
        let t = w.npcs[0].entity.clone();
        bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
        assert_eq!(
            delayed_tick(&mut w, &c, 120),
            row["expected"].as_u64().unwrap() as u32
        );
    }
}
#[test]
fn derived_delayed_conditional_consumption_capacity_failure_preserves_stacks_and_icd() {
    let (mut c, mut w) = delayed(false);
    let d = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "end-carrier")
        .unwrap();
    d.duplicate_policy = crate::config::DuplicatePolicy::Stack;
    d.max_stacks = Some(2);
    d.duration_ms = 6000;
    let r = c.combat_rules.as_mut().unwrap();
    r.triggers=serde_json::from_value(json!([{"id":"consume-to-delay","profiles":["member"],"when":"direct_hit","include_periodic":false,"every":1,"cooldown_ms":6000,"priority":1,"conditions":{"status":{"subject":"owner","definition_id":"end-carrier","at_least":2,"consume":2}},"effect":{"kind":"apply_other_status","status_id":"delayed"}}])).unwrap();
    r.status_policies.insert(
        "delayed".into(),
        serde_json::from_value(
            json!({"disposition":"harmful","dispellable":true,"transferable":true,"blocks":[]}),
        )
        .unwrap(),
    );
    own(&mut w, &c, "end-carrier");
    own(&mut w, &c, "end-carrier");
    let t = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), t, "end-shield", 0);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut e = vec![];
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (1, false),
        &c,
        0,
        &mut e,
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "end-carrier" && s.stacks == Some(2))
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "delayed")
    );
    w.statuses.remove(
        StatusRemovalReason::Cleansed,
        |s| s.definition_id == "end-shield",
        &mut vec![],
    );
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (1, false),
        &c,
        0,
        &mut e,
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "end-carrier")
    );
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .filter(|s| s.definition_id == "delayed")
            .count(),
        1
    );
    w.combat_fact(
        crate::config::CombatFact::DirectHit,
        (Owner::Member(0), Owner::Npc(0)),
        (1, false),
        &c,
        0,
        &mut e,
    );
    assert_eq!(delayed_tick(&mut w, &c, 120), 151);
}
#[test]
fn derived_dot_keeps_once_per_generation_native_lethal_guard() {
    let (mut c, mut w) = setup();
    dot_plan(&mut c);
    c.combat_rules.as_mut().unwrap().death_guards.insert(
        "member".into(),
        serde_json::from_value(json!({"retain_hp":1,"heal_max_hp_bps":0,"self_statuses":[]}))
            .unwrap(),
    );
    let t = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), t, "end-carrier", 0);
    finish(&mut w, &c, 120);
    w.members[0].hp = 5;
    let e = pulse(&mut w, &c, 180);
    assert_eq!(w.members[0].hp, 1);
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::DeathGuardActivated { .. }
                }
            ))
            .count(),
        1
    );
    let e = pulse(&mut w, &c, 240);
    assert_eq!(w.members[0].hp, 0);
    assert!(!e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::DeathGuardActivated { .. }
        }
    )));
}
#[test]
fn derived_lineage_and_source_formula_are_not_accepted_as_public_status_fields() {
    let (c, mut w) = delayed(false);
    let t = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), t, "delayed", 0);
    let status = serde_json::to_value(&w.statuses.entries()[0]).unwrap();
    for key in [
        "derived",
        "end_effects",
        "periodic_inputs",
        "delayed_damage",
    ] {
        let mut bad = status.clone();
        bad[key] = json!(true);
        assert!(serde_json::from_value::<StatusView>(bad).is_err(), "{key}");
    }
}
