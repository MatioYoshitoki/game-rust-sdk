use super::{combat::DirectContact, summons::scale, *};
use crate::config::member_summons::SummonSkillCopy;

#[derive(Default)]
pub(super) struct CopyState {
    pub pending: Option<PendingCopy>,
    cooldowns: BTreeMap<String, u64>,
    recovery_until: u64,
}

pub(super) struct PendingCopy {
    skill: String,
    target: EntityRef,
    due: u64,
}

impl World {
    pub(super) fn advance_summon_copy(
        &mut self,
        i: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let summon = &self.summons[i];
        let Some(copy) = c
            .summon_cast(&summon.cast_skill_id)
            .and_then(|cast| cast.skill_copy.as_ref())
        else {
            return false;
        };
        if self.entity_has_status(Owner::Summon(i), StatusAttribute::Silence, now) {
            self.summons[i].copy.pending = None;
            return false;
        }
        let member = &self.members[summon.member];
        if now < summon.copy.recovery_until {
            return true;
        }
        let (generation, ready) = member.summon_copy_ready;
        if generation == summon.owner.generation && now < ready {
            self.summons[i].copy.pending = None;
            return false;
        }
        if let Some(pending) = &summon.copy.pending {
            if now < pending.due {
                return true;
            }
            let pending = self.summons[i].copy.pending.take().unwrap();
            let skill = c.player_skill(&pending.skill).unwrap();
            let Some(Owner::Npc(target)) = self.resolve_entity(&pending.target) else {
                return false;
            };
            return self.release_summon_copy(i, (skill, target, copy), c, now, events);
        }

        for id in &copy.skill_ids {
            let skill = c.player_skill(id).unwrap();
            let due = now + skill.windup_ms;
            if due >= summon.expires_at_ms
                || !member.skill_equipped(id, c)
                || summon
                    .copy
                    .cooldowns
                    .get(id)
                    .is_some_and(|ready| now < *ready)
                || !self.copy_capacity(skill, c)
            {
                continue;
            }
            let Some(target) = self
                .npcs
                .iter()
                .enumerate()
                .filter(|(j, _)| self.copy_aim(i, *j, skill))
                .min_by(|(_, a), (_, b)| {
                    distance(a.position, summon.position)
                        .total_cmp(&distance(b.position, summon.position))
                        .then(a.entity.id.cmp(&b.entity.id))
                })
                .map(|(j, _)| j)
            else {
                continue;
            };
            if due == now {
                return self.release_summon_copy(i, (skill, target, copy), c, now, events);
            }
            self.summons[i].copy.pending = Some(PendingCopy {
                skill: id.clone(),
                target: self.npcs[target].entity.clone(),
                due,
            });
            return true;
        }
        false
    }

    fn copy_capacity(&self, skill: &SkillConfig, c: &Config) -> bool {
        !skill.is_copy_projectile()
            || self.projectile_slots_used() < c.member_settings().unwrap().projectile_capacity
    }

    fn copy_aim(&self, i: usize, j: usize, skill: &SkillConfig) -> bool {
        let from = self.summons[i].position;
        let to = self.npcs[j].position;
        self.npcs[j].hp > 0
            && !self.npcs[j].returning
            && self.same_training_scope(Owner::Summon(i), Owner::Npc(j))
            && distance(from, to) <= skill.range
            && self.grid.line(from, to, 0., 2)
            && (!skill.is_copy_projectile() || from != to && self.grid.clear(from, skill.radius, 1))
    }

