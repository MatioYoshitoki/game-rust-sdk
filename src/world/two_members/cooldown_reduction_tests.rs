use super::control_immunity_tests::own_events;
use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_cooldown_reduction_validates_own_self_skill_policy() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-cooldown-reduction-host/arena-a.json"
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
    old["mechanism_version"] = json!(91);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("modifier_bps", json!(-1)),
        ("modifier_bps", json!(0)),
        ("modifier_bps", json!(4001)),
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
        invalid["skill"][field] = json!(["cooldown-focus"]);
        assert!(load(&invalid).is_err());
        let mut invalid = doc.clone();
        invalid["pve"]["additional_templates"]["orb-target"]["combat"]["projectile"][field] =
            json!(["cooldown-focus"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] =
        json!({(id.as_str()):{"heal_hp":10,"on_use_self_statuses":["cooldown-focus"]}});
    assert!(load(&item).is_err());
    item["member_item_use"] = json!({id:{"cleanse_self_statuses":["cooldown-focus"]}});
    assert!(load(&item).is_err());
}

fn setup() -> (Config, World) {
    let (mut c, w) = super::control_immunity_tests::setup();
    c.mechanism_version = 92;
    c.skill.cooldown_ms = 601;
    c.additional_skills.push(
        serde_json::from_value(json!({
            "mechanism":"self_status","id":"focus-cooldown","cost":2,"windup_ms":30,
            "recovery_ms":0,"cooldown_ms":2000,"on_commit_self_statuses":["cooldown-focus"]
        }))
        .unwrap(),
    );
    let pve = c.pve_mut();
    pve.status_rules.as_mut().unwrap().max_active_per_entity = 8;
    for id in ["cooldown-focus", "cooldown-extra"] {
        pve.status_definitions.push(serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","attribute":"cooldown_reduction","id":id,
            "duration_ms":300,"modifier_bps":2500,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    (c, w)
}

fn buff(w: &mut World, c: &Config, id: &str, now: u64) {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now);
}

fn cast() -> Action {
    Action::Cast {
        skill_id: "cast".into(),
        direction: [1., 0.],
    }
}

fn commit(w: &mut World, c: &Config) -> Events {
    let pending = w.members[0].pending.take().unwrap();
    let now = pending.commits;
    let mut events = vec![];
    w.commit(0, pending, c, now, &mut events);
    events
}

#[test]
fn member_cooldown_reduction_freezes_the_deadline_and_does_not_discount_its_own_first_grant() {
    let (c, mut w) = setup();
    w.action(
        0,
        &Action::CastSelf {
            skill_id: "focus-cooldown".into(),
        },
        "focus",
        &c,
        0,
    )
    .unwrap();
    let events = commit(&mut w, &c);
    assert!(matches!(own_events(&events).as_slice(), [
        Event::StatusApplied {status}, Event::ActionCommitted {..}
    ] if status.modifier_bps == 2500 && status.expires_at_ms == 330));
    assert_eq!(w.members[0].cooldowns["focus-cooldown"], 2030);
    assert!(w.snapshot(1, &c, Phase::Active, 30).own.statuses.is_empty());
    w.action(0, &cast(), "cast", &c, 30).unwrap();
    commit(&mut w, &c);
    let deadline = 90 + 450;
    assert_eq!(w.members[0].cooldowns["cast"], deadline);
    assert_eq!(w.members[0].recovery_until, 90 + c.skill.recovery_ms);
    buff(&mut w, &c, "cooldown-focus", 120);
    pulse(&mut w, &c, 420);
    assert!(w.statuses.is_empty());
    assert_eq!(w.members[0].cooldowns["cast"], deadline);
    assert_eq!(
        w.action(0, &cast(), "early", &c, deadline - 1),
        Err(Error::InvalidState)
    );
    w.action(0, &cast(), "ready", &c, deadline).unwrap();
    commit(&mut w, &c);
    assert_eq!(w.members[0].cooldowns["cast"], deadline + 60 + 601);
}

#[test]
fn member_cooldown_reduction_caps_total_and_respects_positive_and_zero_bounds() {
    for (base, expected) in [(601, 360), (35, 30), (15, 15), (0, 0)] {
        let (mut c, mut w) = setup();
        c.skill.cooldown_ms = base;
        for id in ["cooldown-focus", "cooldown-extra"] {
            buff(&mut w, &c, id, 0);
        }
        w.action(0, &cast(), "cast", &c, 0).unwrap();
        commit(&mut w, &c);
        assert_eq!(w.members[0].cooldowns["cast"], c.skill.windup_ms + expected);
    }
}

#[test]
fn member_cooldown_reduction_samples_at_commit_and_leaves_other_timers_alone() {
    let (mut c, mut w) = setup();
    buff(&mut w, &c, "cooldown-focus", 0);
    w.action(0, &cast(), "late", &c, 240).unwrap();
    commit(&mut w, &c);
    assert_eq!(w.members[0].cooldowns["cast"], 300 + 601);

    let (_, mut w) = setup();
    buff(&mut w, &c, "cooldown-focus", 0);
    w.action(0, &cast(), "failed", &c, 0).unwrap();
    w.members[0].spirit = 0;
    let rejected = commit(&mut w, &c);
    assert!(matches!(
        own_events(&rejected).as_slice(),
        [Event::ActionRejected {
            reason: Error::NoResource,
            ..
        }]
    ));
    assert!(w.members[0].cooldowns.is_empty() && w.members[0].recovery_until == 0);
    c.basic.windup_ms = 30;
    w.action(0, &Action::Attack {}, "basic", &c, 60).unwrap();
    commit(&mut w, &c);
    assert_eq!(w.members[0].basic_ready, 90 + c.basic.interval_ms);
    let item = "xuan-tie";
    c.member_item_use =
        serde_json::from_value(json!({item:{"heal_hp":10,"cooldown_ms":300}})).unwrap();
    w.members[0].inventory.insert(item.into(), 1);
    w.members[0].looted.insert(item.into(), 1);
    w.members[0].hp -= 20;
    let action = Action::UseItem {
        item_id: item.into(),
        expected_revision: w.members[0].revision,
    };
    w.action(0, &action, "item", &c, 120).unwrap();
    assert_eq!(w.members[0].item_cooldowns[item], 420);
}
