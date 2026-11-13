//! Threshold policies and profile-specific, source-power-capped terminal damage.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThresholdComparison {
    Below,
    AtMost,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum FinishPolicy {
    Execute {},
    PowerDamage {
        fixed: u32,
        power_bps: u32,
        cap_power_bps: u32,
    },
}
impl FinishPolicy {
    pub(crate) fn amount(&self, power: u32) -> Option<u32> {
        match self {
            Self::Execute {} => None,
            Self::PowerDamage {
                fixed,
                power_bps,
                cap_power_bps,
            } => Some(
                (u128::from(*fixed) + u128::from(power) * u128::from(*power_bps) / 10000)
                    .min(u128::from(power) * u128::from(*cap_power_bps) / 10000)
                    .min(u128::from(u32::MAX)) as u32,
            ),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FinishRule {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub source_skill_id: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub consumed_status_bonus: Option<FinishConsumptionBonus>,
    pub hp_threshold_bps: u32,
    pub comparison: ThresholdComparison,
    #[serde(rename = "default")]
    pub default_policy: FinishPolicy,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub profile_overrides: std::collections::BTreeMap<String, FinishPolicy>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FinishConsumptionBonus {
    pub definition_id: String,
    pub hp_threshold_bps_per_stack: u32,
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    for t in &r.triggers {
        let TriggerEffect::FinishOther { rule } = &t.effect else {
            continue;
        };
        if !matches!(
            t.when,
            CombatFact::DirectHit
                | CombatFact::ContactStatusConsumed
                | CombatFact::DamageDealt
                | CombatFact::DamageTaken
                | CombatFact::ShieldAbsorbed
        ) || t.include_periodic
            || t.counting == TriggerCounting::ActionTotal
            || !(1..=10000).contains(&rule.hp_threshold_bps)
            || rule.profile_overrides.len() > 64
            || rule
                .profile_overrides
                .keys()
                .any(|p| !r.profiles.contains_key(p))
        {
            return Err("CONFIG_FINISHER_POLICY");
        }
        if let Some(b) = &rule.consumed_status_bonus
            && (t.when != CombatFact::DirectHit
                || !(1..=10000).contains(&b.hp_threshold_bps_per_stack)
                || !std::iter::once(&r.basic_attack)
                    .chain(r.skills.values())
                    .chain(r.npc_attacks.values())
                    .any(|a| {
                        a.contact_status.as_ref().is_some_and(|s| {
                            s.definition_id == b.definition_id
                                && s.sampling == ContactSampling::Action
                                && matches!(s.subject, TriggerSubject::Owner)
                        })
                    }))
        {
            return Err("CONFIG_FINISHER_CONSUMPTION_BONUS");
        }
        for policy in std::iter::once(&rule.default_policy).chain(rule.profile_overrides.values()) {
            if let FinishPolicy::PowerDamage {
                fixed,
                power_bps,
                cap_power_bps,
            } = policy
                && (*fixed > 1000000
                    || *power_bps > 100000
                    || !(1..=100000).contains(cap_power_bps)
                    || *fixed == 0 && *power_bps == 0)
            {
                return Err("CONFIG_FINISHER_AMOUNT");
            }
        }
    }
    Ok(())
}
