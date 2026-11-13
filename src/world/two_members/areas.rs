//! One bounded area plan owns timing, contact budget and lifetime for both skill forms.
use super::*;
use crate::contact::ContactBudget;
use std::collections::BTreeSet;
#[path = "basic_fields.rs"]
pub(super) mod basic_fields;
use basic_fields::BasicFieldState;
#[path = "moving_enter_area.rs"]
mod moving_enter_area;
use moving_enter_area::MovingEnterArea;
#[path = "npc_ground_field.rs"]
mod npc_ground_field;
use npc_ground_field::GroundArea;

pub(super) enum AnchorRecastIntent {
    Relocate([f64; 2]),
    ReturnToSource,
    End,
}

pub(super) struct AreaPlan {
    action_id: String,
    damage_rule: Option<crate::config::AttackRule>,
    ignore_armor: bool,
    unavoidable: bool,
    view: AreaPlanViewV4,
    raw: u32,
    hit_statuses: Vec<String>,
    contact: Option<ContactBudget<(u8, String, u32)>>,
    basic: Option<BasicFieldState>,
    moving: Option<MovingEnterArea>,
    ground: Option<GroundArea>,
    trap_occupants: BTreeSet<(String, u32)>,
}

struct NpcAreaHit<'a> {
    area: &'a AreaPlanViewV4,
    target: usize,
    max_hp_bps: u32,
    unavoidable: bool,
    end_on_source_invalid: bool,
    released_accuracy: Option<numeric::AttackAccuracy>,
}
impl AreaPlan {
    pub(super) fn trigger_source(&self) -> &EntityRef {
        &self.view.source
    }
    pub(super) fn trigger_action_id(&self) -> Option<&str> {
        self.basic.is_none().then_some(&self.action_id)
    }

    pub(super) fn is_basic(&self) -> bool {
        self.basic.is_some()
    }

    pub(super) fn is_moving(&self) -> bool {
        self.moving.is_some()
    }

    pub(super) fn is_npc_area(&self) -> bool {
        self.moving.is_some() || self.ground.is_some()
    }

    pub(super) fn is_trap(&self) -> bool {
        self.view.mechanism == AreaPlanKind::MemberGroundTrap
    }

    pub(super) fn is_anchor(&self) -> bool {
        self.view.mechanism == AreaPlanKind::MemberCastAnchor
    }

    pub(super) fn follows_anchor(&self) -> bool {
        self.view.attached_anchor_id.is_some()
    }

    fn npc_committed(&self) -> bool {
        self.view.movement.as_ref().is_some_and(|m| m.committed)
            || self.ground.as_ref().is_some_and(|g| g.committed)
    }

    fn survives_source_loss(&self) -> bool {
        self.moving
            .as_ref()
            .is_some_and(MovingEnterArea::survives_source_loss)
            || self
                .ground
                .as_ref()
                .is_some_and(|g| g.delivery.survives_source_loss())
    }

    pub(super) fn view(&self) -> AreaPlanViewV4 {
        let mut v = self.view.clone();
        v.contact_policy = self.contact.as_ref().map(|b| FieldContactPolicyView {
            max_targets_per_tick: b.policy().max_targets_per_tick,
            max_total_hits: b.policy().max_total_hits,
            repeat_interval_ms: b.policy().repeat_interval_ms,
            used_hits: b.used(),
            remaining_hits: b.remaining(),
            contacts: b
                .history()
                .map(|((_, id, generation), last)| FieldContactView {
                    target: UnitTarget {
                        id: id.clone(),
                        generation: *generation,
                    },
                    last_hit_at_ms: *last,
                    next_eligible_at_ms: (b.policy().repeat_interval_ms > 0)
                        .then(|| last.saturating_add(b.policy().repeat_interval_ms)),
                })
                .collect(),
        });
        v
    }
}
impl World {
    pub(super) fn member_anchor_index(&self, i: usize, skill_id: &str) -> Option<usize> {
        let source = &self.members[i].reference.entity;
        self.areas
            .iter()
            .position(|p| p.is_anchor() && p.view.source == *source && p.view.skill_id == skill_id)
    }

