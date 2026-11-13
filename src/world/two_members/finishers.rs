//! A finisher is a finite damage intent, never an assignment to target HP.
use super::*;
use crate::config::{FinishPolicy, FinishRule, ThresholdComparison};
pub(super) struct FinishKillFact {
    pub(super) origin: Option<crate::origin::ActionOrigin>,
    pub(super) finisher_id: String,
    pub(super) source: EntityRef,
    pub(super) target: EntityRef,
    pub(super) mode: FinisherMode,
    pub(super) hp_damage: u32,
    pub(super) enhanced: bool,
    pub(super) action: Option<super::triggers::ActionKey>,
}
pub(super) struct FinishPlan {
    pub(super) enhanced: bool,
    pub(super) mode: FinisherMode,
    pub(super) raw: u32,
}
impl World {
    pub(super) fn plan_finish_other(
        &self,
        statuses: &Statuses,
        actors: (Owner, Owner),
        rule: &FinishRule,
        c: &Config,
        now: u64,
        contact: Option<&super::contact_status::ContactAugment>,
    ) -> Result<FinishPlan> {
        let source = self.owner_entity(actors.0);
        let target = self.owner_entity(actors.1);
        let target_status = status_entity(&target);
        if !self.status_operation_eligible(actors.0)
            || !self.status_operation_eligible(actors.1)
            || self.relation(&source, &target) != Some(combat::Relation::Hostile)
        {
            return Err(Error::InvalidTarget);
        }
        if statuses.entries().iter().any(|s| {
            s.target == target_status
                && now < s.expires_at_ms
                && s.has_attribute(StatusAttribute::DamageImmunity)
        }) {
            return Err(Error::Immune);
        }
        let base = self.base_hp(actors.1, c);
        let max = statuses
            .attribute(
                &target_status,
                base,
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
                StatusAttribute::MaxHp,
            )
            .max(1);
        let hp = self.current_hp(actors.1).min(max);
        let left = u128::from(hp) * 10000;
        let mut threshold = u128::from(rule.hp_threshold_bps);
        let mut enhanced = false;
        if let Some(b) = &rule.consumed_status_bonus
            && let Some(contact) = contact.filter(|s| s.definition_id == b.definition_id)
        {
            if let Some(failure) = contact.failure {
                return Err(failure);
            }
            threshold = (threshold
                + u128::from(contact.stacks) * u128::from(b.hp_threshold_bps_per_stack))
            .min(10000);
            enhanced = contact.stacks > 0;
        }

        let right = u128::from(max) * threshold;
        if match rule.comparison {
            ThresholdComparison::Below => left >= right,
            ThresholdComparison::AtMost => left > right,
        } {
            return Err(Error::InvalidState);
        }
        let r = c.combat_rules.as_ref().unwrap();
        let profile = self.combat_profile_id(actors.1, r);
        let policy = rule
            .profile_overrides
            .get(profile)
            .unwrap_or(&rule.default_policy);
        let (mode, raw) = match policy {
            FinishPolicy::Execute {} => {
                let shields: u64 = statuses
                    .entries()
                    .iter()
                    .filter(|s| s.target == target_status && now < s.expires_at_ms)
                    .filter_map(|s| s.shield.as_ref().map(|s| u64::from(s.remaining)))
                    .sum();
                let amount = u64::from(hp) + shields;
                if amount > u64::from(u32::MAX) {
                    return Err(Error::Capacity);
                }
                (FinisherMode::Execute, amount as u32)
            }
            FinishPolicy::PowerDamage { .. } => (
                FinisherMode::PowerDamage,
                policy
                    .amount(rule.source_skill_id.as_ref().map_or_else(
                        || self.record_power_with(statuses, actors.0, c, now),
                        |id| self.selected_power_with(statuses, actors.0, id, c, now),
                    ))
                    .unwrap(),
            ),
        };
        if raw == 0 {
            return Err(Error::InvalidState);
        }
        Ok(FinishPlan {
            mode,
            raw,
            enhanced,
        })
    }
    pub(super) fn execute_finisher(
        &mut self,
        trigger_id: &str,
        actors: (Owner, Owner),
        rule: (&FinishRule, Option<&super::contact_status::ContactAugment>),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<FinishKillFact> {
        let (rule, contact) = rule;
        self.reconcile_caps(c, now);
        let Ok(plan) = self.plan_finish_other(&self.statuses, actors, rule, c, now, contact) else {
            return None;
        };
        let source = self.owner_entity(actors.0);
        let target = self.owner_entity(actors.1);
        let prior = self.triggers.executing;
        self.triggers.executing = true;
        let result = self.resolve_damage_with_rule(
            combat::DamageIntent {
                source: source.clone(),
                target: target.clone(),
                raw: plan.raw,
                direct: false,
                ignore_armor: true,
            },
            Some(&crate::config::AttackRule {
                damage_kind: crate::config::DamageKind::TrueDamage,
                can_crit: false,
                ..Default::default()
            }),
            c,
            now,
            events,
        );
        self.triggers.executing = prior;
        let lethal = self.current_hp(actors.1) == 0;
        for i in self.status_observers(&status_entity(&source), &status_entity(&target)) {
            private(
                events,
                i,
                Event::FinisherResolved {
                    trigger_id: trigger_id.into(),
                    source: status_entity(&source),
                    target: status_entity(&target),
                    mode: plan.mode,
                    raw_damage: plan.raw,
                    hp_damage: result.hp_damage,
                    absorbed: result.absorbed,
                    lethal,
                },
            );
        }
        lethal.then(|| FinishKillFact {
            origin: self.triggers.origin.clone(),
            finisher_id: trigger_id.into(),
            source,
            target,
            mode: plan.mode,
            hp_damage: result.hp_damage,
            enhanced: plan.enhanced,
            action: None,
        })
    }
}
