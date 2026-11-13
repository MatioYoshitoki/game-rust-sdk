use super::*;
use crate::config::training_targets::{TrainingMetrics, TrainingTargetBehavior};
use std::collections::VecDeque;

pub(super) const MAX_TRAINING_VALUE: u64 = (1 << 53) - 1;

#[derive(Clone, Copy)]
pub(super) enum DamageSource {
    Basic,
    Skill,
    Periodic,
    Summon,
    Other,
}

#[derive(Default)]
pub(super) struct Metrics {
    totals: TrainingMetricsView,
    published: TrainingMetricsView,
    first_hit: Option<u64>,
    last_hit: u64,
    rolling: VecDeque<(u64, u64)>,
    rolling_damage: u64,
    next_sample: u64,
}

impl Metrics {
    pub(super) fn record(&mut self, amount: u32, critical: bool, source: DamageSource, now: u64) {
        self.first_hit.get_or_insert(now);
        self.last_hit = now;
        let amount = u64::from(amount);
        let accepted = amount.min(MAX_TRAINING_VALUE - self.totals.damage);
        self.totals.damage += accepted;
        self.totals.hits = (self.totals.hits + 1).min(MAX_TRAINING_VALUE);
        self.totals.critical_hits =
            (self.totals.critical_hits + u64::from(critical)).min(MAX_TRAINING_VALUE);
        self.totals.highest_hit = self.totals.highest_hit.max(amount as u32);
        *match source {
            DamageSource::Basic => &mut self.totals.by_source.basic,
            DamageSource::Skill => &mut self.totals.by_source.skill,
            DamageSource::Periodic => &mut self.totals.by_source.periodic,
            DamageSource::Summon => &mut self.totals.by_source.summon,
            DamageSource::Other => &mut self.totals.by_source.other,
        } += accepted;
        if let Some((_, subtotal)) = self.rolling.back_mut().filter(|(at, _)| *at == now) {
            *subtotal += accepted;
        } else {
            self.rolling.push_back((now, accepted));
        }
        self.rolling_damage += accepted;
    }

    fn sample(&mut self, rule: TrainingMetrics, now: u64) {
        while self
            .rolling
            .front()
            .is_some_and(|(at, _)| now - at >= rule.window_ms)
        {
            self.rolling_damage -= self.rolling.pop_front().unwrap().1;
        }
        if now < self.next_sample {
            return;
        }
        let per_second = |damage: u64, duration: u64| {
            (u128::from(damage) * 1000 / u128::from(duration)).min(u128::from(MAX_TRAINING_VALUE))
                as u64
        };
        let mut view = self.totals.clone();
        view.sampled_at_ms = self.published.sampled_at_ms;
        view.rolling_dps = per_second(self.rolling_damage, rule.window_ms);
        view.average_dps = self.first_hit.map_or(0, |first| {
            per_second(
                view.damage,
                (self.last_hit - first).max(rule.publish_interval_ms),
            )
        });
        if view != self.published {
            view.sampled_at_ms = now;
            self.published = view;
            self.next_sample = now + rule.publish_interval_ms;
        }
    }
}

pub(super) struct SeatRuntime {
    pub(super) respawn_at_ms: Option<u64>,
    pub(super) clear_defeated_life: bool,
    pub(super) metrics: Option<(TrainingMetrics, Metrics)>,
}

impl SeatRuntime {
    pub(super) fn new(metrics: Option<TrainingMetrics>) -> Self {
        Self {
            respawn_at_ms: None,
            clear_defeated_life: false,
            metrics: metrics.map(|m| (m, Metrics::default())),
        }
    }
}

