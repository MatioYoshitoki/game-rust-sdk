use super::*;
use crate::config::{ProjectileAbility, npc_ai::NpcAiSlot};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NpcCast {
    at: u64,
    effect: NpcCastEffect,
}

#[derive(Clone, Debug, PartialEq)]
enum NpcCastEffect {
    Projectile {
        direction: [f64; 2],
        selected: Option<(usize, EntityRef)>,
    },
    Heal {
        slot: usize,
        target: EntityRef,
        remaining: Option<(u32, u32)>,
    },
}

impl NpcCast {
    pub(super) fn legacy(at: u64, direction: [f64; 2]) -> Self {
        Self {
            at,
            effect: NpcCastEffect::Projectile {
                direction,
                selected: None,
            },
        }
    }

    pub(super) fn aims_at(&self, target: &EntityRef, legacy_target: Option<&EntityRef>) -> bool {
        match &self.effect {
            NpcCastEffect::Projectile { selected, .. } => {
                selected
                    .as_ref()
                    .map(|(_, entity)| entity)
                    .or(legacy_target)
                    == Some(target)
            }
            NpcCastEffect::Heal { .. } => false,
        }
    }
}

fn projectile<'a>(slot: &'a NpcAiSlot, base: &'a ProjectileAbility) -> &'a ProjectileAbility {
    match slot {
        NpcAiSlot::Projectile { ability, .. } => ability.as_deref().unwrap_or(base),
        NpcAiSlot::Heal { .. } => unreachable!("locked projectile slot"),
    }
}

impl World {
    pub(super) fn try_npc_skill(
        &mut self,
        j: usize,
        enemy: Option<(&EntityRef, [f64; 2], f64)>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let Some(ai) = self.npc_ai_profile(j, c) else {
            return false;
        };
        if now < self.npcs[j].recovery || now < self.npcs[j].ai.decide_at {
            return false;
        }
        self.npcs[j].ai.decide_at = now + c.params.runtime.ai_decision_ms;
        let profile = self.npcs[j].profile.clone();
        let phase = profile
            .boss
            .as_ref()
            .map(|boss| &boss.phases[self.npcs[j].boss_phase]);
        let chance_scale = phase.map_or(1., |phase| phase.cast_chance);
        for (slot, rule) in ai.slots_for(phase.map(|phase| phase.id.as_str())) {
            if now < self.npcs[j].ai.ready_at[slot] {
                continue;
            }
            match rule {
                NpcAiSlot::Projectile { chance, .. } => {
                    let Some((target, point, d)) = enemy else {
                        continue;
                    };
                    let ability = projectile(rule, &profile.combat.projectile);
                    let from = self.npcs[j].position;
                    let direction = normalize([point[0] - from[0], point[1] - from[1]]);
                    let ground = c.npc_ground_delivery(&ability.id);
                    let enter = c.npc_enter_area(&ability.id);
                    let has_area = ground.is_some() || enter.is_some();
                    if d > ability.range
                        || self.npcs[j].spirit < ability.cost
                        || !self.grid.line(from, point, 0., 2)
                        || ground.is_none() && direction == [0., 0.]
                        || has_area
                            && (!self.npc_area_slot_available(c)
                                || !self.valid_npc_area_point(j, point)
                                || target.kind != StatusEntityKind::Participant)
                        || !has_area
                            && self.projectile_slots_used()
                                >= c.member_settings().unwrap().projectile_capacity
                        || !self.can_owned_statuses(
                            Owner::Npc(j),
                            &self.npcs[j].entity,
                            &ability.on_commit_self_statuses,
                            c,
                        )
                        || !self.roll(*chance * chance_scale)
                    {
                        continue;
                    }
                    let prepared = if let Some(ground) = ground {
                        self.prepare_ground_area(j, point, ground, c, now, events)
                    } else {
                        enter.is_none_or(|enter| {
                            self.prepare_enter_area(j, (point, direction), enter, c, now, events)
                        })
                    };
                    if !prepared {
                        continue;
                    }
                    self.npcs[j].casting = Some(NpcCast {
                        at: now + ability.windup_ms,
                        effect: NpcCastEffect::Projectile {
                            direction,
                            selected: Some((slot, target.clone())),
                        },
                    });
                }
                NpcAiSlot::Heal {
                    chance,
                    range,
                    include_self,
                } => {
                    let heal = &profile.combat.heal;
                    if self.npcs[j].spirit < heal.cost {
                        continue;
                    }
                    let Some(target) = self.select_npc_heal(
                        j,
                        (*range, *include_self),
                        (heal.hp_threshold, heal.heal_bps),
                        c,
                        now,
                    ) else {
                        continue;
                    };
                    if !self.roll(*chance * chance_scale) {
                        continue;
                    }
                    self.npcs[j].casting = Some(NpcCast {
                        at: now + heal.windup_ms,
                        effect: NpcCastEffect::Heal {
                            slot,
                            target,
                            remaining: None,
                        },
                    });
                }
            }
            return true;
        }
        false
    }

