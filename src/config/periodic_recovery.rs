//! Resource recovery shares the native HOT scheduler and a single excess-heal budget.
use super::*;
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpiritFormula {
    pub fixed: u32,
    pub source_attack_bps: u32,
    pub target_max_spirit_bps: u32,
}
impl SpiritFormula {
    pub(crate) fn amount(&self, attack: u32, maximum: u32) -> u32 {
        self.amount_layers(attack, maximum, 1)
    }
    pub(crate) fn amount_layers(&self, attack: u32, maximum: u32, layers: u32) -> u32 {
        (u128::from(self.fixed) * u128::from(layers)
            + (u128::from(attack) * u128::from(self.source_attack_bps)
                + u128::from(maximum) * u128::from(self.target_max_spirit_bps))
                * u128::from(layers)
                / 10000)
            .min(u128::from(u32::MAX)) as u32
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SpiritConversionWhen {
    FullBeforeTick,
    AnyExcess,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpiritConversion {
    pub bps: u32,
    pub when: SpiritConversionWhen,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PeriodicRecoveryRule {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub spirit: Option<SpiritFormula>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub overheal_spirit: Option<SpiritConversion>,
}
impl PeriodicRecoveryRule {
    pub(crate) fn amount(&self, attack: u32, maximum: u32) -> u32 {
        self.spirit
            .as_ref()
            .map_or(0, |f| f.amount(attack, maximum))
    }
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.periodic_recovery.len() > 16 {
        return Err("CONFIG_PERIODIC_RECOVERY_BUDGET");
    }
    for (id, rule) in &r.periodic_recovery {
        if !c
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .any(|d| d.id == *id && d.is_heal())
        {
            return Err("CONFIG_PERIODIC_RECOVERY_REFERENCE");
        }
        if rule.spirit.is_none() && rule.overheal_spirit.is_none()
            || rule.spirit.as_ref().is_some_and(|f| {
                f.fixed > 1000000
                    || f.source_attack_bps > 100000
                    || f.target_max_spirit_bps > 10000
                    || f.fixed == 0 && f.source_attack_bps == 0 && f.target_max_spirit_bps == 0
            })
            || rule
                .overheal_spirit
                .as_ref()
                .is_some_and(|v| !(1..=10000).contains(&v.bps))
        {
            return Err("CONFIG_PERIODIC_RECOVERY_POLICY");
        }
    }
    Ok(())
}
