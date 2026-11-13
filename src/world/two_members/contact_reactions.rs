//! Capture existing identities, then consume once after the primary damage.
use super::*;
use crate::config::{ContactReactionWhen, ReactionAttacker};
#[derive(Default)]
pub(super) struct ContactReactionState {
    step: Option<u64>,
    used: usize,
}
impl World {
    pub(super) fn capture_contact_reactions(
        &self,
        provoker: &EntityRef,
        target: &EntityRef,
        c: &Config,
        now: u64,
    ) -> Vec<StatusView> {
        if self.triggers.executing || self.record_shields.executing || self.window_records.executing
        {
            return vec![];
        }
        let Some(r) = &c.combat_rules else {
            return vec![];
        };
        if r.contact_reactions.is_empty() {
            return vec![];
        }
        let source = status_entity(provoker);
        let target = status_entity(target);
        let mut statuses = self
            .statuses
            .entries()
            .iter()
            .filter(|s| {
                s.target == target
                    && now < s.expires_at_ms
                    && r.contact_reactions.get(&s.definition_id).is_some_and(|r| {
                        r.attacker == ReactionAttacker::AnyHostile || s.source == source
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        statuses.sort_by(|a, b| {
            a.expires_at_ms
                .cmp(&b.expires_at_ms)
                .then(a.applied_at_ms.cmp(&b.applied_at_ms))
                .then(a.instance_id.cmp(&b.instance_id))
        });
        statuses
    }
    pub(super) fn resolve_contact_reactions(
        &mut self,
        actors: (&EntityRef, &EntityRef),
        candidates: Vec<StatusView>,
        hp_damage: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if candidates.is_empty() {
            return;
        }
        let rules = c.combat_rules.as_ref().unwrap();
        if self.contact_reactions.step != Some(now) {
            self.contact_reactions.step = Some(now);
            self.contact_reactions.used = 0;
        }
        let mut used = 0;
        for status in candidates {
            if used >= rules.max_contact_reactions_per_hit
                || self.contact_reactions.used >= rules.max_contact_reactions_per_step
            {
                break;
            }
            let rule = &rules.contact_reactions[&status.definition_id];
            if rule.when == ContactReactionWhen::HpDamage && hp_damage == 0 {
                continue;
            }
            if !self.statuses.entries().iter().any(|s| {
                s.instance_id == status.instance_id
                    && s.source == status.source
                    && s.target == status.target
                    && s.definition_id == status.definition_id
                    && now < s.expires_at_ms
            }) {
                continue;
            }
            let (Some(source), Some(target)) = (
                self.entity_owner(&status.source),
                self.entity_owner(&status.target),
            ) else {
                continue;
            };
            if rule
                .source_requirement
                .as_ref()
                .is_some_and(|g| !self.meets_skill_requirement(source, g, c))
            {
                continue;
            }
            if !self.status_operation_eligible(source)
                || !self.hostile_contact(&self.owner_entity(source), actors.1)
                || self.periodic_outcome(&status.source, &status.target)
                    != PeriodicTickOutcome::Applied
                || self.entity_has_status(target, StatusAttribute::DamageImmunity, now)
            {
                continue;
            }
            if let Owner::Member(i) = target
                && self.member_damage_immune(i, c, now)
            {
                continue;
            }
            let raw = rule.amount(
                self.status_source_power(&self.statuses, source, &status.definition_id, c, now)
                    .unwrap_or_else(|| self.record_power(source, c, now)),
            );
            if raw == 0 {
                continue;
            }
            self.advance_attack_clocks(c, now);
            let before = self.statuses.entries().to_vec();
            let mut consumed = vec![];
            if !self.statuses.consume(&status.instance_id, 1, &mut consumed) {
                continue;
            }
            self.route_status_events(consumed, &before, events);
            self.reconcile_caps(c, now);
            used += 1;
            self.contact_reactions.used += 1;
            let prior = self.triggers.executing;
            self.triggers.executing = true;
            let result = self.resolve_damage_with_rule(
                combat::DamageIntent {
                    source: self.owner_entity(source),
                    target: self.owner_entity(target),
                    raw,
                    direct: false,
                    ignore_armor: rule.ignore_armor,
                },
                Some(&crate::config::AttackRule {
                    damage_kind: rule.damage_kind,
                    element: rule.element.clone(),
                    ignore_armor: rule.ignore_armor,
                    armor_penetration_cap_bps: Some(rule.armor_penetration_cap_bps),
                    can_crit: false,
                    ..Default::default()
                }),
                c,
                now,
                events,
            );
            self.triggers.executing = prior;
            for i in self.status_observers(&status.source, &status.target) {
                private(
                    events,
                    i,
                    Event::ContactReactionResolved {
                        instance_id: status.instance_id.clone(),
                        definition_id: status.definition_id.clone(),
                        source: status.source.clone(),
                        target: status.target.clone(),
                        provoker: status_entity(actors.0),
                        raw_damage: raw,
                        hp_damage: result.hp_damage,
                        absorbed: result.absorbed,
                    },
                );
            }
        }
    }
}
