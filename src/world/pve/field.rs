//! Fixed-position periodic contacts, independently owned by the committed ground plan.
use super::*;
pub(super) struct PlayerField {
    pub area: ground::PlayerArea,
    pub(super) contact: Option<crate::contact::ContactBudget<(String, u32)>>,
    expires: u64,
    next_tick: u64,
    interval: u64,
    completed: u32,
    total: u32,
}
impl PlayerField {
    pub(super) fn new(area: ground::PlayerArea, duration: u64, interval: u64) -> Self {
        Self {
            contact: None,
            expires: area.warning.resolves_at_ms + duration,
            next_tick: area.warning.resolves_at_ms + interval,
            interval,
            completed: 0,
            total: ((duration - 1) / interval) as u32,
            area,
        }
    }
    pub(super) fn view(&self) -> PlayerFieldView {
        let a = &self.area.warning;
        PlayerFieldView {
            line: self.area.line.clone(),
            contact_policy: self.contact.as_ref().map(|b| FieldContactPolicyView {
                max_targets_per_tick: b.policy().max_targets_per_tick,
                max_total_hits: b.policy().max_total_hits,
                repeat_interval_ms: b.policy().repeat_interval_ms,
                used_hits: b.used(),
                remaining_hits: b.remaining(),
                contacts: b
                    .history()
                    .map(|((id, generation), last)| FieldContactView {
                        target: UnitTarget {
                            id: id.clone(),
                            generation: *generation,
                        },
                        last_hit_at_ms: *last,
                        next_eligible_at_ms: (b.policy().repeat_interval_ms > 0)
                            .then(|| last.saturating_add(b.policy().repeat_interval_ms)),
                    })
                    .collect(),
            }),
            action_id: a.action_id.clone(),
            skill_id: a.skill_id.clone(),
            center: a.center,
            radius: a.radius,
            started_at_ms: a.started_at_ms,
            activates_at_ms: a.resolves_at_ms,
            expires_at_ms: self.expires,
            next_tick_at_ms: (self.next_tick < self.expires).then_some(self.next_tick),
            completed_ticks: self.completed,
            total_ticks: self.total,
        }
    }
}
impl From<PlayerAreaCancelReason> for PlayerFieldEndReason {
    fn from(r: PlayerAreaCancelReason) -> Self {
        match r {
            PlayerAreaCancelReason::OwnerDied => Self::OwnerDied,
            PlayerAreaCancelReason::Terminal => Self::Terminal,
            PlayerAreaCancelReason::Closed => Self::Closed,
            PlayerAreaCancelReason::EncounterReset => Self::EncounterReset,
        }
    }
}
impl Arena {
    pub(crate) fn has_player_fields(&self) -> bool {
        !self.player_fields.is_empty()
    }
    pub(super) fn ground_plan_count(&self) -> usize {
        self.player_fields.len() + self.player_areas.len()
    }
    pub(super) fn cancel_player_fields(
        &mut self,
        reason: PlayerFieldEndReason,
        events: &mut Vec<Event>,
    ) {
        if !self.player_fields.is_empty() {
            events.push(Event::PlayerFieldEnded {
                action_ids: self
                    .player_fields
                    .drain(..)
                    .map(|f| f.area.warning.action_id)
                    .collect(),
                reason,
            });
        }
    }
    pub(super) fn advance_player_fields(
        &mut self,
        w: &mut World,
        c: &Config,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        // Reinsert in the same slot so ties always retain creation order.
        while let Some(i) = self
            .player_fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.next_tick <= now && f.next_tick < f.expires)
            .min_by_key(|(i, f)| (f.next_tick, *i))
            .map(|(i, _)| i)
        {
            let mut f = self.player_fields.remove(i);
            let scheduled = f.next_tick;
            let hit_targets = if let Some(b) = &mut f.contact {
                Some(self.resolve_policy_contacts(w, c, &f.area, b, (scheduled, now), events))
            } else {
                None
            };
            let hits = hit_targets.as_ref().map_or_else(
                || self.resolve_ground_contact(w, c, &f.area, now, events),
                |v| v.len() as u32,
            );
            f.completed += 1;
            f.next_tick += f.interval;
            events.push(Event::PlayerFieldTick {
                hit_targets,
                remaining_hits: f.contact.as_ref().map(|b| b.remaining()),
                action_id: f.area.warning.action_id.clone(),
                skill_id: f.area.warning.skill_id.clone(),
                tick_index: f.completed,
                scheduled_at_ms: scheduled,
                hits,
            });
            if f.contact.as_ref().is_some_and(|b| b.remaining() == 0) {
                events.push(Event::PlayerFieldEnded {
                    action_ids: vec![f.area.warning.action_id],
                    reason: PlayerFieldEndReason::HitBudgetExhausted,
                });
            } else {
                self.player_fields.insert(i, f);
            }
            if w.hp == 0 {
                self.cancel_player_fields(PlayerFieldEndReason::OwnerDied, events);
                break;
            }
        }
        let mut expired = vec![];
        self.player_fields.retain(|f| {
            if now >= f.expires {
                expired.push(f.area.warning.action_id.clone());
                false
            } else {
                true
            }
        });
        if !expired.is_empty() {
            events.push(Event::PlayerFieldEnded {
                action_ids: expired,
                reason: PlayerFieldEndReason::Expired,
            });
        }
    }
    fn resolve_policy_contacts(
        &mut self,
        w: &mut World,
        c: &Config,
        p: &ground::PlayerArea,
        b: &mut crate::contact::ContactBudget<(String, u32)>,
        time: (u64, u64),
        events: &mut Vec<Event>,
    ) -> Vec<UnitTarget> {
        let (scheduled, now) = time;
        let center = p.warning.center;
        let distance2 = |i: usize| {
            let q = self.monsters[i].position;
            (q[0] - center[0]).powi(2) + (q[1] - center[1]).powi(2)
        };
        let mut candidates = (0..self.monsters.len()).collect::<Vec<_>>();
        candidates.sort_by(|&i, &j| {
            let order = |k: usize| {
                p.line
                    .as_ref()
                    .map(|l| line::order(l, self.monsters[k].position))
            };
            order(i)
                .partial_cmp(&order(j))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    if p.line.is_none() {
                        distance2(i).total_cmp(&distance2(j))
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .then(self.monsters[i].id.cmp(&self.monsters[j].id))
                .then(
                    self.monsters[i]
                        .status_generation
                        .cmp(&self.monsters[j].status_generation),
                )
                .then(i.cmp(&j))
        });
        let mut hits = vec![];
        for i in candidates {
            if b.remaining() == 0 || hits.len() >= b.policy().max_targets_per_tick as usize {
                break;
            }
            if w.hp == 0 {
                break;
            }
            let m = &self.monsters[i];
            let key = (m.id.clone(), m.status_generation);
            if !b.allows(&key, scheduled) {
                continue;
            }
            if self.resolve_ground_contact_target(w, c, p, i, now, events) {
                assert!(b.record(key.clone(), scheduled));
                hits.push(UnitTarget {
                    id: key.0,
                    generation: key.1,
                });
            }
        }
        hits
    }
}
