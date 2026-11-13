//! Optional, fixed-step pursuit of selected NPCs.
use super::Config;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_BASIC_PURSUIT_MS: u64 = 1800;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicPursuitConfig {
    pub max_range: f64,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_max_range: Option<f64>,
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    if let Some(definition) = document.get("basic_pursuit") {
        if document["schema_version"] != 7
            || !matches!(document["mechanism_version"].as_u64(), Some(55..=105))
        {
            return Err("CONFIG_BASIC_PURSUIT_CAPABILITY");
        }
        if !definition.is_object() {
            return Err("CONFIG_BASIC_PURSUIT_SHAPE");
        }
        if let Some(range) = definition.get("auto_max_range") {
            if !matches!(document["mechanism_version"].as_u64(), Some(56..=105)) {
                return Err("CONFIG_BASIC_PURSUIT_CAPABILITY");
            }
            if !range.is_number() {
                return Err("CONFIG_BASIC_PURSUIT_SHAPE");
            }
        }
    }
    Ok(())
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    if let Some(definition) = config.basic_pursuit
        && (!definition.max_range.is_finite()
            || config
                .basic_configs()
                .any(|basic| definition.max_range <= basic.range)
            || definition.max_range > 10000.
            || definition.auto_max_range.is_some_and(|range| {
                !range.is_finite()
                    || config.basic_configs().any(|basic| range <= basic.range)
                    || range > definition.max_range
            })
            || config.step_ms == 0
            || !(config.step_ms..=MAX_BASIC_PURSUIT_MS).contains(&definition.duration_ms)
            || !definition.duration_ms.is_multiple_of(config.step_ms))
    {
        return Err("CONFIG_BASIC_PURSUIT_BOUNDS");
    }
    Ok(())
}
