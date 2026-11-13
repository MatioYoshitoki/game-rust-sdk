//! Finite grants reuse the existing status definitions, controls and dispositions.
use super::*;
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    for t in &r.triggers {
        if t.when == CombatFact::UnitSkillCommitted
            && (t.include_periodic
                || !matches!(
                    t.effect,
                    TriggerEffect::ApplyStatusBundle { .. }
                        | TriggerEffect::RefreshStatusDuration { .. }
                        | TriggerEffect::ApplySelfStatus { .. }
                        | TriggerEffect::ApplyOtherStatus { .. }
                        | TriggerEffect::RestoreSpirit { .. }
                        | TriggerEffect::DispelStatuses { .. }
                        | TriggerEffect::TransferStatuses { .. }
                )
                || t.counting == TriggerCounting::ActionTotal)
        {
            return Err("CONFIG_UNIT_COMMIT_FACT");
        }
        if let TriggerEffect::RefreshStatusDuration {
            status_id,
            duration_ms,
            ..
        } = &t.effect
        {
            let def = c
                .pve
                .as_ref()
                .unwrap()
                .status_definitions
                .iter()
                .find(|d| d.id == *status_id);
            if matches!(
                t.when,
                CombatFact::StatusStacksReached | CombatFact::FinisherKill
            ) || t.counting == TriggerCounting::ActionTotal
                || duration_ms
                    .is_some_and(|ms| !(c.step_ms..=c.terminal_ms.min(3600000)).contains(&ms))
                || !r.status_policies.contains_key(status_id)
                || def.is_none_or(|d| {
                    !d.source_scoped
                        || !matches!(
                            d.mechanism,
                            StatusMechanism::TimedAttributeModifier
                                | StatusMechanism::TimedCompoundModifier
                        )
                        || !d.flags.is_empty()
                        || d.attribute
                            .is_some_and(|a| a.is_control_flag() || a.is_immunity_flag())
                })
            {
                return Err("CONFIG_STATUS_DURATION_REFRESH");
            }
        }
        if let TriggerEffect::ApplyStatusBundle {
            owner_statuses,
            other_statuses,
        } = &t.effect
        {
            if !matches!(
                t.when,
                CombatFact::UnitSkillCommitted
                    | CombatFact::DirectHit
                    | CombatFact::BasicHit
                    | CombatFact::DamageDealt
                    | CombatFact::DamageTaken
                    | CombatFact::ShieldAbsorbed
            ) || t.include_periodic
                || t.counting == TriggerCounting::ActionTotal
                || owner_statuses.len() > 2
                || other_statuses.len() > 2
                || owner_statuses.is_empty() && other_statuses.is_empty()
            {
                return Err("CONFIG_STATUS_BUNDLE_POLICY");
            }
            for (ids, harmful) in [(owner_statuses, false), (other_statuses, true)] {
                for (i, id) in ids.iter().enumerate() {
                    if ids[..i].contains(id)
                        || !c.pve.as_ref().unwrap().status_definitions.iter().any(|d| {
                            d.id == *id
                                && d.source_scoped
                                && matches!(
                                    d.mechanism,
                                    StatusMechanism::TimedAttributeModifier
                                        | StatusMechanism::TimedCompoundModifier
                                        | StatusMechanism::TimedControl
                                )
                                && if harmful {
                                    d.is_harmful()
                                        && !d.attribute.is_some_and(|a| a.is_immunity_flag())
                                        && !d.flags.iter().any(|a| a.is_immunity_flag())
                                } else {
                                    !d.is_harmful() && !d.is_hostile_control() && d.attribute!=Some(crate::protocol::StatusAttribute::SpiritRecoveryLock)
                                }
                        })
                        || r.status_policies.get(id).is_none_or(|p| {
                            (p.disposition == super::StatusDisposition::Harmful) != harmful
                        })
                    {
                        return Err("CONFIG_STATUS_BUNDLE_REFERENCE");
                    }
                }
            }
        }
    }
    Ok(())
}
