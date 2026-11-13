//! Action journals end with native plans, never with a client's event receipt.
use super::triggers::{ActionKey, EntityKey, TriggerInput, TriggerKey, entity_key};
use super::*;
use crate::config::{CombatTrigger, TriggerEffect, TriggerScope};
use std::collections::BTreeSet;

pub(super) const MAX_TOTAL_AMOUNT: u64 = crate::config::MAX_COMBAT_RECORD_AMOUNT;
pub(super) fn checked_total(a: u64, b: u64) -> Option<u64> {
    a.checked_add(b).filter(|n| *n <= MAX_TOTAL_AMOUNT)
}
struct Total {
    origin: Option<crate::origin::ActionOrigin>,
    definition: CombatTrigger,
    owner: EntityRef,
    other: EntityRef,
    amount: u64,
    contacts: u32,
}
struct LifeStealTotal {
    origin: crate::origin::ActionOrigin,
    amount: u64,
    base_bps: u32,
}
#[derive(Default)]
pub(super) struct Totals {
    pending: BTreeMap<(ActionKey, TriggerKey), Total>,
    life_steal: BTreeMap<ActionKey, LifeStealTotal>,
    failed: BTreeSet<ActionKey>,
    paused: bool,
    settling: BTreeSet<TriggerKey>,
    canceled: BTreeSet<(EntityKey, ActionKey)>,
}
impl Totals {
    pub(super) fn action_open(&self, source: &EntityRef, id: &str) -> bool {
        let source = entity_key(source);
        let matches = |a: &ActionKey| a.source == source && a.id == id;
        self.pending.keys().any(|(a, _)| matches(a))
            || self.life_steal.keys().any(matches)
            || self.failed.iter().any(matches)
            || self.canceled.iter().any(|(_, a)| matches(a))
    }
    pub(super) fn clear_owner(&mut self, owner: &EntityKey) {
        self.pending.retain(|(_, key), _| key.0 != *owner);
        self.life_steal.retain(|a, _| a.source != *owner);
        self.canceled.retain(|(key, _)| key != owner);
    }
    fn accepts(&self, action: &ActionKey, owner: &EntityKey) -> bool {
        !self.paused
            && !self.failed.contains(action)
            && !self.canceled.contains(&(owner.clone(), action.clone()))
    }
    fn len(&self) -> usize {
        self.pending.len() + self.life_steal.len()
    }
    fn pause(&mut self) {
        self.paused = true;
        self.pending.clear();
        self.life_steal.clear();
        self.failed.clear();
        self.canceled.clear();
    }
    fn invalidate(&mut self, action: ActionKey) {
        self.pending.retain(|(a, _), _| *a != action);
        self.life_steal.remove(&action);
        if self.failed.len() < 256 {
            self.failed.insert(action);
        } else {
            self.pause();
        }
    }
    pub(super) fn held_counters(&self) -> BTreeSet<TriggerKey> {
        self.pending
            .keys()
            .map(|(_, key)| key.clone())
            .chain(self.settling.iter().cloned())
            .collect()
    }
    #[cfg(all(test, match_sdk_test_data))]
    pub(super) fn pending_len(&self) -> usize {
        self.len()
    }
}
impl World {
    pub(super) fn cancel_owner_totals(&mut self, source: &EntityRef) {
        let owner = entity_key(source);
        let mut actions = self
            .active_trigger_actions()
            .into_iter()
            .filter(|a| a.source == owner)
            .collect::<BTreeSet<_>>();
        actions.extend(
            self.triggers
                .totals
                .pending
                .keys()
                .filter(|(_, key)| key.0 == owner)
                .map(|(a, _)| a.clone()),
        );
        let totals = &mut self.triggers.totals;
        actions.extend(
            totals
                .life_steal
                .keys()
                .filter(|a| a.source == owner)
                .cloned(),
        );
        totals.pending.retain(|(_, key), _| key.0 != owner);
        totals.life_steal.retain(|a, _| a.source != owner);
        for action in actions {
            if totals.canceled.len() >= 256 {
                totals.pause();
                break;
            }
            totals.canceled.insert((owner.clone(), action));
        }
    }
    pub(super) fn collect_action_total(
        &mut self,
        definition: CombatTrigger,
        input: TriggerInput,
        origin: &EntityRef,
        c: &Config,
        events: &mut Events,
    ) {
        if input.amount == 0 {
            return;
        }
        let owner = self.owner_entity(input.actors.0);
        let action = input
            .action
            .unwrap_or_else(|| ActionKey::new(origin, &self.next_id("total-contact")));
        let owner_key = entity_key(&owner);
        let totals = &mut self.triggers.totals;
        if !totals.accepts(&action, &owner_key) {
            return;
        }
        let counter = (
            owner_key,
            definition.id.clone(),
            (definition.counter_scope == TriggerScope::Target).then(|| entity_key(&input.other)),
        );
        let key = (action.clone(), counter);
        let failure = if checked_total(0, input.amount).is_none() {
            true
        } else if let Some(total) = totals.pending.get_mut(&key) {
            if total.origin != input.origin {
                true
            } else if let (Some(amount), Some(contacts)) = (
                checked_total(total.amount, input.amount),
                total.contacts.checked_add(1),
            ) {
                total.amount = amount;
                total.contacts = contacts;
                false
            } else {
                true
            }
        } else if totals.len() < c.combat_rules.as_ref().unwrap().max_pending_action_totals {
            totals.pending.insert(
                key,
                Total {
                    origin: input.origin.clone(),
                    definition: definition.clone(),
                    owner: owner.clone(),
                    other: input.other,
                    amount: input.amount,
                    contacts: 1,
                },
            );
            false
        } else {
            true
        };
        if failure {
            totals.invalidate(action);
            self.suppress_trigger(&owner, &definition.id, Error::Capacity, events);
        }
    }
    pub(super) fn collect_action_life_steal(
        &mut self,
        action: ActionKey,
        origin: &crate::origin::ActionOrigin,
        hp_damage: u32,
        base_bps: u32,
        c: &Config,
    ) {
        let totals = &mut self.triggers.totals;
        if hp_damage == 0 || !totals.accepts(&action, &action.source) {
            return;
        }
        if let Some(total) = totals.life_steal.get_mut(&action) {
            if total.origin == *origin
                && total.base_bps == base_bps
                && let Some(amount) = checked_total(total.amount, u64::from(hp_damage))
            {
                total.amount = amount;
                return;
            }
        } else if totals.len() < c.combat_rules.as_ref().unwrap().max_pending_action_totals {
            totals.life_steal.insert(
                action,
                LifeStealTotal {
                    origin: origin.clone(),
                    amount: u64::from(hp_damage),
                    base_bps,
                },
            );
            return;
        }
        totals.invalidate(action);
    }
    fn finish_action_life_steal(
        &mut self,
        action: ActionKey,
        total: LifeStealTotal,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let source = action.source_ref();
        if let Some(Owner::Member(i)) = self.resolve_entity(&source)
            && self.members[i].eligible()
        {
            let bps = self.life_steal_rate(&source, total.base_bps, true, c, now);
            self.ratio_self_heal(
                &source,
                total.amount,
                (bps, HealKind::LifeSteal),
                c,
                now,
                events,
            );
        }
    }
    pub(super) fn finish_trigger_actions(&mut self, c: &Config, now: u64, events: &mut Events) {
        let totals = &self.triggers.totals;
        if totals.len() == 0
            && totals.failed.is_empty()
            && totals.canceled.is_empty()
            && !totals.paused
            && !self.members.iter().any(|m| m.basic_delivery.is_some())
        {
            return;
        }
        let active = self.active_trigger_actions();
        self.finish_basic_deliveries(c, now, &active, events);
        self.triggers.totals.failed.retain(|a| active.contains(a));
        self.triggers
            .totals
            .canceled
            .retain(|(_, a)| active.contains(a));
        if self.triggers.totals.paused {
            if active.is_empty() {
                self.triggers.totals.paused = false;
            }
            return;
        }
        let keys = self
            .triggers
            .totals
            .pending
            .keys()
            .filter(|(a, _)| !active.contains(a))
            .cloned()
            .collect::<Vec<_>>();
        self.triggers.totals.settling = keys.iter().map(|(_, key)| key.clone()).collect();
        let mut completed = keys
            .into_iter()
            .map(|key| {
                let total = self.triggers.totals.pending.remove(&key).unwrap();
                (key, total)
            })
            .collect::<Vec<_>>();
        let completed_life_steal = self
            .triggers
            .totals
            .life_steal
            .extract_if(.., |action, _| !active.contains(action))
            .collect::<Vec<_>>();
        completed.sort_by(|a, b| {
            a.0.0
                .cmp(&b.0.0)
                .then(a.0.1.0.cmp(&b.0.1.0))
                .then(a.1.definition.priority.cmp(&b.1.definition.priority))
                .then(a.1.definition.id.cmp(&b.1.definition.id))
                .then(a.0.1.2.cmp(&b.0.1.2))
        });
        self.trigger_step(now);
        self.triggers.executing = true;
        // Both sequences are in action order, so a heal precedes its own action effects.
        let mut life_steal = completed_life_steal.into_iter().peekable();
        for ((action, _), total) in completed {
            while let Some((key, heal)) = life_steal.next_if(|(key, _)| key <= &action) {
                self.finish_action_life_steal(key, heal, c, now, events);
            }
            let Some(owner) = self.resolve_entity(&total.owner) else {
                continue;
            };
            if !self.status_operation_eligible(owner) {
                continue;
            }
            // Total-mode effects/conditions are self-only. The representative
            // identity remains factual even after death or generation change.
            let mut outcomes = vec![];
            let count_stacks = matches!(
                total.definition.effect,
                TriggerEffect::AddStatusStacks { .. }
            );
            let fired = self.run_combat_trigger(
                &total.definition,
                TriggerInput {
                    origin: total.origin.clone(),
                    actors: (owner, owner),
                    other: total.other.clone(),
                    amount: total.amount,
                    action: count_stacks.then(|| action.clone()),
                    contact: None,
                },
                c,
                now,
                events,
                &mut outcomes,
            );
            for i in
                self.status_observers(&status_entity(&total.owner), &status_entity(&total.owner))
            {
                private(
                    events,
                    i,
                    Event::CombatActionTotal {
                        action_id: action.id.clone(),
                        action_source: status_entity(&action.source_ref()),
                        trigger_id: total.definition.id.clone(),
                        source: status_entity(&total.owner),
                        other: status_entity(&total.other),
                        total_amount: total.amount,
                        contacts: total.contacts,
                        fired,
                    },
                );
            }
            self.dispatch_trigger_outcomes(outcomes, c, now, events);
        }
        for (key, heal) in life_steal {
            self.finish_action_life_steal(key, heal, c, now, events);
        }
        self.triggers.executing = false;
        self.triggers.totals.settling.clear();
        self.prune_trigger_targets();
        self.reconcile_caps(c, now);
    }
}
