//! Movement policies for the existing preparation and recovery phases.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum PhaseMovement {
    Free {},
    Slow {
        speed_bps: u16,
    },
    Locked {},
    InterruptOnNewMove {
        min_turn_degrees: u16,
        min_destination_distance: f64,
    },
}

impl Default for PhaseMovement {
    fn default() -> Self {
        Self::Free {}
    }
}

impl PhaseMovement {
    pub(crate) fn speed_factor(self) -> f64 {
        match self {
            Self::Free {} | Self::InterruptOnNewMove { .. } => 1.0,
            Self::Slow { speed_bps } => f64::from(speed_bps) / 10_000.0,
            Self::Locked {} => 0.0,
        }
    }

    fn valid_for(self, duration_ms: u64) -> bool {
        match self {
            Self::Free {} => true,
            Self::Slow { speed_bps } => duration_ms > 0 && (1..10_000).contains(&speed_bps),
            Self::Locked {} => duration_ms > 0,
            Self::InterruptOnNewMove {
                min_turn_degrees,
                min_destination_distance,
            } => {
                duration_ms > 0
                    && (1..=180).contains(&min_turn_degrees)
                    && min_destination_distance.is_finite()
                    && (0.0..=10000.).contains(&min_destination_distance)
                    && min_destination_distance > 0.
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CastMovement {
    pub preparation: PhaseMovement,
    pub recovery: PhaseMovement,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub active: Option<PhaseMovement>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CastLimits {
    pub mobile_fixed_windup_max_ms: u64,
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    let skills = super::player_skills::raw(document);
    for skill in skills {
        if let Some(policy) = skill.get("cast_movement") {
            if document["schema_version"] != 7
                || !matches!(document["mechanism_version"].as_u64(), Some(46..=105))
            {
                return Err("CONFIG_CAST_MOVEMENT_CAPABILITY");
            }
            if !policy.is_object() {
                return Err("CONFIG_CAST_MOVEMENT_SHAPE");
            }
            if matches!(
                skill["mechanism"].as_str(),
                Some("ally_heal" | "ally_shield")
            ) && document["mechanism_version"].as_u64().unwrap_or(0) < 105
            {
                return Err("CONFIG_CAST_LIFECYCLE_CAPABILITY");
            }
            if (policy.get("active").is_some()
                || ["preparation", "recovery"]
                    .iter()
                    .any(|phase| policy[*phase]["kind"] == "interrupt_on_new_move"))
                && document["mechanism_version"].as_u64().unwrap_or(0) < 105
            {
                return Err("CONFIG_CAST_LIFECYCLE_CAPABILITY");
            }
        }
    }
    Ok(())
}

pub(super) fn validate(config: &super::Config) -> Result<(), &'static str> {
    if let Some(limits) = config.cast_limits {
        if config.schema_version != 7
            || config.mechanism_version < 105
            || config.member_settings().is_none()
        {
            return Err("CONFIG_CAST_LIFECYCLE_CAPABILITY");
        }
        if limits.mobile_fixed_windup_max_ms > 3_600_000 {
            return Err("CONFIG_CAST_LIMIT_BOUNDS");
        }
        if config.player_skills().any(|skill| {
            !skill.is_self()
                && !skill.is_unit_cast()
                && skill.charge.is_none()
                && matches!(
                    skill.cast_movement.unwrap_or_default().preparation,
                    PhaseMovement::Free {}
                )
                && skill.windup_ms > limits.mobile_fixed_windup_max_ms
        }) {
            return Err("CONFIG_MOBILE_WINDUP_BOUNDS");
        }
    }
    for skill in config.player_skills() {
        if let Some(policy) = skill.cast_movement
            && (!policy
                .preparation
                .valid_for(skill.charge.map_or(skill.windup_ms, |charge| charge.max_ms))
                || !policy.recovery.valid_for(skill.recovery_ms)
                || matches!(policy.recovery, PhaseMovement::InterruptOnNewMove { .. })
                || policy.active.is_some_and(|active| {
                    skill
                        .channel
                        .as_ref()
                        .is_none_or(|channel| !active.valid_for(channel.duration_ms))
                }))
        {
            return Err("CONFIG_CAST_MOVEMENT_BOUNDS");
        }
    }
    Ok(())
}
