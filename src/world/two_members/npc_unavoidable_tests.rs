use super::control_immunity_tests::own_events;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_npc_unavoidable_validates_base_and_template_boolean_capability() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-npc-unavoidable-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    for pointer in [
        "/pve/combat/projectile",
        "/pve/additional_templates/unavoidable-probe/combat/projectile",
    ] {
        for value in [json!(true), json!(false)] {
            let mut valid = doc.clone();
            valid.pointer_mut(pointer).unwrap()["unavoidable"] = value.clone();
            let c = load(&valid).unwrap().default_config();
            assert_eq!(
                serde_json::to_value(c).unwrap().pointer(pointer).unwrap()["unavoidable"],
                value
            );
            valid["mechanism_version"] = json!(100);
            assert!(load(&valid).is_err());
        }
        for value in [Value::Null, json!(0), json!(1), json!("true")] {
            let mut invalid = doc.clone();
            invalid.pointer_mut(pointer).unwrap()["unavoidable"] = value;
            assert!(load(&invalid).is_err());
        }
    }
    let mut omitted = doc.clone();
    omitted["pve"]["additional_templates"]["unavoidable-probe"]["combat"]["projectile"]
        .as_object_mut()
        .unwrap()
        .remove("unavoidable");
    omitted["mechanism_version"] = json!(100);
    assert!(load(&omitted).is_ok());
    let mut player = doc;
    player["skill"]["unavoidable"] = json!(true);
    assert!(load(&player).is_err());
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, mut w) = super::armor_penetration_tests::setup();
    c.mechanism_version = 101;
    c.params.metal_multiplier = 1.;
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-npc-unavoidable-host/arena-a.json"
    ))
    .unwrap();
    for id in ["dodge-focus", "damage-guard"] {
        let mut d = doc["pve"]["status_definitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == id)
            .unwrap()
            .clone();
        d["duration_ms"] = json!(300);
        c.pve_mut()
            .status_definitions
            .push(serde_json::from_value(d).unwrap());
    }
    w.npcs.truncate(1);
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 1000;
    let p = Arc::make_mut(&mut w.npcs[0].profile);
    p.combat.projectile.unavoidable = Some(true);
    p.combat.projectile.on_hit_target_statuses = vec!["wound".into()];
    p.stats.insert("atk".into(), 100);
    (c, w)
}

fn status(w: &mut World, c: &Config, id: &str, now: u64) {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now);
}

pub(super) fn release(w: &mut World, c: &Config, now: u64) {
    w.npcs[0].casting = Some(super::npc_skills::NpcCast::legacy(now, [-1., 0.]));
    w.advance_npcs(c, now, 0.03, &mut vec![]);
    assert_eq!(w.shots.len(), 1);
}

pub(super) fn impact(w: &mut World, c: &Config, now: u64) -> Events {
    let mut events = vec![];
    w.advance_shots(c, now, 1., &mut events);
    events
}

#[test]
fn member_npc_unavoidable_freezes_at_release_and_preserves_cost_shield_and_hit_effects() {
    let (c, mut w) = setup();
    status(&mut w, &c, "dodge-focus", 0);
    status(&mut w, &c, "ward", 0);
    let before = w.snapshot(0, &c, Phase::Active, 0).own;
    let spirit = w.npcs[0].spirit;
    let s = w.npcs[0].profile.combat.projectile.clone();
    w.rng = 1;
    release(&mut w, &c, 30);
    assert!(w.shots[0].unavoidable);
    assert_eq!(
        (w.npcs[0].spirit, w.npcs[0].next_cast, w.npcs[0].recovery),
        (spirit - s.cost, 30 + s.cooldown_ms, 30 + s.recovery_ms)
    );
    Arc::make_mut(&mut w.npcs[0].profile)
        .combat
        .projectile
        .unavoidable = Some(false);
    let events = impact(&mut w, &c, 60);
    assert_eq!(w.rng, 1);
    assert!(w.shots.is_empty());
    let after = w.snapshot(0, &c, Phase::Active, 60).own;
    assert_eq!(
        (after.hp, after.shield_total),
        (before.hp, Some(before.shield_total.unwrap() - 66))
    );
    assert!(own_events(&events).iter().any(|e| matches!(e,
        Event::StatusApplied {status} if status.definition_id=="wound")));
    assert_eq!(w.npcs[0].spirit, spirit - s.cost);
}

#[test]
fn member_npc_unavoidable_respects_immunity_target_protection_wall_and_source_validity() {
    for condition in 0..4 {
        let (c, mut w) = setup();
        status(&mut w, &c, "dodge-focus", 0);
        status(&mut w, &c, "ward", 0);
        release(&mut w, &c, 30);
        match condition {
            0 => status(&mut w, &c, "damage-guard", 30),
            1 => w.members[0].protected = true,
            2 => Arc::make_mut(&mut w.grid.config).palette[0].blocks_projectiles = true,
            _ => w.npcs[0].hp = 0,
        }
        let before = w.snapshot(0, &c, Phase::Active, 30).own;
        w.rng = 1;
        let events = impact(&mut w, &c, 60);
        assert_eq!(w.rng, 1);
        assert!(w.shots.is_empty());
        let after = w.snapshot(0, &c, Phase::Active, 60).own;
        assert_eq!(
            (after.hp, after.shield_total, after.statuses),
            (before.hp, before.shield_total, before.statuses)
        );
        assert!(
            events
                .iter()
                .all(|(_, e)| !matches!(e, EventV4::Damage { .. }))
        );
    }
}

#[test]
fn member_npc_unavoidable_false_omission_and_legacy_use_original_dodge_rng() {
    for (version, policy) in [(101, None), (101, Some(false)), (100, Some(true))] {
        let (mut c, mut w) = setup();
        c.mechanism_version = version;
        Arc::make_mut(&mut w.npcs[0].profile)
            .combat
            .projectile
            .unavoidable = policy;
        status(&mut w, &c, "dodge-focus", 0);
        release(&mut w, &c, 30);
        assert!(!w.shots[0].unavoidable);
        let spirit = w.npcs[0].spirit;
        w.rng = 1;
        let events = impact(&mut w, &c, 60);
        assert_eq!(w.rng, 270369);
        assert_eq!((w.members[0].hp, w.npcs[0].spirit), (1000, spirit));
        assert!(w.shots.is_empty());
        assert!(
            events
                .iter()
                .all(|(_, e)| !matches!(e, EventV4::Damage { .. }))
        );
    }
}
