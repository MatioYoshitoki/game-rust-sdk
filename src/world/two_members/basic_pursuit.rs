//! Pursuit owns position until range, interruption, or the exclusive deadline.
use super::*;

pub(super) struct BasicPursuit {
    pub(super) view: BasicPursuitView,
    max_range: f64,
    charged_stage: Option<usize>,
    pub(super) checkpoint_epoch: u32,
}

fn approach(from: [f64; 2], to: [f64; 2], range: f64, budget: f64) -> [f64; 2] {
    let delta = [to[0] - from[0], to[1] - from[1]];
    let gap = distance(from, to);
    // Step just inside the boundary to avoid a rounded distance greater than range.
    let step = budget.min((gap - range + 1e-9).min(gap));
    [
        from[0] + delta[0] / gap * step,
        from[1] + delta[1] / gap * step,
    ]
}

impl World {
    fn pursuit_sight(
        &self,
        from: [f64; 2],
        to: [f64; 2],
        max_range: f64,
        charged: bool,
    ) -> Result<()> {
        if distance(from, to) > max_range {
            return Err(Error::OutOfRange);
        }
        if if charged {
            self.grid.piercing_wall(from, to, 0.).is_some()
        } else {
            !self.grid.line(from, to, 0., 2)
        } {
            return Err(Error::InvalidTarget);
        }
        Ok(())
    }

    pub(super) fn begin_basic_pursuit(
        &mut self,
        i: usize,
        candidate: (&EntityRef, [f64; 2]),
        (max_range, charged_stage): (f64, Option<usize>),
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let definition = c.basic_pursuit.expect("pursuit enabled");
        let (target, position) = candidate;
        let member = &self.members[i];
        if !member.basic_slot_free(now)
            || member.basic_delivery.is_some()
            || now < member.basic_ready
            || member.queued_basic.is_some()
            || (c.mechanism_version >= 88 && self.member_has_status(i, StatusAttribute::Root, now))
        {
            return Err(Error::InvalidState);
        }
        let from = member.position;
        self.pursuit_sight(from, position, max_range, charged_stage.is_some())?;
        let range = charged_stage.map_or_else(
            || self.basic_lock_range(i, c, now),
            |stage| self.charged_wave_range(i, c, stage),
        );
        let goal = approach(from, position, range, f64::INFINITY);
        if !self.grid.line(from, goal, c.params.player_radius, 0) {
            return Err(Error::InvalidTarget);
        }
        if self.members[i].can_replace_recovery_action(None, now) {
            self.members[i].end_recovery(now);
            self.members[i].queued_cast = None;
        }
        self.members[i].basic_pursuit = Some(BasicPursuit {
            max_range,
            charged_stage,
            checkpoint_epoch: self.checkpoint_epoch,
            view: BasicPursuitView {
                action_id: id.into(),
                target: Self::unit_wire(target),
                started_at_ms: now,
                expires_at_ms: now + definition.duration_ms,
            },
        });
        Ok(())
    }

    pub(super) fn advance_basic_pursuit(
        &mut self,
        i: usize,
        c: &Config,
        now: u64,
        dt: f64,
        events: &mut Events,
    ) {
        let plan = self.members[i]
            .basic_pursuit
            .take()
            .expect("active pursuit");
        let result = (|| {
            if now >= plan.view.expires_at_ms {
                return Err(Error::InvalidState);
            }
            let target = self.hostile_unit_ref(&plan.view.target);
            let position = self.basic_target_position(i, &target)?;
            let member = &self.members[i];
            let from = member.position;
            let range = plan.charged_stage.map_or_else(
                || self.basic_lock_range(i, c, now),
                |stage| self.charged_wave_range(i, c, stage),
            );
            self.pursuit_sight(from, position, plan.max_range, plan.charged_stage.is_some())?;
            if distance(from, position) > range {
                let budget = self.member_movement_budget(i, c, now, dt);
                let end = approach(from, position, range, budget);
                if !self.grid.line(from, end, c.params.player_radius, 0) {
                    return Err(Error::InvalidTarget);
                }
                self.members[i].position = end;
                if distance(end, position) > range {
                    return Ok(false);
                }
            }
            let direction = match plan.charged_stage {
                Some(stage) => self.charged_wave_aim(i, &target, stage, c)?,
                None => self.basic_target_aim(i, &target, c, now)?,
            };
            self.prepare(
                i,
                None,
                (direction, Some(target)),
                &plan.view.action_id,
                c,
                now,
            )?;
            let pending = self.members[i].pending.as_mut().expect("pursuit windup");
            pending.pursuit_windup = true;
            pending.checkpoint_epoch = plan.checkpoint_epoch;
            pending.charged_basic = plan.charged_stage.map(ChargedBasic::Wave);
            Ok(true)
        })();
        match result {
            Ok(false) => self.members[i].basic_pursuit = Some(plan),
            Ok(true) => {}
            Err(reason) => private(
                events,
                i,
                Event::ActionRejected {
                    action_id: plan.view.action_id,
                    reason,
                },
            ),
        }
    }
}
