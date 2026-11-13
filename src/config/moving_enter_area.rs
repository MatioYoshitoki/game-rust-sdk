use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcEnterArea {
    pub speed: f64,
    pub max_distance: f64,
    pub radius: f64,
    pub duration_ms: u64,
    pub end_on_source_invalid: bool,
    pub contact_policy: ContactPolicyConfig,
    pub target_max_hp_bps: u32,
    pub knockback: AreaKnockback,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained_hit_statuses: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AreaKnockback {
    pub distance: f64,
    pub slide_speed_multiplier: f64,
}

impl Config {
    pub(crate) fn npc_enter_area(&self, skill_id: &str) -> Option<&NpcEnterArea> {
        self.combat_rules.as_ref()?.npc_enter_areas.get(skill_id)
    }
}

pub(super) fn validate(c: &Config, rules: &CombatRules) -> Result<(), &'static str> {
    if rules.npc_enter_areas.is_empty() {
        return Ok(());
    }
    let p = c.pve.as_ref().ok_or("CONFIG_NPC_ENTER_AREA")?;
    let abilities = std::iter::once(&p.combat.projectile)
        .chain(
            p.additional_templates
                .values()
                .map(|t| &t.combat.projectile),
        )
        .filter(|s| rules.npc_enter_areas.contains_key(&s.id))
        .collect::<Vec<_>>();
    let ids = abilities
        .iter()
        .map(|s| s.id.as_str())
        .collect::<BTreeSet<_>>();
    if rules.npc_enter_areas.len() > 16
        || c.members.as_ref().is_none_or(|m| m.area_capacity.is_none())
        || p.status_rules.is_none()
        || abilities.iter().any(|s| {
            s.damage_bps != 0
                || c.player_skill(&s.id).is_some()
                || s.windup_ms == 0
                || s.windup_ms > 60000
                || {
                    let ids = &rules.npc_enter_areas[&s.id].retained_hit_statuses;
                    let combined = s
                        .on_hit_target_statuses
                        .iter()
                        .chain(ids)
                        .collect::<BTreeSet<_>>();
                    combined.len() != s.on_hit_target_statuses.len() + ids.len()
                        || combined.len() > 2
                }
        })
        || rules.npc_enter_areas.iter().any(|(id, a)| {
            !ids.contains(id.as_str())
                || !(0.1..=10000.).contains(&a.speed)
                || !(0.1..=10000.).contains(&a.max_distance)
                || !(0.1..=128.).contains(&a.radius)
                || !(c.step_ms..=60000).contains(&a.duration_ms)
                || !(1..=8).contains(&a.contact_policy.max_targets_per_tick)
                || !(1..=128).contains(&a.contact_policy.max_total_hits)
                || a.contact_policy.repeat_interval_ms > 60000
                || !(1..=10000).contains(&a.target_max_hp_bps)
                || !(0.1..=1000.).contains(&a.knockback.distance)
                || !(0.1..=10.).contains(&a.knockback.slide_speed_multiplier)
                || a.end_on_source_invalid && !a.retained_hit_statuses.is_empty()
                || a.retained_hit_statuses.iter().any(|id| {
                    rules.status_windows.contains_key(id)
                        || rules.status_cleanses.contains_key(id)
                        || rules.status_ends.contains_key(id)
                        || !p.status_definitions.iter().any(|d| {
                            &d.id == id
                                && matches!(d.mechanism, StatusMechanism::TimedAttributeModifier)
                                && d.attribute
                                    == Some(crate::protocol::StatusAttribute::MovementSlow)
                                && d.modifier_bps < 0
                                && d.source_scoped
                                && d.flags.is_empty()
                                && d.modifiers.is_empty()
                                && d.base_modifiers.is_empty()
                                && d.stack_counter.is_none()
                                && matches!(
                                    d.duplicate_policy,
                                    DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
                                )
                        })
                })
        })
    {
        return Err("CONFIG_NPC_ENTER_AREA");
    }
    Ok(())
}
