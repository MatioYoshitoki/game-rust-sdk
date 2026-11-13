//! End records are consumed once and never persisted as future battle work.
use super::*;
#[derive(Default)]
pub(super) struct EndState {
    pub(super) executing: bool,
    step: Option<u64>,
    used: usize,
}
impl World {
    pub(super) fn take_status_end_budget(&mut self, c: &Config, now: u64) -> bool {
        if self.status_ends.step != Some(now) {
            self.status_ends.step = Some(now);
            self.status_ends.used = 0;
        }
        let maximum = c
            .combat_rules
            .as_ref()
            .unwrap()
            .max_status_end_releases_per_step;
        if self.status_ends.used >= maximum {
            return false;
        }
        self.status_ends.used += 1;
        true
    }

    pub(super) fn finish_status_ends(&mut self, c: &Config, now: u64, events: &mut Events) {
        let Some(r) = &c.combat_rules else { return };
        self.finish_retaliation_ends(c, now, events);
        if r.status_ends.is_empty() {
            return;
        }
        let mut suppressed = vec![0u32; self.members.len()];
        for (source, target, n) in self.statuses.take_end_overflow() {
            for i in self.status_observers(&source, &target) {
                suppressed[i] = suppressed[i].saturating_add(n);
            }
        }
        if self.status_ends.step != Some(now) {
            self.status_ends.step = Some(now);
            self.status_ends.used = 0;
        }
        let prior_guard = self.triggers.executing;
        self.status_ends.executing = true;
        self.triggers.executing = true;
        loop {
            let ended = self.statuses.take_status_ends();
            if ended.is_empty() {
                break;
            }
            for (status, reason) in ended {
                let Some(rule) = r.status_ends.get(&status.definition_id) else {
                    continue;
                };
                if !rule.on.contains(&reason) {
                    continue;
                }
                if !self.take_status_end_budget(c, now) {
                    for i in self.status_observers(&status.source, &status.target) {
                        suppressed[i] = suppressed[i].saturating_add(1);
                    }
                    continue;
                }
                let failure;
                if !self.valid_status_entity(&status.source)
                    || !self.valid_status_entity(&status.target)
                {
                    failure = Some(Error::InvalidTarget);
                } else if self.periodic_outcome(&status.source, &status.target)
                    != PeriodicTickOutcome::Applied
                {
                    failure = Some(Error::InvalidState);
                } else {
                    let owner = self.entity_owner(&status.source).unwrap();
                    let target = Self::status_reference(&status.target);
                    let max = self.entity_max_hp(&target, c, now).unwrap();
                    let start = events.len();
                    self.apply_owned_statuses(
                        owner,
                        (target, max),
                        &rule.target_statuses,
                        c,
                        now,
                        events,
                    );
                    failure = events[start..].iter().find_map(|(_, e)| {
                        if let EventV4::Private {
                            event:
                                Event::StatusRejected {
                                    source,
                                    target,
                                    reason,
                                    ..
                                },
                        } = e
                        {
                            if *source == status.source && *target == status.target {
                                Some(*reason)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    });
                }
                let active_definition_ids = self
                    .statuses
                    .entries()
                    .iter()
                    .filter(|s| {
                        s.source == status.source
                            && s.target == status.target
                            && now < s.expires_at_ms
                            && rule.target_statuses.contains(&s.definition_id)
                    })
                    .map(|s| s.definition_id.clone())
                    .collect();
                let event = Event::StatusEndResolved {
                    instance_id: status.instance_id.clone(),
                    definition_id: status.definition_id.clone(),
                    source: status.source.clone(),
                    target: status.target.clone(),
                    reason,
                    active_definition_ids,
                    failure,
                };
                for i in self.status_observers(&status.source, &status.target) {
                    private(events, i, event.clone());
                }
            }
        }
        self.triggers.executing = prior_guard;
        self.status_ends.executing = false;
        for (i, count) in suppressed.into_iter().enumerate() {
            if count > 0 {
                private(events, i, Event::StatusEndsSuppressed { at_ms: now, count });
            }
        }
    }
}
