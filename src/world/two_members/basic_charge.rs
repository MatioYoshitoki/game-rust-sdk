//! Server-owned held input and the delivery selected when it ends.
use super::*;
use crate::config::{BasicChargeBehavior, BasicChargeWave, DamageKind};

pub(super) struct BasicChargeSession {
    id: String,
    started: u64,
    checkpoint_epoch: u32,
}

#[derive(Clone, Copy)]
pub(super) enum ChargedBasic {
    Wave(usize),
    Dash(f64),
    Flight(usize),
}

pub(super) struct BasicDashPlan {
    pub(super) at: u64,
    end: [f64; 2],
    contacts: Vec<(EntityRef, u32)>,
}

impl Member {
    pub(super) fn charged_wave<'a>(
        &self,
        c: &'a Config,
        stage: usize,
    ) -> Option<&'a BasicChargeWave> {
        let BasicChargeBehavior::StagedWave { upgrades } = &self.basic(c).charge.as_ref()?.behavior
        else {
            return None;
        };
        upgrades.get(stage.checked_sub(1)?)
    }

    pub(super) fn active_guard(&self, c: &Config, now: u64) -> Option<(u32, u32, DamageKind)> {
        let session = self.basic_charge.as_ref()?;
        let charge = self.basic(c).charge.as_ref()?;
        match charge.behavior {
            BasicChargeBehavior::Guard {
                reduction_bps,
                reflect_bps,
                reflect_damage_kind,
            } if now < session.started + charge.max_ms => {
                Some((reduction_bps, reflect_bps, reflect_damage_kind))
            }
            _ => None,
        }
    }

    pub(super) fn charge_view(&self, c: &Config, now: u64) -> Option<BasicChargeView> {
        let session = self.basic_charge.as_ref()?;
        let charge = self.basic(c).charge.as_ref()?;
        let max = session.started + charge.max_ms;
        let (kind, tier) = match &charge.behavior {
            BasicChargeBehavior::StagedWave { upgrades } => (
                BasicChargeKind::StagedWave,
                1 + wave_stage(upgrades, now.saturating_sub(session.started)) as u8,
            ),
            BasicChargeBehavior::Guard { .. } if now < max => (BasicChargeKind::Guard, 1),
            BasicChargeBehavior::Guard { .. } => return None,
            BasicChargeBehavior::Dash(_) => (BasicChargeKind::Dash, 1),
            BasicChargeBehavior::Flight(flight) => (
                BasicChargeKind::Flight,
                flight
                    .stages
                    .partition_point(|stage| stage.after_ms <= now.saturating_sub(session.started))
                    as u8,
            ),
        };
        Some(BasicChargeView {
            session_id: session.id.clone(),
            kind,
            started_at_ms: session.started,
            max_charge_at_ms: max,
            tier,
        })
    }

    pub(super) fn dash_ready_at(&self, c: &Config) -> Option<u64> {
        (self.living()
            && matches!(
                self.basic(c).charge.as_ref()?.behavior,
                BasicChargeBehavior::Dash(_)
            ))
        .then_some(self.basic_charge_ready)
    }
}

pub(super) struct ChargeAim<'a> {
    pub(super) target: Option<&'a UnitTarget>,
    pub(super) direction: Option<[f64; 2]>,
    pub(super) position: Option<[f64; 2]>,
}

fn charge_target(target: &UnitTarget) -> Result<EntityRef> {
    if !valid_id(&target.id) || target.generation == 0 {
        return Err(Error::InvalidTarget);
    }
    Ok(EntityRef {
        kind: StatusEntityKind::Monster,
        id: target.id.clone(),
        generation: target.generation,
    })
}

fn wave_stage(upgrades: &[BasicChargeWave], elapsed: u64) -> usize {
    upgrades.partition_point(|stage| stage.after_ms <= elapsed)
}

impl World {
    pub(super) fn expire_basic_charge(&mut self, i: usize, c: &Config, now: u64) {
        let member = &mut self.members[i];
        if let Some(session) = &member.basic_charge
            && let Some(charge) = &member.basic(c).charge
            && matches!(charge.behavior, BasicChargeBehavior::Guard { .. })
            && now >= session.started + charge.max_ms
        {
            member.basic_charge = None;
        }
    }

    pub(super) fn start_basic_charge(&mut self, i: usize, c: &Config, now: u64) -> Result<()> {
        let member = &self.members[i];
        let charge = member.basic(c).charge.as_ref().ok_or(Error::Unsupported)?;
        if !member.basic_slot_free(now)
            || member.basic_delivery.is_some()
            || member.queued_basic.is_some()
            || member.queued_cast.is_some()
            || now < member.basic_ready
            || matches!(charge.behavior, BasicChargeBehavior::Dash(_))
                && now < member.basic_charge_ready
        {
            return Err(Error::InvalidState);
        }
        let id = self.next_id("basic-charge");
        self.members[i].basic_charge = Some(BasicChargeSession {
            id,
            started: now,
            checkpoint_epoch: self.checkpoint_epoch,
        });
        Ok(())
    }

