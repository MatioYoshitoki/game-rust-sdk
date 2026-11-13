//! World-clock windows with one authoritative receipt per member and window.
use super::*;
use crate::config::checkpoint_zones::{
    CheckpointCounterRule, CheckpointQualification, CheckpointSchedule, CounterActivation,
    CounterBoon,
};

#[derive(Clone, Default)]
pub(super) struct CheckpointCounter {
    collected: u32,
    last_window_index: u32,
    counter: Option<CheckpointCounterView>,
    contact_epoch: u32,
    observed_warning: u32,
    warning: Option<OpenWindow>,
}

impl CheckpointCounter {
    pub(super) fn collected(&self) -> u32 {
        self.collected
    }

    fn view(
        &self,
        rule: &CheckpointSchedule,
        counter: Option<CheckpointCounterView>,
    ) -> CheckpointProgressView {
        CheckpointProgressView {
            schedule_id: rule.id.clone(),
            collected: self.collected,
            required: rule.required,
            total_windows: rule.centers.len() as u32,
            last_window_index: self.last_window_index,
            counter,
        }
    }
}

#[derive(Clone, Copy)]
struct OpenWindow {
    index: u32,
    center: [f64; 2],
    opens_at_ms: u64,
    expires_at_ms: u64,
}

pub(super) struct CounterContact {
    member: usize,
    npc: usize,
    schedule: usize,
    window_index: u32,
}

fn open_window(rule: &CheckpointSchedule, active_at_ms: u64, now: u64) -> Option<OpenWindow> {
    let start = active_at_ms + rule.first_delay_ms;
    let elapsed = now.checked_sub(start)?;
    let index = (elapsed / rule.interval_ms) as usize;
    let center = *rule.centers.get(index)?;
    let opens_at_ms = start + index as u64 * rule.interval_ms;
    let expires_at_ms = opens_at_ms + rule.window_ms;
    (now < expires_at_ms).then_some(OpenWindow {
        index: index as u32 + 1,
        center,
        opens_at_ms,
        expires_at_ms,
    })
}

impl Member {
    pub(super) fn checkpoint_completed(&self, c: &Config, schedule_id: &str) -> bool {
        c.checkpoint_schedules()
            .iter()
            .zip(&self.checkpoints)
            .any(|(rule, progress)| rule.id == schedule_id && progress.collected >= rule.required)
    }
}

impl World {
    pub(super) fn checkpoint_progress(
        &self,
        i: usize,
        c: &Config,
        now: u64,
    ) -> Option<Vec<CheckpointProgressView>> {
        let rules = c.checkpoint_schedules();
        let member = &self.members[i];
        (!rules.is_empty()).then(|| {
            rules
                .iter()
                .zip(&member.checkpoints)
                .map(|(rule, progress)| {
                    let counter = progress
                        .counter
                        .as_ref()
                        .filter(|counter| {
                            member.living()
                                && now < counter.expires_at_ms
                                && self.checkpoint_target(&counter.target).is_some()
                        })
                        .cloned();
                    progress.view(rule, counter)
                })
                .collect()
        })
    }

    fn checkpoint_target(&self, target: &UnitTarget) -> Option<usize> {
        self.npcs.iter().enumerate().find_map(|(j, npc)| {
            (npc.entity.id == target.id && npc.entity.generation == target.generation && npc.hp > 0)
                .then_some(j)
        })
    }

    fn checkpoint_bonus(&self, i: usize, bonus: Option<&CounterBoon>) -> u32 {
        bonus
            .and_then(|bonus| {
                let level = self.boon_level(i, &bonus.id)?;
                bonus.by_level.get(usize::from(level - 1)).copied()
            })
            .unwrap_or(0)
    }

    pub(super) fn clear_checkpoint_counters(&mut self, i: usize) {
        self.clear_checkpoint_warnings(i);
        for progress in &mut self.members[i].checkpoints {
            progress.counter = None;
        }
    }

    pub(super) fn clear_checkpoint_warnings(&mut self, i: usize) {
        for progress in &mut self.members[i].checkpoints {
            progress.warning = None;
        }
    }

