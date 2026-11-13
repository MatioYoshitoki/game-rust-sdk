//! Capability, type and resource bounds for parameterized mechanisms; no value fingerprints.
use super::*;
pub(super) fn validate(c: &Config, skill_limit: usize) -> Result<(), &'static str> {
    super::player_skills::validate(c, skill_limit)?;
    super::status::validate(c)?;
    let finite = |n: f64, lo: f64, hi: f64| n.is_finite() && (lo..=hi).contains(&n);
    let id = |s: &str| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control);
    let time = |n: u64| n <= 3_600_000;
    let p = &c.params;
    let r = &p.runtime;
    if (matches!(c.mechanism_version, 37..=105) && c.schema_version != 7)
        || (c.mechanism_version == 35 && c.schema_version != 5)
        || (c.mechanism_version == 36 && c.schema_version != 6)
    {
        return Err("CONFIG_MECHANISM_VERSION");
    }
    if !matches!(
        (
            c.schema_version,
            c.asset_contract.is_some(),
            c.result_contract.is_some()
        ),
        (2, false, false) | (3, true, false) | (4, true, true)
    ) || !(1..=crate::support::MAX_MECHANISM_VERSION).contains(&c.mechanism_version)
    {
        return Err("CONFIG_MECHANISM_VERSION");
    }
    if let Some(results) = &c.result_contract
        && (!id(&results.extraction.result_id)
            || !id(&results.death.result_id)
            || results.extraction.result_id == results.death.result_id)
    {
        return Err("CONFIG_RESULT_CONTRACT");
    }
    if let Some(contract) = &c.asset_contract {
        let collides = |asset: &str| {
            asset == c.item_id
                || c.pve
                    .as_ref()
                    .is_some_and(|pve| pve.items.contains_key(asset))
        };
        if !id(&contract.entry_cost_asset_id)
            || !id(&contract.reward_asset_id)
            || contract.entry_cost_asset_id == contract.reward_asset_id
            || collides(&contract.entry_cost_asset_id)
            || collides(&contract.reward_asset_id)
        {
            return Err("CONFIG_ASSET_CONTRACT");
        }
    }
    if !(1..=100).contains(&c.step_ms)
        || !(16..=1024).contains(&c.input_capacity)
        || !(32..=1024).contains(&c.output_capacity)
        || !(16..=1024).contains(&c.history_capacity)
    {
        return Err("CONFIG_RUNTIME_BOUNDS");
    }
    if !(100..=60000).contains(&r.io_timeout_ms)
        || !(100..=60000).contains(&r.drain_timeout_ms)
        || !(100..=3600000).contains(&r.reconnect_grace_ms)
        || !(1..=60000).contains(&r.publish_batched_ms)
        || !(1..=262144).contains(&r.nav_budget)
        || !(1..=60000).contains(&r.nav_repath_ms)
        || !(1..=60000).contains(&r.ai_decision_ms)
        || !(1..=60000).contains(&r.resource_sweep_ms)
        || !(100..=10000).contains(&r.call_timeout_ms)
        || !(1..=3).contains(&r.max_attempts)
        || !(1..=10).contains(&r.max_catchup_steps)
        || !(1..=100).contains(&r.max_lag_steps)
    {
        return Err("CONFIG_RUNTIME_BOUNDS");
    }
    if !finite(p.player_radius, 0.1, 128.0)
        || !finite(p.pickup_range, 0.1, 1000.0)
        || !finite(p.extraction_range, 0.1, 1000.0)
        || !finite(p.movement.base, 0.0, 10000.0)
        || !finite(p.movement.bonus, 0.0, 10000.0)
        || !finite(p.movement.reference, 0.001, 1000000.0)
        || !finite(p.movement.log_base, 1.001, 1000000.0)
        || !(1..=60000).contains(&p.regen.interval_ms)
        || !finite(p.regen.near_radius, 0.0, 10000.0)
        || ![
            p.regen.near_ratio,
            p.regen.far_ratio,
            p.orb_hp_ratio,
            p.orb_spirit_ratio,
            p.death.slot_ratio,
            p.death.quantity_ratio,
        ]
        .iter()
        .all(|v| finite(*v, 0.0, 1.0))
        || !finite(p.defense_scale, 0.001, 100.0)
        || !finite(c.defense_cap, 0.0, 1.0)
        || !finite(p.critical_multiplier, 0.0, 10.0)
        || !finite(p.metal_multiplier, 0.0, 10.0)
        || p.minimum_damage > 1000000
        || p.basic_spirit_on_hit > 1000000
        || p.death.minimum_quantity > 10000
        || p.entry_years > 10000
        || !(1..=3600000).contains(&p.resource_ttl_ms)
        || p.drop_min_radius > p.drop_max_radius
        || p.drop_max_radius > 75
        || !finite(p.basic_projectile_radius, 0.0, 128.0)
        || !(1..=3600000).contains(&p.player_projectile_ttl_ms)
    {
        return Err("CONFIG_PARAMETER_BOUNDS");
    }
    let a = &p.initial_player;
    if c.derived_base(a.hp_points, crate::protocol::StatusAttribute::MaxHp)
        > c.resource_bounds().max_hp
        || c.derived_base(a.spirit_points, crate::protocol::StatusAttribute::MaxSpirit)
            > c.resource_bounds().max_spirit
        || c.build_rules().is_none() && a.hp_points == 0
        || [a.atk_points, a.def_points, a.spd_points]
            .iter()
            .any(|n| *n > 1000000)
    {
        return Err("CONFIG_PLAYER_BOUNDS");
    }
    if !id(&c.item_id)
        || !id(&c.target.id)
        || !id(&c.skill.id)
        || !(1..=128).contains(&c.inventory_capacity)
        || !(1..=10000).contains(&c.stack_limit)
        || c.carried_quantity > c.inventory_capacity * c.stack_limit
        || c.target.hp == 0
        || c.target.hp > c.resource_bounds().max_hp
        || c.target.atk > 1000000
        || c.target.armor > 1000000
        || c.target.drop_quantity > 10000
        || !finite(c.target.speed, 0.0, 10000.0)
        || !finite(c.target.radius, 0.1, 128.0)
        || !finite(c.target.aggro_range, 0.0, 10000.0)
        || !finite(c.target.attack_range, 0.0, 10000.0)
        || c.target.interval_ms == 0
        || !time(c.target.interval_ms)
    {
        return Err("CONFIG_ENTITY_BOUNDS");
    }
    if ![
        c.basic.windup_ms,
        c.basic.interval_ms,
        c.skill.windup_ms,
        c.skill.recovery_ms,
        c.skill.cooldown_ms,
        c.extraction_unlock_ms,
        c.extraction_channel_ms,
        c.terminal_ms,
    ]
    .iter()
    .all(|t| time(*t))
        || c.basic.interval_ms == 0
        || c.terminal_ms == 0
        || c.extraction_unlock_ms
            .checked_add(c.extraction_channel_ms)
            .is_none_or(|end| end > c.terminal_ms)
        || !finite(c.basic.range, 0.1, 10000.0)
        || (!c.skill.is_self() && !finite(c.skill.range, 0.1, 10000.0))
        || !finite(c.basic.projectile_speed, 0.1, 10000.0)
        || (!c.skill.is_self()
            && !c.skill.is_ground()
            && !c.skill.is_line()
            && !c.skill.targets_ally()
            && !c.skill.is_unit_effect()
            && !finite(c.skill.speed, 0.1, 10000.0))
        || !finite(c.skill.radius, 0.0, 128.0)
        || c.skill.cost > c.resource_bounds().max_spirit_cost
        || c.skill.damage_bps > 100000
        || !finite(c.cultivation_coefficient, 0.0, 1000.0)
        || !finite(c.death_multiplier, 0.0, 10.0)
        || c.defender_realm_atk == 0
        || c.defender_realm_atk > 1000000
    {
        return Err("CONFIG_COMBAT_BOUNDS");
    }
    if let Some(v) = &c.pve {
        if !id(&v.template_id)
            || v.monster_positions.len() > 32
            || v.chest_spawns.len() > 16
            || v.items.len() > 64
            || !v.chest_spawns.is_empty() && v.treasure_rules.is_none()
        {
            return Err("CONFIG_CONTENT_BOUNDS");
        }
        if v.stats.len() != 7 + usize::from(v.stats.contains_key("hit"))
            || ["hp", "spirit", "atk", "def", "spd", "crit", "dodge"]
                .iter()
                .any(|key| !v.stats.contains_key(*key))
            || v.stats["hp"] == 0
            || v.stats.iter().any(|(key, n)| *n > c.base_stat_limit(key))
            || v.stats["crit"] > 100
            || v.stats["dodge"] > 100
        {
            return Err("CONFIG_STATS");
        }
        if v.items.keys().any(|key| !id(key))
            || v.items
                .get(&c.item_id)
                .is_none_or(|item| item.stack_limit != c.stack_limit)
            || v.items
                .values()
                .any(|i| i.name.is_empty() || !(1..=10000).contains(&i.stack_limit))
        {
            return Err("CONFIG_ITEM_REFERENCE");
        }
        let m = &v.combat;
        let a = &m.projectile;
        let h = &m.heal;
        if ![
            m.radius,
            m.melee_range,
            m.aggro_range,
            m.return_distance,
            v.leash,
            v.help_range,
            v.memory_range,
        ]
        .iter()
        .all(|n| finite(*n, 0.1, 10000.0))
            || m.radius > 128.0
            || !finite(m.cast_chance, 0.0, 1.0)
            || !finite(m.lifesteal_ratio, 0.0, 1.0)
            || !finite(h.hp_threshold, 0.0, 1.0)
            || m.attack_interval_ms == 0
            || !time(m.attack_interval_ms)
            || m.mark_max > 100
            || !time(m.mark_duration_ms)
            || !id(&a.id)
            || !id(&h.id)
            || a.id == h.id
            || a.cost > c.resource_bounds().max_spirit_cost
            || h.cost > c.resource_bounds().max_spirit_cost
            || ![
                a.windup_ms,
                a.cooldown_ms,
                a.recovery_ms,
                a.ttl_ms,
                h.windup_ms,
                h.cooldown_ms,
                h.recovery_ms,
                h.interval_ms,
            ]
            .iter()
            .all(|t| time(*t))
            || a.ttl_ms == 0
            || h.interval_ms == 0
            || !(1..=64).contains(&h.ticks)
            || h.heal_bps > 10000
            || a.damage_bps > 100000
            || !finite(a.range, 0.1, 10000.0)
            || !finite(a.speed, 0.1, 10000.0)
            || !finite(a.radius, 0.0, 128.0)
        {
            return Err("CONFIG_MONSTER_ABILITY_BOUNDS");
        }
        validate_loot(&v.drop_table, &v.items)?;
        if let Some(t) = &v.treasure_rules {
            match (&t.template.mechanism, &t.template.seal) {
                (ContainerMechanism::ImmediateGroundLoot, None) => {}
                (ContainerMechanism::LayeredSealGroundLoot, Some(s))
                    if c.mechanism_version >= 2
                        && (1..=32).contains(&s.layers)
                        && (c.step_ms..=60000).contains(&s.layer_duration_ms)
                        && u64::from(s.layers) * s.layer_duration_ms <= c.terminal_ms
                        && finite(s.interrupt_retention_ratio, 0.0, 1.0) => {}
                _ => return Err("CONFIG_SEAL_CAPABILITY_OR_BOUNDS"),
            }
            if !id(&t.template.id)
                || t.template.name.is_empty()
                || !(1000..=3600000).contains(&t.template.respawn_delay_ms)
                || !(1..=128).contains(&t.ground_capacity)
                || !finite(t.interaction_range, 0.1, 1000.0)
            {
                return Err("CONFIG_CONTAINER_BOUNDS");
            }
            validate_loot(&t.loot, &v.items)?;
        }
        super::population::validate_population(c, v, skill_limit)?;
        super::objective::validate(c, v)?;
        super::reset::validate(c, v)?;
        super::timed_position::validate(c, v)?;
        let ground = v
            .population_bound(c.terminal_ms)
            .iter()
            .map(|(t, n)| {
                *n as usize
                    * (t.drop_table.guaranteed.len()
                        + t.drop_table.random.len()
                        + usize::from(t.drop_orb))
            })
            .sum::<usize>()
            + v.treasure_rules.as_ref().map_or(0, |t| {
                v.chest_spawns.len() * (t.loot.guaranteed.len() + t.loot.random.len()) * 2
            });
        if ground > 128 {
            return Err("CONFIG_RESOURCE_BUDGET");
        }
        for item in v.items.keys() {
            if u64::from(c.pve_loot_bound(item, c.max_kills(c.terminal_ms), c.terminal_ms))
                > 1000000000
            {
                return Err("CONFIG_LOOT_BUDGET");
            }
        }
    }
    super::scoring::validate(c)?;
    if (c.max_kill_score(c.terminal_ms) as f64 + f64::from(c.extraction_score))
        * c.cultivation_coefficient
        * c.death_multiplier.max(1.0)
        > f64::from(u32::MAX)
    {
        return Err("CONFIG_GROWTH_OVERFLOW");
    }
    Ok(())
}
fn validate_loot(
    t: &LootTable,
    items: &std::collections::BTreeMap<String, Item>,
) -> Result<(), &'static str> {
    if t.guaranteed.len() + t.random.len() > 64 {
        return Err("CONFIG_LOOT_BOUNDS");
    }
    for (r, random) in t
        .guaranteed
        .iter()
        .map(|r| (r, false))
        .chain(t.random.iter().map(|r| (r, true)))
    {
        if !items.contains_key(&r.item_id) {
            return Err("CONFIG_ITEM_REFERENCE");
        }
        if r.quantity_min > r.quantity_max
            || r.quantity_max > 10000
            || random && r.chance.is_none()
            || !random && r.chance.is_some()
            || r.chance
                .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            return Err("CONFIG_LOOT_BOUNDS");
        }
    }
    Ok(())
}

