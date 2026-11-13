use super::*;

pub(super) fn participant() -> StatusEntity {
    StatusEntity {
        kind: StatusEntityKind::Participant,
        id: "participant".into(),
        generation: 1,
        encounter_round: None,
    }
}
impl Arena {
    pub(super) fn status_entity(&self, i: usize) -> StatusEntity {
        let m = &self.monsters[i];
        StatusEntity {
            kind: StatusEntityKind::Monster,
            id: m.id.clone(),
            generation: m.status_generation,
            encounter_round: m.encounter_index.map(|j| self.encounters[j].round),
        }
    }
    pub(super) fn status_source_valid(&self, w: &World, entity: &StatusEntity) -> bool {
        if entity.kind == StatusEntityKind::Participant {
            return w.hp > 0 && *entity == participant();
        }
        self.status_monster_index(entity)
            .is_some_and(|i| self.monsters[i].enabled && self.monsters[i].hp > 0)
    }
    pub(super) fn status_monster_index(&self, entity: &StatusEntity) -> Option<usize> {
        if entity.kind != StatusEntityKind::Monster {
            return None;
        }
        self.monsters.iter().position(|m| {
            m.id == entity.id
                && m.status_generation == entity.generation
                && m.encounter_index.map(|i| self.encounters[i].round) == entity.encounter_round
        })
    }

    pub(super) fn prepare_statuses(&mut self, w: &World, now: u64, events: &mut Vec<Event>) {
        self.now = now;
        self.statuses.expire(now, events);
        if self.statuses.is_empty() {
            return;
        }
        let mut live: Vec<_> = self
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| m.enabled && m.hp > 0)
            .map(|(i, _)| self.status_entity(i))
            .collect();
        if w.hp > 0 {
            live.push(participant());
        }
        self.statuses.remove(
            StatusRemovalReason::TargetInvalid,
            |s| !live.contains(&s.target),
            events,
        );
        self.statuses.remove(
            StatusRemovalReason::SourceInvalid,
            |s| !live.contains(&s.source),
            events,
        );
    }
    pub(super) fn clear_status_entity(
        &mut self,
        entity: &StatusEntity,
        reason: StatusRemovalReason,
        events: &mut Vec<Event>,
    ) {
        self.statuses
            .remove(reason, |s| s.target == *entity, events);
        self.statuses.remove(
            if reason == StatusRemovalReason::Reset {
                reason
            } else {
                StatusRemovalReason::SourceInvalid
            },
            |s| s.source == *entity,
            events,
        );
    }
    pub(crate) fn clear_statuses(&mut self, events: &mut Vec<Event>) {
        self.statuses
            .remove(StatusRemovalReason::Closed, |_| true, events);
    }
    fn definitions(&self, ids: &[String]) -> Vec<crate::config::StatusDefinition> {
        ids.iter()
            .map(|id| {
                self.grid
                    .config
                    .status_definitions
                    .iter()
                    .find(|d| &d.id == id)
                    .expect("validated status binding")
                    .clone()
            })
            .collect()
    }
    pub(super) fn commit_statuses(
        &mut self,
        w: &World,
        ids: &[String],
        source: &StatusEntity,
        now: u64,
        events: &mut Vec<Event>,
    ) -> bool {
        if ids.is_empty() {
            return true;
        }
        let defs = self.definitions(ids);
        if defs.iter().any(|d| d.is_heal())
            && (w.desired_outcome.is_some() || !self.status_source_valid(w, source))
        {
            events.push(Event::StatusRejected {
                source: source.clone(),
                target: source.clone(),
                definition_ids: ids.to_vec(),
                reason: Error::InvalidState,
            });
            return false;
        }

        let max_hp = self.status_max_hp(w, source);
        let attack = self.status_attack(w, source, now);
        self.statuses.apply(
            &defs,
            (source, attack),
            (source, max_hp),
            now,
            self.grid.config.status_rules.as_ref().unwrap(),
            events,
        )
    }
    pub(super) fn hit_statuses(
        &mut self,
        w: &World,
        ids: &[String],
        source: &StatusEntity,
        target: &StatusEntity,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        if ids.is_empty()
            || w.desired_outcome.is_some()
            || !self.status_source_valid(w, source)
            || !self.status_source_valid(w, target)
        {
            return;
        }
        let defs = self.definitions(ids);
        if defs
            .iter()
            .any(|d| d.is_heal() || source.kind == target.kind && d.is_periodic())
        {
            events.push(Event::StatusRejected {
                source: source.clone(),
                target: target.clone(),
                definition_ids: ids.to_vec(),
                reason: Error::InvalidState,
            });
            return;
        }
        let max_hp = self.status_max_hp(w, target);
        let attack = self.status_attack(w, source, now);
        self.statuses.apply(
            &defs,
            (source, attack),
            (target, max_hp),
            now,
            self.grid.config.status_rules.as_ref().unwrap(),
            events,
        );
    }
    fn status_attack(&self, w: &World, source: &StatusEntity, now: u64) -> u32 {
        if source.kind == StatusEntityKind::Participant {
            return self.player_attack(w, now);
        }
        self.status_monster_index(source)
            .map_or(0, |i| self.monster_attack(i, now))
    }
    fn status_max_hp(&self, w: &World, target: &StatusEntity) -> u32 {
        if target.kind == StatusEntityKind::Participant {
            return w.facts.hp_points;
        }
        self.status_monster_index(target)
            .map_or(0, |i| self.monsters[i].profile.stats["hp"])
    }

    pub(super) fn player_defense(&self, w: &World, now: u64) -> u32 {
        self.statuses.defense(
            &participant(),
            w.facts.def_points,
            now,
            self.grid.config.status_rules.as_ref(),
        )
    }
    pub(super) fn player_attack(&self, w: &World, now: u64) -> u32 {
        self.statuses.attribute(
            &participant(),
            w.facts.atk_points,
            now,
            self.grid.config.status_rules.as_ref(),
            StatusAttribute::Atk,
        )
    }
    pub(super) fn monster_attack(&self, i: usize, now: u64) -> u32 {
        self.statuses.attribute(
            &self.status_entity(i),
            self.monsters[i].profile.stats["atk"],
            now,
            self.grid.config.status_rules.as_ref(),
            StatusAttribute::Atk,
        )
    }
    pub(super) fn monster_defense(&self, i: usize, now: u64) -> u32 {
        self.statuses.defense(
            &self.status_entity(i),
            self.monsters[i].profile.stats["def"],
            now,
            self.grid.config.status_rules.as_ref(),
        )
    }
}
