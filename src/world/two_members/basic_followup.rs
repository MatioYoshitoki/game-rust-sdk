//! A next basic attack uses the existing preparation with a frozen target identity.
use super::*;

impl Member {
    pub(super) fn basic_slot_free(&self, now: u64) -> bool {
        self.pending.is_none()
            && self.basic_charge.is_none()
            && self.basic_pursuit.is_none()
            && self.channel.is_none()
            && self.basic_beam.is_none()
            && self.extract_end.is_none()
            && (now >= self.recovery_until || self.can_replace_recovery_action(None, now))
    }
}

impl World {
    fn nearest_basic(
        &self,
        i: usize,
        from: [f64; 2],
        range: f64,
        basic: &BasicConfig,
    ) -> Option<&Npc> {
        self.npcs
            .iter()
            .filter(|npc| {
                npc.hp > 0
                    && !npc.returning
                    && npc.training_owner.is_none_or(|owner| owner == i)
                    && self.mode_npc_enabled(&npc.entity.id)
                    && distance(from, npc.position) <= range
                    && self.basic_sight(from, npc.position, basic)
            })
            .min_by(|a, b| {
                distance(from, a.position)
                    .total_cmp(&distance(from, b.position))
                    .then(a.entity.id.cmp(&b.entity.id))
            })
    }

    fn basic_direction(member: &Member, to: [f64; 2], c: &Config) -> Result<[f64; 2]> {
        let basic = member.basic(c);
        if basic.impact.is_some()
            || member.alternate_wave(c).is_some()
            || matches!(
                basic.wave.as_ref().map(|w| &w.geometry),
                Some(crate::config::BasicWaveGeometryConfig::Radial {})
            )
        {
            return Ok([0., 0.]);
        }
        let direction = normalize([to[0] - member.position[0], to[1] - member.position[1]]);
        if direction == [0., 0.] {
            Err(Error::InvalidTarget)
        } else {
            Ok(direction)
        }
    }

    pub(super) fn attack(
        &mut self,
        i: usize,
        explicit: Option<EntityRef>,
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let member = &self.members[i];
        let from = member.position;
        let basic = member.basic(c);
        let range = self.basic_lock_range(i, c, now);
        let explicit = explicit.or_else(|| {
            if !self.mode_opposed() {
                return None;
            }
            self.members.iter().enumerate().find_map(|(j, opponent)| {
                (i != j
                    && opponent.eligible()
                    && self.basic_aim_from(i, opponent.position, c, now).is_ok())
                .then(|| opponent.reference.entity.clone())
            })
        });
        if basic.beam.is_some() {
            return Err(Error::Unsupported);
        }
        let flight = basic.charged_flight().is_some();
        if flight
            || basic
                .impact
                .as_ref()
                .and_then(|impact| impact.field())
                .is_some()
        {
            let point = if let Some(target) = &explicit {
                self.basic_target_position(i, target)?
            } else {
                self.nearest_basic(i, from, range, basic)
                    .ok_or(Error::InvalidTarget)?
                    .position
            };
            return if flight {
                self.prepare_charged_flight(i, point, 0, id, c, now)
            } else {
                self.attack_ground(i, point, id, c, now)
            };
        }
        let (target, direction) = if let Some(target) = &explicit {
            let position = self.basic_target_position(i, target)?;
            if let Some(definition) = c.basic_pursuit
                && distance(from, position) > range
            {
                return self.begin_basic_pursuit(
                    i,
                    (target, position),
                    (definition.max_range, None),
                    id,
                    c,
                    now,
                );
            }
            (target, self.basic_aim_from(i, position, c, now)?)
        } else if let Some(npc) = self.nearest_basic(i, from, range, basic) {
            (&npc.entity, Self::basic_direction(member, npc.position, c)?)
        } else {
            let range = c
                .basic_pursuit
                .and_then(|pursuit| pursuit.auto_max_range)
                .ok_or(Error::InvalidTarget)?;
            let npc = self
                .nearest_basic(i, from, range, basic)
                .ok_or(Error::InvalidTarget)?;
            let target = npc.entity.clone();
            let position = npc.position;
            return self.begin_basic_pursuit(i, (&target, position), (range, None), id, c, now);
        };
        if let Some(definition) = c.basic_followup
            && now < member.basic_ready
        {
            if !member.basic_slot_free(now) {
                return Err(Error::InvalidState);
            }
            let promotes_at_ms = now + (member.basic_ready - now).div_ceil(c.step_ms) * c.step_ms;
            self.members[i].queued_basic = Some(BasicFollowupView {
                action_id: id.into(),
                target: Self::unit_wire(target),
                promotes_at_ms,
                expires_at_ms: promotes_at_ms + definition.grace_ms,
            });
            self.members[i].queued_basic_epoch = self.checkpoint_epoch;
            if self.members[i].can_replace_recovery_action(None, now) {
                self.members[i].end_recovery(now);
                self.members[i].queued_cast = None;
            }
            return Ok(());
        }
        let target = if member.active_basic_profile().is_some()
            || basic.piercing.is_some()
            || basic.non_projectile()
            || basic.projectile_followup.is_some()
        {
            Some(target.clone())
        } else {
            explicit
        };
        self.prepare(i, None, (direction, target), id, c, now)
    }

