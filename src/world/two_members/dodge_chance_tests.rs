use super::combat::{DamageIntent, DirectContact};
use super::control_immunity_tests::own_events;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_dodge_chance_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-dodge-chance-host/arena-a.json"
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
    old["mechanism_version"] = json!(96);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(-1)),
        ("modifier_bps", json!(0)),
        ("modifier_bps", json!(5001)),
        ("duplicate_policy", json!("ignore")),
        ("source_scoped", json!(false)),
        ("mechanism", json!("timed_control")),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][def][field] = value;
        assert!(load(&invalid).is_err());
    }
    for field in ["on_commit_self_statuses", "on_hit_target_statuses"] {
        let mut invalid = doc.clone();
        invalid["skill"][field] = json!(["dodge-focus"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["evasion-probe"]["combat"]["projectile"][field] =
            json!(["dodge-focus"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] =
        json!({(id.as_str()):{"heal_hp":10,"on_use_self_statuses":["dodge-focus"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({id:{"cleanse_self_statuses":["dodge-focus"]}});
    assert!(load(&item).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::armor_penetration_tests::setup();
    c.mechanism_version = 97;
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 1000;
    let pve = c.pve_mut();
    for (id, modifier) in [("dodge-focus", 2500), ("dodge-extra", 4000)] {
        pve.status_definitions.push(serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"dodge_chance","id":id,
            "duration_ms":300,"modifier_bps":modifier,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    (c, w)
}

fn buff(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now)
}

fn contact(
    w: &mut World,
    c: &Config,
    member: usize,
    now: u64,
    statuses: &[String],
) -> (bool, Events) {
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
            hit_statuses: statuses,
        },
        c,
        now,
        &mut events,
    );
    (hit, events)
}

#[test]
fn member_dodge_chance_projectile_preserves_shield_and_skips_attached_effects_on_dodge() {
    let (c, mut w) = setup();
    let own = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), own, "ward", 0);
    buff(&mut w, &c, "dodge-focus", 0);
    let before = w.snapshot(0, &c, Phase::Active, 0).own;
    let statuses = ["wound".into()];
    w.rng = 1;
    let (hit, events) = contact(&mut w, &c, 0, 30, &statuses);
    assert!(!hit && events.is_empty());
    assert_eq!(w.rng, 270369);
    let after = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        (after.hp, after.shield_total, after.statuses),
        (before.hp, before.shield_total, before.statuses)
    );
    let rng = w.rng;
    let (hit, events) = contact(&mut w, &c, 0, 300, &statuses);
    assert!(hit);
    assert_eq!(w.rng, rng);
    let after = w.snapshot(0, &c, Phase::Active, 300).own;
    assert_eq!(
        after.shield_total.unwrap(),
        before.shield_total.unwrap() - 66
    );
    assert!(own_events(&events).iter().any(|event| matches!(event,
        Event::StatusApplied {status} if status.definition_id == "wound")));
}

#[test]
fn member_dodge_chance_caps_refreshes_and_respects_target_eligibility() {
    let (c, mut w) = setup();
    let events = buff(&mut w, &c, "dodge-focus", 0);
    let [Event::StatusApplied { status }] = own_events(&events)[..] else {
        panic!("missing dodge chance");
    };
    let id = status.instance_id.clone();
    let refreshed = buff(&mut w, &c, "dodge-focus", 30);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == id && status.expires_at_ms == 330)
    );
    buff(&mut w, &c, "dodge-extra", 30);
    w.rng = 9999; // 59.7 percent, above the 50-percent cap but below the raw sum.
    assert!(contact(&mut w, &c, 0, 60, &[]).0);
    w.rng = 1;
    w.members[0].protected = true;
    assert!(!contact(&mut w, &c, 0, 90, &[]).0);
    assert_eq!(w.rng, 1);
    assert!(contact(&mut w, &c, 1, 90, &[]).0);
    assert_eq!(w.rng, 1);
    assert!(w.snapshot(1, &c, Phase::Active, 90).own.statuses.is_empty());
    w.members[0].protected = false;
    assert!(!contact(&mut w, &c, 0, 329, &[]).0);
    assert!(contact(&mut w, &c, 0, 330, &[]).0);
}

#[test]
fn member_dodge_chance_melee_keeps_interval_and_skips_critical_draw_on_miss() {
    let (mut c, mut w) = setup();
    w.npcs.truncate(1);
    let profile = Arc::make_mut(&mut w.npcs[0].profile);
    profile.combat.aggro_range = 100.;
    profile.combat.melee_range = 80.;
    profile.combat.cast_chance = 0.;
    profile.stats.insert("crit".into(), 50);
    let interval = profile.combat.attack_interval_ms;
    w.npcs[0].next_attack = 0;
    buff(&mut w, &c, "dodge-focus", 0);
    w.rng = 1;
    w.advance_npcs(&c, 30, 0.03, &mut vec![]);
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.rng, 270369);
    assert_eq!(w.npcs[0].next_attack, 30 + interval);
    w.advance_npcs(&c, 60, 0.03, &mut vec![]);
    assert_eq!(w.rng, 270369);
    let periodic = DamageIntent {
        ignore_armor: false,
        source: w.npcs[0].entity.clone(),
        target: w.members[0].reference.entity.clone(),
        raw: 100,
        direct: false,
    };
    assert_eq!(
        w.resolve_damage(periodic, &c, 90, &mut vec![]).hp_damage,
        66
    );
    assert_eq!(w.rng, 270369);
    c.mechanism_version = 96;
    w.rng = 1;
    assert!(contact(&mut w, &c, 0, 120, &[]).0);
    assert_eq!(w.rng, 1);
}
