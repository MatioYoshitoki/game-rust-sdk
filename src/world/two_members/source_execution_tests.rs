//! Original inputs and expected values drive production arithmetic and scheduling.
use super::*;
use serde_json::{Value, json};
fn vectors(op: &str) -> Vec<Value> {
    let source: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_combat_vectors.json"
    ))
    .unwrap();
    source["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["op"] == op)
        .cloned()
        .collect()
}
fn number(v: &Value, key: &str) -> u32 {
    v[key].as_u64().unwrap().try_into().unwrap()
}
fn healing_vector(row: &Value) -> (u64, u32) {
    let x = &row["input"];
    let (mut c, mut w) = super::unit_effect_tests::setup(false, false);
    w.statuses = Default::default();
    w.members[0].facts.hp_points = number(x, "maxHp");
    w.members[0].hp = number(x, "hp");
    c.combat_rules.as_mut().unwrap().healing.clear();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"source-healing","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":x["receivedPercent"].as_i64().unwrap()*100,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own, "source-healing", 0);
    let heal = serde_json::from_value(json!({"amount":x["base"]})).unwrap();
    let mut e = vec![];
    w.resolve_skill_heal((0, 0), &heal, &c.skill.id, &c, 30, &mut e);
    let actual = e
        .iter()
        .find_map(|(_, e)| match e {
            EventV4::Private {
                event:
                    Event::HealResolved {
                        adjusted_heal,
                        hp_restored,
                        ..
                    },
            } => Some(if x["allowOverheal"].as_bool().unwrap() {
                *adjusted_heal
            } else {
                u64::from(*hp_restored)
            }),
            _ => None,
        })
        .unwrap();
    assert!(w.members[0].hp <= number(x, "maxHp"));
    (actual, w.members[0].hp - number(x, "hp"))
}
#[test]
fn source_execution_four_heals_observe_adjusted_budget_and_real_hp_separately() {
    let rows = vectors("heal");
    assert_eq!(rows.len(), 4);
    for row in rows {
        let (actual, restored) = healing_vector(&row);
        assert_eq!(actual, row["expected"].as_u64().unwrap(), "{}", row["id"]);
        if row["input"]["allowOverheal"] == true {
            assert_eq!(restored, 50);
        }
    }
}
#[test]
fn source_execution_two_shields_measure_native_source_grant_without_reducing_existing_balance() {
    let rows = vectors("shield_grant");
    assert_eq!(rows.len(), 2);
    for row in rows {
        let x = &row["input"];
        let (mut c, mut w) = super::unit_effect_tests::setup(false, false);
        w.statuses = Default::default();
        w.members[0].facts.hp_points = number(x, "maxHp");
        c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(3000);
        for (id, amount) in [
            ("old", number(x, "otherShields")),
            ("new", number(x, "requested")),
        ] {
            let bps = u64::from(amount) * 10000 / u64::from(number(x, "maxHp"));
            c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"absorption_shield","shield_bps":bps,"duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
        }
        let own = w.members[0].reference.entity.clone();
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "old", 0);
        let old = w
            .statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "old")
            .unwrap()
            .clone();
        assert_eq!(
            old.shield.as_ref().unwrap().remaining,
            number(x, "otherShields")
        );
        super::periodic_tests::bind(&mut w, &c, Owner::Member(0), own.clone(), "new", 30);
        assert_eq!(
            w.statuses
                .entries()
                .iter()
                .find(|s| s.instance_id == old.instance_id)
                .unwrap()
                .shield,
            old.shield
        );
        let total = w.statuses.shield_total(&status_entity(&own), 30);
        assert_eq!(
            total - number(x, "otherShields"),
            row["expected"].as_u64().unwrap() as u32,
            "{}",
            row["id"]
        );
    }
}
#[test]
fn source_execution_five_tick_vectors_use_real_preparation_and_world_deadline() {
    let rows = vectors("duration_ticks");
    assert_eq!(rows.len(), 5);
    for row in rows {
        let x = &row["input"];
        let (mut c, mut w) = super::unit_effect_tests::setup(false, false);
        c.step_ms = u64::from(number(x, "stepMs"));
        c.skill.windup_ms = u64::from(number(x, "milliseconds"));
        c.skill.cost = 0;
        let target = w.npcs[0].entity.clone();
        w.action(
            0,
            &Action::CastUnit {
                skill_id: c.skill.id.clone(),
                target: World::unit_wire(&target),
            },
            "original-clock",
            &c,
            0,
        )
        .unwrap();
        let expected = row["expected"].as_u64().unwrap();
        for tick in 1..=expected {
            let e = w.tick(&c, tick * c.step_ms);
            let committed = e
                .iter()
                .filter(|(_, e)| {
                    matches!(
                        e,
                        EventV4::Private {
                            event: Event::ActionCommitted { .. }
                        }
                    )
                })
                .count();
            assert_eq!(
                committed,
                usize::from(tick == expected),
                "{} @{}",
                row["id"],
                tick
            );
            assert_eq!(w.members[0].pending.is_none(), tick == expected);
        }
    }
}

