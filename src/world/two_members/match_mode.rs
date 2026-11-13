use super::*;
use crate::config::match_mode::{MatchMode, StageObjective};
use std::cmp::Ordering;
use std::collections::BTreeSet;

struct ModeSeat {
    checkpoint_baseline: u32,
    submitted: bool,
    respawns_left: u8,
    respawn_at_ms: Option<u64>,
    terminal_view: Option<ModeView>,
}

pub(super) struct ModeState {
    contest_stage: Option<usize>,
    stage: usize,
    opened_at_ms: u64,
    expires_at_ms: u64,
    seats: Vec<ModeSeat>,
    target_stages: BTreeMap<String, usize>,
    unopened: BTreeSet<String>,
    wave: Option<ModeWaveView>,
    progress: Option<ModeProgressView>,
    finished: bool,
}

impl ModeState {
    pub(super) fn new(mode: &MatchMode, seats: usize) -> Self {
        Self {
            contest_stage: mode.opposed().then(|| mode.stages.len() - 1),
            stage: 0,
            opened_at_ms: 0,
            expires_at_ms: 0,
            seats: (0..seats)
                .map(|_| ModeSeat {
                    checkpoint_baseline: 0,
                    submitted: false,
                    respawns_left: mode.respawn.as_ref().map_or(0, |r| r.max_per_member),
                    respawn_at_ms: None,
                    terminal_view: None,
                })
                .collect(),
            target_stages: mode
                .stages
                .iter()
                .enumerate()
                .flat_map(|(i, stage)| {
                    let ids: Vec<_> = match &stage.objective {
                        StageObjective::ClearTargets { target_ids } => target_ids.iter().collect(),
                        StageObjective::KillQuota { target_weights, .. } => {
                            target_weights.keys().collect()
                        }
                        _ => vec![],
                    };
                    ids.into_iter().map(move |id| (id.clone(), i))
                })
                .collect(),
            unopened: mode
                .stages
                .iter()
                .flat_map(|stage| match &stage.objective {
                    StageObjective::Waves { waves } => waves.as_slice(),
                    _ => &[],
                })
                .flat_map(|wave| wave.target_ids.iter().cloned())
                .collect(),
            wave: None,
            progress: None,
            finished: false,
        }
    }

    pub(super) fn hides(&self, id: &str) -> bool {
        self.unopened.contains(id)
    }
}

impl World {
    pub(super) fn spectator_stage(&self, c: &Config) -> Option<SpectatorStageV6> {
        let mode = self.mode.as_ref()?;
        let stage = &c.match_mode.as_ref()?.stages[mode.stage];
        Some(SpectatorStageV6 {
            id: stage.id.clone(),
            index: mode.stage as u8 + 1,
            objective: stage.objective.kind(),
            opens_at_ms: mode.opened_at_ms,
            expires_at_ms: mode.expires_at_ms,
            wave: mode.wave.clone(),
            progress: mode.progress,
        })
    }

    pub(super) fn mode_opposed(&self) -> bool {
        self.active_at.is_some()
            && self
                .mode
                .as_ref()
                .is_some_and(|mode| mode.contest_stage == Some(mode.stage))
    }

    pub(super) fn mode_npc_enabled(&self, id: &str) -> bool {
        self.mode.as_ref().is_none_or(|mode| {
            !mode.hides(id)
                && mode
                    .target_stages
                    .get(id)
                    .is_none_or(|stage| mode.stage >= *stage)
        })
    }

    pub(super) fn mode_npc_visible(&self, id: &str) -> bool {
        self.mode.as_ref().is_none_or(|mode| !mode.hides(id))
    }

