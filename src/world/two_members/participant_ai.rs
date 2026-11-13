use super::*;
use crate::config::participant_ai::ParticipantAi;

#[derive(Default)]
pub(super) struct ParticipantAiState {
    decide_at: u64,
    search_index: usize,
    loot_offset: usize,
    withdrawing: bool,
}

impl World {
    pub(super) fn advance_participant_ai(&mut self, c: &Config, now: u64, events: &mut Events) {
        if c.participant_ai.is_empty() {
            return;
        }
        for i in 0..self.members.len() {
            let m = &self.members[i];
            if !m.eligible()
                || m.pending.is_some()
                || m.channel.is_some()
                || m.extract_end.is_some()
                || m.pending_boon_claim()
                || self.has_container_reservation(i)
                || self.member_has_status(i, StatusAttribute::Stun, now)
                || m.ai.as_ref().is_none_or(|state| now < state.decide_at)
            {
                continue;
            }
            let policy = &c.participant_ai[&m.reference.entity.id];
            self.members[i].ai.as_mut().unwrap().decide_at = now + policy.decision_interval_ms;
            if let Some(action) = self.participant_ai_action(i, policy, c, now) {
                let id = self.next_id("action");
                if let Ok(emitted) = self.action(i, &action, &id, c, now) {
                    events.extend(emitted);
                }
            }
        }
    }