    pub(super) fn attack_ground(
        &mut self,
        i: usize,
        point: [f64; 2],
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        if self.members[i]
            .basic(c)
            .impact
            .as_ref()
            .and_then(|impact| impact.field())
            .is_none()
        {
            return Err(Error::Unsupported);
        }
        self.basic_ground_target(i, point, c)?;
        self.prepare(i, None, ([0., 0.], None), id, c, now)?;
        self.members[i].pending.as_mut().unwrap().ground = Some(point);
        Ok(())
    }

    pub(super) fn basic_ground_target(&self, i: usize, point: [f64; 2], c: &Config) -> Result<()> {
        self.check_ground_bounds(point)?;
        let member = &self.members[i];
        let basic = member.basic(c);
        // Normalizing a charged direction and projecting its range can round outwards.
        let tolerance = if basic.charged_flight().is_some() {
            1e-9
        } else {
            0.
        };
        if distance(member.position, point) > member.basic_range(c) + tolerance
            || self
                .grid
                .piercing_wall(member.position, point, 0.)
                .is_some()
        {
            return Err(Error::OutOfRange);
        }
        Ok(())
    }

    pub(super) fn basic_target_position(&self, i: usize, target: &EntityRef) -> Result<[f64; 2]> {
        self.hostile_unit_position(i, target)
    }

    fn basic_aim_from(&self, i: usize, to: [f64; 2], c: &Config, now: u64) -> Result<[f64; 2]> {
        let member = &self.members[i];
        if distance(member.position, to) > self.basic_lock_range(i, c, now)
            || !self.basic_sight(member.position, to, member.basic(c))
        {
            return Err(Error::InvalidTarget);
        }
        Self::basic_direction(member, to, c)
    }

    fn basic_sight(&self, from: [f64; 2], to: [f64; 2], basic: &BasicConfig) -> bool {
        if basic.projectile_followup.is_some() {
            true
        } else if basic.non_projectile() {
            self.grid.piercing_wall(from, to, 0.).is_none()
        } else {
            self.grid.line(from, to, 0., 2)
        }
    }

    pub(super) fn basic_target_aim(
        &self,
        i: usize,
        target: &EntityRef,
        c: &Config,
        now: u64,
    ) -> Result<[f64; 2]> {
        let position = self.basic_target_position(i, target)?;
        self.basic_aim_from(i, position, c, now)
    }

    pub(super) fn promote_basic(&mut self, i: usize, c: &Config, now: u64, events: &mut Events) {
        let member = &mut self.members[i];
        if member.basic_delivery.is_some() {
            return;
        }
        let Some(queued) = &member.queued_basic else {
            return;
        };
        let ready = member.basic_slot_free(now)
            && now >= member.basic_ready
            && now >= queued.promotes_at_ms;
        if !ready && now < queued.expires_at_ms {
            return;
        }
        let queued = member.queued_basic.take().expect("due basic followup");
        let checkpoint_epoch = member.queued_basic_epoch;
        let result = if ready && now <= queued.expires_at_ms {
            let target = self.hostile_unit_ref(&queued.target);
            self.basic_target_aim(i, &target, c, now)
                .and_then(|direction| {
                    self.prepare(
                        i,
                        None,
                        (direction, Some(target)),
                        &queued.action_id,
                        c,
                        now,
                    )?;
                    self.members[i].pending.as_mut().unwrap().checkpoint_epoch = checkpoint_epoch;
                    Ok(())
                })
        } else {
            Err(Error::InvalidState)
        };
        if let Err(reason) = result {
            private(
                events,
                i,
                Event::ActionRejected {
                    action_id: queued.action_id,
                    reason,
                },
            );
        }
    }
}
