//! Per-skill opt-in to ending recovery at a fixed logical step.
use super::Config;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryCancelConfig {
    pub after_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub by_move: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub by_explicit_action: Option<bool>,
}
impl RecoveryCancelConfig {
    pub(crate) fn expanded(self) -> bool {
        self.by_move.is_some() || self.by_explicit_action.is_some()
    }
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    let skills = super::player_skills::raw(document);
    for skill in skills {
        if let Some(definition) = skill.get("recovery_cancel") {
            if document["schema_version"] != 7
                || !matches!(document["mechanism_version"].as_u64(), Some(52..=106))
            {
                return Err("CONFIG_RECOVERY_CANCEL_CAPABILITY");
            }
            if !definition.is_object() {
                return Err("CONFIG_RECOVERY_CANCEL_SHAPE");
            }
            if (definition.get("by_move").is_some()
                || definition.get("by_explicit_action").is_some())
                && document["mechanism_version"].as_u64().unwrap_or(0) < 105
            {
                return Err("CONFIG_CAST_LIFECYCLE_CAPABILITY");
            }
        }
    }
    Ok(())
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    for skill in config.player_skills() {
        if let Some(definition) = skill.recovery_cancel
            && ((!skill.supports_followup() && !definition.expanded())
                || config.step_ms == 0
                || !definition.after_ms.is_multiple_of(config.step_ms)
                || definition
                    .after_ms
                    .checked_add(config.step_ms)
                    .is_none_or(|end| end > skill.recovery_ms))
        {
            return Err("CONFIG_RECOVERY_CANCEL_BOUNDS");
        }
    }
    Ok(())
}
