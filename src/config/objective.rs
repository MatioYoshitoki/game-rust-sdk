use super::*;
use crate::protocol::{ItemAmount, MapProgressSummary, ScoreSummary};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MapObjectiveConfig {
    pub id: String,
    pub name: String,
    pub required_encounter_ids: Vec<String>,
    pub reward_chest_id: String,
    pub attune_duration_ms: u64,
    pub cost: ItemAmount,
}
pub(super) fn validate(c: &Config, p: &PveConfig) -> Result<(), &'static str> {
    let id = |s: &str| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control);
    let graph: BTreeMap<_, _> = p.encounters.iter().map(|e| (e.id.as_str(), e)).collect();
    for e in &p.encounters {
        if !e.requires.is_empty() && c.mechanism_version < 9 {
            return Err("CONFIG_OBJECTIVE_CAPABILITY");
        }
        let mut seen = BTreeSet::new();
        if e.requires
            .iter()
            .any(|v| v == &e.id || !graph.contains_key(v.as_str()) || !seen.insert(v))
        {
            return Err("CONFIG_OBJECTIVE_DEPENDENCY");
        }
    }
    let mut done = BTreeSet::new();
    for _ in 0..graph.len() {
        for e in &p.encounters {
            if e.requires.iter().all(|v| done.contains(v.as_str())) {
                done.insert(e.id.as_str());
            }
        }
    }
    if done.len() != graph.len() {
        return Err("CONFIG_OBJECTIVE_CYCLE");
    }
    let Some(o) = &p.map_objective else {
        return Ok(());
    };
    if c.mechanism_version < 9 {
        return Err("CONFIG_OBJECTIVE_CAPABILITY");
    }
    let required: BTreeSet<_> = o
        .required_encounter_ids
        .iter()
        .map(String::as_str)
        .collect();
    if !id(&o.id)
        || !id(&o.name)
        || required.is_empty()
        || required.len() != o.required_encounter_ids.len()
        || required.iter().any(|key| {
            graph
                .get(key)
                .is_none_or(|e| e.requires.iter().any(|d| !required.contains(d.as_str())))
        })
        || !p.chest_spawns.iter().any(|ch| ch.id == o.reward_chest_id)
        || !p.items.contains_key(&o.cost.item_id)
        || o.cost.quantity > 1_000_000
        || !(c.step_ms..=60000).contains(&o.attune_duration_ms)
        || o.attune_duration_ms > c.terminal_ms
    {
        return Err("CONFIG_OBJECTIVE_REFERENCE_OR_BOUNDS");
    }
    Ok(())
}
impl Config {
    pub(crate) fn objective_loot_bound(&self, item: &str) -> u32 {
        self.pve
            .as_ref()
            .filter(|p| p.map_objective.is_some())
            .and_then(|p| p.treasure_rules.as_ref())
            .map_or(0, |r| {
                r.loot
                    .guaranteed
                    .iter()
                    .chain(&r.loot.random)
                    .filter(|v| v.item_id == item && v.chance != Some(0.0))
                    .map(|v| v.quantity_max)
                    .sum()
            })
    }

    pub(crate) fn valid_map_progress(
        &self,
        summary: Option<&MapProgressSummary>,
        score: Option<&ScoreSummary>,
    ) -> bool {
        let Some(o) = self.pve.as_ref().and_then(|p| p.map_objective.as_ref()) else {
            return summary.is_none();
        };
        let (Some(s), Some(score)) = (summary, score) else {
            return false;
        };
        let p = self.pve.as_ref().unwrap();
        let completed: BTreeSet<_> = s.completed_encounter_ids.iter().collect();
        if s.completed_encounter_ids.windows(2).any(|w| w[0] >= w[1])
            || completed.len() > p.encounters.len()
        {
            return false;
        }
        let mut necessary: BTreeMap<&str, u32> = BTreeMap::new();
        for id in completed.iter() {
            let Some(e) = p.encounters.iter().find(|e| &e.id == *id) else {
                return false;
            };
            if e.requires.iter().any(|v| !completed.contains(v)) {
                return false;
            }
            for monster in e
                .all_monsters()
                .filter(|m| e.all_required().any(|id| id == &m.id))
            {
                *necessary.entry(&monster.template_id).or_default() += 1;
            }
        }
        if necessary.iter().any(|(id, n)| {
            score
                .templates
                .iter()
                .find(|t| &t.template_id == id)
                .is_none_or(|t| t.kills < *n)
        }) {
            return false;
        }
        if (s.attuned
            && o.required_encounter_ids
                .iter()
                .any(|v| !completed.contains(v)))
            || (s.reward_opened && !s.attuned)
        {
            return false;
        }
        let expected = if s.attuned && o.cost.quantity > 0 {
            vec![o.cost.clone()]
        } else {
            vec![]
        };
        s.consumed == expected
    }
}
