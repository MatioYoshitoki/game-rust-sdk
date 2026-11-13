//! Bounded entity flights within one basic attack and its terminal journal.
use super::triggers::ActionKey;
use super::*;
use crate::config::{BasicProjectileFollowup, BasicProjectilePattern};

#[path = "basic_projectile_paths.rs"]
mod paths;
use paths::{Corridor, map_diagonal, travel_ms};

struct Flight {
    view: ProjectileViewV4,
    motion: Motion,
    direction: [f64; 2],
    remaining: f64,
    expires: u64,
    speed: f64,
    raw: u32,
}

enum Motion {
    Targeted {
        target: EntityRef,
        fixed_endpoint: bool,
        ignore_interceptors: bool,
    },
    Corridor(Corridor),
    Return,
}

enum FlightStep {
    Flying,
    End(ProjectileEndReasonV4),
    Contact(Owner, [f64; 2]),
}

impl Flight {
    fn segment(&self) -> &BasicProjectileSegment {
        self.view.basic_segment.as_ref().unwrap()
    }

    fn target(&self) -> Option<&EntityRef> {
        match &self.motion {
            Motion::Targeted { target, .. } => Some(target),
            Motion::Corridor(corridor) => corridor.primary.as_ref().map(|(target, _)| target),
            Motion::Return => None,
        }
    }

    fn step(&mut self, world: &World, c: &Config, now: u64, dt: f64) -> FlightStep {
        use ProjectileEndReasonV4::*;
        if now <= self.segment().launched_at_ms {
            return FlightStep::Flying;
        }
        let source = &self.view.source;
        if now >= self.expires {
            return FlightStep::End(Expired);
        }
        let Motion::Targeted {
            target,
            fixed_endpoint,
            ignore_interceptors,
        } = &self.motion
        else {
            debug_assert!(matches!(self.motion, Motion::Return));
            let travel = (dt * self.speed).min(self.remaining);
            self.move_to(travel);
            return if self.remaining == 0. {
                FlightStep::End(RangeExhausted)
            } else {
                FlightStep::Flying
            };
        };
        if !fixed_endpoint && !world.hostile_contact(source, target) {
            return FlightStep::End(TargetInvalid);
        }
        let start = self.view.position;
        let travel = (dt * self.speed).min(self.remaining);
        let at = |t| {
            [
                start[0] + self.direction[0] * travel * t,
                start[1] + self.direction[1] * travel * t,
            ]
        };
        let end = at(1.);
        let radius = self.view.radius;
        let wall = world.grid.piercing_wall(start, end, radius);
        let hit = if *fixed_endpoint || *ignore_interceptors {
            world
                .hostile_sweeps(source, (start, end), radius, c)
                .find_map(|(t, entity, owner)| (entity == target).then_some((t, owner)))
        } else {
            world.first_hostile_contact(source, (start, end), radius, c)
        };
        if let Some((t, owner)) = hit.filter(|(t, _)| wall.is_none_or(|w| *t < w)) {
            self.view.position = at(t);
            if world.owner_entity(owner) == *target {
                return FlightStep::Contact(owner, self.view.position);
            }
            return FlightStep::End(TargetInvalid);
        }
        if let Some(t) = wall {
            self.view.position = at(t);
            return FlightStep::End(Wall);
        }
        if travel >= self.remaining {
            self.view.position = end;
            return FlightStep::End(RangeExhausted);
        }
        self.view.position = end;
        self.remaining -= travel;
        FlightStep::Flying
    }

    fn move_to(&mut self, travel: f64) {
        self.view.position[0] += self.direction[0] * travel;
        self.view.position[1] += self.direction[1] * travel;
        self.remaining = (self.remaining - travel).max(0.);
    }

    fn end(self, reason: Option<ProjectileEndReasonV4>, events: &mut Events) {
        events.push((
            None,
            EventV4::ProjectileEnded {
                projectile_id: self.view.id,
                reason,
            },
        ));
    }
}

enum Focus {
    Companion(EntityRef),
    Fixed(EntityRef, [f64; 2]),
    Recall,
}

struct Followups {
    focus: Focus,
    index: u32,
    count: u32,
    next_at: u64,
    interval: u64,
}

pub(super) struct BasicProjectilePlan {
    pub(super) action: ActionKey,
    pub(super) deadline: u64,
    pub(super) slots: usize,
    source: EntityRef,
    plan_id: String,
    origin: [f64; 2],
    pattern: BasicProjectilePattern,
    raw: u32,
    flights: Vec<Flight>,
    next: Option<Followups>,
    visited: Vec<EntityRef>,
    damaged_targets: u32,
}

