//! Status removal and HOT ticks own the schedule; these hooks only select dispels.
use super::status_operations::StatusSelection;
use super::*;
use crate::config::{CleanseAction, CleanseScope};
#[derive(Default)]
pub(super) struct CleanseState {
    executing: bool,
    step: Option<u64>,
    used: usize,
}
impl World {
    pub(super) fn finish_status_cleanses(&mut self, c: &Config, now: u64, events: &mut Events) {
        let mut ended = self.statuses.take_cleanse_ends();
        ended.sort_by(|(a, _), (b, _)| {
            a.expires_at_ms
                .cmp(&b.expires_at_ms)
                .then(a.applied_at_ms.cmp(&b.applied_at_ms))
                .then(a.instance_id.cmp(&b.instance_id))
        });
        let Some(r) = &c.combat_rules else { return };
        for (status, reason) in ended {
            if reason != StatusRemovalReason::Expired {
                continue;
            }
            let Some(action) = r
                .status_cleanses
                .get(&status.definition_id)
                .and_then(|r| r.on_expired.as_ref())
            else {
                continue;
            };
            self.resolve_status_cleanse(
                &status,
                action,
                (StatusCleansePhase::Expired, None, status.expires_at_ms),
                c,
                now,
                events,
            );
        }
    }
    pub(super) fn cleanse_status_tick(
        &mut self,
        status: &StatusView,
        due: u64,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let Some(action) = c
            .combat_rules
            .as_ref()
            .and_then(|r| r.status_cleanses.get(&status.definition_id))
            .and_then(|r| r.on_tick.as_ref())
        else {
            return;
        };
        if !self.statuses.entries().iter().any(|s| {
            s.instance_id == status.instance_id
                && s.source == status.source
                && s.target == status.target
                && s.applied_at_ms == status.applied_at_ms
                && s.expires_at_ms == status.expires_at_ms
                && (now < s.expires_at_ms
                    || due == s.expires_at_ms
                        && s.periodic
                            .as_ref()
                            .is_some_and(|p| p.tick_on_expiry == Some(true)))
        }) {
            return;
        }
        self.resolve_status_cleanse(
            status,
            action,
            (
                StatusCleansePhase::Tick,
                Some(status.periodic.as_ref().unwrap().ticks_done),
                due,
            ),
            c,
            now,
            events,
        );
    }
    fn resolve_status_cleanse(
        &mut self,
        status: &StatusView,
        action: &CleanseAction,
        (phase, tick_index, scheduled_at_ms): (StatusCleansePhase, Option<u32>, u64),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if self.status_cleanses.step != Some(now) {
            self.status_cleanses.step = Some(now);
            self.status_cleanses.used = 0;
        }
        let source = self.entity_owner(&status.source);
        let target = self.entity_owner(&status.target);
        let suppressed = if !self.valid_status_entity(&status.source)
            || !self.valid_status_entity(&status.target)
        {
            Some(Error::InvalidTarget)
        } else if self.status_cleanses.executing
            || self.record_shields.executing
            || self.triggers.executing
            || [source, target]
                .iter()
                .any(|o| !self.status_operation_eligible(o.unwrap()))
        {
            Some(Error::InvalidState)
        } else if self.status_cleanses.used
            >= c.combat_rules
                .as_ref()
                .unwrap()
                .max_cleanse_operations_per_step
        {
            Some(Error::Capacity)
        } else {
            None
        };
        let mut removed_count = 0;
        if suppressed.is_none() {
            self.status_cleanses.used += 1;
            self.status_cleanses.executing = true;
            self.advance_attack_clocks(c, now);
            self.reconcile_caps(c, now);
            let ids = self
                .select_statuses(
                    &self.statuses,
                    &status.target,
                    StatusSelection {
                        disposition: action.disposition,
                        max_count: action.max_count,
                        transferable: false,
                        exclude: Some(&status.instance_id),
                        applied_since: match action.scope {
                            CleanseScope::All => None,
                            CleanseScope::SinceApplication => Some(status.applied_at_ms),
                        },
                    },
                    c,
                    now,
                )
                .iter()
                .map(|s| s.instance_id.clone())
                .collect::<Vec<_>>();
            removed_count = ids.len() as u32;
            let before = self.statuses.entries().to_vec();
            let mut raw = vec![];
            self.statuses.remove(
                StatusRemovalReason::Cleansed,
                |s| ids.contains(&s.instance_id),
                &mut raw,
            );
            self.route_status_events(raw, &before, events);
            self.reconcile_caps(c, now);
            self.status_cleanses.executing = false;
        }
        for i in self.status_observers(&status.source, &status.target) {
            private(
                events,
                i,
                Event::StatusCleanseResolved {
                    instance_id: status.instance_id.clone(),
                    definition_id: status.definition_id.clone(),
                    source: status.source.clone(),
                    target: status.target.clone(),
                    phase,
                    tick_index,
                    scheduled_at_ms,
                    executed_at_ms: now,
                    removed_count,
                    suppressed,
                },
            );
        }
    }
}
