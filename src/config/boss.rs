use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BossConfig {
    pub phases: Vec<BossPhase>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BossPhase {
    pub id: String,
    pub name: String,
    pub hp_threshold_bps: u32,
    pub attack_interval_ms: u64,
    pub cast_chance: f64,
}
pub(super) fn validate(c: &Config, t: &MonsterTemplate) -> Result<(), &'static str> {
    let Some(b) = &t.boss else {
        return Ok(());
    };
    if c.mechanism_version < 6 {
        return Err("CONFIG_BOSS_CAPABILITY");
    }
    if !(1..=8).contains(&b.phases.len()) {
        return Err("CONFIG_BOSS_PHASE_BUDGET");
    }
    let id = |s: &str| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control);
    let mut ids = BTreeSet::new();
    for (i, p) in b.phases.iter().enumerate() {
        if !id(&p.id) || !id(&p.name) || !ids.insert(&p.id) {
            return Err("CONFIG_BOSS_PHASE_ID");
        }
        if !(1..=10000).contains(&p.hp_threshold_bps)
            || i == 0 && p.hp_threshold_bps != 10000
            || i > 0 && p.hp_threshold_bps >= b.phases[i - 1].hp_threshold_bps
        {
            return Err("CONFIG_BOSS_THRESHOLDS");
        }
        if !(1..=3600000).contains(&p.attack_interval_ms)
            || !p.cast_chance.is_finite()
            || !(0.0..=1.0).contains(&p.cast_chance)
        {
            return Err("CONFIG_BOSS_BEHAVIOR_BOUNDS");
        }
    }
    Ok(())
}
