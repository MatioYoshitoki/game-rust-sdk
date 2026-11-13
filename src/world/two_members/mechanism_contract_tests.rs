//! Synthetic inputs for the mechanism contract. No game tables are needed.
use super::*;
use serde_json::{Value, json};

#[path = "boon_contract_tests.rs"]
mod boon_contract_tests;
#[path = "checkpoint_counter_contract_tests.rs"]
mod checkpoint_counter_contract_tests;
#[path = "contested_equipment_contract_tests.rs"]
mod contested_equipment_contract_tests;
#[path = "encounter_progress_contract_tests.rs"]
mod encounter_progress_contract_tests;
#[path = "extraction_lifecycle_contract_tests.rs"]
mod extraction_lifecycle_contract_tests;
#[path = "match_mode_contract_tests.rs"]
mod match_mode_contract_tests;
#[path = "member_conflict_contract_tests.rs"]
mod member_conflict_contract_tests;
#[path = "module03_contract_tests.rs"]
mod module03_contract_tests;
#[path = "npc_ai_contract_tests.rs"]
mod npc_ai_contract_tests;
#[path = "settlement_progression_contract_tests.rs"]
mod settlement_progression_contract_tests;
#[path = "settlement_reward_contract_tests.rs"]
mod settlement_reward_contract_tests;
#[cfg(all(test, match_sdk_test_data))]
#[path = "source_scenario_combat_tests.rs"]
mod source_scenario_combat_tests;
#[cfg(all(test, match_sdk_test_data))]
#[path = "source_vector_oracle_tests.rs"]
mod source_vector_oracle_tests;
#[path = "training_target_contract_tests.rs"]
mod training_target_contract_tests;
#[path = "world_loot_contract_tests.rs"]
mod world_loot_contract_tests;
#[path = "zone_exposure_contract_tests.rs"]
mod zone_exposure_contract_tests;

pub(crate) fn input() -> Value {
    let params = json!({
        "player_radius":2, "pickup_range":24, "extraction_range":24,
        "movement":{"base":60,"bonus":0,"reference":1,"log_base":10},
        "regen":{"interval_ms":1000,"near_radius":0,"near_ratio":0,"far_ratio":0},
        "basic_spirit_on_hit":0, "orb_hp_ratio":0, "orb_spirit_ratio":0,
        "defense_scale":1, "minimum_damage":1, "critical_multiplier":1, "metal_multiplier":1,
        "resource_ttl_ms":1000, "drop_min_radius":0, "drop_max_radius":0,
        "basic_projectile_radius":1, "player_projectile_ttl_ms":1000, "entry_years":1,
        "death":{"slot_ratio":0,"quantity_ratio":0,"minimum_quantity":0},
        "initial_player":{"hp_points":100,"spirit_points":100,"atk_points":10,
                          "def_points":0,"spd_points":1},
        "runtime":{"io_timeout_ms":500,"drain_timeout_ms":1000,"reconnect_grace_ms":500,
                   "publish_batched_ms":60,"nav_budget":16,"nav_repath_ms":30,
                   "ai_decision_ms":30,"resource_sweep_ms":100,"call_timeout_ms":1000,
                   "max_attempts":2,"max_catchup_steps":2,"max_lag_steps":8}
    });
    let pve = json!({
        "template_id":"npc", "map_id":"test", "width":4, "height":4, "cell_size":32,
        "palette":[{"terrain":"floor","dotDamagePerSecond":0,"elevation":0,
                   "maxClimbHeight":0,"maxDropHeight":0,"walkable":true,"speedMultiplier":1,
                   "blocksProjectiles":false,"blocksVision":false}],
        "cells_rle":[[0,16]], "monster_positions":[[52,20]],
        "stats":{"hp":100,"spirit":100,"atk":10,"def":0,"spd":1,"crit":0,"dodge":0},
        "combat":{"radius":2,"attack_interval_ms":1000,"aggro_range":64,"melee_range":2,
                  "cast_chance":1,"return_distance":8,"lifesteal_ratio":0,"mark_max":0,
                  "mark_duration_ms":0,
                  "projectile":{"mechanism":"projectile","id":"npc-cast","cost":60,
                                "windup_ms":30,"cooldown_ms":300,"recovery_ms":60,
                                "range":80,"speed":10,"radius":1,"ttl_ms":1000,"damage_bps":10000},
                  "heal":{"mechanism":"budget_heal_channel","id":"npc-heal","cost":0,
                          "windup_ms":30,"cooldown_ms":300,"recovery_ms":60,"hp_threshold":0,
                          "heal_bps":0,"interval_ms":30,"ticks":1}},
        "drop_table":{"guaranteed":[],"random":[]},
        "items":{"item":{"name":"item","stackLimit":4}},
        "leash":64,"help_range":1,"memory_range":64,
        "status_rules":{"max_active_per_entity":8,"defense_bonus_cap_bps":10000,
                            "attack_bonus_cap_bps":10000,"shield_cap_bps":10000}
    });
    let combat_rules = json!({
        "profiles":{"test":{"tier":0,"defense_baseline_attack":10,"hit_bps":10000,
                            "critical_chance_bps":0,"dodge_bps":0,"critical_multiplier_bps":10000,
                            "power_points_bps":0}},
        "default_member_profile":"test","default_npc_profile":"test",
        "member_profiles":{},"npc_profiles":{},
        "elements":[],"tier_multipliers":{"0":10000},
        "element_multipliers_bps":{},"weapon_skill_multipliers_bps":{},
        "basic_attack":crate::config::AttackRule::default(),
        "skills":{},"npc_attacks":{},"periodic_damage":{}
    });
    json!({
        "schema_version": 7,
        "mechanism_version": 105,
        "id": "contract-test",
        "source_commit": "test",
        "purpose": "mechanism regression",
        "step_ms": 30,
        "geometry": {"min":[0,0], "max":[128,128], "walls":[]},
        "spawn": [20,20],
        "extraction_point": [108,108],
        "target": {"id":"target", "position":[52,20], "hp":100, "atk":10, "armor":0,
                   "speed":0, "radius":2, "aggro_range":64, "attack_range":2,
                   "interval_ms":1000, "drop_quantity":0},
        "basic": {"mechanism":"projectile", "windup_ms":30, "interval_ms":300,
                  "range":80, "projectile_speed":300},
        "skill": {"mechanism":"unit_effect", "id":"skill", "range":80, "cost":20,
                  "windup_ms":30, "recovery_ms":60, "cooldown_ms":300,
                  "damage_bps":10000, "ignore_armor":true, "unavoidable":true},
        "inventory_capacity": 4,
        "stack_limit": 4,
        "item_id": "item",
        "carried_quantity": 1,
        "extraction_unlock_ms": 0,
        "extraction_channel_ms": 90,
        "terminal_ms": 3000,
        "kill_score": 0,
        "extraction_score": 0,
        "cultivation_coefficient": 0,
        "death_multiplier": 1,
        "defender_realm_atk": 10,
        "defense_cap": 0.9,
        "input_capacity": 64,
        "output_capacity": 256,
        "history_capacity": 64,
        "asset_contract": {"entry_cost_asset_id":"entry", "reward_asset_id":"reward"},
        "result_contract": {"extraction":{"result_id":"extract", "policy":"preserve"},
                            "death":{"result_id":"death", "policy":"attrition"}},
        "members": {"capacity":2, "spawns":[[20,20],[20,40]], "deployment_timeout_ms":1500,
                    "publication_queue_capacity":4, "projectile_capacity":8, "area_capacity":2},
        "replication": {"ack_timeout_ms":100, "baseline_ttl_ms":1000,
                        "full_interval_ms":1000, "terminal_grace_ms":200},
        "params":params,
        "pve":pve,
        "combat_rules":combat_rules
    })
}

pub(crate) fn catalog(
    mut input: Value,
    change: impl FnOnce(&mut Value),
) -> std::io::Result<crate::ConfigCatalog> {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "sdk-contract-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    let dir = Scratch(path);
    change(&mut input);
    std::fs::write(
        dir.0.join("input.json"),
        serde_json::to_vec(&input).unwrap(),
    )
    .unwrap();
    let manifest = dir.0.join("catalog.json");
    std::fs::write(
        &manifest,
        br#"{"schema_version":1,"active":{"test":"input.json"},"archives":[]}"#,
    )
    .unwrap();
    crate::ConfigCatalog::load(manifest)
}

pub(crate) fn world(c: &Config) -> World {
    world_with_seats(c, &["a", "b"])
}

pub(crate) fn world_with_seats(c: &Config, ids: &[&str]) -> World {
    let seats: Vec<_> = ids
        .iter()
        .map(|&id| ParticipantSeat {
            participant_id: id.into(),
            authenticated_principal: id.into(),
            connection: Connection {
                id: id.into(),
                generation: 1,
            },
            weapon: None,
            basic_profile: None,
            combat_build: None,
            skills: None,
        })
        .collect();
    let facts = seats
        .iter()
        .map(|seat| ParticipantFacts {
            principal: seat.authenticated_principal.clone(),
            revision: 0,
            hp_points: 100,
            spirit_points: 100,
            atk_points: 10,
            def_points: 0,
            spd_points: 1,
            warehouse_quantity: 4,
        })
        .collect();
    let mut w = World::new(c, &seats, facts, 7);
    for member in &mut w.members {
        member.joined = true;
        member.ready = true;
    }
    w.activate(c, 0, &mut vec![]);
    w
}