impl World {
    pub(super) fn training_target_behavior<'a>(
        &self,
        j: usize,
        c: &'a Config,
    ) -> Option<&'a TrainingTargetBehavior> {
        let npc = &self.npcs[j];
        c.training_targets()?[npc.training_owner?]
            .behaviors
            .get(&npc.entity.id)
    }

    pub(super) fn rebuilds_training_dummy(&self, j: usize, c: &Config) -> bool {
        matches!(
            self.training_target_behavior(j, c),
            Some(TrainingTargetBehavior::Dummy {
                reset_on_defeat: true,
                ..
            })
        )
    }

    pub(super) fn move_training_dummy(&mut self, j: usize, c: &Config, now: u64, dt: f64) {
        let Some(TrainingTargetBehavior::Dummy {
            motion: Some(motion),
            ..
        }) = self.training_target_behavior(j, c)
        else {
            return;
        };
        if self.entity_has_status(Owner::Npc(j), StatusAttribute::Stun, now) {
            return;
        }
        let factor = self.npc_movement_factor(j, c, now);
        let npc = &mut self.npcs[j];
        let goal = if npc.training_reverse {
            npc.home
        } else {
            motion.end
        };
        let remaining = distance(npc.position, goal);
        let step = motion.speed * self.grid.speed(npc.position) * dt * factor;
        let end = if step >= remaining {
            goal
        } else {
            let direction = normalize([goal[0] - npc.position[0], goal[1] - npc.position[1]]);
            [
                npc.position[0] + direction[0] * step,
                npc.position[1] + direction[1] * step,
            ]
        };
        if self
            .grid
            .line(npc.position, end, npc.profile.combat.radius, 0)
        {
            npc.position = end;
            if end == goal {
                npc.training_reverse = !npc.training_reverse;
            }
        }
    }

    pub(super) fn training_respawn_at(&self, i: usize) -> Option<u64> {
        self.training_runtime.as_ref()?[i].respawn_at_ms
    }

    pub(super) fn handle_member_defeat(
        &mut self,
        i: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let Some(rule) = c.training_runtime().and_then(|r| r.respawn.as_ref()) else {
            self.terminal_intent(i, Outcome::Death, events);
            return;
        };
        if self.training_respawn_at(i).is_none() {
            let seat = &mut self.training_runtime.as_mut().unwrap()[i];
            seat.respawn_at_ms = Some(now + rule.delay_ms);
            seat.clear_defeated_life = true;
        }
    }

    pub(super) fn finish_training_defeats(&mut self, now: u64, events: &mut Events) {
        if self.training_runtime.is_none() {
            return;
        }
        for i in 0..self.members.len() {
            if !std::mem::take(&mut self.training_runtime.as_mut().unwrap()[i].clear_defeated_life)
            {
                continue;
            }
            // Some effects temporarily remove their plan while resolving contact.
            // Retire the defeated life after those plans have finished updating.
            let old = self.members[i].reference.entity.clone();
            self.clear_member_combat(i, None, events);
            self.clear_combat_target(&old, now, events);
            self.terrain_damage_remainders
                .remove(&triggers::entity_key(&old));
        }
    }

    pub(super) fn advance_training_lifecycle(&mut self, c: &Config, now: u64, events: &mut Events) {
        if !self.training {
            return;
        }
        if let Some(rule) = c.training_runtime().and_then(|r| r.respawn.as_ref()) {
            for (i, &point) in rule.points.iter().enumerate() {
                if self.training_respawn_at(i).is_none_or(|at| now < at) {
                    continue;
                }
                if !self.grid.clear(point, c.params.player_radius, 0) {
                    self.training_runtime.as_mut().unwrap()[i].respawn_at_ms =
                        Some(now + c.step_ms);
                    continue;
                }
                self.training_runtime.as_mut().unwrap()[i].respawn_at_ms = None;
                self.restore_member_life(i, point, (10000, 10000), c, now, events);
            }
        }
        for j in 0..self.npcs.len() {
            let npc = &self.npcs[j];
            if npc.training_active
                && npc.hp == 0
                && npc.entity.generation < u32::MAX
                && self.rebuilds_training_dummy(j, c)
                && self.grid.clear(npc.home, npc.profile.combat.radius, 0)
            {
                let old = npc.entity.clone();
                self.clear_combat_target(&old, now, events);
                self.replace_training_npc(j, true, events);
            }
        }
    }

    pub(super) fn training_view(&self, i: usize) -> Option<TrainingView> {
        let seat = &self.training_runtime.as_ref()?[i];
        Some(TrainingView {
            respawn_at_ms: seat.respawn_at_ms,
            metrics: seat
                .metrics
                .as_ref()
                .map(|(_, metrics)| metrics.published.clone()),
        })
    }

    pub(super) fn reset_training_metrics(&mut self, i: usize, now: u64) -> Result<()> {
        let (_, metrics) = self
            .training_runtime
            .as_mut()
            .and_then(|seats| seats[i].metrics.as_mut())
            .ok_or(Error::Unsupported)?;
        let member = &self.members[i];
        if member.phase != MemberPhase::Active || !member.ready || member.protected {
            return Err(Error::NotReady);
        }
        *metrics = Metrics::default();
        metrics.published.sampled_at_ms = now;
        Ok(())
    }

    pub(super) fn sample_training_metrics(&mut self, now: u64) {
        if let Some(seats) = &mut self.training_runtime {
            for (seat, member) in seats.iter_mut().zip(&self.members) {
                if member.phase == MemberPhase::Active
                    && let Some((rule, metrics)) = &mut seat.metrics
                {
                    metrics.sample(*rule, now);
                }
            }
        }
    }

    pub(super) fn same_training_scope(&self, a: Owner, b: Owner) -> bool {
        if !self.training {
            return true;
        }
        let seat = |owner| match owner {
            Owner::Member(i) => Some(i),
            Owner::Npc(i) => self.npcs[i].training_owner,
            Owner::Summon(i) => Some(self.summons[i].member),
        };
        seat(a) == seat(b)
    }

    pub(super) fn change_training_targets(
        &mut self,
        i: usize,
        preset: Option<&str>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let spec = &c.training_targets().ok_or(Error::Unsupported)?[i];
        let selected = if let Some(preset) = preset {
            spec.presets
                .get(preset)
                .ok_or(Error::InvalidTarget)?
                .as_slice()
        } else {
            &[]
        };
        for npc in self.npcs.iter().filter(|npc| npc.training_owner == Some(i)) {
            let enabled = selected.contains(&npc.entity.id);
            if enabled && !self.grid.clear(npc.home, npc.profile.combat.radius, 0) {
                return Err(Error::InvalidTarget);
            }
            if (enabled || npc.training_active) && npc.entity.generation == u32::MAX {
                return Err(Error::Capacity);
            }
        }
        self.clear_member_combat(i, None, events);
        self.members[i].end_recovery(now);
        self.replace_training_npcs(i, selected, events);
        Ok(())
    }

    pub(super) fn replace_training_npcs(
        &mut self,
        seat: usize,
        selected: &[String],
        events: &mut Events,
    ) {
        let owned: Vec<_> = self
            .npcs
            .iter()
            .enumerate()
            .filter(|(_, npc)| npc.training_owner == Some(seat))
            .map(|(j, _)| j)
            .collect();
        for &j in &owned {
            let npc = &self.npcs[j];
            let enabled = selected.contains(&npc.entity.id);
            if !npc.training_active && !enabled {
                continue;
            }
            self.replace_training_npc(j, enabled, events);
        }
        // Retained NPC statuses can outlive the original entity generation.
        let mut removed = vec![];
        let before = self.statuses.entries().to_vec();
        self.statuses.remove(
            StatusRemovalReason::Reset,
            |status| {
                status.source.kind == StatusEntityKind::Monster
                    && owned
                        .iter()
                        .any(|&j| self.npcs[j].entity.id == status.source.id)
            },
            &mut removed,
        );
        self.route_status_events(removed, &before, events);
    }

    fn replace_training_npc(&mut self, j: usize, enabled: bool, events: &mut Events) {
        let old = self.npcs[j].entity.clone();
        self.clear_owned_statuses(&old, StatusRemovalReason::Reset, events);
        self.clear_areas(&old, events);
        self.clear_monster_contribution(j);
        self.terrain_damage_remainders
            .remove(&triggers::entity_key(&old));
        self.shots.retain(|shot| shot.view.source != old);
        let npc = &mut self.npcs[j];
        let mut next = Npc::new(
            EntityRef {
                generation: old.generation.saturating_add(1),
                ..old
            },
            npc.template_id.clone(),
            npc.profile.clone(),
            npc.home,
        );
        next.training_owner = npc.training_owner;
        next.training_active = enabled;
        if !enabled {
            next.hp = 0;
        }
        *npc = next;
    }
}

