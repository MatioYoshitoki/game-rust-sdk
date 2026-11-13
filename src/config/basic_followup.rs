//! Opt-in to buffering the next basic attack during a committed cooldown.
use super::{Config, MAX_FOLLOWUP_WINDOW_MS};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicFollowupConfig {
    pub grace_ms: u64,
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    if let Some(definition) = document.get("basic_followup") {
        if document["schema_version"] != 7
            || !matches!(document["mechanism_version"].as_u64(), Some(53..=106))
        {
            return Err("CONFIG_BASIC_FOLLOWUP_CAPABILITY");
        }
        if !definition.is_object() {
            return Err("CONFIG_BASIC_FOLLOWUP_SHAPE");
        }
    }
    Ok(())
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    if let Some(definition) = config.basic_followup
        && (config.step_ms == 0
            || definition.grace_ms > MAX_FOLLOWUP_WINDOW_MS
            || !definition.grace_ms.is_multiple_of(config.step_ms))
    {
        return Err("CONFIG_BASIC_FOLLOWUP_BOUNDS");
    }
    Ok(())
}
