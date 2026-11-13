//! Retaliate against the actual damage source using a live self-status.
use super::*;
use crate::config::{StatusDisposition, TriggerEffect, TriggerSubject};

impl World {
    pub(super) fn retaliate(
        &mut self,
        holder: Owner,
        attacker: Owner,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let Some(r) = &c.combat_rules else { return };
        if r.status_retaliations.is_empty()
            || !matches!(holder, Owner::Member(_))
            || !self.status_operation_eligible(holder)
            || !self.status_operation_eligible(attacker)
        {
            return;
        }
        let source = status_entity(&self.owner_entity(holder));
        let target = self.owner_entity(attacker);
        let target_key = status_entity(&target);
        let active: Vec<_> = self
            .statuses
            .entries()
            .iter()
            .filter(|s| s.source == source && s.target == source && now < s.expires_at_ms)
            .filter_map(|s| {
                let rule = r.status_retaliations.get(&s.definition_id)?;
                (!self.statuses.is_derived(&s.instance_id)).then(|| (s.instance_id.clone(), rule))
            })
            .collect();
        for (id, rule) in active {
            if !self
                .statuses
                .retaliation_ready(&id, &target_key, rule.max_sources, now)
                || !self.roll(f64::from(rule.chance_bps) / 10000.)
            {
                continue;
            }
            let start = events.len();
            let defs = Self::definitions(c, std::slice::from_ref(&rule.reply_status_id));
            self.apply_owned_status_definitions(holder, target.clone(), &defs, c, now, events);
            if !events[start..].iter().any(|(_, event)| {
                matches!(event, EventV4::Private {
                    event: Event::StatusApplied { status } | Event::StatusRefreshed { status }
                } if status.source == source && status.target == target_key)
            }) {
                continue;
            }
            self.statuses.record_retaliation(
                id,
                target_key.clone(),
                now.saturating_add(rule.source_cooldown_ms),
            );
            if rule.dispel_beneficial > 0 {
                self.execute_status_operation(
                    (holder, attacker),
                    &TriggerEffect::DispelStatuses {
                        subject: TriggerSubject::Other,
                        disposition: StatusDisposition::Beneficial,
                        max_count: rule.dispel_beneficial,
                    },
                    c,
                    now,
                    events,
                );
            }
        }
    }

    pub(super) fn finish_retaliation_ends(&mut self, c: &Config, now: u64, events: &mut Events) {
        let rules = &c.combat_rules.as_ref().unwrap().status_retaliations;
        if rules.is_empty() {
            return;
        }
        for (carrier, attackers) in self.statuses.take_retaliation_expiries() {
            let Some(id) = rules
                .get(&carrier.definition_id)
                .and_then(|rule| rule.on_expired_status_id.as_ref())
            else {
                continue;
            };
            let Some(holder @ Owner::Member(i)) = self.entity_owner(&carrier.source) else {
                continue;
            };
            if !self.status_operation_eligible(holder) {
                continue;
            }
            let source = self.owner_entity(holder);
            let defs = Self::definitions(c, std::slice::from_ref(id));
            let mut suppressed = 0;
            for attacker in attackers {
                let target = Self::status_reference(&attacker);
                if self.hostile_contact(&source, &target) {
                    if self.take_status_end_budget(c, now) {
                        self.apply_owned_status_definitions(holder, target, &defs, c, now, events);
                    } else {
                        suppressed += 1;
                    }
                }
            }
            if suppressed > 0 {
                private(
                    events,
                    i,
                    Event::StatusEndsSuppressed {
                        at_ms: now,
                        count: suppressed,
                    },
                );
            }
        }
    }
}
