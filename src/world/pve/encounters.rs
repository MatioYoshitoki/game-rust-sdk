use super::*;

pub(super) struct Encounter {
    pub(super) round: u32,
    pub(super) reset_at: Option<u64>,
    pub(super) defeats: BTreeMap<usize, (u32, u32)>,
    pub(super) phase: EncounterPhase,
    pub(super) slots: Vec<usize>,
    pub(super) groups: Vec<Vec<usize>>,
    pub(super) current_wave: usize,
    pub(super) completed_waves: u32,
    pub(super) ready_at: Option<u64>,
    pub(super) retry_at: u64,
    pub(super) blocked_reported: bool,
}
pub(super) fn reserve(
    c: &PveConfig,
    profiles: &BTreeMap<String, Arc<MonsterTemplate>>,
    monsters: &mut Vec<Monster>,
) -> Vec<Encounter> {
    c.encounters
        .iter()
        .enumerate()
        .map(|(encounter_index, e)| {
            let mut groups = vec![];
            for index in 0..e.group_count() {
                let mut slots = vec![];
                for spawn in e.group(index).0 {
                    let mut m = Monster::new(
                        spawn.id.clone(),
                        spawn.template_id.clone(),
                        profiles[&spawn.template_id].clone(),
                        spawn.position,
                        None,
                    );
                    m.encounter_index = Some(encounter_index);
                    m.encounter_id = Some(e.id.clone());
                    m.wave_id = e.waves.get(index).map(|w| w.id.clone());
                    m.enabled = false;
                    m.hp = 0;
                    slots.push(monsters.len());
                    monsters.push(m);
                }
                groups.push(slots);
            }
            Encounter {
                round: 1,
                reset_at: None,
                defeats: BTreeMap::new(),
                phase: EncounterPhase::Dormant,
                slots: groups.iter().flatten().copied().collect(),
                groups,
                current_wave: 0,
                completed_waves: 0,
                ready_at: None,
                retry_at: 0,
                blocked_reported: false,
            }
        })
        .collect()
}
fn playable(w: &World) -> bool {
    w.phase == Phase::Active
        && w.hp > 0
        && w.ready
        && w.foreground
        && !w.protected
        && w.desired_outcome.is_none()
}
fn defeated(monsters: &[Monster], slots: &[usize], required: &[String]) -> u32 {
    slots
        .iter()
        .filter(|&&i| {
            monsters[i].enabled && monsters[i].hp == 0 && required.contains(&monsters[i].id)
        })
        .count() as u32
}
impl Arena {
    pub(super) fn encounter_gate(&self, chest: &str) -> Option<EncounterChestView> {
        self.grid
            .config
            .encounters
            .iter()
            .enumerate()
            .find(|(_, e)| e.reward_chest_ids.iter().any(|id| id == chest))
            .map(|(i, e)| EncounterChestView {
                encounter_id: e.id.clone(),
                completed: self.encounters[i].phase == EncounterPhase::Completed,
            })
    }
    pub(super) fn encounter_views(&self) -> Vec<EncounterView> {
        self.grid
            .config
            .encounters
            .iter()
            .zip(&self.encounters)
            .map(|(c, s)| {
                let required: Vec<_> = c.all_required().cloned().collect();
                let wave = c.waves.get(s.current_wave).map(|w| EncounterWaveView {
                    id: w.id.clone(),
                    index: s.current_wave as u32 + 1,
                    total: c.waves.len() as u32,
                    completed_waves: s.completed_waves,
                    ready_at_ms: if s.phase == EncounterPhase::Waiting {
                        s.ready_at
                    } else {
                        None
                    },
                    monster_ids: s.groups[s.current_wave]
                        .iter()
                        .filter(|&&i| self.monsters[i].enabled)
                        .map(|&i| self.id(i))
                        .collect(),
                    required_monster_ids: w.required_monster_ids.clone(),
                    defeated_required: defeated(
                        &self.monsters,
                        &s.groups[s.current_wave],
                        &w.required_monster_ids,
                    ),
                });
                EncounterView {
                    reset: c.reset.as_ref().map(|r| EncounterResetView {
                        round: s.round,
                        reset_at_ms: s.reset_at,
                        delay_ms: r.delay_ms,
                        pursuit_radius: r.pursuit_radius,
                        actual_defeats: s.defeats.values().map(|(n, _)| *n).sum(),
                        rewarded_defeats: s.defeats.len() as u32,
                    }),
                    blocked_by: self.blocked_by(c),
                    wave,
                    id: c.id.clone(),
                    phase: s.phase,
                    monster_ids: s
                        .slots
                        .iter()
                        .filter(|&&i| self.monsters[i].enabled)
                        .map(|&i| self.id(i))
                        .collect(),
                    defeated_required: defeated(&self.monsters, &s.slots, &required),
                    required_monster_ids: required,
                    reward_chest_ids: c.reward_chest_ids.clone(),
                }
            })
            .collect()
    }
    pub(super) fn prepare_encounters(&mut self, w: &World, now: u64, events: &mut Vec<Event>) {
        self.now = now;
        if !playable(w) || w.extraction_end.is_some() {
            return;
        }
        let blocked: Vec<_> = self
            .grid
            .config
            .encounters
            .iter()
            .map(|c| !self.blocked_by(c).is_empty())
            .collect();
        for (index, is_blocked) in blocked.into_iter().enumerate() {
            if is_blocked {
                continue;
            }
            let c = &self.grid.config.encounters[index];
            let s = &mut self.encounters[index];
            if s.phase == EncounterPhase::Dormant
                && distance(w.position, c.trigger.center) <= c.trigger.radius
            {
                events.push(Event::EncounterTriggered {
                    encounter_id: c.id.clone(),
                });
                if let Some(first) = c.waves.first() {
                    s.phase = EncounterPhase::Waiting;
                    s.ready_at = Some(now + first.delay_ms);
                    events.push(Event::EncounterWaveScheduled {
                        encounter_id: c.id.clone(),
                        wave_id: first.id.clone(),
                        wave_index: 1,
                        ready_at_ms: s.ready_at.unwrap(),
                    });
                } else {
                    s.phase = EncounterPhase::Spawning;
                }
            }
            if s.reset_at.is_some() {
                continue;
            }
            if s.phase == EncounterPhase::Waiting && s.ready_at.is_some_and(|at| now >= at) {
                s.phase = EncounterPhase::Spawning;
            }
            if s.phase != EncounterPhase::Spawning || now < s.retry_at {
                continue;
            }
            let slots = &s.groups[s.current_wave];
            let allowed = slots.iter().all(|&i| {
                let m = &self.monsters[i];
                let r = m.profile.combat.radius;
                self.grid.clear(m.home, r, 0)
                    && distance(m.home, w.position)
                        >= r + self.grid.config.prepared.as_ref().unwrap().player_radius
                    && self.monsters.iter().all(|other| {
                        other.hp == 0
                            || distance(m.home, other.position) >= r + other.profile.combat.radius
                    })
            });
            if !allowed {
                s.retry_at = now
                    + self
                        .grid
                        .config
                        .prepared
                        .as_ref()
                        .unwrap()
                        .runtime
                        .ai_decision_ms;
                if !s.blocked_reported {
                    s.blocked_reported = true;
                    if let Some(wave) = c.waves.get(s.current_wave) {
                        events.push(Event::EncounterWaveSpawnBlocked {
                            encounter_id: c.id.clone(),
                            wave_id: wave.id.clone(),
                            wave_index: s.current_wave as u32 + 1,
                        });
                    } else {
                        events.push(Event::EncounterSpawnBlocked {
                            encounter_id: c.id.clone(),
                        });
                    }
                }
                continue;
            }
            for &i in slots {
                let m = &mut self.monsters[i];
                m.enabled = true;
                m.hp = m.profile.stats["hp"];
                m.next_attack = now + m.attack_interval();
            }
            s.phase = EncounterPhase::Active;
            let ids: Vec<_> = slots.iter().map(|&i| self.monsters[i].id.clone()).collect();
            if s.current_wave == 0 {
                events.push(Event::EncounterStarted {
                    encounter_id: c.id.clone(),
                    monster_ids: ids.clone(),
                });
            }
            if let Some(wave) = c.waves.get(s.current_wave) {
                events.push(Event::EncounterWaveStarted {
                    encounter_id: c.id.clone(),
                    wave_id: wave.id.clone(),
                    wave_index: s.current_wave as u32 + 1,
                    monster_ids: ids,
                });
            }
        }
    }
    pub(super) fn advance_encounters(&mut self, w: &World, events: &mut Vec<Event>) {
        if !playable(w) {
            return;
        }
        let mut cancelled = vec![];
        for (c, s) in self.grid.config.encounters.iter().zip(&mut self.encounters) {
            let required = c.group(s.current_wave).1;
            if s.phase != EncounterPhase::Active
                || defeated(&self.monsters, &s.groups[s.current_wave], required)
                    != required.len() as u32
            {
                continue;
            }
            if let Some(wave) = c.waves.get(s.current_wave) {
                s.completed_waves += 1;
                events.push(Event::EncounterWaveCompleted {
                    encounter_id: c.id.clone(),
                    wave_id: wave.id.clone(),
                    wave_index: s.current_wave as u32 + 1,
                });
                if s.current_wave + 1 < c.waves.len() {
                    s.current_wave += 1;
                    s.phase = EncounterPhase::Waiting;
                    s.ready_at = Some(self.now + c.waves[s.current_wave].delay_ms);
                    s.retry_at = 0;
                    s.blocked_reported = false;
                    events.push(Event::EncounterWaveScheduled {
                        encounter_id: c.id.clone(),
                        wave_id: c.waves[s.current_wave].id.clone(),
                        wave_index: s.current_wave as u32 + 1,
                        ready_at_ms: s.ready_at.unwrap(),
                    });
                    continue;
                }
            }
            if s.reset_at.take().is_some() {
                cancelled.push(c.id.clone());
            }
            s.phase = EncounterPhase::Completed;
            events.push(Event::EncounterCompleted {
                encounter_id: c.id.clone(),
                reward_chest_ids: c.reward_chest_ids.clone(),
            });
        }
        if !cancelled.is_empty() {
            events.push(Event::EncounterResetCancelled {
                encounter_ids: cancelled,
            });
        }
    }
}
