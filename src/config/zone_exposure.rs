use super::*;
use crate::protocol::valid_id;
use std::collections::BTreeSet;

pub(crate) const MAX_BANDS: usize = 8;
pub(crate) const MAX_NODES: usize = 16;
pub(crate) const MAX_STACKS: u8 = 64;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ZoneExposure {
    pub axis: [f64; 2],
    pub bands: Vec<ExposureBand>,
    pub max_stacks: u8,
    pub reduction_per_stack_bps: u16,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub interaction: Option<ExposureInteraction>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExposureBand {
    pub id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub below: Option<f64>,
    pub interval_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExposureInteraction {
    pub range: f64,
    pub cooldown_ms: u64,
    pub remove_stacks: u8,
    pub boost_bps: u16,
    pub boost_duration_ms: u64,
    pub nodes: Vec<ExposureNode>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExposureNode {
    pub id: String,
    pub position: [f64; 2],
}

impl ZoneExposure {
    pub(crate) fn band_at(&self, [x, y]: [f64; 2]) -> usize {
        let coordinate = x * self.axis[0] + y * self.axis[1];
        self.bands
            .iter()
            .position(|band| band.below.is_none_or(|limit| coordinate < limit))
            .expect("validated final band")
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(rule) = &c.zone_exposure else {
        return Ok(());
    };
    if c.protocol_version() != 6
        || c.mechanism_version != 105
        || c.members.is_none()
        || c.pve.is_none()
        || c.combat_rules.is_none()
        || c.match_mode.is_some()
        || c.training_targets().is_some()
    {
        return Err("CONFIG_ZONE_EXPOSURE_CAPABILITY");
    }
    let mut ids = BTreeSet::new();
    let short_id = |id: &str| valid_id(id) && id.len() <= 64;
    if rule.axis == [0., 0.]
        || !rule.axis.iter().all(|v| (-1000. ..=1000.).contains(v))
        || !(1..=MAX_BANDS).contains(&rule.bands.len())
        || !(1..=MAX_STACKS).contains(&rule.max_stacks)
        || rule.reduction_per_stack_bps == 0
        || u32::from(rule.reduction_per_stack_bps) * u32::from(rule.max_stacks) >= 10_000
        || rule.bands.iter().enumerate().any(|(i, band)| {
            !short_id(&band.id)
                || !ids.insert(&band.id)
                || !(1..=3_600_000).contains(&band.interval_ms)
                || band.below.is_none() != (i + 1 == rule.bands.len())
                || band.below.is_some_and(|limit| !limit.is_finite())
        })
        || rule
            .bands
            .windows(2)
            .any(|pair| matches!((pair[0].below, pair[1].below), (Some(a), Some(b)) if a >= b))
    {
        return Err("CONFIG_ZONE_EXPOSURE");
    }
    if let Some(interaction) = &rule.interaction {
        let mut ids = BTreeSet::new();
        if !(0.1..=1000.).contains(&interaction.range)
            || !(c.step_ms..=3_600_000).contains(&interaction.cooldown_ms)
            || !(1..=rule.max_stacks).contains(&interaction.remove_stacks)
            || !(1..=10_000).contains(&interaction.boost_bps)
            || !(c.step_ms..=3_600_000).contains(&interaction.boost_duration_ms)
            || !(1..=MAX_NODES).contains(&interaction.nodes.len())
            || interaction
                .nodes
                .iter()
                .any(|node| !short_id(&node.id) || !ids.insert(&node.id))
        {
            return Err("CONFIG_ZONE_EXPOSURE_INTERACTION");
        }
    }
    Ok(())
}

pub(super) fn validate_prepared(c: &Config) -> Result<(), &'static str> {
    if let Some(interaction) = c
        .zone_exposure
        .as_ref()
        .and_then(|rule| rule.interaction.as_ref())
        && !crate::world::pve::validate_walkable_centers(
            c.pve.as_ref().unwrap(),
            interaction.nodes.iter().map(|node| node.position),
            c.params.player_radius,
        )
    {
        return Err("CONFIG_ZONE_EXPOSURE_POSITION");
    }
    Ok(())
}
