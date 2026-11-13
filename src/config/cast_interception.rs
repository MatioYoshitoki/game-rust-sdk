//! One-shot status gates at the final native skill commit, before payment.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CastInterception {
    pub action_filter: ActionFilter,
    pub default: CastInterceptionEffect,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profile_overrides: BTreeMap<String, CastInterceptionEffect>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CastInterceptionEffect {
    Interrupt { status_id: String },
    AllowAndApply { status_id: String },
}
impl CastInterceptionEffect {
    pub(crate) fn followup(&self) -> &str {
        match self {
            Self::Interrupt { status_id } | Self::AllowAndApply { status_id } => status_id,
        }
    }
    pub(crate) fn interrupts(&self) -> bool {
        matches!(self, Self::Interrupt { .. })
    }
}
fn simple_harmful<'a>(c: &'a Config, id: &str) -> Option<&'a StatusDefinition> {
    let r = c.combat_rules.as_ref()?;
    let d = c
        .pve
        .as_ref()?
        .status_definitions
        .iter()
        .find(|d| d.id == id)?;
    (d.source_scoped
        && d.is_harmful()
        && matches!(
            d.mechanism,
            StatusMechanism::TimedAttributeModifier
                | StatusMechanism::TimedCompoundModifier
                | StatusMechanism::TimedControl
        )
        && !d.attribute.is_some_and(|a| a.is_immunity_flag())
        && !d.flags.iter().any(|a| a.is_immunity_flag())
        && r.status_policies
            .get(id)
            .is_some_and(|p| p.disposition == StatusDisposition::Harmful)
        && !r.status_ends.contains_key(id)
        && !r.status_windows.contains_key(id)
        && !r.status_cleanses.contains_key(id)
        && !r.contact_reactions.contains_key(id))
    .then_some(d)
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.cast_interceptions.len() > 16 {
        return Err("CONFIG_CAST_INTERCEPTION_BUDGET");
    }
    for (id, rule) in &r.cast_interceptions {
        if rule.action_filter.basic
            || !rule.action_filter.valid(c)
            || rule.profile_overrides.len() > 16
            || rule
                .profile_overrides
                .keys()
                .any(|id| !r.profiles.contains_key(id))
            || simple_harmful(c, id).is_none_or(|d| {
                d.is_hostile_control()
                    || !d.flags.is_empty()
                    || d.attribute.is_some_and(|a| a.is_control_flag())
                    || d.modifier_bps != 0
                    || d.modifiers
                        .iter()
                        .chain(&d.base_modifiers)
                        .any(|m| m.modifier_bps != 0)
                    || d.stack_counter.is_some()
                    || d.max_stacks.is_some()
                    || matches!(d.duplicate_policy, DuplicatePolicy::Stack)
            })
            || r.control_policies
                .values()
                .any(|p| p.replacements.values().any(|to| to == id))
        {
            return Err("CONFIG_CAST_INTERCEPTION_MARK");
        }
        for effect in std::iter::once(&rule.default).chain(rule.profile_overrides.values()) {
            let key = effect.followup();
            if r.cast_interceptions.contains_key(key)
                || r.control_policies.values().any(|p| {
                    p.replacements
                        .get(key)
                        .is_some_and(|actual| simple_harmful(c, actual).is_none())
                })
                || simple_harmful(c, key).is_none_or(|d| {
                    !effect.interrupts()
                        && (super::hard_control(d)
                            || d.attribute
                                .into_iter()
                                .chain(
                                    d.modifiers
                                        .iter()
                                        .chain(&d.base_modifiers)
                                        .map(|m| m.attribute),
                                )
                                .any(|a| {
                                    matches!(
                                        a,
                                        crate::protocol::StatusAttribute::MaxHp
                                            | crate::protocol::StatusAttribute::MaxSpirit
                                    )
                                }))
                })
            {
                return Err("CONFIG_CAST_INTERCEPTION_FOLLOWUP");
            }
        }
    }
    Ok(())
}
