//! Trusted raw growth facts. None of these fields is a player Action.
use super::*;
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerGrowth {
    pub model_id: String,
    pub rank: usize,
    pub quality: usize,
    pub level: usize,
    pub mastery: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatBuild {
    pub power_points: u32,
    pub skill_power_points: u32,
    pub element_resistance_points: u32,
    pub powers: BTreeMap<String, PowerGrowth>,
}
