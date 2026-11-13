//! A wave schedules contacts against frozen candidates, with no moving collision body.
use super::areas::basic_fields::PendingBasicField;
use super::triggers::ActionKey;
use super::*;
use crate::config::{BasicFieldTrigger, BasicWaveConfig, BasicWaveGeometryConfig};
use std::collections::VecDeque;

struct Contact {
    view: BasicWaveTargetView,
    raw: u32,
}

pub(super) struct BasicWavePlan {
    view: BasicWaveView,
    action: ActionKey,
    phase: Option<BasicWavePhase>,
    charged_stage: Option<usize>,
    primary: EntityRef,
    field: Option<PendingBasicField>,
    contacts: VecDeque<Contact>,
}

impl BasicWavePlan {
    pub(super) fn pending_field(&self) -> Option<(&EntityRef, &PendingBasicField)> {
        self.field.as_ref().map(|field| (&self.view.source, field))
    }
    pub(super) fn action(&self) -> ActionKey {
        self.action.clone()
    }

    pub(super) fn deadline(&self) -> u64 {
        self.contacts
            .back()
            .expect("wave has a primary target")
            .view
            .at_ms
    }

    pub(super) fn view(&self) -> BasicWaveView {
        let mut view = self.view.clone();
        view.pending = self.contacts.iter().map(|c| c.view.clone()).collect();
        view
    }

    fn end(&self, reason: BasicWaveEndReason, events: &mut Events) {
        events.push((
            None,
            EventV4::BasicWaveEnded {
                wave_id: self.view.id.clone(),
                reason,
            },
        ));
    }
}

