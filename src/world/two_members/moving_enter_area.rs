//! A linear area uses the shared area pool and contact budget, with entry edges instead of pulses.
use super::*;
use crate::config::{StatusDefinition, moving_enter_area::NpcEnterArea};
use std::collections::BTreeSet;

pub(super) struct MovingEnterArea {
    rule: NpcEnterArea,
    cast_origin: [f64; 2],
    inside: BTreeSet<usize>,
    retained_statuses: Vec<StatusDefinition>,
    released_accuracy: Option<numeric::AttackAccuracy>,
}

impl MovingEnterArea {
    pub(super) fn survives_source_loss(&self) -> bool {
        !self.rule.end_on_source_invalid
    }
}

impl World {
    pub(crate) fn prepare_enter_area(
        &mut self,
        j: usize,
        (origin, direction): ([f64; 2], [f64; 2]),
        rule: &NpcEnterArea,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let npc = &self.npcs[j];
        let s = &npc.profile.combat.projectile;
        let cast_origin = npc.position;
        if !self.npc_area_slot_available(c) || !self.valid_npc_area_point(j, origin) {
            return false;
        }
        let map = &self.grid.config;
        let mut travel = rule.max_distance;
        if let Some(limit) = map.isometric_ray_limit(origin, direction, 0.) {
            travel = travel.min((limit - 1e-6).max(0.));
        } else {
            let (_, size) = map.world_bounds();
            for axis in 0..2 {
                if direction[axis] > 0. {
                    travel = travel.min((size[axis] - 1e-6 - origin[axis]) / direction[axis]);
                } else if direction[axis] < 0. {
                    travel = travel.min(-origin[axis] / direction[axis]);
                }
            }
        }
        let skill_id = s.id.clone();
        let hit_statuses = s.on_hit_target_statuses.clone();
        let mut retained_statuses = Self::definitions(c, &rule.retained_hit_statuses);
        for definition in &mut retained_statuses {
            definition.retained_source = true;
        }
        let unavoidable = s.unavoidable == Some(true);
        let source = npc.entity.clone();
        let activates_at_ms = now + s.windup_ms;
        let id = self.next_id("enter-area");
        let view = AreaPlanViewV4 {
            attached_anchor_id: None,
            anchor_recast: None,
            movement: Some(AreaMovementView {
                origin,
                direction,
                speed: rule.speed,
                distance: travel.max(0.),
                committed: false,
            }),
            basic_action_id: None,
            line: None,
            id: id.clone(),
            source,
            skill_id,
            mechanism: AreaPlanKind::MovingEnter,
            center: origin,
            radius: rule.radius,
            created_at_ms: now,
            activates_at_ms,
            expires_at_ms: activates_at_ms + rule.duration_ms,
            interval_ms: 0,
            next_tick_at_ms: None,
            completed_ticks: 0,
            total_ticks: 0,
            contact_policy: None,
        };
        let plan = AreaPlan {
            action_id: id,
            damage_rule: None,
            ignore_armor: false,
            unavoidable,
            view,
            raw: 0,
            hit_statuses,
            contact: Some(ContactBudget::new(rule.contact_policy.clone())),
            basic: None,
            moving: Some(MovingEnterArea {
                rule: rule.clone(),
                cast_origin,
                inside: BTreeSet::new(),
                retained_statuses,
                released_accuracy: None,
            }),
            ground: None,
            trap_occupants: Default::default(),
        };
        events.push((None, EventV4::AreaStarted { area: plan.view() }));
        self.areas.push(plan);
        true
    }

    pub(super) fn commit_enter_area(
        &mut self,
        index: usize,
        j: usize,
        c: &Config,
        now: u64,
    ) -> String {
        let accuracy = (!self.areas[index].unavoidable
            && !self.areas[index]
                .moving
                .as_ref()
                .unwrap()
                .rule
                .end_on_source_invalid)
            .then(|| self.configured_accuracy(Owner::Npc(j), c, now));
        let plan = &mut self.areas[index];
        plan.moving.as_mut().unwrap().released_accuracy = accuracy;
        plan.view.movement.as_mut().unwrap().committed = true;
        plan.view.id.clone()
    }