impl BasicProjectilePlan {
    fn view(
        &self,
        (index, role): (u32, Option<BasicProjectileRole>),
        (origin, aim_point): ([f64; 2], [f64; 2]),
        radius: f64,
        now: u64,
    ) -> ProjectileViewV4 {
        ProjectileViewV4 {
            basic_segment: Some(BasicProjectileSegment {
                tier: None,
                action_id: self.action.id.clone(),
                plan_id: self.plan_id.clone(),
                index,
                launched_at_ms: now,
                aim_point,
                role,
            }),
            piercing: None,
            unit_target: None,
            id: String::new(),
            source: self.source.clone(),
            position: origin,
            radius,
        }
    }

    fn flight(
        &self,
        index: u32,
        target: (EntityRef, [f64; 2]),
        origin: [f64; 2],
        (basic, c): (&BasicConfig, &Config),
        now: u64,
    ) -> Flight {
        let (target, aim_point) = target;
        let delta = [aim_point[0] - origin[0], aim_point[1] - origin[1]];
        let fixed_endpoint =
            index > 0 && matches!(self.pattern, BasicProjectilePattern::Volley { .. });
        let ricochet = match &self.pattern {
            BasicProjectilePattern::Ricochet(rule) => Some(rule),
            _ => None,
        };
        let role = ricochet.map(|_| BasicProjectileRole::Ricochet);
        let mut view = self.view(
            (index, role),
            (origin, aim_point),
            c.params.basic_projectile_radius,
            now,
        );
        view.unit_target = Some(World::unit_wire(&target));
        let range = ricochet
            .filter(|_| index > 0)
            .map_or(basic.range, |rule| rule.search_radius);
        Flight {
            view,
            motion: Motion::Targeted {
                target,
                fixed_endpoint,
                ignore_interceptors: ricochet.is_some() && index > 0,
            },
            direction: normalize(delta),
            remaining: if fixed_endpoint {
                delta[0].hypot(delta[1]).min(range)
            } else {
                range
            },
            expires: now
                + ricochet.map_or(c.params.player_projectile_ttl_ms, |_| {
                    travel_ms(range, basic.projectile_speed, c)
                }),
            speed: basic.projectile_speed,
            raw: self.raw,
        }
    }
}

impl Member {
    pub(super) fn projectile_progress(&self, c: &Config) -> Option<BasicProjectileProgress> {
        if !matches!(self.phase, MemberPhase::Deploying | MemberPhase::Active) {
            return None;
        }
        let BasicProjectilePattern::Volley {
            hits,
            attack_speed_bps_per_hit,
            ..
        } = &self.basic(c).projectile_followup.as_ref()?.pattern
        else {
            return None;
        };
        Some(BasicProjectileProgress {
            stored: self.basic_projectile_hits,
            maximum: *hits,
            attack_speed_bonus_bps: self.basic_projectile_hits * attack_speed_bps_per_hit,
        })
    }
}

impl World {
    pub(super) fn projectile_slots_used(&self) -> usize {
        self.shots.len()
            + self
                .members
                .iter()
                .filter_map(|m| m.basic_projectile.as_ref())
                .map(|p| p.slots)
                .sum::<usize>()
    }

