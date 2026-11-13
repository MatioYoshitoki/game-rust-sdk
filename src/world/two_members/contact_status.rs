//! Native contact consumption with optional, bounded snapshots for one action.
use super::triggers::{ActionKey, EntityKey};
use super::*;
use crate::config::ContactSampling;
#[derive(Clone, Debug)]
pub(super) struct ContactAugment {
    pub(super) definition_id: String,
    pub(super) stacks: u32,
    pub(super) failure: Option<Error>,
}
pub(super) struct ContactModification {
    pub(super) raw: u32,
    pub(super) rule: crate::config::AttackRule,
    pub(super) consumed: u32,
    pub(super) augment: Option<ContactAugment>,
}
#[derive(Default)]
pub(super) struct ContactActions {
    pub(super) entries: BTreeMap<(ActionKey, String), u32>,
    pub(super) paused: bool,
}
impl ContactActions {
    pub(super) fn empty(&self) -> bool {
        self.entries.is_empty() && !self.paused
    }
    pub(super) fn clear(&mut self, owner: &EntityKey) {
        self.entries.retain(|(a, _), _| a.source != *owner);
    }
    pub(super) fn prune(&mut self, active: &std::collections::BTreeSet<ActionKey>) {
        self.entries.retain(|(a, _), _| active.contains(a));
        if active.is_empty() {
            self.paused = false;
        }
    }
}
impl World {
    pub(super) fn contact_status_modifier(
        &mut self,
        actors: (Owner, Owner, Option<&str>),
        raw: u32,
        attack: &crate::config::AttackRule,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<ContactModification> {
        if c.combat_rules.is_none()
            || self.triggers.executing
            || self.record_shields.executing
            || self.window_records.executing
        {
            return None;
        }
        let rule = attack.contact_status.as_ref()?;
        let source_ref = self.owner_entity(actors.0);
        let source = status_entity(&source_ref);
        let target = status_entity(&self.owner_entity(match rule.subject {
            crate::config::TriggerSubject::Owner => actors.0,
            crate::config::TriggerSubject::Other => actors.1,
        }));
        let action_mode = rule.sampling == ContactSampling::Action;
        let mut failure = None;
        let key = actors
            .2
            .map(|id| (ActionKey::new(&source_ref, id), rule.definition_id.clone()));
        let cached = if action_mode {
            key.as_ref()
                .and_then(|k| self.triggers.contacts.entries.get(k))
                .copied()
        } else {
            None
        };
        if action_mode && cached.is_none() {
            if key.is_none() {
                failure = Some(Error::InvalidState);
            } else if self.triggers.contacts.paused
                || self.triggers.contacts.entries.len()
                    >= c.combat_rules
                        .as_ref()
                        .unwrap()
                        .max_contact_action_snapshots
            {
                self.triggers.contacts.paused = true;
                failure = Some(Error::Capacity);
            }
        }
        let mut consumed = 0;
        let count = if let Some(count) = cached {
            count
        } else if failure.is_some() {
            0
        } else {
            let status = self.statuses.entries().iter().find(|s| {
                s.source == source
                    && s.target == target
                    && s.definition_id == rule.definition_id
                    && now < s.expires_at_ms
                    && s.stacks.unwrap_or(1) >= rule.at_least
            });
            let count = if let Some(s) = status {
                let id = s.instance_id.clone();
                let count = s.stacks.unwrap_or(1).min(rule.consume_max);
                self.advance_attack_clocks(c, now);
                let before = self.statuses.entries().to_vec();
                let mut events_raw = vec![];
                if !self.statuses.consume(&id, count, &mut events_raw) {
                    return None;
                }
                self.route_status_events(events_raw, &before, events);
                self.reconcile_caps(c, now);
                consumed = count;
                count
            } else if action_mode {
                0
            } else {
                return None;
            };
            if action_mode {
                self.triggers.contacts.entries.insert(key.unwrap(), count);
            }
            count
        };
        let bonus = u128::from(count) * u128::from(rule.damage_bonus_bps_per_stack);
        let raw = (u128::from(raw) * (10000 + bonus) / 10000).min(u128::from(u32::MAX)) as u32;
        let mut modified = attack.clone();
        modified.armor_penetration_bps = modified
            .armor_penetration_bps
            .saturating_add(count.saturating_mul(rule.armor_penetration_bps_per_stack));
        Some(ContactModification {
            raw,
            rule: modified,
            consumed,
            augment: action_mode.then(|| ContactAugment {
                definition_id: rule.definition_id.clone(),
                stacks: count,
                failure,
            }),
        })
    }
}
