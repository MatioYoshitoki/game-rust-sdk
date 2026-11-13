//! Explicit counters are persistent carrier state, distinct from Stack duplication.
use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StackCounterConfig {
    #[serde(default, skip_serializing_if = "CounterZeroPolicy::is_keep")]
    pub zero_policy: CounterZeroPolicy,
    pub initial: u32,
    pub maximum: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CounterZeroPolicy {
    #[default]
    Keep,
    Remove,
}
impl CounterZeroPolicy {
    fn is_keep(&self) -> bool {
        *self == Self::Keep
    }
}
impl StackCounterConfig {
    pub(crate) fn removes_at_zero(&self) -> bool {
        self.zero_policy == CounterZeroPolicy::Remove
    }
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    for d in c.pve.iter().flat_map(|p| &p.status_definitions) {
        if (!d.base_modifiers.is_empty() || d.stack_counter.is_some())
            && (c.schema_version != 7
                || c.mechanism_version < 105
                || c.combat_rules.is_none()
                || !matches!(d.mechanism, StatusMechanism::TimedCompoundModifier))
        {
            return Err("CONFIG_STACK_COUNTER_CAPABILITY");
        }
        if let Some(counter) = &d.stack_counter
            && (!d.source_scoped
                || !d.flags.is_empty()
                || d.max_stacks.is_some()
                || !matches!(
                    d.duplicate_policy,
                    DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
                )
                || !(1..=128).contains(&counter.maximum)
                || counter.initial > counter.maximum
                || counter.removes_at_zero() && counter.initial == 0
                || d.modifiers
                    .iter()
                    .chain(&d.base_modifiers)
                    .any(|m| m.attribute == crate::protocol::StatusAttribute::MovementSlow)
                    && counter.maximum > 32)
        {
            return Err("CONFIG_STACK_COUNTER_POLICY");
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StackThreshold {
    pub definition_id: String,
    pub at_least: u32,
}
pub(super) fn is_false(v: &bool) -> bool {
    !*v
}
pub(crate) fn threshold_leaf(c: &Config, effect: &super::TriggerEffect) -> bool {
    match effect {
        super::TriggerEffect::DispelStatuses { .. }
        | super::TriggerEffect::RestoreSpirit { .. } => true,
        super::TriggerEffect::ApplySelfStatus { status_id } => {
            super::status_ends::derived_leaf(c, status_id, true)
        }
        _ => false,
    }
}
pub(super) fn validate_triggers(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    let defs = &c.pve.as_ref().unwrap().status_definitions;
    for t in &r.triggers {
        let is_threshold = t.when == super::CombatFact::StatusStacksReached;
        if is_threshold != t.stack_change.is_some() {
            return Err("CONFIG_STACK_THRESHOLD_SELECTOR");
        }
        if let Some(selector) = &t.stack_change
            && (t.include_periodic
                || t.counting != super::TriggerCounting::Contact
                || !threshold_leaf(c, &t.effect)
                || !defs.iter().any(|d| {
                    d.id == selector.definition_id
                        && d.stack_counter
                            .as_ref()
                            .is_some_and(|s| (1..=s.maximum).contains(&selector.at_least))
                }))
        {
            return Err("CONFIG_STACK_THRESHOLD_POLICY");
        }

        if let super::TriggerEffect::AddStatusStacks {
            status_id, amount, ..
        } = &t.effect
            && (!(1..=128).contains(amount)
                || !defs
                    .iter()
                    .any(|d| d.id == *status_id && d.stack_counter.is_some())
                || !r.status_policies.contains_key(status_id))
        {
            return Err("CONFIG_STACK_ADJUSTMENT_RULE");
        }
    }
    Ok(())
}