pub(super) fn shapes(v: &serde_json::Value) -> Result<(), &'static str> {
    for path in [
        "/target",
        "/basic",
        "/skill",
        "/params",
        "/params/movement",
        "/params/regen",
        "/params/death",
        "/params/initial_player",
        "/params/runtime",
    ] {
        if !v.pointer(path).is_some_and(serde_json::Value::is_object) {
            return Err("CONFIG_SCHEMA");
        }
    }
    if v.get("pve").is_some_and(serde_json::Value::is_object) {
        if let Some(check) = v.pointer("/pve/timed_position")
            && (!check.is_object()
                || ["warning_bonus_ms", "failure_scale_bps"]
                    .iter()
                    .any(|key| check.get(key).is_some_and(|rule| !rule.is_object())))
        {
            return Err("CONFIG_SCHEMA");
        }
        if let Some(encounters) = v.pointer("/pve/encounters") {
            let Some(rows) = encounters.as_array() else {
                return Err("CONFIG_SCHEMA");
            };
            for e in rows {
                let objects = |v: &serde_json::Value| {
                    v.as_array()
                        .is_some_and(|a| a.iter().all(serde_json::Value::is_object))
                };
                if !e.is_object()
                    || !e.get("trigger").is_some_and(serde_json::Value::is_object)
                    || e.get("monsters").is_some_and(|v| !objects(v))
                {
                    return Err("CONFIG_SCHEMA");
                }
                if let Some(waves) = e.get("waves") {
                    let Some(rows) = waves.as_array() else {
                        return Err("CONFIG_SCHEMA");
                    };
                    if rows
                        .iter()
                        .any(|w| !w.is_object() || !w.get("monsters").is_some_and(objects))
                    {
                        return Err("CONFIG_SCHEMA");
                    }
                }
            }
        }
        if v.pointer("/pve")
            .is_some_and(|p| p.get("kill_score").is_some_and(|n| n.as_u64().is_none()))
        {
            return Err("CONFIG_SCHEMA");
        }
        if let Some(templates) = v.pointer("/pve/additional_templates") {
            let Some(rows) = templates.as_object() else {
                return Err("CONFIG_SCHEMA");
            };
            for t in rows.values() {
                if !t.is_object() || t.get("kill_score").is_some_and(|n| n.as_u64().is_none()) {
                    return Err("CONFIG_SCHEMA");
                }
                if t.pointer("/support/on_encounter_completed")
                    .is_some_and(|variant| !variant.is_object())
                {
                    return Err("CONFIG_SCHEMA");
                }
                if let Some(area) = t.get("area").filter(|a| !a.is_null())
                    && !area.is_object()
                {
                    return Err("CONFIG_SCHEMA");
                }
                if let Some(boss) = t.get("boss").filter(|b| !b.is_null())
                    && (!boss.is_object()
                        || !boss
                            .get("phases")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|a| a.iter().all(serde_json::Value::is_object)))
                {
                    return Err("CONFIG_SCHEMA");
                }
                for path in [
                    "/stats",
                    "/combat",
                    "/combat/projectile",
                    "/combat/heal",
                    "/drop_table",
                ] {
                    if !t.pointer(path).is_some_and(serde_json::Value::is_object) {
                        return Err("CONFIG_SCHEMA");
                    }
                }
                for key in ["guaranteed", "random"] {
                    if t["drop_table"][key]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|r| !r.is_object()))
                    {
                        return Err("CONFIG_SCHEMA");
                    }
                }
            }
        }
        if let Some(spawns) = v.pointer("/pve/extra_spawns")
            && !spawns
                .as_array()
                .is_some_and(|rows| rows.iter().all(serde_json::Value::is_object))
        {
            return Err("CONFIG_SCHEMA");
        }
        if v.pointer("/pve/treasure_rules/template/guardian")
            .is_some_and(|g| !g.is_null() && !g.is_object())
        {
            return Err("CONFIG_SCHEMA");
        }
        if v.pointer("/pve/treasure_rules/template/seal")
            .is_some_and(|seal| !seal.is_null() && !seal.is_object())
        {
            return Err("CONFIG_SCHEMA");
        }
        for path in [
            "/pve/stats",
            "/pve/items",
            "/pve/combat",
            "/pve/combat/projectile",
            "/pve/combat/heal",
            "/pve/drop_table",
        ] {
            if !v.pointer(path).is_some_and(serde_json::Value::is_object) {
                return Err("CONFIG_SCHEMA");
            }
        }
        if v["pve"]["items"]
            .as_object()
            .unwrap()
            .values()
            .any(|v| !v.is_object())
        {
            return Err("CONFIG_SCHEMA");
        }
        for path in [
            "/pve/chest_spawns",
            "/pve/palette",
            "/pve/drop_table/guaranteed",
            "/pve/drop_table/random",
            "/pve/treasure_rules/loot/guaranteed",
            "/pve/treasure_rules/loot/random",
        ] {
            if v.pointer(path)
                .and_then(serde_json::Value::as_array)
                .is_some_and(|a| a.iter().any(|v| !v.is_object()))
            {
                return Err("CONFIG_SCHEMA");
            }
        }
        if v["pve"]
            .get("treasure_rules")
            .is_some_and(serde_json::Value::is_object)
            && !v
                .pointer("/pve/treasure_rules/template")
                .is_some_and(serde_json::Value::is_object)
        {
            return Err("CONFIG_SCHEMA");
        }
    }
    Ok(())
}