    pub(super) fn cast_anchor_target(
        &self,
        i: usize,
        skill: &SkillConfig,
        anchor_id: &str,
        c: &Config,
        now: u64,
    ) -> Result<([f64; 2], bool)> {
        let member = &self.members[i];
        let anchor = self
            .areas
            .iter()
            .find(|p| {
                p.is_anchor()
                    && p.view.id == anchor_id
                    && p.view.source == member.reference.entity
                    && now < p.view.expires_at_ms
            })
            .ok_or(Error::InvalidTarget)?;
        let rule = c
            .member_cast_anchor(&anchor.view.skill_id)
            .filter(|rule| rule.consumer_skill_ids.contains(&skill.id))
            .ok_or(Error::InvalidTarget)?;
        let point = anchor.view.center;
        self.check_ground_bounds(point)?;
        if !self.grid.clear(point, 0., 0)
            || (!skill.is_ground() && !self.grid.clear(point, skill.radius, 1))
        {
            return Err(Error::InvalidTarget);
        }
        self.check_anchor_reach(i, point, rule.max_use_distance)?;
        Ok((point, rule.following_field_skill_ids.contains(&skill.id)))
    }

    fn check_anchor_reach(&self, i: usize, point: [f64; 2], range: f64) -> Result<()> {
        let origin = self.members[i].position;
        if distance(origin, point) > range || !self.grid.line(origin, point, 0., 2) {
            return Err(Error::OutOfRange);
        }
        Ok(())
    }

    pub(super) fn recast_anchor(
        &mut self,
        i: usize,
        (anchor_id, action_id, recast_index): (&str, &str, u32),
        intent: AnchorRecastIntent,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let member = &self.members[i];
        let index = self
            .areas
            .iter()
            .position(|area| {
                area.view.id == anchor_id
                    && area.view.source == member.reference.entity
                    && area.view.anchor_recast.as_ref().is_some_and(|recast| {
                        recast.action_id == action_id
                            && now < recast.expires_at_ms
                            && recast.used_count < recast.max_count
                            && recast_index == recast.used_count + 1
                    })
            })
            .ok_or(Error::InvalidTarget)?;
        if member.pending.is_some()
            || member.channel.is_some()
            || member.basic_pursuit.is_some()
            || member.extract_end.is_some()
            || now < member.recovery_until
        {
            return Err(Error::InvalidState);
        }
        let anchor = &self.areas[index].view;
        let skill_id = &anchor.skill_id;
        if !member.skill_equipped(skill_id, c) {
            return Err(Error::InvalidTarget);
        }
        let skill = c.player_skill(skill_id).expect("validated anchor skill");
        let rule = c
            .member_cast_anchor(skill_id)
            .expect("validated anchor rule");
        let recast = rule.recast.as_ref().expect("validated anchor recast");
        let (behavior, destination) = match intent {
            AnchorRecastIntent::Relocate(position) => {
                (AnchorRecastBehavior::Relocate, Some(position))
            }
            AnchorRecastIntent::ReturnToSource => {
                (AnchorRecastBehavior::ReturnToSource, Some(member.position))
            }
            AnchorRecastIntent::End => (AnchorRecastBehavior::End, None),
        };
        if !recast.permits(behavior) {
            return Err(Error::Unsupported);
        }
        self.check_anchor_reach(i, anchor.center, rule.max_use_distance)?;
        let center = destination
            .map(|point| self.configured_ground_target(i, skill, point, c))
            .transpose()?;
        if center == Some(anchor.center) {
            return Err(Error::InvalidTarget);
        }
        let cost = member.effective_spirit_cost(recast.spirit_cost);
        if member.spirit < cost {
            return Err(Error::NoResource);
        }
        self.members[i].spirit -= cost;
        let Some(center) = center else {
            events.push((
                None,
                EventV4::AnchorRecastEnded {
                    area_id: anchor.id.clone(),
                    action_id: action_id.into(),
                    source: anchor.source.clone(),
                    recast_index,
                },
            ));
            self.end_anchor(index, AreaEndReason::RecastConsumed, events);
            return Ok(());
        };
        let anchor = &mut self.areas[index].view;
        anchor.center = center;
        anchor.anchor_recast.as_mut().unwrap().used_count = recast_index;
        events.push((
            None,
            EventV4::AnchorRelocated {
                area_id: anchor.id.clone(),
                action_id: action_id.into(),
                source: anchor.source.clone(),
                recast_index,
                center,
            },
        ));
        for field in &mut self.areas {
            if field.view.attached_anchor_id.as_deref() == Some(anchor_id) {
                field.view.center = center;
            }
        }
        Ok(())
    }