impl World {
    pub(super) fn ready_stacked_wave<'a>(
        &'a self,
        i: usize,
        c: &'a Config,
        now: u64,
    ) -> Option<(&'a crate::config::BasicStackedWave, &'a StatusView)> {
        let member = &self.members[i];
        let upgrade = member.basic(c).stacked_wave.as_ref()?;
        let source = status_entity(&member.reference.entity);
        let status = self.statuses.entries().iter().find(|status| {
            status.source == source
                && status.target == source
                && status.definition_id == upgrade.stack_status
                && now < status.expires_at_ms
                && status.stacks == status.max_stacks
        })?;
        Some((upgrade, status))
    }

    pub(super) fn basic_lock_range(&self, i: usize, c: &Config, now: u64) -> f64 {
        self.ready_stacked_wave(i, c, now).map_or_else(
            || self.members[i].basic_range(c),
            |(upgrade, _)| upgrade.range,
        )
    }

    pub(super) fn plan_basic_wave(
        &self,
        i: usize,
        p: &Pending,
        wave: &BasicWaveConfig,
        c: &Config,
        now: u64,
    ) -> Result<BasicWavePlan> {
        let primary = p.target.as_ref().ok_or(Error::InvalidTarget)?;
        let from = self.members[i].position;
        let source = &self.members[i].reference.entity;
        let member = &self.members[i];
        let charged_stage = match p.charged_basic {
            Some(ChargedBasic::Wave(stage)) => Some(stage),
            _ => None,
        };
        let (wave, lock_range) =
            if let Some(stage) = charged_stage.and_then(|stage| member.charged_wave(c, stage)) {
                (&stage.wave, stage.range)
            } else if let Some((upgrade, _)) = self.ready_stacked_wave(i, c, now) {
                (&upgrade.finisher, upgrade.range)
            } else {
                (wave, member.basic_range(c))
            };
        let phase = member.wave_phase(c);
        let (plan_id, speed, primary_bps, additional_bps) =
            if let Some(alternate) = member.alternate_wave(c) {
                (
                    &alternate.plan_id,
                    alternate.speed,
                    alternate.primary_damage_bps,
                    alternate.additional_damage_bps,
                )
            } else {
                (
                    &wave.plan_id,
                    wave.speed,
                    wave.primary_damage_bps,
                    wave.additional_damage_bps,
                )
            };
        let shape = if phase == Some(BasicWavePhase::Alternate) {
            BasicWaveGeometryConfig::Radial {}
        } else {
            wave.geometry.clone()
        };
        let (geometry, half_cosine) = match shape {
            BasicWaveGeometryConfig::Directional { cone_degrees } => (
                BasicWaveGeometry::Directional {
                    direction: p.direction,
                    cone_degrees,
                },
                Some((cone_degrees.to_radians() * 0.5).cos()),
            ),
            BasicWaveGeometryConfig::Radial {} => (BasicWaveGeometry::Radial {}, None),
        };
        let landing = wave.field.as_ref().and_then(|field| match field.trigger {
            BasicFieldTrigger::Landing { impact_radius } => Some(impact_radius),
            _ => None,
        });
        let (origin, range, geometry) = if let Some(radius) = landing {
            let position = self.basic_target_position(i, primary)?;
            if distance(from, position) > lock_range
                || self.grid.piercing_wall(from, position, 0.).is_some()
            {
                return Err(Error::InvalidTarget);
            }
            (position, radius, BasicWaveGeometry::Landing { from })
        } else {
            (from, lock_range, geometry)
        };
        let land_after =
            landing.map(|_| ((distance(from, origin) * 1000. / speed).ceil() as u64).max(1));
        let mut candidates = self
            .npcs
            .iter()
            .filter_map(|npc| {
                let delta = [npc.position[0] - origin[0], npc.position[1] - origin[1]];
                let distance = delta[0].hypot(delta[1]);
                if npc.hp == 0 || npc.returning {
                    return None;
                }
                if landing.is_some() && !self.basic_circle_contact(origin, range, npc)
                    || landing.is_none() && distance > range
                {
                    return None;
                }
                let travel = if let Some(cosine) = half_cosine {
                    let projection = delta[0] * p.direction[0] + delta[1] * p.direction[1];
                    if projection < 0. || projection + 1e-9 < cosine * distance {
                        return None;
                    }
                    projection.min(distance)
                } else {
                    distance
                };
                if !self.hostile_contact(source, &npc.entity)
                    || landing.is_none()
                        && self.grid.piercing_wall(origin, npc.position, 0.).is_some()
                {
                    return None;
                }
                Some((&npc.entity, distance, travel))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            (a.0 != primary)
                .cmp(&(b.0 != primary))
                .then(a.1.total_cmp(&b.1))
                .then(a.0.id.cmp(&b.0.id))
        });
        if candidates.first().is_none_or(|c| c.0 != primary) {
            return Err(Error::InvalidTarget);
        }
        let rule = &c
            .combat_rules
            .as_ref()
            .expect("basic wave capability")
            .basic_attack;
        let attack = self.stats(i, c, now).0;
        let primary_raw = rule.budget(attack, primary_bps, 10000, 0);
        let additional_raw = rule.budget(attack, additional_bps, 10000, 0);
        let mut contacts = candidates
            .into_iter()
            .take(wave.target_cap)
            .map(|(target, _, travel)| Contact {
                view: BasicWaveTargetView {
                    target: Self::unit_wire(target),
                    at_ms: now
                        + land_after
                            .unwrap_or_else(|| ((travel * 1000. / speed).ceil() as u64).max(1)),
                },
                raw: if target == primary {
                    primary_raw
                } else {
                    additional_raw
                },
            })
            .collect::<Vec<_>>();
        contacts.sort_by_key(|c| c.view.at_ms);
        Ok(BasicWavePlan {
            view: BasicWaveView {
                id: String::new(),
                plan_id: plan_id.clone(),
                source: source.clone(),
                origin,
                geometry,
                range,
                speed,
                released_at_ms: now,
                pending: vec![],
            },
            action: ActionKey::new(source, &p.id),
            phase,
            charged_stage,
            primary: primary.clone(),
            field: wave
                .field
                .as_ref()
                .map(|rule| self.reserve_basic_field(i, rule, c, now))
                .transpose()?,
            contacts: contacts.into(),
        })
    }

    pub(super) fn start_basic_wave(
        &mut self,
        i: usize,
        mut wave: BasicWavePlan,
        events: &mut Events,
    ) {
        if let Some(phase) = wave.phase {
            self.members[i].basic_wave_phase = match phase {
                BasicWavePhase::Primary => BasicWavePhase::Alternate,
                BasicWavePhase::Alternate => BasicWavePhase::Primary,
            };
        }
        wave.view.id = self.next_id("basic-wave");
        events.push((None, EventV4::BasicWaveStarted { wave: wave.view() }));
        self.basic_waves.push(wave);
    }

    pub(super) fn clear_basic_waves(&mut self, source: &EntityRef, events: &mut Events) {
        self.basic_waves.retain(|wave| {
            if wave.view.source == *source {
                wave.end(BasicWaveEndReason::SourceInvalid, events);
                false
            } else {
                true
            }
        });
    }

    pub(super) fn advance_basic_waves(&mut self, c: &Config, now: u64, events: &mut Events) {
        for mut wave in std::mem::take(&mut self.basic_waves) {
            let source = &wave.view.source;
            let Some(Owner::Member(i)) = self.resolve_entity(source) else {
                wave.end(BasicWaveEndReason::SourceInvalid, events);
                continue;
            };
            let member = &self.members[i];
            let behavior = wave
                .charged_stage
                .and_then(|stage| member.charged_wave(c, stage))
                .map(|stage| &stage.wave)
                .or_else(|| member.basic(c).wave.as_ref())
                .and_then(|wave| wave.behavior.as_ref());
            let landing = matches!(wave.view.geometry, BasicWaveGeometry::Landing { .. });
            while self.members[i].living()
                && wave.contacts.front().is_some_and(|c| c.view.at_ms <= now)
            {
                let contact = wave.contacts.pop_front().unwrap();
                let target = EntityRef {
                    kind: StatusEntityKind::Monster,
                    id: contact.view.target.id,
                    generation: contact.view.target.generation,
                };
                let field = if !landing && target == wave.primary {
                    wave.field.take()
                } else {
                    None
                };
                if !self.hostile_contact(source, &target) {
                    continue;
                }
                let Some(Owner::Npc(j)) = self.resolve_entity(&target) else {
                    continue;
                };
                if landing
                    && !self.basic_circle_contact(wave.view.origin, wave.view.range, &self.npcs[j])
                {
                    continue;
                }
                let contact_position = self.npcs[j].position;
                events.push((
                    None,
                    EventV4::BasicWaveContact {
                        wave_id: wave.view.id.clone(),
                        source: source.clone(),
                        target,
                        scheduled_at_ms: contact.view.at_ms,
                    },
                ));
                let raw = self.wave_contact_budget((i, j), wave.phase, behavior, contact.raw, now);
                if self.resolve_native_basic_contact(
                    i,
                    Owner::Npc(j),
                    (source, &wave.action.id, raw),
                    c,
                    now,
                    events,
                ) {
                    if let Some(behavior) = behavior {
                        self.apply_wave_control(
                            (i, j),
                            (wave.phase, &wave.view.geometry, wave.view.origin, behavior),
                            c,
                            now,
                            events,
                        );
                    }
                    if let Some(field) = field {
                        self.start_basic_field(
                            i,
                            field,
                            &wave.action.id,
                            contact_position,
                            now,
                            events,
                        );
                    }
                }
            }
            if !self.members[i].living() {
                wave.end(BasicWaveEndReason::SourceInvalid, events);
            } else if wave.contacts.is_empty() {
                if let Some(field) = wave.field.take() {
                    self.start_basic_field(
                        i,
                        field,
                        &wave.action.id,
                        wave.view.origin,
                        now,
                        events,
                    );
                }
                wave.end(BasicWaveEndReason::Resolved, events);
            } else {
                self.basic_waves.push(wave);
            }
        }
    }
}

#[cfg(all(test, match_sdk_test_data))]
impl World {
    pub(crate) fn place_wave_test_npc(&mut self, index: usize, position: [f64; 2]) {
        self.npcs[index].position = position;
    }

    pub(crate) fn replace_wave_test_npc(&mut self, index: usize) {
        self.npcs[index].entity.generation += 1;
    }
}
