use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AreaSkill {
    pub mechanism: AreaMechanism,
    pub id: String,
    pub name: String,
    pub range: f64,
    pub radius: f64,
    pub windup_ms: u64,
    pub cooldown_ms: u64,
    pub recovery_ms: u64,
    pub cost: u32,
    pub damage_bps: u32,
    pub cast_chance: f64,
    pub min_phase_index: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum AreaMechanism {
    #[serde(rename = "locked_circle")]
    LockedCircle,
}
pub(super) fn validate(c: &Config, t: &MonsterTemplate) -> Result<(), &'static str> {
    let Some(a) = &t.area else { return Ok(()) };
    if c.mechanism_version < 8 {
        return Err("CONFIG_AREA_CAPABILITY");
    }
    let id = |v: &str| !v.is_empty() && v.len() <= 64 && !v.chars().any(char::is_control);
    if !id(&a.id) || !id(&a.name) || a.id == t.combat.projectile.id || a.id == t.combat.heal.id {
        return Err("CONFIG_AREA_ID");
    }
    if !a.range.is_finite()
        || !(0.1..=10000.0).contains(&a.range)
        || !a.radius.is_finite()
        || !(0.1..=512.0).contains(&a.radius)
        || !(c.step_ms..=60000).contains(&a.windup_ms)
        || !(1..=3600000).contains(&a.cooldown_ms)
        || a.recovery_ms > 3600000
        || a.cost > 1000000
        || a.damage_bps > 100000
        || !a.cast_chance.is_finite()
        || !(0.0..=1.0).contains(&a.cast_chance)
    {
        return Err("CONFIG_AREA_BOUNDS");
    }
    let phases = t.boss.as_ref().map_or(1, |b| b.phases.len() as u32);
    if a.min_phase_index == 0 || a.min_phase_index > phases {
        return Err("CONFIG_AREA_PHASE");
    }
    Ok(())
}