    pub(super) fn valid_npc_area_point(&self, npc: usize, point: [f64; 2]) -> bool {
        self.grid.clear(point, 0., 0)
            && self
                .grid
                .piercing_wall(self.npcs[npc].position, point, 0.)
                .is_none()
    }

    pub(super) fn npc_area_slot_available(&self, c: &Config) -> bool {
        self.areas.iter().filter(|a| !a.is_basic()).count()
            < c.members.as_ref().unwrap().area_capacity.unwrap()
    }

    pub(crate) fn prepared_npc_area(&self, j: usize) -> Option<usize> {
        let npc = &self.npcs[j];
        self.areas.iter().position(|p| {
            p.view.source == npc.entity
                && p.is_npc_area()
                && !p.npc_committed()
                && self.valid_npc_area_point(j, p.view.center)
                && (p.ground.is_none()
                    || distance(npc.position, p.view.center) <= npc.profile.combat.projectile.range
                        && self.grid.line(npc.position, p.view.center, 0., 2))
        })
    }

    pub(crate) fn commit_npc_area(
        &mut self,
        index: usize,
        j: usize,
        c: &Config,
        now: u64,
    ) -> String {
        if let Some(ground) = &self.areas[index].ground {
            let accuracy = (!self.areas[index].unavoidable
                && ground.delivery.survives_source_loss())
            .then(|| self.configured_accuracy(Owner::Npc(j), c, now));
            let ground = self.areas[index].ground.as_mut().unwrap();
            ground.released_accuracy = accuracy;
            ground.committed = true;
            self.areas[index].view.id.clone()
        } else {
            self.commit_enter_area(index, j, c, now)
        }
    }

    fn live_npc_area_source(&self, source: &EntityRef) -> Option<usize> {
        match self.resolve_entity(source) {
            Some(Owner::Npc(j)) if self.npcs[j].hp > 0 => Some(j),
            _ => None,
        }
    }

    fn npc_area_contact(
        &mut self,
        hit: NpcAreaHit<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let source = &hit.area.source;
        let target = &self.members[hit.target].reference.entity;
        let max = self.entity_max_hp(target, c, now).unwrap();
        let amount = u64::from(max) * u64::from(hit.max_hp_bps) / 10000;
        let target_owner = Owner::Member(hit.target);
        let reduction = self.effective_damage_reduction(target_owner, target, c, now);
        let raw = (amount * u64::from(10000 - reduction) / 10000) as u32;
        match self.live_npc_area_source(source) {
            Some(j) => self.direct_contact(
                Owner::Npc(j),
                target_owner,
                combat::DirectContact {
                    origin: Some(crate::origin::ActionOrigin::Skill(
                        hit.area.skill_id.clone(),
                    )),
                    action_id: Some(&hit.area.id),
                    rule: Some(&crate::config::AttackRule {
                        damage_kind: crate::config::DamageKind::TrueDamage,
                        can_crit: false,
                        ..Default::default()
                    }),
                    ignore_armor: false,
                    unavoidable: hit.unavoidable,
                    dead_source: false,
                    source,
                    raw,
                    metal: false,
                    hit_statuses: &[],
                },
                c,
                now,
                events,
            ),
            None if !hit.end_on_source_invalid => self.detached_area_contact(
                (hit.target, source, raw),
                hit.released_accuracy,
                c,
                now,
                events,
            ),
            None => false,
        }
    }

