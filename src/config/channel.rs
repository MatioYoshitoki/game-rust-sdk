//! Bounded fixed-duration projectile channels.
use super::{Config, PlayerSkillMechanism};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_CHANNEL_MS: u64 = 30_000;
pub(crate) const MAX_CHANNEL_PULSES: u32 = 128;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChannelConfig {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub damage_distribution: Option<ChannelDamage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_success: Vec<ChannelSuccess>,
    pub duration_ms: u64,
    pub interval_ms: u64,
    pub spirit_per_pulse: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub pulse_count: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub recovery: Option<ChannelRecovery>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) enum ChannelDamage {
    Total,
}
impl TryFrom<String> for ChannelDamage {
    type Error = &'static str;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s == "total" {
            Ok(Self::Total)
        } else {
            Err("CONFIG_CHANNEL_DAMAGE_DISTRIBUTION")
        }
    }
}
impl From<ChannelDamage> for String {
    fn from(_: ChannelDamage) -> Self {
        "total".into()
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChannelSuccess {
    pub after_pulse: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub self_statuses: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_statuses: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChannelRecovery {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub remainder_policy: Option<BudgetRemainder>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub hp: Option<super::combat::RecoveryFormula>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub spirit: Option<super::periodic_recovery::SpiritFormula>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) enum BudgetRemainder {
    #[default]
    Earliest,
    Final,
}
impl TryFrom<String> for BudgetRemainder {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "earliest" => Ok(Self::Earliest),
            "final" => Ok(Self::Final),
            _ => Err("CONFIG_CHANNEL_REMAINDER_POLICY"),
        }
    }
}
impl From<BudgetRemainder> for String {
    fn from(value: BudgetRemainder) -> Self {
        match value {
            BudgetRemainder::Earliest => "earliest",
            BudgetRemainder::Final => "final",
        }
        .into()
    }
}

impl ChannelConfig {
    pub(crate) fn share(&self, total: u32, index: u32) -> u32 {
        let count = self.pulse_count.expect("bounded recovery pulse count");
        total / count
            + match self.recovery.and_then(|r| r.remainder_policy).unwrap_or(
                if self.damage_distribution.is_some() {
                    BudgetRemainder::Final
                } else {
                    BudgetRemainder::Earliest
                },
            ) {
                BudgetRemainder::Earliest => u32::from(index < total % count),
                BudgetRemainder::Final => {
                    if index + 1 == count {
                        total % count
                    } else {
                        0
                    }
                }
            }
    }
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    let skills = super::player_skills::raw(document);
    for skill in skills {
        if skill["mechanism"] == "unit_effect"
            && (document["schema_version"] != 7
                || !matches!(document["mechanism_version"].as_u64(), Some(105..=106))
                || !document["members"].is_object())
        {
            return Err("CONFIG_UNIT_EFFECT_CAPABILITY");
        }

        if let Some(channel) = skill.get("channel") {
            if document["schema_version"] != 7
                || !matches!(document["mechanism_version"].as_u64(), Some(49..=106))
            {
                return Err("CONFIG_CHANNEL_CAPABILITY");
            }
            if channel
                .get("on_success")
                .is_some_and(|v| v.as_array().is_none_or(|a| a.is_empty()))
            {
                return Err("CONFIG_CHANNEL_SUCCESS_SHAPE");
            }
            if !channel.is_object() {
                return Err("CONFIG_CHANNEL_SHAPE");
            }
            if (channel.get("pulse_count").is_some()
                || channel.get("recovery").is_some()
                || channel.get("damage_distribution").is_some()
                || channel.get("on_success").is_some())
                && document["mechanism_version"].as_u64().unwrap_or(0) < 105
            {
                return Err("CONFIG_CAST_LIFECYCLE_CAPABILITY");
            }
        }
    }
    Ok(())
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    let step = config.step_ms;
    for skill in config.player_skills() {
        if let Some(channel) = &skill.channel {
            let targeted = skill.is_unit_effect() || skill.is_ally_heal();
            if !channel.on_success.is_empty() && config.combat_rules.is_none() {
                return Err("CONFIG_CHANNEL_SUCCESS_COMBAT");
            }
            for gate in &channel.on_success {
                for (self_target, ids) in
                    [(true, &gate.self_statuses), (false, &gate.target_statuses)]
                {
                    for id in ids {
                        let d = config
                            .pve
                            .as_ref()
                            .and_then(|p| p.status_definitions.iter().find(|d| d.id == *id))
                            .ok_or("CONFIG_CHANNEL_SUCCESS_REFERENCE")?;
                        if self_target && d.is_harmful()
                            || !self_target
                                && (d.is_harmful() != skill.is_unit_effect() || !d.source_scoped)
                        {
                            return Err("CONFIG_CHANNEL_SUCCESS_RELATION");
                        }
                    }
                }
            }

            if skill.is_unit_effect() != channel.damage_distribution.is_some()
                || targeted && channel.pulse_count.is_none()
                || !channel.on_success.is_empty() && !targeted
                || channel.on_success.len() > 4
                || channel.on_success.iter().any(|gate| {
                    gate.after_pulse == 0
                        || channel.pulse_count.is_none_or(|n| gate.after_pulse > n)
                        || gate.self_statuses.len() > 2
                        || gate.target_statuses.len() > 2
                        || gate.self_statuses.is_empty() && gate.target_statuses.is_empty()
                        || [&gate.self_statuses, &gate.target_statuses]
                            .iter()
                            .any(|ids| {
                                let mut seen = std::collections::BTreeSet::new();
                                ids.iter().any(|id| {
                                    !seen.insert(id)
                                        || !config.pve.as_ref().is_some_and(|p| {
                                            p.status_definitions.iter().any(|d| d.id == *id)
                                        })
                                })
                            })
                })
            {
                return Err("CONFIG_TARGET_CHANNEL_BINDING");
            }
        }
        if let Some(channel) = &skill.channel
            && ((if channel.recovery.is_some() {
                !(skill.is_self() || skill.is_ally_heal())
            } else {
                !(matches!(skill.mechanism, PlayerSkillMechanism::Projectile)
                    || skill.is_unit_effect())
            }) || skill.charge.is_some()
                || step == 0
                || channel.interval_ms < step
                || channel.duration_ms == 0
                || channel.duration_ms > MAX_CHANNEL_MS
                || (channel.pulse_count.is_none()
                    && channel.duration_ms < channel.interval_ms.saturating_mul(2))
                || channel.pulse_count.is_none()
                    && (!channel.duration_ms.is_multiple_of(step)
                        || !channel.interval_ms.is_multiple_of(step))
                || (channel.duration_ms - 1) / channel.interval_ms > u64::from(MAX_CHANNEL_PULSES)
                || channel.spirit_per_pulse > config.resource_bounds().max_spirit_cost
                || channel.pulse_count.is_some_and(|count| {
                    !(1..=MAX_CHANNEL_PULSES).contains(&count)
                        || channel.interval_ms.checked_mul(u64::from(count))
                            != Some(channel.duration_ms)
                })
                || channel.recovery.is_some_and(|recovery| {
                    channel.pulse_count.is_none()
                        || skill.instant_heal.is_some()
                        || skill.ally_status.is_some()
                        || recovery.hp.is_none() && recovery.spirit.is_none()
                        || recovery.hp.is_some_and(|f| {
                            f.fixed > 1_000_000
                                || f.source_attack_bps > 100_000
                                || f.target_max_hp_bps > 10_000
                                || f.fixed == 0
                                    && f.source_attack_bps == 0
                                    && f.target_max_hp_bps == 0
                        })
                        || recovery.spirit.is_some_and(|f| {
                            f.fixed > 1_000_000
                                || f.source_attack_bps > 100_000
                                || f.target_max_spirit_bps > 10_000
                                || f.fixed == 0
                                    && f.source_attack_bps == 0
                                    && f.target_max_spirit_bps == 0
                        })
                }))
        {
            return Err("CONFIG_CHANNEL_BOUNDS");
        }
    }
    Ok(())
}
