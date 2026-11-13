use super::*;
use crate::protocol::{ItemAmount, valid_id};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorldLoot {
    pub environment_capacity: usize,
    pub container_capacity: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub one_shot_capacity: usize,
    pub partial_pickup: bool,
    pub interaction_range: f64,
    pub shared_clearance: f64,
    pub pools: BTreeMap<String, LootPool>,
    pub nodes: Vec<LootNode>,
    pub containers: Vec<LootContainer>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub monster_pools: BTreeMap<String, MonsterLoot>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub instances: BTreeMap<String, InstanceDefinition>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub relics: Option<DeathRelics>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstanceDefinition {
    pub quality: u16,
    pub relic_eligible: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeathRelics {
    pub capacity: usize,
    pub interaction_range: f64,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub excluded_items: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stack_qualities: BTreeMap<String, u16>,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MonsterLoot {
    pub pool: String,
    pub priority_ms: u64,
    pub correction_radius: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LootPool {
    pub guaranteed: Vec<LootEntry>,
    pub independent: Vec<ProbableEntry>,
    pub groups: Vec<LootGroup>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LootEntry {
    pub item_id: String,
    pub quantity_min: u32,
    pub quantity_max: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProbableEntry {
    pub chance_bps: u32,
    pub entry: LootEntry,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LootGroup {
    pub chance_bps: u32,
    pub count_min: usize,
    pub count_max: usize,
    pub entries: Vec<LootEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LootNode {
    pub id: String,
    pub pool: String,
    pub points: Vec<[f64; 2]>,
    pub replenish_ms: u64,
    pub ttl_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub hotspot: Option<LootHotspot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LootHotspot {
    pub group: String,
    pub radius: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LootContainer {
    pub id: String,
    pub position: [f64; 2],
    pub pool: String,
    pub visibility: ContainerVisibility,
    pub unlock_ms: u64,
    pub replenish_ms: u64,
    pub ttl_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drop_points: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guardians: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub hotspot_group: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub cost: Option<ItemAmount>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ContainerVisibility {
    Shared,
    Personal { participant_id: String },
    Hidden { reveal_radius: f64 },
}

impl LootPool {
    pub(super) fn max_stacks(&self) -> usize {
        self.guaranteed.len()
            + self
                .independent
                .iter()
                .filter(|entry| entry.chance_bps > 0)
                .count()
            + self
                .groups
                .iter()
                .filter(|group| group.chance_bps > 0)
                .map(|group| group.count_max)
                .sum::<usize>()
    }

    pub(super) fn item_bound(&self, item: &str) -> u64 {
        let amount = |entry: &LootEntry| {
            u64::from(if entry.item_id == item {
                entry.quantity_max
            } else {
                0
            })
        };
        self.guaranteed.iter().map(amount).sum::<u64>()
            + self
                .independent
                .iter()
                .filter(|entry| entry.chance_bps > 0)
                .map(|entry| amount(&entry.entry))
                .sum::<u64>()
            + self
                .groups
                .iter()
                .filter(|group| group.chance_bps > 0)
                .map(|group| {
                    let mut quantities: Vec<_> = group
                        .entries
                        .iter()
                        .map(amount)
                        .filter(|q| *q != 0)
                        .collect();
                    quantities.sort_unstable_by(|a, b| b.cmp(a));
                    quantities.into_iter().take(group.count_max).sum::<u64>()
                })
                .sum::<u64>()
    }
}

impl Config {
    pub(crate) fn instance_definition(&self, item: &str) -> Option<&InstanceDefinition> {
        self.world_loot.as_ref()?.instances.get(item)
    }

    pub(crate) fn death_relics(&self) -> Option<&DeathRelics> {
        self.world_loot.as_ref()?.relics.as_ref()
    }

    pub(crate) fn monster_loot_bound(&self, id: &str, legacy: &LootTable, item: &str) -> u64 {
        if let Some(world) = &self.world_loot
            && let Some(source) = world.monster_pools.get(id)
        {
            return std::iter::once(&source.pool)
                .chain(
                    self.encounter_progress
                        .iter()
                        .flat_map(|rule| rule.modifiers())
                        .filter_map(|modifier| modifier.loot_pools.get(id)),
                )
                .filter_map(|pool| world.pools.get(pool))
                .map(|pool| pool.item_bound(item))
                .max()
                .unwrap_or(0);
        }
        legacy
            .guaranteed
            .iter()
            .chain(&legacy.random)
            .filter(|r| r.item_id == item && r.chance != Some(0.0))
            .map(|r| u64::from(r.quantity_max))
            .sum()
    }

    pub(crate) fn world_loot_bound(&self, item: &str, now: u64) -> u64 {
        let Some(world) = &self.world_loot else {
            return 0;
        };
        let now = now.min(self.terminal_ms);
        world
            .nodes
            .iter()
            .map(|node| (&node.pool, node.replenish_ms))
            .chain(
                world
                    .containers
                    .iter()
                    .map(|container| (&container.pool, container.replenish_ms)),
            )
            .map(|(pool, replenish)| {
                world.pools.get(pool).map_or(0, |pool| {
                    pool.item_bound(item) * (now / replenish.max(1) + 1)
                })
            })
            .sum::<u64>()
            + u64::from(self.contested_reward(item))
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(world) = &c.world_loot else {
        return Ok(());
    };
    let Some(pve) = &c.pve else {
        return Err("CONFIG_WORLD_LOOT_CAPABILITY");
    };
    if c.protocol_version() != 6 || c.mechanism_version < 105 || c.members.is_none() {
        return Err("CONFIG_WORLD_LOOT_CAPABILITY");
    }
    if world.nodes.len() > 32
        || world.containers.len() > 16
        || world.pools.len() > 32
        || world.environment_capacity > 96
        || world.container_capacity > 96
        || world.one_shot_capacity > 96
        || !(0.1..=1000.).contains(&world.interaction_range)
        || !(0.0..=1000.).contains(&world.shared_clearance)
    {
        return Err("CONFIG_WORLD_LOOT_BOUNDS");
    }
    if world.instances.len() > 32
        || world.instances.keys().any(|id| {
            id == &c.item_id
                || !valid_id(id)
                || pve.items.get(id).is_none_or(|item| item.stack_limit != 1)
                || c.member_item_use
                    .as_ref()
                    .is_some_and(|uses| uses.contains_key(id))
        })
        || !world.instances.is_empty()
            && pve.build_profiles().values().any(|template| {
                template
                    .drop_table
                    .guaranteed
                    .iter()
                    .chain(&template.drop_table.random)
                    .any(|row| world.instances.contains_key(&row.item_id))
            })
    {
        return Err("CONFIG_WORLD_LOOT_INSTANCE");
    }
    if let Some(relics) = &world.relics
        && (!(1..=8).contains(&relics.capacity)
            || !(0.1..=1000.).contains(&relics.interaction_range)
            || relics.excluded_items.len() > 128
            || relics
                .excluded_items
                .iter()
                .any(|id| !pve.items.contains_key(id))
            || relics.stack_qualities.len() > 128
            || relics
                .stack_qualities
                .keys()
                .any(|id| !pve.items.contains_key(id) || world.instances.contains_key(id))
            || c.result_contract.as_ref().is_none_or(|contract| {
                contract.death.policy != crate::protocol::SettlementPolicy::Attrition
                    || contract.extraction.policy != crate::protocol::SettlementPolicy::Preserve
            }))
    {
        return Err("CONFIG_WORLD_LOOT_RELIC");
    }
    let entry_valid = |e: &LootEntry| {
        pve.items.contains_key(&e.item_id)
            && (1..=10000).contains(&e.quantity_max)
            && (1..=e.quantity_max).contains(&e.quantity_min)
            && (!world.instances.contains_key(&e.item_id) || e.quantity_max == 1)
    };
    for (id, pool) in &world.pools {
        if !valid_id(id)
            || pool.guaranteed.len() > 8
            || pool.independent.len() > 8
            || pool.groups.len() > 4
            || !pool.guaranteed.iter().all(entry_valid)
            || pool
                .independent
                .iter()
                .any(|e| e.chance_bps > 10000 || !entry_valid(&e.entry))
            || pool.groups.iter().any(|g| {
                g.chance_bps > 10000
                    || !(1..=16).contains(&g.entries.len())
                    || g.count_max > g.entries.len()
                    || g.count_min > g.count_max
                    || !g.entries.iter().all(entry_valid)
            })
            || pool.max_stacks() > 8
        {
            return Err("CONFIG_WORLD_LOOT_POOL");
        }
    }
    if !world.monster_pools.is_empty() {
        let profiles = pve.build_profiles();
        for (template_id, source) in &world.monster_pools {
            if profiles.get(template_id).is_none_or(|template| {
                !template.drop_table.guaranteed.is_empty() || !template.drop_table.random.is_empty()
            }) || world
                .pools
                .get(&source.pool)
                .is_none_or(|p| p.max_stacks() > world.one_shot_capacity)
                || source.priority_ms > c.params.resource_ttl_ms
                || !(0.0..=75.0).contains(&source.correction_radius)
            {
                return Err("CONFIG_WORLD_LOOT_MONSTER");
            }
        }
    }
    let point_valid = |p: [f64; 2]| {
        (0..2).all(|axis| (c.geometry.min[axis]..=c.geometry.max[axis]).contains(&p[axis]))
    };
    let duration_valid = |ms| (c.step_ms..=3_600_000).contains(&ms);
    let mut ids = BTreeSet::new();
    for node in &world.nodes {
        if !valid_id(&node.id)
            || !ids.insert(&node.id)
            || !(1..=16).contains(&node.points.len())
            || !node.points.iter().copied().all(point_valid)
            || !duration_valid(node.replenish_ms)
            || !duration_valid(node.ttl_ms)
            || world
                .pools
                .get(&node.pool)
                .is_none_or(|p| p.max_stacks() > world.environment_capacity)
            || node
                .hotspot
                .as_ref()
                .is_some_and(|h| !valid_id(&h.group) || !(0.1..=10000.).contains(&h.radius))
        {
            return Err("CONFIG_WORLD_LOOT_NODE");
        }
    }
    let npc_ids: BTreeSet<_> = (0..pve.monster_positions.len())
        .map(|i| format!("{}-{}", pve.template_id, i + 1))
        .chain(pve.extra_spawns.iter().map(|spawn| spawn.id.clone()))
        .collect();
    for container in &world.containers {
        if !valid_id(&container.id)
            || !ids.insert(&container.id)
            || !point_valid(container.position)
            || !duration_valid(container.unlock_ms)
            || !duration_valid(container.replenish_ms)
            || !duration_valid(container.ttl_ms)
            || container.drop_points.len() > 16
            || !container.drop_points.iter().copied().all(point_valid)
            || world
                .pools
                .get(&container.pool)
                .is_none_or(|p| p.max_stacks() > world.container_capacity)
            || container.guardians.len() > 8
            || container.guardians.iter().any(|id| !npc_ids.contains(id))
            || container.guardians.iter().collect::<BTreeSet<_>>().len()
                != container.guardians.len()
            || container.hotspot_group.as_ref().is_some_and(|group| {
                !valid_id(group) || !matches!(container.visibility, ContainerVisibility::Shared)
            })
            || container.cost.as_ref().is_some_and(|cost| {
                !pve.items.contains_key(&cost.item_id)
                    || !(1..=10000).contains(&cost.quantity)
                    || world.instances.contains_key(&cost.item_id)
            })
            || match &container.visibility {
                ContainerVisibility::Shared => false,
                ContainerVisibility::Personal { participant_id } => !valid_id(participant_id),
                ContainerVisibility::Hidden { reveal_radius } => {
                    !(world.interaction_range..=1000.).contains(reveal_radius)
                }
            }
        {
            return Err("CONFIG_WORLD_LOOT_CONTAINER");
        }
    }
    Ok(())
}