    pub(super) fn line_plan(
        &self,
        i: usize,
        s: &SkillConfig,
        direction: [f64; 2],
    ) -> Result<LineFieldView> {
        let start = self.members[i].position;
        self.projectile_origin(i, s.radius)?;
        let requested = [
            start[0] + direction[0] * s.range,
            start[1] + direction[1] * s.range,
        ];
        let t = self
            .grid
            .piercing_wall(start, requested, s.radius)
            .unwrap_or(1.);
        Ok(LineFieldView {
            start,
            end: [
                start[0] + (requested[0] - start[0]) * t,
                start[1] + (requested[1] - start[1]) * t,
            ],
            width: 2. * s.radius,
        })
    }
    pub(super) fn configured_ground_target(
        &self,
        i: usize,
        skill: &SkillConfig,
        point: [f64; 2],
        c: &Config,
    ) -> Result<[f64; 2]> {
        if !point.iter().all(|x| x.is_finite()) {
            return Err(Error::InvalidMessage);
        }
        let mut point = point;
        if c.combat_rules
            .as_ref()
            .and_then(|r| r.ground_target_policies.get(&skill.id))
            .is_some_and(|p| matches!(p, crate::config::GroundTargetPolicy::ClampToRange))
        {
            let origin = self.members[i].position;
            let delta = [point[0] - origin[0], point[1] - origin[1]];
            let length = delta[0].hypot(delta[1]);
            if !length.is_finite() {
                return Err(Error::InvalidMessage);
            }
            if length > skill.range {
                point = [
                    origin[0] + delta[0] / length * skill.range,
                    origin[1] + delta[1] / length * skill.range,
                ];
            }
        }
        self.ground_target(i, skill, point)?;
        Ok(point)
    }
    pub(super) fn ground_target(&self, i: usize, skill: &SkillConfig, p: [f64; 2]) -> Result<()> {
        self.check_ground_bounds(p)?;
        if !self.grid.clear(p, 0., 0) {
            return Err(Error::InvalidTarget);
        }
        if distance(self.members[i].position, p) > skill.range
            || !self.grid.line(self.members[i].position, p, 0., 2)
        {
            return Err(Error::OutOfRange);
        }
        Ok(())
    }

