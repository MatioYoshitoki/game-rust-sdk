use super::*;
use crate::protocol::{MatchResult, ModeObjective, SettlementPolicy, valid_id};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MAX_STAGES: usize = 8;
pub(crate) const MAX_WAVES: usize = 8;
pub(crate) const MAX_RESPAWNS: u8 = 8;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MatchMode {
    pub stages: Vec<ModeStage>,
    pub results: ModeResults,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub respawn: Option<ModeRespawn>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub disconnect_forfeit_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModeResults {
    pub win: MatchResult,
    pub loss: MatchResult,
    pub draw: MatchResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModeStage {
    pub id: String,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub absolute_deadline: bool,
    pub objective: StageObjective,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum StageObjective {
    Wait {},
    Checkpoint {
        schedule_id: String,
        members_required: usize,
    },
    ClearTargets {
        target_ids: Vec<String>,
    },
    Survive {
        members_required: usize,
    },
    Waves {
        waves: Vec<ModeWave>,
    },
    KillQuota {
        target_weights: BTreeMap<String, u32>,
        required: u32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        advance_on_timeout: bool,
    },
    Contest {},
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModeWave {
    pub delay_ms: u64,
    pub target_ids: Vec<String>,
}

impl StageObjective {
    pub(crate) fn kind(&self) -> ModeObjective {
        match self {
            Self::Wait {} => ModeObjective::Wait,
            Self::Checkpoint { .. } => ModeObjective::Checkpoint,
            Self::ClearTargets { .. } => ModeObjective::ClearTargets,
            Self::Survive { .. } => ModeObjective::Survive,
            Self::Waves { .. } => ModeObjective::Waves,
            Self::KillQuota { .. } => ModeObjective::KillQuota,
            Self::Contest {} => ModeObjective::Contest,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModeRespawn {
    pub max_per_member: u8,
    pub delay_ms: u64,
    pub hp_bps: u32,
    pub spirit_bps: u32,
}

impl MatchMode {
    pub(crate) fn opposed(&self) -> bool {
        matches!(
            self.stages.last().map(|stage| &stage.objective),
            Some(StageObjective::Contest {})
        )
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(mode) = &c.match_mode else {
        return Ok(());
    };
    let (Some(members), Some(base)) = (&c.members, &c.result_contract) else {
        return Err("CONFIG_MATCH_MODE_CAPABILITY");
    };
    if c.schema_version != 7
        || !matches!(c.mechanism_version, 105..=107)
        || c.combat_rules.is_none()
        || mode.opposed() && members.capacity != 2
    {
        return Err("CONFIG_MATCH_MODE_CAPABILITY");
    }
    let ids = [
        &base.extraction,
        &base.death,
        &mode.results.win,
        &mode.results.loss,
        &mode.results.draw,
    ];
    if ids
        .iter()
        .any(|r| r.result_id.len() > 64 || !valid_id(&r.result_id))
        || ids
            .iter()
            .map(|r| &r.result_id)
            .collect::<BTreeSet<_>>()
            .len()
            != ids.len()
        || mode.results.draw.policy != SettlementPolicy::Preserve
    {
        return Err("CONFIG_MATCH_MODE_RESULTS");
    }
    let time = |ms: u64| (c.step_ms..=c.terminal_ms).contains(&ms) && ms.is_multiple_of(c.step_ms);
    if mode.respawn.as_ref().is_some_and(|r| {
        !(1..=MAX_RESPAWNS).contains(&r.max_per_member)
            || !time(r.delay_ms)
            || !(1..=10000).contains(&r.hp_bps)
            || r.spirit_bps > 10000
    }) || mode
        .disconnect_forfeit_ms
        .is_some_and(|ms| !time(ms) || ms > c.params.runtime.reconnect_grace_ms)
    {
        return Err("CONFIG_MATCH_MODE_LIFECYCLE");
    }
    if !(1..=MAX_STAGES).contains(&mode.stages.len()) {
        return Err("CONFIG_MATCH_MODE_STAGES");
    }
    let targets = crate::world::two_members::World::npc_ids(c);
    let mut stage_ids = BTreeSet::new();
    let mut stage_targets = BTreeSet::new();
    let mut total = 0u64;
    for (i, stage) in mode.stages.iter().enumerate() {
        if !valid_id(&stage.id)
            || stage.id.len() > 64
            || !stage_ids.insert(&stage.id)
            || !time(stage.duration_ms)
        {
            return Err("CONFIG_MATCH_MODE_STAGES");
        }
        total += stage.duration_ms;
        let mut valid_targets = |ids: &[String]| {
            (1..=32).contains(&ids.len())
                && ids
                    .iter()
                    .all(|id| targets.contains(id) && stage_targets.insert(id.clone()))
        };
        let valid = match &stage.objective {
            StageObjective::Wait {} => i == 0 && mode.stages.len() > 1,
            StageObjective::Contest {} => i + 1 == mode.stages.len(),
            StageObjective::Checkpoint {
                schedule_id,
                members_required,
            } => {
                (1..=members.capacity).contains(members_required)
                    && members
                        .checkpoint_schedules
                        .iter()
                        .any(|rule| &rule.id == schedule_id)
            }
            StageObjective::ClearTargets { target_ids } => valid_targets(target_ids),
            StageObjective::Survive { members_required } => {
                (1..=members.capacity).contains(members_required)
            }
            StageObjective::Waves { waves } => {
                (1..=MAX_WAVES).contains(&waves.len())
                    && waves.iter().all(|wave| {
                        wave.delay_ms < stage.duration_ms
                            && wave.delay_ms.is_multiple_of(c.step_ms)
                            && valid_targets(&wave.target_ids)
                    })
                    && waves.iter().map(|wave| wave.delay_ms).sum::<u64>() < stage.duration_ms
            }
            StageObjective::KillQuota {
                target_weights,
                required,
                advance_on_timeout,
            } => {
                *required > 0
                    && (!advance_on_timeout || i + 1 < mode.stages.len())
                    && !target_weights.is_empty()
                    && target_weights.iter().all(|(id, &weight)| {
                        weight > 0 && targets.contains(id) && stage_targets.insert(id.clone())
                    })
                    && target_weights.values().map(|&w| u64::from(w)).sum::<u64>()
                        >= u64::from(*required)
            }
        };
        if !valid {
            return Err("CONFIG_MATCH_MODE_OBJECTIVE");
        }
    }
    if total > c.terminal_ms {
        return Err("CONFIG_MATCH_MODE_BOUNDS");
    }
    Ok(())
}
