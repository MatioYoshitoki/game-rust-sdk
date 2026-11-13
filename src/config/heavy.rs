use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HeavySkill {
    pub mechanism: HeavyMechanism,
    pub id: String,
    pub name: String,
    pub range: f64,
    pub half_width: f64,
    pub windup_ms: u64,
    pub travel_ms: u64,
    pub cooldown_ms: u64,
    pub recovery_ms: u64,
    pub cost: u32,
    pub damage_bps: u32,
    pub cast_chance: f64,
    pub min_phase_index: u32,
    pub counter_window_ms: u64,
    pub counter_damage_bps: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter_window_bonus_ms: Option<AfterEncounter<u64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter_damage_scale_bps: Option<AfterEncounter<u32>>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum HeavyMechanism {
    #[serde(rename = "locked_line_counter")]
    LockedLineCounter,
}
pub(super) fn validate(c: &Config, t: &MonsterTemplate) -> Result<(), &'static str> {
    let Some(h) = &t.heavy else { return Ok(()) };
    if c.mechanism_version < 11 {
        return Err("CONFIG_HEAVY_CAPABILITY");
    }
    if c.mechanism_version < 18
        && (h.counter_window_bonus_ms.is_some() || h.counter_damage_scale_bps.is_some())
    {
        return Err("CONFIG_COUNTER_SIDE_CAPABILITY");
    }
    if h.counter_window_bonus_ms
        .iter()
        .map(|rule| &rule.encounter_id)
        .chain(
            h.counter_damage_scale_bps
                .iter()
                .map(|rule| &rule.encounter_id),
        )
        .any(|id| {
            c.pve
                .as_ref()
                .is_none_or(|p| !p.encounters.iter().any(|e| &e.id == id))
        })
    {
        return Err("CONFIG_COUNTER_SIDE_REFERENCE");
    }
    if h.counter_window_bonus_ms.as_ref().is_some_and(|rule| {
        rule.value == 0
            || h.counter_window_ms
                .checked_add(rule.value)
                .is_none_or(|n| n > 60000)
    }) || h.counter_damage_scale_bps.as_ref().is_some_and(|rule| {
        !(10000..=100000).contains(&rule.value)
            || u64::from(h.counter_damage_bps) * u64::from(rule.value) > 1_000_000_000
    }) {
        return Err("CONFIG_COUNTER_SIDE_BOUNDS");
    }
    let id = |s: &str| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control);
    if !id(&h.id)
        || !id(&h.name)
        || h.id == t.combat.projectile.id
        || h.id == t.combat.heal.id
        || t.area.as_ref().is_some_and(|a| a.id == h.id)
        || t.support.as_ref().is_some_and(|s| s.id == h.id)
    {
        return Err("CONFIG_HEAVY_ID");
    }
    if !h.range.is_finite()
        || !(0.1..=10000.0).contains(&h.range)
        || !h.half_width.is_finite()
        || !(0.1..=512.0).contains(&h.half_width)
        || !(c.step_ms..=60000).contains(&h.windup_ms)
        || !(c.step_ms..=60000).contains(&h.travel_ms)
        || h.windup_ms
            .checked_add(h.travel_ms)
            .is_none_or(|n| n > c.terminal_ms || n > 3600000)
        || !(1..=3600000).contains(&h.cooldown_ms)
        || h.recovery_ms > 3600000
        || h.cost > 1000000
        || h.damage_bps > 100000
        || h.counter_damage_bps > 100000
        || !(c.step_ms..=60000).contains(&h.counter_window_ms)
        || !h.cast_chance.is_finite()
        || !(0.0..=1.0).contains(&h.cast_chance)
    {
        return Err("CONFIG_HEAVY_BOUNDS");
    }
    let phases = t.boss.as_ref().map_or(1, |b| b.phases.len() as u32);
    if !(1..=phases).contains(&h.min_phase_index) {
        return Err("CONFIG_HEAVY_PHASE");
    }
    Ok(())
}