    pub(super) fn check_ground_bounds(&self, p: [f64; 2]) -> Result<()> {
        if !p.iter().all(|n| n.is_finite()) {
            return Err(Error::InvalidMessage);
        }
        if self.grid.index(p).is_none() {
            return Err(Error::InvalidTarget);
        }
        Ok(())
    }
    pub(super) fn start_area(
        &mut self,
        i: usize,
        s: &SkillConfig,
        geometry: ([f64; 2], Option<LineFieldView>, Option<String>),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let (center, line, attached_anchor_id) = geometry;
        let field = s.is_field();
        let trap = c.member_ground_trap(&s.id);
        let anchor = c.member_cast_anchor(&s.id);
        if anchor.is_some()
            && let Some(index) = self.member_anchor_index(i, &s.id)
        {
            let reason = if now >= self.areas[index].view.expires_at_ms {
                AreaEndReason::Expired
            } else {
                AreaEndReason::Replaced
            };
            self.end_anchor(index, reason, events);
        }
        let inert = trap.is_some() || anchor.is_some();
        let activate = now + s.warning_ms;
        let id = self.next_id("area");
        let source = self.members[i].reference.entity.clone();
        let action_id = self
            .triggers
            .action
            .clone()
            .filter(|_| self.triggers.action_source.as_ref() == Some(&source))
            .unwrap_or_else(|| id.clone());
        let view = AreaPlanViewV4 {
            attached_anchor_id,
            anchor_recast: anchor.and_then(|rule| rule.recast.as_ref()).map(|recast| {
                AnchorRecastView {
                    action_id: action_id.clone(),
                    expires_at_ms: now + recast.window_ms,
                    max_count: recast.max_count,
                    used_count: 0,
                    behaviors: recast.behaviors.clone(),
                }
            }),
            movement: None,
            basic_action_id: None,
            line,
            id,
            source,
            skill_id: s.id.clone(),
            mechanism: if anchor.is_some() {
                AreaPlanKind::MemberCastAnchor
            } else if trap.is_some() {
                AreaPlanKind::MemberGroundTrap
            } else if s.is_line() {
                AreaPlanKind::LineField
            } else if field {
                AreaPlanKind::Field
            } else {
                AreaPlanKind::Circle
            },
            center,
            radius: if anchor.is_some() { 0. } else { s.radius },
            created_at_ms: now,
            activates_at_ms: activate,
            expires_at_ms: activate
                + anchor
                    .map(|rule| rule.duration_ms)
                    .or_else(|| trap.map(|rule| rule.duration_ms))
                    .unwrap_or(if field { s.duration_ms } else { 0 }),
            interval_ms: if field { s.tick_interval_ms } else { 0 },
            next_tick_at_ms: (!inert)
                .then_some(activate + if field { s.tick_interval_ms } else { 0 }),
            completed_ticks: 0,
            total_ticks: if inert {
                0
            } else if field {
                ((s.duration_ms - 1) / s.tick_interval_ms) as u32
            } else {
                1
            },
            contact_policy: None,
        };
        let raw = if anchor.is_some() {
            0
        } else {
            self.configured_budget(i, Some(s), c, now)
        };
        let p = AreaPlan {
            action_id,
            damage_rule: self.attack_rule(Owner::Member(i), Some(&s.id), c),
            ignore_armor: c.mechanism_version >= 103 && s.ignore_armor == Some(true),
            unavoidable: c.mechanism_version >= 102 && s.unavoidable == Some(true),
            view,
            raw,
            hit_statuses: s.on_hit_target_statuses.clone(),
            contact: trap
                .map(|rule| &rule.contact_policy)
                .or(s.contact_policy.as_ref())
                .cloned()
                .map(ContactBudget::new),
            basic: None,
            moving: None,
            ground: None,
            trap_occupants: BTreeSet::new(),
        };
        events.push((None, EventV4::AreaStarted { area: p.view() }));
        self.areas.push(p);
    }

    fn end_anchor(&mut self, index: usize, reason: AreaEndReason, events: &mut Events) {
        let id = self.areas.remove(index).view.id;
        self.areas.retain(|field| {
            if field.view.attached_anchor_id.as_deref() != Some(id.as_str()) {
                return true;
            }
            events.push((
                None,
                EventV4::AreaEnded {
                    area_id: field.view.id.clone(),
                    reason: AreaEndReason::AnchorInvalid,
                },
            ));
            false
        });
        events.push((
            None,
            EventV4::AreaEnded {
                area_id: id,
                reason,
            },
        ));
    }

    pub(super) fn clear_areas(&mut self, source: &EntityRef, events: &mut Events) {
        self.areas.retain(|p| {
            if p.view.source == *source {
                events.push((
                    None,
                    EventV4::AreaEnded {
                        area_id: p.view.id.clone(),
                        reason: AreaEndReason::SourceInvalid,
                    },
                ));
                false
            } else {
                true
            }
        });
    }
    pub(super) fn stop_npc_areas(
        &mut self,
        source: &EntityRef,
        reason: AreaEndReason,
        events: &mut Events,
    ) {
        self.areas.retain(|plan| {
            if plan.view.source != *source
                || !plan.is_npc_area()
                || plan.npc_committed()
                    && (reason != AreaEndReason::SourceInvalid || plan.survives_source_loss())
            {
                return true;
            }
            events.push((
                None,
                EventV4::AreaEnded {
                    area_id: plan.view.id.clone(),
                    reason,
                },
            ));
            false
        });
    }
    pub(crate) fn advance_npc_areas(
        &mut self,
        previous: Option<&[[f64; 2]]>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let ids: Vec<_> = self
            .areas
            .iter()
            .filter(|p| p.is_npc_area())
            .map(|p| p.view.id.clone())
            .collect();
        for id in ids {
            let Some(index) = self.areas.iter().position(|p| p.view.id == id) else {
                continue;
            };
            let mut plan = self.areas.remove(index);
            let end = if plan.is_moving() {
                // A field prepared this step needs no sweep before it commits.
                previous.and_then(|before| self.step_enter_area(&mut plan, before, c, now, events))
            } else {
                self.step_ground_area(&mut plan, c, now, events)
            };
            if let Some(reason) = end {
                events.push((
                    None,
                    EventV4::AreaEnded {
                        area_id: id,
                        reason,
                    },
                ));
            } else {
                self.areas.insert(index.min(self.areas.len()), plan);
            }
        }
    }

