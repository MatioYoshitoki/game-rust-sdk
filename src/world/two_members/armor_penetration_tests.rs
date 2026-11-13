use super::combat::DamageIntent;
use super::control_immunity_tests::own_events;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_armor_penetration_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-armor-penetration-host/arena-a.json"
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
    old["mechanism_version"] = json!(92);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(-1)),
        ("modifier_bps", json!(0)),
        ("modifier_bps", json!(10001)),
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
        invalid["skill"][field] = json!(["armor-focus"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["armor-target"]["combat"]["projectile"][field] =
            json!(["armor-focus"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] =
        json!({(id.as_str()):{"heal_hp":10,"on_use_self_statuses":["armor-focus"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({id:{"cleanse_self_statuses":["armor-focus"]}});
    assert!(load(&item).is_err());
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, mut w) = super::control_immunity_tests::setup();
    c.mechanism_version = 93;
    c.defender_realm_atk = 100;
    c.defense_cap = 0.7;
    c.params.defense_scale = 2.;
    c.params.minimum_damage = 1;
    let pve = c.pve_mut();
    pve.status_rules.as_mut().unwrap().max_active_per_entity = 8;
    for (id, modifier) in [("armor-focus", 5000), ("armor-extra", 7000)] {
        pve.status_definitions.push(serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"armor_penetration","id":id,
            "duration_ms":300,"modifier_bps":modifier,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    pve.status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_defense_modifier","id":"def-boost",
            "duration_ms":600,"modifier_bps":5000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    w.members[0].facts.def_points = 100;
    w.members[0].facts.atk_points = 100;
    w.npcs[0].position = [460., 400.];
    let profile = Arc::make_mut(&mut w.npcs[0].profile);
    profile.stats.insert("def".into(), 100);
    profile.stats.insert("hp".into(), 1000);
    w.npcs[0].hp = 1000;
    (c, w)
}

fn buff(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now)
}

#[test]
fn member_armor_penetration_matches_the_defense_examples_and_preserves_other_attackers() {
    let (c, mut w) = setup();
    let source = w.members[0].reference.entity.clone();
    let target = w.npcs[0].entity.clone();
    let attack = || DamageIntent {
        ignore_armor: false,
        source: source.clone(),
        target: target.clone(),
        raw: 100,
        direct: true,
    };
    // defense-basic and defense-pierce in the handoff's numeric vectors.
    assert_eq!(w.resolve_damage(attack(), &c, 0, &mut vec![]).hp_damage, 66);
    let events = buff(&mut w, &c, "armor-focus", 30);
    let [Event::StatusApplied { status }] = own_events(&events)[..] else {
        panic!("missing armor penetration");
    };
    let id = status.instance_id.clone();
    assert_eq!(
        w.resolve_damage(attack(), &c, 60, &mut vec![]).hp_damage,
        80
    );
    let peer = w.members[1].reference.entity.clone();
    let other = DamageIntent {
        ignore_armor: false,
        source: peer,
        ..attack()
    };
    assert_eq!(w.resolve_damage(other, &c, 60, &mut vec![]).hp_damage, 66);
    assert!(w.snapshot(1, &c, Phase::Active, 60).own.statuses.is_empty());
    assert_eq!(w.npcs[0].profile.stats["def"], 100);
    let events = buff(&mut w, &c, "armor-focus", 90);
    assert!(
        matches!(own_events(&events).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == id && status.expires_at_ms == 390)
    );
    buff(&mut w, &c, "armor-extra", 90);
    assert_eq!(
        w.resolve_damage(attack(), &c, 120, &mut vec![]).hp_damage,
        100
    );
}

#[test]
fn member_armor_penetration_uses_effective_def_before_shield_and_preserves_fractional_armor() {
    let (c, mut w) = setup();
    let source = w.members[0].reference.entity.clone();
    let target = w.npcs[0].entity.clone();
    for id in ["def-boost", "barrier"] {
        bind(&mut w, &c, Owner::Npc(0), target.clone(), id, 0);
    }
    buff(&mut w, &c, "armor-focus", 0);
    let attack = || DamageIntent {
        ignore_armor: false,
        source: source.clone(),
        target: target.clone(),
        raw: 100,
        direct: true,
    };
    let damage = w.resolve_damage(attack(), &c, 30, &mut vec![]);
    assert_eq!((damage.absorbed, damage.hp_damage), (10, 62));
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("def".into(), 1);
    let damage = w.resolve_damage(
        DamageIntent {
            ignore_armor: false,
            raw: 30,
            ..attack()
        },
        &c,
        60,
        &mut vec![],
    );
    assert_eq!(damage.hp_damage, 29);
}

#[test]
fn member_armor_penetration_samples_on_impact_and_keeps_periodic_and_npc_damage() {
    let (mut c, mut w) = setup();
    c.basic.windup_ms = 30;
    c.basic.range = 85.;
    let source = w.members[0].reference.entity.clone();
    let target = w.npcs[0].entity.clone();
    buff(&mut w, &c, "armor-focus", 0);
    w.action(0, &Action::Attack {}, "basic", &c, 210).unwrap();
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 240, &mut vec![]);
    assert_eq!(w.shots.len(), 1);
    assert!(w.member_has_status(0, StatusAttribute::ArmorPenetration, 299));
    let hp = w.npcs[0].hp;
    w.advance_shots(&c, 300, 1., &mut vec![]);
    assert_eq!(hp - w.npcs[0].hp, 66);
    buff(&mut w, &c, "armor-focus", 330);
    let periodic = DamageIntent {
        ignore_armor: false,
        source: source.clone(),
        target: target.clone(),
        raw: 100,
        direct: false,
    };
    assert_eq!(
        w.resolve_damage(periodic, &c, 360, &mut vec![]).hp_damage,
        66
    );
    let inbound = DamageIntent {
        ignore_armor: false,
        source: target,
        target: source,
        raw: 100,
        direct: true,
    };
    assert_eq!(
        w.resolve_damage(inbound, &c, 360, &mut vec![]).hp_damage,
        66
    );
}
