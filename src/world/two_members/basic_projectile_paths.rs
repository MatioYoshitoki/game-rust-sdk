use super::*;
use crate::config::{BasicProjectileReturn, BasicProjectileRicochet};

pub(super) struct Corridor {
    pub(super) primary: Option<(EntityRef, u32)>,
    half_width: f64,
    seen: BTreeMap<usize, u32>,
    primary_damaged: bool,
    slow_status: Option<String>,
}

pub(super) fn travel_ms(length: f64, speed: f64, c: &Config) -> u64 {
    ((length.min(map_diagonal(c)) * 1000. / speed).ceil() as u64) + c.step_ms * 2
}

pub(super) fn map_diagonal(c: &Config) -> f64 {
    c.pve.as_ref().unwrap().map_diagonal()
}

pub(super) fn seal_endpoint(
    origin: [f64; 2],
    target: [f64; 2],
    overtravel: f64,
    c: &Config,
) -> [f64; 2] {
    let delta = [target[0] - origin[0], target[1] - origin[1]];
    let direction = normalize(delta);
    let mut length = delta[0].hypot(delta[1]) + overtravel;
    let radius = c.params.basic_projectile_radius;
    let pve = c.pve.as_ref().unwrap();
    if let Some(limit) = pve.isometric_ray_limit(origin, direction, radius) {
        length = length.min(limit);
    } else {
        let (_, limits) = pve.world_bounds();
        for axis in 0..2 {
            let d = direction[axis];
            if d > 0. {
                length = length.min((limits[axis] - radius - origin[axis]) / d);
            } else if d < 0. {
                length = length.min((radius - origin[axis]) / d);
            }
        }
    }
    [
        origin[0] + direction[0] * length.max(0.),
        origin[1] + direction[1] * length.max(0.),
    ]
}

impl Member {
    pub(in crate::world::two_members) fn projectile_seals(
        &self,
        c: &Config,
    ) -> Option<Vec<[f64; 2]>> {
        (matches!(self.phase, MemberPhase::Deploying | MemberPhase::Active)
            && matches!(
                self.basic(c).projectile_followup.as_ref()?.pattern,
                BasicProjectilePattern::SealRecall(_)
            ))
        .then(|| self.basic_seals.clone())
    }
}

impl Flight {
    pub(super) fn publish_contact(
        &self,
        target: EntityRef,
        point: [f64; 2],
        now: u64,
        events: &mut Events,
    ) {
        Self::emit_contact(&self.view, target, point, now, events);
    }

    fn emit_contact(
        view: &ProjectileViewV4,
        target: EntityRef,
        point: [f64; 2],
        now: u64,
        events: &mut Events,
    ) {
        events.push((
            None,
            EventV4::ProjectileContact {
                projectile_id: view.id.clone(),
                source: view.source.clone(),
                target,
                position: point,
                hit_at_ms: now,
                remaining_hits: 0,
            },
        ));
    }
}

impl BasicProjectilePlan {
    pub(super) fn corridor(
        &self,
        index: u32,
        (origin, end): ([f64; 2], [f64; 2]),
        primary: Option<(EntityRef, u32)>,
        (basic, c): (&BasicConfig, &Config),
        now: u64,
    ) -> Flight {
        let BasicProjectilePattern::SealRecall(rule) = &self.pattern else {
            unreachable!("corridor requires a recall profile");
        };
        let role = if primary.is_some() {
            BasicProjectileRole::SealOutbound
        } else {
            BasicProjectileRole::SealRecall
        };
        let radius = c.params.basic_projectile_radius;
        let mut view = self.view((index, Some(role)), (origin, end), radius, now);
        view.unit_target = primary.as_ref().map(|(target, _)| World::unit_wire(target));
        let delta = [end[0] - origin[0], end[1] - origin[1]];
        let remaining = distance(origin, end);
        Flight {
            view,
            motion: Motion::Corridor(Corridor {
                primary,
                half_width: rule.corridor_half_width,
                seen: BTreeMap::new(),
                primary_damaged: false,
                slow_status: (role == BasicProjectileRole::SealRecall)
                    .then(|| rule.slow_status.clone()),
            }),
            direction: normalize(delta),
            remaining,
            expires: now + travel_ms(remaining, basic.projectile_speed, c),
            speed: basic.projectile_speed,
            raw: self.raw,
        }
    }

    fn return_flight(
        &self,
        index: u32,
        (origin, end): ([f64; 2], [f64; 2]),
        speed: f64,
        c: &Config,
        now: u64,
    ) -> Flight {
        let remaining = distance(origin, end);
        Flight {
            view: self.view(
                (index, Some(BasicProjectileRole::ReturnToOwner)),
                (origin, end),
                c.params.basic_projectile_radius,
                now,
            ),
            motion: Motion::Return,
            direction: normalize([end[0] - origin[0], end[1] - origin[1]]),
            remaining,
            expires: now + travel_ms(remaining, speed, c),
            speed,
            raw: 0,
        }
    }
}