fn geometry_input(v: &mut Value) {
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["pve"]["monster_positions"] = json!([[88, 90]]);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["world_geometry"] = json!({
        "boundary":[[0,0],[112,0],[128,16],[128,128],[16,128],[0,112]],
        "blockers":[
            {"id":"gate","vertices":[[47,4],[49,4],[49,60],[47,60]],
             "blocks_movement":true,"blocks_projectiles":true,"blocks_vision":false,
             "active_window":{"start_ms":60,"end_ms":240}},
            {"id":"screen","vertices":[[30.2,48],[30.3,48],[30.3,56],[30.2,56]],
             "blocks_movement":false,"blocks_projectiles":false,"blocks_vision":true}
        ]
    });
}

fn terrain_input(v: &mut Value) {
    geometry_input(v);
    v["pve"]["world_geometry"]["blockers"] = json!([]);
    let mut low = v["pve"]["palette"][0].clone();
    low["maxClimbHeight"] = json!(5);
    let mut high = low.clone();
    high["terrain"] = json!("high");
    high["elevation"] = json!(20);
    high["maxDropHeight"] = json!(20);
    let mut slope = low.clone();
    slope["terrain"] = json!("slope");
    slope["slope"] = json!({"axis":"x","endElevation":20});
    v["pve"]["palette"] = json!([low, high, slope]);
    v["pve"]["cells_rle"] = json!([
        [0, 1],
        [1, 3],
        [0, 1],
        [2, 1],
        [1, 2],
        [0, 1],
        [2, 1],
        [1, 2],
        [0, 1],
        [2, 1],
        [1, 2]
    ]);
}

fn iso_point([x, y]: [f64; 2]) -> [f64; 2] {
    [-100. + (x - y) * 32., 20. + (x + y) * 16.]
}

fn isometric_input(v: &mut Value) {
    let pve = &mut v["pve"];
    pve.as_object_mut().unwrap().remove("status_rules");
    pve["monster_positions"] = json!([]);
    pve["combat"]["cast_chance"] = json!(0);
    pve["width"] = json!(6);
    pve["height"] = json!(6);
    pve["cell_size"] = json!(64);
    pve["isometric_grid"] = json!({"tile_height":32,"origin":[-100,20]});
    let low = pve["palette"][0].clone();
    let mut solid = low.clone();
    solid["walkable"] = json!(false);
    solid["blocksProjectiles"] = json!(true);
    let mut ramp = low.clone();
    ramp["slope"] = json!({"axis":"x","endElevation":8});
    let mut high = low.clone();
    high["terrain"] = json!("high");
    high["elevation"] = json!(8);
    high["dotDamagePerSecond"] = json!(100);
    high["speedMultiplier"] = json!(0.5);
    pve["palette"] = json!([low, solid, ramp, high]);
    pve["cells_rle"] = json!([[0, 8], [1, 1], [0, 13], [2, 1], [3, 1], [0, 12]]);
    v["geometry"]["min"] = json!([-292, 20]);
    v["geometry"]["max"] = json!([92, 212]);
    v["spawn"] = json!(iso_point([1.5, 1.5]));
    v["extraction_point"] = json!(iso_point([1.5, 4.5]));
    v["target"]["position"] = json!(iso_point([3.5, 1.5]));
    v["members"]["spawns"] = json!([iso_point([1.5, 1.5]), iso_point([1.5, 3.5])]);
}

#[test]
fn authoritative_isometric_grid_routes_and_sweeps_using_world_distances() {
    let c = catalog(input(), isometric_input).unwrap().default_config();
    let mut w = world(&c);
    let from = iso_point([1.5, 1.5]);
    let to = iso_point([3.5, 1.5]);
    assert_eq!(w.grid.index(from), Some(7));
    assert_eq!(w.grid.index(to), Some(9));
    assert_eq!(w.grid.index([-f64::MAX, f64::MAX]), None);
    assert!(!w.grid.clear([-290., 25.], 0., 0)); // inside the AABB, outside the diamond
    assert!(w.grid.clear(iso_point([0.05, 1.5]), 0., 0));
    assert!(!w.grid.clear(iso_point([0.05, 1.5]), 2., 0));
    assert!(!w.grid.line(from, to, 2., 0));
    assert!(w.grid.line(from, to, 0., 2));
    let expected = (0.5 - 2. * (1_f64 / 64.).hypot(1. / 32.)) / 2.;
    assert!((w.grid.piercing_wall(from, to, 2.).unwrap() - expected).abs() < 1e-10);
    assert!(w.grid.walk_until_blocked(from, to, 2.)[0] < iso_point([2., 1.5])[0]);
    let outward = iso_point([-0.5, 1.5]);
    assert!((w.grid.piercing_wall(from, outward, 0.).unwrap() - 0.75).abs() < 1e-10);

    w.action(0, &Action::Navigate { destination: to }, "route", &c, 0)
        .unwrap();
    assert!(w.members[0].path.len() > 1);
    assert_eq!(w.members[0].path.last(), Some(&to));
    for t in (30..=1500).step_by(30) {
        w.tick(&c, t);
        assert!(w.grid.clear(w.members[0].position, 2., 0));
    }
    assert!(distance(w.members[0].position, from) > 30.);

    let high = iso_point([5.5, 3.5]);
    assert_eq!(w.grid.speed(high), 0.5);
    assert!(w.grid.line(iso_point([3.5, 3.5]), high, 2., 0));
    assert!(!w.grid.line(iso_point([5.5, 2.5]), high, 2., 0));
    assert!(w.grid.line(iso_point([5.5, 2.5]), high, 0., 1));
    assert!(w.check_ground_bounds(high).is_ok());
    assert_eq!(
        w.check_ground_bounds([-290., 25.]),
        Err(Error::InvalidTarget)
    );
    w.members[1].position = high;
    let hp = w.members[1].hp;
    w.tick(&c, 1530);
    assert_eq!(w.members[1].hp, hp - 3);
}

#[test]
fn isometric_config_rejects_mismatched_bounds_and_invalid_tiles() {
    for (change, error) in [
        (0, "CONFIG_MAP_GEOMETRY"),
        (1, "CONFIG_ISOMETRIC_GRID"),
        (2, "CONFIG_MAP_POINT_OR_PATH"),
    ] {
        let result = catalog(input(), |v| {
            isometric_input(v);
            match change {
                0 => v["geometry"]["min"][0] = json!(-293),
                1 => v["pve"]["isometric_grid"]["tile_height"] = json!(0),
                _ => {
                    v["spawn"] = json!([-290, 25]);
                    v["members"]["spawns"][0] = v["spawn"].clone();
                }
            }
        });
        assert_eq!(result.err().unwrap().to_string(), error);
    }
}

#[test]
fn terrain_rejects_advanced_values_without_geometry_and_checks_bounds() {
    for (field, value) in [
        ("elevation", json!(-1)),
        ("maxClimbHeight", json!(10001)),
        ("maxDropHeight", json!(-1)),
        ("dotDamagePerSecond", json!(0.0001)),
        ("slope", json!({"axis":"x","endElevation":10001})),
    ] {
        let error = catalog(input(), |v| {
            terrain_input(v);
            v["pve"]["palette"][0][field] = value;
        })
        .err()
        .unwrap();
        assert_eq!(error.to_string(), "CONFIG_UNSUPPORTED_TERRAIN");
    }
    assert!(
        catalog(input(), |v| {
            terrain_input(v);
            v["pve"].as_object_mut().unwrap().remove("world_geometry");
        })
        .is_err()
    );
    assert!(
        catalog(input(), |v| {
            terrain_input(v);
            v["pve"]["palette"][0]["slope"] = Value::Null;
        })
        .is_err()
    );
    assert!(
        catalog(input(), |v| {
            terrain_input(v);
            v["pve"]["palette"][0]["dotDamagePerSecond"] = json!(50);
            v["output_capacity"] = json!(128);
        })
        .err()
        .unwrap()
        .to_string()
        .contains("CONFIG_TERRAIN_OUTPUT_BUDGET")
    );
}

#[test]
fn terrain_height_routes_via_slope_and_clips_forced_movement_at_the_first_seam() {
    let c = catalog(input(), terrain_input).unwrap().default_config();
    let mut w = world(&c);
    let low = [20., 20.];
    let high = [80., 20.];
    assert!(!w.grid.line(low, high, 2., 0));
    assert!(w.grid.line(high, low, 2., 0));
    assert!(w.grid.line(low, high, 0., 2));
    assert!(w.grid.piercing_wall(low, high, 1.).is_none());
    assert!(w.grid.line([20., 48.], [80., 48.], 2., 0));
    assert!(!w.grid.line([48., 48.], [48., 20.], 2., 0));
    assert!(!w.grid.line(low, [44., 44.], 2., 0));
    let clipped = w.grid.walk_until_blocked(low, [140., 20.], 2.);
    assert!(clipped[0] < 32. && clipped[0] > 31.999);

    w.action(
        0,
        &Action::Move {
            direction: [1., 0.],
        },
        "direct",
        &c,
        0,
    )
    .unwrap();
    for step in 1..=15 {
        w.tick(&c, step * 30);
    }
    assert!((30.0..32.0).contains(&w.members[0].position[0]));
    w.action(0, &Action::Navigate { destination: high }, "ramp", &c, 450)
        .unwrap();
    assert!(w.members[0].path.iter().any(|point| point[1] >= 32.));
    for step in 16..=92 {
        w.tick(&c, step * 30);
    }
    assert!(distance(w.members[0].position, high) < 1e-6);
}

