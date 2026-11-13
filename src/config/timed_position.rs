use super::*;
use crate::protocol::TimedPositionSite;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AfterEncounter<T> {
    pub encounter_id: String,
    pub value: T,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TimedPositionConfig {
    pub id: String,
    pub name: String,
    pub encounter_id: String,
    pub source_id: String,
    pub interval_ms: u64,
    pub warning_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<PositionCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_hit: Option<DirectHitCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_damage_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_spirit_restore_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub skip_sites_after: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_damage_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning_bonus_ms: Option<AfterEncounter<u64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_scale_bps: Option<AfterEncounter<u32>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectHitCondition {
    pub miss_heal_bps: u32,
}

impl TimedPositionConfig {
    pub(crate) fn sites(&self) -> &[TimedPositionSite] {
        self.position.as_ref().map_or(&[], PositionCondition::sites)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum PositionCondition {
    NearLivingMember {
        radius: f64,
    },
    RotatingSites {
        radius: f64,
        sites: Vec<TimedPositionSite>,
    },
}

impl PositionCondition {
    pub(crate) fn radius(&self) -> f64 {
        match self {
            Self::NearLivingMember { radius } | Self::RotatingSites { radius, .. } => *radius,
        }
    }

    pub(crate) fn sites(&self) -> &[TimedPositionSite] {
        match self {
            Self::NearLivingMember { .. } => &[],
            Self::RotatingSites { sites, .. } => sites,
        }
    }
}

pub(super) fn validate(c: &Config, p: &PveConfig) -> Result<(), &'static str> {
    let Some(check) = &p.timed_position else {
        return Ok(());
    };
    if c.mechanism_version < 14 {
        return Err("CONFIG_TIMED_POSITION_CAPABILITY");
    }
    if c.mechanism_version < 16
        && (check.success_spirit_restore_bps.is_some() || !check.skip_sites_after.is_empty())
    {
        return Err("CONFIG_POSITION_EFFECT_CAPABILITY");
    }
    if c.mechanism_version < 17 && check.direct_hit.is_some() {
        return Err("CONFIG_TIMED_HIT_CAPABILITY");
    }
    let valid_id = |v: &str| !v.is_empty() && v.len() <= 64 && !v.chars().any(char::is_control);
    if !valid_id(&check.id) || !valid_id(&check.name) {
        return Err("CONFIG_TIMED_POSITION_ID");
    }
    let Some(encounter) = p.encounters.iter().find(|e| e.id == check.encounter_id) else {
        return Err("CONFIG_TIMED_POSITION_REFERENCE");
    };
    let invalid_side =
        |id: &str| id == check.encounter_id || !p.encounters.iter().any(|e| e.id == id);
    if !encounter.all_monsters().any(|m| m.id == check.source_id)
        || check
            .warning_bonus_ms
            .iter()
            .map(|rule| rule.encounter_id.as_str())
            .chain(
                check
                    .failure_scale_bps
                    .iter()
                    .map(|rule| rule.encounter_id.as_str()),
            )
            .any(invalid_side)
    {
        return Err("CONFIG_TIMED_POSITION_REFERENCE");
    }
    let bonus = check.warning_bonus_ms.as_ref().map_or(0, |rule| rule.value);
    if !(c.step_ms..=60000).contains(&check.warning_ms)
        || !(c.step_ms..=60000.min(c.terminal_ms)).contains(&check.interval_ms)
        || check
            .warning_ms
            .checked_add(bonus)
            .is_none_or(|total| total >= check.interval_ms)
        || check.position.as_ref().is_some_and(|position| {
            let radius = position.radius();
            !radius.is_finite() || !(0.1..=encounter.trigger.radius).contains(&radius)
        })
        || check
            .success_damage_bps
            .is_some_and(|bps| !(1..=100000).contains(&bps))
        || check
            .failure_damage_bps
            .is_some_and(|bps| !(1..=100000).contains(&bps))
        || check
            .failure_scale_bps
            .as_ref()
            .is_some_and(|rule| !(1..=10000).contains(&rule.value))
    {
        return Err("CONFIG_TIMED_POSITION_BOUNDS");
    }
    if let Some(hit) = &check.direct_hit {
        if check.position.is_some()
            || check.failure_damage_bps.is_some()
            || check.success_damage_bps.is_none()
            || check.success_spirit_restore_bps.is_some()
            || !check.skip_sites_after.is_empty()
            || !(1..=10000).contains(&hit.miss_heal_bps)
        {
            return Err("CONFIG_TIMED_HIT_CONDITION");
        }
        return Ok(());
    }
    let (Some(position), Some(_)) = (&check.position, check.failure_damage_bps) else {
        return Err("CONFIG_TIMED_POSITION_CONDITION");
    };
    let sites = position.sites();
    match (check.success_damage_bps, check.success_spirit_restore_bps) {
        (Some(_), None) => {}
        (None, Some(bps)) if !sites.is_empty() && (1..=10000).contains(&bps) => {}
        _ => return Err("CONFIG_POSITION_SUCCESS_EFFECT"),
    }
    if !check.skip_sites_after.is_empty()
        && (check.skip_sites_after.len() >= sites.len()
            || check.skip_sites_after.iter().any(|(site, side)| {
                invalid_side(side) || !sites.iter().any(|entry| entry.id == *site)
            }))
    {
        return Err("CONFIG_POSITION_SITE_FILTER");
    }
    if let PositionCondition::RotatingSites { radius, sites } = position {
        if c.mechanism_version < 15 {
            return Err("CONFIG_POSITION_SITES_CAPABILITY");
        }
        if !(1..=8).contains(&sites.len())
            || sites.iter().enumerate().any(|(index, site)| {
                !valid_id(&site.id)
                    || !valid_id(&site.name)
                    || !site
                        .position
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 32768.0)
                    || distance(site.position, encounter.trigger.center) + radius
                        > encounter.trigger.radius
                    || sites[..index]
                        .iter()
                        .any(|other| other.id == site.id || other.position == site.position)
            })
        {
            return Err("CONFIG_POSITION_SITES");
        }
    }
    Ok(())
}