impl World {
    pub(super) fn advance_basic_corridor(
        &mut self,
        i: usize,
        action: &ActionKey,
        flight: &mut Flight,
        c: &Config,
        (now, dt): (u64, f64),
        events: &mut Events,
    ) -> Option<ProjectileEndReasonV4> {
        use ProjectileEndReasonV4::*;
        if now <= flight.segment().launched_at_ms {
            return None;
        }
        if now >= flight.expires {
            return Some(Expired);
        }
        let Motion::Corridor(path) = &mut flight.motion else {
            unreachable!("corridor motion required");
        };
        let start = flight.view.position;
        let travel = (flight.speed * dt).min(flight.remaining);
        let point = |d: f64| {
            [
                start[0] + flight.direction[0] * d,
                start[1] + flight.direction[1] * d,
            ]
        };
        let wall = self
            .grid
            .piercing_wall(start, point(travel), flight.view.radius);
        let stop = wall.map_or(travel, |t| t * travel);
        let source = &flight.view.source;
        let mut contacts = self
            .npcs
            .iter()
            .enumerate()
            .filter_map(|(j, npc)| {
                if !self.hostile_contact(source, &npc.entity)
                    || path.seen.get(&j) == Some(&npc.entity.generation)
                {
                    return None;
                }
                let delta = [npc.position[0] - start[0], npc.position[1] - start[1]];
                let along = delta[0] * flight.direction[0] + delta[1] * flight.direction[1];
                let orthogonal = [
                    delta[0] - flight.direction[0] * along,
                    delta[1] - flight.direction[1] * along,
                ];
                (along >= -1e-9
                    && along <= travel + 1e-9
                    && wall.is_none_or(|_| along < stop)
                    && orthogonal[0].hypot(orthogonal[1]) <= path.half_width
                    && self
                        .grid
                        .piercing_wall(point(along.max(0.)), npc.position, 0.)
                        .is_none())
                .then(|| (along.max(0.), npc.entity.clone(), j))
            })
            .collect::<Vec<_>>();
        contacts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        for (along, target, j) in contacts {
            if !self.members[i].living() {
                return Some(SourceInvalid);
            }
            path.seen.insert(j, target.generation);
            let primary = path
                .primary
                .as_ref()
                .filter(|(entity, _)| *entity == target);
            let raw = primary.map_or(flight.raw, |(_, raw)| *raw);
            if let Some(actual) = self.resolve_native_basic_damage(
                i,
                Owner::Npc(j),
                (source, &action.id, raw),
                c,
                now,
                events,
            ) {
                path.primary_damaged |= primary.is_some() && actual > 0;
                Flight::emit_contact(&flight.view, target.clone(), point(along), now, events);
                if let Some(status) = &path.slow_status
                    && self.hostile_contact(source, &target)
                {
                    let defs = Self::definitions(c, std::slice::from_ref(status));
                    self.apply_owned_status_definitions(
                        Owner::Member(i),
                        target,
                        &defs,
                        c,
                        now,
                        events,
                    );
                }
            }
        }
        // Keep a landed seal on the clear side of the wall for its reverse sweep.
        flight.move_to(wall.map_or(stop, |_| (stop - 1e-6).max(0.)));
        if wall.is_some() {
            Some(Wall)
        } else if flight.remaining == 0. {
            Some(RangeExhausted)
        } else {
            None
        }
    }

    pub(super) fn finish_basic_flight(
        &mut self,
        i: usize,
        plan: &mut BasicProjectilePlan,
        flight: Flight,
        (reason, contact): (Option<ProjectileEndReasonV4>, Option<(EntityRef, u32)>),
        (basic, c, now): (&BasicConfig, &Config, u64),
        events: &mut Events,
    ) {
        let index = flight.segment().index;
        let position = flight.view.position;
        let returned = matches!(flight.motion, Motion::Return);
        let landed = matches!(&flight.motion, Motion::Corridor(path) if path.primary_damaged)
            && matches!(
                reason,
                Some(ProjectileEndReasonV4::Wall | ProjectileEndReasonV4::RangeExhausted)
            );
        let completed_return = returned && reason == Some(ProjectileEndReasonV4::RangeExhausted);
        flight.end(reason.filter(|_| !completed_return), events);
        if !self.members[i].living() {
            return;
        }
        if returned {
            if completed_return
                && let BasicProjectilePattern::Ricochet(rule) = &plan.pattern
                && let Some(returning) = &rule.return_to_owner
            {
                self.finish_return_shield(i, plan.damaged_targets, returning, c, now, events);
            }
        } else if landed {
            self.store_basic_seal(i, plan, position, now, events);
        } else if let BasicProjectilePattern::Ricochet(rule) = &plan.pattern {
            let mut next = None;
            if let Some((target, actual)) = contact {
                self.apply_ricochet_mark(i, &target, rule, c, now, events);
                plan.visited.push(target);
                plan.damaged_targets += u32::from(actual > 0);
                if index + 1 < rule.max_targets {
                    next = self
                        .ricochet_target(plan, position, rule.search_radius)
                        .map(|target| plan.flight(index + 1, target, position, (basic, c), now));
                }
            }
            if next.is_none()
                && let Some(returning) = &rule.return_to_owner
            {
                next = Some(plan.return_flight(
                    index + 1,
                    (position, self.members[i].position),
                    basic.projectile_speed * f64::from(returning.speed_bps) / 10000.,
                    c,
                    now,
                ));
            }
            if let Some(mut next) = next {
                self.launch_basic_flight(&mut next, events);
                plan.flights.push(next);
            }
        }
    }

