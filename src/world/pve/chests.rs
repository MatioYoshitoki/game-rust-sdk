use super::*;
use crate::config::ChestSpawn;
use crate::world::interaction::LayeredProgress;

#[cfg(all(test, match_sdk_test_data))]
mod sealed_tests;
#[cfg(all(test, match_sdk_test_data))]
mod tests;

pub(super) struct Chest {
    pub(super) id: String,
    pub(super) position: [f64; 2],
    generation: u32,
    pub(super) respawn_at: Option<u64>,
    progress: LayeredProgress,
    pub(super) guardian_slots: Vec<usize>,
    planned_count: Option<u32>,
    guards_ready: bool,
    retry_at: u64,
    blocked_reported: bool,
}
impl Chest {
    pub(super) fn new(spawn: &ChestSpawn) -> Self {
        Self {
            id: spawn.id.clone(),
            position: spawn.position,
            generation: 1,
            respawn_at: None,
            progress: LayeredProgress::default(),
            guardian_slots: vec![],
            planned_count: None,
            guards_ready: false,
            retry_at: 0,
            blocked_reported: false,
        }
    }
}
impl Arena {
    pub(super) fn chest_views(&self) -> Vec<ChestView> {
        self.chests
            .iter()
            .map(|chest| ChestView {
                encounter: self.encounter_gate(&chest.id),
                id: chest.id.clone(),
                type_id: self
                    .grid
                    .config
                    .treasure_rules
                    .as_ref()
                    .unwrap()
                    .template
                    .id
                    .clone(),
                position: chest.position,
                generation: chest.generation,
                opened: chest.respawn_at.is_some(),
                respawn_at_ms: if self
                    .grid
                    .config
                    .map_objective
                    .as_ref()
                    .is_some_and(|o| o.reward_chest_id == chest.id)
                {
                    None
                } else {
                    chest.respawn_at
                },
                seal: self
                    .grid
                    .config
                    .treasure_rules
                    .as_ref()
                    .unwrap()
                    .template
                    .seal
                    .as_ref()
                    .map(|s| SealView {
                        layers: s.layers,
                        completed_layers: chest.progress.completed,
                        layer_progress_ms: chest.progress.elapsed_ms,
                        layer_duration_ms: s.layer_duration_ms,
                        unlocking: chest.progress.active_since.is_some(),
                        interruption_reason: chest.progress.interruption,
                    }),
                guardians: self.grid.config.guardian().map(|_| GuardianView {
                    pending: !chest.guards_ready,
                    planned_count: chest.planned_count,
                    alive: chest
                        .guardian_slots
                        .iter()
                        .filter(|&&i| self.monsters[i].hp > 0)
                        .count() as u32,
                    monster_ids: chest
                        .guardian_slots
                        .iter()
                        .filter(|&&i| self.monsters[i].enabled)
                        .map(|&i| self.id(i))
                        .collect(),
                }),
            })
            .collect()
    }
    pub(super) fn refresh_chests(&mut self, now: u64, events: &mut Vec<Event>) {
        let replacing: Vec<_> = self
            .chests
            .iter()
            .filter(|chest| {
                chest.respawn_at.is_some_and(|at| now >= at)
                    && self
                        .grid
                        .config
                        .map_objective
                        .as_ref()
                        .is_none_or(|o| o.reward_chest_id != chest.id)
            })
            .flat_map(|ch| ch.guardian_slots.iter().copied())
            .collect();
        for index in replacing {
            self.clear_status_entity(
                &self.status_entity(index),
                StatusRemovalReason::Reset,
                events,
            );
            self.cancel_supports_for(index, SupportCancelReason::Replaced, now, events);
            self.cancel_heavy_for(index, HeavyCancelReason::Replaced, now, events);
        }
        for chest in &mut self.chests {
            if self
                .grid
                .config
                .map_objective
                .as_ref()
                .is_some_and(|o| o.reward_chest_id == chest.id)
            {
                continue;
            }
            if chest.respawn_at.is_some_and(|at| now >= at) {
                // Match duration bounds the number of generations well below u32::MAX.
                chest.generation += 1;
                chest.respawn_at = None;
                chest.progress = LayeredProgress::default();
                chest.planned_count = None;
                chest.guards_ready = false;
                chest.retry_at = 0;
                chest.blocked_reported = false;
                for &i in &chest.guardian_slots {
                    let m = &mut self.monsters[i];
                    if let Some(cast) = m.remove_area(now) {
                        events.push(Event::AreaCancelled {
                            casts: vec![cast],
                            reason: AreaCancelReason::Replaced,
                        });
                    }
                    m.hp = 0;
                    m.enabled = false;
                    m.casting = None;
                    m.heal = None;
                    m.path.clear();
                }
                if self
                    .locked
                    .is_some_and(|i| chest.guardian_slots.contains(&i))
                {
                    self.locked = None;
                }
                self.shots.retain(|shot| {
                    shot.owner
                        .is_none_or(|owner| !chest.guardian_slots.contains(&owner))
                });
                events.push(Event::ChestRespawned {
                    chest_id: chest.id.clone(),
                    generation: chest.generation,
                });
            }
        }
    }
    pub(super) fn prepare_guardians(&mut self, w: &mut World, now: u64, events: &mut Vec<Event>) {
        let Some(g) = self.grid.config.guardian().cloned() else {
            return;
        };
        for index in 0..self.chests.len() {
            if self.chests[index].guards_ready || now < self.chests[index].retry_at {
                continue;
            }
            if self.chests[index].planned_count.is_none() {
                let generated = g.chance >= 1.0
                    || g.chance > 0.0
                        && f64::from(w.random()) / (f64::from(u32::MAX) + 1.0) < g.chance;
                let count = if generated {
                    g.min_count
                        + if g.min_count == g.max_count {
                            0
                        } else {
                            w.random() % (g.max_count - g.min_count + 1)
                        }
                } else {
                    0
                };
                self.chests[index].planned_count = Some(count);
            }
            let chest = &self.chests[index];
            let count = chest.planned_count.unwrap() as usize;
            let origin = chest.position;
            let slots = chest.guardian_slots.clone();
            let profile = self.monsters[slots[0]].profile.clone();
            let radius = profile.combat.radius;
            let mut positions = vec![];
            for offset in &g.spawn_offsets {
                if positions.len() == count {
                    break;
                }
                let p = [origin[0] + offset[0], origin[1] + offset[1]];
                if self.grid.line(origin, p, radius, 0)
                    && distance(p, w.position)
                        >= radius + self.grid.config.prepared.as_ref().unwrap().player_radius
                    && self.monsters.iter().all(|m| {
                        m.hp == 0 || distance(p, m.position) >= radius + m.profile.combat.radius
                    })
                    && positions
                        .iter()
                        .all(|other| distance(p, *other) >= radius * 2.0)
                {
                    positions.push(p);
                }
            }
            let chest = &mut self.chests[index];
            if positions.len() != count {
                chest.retry_at = now
                    + self
                        .grid
                        .config
                        .prepared
                        .as_ref()
                        .unwrap()
                        .runtime
                        .ai_decision_ms;
                if !chest.blocked_reported {
                    events.push(Event::GuardianSpawnBlocked {
                        chest_id: chest.id.clone(),
                        generation: chest.generation,
                    });
                    chest.blocked_reported = true;
                }
                continue;
            }
            for (n, p) in positions.into_iter().enumerate() {
                self.monsters[slots[n]] = Monster::new(
                    format!("guardian-{}-g{}-{}", chest.id, chest.generation, n + 1),
                    g.template_id.clone(),
                    profile.clone(),
                    p,
                    Some(GuardianBinding {
                        chest_id: chest.id.clone(),
                        generation: chest.generation,
                    }),
                );
            }
            chest.guards_ready = true;
            let monster_ids = slots
                .iter()
                .filter(|&&i| self.monsters[i].enabled)
                .map(|&i| self.monsters[i].id.clone())
                .collect();
            events.push(Event::GuardiansReady {
                chest_id: chest.id.clone(),
                generation: chest.generation,
                monster_ids,
            });
        }
    }
    pub(crate) fn has_unlock(&self) -> bool {
        self.attune_finishes.is_some()
            || self
                .chests
                .iter()
                .any(|chest| chest.progress.active_since.is_some())
    }
    pub(crate) fn interrupt_unlock(
        &mut self,
        reason: ChestInterruptReason,
        events: &mut Vec<Event>,
    ) {
        self.interrupt_attunement(reason, events);
        let Some(seal) = self
            .grid
            .config
            .treasure_rules
            .as_ref()
            .and_then(|r| r.template.seal.as_ref())
        else {
            return;
        };
        for chest in &mut self.chests {
            if chest.progress.interrupt(seal, reason) {
                events.push(Event::ChestUnlockInterrupted {
                    chest_id: chest.id.clone(),
                    generation: chest.generation,
                    reason,
                });
            }
        }
    }
    pub(super) fn cancel_unlock(
        &mut self,
        id: &str,
        generation: u32,
        events: &mut Vec<Event>,
    ) -> Result<()> {
        if self
            .grid
            .config
            .treasure_rules
            .as_ref()
            .and_then(|r| r.template.seal.as_ref())
            .is_none()
        {
            return Err(Error::Unsupported);
        }
        let chest = self
            .chests
            .iter()
            .find(|chest| chest.id == id)
            .ok_or(Error::NoResource)?;
        if chest.generation != generation {
            return Err(Error::RevisionConflict);
        }
        if chest.progress.active_since.is_none() {
            return Err(Error::NoResource);
        }
        self.interrupt_unlock(ChestInterruptReason::Cancelled, events);
        Ok(())
    }
    pub(super) fn advance_unlock(&mut self, w: &World, now: u64, events: &mut Vec<Event>) {
        let Some(index) = self
            .chests
            .iter()
            .position(|chest| chest.progress.active_since.is_some())
        else {
            return;
        };
        if w.hp == 0 || w.desired_outcome.is_some() {
            self.interrupt_unlock(ChestInterruptReason::Closed, events);
            return;
        }
        if w.protected || !w.ready || !w.foreground {
            self.interrupt_unlock(ChestInterruptReason::Background, events);
            return;
        }
        let rules = self.grid.config.treasure_rules.as_ref().unwrap();
        if distance(w.position, self.chests[index].position) > rules.interaction_range
            || !self
                .grid
                .line(w.position, self.chests[index].position, 0.0, 0)
        {
            self.interrupt_unlock(ChestInterruptReason::OutOfRange, events);
            return;
        }
        let seal = rules.template.seal.as_ref().unwrap();
        let chest = &mut self.chests[index];
        for previous in chest.progress.advance(now, seal) {
            events.push(Event::ChestLayerCompleted {
                chest_id: chest.id.clone(),
                generation: chest.generation,
                completed_layers: previous + 1,
            });
            if previous + 1 == seal.layers {
                events.push(Event::ChestUnlocked {
                    chest_id: chest.id.clone(),
                    generation: chest.generation,
                });
            }
        }
    }
    pub(super) fn open_chest(
        &mut self,
        w: &mut World,
        id: &str,
        generation: u32,
        now: u64,
        events: &mut Vec<Event>,
    ) -> Result<()> {
        let rules = self
            .grid
            .config
            .treasure_rules
            .as_ref()
            .ok_or(Error::Unsupported)?;
        let index = self
            .chests
            .iter()
            .position(|chest| chest.id == id)
            .ok_or(Error::NoResource)?;
        let chest = &self.chests[index];
        if generation != chest.generation {
            return Err(Error::RevisionConflict);
        }
        if chest.respawn_at.is_some() {
            return Err(Error::NoResource);
        }
        if self
            .grid
            .config
            .map_objective
            .as_ref()
            .is_some_and(|o| o.reward_chest_id == id)
            && !self.attuned
        {
            return Err(Error::EncounterLocked);
        }
        if self.encounter_gate(id).is_some_and(|gate| !gate.completed) {
            return Err(Error::EncounterLocked);
        }
        if rules.template.guardian.is_some() {
            if !chest.guards_ready {
                return Err(Error::GuardianPending);
            }
            if chest
                .guardian_slots
                .iter()
                .any(|&i| self.monsters[i].hp > 0)
            {
                return Err(Error::GuardianAlive);
            }
        }
        let origin = chest.position;
        if distance(w.position, origin) > rules.interaction_range
            || !self.grid.line(w.position, origin, 0.0, 0)
        {
            return Err(Error::OutOfRange);
        }
        if self.has_unlock() {
            return Err(Error::InvalidState);
        }
        if let Some(seal) = &rules.template.seal
            && chest.progress.completed < seal.layers
        {
            self.chests[index].progress.start(now);
            self.player_path.clear();
            w.movement = [0.0, 0.0];
            events.push(Event::ChestUnlockStarted {
                chest_id: id.into(),
                generation,
            });
            return Ok(());
        }
        // Prepare all drops before committing any world changes. Failure restores RNG as well.
        let p = self.grid.config.prepared.as_ref().unwrap();
        let old_rng = w.rng;
        let mut drops = Vec::new();
        for (slot, (rule, guaranteed)) in rules
            .loot
            .guaranteed
            .iter()
            .map(|r| (r, true))
            .chain(rules.loot.random.iter().map(|r| (r, false)))
            .enumerate()
        {
            if !guaranteed {
                let chance = rule.chance.unwrap();
                if chance <= 0.0
                    || (chance < 1.0
                        && f64::from(w.random()) / (f64::from(u32::MAX) + 1.0) >= chance)
                {
                    continue;
                }
            }
            let quantity =
                rule.quantity_min + w.random() % (rule.quantity_max - rule.quantity_min + 1);
            if quantity == 0 {
                continue;
            }
            let angle = f64::from(w.random()) / (f64::from(u32::MAX) + 1.0) * std::f64::consts::TAU;
            let radius = f64::from(
                p.drop_min_radius + w.random() % (p.drop_max_radius - p.drop_min_radius + 1),
            );
            let candidate = [
                origin[0] + angle.cos() * radius,
                origin[1] + angle.sin() * radius,
            ];
            let position = if self.grid.line(origin, candidate, p.player_radius, 0) {
                candidate
            } else {
                origin
            };
            if !self.grid.clear(position, p.player_radius, 0) {
                w.rng = old_rng;
                return Err(Error::OutOfRange);
            }
            let resource_id = format!("chest-{id}-g{generation}-drop-{slot}");
            drops.push((resource_id, rule.item_id.clone(), position, quantity));
        }
        if w.resources.len() + drops.len() > rules.ground_capacity {
            w.rng = old_rng;
            return Err(Error::Capacity);
        }
        self.chests[index].respawn_at = Some(now + rules.template.respawn_delay_ms);
        self.player_path.clear();
        w.movement = [0.0, 0.0];
        let resource_ids = drops.iter().map(|drop| drop.0.clone()).collect();
        for (id, item, position, quantity) in drops {
            self.resource_expiry
                .insert(id.clone(), now + p.resource_ttl_ms);
            self.resource_items.insert(id.clone(), item);
            w.resources.push(Resource {
                view: ResourceView {
                    id,
                    position,
                    quantity,
                    orb: false,
                },
                hp: 0,
                spirit: 0,
            });
        }
        events.push(Event::ChestOpened {
            chest_id: id.into(),
            generation,
            resource_ids,
        });
        Ok(())
    }
}
