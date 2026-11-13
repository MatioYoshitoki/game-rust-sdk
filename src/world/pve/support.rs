use super::*;
use crate::config::{CompletedEncounterSupport, SupportSkill};

pub(super) struct SupportCast {
    id: u32,
    target: usize,
    target_id: String,
    target_epoch: u32,
    started: u64,
    commits: u64,
    next_tick: u64,
    interrupt_until: u64,
    completed: u32,
    budget: u32,
    uses_completed_variant: bool,
}
impl Arena {
    fn completed_support_variant<'a>(
        &self,
        skill: &'a SupportSkill,
    ) -> Option<&'a CompletedEncounterSupport> {
        let variant = skill.on_encounter_completed.as_ref()?;
        self.after_encounter_completed(&variant.encounter_id, variant)
    }

    pub(super) fn support_view(&self, caster: &Monster) -> Option<SupportView> {
        let skill = caster.profile.support.as_ref()?;
        let variant = match &caster.support_cast {
            Some(cast) => skill
                .on_encounter_completed
                .as_ref()
                .filter(|_| cast.uses_completed_variant),
            None => self.completed_support_variant(skill),
        };
        Some(SupportView {
            skill_id: variant.map_or(&skill.id, |v| &v.id).clone(),
            name: variant.map_or(&skill.name, |v| &v.name).clone(),
            remaining_casts: skill.max_casts - caster.support_used,
            max_casts: skill.max_casts,
            ready_at_ms: caster.support_ready,
            interrupt_ready_at_ms: self.support_interrupt_ready,
            cast: caster.support_cast.as_ref().map(|a| SupportCastView {
                cast_id: a.id,
                target_id: a.target_id.clone(),
                committed: a.completed > 0,
                started_at_ms: a.started,
                commits_at_ms: a.commits,
                next_tick_at_ms: a.next_tick,
                interrupt_until_ms: a.interrupt_until,
                completed_ticks: a.completed,
                total_ticks: skill.ticks,
            }),
        })
    }
}
impl Monster {
    fn take_support(&mut self, now: u64) -> Option<SupportCastIdentity> {
        let cast = self.support_cast.take()?;
        self.recovery = self
            .recovery
            .max(now + self.profile.support.as_ref().unwrap().recovery_ms);
        Some(SupportCastIdentity {
            monster_id: self.id.clone(),
            cast_id: cast.id,
            target_id: cast.target_id,
            committed: cast.completed > 0,
        })
    }
}
impl Arena {
    pub(crate) fn has_support_cast(&self) -> bool {
        self.monsters.iter().any(|m| m.support_cast.is_some())
    }
    pub(crate) fn cancel_all_supports(
        &mut self,
        reason: SupportCancelReason,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let casts: Vec<_> = self
            .monsters
            .iter_mut()
            .filter_map(|m| m.take_support(now))
            .collect();
        if !casts.is_empty() {
            events.push(Event::SupportCancelled { casts, reason });
        }
    }
    pub(super) fn cancel_supports_for(
        &mut self,
        index: usize,
        reason: SupportCancelReason,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        self.cancel_supports_for_members(&[index], reason, now, events);
    }
    pub(super) fn cancel_supports_for_members(
        &mut self,
        members: &[usize],
        reason: SupportCancelReason,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let casts: Vec<_> = self
            .monsters
            .iter_mut()
            .enumerate()
            .filter(|(i, m)| {
                members.contains(i)
                    || m.support_cast
                        .as_ref()
                        .is_some_and(|c| members.contains(&c.target))
            })
            .filter_map(|(_, m)| m.take_support(now))
            .collect();
        if !casts.is_empty() {
            events.push(Event::SupportCancelled { casts, reason });
        }
    }
    fn support_group(&self, from: usize, to: usize) -> bool {
        let a = &self.monsters[from];
        let b = &self.monsters[to];
        from == to
            || a.encounter_id
                .as_ref()
                .is_some_and(|id| b.encounter_id.as_ref() == Some(id))
            || a.binding
                .as_ref()
                .is_some_and(|binding| b.binding.as_ref() == Some(binding))
    }
    fn support_target_valid(&self, from: usize, to: usize) -> bool {
        let caster = &self.monsters[from];
        let target = &self.monsters[to];
        let s = caster.profile.support.as_ref().unwrap();
        target.enabled
            && target.hp > 0
            && !target.returning
            && (from != to || s.include_self)
            && self.support_group(from, to)
            && distance(caster.position, target.position) <= s.range
            && self.grid.line(caster.position, target.position, 0.0, 2)
    }
    pub(super) fn try_start_support(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        now: u64,
        events: &mut Vec<Event>,
    ) -> bool {
        let caster = &self.monsters[i];
        let Some(s) = caster.profile.support.as_ref() else {
            return false;
        };
        if c.mechanism_version < 10
            || w.phase != Phase::Active
            || w.protected
            || !w.ready
            || !w.foreground
            || w.hp == 0
            || w.desired_outcome.is_some()
            || !caster.enabled
            || caster.hp == 0
            || !caster.aggro
            || caster.returning
            || caster.heavy_cast.is_some()
            || caster.support_cast.is_some()
            || caster.area_cast.is_some()
            || caster.casting.is_some()
            || caster.heal.is_some()
            || now < caster.recovery
            || now < caster.support_ready
            || caster.support_used >= s.max_casts
            || caster.spirit < s.cost
            || s.cast_chance == 0.0
        {
            return false;
        }
        let target = self
            .monsters
            .iter()
            .enumerate()
            .filter(|(j, m)| {
                self.support_target_valid(i, *j)
                    && f64::from(m.hp) < f64::from(m.profile.stats["hp"]) * s.hp_threshold
            })
            .min_by(|(_, a), (_, b)| {
                (u64::from(a.hp) * u64::from(b.profile.stats["hp"]))
                    .cmp(&(u64::from(b.hp) * u64::from(a.profile.stats["hp"])))
                    .then(a.id.cmp(&b.id))
            })
            .map(|(j, _)| j);
        let Some(target) = target else {
            return false;
        };
        if s.cast_chance < 1.0
            && f64::from(w.random()) / (f64::from(u32::MAX) + 1.0) >= s.cast_chance
        {
            return false;
        }
        let t = &self.monsters[target];
        let target_id = t.id.clone();
        let epoch = t.boss_epoch;
        let variant = self.completed_support_variant(s);
        let heal_bps = variant.map_or(s.heal_bps, |v| v.heal_bps);
        let uses_completed_variant = variant.is_some();
        let budget = (u64::from(t.profile.stats["hp"]) * u64::from(heal_bps) / 10000) as u32;
        let commits = now + s.windup_ms;
        let interrupt_until = now + s.interrupt_window_ms;
        let caster = &mut self.monsters[i];
        caster.support_sequence += 1;
        let id = caster.support_sequence;
        caster.support_cast = Some(SupportCast {
            id,
            target,
            target_id: target_id.clone(),
            target_epoch: epoch,
            started: now,
            commits,
            next_tick: commits,
            interrupt_until,
            completed: 0,
            budget,
            uses_completed_variant,
        });
        caster.path.clear();
        events.push(Event::SupportStarted {
            monster_id: caster.id.clone(),
            cast_id: id,
            target_id,
            commits_at_ms: commits,
            interrupt_until_ms: interrupt_until,
        });
        true
    }
    pub(super) fn interrupt_support_on_contact(
        &mut self,
        index: usize,
        started: Option<u64>,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let Some(started) = started else {
            return;
        };
        let Some(cast) = self.monsters[index].support_cast.as_ref() else {
            return;
        };
        if started < cast.started
            || now > cast.interrupt_until
            || now < self.support_interrupt_ready
        {
            return;
        }
        let s = self.monsters[index].profile.support.as_ref().unwrap();
        self.support_interrupt_ready = now + s.interrupt_cooldown_ms;
        let cast = self.monsters[index].take_support(now).unwrap();
        events.push(Event::SupportInterrupted { cast });
    }
    pub(super) fn advance_supports(
        &mut self,
        w: &World,
        c: &Config,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        if w.phase != Phase::Active || w.hp == 0 || w.desired_outcome.is_some() {
            self.cancel_all_supports(SupportCancelReason::Closed, now, events);
            return;
        }
        if w.protected || !w.ready || !w.foreground {
            self.cancel_all_supports(SupportCancelReason::Background, now, events);
            return;
        }
        let mut invalid = vec![];
        let mut resource = vec![];
        for i in 0..self.monsters.len() {
            let Some(cast) = self.monsters[i].support_cast.as_ref() else {
                continue;
            };
            let target = cast.target;
            let valid = self.monsters[i].enabled
                && self.monsters[i].hp > 0
                && !self.monsters[i].returning
                && self.support_target_valid(i, target)
                && self.monsters[target].id == cast.target_id
                && self.monsters[target].boss_epoch == cast.target_epoch;
            if !valid {
                let cast = self.monsters[i].take_support(now).unwrap();
                invalid.push(cast);
                continue;
            }
            if now < cast.next_tick {
                continue;
            }
            let s = self.monsters[i].profile.support.as_ref().unwrap().clone();
            if cast.completed == 0 {
                if self.monsters[i].spirit < s.cost {
                    let cast = self.monsters[i].take_support(now).unwrap();
                    resource.push(cast);
                    continue;
                }
                self.monsters[i].spirit -= s.cost;
                self.monsters[i].support_used += 1;
                self.monsters[i].support_ready = now + s.cooldown_ms;
            }
            let mut cast = self.monsters[i].support_cast.take().unwrap();
            let share = cast.budget / s.ticks + u32::from(cast.completed < cast.budget % s.ticks);
            let source_entity = self.status_entity(i);
            let target_entity = self.status_entity(target);
            let amount = self.resolve_heal(
                w,
                c,
                (&source_entity, &target_entity),
                (HealKind::SupportChannel, share),
                now,
                events,
            );
            self.monsters[target].hp += amount;
            cast.completed += 1;
            events.push(Event::SupportPulse {
                monster_id: self.id(i),
                cast_id: cast.id,
                target_id: cast.target_id.clone(),
                tick: cast.completed,
                amount,
            });
            if cast.completed == s.ticks {
                self.monsters[i].recovery = self.monsters[i].recovery.max(now + s.recovery_ms);
                events.push(Event::SupportCompleted {
                    monster_id: self.id(i),
                    cast_id: cast.id,
                    target_id: cast.target_id,
                });
            } else {
                cast.next_tick += s.tick_interval_ms;
                self.monsters[i].support_cast = Some(cast);
            }
        }
        if !invalid.is_empty() {
            events.push(Event::SupportCancelled {
                casts: invalid,
                reason: SupportCancelReason::TargetInvalid,
            });
        }
        if !resource.is_empty() {
            events.push(Event::SupportCancelled {
                casts: resource,
                reason: SupportCancelReason::Resource,
            });
        }
    }
}