    pub(super) fn step_enter_area(
        &mut self,
        p: &mut AreaPlan,
        previous: &[[f64; 2]],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<AreaEndReason> {
        let moving = p.moving.as_mut().unwrap();
        let view = &mut p.view;
        let motion = view.movement.as_ref().unwrap();
        let source = self.live_npc_area_source(&view.source);
        if !motion.committed {
            let Some(j) = source else {
                return Some(AreaEndReason::SourceInvalid);
            };
            return (self.npcs[j].casting.is_none() || now >= view.activates_at_ms)
                .then_some(AreaEndReason::Cancelled);
        }
        if source.is_none() && moving.rule.end_on_source_invalid {
            return Some(AreaEndReason::SourceInvalid);
        }
        if now < view.activates_at_ms {
            return None;
        }
        let start = view.center;
        let at = now.min(view.expires_at_ms);
        let travel =
            (motion.speed * (at - view.activates_at_ms) as f64 / 1000.).min(motion.distance);
        let end = [
            motion.origin[0] + motion.direction[0] * travel,
            motion.origin[1] + motion.direction[1] * travel,
        ];
        let radius = view.radius + c.params.player_radius;
        let budget = p.contact.as_mut().unwrap();
        {
            let last = now.saturating_sub(c.step_ms);
            let fraction = |at: u64| at.saturating_sub(last) as f64 / c.step_ms as f64;
            let mut candidates = Vec::new();
            for (i, member) in self.members.iter().enumerate() {
                if !member.eligible() || moving.inside.contains(&i) {
                    continue;
                }
                let position_at = |at| {
                    let t = fraction(at);
                    std::array::from_fn(|axis| {
                        previous[i][axis] + (member.position[axis] - previous[i][axis]) * t
                    })
                };
                let to = relative(position_at(at), end);
                let before = position_at(view.activates_at_ms.max(last));
                if let Some(t) = segment_circle(relative(before, start), to, [0., 0.], radius) {
                    candidates.push((t, to[0].hypot(to[1]), i));
                }
            }
            candidates.sort_by(|a, b| {
                a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(
                    self.members[a.2]
                        .reference
                        .entity
                        .id
                        .cmp(&self.members[b.2].reference.entity.id),
                )
            });
            let mut hits = 0;
            for (_, _, i) in candidates {
                if hits >= budget.policy().max_targets_per_tick || budget.remaining() == 0 {
                    break;
                }
                let target = self.members[i].reference.entity.clone();
                let key = (0, target.id.clone(), target.generation);
                if !budget.allows(&key, now) || !self.members[i].eligible() {
                    continue;
                }
                if moving.rule.end_on_source_invalid
                    && self.live_npc_area_source(&view.source).is_none()
                {
                    return Some(AreaEndReason::SourceInvalid);
                }
                let hit = self.npc_area_contact(
                    NpcAreaHit {
                        area: view,
                        target: i,
                        max_hp_bps: moving.rule.target_max_hp_bps,
                        unavoidable: p.unavoidable,
                        end_on_source_invalid: moving.rule.end_on_source_invalid,
                        released_accuracy: moving.released_accuracy,
                    },
                    c,
                    now,
                    events,
                );
                if !hit {
                    continue;
                }
                budget.record(key, now);
                hits += 1;
                events.push((
                    None,
                    EventV4::AreaEntered {
                        area_id: view.id.clone(),
                        source: view.source.clone(),
                        target: target.clone(),
                        at_ms: now,
                    },
                ));
                if self.members[i].living() {
                    if !self.member_has_status(i, StatusAttribute::ControlImmunity, now) {
                        self.displace_area_target(i, view, moving, c, events);
                    }
                    if let Some(j) = self.live_npc_area_source(&view.source) {
                        let mut defs = Self::definitions(c, &p.hit_statuses);
                        defs.extend_from_slice(&moving.retained_statuses);
                        if !defs.is_empty() {
                            self.apply_owned_status_definitions(
                                Owner::Npc(j),
                                target,
                                &defs,
                                c,
                                now,
                                events,
                            );
                        }
                    } else if !moving.rule.end_on_source_invalid {
                        self.apply_detached_area_statuses(
                            &view.source,
                            target,
                            &moving.retained_statuses,
                            c,
                            now,
                            events,
                        );
                    }
                }
            }
        }
        view.center = end;
        moving.inside.clear();
        moving.inside.extend(
            self.members
                .iter()
                .enumerate()
                .filter(|(_, m)| distance(m.position, end) <= radius)
                .map(|(i, _)| i),
        );
        if moving.rule.end_on_source_invalid && self.live_npc_area_source(&view.source).is_none() {
            Some(AreaEndReason::SourceInvalid)
        } else if budget.remaining() == 0 {
            Some(AreaEndReason::HitBudgetExhausted)
        } else if travel >= motion.distance {
            Some(AreaEndReason::Resolved)
        } else {
            (now >= view.expires_at_ms).then_some(AreaEndReason::Expired)
        }
    }

    fn displace_area_target(
        &mut self,
        i: usize,
        view: &AreaPlanViewV4,
        moving: &MovingEnterArea,
        c: &Config,
        events: &mut Events,
    ) {
        let member = &mut self.members[i];
        let from = member.position;
        let dir = normalize(relative(from, moving.cast_origin));
        if dir == [0., 0.] {
            return;
        }
        let length = moving.rule.knockback.distance;
        let end = [from[0] + dir[0] * length, from[1] + dir[1] * length];
        let to = self
            .grid
            .walk_until_blocked(from, end, c.params.player_radius);
        if to != from {
            member.position = to;
            member.path.clear();
            events.push((
                None,
                EventV4::ForcedDisplacement {
                    area_id: view.id.clone(),
                    source: view.source.clone(),
                    target: member.reference.entity.clone(),
                    from,
                    to,
                    slide_speed_multiplier: moving.rule.knockback.slide_speed_multiplier,
                },
            ));
        }
    }
}

fn relative(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