    pub(super) fn register_checkpoint_action(
        &mut self,
        i: usize,
        action: &str,
        epoch: u32,
        now: u64,
    ) {
        let member = &self.members[i];
        if epoch > 0
            && member.checkpoints.iter().any(|progress| {
                progress.contact_epoch != 0
                    && epoch >= progress.contact_epoch
                    && progress
                        .counter
                        .as_ref()
                        .is_some_and(|view| now < view.expires_at_ms)
            })
        {
            let key = triggers::ActionKey::new(&member.reference.entity, action);
            self.triggers.checkpoint_actions.insert(key, epoch);
        }
    }

    pub(super) fn retain_committed_checkpoint_action(&mut self, i: usize) {
        let Some(action) = self.triggers.action.as_deref() else {
            return;
        };
        let key = triggers::ActionKey::new(&self.members[i].reference.entity, action);
        if self.triggers.checkpoint_actions.contains_key(&key)
            && !self.active_trigger_actions().contains(&key)
        {
            self.triggers.checkpoint_actions.remove(&key);
        }
    }

    pub(super) fn take_checkpoint_contact(
        &mut self,
        (owner, target): (Owner, Owner),
        action: Option<&str>,
        c: &Config,
        now: u64,
    ) -> Option<CounterContact> {
        let (Owner::Member(i), Owner::Npc(j), Some(action)) = (owner, target, action) else {
            return None;
        };
        if self.triggers.checkpoint_actions.is_empty()
            || !self.members[i].eligible()
            || self.triggers.executing
            || self.record_shields.executing
            || self.window_records.executing
            || self.status_ends.executing
        {
            return None;
        }
        let member = &self.members[i].reference.entity;
        let epoch = *self
            .triggers
            .checkpoint_actions
            .get(&triggers::ActionKey::new(member, action))?;
        let target = Self::unit_wire(&self.npcs[j].entity);
        for (index, schedule) in c.checkpoint_schedules().iter().enumerate() {
            let Some(rule) = schedule
                .counter
                .as_ref()
                .filter(|rule| rule.activation == CounterActivation::FirstContact)
            else {
                continue;
            };
            let progress = &self.members[i].checkpoints[index];
            let Some(window_index) = progress
                .counter
                .as_ref()
                .filter(|view| {
                    epoch >= progress.contact_epoch
                        && view.target == target
                        && now >= view.opens_at_ms
                        && now < view.expires_at_ms
                })
                .map(|view| view.window_index)
            else {
                continue;
            };
            if self
                .plan_checkpoint_counter(i, &target, rule, c, now)
                .is_ok()
            {
                self.members[i].checkpoints[index].counter = None;
                return Some(CounterContact {
                    member: i,
                    npc: j,
                    schedule: index,
                    window_index,
                });
            }
        }
        None
    }

    pub(super) fn finish_checkpoint_contact(
        &mut self,
        contact: CounterContact,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let schedule = &c.checkpoint_schedules()[contact.schedule];
        let rule = schedule.counter.as_ref().unwrap();
        self.resolve_checkpoint_counter(
            (contact.member, contact.npc),
            (&schedule.id, contact.window_index, rule),
            c,
            now,
            events,
        );
    }

    pub(super) fn counter_checkpoint(
        &mut self,
        i: usize,
        (schedule_id, window_index, target): (&str, u32, &UnitTarget),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let (index, schedule) = c
            .checkpoint_schedules()
            .iter()
            .enumerate()
            .find(|(_, schedule)| schedule.id == schedule_id)
            .ok_or(Error::Unsupported)?;
        let rule = schedule
            .counter
            .as_ref()
            .filter(|rule| rule.activation == CounterActivation::Manual)
            .ok_or(Error::Unsupported)?;
        let m = &self.members[i];
        let counter = m.checkpoints[index]
            .counter
            .as_ref()
            .filter(|counter| counter.window_index == window_index && now < counter.expires_at_ms)
            .ok_or(Error::InvalidState)?;
        if &counter.target != target {
            return Err(Error::InvalidTarget);
        }
        if m.pending.is_some()
            || m.channel.is_some()
            || m.basic_beam.is_some()
            || m.basic_charge.is_some()
            || m.basic_pursuit.is_some()
            || m.extract_end.is_some()
            || m.pending_boon_claim()
            || self.has_container_reservation(i)
            || self.member_has_status(i, StatusAttribute::Stun, now)
        {
            return Err(Error::InvalidState);
        }
        let j = self.plan_checkpoint_counter(i, target, rule, c, now)?;
        self.members[i].checkpoints[index].counter = None;
        self.resolve_checkpoint_counter((i, j), (&schedule.id, window_index, rule), c, now, events);
        Ok(())
    }

