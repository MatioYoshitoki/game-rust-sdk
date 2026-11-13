use super::control_immunity_tests::own_events;
use super::member_unavoidable_tests::{commit, setup as base_setup, skill};
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_skill_power_validates_own_self_skill_configuration() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-skill-power-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |v: &Value| {
        package.write(v);
        package.load()
    };
    let index = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    let c = load(&doc).unwrap().default_config();
    assert_eq!(
        serde_json::to_value(c).unwrap()["pve"]["status_definitions"][index],
        doc["pve"]["status_definitions"][index]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(103);
    assert!(load(&old).is_err());
    for (key, value) in [
        ("modifier_bps", json!(-1)),
        ("modifier_bps", json!(0)),
        ("modifier_bps", json!(100001)),
        ("source_scoped", json!(false)),
        ("duplicate_policy", json!("ignore")),
        ("mechanism", json!("timed_control")),
    ] {
        let mut v = doc.clone();
        v["pve"]["status_definitions"][index][key] = value;
        assert!(load(&v).is_err());
    }
    for key in ["on_commit_self_statuses", "on_hit_target_statuses"] {
        let mut v = doc.clone();
        v["skill"][key] = json!(["power-focus"]);
        assert!(load(&v).is_err());
        let mut v = doc.clone();
        v["pve"]["additional_templates"]["power-target"]["combat"]["projectile"][key] =
            json!(["power-focus"]);
        assert!(load(&v).is_err());
    }
    for key in ["on_use_self_statuses", "cleanse_self_statuses"] {
        let mut v = doc.clone();
        let item = v["item_id"].as_str().unwrap().to_owned();
        v["member_item_use"] = json!({item:{"heal_hp":10,key:["power-focus"]}});
        assert!(load(&v).is_err());
    }
}

fn setup() -> (Config, World) {
    let (mut c, w) = base_setup();
    c.mechanism_version = 104;
    for (id, bps) in [
        ("power-focus", 5000),
        ("power-quarter", 2500),
        ("power-high-a", 100000),
        ("power-high-b", 100000),
    ] {
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({
            "id":id,"mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus",
            "modifier_bps":bps,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    (c, w)
}

fn buff(w: &mut World, c: &Config, id: &str, now: u64) -> Events {
    let target = w.members[0].reference.entity.clone();
    bind(w, c, Owner::Member(0), target, id, now)
}

#[test]
fn member_skill_power_rounds_power_before_damage_bps_and_saturates_without_affecting_basic() {
    let (mut c, mut w) = setup();
    w.members[0].facts.atk_points = 107;
    buff(&mut w, &c, "power-quarter", 0);
    assert_eq!(w.member_raw_attack(0, 15000, true, &c, 30), 199);
    assert_eq!(w.member_raw_attack(0, 15000, false, &c, 30), 160);
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own.effective_atk, 107);
    assert_eq!(w.member_raw_attack(0, 15000, true, &c, 300), 160);
    c.mechanism_version = 103;
    assert_eq!(w.member_raw_attack(0, 15000, true, &c, 30), 160);
    let (c, mut w) = setup();
    buff(&mut w, &c, "power-high-a", 0);
    buff(&mut w, &c, "power-high-b", 0);
    assert_eq!(w.member_raw_attack(0, 10000, true, &c, 30), 2100);
    w.members[0].facts.atk_points = u32::MAX;
    assert_eq!(w.member_raw_attack(0, 10000, true, &c, 30), u32::MAX);
    assert_eq!(w.member_raw_attack(0, 100000, true, &c, 30), u32::MAX);
}

#[test]
fn member_skill_power_six_damage_plans_and_channel_freeze_at_commit() {
    for kind in [
        "projectile",
        "unit_projectile",
        "piercing_projectile",
        "ground_circle",
        "ground_field",
        "line_field",
    ] {
        let (mut c, mut w) = setup();
        c.skill = serde_json::from_value(skill(kind)).unwrap();
        buff(&mut w, &c, "power-focus", 0);
        commit(&mut w, &c, kind);
        w.rng = 1;
        if c.skill.is_ground() || c.skill.is_line() {
            if c.skill.is_field() {
                w.advance_areas(&c, 150, &mut vec![]);
                assert_eq!(w.npcs[0].hp, 900);
                w.advance_areas(&c, 330, &mut vec![]);
                assert_eq!(w.npcs[0].hp, 800);
            } else {
                w.advance_areas(&c, 330, &mut vec![]);
                assert_eq!(w.npcs[0].hp, 900);
            }
        } else {
            w.advance_shots(&c, 330, 1., &mut vec![]);
            assert_eq!(w.npcs[0].hp, 900);
        }
        assert_eq!(w.rng, 1);
    }
    let (mut c, mut w) = setup();
    let mut v = skill("projectile");
    v["channel"] = json!({"duration_ms":390,"interval_ms":120,"spirit_per_pulse":1});
    c.skill = serde_json::from_value(v).unwrap();
    buff(&mut w, &c, "power-focus", 0);
    commit(&mut w, &c, "projectile");
    assert_eq!(w.members[0].channel.as_ref().unwrap().raw, 150);
    w.advance_channel(0, &c, 330, &mut vec![]);
    assert_eq!(w.shots[0].raw, 150);
    w.advance_shots(&c, 360, 1., &mut vec![]);
    assert_eq!(w.npcs[0].hp, 900);
}

#[test]
fn member_skill_power_refresh_expiry_member_scope_and_exit_cleanup_are_private() {
    let (c, mut w) = setup();
    let peer = w.member_raw_attack(1, 10000, true, &c, 0);
    let applied = buff(&mut w, &c, "power-focus", 0);
    let [Event::StatusApplied { status }] = own_events(&applied)[..] else {
        panic!("missing power status")
    };
    let id = status.instance_id.clone();
    let refreshed = buff(&mut w, &c, "power-focus", 30);
    assert!(
        matches!(own_events(&refreshed).as_slice(),[Event::StatusRefreshed {status}]
        if status.instance_id==id && status.expires_at_ms==330)
    );
    assert_eq!(w.member_raw_attack(0, 10000, true, &c, 329), 150);
    assert_eq!(w.member_raw_attack(0, 10000, true, &c, 330), 100);
    assert_eq!(w.member_raw_attack(1, 10000, true, &c, 30), peer);
    assert!(w.snapshot(1, &c, Phase::Active, 30).own.statuses.is_empty());
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "power-focus", 30);
    assert!(w.snapshot(1, &c, Phase::Active, 30).own.statuses.is_empty());
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert!(w.statuses.entries().is_empty());
}
