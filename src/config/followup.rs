//! Opt-in, bounded admission of one cast at the end of recovery.
use super::{Config, PlayerSkillMechanism, SkillConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_FOLLOWUP_WINDOW_MS: u64 = 1000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FollowupConfig {
    pub window_ms: u64,
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    if let Some(definition) = document.get("skill_followup") {
        if document["schema_version"] != 7
            || !matches!(document["mechanism_version"].as_u64(), Some(51..=107))
        {
            return Err("CONFIG_FOLLOWUP_CAPABILITY");
        }
        if !definition.is_object() {
            return Err("CONFIG_FOLLOWUP_SHAPE");
        }
    }
    Ok(())
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    if let Some(definition) = config.skill_followup
        && (config.step_ms == 0
            || !(config.step_ms..=MAX_FOLLOWUP_WINDOW_MS).contains(&definition.window_ms)
            || !definition.window_ms.is_multiple_of(config.step_ms))
    {
        return Err("CONFIG_FOLLOWUP_BOUNDS");
    }
    Ok(())
}

impl SkillConfig {
    pub(crate) fn supports_followup(&self) -> bool {
        self.mechanism == PlayerSkillMechanism::Projectile
            && self.charge.is_none()
            && self.channel.is_none()
            && self.recast.is_none()
    }
}
