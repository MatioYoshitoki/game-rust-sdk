use super::*;
use std::collections::BTreeSet;

pub(crate) const MAX_CHECKPOINT_SCHEDULES: usize = 4;
pub(crate) const MAX_CHECKPOINT_WINDOWS: usize = 32;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointSchedule {
    pub id: String,
    pub first_delay_ms: u64,
    pub interval_ms: u64,
    pub window_ms: u64,
    pub radius: f64,
    pub required: u32,
    pub centers: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "CheckpointQualification::is_reach")]
    pub qualification: CheckpointQualification,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub counter: Option<CheckpointCounterRule>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CounterActivation {
    Manual,
    OnReach,
    FirstContact,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CheckpointQualification {
    #[default]
    Reach,
    Avoid,
}

impl CheckpointQualification {
    fn is_reach(&self) -> bool {
        *self == Self::Reach
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CounterBoon {
    pub id: String,
    pub by_level: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointCounterRule {
    pub activation: CounterActivation,
    pub target_ids: Vec<String>,
    pub range: f64,
    pub base_damage: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub window_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub window_extension: Option<CounterBoon>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub damage_bonus: Option<CounterBoon>,
}

impl CheckpointCounterRule {
    pub(super) fn uses_boon(&self, id: &str) -> bool {
        [&self.window_extension, &self.damage_bonus]
            .into_iter()
            .flatten()
            .any(|rule| rule.id == id)
    }
}

impl Config {
    pub(crate) fn checkpoint_schedules(&self) -> &[CheckpointSchedule] {
        self.members
            .as_ref()
            .map_or(&[], |members| members.checkpoint_schedules.as_slice())
    }
}

pub(super) fn validate(c: &Config, capacity: usize) -> Result<(), &'static str> {
    let schedules = c.checkpoint_schedules();
    if schedules.is_empty() {
        return Ok(());
    }
    let mut ids = BTreeSet::new();
    if c.mechanism_version < 105
        || c.schema_version != 7
        || schedules.len() > MAX_CHECKPOINT_SCHEDULES
        || (64 + schedules.len()) * capacity > c.output_capacity
        || schedules.iter().any(|schedule| {
            let windows = schedule.centers.len();
            !crate::protocol::valid_id(&schedule.id)
                || !ids.insert(&schedule.id)
                || !(1..=MAX_CHECKPOINT_WINDOWS).contains(&windows)
                || !(1..=windows as u32).contains(&schedule.required)
                || !(0.1..=512.).contains(&schedule.radius)
                || !(c.step_ms..=60000).contains(&schedule.window_ms)
                || !(schedule.window_ms..=c.terminal_ms).contains(&schedule.interval_ms)
                || !schedule.centers.iter().flatten().all(|v| v.is_finite())
                || schedule
                    .interval_ms
                    .checked_mul((windows - 1) as u64)
                    .and_then(|last| last.checked_add(schedule.first_delay_ms))
                    .and_then(|last| last.checked_add(schedule.window_ms))
                    .is_none_or(|end| {
                        end > c.terminal_ms
                            || schedule.qualification == CheckpointQualification::Avoid
                                && end.div_ceil(c.step_ms) >= c.terminal_ms.div_ceil(c.step_ms)
                    })
        })
    {
        return Err("CONFIG_CHECKPOINT_SCHEDULES");
    }
    if schedules.iter().any(|s| s.counter.is_some()) {
        if c.combat_rules.is_none() || c.pve.is_none() {
            return Err("CONFIG_CHECKPOINT_COUNTER");
        }
        let targets = crate::world::two_members::World::npc_ids(c);
        let bonus_ok = |bonus: &CounterBoon, max| {
            c.world_boons
                .as_ref()
                .and_then(|boons| boons.definitions.get(&bonus.id))
                .is_some_and(|boon| boon.levels.len() == bonus.by_level.len())
                && bonus
                    .by_level
                    .last()
                    .is_some_and(|last| (1..=max).contains(last))
                && bonus.by_level.windows(2).all(|row| row[0] <= row[1])
        };
        for schedule in schedules {
            let Some(rule) = &schedule.counter else {
                continue;
            };
            let window_ok = match rule.activation {
                CounterActivation::OnReach => {
                    rule.window_ms.is_none() && rule.window_extension.is_none()
                }
                CounterActivation::Manual | CounterActivation::FirstContact => {
                    rule.window_ms.is_some_and(|base| {
                        (c.step_ms..=60000).contains(&base)
                            && rule
                                .window_extension
                                .as_ref()
                                .is_none_or(|bonus| bonus_ok(bonus, (60000 - base) as u32))
                    })
                }
            };
            if !window_ok
                || !(1..=1_000_000).contains(&rule.base_damage)
                || !(0.1..=1000.).contains(&rule.range)
                || rule.target_ids.len() != schedule.centers.len()
                || rule.target_ids.iter().any(|id| !targets.contains(id))
                || rule
                    .damage_bonus
                    .as_ref()
                    .is_some_and(|bonus| !bonus_ok(bonus, 10000))
            {
                return Err("CONFIG_CHECKPOINT_COUNTER");
            }
        }
    }
    Ok(())
}
