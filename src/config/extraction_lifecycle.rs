use super::*;
use crate::protocol::valid_id;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExtractionLifecycle {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub broadcast_channels: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub pressure: Option<ExtractionPressure>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExtractionPressure {
    pub grace_ms: u64,
    pub damage_interval_ms: u64,
    pub damage_max_hp_bps: u32,
    pub warnings: Vec<ExtractionWarning>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExtractionWarning {
    pub id: String,
    pub remaining_ms: u64,
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(lifecycle) = &c.extraction_lifecycle else {
        return Ok(());
    };
    if c.protocol_version() != 6
        || c.mechanism_version != 105
        || c.members.is_none()
        || c.pve.is_none()
        || c.combat_rules.is_none()
        || c.match_mode.is_some()
        || c.training_targets().is_some()
    {
        return Err("CONFIG_EXTRACTION_LIFECYCLE_CAPABILITY");
    }
    if !lifecycle.broadcast_channels && lifecycle.pressure.is_none()
        || lifecycle.broadcast_channels && c.extraction_channel_ms == 0
    {
        return Err("CONFIG_EXTRACTION_LIFECYCLE");
    }
    let Some(rule) = &lifecycle.pressure else {
        return Ok(());
    };
    let mut ids = BTreeSet::new();
    if rule.grace_ms >= c.terminal_ms
        || c.extraction_unlock_ms >= c.terminal_ms - rule.grace_ms
        || !(c.step_ms..rule.grace_ms).contains(&rule.damage_interval_ms)
        || (c.terminal_ms - rule.grace_ms + rule.damage_interval_ms).div_ceil(c.step_ms) * c.step_ms
            >= c.terminal_ms
        || !(1..=10000).contains(&rule.damage_max_hp_bps)
        || rule.warnings.len() > 8
        || rule
            .warnings
            .windows(2)
            .any(|w| w[0].remaining_ms <= w[1].remaining_ms)
        || rule.warnings.iter().any(|w| {
            !(rule.grace_ms + 1..c.terminal_ms).contains(&w.remaining_ms)
                || !valid_id(&w.id)
                || w.id.len() > 64
                || !ids.insert(&w.id)
        })
    {
        return Err("CONFIG_EXTRACTION_PRESSURE");
    }
    Ok(())
}
