use super::*;
use crate::config::{BasicFieldSampling, BasicWaveField};

#[derive(Clone, Copy)]
enum FieldPower {
    Live(u32),
    Frozen(u32),
}

#[derive(Clone, Copy)]
pub(super) struct BasicFieldState {
    power: FieldPower,
    target_cap: usize,
    owner_reduction_bps: u32,
}

pub(in crate::world::two_members) struct PendingBasicField {
    rule: BasicWaveField,
    power: FieldPower,
    ignore_armor: bool,
    replace: bool,
}

impl PendingBasicField {
    fn reserves_new_slot(&self, world: &World, source: &EntityRef) -> bool {
        !self.replace
            || world
                .oldest_basic_field(source, &self.rule.plan_id)
                .is_none()
    }
}

impl World {
    fn oldest_basic_field(&self, source: &EntityRef, plan_id: &str) -> Option<usize> {
        self.areas.iter().position(|area| {
            area.is_basic() && area.view.source == *source && area.view.skill_id == plan_id
        })
    }

    pub(in crate::world::two_members) fn reserve_basic_field(
        &self,
        i: usize,
        field: &BasicWaveField,
        c: &Config,
        now: u64,
    ) -> Result<PendingBasicField> {
        let source = &self.members[i].reference.entity;
        let active = self.areas.iter().filter(|area| area.is_basic());
        let owned = active
            .clone()
            .filter(|area| area.view.source == *source && area.view.skill_id == field.plan_id)
            .count();
        let pending = self
            .basic_waves
            .iter()
            .filter_map(|wave| wave.pending_field())
            .chain(
                self.basic_impacts
                    .iter()
                    .filter_map(|impact| impact.pending_field()),
            )
            .filter(|(source, plan)| plan.reserves_new_slot(self, source))
            .count();
        let capacity = c
            .members
            .as_ref()
            .and_then(|m| m.basic_field_capacity)
            .expect("validated field pool");
        let full = active.count() + pending >= capacity;
        if full && owned == 0 {
            return Err(Error::Capacity);
        }
        let rule = field
            .damage_rule
            .as_ref()
            .unwrap_or(&c.combat_rules.as_ref().unwrap().basic_attack);
        let power = match field.sampling {
            BasicFieldSampling::Live => FieldPower::Live(field.damage_bps),
            BasicFieldSampling::Frozen => {
                FieldPower::Frozen(rule.budget(self.stats(i, c, now).0, field.damage_bps, 10000, 0))
            }
        };
        Ok(PendingBasicField {
            rule: field.clone(),
            power,
            ignore_armor: self.members[i].basic(c).ignore_armor == Some(true),
            replace: full || owned >= field.max_per_source,
        })
    }

    pub(in crate::world::two_members) fn start_basic_field(
        &mut self,
        i: usize,
        field: PendingBasicField,
        action: &str,
        center: [f64; 2],
        now: u64,
        events: &mut Events,
    ) {
        if !self.members[i].living() {
            return;
        }
        let source = self.members[i].reference.entity.clone();
        let rule = field.rule;
        if field.replace
            && let Some(index) = self.oldest_basic_field(&source, &rule.plan_id)
        {
            let old = self.areas.remove(index);
            events.push((
                None,
                EventV4::AreaEnded {
                    area_id: old.view.id,
                    reason: AreaEndReason::Replaced,
                },
            ));
        }
        let view = AreaPlanViewV4 {
            attached_anchor_id: None,
            anchor_recast: None,
            movement: None,
            basic_action_id: Some(action.to_owned()),
            line: None,
            id: self.next_id("area"),
            source,
            skill_id: rule.plan_id,
            mechanism: AreaPlanKind::BasicField,
            center,
            radius: rule.radius,
            created_at_ms: now,
            activates_at_ms: now,
            expires_at_ms: now + rule.duration_ms,
            interval_ms: rule.interval_ms,
            next_tick_at_ms: Some(now + rule.interval_ms),
            completed_ticks: 0,
            total_ticks: (rule.duration_ms / rule.interval_ms) as u32,
            contact_policy: None,
        };
        events.push((None, EventV4::AreaStarted { area: view.clone() }));
        self.areas.push(AreaPlan {
            action_id: action.to_owned(),
            damage_rule: rule.damage_rule,
            ignore_armor: field.ignore_armor,
            unavoidable: false,
            view,
            raw: 0,
            hit_statuses: rule.on_tick_statuses,
            contact: None,
            basic: Some(BasicFieldState {
                power: field.power,
                target_cap: rule.target_cap,
                owner_reduction_bps: rule.owner_reduction_bps,
            }),
            moving: None,
            ground: None,
            trap_occupants: Default::default(),
        });
    }

