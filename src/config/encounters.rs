use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncounterConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset: Option<EncounterResetConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    pub id: String,
    pub name: String,
    pub trigger: EncounterTrigger,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub monsters: Vec<MonsterSpawn>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_monster_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waves: Vec<EncounterWave>,
    pub reward_chest_ids: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncounterTrigger {
    pub center: [f64; 2],
    pub radius: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncounterWave {
    pub id: String,
    pub delay_ms: u64,
    pub monsters: Vec<MonsterSpawn>,
    pub required_monster_ids: Vec<String>,
}
impl EncounterConfig {
    pub fn group_count(&self) -> usize {
        self.waves.len().max(1)
    }
    pub fn group(&self, index: usize) -> (&[MonsterSpawn], &[String]) {
        if self.waves.is_empty() {
            (&self.monsters, &self.required_monster_ids)
        } else {
            (
                &self.waves[index].monsters,
                &self.waves[index].required_monster_ids,
            )
        }
    }
    pub fn all_monsters(&self) -> impl Iterator<Item = &MonsterSpawn> {
        self.monsters
            .iter()
            .chain(self.waves.iter().flat_map(|w| &w.monsters))
    }
    pub fn all_required(&self) -> impl Iterator<Item = &String> {
        self.required_monster_ids
            .iter()
            .chain(self.waves.iter().flat_map(|w| &w.required_monster_ids))
    }
}
pub(super) fn validate(c: &Config, p: &PveConfig) -> Result<(), &'static str> {
    if p.encounters.is_empty() {
        return Ok(());
    }
    if c.mechanism_version < 4 {
        return Err("CONFIG_ENCOUNTER_CAPABILITY");
    }
    if p.encounters.len() > 8 {
        return Err("CONFIG_ENCOUNTER_BUDGET");
    }
    let id = |s: &str| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control);
    let mut names = BTreeSet::new();
    let mut rewards = BTreeSet::new();
    let profiles = p.build_profiles();
    for e in &p.encounters {
        if !id(&e.id)
            || !id(&e.name)
            || !names.insert(&e.id)
            || (e.reward_chest_ids.is_empty() && p.map_objective.is_none())
            || e.reward_chest_ids.len() > p.chest_spawns.len()
        {
            return Err("CONFIG_ENCOUNTER_REFERENCE");
        }
        if !e.waves.is_empty() {
            if c.mechanism_version < 5 {
                return Err("CONFIG_WAVE_CAPABILITY");
            }
            if !e.monsters.is_empty() || !e.required_monster_ids.is_empty() {
                return Err("CONFIG_WAVE_MIXED_FORM");
            }
            if e.waves.len() > 8 {
                return Err("CONFIG_WAVE_BUDGET");
            }
            let mut waves = BTreeSet::new();
            if e.waves
                .iter()
                .any(|w| !id(&w.id) || !waves.insert(&w.id) || w.delay_ms > 60000)
                || e.waves.iter().map(|w| w.delay_ms).sum::<u64>() > c.terminal_ms
            {
                return Err("CONFIG_WAVE_BOUNDS");
            }
        }
        if !e.trigger.radius.is_finite()
            || !(0.1..=1000.0).contains(&e.trigger.radius)
            || e.trigger
                .center
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 32768.0)
        {
            return Err("CONFIG_ENCOUNTER_TRIGGER");
        }
        for chest in &e.reward_chest_ids {
            if !rewards.insert(chest) || !p.chest_spawns.iter().any(|v| &v.id == chest) {
                return Err("CONFIG_ENCOUNTER_REFERENCE");
            }
        }
        for index in 0..e.group_count() {
            let (monsters, required) = e.group(index);
            if monsters.is_empty()
                || monsters.len() > 32
                || required.is_empty()
                || required.len() > monsters.len()
            {
                return Err("CONFIG_ENCOUNTER_REFERENCE");
            }
            let mut targets = BTreeSet::new();
            for target in required {
                if !targets.insert(target) || !monsters.iter().any(|m| &m.id == target) {
                    return Err("CONFIG_ENCOUNTER_REFERENCE");
                }
            }
            for (i, a) in monsters.iter().enumerate() {
                let Some(t) = profiles.get(&a.template_id) else {
                    return Err("CONFIG_MONSTER_REFERENCE");
                };
                for b in &monsters[..i] {
                    let Some(u) = profiles.get(&b.template_id) else {
                        return Err("CONFIG_MONSTER_REFERENCE");
                    };
                    if distance(a.position, b.position) < t.combat.radius + u.combat.radius {
                        return Err("CONFIG_ENCOUNTER_PLACEMENT");
                    }
                }
            }
        }
    }
    Ok(())
}