    pub(super) fn advance_areas(&mut self, c: &Config, now: u64, events: &mut Events) {
        let invalid = self
            .areas
            .iter()
            .filter(|p| {
                !p.is_npc_area() && !self.valid_status_entity(&status_entity(&p.view.source))
            })
            .map(|p| p.view.source.clone())
            .collect::<Vec<_>>();
        for source in invalid {
            self.clear_areas(&source, events);
        }
        while let Some(index) = self.areas.iter().position(|p| {
            p.is_anchor()
                && now >= p.view.expires_at_ms
                && self.areas.iter().any(|field| {
                    field.view.attached_anchor_id.as_deref() == Some(p.view.id.as_str())
                })
        }) {
            self.end_anchor(index, AreaEndReason::Expired, events);
        }
        self.advance_member_traps(c, now, events);
        while let Some(i) = self
            .areas
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                !p.is_npc_area()
                    && p.view.created_at_ms < now
                    && p.view.next_tick_at_ms.is_some_and(|at| at <= now)
            })
            .min_by_key(|(i, p)| (p.view.next_tick_at_ms.unwrap(), *i))
            .map(|(i, _)| i)
        {
            if self.areas[i].is_basic() {
                self.advance_basic_field(i, c, now, events);
                continue;
            }
            let mut p = self.areas.remove(i);
            let scheduled = p.view.next_tick_at_ms.unwrap();
            let Some(owner) = self.resolve_entity(&p.view.source) else {
                continue;
            };
            let targets = self.resolve_area_contacts(&mut p, owner, (scheduled, now), c, events);
            p.view.completed_ticks += 1;
            p.view.next_tick_at_ms = (p.view.mechanism != AreaPlanKind::Circle
                && p.view.completed_ticks < p.view.total_ticks)
                .then(|| scheduled + p.view.interval_ms);
            events.push((
                None,
                EventV4::AreaTick {
                    area_id: p.view.id.clone(),
                    tick_index: p.view.completed_ticks,
                    scheduled_at_ms: scheduled,
                    targets,
                },
            ));
            let end = if p.view.mechanism == AreaPlanKind::Circle {
                Some(AreaEndReason::Resolved)
            } else if p.follows_anchor()
                && !self.valid_status_entity(&status_entity(&p.view.source))
            {
                Some(AreaEndReason::SourceInvalid)
            } else if p.contact.as_ref().is_some_and(|b| b.remaining() == 0) {
                Some(AreaEndReason::HitBudgetExhausted)
            } else {
                None
            };
            if let Some(reason) = end {
                events.push((
                    None,
                    EventV4::AreaEnded {
                        area_id: p.view.id,
                        reason,
                    },
                ));
            } else {
                self.areas.insert(i.min(self.areas.len()), p);
            }
        }
        self.areas.retain(|p| {
            if !p.is_npc_area()
                && p.view.mechanism != AreaPlanKind::Circle
                && now >= p.view.expires_at_ms
            {
                events.push((
                    None,
                    EventV4::AreaEnded {
                        area_id: p.view.id.clone(),
                        reason: AreaEndReason::Expired,
                    },
                ));
                false
            } else {
                true
            }
        });
    }

    fn advance_member_traps(&mut self, c: &Config, now: u64, events: &mut Events) {
        let ready: Vec<_> = self
            .areas
            .iter()
            .filter(|plan| {
                plan.is_trap() && plan.view.activates_at_ms <= now && now < plan.view.expires_at_ms
            })
            .map(|p| p.view.id.clone())
            .collect();
        for id in ready {
            let Some(index) = self.areas.iter().position(|p| p.view.id == id) else {
                continue;
            };
            let mut plan = self.areas.remove(index);
            let source = plan.view.source.clone();
            let end = if let Some(owner) = self.resolve_entity(&source) {
                for target in self.resolve_area_contacts(&mut plan, owner, (now, now), c, events) {
                    events.push((
                        None,
                        EventV4::AreaEntered {
                            area_id: id.clone(),
                            source: source.clone(),
                            target,
                            at_ms: now,
                        },
                    ));
                }
                if !self.valid_status_entity(&status_entity(&source)) {
                    Some(AreaEndReason::SourceInvalid)
                } else if plan.contact.as_ref().unwrap().remaining() == 0 {
                    Some(AreaEndReason::HitBudgetExhausted)
                } else {
                    None
                }
            } else {
                Some(AreaEndReason::SourceInvalid)
            };
            if let Some(reason) = end {
                events.push((
                    None,
                    EventV4::AreaEnded {
                        area_id: id,
                        reason,
                    },
                ));
            } else {
                self.areas.insert(index.min(self.areas.len()), plan);
            }
        }
    }

    fn resolve_area_contacts(
        &mut self,
        p: &mut AreaPlan,
        owner: Owner,
        (scheduled, now): (u64, u64),
        c: &Config,
        events: &mut Events,
    ) -> Vec<EntityRef> {
        let mut candidates = self.area_candidates(&p.view);
        if p.is_trap() {
            let mut occupied = BTreeSet::new();
            candidates.retain(|&j| {
                let entity = &self.npcs[j].entity;
                let key = (entity.id.clone(), entity.generation);
                let first_entry = !p.trap_occupants.contains(&key);
                occupied.insert(key);
                first_entry
            });
            p.trap_occupants = occupied;
        }
        let mut targets = vec![];
        for j in candidates {
            let target = self.npcs[j].entity.clone();
            let key = (1, target.id.clone(), target.generation);
            if let Some(b) = &p.contact {
                if b.remaining() == 0 || targets.len() >= b.policy().max_targets_per_tick as usize {
                    break;
                }
                if !b.allows(&key, scheduled) {
                    continue;
                }
            }
            if self.direct_contact(
                owner,
                Owner::Npc(j),
                combat::DirectContact {
                    origin: Some(crate::origin::ActionOrigin::Skill(p.view.skill_id.clone())),
                    action_id: Some(&p.action_id),
                    rule: p.damage_rule.as_ref(),
                    ignore_armor: p.ignore_armor,
                    unavoidable: p.unavoidable,
                    dead_source: false,
                    source: &p.view.source,
                    raw: p.raw,
                    metal: true,
                    hit_statuses: &p.hit_statuses,
                },
                c,
                now,
                events,
            ) {
                if let Some(b) = &mut p.contact {
                    assert!(b.record(key, scheduled));
                }
                targets.push(target);
            }
        }
        targets
    }

    fn area_candidates(&self, view: &AreaPlanViewV4) -> Vec<usize> {
        let mut candidates = self
            .npcs
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                self.hostile_contact(&view.source, &n.entity)
                    && match (&view.line, view.mechanism) {
                        (Some(line), _) => {
                            self.grid
                                .line_field_contact(line, n.position, n.profile.combat.radius)
                        }
                        (None, AreaPlanKind::BasicField) => {
                            self.basic_circle_contact(view.center, view.radius, n)
                        }
                        (None, _) => self.grid.circle_contact(
                            view.center,
                            view.radius,
                            n.position,
                            n.profile.combat.radius,
                        ),
                    }
            })
            .map(|(j, n)| {
                let order = view.line.as_ref().map_or_else(
                    || (distance(view.center, n.position).powi(2), 0.),
                    |line| super::super::pve::line::order(line, n.position),
                );
                (order, &n.entity.id, n.entity.generation, j)
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            a.0.0
                .total_cmp(&b.0.0)
                .then(a.0.1.total_cmp(&b.0.1))
                .then(a.1.cmp(b.1))
                .then(a.2.cmp(&b.2))
        });
        candidates.into_iter().map(|(_, _, _, j)| j).collect()
    }
}
