//! Admission budgets are explicit configuration, separate from gameplay amounts.
use super::*;
const LEGACY: u32 = 1_000_000;
pub(crate) const MAX_BASE_RESOURCE: u32 = 1_000_000_000;
pub(crate) const MAX_SPIRIT_COST: u32 = 8_000_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceLimits {
    pub max_hp: u32,
    pub max_spirit: u32,
    pub max_spirit_cost: u32,
}
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_hp: LEGACY,
            max_spirit: LEGACY,
            max_spirit_cost: LEGACY,
        }
    }
}
impl Config {
    pub(crate) fn resource_bounds(&self) -> ResourceLimits {
        self.resource_limits.unwrap_or_default()
    }
    pub(crate) fn base_stat_limit(&self, key: &str) -> u32 {
        match key {
            "hp" => self.resource_bounds().max_hp,
            "spirit" => self.resource_bounds().max_spirit,
            _ => LEGACY,
        }
    }
}
pub(super) fn validate(c: &Config) -> std::result::Result<(), &'static str> {
    if let Some(limits) = c.resource_limits {
        if c.schema_version != 7 || c.mechanism_version < 105 || c.member_settings().is_none() {
            return Err("CONFIG_RESOURCE_LIMIT_CAPABILITY");
        }
        if [limits.max_hp, limits.max_spirit]
            .iter()
            .any(|v| !(1..=MAX_BASE_RESOURCE).contains(v))
            || !(1..=MAX_SPIRIT_COST).contains(&limits.max_spirit_cost)
            || limits.max_spirit_cost > limits.max_spirit
        {
            return Err("CONFIG_RESOURCE_LIMIT_BOUNDS");
        }
    }
    Ok(())
}