    pub(super) fn open_mode_stage(
        &mut self,
        index: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let Some(mode) = &mut self.mode else {
            return;
        };
        let stages = &c.match_mode.as_ref().unwrap().stages;
        let stage = &stages[index];
        mode.stage = index;
        mode.opened_at_ms = now;
        mode.expires_at_ms = if stage.absolute_deadline {
            self.active_at.unwrap() + stages[..=index].iter().map(|s| s.duration_ms).sum::<u64>()
        } else {
            now + stage.duration_ms
        };
        mode.wave = None;
        mode.progress = match stage.objective {
            StageObjective::KillQuota { required, .. } => Some(ModeProgressView {
                current: 0,
                required,
            }),
            _ => None,
        };
        let checkpoint = match &stage.objective {
            StageObjective::Checkpoint { schedule_id, .. } => c
                .checkpoint_schedules()
                .iter()
                .position(|r| r.id == *schedule_id),
            _ => None,
        };
        for (i, seat) in mode.seats.iter_mut().enumerate() {
            seat.submitted = false;
            seat.checkpoint_baseline =
                checkpoint.map_or(0, |j| self.members[i].checkpoints[j].collected());
        }
        events.push((
            None,
            EventV4::ModeStageOpened {
                stage_id: stage.id.clone(),
                stage_index: index as u8 + 1,
                opens_at_ms: now,
                expires_at_ms: mode.expires_at_ms,
            },
        ));
        if matches!(&stage.objective, StageObjective::Waves { .. }) {
            self.schedule_mode_wave(0, c, now);
        }
    }

    fn schedule_mode_wave(&mut self, index: usize, c: &Config, now: u64) {
        let state = self.mode.as_mut().unwrap();
        let StageObjective::Waves { waves } =
            &c.match_mode.as_ref().unwrap().stages[state.stage].objective
        else {
            unreachable!()
        };
        state.wave = Some(ModeWaveView {
            index: index as u8 + 1,
            total: waves.len() as u8,
            opens_at_ms: (now + waves[index].delay_ms).min(state.expires_at_ms),
            opened: false,
        });
        self.activate_mode_wave(c, now);
    }

    fn activate_mode_wave(&mut self, c: &Config, now: u64) {
        let state = self.mode.as_mut().unwrap();
        let Some(wave) = &mut state.wave else { return };
        if wave.opened || now < wave.opens_at_ms || now >= state.expires_at_ms {
            return;
        }
        let StageObjective::Waves { waves } =
            &c.match_mode.as_ref().unwrap().stages[state.stage].objective
        else {
            unreachable!()
        };
        let ids = &waves[usize::from(wave.index - 1)].target_ids;
        if self.npcs.iter().any(|npc| {
            ids.contains(&npc.entity.id) && !self.grid.clear(npc.home, npc.profile.combat.radius, 0)
        }) {
            return;
        }
        for npc in &mut self.npcs {
            if ids.contains(&npc.entity.id) && state.unopened.remove(&npc.entity.id) {
                npc.hp = npc.profile.stats["hp"];
            }
        }
        wave.opened = true;
    }

    pub(super) fn mode_view(&self, i: usize, c: &Config) -> Option<ModeView> {
        self.active_at?;
        let mode = self.mode.as_ref()?;
        let stage = &c.match_mode.as_ref()?.stages[mode.stage];
        let seat = &mode.seats[i];
        if let Some(view) = &seat.terminal_view {
            return Some(view.clone());
        }
        Some(ModeView {
            stage_id: stage.id.clone(),
            stage_index: mode.stage as u8 + 1,
            objective: stage.objective.kind(),
            opens_at_ms: mode.opened_at_ms,
            expires_at_ms: mode.expires_at_ms,
            wave: mode.wave.clone(),
            progress: mode.progress,
            submitted: seat.submitted,
            respawns_left: seat.respawns_left,
            respawn_at_ms: seat.respawn_at_ms,
            forfeit_at_ms: None,
            outcome: self.members[i].desired,
        })
    }

    pub(super) fn mode_waiting(&self, c: &Config) -> bool {
        self.mode.as_ref().is_some_and(|state| {
            matches!(
                c.match_mode.as_ref().unwrap().stages[state.stage].objective,
                StageObjective::Wait {}
            )
        })
    }