    fn plan_checkpoint_counter(
        &mut self,
        i: usize,
        target: &UnitTarget,
        rule: &CheckpointCounterRule,
        c: &Config,
        now: u64,
    ) -> Result<usize> {
        let j = self.checkpoint_target(target).ok_or(Error::InvalidTarget)?;
        let member = &self.members[i];
        let npc = &self.npcs[j];
        if distance(member.position, npc.position) > rule.range {
            return Err(Error::OutOfRange);
        }
        if !self.grid.line(member.position, npc.position, 0., 1)
            || !self.grid.line(member.position, npc.position, 0., 2)
        {
            return Err(Error::InvalidTarget);
        }
        let source = member.reference.entity.clone();
        if !self.hostile_contact(&source, &npc.entity) {
            return Err(Error::InvalidTarget);
        }
        if !self.direct_contact_eligible(
            Owner::Member(i),
            Owner::Npc(j),
            &source,
            (true, false),
            c,
            now,
        ) {
            return Err(Error::Immune);
        }
        Ok(j)
    }

    fn resolve_checkpoint_counter(
        &mut self,
        (i, j): (usize, usize),
        (schedule_id, window_index, rule): (&str, u32, &CheckpointCounterRule),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let bonus = self.checkpoint_bonus(i, rule.damage_bonus.as_ref());
        let raw = (u64::from(rule.base_damage) * u64::from(10000 + bonus) / 10000) as u32;
        let target = self.npcs[j].entity.clone();
        let attack = crate::config::AttackRule {
            can_crit: false,
            ..Default::default()
        };
        let prior = std::mem::replace(&mut self.triggers.executing, true);
        let damage = self.resolve_damage_with_rule(
            combat::DamageIntent {
                source: self.members[i].reference.entity.clone(),
                target: target.clone(),
                raw,
                direct: false,
                ignore_armor: false,
            },
            Some(&attack),
            c,
            now,
            events,
        );
        self.triggers.executing = prior;
        private(
            events,
            i,
            Event::CheckpointCounterResolved {
                schedule_id: schedule_id.into(),
                window_index,
                target,
                raw_damage: raw,
                absorbed: damage.absorbed,
                hp_damage: damage.hp_damage,
            },
        );
    }

    pub(super) fn checkpoint_zones(
        &self,
        member: usize,
        c: &Config,
        now: u64,
    ) -> Option<Vec<CheckpointZoneView>> {
        let rules = c.checkpoint_schedules();
        (!rules.is_empty()).then(|| {
            let Some(active) = self.active_at.filter(|_| self.members[member].living()) else {
                return Vec::new();
            };
            rules
                .iter()
                .filter_map(|rule| {
                    let window = open_window(rule, active, now)?;
                    Some(CheckpointZoneView {
                        schedule_id: rule.id.clone(),
                        window_index: window.index,
                        center: window.center,
                        radius: rule.radius,
                        opens_at_ms: window.opens_at_ms,
                        expires_at_ms: window.expires_at_ms,
                        avoid: (rule.qualification == CheckpointQualification::Avoid)
                            .then_some(true),
                    })
                })
                .collect()
        })
    }

