use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncounterProgress {
    pub initial: u32,
    pub kill_weights: BTreeMap<String, u32>,
    pub affected_templates: BTreeSet<String>,
    pub tiers: Vec<EncounterTier>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub surge: Option<EncounterSurge>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncounterTier {
    pub remaining_at_most: u32,
    pub modifier: EncounterModifier,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncounterSurge {
    pub duration_ms: u64,
    pub modifier: EncounterModifier,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncounterModifier {
    pub stats_bps: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub loot_pools: BTreeMap<String, String>,
}

impl EncounterProgress {
    pub(crate) fn modifiers(&self) -> impl Iterator<Item = &EncounterModifier> {
        self.tiers
            .iter()
            .map(|tier| &tier.modifier)
            .chain(self.surge.iter().map(|surge| &surge.modifier))
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(rule) = &c.encounter_progress else {
        return Ok(());
    };
    if c.protocol_version() != 6
        || !matches!(c.mechanism_version, 105..=106)
        || c.combat_rules.is_none()
        || c.training_targets().is_some()
        || c.members.is_none()
        || c.pve.is_none()
    {
        return Err("CONFIG_ENCOUNTER_CAPABILITY");
    }
    let pve = c.pve.as_ref().unwrap();
    let ids = crate::world::two_members::World::npc_ids(c);
    let profiles = pve.build_profiles();
    let exists = |id: &String| {
        id == &pve.template_id && !pve.monster_positions.is_empty()
            || pve
                .extra_spawns
                .iter()
                .any(|spawn| &spawn.template_id == id)
    };
    if rule.initial == 0
        || rule.kill_weights.is_empty()
        || rule
            .kill_weights
            .iter()
            .any(|(id, &weight)| !ids.contains(id) || weight == 0 || weight > rule.initial)
        || rule.affected_templates.is_empty()
        || !rule.affected_templates.iter().all(exists)
        || rule.tiers.len() > 8
        || rule.tiers.is_empty() && rule.surge.is_none()
        || rule
            .tiers
            .iter()
            .any(|tier| tier.remaining_at_most > rule.initial)
        || rule
            .tiers
            .windows(2)
            .any(|pair| pair[0].remaining_at_most >= pair[1].remaining_at_most)
        || rule
            .surge
            .as_ref()
            .is_some_and(|s| !(c.step_ms..=c.terminal_ms).contains(&s.duration_ms))
    {
        return Err("CONFIG_ENCOUNTER_PROGRESS");
    }
    for modifier in rule.modifiers() {
        if !(1..=100_000).contains(&modifier.stats_bps)
            || rule.affected_templates.iter().any(|id| {
                ["hp", "atk", "def"].into_iter().any(|stat| {
                    u64::from(profiles[id].stats[stat]) * u64::from(modifier.stats_bps) / 10000
                        > u64::from(u32::MAX)
                })
            })
            || modifier.loot_pools.iter().any(|(id, pool)| {
                !rule.affected_templates.contains(id)
                    || c.world_loot.as_ref().is_none_or(|loot| {
                        !loot.monster_pools.contains_key(id)
                            || loot
                                .pools
                                .get(pool)
                                .is_none_or(|pool| pool.max_stacks() > loot.one_shot_capacity)
                    })
            })
        {
            return Err("CONFIG_ENCOUNTER_MODIFIER");
        }
    }
    Ok(())
}
