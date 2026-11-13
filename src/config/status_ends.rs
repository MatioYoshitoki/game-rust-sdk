//! Finite status end plans. A configured leaf cannot own another release.
use super::*;
use crate::protocol::StatusRemovalReason;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusEndRule {
    pub on: Vec<StatusRemovalReason>,
    pub target_statuses: Vec<String>,
}
pub(super) fn default_budget() -> usize {
    32
}
pub(super) fn default_value(v: &usize) -> bool {
    *v == 32
}
pub(crate) fn derived_leaf(c: &Config, id: &str, beneficial: bool) -> bool {
    let Some(r) = &c.combat_rules else {
        return false;
    };
    c.pve.as_ref().unwrap().status_definitions.iter().any(|d| {
        d.id == id
            && d.source_scoped
            && !matches!(d.duplicate_policy, DuplicatePolicy::Stack)
            && d.stack_counter.is_none()
            && d.overheal_shield.is_none()
            && !r.status_ends.contains_key(id)
            && !r.cast_interceptions.contains_key(id)
            && !r.shield_records.contains_key(id)
            && !r.status_windows.contains_key(id)
            && !r.status_cleanses.contains_key(id)
            && !r.contact_reactions.contains_key(id)
            && !r.status_retaliations.contains_key(id)
            && !r.status_death_guards.contains_key(id)
            && r.periodic_damage
                .get(id)
                .is_none_or(|rule| rule.life_steal_bps == 0)
            && if beneficial {
                !d.is_hostile_control() && (!d.is_periodic() || d.is_heal())
            } else {
                !d.is_heal() && !d.is_shield()
            }
    })
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.status_ends.len() > 16 || !(1..=128).contains(&r.max_status_end_releases_per_step) {
        return Err("CONFIG_STATUS_END_BUDGET");
    }
    for (id, rule) in &r.status_ends {
        if !c
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .any(|d| d.id == *id && d.source_scoped)
            || rule.on.is_empty()
            || rule.on.len() > 4
            || rule.on.iter().enumerate().any(|(i, v)| {
                rule.on[..i].contains(v)
                    || !matches!(
                        v,
                        StatusRemovalReason::Expired
                            | StatusRemovalReason::Depleted
                            | StatusRemovalReason::Consumed
                            | StatusRemovalReason::Cleansed
                    )
            })
            || rule.target_statuses.is_empty()
            || rule.target_statuses.len() > 2
            || rule.target_statuses.iter().enumerate().any(|(i, v)| {
                rule.target_statuses[..i].contains(v)
                    || !(derived_leaf(c, v, true) || derived_leaf(c, v, false))
            })
        {
            return Err("CONFIG_STATUS_END_RULE");
        }
    }
    Ok(())
}
