//! Straight swept flights; identity accounting is shared with fields, geometry stays here.
use super::*;
use crate::contact::ContactBudget;
use ground::PlayerContact;
pub(super) struct PiercingProjectile {
    action_id: String,
    skill_id: String,
    position: [f64; 2],
    direction: [f64; 2],
    radius: f64,
    speed: f64,
    remaining: f64,
    raw: u32,
    accepted_at: u64,
    created: u64,
    expires: u64,
    pub rounds: Vec<u32>,
    hit_statuses: Vec<String>,
    repeat_interval_ms: Option<u64>,
    contact: ContactBudget<(String, u32)>,
}
impl PiercingProjectile {
    pub(super) fn new(
        a: &PendingAction,
        s: &SkillConfig,
        position: [f64; 2],
        attack: u32,
        now: u64,
    ) -> Self {
        Self {
            action_id: a.id.clone(),
            skill_id: s.id.clone(),
            position,
            direction: a.direction,
            radius: s.radius,
            speed: s.speed,
            remaining: s.range,
            raw: (u64::from(attack) * u64::from(s.damage_bps) / 10000) as u32,
            accepted_at: a.accepted_at,
            created: now,
            expires: now + s.projectile_ttl_ms,
            rounds: a.rounds.clone(),
            hit_statuses: s.on_hit_target_statuses.clone(),
            repeat_interval_ms: s.projectile_repeat_interval_ms,
            contact: s.projectile_repeat_interval_ms.map_or_else(
                || ContactBudget::once(s.max_hits),
                |interval| ContactBudget::projectile(s.max_hits, interval),
            ),
        }
    }
    pub(super) fn view(&self) -> PiercingProjectileView {
        PiercingProjectileView {
            repeat_interval_ms: self.repeat_interval_ms,
            action_id: self.action_id.clone(),
            skill_id: self.skill_id.clone(),
            position: self.position,
            direction: self.direction,
            radius: self.radius,
            speed: self.speed,
            remaining_range: self.remaining,
            expires_at_ms: self.expires,
            max_hits: self.contact.policy().max_total_hits,
            used_hits: self.contact.used(),
            remaining_hits: self.contact.remaining(),
            contacts: self
                .contact
                .history()
                .map(|((id, g), at)| PiercingContactView {
                    target: UnitTarget {
                        id: id.clone(),
                        generation: *g,
                    },
                    hit_at_ms: *at,
                    next_eligible_at_ms: self
                        .repeat_interval_ms
                        .filter(|v| *v > 0)
                        .map(|interval| at.saturating_add(interval)),
                })
                .collect(),
        }
    }
    fn end(self, reason: PiercingEndReason, events: &mut Vec<Event>) {
        events.push(Event::PiercingEnded {
            action_ids: vec![self.action_id],
            reason,
        });
    }
}
impl From<PlayerAreaCancelReason> for PiercingEndReason {
    fn from(r: PlayerAreaCancelReason) -> Self {
        match r {
            PlayerAreaCancelReason::OwnerDied => Self::OwnerDied,
            PlayerAreaCancelReason::Terminal => Self::Terminal,
            PlayerAreaCancelReason::Closed => Self::Closed,
            PlayerAreaCancelReason::EncounterReset => Self::EncounterReset,
        }
    }
}
impl Grid {
    // Exact swept disk against solid tiles (rounded corners), including map boundary.
    // Existing ordinary projectiles retain their original sampling behavior.
    pub(in crate::world) fn piercing_wall(
        &self,
        start: [f64; 2],
        end: [f64; 2],
        radius: f64,
    ) -> Option<f64> {
        self.swept_tile_collision(start, end, radius, false)
    }

    pub(in crate::world) fn walk_until_blocked(
        &self,
        start: [f64; 2],
        end: [f64; 2],
        radius: f64,
    ) -> [f64; 2] {
        let Some(t) = self.swept_tile_collision(start, end, radius, true) else {
            return end;
        };
        if t <= 0. {
            return start;
        }
        let travel = [end[0] - start[0], end[1] - start[1]];
        let t = (t - 1e-6 / travel[0].hypot(travel[1])).max(0.);
        [start[0] + travel[0] * t, start[1] + travel[1] * t]
    }

