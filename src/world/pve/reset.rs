use super::*;

impl Arena {
    pub(super) fn round_stamp(&self) -> Vec<u32> {
        if self
            .grid
            .config
            .encounters
            .iter()
            .any(|e| e.reset.is_some())
        {
            self.encounters.iter().map(|e| e.round).collect()
        } else {
            vec![]
        }
    }
    pub(super) fn accepts_round(&self, m: &Monster, stamp: &[u32]) -> bool {
        m.encounter_index.is_none_or(|i| {
            self.grid.config.encounters[i].reset.is_none()
                || stamp.get(i) == Some(&self.encounters[i].round)
        })
    }
    pub(super) fn group_managed(&self, i: usize) -> bool {
        self.monsters[i].encounter_index.is_some_and(|j| {
            self.grid.config.encounters[j].reset.is_some()
                && self.encounters[j].phase != EncounterPhase::Completed
        })
    }
    pub(crate) fn has_reset_countdown(&self) -> bool {
        self.encounters.iter().any(|e| e.reset_at.is_some())
    }
    pub(crate) fn cancel_all_reset_countdowns(&mut self, events: &mut Vec<Event>) {
        let ids: Vec<_> = self
            .encounters
            .iter_mut()
            .zip(&self.grid.config.encounters)
            .filter_map(|(s, c)| s.reset_at.take().map(|_| c.id.clone()))
            .collect();
        if !ids.is_empty() {
            events.push(Event::EncounterResetCancelled { encounter_ids: ids });
        }
    }
    fn cancel_group_actions(
        &mut self,
        slots: &[usize],
        replaced: bool,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        self.cancel_supports_for_members(
            slots,
            if replaced {
                SupportCancelReason::Replaced
            } else {
                SupportCancelReason::Returning
            },
            now,
            events,
        );
        let areas: Vec<_> = slots
            .iter()
            .filter_map(|&i| self.monsters[i].remove_area(now))
            .collect();
        if !areas.is_empty() {
            events.push(Event::AreaCancelled {
                casts: areas,
                reason: if replaced {
                    AreaCancelReason::Replaced
                } else {
                    AreaCancelReason::Returning
                },
            });
        }
        self.cancel_heavies_for_members(
            slots,
            if replaced {
                HeavyCancelReason::Replaced
            } else {
                HeavyCancelReason::Returning
            },
            now,
            events,
        );
    }
    fn withdraw(&mut self, slots: &[usize]) {
        for &i in slots {
            let m = &mut self.monsters[i];
            if m.enabled && m.hp > 0 {
                m.returning = true;
                m.aggro = false;
                m.casting = None;
                m.heal = None;
            }
        }
    }
    pub(super) fn advance_encounter_resets(
        &mut self,
        w: &World,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        if w.phase != Phase::Active || w.hp == 0 || w.desired_outcome.is_some() {
            self.cancel_all_reset_countdowns(events);
            return;
        }
        let mut cancelled = vec![];
        for j in 0..self.encounters.len() {
            let c = &self.grid.config.encounters[j];
            let Some(r) = &c.reset else { continue };
            if matches!(
                self.encounters[j].phase,
                EncounterPhase::Dormant | EncounterPhase::Completed
            ) {
                continue;
            }
            let id = c.id.clone();
            let delay = r.delay_ms;
            let inside = distance(w.position, c.trigger.center) <= r.pursuit_radius;
            let slots = self.encounters[j].slots.clone();
            if inside {
                if self.encounters[j].reset_at.take().is_some() {
                    for &i in &slots {
                        let m = &mut self.monsters[i];
                        if m.enabled && m.hp > 0 {
                            m.returning = false;
                            m.aggro = true;
                            m.path.clear();
                        }
                    }
                    cancelled.push(id);
                }
                continue;
            }
            if self.encounters[j].reset_at.is_none() {
                self.encounters[j].reset_at = Some(now + delay);
                self.cancel_group_actions(&slots, false, now, events);
                for &i in &slots {
                    self.monsters[i].path.clear();
                }
                events.push(Event::EncounterResetScheduled {
                    encounter_id: id.clone(),
                    round: self.encounters[j].round,
                    resets_at_ms: now + delay,
                });
            }
            self.withdraw(&slots);
            if now < self.encounters[j].reset_at.unwrap() {
                continue;
            }
            self.cancel_group_actions(&slots, true, now, events);
            self.shots
                .retain(|s| s.owner.is_none_or(|i| !slots.contains(&i)));
            if self.locked.is_some_and(|i| slots.contains(&i)) {
                self.locked = None;
            }
            self.mark_sources.retain(|i| !slots.contains(i));
            self.marks = self.mark_sources.len() as u32;
            if self.marks == 0 {
                self.marks_until = 0;
            }
            for &i in &slots {
                self.clear_status_entity(
                    &self.status_entity(i),
                    StatusRemovalReason::Reset,
                    events,
                );
                let old = &self.monsters[i];
                let mut m = Monster::new(
                    old.id.clone(),
                    old.template_id.clone(),
                    old.profile.clone(),
                    old.home,
                    None,
                );
                m.encounter_id = old.encounter_id.clone();
                m.encounter_index = old.encounter_index;
                m.wave_id = old.wave_id.clone();
                m.boss_epoch = old.boss_epoch + 1;
                m.status_generation = old.status_generation + 1;
                m.heavy_sequence = old.heavy_sequence;
                m.area_sequence = old.area_sequence;
                m.support_sequence = old.support_sequence;
                m.enabled = false;
                m.hp = 0;
                self.monsters[i] = m;
            }
            let s = &mut self.encounters[j];
            s.round += 1;
            s.phase = EncounterPhase::Dormant;
            s.current_wave = 0;
            s.completed_waves = 0;
            s.ready_at = None;
            s.retry_at = 0;
            s.blocked_reported = false;
            s.reset_at = None;
            events.push(Event::EncounterReset {
                encounter_id: id,
                round: s.round,
            });
        }
        if !cancelled.is_empty() {
            events.push(Event::EncounterResetCancelled {
                encounter_ids: cancelled,
            });
        }
    }
    pub(super) fn record_defeat(&mut self, i: usize) -> bool {
        let Some(j) = self.monsters[i].encounter_index else {
            return true;
        };
        if self.grid.config.encounters[j].reset.is_none() {
            return true;
        }
        let s = &mut self.encounters[j];
        let d = s.defeats.entry(i).or_default();
        debug_assert_ne!(d.1, s.round, "one physical death per member per round");
        let first = d.0 == 0;
        d.0 += 1;
        d.1 = s.round;
        first
    }
    pub(super) fn actual_kills(&self, c: &Config) -> u32 {
        c.score_summary(&self.score_tally).kills
            + self
                .encounters
                .iter()
                .flat_map(|e| e.defeats.values())
                .map(|(n, _)| n - 1)
                .sum::<u32>()
    }
    pub(super) fn reset_summary(&self, c: &Config) -> Option<ResetProgressSummary> {
        if !c.resets_enabled() {
            return None;
        }
        let mut rows: Vec<_> = self
            .encounters
            .iter()
            .zip(&self.grid.config.encounters)
            .filter(|(_, c)| c.reset.is_some())
            .map(|(s, c)| {
                let mut defeats: Vec<_> = s
                    .defeats
                    .iter()
                    .map(|(&i, &(deaths, last_round))| SpawnDefeatSummary {
                        monster_id: self.id(i),
                        deaths,
                        last_round,
                    })
                    .collect();
                defeats.sort_by(|a, b| a.monster_id.cmp(&b.monster_id));
                EncounterRoundSummary {
                    encounter_id: c.id.clone(),
                    round: s.round,
                    completed: s.phase == EncounterPhase::Completed,
                    defeats,
                }
            })
            .collect();
        rows.sort_by(|a, b| a.encounter_id.cmp(&b.encounter_id));
        Some(ResetProgressSummary {
            actual_kills: self.actual_kills(c),
            encounters: rows,
        })
    }
}