    fn take_basic_charge(&mut self, i: usize, session_id: &str) -> Result<BasicChargeSession> {
        self.members[i]
            .basic_charge
            .take_if(|s| s.id == session_id)
            .ok_or(Error::InvalidTarget)
    }

    pub(super) fn cancel_basic_charge(&mut self, i: usize, session_id: &str) -> Result<()> {
        self.take_basic_charge(i, session_id).map(|_| ())
    }

    pub(super) fn release_basic_charge(
        &mut self,
        i: usize,
        session_id: &str,
        ChargeAim {
            target,
            direction,
            position,
        }: ChargeAim<'_>,
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let session = self.take_basic_charge(i, session_id)?;
        let basic = self.members[i].basic(c);
        let charge = basic.charge.as_ref().expect("held charge has a definition");
        let held = (now - session.started).min(charge.max_ms);
        if let BasicChargeBehavior::Flight(flight) = &charge.behavior {
            let stage = flight
                .stages
                .partition_point(|stage| stage.after_ms <= held)
                - 1;
            let point = match (target, direction, position) {
                (Some(target), None, None) => {
                    self.basic_target_position(i, &charge_target(target)?)?
                }
                (None, Some(direction), None) => {
                    let direction = validated_direction(direction)?;
                    let from = self.members[i].position;
                    [
                        from[0] + direction[0] * basic.range,
                        from[1] + direction[1] * basic.range,
                    ]
                }
                (None, None, Some(point)) => point,
                _ => return Err(Error::InvalidTarget),
            };
            self.prepare_charged_flight(i, point, stage, id, c, now)?;
            self.members[i].pending.as_mut().unwrap().checkpoint_epoch = session.checkpoint_epoch;
            return Ok(());
        }
        if position.is_some() {
            return Err(Error::InvalidTarget);
        }
        let stage = match &charge.behavior {
            BasicChargeBehavior::StagedWave { upgrades } => wave_stage(upgrades, held),
            BasicChargeBehavior::Guard { .. } => 0,
            BasicChargeBehavior::Dash(dash) => {
                if target.is_some() || self.member_has_status(i, StatusAttribute::Root, now) {
                    return Err(Error::InvalidTarget);
                }
                let direction = validated_direction(direction.ok_or(Error::InvalidTarget)?)?;
                let ratio = held.saturating_sub(dash.tap_max_ms) as f64
                    / (charge.max_ms - dash.tap_max_ms) as f64;
                let travel = dash.min_distance + (dash.max_distance - dash.min_distance) * ratio;
                self.prepare(i, None, (direction, None), id, c, now)?;
                let pending = self.members[i].pending.as_mut().unwrap();
                pending.charged_basic = Some(ChargedBasic::Dash(travel));
                pending.checkpoint_epoch = session.checkpoint_epoch;
                return Ok(());
            }
            BasicChargeBehavior::Flight(_) => unreachable!(),
        };
        if direction.is_some() {
            return Err(Error::InvalidTarget);
        }
        let target = charge_target(target.ok_or(Error::InvalidTarget)?)?;
        let position = self.basic_target_position(i, &target)?;
        let from = self.members[i].position;
        let range = self.charged_wave_range(i, c, stage);
        if distance(from, position) > range
            && let Some(pursuit) = c.basic_pursuit
        {
            let extra = (pursuit.max_range - basic.range).max(0.);
            self.begin_basic_pursuit(
                i,
                (&target, position),
                ((range + extra).min(pursuit.max_range), Some(stage)),
                id,
                c,
                now,
            )?;
            self.members[i]
                .basic_pursuit
                .as_mut()
                .unwrap()
                .checkpoint_epoch = session.checkpoint_epoch;
            return Ok(());
        }
        let direction = self.charged_wave_aim(i, &target, stage, c)?;
        self.prepare(i, None, (direction, Some(target)), id, c, now)?;
        let pending = self.members[i].pending.as_mut().unwrap();
        pending.charged_basic = Some(ChargedBasic::Wave(stage));
        pending.checkpoint_epoch = session.checkpoint_epoch;
        Ok(())
    }

    pub(super) fn charged_wave_range(&self, i: usize, c: &Config, stage: usize) -> f64 {
        let member = &self.members[i];
        member
            .charged_wave(c, stage)
            .map_or(member.basic(c).range, |s| s.range)
    }

    pub(super) fn charged_wave_aim(
        &self,
        i: usize,
        target: &EntityRef,
        stage: usize,
        c: &Config,
    ) -> Result<[f64; 2]> {
        let member = &self.members[i];
        let from = member.position;
        let to = self.basic_target_position(i, target)?;
        if distance(from, to) > self.charged_wave_range(i, c, stage)
            || self.grid.piercing_wall(from, to, 0.).is_some()
        {
            return Err(Error::InvalidTarget);
        }
        let wave = member
            .charged_wave(c, stage)
            .map_or_else(|| member.basic(c).wave.as_ref().unwrap(), |s| &s.wave);
        match wave.geometry {
            crate::config::BasicWaveGeometryConfig::Radial {} => Ok([0., 0.]),
            _ => validated_direction([to[0] - from[0], to[1] - from[1]])
                .map_err(|_| Error::InvalidTarget),
        }
    }

