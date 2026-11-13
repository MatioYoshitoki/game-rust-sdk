use super::*;
use crate::config::npc_ai::{MAX_SLOTS, NpcAiConfig, NpcAiProfile};
use std::cmp::Reverse;

#[derive(Default)]
pub(super) struct NpcAiState {
    threat: Vec<(EntityRef, u64)>,
    signals_help: bool,
    forced: bool,
    pub(super) retreating: bool,
    pub(super) ready_at: [u64; MAX_SLOTS],
    pub(super) decide_at: u64,
}

pub(super) type NpcTarget = (Owner, EntityRef, [f64; 2], f64);
pub(super) type HelpSignal = (usize, EntityRef, [f64; 2]);

impl Npc {
    pub(super) fn boss_phase(&self) -> Option<&crate::config::BossPhase> {
        self.profile
            .boss
            .as_ref()
            .map(|boss| &boss.phases[self.boss_phase])
    }

    pub(super) fn boss_view(&self) -> Option<BossPhaseView> {
        let boss = self.profile.boss.as_ref()?;
        let phase = &boss.phases[self.boss_phase];
        Some(BossPhaseView {
            phase_id: phase.id.clone(),
            phase_name: phase.name.clone(),
            phase_index: self.boss_phase as u32 + 1,
            phase_count: boss.phases.len() as u32,
            epoch: self.entity.generation,
            attack_interval_ms: phase.attack_interval_ms,
            cast_chance: phase.cast_chance,
        })
    }
}

