use super::*;
use crate::protocol::valid_id;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MAX_PRESETS: usize = 8;
pub(crate) const MAX_TARGETS_PER_PRESET: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OwnerScope {
    Seat,
    All,
}
impl Default for OwnerScope {
    fn default() -> Self {
        Self::Seat
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrainingSeat {
    #[serde(default)]
    pub owner_scope: OwnerScope,
    pub presets: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub behaviors: BTreeMap<String, TrainingTargetBehavior>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub initial_preset: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum TrainingTargetBehavior {
    Dummy {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        reset_on_defeat: bool,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        motion: Option<TrainingMotion>,
    },
    Combat {},
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrainingMotion {
    pub end: [f64; 2],
    pub speed: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrainingRuntime {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub respawn: Option<TrainingRespawn>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub metrics: Option<TrainingMetrics>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrainingRespawn {
    pub delay_ms: u64,
    pub points: Vec<[f64; 2]>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrainingMetrics {
    pub window_ms: u64,
    pub publish_interval_ms: u64,
}

impl Config {
    pub(crate) fn training_targets(&self) -> Option<&[TrainingSeat]> {
        self.members.as_ref()?.training_targets.as_deref()
    }
    pub(crate) fn training_runtime(&self) -> Option<&TrainingRuntime> {
        self.members.as_ref()?.training_runtime.as_ref()
    }
    pub(crate) fn training_tools(&self) -> Option<&super::two_members::TrainingTools> {
        self.members.as_ref()?.training_tools.as_ref()
    }
    pub(crate) fn training_shared_seats(&self) -> Vec<usize> {
        self.members
            .as_ref()
            .map(|members| {
                members
                    .training_targets
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .enumerate()
                    .filter(|(_, seat)| seat.owner_scope == OwnerScope::All)
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(seats) = c.training_targets() else {
        return if c.training_runtime().is_some() || c.training_tools().is_some() {
            Err("CONFIG_TRAINING_CAPABILITY")
        } else {
            Ok(())
        };
    };
    if let Some(tools) = c.training_tools() {
        if !tools.enabled {
            return Err("CONFIG_TRAINING_TOOLS");
        }
        if !matches!(c.mechanism_version, 105..=107) {
            return Err("CONFIG_TRAINING_TOOLS");
        }
        if c.pve.as_ref().is_some_and(|pve| {
            tools
                .restore_clears
                .iter()
                .any(|id| !pve.status_definitions.iter().any(|d| d.id == *id))
        }) {
            return Err("CONFIG_TRAINING_TOOLS");
        }
        if !(1..=16).contains(&tools.spawn_max_alive) {
            return Err("CONFIG_TRAINING_TOOLS");
        }
    }
    if c.protocol_version() != 6
        || !matches!(c.mechanism_version, 105..=107)
        || c.combat_rules.is_none()
        || c.pve.is_none()
        || c.match_mode.is_some()
        || c.member_conflict()
        || c.params.entry_years != 0
        || c.carried_quantity != 0
        || c.cultivation_coefficient != 0.0
        || !c.settlement_rewards.is_empty()
        || !c.settlement_facts.is_empty()
        || c.settlement_progression.is_some()
        || c.world_loot.is_some()
        || c.world_boons.is_some()
        || !c.checkpoint_schedules().is_empty()
        || !c.participant_ai.is_empty()
    {
        return Err("CONFIG_TRAINING_CAPABILITY");
    }
    let targets = crate::world::two_members::World::npc_ids(c);
    let mut assigned = BTreeSet::new();
    if seats.len() != c.member_settings().unwrap().capacity
        || seats.iter().any(|seat| {
            let mut owned = BTreeSet::new();
            !(1..=MAX_PRESETS).contains(&seat.presets.len())
                || seat
                    .initial_preset
                    .as_ref()
                    .is_some_and(|id| !seat.presets.contains_key(id))
                || seat.presets.iter().any(|(id, group)| {
                    let unique: BTreeSet<_> = group.iter().collect();
                    !valid_id(id)
                        || id.len() > 64
                        || !(1..=MAX_TARGETS_PER_PRESET).contains(&group.len())
                        || unique.len() != group.len()
                        || group.iter().any(|target| !targets.contains(target))
                        || !unique.into_iter().all(|target| {
                            owned.insert(target);
                            !assigned.contains(target)
                        })
                })
                || seat.behaviors.iter().any(|(id, behavior)| {
                    !owned.contains(id)
                        || matches!(behavior, TrainingTargetBehavior::Dummy { motion: Some(m), .. }
                            if !m.end.iter().all(|v| v.is_finite())
                                || !(0.01..=32768.).contains(&m.speed))
                })
                || !owned.into_iter().all(|target| assigned.insert(target))
        })
        || assigned.len() != targets.len()
    {
        return Err("CONFIG_TRAINING_TARGETS");
    }
    if c.training_runtime().is_some_and(|runtime| {
        runtime.metrics.is_none() && runtime.respawn.is_none()
            || runtime.metrics.is_some_and(|m| {
                !(c.step_ms..=60000).contains(&m.window_ms)
                    || !(c.step_ms..=m.window_ms).contains(&m.publish_interval_ms)
                    || m.window_ms.div_ceil(c.step_ms) > 4096
            })
            || runtime.respawn.as_ref().is_some_and(|r| {
                !(c.step_ms..=c.terminal_ms).contains(&r.delay_ms)
                    || r.points.len() != seats.len()
                    || r.points.iter().enumerate().any(|(i, &point)| {
                        !point.iter().all(|v| v.is_finite())
                            || r.points[..i]
                                .iter()
                                .any(|&other| distance(point, other) <= 2. * c.params.player_radius)
                    })
            })
    }) {
        return Err("CONFIG_TRAINING_RUNTIME");
    }
    Ok(())
}

pub(super) fn validate_prepared(c: &Config) -> Result<(), &'static str> {
    let Some(seats) = c.training_targets() else {
        return Ok(());
    };
    let pve = c.pve.as_ref().unwrap();
    if c.training_runtime()
        .and_then(|r| r.respawn.as_ref())
        .is_some_and(|r| {
            r.points
                .iter()
                .any(|&point| !crate::world::pve::validate_map(pve, point, c.extraction_point))
        })
    {
        return Err("CONFIG_TRAINING_RESPAWN_POINT");
    }
    let ids = crate::world::two_members::World::npc_ids(c);
    let spawns = pve
        .monster_positions
        .iter()
        .map(|&p| (p, pve.combat.radius))
        .chain(
            pve.extra_spawns
                .iter()
                .map(|s| (s.position, pve.profiles[&s.template_id].combat.radius)),
        );
    for (id, (home, radius)) in ids.iter().zip(spawns) {
        if let Some(TrainingTargetBehavior::Dummy {
            motion: Some(m), ..
        }) = seats.iter().find_map(|seat| seat.behaviors.get(id))
            && (m.end == home
                || !crate::world::pve::validate_walkable_segment(pve, home, m.end, radius))
        {
            return Err("CONFIG_TRAINING_MOTION_PATH");
        }
    }
    Ok(())
}
