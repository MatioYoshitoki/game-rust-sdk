use super::*;
use crate::protocol::{BoonOffer, ItemAmount, StatusAttribute, StatusModifier, valid_id};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MAX_SLOTS: u8 = 3;
pub(crate) const MAX_LEVELS: usize = 3;
pub(crate) const MAX_OPPORTUNITIES: usize = 32;
pub(crate) const MAX_SHARED_USES: u8 = 8;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorldBoons {
    pub capacity: u8,
    pub interaction_range: f64,
    pub definitions: BTreeMap<String, BoonDefinition>,
    pub opportunities: Vec<BoonOpportunity>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoonDefinition {
    pub levels: Vec<Vec<StatusModifier>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spirit_cost_bps: Vec<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub shield: Option<BoonShield>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoonShield {
    pub capacity_bps: Vec<u32>,
    pub rebuild_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BoonScope {
    Shared,
    PerMember,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoonOpportunity {
    pub id: String,
    pub position: [f64; 2],
    pub scope: BoonScope,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub shared_uses: Option<u8>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub requires_checkpoint: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub interaction_ms: Option<u64>,
    pub offer: BoonOffer,
    #[serde(default)]
    pub human_only: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guardians: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub cost: Option<ItemAmount>,
}

fn short_id(id: &str) -> bool {
    valid_id(id) && id.len() <= 64
}

fn valid_levels(levels: &[Vec<StatusModifier>]) -> bool {
    if !(1..=MAX_LEVELS).contains(&levels.len()) {
        return false;
    }
    let mut previous = BTreeMap::new();
    for level in levels {
        let mut modifiers = BTreeMap::new();
        if level.len() > 8
            || level.iter().any(|row| {
                !matches!(
                    row.attribute,
                    StatusAttribute::Atk
                        | StatusAttribute::Def
                        | StatusAttribute::Spd
                        | StatusAttribute::AttackSpeed
                        | StatusAttribute::MaxHp
                        | StatusAttribute::MaxSpirit
                        | StatusAttribute::SkillPowerBonus
                        | StatusAttribute::HealingReceived
                        | StatusAttribute::RecoveryStrength
                        | StatusAttribute::DamageReduction
                ) || !(1..=10000).contains(&row.modifier_bps)
                    || modifiers.insert(row.attribute, row.modifier_bps).is_some()
            })
            || previous
                .iter()
                .any(|(attr, value)| modifiers.get(attr).is_none_or(|next| next < value))
        {
            return false;
        }
        previous = modifiers;
    }
    true
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(boons) = &c.world_boons else {
        return Ok(());
    };
    let Some(pve) = &c.pve else {
        return Err("CONFIG_WORLD_BOONS_CAPABILITY");
    };
    let Some(combat) = &c.combat_rules else {
        return Err("CONFIG_WORLD_BOONS_CAPABILITY");
    };
    if c.protocol_version() != 6 || c.mechanism_version < 105 || pve.status_rules.is_none() {
        return Err("CONFIG_WORLD_BOONS_CAPABILITY");
    }
    if !(1..=MAX_SLOTS).contains(&boons.capacity)
        || !(0.1..=1000.).contains(&boons.interaction_range)
        || !(1..=32).contains(&boons.definitions.len())
        || !(1..=MAX_OPPORTUNITIES).contains(&boons.opportunities.len())
    {
        return Err("CONFIG_WORLD_BOONS_BOUNDS");
    }
    if boons.definitions.iter().any(|(id, def)| {
        !short_id(id)
            || !valid_levels(&def.levels)
            || !def.spirit_cost_bps.is_empty()
                && (def.spirit_cost_bps.len() != def.levels.len()
                    || def
                        .spirit_cost_bps
                        .iter()
                        .any(|bps| !(1..10000).contains(bps))
                    || def.spirit_cost_bps.windows(2).any(|p| p[0] < p[1]))
            || def.shield.as_ref().is_some_and(|shield| {
                shield.capacity_bps.len() != def.levels.len()
                    || shield
                        .capacity_bps
                        .iter()
                        .any(|bps| !(1..=10000).contains(bps))
                    || shield.capacity_bps.windows(2).any(|p| p[0] > p[1])
                    || !(c.step_ms..=3600000).contains(&shield.rebuild_ms)
                    || pve.status_rules.as_ref().unwrap().shield_cap_bps.is_none()
                    || pve.status_definitions.iter().any(|d| d.id == *id)
            })
            || def.levels.first().is_some_and(Vec::is_empty)
                && def.spirit_cost_bps.is_empty()
                && def.shield.is_none()
                && !c.checkpoint_schedules().iter().any(|schedule| {
                    schedule
                        .counter
                        .as_ref()
                        .is_some_and(|rule| rule.uses_boon(id))
                })
                && !combat
                    .triggers
                    .iter()
                    .any(|t| t.requires_boon.as_ref().is_some_and(|gate| gate.id == *id))
    }) {
        return Err("CONFIG_WORLD_BOONS_DEFINITION");
    }
    let npc_ids: BTreeSet<_> = (0..pve.monster_positions.len())
        .map(|i| format!("{}-{}", pve.template_id, i + 1))
        .chain(pve.extra_spawns.iter().map(|spawn| spawn.id.clone()))
        .collect();
    let mut ids = BTreeSet::new();
    for source in &boons.opportunities {
        if !short_id(&source.id)
            || !ids.insert(&source.id)
            || source.interaction_ms.is_some_and(|duration| {
                !(c.step_ms..=60000).contains(&duration) || duration >= c.terminal_ms
            })
            || source.shared_uses.is_some_and(|uses| {
                !matches!(source.scope, BoonScope::Shared)
                    || !(1..=MAX_SHARED_USES).contains(&uses)
                    || c.members
                        .as_ref()
                        .is_none_or(|m| usize::from(uses) > m.capacity)
            })
            || source.requires_checkpoint.as_ref().is_some_and(|id| {
                !c.checkpoint_schedules()
                    .iter()
                    .any(|schedule| schedule.id == *id)
            })
            || !(0..2).all(|axis| {
                (c.geometry.min[axis]..=c.geometry.max[axis]).contains(&source.position[axis])
            })
            || source.guardians.len() > 8
            || source.guardians.iter().collect::<BTreeSet<_>>().len() != source.guardians.len()
            || source.guardians.iter().any(|id| !npc_ids.contains(id))
            || source.cost.as_ref().is_some_and(|cost| {
                !(1..=10000).contains(&cost.quantity)
                    || c.item_limit(&cost.item_id).is_none()
                    || c.instance_definition(&cost.item_id).is_some()
            })
            || match &source.offer {
                BoonOffer::Grant { choices } => {
                    !(1..=8).contains(&choices.len())
                        || choices.iter().collect::<BTreeSet<_>>().len() != choices.len()
                        || choices.iter().any(|id| !boons.definitions.contains_key(id))
                }
                BoonOffer::Upgrade {} => false,
            }
        {
            return Err("CONFIG_WORLD_BOONS_OPPORTUNITY");
        }
    }
    Ok(())
}
