use super::combat::{DamageIntent, DirectContact};
use super::control_immunity_tests::own_events;
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_damage_immunity_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-damage-immunity-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let config = load(&doc).unwrap().default_config();
    let definitions = &config.pve.as_ref().unwrap().status_definitions;
    let def = definitions.len() - 1;
    assert_eq!(
        serde_json::to_value(&definitions[def]).unwrap(),
        doc["pve"]["status_definitions"][def]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(99);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(0)),
        ("modifier_bps", Value::Null),
        ("duplicate_policy", json!("ignore")),
        ("source_scoped", json!(false)),
        ("mechanism", json!("timed_attribute_modifier")),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][def][field] = value;
        assert!(load(&invalid).is_err());
    }
    for field in ["on_commit_self_statuses", "on_hit_target_statuses"] {
        let mut invalid = doc.clone();
        invalid["skill"][field] = json!(["damage-guard"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["immunity-probe"]["combat"]["projectile"][field] =
            json!(["damage-guard"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] =
        json!({(id.as_str()):{"heal_hp":10,"on_use_self_statuses":["damage-guard"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({id:{"cleanse_self_statuses":["damage-guard"]}});
    assert!(load(&item).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::armor_penetration_tests::setup();
    c.mechanism_version = 100;
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 1000;
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_control","attribute":"damage_immunity","id":"damage-guard",
            "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    (c, w)
}

fn guard(w: &mut World, c: &Config, now: u64) -> Events {
    let own = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), own, "damage-guard", now)
}

fn contact(w: &mut World, c: &Config, member: usize, now: u64) -> (bool, Events) {
    let source = w.npcs[0].entity.clone();
    let mut events = vec![];
    let hit = w.direct_contact(
        Owner::Npc(0),
        Owner::Member(member),
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
            hit_statuses: &["wound".into()],
        },
        c,
        now,
        &mut events,
    );
    (hit, events)
}

#[test]
fn member_damage_immunity_precedes_dodge_shield_and_attached_statuses() {
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"dodge_chance","id":"dodge-focus",
            "duration_ms":300,"modifier_bps":5000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own.clone(), "ward", 0);
    bind(&mut w, &c, Owner::Member(0), own, "dodge-focus", 0);
    guard(&mut w, &c, 0);
    let before = w.snapshot(0, &c, Phase::Active, 0).own;
    w.members[0].extract_end = Some(600);
    w.rng = 1;
    let (hit, events) = contact(&mut w, &c, 0, 30);
    assert!(!hit && events.is_empty());
    assert_eq!(w.rng, 1);
    assert_eq!(w.members[0].extract_end, Some(600));
    let after = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        (after.hp, after.shield_total, after.statuses),
        (before.hp, before.shield_total, before.statuses)
    );
    assert!(contact(&mut w, &c, 1, 30).0);
    assert_eq!(w.rng, 1);
    let (hit, events) = contact(&mut w, &c, 0, 300);
    assert!(hit);
    let after = w.snapshot(0, &c, Phase::Active, 300).own;
    assert_eq!(after.hp, 1000);
    assert_eq!(
        after.shield_total.unwrap(),
        before.shield_total.unwrap() - 66
    );
    assert!(own_events(&events).iter().any(|e| matches!(e,
        Event::StatusApplied {status} if status.definition_id == "wound")));
    assert!(w.members[0].extract_end.is_none());
}

#[test]
fn member_damage_immunity_keeps_melee_interval_and_skips_critical_rng() {
    let (mut c, mut w) = setup();
    w.npcs.truncate(1);
    let profile = Arc::make_mut(&mut w.npcs[0].profile);
    profile.combat.aggro_range = 100.;
    profile.combat.melee_range = 80.;
    profile.combat.cast_chance = 0.;
    profile.stats.insert("crit".into(), 50);
    let interval = profile.combat.attack_interval_ms;
    w.npcs[0].next_attack = 0;
    guard(&mut w, &c, 0);
    w.rng = 1;
    w.advance_npcs(&c, 30, 0.03, &mut vec![]);
    assert_eq!(
        (w.members[0].hp, w.rng, w.npcs[0].next_attack),
        (1000, 1, 30 + interval)
    );
    w.advance_npcs(&c, 60, 0.03, &mut vec![]);
    assert_eq!(w.rng, 1);
    c.mechanism_version = 99;
    let intent = DamageIntent {
        ignore_armor: false,
        source: w.npcs[0].entity.clone(),
        target: w.members[0].reference.entity.clone(),
        raw: 100,
        direct: false,
    };
    assert_eq!(w.resolve_damage(intent, &c, 90, &mut vec![]).hp_damage, 66);
    c.mechanism_version = 100;
    w.npcs[0].next_attack = 300;
    w.advance_npcs(&c, 300, 0.03, &mut vec![]);
    assert_eq!(w.rng, 270369);
    assert_eq!(w.members[0].hp, 1000 - 66 - 20);
    assert_eq!(w.npcs[0].next_attack, 300 + interval);
}

#[test]
fn member_damage_immunity_refresh_expiry_periodic_progress_and_exit_are_private() {
    let (mut c, mut w) = setup();
    let dot = c
        .pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "wound")
        .unwrap();
    dot.duration_ms = 600;
    dot.tick_interval_ms = Some(120);
    dot.damage_bps = Some(50000);
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), own, "wound", 0);
    let applied = guard(&mut w, &c, 0);
    let [Event::StatusApplied { status }] = own_events(&applied)[..] else {
        panic!("missing immunity")
    };
    let instance = status.instance_id.clone();
    let refreshed = guard(&mut w, &c, 30);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == instance && status.expires_at_ms == 330)
    );
    w.rng = 1;
    for (time, tick, damage) in [(120, 1, 0), (240, 2, 0), (360, 3, 66)] {
        let events = pulse(&mut w, &c, time);
        assert!(own_events(&events).iter().any(|e| matches!(e,
            Event::PeriodicTick {tick_index,hp_damage,absorbed,..}
                if *tick_index == tick && *hp_damage == damage && *absorbed == 0)));
    }
    assert_eq!((w.members[0].hp, w.rng), (934, 1));
    // Delayed ticks use execution time, after the immunity has expired.
    let (_, mut delayed) = setup();
    let target = delayed.members[0].reference.entity.clone();
    bind(&mut delayed, &c, Owner::Npc(0), target, "wound", 0);
    guard(&mut delayed, &c, 0);
    let events = pulse(&mut delayed, &c, 330);
    assert_eq!(delayed.members[0].hp, 868);
    assert_eq!(
        own_events(&events)
            .iter()
            .filter(|e| matches!(
                e,
                Event::PeriodicTick {
                    hp_damage: 66,
                    executed_at_ms: 330,
                    ..
                }
            ))
            .count(),
        2
    );

    assert!(!w.member_damage_immune(0, &c, 330));
    assert!(
        w.snapshot(1, &c, Phase::Active, 360)
            .own
            .statuses
            .is_empty()
    );
    guard(&mut w, &c, 400);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert!(!w.member_damage_immune(0, &c, 400));
    assert!(w.statuses.entries().is_empty());
}
