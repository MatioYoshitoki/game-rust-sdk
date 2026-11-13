//! Native HOT ticks and status expiry select bounded whole-instance dispels.
use super::*;
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CleanseScope {
    All,
    SinceApplication,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CleanseAction {
    pub disposition: StatusDisposition,
    pub max_count: usize,
    pub scope: CleanseScope,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusCleanseRule {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub on_tick: Option<CleanseAction>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub on_expired: Option<CleanseAction>,
}
pub(super) fn default_budget() -> usize {
    32
}
pub(super) fn is_default_budget(n: &usize) -> bool {
    *n == default_budget()
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.status_cleanses.len() > 16 || !(1..=128).contains(&r.max_cleanse_operations_per_step) {
        return Err("CONFIG_STATUS_CLEANSE_BUDGET");
    }
    for (id, rule) in &r.status_cleanses {
        let d = c
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .find(|d| d.id == *id)
            .ok_or("CONFIG_STATUS_CLEANSE_REFERENCE")?;
        if !d.source_scoped
            || !matches!(
                d.duplicate_policy,
                DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
            )
            || d.max_stacks.is_some()
            || d.is_shield()
            || d.is_periodic() && !d.is_heal()
            || rule.on_tick.is_some() && !d.is_heal()
        {
            return Err("CONFIG_STATUS_CLEANSE_CARRIER");
        }
        if rule.on_tick.is_none() && rule.on_expired.is_none()
            || rule
                .on_tick
                .iter()
                .chain(rule.on_expired.iter())
                .any(|a| !(1..=16).contains(&a.max_count))
        {
            return Err("CONFIG_STATUS_CLEANSE_POLICY");
        }
    }
    Ok(())
}