impl World {
    /// 训练工具：清除目标席位的战斗技能与携带神通冷却（物品冷却不受影响）。
    fn training_tools_gate(&self, c: &Config) -> Result<()> {
        if c.training_targets().is_none()
            || c.mechanism_version != 106
            || c.training_tools().is_none_or(|tools| !tools.enabled)
        {
            return Err(Error::Unsupported);
        }
        Ok(())
    }

    pub(crate) fn reset_training_cooldowns(
        &mut self,
        seats: &[usize],
        c: &Config,
        events: &mut Events,
    ) -> Result<()> {
        self.training_tools_gate(c)?;
        for &i in seats {
            let participant = self.members[i].reference.owner_id.clone();
            self.members[i].cooldowns.clear();
            events.push((
                Some(i),
                EventV4::Private {
                    event: Event::TrainingCooldownsReset {
                        participant_id: participant.clone(),
                    },
                },
            ));
        }
        Ok(())
    }

    /// 训练工具：存活成员补满生命/灵力，并按配置清单清除（净化）状态。
    /// 清单为空则只做数值恢复。死亡成员 `InvalidState`（先整体校验再施加，保持原子）。
    pub(crate) fn restore_training_member(
        &mut self,
        seats: &[usize],
        clears: &[String],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        self.training_tools_gate(c)?;
        for &i in seats {
            if self.members[i].hp == 0 {
                return Err(Error::InvalidState);
            }
        }
        // 负面分类：clears 为空 = 清除全部 harmful 定义（内核 is_harmful 分类）；
        // 非空 = 只清除清单内 id（机制 87 显式列表语义）。正面增益永不触碰。
        let harmful_defs: std::collections::BTreeSet<String> = c
            .pve
            .as_ref()
            .map(|pve| {
                pve.status_definitions
                    .iter()
                    .filter(|d| d.is_harmful())
                    .map(|d| d.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        for &i in seats {
            let entity = self.members[i].reference.entity.clone();
            let max_hp = self.entity_max_hp(&entity, c, now).ok_or(Error::NotReady)?;
            let max_spirit = self.member_max_spirit(i, c, now);
            let participant = self.members[i].reference.owner_id.clone();
            let internal = status_entity(&entity);
            let mut cleansed = vec![];
            {
                let before = self.statuses.entries().to_vec();
                let to_cleanse: std::collections::BTreeSet<String> = before
                    .iter()
                    .filter(|s| {
                        s.target == internal
                            && (clears.is_empty() || clears.contains(&s.definition_id))
                            && harmful_defs.contains(&s.definition_id)
                    })
                    .map(|s| s.definition_id.clone())
                    .collect();
                let instance_ids: Vec<String> = before
                    .iter()
                    .filter(|s| s.target == internal && to_cleanse.contains(&s.definition_id))
                    .map(|s| s.instance_id.clone())
                    .collect();
                if !instance_ids.is_empty() {
                    let mut raw = vec![];
                    self.statuses.remove(
                        StatusRemovalReason::Cleansed,
                        |s| instance_ids.contains(&s.instance_id),
                        &mut raw,
                    );
                    self.route_status_events(raw, &before, events);
                    self.reconcile_caps(c, now);
                }
                cleansed.extend(to_cleanse);
            }
            let m = &mut self.members[i];
            m.hp = max_hp;
            m.spirit = max_spirit;
            events.push((
                Some(i),
                EventV4::Private {
                    event: Event::TrainingMemberRestored {
                        participant_id: participant,
                        hp: max_hp,
                        spirit: max_spirit,
                        cleansed,
                    },
                },
            ));
        }
        Ok(())
    }
}
