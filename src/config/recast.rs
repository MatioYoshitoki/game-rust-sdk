//! A bounded opportunity to redirect an ordinary in-flight projectile.
use super::Config;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_RECAST_WINDOW_MS: u64 = 30_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecastConfig {
    pub window_ms: u64,
    pub spirit_cost: u32,
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    let skills = super::player_skills::raw(document);
    for skill in skills {
        if let Some(recast) = skill.get("recast") {
            if document["schema_version"] != 7
                || !matches!(document["mechanism_version"].as_u64(), Some(50..=107))
            {
                return Err("CONFIG_RECAST_CAPABILITY");
            }
            if !recast.is_object() {
                return Err("CONFIG_RECAST_SHAPE");
            }
        }
    }
    Ok(())
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    for skill in config.player_skills() {
        if let Some(recast) = skill.recast
            && (skill.charge.is_some()
                || skill.channel.is_some()
                || config.step_ms == 0
                || recast.window_ms < config.step_ms
                || !recast.window_ms.is_multiple_of(config.step_ms)
                || recast.window_ms
                    > MAX_RECAST_WINDOW_MS.min(config.params.player_projectile_ttl_ms)
                || recast.spirit_cost > config.resource_bounds().max_spirit_cost)
        {
            return Err("CONFIG_RECAST_BOUNDS");
        }
    }
    Ok(())
}
