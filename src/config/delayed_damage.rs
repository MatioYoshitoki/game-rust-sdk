//! One native periodic deadline, live target deficit and explicit source sampling.
use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DelayedDamageRule {
    pub fixed: u32,
    pub source_power_bps: u32,
    pub target_missing_hp_bps: u32,
    pub caps_by_target_profile: std::collections::BTreeMap<String, u32>,
}
impl DelayedDamageRule {
    pub(crate) fn amount(&self, power: u32, missing: u32, profile: &str) -> u32 {
        let amount = (u128::from(self.fixed) * 10000
            + u128::from(power) * u128::from(self.source_power_bps)
            + u128::from(missing) * u128::from(self.target_missing_hp_bps))
            / 10000;
        let cap = self
            .caps_by_target_profile
            .get(profile)
            .map_or(u128::MAX, |bps| {
                u128::from(power) * u128::from(*bps) / 10000
            });
        amount.min(cap).min(u128::from(u32::MAX)) as u32
    }
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.delayed_damage.len() > 16 {
        return Err("CONFIG_DELAYED_DAMAGE_BUDGET");
    }
    for (id, f) in &r.delayed_damage {
        let d = c
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .find(|d| d.id == *id);
        if !d.is_some_and(|d| {
            d.is_periodic()
                && !d.is_heal()
                && d.periodic_stack.is_none()
                && d.source_scoped
                && d.tick_interval_ms.is_some_and(|i| {
                    i >= c.step_ms && Some(d.duration_ms) == i.checked_add(c.step_ms)
                })
        }) || r.periodic_sampling.get(id).is_none_or(|s| s.target())
            || f.fixed > 1000000
            || f.source_power_bps > 100000
            || f.target_missing_hp_bps > 100000
            || f.fixed == 0 && f.source_power_bps == 0 && f.target_missing_hp_bps == 0
            || f.caps_by_target_profile.len() > 64
            || f.caps_by_target_profile
                .iter()
                .any(|(p, b)| !r.profiles.contains_key(p) || *b > 100000)
            || r.periodic_damage.get(id).is_some_and(|a| {
                a.base_power.is_some()
                    || a.fixed != 0
                    || a.cultivation_bps != 10000
                    || a.contact_status.is_some()
            })
        {
            return Err("CONFIG_DELAYED_DAMAGE_RULE");
        }
    }
    Ok(())
}