    pub(in crate::world::two_members) fn basic_circle_contact(
        &self,
        center: [f64; 2],
        radius: f64,
        npc: &Npc,
    ) -> bool {
        distance(center, npc.position) <= radius + npc.profile.combat.radius
            && self.grid.piercing_wall(center, npc.position, 0.).is_none()
    }

    pub(super) fn advance_basic_field(
        &mut self,
        index: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let area = &self.areas[index];
        let view = area.view.clone();
        let field = area.basic.unwrap();
        let Some(Owner::Member(i)) = self.resolve_entity(&view.source) else {
            self.areas.remove(index);
            events.push((
                None,
                EventV4::AreaEnded {
                    area_id: view.id,
                    reason: AreaEndReason::SourceInvalid,
                },
            ));
            return;
        };
        let rule = area.damage_rule.clone();
        let ignore_armor = area.ignore_armor;
        let rule = rule
            .as_ref()
            .unwrap_or(&c.combat_rules.as_ref().unwrap().basic_attack);
        let defs = Self::definitions(c, &area.hit_statuses);
        let raw = match field.power {
            FieldPower::Live(bps) => rule.budget(self.stats(i, c, now).0, bps, 10000, 0),
            FieldPower::Frozen(raw) => raw,
        };
        let mut targets = vec![];
        for j in self
            .area_candidates(&view)
            .into_iter()
            .take(field.target_cap)
        {
            let target = self.npcs[j].entity.clone();
            if !self.members[i].living() {
                break;
            }
            if self.hostile_contact(&view.source, &target)
                && !self.entity_has_status(Owner::Npc(j), StatusAttribute::DamageImmunity, now)
            {
                self.resolve_basic_field_damage(
                    combat::DamageIntent {
                        source: view.source.clone(),
                        target: target.clone(),
                        raw,
                        direct: false,
                        ignore_armor,
                    },
                    rule,
                    c,
                    now,
                    events,
                );
                for definition in &defs {
                    if !self.hostile_contact(&view.source, &target) {
                        break;
                    }
                    self.apply_owned_status_definitions(
                        Owner::Member(i),
                        target.clone(),
                        std::slice::from_ref(definition),
                        c,
                        now,
                        events,
                    );
                }
                targets.push(target);
            }
        }
        if let Some(area) = self.areas.iter_mut().find(|area| area.view.id == view.id) {
            let scheduled = view.next_tick_at_ms.unwrap();
            area.view.completed_ticks += 1;
            area.view.next_tick_at_ms = (area.view.completed_ticks < view.total_ticks)
                .then(|| scheduled + view.interval_ms);
            events.push((
                None,
                EventV4::AreaTick {
                    area_id: view.id,
                    tick_index: area.view.completed_ticks,
                    scheduled_at_ms: scheduled,
                    targets,
                },
            ));
        }
    }

    pub(in crate::world::two_members) fn effective_damage_reduction(
        &self,
        owner: Owner,
        target: &EntityRef,
        c: &Config,
        now: u64,
    ) -> u32 {
        let rules = c.pve.as_ref().unwrap().status_rules.as_ref();
        let extra = match owner {
            Owner::Member(i) => self
                .areas
                .iter()
                .filter_map(|area| {
                    let field = area.basic?;
                    (field.owner_reduction_bps > 0
                        && area.view.source == *target
                        && now < area.view.expires_at_ms
                        && distance(self.members[i].position, area.view.center) <= area.view.radius)
                        .then_some(field.owner_reduction_bps)
                })
                .max()
                .unwrap_or(0),
            Owner::Npc(_) | Owner::Summon(_) => 0,
        };
        let extra = extra
            + self.summon_owner_bonus(owner, now).damage_reduction_bps
            + match owner {
                Owner::Member(i) => self.members[i]
                    .active_guard(c, now)
                    .map_or(0, |guard| guard.0),
                Owner::Npc(_) | Owner::Summon(_) => 0,
            };
        if extra == 0 {
            self.statuses
                .damage_reduction_bps(&status_entity(target), now, rules)
        } else {
            self.statuses.attribute_with_bonus(
                &status_entity(target),
                10000,
                now,
                rules,
                StatusAttribute::DamageReduction,
                i64::from(extra),
            ) - 10000
        }
    }
}
