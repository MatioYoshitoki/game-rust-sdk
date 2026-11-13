use super::*;
use crate::config::EncounterConfig;
impl Arena {
    pub(super) fn blocked_by(&self, c: &EncounterConfig) -> Vec<String> {
        c.requires
            .iter()
            .filter(|id| {
                self.grid
                    .config
                    .encounters
                    .iter()
                    .position(|e| &e.id == *id)
                    .is_some_and(|i| self.encounters[i].phase != EncounterPhase::Completed)
            })
            .cloned()
            .collect()
    }
    fn completed_encounters(&self) -> Vec<String> {
        let mut ids: Vec<_> = self
            .grid
            .config
            .encounters
            .iter()
            .zip(&self.encounters)
            .filter(|(_, s)| s.phase == EncounterPhase::Completed)
            .map(|(c, _)| c.id.clone())
            .collect();
        ids.sort();
        ids
    }
    pub(super) fn encounter_completed(&self, id: &str) -> bool {
        self.grid
            .config
            .encounters
            .iter()
            .zip(&self.encounters)
            .any(|(config, state)| config.id == id && state.phase == EncounterPhase::Completed)
    }

    pub(super) fn after_encounter_completed<T>(&self, id: &str, value: T) -> Option<T> {
        self.encounter_completed(id).then_some(value)
    }

    pub(super) fn completed_value<T: Copy>(
        &self,
        rule: Option<&crate::config::AfterEncounter<T>>,
    ) -> Option<T> {
        let rule = rule?;
        self.after_encounter_completed(&rule.encounter_id, rule.value)
    }

    fn objective_available(&self) -> bool {
        self.grid.config.map_objective.as_ref().is_some_and(|o| {
            o.required_encounter_ids
                .iter()
                .all(|id| self.encounter_completed(id))
        })
    }
    pub(super) fn consumed(&self) -> Vec<ItemAmount> {
        self.grid
            .config
            .map_objective
            .as_ref()
            .filter(|o| self.attuned && o.cost.quantity > 0)
            .map(|o| vec![o.cost.clone()])
            .unwrap_or_default()
    }
    fn reward_opened(&self) -> bool {
        self.grid.config.map_objective.as_ref().is_some_and(|o| {
            self.chests
                .iter()
                .any(|ch| ch.id == o.reward_chest_id && ch.respawn_at.is_some())
        })
    }
    pub(super) fn map_summary(&self) -> Option<MapProgressSummary> {
        self.grid
            .config
            .map_objective
            .as_ref()
            .map(|_| MapProgressSummary {
                completed_encounter_ids: self.completed_encounters(),
                attuned: self.attuned,
                reward_opened: self.reward_opened(),
                consumed: self.consumed(),
            })
    }
    pub(super) fn objective_view(&self) -> Option<MapObjectiveView> {
        self.grid
            .config
            .map_objective
            .as_ref()
            .map(|o| MapObjectiveView {
                id: o.id.clone(),
                name: o.name.clone(),
                required_encounter_ids: o.required_encounter_ids.clone(),
                completed_encounter_ids: self.completed_encounters(),
                reward_chest_id: o.reward_chest_id.clone(),
                available: self.objective_available(),
                attuned: self.attuned,
                reward_opened: self.reward_opened(),
                finishes_at_ms: self.attune_finishes,
                duration_ms: o.attune_duration_ms,
                cost: o.cost.clone(),
                interruption_reason: self.attune_interruption,
            })
    }
    pub(super) fn interrupt_attunement(
        &mut self,
        reason: ChestInterruptReason,
        events: &mut Vec<Event>,
    ) {
        if self.attune_finishes.take().is_some() {
            self.attune_interruption = Some(reason);
            events.push(Event::AttunementInterrupted {
                objective_id: self.grid.config.map_objective.as_ref().unwrap().id.clone(),
                reason,
            });
        }
    }
    fn attune_position_valid(&self, w: &World) -> bool {
        let o = self.grid.config.map_objective.as_ref().unwrap();
        let position = self
            .chests
            .iter()
            .find(|ch| ch.id == o.reward_chest_id)
            .unwrap()
            .position;
        distance(w.position, position)
            <= self
                .grid
                .config
                .treasure_rules
                .as_ref()
                .unwrap()
                .interaction_range
            && self.grid.line(w.position, position, 0.0, 0)
    }
    pub(super) fn start_attunement(
        &mut self,
        w: &mut World,
        c: &Config,
        id: &str,
        revision: u32,
        now: u64,
        events: &mut Vec<Event>,
    ) -> Result<()> {
        let o = self
            .grid
            .config
            .map_objective
            .as_ref()
            .ok_or(Error::Unsupported)?;
        if o.id != id {
            return Err(Error::NoResource);
        }
        if revision != w.inventory_revision {
            return Err(Error::RevisionConflict);
        }
        if self.attuned || self.has_unlock() {
            return Err(Error::InvalidState);
        }
        if !self.objective_available() {
            return Err(Error::EncounterLocked);
        }
        if !self.attune_position_valid(w) {
            return Err(Error::OutOfRange);
        }
        let quantity = if o.cost.item_id == c.item_id {
            w.inventory
        } else {
            self.items.get(&o.cost.item_id).copied().unwrap_or(0)
        };
        if quantity < o.cost.quantity {
            return Err(Error::NoResource);
        }
        let until = now + o.attune_duration_ms;
        let id = o.id.clone();
        self.attune_finishes = Some(until);
        self.attune_interruption = None;
        self.player_path.clear();
        w.movement = [0.0, 0.0];
        events.push(Event::AttunementStarted {
            objective_id: id,
            finishes_at_ms: until,
        });
        Ok(())
    }
    pub(super) fn advance_attunement(
        &mut self,
        w: &mut World,
        c: &Config,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let Some(until) = self.attune_finishes else {
            return;
        };
        let reason = if w.hp == 0 || w.desired_outcome.is_some() {
            Some(ChestInterruptReason::Closed)
        } else if w.protected || !w.ready || !w.foreground {
            Some(ChestInterruptReason::Background)
        } else if !self.attune_position_valid(w) {
            Some(ChestInterruptReason::OutOfRange)
        } else {
            None
        };
        if let Some(reason) = reason {
            self.interrupt_attunement(reason, events);
            return;
        }
        if now < until {
            return;
        }
        let o = self.grid.config.map_objective.as_ref().unwrap();
        let quantity = if o.cost.item_id == c.item_id {
            &mut w.inventory
        } else {
            self.items.entry(o.cost.item_id.clone()).or_default()
        };
        if *quantity < o.cost.quantity {
            self.interrupt_attunement(ChestInterruptReason::Cancelled, events);
            return;
        }
        *quantity -= o.cost.quantity;
        w.inventory_revision += 1;
        self.attune_finishes = None;
        self.attuned = true;
        events.push(Event::AttunementCompleted {
            objective_id: o.id.clone(),
            consumed: self.consumed(),
        });
    }
}
