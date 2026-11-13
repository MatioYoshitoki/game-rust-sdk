use super::member_unavoidable_tests::{commit, setup, skill};
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn direct_ignore_armor_validates_boolean_capability_and_all_supported_wires() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/direct-ignore-armor-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |v: &Value| {
        package.write(v);
        package.load()
    };
    for pointer in [
        "/basic",
        "/skill",
        "/pve/combat/projectile",
        "/pve/additional_templates/armor-target/combat/projectile",
    ] {
        for value in [json!(false), json!(true)] {
            let mut valid = doc.clone();
            valid.pointer_mut(pointer).unwrap()["ignore_armor"] = value.clone();
            let c = load(&valid).unwrap().default_config();
            assert_eq!(
                serde_json::to_value(c).unwrap().pointer(pointer).unwrap()["ignore_armor"],
                value
            );
            valid["mechanism_version"] = json!(102);
            for location in [
                "/basic",
                "/skill",
                "/pve/combat/projectile",
                "/pve/additional_templates/armor-target/combat/projectile",
            ] {
                valid
                    .pointer_mut(location)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove("ignore_armor");
            }
            valid.pointer_mut(pointer).unwrap()["ignore_armor"] = json!(false);
            assert!(load(&valid).is_err());
        }
        for value in [Value::Null, json!(1), json!("true")] {
            let mut invalid = doc.clone();
            invalid.pointer_mut(pointer).unwrap()["ignore_armor"] = value;
            assert!(load(&invalid).is_err());
        }
    }
    for kind in [
        "projectile",
        "unit_projectile",
        "piercing_projectile",
        "ground_circle",
        "ground_field",
        "line_field",
    ] {
        let mut v = doc.clone();
        v["additional_skills"] = json!([]);
        v["skill"] = skill(kind);
        v["skill"]["ignore_armor"] = json!(true);
        let c = load(&v).unwrap().default_config();
        assert_eq!(
            serde_json::to_value(c).unwrap()["skill"]["ignore_armor"],
            true
        );
    }
    for index in [3, 4] {
        let mut v = doc.clone();
        v["additional_skills"][index]["ignore_armor"] = json!(true);
        assert!(load(&v).is_err());
    }
}

#[test]
fn direct_ignore_armor_member_shots_match_original_vector_and_keep_crit_shield_and_status_order() {
    for kind in [
        "basic",
        "projectile",
        "unit_projectile",
        "piercing_projectile",
    ] {
        let (mut c, mut w) = setup();
        c.mechanism_version = 103;
        c.basic.ignore_armor = Some(true);
        c.skill = serde_json::from_value(skill(if kind == "basic" { "projectile" } else { kind }))
            .unwrap();
        c.skill.ignore_armor = Some(true);
        c.skill.on_hit_target_statuses = vec!["wound".into()];
        if kind == "unit_projectile" {
            c.params.critical_multiplier = 1.5;
            c.pve_mut().status_definitions.push(serde_json::from_value(json!({
                "id":"crit-all","mechanism":"timed_attribute_modifier","attribute":"critical_chance",
                "modifier_bps":10000,"duration_ms":300,"source_scoped":true,"duplicate_policy":"refresh"
            })).unwrap());
            let target = w.members[0].reference.entity.clone();
            bind(&mut w, &c, Owner::Member(0), target, "crit-all", 0);
        }
        let spirit = w.members[0].spirit;
        commit(&mut w, &c, kind);
        c.basic.ignore_armor = Some(false);
        c.skill.ignore_armor = Some(false);
        w.rng = 1;
        w.advance_shots(&c, 60, 1., &mut vec![]);
        // Original defense-true is 100 raw against 100 DEF, yielding 100.
        let amount = if kind == "unit_projectile" { 150 } else { 100 };
        assert_eq!((w.npcs[0].hp, w.rng), (1000 - amount, 1));
        if kind != "basic" {
            assert_eq!(w.members[0].spirit, spirit - 2);
            assert!(
                w.statuses
                    .entries()
                    .iter()
                    .any(|s| s.definition_id == "wound")
            );
        }
    }
    let (mut c, mut w) = setup();
    c.mechanism_version = 103;
    c.basic.ignore_armor = Some(true);
    let target = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target, "ward", 0);
    commit(&mut w, &c, "basic");
    w.advance_shots(&c, 60, 1., &mut vec![]);
    assert_eq!(w.npcs[0].hp, 1000);
    assert_eq!(
        w.statuses
            .shield_total(&status_entity(&w.npcs[0].entity), 60),
        0
    );
}

