use super::*;
use crate::protocol::valid_id;
use std::collections::BTreeSet;

pub(crate) const MAX_CLAIMS: usize = 8;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContestedEquipment {
    pub nodes: Vec<EquipmentNode>,
    pub interaction_range: f64,
    pub pursuit_range: f64,
    pub retire_delay_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EquipmentNode {
    pub id: String,
    pub position: [f64; 2],
    pub basic_profile: String,
    pub secured_item_id: String,
    pub pursuit_ids: Vec<String>,
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(rule) = &c.contested_equipment else {
        return Ok(());
    };
    let Some(pve) = &c.pve else {
        return Err("CONFIG_CONTESTED_EQUIPMENT_CAPABILITY");
    };
    if c.protocol_version() != 6
        || c.mechanism_version != 105
        || c.members.is_none()
        || c.asset_contract.is_none()
        || c.match_mode.is_some()
        || c.training_targets().is_some()
        || c.combat_rules.is_none()
    {
        return Err("CONFIG_CONTESTED_EQUIPMENT_CAPABILITY");
    }
    let mut nodes = BTreeSet::new();
    let mut items = BTreeSet::new();
    let mut hunters = BTreeSet::new();
    if !(1..=MAX_CLAIMS).contains(&rule.nodes.len())
        || !(0.1..=1000.).contains(&rule.interaction_range)
        || !(0.1..=100_000.).contains(&rule.pursuit_range)
        || !(c.step_ms..=c.terminal_ms).contains(&rule.retire_delay_ms)
        || rule.nodes.iter().any(|node| {
            !valid_id(&node.id)
                || node.id.len() > 64
                || !nodes.insert(&node.id)
                || !pve.contains_world(node.position)
                || !c
                    .basic_profiles()
                    .unwrap()
                    .contains_key(&node.basic_profile)
                || c.instance_definition(&node.secured_item_id).is_none()
                || !items.insert(&node.secured_item_id)
                || c.pve_chest_loot_bound(&node.secured_item_id, c.terminal_ms) > 0
                || c.world_loot
                    .as_ref()
                    .unwrap()
                    .pools
                    .values()
                    .any(|pool| pool.item_bound(&node.secured_item_id) > 0)
                || node.pursuit_ids.len() > 4
                || node.pursuit_ids.iter().any(|id| {
                    !hunters.insert(id)
                        || !pve.extra_spawns.iter().any(|spawn| &spawn.id == id)
                        || c.encounter_progress
                            .as_ref()
                            .is_some_and(|progress| progress.kill_weights.contains_key(id))
                        || c.world_boons.as_ref().is_some_and(|boons| {
                            boons
                                .opportunities
                                .iter()
                                .any(|source| source.guardians.contains(id))
                        })
                        || c.world_loot
                            .as_ref()
                            .unwrap()
                            .containers
                            .iter()
                            .any(|container| container.guardians.contains(id))
                })
        })
    {
        return Err("CONFIG_CONTESTED_EQUIPMENT");
    }
    Ok(())
}

pub(super) fn validate_prepared(c: &Config) -> Result<(), &'static str> {
    if let Some(rule) = &c.contested_equipment
        && !crate::world::pve::validate_walkable_centers(
            c.pve.as_ref().unwrap(),
            rule.nodes.iter().map(|node| node.position),
            c.params.player_radius,
        )
    {
        return Err("CONFIG_CONTESTED_EQUIPMENT_POSITION");
    }
    Ok(())
}

impl Config {
    pub(crate) fn contested_reward(&self, item: &str) -> bool {
        self.contested_equipment
            .as_ref()
            .is_some_and(|rule| rule.nodes.iter().any(|node| node.secured_item_id == item))
    }
}
