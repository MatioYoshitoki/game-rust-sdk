//! A straight native basic flight with per-target contacts and one terminal burst.
use super::*;

struct TargetContacts {
    target: EntityRef,
    hits: u32,
    next_at: u64,
}

pub(super) struct FlightContactPlan {
    target_cap: usize,
    contact_interval_ms: u64,
    contact_limit: u32,
    contact_stacks: u32,
    burst_stacks: u32,
    burst_radius: f64,
    burst_raw: u32,
    status_id: String,
    contacts: Vec<TargetContacts>,
}

impl World {
    pub(super) fn prepare_charged_flight(
        &mut self,
        i: usize,
        point: [f64; 2],
        stage: usize,
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        self.basic_ground_target(i, point, c)?;
        let from = self.members[i].position;
        let direction = validated_direction([point[0] - from[0], point[1] - from[1]])?;
        let flight = self.members[i].basic(c).charged_flight().unwrap();
        self.projectile_origin(i, flight.stages[stage].radius)?;
        self.prepare(i, None, (direction, None), id, c, now)?;
        let pending = self.members[i].pending.as_mut().unwrap();
        pending.ground = Some(point);
        pending.charged_basic = Some(ChargedBasic::Flight(stage));
        Ok(())
    }

    pub(super) fn plan_charged_flight(
        &self,
        i: usize,
        pending: &Pending,
        stage: usize,
        c: &Config,
        now: u64,
    ) -> Result<Shot> {
        let member = &self.members[i];
        let basic = member.basic(c);
        let flight = basic.charged_flight().unwrap();
        let tier = &flight.stages[stage];
        let point = pending.ground.unwrap();
        self.projectile_origin(i, tier.radius)?;
        let origin = member.position;
        let remaining = distance(origin, point);
        if remaining <= 1e-9 {
            return Err(Error::InvalidTarget);
        }
        // The fireball preparation locks this origin until the frozen ray is launched.
        let direction = validated_direction([point[0] - origin[0], point[1] - origin[1]])?;
        let speed = basic.projectile_speed;
        let travel_ms = (remaining * 1000. / speed).ceil() as u64;
        let expires = now + (travel_ms.div_ceil(c.step_ms) + 1) * c.step_ms;
        let attack = self.stats(i, c, now).0;
        let budget = |bps| flight.damage_rule.budget(attack, bps, 10000, 0);
        Ok(Shot {
            end_on_source_invalid: false,
            flight: Some(Box::new(FlightContactPlan {
                target_cap: flight.target_cap,
                contact_interval_ms: flight.contact_interval_ms,
                contact_limit: tier.max_contacts_per_target,
                contact_stacks: flight.contact_stacks,
                burst_stacks: tier.burst_stacks,
                burst_radius: tier.burst_radius,
                burst_raw: budget(tier.burst_damage_bps),
                status_id: flight.status_id.clone(),
                contacts: Vec::new(),
            })),
            origin: crate::origin::ActionOrigin::Basic,
            action_id: pending.id.clone(),
            damage_rule: Some(flight.damage_rule.clone()),
            ignore_armor: basic.ignore_armor == Some(true),
            unavoidable: basic.unavoidable == Some(true),
            contact: None,
            view: ProjectileViewV4 {
                basic_segment: Some(BasicProjectileSegment {
                    tier: Some(stage as u8 + 1),
                    role: Some(BasicProjectileRole::ChargedFlight),
                    action_id: pending.id.clone(),
                    plan_id: flight.plan_id.clone(),
                    index: 0,
                    launched_at_ms: now,
                    aim_point: point,
                }),
                piercing: None,
                unit_target: None,
                id: String::new(),
                source: member.reference.entity.clone(),
                position: origin,
                radius: tier.radius,
            },
            target: None,
            owner: Owner::Member(i),
            direction,
            remaining,
            speed,
            raw: budget(tier.contact_damage_bps),
            metal: false,
            hit_statuses: vec![],
            created: now,
            expires,
        })
    }

    pub(super) fn start_charged_flight(&mut self, mut shot: Shot, events: &mut Events) {
        shot.view.id = self.next_id("basic-flight");
        events.push((
            None,
            EventV4::ProjectileStarted {
                projectile: shot.view.clone(),
            },
        ));
        self.shots.push(shot);
    }