#[test]
fn terrain_height_routes_npc_return_without_snapping_through_a_cliff() {
    let c = catalog(input(), |v| {
        terrain_input(v);
        v["pve"]["palette"][1]["maxDropHeight"] = json!(19);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    for i in 0..w.members.len() {
        w.protect(i, 0, &mut vec![]);
    }
    let npc = &mut w.npcs[0];
    npc.home = [27., 20.];
    npc.position = [33., 20.];
    npc.hp = 50;
    npc.returning = true;
    w.tick(&c, 30);
    assert!(w.npcs[0].position[0] >= 32.);
    assert_eq!(w.npcs[0].hp, 50);
    assert!(w.npcs[0].path.iter().any(|point| point[1] >= 32.));
    for step in 2..=90 {
        w.tick(&c, step * 30);
    }
    assert_eq!(w.npcs[0].position, w.npcs[0].home);
    assert!(!w.npcs[0].returning);
    assert_eq!(w.npcs[0].hp, 100);
}

#[test]
fn terrain_damage_accumulates_only_during_exposure_and_does_not_credit_npc_death() {
    let c = catalog(input(), |v| {
        terrain_input(v);
        v["pve"]["palette"][0]["dotDamagePerSecond"] = json!(50);
        v["pve"]["monster_positions"] = json!([[20, 80]]);
        v["members"]["spawns"][1] = json!([88, 40]);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    w.npcs[0].hp = 2;
    let first = w.tick(&c, 30);
    assert_eq!(
        (w.members[0].hp, w.members[1].hp, w.npcs[0].hp),
        (99, 100, 1)
    );
    assert_eq!(
        first
            .iter()
            .filter(|(_, e)| matches!(e, EventV4::TerrainDamage { amount: 1, .. }))
            .count(),
        2
    );
    let second = w.tick(&c, 60);
    assert_eq!(
        (w.members[0].hp, w.members[1].hp, w.npcs[0].hp),
        (97, 100, 0)
    );
    assert!(!second.iter().any(|(_, e)| matches!(
        e,
        EventV4::MonsterDefeated { .. } | EventV4::ResourceSpawned { .. }
    )));
    assert!(w.members[0].tally.is_empty());

    w.tick(&c, 90);
    w.members[0].position = [40., 40.];
    w.tick(&c, 120);
    w.members[0].position = [20., 20.];
    w.tick(&c, 150);
    assert_eq!(w.members[0].hp, 95);
    w.protect(0, 150, &mut vec![]);
    w.tick(&c, 180);
    assert_eq!(w.members[0].hp, 95);
    w.members[0].protected = false;
    w.members[0].ready = true;
    w.tick(&c, 210);
    assert_eq!(w.members[0].hp, 94);
}

#[test]
fn terrain_damage_obeys_immunity_reduction_shield_and_death_guard_in_order() {
    let c = catalog(input(), |v| {
        let rules = v["pve"]["status_rules"].clone();
        terrain_input(v);
        v["pve"]["status_rules"] = rules;
        v["pve"]["palette"][0]["dotDamagePerSecond"] = json!(1000);
        v["members"]["spawns"][1] = json!([88, 40]);
        v["pve"]["status_definitions"] = json!([
            {"id":"shield","mechanism":"absorption_shield","duration_ms":300,
             "shield_bps":2000,"duplicate_policy":"refresh","source_scoped":true},
            {"id":"reduction","mechanism":"timed_attribute_modifier","attribute":"damage_reduction",
             "modifier_bps":2000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true},
            {"id":"immune","mechanism":"timed_control","attribute":"damage_immunity",
             "duration_ms":60,"duplicate_policy":"refresh","source_scoped":true}
        ]);
        v["combat_rules"]["death_guards"] = json!({"test":{"retain_hp":1,"heal_max_hp_bps":0,"self_statuses":[]}});
    }).unwrap().default_config();
    let mut w = world(&c);
    let target = w.members[0].reference.entity.clone();
    w.apply_owned_statuses(
        Owner::Member(0),
        (target.clone(), 100),
        &["shield".into(), "reduction".into(), "immune".into()],
        &c,
        0,
        &mut vec![],
    );
    assert!(
        !w.tick(&c, 30)
            .iter()
            .any(|(_, e)| matches!(e, EventV4::TerrainDamage { .. }))
    );
    assert!(w.tick(&c, 60).iter().any(|(_, e)| matches!(e,
        EventV4::TerrainDamage { target: entity, amount: 4, absorbed: 20, hp: 96, .. } if entity == &target
    )));
    w.members[0].hp = 5;
    w.members[0].extract_end = Some(90);
    let saved = w.tick(&c, 90);
    assert_eq!(w.members[0].hp, 1);
    assert!(w.members[0].extract_end.is_none());
    assert!(saved.iter().any(|(recipient, e)| *recipient == Some(0)
        && matches!(
            e,
            EventV4::Private {
                event: Event::TerrainDeathGuardActivated {
                    prevented_hp_loss: 1,
                    hp: 1,
                    ..
                }
            }
        )));
    w.tick(&c, 120);
    assert_eq!(w.members[0].desired, Some(Outcome::Death));
    assert!(w.members[1].eligible());
}

#[test]
fn terrain_damage_reduction_preserves_sub_hp_exposure_across_steps() {
    let c = catalog(input(), |v| {
        let rules = v["pve"]["status_rules"].clone();
        terrain_input(v);
        v["pve"]["status_rules"] = rules;
        v["pve"]["palette"][0]["dotDamagePerSecond"] = json!(50);
        v["pve"]["status_definitions"] = json!([
            {"id":"reduction","mechanism":"timed_attribute_modifier","attribute":"damage_reduction",
             "modifier_bps":5000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}
        ]);
    }).unwrap().default_config();
    let mut w = world(&c);
    let target = w.members[0].reference.entity.clone();
    w.apply_owned_statuses(
        Owner::Member(0),
        (target, 100),
        &["reduction".into()],
        &c,
        0,
        &mut vec![],
    );
    for step in 1..=4 {
        w.tick(&c, step * 30);
    }
    assert_eq!(w.members[0].hp, 97);
    assert_eq!(w.members[1].hp, 94);
}

#[test]
fn terrain_damage_retires_the_summon_without_retiring_its_owner() {
    let c = catalog(input(), |v| {
        terrain_input(v);
        summon_input(v, 360);
        v["pve"]["palette"][0]["dotDamagePerSecond"] = json!(50);
        v["members"]["spawns"][1] = json!([88, 40]);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    cast_summon(&mut w, &c, 0, 0, "summon-on-terrain");
    w.tick(&c, 30);
    let summon = w.summons[0].entity.clone();
    w.summons[0].hp = 1;
    let events = w.tick(&c, 60);
    assert!(events.iter().any(|(_, event)| matches!(event,
        EventV4::TerrainDamage { target, amount: 1, hp: 0, .. } if *target == summon
    )));
    assert!(!w.summon_alive(0));
    assert!(w.members[0].eligible());
    assert!(
        w.snapshot(0, &c, Phase::Active, 60)
            .monsters
            .iter()
            .all(|n| n.entity != summon)
    );
    w.tick(&c, 90);
    assert!(w.summons.is_empty());
}

#[test]
fn world_geometry_rejects_invalid_polygons_windows_and_spawns() {
    for (field, index, value) in [
        ("vertices", 0, json!([[47, 4], [49, 60], [49, 4], [47, 60]])),
        ("active_window", 0, json!({"start_ms":240,"end_ms":240})),
        ("id", 1, json!("gate")),
    ] {
        let error = catalog(input(), |v| {
            geometry_input(v);
            v["pve"]["world_geometry"]["blockers"][index][field] = value;
        })
        .err()
        .unwrap();
        assert_eq!(error.to_string(), "CONFIG_WORLD_GEOMETRY");
    }
    let error = catalog(input(), |v| {
        geometry_input(v);
        v["spawn"] = json!([1, 125]);
        v["members"]["spawns"][0] = json!([1, 125]);
    })
    .err()
    .unwrap();
    assert_eq!(error.to_string(), "CONFIG_MAP_POINT_OR_PATH");
}

#[test]
fn world_geometry_uses_one_overlay_for_footprints_routes_and_flights() {
    let c = catalog(input(), geometry_input).unwrap().default_config();
    let mut w = world(&c);
    let from = [20., 20.];
    let to = [80., 20.];
    assert!(!w.grid.clear([3., 119.], 2., 0));
    assert!(w.grid.clear([5., 114.], 2., 0));
    assert!(w.grid.line(from, to, 2., 0));
    assert!(!w.grid.line([20., 52.], [40., 52.], 0., 2));
    assert!(w.grid.line([20., 52.], [40., 52.], 2., 0));
    assert!(w.grid.piercing_wall([20., 52.], [40., 52.], 1.).is_none());

    w.members[0].position = [48., 20.];
    w.action(
        0,
        &Action::Move {
            direction: [-1., 0.],
        },
        "clear-gate",
        &c,
        30,
    )
    .unwrap();
    let destination = [80., 40.];
    w.action(1, &Action::Navigate { destination }, "cross-gate", &c, 30)
        .unwrap();
    assert_eq!(w.members[1].path, [destination]);
    w.tick(&c, 60);
    w.tick(&c, 90);
    let before = w.snapshot(1, &c, Phase::Active, 90);
    assert_eq!(
        before.active_blockers.as_deref(),
        Some(&["screen".to_owned()][..])
    );
    w.tick(&c, 120);
    let closed = w.snapshot(1, &c, Phase::Active, 120);
    assert_eq!(
        closed.active_blockers.as_deref(),
        Some(&["gate".to_owned(), "screen".to_owned()][..])
    );
    assert!(!w.grid.line(from, to, 2., 0));
    assert!(w.grid.line(from, to, 0., 2));
    assert!(!w.grid.clear([48., 20.], 2., 0));
    assert!((w.grid.piercing_wall(from, to, 1.).unwrap() - 26. / 60.).abs() < 1e-10);
    let clipped = w.grid.walk_until_blocked(from, to, 2.);
    assert!(clipped[0] < 45. && clipped[0] > 44.999);
    let path = &w.members[1].path;
    assert!(path.len() > 1);
    assert_eq!(path.last(), Some(&destination));
    assert!(
        std::iter::once(&w.members[1].position)
            .chain(path)
            .zip(path)
            .all(|(a, b)| w.grid.line(*a, *b, 2., 0))
    );

    let key = MatchKey {
        id: "geometry".into(),
        incarnation: 1,
    };
    let mut reader = crate::replication::ReplicaV6::new(key.clone(), "b".into());
    let mut frame = PublicationEnvelopeV6 {
        key,
        publication_id: "before".into(),
        recipient_id: "b".into(),
        connection: Connection {
            id: "b".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            reason: FullReason::Initial,
            snapshot: Box::new(before.clone()),
        },
    };
    reader.apply(&frame).unwrap();
    frame.publication_id = "closed".into();
    frame.revision = 2;
    frame.payload = PublicationPayloadV6::Delta {
        base_publication_id: "before".into(),
        base_revision: 1,
        delta: Box::new(crate::replication::diff(&before, &closed)),
    };
    assert_eq!(
        reader.apply(&frame).unwrap().active_blockers,
        closed.active_blockers
    );
    w.tick(&c, 240);
    assert!(w.grid.line(from, to, 2., 0));
    assert_eq!(w.members[1].path, [destination]);
}

fn summon_input(v: &mut Value, duration: u64) {
    v["skill"] = json!({
        "id":"summon-skill", "mechanism":"self_status", "cost":10,
        "windup_ms":30, "recovery_ms":0, "cooldown_ms":30
    });
    let pve = &mut v["pve"];
    let mut ally = json!({
        "name":"synthetic ally", "mechanism":"melee_projectile_heal",
        "stats":pve["stats"], "combat":pve["combat"], "drop_table":pve["drop_table"],
        "leash":pve["leash"], "help_range":pve["help_range"], "memory_range":pve["memory_range"],
        "kill_credit":true, "drop_orb":true
    });
    ally["stats"]["hp"] = json!(30);
    ally["stats"]["atk"] = json!(7);
    ally["combat"]["melee_range"] = json!(40);
    ally["combat"]["attack_interval_ms"] = json!(60);
    pve["additional_templates"] = json!({"ally":ally});
    pve["combat"]["cast_chance"] = json!(0);
    pve.as_object_mut().unwrap().remove("status_rules");
    v["combat_rules"]["member_summons"] = json!({
        "max_active_per_member":1,
        "skills":{"summon-skill":{"template_id":"ally","count":1,"duration_ms":duration}}
    });
}

fn cast_summon(w: &mut World, c: &Config, member: usize, at: u64, id: &str) {
    w.action(
        member,
        &Action::CastSelf {
            skill_id: "summon-skill".into(),
        },
        id,
        c,
        at,
    )
    .unwrap();
}

fn summon_copy_input(v: &mut Value, mechanism: &str) {
    summon_input(v, 360);
    v["output_capacity"] = json!(512);
    v["members"]["area_capacity"] = json!(3);
    v["params"]["movement"]["base"] = json!(0);
    v["pve"]["monster_positions"] = json!([[52, 20], [58, 20]]);
    v["pve"]["stats"]["hp"] = json!(500);
    v["pve"]["combat"]["aggro_range"] = json!(120);
    v["pve"]["memory_range"] = json!(120);
    v["pve"]["additional_templates"]["ally"]["combat"]["melee_range"] = json!(2);
    let mut echo = json!({
        "id":"echo","mechanism":mechanism,"cost":80,"range":64,"windup_ms":30,
        "recovery_ms":30,"cooldown_ms":120,"damage_bps":10000
    });
    match mechanism {
        "projectile" | "unit_projectile" => {
            echo["radius"] = json!(1);
            echo["speed"] = json!(600);
        }
        "ground_circle" => {
            echo["radius"] = json!(9);
            echo["warning_ms"] = json!(0);
        }
        _ => {}
    }
    v["additional_skills"] = json!([
        echo,
        {"id":"trap","mechanism":"ground_circle","cost":1,"range":80,"radius":8,
            "windup_ms":30,"recovery_ms":0,"cooldown_ms":300,"damage_bps":10000,"warning_ms":30},
        {"id":"anchor","mechanism":"ground_circle","cost":1,"range":80,"radius":2,
            "windup_ms":30,"recovery_ms":0,"cooldown_ms":300,"damage_bps":0,"warning_ms":0},
        {"id":"field","mechanism":"ground_field","cost":2,"range":80,"radius":8,"windup_ms":30,
            "recovery_ms":0,"cooldown_ms":300,"damage_bps":10000,"warning_ms":0,"duration_ms":150,
            "tick_interval_ms":30,"contact_policy":{"max_targets_per_tick":1,"max_total_hits":4,"repeat_interval_ms":30}}
    ]);
    let rules = &mut v["combat_rules"];
    rules["member_ground_traps"] = json!({"trap":{"duration_ms":180,"contact_policy":{
        "max_targets_per_tick":1,"max_total_hits":2,"repeat_interval_ms":0
    }}});
    rules["member_cast_anchors"] = json!({"anchor":{"duration_ms":300,"max_use_distance":80,
        "consumer_skill_ids":["field"],"following_field_skill_ids":["field"],
        "recast":{"window_ms":180,"max_count":2,"spirit_cost":1,"behaviors":["relocate","end"]}}});
    rules["member_summons"] = json!({
        "max_active_per_member":2,
        "skills":{"summon-skill":{"template_id":"ally","count":2,"duration_ms":360,
           "inherit_attributes_bps":5000,
           "skill_copy":{"skill_ids":["echo"],"shared_interval_ms":60,"damage_bps":5000,"max_circle_targets":1},
           "death_burst":{"radius":48,"max_targets":1,"damage_bps":20000}
        }}
    });
}

#[test]
fn summon_copy_whitelist_and_shared_interval_are_authoritative() {
    for ids in [
        json!(["summon-skill"]),
        json!(["trap"]),
        json!(["anchor"]),
        json!(["missing"]),
        json!(["echo", "echo"]),
    ] {
        let Err(error) = catalog(input(), |v| {
            summon_copy_input(v, "unit_effect");
            v["combat_rules"]["member_summons"]["skills"]["summon-skill"]["skill_copy"]["skill_ids"] =
                ids;
        }) else {
            panic!("accepted unsupported summon copy");
        };
        assert_eq!(error.to_string(), "CONFIG_MEMBER_SUMMON_COPY");
    }
    for mechanism in ["unit_effect", "ground_circle"] {
        let c = catalog(input(), |v| summon_copy_input(v, mechanism))
            .unwrap()
            .default_config();
        let mut w = world(&c);
        w.members[0].facts.atk_points = 40;
        w.members[0].facts.def_points = 12;
        w.members[0].facts.hp_points = 120;
        for i in 0..2 {
            cast_summon(&mut w, &c, i, 0, "summon");
        }
        w.tick(&c, 30);
        assert_eq!(
            (
                w.summons[0].hp,
                w.summons[0].base_atk,
                w.summons[0].base_def
            ),
            (60, 20, 6)
        );
        let origins: Vec<_> = w.summons.iter().map(|s| s.entity.clone()).collect();
        w.members[0].facts.atk_points = 100;
        w.tick(&c, 60);
        for npc in &mut w.npcs {
            npc.position = [112., 20.];
        }
        let copies = |events: Events| {
            events
                .into_iter()
                .filter_map(|(_, e)| match e {
                    EventV4::SummonSkillCopied {
                        source, skill_id, ..
                    } => {
                        assert_eq!(skill_id, "echo");
                        Some(source)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert!(copies(w.tick(&c, 90)).is_empty());
        assert!(w.members.iter().all(|m| m.summon_copy_ready == (0, 0)));
        w.npcs[0].position = [52., 20.];
        w.npcs[1].position = [58., 20.];
        w.tick(&c, 120);
        assert_eq!(
            copies(w.tick(&c, 150)),
            [origins[0].clone(), origins[2].clone()]
        );
        assert_eq!(w.members[0].summon_copy_ready, (1, 210));
        assert_eq!(w.members[1].summon_copy_ready, (1, 210));
        assert_eq!((w.npcs[0].hp, w.npcs[1].hp), (488, 500));
        assert!(copies(w.tick(&c, 180)).is_empty());
        w.tick(&c, 210);
        assert_eq!(
            copies(w.tick(&c, 240)),
            [origins[1].clone(), origins[3].clone()]
        );
        assert_eq!((w.npcs[0].hp, w.npcs[1].hp), (476, 500));
        for member in &w.members {
            assert_eq!(member.spirit, 90);
            assert!(!member.cooldowns.contains_key("echo"));
        }
    }
}

#[test]
fn summon_flights_compose_with_traps_anchors_and_death_cleanup() {
    let c = catalog(input(), |v| summon_copy_input(v, "unit_projectile"))
        .unwrap()
        .default_config();
    let mut w = world(&c);
    for i in 0..2 {
        cast_summon(&mut w, &c, i, 0, "summon");
    }
    w.tick(&c, 30);
    for (i, id, position) in [(0, "trap", [52., 20.]), (1, "anchor", [52., 20.])] {
        w.action(
            i,
            &Action::CastGround {
                skill_id: id.into(),
                position,
            },
            id,
            &c,
            30,
        )
        .unwrap();
    }
    w.tick(&c, 60);
    let (trap, anchor) = (
        w.areas.iter().find(|p| p.is_trap()).unwrap().view().id,
        w.areas.iter().find(|p| p.is_anchor()).unwrap().view().id,
    );
    assert_eq!(
        w.action(
            0,
            &Action::CastAtAnchor {
                skill_id: "field".into(),
                anchor_id: anchor.clone(),
                direction: None
            },
            "illegal",
            &c,
            60
        )
        .unwrap_err(),
        Error::InvalidTarget
    );
    w.action(
        1,
        &Action::CastAtAnchor {
            skill_id: "field".into(),
            anchor_id: anchor.clone(),
            direction: None,
        },
        "field",
        &c,
        60,
    )
    .unwrap();
    let started = w.tick(&c, 90);
    assert!(
        started
            .iter()
            .any(|(_, e)| matches!(e,EventV4::AreaEntered{area_id,..} if *area_id==trap))
    );
    assert_eq!(w.shots.len(), 2);
    let (a, b) = (w.summons[0].entity.clone(), w.summons[2].entity.clone());
    let shot_a = w
        .shots
        .iter()
        .find(|s| s.view.source == a)
        .unwrap()
        .view
        .id
        .clone();
    let shot_b = w
        .shots
        .iter()
        .find(|s| s.view.source == b)
        .unwrap()
        .view
        .id
        .clone();
    let enemy = w.npcs[0].entity.clone();
    let before = w.npcs[0].hp;
    let mut events = vec![];
    w.resolve_damage(
        combat::DamageIntent {
            ignore_armor: false,
            source: enemy.clone(),
            target: a.clone(),
            raw: 1000,
            direct: false,
        },
        &c,
        90,
        &mut events,
    );
    assert_eq!(w.npcs[0].hp, before - 20);
    assert_eq!(w.npcs[1].hp, 500);
    assert!(events.iter().any(|(_,e)| matches!(e,EventV4::ProjectileEnded{projectile_id,reason:Some(ProjectileEndReasonV4::SourceInvalid)} if *projectile_id==shot_a)));
    assert_eq!(
        events
            .iter()
            .filter(|(_, e)| matches!(e,EventV4::Damage{source,..} if *source==a))
            .count(),
        1
    );
    let exit = w.action(0, &Action::Leave {}, "exit", &c, 90).unwrap();
    assert!(exit.iter().any(|(_,e)| matches!(e,EventV4::AreaEnded{area_id,reason:AreaEndReason::SourceInvalid} if *area_id==trap)));
    assert!(
        !exit
            .iter()
            .any(|(_, e)| matches!(e, EventV4::Damage { .. }))
    );
    let moved = w
        .action(
            1,
            &Action::RecastAnchor {
                anchor_id: anchor.clone(),
                action_id: "anchor".into(),
                recast_index: 1,
                position: [58., 20.],
            },
            "move-anchor",
            &c,
            90,
        )
        .unwrap();
    assert!(
        moved
            .iter()
            .any(|(_, e)| matches!(e,EventV4::AnchorRelocated{area_id,..} if *area_id==anchor))
    );
    let overlap = w.tick(&c, 120);
    let field = w
        .areas
        .iter()
        .find(|p| p.view().attached_anchor_id.as_deref() == Some(&anchor))
        .unwrap()
        .view();
    assert_eq!(field.center, [58., 20.]);
    assert!(overlap.iter().any(|(_,e)| matches!(e,EventV4::AreaTick{area_id,targets,..} if *area_id==field.id && targets==&[w.npcs[1].entity.clone()])));
    assert_eq!(w.shots[0].view.id, shot_b);
    let arrived = w.tick(&c, 150);
    assert!(arrived.iter().any(|(_,e)| matches!(e,EventV4::Damage{source,target,amount:2,..} if *source==b && *target==enemy)));
    assert!(
        !w.snapshot(0, &c, Phase::Active, 150)
            .projectiles
            .iter()
            .any(|s| s.source == b)
    );
    assert!(
        w.snapshot(1, &c, Phase::Active, 150)
            .monsters
            .iter()
            .any(|s| s.entity == b)
    );
    let expired = w.tick(&c, 390);
    assert!(w.summons.is_empty());
    assert!(
        !expired
            .iter()
            .any(|(_, e)| matches!(e,EventV4::Damage{source,..} if *source==b))
    );
}

#[test]
fn summon_copy_projectile_respects_wall_collision() {
    let c = catalog(input(), |v| {
        summon_copy_input(v, "projectile");
        v["pve"]["monster_positions"] = json!([[84, 20], [90, 20]]);
        v["additional_skills"][0]["range"] = json!(90);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["terrain"] = json!("barrier");
        wall["walkable"] = json!(false);
        wall["blocksProjectiles"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        v["pve"]["cells_rle"] = json!([[0, 1], [1, 1], [0, 14]]);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    cast_summon(&mut w, &c, 0, 0, "summon");
    w.tick(&c, 30);
    w.tick(&c, 60);
    let released = w.tick(&c, 90);
    assert!(
        released
            .iter()
            .any(|(_, e)| matches!(e, EventV4::SummonSkillCopied { .. }))
    );
    let shot = w.shots[0].view.id.clone();
    let impact = w.tick(&c, 120);
    assert!(impact.iter().any(
        |(_, e)| matches!(e,EventV4::ProjectileEnded{projectile_id,..} if *projectile_id==shot)
    ));
    assert!(w.shots.is_empty());
    assert!(w.npcs.iter().all(|n| n.hp == 500));
}

#[test]
fn bounded_summons_use_independent_member_slots_and_real_combat() {
    let c = catalog(input(), |v| {
        summon_input(v, 180);
        v["pve"]["combat"]["attack_interval_ms"] = json!(90);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    cast_summon(&mut w, &c, 0, 0, "cast-a");
    cast_summon(&mut w, &c, 1, 0, "cast-b");
    let created = w.tick(&c, 30);
    assert_eq!(w.summons.len(), 2);
    assert_eq!(
        w.members.iter().map(|m| m.spirit).collect::<Vec<_>>(),
        [90, 90]
    );
    assert!(!created.iter().any(|(_, e)| matches!(e, EventV4::Damage { source, .. } if source == &w.summons[0].entity || source == &w.summons[1].entity)));
    let a = w.summons[0].entity.clone();
    let b = w.summons[1].entity.clone();
    let npc = w.npcs[0].entity.clone();
    assert_ne!(a, b);
    assert_eq!(w.relation(&a, &b), Some(combat::Relation::Friendly));
    assert_eq!(
        w.relation(&a, &w.members[1].reference.entity),
        Some(combat::Relation::Friendly)
    );
    assert_eq!(w.relation(&a, &npc), Some(combat::Relation::Hostile));
    assert!(matches!(
        w.unit_aim(0, &c.skill, &a),
        Err(Error::InvalidTarget)
    ));
    cast_summon(&mut w, &c, 0, 60, "full");
    let fight = w.tick(&c, 60);
    assert_eq!(w.npcs[0].hp, 86);
    assert!(fight.iter().any(|(_, e)| matches!(e, EventV4::Damage{ source, target, amount: 7,.. } if *source == a && *target == npc)));
    assert!(fight.iter().any(|(_, e)| matches!(e, EventV4::Damage{ source, target, amount: 7,.. } if *source == b && *target == npc)));
    w.members[0].position = [92., 20.];
    w.members[1].position = [92., 40.];
    w.npcs[0].position = [22., 20.];
    let rejected = w.tick(&c, 90);
    assert!(rejected.iter().any(|(recipient, e)| *recipient == Some(0)
        && matches!(e, EventV4::Private { event: Event::ActionRejected{action_id, reason:Error::Capacity}, .. } if action_id == "full")));
    assert_eq!(w.members[0].spirit, 90);
    assert_eq!(w.members[0].cooldowns["summon-skill"], 60);
    assert!(rejected.iter().any(|(_, e)| matches!(e, EventV4::Damage{ source, target, amount: 10, hp:20 } if *source == npc && *target == a)), "events {rejected:?}, NPC position {:?}, target {:?}, ready {}", w.npcs[0].position, w.npcs[0].target, w.npcs[0].next_attack);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&a));

    w.npcs[0].hp = 5;
    let defeated = w.tick(&c, 120);
    assert!(defeated.iter().any(|(_, e)| matches!(e, EventV4::MonsterDefeated{ entity, credited_to } if *entity == npc && *credited_to == w.members[0].reference.entity)));
    assert_eq!(w.members[0].tally["npc"], 1);
    assert!(w.members[1].tally.is_empty());

    let mut events = vec![];
    w.terminal_intent(0, Outcome::Death, &mut events);
    assert!(
        w.snapshot(1, &c, Phase::Active, 120)
            .monsters
            .iter()
            .any(|m| m.entity == b)
    );
    assert!(
        !w.snapshot(1, &c, Phase::Active, 120)
            .monsters
            .iter()
            .any(|m| m.entity == a)
    );
    assert!(
        !w.snapshot(0, &c, Phase::Active, 120)
            .monsters
            .iter()
            .any(|m| m.summon.is_some())
    );
    w.tick(&c, 210);
    assert!(w.summons.is_empty());
    assert!(
        !w.snapshot(1, &c, Phase::Active, 210)
            .monsters
            .iter()
            .any(|m| m.summon.is_some())
    );
}

#[test]
fn summon_projectile_status_and_replication_follow_owner_lifetime() {
    let c = catalog(input(), |v| {
        let status_rules = v["pve"]["status_rules"].clone();
        summon_input(v, 300);
        v["pve"]["status_rules"] = status_rules;
        v["pve"]["status_definitions"] = json!([{
            "id":"burn","mechanism":"periodic_damage","damage_bps":10000,"tick_interval_ms":30,
            "duration_ms":150,"duplicate_policy":"refresh","source_scoped":true
        }]);
        v["pve"]["combat"]["cast_chance"] = json!(1);
        v["pve"]["combat"]["projectile"]["speed"] = json!(1000);
        v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!(["burn"]);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    for i in 0..2 {
        cast_summon(&mut w, &c, i, 0, "summon");
    }
    w.tick(&c, 30);
    let ally = w.summons[0].entity.clone();
    let other = w.summons[1].entity.clone();
    let enemy = w.npcs[0].entity.clone();
    w.members[0].position = [10., 20.];
    w.members[1].position = [10., 40.];
    w.tick(&c, 60);
    let impact = w.tick(&c, 90);
    assert!(impact.iter().any(|(_, e)| matches!(e, EventV4::Damage{source,target,amount:10,hp:20} if *source == enemy && *target == ally)));
    assert!(w.statuses.entries().iter().any(|s| s.target.id == ally.id));
    let own = w.snapshot(0, &c, Phase::Active, 90);
    let peer = w.snapshot(1, &c, Phase::Active, 90);
    let status_count = |s: &SnapshotV4| {
        s.monsters
            .iter()
            .find(|m| m.entity == ally)
            .unwrap()
            .statuses
            .as_ref()
            .unwrap()
            .len()
    };
    assert_eq!(status_count(&own), 1);
    assert_eq!(status_count(&peer), 0);

    let key = MatchKey {
        id: "summon-test".into(),
        incarnation: 1,
    };
    let mut replica = crate::replication::ReplicaV6::new(key.clone(), "a".into());
    let mut frame = PublicationEnvelopeV6 {
        key,
        publication_id: "full".into(),
        recipient_id: "a".into(),
        connection: Connection {
            id: "first".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            reason: FullReason::Initial,
            snapshot: Box::new(own.clone()),
        },
    };
    assert_eq!(replica.apply(&frame).unwrap(), &own);

    w.protect(0, 90, &mut vec![]);
    let protected = w.tick(&c, 120);
    assert!(protected.iter().any(|(recipient,e)| *recipient == Some(0) &&
        matches!(e,EventV4::Private {event:Event::PeriodicTick {target,outcome:PeriodicTickOutcome::Protected,..},..} if target.id == ally.id)));
    assert_eq!(w.summons[0].hp, 20);
    w.members[0].protected = false;
    w.members[0].ready = true;
    w.tick(&c, 150);
    let before_death = w.snapshot(0, &c, Phase::Active, 150);
    assert_eq!(w.summons[0].hp, 10);
    frame.publication_id = "reconnect".into();
    frame.connection = Connection {
        id: "second".into(),
        generation: 2,
    };
    frame.stream_epoch = 2;
    frame.revision = 2;
    frame.payload = PublicationPayloadV6::Full {
        reason: FullReason::Lifecycle,
        snapshot: Box::new(before_death.clone()),
    };
    assert_eq!(replica.apply(&frame).unwrap(), &before_death);

    let death = w.tick(&c, 180);
    assert!(death.iter().any(|(_,e)| matches!(e,EventV4::Damage{source,target,hp:0,..} if *source == enemy && *target == ally)));
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.target.id != ally.id && s.source.id != ally.id)
    );
    let after = w.snapshot(0, &c, Phase::Active, 180);
    assert!(
        after
            .monsters
            .iter()
            .all(|m| m.entity != ally && m.target.as_ref() != Some(&ally))
    );
    assert!(after.monsters.iter().any(|m| m.entity == other));
    frame.publication_id = "remove".into();
    frame.revision = 3;
    frame.payload = PublicationPayloadV6::Delta {
        base_publication_id: "reconnect".into(),
        base_revision: 2,
        delta: Box::new(crate::replication::diff(&before_death, &after)),
    };
    assert_eq!(replica.apply(&frame).unwrap(), &after);
    w.members[1].reference.entity.generation += 1;
    w.tick(&c, 210);
    assert!(w.summons.is_empty());
}

#[test]
fn summon_config_rejects_bad_bindings_and_reserves_output_capacity() {
    let cases = [
        ("template_id", json!("missing")),
        ("duration_ms", json!(30)),
        ("count", json!(2)),
    ];
    for (key, value) in cases {
        let Err(err) = catalog(input(), |v| {
            summon_input(v, 180);
            v["combat_rules"]["member_summons"]["skills"]["summon-skill"][key] = value;
        }) else {
            panic!("invalid summon binding accepted");
        };
        assert_eq!(err.to_string(), "CONFIG_MEMBER_SUMMON_BINDING");
    }
    for capacity in [151, 152] {
        let result = catalog(input(), |v| {
            summon_input(v, 180);
            v["output_capacity"] = json!(capacity);
            v["skill"]["hp_cost"] = json!({"mode":"fixed","value":7,"min_hp_after_cost":1});
            v["pve"]["additional_templates"]["ally"]["combat"]["radius"] = json!(4);
        });
        if capacity == 152 {
            let c = result.unwrap().default_config();
            let mut w = world(&c);
            cast_summon(&mut w, &c, 0, 0, "blocked");
            w.members[0].position = [3., 20.];
            let events = w.tick(&c, 30);
            assert!(events.iter().any(|(i, e)| *i == Some(0)
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionRejected {
                            reason: Error::InvalidTarget,
                            ..
                        },
                        ..
                    }
                )));
            assert!(w.summons.is_empty());
            assert_eq!((w.members[0].hp, w.members[0].spirit), (100, 100));
            assert!(w.members[0].cooldowns.is_empty());
        } else {
            let Err(err) = result else {
                panic!("insufficient summon budget accepted");
            };
            assert_eq!(err.to_string(), "CONFIG_MEMBER_SUMMON_OUTPUT_BUDGET");
        }
    }
}

#[test]
fn npc_commits_cost_before_its_own_spirit_cap_changes() {
    for (initial, cost, expected) in [
        (100, 60, 30),
        (100, 70, 30),
        (100, 80, 20),
        (8_000_000, 8_000_000, 0),
    ] {
        let c = catalog(input(), |v| {
            v["resource_limits"] = json!({
                "max_hp":100,"max_spirit":8_000_000,"max_spirit_cost":8_000_000
            });
            v["pve"]["stats"]["spirit"] = json!(initial);
            v["pve"]["status_definitions"] = json!([{
                "id":"lower-cap", "mechanism":"timed_attribute_modifier", "attribute":"max_spirit",
                "modifier_bps":-7000, "duration_ms":300, "duplicate_policy":"refresh",
                "source_scoped":true
            }]);
            let cast = &mut v["pve"]["combat"]["projectile"];
            cast["cost"] = json!(cost);
            cast["on_commit_self_statuses"] = json!(["lower-cap"]);
        })
        .unwrap()
        .default_config();
        let mut w = world(&c);
        assert!(w.tick(&c, 30).is_empty());
        assert!(w.npcs[0].casting.is_some(), "NPC must enter preparation");
        assert_eq!(w.npcs[0].spirit, initial);
        let events = w.tick(&c, 60);
        assert_eq!(w.npcs[0].spirit, expected);
        assert_eq!(w.npcs[0].next_cast, 360);
        assert_eq!(w.shots.len(), 1);
        assert!(
            events
                .iter()
                .any(|(_, event)| matches!(event, EventV4::ProjectileStarted { .. }))
        );
        w.tick(&c, 90);
        assert_eq!(w.npcs[0].spirit, expected);
        assert_eq!(w.shots.len(), 1);

        let mut rejected = world(&c);
        rejected.tick(&c, 30);
        assert!(rejected.npcs[0].casting.is_some());
        rejected.npcs[0].spirit = cost - 1;
        rejected.tick(&c, 60);
        assert_eq!(rejected.npcs[0].spirit, cost - 1);
        assert_eq!(rejected.npcs[0].next_cast, 0);
        assert!(rejected.shots.is_empty());
        assert!(rejected.statuses.entries().is_empty());
    }
}

#[test]
fn failed_cast_preserves_owned_condition_then_valid_commit_consumes_once() {
    let c = catalog(input(), |v| {
        v["pve"]["combat"]["cast_chance"] = json!(0);
        v["pve"]["status_definitions"] = json!([{
            "id":"token", "mechanism":"timed_attribute_modifier", "attribute":"def",
            "modifier_bps":100, "duration_ms":600, "duplicate_policy":"refresh",
            "source_scoped":true
        }]);
        v["combat_rules"]["triggers"] = json!([{
            "id":"refund", "profiles":["test"], "when":"skill_cost_committed",
            "include_periodic":false, "every":1, "cooldown_ms":0, "priority":1,
            "conditions":{"status":{"subject":"owner","definition_id":"token","at_least":1,"consume":1}},
            "effect":{"kind":"restore_paid_spirit","bps":5000}
        }]);
    }).unwrap().default_config();
    let mut w = world(&c);
    for i in 0..2 {
        let target = w.members[i].reference.entity.clone();
        w.apply_owned_statuses(
            Owner::Member(i),
            (target, 100),
            &["token".into()],
            &c,
            0,
            &mut vec![],
        );
    }
    let action = Action::CastUnit {
        skill_id: c.skill.id.clone(),
        target: World::unit_wire(&w.npcs[0].entity),
    };
    w.action(0, &action, "reject", &c, 0).unwrap();
    w.npcs[0].home = [120., 120.];
    w.npcs[0].position = w.npcs[0].home;
    let rejected = w.tick(&c, 30);
    assert!(rejected.iter().any(|(_, event)| matches!(
        event,
        EventV4::Private {
            event: Event::ActionRejected {
                reason: Error::OutOfRange,
                ..
            }
        }
    )));
    assert_eq!(w.members[0].spirit, 100);
    assert!(w.members[0].cooldowns.is_empty());
    assert_eq!(w.statuses.entries().len(), 2);

    w.npcs[0].home = [52., 20.];
    w.npcs[0].position = w.npcs[0].home;
    w.action(0, &action, "commit", &c, 30).unwrap();
    let committed = w.tick(&c, 60);
    let consumed = committed
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event:
                    Event::StatusConsumed {
                        definition_id,
                        consumed_stacks,
                        ..
                    },
            } if definition_id == "token" => Some((*recipient, *consumed_stacks)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(consumed, [(Some(0), 1)]);
    assert_eq!(w.members[0].spirit, 90);
    assert_eq!(w.members[0].cooldowns[&c.skill.id], 360);
    assert_eq!(w.members[1].spirit, 100);
    assert!(w.members[1].cooldowns.is_empty());
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(w.statuses.entries()[0].source.id, "b");
    w.tick(&c, 90);
    assert_eq!(w.members[0].spirit, 90);
}

#[test]
fn direct_and_periodic_damage_share_reduction_shield_and_once_only_counter() {
    let c = catalog(input(), |v| {
        v["pve"]["combat"]["cast_chance"] = json!(0);
        v["pve"]["stats"]["atk"] = json!(13);
        v["pve"]["status_definitions"] = json!([
            {"id":"shield","mechanism":"absorption_shield","duration_ms":300,
             "shield_bps":2000,"duplicate_policy":"refresh","source_scoped":true},
            {"id":"pulse","mechanism":"periodic_damage","duration_ms":90,
             "tick_interval_ms":30,"damage_bps":10000,"duplicate_policy":"refresh","source_scoped":true}
        ]);
        v["combat_rules"]["shield_records"] = json!({
            "shield":{"basis":"own_absorbed","cap_fixed":100,"cap_power_bps":0,
                      "damage_reduction_bps":2000,"refresh":"preserve","on_depleted":true,
                      "on_expired":false,"on_consumed":false,"counter":{
                          "ratio_bps":10000,"damage_kind":"physical",
                          "ignore_armor":true,"radius":64,"max_targets":1}}
        });
    }).unwrap().default_config();
    let mut w = world(&c);
    let member = w.members[0].reference.entity.clone();
    let attacker = w.npcs[0].entity.clone();
    let mut direct_events = vec![];
    w.apply_owned_statuses(
        Owner::Member(0),
        (member.clone(), 100),
        &["shield".into()],
        &c,
        0,
        &mut direct_events,
    );
    let result = w.resolve_damage_mode(
        combat::DamageIntent {
            source: attacker,
            target: member.clone(),
            raw: 13,
            direct: true,
            ignore_armor: false,
        },
        None,
        false,
        &c,
        0,
        &mut direct_events,
    );
    assert_eq!((result.absorbed, result.hp_damage), (10, 0));
    w.apply_owned_statuses(
        Owner::Npc(0),
        (member, 100),
        &["pulse".into()],
        &c,
        0,
        &mut direct_events,
    );

    let events = w.tick(&c, 30);
    assert!(events.iter().any(|(_, event)| matches!(
        event,
        EventV4::Private {
            event: Event::PeriodicTick {
                raw_damage: 13,
                absorbed: 10,
                hp_damage: 0,
                ..
            }
        }
    )));
    let counter = |events: &Events| {
        events
            .iter()
            .filter_map(|(recipient, event)| match event {
                EventV4::Private {
                    event:
                        Event::ShieldRecordEnded {
                            reason,
                            recorded,
                            raw_counter,
                            targets_hit,
                            ..
                        },
                } => Some((*recipient, *reason, *recorded, *raw_counter, *targets_hit)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert!(counter(&direct_events).is_empty());
    assert_eq!(
        counter(&events),
        [(Some(0), StatusRemovalReason::Depleted, 20, 20, 1)]
    );
    assert_eq!(
        (w.members[0].hp, w.members[1].hp, w.npcs[0].hp),
        (100, 100, 80)
    );
    assert!(counter(&w.tick(&c, 60)).is_empty());
    assert_eq!(
        (w.members[0].hp, w.members[1].hp, w.npcs[0].hp),
        (87, 100, 80)
    );
}

#[test]
fn retaliation_uses_actual_direct_damage_and_live_source() {
    let c = catalog(input(), |v| {
        v["pve"]["combat"]["cast_chance"] = json!(0);
        v["pve"]["status_definitions"] = json!([
            {"id":"window", "mechanism":"timed_attribute_modifier", "attribute":"def",
             "modifier_bps":100, "duration_ms":90, "duplicate_policy":"refresh","source_scoped":true},
            {"id":"reply", "mechanism":"timed_attribute_modifier", "attribute":"atk",
             "modifier_bps":-100, "duration_ms":60, "duplicate_policy":"refresh","source_scoped":true},
            {"id":"aftermath", "mechanism":"timed_attribute_modifier", "attribute":"def",
             "modifier_bps":-100, "duration_ms":60, "duplicate_policy":"refresh","source_scoped":true},
            {"id":"ward", "mechanism":"absorption_shield", "shield_bps":2000,
             "duration_ms":150, "duplicate_policy":"refresh","source_scoped":true}
        ]);
        v["combat_rules"]["status_policies"] = json!({
            "window":{"disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]},
            "reply":{"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]},
            "aftermath":{"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]}
        });
        v["combat_rules"]["status_retaliations"] = json!({
            "window":{"chance_bps":10000,"source_cooldown_ms":60,"max_sources":1,
                      "reply_status_id":"reply","on_expired_status_id":"aftermath"}
        });
    }).unwrap().default_config();
    let mut w = world(&c);
    let holder = w.owner_entity(Owner::Member(0));
    let attacker = w.owner_entity(Owner::Npc(0));
    w.apply_owned_statuses(
        Owner::Member(0),
        (holder, 100),
        &["window".into(), "ward".into()],
        &c,
        0,
        &mut vec![],
    );
    let rule = crate::config::AttackRule {
        damage_kind: crate::config::DamageKind::TrueDamage,
        ..Default::default()
    };
    let direct = |w: &mut World, amount, now| {
        let mut events = vec![];
        let loss = w.direct_contact_damage(
            Owner::Npc(0),
            Owner::Member(0),
            combat::DirectContact {
                origin: None,
                action_id: None,
                rule: Some(&rule),
                ignore_armor: false,
                unavoidable: true,
                dead_source: false,
                source: &attacker,
                raw: amount,
                metal: false,
                hit_statuses: &[],
            },
            &c,
            now,
            &mut events,
        );
        (loss, events)
    };
    let application = |event: &EventV4, id| {
        matches!(
            event, EventV4::Private {event: Event::StatusApplied {status} | Event::StatusRefreshed {status}}
              if status.definition_id == id && status.source.id == "a" && status.target.id == attacker.id
        )
    };
    let (loss, zero) = direct(&mut w, 0, 0);
    assert_eq!(loss, Some(0));
    assert!(!zero.iter().any(|(_, e)| application(e, "reply")));
    let (loss, blocked_by_shield) = direct(&mut w, 10, 30);
    assert_eq!(loss, Some(0));
    let absorbed = blocked_by_shield
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::ShieldAbsorbed { amount: 10, .. }
                }
            )
        })
        .unwrap();
    let reply = blocked_by_shield
        .iter()
        .position(|(recipient, e)| *recipient == Some(0) && application(e, "reply"))
        .unwrap();
    assert!(absorbed < reply);
    assert!(
        !direct(&mut w, 1, 60)
            .1
            .iter()
            .any(|(_, e)| application(e, "reply"))
    );
    let expired = w.tick(&c, 90);
    assert_eq!(
        expired
            .iter()
            .filter(|(_, e)| application(e, "aftermath"))
            .count(),
        1
    );
    assert!(
        !direct(&mut w, 10, 90)
            .1
            .iter()
            .any(|(_, e)| application(e, "reply"))
    );
    assert!(
        !w.tick(&c, 120)
            .iter()
            .any(|(_, e)| application(e, "aftermath"))
    );

    let mut w = world(&c);
    let holder = w.owner_entity(Owner::Member(0));
    w.apply_owned_statuses(
        Owner::Member(0),
        (holder.clone(), 100),
        &["window".into()],
        &c,
        0,
        &mut vec![],
    );
    let mut dot = vec![];
    w.resolve_damage_with_rule(
        combat::DamageIntent {
            source: attacker.clone(),
            target: holder,
            raw: 10,
            direct: false,
            ignore_armor: false,
        },
        Some(&rule),
        &c,
        30,
        &mut dot,
    );
    assert_eq!(w.members[0].hp, 90);
    assert!(!dot.iter().any(|(_, e)| application(e, "reply")));
    w.members[0].protected = true;
    assert_eq!(direct(&mut w, 10, 30).0, None);
    w.members[0].protected = false;
    let (loss, lethal) = direct(&mut w, 90, 60);
    assert_eq!(loss, Some(90));
    assert_eq!(w.members[0].hp, 0);
    assert!(!lethal.iter().any(|(_, e)| application(e, "reply")));
    assert!(
        !w.tick(&c, 90)
            .iter()
            .any(|(_, e)| application(e, "aftermath"))
    );
}

#[test]
fn expiry_tick_runs_its_cleanse_before_end_effects_and_same_step_cast() {
    let c = catalog(input(), |v| {
        v["pve"]["combat"]["cast_chance"] = json!(0);
        v["skill"] = json!({"id":"heal","mechanism":"ally_heal","cost":1,"range":80,
                            "windup_ms":30,"recovery_ms":0,"cooldown_ms":300});
        v["additional_skills"] = json!([{
            "id":"self","mechanism":"self_status","cost":0,"windup_ms":30,
            "recovery_ms":0,"cooldown_ms":300,"on_commit_self_statuses":["plain"]
        }]);
        v["pve"]["status_definitions"] = json!([
            {"id":"hot","mechanism":"periodic_heal","heal_bps":500,"tick_interval_ms":30,
             "duration_ms":90,"duplicate_policy":"refresh","source_scoped":true},
            {"id":"dot","mechanism":"periodic_damage","damage_bps":10000,"tick_interval_ms":30,
             "duration_ms":90,"duplicate_policy":"refresh","source_scoped":true},
            {"id":"plain","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":100,
             "duration_ms":60,"duplicate_policy":"refresh","source_scoped":true},
            {"id":"debuff","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":-100,
             "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true},
            {"id":"debuff-2","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":-100,
             "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}
        ]);
        v["combat_rules"]["ally_recovery_by_mastery"] = json!({"heal":[{
            "minimum_mastery":0,"heal":{"fixed":20,"source_attack_bps":0,"target_max_hp_bps":0},
            "hot":{"status_id":"hot","duration_ms":60,"tick_bps":10000}
        }]});
        v["combat_rules"]["status_policies"] = json!({
            "debuff":{"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]},
            "debuff-2":{"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]}
        });
        let cleanse = json!({"disposition":"harmful","max_count":1,"scope":"all"});
        v["combat_rules"]["status_cleanses"] = json!({"hot":{"on_tick":cleanse,"on_expired":cleanse}});
        let recovery = |fixed| json!({"basis":"none","include_periodic":false,"record_cap":0,"refresh":"reset",
            "maximum_sampling":"live","on_expired":true,"on_consumed":false,"hp":{
                "fixed":fixed,"recorded_bps":0,"max_resource_bps":0,"cap_max_resource_bps":10000
            }});
        v["combat_rules"]["status_windows"] = json!({"plain":recovery(5),"hot":recovery(7)});
    }).unwrap().default_config();
    let setup = || {
        let mut w = world(&c);
        w.members[1].hp = 20;
        let target = w.owner_entity(Owner::Member(1));
        w.action(
            0,
            &Action::CastUnit {
                skill_id: "heal".into(),
                target: World::unit_wire(&target),
            },
            "prepare-heal",
            &c,
            0,
        )
        .unwrap();
        w.tick(&c, 30);
        assert_eq!(w.members[1].hp, 40);
        w.apply_owned_statuses(
            Owner::Member(1),
            (target.clone(), 100),
            &["plain".into()],
            &c,
            30,
            &mut vec![],
        );
        w.apply_owned_statuses(
            Owner::Npc(0),
            (target, 100),
            &["dot".into()],
            &c,
            30,
            &mut vec![],
        );
        w.tick(&c, 60);
        assert_eq!(w.members[1].hp, 50);
        w
    };
    let mut w = setup();
    let target = w.owner_entity(Owner::Member(1));
    w.apply_owned_statuses(
        Owner::Npc(0),
        (target, 100),
        &["debuff".into(), "debuff-2".into()],
        &c,
        60,
        &mut vec![],
    );
    w.action(
        1,
        &Action::CastSelf {
            skill_id: "self".into(),
        },
        "same-step-cast",
        &c,
        60,
    )
    .unwrap();
    let events = w.tick(&c, 90);
    let ordered = events
        .iter()
        .filter_map(|(recipient, e)| match e {
            EventV4::Private {
                event:
                    Event::WindowRecordEnded {
                        definition_id,
                        reason: StatusRemovalReason::Expired,
                        hp_restored: 5,
                        ..
                    },
            } if *recipient == Some(1) && definition_id == "plain" => Some("ordinary expiry"),
            EventV4::Private {
                event:
                    Event::PeriodicTick {
                        hp_damage: 10,
                        scheduled_at_ms: 90,
                        ..
                    },
            } if *recipient == Some(1) => Some("damage"),
            EventV4::Private {
                event:
                    Event::PeriodicHealTick {
                        hp_restored: 20,
                        tick_index: 2,
                        scheduled_at_ms: 90,
                        ..
                    },
            } if *recipient == Some(1) => Some("final heal"),
            EventV4::Private {
                event:
                    Event::StatusCleanseResolved {
                        phase: StatusCleansePhase::Tick,
                        tick_index: Some(2),
                        removed_count: 1,
                        ..
                    },
            } if *recipient == Some(1) => Some("tick cleanse"),
            EventV4::Private {
                event:
                    Event::StatusCleanseResolved {
                        phase: StatusCleansePhase::Expired,
                        removed_count: 1,
                        ..
                    },
            } if *recipient == Some(1) => Some("expiry cleanse"),
            EventV4::Private {
                event:
                    Event::WindowRecordEnded {
                        definition_id,
                        reason: StatusRemovalReason::Expired,
                        hp_restored: 7,
                        ..
                    },
            } if *recipient == Some(1) && definition_id == "hot" => Some("expiry recovery"),
            EventV4::Private {
                event:
                    Event::ActionCommitted {
                        skill_id: Some(id), ..
                    },
            } if *recipient == Some(1) && id == "self" => Some("cast"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ordered,
        [
            "ordinary expiry",
            "damage",
            "final heal",
            "tick cleanse",
            "expiry cleanse",
            "expiry recovery",
            "cast",
        ]
    );
    assert_eq!(w.members[1].hp, 72);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| matches!(s.definition_id.as_str(), "hot" | "debuff" | "debuff-2"))
    );
    assert_eq!(w.members[0].hp, 100);
    assert!(!w.tick(&c, 120).iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::PeriodicHealTick { .. } | Event::StatusCleanseResolved { .. }
        }
    )));

    let mut w = setup();
    w.members[1].hp = 5; // The ordinary expiry restores 5 before the lethal 10-point tick.
    let lethal = w.tick(&c, 90);
    assert_eq!(w.members[1].hp, 0);
    assert_eq!(w.members[1].desired, Some(Outcome::Death));
    assert!(!lethal.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::PeriodicHealTick { .. } | Event::StatusCleanseResolved { .. }
        }
    )));
    assert!(w.statuses.entries().iter().all(|s| s.target.id != "b"));

    let mut w = setup();
    w.members[1].protected = true;
    let protected = w.tick(&c, 90);
    assert!(protected.iter().any(|(recipient, e)| matches!(
        e, EventV4::Private {event: Event::PeriodicHealTick {
            tick_index:2, hp_restored:0, outcome:PeriodicTickOutcome::Protected, ..
        }} if *recipient == Some(1)
    )));
    assert_eq!(w.members[1].hp, 50);
    w.members[1].protected = false;
    assert!(!w.tick(&c, 120).iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::PeriodicHealTick { .. } | Event::StatusCleanseResolved { .. }
        }
    )));
}

#[test]
fn area_budget_uses_native_trap_limit_and_anchor_has_no_contacts() {
    let candidate = |capacity, is_trap| {
        catalog(input(), |v| {
            v["output_capacity"] = json!(capacity);
            v["pve"].as_object_mut().unwrap().remove("status_rules");
            v["pve"]["combat"]["cast_chance"] = json!(0);
            v["pve"]["monster_positions"] = json!([[52, 20], [72, 20], [90, 20]]);
            v["skill"] = json!({
                "id":"area", "mechanism":"ground_circle", "cost":1, "range":80, "radius":48,
                "windup_ms":30, "recovery_ms":0, "cooldown_ms":300, "damage_bps":10000,
                "warning_ms":30
            });
            v["additional_skills"] = json!([
                {"id":"place","mechanism":"ground_circle","cost":1,"range":80,"radius":2,
                 "windup_ms":30,"recovery_ms":0,"cooldown_ms":300,"damage_bps":0,"warning_ms":0},
                {"id":"consumer","mechanism":"projectile","cost":1,"range":80,"radius":1,
                 "windup_ms":30,"recovery_ms":0,"cooldown_ms":300,"damage_bps":10000,"speed":10}
            ]);
            if is_trap {
                v["combat_rules"]["member_ground_traps"] = json!({
                    "area":{"duration_ms":120,"contact_policy":{
                        "max_targets_per_tick":1,"max_total_hits":2,"repeat_interval_ms":0
                    }}
                });
            }
            v["combat_rules"]["member_cast_anchors"] = json!({
                "place":{"duration_ms":120,"max_use_distance":80,"consumer_skill_ids":["consumer"]}
            });
        })
    };
    let c = candidate(152, true).unwrap().default_config();
    for (capacity, is_trap) in [(151, true), (152, false)] {
        let Err(error) = candidate(capacity, is_trap) else {
            panic!("accepted an insufficient area publication budget");
        };
        assert_eq!(error.to_string(), "CONFIG_MEMBER_AREA_OUTPUT_BUDGET");
    }
    let mut w = world(&c);
    for (owner, skill) in [(0, "area"), (1, "place")] {
        w.action(
            owner,
            &Action::CastGround {
                skill_id: skill.into(),
                position: [52., 20.],
            },
            skill,
            &c,
            0,
        )
        .unwrap();
    }
    w.tick(&c, 30);
    assert_eq!(w.areas.len(), 2);
    assert_eq!((w.members[0].spirit, w.members[1].spirit), (99, 99));
    let trap = w.areas.iter().find(|p| p.is_trap()).unwrap().view().id;
    let anchor = w.areas.iter().find(|p| p.is_anchor()).unwrap().view().id;
    let contacts = |events: &Events| {
        events
            .iter()
            .filter(|(_, e)| {
                matches!(
                    e, EventV4::AreaEntered {area_id, ..} if *area_id == trap
                )
            })
            .count()
    };
    assert_eq!(contacts(&w.tick(&c, 60)), 1);
    assert_eq!(contacts(&w.tick(&c, 90)), 0);
    let leave = w.action(0, &Action::Leave {}, "leave", &c, 90).unwrap();
    assert!(leave.iter().any(|(_, e)| matches!(
        e, EventV4::AreaEnded {area_id,reason:AreaEndReason::SourceInvalid} if *area_id == trap
    )));
    assert_eq!(
        w.areas.iter().map(|p| p.view().id).collect::<Vec<_>>(),
        [anchor]
    );
}
