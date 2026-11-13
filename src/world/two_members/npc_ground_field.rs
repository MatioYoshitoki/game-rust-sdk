//! Fixed world-space warnings for instant strikes and fields with separate entry and pulse clocks.
use super::*;
use crate::config::{ContactPolicyConfig, npc_ground_field::NpcGroundDelivery};
use std::collections::BTreeSet;

pub(super) struct GroundArea {
    pub(super) delivery: NpcGroundDelivery,
    pub(super) committed: bool,
    pub(super) released_accuracy: Option<numeric::AttackAccuracy>,
    entered: BTreeSet<(String, u32)>,
}

impl World {
    pub(in crate::world::two_members) fn cancel_ground_areas(&mut self, events: &mut Events) {
        self.areas.retain(|p| {
            if p.ground.is_none() {
                return true;
            }
            events.push((
                None,
                EventV4::AreaEnded {
                    area_id: p.view.id.clone(),
                    reason: AreaEndReason::Cancelled,
                },
            ));
            false
        });
    }

    pub(crate) fn prepare_ground_area(
        &mut self,
        j: usize,
        point: [f64; 2],
        delivery: NpcGroundDelivery,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        if !self.npc_area_slot_available(c) || !self.valid_npc_area_point(j, point) {
            return false;
        }
        let npc = &self.npcs[j];
        let ability = &npc.profile.combat.projectile;
        let activates_at_ms = now + ability.windup_ms;
        let skill_id = ability.id.clone();
        let source = npc.entity.clone();
        let hit_statuses = ability.on_hit_target_statuses.clone();
        let unavoidable = ability.unavoidable == Some(true);
        let (mechanism, radius, duration, interval, total_ticks, contact) = match delivery {
            NpcGroundDelivery::Field(rule) => (
                AreaPlanKind::NpcGroundField,
                rule.radius,
                rule.duration_ms,
                rule.interval_ms,
                (rule.duration_ms / rule.interval_ms) as u32,
                Some(ContactBudget::new(ContactPolicyConfig {
                    max_targets_per_tick: rule.max_targets_per_tick,
                    max_total_hits: rule.max_total_hits,
                    repeat_interval_ms: rule.interval_ms,
                })),
            ),
            NpcGroundDelivery::Strike(rule) => {
                (AreaPlanKind::NpcGroundStrike, rule.radius, 0, 0, 1, None)
            }
        };
        let id = self.next_id(match delivery {
            NpcGroundDelivery::Field(_) => "ground-field",
            NpcGroundDelivery::Strike(_) => "ground-strike",
        });
        let p = AreaPlan {
            action_id: id.clone(),
            damage_rule: None,
            ignore_armor: false,
            unavoidable,
            view: AreaPlanViewV4 {
                attached_anchor_id: None,
                anchor_recast: None,
                movement: None,
                basic_action_id: None,
                line: None,
                id,
                source,
                skill_id,
                mechanism,
                center: point,
                radius,
                created_at_ms: now,
                activates_at_ms,
                expires_at_ms: activates_at_ms + duration,
                interval_ms: interval,
                next_tick_at_ms: Some(activates_at_ms + interval),
                completed_ticks: 0,
                total_ticks,
                contact_policy: None,
            },
            raw: 0,
            hit_statuses,
            contact,
            basic: None,
            moving: None,
            trap_occupants: Default::default(),
            ground: Some(GroundArea {
                delivery,
                committed: false,
                released_accuracy: None,
                entered: BTreeSet::new(),
            }),
        };
        events.push((None, EventV4::AreaStarted { area: p.view() }));
        self.areas.push(p);
        true
    }

