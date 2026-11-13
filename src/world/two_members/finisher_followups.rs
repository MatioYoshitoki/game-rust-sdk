//! Synchronous result dispatch over captured identities, with existing bounded rules.
use super::finishers::FinishKillFact;
use super::triggers::TriggerInput;
use super::*;
use crate::config::{CombatFact, FinisherSelector};
impl World {
    pub(super) fn dispatch_finisher_kills(
        &mut self,
        facts: Vec<FinishKillFact>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if facts.is_empty() {
            return;
        }
        let r = c.combat_rules.as_ref().unwrap();
        let prior = self.triggers.executing;
        self.triggers.executing = true;
        for fact in facts {
            let (Some(owner), Some(other)) = (
                self.resolve_entity(&fact.source),
                self.resolve_entity(&fact.target),
            ) else {
                continue;
            };
            if !self.valid_status_entity(&status_entity(&fact.source)) {
                continue;
            }
            let profile = self.combat_profile_id(owner, r);
            let mut definitions = r
                .triggers
                .iter()
                .filter(|t| {
                    t.when == CombatFact::FinisherKill
                        && t.profiles.iter().any(|id| id == profile)
                        && FinisherSelector::leaf(&t.effect)
                        && t.finisher.as_ref().is_some_and(|s| {
                            s.trigger_id == fact.finisher_id
                                && s.matches(fact.mode)
                                && s.enhanced.is_none_or(|e| e == fact.enhanced)
                        })
                })
                .cloned()
                .collect::<Vec<_>>();
            definitions.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.id.cmp(&b.id)));
            for definition in definitions {
                if !self.valid_status_entity(&status_entity(&fact.source)) {
                    break;
                }
                self.run_combat_trigger(
                    &definition,
                    TriggerInput {
                        origin: fact.origin.clone(),
                        actors: (owner, other),
                        other: fact.target.clone(),
                        amount: u64::from(fact.hp_damage),
                        action: fact.action.clone(),
                        contact: None,
                    },
                    c,
                    now,
                    events,
                    &mut vec![],
                );
            }
        }
        self.triggers.executing = prior;
        self.reconcile_caps(c, now);
    }
}