    fn swept_tile_collision(
        &self,
        start: [f64; 2],
        end: [f64; 2],
        radius: f64,
        movement: bool,
    ) -> Option<f64> {
        if !self.clear(start, radius, if movement { 0 } else { 1 }) {
            return Some(0.);
        }
        let mut first = self.geometry_contact(start, end, radius, if movement { 0 } else { 1 });
        let mut offer = |t: Option<f64>| {
            if let Some(t) = t {
                first = Some(first.map_or(t, |old| old.min(t)));
            }
        };
        if movement {
            offer(self.terrain_step_contact(start, end));
        }
        if self.isometric_tiles.is_some() {
            offer(self.isometric_tile_contact(start, end, radius, if movement { 0 } else { 1 }));
            return first;
        }
        let size = self.config.cell_size;
        for axis in 0..2 {
            let bound = if axis == 0 {
                self.config.width
            } else {
                self.config.height
            } as f64
                * size;
            let d = end[axis] - start[axis];
            if d > 0. && end[axis] + radius >= bound {
                offer(Some(((bound - radius - start[axis]) / d).clamp(0., 1.)));
            }
            if d < 0. && end[axis] - radius <= 0. {
                offer(Some(((radius - start[axis]) / d).clamp(0., 1.)));
            }
        }
        let lo = [start[0].min(end[0]) - radius, start[1].min(end[1]) - radius];
        let hi = [start[0].max(end[0]) + radius, start[1].max(end[1]) + radius];
        let x0 = (lo[0] / size).floor().max(0.) as usize;
        let y0 = (lo[1] / size).floor().max(0.) as usize;
        let x1 = ((hi[0] / size).floor().max(0.) as usize).min(self.config.width - 1);
        let y1 = ((hi[1] / size).floor().max(0.) as usize).min(self.config.height - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let tile = &self.config.palette[self.cells[y * self.config.width + x]];
                if if movement {
                    tile.walkable
                } else {
                    !tile.blocks_projectiles
                } {
                    continue;
                }
                let a = [x as f64 * size, y as f64 * size];
                let b = [a[0] + size, a[1] + size];
                if movement {
                    let normal = [
                        start[0] - start[0].clamp(a[0], b[0]),
                        start[1] - start[1].clamp(a[1], b[1]),
                    ];
                    // A disk touching a convex tile can move along or away from its tangent.
                    if normal[0].hypot(normal[1]) <= radius
                        && normal[0] * (end[0] - start[0]) + normal[1] * (end[1] - start[1]) >= 0.
                    {
                        continue;
                    }
                }
                offer(segment_rect(
                    start,
                    end,
                    [a[0] - radius, a[1]],
                    [b[0] + radius, b[1]],
                    0.,
                ));
                offer(segment_rect(
                    start,
                    end,
                    [a[0], a[1] - radius],
                    [b[0], b[1] + radius],
                    0.,
                ));
                for corner in [a, b, [a[0], b[1]], [b[0], a[1]]] {
                    offer(segment_circle(start, end, corner, radius));
                }
            }
        }
        first
    }
}
impl Arena {
    pub(crate) fn has_piercing(&self) -> bool {
        !self.piercing_projectiles.is_empty()
    }
    pub(crate) fn cancel_piercing(&mut self, reason: PiercingEndReason, events: &mut Vec<Event>) {
        if self.has_piercing() {
            events.push(Event::PiercingEnded {
                action_ids: self
                    .piercing_projectiles
                    .drain(..)
                    .map(|p| p.action_id)
                    .collect(),
                reason,
            });
        }
    }
    pub(super) fn advance_piercing(
        &mut self,
        w: &mut World,
        c: &Config,
        now: u64,
        dt: f64,
        events: &mut Vec<Event>,
    ) {
        let mut plans = std::mem::take(&mut self.piercing_projectiles).into_iter();
        while let Some(mut p) = plans.next() {
            if now >= p.expires {
                p.end(PiercingEndReason::Expired, events);
                continue;
            }
            if now == p.created {
                self.piercing_projectiles.push(p);
                continue;
            }
            let travel = (p.speed * dt).min(p.remaining);
            let start = p.position;
            let at = |t: f64| {
                [
                    start[0] + p.direction[0] * travel * t,
                    start[1] + p.direction[1] * travel * t,
                ]
            };
            let end = at(1.);
            let wall = self.grid.piercing_wall(start, end, p.radius);
            let mut candidates = self
                .monsters
                .iter()
                .enumerate()
                .filter_map(|(i, m)| {
                    let target = self.unit_reference(i)?;
                    if !self.accepts_round(m, &p.rounds)
                        || !p
                            .contact
                            .allows(&(target.id.clone(), target.generation), now)
                    {
                        return None;
                    }
                    let t =
                        segment_circle(start, end, m.position, m.profile.combat.radius + p.radius)?;
                    wall.is_none_or(|wall| t < wall).then_some((t, target, i))
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then(a.1.id.cmp(&b.1.id))
                    .then(a.1.generation.cmp(&b.1.generation))
                    .then(a.2.cmp(&b.2))
            });
            for (t, target, i) in candidates {
                if self.unit_reference(i).as_ref() != Some(&target)
                    || !self.accepts_round(&self.monsters[i], &p.rounds)
                {
                    continue;
                }
                if self.player_direct_contact(
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
                ) {
                    assert!(
                        p.contact
                            .record((target.id.clone(), target.generation), now)
                    );
                    p.position = at(t);
                    events.push(Event::PiercingHit {
                        hit_at_ms: p.repeat_interval_ms.map(|_| now),
                        action_id: p.action_id.clone(),
                        skill_id: p.skill_id.clone(),
                        target,
                        position: p.position,
                        remaining_hits: p.contact.remaining(),
                    });
                    if p.contact.remaining() == 0 || w.hp == 0 {
                        break;
                    }
                }
            }
            if w.hp == 0 {
                self.piercing_projectiles.push(p);
                self.piercing_projectiles.extend(plans);
                self.cancel_piercing(PiercingEndReason::OwnerDied, events);
                return;
            }
            if p.contact.remaining() == 0 {
                p.end(PiercingEndReason::HitBudgetExhausted, events);
            } else if let Some(t) = wall {
                p.position = at(t);
                p.end(PiercingEndReason::Wall, events);
            } else {
                p.position = end;
                p.remaining = (p.remaining - travel).max(0.);
                if p.remaining == 0. {
                    p.end(PiercingEndReason::RangeExhausted, events);
                } else {
                    self.piercing_projectiles.push(p);
                }
            }
        }
    }
}
