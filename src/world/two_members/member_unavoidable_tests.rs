use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

pub(super) fn skill(kind: &str) -> Value {
    let mut v = json!({"mechanism":kind,"id":"test-strike","cost":2,"windup_ms":30,
        "recovery_ms":0,"cooldown_ms":60,"damage_bps":10000,"range":100,"radius":8,
        "on_commit_self_statuses":[],"on_hit_target_statuses":[],"unavoidable":true});
    match kind {
        "projectile" | "unit_projectile" | "piercing_projectile" => {
            v["speed"] = json!(560);
        }
        _ => {
            v["warning_ms"] = json!(0);
        }
    }
    if kind == "piercing_projectile" {
        v["ttl_ms"] = json!(1000);
        v["max_hits"] = json!(2);
    }
    if kind == "ground_field" || kind == "line_field" {
        v["duration_ms"] = json!(390);
        v["tick_interval_ms"] = json!(120);
        v["contact_policy"] =
            json!({"max_targets_per_tick":1,"max_total_hits":2,"repeat_interval_ms":120});
    }
    if kind == "line_field" {
        v.as_object_mut().unwrap().remove("radius");
        v.as_object_mut().unwrap().remove("range");
        v["length"] = json!(100);
        v["width"] = json!(16);
    }
    v
}

#[test]
fn member_unavoidable_validates_basic_and_six_damage_skill_wires() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-unavoidable-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    for kind in [
        "projectile",
        "unit_projectile",
        "piercing_projectile",
        "ground_circle",
        "ground_field",
        "line_field",
    ] {
        let mut valid = doc.clone();
        valid["skill"] = skill(kind);
        valid["additional_skills"] = json!([]);
        for value in [json!(true), json!(false)] {
            valid["skill"]["unavoidable"] = value.clone();
            let c = load(&valid)
                .unwrap_or_else(|e| panic!("{kind}: {e}"))
                .default_config();
            assert_eq!(
                serde_json::to_value(c).unwrap()["skill"]["unavoidable"],
                value
            );
        }
        for value in [Value::Null, json!(0), json!("true")] {
            valid["skill"]["unavoidable"] = value;
            assert!(load(&valid).is_err());
        }
    }
    for pointer in ["/basic", "/skill"] {
        for value in [Value::Null, json!(1), json!("false")] {
            let mut invalid = doc.clone();
            invalid.pointer_mut(pointer).unwrap()["unavoidable"] = value;
            assert!(load(&invalid).is_err());
        }
        let mut old = doc.clone();
        old["mechanism_version"] = json!(101);
        old["basic"].as_object_mut().unwrap().remove("unavoidable");
        old["skill"].as_object_mut().unwrap().remove("unavoidable");
        old.pointer_mut(pointer).unwrap()["unavoidable"] = json!(false);
        assert!(load(&old).is_err());
    }
    for index in [3, 4] {
        let mut invalid = doc.clone();
        invalid["additional_skills"][index]["unavoidable"] = json!(true);
        assert!(load(&invalid).is_err());
    }
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, mut w) = super::armor_penetration_tests::setup();
    c.mechanism_version = 102;
    c.params.metal_multiplier = 1.;
    c.basic.unavoidable = Some(true);
    c.basic.windup_ms = 30;
    c.basic.range = 100.;
    c.members.as_mut().unwrap().area_capacity = Some(8);
    w.npcs.truncate(1);
    Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("dodge".into(), 60);
    (c, w)
}

pub(super) fn commit(w: &mut World, c: &Config, kind: &str) {
    let action = match kind {
        "basic" => Action::Attack {},
        "unit_projectile" => Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.npcs[0].entity),
        },
        "ground_circle" | "ground_field" => Action::CastGround {
            skill_id: c.skill.id.clone(),
            position: w.npcs[0].position,
        },
        _ => Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
    };
    w.action(0, &action, "unavoidable-test", c, 0).unwrap();
    let pending = w.members[0].pending.take().unwrap();
    w.commit(0, pending, c, 30, &mut vec![]);
}

