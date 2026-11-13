//! Ground-loot lifetime against a world clock that has already run past the
//! drop TTL. Regression for the arena clock never advancing: drops used to be
//! stamped with a stale `now`, so any kill after `resource_ttl_ms` of match
//! time vanished in the next sweep before a player could see it.
use super::*;

fn config() -> Config {
    // Keep this a plain string literal: a json! literal this size busts the macro
    // recursion limit on this toolchain.
    let value: serde_json::Value = serde_json::from_str(r#"{"schema_version":4,"mechanism_version":34,"id":"drop-ttl","source_commit":"synthetic","purpose":"mechanism regression","step_ms":30,"terminal_ms":900000,"geometry":{"min":[0.0,0.0],"max":[2000.0,2000.0],"walls":[]},"spawn":[40.0,40.0],"extraction_point":[1960.0,1960.0],"extraction_unlock_ms":4000,"extraction_channel_ms":3000,"target":{"id":"target","position":[1000.0,1000.0],"hp":100000,"atk":10,"armor":0,"speed":0,"radius":2,"aggro_range":8,"attack_range":2,"interval_ms":1000,"drop_quantity":0},"basic":{"mechanism":"projectile","windup_ms":30,"interval_ms":300,"range":600,"projectile_speed":300},"skill":{"mechanism":"unit_projectile","id":"skill","cost":5,"windup_ms":30,"recovery_ms":60,"cooldown_ms":300,"damage_bps":10000,"range":600,"speed":600,"radius":4},"additional_skills":[{"mechanism":"projectile","id":"snipe","cost":5,"windup_ms":30,"recovery_ms":60,"cooldown_ms":600,"damage_bps":20000,"range":700,"speed":800,"radius":2}],"inventory_capacity":24,"stack_limit":99,"item_id":"item","carried_quantity":0,"kill_score":0,"extraction_score":0,"cultivation_coefficient":0,"death_multiplier":1,"defender_realm_atk":100,"defense_cap":0.7,"input_capacity":64,"output_capacity":1024,"history_capacity":64,"asset_contract":{"entry_cost_asset_id":"entry","reward_asset_id":"reward"},"result_contract":{"extraction":{"result_id":"extract","policy":"preserve"},"death":{"result_id":"death","policy":"attrition"}},"params":{"player_radius":8,"pickup_range":24,"extraction_range":24,"movement":{"base":180,"bonus":120,"reference":8,"log_base":100},"regen":{"interval_ms":1000,"near_radius":0,"near_ratio":0,"far_ratio":0},"basic_spirit_on_hit":0,"orb_hp_ratio":0,"orb_spirit_ratio":0,"defense_scale":2,"minimum_damage":1,"critical_multiplier":1,"metal_multiplier":1,"resource_ttl_ms":10000,"drop_min_radius":0,"drop_max_radius":0,"basic_projectile_radius":1,"player_projectile_ttl_ms":1000,"entry_years":1,"death":{"slot_ratio":0,"quantity_ratio":0,"minimum_quantity":1},"initial_player":{"hp_points":1000,"spirit_points":200,"atk_points":100,"def_points":0,"spd_points":8},"runtime":{"io_timeout_ms":500,"drain_timeout_ms":1000,"reconnect_grace_ms":500,"publish_batched_ms":60,"nav_budget":64,"nav_repath_ms":30,"ai_decision_ms":30,"resource_sweep_ms":100,"call_timeout_ms":1000,"max_attempts":2,"max_catchup_steps":2,"max_lag_steps":8}},"pve":{"template_id":"npc","map_id":"drop-ttl","width":50,"height":50,"cell_size":40,"palette":[{"terrain":"floor","dotDamagePerSecond":0,"elevation":0,"maxClimbHeight":0,"maxDropHeight":0,"walkable":true,"speedMultiplier":1,"blocksProjectiles":false,"blocksVision":false}],"cells_rle":[[0,2500]],"monster_positions":[[240.0,40.0]],"stats":{"hp":600,"spirit":100,"atk":20,"def":0,"spd":8,"crit":0,"dodge":0},"combat":{"radius":2,"attack_interval_ms":1000,"aggro_range":120,"melee_range":16,"cast_chance":0,"return_distance":40,"lifesteal_ratio":0,"mark_max":0,"mark_duration_ms":0,"projectile":{"mechanism":"projectile","id":"npc-cast","cost":10,"windup_ms":60,"cooldown_ms":600,"recovery_ms":60,"range":400,"speed":300,"radius":4,"ttl_ms":2000,"damage_bps":3000},"heal":{"mechanism":"budget_heal_channel","id":"npc-heal","cost":0,"windup_ms":30,"cooldown_ms":300,"recovery_ms":60,"hp_threshold":0,"heal_bps":0,"interval_ms":30,"ticks":1}},"drop_table":{"guaranteed":[{"itemId":"item","quantityMin":2,"quantityMax":3}],"random":[]},"items":{"item":{"name":"item","stackLimit":99}},"leash":200,"help_range":80,"memory_range":400,"status_rules":{"max_active_per_entity":6,"defense_bonus_cap_bps":10000,"attack_bonus_cap_bps":10000,"shield_cap_bps":10000},"status_definitions":[{"id":"burn","mechanism":"periodic_damage","damage_bps":2000,"tick_interval_ms":120,"duration_ms":2400,"duplicate_policy":"refresh","source_scoped":true}]}}"#).unwrap();
    let dir = std::env::temp_dir().join(format!("match-sdk-drop-ttl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("arena.json"), value.to_string()).unwrap();
    std::fs::write(
        dir.join("catalog.json"),
        serde_json::json!({"schema_version": 1, "default_scenario": "bench",
            "active": {"bench": "arena.json"}, "archives": []})
        .to_string(),
    )
    .unwrap();
    let catalog = crate::config::ConfigCatalog::load(dir.join("catalog.json")).unwrap();
    let config = catalog.default_config().as_ref().clone();
    std::fs::remove_dir_all(&dir).ok();
    config
}

fn world(c: &Config, seed: u32) -> World {
    let mut w = World::new(
        c,
        ParticipantFacts {
            principal: "principal-A".into(),
            revision: 1,
            hp_points: 1000,
            spirit_points: 200,
            atk_points: 100,
            def_points: 0,
            spd_points: 8,
            warehouse_quantity: 0,
        },
        seed,
    );
    w.phase = Phase::Active;
    w.ready = true;
    w.joined = true;
    w.protected = false;
    w.active_at = Some(0);
    w.position = c.spawn;
    w
}

fn kill_first_monster(w: &mut World, c: &Config) {
    let mut arena = w.pve.take().unwrap();
    let mut events = vec![];
    arena.hurt_monster_contact(w, c, 0, (10000, false, None), &mut events);
    w.pve = Some(arena);
}

#[test]
fn kill_drop_survives_when_match_clock_already_exceeds_the_ttl() {
    let c = config();
    let mut w = world(&c, 7);
    // Run the match far past resource_ttl_ms (10s) before the kill happens.
    let kill_at = 60000u64;
    w.tick(&c, kill_at);
    kill_first_monster(&mut w, &c);
    // The sweep runs every 100ms; step well past one sweep interval.
    w.tick(&c, kill_at + 500);
    assert!(
        !w.resources.is_empty(),
        "drop vanished immediately: arena clock stamped the expiry in the past"
    );
    // The template also drops an orb by default; the guaranteed rule drop is what matters.
    assert!(
        w.resources
            .iter()
            .any(|r| !r.view.orb && r.view.quantity >= 2)
    );
}

#[test]
fn kill_drop_still_expires_after_its_ttl() {
    let c = config();
    let mut w = world(&c, 7);
    let kill_at = 60000u64;
    w.tick(&c, kill_at);
    kill_first_monster(&mut w, &c);
    w.tick(&c, kill_at + c.params.resource_ttl_ms + 500);
    assert!(w.resources.is_empty(), "ttl no longer sweeps the drop");
}
