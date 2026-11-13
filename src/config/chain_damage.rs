//! Finite derived contact paths; all gameplay coefficients belong to configuration.
use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChainFormula {
    pub fixed: u32,
    pub source_attack_bps: u32,
    pub fact_amount_bps: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChainDirection {
    #[default]
    All,
    Forward,
}
impl ChainDirection {
    fn all(&self) -> bool {
        *self == Self::All
    }
    pub(crate) fn allows(self, from: [f64; 2], point: [f64; 2], candidate: [f64; 2]) -> bool {
        self == Self::All
            || (candidate[0] - point[0]) * (point[0] - from[0])
                + (candidate[1] - point[1]) * (point[1] - from[1])
                > 0.
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChainDamageRule {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub source_skill_id: Option<String>,

    #[serde(default, skip_serializing_if = "ChainDirection::all")]
    pub direction: ChainDirection,
    pub formula: ChainFormula,
    pub max_hops: usize,
    pub radius: f64,
    pub retention_bps: u32,
    pub final_multiplier_bps: u32,
    pub damage_kind: DamageKind,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub element: Option<String>,
    pub ignore_armor: bool,
    pub armor_penetration_cap_bps: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_hit_target_statuses: Vec<String>,
}
pub(super) fn default_budget() -> usize {
    32
}
pub(super) fn default_value(n: &usize) -> bool {
    *n == 32
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if !(1..=128).contains(&r.max_chain_hits_per_step) {
        return Err("CONFIG_CHAIN_BUDGET");
    }
    for t in &r.triggers {
        let TriggerEffect::ChainDamage { rule: d } = &t.effect else {
            continue;
        };
        if !matches!(
            t.when,
            CombatFact::DirectHit
                | CombatFact::ContactStatusConsumed
                | CombatFact::FinisherKill
                | CombatFact::DamageDealt
                | CombatFact::DamageTaken
                | CombatFact::ShieldAbsorbed
                | CombatFact::Kill
        ) || t.counting == TriggerCounting::ActionTotal
            || !(1..=8).contains(&d.max_hops)
            || !d.radius.is_finite()
            || !(0.0..=2000.0).contains(&d.radius)
            || d.radius == 0.
            || d.retention_bps > 10000
            || !(1..=100000).contains(&d.final_multiplier_bps)
            || d.armor_penetration_cap_bps > 10000
            || d.formula.fixed > 1000000
            || d.formula.source_attack_bps > 100000
            || d.formula.fact_amount_bps > 10000
            || d.formula.fixed == 0
                && d.formula.source_attack_bps == 0
                && d.formula.fact_amount_bps == 0
            || d.element.as_ref().is_some_and(|e| !r.elements.contains(e))
            || d.damage_kind == DamageKind::Elemental && d.element.is_none()
            || d.damage_kind == DamageKind::TrueDamage && d.element.is_some()
        {
            return Err("CONFIG_CHAIN_POLICY");
        }
        if d.on_hit_target_statuses.len() > 2
            || d.on_hit_target_statuses
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != d.on_hit_target_statuses.len()
            || d.on_hit_target_statuses.iter().any(|id| {
                c.pve
                    .as_ref()
                    .unwrap()
                    .status_definitions
                    .iter()
                    .find(|d| d.id == *id)
                    .is_none_or(|d| {
                        !d.source_scoped
                            || !d.is_harmful()
                            || d.attribute.is_some_and(|a| a.is_immunity_flag())
                            || !matches!(
                                d.mechanism,
                                StatusMechanism::TimedAttributeModifier
                                    | StatusMechanism::TimedCompoundModifier
                                    | StatusMechanism::TimedControl
                            )
                            || d.flags.iter().any(|a| !a.is_control_flag())
                            || r.status_windows.contains_key(id)
                            || r.status_cleanses.contains_key(id)
                    })
                    || r.status_policies
                        .get(id)
                        .is_none_or(|p| p.disposition != StatusDisposition::Harmful)
            })
        {
            return Err("CONFIG_CHAIN_STATUS");
        }
    }
    Ok(())
}