    pub(super) fn step_ground_area(
        &mut self,
        p: &mut AreaPlan,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<AreaEndReason> {
        let field = p.ground.as_mut().unwrap();
        let view = &mut p.view;
        let source = self.live_npc_area_source(&view.source);
        if !field.committed {
            let Some(j) = source else {
                return Some(AreaEndReason::SourceInvalid);
            };
            return (self.npcs[j].casting.is_none() || now >= view.activates_at_ms)
                .then_some(AreaEndReason::Cancelled);
        }
        let NpcGroundDelivery::Field(rule) = field.delivery else {
            return Some(AreaEndReason::Cancelled);
        };
        if source.is_none() && rule.end_on_source_invalid {
            return Some(AreaEndReason::SourceInvalid);
        }
        let scheduled = view.next_tick_at_ms.unwrap();
        if scheduled > now && (source.is_none() || p.hit_statuses.is_empty()) {
            return None;
        }
        let candidates = self.ground_members(view, c);
        if !p.hit_statuses.is_empty() {
            for &(_, i) in &candidates {
                let Some(j) = self.live_npc_area_source(&view.source) else {
                    break;
                };
                let target = self.members[i].reference.entity.clone();
                if !self.members[i].eligible()
                    || self.member_damage_immune(i, c, now)
                    || !field.entered.insert((target.id.clone(), target.generation))
                {
                    continue;
                }
                self.apply_ground_statuses(j, &target, (view, &p.hit_statuses), c, now, events);
                events.push((
                    None,
                    EventV4::AreaEntered {
                        area_id: view.id.clone(),
                        source: view.source.clone(),
                        target,
                        at_ms: now,
                    },
                ));
            }
        }
        if scheduled > now {
            return None;
        }
        let budget = p.contact.as_mut().unwrap();
        let mut targets = Vec::new();
        for (_, i) in candidates {
            if budget.remaining() == 0 || targets.len() == rule.max_targets_per_tick as usize {
                break;
            }
            let target = self.members[i].reference.entity.clone();
            let key = (0, target.id.clone(), target.generation);
            if self.members[i].eligible()
                && budget.allows(&key, scheduled)
                && self.npc_area_contact(
                    NpcAreaHit {
                        area: view,
                        target: i,
                        max_hp_bps: rule.target_max_hp_bps,
                        unavoidable: p.unavoidable,
                        end_on_source_invalid: rule.end_on_source_invalid,
                        released_accuracy: field.released_accuracy,
                    },
                    c,
                    now,
                    events,
                )
            {
                budget.record(key, scheduled);
                targets.push(target);
            }
        }
        view.completed_ticks += 1;
        events.push((
            None,
            EventV4::AreaTick {
                area_id: view.id.clone(),
                tick_index: view.completed_ticks,
                scheduled_at_ms: scheduled,
                targets,
            },
        ));
        if self.members.iter().all(|member| !member.living()) {
            Some(AreaEndReason::Cancelled)
        } else if rule.end_on_source_invalid && self.live_npc_area_source(&view.source).is_none() {
            Some(AreaEndReason::SourceInvalid)
        } else if budget.remaining() == 0 {
            Some(AreaEndReason::HitBudgetExhausted)
        } else if view.completed_ticks == view.total_ticks {
            Some(AreaEndReason::Expired)
        } else {
            view.next_tick_at_ms = Some(scheduled + view.interval_ms);
            None
        }
    }

    fn ground_members(&self, view: &AreaPlanViewV4, c: &Config) -> Vec<(f64, usize)> {
        let mut candidates: Vec<_> = self
            .members
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                m.eligible()
                    && distance(view.center, m.position) <= view.radius + c.params.player_radius
                    && self
                        .grid
                        .piercing_wall(view.center, m.position, 0.)
                        .is_none()
            })
            .map(|(i, m)| (distance(view.center, m.position), i))
            .collect();
        candidates.sort_by(|&(da, a), &(db, b)| {
            da.total_cmp(&db).then(
                self.members[a]
                    .reference
                    .entity
                    .id
                    .cmp(&self.members[b].reference.entity.id),
            )
        });
        candidates
    }

    fn apply_ground_statuses(
        &mut self,
        j: usize,
        target: &EntityRef,
        (view, ids): (&AreaPlanViewV4, &[String]),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let max = self.entity_max_hp(target, c, now).unwrap();
        let origin = self
            .triggers
            .origin
            .replace(crate::origin::ActionOrigin::Skill(view.skill_id.clone()));
        let source = self.triggers.action_source.replace(view.source.clone());
        let action = self.triggers.action.replace(view.id.clone());
        self.apply_owned_statuses(Owner::Npc(j), (target.clone(), max), ids, c, now, events);
        self.triggers.origin = origin;
        self.triggers.action_source = source;
        self.triggers.action = action;
    }

    pub(in crate::world::two_members) fn resolve_ground_strike(
        &mut self,
        action_id: &str,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let Some(index) = self.areas.iter().position(|p| {
            p.view.mechanism == AreaPlanKind::NpcGroundStrike && p.view.id == action_id
        }) else {
            return;
        };
        let p = self.areas.remove(index);
        let view = p.view;
        let NpcGroundDelivery::Strike(rule) = p.ground.unwrap().delivery else {
            unreachable!("strike plan must carry strike delivery");
        };
        let reason = if self.live_npc_area_source(&view.source).is_some() {
            let mut targets = Vec::new();
            if let Some((_, i)) = self.ground_members(&view, c).into_iter().next()
                && self.npc_area_contact(
                    NpcAreaHit {
                        area: &view,
                        target: i,
                        max_hp_bps: rule.target_max_hp_bps,
                        unavoidable: p.unavoidable,
                        end_on_source_invalid: true,
                        released_accuracy: None,
                    },
                    c,
                    now,
                    events,
                )
            {
                let target = self.members[i].reference.entity.clone();
                if self.members[i].living()
                    && let Some(j) = self.live_npc_area_source(&view.source)
                    && self.roll(f64::from(rule.hit_status_chance_bps) / 10000.)
                {
                    self.apply_ground_statuses(
                        j,
                        &target,
                        (&view, &p.hit_statuses),
                        c,
                        now,
                        events,
                    );
                }
                targets.push(target);
            }
            events.push((
                None,
                EventV4::AreaTick {
                    area_id: view.id.clone(),
                    tick_index: 1,
                    scheduled_at_ms: view.activates_at_ms,
                    targets,
                },
            ));
            AreaEndReason::Resolved
        } else {
            AreaEndReason::SourceInvalid
        };
        events.push((
            None,
            EventV4::AreaEnded {
                area_id: view.id,
                reason,
            },
        ));
    }
}