    pub(super) fn plan_basic_dash(
        &self,
        i: usize,
        direction: [f64; 2],
        length: f64,
        c: &Config,
        now: u64,
    ) -> Result<BasicDashPlan> {
        if self.member_has_status(i, StatusAttribute::Root, now) {
            return Err(Error::InvalidState);
        }
        let member = &self.members[i];
        let BasicChargeBehavior::Dash(dash) = &member.basic(c).charge.as_ref().unwrap().behavior
        else {
            unreachable!("dash release requires a dash profile");
        };
        let from = member.position;
        let wanted = [
            from[0] + direction[0] * length,
            from[1] + direction[1] * length,
        ];
        let end = self
            .grid
            .walk_until_blocked(from, wanted, c.params.player_radius);
        let traveled = distance(from, end);
        if traveled <= 1e-6 {
            return Err(Error::InvalidTarget);
        }
        let source = &member.reference.entity;
        let mut contacts: Vec<_> = self
            .npcs
            .iter()
            .filter_map(|npc| {
                let delta = [npc.position[0] - from[0], npc.position[1] - from[1]];
                let along = delta[0] * direction[0] + delta[1] * direction[1];
                let across = (delta[0] * direction[1] - delta[1] * direction[0]).abs();
                let axis = [
                    from[0] + direction[0] * along,
                    from[1] + direction[1] * along,
                ];
                ((0. ..=traveled).contains(&along)
                    && across <= dash.half_width + npc.profile.combat.radius
                    && self.hostile_contact(source, &npc.entity)
                    && self.grid.piercing_wall(axis, npc.position, 0.).is_none())
                .then_some((along, &npc.entity))
            })
            .collect();
        contacts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        let attack = self.stats(i, c, now).0;
        let rule = &c.combat_rules.as_ref().unwrap().basic_attack;
        let contacts = contacts
            .into_iter()
            .take(dash.target_cap)
            .enumerate()
            .map(|(index, (_, target))| {
                let bps = if index == 0 {
                    dash.primary_damage_bps
                } else {
                    dash.additional_damage_bps
                };
                (target.clone(), rule.budget(attack, bps, 10000, 0))
            })
            .collect();
        Ok(BasicDashPlan {
            at: now,
            end,
            contacts,
        })
    }

    pub(super) fn start_basic_dash(
        &mut self,
        i: usize,
        pending: &Pending,
        plan: BasicDashPlan,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let member = &mut self.members[i];
        let BasicChargeBehavior::Dash(dash) = &member.basic(c).charge.as_ref().unwrap().behavior
        else {
            unreachable!("dash plan requires a dash profile");
        };
        member.position = plan.end;
        if let Some(destination) = member.path.last().copied() {
            member.path = self
                .grid
                .route(plan.end, destination, c.params.player_radius);
            if member.path.is_empty() {
                member.movement = [0., 0.];
            }
        }
        member.basic_charge_ready = now + dash.ready_interval_ms;
        let source = member.reference.entity.clone();
        let rules = c.combat_rules.as_ref().unwrap();
        let stun = Self::definitions(c, std::slice::from_ref(&dash.stun_status));
        for (target, raw) in plan.contacts {
            if !self.members[i].living() {
                break;
            }
            let Some(Owner::Npc(j)) = self.resolve_entity(&target) else {
                continue;
            };
            if self.resolve_native_basic_contact(
                i,
                Owner::Npc(j),
                (&source, &pending.id, raw),
                c,
                now,
                events,
            ) && self.hostile_contact(&source, &target)
            {
                let profile = rules
                    .npc_profiles
                    .get(&self.npcs[j].template_id)
                    .unwrap_or(&rules.default_npc_profile);
                self.displace_wave_target(j, pending.direction, dash.knockback_distance);
                if !dash.stun_resistant_profiles.contains(profile) {
                    self.apply_owned_status_definitions(
                        Owner::Member(i),
                        target,
                        &stun,
                        c,
                        now,
                        events,
                    );
                }
            }
        }
    }
}

#[cfg(all(test, match_sdk_test_data))]
impl World {
    pub(crate) fn charge_test_hit(&mut self, i: usize, j: usize, c: &Config, now: u64) -> Events {
        let mut events = vec![];
        self.resolve_damage_with_rule(
            combat::DamageIntent {
                source: self.npcs[j].entity.clone(),
                target: self.members[i].reference.entity.clone(),
                raw: 200,
                direct: true,
                ignore_armor: true,
            },
            Some(&crate::config::AttackRule {
                can_crit: false,
                ..Default::default()
            }),
            c,
            now,
            &mut events,
        );
        events
    }
}
