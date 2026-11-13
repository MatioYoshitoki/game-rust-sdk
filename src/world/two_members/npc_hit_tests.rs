use super::combat::{DamageIntent, DirectContact, dodge_probability};
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_npc_hit_validates_base_and_additional_template_stats() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-npc-hit-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    for pointer in [
        "/pve/stats",
        "/pve/additional_templates/evasion-probe/stats",
    ] {
        for value in [json!(0), json!(100), json!(1000000)] {
            let mut valid = doc.clone();
            valid.pointer_mut(pointer).unwrap()["hit"] = value.clone();
            let c = load(&valid).unwrap().default_config();
            assert_eq!(
                serde_json::to_value(c).unwrap().pointer(pointer).unwrap()["hit"],
                value
            );
        }
        for value in [json!(-1), json!(0.5), json!(1000001), Value::Null] {
            let mut invalid = doc.clone();
            invalid.pointer_mut(pointer).unwrap()["hit"] = value;
            assert!(load(&invalid).is_err());
        }
        let mut unknown = doc.clone();
        unknown.pointer_mut(pointer).unwrap()["hitt"] = json!(1);
        assert!(load(&unknown).is_err());
        let mut old = doc.clone();
        old["pve"]["additional_templates"]["evasion-probe"]["stats"]
            .as_object_mut()
            .unwrap()
            .remove("hit");
        old.pointer_mut(pointer).unwrap()["hit"] = json!(0);
        old["mechanism_version"] = json!(98);
        assert!(load(&old).is_err());
    }
    let mut omitted = doc;
    omitted["pve"]["additional_templates"]["evasion-probe"]["stats"]
        .as_object_mut()
        .unwrap()
        .remove("hit");
    assert!(load(&omitted).is_ok());
    omitted["mechanism_version"] = json!(98);
    assert!(load(&omitted).is_ok());
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::armor_penetration_tests::setup();
    c.mechanism_version = 99;
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 1000;
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"dodge_chance","id":"dodge-focus",
            "duration_ms":300,"modifier_bps":5000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "dodge-focus", 0);
    (c, w)
}

fn hit_points(w: &mut World, npc: usize, hit: u32) {
    Arc::make_mut(&mut w.npcs[npc].profile)
        .stats
        .insert("hit".into(), hit);
}

fn contact(w: &mut World, c: &Config, npc: usize, now: u64, statuses: &[String]) -> (bool, Events) {
    let source = w.npcs[npc].entity.clone();
    let mut events = vec![];
    let hit = w.direct_contact(
        Owner::Npc(npc),
        Owner::Member(0),
        DirectContact {
            origin: None,
            action_id: None,
            rule: None,
            ignore_armor: false,
            unavoidable: false,
            dead_source: false,
            source: &source,
            raw: 100,
            metal: false,
            hit_statuses: statuses,
        },
        c,
        now,
        &mut events,
    );
    (hit, events)
}

#[test]
fn member_npc_hit_uses_actual_source_and_preserves_shield_and_attached_effect_order() {
    assert_eq!(dodge_probability(5000, 0, 10000), 0.25);
    assert_eq!(dodge_probability(5000, 0, 5000), 1.0 / 3.0);
    assert_eq!(dodge_probability(5000, 0, 100000000), 5000.0 / 100010000.0);
    let (c, mut w) = setup();
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "ward", 0);
    hit_points(&mut w, 0, 50);
    hit_points(&mut w, 1, 100);
    let before = w.snapshot(0, &c, Phase::Active, 0).own;
    w.rng = 4608;
    let (hit, events) = contact(&mut w, &c, 0, 30, &["wound".into()]);
    assert!(!hit && events.is_empty());
    let after = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        (after.hp, after.shield_total, after.statuses),
        (before.hp, before.shield_total, before.statuses)
    );
    w.rng = 4608;
    assert!(contact(&mut w, &c, 1, 30, &["wound".into()]).0);
    let after = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(after.hp, before.hp);
    assert_eq!(
        after.shield_total.unwrap(),
        before.shield_total.unwrap() - 66
    );
    assert!(
        after
            .statuses
            .iter()
            .any(|s| s.definition_id == "wound" && s.source.id == w.npcs[1].entity.id)
    );
    w.members[0].protected = true;
    w.rng = 4608;
    assert!(!contact(&mut w, &c, 1, 60, &[]).0);
    assert_eq!(w.rng, 4608);
}

#[test]
fn member_npc_hit_keeps_melee_interval_critical_rng_and_periodic_rules() {
    let (c, mut w) = setup();
    w.npcs.truncate(1);
    let profile = Arc::make_mut(&mut w.npcs[0].profile);
    profile.combat.aggro_range = 100.;
    profile.combat.melee_range = 80.;
    profile.combat.cast_chance = 0.;
    profile.stats.insert("crit".into(), 50);
    let interval = profile.combat.attack_interval_ms;
    hit_points(&mut w, 0, 50);
    w.npcs[0].next_attack = 0;
    w.rng = 4608;
    w.advance_npcs(&c, 30, 0.03, &mut vec![]);
    let after_dodge = w.rng;
    assert_eq!(
        (w.members[0].hp, w.npcs[0].next_attack),
        (1000, 30 + interval)
    );
    w.advance_npcs(&c, 60, 0.03, &mut vec![]);
    assert_eq!(w.rng, after_dodge);
    hit_points(&mut w, 0, 100);
    w.npcs[0].next_attack = 60;
    w.rng = 4608;
    let _ = w.random();
    let critical = f64::from(w.random()) / 4294967296.0 < 0.5;
    let expected_rng = w.rng;
    w.rng = 4608;
    let mut events = vec![];
    w.advance_npcs(&c, 90, 0.03, &mut events);
    assert_eq!(w.rng, expected_rng);
    assert_eq!(w.npcs[0].next_attack, 90 + interval);
    assert!(events.iter().any(|(_, event)| matches!(event,
        EventV4::Damage {amount, ..} if *amount == if critical {20} else {13})));
    let periodic = DamageIntent {
        ignore_armor: false,
        source: w.npcs[0].entity.clone(),
        target: w.members[0].reference.entity.clone(),
        raw: 100,
        direct: false,
    };
    assert_eq!(
        w.resolve_damage(periodic, &c, 120, &mut vec![]).hp_damage,
        66
    );
    assert_eq!(w.rng, expected_rng);
}

#[test]
fn member_npc_hit_zero_omission_expiry_and_old_mechanism_preserve_rng() {
    let (mut c, mut w) = setup();
    hit_points(&mut w, 0, 0);
    w.rng = 4608;
    let (zero, _) = contact(&mut w, &c, 0, 30, &[]);
    let zero_rng = w.rng;
    Arc::make_mut(&mut w.npcs[0].profile).stats.remove("hit");
    w.rng = 4608;
    assert_eq!(contact(&mut w, &c, 0, 30, &[]).0, zero);
    assert_eq!(w.rng, zero_rng);
    hit_points(&mut w, 0, 100);
    c.mechanism_version = 98;
    w.rng = 4608;
    assert!(!contact(&mut w, &c, 0, 30, &[]).0);
    assert_eq!(w.rng, zero_rng);
    c.mechanism_version = 99;
    w.rng = 4608;
    assert!(contact(&mut w, &c, 0, 299, &[]).0);
    assert_eq!(w.rng, zero_rng);
    w.rng = 1;
    assert!(contact(&mut w, &c, 0, 300, &[]).0);
    assert_eq!(w.rng, 1);
}
