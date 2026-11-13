//! Explicit permissions for native status mutation and once-per-entity lethal guards.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StatusDisposition {
    Beneficial,
    Harmful,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusPolicy {
    pub disposition: StatusDisposition,
    pub dispellable: bool,
    pub transferable: bool,
    pub blocks: Vec<StatusDisposition>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeathGuard {
    pub retain_hp: u32,
    pub heal_max_hp_bps: u32,
    pub self_statuses: Vec<String>,
}

impl DeathGuard {
    fn valid(&self, defs: &[StatusDefinition]) -> bool {
        (1..=1000000).contains(&self.retain_hp)
            && self.heal_max_hp_bps <= 10000
            && self.self_statuses.len() <= 2
            && self.self_statuses.iter().collect::<BTreeSet<_>>().len() == self.self_statuses.len()
            && self.self_statuses.iter().all(|id| {
                defs.iter().any(|d| {
                    d.id == *id
                        && !d.is_periodic()
                        && !d.is_shield()
                        && !d.is_hostile_control()
                        && d.attribute != Some(crate::protocol::StatusAttribute::SpiritRecoveryLock)
                })
            })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusDeathGuard {
    pub cooldown_group: String,
    pub cooldown_ms: u64,
    pub guard: DeathGuard,
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    let defs = &c.pve.as_ref().unwrap().status_definitions;
    if r.status_policies.len() > 128 || r.death_guards.len() > 64 {
        return Err("CONFIG_STATUS_POLICY_BUDGET");
    }
    for (id, policy) in &r.status_policies {
        let Some(d) = defs.iter().find(|d| d.id == *id) else {
            return Err("CONFIG_STATUS_POLICY_REFERENCE");
        };
        if policy.blocks.len() > 2
            || policy.blocks.iter().collect::<BTreeSet<_>>().len() != policy.blocks.len()
            || policy.transferable
                && (!policy.dispellable
                    || !matches!(
                        d.mechanism,
                        StatusMechanism::TimedAttributeModifier
                            | StatusMechanism::TimedCompoundModifier
                            | StatusMechanism::AbsorptionShield
                            | StatusMechanism::PeriodicDamage
                            | StatusMechanism::PeriodicHeal
                            | StatusMechanism::TimedControl
                    ))
        {
            return Err("CONFIG_STATUS_TRANSFER_POLICY");
        }
    }
    for (profile, guard) in &r.death_guards {
        if !r.profiles.contains_key(profile) || !guard.valid(defs) {
            return Err("CONFIG_DEATH_GUARD_POLICY");
        }
    }
    let table = &r.status_death_guards;
    if !table.is_empty() && (c.protocol_version() != 6 || table.len() > 16) {
        return Err("CONFIG_STATUS_DEATH_GUARD");
    }
    let mut groups = BTreeMap::new();
    for (id, entry) in table {
        if entry.cooldown_group.is_empty()
            || entry.cooldown_group.len() > 64
            || !entry
                .cooldown_group
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
            || !(c.step_ms..=600000).contains(&entry.cooldown_ms)
            || groups
                .insert(&entry.cooldown_group, entry.cooldown_ms)
                .is_some_and(|previous| previous != entry.cooldown_ms)
            || !entry.guard.valid(defs)
            || entry
                .guard
                .self_statuses
                .iter()
                .any(|id| !super::status_ends::derived_leaf(c, id, true))
            || !r
                .status_policies
                .get(id)
                .is_some_and(|p| p.disposition == StatusDisposition::Beneficial && !p.transferable)
            || !defs.iter().any(|d| {
                d.id == *id
                    && d.source_scoped
                    && d.stack_counter.is_none()
                    && !d.is_hostile_control()
                    && matches!(
                        d.mechanism,
                        StatusMechanism::TimedAttributeModifier
                            | StatusMechanism::TimedCompoundModifier
                    )
                    && matches!(
                        d.duplicate_policy,
                        DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
                    )
            })
        {
            return Err("CONFIG_STATUS_DEATH_GUARD");
        }
    }
    Ok(())
}

pub(crate) fn blocked(
    statuses: &crate::status::Statuses,
    id: &str,
    target: &crate::protocol::StatusEntity,
    policies: &BTreeMap<String, StatusPolicy>,
    now: u64,
) -> bool {
    let Some(incoming) = policies.get(id).filter(|p| p.dispellable) else {
        return false;
    };
    statuses.entries().iter().any(|s| {
        s.target == *target
            && now < s.expires_at_ms
            && policies
                .get(&s.definition_id)
                .is_some_and(|p| p.blocks.contains(&incoming.disposition))
    })
}