    pub(super) fn plan_basic_projectile(
        &self,
        i: usize,
        pending: &Pending,
        config: &BasicProjectileFollowup,
        c: &Config,
        now: u64,
    ) -> Result<BasicProjectilePlan> {
        let target = pending.target.as_ref().ok_or(Error::InvalidTarget)?;
        let position = self.basic_target_position(i, target)?;
        let member = &self.members[i];
        let basic = member.basic(c);
        let ttl = c.params.player_projectile_ttl_ms;
        let (slots, duration, damage) = match &config.pattern {
            BasicProjectilePattern::Companion {
                delay_ms,
                damage_bps,
            } => (1, 2 * ttl + delay_ms + c.step_ms, *damage_bps),
            BasicProjectilePattern::Volley {
                hits,
                interval_ms,
                damage_bps,
                ..
            } => {
                if member.basic_projectile_hits + 1 == *hits {
                    (
                        *hits as usize,
                        2 * ttl + u64::from(hits - 1) * interval_ms + c.step_ms,
                        *damage_bps,
                    )
                } else {
                    (1, ttl, *damage_bps)
                }
            }
            BasicProjectilePattern::SealRecall(rule) => {
                let outbound = travel_ms(basic.range + rule.overtravel, basic.projectile_speed, c);
                if member.basic_seals.len() + 1 == rule.required_seals as usize {
                    (
                        rule.required_seals as usize,
                        outbound
                            + rule.settle_delay_ms
                            + u64::from(rule.required_seals - 1) * rule.interval_ms
                            + travel_ms(map_diagonal(c), basic.projectile_speed, c)
                            + c.step_ms,
                        rule.recall_damage_bps,
                    )
                } else {
                    (1, outbound, rule.recall_damage_bps)
                }
            }
            BasicProjectilePattern::Ricochet(rule) => (
                1,
                travel_ms(basic.range, basic.projectile_speed, c)
                    + u64::from(rule.max_targets - 1)
                        * travel_ms(rule.search_radius, basic.projectile_speed, c)
                    + rule.return_to_owner.as_ref().map_or(0, |returning| {
                        travel_ms(
                            map_diagonal(c),
                            basic.projectile_speed * f64::from(returning.speed_bps) / 10000.,
                            c,
                        )
                    }),
                rule.damage_bps,
            ),
        };
        let rule = &c.combat_rules.as_ref().unwrap().basic_attack;
        let attack = self.stats(i, c, now).0;
        let primary_raw = rule.budget(attack, config.primary_damage_bps, 10000, 0);
        let mut plan = BasicProjectilePlan {
            action: ActionKey::new(&member.reference.entity, &pending.id),
            deadline: now + duration,
            slots,
            source: member.reference.entity.clone(),
            plan_id: config.plan_id.clone(),
            origin: member.position,
            pattern: config.pattern.clone(),
            raw: rule.budget(attack, damage, 10000, 0),
            flights: vec![],
            next: None,
            visited: vec![],
            damaged_targets: 0,
        };
        let main = if let BasicProjectilePattern::SealRecall(recall) = &plan.pattern {
            let endpoint = paths::seal_endpoint(plan.origin, position, recall.overtravel, c);
            let mut main = plan.corridor(
                0,
                (plan.origin, endpoint),
                Some((target.clone(), primary_raw)),
                (basic, c),
                now,
            );
            main.raw = rule.budget(attack, recall.additional_damage_bps, 10000, 0);
            main
        } else {
            let mut main = plan.flight(0, (target.clone(), position), plan.origin, (basic, c), now);
            main.raw = primary_raw;
            main
        };
        plan.flights.push(main);
        Ok(plan)
    }

    fn launch_basic_flight(&mut self, flight: &mut Flight, events: &mut Events) {
        flight.view.id = self.next_id("shot");
        if flight
            .target()
            .is_some_and(|target| !self.hostile_contact(&flight.view.source, target))
        {
            flight.view.unit_target = None;
        }
        events.push((
            None,
            EventV4::ProjectileStarted {
                projectile: flight.view.clone(),
            },
        ));
    }

    pub(super) fn start_basic_projectile(
        &mut self,
        i: usize,
        mut plan: BasicProjectilePlan,
        events: &mut Events,
    ) {
        if self.members[i].living() {
            self.launch_basic_flight(&mut plan.flights[0], events);
            self.members[i].basic_projectile = Some(plan);
        }
    }

    pub(super) fn clear_basic_projectile(&mut self, i: usize, events: &mut Events) {
        let member = &mut self.members[i];
        member.basic_projectile_hits = 0;
        member.basic_seals.clear();
        member.basic_ricochet_marks.clear();
        if let Some(plan) = member.basic_projectile.take() {
            for flight in plan.flights {
                flight.end(Some(ProjectileEndReasonV4::SourceInvalid), events);
            }
        }
    }

