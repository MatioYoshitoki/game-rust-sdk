use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SupportSkill {
    pub mechanism: SupportMechanism,
    pub id: String,
    pub name: String,
    pub range: f64,
    pub include_self: bool,
    pub hp_threshold: f64,
    pub cast_chance: f64,
    pub max_casts: u32,
    pub windup_ms: u64,
    pub tick_interval_ms: u64,
    pub ticks: u32,
    pub heal_bps: u32,
    pub cost: u32,
    pub cooldown_ms: u64,
    pub recovery_ms: u64,
    pub interrupt_window_ms: u64,
    pub interrupt_cooldown_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_encounter_completed: Option<CompletedEncounterSupport>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompletedEncounterSupport {
    pub encounter_id: String,
    pub id: String,
    pub name: String,
    pub heal_bps: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum SupportMechanism {
    #[serde(rename = "ally_heal_channel")]
    AllyHealChannel,
}
pub(super) fn validate(c: &Config, p: &PveConfig, t: &MonsterTemplate) -> Result<(), &'static str> {
    let Some(s) = &t.support else {
        return Ok(());
    };
    if c.mechanism_version < 10 {
        return Err("CONFIG_SUPPORT_CAPABILITY");
    }
    let id = |v: &str| !v.is_empty() && v.len() <= 64 && !v.chars().any(char::is_control);
    if !id(&s.id)
        || !id(&s.name)
        || s.id == t.combat.projectile.id
        || s.id == t.combat.heal.id
        || t.area.as_ref().is_some_and(|a| a.id == s.id)
    {
        return Err("CONFIG_SUPPORT_ID");
    }
    if let Some(variant) = &s.on_encounter_completed {
        if c.mechanism_version < 13 {
            return Err("CONFIG_SUPPORT_VARIANT_CAPABILITY");
        }
        if !p.encounters.iter().any(|e| e.id == variant.encounter_id) {
            return Err("CONFIG_SUPPORT_VARIANT_ENCOUNTER");
        }
        if !id(&variant.id)
            || !id(&variant.name)
            || variant.id == s.id
            || variant.id == t.combat.projectile.id
            || variant.id == t.combat.heal.id
            || t.area.as_ref().is_some_and(|a| a.id == variant.id)
            || t.heavy.as_ref().is_some_and(|h| h.id == variant.id)
        {
            return Err("CONFIG_SUPPORT_VARIANT_ID");
        }
        if variant.heal_bps > 100000 {
            return Err("CONFIG_SUPPORT_VARIANT_BOUNDS");
        }
    }
    let duration = s
        .windup_ms
        .checked_add(u64::from(s.ticks.saturating_sub(1)).saturating_mul(s.tick_interval_ms));
    if !s.range.is_finite()
        || !(0.1..=10000.0).contains(&s.range)
        || !s.hp_threshold.is_finite()
        || !(0.0..=1.0).contains(&s.hp_threshold)
        || !s.cast_chance.is_finite()
        || !(0.0..=1.0).contains(&s.cast_chance)
        || s.max_casts > 64
        || !(1..=64).contains(&s.ticks)
        || s.heal_bps > 100000
        || s.cost > 1000000
        || !(c.step_ms..=60000).contains(&s.windup_ms)
        || !(c.step_ms..=60000).contains(&s.tick_interval_ms)
        || duration.is_none_or(|d| {
            d > c.terminal_ms || d > 3600000 || !(c.step_ms..=d).contains(&s.interrupt_window_ms)
        })
        || !(1..=3600000).contains(&s.cooldown_ms)
        || s.recovery_ms > 3600000
        || s.interrupt_cooldown_ms > 3600000
    {
        return Err("CONFIG_SUPPORT_BOUNDS");
    }
    Ok(())
}
