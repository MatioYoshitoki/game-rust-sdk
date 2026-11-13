//! Catalog definitions do not grant ownership or add equipped slots.
use super::*;
use crate::protocol::{SkillLoadout, SkillLoadoutState};
use std::collections::BTreeSet;
pub(crate) const MAX_SKILL_DEFINITIONS: usize = 64;
pub(super) fn fields(v: &serde_json::Value) -> Result<(), &'static str> {
    if let Some(library) = v.get("skill_library")
        && (v["schema_version"] != 7
            || v["mechanism_version"] != 105
            || library
                .as_array()
                .is_none_or(|a| a.is_empty() || a.len() >= MAX_SKILL_DEFINITIONS))
    {
        return Err("CONFIG_SKILL_LIBRARY_CAPABILITY");
    }
    Ok(())
}
pub(crate) fn valid_loadout(c: &Config, loadout: &SkillLoadout) -> bool {
    loadout.owned_skill_ids.len() <= MAX_SKILL_DEFINITIONS
        && loadout.equipped_skill_ids.len() <= MAX_MEMBER_SKILLS
        && loadout
            .owned_skill_ids
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            == loadout.owned_skill_ids.len()
        && loadout
            .equipped_skill_ids
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            == loadout.equipped_skill_ids.len()
        && loadout
            .owned_skill_ids
            .iter()
            .all(|id| c.player_skill(id).is_some())
        && loadout
            .equipped_skill_ids
            .iter()
            .all(|id| loadout.owned_skill_ids.contains(id))
}
pub(crate) fn initial_skill_loadout(
    c: &Config,
    declared: Option<&SkillLoadout>,
) -> SkillLoadoutState {
    let defaults: Vec<_> = c.default_player_skills().map(|s| s.id.clone()).collect();
    SkillLoadoutState {
        loadout: declared.cloned().unwrap_or(SkillLoadout {
            owned_skill_ids: defaults.clone(),
            equipped_skill_ids: defaults,
        }),
        revision: 0,
        declared: declared.is_some(),
    }
}