    pub(super) fn basic_projectile_views(&self) -> impl Iterator<Item = ProjectileViewV4> + '_ {
        self.members
            .iter()
            .filter_map(|m| m.basic_projectile.as_ref())
            .flat_map(|plan| {
                plan.flights.iter().map(|flight| {
                    let mut view = flight.view.clone();
                    if flight
                        .target()
                        .is_some_and(|target| !self.hostile_contact(&view.source, target))
                    {
                        view.unit_target = None;
                    }
                    view
                })
            })
    }

    pub(super) fn advance_basic_projectiles(
        &mut self,
        c: &Config,
        now: u64,
        dt: f64,
        events: &mut Events,
    ) {
        for i in 0..self.members.len() {
            let Some(mut plan) = self.members[i].basic_projectile.take() else {
                continue;
            };
            let basic = self.members[i].basic(c);
            for mut flight in std::mem::take(&mut plan.flights) {
                if !self.members[i].living() {
                    flight.end(Some(ProjectileEndReasonV4::SourceInvalid), events);
                    continue;
                }
                if matches!(flight.motion, Motion::Corridor(_)) {
                    if let Some(reason) = self.advance_basic_corridor(
                        i,
                        &plan.action,
                        &mut flight,
                        c,
                        (now, dt),
                        events,
                    ) {
                        self.finish_basic_flight(
                            i,
                            &mut plan,
                            flight,
                            (Some(reason), None),
                            (basic, c, now),
                            events,
                        );
                    } else {
                        plan.flights.push(flight);
                    }
                    continue;
                }
                match flight.step(self, c, now, dt) {
                    FlightStep::Flying => plan.flights.push(flight),
                    FlightStep::End(reason) => self.finish_basic_flight(
                        i,
                        &mut plan,
                        flight,
                        (Some(reason), None),
                        (basic, c, now),
                        events,
                    ),
                    FlightStep::Contact(owner, point) => {
                        let target = self.owner_entity(owner);
                        let aim_point = self.actor_position(owner);
                        let damage = self.resolve_native_basic_damage(
                            i,
                            owner,
                            (&flight.view.source, &plan.action.id, flight.raw),
                            c,
                            now,
                            events,
                        );
                        if damage.is_some() {
                            flight.publish_contact(target.clone(), point, now, events);
                            if flight.segment().index == 0 && self.members[i].living() {
                                self.arm_basic_followups(
                                    i,
                                    &mut plan,
                                    target.clone(),
                                    aim_point,
                                    now,
                                );
                            }
                        }
                        self.finish_basic_flight(
                            i,
                            &mut plan,
                            flight,
                            (None, damage.map(|value| (target, value))),
                            (basic, c, now),
                            events,
                        );
                    }
                }
            }
            if self.members[i].living() {
                self.launch_due_basic_followups(i, &mut plan, (basic, c), now, events);
                if plan.next.is_some() || !plan.flights.is_empty() {
                    self.members[i].basic_projectile = Some(plan);
                } else if matches!(plan.pattern, BasicProjectilePattern::Volley { hits, .. }
                    if self.members[i].basic_projectile_hits == hits)
                {
                    self.members[i].basic_projectile_hits = 0;
                }
            } else {
                for flight in plan.flights {
                    flight.end(Some(ProjectileEndReasonV4::SourceInvalid), events);
                }
            }
        }
    }

    fn arm_basic_followups(
        &mut self,
        i: usize,
        plan: &mut BasicProjectilePlan,
        target: EntityRef,
        point: [f64; 2],
        now: u64,
    ) {
        plan.next = match &plan.pattern {
            BasicProjectilePattern::Companion { delay_ms, .. } => Some(Followups {
                focus: Focus::Companion(target),
                index: 1,
                count: 1,
                next_at: now + delay_ms,
                interval: 0,
            }),
            BasicProjectilePattern::Volley {
                hits, interval_ms, ..
            } => {
                self.members[i].basic_projectile_hits += 1;
                (self.members[i].basic_projectile_hits == *hits).then_some(Followups {
                    focus: Focus::Fixed(target, point),
                    index: 1,
                    count: *hits,
                    next_at: now,
                    interval: *interval_ms,
                })
            }
            BasicProjectilePattern::SealRecall(_) | BasicProjectilePattern::Ricochet(_) => None,
        };
    }

    fn launch_due_basic_followups(
        &mut self,
        i: usize,
        plan: &mut BasicProjectilePlan,
        (basic, c): (&BasicConfig, &Config),
        now: u64,
        events: &mut Events,
    ) {
        while let Some(mut next) = plan.next.take_if(|next| next.next_at <= now) {
            let mut flight = match &next.focus {
                Focus::Recall => {
                    let member = &mut self.members[i];
                    let from = member.basic_seals.remove(0);
                    plan.corridor(next.index, (from, member.position), None, (basic, c), now)
                }
                Focus::Fixed(target, point) => plan.flight(
                    next.index,
                    (target.clone(), *point),
                    plan.origin,
                    (basic, c),
                    now,
                ),
                Focus::Companion(target) => {
                    let origin = self.members[i].position;
                    let source = &self.members[i].reference.entity;
                    let target = self
                        .basic_target_position(i, target)
                        .ok()
                        .filter(|p| distance(origin, *p) <= basic.range)
                        .map(|p| (target.clone(), p))
                        .or_else(|| {
                            self.npcs
                                .iter()
                                .filter(|npc| {
                                    self.hostile_contact(source, &npc.entity)
                                        && distance(origin, npc.position) <= basic.range
                                        && self
                                            .grid
                                            .piercing_wall(
                                                origin,
                                                npc.position,
                                                c.params.basic_projectile_radius,
                                            )
                                            .is_none()
                                })
                                .min_by(|a, b| {
                                    distance(origin, a.position)
                                        .total_cmp(&distance(origin, b.position))
                                        .then(a.entity.id.cmp(&b.entity.id))
                                })
                                .map(|npc| (npc.entity.clone(), npc.position))
                        });
                    let Some(target) = target else {
                        return;
                    };
                    plan.flight(next.index, target, origin, (basic, c), now)
                }
            };
            self.launch_basic_flight(&mut flight, events);
            plan.flights.push(flight);
            next.index += 1;
            next.next_at += next.interval;
            if next.index <= next.count {
                plan.next = Some(next);
            }
        }
    }
}