    pub(super) fn record_mode_kill(&mut self, j: usize, c: &Config) {
        let Some(state) = &mut self.mode else { return };
        let Some(progress) = &mut state.progress else {
            return;
        };
        let StageObjective::KillQuota { target_weights, .. } =
            &c.match_mode.as_ref().unwrap().stages[state.stage].objective
        else {
            unreachable!()
        };
        if let Some(&weight) = target_weights.get(&self.npcs[j].entity.id) {
            progress.current = progress
                .current
                .saturating_add(weight)
                .min(progress.required);
        }
    }

    pub(super) fn submit_mode_checkpoint(
        &mut self,
        i: usize,
        stage_id: &str,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let mode = self.mode.as_mut().ok_or(Error::Unsupported)?;
        let stage = &c.match_mode.as_ref().unwrap().stages[mode.stage];
        let StageObjective::Checkpoint {
            schedule_id,
            members_required,
        } = &stage.objective
        else {
            return Err(Error::InvalidState);
        };
        let seat = &mut mode.seats[i];
        if mode.finished || stage.id != stage_id || now >= mode.expires_at_ms || seat.submitted {
            return Err(Error::InvalidState);
        }
        let (index, rule) = c
            .checkpoint_schedules()
            .iter()
            .enumerate()
            .find(|(_, r)| r.id == *schedule_id)
            .unwrap();
        if self.members[i].checkpoints[index].collected() - seat.checkpoint_baseline < rule.required
        {
            return Err(Error::InvalidState);
        }
        seat.submitted = true;
        if mode
            .seats
            .iter()
            .zip(&self.members)
            .filter(|(seat, member)| seat.submitted && member.desired.is_none())
            .count()
            >= *members_required
        {
            self.complete_mode_stage(c, now, events);
        }
        Ok(())
    }

    pub(crate) fn withdraw_member(&mut self, i: usize, c: &Config, events: &mut Events) {
        if self.mode.is_some() && self.active_at.is_some() {
            self.conclude_mode_member(i, Outcome::Loss, c, events);
        } else {
            self.terminal_intent(i, Outcome::Death, events);
        }
    }

    fn conclude_mode_member(
        &mut self,
        i: usize,
        outcome: Outcome,
        c: &Config,
        events: &mut Events,
    ) {
        if self.members[i].desired.is_some() {
            return;
        }
        self.terminal_intent(i, outcome, events);
        let terminal = self.mode_view(i, c);
        self.mode.as_mut().unwrap().seats[i].terminal_view = terminal;
        events.push((
            Some(i),
            EventV4::ModeConcluded {
                outcome,
                result: c.result_binding(outcome).unwrap().clone(),
            },
        ));
    }

    fn conclude_mode(
        &mut self,
        outcomes: impl Iterator<Item = Outcome>,
        c: &Config,
        events: &mut Events,
    ) {
        self.mode.as_mut().unwrap().finished = true;
        for (i, outcome) in outcomes.enumerate() {
            self.conclude_mode_member(i, outcome, c, events);
        }
    }

    fn mode_elimination(&mut self, c: &Config, events: &mut Events) -> bool {
        if self.mode.as_ref().unwrap().contest_stage.is_none() {
            return false;
        }
        let state = self.mode.as_ref().unwrap();
        let remaining = [0, 1].map(|i| {
            self.members[i].desired.is_none()
                && (self.members[i].hp > 0 || state.seats[i].respawn_at_ms.is_some())
        });
        let outcomes = match remaining {
            [true, true] => return false,
            [true, false] => [Outcome::Win, Outcome::Loss],
            [false, true] => [Outcome::Loss, Outcome::Win],
            [false, false] => [Outcome::Draw, Outcome::Draw],
        };
        self.conclude_mode(outcomes.into_iter(), c, events);
        true
    }

    fn complete_mode_stage(&mut self, c: &Config, now: u64, events: &mut Events) {
        let next = self.mode.as_ref().unwrap().stage + 1;
        if next == c.match_mode.as_ref().unwrap().stages.len() {
            self.conclude_mode(
                std::iter::repeat_n(Outcome::Win, self.members.len()),
                c,
                events,
            );
        } else {
            self.open_mode_stage(next, c, now, events);
        }
    }

