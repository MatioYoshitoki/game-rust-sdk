use super::Config;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_MEMBER_LOOT_DROPS: usize = 50;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberLootDropConfig {
    pub max_active: usize,
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    if let Some(definition) = document.get("member_loot_drop") {
        if document["schema_version"] != 7
            || !matches!(document["mechanism_version"].as_u64(), Some(57..=106))
        {
            return Err("CONFIG_MEMBER_LOOT_DROP_CAPABILITY");
        }
        if !definition.is_object() {
            return Err("CONFIG_MEMBER_LOOT_DROP_SHAPE");
        }
    }
    Ok(())
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    if config
        .member_loot_drop
        .is_some_and(|definition| !(1..=MAX_MEMBER_LOOT_DROPS).contains(&definition.max_active))
    {
        return Err("CONFIG_MEMBER_LOOT_DROP_BOUNDS");
    }
    Ok(())
}
