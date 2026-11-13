//! Native status ends are consumed once; recovery never re-enters damage facts.
use super::*;
#[derive(Default)]
pub(super) struct WindowState {
    pub(super) executing: bool,
    step: Option<u64>,
    used: usize,
}
#[derive(Default)]
struct Recovered {
    raw_hp: u32,
    hp: u32,
    raw_spirit: u32,
    spirit: u32,
}
impl World {
    fn window_end_event(
        &self,
        status: &StatusView,
        reason: StatusRemovalReason,
        result: Recovered,
        suppressed: Option<Error>,
        events: &mut Events,
    ) {
        let window = status.window.as_ref().unwrap();
        for i in self.status_observers(&status.source, &status.target) {
            private(
                events,
                i,
                Event::WindowRecordEnded {
                    instance_id: status.instance_id.clone(),
                    definition_id: status.definition_id.clone(),
                    source: status.source.clone(),
                    target: status.target.clone(),
                    reason,
                    recorded: window.recorded,
                    contacts: window.contacts,
                    raw_hp: result.raw_hp,
                    hp_restored: result.hp,
                    raw_spirit: result.raw_spirit,
                    spirit_restored: result.spirit,
                    suppressed,
                },
            );
        }
    }
    pub(super) fn finish_window_records(&mut self, c: &Config, now: u64, events: &mut Events) {
        let ended = self.statuses.take_window_ends();
        if ended.is_empty() {
            return;
        }
        let Some(rules) = &c.combat_rules else {
            return;
        };
        if self.window_records.step != Some(now) {
            self.window_records.step = Some(now);
            self.window_records.used = 0;
        }
        for (status, reason) in ended {
            let Some(rule) = rules.status_windows.get(&status.definition_id) else {
                continue;
            };
            let enabled = match reason {
                StatusRemovalReason::Expired => rule.on_expired,
                StatusRemovalReason::Consumed => rule.on_consumed,
                _ => false,
            };
            let failure = if !enabled {
                None
            } else if self.window_records.executing
                || self.triggers.executing
                || self.record_shields.executing
            {
                Some(Error::InvalidState)
            } else if !self.valid_status_entity(&status.source)
                || !self.valid_status_entity(&status.target)
            {
                Some(Error::InvalidTarget)
            } else if self.periodic_outcome(&status.source, &status.target)
                != PeriodicTickOutcome::Applied
            {
                Some(Error::InvalidState)
            } else if self.window_records.used >= rules.max_window_releases_per_step {
                Some(Error::Capacity)
            } else {
                None
            };
            if !enabled || failure.is_some() {
                self.window_end_event(&status, reason, Recovered::default(), failure, events);
                continue;
            }
            self.window_records.used += 1;
            self.window_records.executing = true;
            let result = self.recover_window(&status, rule, c, now, events);
            self.window_records.executing = false;
            self.window_end_event(&status, reason, result, None, events);
        }
        self.reconcile_caps(c, now);
    }
    fn recover_window(
        &mut self,
        status: &StatusView,
        rule: &crate::config::StatusWindowRule,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Recovered {
        let record = status.window.as_ref().unwrap();
        let target = Self::status_reference(&status.target);
        let live_hp = self.entity_max_hp(&target, c, now).unwrap();
        let live_spirit = self.entity_max_spirit(&target, c, now).unwrap();
        let (maximum_hp, maximum_spirit) = match record.maximum_sampling {
            WindowMaximumSampling::Snapshot => (record.max_hp_at_apply, record.max_spirit_at_apply),
            WindowMaximumSampling::Live => (live_hp, live_spirit),
        };
        let mut result = Recovered::default();
        if let Some(formula) = &rule.hp {
            result.raw_hp = formula.amount(record.recorded, maximum_hp);
            let multiplier_bps = self.statuses.healing_received_bps(
                &status.target,
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
            );
            let adjusted_heal = u64::from(result.raw_hp) * u64::from(multiplier_bps) / 10000;
            result.hp = self.restore_hp(
                &target,
                adjusted_heal.min(u64::from(u32::MAX)) as u32,
                c,
                now,
            );
            for i in self.status_observers(&status.source, &status.target) {
                private(
                    events,
                    i,
                    Event::HealResolved {
                        source: status.source.clone(),
                        target: status.target.clone(),
                        heal_kind: HealKind::WindowRecovery,
                        raw_heal: result.raw_hp,
                        multiplier_bps,
                        adjusted_heal,
                        hp_restored: result.hp,
                    },
                );
            }
        }
        if let Some(formula) = &rule.spirit {
            result.raw_spirit = formula.amount(record.recorded, maximum_spirit);
            match self.entity_owner(&status.target).unwrap() {
                Owner::Member(i) => {
                    result.spirit = result.raw_spirit.min(self.members[i].spirit_recovery_gap(
                        &self.statuses,
                        now,
                        live_spirit,
                    ));
                    self.members[i].spirit += result.spirit;
                }
                Owner::Npc(j) => {
                    result.spirit = result
                        .raw_spirit
                        .min(live_spirit.saturating_sub(self.npcs[j].spirit));
                    self.npcs[j].spirit += result.spirit;
                }
                Owner::Summon(_) => {}
            }
        }
        result
    }
}