    pub(super) fn advance_npc_cast(&mut self, j: usize, c: &Config, now: u64, events: &mut Events) {
        if self.npcs[j]
            .casting
            .as_ref()
            .is_none_or(|cast| cast.at > now)
        {
            return;
        }
        let cast = self.npcs[j].casting.take().unwrap();
        match cast.effect {
            NpcCastEffect::Projectile {
                direction,
                selected,
            } => {
                let profile = self.npcs[j].profile.clone();
                let base = &profile.combat.projectile;
                let Some((slot, target)) = selected else {
                    self.commit_npc_projectile(j, (base, None, direction), c, now, events);
                    return;
                };
                let rule = &self.npc_ai_profile(j, c).unwrap().slots[slot];
                let ability = projectile(rule, base);
                let from = self.npcs[j].position;
                let legal = self.npc_enemy(&target).is_some_and(|owner| {
                    let to = self.actor_position(owner);
                    self.same_training_scope(Owner::Npc(j), owner)
                        && distance(from, to) <= ability.range.min(profile.memory_range)
                        && self.grid.line(from, to, 0., 2)
                });
                if legal
                    && self.commit_npc_projectile(
                        j,
                        (ability, Some(slot), direction),
                        c,
                        now,
                        events,
                    )
                {
                    self.publish_npc_skill(j, target, &ability.id, events);
                } else {
                    self.stop_npc_areas(
                        &self.npcs[j].entity.clone(),
                        AreaEndReason::Cancelled,
                        events,
                    );
                }
            }
            NpcCastEffect::Heal {
                slot,
                target,
                remaining,
            } => {
                self.advance_npc_heal(j, (slot, target, remaining), c, now, events);
            }
        }
    }

    fn npc_heal_target(
        &self,
        j: usize,
        k: usize,
        (range, include_self): (f64, bool),
        c: &Config,
        now: u64,
    ) -> Option<u32> {
        let ai = c.pve.as_ref()?.npc_ai.as_ref()?;
        let n = &self.npcs[k];
        if (j == k && !include_self)
            || !self.same_training_scope(Owner::Npc(j), Owner::Npc(k))
            || n.returning
            || !self.npc_can_be_attacked(k)
            || !self.same_npc_group(ai, j, k)
            || distance(self.npcs[j].position, n.position) > range
            || !self.grid.line(self.npcs[j].position, n.position, 0., 2)
        {
            return None;
        }
        let max_hp = self.entity_max_hp(&n.entity, c, now)?;
        (n.hp < max_hp).then_some(max_hp)
    }

    fn select_npc_heal(
        &self,
        j: usize,
        policy: (f64, bool),
        (threshold, heal_bps): (f64, u32),
        c: &Config,
        now: u64,
    ) -> Option<EntityRef> {
        self.npcs
            .iter()
            .enumerate()
            .filter_map(|(k, n)| {
                let max_hp = self.npc_heal_target(j, k, policy, c, now)?;
                (f64::from(n.hp) / f64::from(max_hp) <= threshold
                    && u64::from(max_hp) * u64::from(heal_bps) >= 10000)
                    .then_some((n, max_hp))
            })
            .min_by(|(a, max_a), (b, max_b)| {
                (u64::from(a.hp) * u64::from(*max_b))
                    .cmp(&(u64::from(b.hp) * u64::from(*max_a)))
                    .then(a.entity.id.cmp(&b.entity.id))
            })
            .map(|(n, _)| n.entity.clone())
    }

    fn advance_npc_heal(
        &mut self,
        j: usize,
        (slot, target, remaining): (usize, EntityRef, Option<(u32, u32)>),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let NpcAiSlot::Heal {
            range,
            include_self,
            ..
        } = self.npc_ai_profile(j, c).unwrap().slots[slot]
        else {
            unreachable!("locked healing slot")
        };
        let Some(Owner::Npc(k)) = self.resolve_entity(&target) else {
            return;
        };
        let Some(max_hp) = self.npc_heal_target(j, k, (range, include_self), c, now) else {
            return;
        };
        let profile = self.npcs[j].profile.clone();
        let ability = &profile.combat.heal;
        let (budget, ticks) = if let Some(remaining) = remaining {
            remaining
        } else {
            if self.npcs[j].spirit < ability.cost
                || self.intercept_cast((Owner::Npc(j), &ability.id, None), c, now, events)
            {
                return;
            }
            let budget = (u64::from(max_hp) * u64::from(ability.heal_bps) / 10000) as u32;
            if budget == 0 {
                return;
            }
            self.npcs[j].spirit -= ability.cost;
            self.npcs[j].ai.ready_at[slot] = now + ability.cooldown_ms;
            self.npcs[j].recovery =
                now + u64::from(ability.ticks - 1) * ability.interval_ms + ability.recovery_ms;
            self.publish_npc_skill(j, target.clone(), &ability.id, events);
            (budget, ability.ticks)
        };
        let raw = budget / ticks;
        let adjusted = self.adjusted_heal(&status_entity(&target), u64::from(raw), c, now);
        let amount = self.restore_hp(&target, u32::try_from(adjusted).unwrap_or(u32::MAX), c, now);
        events.push((
            None,
            EventV4::NpcHealTick {
                source: self.npcs[j].entity.clone(),
                target: target.clone(),
                skill_id: ability.id.clone(),
                amount,
                hp: self.npcs[k].hp,
            },
        ));
        if ticks > 1 && self.npcs[k].hp < max_hp {
            self.npcs[j].casting = Some(NpcCast {
                at: now + ability.interval_ms,
                effect: NpcCastEffect::Heal {
                    slot,
                    target,
                    remaining: Some((budget - raw, ticks - 1)),
                },
            });
        }
    }

