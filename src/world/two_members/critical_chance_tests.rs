use super::combat::{DamageIntent, DirectContact};
use super::control_immunity_tests::own_events;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_critical_chance_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-critical-chance-host/arena-a.json"
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
    old["mechanism_version"] = json!(93);
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
        invalid["skill"][field] = json!(["critical-focus"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["armor-target"]["combat"]["projectile"][field] =
            json!(["critical-focus"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] =
        json!({(id.as_str()):{"heal_hp":10,"on_use_self_statuses":["critical-focus"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({id:{"cleanse_self_statuses":["critical-focus"]}});
    assert!(load(&item).is_err());
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, w) = super::armor_penetration_tests::setup();
    c.mechanism_version = 94;
    c.params.critical_multiplier = 1.5;
    let pve = c.pve_mut();
    for (id, modifier) in [
        ("critical-focus", 10000),
        ("critical-half", 5000),
        ("critical-extra", 6000),
    ] {
        pve.status_definitions.push(serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"critical_chance","id":id,
            "duration_ms":300,"modifier_bps":modifier,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    (c, w)
}

fn buff(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now)
}

#[test]
fn member_critical_chance_multiplies_before_rounding_and_shield() {
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
    let rng = w.rng;
    assert_eq!(w.resolve_damage(attack(), &c, 0, &mut vec![]).hp_damage, 66);
    buff(&mut w, &c, "critical-focus", 30);
    // defense-crit in the handoff's numeric vectors.
    assert_eq!(
        w.resolve_damage(attack(), &c, 60, &mut vec![]).hp_damage,
        100
    );
    buff(&mut w, &c, "armor-focus", 90);
    bind(&mut w, &c, Owner::Npc(0), target.clone(), "barrier", 90);
    let damage = w.resolve_damage(attack(), &c, 120, &mut vec![]);
    assert_eq!((damage.absorbed, damage.hp_damage), (10, 110));
    assert_eq!(w.rng, rng);
}

#[test]
fn member_critical_chance_rolls_only_after_valid_contact_and_dodge() {
    let (c, mut w) = setup();
    buff(&mut w, &c, "critical-half", 0);
    let source = w.members[0].reference.entity.clone();
    let contact = || DirectContact {
        origin: None,
        action_id: None,
        rule: None,
        ignore_armor: false,
        unavoidable: false,
        dead_source: false,
        source: &source,
        raw: 100,
        metal: false,
        hit_statuses: &[],
    };
    w.rng = 1;
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("dodge".into(), 100);
    let hp = w.npcs[0].hp;
    assert!(!w.direct_contact(
        Owner::Member(0),
        Owner::Npc(0),
        contact(),
        &c,
        30,
        &mut vec![]
    ));
    assert_eq!(w.rng, 1);
    assert_eq!(w.npcs[0].hp, hp);
    let mut stale = source.clone();
    stale.generation += 1;
    assert!(!w.direct_contact(
        Owner::Member(0),
        Owner::Npc(0),
        DirectContact {
            origin: None,
            action_id: None,
            rule: None,
            ignore_armor: false,
            unavoidable: false,
            dead_source: false,
            source: &stale,
            ..contact()
        },
        &c,
        30,
        &mut vec![]
    ));
    assert_eq!(w.rng, 1);
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("dodge".into(), 0);
    for (seed, next, expected) in [(1, 270369, 100), (9999, 2564385439, 66)] {
        w.rng = seed;
        let hp = w.npcs[0].hp;
        assert!(w.direct_contact(
            Owner::Member(0),
            Owner::Npc(0),
            contact(),
            &c,
            60,
            &mut vec![]
        ));
        assert_eq!(w.rng, next);
        assert_eq!(hp - w.npcs[0].hp, expected);
    }
}

#[test]
fn member_critical_chance_caps_and_samples_at_impact_without_affecting_other_paths() {
    let (mut c, mut w) = setup();
    c.basic.windup_ms = 30;
    c.basic.range = 85.;
    let source = w.members[0].reference.entity.clone();
    let target = w.npcs[0].entity.clone();
    let events = buff(&mut w, &c, "critical-half", 0);
    let [Event::StatusApplied { status }] = own_events(&events)[..] else {
        panic!("missing critical chance status");
    };
    let id = status.instance_id.clone();
    let refreshed = buff(&mut w, &c, "critical-half", 30);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == id && status.expires_at_ms == 330)
    );
    buff(&mut w, &c, "critical-extra", 30);
    let rng = w.rng;
    let attack = || DamageIntent {
        ignore_armor: false,
        source: source.clone(),
        target: target.clone(),
        raw: 100,
        direct: true,
    };
    assert_eq!(
        w.resolve_damage(attack(), &c, 60, &mut vec![]).hp_damage,
        100
    );
    let other = DamageIntent {
        ignore_armor: false,
        source: w.members[1].reference.entity.clone(),
        ..attack()
    };
    assert_eq!(w.resolve_damage(other, &c, 60, &mut vec![]).hp_damage, 66);
    assert!(w.snapshot(1, &c, Phase::Active, 60).own.statuses.is_empty());
    let periodic = DamageIntent {
        ignore_armor: false,
        direct: false,
        ..attack()
    };
    assert_eq!(
        w.resolve_damage(periodic, &c, 60, &mut vec![]).hp_damage,
        66
    );
    let inbound = DamageIntent {
        ignore_armor: false,
        source: target.clone(),
        target: source.clone(),
        raw: 100,
        direct: true,
    };
    assert_eq!(w.resolve_damage(inbound, &c, 60, &mut vec![]).hp_damage, 66);
    w.action(0, &Action::Attack {}, "basic", &c, 240).unwrap();
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 270, &mut vec![]);
    assert_eq!(w.shots.len(), 1);
    let hp = w.npcs[0].hp;
    w.advance_shots(&c, 330, 1., &mut vec![]);
    assert_eq!(hp - w.npcs[0].hp, 66);
    assert_eq!(w.rng, rng);
}
