use super::combat::{DirectContact, dodge_probability};
use super::control_immunity_tests::own_events;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_accuracy_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-accuracy-host/arena-a.json"
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
    old["mechanism_version"] = json!(97);
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
        invalid["skill"][field] = json!(["accuracy-focus"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["accuracy-target"]["combat"]["projectile"][field] =
            json!(["accuracy-focus"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] =
        json!({(id.as_str()):{"heal_hp":10,"on_use_self_statuses":["accuracy-focus"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({id:{"cleanse_self_statuses":["accuracy-focus"]}});
    assert!(load(&item).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, w) = super::critical_chance_tests::setup();
    c.mechanism_version = 98;
    let pve = c.pve_mut();
    for (id, modifier) in [
        ("accuracy-focus", 4000),
        ("accuracy-extra", 7000),
        ("accuracy-full", 10000),
    ] {
        pve.status_definitions.push(serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"accuracy","id":id,
            "duration_ms":300,"modifier_bps":modifier,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    pve.status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"dodge_penalty","id":"dodge-full",
            "duration_ms":300,"modifier_bps":10000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    (c, w)
}

fn buff(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now)
}

fn dodge(w: &mut World, percent: u32) {
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("dodge".into(), percent);
}

fn contact(
    w: &mut World,
    c: &Config,
    member: usize,
    now: u64,
    statuses: &[String],
) -> (bool, Events) {
    let source = w.members[member].reference.entity.clone();
    let mut events = vec![];
    let hit = w.direct_contact(
        Owner::Member(member),
        Owner::Npc(0),
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
fn member_accuracy_matches_original_dodge_half_and_formula_order() {
    // Original dodge-half vector: 60% raw dodge and 100 hit points yield 30%.
    assert_eq!(dodge_probability(6000, 0, 10000), 0.3);
    assert_eq!(dodge_probability(6000, 2000, 10000), 0.2);
    assert_eq!(dodge_probability(10000, 0, 7500), 0.5);
    assert_eq!(dodge_probability(6000, 6001, 10000), 0.0);
    assert_eq!(dodge_probability(3000, 0, 1), 3000.0 / 10001.0);
    assert_eq!(dodge_probability(u64::from(u32::MAX) * 100, 0, 10000), 0.5);
}

#[test]
fn member_accuracy_stacks_with_a_cap_refreshes_and_reads_exact_expiry_in_flight() {
    let (mut c, mut w) = setup();
    c.basic.windup_ms = 30;
    c.basic.range = 85.;
    dodge(&mut w, 60);
    let applied = buff(&mut w, &c, "accuracy-focus", 0);
    let [Event::StatusApplied { status }] = own_events(&applied)[..] else {
        panic!("missing accuracy buff");
    };
    let id = status.instance_id.clone();
    let refreshed = buff(&mut w, &c, "accuracy-focus", 30);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == id && status.expires_at_ms == 330)
    );
    buff(&mut w, &c, "accuracy-extra", 30);
    w.rng = 4608;
    assert!(!contact(&mut w, &c, 0, 60, &[]).0); // 29% draw, still dodged at the 30% cap.
    w.rng = 6488889;
    assert!(contact(&mut w, &c, 0, 60, &[]).0);
    w.rng = 6488889;
    assert!(!contact(&mut w, &c, 1, 60, &[]).0);
    assert!(w.snapshot(1, &c, Phase::Active, 60).own.statuses.is_empty());
    w.action(0, &Action::Attack {}, "basic", &c, 270).unwrap();
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 300, &mut vec![]);
    assert_eq!(w.shots.len(), 1);
    w.rng = 6488889;
    let hp = w.npcs[0].hp;
    w.advance_shots(&c, 330, 1., &mut vec![]);
    assert_eq!(w.npcs[0].hp, hp);
    assert_eq!(w.rng, 1619424347);
}

#[test]
fn member_accuracy_preserves_contact_effects_random_draws_and_legacy() {
    let (mut c, mut w) = setup();
    dodge(&mut w, 60);
    buff(&mut w, &c, "accuracy-full", 0);
    w.members[0].spirit -= 20;
    let spirit = w.members[0].spirit;
    let hp = w.npcs[0].hp;
    let statuses = ["wound".into()];

    c.mechanism_version = 97;
    w.rng = 6488889;
    let (hit, events) = contact(&mut w, &c, 0, 30, &statuses);
    assert!(!hit && events.is_empty());
    assert_eq!(w.rng, 1619424347);
    assert_eq!((w.members[0].spirit, w.npcs[0].hp), (spirit, hp));
    assert_eq!(w.statuses.entries().len(), 1);

    c.mechanism_version = 98;
    w.rng = 6488889;
    let (hit, events) = contact(&mut w, &c, 0, 30, &statuses);
    assert!(hit);
    assert_eq!(w.rng, 1619424347);
    assert_eq!(w.npcs[0].hp, hp - 66);
    assert_eq!(w.members[0].spirit, spirit + c.params.basic_spirit_on_hit);
    assert!(events.iter().any(|(_, event)| matches!(event,
        EventV4::Private {event: Event::StatusApplied {status}} if status.definition_id == "wound")));

    buff(&mut w, &c, "dodge-full", 60);
    w.rng = 1;
    assert!(contact(&mut w, &c, 0, 90, &[]).0);
    assert_eq!(w.rng, 1);
}