#[test]
fn source_execution_s08_runs_exact_original_numeric_selection_and_assertion() {
    let routes: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_execution_routes.json"
    ))
    .unwrap();
    let scenario = &routes["r3Scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "S08")
        .unwrap()["original"];
    assert_eq!(scenario["scope"], "numeric");
    assert!(scenario["when"].as_array().unwrap().is_empty());
    assert_eq!(scenario["then"], json!([{"op":"allNumericVectorsPass"}]));
    let ids = scenario["given"]["numericVectorIds"].as_array().unwrap();
    assert_eq!(
        ids,
        &vec![
            json!("defense-basic"),
            json!("defense-pierce"),
            json!("defense-cap"),
            json!("defense-crit"),
            json!("heal-cap")
        ]
    );
    for id in ids {
        let row = &routes["vectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == *id)
            .unwrap()["original"];
        let x = &row["input"];
        let actual = match row["op"].as_str().unwrap() {
            "defense_damage" => u64::from(crate::combat_math::defense_damage(
                x["raw"].as_f64().unwrap(),
                x["armor"].as_f64().unwrap(),
                crate::combat_math::DefenseContext {
                    baseline: number(x, "defenderRealmBaseAtk"),
                    pierce: number(x, "pierceBps"),
                    ignore: x["irreducible"].as_bool().unwrap(),
                    scale: 2.,
                    cap: 0.7,
                    critical: x["critMultiplier"].as_f64().unwrap(),
                    minimum: 1,
                },
            )),
            "heal" => healing_vector(row).0,
            op => panic!("unbound S08 operation {op}"),
        };
        assert_eq!(actual, row["expected"].as_u64().unwrap(), "{id}");
    }
}
#[test]
fn source_execution_fifty_routes_preserve_exact_oracles_and_keep_unexecuted_work_open() {
    let data: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_combat_vectors.json"
    ))
    .unwrap();
    let routes: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_execution_routes.json"
    ))
    .unwrap();
    let vectors = routes["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 50);
    assert_eq!(data["tolerance"], routes["tolerance"]);
    let mut native = 0;
    let mut r3 = 0;
    for (i, r) in vectors.iter().enumerate() {
        assert_eq!(r["original"], data["vectors"][i]);
        assert_eq!(r["id"], r["original"]["id"]);
        assert_eq!(r["source"]["pointer"], format!("/vectors/{i}"));
        assert_eq!(r["fullWorkPackageAccepted"], false);
        if r["exactExecutionStatus"] == "native_assertion_verified" {
            assert!(
                r["test"]
                    .as_str()
                    .unwrap()
                    .starts_with("world::two_members::")
            );
            assert!(r["executor"].as_str().is_some());
            native += 1;
        } else {
            assert!(
                r["test"].is_null() && r["executor"].is_null() && r["reason"].as_str().is_some()
            );
        }
        if r["workPackages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| ["R3.1", "R3.2", "R3.3"].iter().any(|id| w == id))
        {
            assert_eq!(r["exactExecutionStatus"], "native_assertion_verified");
            r3 += 1;
        }
    }
    assert_eq!((native, r3), (27, 13));
    assert_eq!(routes["wholeR6_2Accepted"], false);
    assert_eq!(routes["wholeDelivery01Accepted"], false);
}