    fn release_summon_copy(
        &mut self,
        i: usize,
        (skill, target, copy): (&SkillConfig, usize, &SummonSkillCopy),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let summon = &self.summons[i];
        let member = summon.member;
        if !self.members[member].skill_equipped(&skill.id, c)
            || !self.copy_capacity(skill, c)
            || !self.copy_aim(i, target, skill)
        {
            return false;
        }
        let source = summon.entity.clone();
        let target_ref = self.npcs[target].entity.clone();
        let from = summon.position;
        let to = self.npcs[target].position;
        let mut rule = self
            .attack_rule(Owner::Member(member), Some(&skill.id), c)
            .unwrap();
        rule.contact_status = None;
        rule.life_steal_bps = 0;
        rule.weapon_element = None;
        let attack = self.entity_attack(&status_entity(&source), c, now).unwrap();
        let raw = scale(
            rule.budget(attack, skill.damage_bps, 10000, 0),
            copy.damage_bps,
        );
        let state = &mut self.summons[i].copy;
        state
            .cooldowns
            .insert(skill.id.clone(), now + skill.cooldown_ms);
        state.recovery_until = now + skill.recovery_ms;
        self.members[member].summon_copy_ready = (
            self.summons[i].owner.generation,
            now + copy.shared_interval_ms,
        );
        events.push((
            None,
            EventV4::SummonSkillCopied {
                source: source.clone(),
                skill_id: skill.id.clone(),
                target: target_ref.clone(),
            },
        ));
        if skill.is_copy_projectile() {
            let view = ProjectileViewV4 {
                id: self.next_id("shot"),
                source,
                position: from,
                radius: skill.radius,
                piercing: None,
                basic_segment: None,
                unit_target: None,
            };
            events.push((
                None,
                EventV4::ProjectileStarted {
                    projectile: view.clone(),
                },
            ));
            self.shots.push(Shot {
                end_on_source_invalid: false,
                action_id: view.id.clone(),
                origin: crate::origin::ActionOrigin::Skill(skill.id.clone()),
                damage_rule: Some(rule),
                ignore_armor: skill.ignore_armor == Some(true),
                unavoidable: skill.unavoidable == Some(true),
                contact: None,
                flight: None,
                view,
                target: skill.is_unit().then_some(target_ref),
                owner: Owner::Summon(i),
                direction: normalize([to[0] - from[0], to[1] - from[1]]),
                remaining: skill.range,
                speed: skill.speed,
                raw,
                metal: false,
                hit_statuses: vec![],
                created: now,
                expires: now + c.params.player_projectile_ttl_ms,
            });
        } else {
            let targets = if skill.is_ground() {
                let cap = skill
                    .contact_policy
                    .as_ref()
                    .map_or(copy.max_circle_targets, |policy| {
                        copy.max_circle_targets
                            .min(policy.max_targets_per_tick as usize)
                            .min(policy.max_total_hits as usize)
                    });
                self.summon_circle_targets(i, to, skill.radius, cap)
            } else {
                vec![target]
            };
            for j in targets {
                self.direct_contact(
                    Owner::Summon(i),
                    Owner::Npc(j),
                    DirectContact {
                        origin: Some(crate::origin::ActionOrigin::Skill(skill.id.clone())),
                        action_id: None,
                        rule: Some(&rule),
                        ignore_armor: skill.ignore_armor == Some(true),
                        unavoidable: skill.unavoidable == Some(true),
                        dead_source: false,
                        source: &source,
                        raw,
                        metal: false,
                        hit_statuses: &[],
                    },
                    c,
                    now,
                    events,
                );
            }
        }
        true
    }

    fn summon_circle_targets(
        &self,
        i: usize,
        center: [f64; 2],
        radius: f64,
        cap: usize,
    ) -> Vec<usize> {
        let mut targets = self
            .npcs
            .iter()
            .enumerate()
            .filter(|(j, n)| {
                n.hp > 0
                    && !n.returning
                    && self.same_training_scope(Owner::Summon(i), Owner::Npc(*j))
            })
            .map(|(j, n)| (j, n, distance(center, n.position)))
            .filter(|(_, n, _)| {
                self.grid
                    .circle_contact(center, radius, n.position, n.profile.combat.radius)
            })
            .collect::<Vec<_>>();
        targets.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.1.entity.id.cmp(&b.1.entity.id)));
        targets.into_iter().take(cap).map(|(j, _, _)| j).collect()
    }

    pub(super) fn cancel_summon_shots(&mut self, source: &EntityRef, events: &mut Events) {
        self.shots.retain(|shot| {
            if shot.view.source != *source {
                return true;
            }
            events.push((
                None,
                EventV4::ProjectileEnded {
                    projectile_id: shot.view.id.clone(),
                    reason: Some(ProjectileEndReasonV4::SourceInvalid),
                },
            ));
            false
        });
    }

    pub(super) fn interrupt_owned_copies(&mut self, member: usize, events: &mut Events) {
        for i in 0..self.summons.len() {
            if self.summons[i].member == member {
                self.summons[i].copy.pending = None;
                let source = self.summons[i].entity.clone();
                self.cancel_summon_shots(&source, events);
            }
        }
    }

    pub(super) fn summon_death_burst(
        &mut self,
        i: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let summon = &self.summons[i];
        let Some(burst) = c
            .summon_cast(&summon.cast_skill_id)
            .and_then(|cast| cast.death_burst)
        else {
            return;
        };
        let source = summon.entity.clone();
        let raw = summon.death_damage;
        if raw == 0 {
            return;
        }
        for j in self.summon_circle_targets(i, summon.position, burst.radius, burst.max_targets) {
            self.resolve_summon_burst_damage(
                combat::DamageIntent {
                    ignore_armor: false,
                    source: source.clone(),
                    target: self.npcs[j].entity.clone(),
                    raw,
                    direct: false,
                },
                c,
                now,
                events,
            );
        }
    }
}
