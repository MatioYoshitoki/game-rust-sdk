//! A frozen cost definition, evaluated only at the atomic commit boundary.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum HpCost {
    Fixed { value: f64, min_hp_after_cost: u32 },
    CurrentHpRatio { value: f64, min_hp_after_cost: u32 },
}
impl HpCost {
    pub(crate) fn minimum(&self) -> u32 {
        match *self {
            Self::Fixed {
                min_hp_after_cost, ..
            }
            | Self::CurrentHpRatio {
                min_hp_after_cost, ..
            } => min_hp_after_cost,
        }
    }
    pub(crate) fn evaluate(&self, hp: u32) -> u32 {
        let (raw, minimum) = match *self {
            Self::Fixed {
                value,
                min_hp_after_cost,
            } => (value.floor(), min_hp_after_cost),
            Self::CurrentHpRatio {
                value,
                min_hp_after_cost,
            } => ((f64::from(hp) * value).floor(), min_hp_after_cost),
        };
        (raw as u32).min(hp.saturating_sub(minimum))
    }
    fn valid(&self, maximum: u32) -> bool {
        let (value, minimum, max) = match *self {
            Self::Fixed {
                value,
                min_hp_after_cost,
            } => (value, min_hp_after_cost, f64::from(maximum)),
            Self::CurrentHpRatio {
                value,
                min_hp_after_cost,
            } => (value, min_hp_after_cost, 1.),
        };
        value.is_finite() && (0. ..=max).contains(&value) && (1..=maximum).contains(&minimum)
    }
}
pub(super) fn fields(v: &serde_json::Value) -> Result<(), &'static str> {
    for s in super::player_skills::raw(v) {
        if let Some(cost) = s.get("hp_cost") {
            if v["schema_version"] != 7
                || !matches!(v["mechanism_version"].as_u64(), Some(45..=105))
            {
                return Err("CONFIG_HP_COST_CAPABILITY");
            }
            if !cost.is_object() {
                return Err("CONFIG_HP_COST_SHAPE");
            }
        }
    }
    Ok(())
}
pub(super) fn validate(c: &super::Config) -> Result<(), &'static str> {
    if c.player_skills().any(|s| {
        s.hp_cost
            .as_ref()
            .is_some_and(|cost| !cost.valid(c.resource_bounds().max_hp))
    }) {
        return Err("CONFIG_HP_COST_BOUNDS");
    }
    Ok(())
}
