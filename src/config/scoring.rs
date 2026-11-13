use super::*;
use crate::protocol::{ScoreSummary, TemplateKills};
use std::collections::BTreeMap;

impl MonsterTemplate {
    pub(crate) fn effective_score(&self, c: &Config) -> u32 {
        if self.kill_credit {
            self.kill_score.unwrap_or(c.kill_score)
        } else {
            0
        }
    }
}
impl PveConfig {
    pub(crate) fn profile(&self, id: &str) -> Option<Arc<MonsterTemplate>> {
        if self.profiles.is_empty() {
            self.build_profiles().get(id).cloned()
        } else {
            self.profiles.get(id).cloned()
        }
    }
}
impl Config {
    pub(crate) fn scoring_enabled(&self) -> bool {
        self.mechanism_version >= 7 && self.pve.is_some()
    }
    pub(crate) fn max_kill_score(&self, now: u64) -> u64 {
        self.pve.as_ref().map_or(u64::from(self.kill_score), |p| {
            p.population_bound(now.min(self.terminal_ms))
                .iter()
                .map(|(t, n)| u64::from(t.effective_score(self)) * u64::from(*n))
                .sum()
        })
    }
    pub(crate) fn score_summary(&self, tally: &BTreeMap<String, u32>) -> ScoreSummary {
        let p = self.pve.as_ref().unwrap();
        let mut kills = 0;
        let mut credited_kills = 0;
        let mut points = 0u64;
        let templates = tally
            .iter()
            .map(|(id, &count)| {
                let t = p.profile(id).expect("registered runtime template");
                kills += count;
                if t.kill_credit {
                    credited_kills += count;
                }
                points += u64::from(count) * u64::from(t.effective_score(self));
                TemplateKills {
                    template_id: id.clone(),
                    kills: count,
                }
            })
            .collect();
        ScoreSummary {
            kills,
            credited_kills,
            kill_score: u32::try_from(points).expect("validated scoring budget"),
            templates,
        }
    }
    pub(crate) fn valid_scoring(&self, s: &ScoreSummary, now: u64) -> bool {
        if !self.scoring_enabled() || s.templates.len() > 17 {
            return false;
        }
        let p = self.pve.as_ref().unwrap();
        let population = p.population_counts(now.min(self.terminal_ms));
        let mut previous = None;
        let mut kills = 0u64;
        let mut credited = 0u64;
        let mut points = 0u64;
        for item in &s.templates {
            if item.kills == 0 || previous.is_some_and(|id: &str| id >= item.template_id.as_str()) {
                return false;
            }
            previous = Some(item.template_id.as_str());
            let Some(t) = p.profile(&item.template_id) else {
                return false;
            };
            let max = population.get(&item.template_id).copied().unwrap_or(0);
            if item.kills > max {
                return false;
            }
            kills += u64::from(item.kills);
            if t.kill_credit {
                credited += u64::from(item.kills);
            }
            points += u64::from(item.kills) * u64::from(t.effective_score(self));
        }
        kills == u64::from(s.kills)
            && credited == u64::from(s.credited_kills)
            && points == u64::from(s.kill_score)
    }
    pub(crate) fn scored_loot_bound(&self, item: &str, s: &ScoreSummary, now: u64) -> u32 {
        let p = self.pve.as_ref().unwrap();
        let monster = s
            .templates
            .iter()
            .map(|row| {
                let t = p
                    .profile(&row.template_id)
                    .expect("validated score template");
                self.monster_loot_bound(&row.template_id, &t.drop_table, item)
                    * u64::from(row.kills)
            })
            .sum::<u64>();
        (monster + self.pve_chest_loot_bound(item, now) + self.world_loot_bound(item, now))
            .min(u64::from(u32::MAX)) as u32
    }
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    if c.mechanism_version >= 7 && c.pve.is_none() {
        return Err("CONFIG_SCORING_CAPABILITY");
    }
    if let Some(p) = &c.pve {
        if c.mechanism_version < 7
            && (p.kill_score.is_some()
                || p.additional_templates
                    .values()
                    .any(|t| t.kill_score.is_some()))
        {
            return Err("CONFIG_SCORING_CAPABILITY");
        }
        if c.scoring_enabled() {
            for t in p.build_profiles().values() {
                if t.kill_score
                    .is_some_and(|n| n > 1000000 || !t.kill_credit && n > 0)
                    || t.kill_credit && t.effective_score(c) > 1000000
                {
                    return Err("CONFIG_KILL_SCORE_BOUNDS");
                }
            }
            if c.max_kill_score(c.terminal_ms) > u64::from(u32::MAX) {
                return Err("CONFIG_KILL_SCORE_OVERFLOW");
            }
        }
    }
    Ok(())
}
