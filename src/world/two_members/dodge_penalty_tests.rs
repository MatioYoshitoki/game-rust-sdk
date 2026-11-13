use super::combat::DirectContact;
use super::control_immunity_tests::own_events;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_dodge_penalty_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-dodge-penalty-host/arena-a.json"
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
    old["mechanism_version"] = json!(95);
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
        invalid["skill"][field] = json!(["dodge-focus"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["dodge-target"]["combat"]["projectile"][field] =
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
    let (mut c, w) = super::critical_chance_tests::setup();
    c.mechanism_version = 96;
    let pve = c.pve_mut();
    for (id, modifier) in [
        ("dodge-focus", 4000),
        ("dodge-extra", 3000),
        ("dodge-full", 10000),
    ] {
        pve.status_definitions.push(serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"dodge_penalty","id":id,
            "duration_ms":300,"modifier_bps":modifier,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
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
fn member_dodge_penalty_reproduces_penalty_and_cap_and_preserves_legacy() {
    let (mut c, mut w) = setup();
    buff(&mut w, &c, "dodge-focus", 0);
    dodge(&mut w, 50);
    // dodge-penalty: 50 - 40 = 10 percent.
    w.rng = 1234;
    assert!(!contact(&mut w, &c, 0, 30, &[]).0);
    w.rng = 2500;
    assert!(contact(&mut w, &c, 0, 30, &[]).0);
    dodge(&mut w, 60);
    w.rng = 2500;
    assert!(!contact(&mut w, &c, 0, 30, &[]).0); // 60 - 40 = 20 percent.
    // dodge-cap: the result at 200 percent is capped at 50 percent.
    dodge(&mut w, 200);
    w.rng = 9999;
    assert!(contact(&mut w, &c, 0, 300, &[]).0);
    assert_eq!(w.rng, 2564385439);
    c.mechanism_version = 95;
    w.rng = 9999;
    assert!(!contact(&mut w, &c, 0, 300, &[]).0);
    assert_eq!(w.rng, 9999);
}

#[test]
fn member_dodge_penalty_stacks_to_zero_and_samples_exact_expiry_on_impact() {
    let (mut c, mut w) = setup();
    c.basic.windup_ms = 30;
    c.basic.range = 85.;
    dodge(&mut w, 60);
    let applied = buff(&mut w, &c, "dodge-focus", 0);
    let [Event::StatusApplied { status }] = own_events(&applied)[..] else {
        panic!("missing dodge penalty");
    };
    let id = status.instance_id.clone();
    let refreshed = buff(&mut w, &c, "dodge-focus", 30);
    assert!(
        matches!(own_events(&refreshed).as_slice(), [Event::StatusRefreshed {status}]
        if status.instance_id == id && status.expires_at_ms == 330)
    );
    buff(&mut w, &c, "dodge-extra", 30);
    w.rng = 1;
    assert!(contact(&mut w, &c, 0, 60, &[]).0);
    assert_eq!(w.rng, 1);
    assert!(!contact(&mut w, &c, 1, 60, &[]).0);
    assert!(w.snapshot(1, &c, Phase::Active, 60).own.statuses.is_empty());
    w.action(0, &Action::Attack {}, "basic", &c, 270).unwrap();
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, &c, 300, &mut vec![]);
    assert_eq!(w.shots.len(), 1);
    w.rng = 1;
    let hp = w.npcs[0].hp;
    w.advance_shots(&c, 330, 1., &mut vec![]);
    assert_eq!(w.npcs[0].hp, hp);
    assert_eq!(w.rng, 270369);
}

#[test]
fn member_dodge_penalty_preserves_critical_rng_damage_spirit_and_status_order() {
    let (c, mut w) = setup();
    dodge(&mut w, 60);
    buff(&mut w, &c, "critical-half", 0);
    w.members[0].spirit -= 10;
    let spirit = w.members[0].spirit;
    let hp = w.npcs[0].hp;
    let statuses = ["wound".into()];
    w.rng = 1;
    let (hit, events) = contact(&mut w, &c, 0, 30, &statuses);
    assert!(!hit && events.is_empty());
    assert_eq!(w.rng, 270369);
    assert_eq!((w.members[0].spirit, w.npcs[0].hp), (spirit, hp));
    assert_eq!(w.statuses.entries().len(), 1);
    buff(&mut w, &c, "dodge-full", 60);
    w.rng = 1;
    let (hit, events) = contact(&mut w, &c, 0, 90, &statuses);
    assert!(hit);
    assert_eq!(w.rng, 270369);
    assert_eq!(w.npcs[0].hp, hp - 100);
    assert_eq!(w.members[0].spirit, spirit + c.params.basic_spirit_on_hit);
    assert!(events.iter().any(|(_, event)| matches!(event,
        EventV4::Private {event: Event::StatusApplied {status}} if status.definition_id == "wound")));
}