    // Deadlines are exclusive. Resolve the old stage before another combat step can change it.
    pub(super) fn mode_before_step(&mut self, c: &Config, now: u64, events: &mut Events) -> bool {
        let Some(state) = &self.mode else {
            return false;
        };
        if state.finished {
            return true;
        }
        if self.mode_elimination(c, events) {
            return true;
        }
        let state = self.mode.as_ref().unwrap();
        let mode = c.match_mode.as_ref().unwrap();
        let objective = &mode.stages[state.stage].objective;
        if now >= state.expires_at_ms {
            match objective {
                StageObjective::Wait {}
                | StageObjective::KillQuota {
                    advance_on_timeout: true,
                    ..
                } => {
                    self.complete_mode_stage(c, now, events);
                }
                StageObjective::Survive { members_required }
                    if self
                        .members
                        .iter()
                        .filter(|member| member.eligible())
                        .count()
                        >= *members_required =>
                {
                    for i in 0..self.members.len() {
                        if !self.members[i].eligible() {
                            self.conclude_mode_member(i, Outcome::Loss, c, events);
                        }
                    }
                    self.complete_mode_stage(c, now, events);
                }
                StageObjective::Contest {} => {
                    let [a, b] = [0, 1].map(|i| {
                        let member = &self.members[i];
                        (
                            u64::from(member.hp),
                            u64::from(
                                self.entity_max_hp(&member.reference.entity, c, now)
                                    .unwrap(),
                            ),
                        )
                    });
                    let outcomes = match (a.0 * b.1).cmp(&(b.0 * a.1)) {
                        Ordering::Greater => [Outcome::Win, Outcome::Loss],
                        Ordering::Less => [Outcome::Loss, Outcome::Win],
                        Ordering::Equal => [Outcome::Draw, Outcome::Draw],
                    };
                    self.conclude_mode(outcomes.into_iter(), c, events);
                }
                _ => self.conclude_mode(
                    std::iter::repeat_n(Outcome::Loss, self.members.len()),
                    c,
                    events,
                ),
            }
            return true;
        }
        if matches!(objective, StageObjective::Wait {}) {
            return true;
        }
        self.activate_mode_wave(c, now);
        for i in 0..self.members.len() {
            if self.mode.as_ref().unwrap().seats[i]
                .respawn_at_ms
                .is_some_and(|at| now >= at)
            {
                self.respawn_mode_member(i, c, now, events);
            }
        }
        false
    }

    pub(super) fn mode_after_step(&mut self, c: &Config, now: u64, events: &mut Events) {
        let Some(mode) = &c.match_mode else {
            return;
        };
        for i in 0..self.members.len() {
            let seat = &mut self.mode.as_mut().unwrap().seats[i];
            if self.members[i].desired.is_none()
                && self.members[i].hp == 0
                && seat.respawn_at_ms.is_none()
                && seat.respawns_left > 0
            {
                seat.respawns_left -= 1;
                seat.respawn_at_ms = Some(now + mode.respawn.as_ref().unwrap().delay_ms);
                self.clear_mode_life(i, now, events);
            }
        }
        if self.mode_elimination(c, events) {
            return;
        }
        for i in 0..self.members.len() {
            if self.members[i].desired.is_none()
                && self.members[i].hp == 0
                && self.mode.as_ref().unwrap().seats[i].respawn_at_ms.is_none()
            {
                self.conclude_mode_member(i, Outcome::Loss, c, events);
            }
        }
        let state = self.mode.as_ref().unwrap();
        let objective = &mode.stages[state.stage].objective;
        let cleared = |ids: &[String]| {
            self.members.iter().any(|m| m.living())
                && ids.iter().all(|id| {
                    self.npcs
                        .iter()
                        .any(|npc| &npc.entity.id == id && npc.hp == 0)
                })
        };
        let completed = match objective {
            StageObjective::ClearTargets { target_ids } => cleared(target_ids),
            StageObjective::KillQuota { .. } => {
                self.members.iter().any(|m| m.living())
                    && state.progress.is_some_and(|p| p.current == p.required)
            }
            StageObjective::Waves { waves } => {
                let wave = state.wave.as_ref().unwrap();
                let index = usize::from(wave.index);
                if !wave.opened || !cleared(&waves[index - 1].target_ids) {
                    false
                } else if index < waves.len() {
                    self.schedule_mode_wave(index, c, now);
                    false
                } else {
                    true
                }
            }
            _ => false,
        };
        if completed {
            self.complete_mode_stage(c, now, events);
        }
    }