    fn participant_ai_action(
        &mut self,
        i: usize,
        policy: &ParticipantAi,
        c: &Config,
        now: u64,
    ) -> Option<Action> {
        let m = &self.members[i];
        let max_hp = self.entity_max_hp(&m.reference.entity, c, now).unwrap();
        let looted =
            m.looted.values().copied().map(u64::from).sum::<u64>() + m.instances.len() as u64;
        let remaining = c
            .terminal_ms
            .saturating_sub(now.saturating_sub(self.active_at.unwrap()));
        let enemy = self
            .npcs
            .iter()
            .enumerate()
            .filter(|(j, _)| self.npc_can_be_attacked(*j))
            .map(|(_, n)| (n, distance(m.position, n.position)))
            .filter(|(_, d)| policy.threat_radius > 0. && *d <= policy.threat_radius)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.entity.id.cmp(&b.0.entity.id)));
        let skill = policy
            .combat_skill_id
            .as_deref()
            .filter(|id| m.skill_equipped(id, c))
            .and_then(|id| c.player_skill(id))
            .filter(|skill| m.spirit >= m.effective_spirit_cost(skill.cost));
        if m.ai.as_ref().unwrap().withdrawing
            || u64::from(m.hp) * 10000 <= u64::from(max_hp) * u64::from(policy.withdraw_hp_bps)
            || policy.extract_after_looted > 0 && looted >= u64::from(policy.extract_after_looted)
            || remaining <= policy.extract_before_timeout_ms
            || enemy.is_some() && skill.is_none()
        {
            let next = if distance(m.position, c.extraction_point) <= c.params.extraction_range {
                if now >= self.active_at.unwrap() + c.extraction_unlock_ms {
                    Some(Action::ExtractStart {})
                } else {
                    stop_if_moving(m)
                }
            } else {
                self.navigate_if_changed(m, c.extraction_point, c)
            };
            self.members[i].ai.as_mut().unwrap().withdrawing = true;
            return next;
        }
        if let Some(((enemy, d), skill)) = enemy.zip(skill) {
            if d > skill.range || !self.grid.line(m.position, enemy.position, 0., 2) {
                return self.navigate_if_changed(m, enemy.position, c);
            }
            if let Some(stop) = stop_if_moving(m) {
                return Some(stop);
            }
            if now < m.recovery_until
                || m.cooldowns.get(&skill.id).is_some_and(|ready| now < *ready)
            {
                return None;
            }
            return Some(Action::CastUnit {
                skill_id: skill.id.clone(),
                target: UnitTarget {
                    id: enemy.entity.id.clone(),
                    generation: enemy.entity.generation,
                },
            });
        }
        if let Some(action) = self.participant_ai_boon(i, c, policy.loot_radius, now) {
            return Some(action);
        }
        if let Some(action) = self.participant_ai_relic(i, c, policy.loot_radius) {
            return Some(action);
        }
        if let Some(resource) = self.participant_ai_loot(i, policy, c, now) {
            let action = if distance(m.position, resource.view.position) <= c.params.pickup_range {
                Some(Action::Pickup {
                    resource_id: resource.view.id.clone(),
                    expected_revision: m.revision,
                })
            } else {
                self.navigate_if_changed(m, resource.view.position, c)
            };
            self.members[i].ai.as_mut().unwrap().loot_offset = 0;
            return action;
        }
        if policy.loot_radius > 0.
            && let Some((container, generation)) = self.container_for_ai(i, policy.loot_radius, c)
        {
            if distance(m.position, container.position)
                <= c.world_loot.as_ref().unwrap().interaction_range
                && self
                    .grid
                    .line(m.position, container.position, c.params.player_radius, 0)
            {
                return Some(Action::OpenChest {
                    chest_id: container.id.clone(),
                    expected_generation: generation,
                });
            }
            if self.can_navigate_to(m, container.position, c) {
                return self.navigate_if_changed(m, container.position, c);
            }
        }
        let next_loot_offset = m.ai.as_ref().unwrap().loot_offset + 4;
        let action = self.participant_ai_search(i, policy, c);
        self.members[i].ai.as_mut().unwrap().loot_offset = next_loot_offset;
        action
    }

    fn participant_ai_search(
        &mut self,
        i: usize,
        policy: &ParticipantAi,
        c: &Config,
    ) -> Option<Action> {
        let m = &self.members[i];
        if policy.search_points.is_empty() {
            return stop_if_moving(m);
        }
        let start = m.ai.as_ref().unwrap().search_index;
        for offset in 0..policy.search_points.len().min(4) {
            let index = (start + offset) % policy.search_points.len();
            let point = policy.search_points[index];
            if distance(m.position, point) > 1e-6 && self.can_navigate_to(m, point, c) {
                let next = self.navigate_if_changed(m, point, c);
                self.members[i].ai.as_mut().unwrap().search_index = index;
                return next;
            }
        }
        let stop = stop_if_moving(m);
        self.members[i].ai.as_mut().unwrap().search_index =
            (start + 4) % policy.search_points.len();
        stop
    }

    fn participant_ai_loot(
        &self,
        i: usize,
        policy: &ParticipantAi,
        c: &Config,
        now: u64,
    ) -> Option<&Resource> {
        if policy.loot_radius == 0. {
            return None;
        }
        let m = &self.members[i];
        let mut candidates: Vec<_> = self
            .resources
            .iter()
            .filter(|r| {
                r.view.item_id.is_some()
                    && r.owner_at(now)
                        .is_none_or(|owner| *owner == m.reference.entity)
            })
            .map(|r| (r, distance(m.position, r.view.position)))
            .filter(|(r, d)| {
                let capacity = self.stack_credit_capacity(i, r.view.item_id.as_deref().unwrap(), c);
                *d <= policy.loot_radius
                    && capacity > 0
                    && (capacity >= r.view.quantity
                        || c.world_loot.as_ref().is_some_and(|w| w.partial_pickup))
            })
            .collect();
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.view.id.cmp(&b.0.view.id)));
        if candidates.is_empty() {
            return None;
        }
        let offset = m.ai.as_ref().unwrap().loot_offset % candidates.len();
        candidates
            .iter()
            .cycle()
            .skip(offset)
            .take(candidates.len().min(4))
            .find_map(|&(r, d)| {
                let reachable = if d <= c.params.pickup_range {
                    !r.requires_reachable_pickup()
                        || self
                            .grid
                            .line(m.position, r.view.position, c.params.player_radius, 0)
                } else {
                    self.can_navigate_to(m, r.view.position, c)
                };
                reachable.then_some(r)
            })
    }

    fn can_navigate_to(&self, m: &Member, point: [f64; 2], c: &Config) -> bool {
        self.follows_route(m, point, c)
            || self.grid.clear(point, c.params.player_radius, 0)
                && !self
                    .grid
                    .route(m.position, point, c.params.player_radius)
                    .is_empty()
    }

    fn follows_route(&self, m: &Member, destination: [f64; 2], c: &Config) -> bool {
        m.path.last() == Some(&destination)
            && m.path.first().is_some_and(|point| {
                self.grid
                    .line(m.position, *point, c.params.player_radius, 0)
            })
    }

    fn navigate_if_changed(&self, m: &Member, destination: [f64; 2], c: &Config) -> Option<Action> {
        (!self.follows_route(m, destination, c)).then_some(Action::Navigate { destination })
    }
}

fn stop_if_moving(m: &Member) -> Option<Action> {
    (!m.path.is_empty() || m.movement != [0., 0.]).then_some(Action::Stop {})
}
