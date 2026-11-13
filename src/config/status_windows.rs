//! A status owns its lifetime; a window only observes facts and resolves its end.
use super::*;
use crate::protocol::{WindowBasis, WindowMaximumSampling};
pub(crate) const MAX_COMBAT_RECORD_AMOUNT: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WindowRecovery {
    pub fixed: u32,
    pub recorded_bps: u32,
    pub max_resource_bps: u32,
    pub cap_max_resource_bps: u32,
}
impl WindowRecovery {
    pub(crate) fn amount(&self, recorded: u64, maximum: u32) -> u32 {
        let amount = u128::from(self.fixed)
            + (u128::from(recorded) * u128::from(self.recorded_bps)
                + u128::from(maximum) * u128::from(self.max_resource_bps))
                / 10000;
        amount
            .min(u128::from(maximum) * u128::from(self.cap_max_resource_bps) / 10000)
            .min(u128::from(u32::MAX)) as u32
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusWindowRule {
    pub basis: WindowBasis,
    pub include_periodic: bool,
    pub record_cap: u64,
    pub refresh: RecordRefresh,
    pub maximum_sampling: WindowMaximumSampling,
    pub on_expired: bool,
    pub on_consumed: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub hp: Option<WindowRecovery>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub spirit: Option<WindowRecovery>,
}
pub(super) fn default_release_budget() -> usize {
    32
}
pub(super) fn is_default_release_budget(n: &usize) -> bool {
    *n == default_release_budget()
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.status_windows.len() > 16 || !(1..=128).contains(&r.max_window_releases_per_step) {
        return Err("CONFIG_STATUS_WINDOW_BUDGET");
    }
    let defs = &c.pve.as_ref().unwrap().status_definitions;
    for (id, w) in &r.status_windows {
        let Some(d) = defs.iter().find(|d| d.id == *id) else {
            return Err("CONFIG_STATUS_WINDOW_REFERENCE");
        };
        if !d.flags.is_empty()
            || d.attribute == Some(crate::protocol::StatusAttribute::SpiritRecoveryLock)
            || !d.source_scoped
            || !matches!(
                d.duplicate_policy,
                DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
            )
            || !(matches!(
                d.mechanism,
                StatusMechanism::TimedAttributeModifier | StatusMechanism::TimedCompoundModifier
            ) || d.is_heal())
        {
            return Err("CONFIG_STATUS_WINDOW_CARRIER");
        }
        if w.record_cap > MAX_COMBAT_RECORD_AMOUNT
            || (w.basis == WindowBasis::None && (w.record_cap != 0 || w.include_periodic))
            || (w.basis != WindowBasis::None && w.record_cap == 0)
            || w.hp.is_none() && w.spirit.is_none()
        {
            return Err("CONFIG_STATUS_WINDOW_POLICY");
        }
        for f in w.hp.iter().chain(w.spirit.iter()) {
            if f.fixed > 1000000
                || f.recorded_bps > 100000
                || f.max_resource_bps > 10000
                || !(1..=10000).contains(&f.cap_max_resource_bps)
                || f.fixed == 0 && f.recorded_bps == 0 && f.max_resource_bps == 0
                || w.basis == WindowBasis::None && f.recorded_bps != 0
            {
                return Err("CONFIG_STATUS_WINDOW_RECOVERY");
            }
        }
    }
    Ok(())
}
