use super::*;
use crate::protocol::{MapProgressSummary, ResetProgressSummary, ScoreSummary};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncounterResetConfig {
    pub mechanism: ResetMechanism,
    pub reward_policy: ResetRewardPolicy,
    pub pursuit_radius: f64,
    pub delay_ms: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum ResetMechanism {
    #[serde(rename = "whole_encounter_round")]
    WholeEncounterRound,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum ResetRewardPolicy {
    #[serde(rename = "first_defeat_per_spawn")]
    FirstDefeatPerSpawn,
}
impl Config {
    pub(crate) fn resets_enabled(&self) -> bool {
        self.pve
            .as_ref()
            .is_some_and(|p| p.encounters.iter().any(|e| e.reset.is_some()))
    }
    pub(crate) fn valid_reset_progress(
        &self,
        value: Option<&ResetProgressSummary>,
        score: Option<&ScoreSummary>,
        map: Option<&MapProgressSummary>,
        now: u64,
    ) -> bool {
        if !self.resets_enabled() {
            return value.is_none();
        }
        let (Some(s), Some(score)) = (value, score) else {
            return false;
        };
        let p = self.pve.as_ref().unwrap();
        let mut configured: Vec<_> = p.encounters.iter().filter(|e| e.reset.is_some()).collect();
        configured.sort_by(|a, b| a.id.cmp(&b.id));
        if s.encounters.len() != configured.len() || s.actual_kills > 4096 {
            return false;
        }
        let mut first: BTreeMap<&str, u32> = BTreeMap::new();
        let mut repeats = 0u64;
        for (e, row) in configured.iter().zip(&s.encounters) {
            let r = e.reset.as_ref().unwrap();
            if row.encounter_id != e.id
                || row.round == 0
                || u64::from(row.round) > 1 + now.min(self.terminal_ms) / r.delay_ms
                || row.defeats.len() > e.all_monsters().count()
            {
                return false;
            }
            let mut previous = None;
            for d in &row.defeats {
                if previous.is_some_and(|id: &str| id >= d.monster_id.as_str())
                    || d.deaths == 0
                    || d.last_round > row.round
                    || d.deaths > d.last_round
                {
                    return false;
                }
                previous = Some(d.monster_id.as_str());
                let Some(spawn) = e.all_monsters().find(|m| m.id == d.monster_id) else {
                    return false;
                };
                *first.entry(&spawn.template_id).or_default() += 1;
                repeats += u64::from(d.deaths - 1);
            }
            if row.completed
                && e.all_required().any(|id| {
                    !row.defeats
                        .iter()
                        .any(|d| &d.monster_id == id && d.last_round == row.round)
                })
            {
                return false;
            }
            if map.is_some_and(|m| m.completed_encounter_ids.contains(&e.id) != row.completed) {
                return false;
            }
        }
        if u64::from(s.actual_kills) != u64::from(score.kills) + repeats {
            return false;
        }
        let mut other = p.population_counts(now.min(self.terminal_ms));
        for e in configured {
            for spawn in e.all_monsters() {
                *other.get_mut(&spawn.template_id).unwrap() -= 1;
            }
        }
        for (id, count) in &first {
            if score
                .templates
                .iter()
                .find(|t| t.template_id == *id)
                .is_none_or(|t| t.kills < *count)
            {
                return false;
            }
        }
        score.templates.iter().all(|t| {
            t.kills >= first.get(t.template_id.as_str()).copied().unwrap_or(0)
                && t.kills - first.get(t.template_id.as_str()).copied().unwrap_or(0)
                    <= other.get(&t.template_id).copied().unwrap_or(0)
        })
    }
}
pub(super) fn validate(c: &Config, p: &PveConfig) -> Result<(), &'static str> {
    if !p.encounters.iter().any(|e| e.reset.is_some()) {
        return Ok(());
    }
    if c.mechanism_version < 12 {
        return Err("CONFIG_RESET_CAPABILITY");
    }
    let mut deaths: u64 = p
        .population_counts(c.terminal_ms)
        .values()
        .map(|n| u64::from(*n))
        .sum();
    let mut extra_orbs = 0u64;
    for e in &p.encounters {
        let Some(r) = &e.reset else { continue };
        if !r.pursuit_radius.is_finite()
            || !(e.trigger.radius..=10000.0).contains(&r.pursuit_radius)
            || !(c.step_ms..=60000).contains(&r.delay_ms)
            || r.delay_ms > c.terminal_ms
        {
            return Err("CONFIG_RESET_BOUNDS");
        }
        for m in e.all_monsters() {
            let t = p
                .profile(&m.template_id)
                .ok_or("CONFIG_MONSTER_REFERENCE")?;
            if distance(m.position, e.trigger.center) + t.combat.radius > r.pursuit_radius {
                return Err("CONFIG_RESET_PLACEMENT");
            }
            deaths += c.terminal_ms / r.delay_ms;
            if t.drop_orb {
                extra_orbs += (c.params.resource_ttl_ms + c.params.runtime.resource_sweep_ms)
                    .min(c.terminal_ms)
                    / r.delay_ms;
            }
        }
    }
    if deaths > 4096 {
        return Err("CONFIG_RESET_DEATH_BUDGET");
    }
    let original: u64 = p
        .population_bound(c.terminal_ms)
        .iter()
        .map(|(t, n)| {
            u64::from(*n)
                * (t.drop_table.guaranteed.len()
                    + t.drop_table.random.len()
                    + usize::from(t.drop_orb)) as u64
        })
        .sum::<u64>()
        + p.treasure_rules.as_ref().map_or(0, |t| {
            p.chest_spawns.len() * (t.loot.guaranteed.len() + t.loot.random.len()) * 2
        }) as u64;
    if original + extra_orbs > 128 {
        return Err("CONFIG_RESET_RESOURCE_BUDGET");
    }
    Ok(())
}