    fn clear_mode_life(&mut self, i: usize, now: u64, events: &mut Events) {
        self.clear_member_combat(i, None, events);
        let old = self.members[i].reference.entity.clone();
        self.clear_combat_target(&old, now, events);
    }

    pub(super) fn clear_combat_target(&mut self, old: &EntityRef, now: u64, events: &mut Events) {
        let target = Self::unit_wire(old);
        for j in 0..self.members.len() {
            let member = &mut self.members[j];
            let pending = member
                .pending
                .take_if(|p| p.target.as_ref() == Some(old))
                .is_some();
            let queued = member
                .queued_basic
                .take_if(|p| p.target == target)
                .is_some();
            let pursuit = member
                .basic_pursuit
                .take_if(|p| p.view.target == target)
                .is_some();
            if pending || queued || pursuit {
                private(events, j, Event::ActionInterrupted);
            }
            if member
                .channel
                .as_ref()
                .is_some_and(|p| p.target.as_ref() == Some(old))
            {
                self.finish_channel(j, ChannelEndReason::Interrupted, now, events);
            }
        }
    }

    pub(super) fn clear_mode_respawn(&mut self, i: usize) {
        if let Some(mode) = &mut self.mode {
            mode.seats[i].respawn_at_ms = None;
        }
    }

    fn respawn_mode_member(&mut self, i: usize, c: &Config, now: u64, events: &mut Events) {
        self.mode.as_mut().unwrap().seats[i].respawn_at_ms = None;
        let rule = c.match_mode.as_ref().unwrap().respawn.as_ref().unwrap();
        let spawn = c.member_settings().unwrap().spawns[i];
        if !self.grid.clear(spawn, c.params.player_radius, 0) {
            self.mode.as_mut().unwrap().seats[i].respawn_at_ms = Some(now + c.step_ms);
            return;
        }
        self.restore_member_life(i, spawn, (rule.hp_bps, rule.spirit_bps), c, now, events);
        let m = &mut self.members[i];
        m.cooldowns.clear();
        m.basic_ready = now;
        m.basic_charge_ready = now;
    }

    pub(super) fn restore_member_life(
        &mut self,
        i: usize,
        spawn: [f64; 2],
        (hp_bps, spirit_bps): (u32, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let old = self.members[i].reference.entity.clone();
        let max_hp = self.entity_max_hp(&old, c, now).unwrap();
        let max_spirit = self.member_max_spirit(i, c, now);
        let m = &mut self.members[i];
        m.reference.entity.generation += 1;
        m.position = spawn;
        m.hp = ((u64::from(max_hp) * u64::from(hp_bps) / 10000) as u32).max(1);
        m.spirit = (u64::from(max_spirit) * u64::from(spirit_bps) / 10000) as u32;
        m.basic_wave_phase = BasicWavePhase::Primary;
        m.summon_copy_ready = (0, 0);
        m.attack_clock = None;
        m.end_recovery(now);
        let entity = m.reference.entity.clone();
        for resource in &mut self.resources {
            if resource.view.owner.as_ref() == Some(&old) {
                resource.view.owner = Some(entity.clone());
            }
        }
        events.push((None, EventV4::ModeRespawned { entity }));
    }
}
