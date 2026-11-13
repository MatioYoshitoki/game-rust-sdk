//! Trusted member grants are separate from ordinary player actions.
use super::*;
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillLoadout {
    pub owned_skill_ids: Vec<String>,
    pub equipped_skill_ids: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetSkillLoadout {
    pub key: MatchKey,
    pub participant_id: String,
    pub entity_generation: u32,
    pub operation_id: String,
    pub expected_revision: u32,
    pub loadout: SkillLoadout,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillLoadoutReceipt {
    pub operation_id: String,
    pub revision: u32,
    pub equipped_skill_ids: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillLoadoutState {
    pub loadout: SkillLoadout,
    pub revision: u32,
    pub declared: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillLoadoutOperation {
    pub request: SetSkillLoadout,
    pub receipt: SkillLoadoutReceipt,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillControlCheckpoint {
    pub members: BTreeMap<String, SkillLoadoutState>,
    pub operations: Vec<SkillLoadoutOperation>,
}