impl World {
    pub(super) fn advance_npc_boss(&mut self, j: usize, c: &Config, now: u64, events: &mut Events) {
        let n = &self.npcs[j];
        if n.profile.boss.is_none() || n.hp == 0 || n.returning {
            return;
        }
        let max_hp = self.entity_max_hp(&n.entity, c, now).unwrap();
        let n = &mut self.npcs[j];
        let boss = n.profile.boss.as_ref().unwrap();
        while let Some(next) = boss.phases.get(n.boss_phase + 1) {
            if u64::from(n.hp) * 10000 > u64::from(max_hp) * u64::from(next.hp_threshold_bps) {
                break;
            }
            let from_index = n.boss_phase as u32 + 1;
            n.boss_phase += 1;
            n.ai.decide_at = 0;
            events.push((
                None,
                EventV4::BossPhaseChanged {
                    entity: n.entity.clone(),
                    from_index,
                    phase: n.boss_view().unwrap(),
                },
            ));
        }
    }
    pub(super) fn npc_ai_profile<'a>(&self, j: usize, c: &'a Config) -> Option<&'a NpcAiProfile> {
        c.pve
            .as_ref()?
            .npc_ai
            .as_ref()?
            .profiles
            .get(&self.npcs[j].template_id)
    }

    pub(super) fn npc_can_be_attacked(&self, j: usize) -> bool {
        let n = &self.npcs[j];
        n.hp > 0
            && self.mode_npc_enabled(&n.entity.id)
            && if n.pursuit.is_some() {
                n.pursuit_can_fight()
            } else if self.grid.config.npc_ai.is_some() {
                distance(n.position, n.home) <= n.profile.leash
            } else {
                !n.returning
            }
    }

    pub(super) fn npc_ai_hurt(&mut self, j: usize, source: &EntityRef, hp_damage: u32) {
        if self.grid.config.npc_ai.is_none() || hp_damage == 0 {
            return;
        }
        let source_valid = self.npc_enemy(source).is_some();
        let n = &mut self.npcs[j];
        if n.hp == 0 {
            n.ai = NpcAiState::default();
        } else if source_valid {
            if let Some((_, threat)) = n.ai.threat.iter_mut().find(|(entity, _)| entity == source) {
                *threat = threat.saturating_add(u64::from(hp_damage));
            } else {
                n.ai.threat.push((source.clone(), u64::from(hp_damage)));
            }
            if n.returning {
                n.returning = false;
                n.path.clear();
                n.repath = 0;
            }
            n.ai.decide_at = 0;
        }
    }

    pub(super) fn forget_npc_enemy(&mut self, entity: &EntityRef, events: &mut Events) {
        let ai_enabled = self.grid.config.npc_ai.is_some();
        if !ai_enabled && self.contested.is_none() {
            return;
        }
        let mut interrupted = vec![];
        for n in &mut self.npcs {
            if !ai_enabled && n.pursuit.is_none() {
                continue;
            }
            n.ai.threat.retain(|(source, _)| source != entity);
            if n.casting
                .as_ref()
                .is_some_and(|cast| cast.aims_at(entity, n.target.as_ref()))
            {
                n.casting = None;
                interrupted.push(n.entity.clone());
            }
            if n.target.as_ref() == Some(entity) {
                n.target = None;
                n.ai.signals_help = false;
                n.ai.retreating = false;
            }
        }
        for source in interrupted {
            self.stop_npc_areas(&source, AreaEndReason::Cancelled, events);
        }
    }

    pub(super) fn npc_enemy(&self, entity: &EntityRef) -> Option<Owner> {
        match self.resolve_entity(entity)? {
            Owner::Member(i) if self.members[i].eligible() => Some(Owner::Member(i)),
            Owner::Summon(i) if self.summon_eligible(i) => Some(Owner::Summon(i)),
            _ => None,
        }
    }

    pub(super) fn npc_help_signals(&self) -> Vec<HelpSignal> {
        if self.grid.config.npc_ai.is_none() {
            return vec![];
        }
        self.npcs
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let target = n.target.as_ref()?;
                let enemy = self.npc_enemy(target)?;
                (n.ai.signals_help
                    && !n.returning
                    && self.npc_can_be_attacked(i)
                    && distance(n.position, self.actor_position(enemy)) <= n.profile.memory_range)
                    .then(|| (i, target.clone(), n.position))
            })
            .collect()
    }

    pub(super) fn same_npc_group(&self, ai: &NpcAiConfig, a: usize, b: usize) -> bool {
        ai.groups.get(&self.npcs[a].entity.id) == ai.groups.get(&self.npcs[b].entity.id)
    }

    pub(super) fn retreat_npc(
        &mut self,
        j: usize,
        profile: &NpcAiProfile,
        (target, distance_to_target): ([f64; 2], f64),
        movement: f64,
    ) -> bool {
        let Some(minimum) = profile.retreat_below else {
            return false;
        };
        let n = &mut self.npcs[j];
        let preferred = n.profile.combat.melee_range;
        n.ai.retreating = distance_to_target < minimum
            || n.ai.retreating && distance_to_target + 1e-6 < preferred;
        if !n.ai.retreating {
            return false;
        }
        let from = n.position;
        let mut away = normalize([from[0] - target[0], from[1] - target[1]]);
        if away == [0., 0.] {
            away = [1., 0.];
        }
        let step = movement.min(preferred - distance_to_target);
        let options = [
            away,
            normalize([away[0] - away[1], away[1] + away[0]]),
            normalize([away[0] + away[1], away[1] - away[0]]),
        ];
        for dir in options {
            let end = [from[0] + step * dir[0], from[1] + step * dir[1]];
            if distance(end, n.home) <= n.profile.leash
                && self.grid.line(from, end, n.profile.combat.radius, 0)
            {
                n.position = end;
                n.path.clear();
                n.repath = 0;
                break;
            }
        }
        true
    }

    pub(super) fn select_npc_ai_target(
        &mut self,
        j: usize,
        ai: &NpcAiConfig,
        help: &[HelpSignal],
        now: u64,
    ) -> Option<NpcTarget> {
        let n = &self.npcs[j];
        if n.returning || !self.npc_can_be_attacked(j) {
            return None;
        }
        let from = n.position;
        let memory = n.profile.memory_range;
        let natural = n.profile.combat.aggro_range;
        let candidate = |entity: &EntityRef| {
            let owner = self.npc_enemy(entity)?;
            if !self.same_training_scope(Owner::Npc(j), owner) {
                return None;
            }
            let position = self.actor_position(owner);
            let d = distance(from, position);
            Some((owner, entity.clone(), position, d))
        };
        let remembered = |entity: &EntityRef| candidate(entity).filter(|t| t.3 <= memory);
        let live: Vec<_> =
            n.ai.threat
                .iter()
                .filter_map(|(entity, amount)| remembered(entity).map(|target| (target, *amount)))
                .collect();
        let mark_target = status_entity(&n.entity);
        let forced = self
            .statuses
            .entries()
            .iter()
            .filter_map(|status| {
                if status.target != mark_target
                    || now >= status.expires_at_ms
                    || !ai.forced_target_statuses.contains(&status.definition_id)
                {
                    return None;
                }
                let source = Self::status_reference(&status.source);
                Some((remembered(&source)?, status.applied_at_ms))
            })
            .min_by(|a, b| (Reverse(a.1), &a.0.1.id).cmp(&(Reverse(b.1), &b.0.1.id)))
            .map(|(target, _)| target);
        let best = live
            .iter()
            .min_by(|a, b| (Reverse(a.1), &a.0.1.id).cmp(&(Reverse(b.1), &b.0.1.id)));
        let current_threat = live
            .iter()
            .find(|(t, _)| !n.ai.forced && n.target.as_ref() == Some(&t.1));
        let threat = best.map(|best| {
            current_threat
                .filter(|current| {
                    best.1 <= current.1
                        || u128::from(best.1) * 10000
                            < u128::from(current.1) * u128::from(ai.switch_threat_bps)
                })
                .unwrap_or(best)
                .0
                .clone()
        });
        let current = n
            .target
            .as_ref()
            .filter(|_| !n.ai.forced)
            .and_then(remembered);
        let assist = || {
            help.iter()
                .filter_map(|(i, entity, position)| {
                    (*i != j
                        && self.same_training_scope(Owner::Npc(j), Owner::Npc(*i))
                        && self.same_npc_group(ai, j, *i)
                        && self.npc_can_be_attacked(*i)
                        && distance(from, *position) <= n.profile.help_range)
                        .then(|| remembered(entity))
                        .flatten()
                })
                .min_by(target_distance_order)
        };
        let acquire = || {
            self.members
                .iter()
                .map(|m| &m.reference.entity)
                .chain(self.summons.iter().map(|s| &s.entity))
                .filter_map(candidate)
                .filter(|t| t.3 <= natural.min(memory) && self.grid.line(from, t.2, 0., 2))
                .min_by(target_distance_order)
        };
        let is_forced = forced.is_some();
        let chosen = forced
            .or(threat)
            .map(|t| (t, true))
            .or_else(|| current.map(|t| (t, n.ai.signals_help)))
            .or_else(|| assist().map(|t| (t, false)))
            .or_else(|| acquire().map(|t| (t, true)));
        let state = &mut self.npcs[j].ai;
        state.forced = is_forced;
        state.threat = live.into_iter().map(|(t, amount)| (t.1, amount)).collect();
        state.signals_help = chosen.as_ref().is_some_and(|(_, signal)| *signal);
        chosen.map(|(target, _)| target)
    }
}

fn target_distance_order(a: &NpcTarget, b: &NpcTarget) -> std::cmp::Ordering {
    a.3.total_cmp(&b.3).then(a.1.id.cmp(&b.1.id))
}
