//! Only native member skill deadlines are edited, never work clocks or item debt.
use super::*;
impl World {
    pub(super) fn plan_cooldown_reduction(
        &self,
        owner: Owner,
        ids: &[String],
        bps: u32,
        fixed: u64,
        now: u64,
    ) -> Vec<SkillCooldownChange> {
        let Owner::Member(i) = owner else {
            return vec![];
        };
        ids.iter()
            .filter_map(|id| {
                let ready = *self.members[i].cooldowns.get(id)?;
                let remaining = ready.saturating_sub(now);
                let reduced = (u128::from(remaining) * u128::from(bps) / 10000 + u128::from(fixed))
                    .min(u128::from(remaining)) as u64;
                (reduced > 0).then(|| SkillCooldownChange {
                    skill_id: id.clone(),
                    previous_ready_at_ms: ready,
                    ready_at_ms: ready - reduced,
                    reduced_ms: reduced,
                })
            })
            .collect()
    }
    pub(super) fn execute_cooldown_reduction(
        &mut self,
        trigger_id: &str,
        owner: Owner,
        ids: &[String],
        amount: (u32, u64),
        now: u64,
        events: &mut Events,
    ) {
        let changes = self.plan_cooldown_reduction(owner, ids, amount.0, amount.1, now);
        let Owner::Member(i) = owner else {
            return;
        };
        if changes.is_empty() {
            return;
        }
        for change in &changes {
            self.members[i]
                .cooldowns
                .insert(change.skill_id.clone(), change.ready_at_ms);
        }
        let source = status_entity(&self.owner_entity(owner));
        for observer in self.status_observers(&source, &source) {
            private(
                events,
                observer,
                Event::SkillCooldownsReduced {
                    at_ms: now,
                    trigger_id: trigger_id.into(),
                    source: source.clone(),
                    changes: changes.clone(),
                },
            );
        }
    }
}