#[test]
fn member_unavoidable_projectiles_freeze_policy_and_keep_cost_spirit_shield_and_status_order() {
    for kind in [
        "basic",
        "projectile",
        "unit_projectile",
        "piercing_projectile",
    ] {
        let (mut c, mut w) = setup();
        c.skill = serde_json::from_value(skill(if kind == "basic" { "projectile" } else { kind }))
            .unwrap();
        c.skill.on_hit_target_statuses = vec!["wound".into()];
        w.members[0].spirit -= 20;
        let spirit = w.members[0].spirit;
        if kind == "basic" {
            let target = w.npcs[0].entity.clone();
            bind(&mut w, &c, Owner::Npc(0), target, "ward", 0);
        }
        commit(&mut w, &c, kind);
        assert!(w.shots[0].unavoidable);
        c.basic.unavoidable = Some(false);
        c.skill.unavoidable = Some(false);
        w.rng = 1;
        w.advance_shots(&c, 60, 1., &mut vec![]);
        assert_eq!(w.rng, 1);
        if kind == "basic" {
            assert_eq!(w.npcs[0].hp, 1000);
            assert_eq!(
                w.statuses
                    .shield_total(&status_entity(&w.npcs[0].entity), 60),
                34
            );
            assert_eq!(w.members[0].spirit, spirit + c.params.basic_spirit_on_hit);
        } else {
            assert_eq!(w.npcs[0].hp, 934);
            assert_eq!(w.members[0].spirit, spirit - 2);
            assert!(
                w.statuses
                    .entries()
                    .iter()
                    .any(|s| s.definition_id == "wound")
            );
        }
    }
}

#[test]
fn member_unavoidable_areas_freeze_policy_repeat_with_budget_and_clean_on_exit() {
    for kind in ["ground_circle", "ground_field", "line_field"] {
        let (mut c, mut w) = setup();
        c.skill = serde_json::from_value(skill(kind)).unwrap();
        commit(&mut w, &c, kind);
        c.skill.unavoidable = Some(false);
        w.rng = 1;
        w.advance_areas(&c, 150, &mut vec![]);
        assert_eq!((w.npcs[0].hp, w.rng), (934, 1));
        if kind != "ground_circle" {
            let view = w.snapshot(0, &c, Phase::Active, 150).areas.unwrap();
            assert_eq!(view[0].contact_policy.as_ref().unwrap().used_hits, 1);
            w.advance_areas(&c, 270, &mut vec![]);
            assert_eq!((w.npcs[0].hp, w.rng), (868, 1));
        }
        w.terminal_intent(0, Outcome::Death, &mut vec![]);
        assert!(w.areas.is_empty());
    }
}

#[test]
fn member_unavoidable_respects_geometry_source_and_legacy_dodge_paths() {
    for (version, policy) in [(102, None), (102, Some(false)), (101, Some(true))] {
        let (mut c, mut w) = setup();
        c.mechanism_version = version;
        c.basic.unavoidable = policy;
        commit(&mut w, &c, "basic");
        w.rng = 1;
        w.advance_shots(&c, 60, 1., &mut vec![]);
        assert_eq!((w.npcs[0].hp, w.rng), (1000, 270369));
    }
    for blocked in [true, false] {
        let (c, mut w) = setup();
        commit(&mut w, &c, "basic");
        if blocked {
            Arc::make_mut(&mut w.grid.config).palette[0].blocks_projectiles = true;
        } else {
            w.terminal_intent(0, Outcome::Death, &mut vec![]);
        }
        w.rng = 1;
        w.advance_shots(&c, 60, 1., &mut vec![]);
        assert_eq!((w.npcs[0].hp, w.rng), (1000, 1));
        assert!(w.shots.is_empty());
    }
}
