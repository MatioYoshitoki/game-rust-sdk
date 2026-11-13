//! Piercing selects ordered swept contacts; budget and actual damage stay shared.
use super::*;
impl Shot {
    pub(super) fn view(&self) -> ProjectileViewV4 {
        let mut v = self.view.clone();
        if let (Some(p), Some(b)) = (&mut v.piercing, &self.contact) {
            p.remaining_range = self.remaining;
            p.used_hits = b.used();
            p.remaining_hits = b.remaining();
            p.contacts = b
                .history()
                .map(|((_, id, generation), at)| ProjectileContactViewV4 {
                    target: UnitTarget {
                        id: id.clone(),
                        generation: *generation,
                    },
                    hit_at_ms: *at,
                    next_eligible_at_ms: (p.repeat_interval_ms > 0)
                        .then(|| at.saturating_add(p.repeat_interval_ms)),
                })
                .collect();
        }
        v
    }
    pub(super) fn end(self, reason: ProjectileEndReasonV4, events: &mut Events) {
        events.push((
            None,
            EventV4::ProjectileEnded {
                projectile_id: self.view.id,
                reason: Some(reason),
            },
        ));
    }
}
impl World {
    pub(super) fn advance_piercing_shot(
        &mut self,
        mut s: Shot,
        c: &Config,
        now: u64,
        dt: f64,
        events: &mut Events,
    ) {
        let start = s.view.position;
        let travel = (s.speed * dt).min(s.remaining);
        let at = |t: f64| {
            [
                start[0] + s.direction[0] * travel * t,
                start[1] + s.direction[1] * travel * t,
            ]
        };
        let end = at(1.);
        let wall = self.grid.piercing_wall(start, end, s.view.radius);
        let mut targets = self
            .hostile_sweeps(&s.view.source, (start, end), s.view.radius, c)
            .filter(|(t, _, _)| wall.is_none_or(|w| *t < w))
            .map(|(t, e, o)| (t, e.clone(), o))
            .collect::<Vec<_>>();
        targets.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.id.cmp(&b.1.id))
                .then(a.1.generation.cmp(&b.1.generation))
        });
        for (t, target, owner) in targets {
            let key = (
                u8::from(target.kind == StatusEntityKind::Monster),
                target.id.clone(),
                target.generation,
            );
            if !s.contact.as_ref().unwrap().allows(&key, now) {
                continue;
            }
            if self.direct_contact(
                s.owner,
                owner,
                combat::DirectContact {
                    origin: Some(s.origin.clone()),
                    action_id: Some(&s.action_id),
                    rule: s.damage_rule.as_ref(),
                    ignore_armor: s.ignore_armor,
                    unavoidable: s.unavoidable,
                    dead_source: false,
                    source: &s.view.source,
                    raw: s.raw,
                    metal: s.metal,
                    hit_statuses: &s.hit_statuses,
                },
                c,
                now,
                events,
            ) {
                let b = s.contact.as_mut().unwrap();
                assert!(b.record(key, now));
                events.push((
                    None,
                    EventV4::ProjectileContact {
                        projectile_id: s.view.id.clone(),
                        source: s.view.source.clone(),
                        target,
                        position: at(t),
                        hit_at_ms: now,
                        remaining_hits: b.remaining(),
                    },
                ));
                if b.remaining() == 0 {
                    s.end(ProjectileEndReasonV4::HitBudgetExhausted, events);
                    return;
                }
            }
        }
        if wall.is_some() {
            s.end(ProjectileEndReasonV4::Wall, events);
        } else if travel >= s.remaining {
            s.end(ProjectileEndReasonV4::RangeExhausted, events);
        } else {
            s.view.position = end;
            s.remaining = (s.remaining - travel).max(0.);
            self.shots.push(s);
        }
    }
}