    pub(super) fn advance_charged_flight(
        &mut self,
        mut shot: Shot,
        c: &Config,
        now: u64,
        dt: f64,
        events: &mut Events,
    ) {
        let mut flight = shot.flight.take().expect("charged flight state");
        let start = shot.view.position;
        let travel = (shot.speed * dt).min(shot.remaining);
        let arrives = shot.remaining - travel <= 1e-9;
        let point = shot.view.basic_segment.as_ref().unwrap().aim_point;
        let at = |t| {
            [
                start[0] + shot.direction[0] * travel * t,
                start[1] + shot.direction[1] * travel * t,
            ]
        };
        let end = if arrives { point } else { at(1.) };
        let wall = self.grid.piercing_wall(start, end, shot.view.radius);
        let mut targets: Vec<_> = self
            .hostile_sweeps(&shot.view.source, (start, end), shot.view.radius, c)
            .filter(|(t, _, _)| wall.is_none_or(|w| *t < w))
            .map(|(t, e, owner)| (t, e.clone(), owner))
            .collect();
        targets.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.id.cmp(&b.1.id))
                .then(a.1.generation.cmp(&b.1.generation))
        });
        let mut defs = Self::definitions(c, std::slice::from_ref(&flight.status_id));
        defs[0].periodic_added_stacks = flight.contact_stacks;
        for (t, target, owner) in targets {
            let previous = flight
                .contacts
                .iter()
                .position(|entry| entry.target == target);
            if previous.map_or(flight.contacts.len() >= flight.target_cap, |i| {
                flight.contacts[i].hits >= flight.contact_limit || now < flight.contacts[i].next_at
            }) {
                continue;
            }
            if self.charged_flight_hit(&shot, (owner, &target, shot.raw, &defs), c, now, events) {
                let next_at = now + flight.contact_interval_ms;
                let target_hits = if let Some(i) = previous {
                    flight.contacts[i].next_at = next_at;
                    flight.contacts[i].hits += 1;
                    flight.contacts[i].hits
                } else {
                    flight.contacts.push(TargetContacts {
                        target: target.clone(),
                        hits: 1,
                        next_at,
                    });
                    1
                };
                events.push((
                    None,
                    EventV4::BasicFlightContact {
                        projectile_id: shot.view.id.clone(),
                        source: shot.view.source.clone(),
                        target,
                        position: at(t),
                        hit_at_ms: now,
                        target_hits,
                    },
                ));
            }
            if !self.valid_status_entity(&status_entity(&shot.view.source)) {
                shot.end(ProjectileEndReasonV4::SourceInvalid, events);
                return;
            }
        }
        if wall.is_some() {
            shot.end(ProjectileEndReasonV4::Wall, events);
        } else if arrives {
            defs[0].periodic_added_stacks = flight.burst_stacks;
            self.charged_flight_burst(&shot, (point, &flight, &defs), c, now, events);
            shot.end(ProjectileEndReasonV4::RangeExhausted, events);
        } else {
            shot.view.position = end;
            shot.remaining -= travel;
            shot.flight = Some(flight);
            self.shots.push(shot);
        }
    }

    fn charged_flight_hit(
        &mut self,
        shot: &Shot,
        (owner, target, raw, defs): (Owner, &EntityRef, u32, &[crate::config::StatusDefinition]),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let hit = self
            .direct_contact_damage(
                shot.owner,
                owner,
                combat::DirectContact {
                    origin: Some(shot.origin.clone()),
                    action_id: Some(&shot.action_id),
                    rule: shot.damage_rule.as_ref(),
                    ignore_armor: shot.ignore_armor,
                    unavoidable: shot.unavoidable,
                    dead_source: false,
                    source: &shot.view.source,
                    raw,
                    metal: shot.metal,
                    hit_statuses: &[],
                },
                c,
                now,
                events,
            )
            .is_some();
        if hit && self.hostile_contact(&shot.view.source, target) {
            self.apply_owned_status_definitions(shot.owner, target.clone(), defs, c, now, events);
        }
        hit
    }

    fn charged_flight_burst(
        &mut self,
        shot: &Shot,
        (center, flight, defs): (
            [f64; 2],
            &FlightContactPlan,
            &[crate::config::StatusDefinition],
        ),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let mut candidates: Vec<_> = self
            .npcs
            .iter()
            .enumerate()
            .filter(|(_, npc)| {
                self.hostile_contact(&shot.view.source, &npc.entity)
                    && self.basic_circle_contact(center, flight.burst_radius, npc)
            })
            .map(|(i, npc)| {
                (
                    distance(center, npc.position),
                    npc.entity.clone(),
                    Owner::Npc(i),
                )
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        let mut targets = vec![];
        for (_, target, owner) in candidates.into_iter().take(flight.target_cap) {
            if !self.valid_status_entity(&status_entity(&shot.view.source)) {
                break;
            }
            if self.charged_flight_hit(
                shot,
                (owner, &target, flight.burst_raw, defs),
                c,
                now,
                events,
            ) {
                targets.push(target);
            }
        }
        events.push((
            None,
            EventV4::BasicFlightBurst {
                projectile_id: shot.view.id.clone(),
                source: shot.view.source.clone(),
                center,
                radius: flight.burst_radius,
                targets,
            },
        ));
    }
}

#[cfg(all(test, match_sdk_test_data))]
impl World {
    pub(crate) fn grant_npc_test_shield(&mut self, index: usize, c: &Config, now: u64) {
        let entity = self.npcs[index].entity.clone();
        let defs = Self::definitions(c, &["shield-absorption".to_owned()]);
        self.apply_owned_status_definitions(Owner::Npc(index), entity, &defs, c, now, &mut vec![]);
    }
}
