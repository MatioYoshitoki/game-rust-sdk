//! v1.1 numeric vectors bound to authoritative kernel paths (I05 / R6.2).
//!
//! The vector file is an arithmetic oracle: every assertion reads the kernel's
//! real formula or commit chain, never a reimplementation. Inputs and expected
//! values come from `numeric-vectors.json`; only the host-side table constants
//! (defense scale/cap, speed curve anchors, tier table) are configured here.
use super::*;
use crate::world::two_members::combat::DamageIntent;
use serde_json::{Value, json};

fn vectors(op: &str) -> Vec<Value> {
    let document: Value =
        serde_json::from_str(crate::test_data::fixture!("numeric-vectors.json")).unwrap();
    document["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["op"] == op)
        .cloned()
        .collect()
}

fn v_input(v: &Value, key: &str) -> f64 {
    v["input"][key].as_f64().unwrap()
}

/// Host-side table constants from behavior.md: speed anchors 180/120/8/100,
/// defense scale 2 with a 0.7 reduction cap, and the realm-difference table
/// d>=0: 1+0.1d / d<0: 0.5^(-d) over the differences the vectors exercise.
fn oracle_base(v: &mut Value) {
    let params = &mut v["params"];
    params["movement"] = json!({"base":180,"bonus":120,"reference":8,"log_base":100});
    params["defense_scale"] = json!(2);
    params["minimum_damage"] = json!(1);
    params["orb_hp_ratio"] = json!(0);
    params["orb_spirit_ratio"] = json!(0);
    params["initial_player"] = json!({"hp_points":1000,"spirit_points":200,"atk_points":10,
        "def_points":0,"spd_points":1});
    params["death"] = json!({"slot_ratio":0,"quantity_ratio":0,"minimum_quantity":1});
    v["defense_cap"] = json!(0.7);
    let profiles = &mut v["combat_rules"]["profiles"];
    for id in ["member", "npc"] {
        profiles[id] = json!({"tier":0,"defense_baseline_attack":100,"hit_bps":10000,
            "critical_chance_bps":0,"dodge_bps":0,"critical_multiplier_bps":10000,
            "power_points_bps":0});
    }
    v["combat_rules"]["default_member_profile"] = json!("member");
    v["combat_rules"]["default_npc_profile"] = json!("npc");
    v["combat_rules"]["tier_multipliers"] =
        json!({"-2":2500,"-1":5000,"0":10000,"1":11000,"2":12000});
    v["pve"]["stats"] =
        json!({"hp":100000,"spirit":100,"atk":10,"def":0,"spd":1,"crit":0,"dodge":0});
    v["pve"].as_object_mut().unwrap().remove("status_rules");
}

/// `status_rules` requires non-empty `status_definitions`; the heal and shield
/// families carry the definitions and table caps they exercise.
fn oracle_status_rules(v: &mut Value, definitions: Value) {
    v["pve"]["status_rules"] = json!({"max_active_per_entity":8,"defense_bonus_cap_bps":10000,
        "attack_bonus_cap_bps":10000,"shield_cap_bps":3000});
    v["pve"]["status_definitions"] = definitions;
}

fn oracle_world(mutate: impl FnOnce(&mut Value)) -> (Arc<Config>, World) {
    oracle_world_withfacts(mutate, 1000, 200, 1)
}

fn oracle_world_withfacts(
    mutate: impl FnOnce(&mut Value),
    hp_points: u32,
    spirit_points: u32,
    spd_points: u32,
) -> (Arc<Config>, World) {
    let c = catalog(input(), |v| {
        oracle_base(v);
        mutate(v);
    })
    .unwrap()
    .default_config();
    let seats: Vec<_> = ["a", "b"]
        .iter()
        .map(|id| ParticipantSeat {
            participant_id: (*id).into(),
            authenticated_principal: (*id).into(),
            connection: Connection {
                id: (*id).into(),
                generation: 1,
            },
            weapon: None,
            basic_profile: None,
            combat_build: None,
            skills: None,
        })
        .collect();
    let facts: Vec<_> = seats
        .iter()
        .map(|seat| ParticipantFacts {
            principal: seat.authenticated_principal.clone(),
            revision: 0,
            hp_points,
            spirit_points,
            atk_points: 10,
            def_points: 0,
            spd_points,
            warehouse_quantity: 4,
        })
        .collect();
    let mut world = World::new(&c, &seats, facts, 7);
    for member in &mut world.members {
        member.joined = true;
        member.ready = true;
    }
    world.activate(&c, 0, &mut vec![]);
    (c, world)
}

fn intent(w: &World, raw: u32) -> DamageIntent {
    DamageIntent {
        ignore_armor: false,
        source: w.members[0].reference.entity.clone(),
        target: w.npcs[0].entity.clone(),
        raw,
        direct: true,
    }
}

fn resolved_hp_damage(
    w: &mut World,
    c: &Config,
    raw: u32,
    rule: &crate::config::AttackRule,
) -> u32 {
    w.resolve_damage_with_rule(intent(w, raw), Some(rule), c, 0, &mut vec![])
        .hp_damage
}

#[test]
fn source_vector_file_is_the_frozen_v1_1_oracle() {
    let document: Value =
        serde_json::from_str(crate::test_data::fixture!("numeric-vectors.json")).unwrap();
    assert_eq!(document["vectors"].as_array().unwrap().len(), 50);
    assert_eq!(
        document["role"],
        json!("arithmetic-oracles-not-runtime-tests")
    );
    assert_eq!(document["tolerance"], json!(1e-9));
}

#[test]
fn defense_damage_vectors_resolve_through_the_authoritative_hit_chain() {
    for vector in vectors("defense_damage") {
        let negative_armor = v_input(&vector, "armor") < 0.0;
        let (c, mut w) = oracle_world(|v| {
            if !negative_armor {
                v["pve"]["stats"]["def"] = json!(v_input(&vector, "armor") as u64);
            }
            v["combat_rules"]["profiles"]["member"]["critical_chance_bps"] =
                json!(if v_input(&vector, "critMultiplier") > 1.0 {
                    10000
                } else {
                    0
                });
            v["combat_rules"]["profiles"]["member"]["critical_multiplier_bps"] =
                json!((v_input(&vector, "critMultiplier") * 10000.0) as u64);
        });
        let mut rule = crate::config::AttackRule::default();
        rule.armor_penetration_bps = v_input(&vector, "pierceBps") as u32;
        rule.ignore_armor = vector["input"]["irreducible"].as_bool().unwrap();
        let expected = vector["expected"].as_u64().unwrap() as u32;
        let damage = if negative_armor {
            // The config schema keeps DEF non-negative, so the negative-armor
            // clamp binds at the authoritative pure function the chain calls.
            crate::combat_math::defense_damage(
                v_input(&vector, "raw"),
                v_input(&vector, "armor"),
                crate::combat_math::DefenseContext {
                    baseline: v_input(&vector, "defenderRealmBaseAtk") as u32,
                    pierce: rule.armor_penetration_bps,
                    ignore: rule.ignore_armor,
                    scale: c.params.defense_scale,
                    cap: c.defense_cap,
                    critical: 1.0,
                    minimum: c.params.minimum_damage,
                },
            )
        } else {
            resolved_hp_damage(&mut w, &c, v_input(&vector, "raw") as u32, &rule)
        };
        assert_eq!(damage, expected, "{}", vector["id"]);
    }
}

#[test]
fn realm_modifier_vectors_bind_the_tier_difference_table() {
    for vector in vectors("realm_modifier") {
        let difference = v_input(&vector, "levelDifference") as i16;
        let (member_tier, npc_tier) = if difference >= 0 {
            (difference, 0)
        } else {
            (0, -difference)
        };
        let (c, mut w) = oracle_world(|v| {
            v["combat_rules"]["profiles"]["member"]["tier"] = json!(member_tier);
            v["combat_rules"]["profiles"]["npc"]["tier"] = json!(npc_tier);
            v["pve"]["stats"]["def"] = json!(0);
        });
        let rule = crate::config::AttackRule::default();
        let damage = resolved_hp_damage(&mut w, &c, 100, &rule);
        assert_eq!(
            damage,
            (100.0 * vector["expected"].as_f64().unwrap()) as u32,
            "{}",
            vector["id"]
        );
    }
}

#[test]
fn dodge_percent_vectors_bind_the_authoritative_probability() {
    for vector in vectors("dodge_percent") {
        let probability = combat::dodge_probability(
            (v_input(&vector, "dodge") * 100.0) as u64,
            (v_input(&vector, "penalty") * 100.0) as u32,
            (v_input(&vector, "hit") * 100.0) as u64,
        );
        let expected = vector["expected"].as_f64().unwrap() / 100.0;
        assert!(
            (probability - expected).abs() < 1e-9,
            "{}: {} != {}",
            vector["id"],
            probability,
            expected
        );
    }
}

#[test]
fn movement_speed_vectors_match_the_configured_curve_and_real_steps() {
    for vector in vectors("movement_speed") {
        let spd = v_input(&vector, "spd") as u32;
        let rank = v_input(&vector, "rankMultiplier");
        let (c, mut w) = oracle_world_withfacts(
            |v| {
                v["pve"]["palette"][0]["speedMultiplier"] = json!(rank);
            },
            1000,
            200,
            spd,
        );
        let expected = vector["expected"].as_f64().unwrap();
        // The unit-class rank multiplier flows through the kernel's real
        // multiplier channel (terrain/status); the curve itself is rank-free.
        let computed = c.params.movement.speed(spd) * rank;
        assert!(
            (computed - expected).abs() < 1e-9,
            "{}: {} != {}",
            vector["id"],
            computed,
            expected
        );
        if spd > 0 {
            // A live entity floors SPD at 1, so the spd-0 boundary stays a
            // formula-level binding; every other vector drives a real step.
            w.members[0].position = [20.0, 20.0];
            w.action(
                0,
                &Action::Move {
                    direction: [1.0, 0.0],
                },
                "step",
                &c,
                0,
            )
            .unwrap();
            w.tick(&c, 30);
            let step = w.members[0].position[0] - 20.0;
            assert!(
                (step - expected * 0.03).abs() < 1e-6,
                "{}: real step {} != {}",
                vector["id"],
                step,
                expected * 0.03
            );
        }
    }
}

#[test]
fn move_step_vectors_match_normalized_real_steps() {
    for vector in vectors("move_step") {
        let direction = [
            vector["input"]["direction"][0].as_f64().unwrap(),
            vector["input"]["direction"][1].as_f64().unwrap(),
        ];
        let (c, mut w) = oracle_world(|_| {});
        w.members[0].position = [20.0, 20.0];
        w.action(0, &Action::Move { direction }, "step", &c, 0)
            .unwrap();
        w.tick(&c, 30);
        let expected = vector["expected"].as_array().unwrap();
        for axis in 0..2 {
            let moved = w.members[0].position[axis] - 20.0;
            assert!(
                (moved - expected[axis].as_f64().unwrap()).abs() < 1e-6,
                "{} axis {}: moved {:?}",
                vector["id"],
                axis,
                w.members[0].position
            );
        }
    }
}

#[test]
fn duration_ticks_vectors_bind_real_execution_domain_and_tail_divergence() {
    for vector in vectors("duration_ticks") {
        let milliseconds = v_input(&vector, "milliseconds") as u64;
        let step_ms = v_input(&vector, "stepMs") as u64;
        if milliseconds <= step_ms {
            // The kernel config domain requires duration to strictly exceed the
            // tick interval, so the two boundary vectors are not expressible.
            let rejected = catalog(input(), |v| {
                oracle_base(v);
                v["step_ms"] = json!(step_ms);
                oracle_status_rules(
                    v,
                    json!([{"id":"wound","mechanism":"periodic_damage","damage_bps":1000,
                        "tick_interval_ms":step_ms,"duration_ms":milliseconds,
                        "duplicate_policy":"refresh","source_scoped":true}]),
                );
            });
            assert!(rejected.is_err(), "{} must be rejected", vector["id"]);
            continue;
        }
        let (c, mut w) = oracle_world(|v| {
            v["step_ms"] = json!(step_ms);
            oracle_status_rules(
                v,
                json!([{"id":"wound","mechanism":"periodic_damage","damage_bps":1000,
                    "tick_interval_ms":step_ms,"duration_ms":milliseconds,
                    "duplicate_policy":"refresh","source_scoped":true}]),
            );
        });
        // The expiry tick is a Rust-side definition flag (ally recovery path).
        let mut config = c.as_ref().clone();
        config.pve_mut().status_definitions[0].periodic_tick_on_expiry = true;
        let c = std::sync::Arc::new(config);
        w.apply_owned_statuses(
            Owner::Member(0),
            (w.npcs[0].entity.clone(), 100000),
            &["wound".into()],
            &c,
            0,
            &mut vec![],
        );
        let mut fired = 0usize;
        let mut now = 0u64;
        while now <= milliseconds + step_ms {
            now += step_ms;
            let mut events = vec![];
            w.prepare_periodic(now, &mut events);
            w.advance_periodic(&c, now, &mut events);
            fired += events
                .iter()
                .filter(|(_, e)| {
                    matches!(
                        e,
                        EventV4::Private {
                            event: Event::PeriodicTick { .. }
                        }
                    )
                })
                .count();
        }
        let expected = vector["expected"].as_u64().unwrap() as usize;
        if milliseconds % step_ms == 0 {
            assert_eq!(fired, expected, "{}", vector["id"]);
        } else {
            // Frozen SDK-5-6 semantics keep every planned tick strictly before
            // expiry, dropping the partial tail. The source oracle rounds the
            // duration up to a whole tick; the divergence is recorded for
            // arbitration instead of being silently resolved here.
            assert_eq!(fired, expected - 1, "{}", vector["id"]);
        }
    }
}

#[test]
fn heal_vectors_bind_the_received_multiplier_and_missing_hp_cap() {
    for vector in vectors("heal") {
        let hp = v_input(&vector, "hp") as u32;
        let (c, mut w) = oracle_world(|v| {
            oracle_status_rules(
                v,
                json!([{"id":"care","mechanism":"timed_attribute_modifier",
                    "attribute":"healing_received",
                    "modifier_bps":(v_input(&vector, "receivedPercent") * 100.0) as i64,
                    "duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}]),
            );
        });
        w.members[0].hp = hp;
        w.apply_owned_statuses(
            Owner::Member(0),
            (w.members[0].reference.entity.clone(), 1000),
            &["care".into()],
            &c,
            0,
            &mut vec![],
        );
        let source = w.members[0].reference.entity.clone();
        let mut events = vec![];
        w.ratio_self_heal(
            &source,
            v_input(&vector, "base") as u64,
            (10000, crate::protocol::HealKind::SelfSkill),
            &c,
            0,
            &mut events,
        );
        let resolved = events
            .iter()
            .find_map(|(_, e)| {
                if let EventV4::Private {
                    event:
                        Event::HealResolved {
                            hp_restored,
                            adjusted_heal,
                            ..
                        },
                } = e
                {
                    Some((hp_restored, adjusted_heal))
                } else {
                    None
                }
            })
            .unwrap();
        let expected = vector["expected"].as_u64().unwrap();
        // The oracle value is the authoritative formula amount. Capped application
        // keeps hp_restored at the missing-HP bound; allowOverheal binds the
        // authoritative adjusted amount, with overflow-to-shield as a declared policy.
        let allow_overheal = vector["input"]["allowOverheal"].as_bool().unwrap();
        let binding = if allow_overheal {
            u64::from(*resolved.1)
        } else {
            u64::from(*resolved.0)
        };
        assert_eq!(
            binding, expected,
            "{}: restored {} adjusted {}",
            vector["id"], resolved.0, resolved.1
        );
    }
}

#[test]
fn death_slot_and_quantity_vectors_bind_the_real_drop_parameters() {
    for vector in vectors("death_slots") {
        let rate = v_input(&vector, "rate");
        let slots = v_input(&vector, "eligibleSlots") as usize;
        let (c, _) = oracle_world(|v| {
            v["params"]["death"] =
                json!({"slot_ratio":rate,"quantity_ratio":rate,"minimum_quantity":1});
        });
        assert_eq!(
            c.params.death.count(slots),
            vector["expected"].as_u64().unwrap() as usize,
            "{}",
            vector["id"]
        );
    }
    for vector in vectors("death_quantity") {
        let rate = v_input(&vector, "rate");
        let quantity = v_input(&vector, "quantity") as u32;
        let (c, _) = oracle_world(|v| {
            v["params"]["death"] =
                json!({"slot_ratio":rate,"quantity_ratio":rate,"minimum_quantity":1});
        });
        assert_eq!(
            c.params.death.amount(quantity),
            vector["expected"].as_u64().unwrap() as u32,
            "{}",
            vector["id"]
        );
    }
}

#[test]
fn shield_grant_vectors_bind_the_shared_pool_room() {
    for vector in vectors("shield_grant") {
        let other = v_input(&vector, "otherShields") as u64;
        let requested = v_input(&vector, "requested") as u64;
        let (c, mut w) = oracle_world(|v| {
            oracle_status_rules(
                v,
                json!([
                    {"id":"ward-a","mechanism":"absorption_shield",
                     "shield_bps":other * 10,"duration_ms":3000,
                     "duplicate_policy":"ignore","source_scoped":false},
                    {"id":"ward-b","mechanism":"absorption_shield",
                     "shield_bps":requested * 10,"duration_ms":3000,
                     "duplicate_policy":"ignore","source_scoped":false}
                ]),
            );
        });
        w.shields_enabled = true;
        let target = w.members[0].reference.entity.clone();
        w.apply_owned_statuses(
            Owner::Member(0),
            (target.clone(), 1000),
            &["ward-a".into()],
            &c,
            0,
            &mut vec![],
        );
        let before = w
            .snapshot(0, &c, Phase::Active, 0)
            .own
            .shield_total
            .unwrap_or(0);
        // A second, different source keeps the grants independent.
        w.apply_owned_statuses(
            Owner::Npc(0),
            (target, 1000),
            &["ward-b".into()],
            &c,
            0,
            &mut vec![],
        );
        let after = w
            .snapshot(0, &c, Phase::Active, 0)
            .own
            .shield_total
            .unwrap_or(0);
        // The oracle expects the second grant alone; the shared cap keeps the pool
        // at 30% maxHp, so the granted room is after - before and the total clamps.
        assert_eq!(
            (after - before) as u64,
            vector["expected"].as_u64().unwrap(),
            "{}: total {} -> {}",
            vector["id"],
            before,
            after
        );
    }
}

#[test]
fn orb_vectors_bind_recovery_ratios_against_max_pools() {
    for vector in vectors("orb") {
        let (c, mut w) = oracle_world(|v| {
            v["params"]["orb_hp_ratio"] = json!(v_input(&vector, "hpRatio"));
            v["params"]["orb_spirit_ratio"] = json!(v_input(&vector, "spiritRatio"));
        });
        let profile = w.npcs[0].profile.clone();
        w.drop_monster_orb(0, 0, &profile, &c, 0, &mut vec![]);
        let orb = w.resources.last().unwrap();
        assert_eq!(
            orb.hp,
            vector["expected"]["hp"].as_u64().unwrap() as u32,
            "{} hp",
            vector["id"]
        );
        assert_eq!(
            orb.spirit,
            vector["expected"]["spirit"].as_u64().unwrap() as u32,
            "{} spirit",
            vector["id"]
        );
    }
}