#[test]
fn direct_ignore_armor_areas_freeze_and_repeat_under_contact_budget() {
    for kind in ["ground_circle", "ground_field", "line_field"] {
        let (mut c, mut w) = setup();
        c.mechanism_version = 103;
        c.skill = serde_json::from_value(skill(kind)).unwrap();
        c.skill.ignore_armor = Some(true);
        commit(&mut w, &c, kind);
        c.skill.ignore_armor = Some(false);
        w.rng = 1;
        w.advance_areas(&c, 150, &mut vec![]);
        assert_eq!((w.npcs[0].hp, w.rng), (900, 1));
        if kind != "ground_circle" {
            let areas = w.snapshot(0, &c, Phase::Active, 150).areas.unwrap();
            assert_eq!(areas[0].contact_policy.as_ref().unwrap().used_hits, 1);
            w.advance_areas(&c, 270, &mut vec![]);
            assert_eq!((w.npcs[0].hp, w.rng), (800, 1));
        }
        w.terminal_intent(0, Outcome::Death, &mut vec![]);
        assert!(w.areas.is_empty());
    }
}

#[test]
fn direct_ignore_armor_npc_shots_keep_reduction_shield_immunity_dodge_and_legacy_order() {
    use super::npc_unavoidable_tests::{impact, release, setup as npc_setup};
    let (mut c, mut w) = npc_setup();
    c.mechanism_version = 103;
    Arc::make_mut(&mut w.npcs[0].profile)
        .combat
        .projectile
        .ignore_armor = Some(true);
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "id":"half-guard","mechanism":"timed_attribute_modifier","attribute":"damage_reduction",
            "modifier_bps":5000,"duration_ms":300,"source_scoped":true,"duplicate_policy":"refresh"
        }))
        .unwrap(),
    );
    let target = w.members[0].reference.entity.clone();
    for id in ["half-guard", "ward"] {
        bind(&mut w, &c, Owner::Member(0), target.clone(), id, 0);
    }
    release(&mut w, &c, 30);
    Arc::make_mut(&mut w.npcs[0].profile)
        .combat
        .projectile
        .ignore_armor = Some(false);
    w.rng = 1;
    impact(&mut w, &c, 60);
    assert_eq!((w.members[0].hp, w.rng), (1000, 1));
    assert_eq!(w.statuses.shield_total(&status_entity(&target), 60), 50);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "wound")
    );

    for guarded in [true, false] {
        let (mut c, mut w) = npc_setup();
        c.mechanism_version = 103;
        let p = Arc::make_mut(&mut w.npcs[0].profile);
        p.combat.projectile.ignore_armor = Some(true);
        p.combat.projectile.unavoidable = Some(guarded);
        c.pve_mut().status_definitions.push(serde_json::from_value(if guarded {
            json!({"id":"test-defense","mechanism":"timed_control","attribute":"damage_immunity",
                "duration_ms":300,"source_scoped":true,"duplicate_policy":"refresh"})
        } else {
            json!({"id":"test-defense","mechanism":"timed_attribute_modifier","attribute":"dodge_chance",
                "modifier_bps":5000,"duration_ms":300,"source_scoped":true,"duplicate_policy":"refresh"})
        }).unwrap());
        let target = w.members[0].reference.entity.clone();
        bind(&mut w, &c, Owner::Member(0), target, "test-defense", 0);
        release(&mut w, &c, 30);
        w.rng = 1;
        impact(&mut w, &c, 60);
        assert_eq!(w.members[0].hp, 1000);
        assert_eq!(w.rng, if guarded { 1 } else { 270369 });
    }
    for (version, policy) in [(103, None), (103, Some(false)), (102, Some(true))] {
        let (mut c, mut w) = npc_setup();
        c.mechanism_version = version;
        Arc::make_mut(&mut w.npcs[0].profile)
            .combat
            .projectile
            .ignore_armor = policy;
        release(&mut w, &c, 30);
        w.rng = 1;
        impact(&mut w, &c, 60);
        assert_eq!((w.members[0].hp, w.rng), (934, 1));
        let source = w.npcs[0].entity.clone();
        let target = w.members[0].reference.entity.clone();
        let result = w.resolve_damage(
            super::combat::DamageIntent {
                ignore_armor: true,
                source,
                target,
                raw: 100,
                direct: false,
            },
            &c,
            90,
            &mut vec![],
        );
        assert_eq!(result.hp_damage, 66);
    }
}
