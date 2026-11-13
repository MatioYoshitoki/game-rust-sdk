use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcGroundField {
    pub radius: f64,
    pub duration_ms: u64,
    pub interval_ms: u64,
    pub max_targets_per_tick: u32,
    pub max_total_hits: u32,
    pub end_on_source_invalid: bool,
    pub target_max_hp_bps: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcGroundStrike {
    pub radius: f64,
    pub target_max_hp_bps: u32,
    pub hit_status_chance_bps: u32,
}

#[derive(Clone, Copy)]
pub(crate) enum NpcGroundDelivery {
    Field(NpcGroundField),
    Strike(NpcGroundStrike),
}

impl NpcGroundDelivery {
    pub(crate) fn survives_source_loss(self) -> bool {
        matches!(self, Self::Field(field) if !field.end_on_source_invalid)
    }
}

impl Config {
    pub(crate) fn npc_ground_delivery(&self, skill_id: &str) -> Option<NpcGroundDelivery> {
        let rules = self.combat_rules.as_ref()?;
        rules
            .npc_ground_fields
            .get(skill_id)
            .copied()
            .map(NpcGroundDelivery::Field)
            .or_else(|| {
                rules
                    .npc_ground_strikes
                    .get(skill_id)
                    .copied()
                    .map(NpcGroundDelivery::Strike)
            })
    }
}

pub(super) fn validate(c: &Config, rules: &CombatRules) -> Result<(), &'static str> {
    let fields = &rules.npc_ground_fields;
    let strikes = &rules.npc_ground_strikes;
    let error = if fields.is_empty() {
        "CONFIG_NPC_GROUND_STRIKE"
    } else {
        "CONFIG_NPC_GROUND_FIELD"
    };
    if fields.is_empty() && strikes.is_empty() {
        return Ok(());
    }
    let p = c.pve.as_ref().ok_or(error)?;
    let mut known = BTreeSet::new();
    for ability in std::iter::once(&p.combat.projectile).chain(
        p.additional_templates
            .values()
            .map(|t| &t.combat.projectile),
    ) {
        if fields.contains_key(&ability.id) || strikes.contains_key(&ability.id) {
            if ability.damage_bps != 0
                || !(1..=60000).contains(&ability.windup_ms)
                || c.player_skill(&ability.id).is_some()
                || rules.npc_enter_areas.contains_key(&ability.id)
                || fields.contains_key(&ability.id) && strikes.contains_key(&ability.id)
            {
                return Err(error);
            }
            if strikes.get(&ability.id).is_some_and(|s| {
                s.hit_status_chance_bps > 0 && ability.on_hit_target_statuses.is_empty()
            }) {
                return Err("CONFIG_NPC_GROUND_STRIKE");
            }
            known.insert(ability.id.as_str());
        }
    }
    if fields.len() + strikes.len() > 16
        || c.members.as_ref().is_none_or(|m| m.area_capacity.is_none())
        || p.status_rules.is_none()
    {
        return Err(error);
    }
    if fields.iter().any(|(id, f)| {
        !known.contains(id.as_str())
            || !(0.1..=512.).contains(&f.radius)
            || !(c.step_ms..=60000).contains(&f.interval_ms)
            || !(f.interval_ms..=60000).contains(&f.duration_ms)
            || !f.duration_ms.is_multiple_of(f.interval_ms)
            || f.duration_ms / f.interval_ms > 128
            || !(1..=16).contains(&f.max_targets_per_tick)
            || !(1..=256).contains(&f.max_total_hits)
            || !(1..=10000).contains(&f.target_max_hp_bps)
    }) {
        return Err("CONFIG_NPC_GROUND_FIELD");
    }
    if strikes.iter().any(|(id, s)| {
        !known.contains(id.as_str())
            || !(0.1..=512.).contains(&s.radius)
            || !(1..=10000).contains(&s.target_max_hp_bps)
            || s.hit_status_chance_bps > 10000
    }) {
        return Err("CONFIG_NPC_GROUND_STRIKE");
    }
    Ok(())
}
