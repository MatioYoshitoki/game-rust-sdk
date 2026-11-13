//! A local, captured threshold fact follows a legal mutation, not an external timer.
use super::*;
use crate::config::{CombatFact, TriggerSubject};
pub(super) struct StackChangeFact {
    pub(super) instance_id: String,
    pub(super) definition_id: String,
    pub(super) source: StatusEntity,
    pub(super) target: StatusEntity,
    pub(super) stacks: u32,
    pub(super) previous: u32,
    pub(super) origin: Option<crate::origin::ActionOrigin>,
    pub(super) action: Option<super::triggers::ActionKey>,
}
pub(super) enum TriggerOutcome {
    Finisher(super::finishers::FinishKillFact),
    Stacks(StackChangeFact),
}
impl World {
    pub(super) fn counter_target(actors: (Owner, Owner), subject: TriggerSubject) -> Owner {
        match subject {
            TriggerSubject::Owner => actors.0,
            TriggerSubject::Other => actors.1,
        }
    }
    pub(super) fn plan_counter_adjustment(
        &self,
        statuses: &Statuses,
        actors: (Owner, Owner),
        subject: TriggerSubject,
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<String> {
        self.owned_status_instance(statuses, actors, (subject, id), c, now)
            .and_then(|s| {
                if s.stack_counter == Some(true) {
                    Ok(s.instance_id.clone())
                } else {
                    Err(Error::InvalidState)
                }
            })
    }
    pub(super) fn add_status_stacks(
        &mut self,
        actors: (Owner, Owner),
        operation: (TriggerSubject, &str, u32, bool),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<StackChangeFact> {
        let (subject, id, amount, refresh) = operation;
        let instance = self
            .plan_counter_adjustment(&self.statuses, actors, subject, id, c, now)
            .ok()?;
        let target = status_entity(&self.owner_entity(Self::counter_target(actors, subject)));
        let duration = refresh.then(|| {
            let d = c
                .pve
                .as_ref()
                .unwrap()
                .status_definitions
                .iter()
                .find(|d| d.id == id)
                .unwrap();
            let reduction = if d.is_harmful() {
                self.statuses.attribute(
                    &target,
                    10000,
                    now,
                    c.pve.as_ref().unwrap().status_rules.as_ref(),
                    StatusAttribute::HarmfulDurationReduction,
                ) - 10000
            } else {
                0
            };
            (u128::from(d.duration_ms) * u128::from(10000 - reduction) / 10000).max(1) as u64
        });
        self.advance_attack_clocks(c, now);
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        let (status, previous) = self
            .statuses
            .adjust_counter(&instance, amount, now, duration, &mut raw)?;
        self.route_status_events(raw, &before, events);
        self.reconcile_caps(c, now);
        (status.stacks.unwrap() > previous).then_some(StackChangeFact {
            stacks: status.stacks.unwrap(),
            instance_id: status.instance_id,
            definition_id: status.definition_id,
            source: status.source,
            target: status.target,
            previous,
            origin: None,
            action: None,
        })
    }
    pub(super) fn dispatch_trigger_outcomes(
        &mut self,
        outcomes: Vec<TriggerOutcome>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        for outcome in outcomes {
            match outcome {
                TriggerOutcome::Finisher(f) => {
                    self.dispatch_finisher_kills(vec![f], c, now, events)
                }
                TriggerOutcome::Stacks(f) => self.dispatch_stack_change(f, c, now, events),
            }
        }
    }
    fn dispatch_stack_change(
        &mut self,
        fact: StackChangeFact,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let (Some(owner), Some(holder)) = (
            self.entity_owner(&fact.source),
            self.entity_owner(&fact.target),
        ) else {
            return;
        };
        if !self.status_operation_eligible(owner)
            || !self.status_operation_eligible(holder)
            || !self.statuses.entries().iter().any(|s| {
                s.instance_id == fact.instance_id
                    && s.source == fact.source
                    && s.target == fact.target
                    && s.stack_counter == Some(true)
            })
        {
            return;
        }
        let r = c.combat_rules.as_ref().unwrap();
        let profile = self.combat_profile_id(owner, r);
        let mut rules = r
            .triggers
            .iter()
            .filter(|t| {
                t.when == CombatFact::StatusStacksReached
                    && t.profiles.iter().any(|id| id == profile)
                    && crate::config::stack_threshold_leaf(c, &t.effect)
                    && t.stack_change.as_ref().is_some_and(|s| {
                        s.definition_id == fact.definition_id
                            && fact.previous < s.at_least
                            && fact.stacks >= s.at_least
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        rules.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.id.cmp(&b.id)));
        self.trigger_step(now);
        let prior = self.triggers.executing;
        self.triggers.executing = true;
        let prior_leaf = std::mem::replace(&mut self.stack_leaves, true);
        let prior_scope = self.statuses.end_cause_scope(true);
        for rule in rules {
            self.run_combat_trigger(
                &rule,
                super::triggers::TriggerInput {
                    origin: fact.origin.clone(),
                    actors: (owner, holder),
                    other: Self::status_reference(&fact.target),
                    amount: u64::from(fact.stacks - fact.previous),
                    action: fact.action.clone(),
                    contact: None,
                },
                c,
                now,
                events,
                &mut vec![],
            );
        }
        self.statuses.end_cause_scope(prior_scope);
        self.stack_leaves = prior_leaf;
        self.triggers.executing = prior;
        self.reconcile_caps(c, now);
    }
}