    fn store_basic_seal(
        &mut self,
        i: usize,
        plan: &mut BasicProjectilePlan,
        position: [f64; 2],
        now: u64,
        events: &mut Events,
    ) {
        let BasicProjectilePattern::SealRecall(rule) = &plan.pattern else {
            return;
        };
        let seals = &mut self.members[i].basic_seals;
        seals.push(position);
        events.push((
            None,
            EventV4::BasicAttackSwordLanded {
                source: plan.source.clone(),
                action_id: plan.action.id.clone(),
                position,
                stored: seals.len() as u32,
            },
        ));
        if seals.len() as u32 == rule.required_seals {
            plan.next = Some(Followups {
                focus: Focus::Recall,
                index: 1,
                count: rule.required_seals,
                next_at: now + rule.settle_delay_ms,
                interval: rule.interval_ms,
            });
        }
    }

    fn ricochet_target(
        &self,
        plan: &BasicProjectilePlan,
        origin: [f64; 2],
        radius: f64,
    ) -> Option<(EntityRef, [f64; 2])> {
        self.npcs
            .iter()
            .filter(|npc| {
                !plan.visited.contains(&npc.entity)
                    && self.hostile_contact(&plan.source, &npc.entity)
                    && distance(origin, npc.position) <= radius
            })
            .min_by(|a, b| {
                distance(origin, a.position)
                    .total_cmp(&distance(origin, b.position))
                    .then(a.entity.id.cmp(&b.entity.id))
            })
            .map(|npc| (npc.entity.clone(), npc.position))
    }

    fn apply_ricochet_mark(
        &mut self,
        i: usize,
        target: &EntityRef,
        rule: &BasicProjectileRicochet,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if !self.hostile_contact(&self.members[i].reference.entity, target) {
            return;
        }
        let Some(Owner::Npc(j)) = self.resolve_entity(target) else {
            return;
        };
        let marks = &mut self.members[i].basic_ricochet_marks;
        let (generation, count, expires) = marks.entry(j).or_insert((target.generation, 0, now));
        if *generation != target.generation || *expires <= now {
            *generation = target.generation;
            *count = 0;
        }
        *count += 1;
        *expires = now + rule.mark_duration_ms;
        if *count == rule.hits_to_stun {
            marks.remove(&j);
            let rules = c.combat_rules.as_ref().unwrap();
            let profile = rules
                .npc_profiles
                .get(&self.npcs[j].template_id)
                .unwrap_or(&rules.default_npc_profile);
            if !rule.resistant_profiles.contains(profile) {
                let defs = Self::definitions(c, std::slice::from_ref(&rule.stun_status));
                self.apply_owned_status_definitions(
                    Owner::Member(i),
                    target.clone(),
                    &defs,
                    c,
                    now,
                    events,
                );
            }
        }
    }

    fn finish_return_shield(
        &mut self,
        i: usize,
        targets: u32,
        returning: &BasicProjectileReturn,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let member = &self.members[i];
        if targets == 0
            || !member.eligible()
            || !member
                .basic_delivery
                .as_ref()
                .is_some_and(|delivery| delivery.earned())
        {
            return;
        }
        let mut defs = Self::definitions(c, std::slice::from_ref(&returning.shield_status));
        defs[0].shield_bps = Some(
            (returning.base_max_hp_bps + returning.per_extra_max_hp_bps * (targets - 1))
                .min(returning.max_hp_bps),
        );
        defs[0].shield_preserve_higher = true;
        self.apply_owned_status_definitions(
            Owner::Member(i),
            member.reference.entity.clone(),
            &defs,
            c,
            now,
            events,
        );
    }
}