    pub(super) fn advance_checkpoints(
        &mut self,
        c: &Config,
        active: u64,
        now: u64,
        events: &mut Events,
    ) {
        for (schedule, rule) in c.checkpoint_schedules().iter().enumerate() {
            let open = open_window(rule, active, now);
            for i in 0..self.members.len() {
                let window = match rule.qualification {
                    CheckpointQualification::Reach => open.filter(|window| {
                        self.members[i].eligible()
                            && self.members[i].checkpoints[schedule].last_window_index
                                < window.index
                            && self.inside_checkpoint(i, rule, window)
                    }),
                    CheckpointQualification::Avoid => {
                        self.advance_checkpoint_warning(i, (schedule, rule), open, now)
                    }
                };
                if let Some(window) = window {
                    self.complete_checkpoint(i, (schedule, rule, window.index), c, now, events);
                }
            }
        }
    }

    fn inside_checkpoint(&self, i: usize, rule: &CheckpointSchedule, window: &OpenWindow) -> bool {
        let position = self.members[i].position;
        distance(window.center, position) <= rule.radius
            && self.grid.walk_until_blocked(window.center, position, 0.) == position
    }

    fn advance_checkpoint_warning(
        &mut self,
        i: usize,
        (schedule, rule): (usize, &CheckpointSchedule),
        open: Option<OpenWindow>,
        now: u64,
    ) -> Option<OpenWindow> {
        let eligible = self.members[i].eligible();
        let closed = self.members[i].checkpoints[schedule]
            .warning
            .take_if(|window| now >= window.expires_at_ms);
        if let Some(window) = open
            .filter(|window| window.index > self.members[i].checkpoints[schedule].observed_warning)
        {
            let exposed = eligible && self.inside_checkpoint(i, rule, &window);
            let progress = &mut self.members[i].checkpoints[schedule];
            progress.observed_warning = window.index;
            progress.warning = exposed.then_some(window);
        }
        closed.filter(|window| eligible && !self.inside_checkpoint(i, rule, window))
    }

    fn complete_checkpoint(
        &mut self,
        i: usize,
        (schedule, rule, window_index): (usize, &CheckpointSchedule, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let target = rule.counter.as_ref().and_then(|counter| {
            let id = &counter.target_ids[(window_index - 1) as usize];
            self.npcs
                .iter()
                .find(|npc| npc.entity.id == *id && npc.hp > 0)
                .map(|npc| Self::unit_wire(&npc.entity))
        });
        let opened = rule
            .counter
            .as_ref()
            .zip(target.as_ref())
            .and_then(|(counter, target)| {
                let base = counter.window_ms?;
                let extension = self.checkpoint_bonus(i, counter.window_extension.as_ref());
                Some(CheckpointCounterView {
                    window_index,
                    target: target.clone(),
                    opens_at_ms: now,
                    expires_at_ms: (now + base + u64::from(extension))
                        .min(self.active_at.unwrap() + c.terminal_ms),
                    on_contact: (counter.activation == CounterActivation::FirstContact)
                        .then_some(true),
                })
            });
        let progress = &mut self.members[i].checkpoints[schedule];
        progress.collected += 1;
        progress.last_window_index = window_index;
        if opened
            .as_ref()
            .is_some_and(|view| view.on_contact == Some(true))
        {
            // A window and an older accepted action can share one World timestamp.
            self.checkpoint_epoch += 1;
            progress.contact_epoch = self.checkpoint_epoch;
        }
        progress.counter = opened.clone();
        let view = progress.view(rule, opened);
        let event = if rule.qualification == CheckpointQualification::Avoid {
            Event::CheckpointAvoided {
                progress: view,
                at_ms: now,
            }
        } else {
            Event::CheckpointReached {
                progress: view,
                at_ms: now,
            }
        };
        private(events, i, event);
        if let Some((counter, target)) = rule.counter.as_ref().zip(target.as_ref())
            && counter.activation == CounterActivation::OnReach
            && let Ok(j) = self.plan_checkpoint_counter(i, target, counter, c, now)
        {
            self.resolve_checkpoint_counter(
                (i, j),
                (&rule.id, window_index, counter),
                c,
                now,
                events,
            );
        }
    }
}
