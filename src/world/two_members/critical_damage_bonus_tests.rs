use super::combat::DamageIntent;
use super::control_immunity_tests::own_events;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_critical_damage_bonus_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-critical-damage-bonus-host/arena-a.json"
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
    old["mechanism_version"] = json!(94);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(-1)),
        ("modifier_bps", json!(0)),
        ("modifier_bps", json!(15001)),
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
        invalid["skill"][field] = json!(["critical-damage-focus"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["armor-target"]["combat"]["projectile"][field] =
            json!(["critical-damage-focus"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] =
        json!({(id.as_str()):{"heal_hp":10,"on_use_self_statuses":["critical-damage-focus"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({id:{"cleanse_self_statuses":["critical-damage-focus"]}});
    assert!(load(&item).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, w) = super::critical_chance_tests::setup();
    c.mechanism_version = 95;
    let pve = c.pve_mut();
    pve.status_definitions
        .iter_mut()
        .find(|d| d.id == "critical-focus")
        .unwrap()
        .duration_ms = 1200;
    for (id, duration, modifier) in [("bonus-a", 300, 7500), ("bonus-b", 210, 10000)] {
        pve.status_definitions.push(serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"critical_damage_bonus","id":id,
            "duration_ms":duration,"modifier_bps":modifier,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    (c, w)
}

fn buff(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now)
}

fn hit(w: &World) -> DamageIntent {
    DamageIntent {
        ignore_armor: false,
        source: w.members[0].reference.entity.clone(),
        target: w.npcs[0].entity.clone(),
        raw: 100,
        direct: true,
    }
}

#[test]
fn member_critical_damage_bonus_applies_only_to_critical_damage_before_shields() {
    let (c, mut w) = setup();
    let rng = w.rng;
    let target = w.npcs[0].entity.clone();
    buff(&mut w, &c, "bonus-a", 0);
    assert_eq!(w.resolve_damage(hit(&w), &c, 30, &mut vec![]).hp_damage, 66);
    buff(&mut w, &c, "critical-focus", 30);
    assert_eq!(
        w.resolve_damage(hit(&w), &c, 60, &mut vec![]).hp_damage,
        150
    );
    buff(&mut w, &c, "armor-focus", 90);
    bind(&mut w, &c, Owner::Npc(0), target, "barrier", 90);
    let damage = w.resolve_damage(hit(&w), &c, 120, &mut vec![]);
    assert_eq!((damage.absorbed, damage.hp_damage), (10, 170));
    assert_eq!(w.rng, rng);
}

#[test]
fn member_critical_damage_bonus_caps_aggregate_and_keeps_owner_and_deadline() {
    let (c, mut w) = setup();
    buff(&mut w, &c, "critical-focus", 0);
    let applied = buff(&mut w, &c, "bonus-a", 0);
    let [Event::StatusApplied { status }] = own_events(&applied)[..] else {
        panic!("missing critical damage bonus");
    };
    let id = status.instance_id.clone();
    buff(&mut w, &c, "bonus-b", 30);
    assert_eq!(
        w.resolve_damage(hit(&w), &c, 60, &mut vec![]).hp_damage,
        200
    );
    let refreshed = buff(&mut w, &c, "bonus-a", 90);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == id && status.expires_at_ms == 390)
    );
    let peer = DamageIntent {
        ignore_armor: false,
        source: w.members[1].reference.entity.clone(),
        ..hit(&w)
    };
    assert_eq!(w.resolve_damage(peer, &c, 90, &mut vec![]).hp_damage, 66);
    assert!(w.snapshot(1, &c, Phase::Active, 90).own.statuses.is_empty());
    assert_eq!(
        w.resolve_damage(hit(&w), &c, 239, &mut vec![]).hp_damage,
        200
    );
    assert_eq!(
        w.resolve_damage(hit(&w), &c, 240, &mut vec![]).hp_damage,
        150
    );
    assert_eq!(
        w.resolve_damage(hit(&w), &c, 390, &mut vec![]).hp_damage,
        100
    );
}

#[test]
fn member_critical_damage_bonus_samples_at_impact_without_extra_random_draws() {
    let (mut c, mut w) = setup();
    c.basic.windup_ms = 30;
    c.basic.range = 85.;
    buff(&mut w, &c, "critical-half", 0);
    buff(&mut w, &c, "bonus-a", 30);
    w.rng = 9999;
    assert_eq!(w.resolve_damage(hit(&w), &c, 60, &mut vec![]).hp_damage, 66);
    assert_eq!(w.rng, 2564385439);
    w.rng = 1;
    assert_eq!(
        w.resolve_damage(hit(&w), &c, 90, &mut vec![]).hp_damage,
        150
    );
    assert_eq!(w.rng, 270369);
    buff(&mut w, &c, "critical-focus", 90);
    let periodic = DamageIntent {
        ignore_armor: false,
        direct: false,
        ..hit(&w)
    };
    assert_eq!(
        w.resolve_damage(periodic, &c, 120, &mut vec![]).hp_damage,
        66
    );
    let DamageIntent { source, target, .. } = hit(&w);
    let inbound = DamageIntent {
        ignore_armor: false,
        source: target,
        target: source,
        raw: 100,
        direct: true,
    };
    assert_eq!(
        w.resolve_damage(inbound, &c, 120, &mut vec![]).hp_damage,
        66
    );
    w.action(0, &Action::Attack {}, "basic", &c, 270).unwrap();
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 300, &mut vec![]);
    assert_eq!(w.shots.len(), 1);
    let hp = w.npcs[0].hp;
    w.advance_shots(&c, 330, 1., &mut vec![]);
    assert_eq!(hp - w.npcs[0].hp, 100);
    assert_eq!(w.rng, 270369);
}