    fn publish_npc_skill(&self, j: usize, target: EntityRef, skill_id: &str, events: &mut Events) {
        events.push((
            None,
            EventV4::NpcSkillCommitted {
                source: self.npcs[j].entity.clone(),
                target,
                skill_id: skill_id.into(),
            },
        ));
    }

    fn commit_npc_projectile(
        &mut self,
        j: usize,
        (s, slot, direction): (&ProjectileAbility, Option<usize>, [f64; 2]),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let npc_area = self.prepared_npc_area(j);
        let delivery_available =
            if c.npc_enter_area(&s.id).is_some() || c.npc_ground_delivery(&s.id).is_some() {
                npc_area.is_some()
            } else {
                self.projectile_slots_used() < c.member_settings().unwrap().projectile_capacity
            };
        if self.npcs[j].spirit >= s.cost
            && delivery_available
            && self.can_owned_statuses(
                Owner::Npc(j),
                &self.npcs[j].entity,
                &s.on_commit_self_statuses,
                c,
            )
        {
            if self.intercept_cast((Owner::Npc(j), &s.id, None), c, now, events) {
                return false;
            }
            let action_id = if let Some(index) = npc_area {
                self.commit_npc_area(index, j, c, now)
            } else {
                self.next_id("shot")
            };
            // The commit status may clamp current spirit to a lower maximum.
            self.npcs[j].spirit -= s.cost;
            if self.periodic_enabled {
                let previous_origin = self
                    .triggers
                    .origin
                    .replace(crate::origin::ActionOrigin::Skill(s.id.clone()));
                let previous_source = self
                    .triggers
                    .action_source
                    .replace(self.npcs[j].entity.clone());
                let previous_action = self.triggers.action.replace(action_id.clone());
                self.apply_owned_statuses(
                    Owner::Npc(j),
                    (
                        self.npcs[j].entity.clone(),
                        self.npcs[j].profile.stats["hp"],
                    ),
                    &s.on_commit_self_statuses,
                    c,
                    now,
                    events,
                );
                self.triggers.origin = previous_origin;
                self.triggers.action_source = previous_source;
                self.triggers.action = previous_action;
            }
            if let Some(slot) = slot {
                self.npcs[j].ai.ready_at[slot] = now + s.cooldown_ms;
            } else {
                self.npcs[j].next_cast = now + s.cooldown_ms;
            }
            self.npcs[j].recovery = now + s.recovery_ms;
            if npc_area.is_some() {
                self.resolve_ground_strike(&action_id, c, now, events);
                return true;
            }
            let view = ProjectileViewV4 {
                basic_segment: None,
                piercing: None,
                unit_target: None,
                id: action_id,
                source: self.npcs[j].entity.clone(),
                position: self.npcs[j].position,
                radius: s.radius,
            };
            events.push((
                None,
                EventV4::ProjectileStarted {
                    projectile: view.clone(),
                },
            ));
            let entity = status_entity(&self.npcs[j].entity);
            let atk = self.statuses.attribute(
                &entity,
                self.npcs[j].profile.stats["atk"],
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
                StatusAttribute::Atk,
            );
            self.shots.push(Shot {
                end_on_source_invalid: false,
                flight: None,
                origin: crate::origin::ActionOrigin::Skill(s.id.clone()),
                action_id: view.id.clone(),
                damage_rule: self.attack_rule(Owner::Npc(j), None, c),
                ignore_armor: c.mechanism_version >= 103 && s.ignore_armor == Some(true),
                unavoidable: c.mechanism_version >= 101 && s.unavoidable == Some(true),
                contact: None,
                view,
                target: None,
                owner: Owner::Npc(j),
                direction,
                remaining: s.range,
                speed: s.speed,
                raw: self.npc_attack_budget(j, atk, s.damage_bps, true, c, now),
                metal: true,
                hit_statuses: s.on_hit_target_statuses.clone(),
                created: now,
                expires: now + s.ttl_ms,
            });
            return true;
        }
        false
    }
}
