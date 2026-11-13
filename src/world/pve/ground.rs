//! Bounded player plans; circle geometry is shared with NPC warnings, ownership is explicit.
use super::*;
pub(super) struct PlayerContact<'a> {
    pub raw: u32,
    pub accepted_at: u64,
    pub hit_statuses: &'a [String],
}
pub(super) struct PlayerArea {
    pub line: Option<LineFieldView>,
    pub warning: PlayerAreaWarningView,
    pub rounds: Vec<u32>,
    pub raw: u32,
    pub accepted_at: u64,
    pub hit_statuses: Vec<String>,
}
impl Grid {
    pub(in crate::world) fn circle_contact(
        &self,
        center: [f64; 2],
        radius: f64,
        position: [f64; 2],
        target_radius: f64,
    ) -> bool {
        distance(center, position) <= radius + target_radius && self.line(center, position, 0., 1)
    }
}
impl Arena {
    pub(crate) fn has_player_areas(&self) -> bool {
        !self.player_areas.is_empty()
    }
    pub(super) fn ground_target(&self, w: &World, s: &SkillConfig, p: [f64; 2]) -> Result<()> {
        if !p.iter().all(|v| v.is_finite()) {
            return Err(Error::InvalidMessage);
        }
        if self.grid.index(p).is_none() || !self.grid.clear(p, 0., 0) {
            return Err(Error::InvalidTarget);
        }
        if distance(w.position, p) > s.range || !self.grid.line(w.position, p, 0., 2) {
            return Err(Error::OutOfRange);
        }
        Ok(())
    }
    pub(crate) fn cancel_player_areas(
        &mut self,
        reason: PlayerAreaCancelReason,
        events: &mut Vec<Event>,
    ) {
        if !self.player_areas.is_empty() {
            events.push(Event::PlayerAreaCancelled {
                action_ids: self
                    .player_areas
                    .drain(..)
                    .map(|p| p.warning.action_id)
                    .collect(),
                reason,
            });
        }
        self.cancel_player_fields(reason.into(), events);
        self.cancel_piercing(reason.into(), events);
    }
    pub(super) fn advance_player_areas(
        &mut self,
        w: &mut World,
        c: &Config,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        for p in std::mem::take(&mut self.player_areas) {
            if now < p.warning.resolves_at_ms || now <= p.warning.started_at_ms {
                self.player_areas.push(p);
                continue;
            }
            let hits = self.resolve_ground_contact(w, c, &p, now, events);
            events.push(Event::PlayerAreaResolved {
                action_id: p.warning.action_id,
                skill_id: p.warning.skill_id,
                hits,
            });
        }
    }
    pub(super) fn resolve_ground_contact(
        &mut self,
        w: &mut World,
        c: &Config,
        p: &PlayerArea,
        now: u64,
        events: &mut Vec<Event>,
    ) -> u32 {
        let mut hits = 0;
        for i in 0..self.monsters.len() {
            if self.resolve_ground_contact_target(w, c, p, i, now, events) {
                hits += 1;
            }
        }
        hits
    }
    pub(super) fn resolve_ground_contact_target(
        &mut self,
        w: &mut World,
        c: &Config,
        p: &PlayerArea,
        i: usize,
        now: u64,
        events: &mut Vec<Event>,
    ) -> bool {
        let m = &self.monsters[i];
        if self.unit_reference(i).is_none()
            || !self.accepts_round(m, &p.rounds)
            || !p.line.as_ref().map_or_else(
                || {
                    self.grid.circle_contact(
                        p.warning.center,
                        p.warning.radius,
                        m.position,
                        m.profile.combat.radius,
                    )
                },
                |line| {
                    self.grid
                        .line_field_contact(line, m.position, m.profile.combat.radius)
                },
            )
        {
            return false;
        }
        self.player_direct_contact(
            w,
            c,
            i,
            PlayerContact {
                raw: p.raw,
                accepted_at: p.accepted_at,
                hit_statuses: &p.hit_statuses,
            },
            now,
            events,
        )
    }
    pub(super) fn player_direct_contact(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        hit: PlayerContact<'_>,
        now: u64,
        events: &mut Vec<Event>,
    ) -> bool {
        let start = events.len();
        self.hurt_monster_contact(w, c, i, (hit.raw, true, Some(hit.accepted_at)), events);
        let contacted=events[start..].iter().any(|e|matches!(e,Event::Damage{source,target,..} if source=="participant" && target==&self.monsters[i].id));
        if contacted {
            self.hit_statuses(
                w,
                hit.hit_statuses,
                &status::participant(),
                &self.status_entity(i),
                now,
                events,
            );
        }
        contacted
    }
}
