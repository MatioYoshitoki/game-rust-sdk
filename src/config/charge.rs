//! Bounds and the linear power curve for a frozen charge definition.
use super::{Config, PlayerSkillMechanism};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_CHARGE_MS: u64 = 5000;
pub(crate) const MIN_AIM_INTERVAL_MS: u64 = 50;
pub(crate) const MAX_AIM_TURN_DEG_PER_SEC: u16 = 720;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeAimConfig {
    pub min_interval_ms: u64,
    pub max_turn_deg_per_sec: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeConfig {
    pub min_ms: u64,
    pub max_ms: u64,
    pub min_damage_bps: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub early_release: Option<EarlyRelease>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub release_windup_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aim_update: Option<ChargeAimConfig>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum EarlyRelease {
    QueueUntilMin {},
}

impl ChargeConfig {
    pub(crate) fn damage_factor(self, held_ms: u64) -> u32 {
        let elapsed = held_ms.clamp(self.min_ms, self.max_ms) - self.min_ms;
        let increase =
            u64::from(10_000 - self.min_damage_bps) * elapsed / (self.max_ms - self.min_ms);
        self.min_damage_bps + increase as u32
    }
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    let skills = super::player_skills::raw(document);
    for skill in skills {
        if let Some(charge) = skill.get("charge") {
            if document["schema_version"] != 7
                || !matches!(document["mechanism_version"].as_u64(), Some(47..=106))
            {
                return Err("CONFIG_CHARGE_CAPABILITY");
            }
            if !charge.is_object() {
                return Err("CONFIG_CHARGE_SHAPE");
            }
            if (charge.get("early_release").is_some() || charge.get("release_windup_ms").is_some())
                && document["mechanism_version"].as_u64().unwrap_or(0) < 105
            {
                return Err("CONFIG_CAST_LIFECYCLE_CAPABILITY");
            }
            if let Some(aim) = charge.get("aim_update") {
                if !matches!(document["mechanism_version"].as_u64(), Some(48..=106)) {
                    return Err("CONFIG_CHARGE_AIM_CAPABILITY");
                }
                if !aim.is_object() {
                    return Err("CONFIG_CHARGE_AIM_SHAPE");
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    for skill in config.player_skills() {
        if let Some(charge) = skill.charge
            && (!matches!(
                skill.mechanism,
                PlayerSkillMechanism::Projectile | PlayerSkillMechanism::PiercingProjectile
            ) || skill.windup_ms != 0
                || config.step_ms == 0
                || charge.min_ms < config.step_ms
                || charge.max_ms > MAX_CHARGE_MS
                || charge.release_windup_ms.is_some_and(|windup| {
                    windup == 0
                        || charge
                            .max_ms
                            .checked_add(windup)
                            .is_none_or(|total| total > MAX_CHARGE_MS)
                        || charge.max_ms.div_ceil(config.step_ms) * config.step_ms
                            + windup.div_ceil(config.step_ms) * config.step_ms
                            > MAX_CHARGE_MS
                })
                || charge.max_ms < charge.min_ms.saturating_add(config.step_ms)
                || !(1..=10_000).contains(&charge.min_damage_bps)
                || charge.aim_update.is_some_and(|aim| {
                    !(MIN_AIM_INTERVAL_MS..=charge.max_ms.saturating_sub(config.step_ms))
                        .contains(&aim.min_interval_ms)
                        || !(1..=MAX_AIM_TURN_DEG_PER_SEC).contains(&aim.max_turn_deg_per_sec)
                }))
        {
            return Err("CONFIG_CHARGE_BOUNDS");
        }
    }
    Ok(())
}
