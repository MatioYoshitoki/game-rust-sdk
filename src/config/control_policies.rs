//! Bounded target-profile control resolution. Content identifiers stay in tables.
use super::*;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ControlPolicy {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub max_duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub replacements: BTreeMap<String, String>,
}
pub(crate) fn hard_control(d: &StatusDefinition) -> bool {
    d.attribute
        .is_some_and(crate::protocol::StatusAttribute::is_control_flag)
        || d.flags.iter().any(|a| a.is_control_flag())
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    let defs = &c.pve.as_ref().unwrap().status_definitions;
    if r.control_policies.len() > 64
        || r.control_policies
            .values()
            .map(|p| p.replacements.len())
            .sum::<usize>()
            > 128
    {
        return Err("CONFIG_CONTROL_POLICY_BUDGET");
    }
    for (profile, p) in &r.control_policies {
        if !r.profiles.contains_key(profile)
            || p.max_duration_ms
                .is_some_and(|n| !(1..=3600000).contains(&n))
            || p.max_duration_ms.is_none() && p.replacements.is_empty()
            || p.replacements.len() > 16
        {
            return Err("CONFIG_CONTROL_POLICY");
        }
        for (from, to) in &p.replacements {
            let Some(original) = defs.iter().find(|d| d.id == *from) else {
                return Err("CONFIG_CONTROL_REFERENCE");
            };
            let Some(replacement) = defs.iter().find(|d| d.id == *to) else {
                return Err("CONFIG_CONTROL_REFERENCE");
            };
            if !hard_control(original)
                || r.status_cleanses.contains_key(from)
                || !matches!(
                    replacement.mechanism,
                    StatusMechanism::TimedAttributeModifier
                        | StatusMechanism::TimedCompoundModifier
                )
                || !replacement.flags.is_empty()
                || replacement.attribute
                    == Some(crate::protocol::StatusAttribute::SpiritRecoveryLock)
                || !replacement.source_scoped
                || !matches!(
                    replacement.duplicate_policy,
                    DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
                )
                || replacement.max_stacks.is_some()
                || !replacement.is_harmful()
                || r.status_windows.contains_key(to)
                || r.status_cleanses.contains_key(to)
                || [from, to].iter().any(|id| {
                    r.status_policies
                        .get(*id)
                        .is_none_or(|p| p.disposition != StatusDisposition::Harmful)
                })
            {
                return Err("CONFIG_CONTROL_REPLACEMENT");
            }
        }
    }
    Ok(())
}
