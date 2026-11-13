//! Native retaliation behavior attached to one self-status instance.
use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetaliationRule {
    pub chance_bps: u32,
    pub source_cooldown_ms: u64,
    pub max_sources: usize,
    pub reply_status_id: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dispel_beneficial: usize,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub on_expired_status_id: Option<String>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

pub(super) fn validate(c: &Config, rules: &CombatRules) -> Result<(), &'static str> {
    let table = &rules.status_retaliations;
    if table.is_empty() {
        return Ok(());
    }
    let defs = &c
        .pve
        .as_ref()
        .ok_or("CONFIG_COMBAT_PVE")?
        .status_definitions;
    let leaf = |id: &str| {
        rules
            .status_policies
            .get(id)
            .is_some_and(|p| p.disposition == StatusDisposition::Harmful)
            && super::status_ends::derived_leaf(c, id, false)
            && defs.iter().any(|d| d.id == id && !d.is_periodic())
    };
    let reply = |id: &str| {
        leaf(id)
            && rules.control_policies.values().all(|p| {
                p.replacements
                    .get(id)
                    .is_none_or(|replacement| leaf(replacement))
            })
    };
    if c.protocol_version() != 6
        || table.len() > 16
        || table.iter().any(|(id, rule)| {
            !defs.iter().any(|d| {
                d.id == *id
                    && d.source_scoped
                    && d.stack_counter.is_none()
                    && matches!(
                        d.mechanism,
                        StatusMechanism::TimedAttributeModifier
                            | StatusMechanism::TimedCompoundModifier
                    )
                    && matches!(
                        d.duplicate_policy,
                        DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
                    )
                    && !d.is_hostile_control()
            }) || !rules
                .status_policies
                .get(id)
                .is_some_and(|p| p.disposition == StatusDisposition::Beneficial && !p.transferable)
                || rule.chance_bps > 10000
                || !(c.step_ms..=60000).contains(&rule.source_cooldown_ms)
                || !(1..=32).contains(&rule.max_sources)
                || rule.dispel_beneficial > 16
                || !reply(&rule.reply_status_id)
                || rule
                    .on_expired_status_id
                    .as_deref()
                    .is_some_and(|id| !reply(id))
        })
    {
        return Err("CONFIG_STATUS_RETALIATION");
    }
    Ok(())
}
