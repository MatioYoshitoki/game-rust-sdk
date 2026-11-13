//! Explicit, one-stage result facts. No result can authorize another finisher.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FinisherModeFilter {
    Execute,
    PowerDamage,
    Any,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FinisherSelector {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub enhanced: Option<bool>,
    pub trigger_id: String,
    #[serde(deserialize_with = "mode")]
    pub mode: FinisherModeFilter,
}
// Serde's external unit-enum representation also accepts {"execute":null}.
// This selector is a string token in the published configuration schema.
fn mode<'de, D: serde::Deserializer<'de>>(d: D) -> Result<FinisherModeFilter, D::Error> {
    match String::deserialize(d)?.as_str() {
        "execute" => Ok(FinisherModeFilter::Execute),
        "power_damage" => Ok(FinisherModeFilter::PowerDamage),
        "any" => Ok(FinisherModeFilter::Any),
        value => Err(serde::de::Error::unknown_variant(
            value,
            &["execute", "power_damage", "any"],
        )),
    }
}
impl FinisherSelector {
    pub(crate) fn matches(&self, mode: crate::protocol::FinisherMode) -> bool {
        self.mode == FinisherModeFilter::Any
            || matches!(
                (self.mode, mode),
                (
                    FinisherModeFilter::Execute,
                    crate::protocol::FinisherMode::Execute
                ) | (
                    FinisherModeFilter::PowerDamage,
                    crate::protocol::FinisherMode::PowerDamage
                )
            )
    }
    pub(crate) fn leaf(effect: &TriggerEffect) -> bool {
        matches!(
            effect,
            TriggerEffect::HealFromDamage { .. }
                | TriggerEffect::RestoreSpirit { .. }
                | TriggerEffect::ApplySelfStatus { .. }
                | TriggerEffect::ReduceSkillCooldowns { .. }
                | TriggerEffect::ChainDamage { .. }
        )
    }
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    for t in &r.triggers {
        if t.when != CombatFact::FinisherKill {
            if t.finisher.is_some() {
                return Err("CONFIG_FINISHER_RESULT_FACT");
            }
            continue;
        }
        let Some(selector) = &t.finisher else {
            return Err("CONFIG_FINISHER_RESULT_SELECTOR");
        };
        if !r.triggers.iter().any(|s| {
            s.id == selector.trigger_id
                && matches!(&s.effect, TriggerEffect::FinishOther {rule} if selector.enhanced.is_none() || rule.consumed_status_bonus.is_some())
                && s.profiles.iter().any(|p| t.profiles.contains(p))
        }) || t.include_periodic
            || t.counting == TriggerCounting::ActionTotal
            || t.counter_scope != TriggerScope::Owner
            || t.conditions.other_hp_at_most_bps.is_some()
            || t.conditions
                .status
                .as_ref()
                .is_some_and(|s| matches!(s.subject, TriggerSubject::Other))
            || !FinisherSelector::leaf(&t.effect)
        {
            return Err("CONFIG_FINISHER_RESULT_POLICY");
        }
    }
    Ok(())
}
