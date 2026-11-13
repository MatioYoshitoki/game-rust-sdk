//! A held basic attack owns the preparation slot, but keeps the latest movement intent.
use super::triggers::ActionKey;
use super::*;

pub(super) struct BasicBeam {
    view: BasicBeamView,
    end: Option<BasicBeamEndReason>,
}

impl BasicBeam {
    pub(super) fn view(&self) -> Option<&BasicBeamView> {
        self.end.is_none().then_some(&self.view)
    }

    pub(super) fn action(&self) -> ActionKey {
        ActionKey::new(&self.view.source, &self.view.action_id)
    }
}

fn rectangle_contact(
    origin: [f64; 2],
    direction: [f64; 2],
    range: f64,
    half_width: f64,
    center: [f64; 2],
    radius: f64,
) -> Option<f64> {
    let [dx, dy] = [center[0] - origin[0], center[1] - origin[1]];
    let [x, y] = direction;
    let along = dx * x + dy * y;
    let across = dx * -y + dy * x;
    let nearest = along.clamp(0., range);
    let offset = (across.abs() - half_width).max(0.);
    ((along - nearest).hypot(offset) <= radius).then_some(nearest)
}

impl World {
    pub(super) fn start_basic_beam(
        &mut self,
        i: usize,
        direction: [f64; 2],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let member = &self.members[i];
        let basic = member.basic(c);
        let beam = basic.beam.as_ref().ok_or(Error::Unsupported)?;
        if !member.basic_slot_free(now)
            || member.basic_delivery.is_some()
            || member.queued_basic.is_some()
            || member.queued_cast.is_some()
            || now < member.basic_ready
        {
            return Err(Error::InvalidState);
        }
        let direction = validated_direction(direction)?;
        let release_status = basic
            .effects
            .as_ref()
            .and_then(|effects| effects.on_release_self_status.as_ref());
        if let Some(status) = release_status
            && !self.can_statuses(i, c, std::slice::from_ref(status))
        {
            return Err(Error::Capacity);
        }
        let source = member.reference.entity.clone();
        if member.can_replace_recovery_action(None, now) {
            self.members[i].end_recovery(now);
        }
        let view = BasicBeamView {
            action_id: self.next_id("basic-beam"),
            plan_id: beam.plan_id.clone(),
            source,
            direction,
            range: basic.range,
            half_width: beam.half_width,
            interval_ms: beam.interval_ms,
            started_at_ms: now,
            next_tick_at_ms: now + beam.interval_ms,
            ticks: 0,
            released: false,
        };
        self.start_basic_delivery(i, &view.action_id, now, None);
        self.members[i].basic_beam = Some(BasicBeam {
            view: view.clone(),
            end: None,
        });
        private(
            events,
            i,
            Event::ActionCommitted {
                action_id: view.action_id.clone(),
                skill: false,
                spirit_cost: 0,
                hp_cost: None,
                charge: None,
                skill_id: None,
                unit_target: None,
                ground_position: None,
            },
        );
        events.push((None, EventV4::BasicBeamStarted { beam: view }));
        self.apply_basic_self_status(i, release_status, c, now, events);
        Ok(())
    }

    fn held_basic_beam(&mut self, i: usize, action_id: &str) -> Result<&mut BasicBeam> {
        self.members[i]
            .basic_beam
            .as_mut()
            .filter(|beam| {
                beam.view.action_id == action_id && !beam.view.released && beam.end.is_none()
            })
            .ok_or(Error::InvalidTarget)
    }

    pub(super) fn aim_basic_beam(
        &mut self,
        i: usize,
        action_id: &str,
        direction: [f64; 2],
    ) -> Result<()> {
        self.held_basic_beam(i, action_id)?.view.direction = validated_direction(direction)?;
        Ok(())
    }

    pub(super) fn release_basic_beam(&mut self, i: usize, action_id: &str) -> Result<()> {
        let beam = self.held_basic_beam(i, action_id)?;
        if beam.view.ticks == 0 {
            beam.view.released = true;
        } else {
            beam.end = Some(BasicBeamEndReason::Released);
        }
        Ok(())
    }

    pub(super) fn schedule_basic_beam_end(&mut self, i: usize, reason: BasicBeamEndReason) {
        if let Some(beam) = &mut self.members[i].basic_beam
            && beam.end != Some(BasicBeamEndReason::Protected)
        {
            beam.end = Some(reason);
        }
    }

    pub(super) fn end_basic_beam(
        &mut self,
        i: usize,
        reason: BasicBeamEndReason,
        events: &mut Events,
    ) {
        if let Some(beam) = self.members[i].basic_beam.take() {
            events.push((
                None,
                EventV4::BasicBeamEnded {
                    action_id: beam.view.action_id,
                    source: beam.view.source,
                    reason,
                    ticks: beam.view.ticks,
                },
            ));
        }
    }

    pub(super) fn settle_pending_basic_beams(&mut self, events: &mut Events) {
        for i in 0..self.members.len() {
            if let Some(reason) = self.members[i]
                .basic_beam
                .as_ref()
                .and_then(|beam| beam.end)
            {
                self.end_basic_beam(i, reason, events);
            }
        }
    }

    pub(super) fn advance_basic_beams(&mut self, c: &Config, now: u64, events: &mut Events) {
        for i in 0..self.members.len() {
            let member = &mut self.members[i];
            if !member.eligible() {
                continue;
            }
            let Some(beam) = &mut member.basic_beam else {
                continue;
            };
            if beam.end.is_some() || now < beam.view.next_tick_at_ms {
                continue;
            }
            beam.view.ticks += 1;
            beam.view.next_tick_at_ms = now + beam.view.interval_ms;
            let view = beam.view.clone();
            self.pulse_basic_beam(i, &view, c, now, events);
            if view.released
                && self.members[i]
                    .basic_beam
                    .as_ref()
                    .is_some_and(|beam| beam.end.is_none())
            {
                self.end_basic_beam(i, BasicBeamEndReason::Released, events);
            }
        }
    }

    fn pulse_basic_beam(
        &mut self,
        i: usize,
        beam: &BasicBeamView,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let origin = self.members[i].position;
        let config = self.members[i]
            .basic(c)
            .beam
            .as_ref()
            .expect("active beam profile");
        let rule = &c.combat_rules.as_ref().unwrap().basic_attack;
        let raw = rule.budget(self.stats(i, c, now).0, config.damage_bps, 10000, 0);
        let mut targets = self
            .npcs
            .iter()
            .filter_map(|npc| {
                let along = rectangle_contact(
                    origin,
                    beam.direction,
                    beam.range,
                    beam.half_width,
                    npc.position,
                    npc.profile.combat.radius,
                )?;
                (self.hostile_contact(&beam.source, &npc.entity)
                    && self.grid.piercing_wall(origin, npc.position, 0.).is_none())
                .then_some((along, &npc.entity))
            })
            .collect::<Vec<_>>();
        targets.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        let targets = targets
            .into_iter()
            .take(config.target_cap)
            .map(|(_, e)| e.clone())
            .collect::<Vec<_>>();

        events.push((
            None,
            EventV4::BasicBeamPulse {
                action_id: beam.action_id.clone(),
                source: beam.source.clone(),
                origin,
                direction: beam.direction,
                tick: beam.ticks,
                at_ms: now,
            },
        ));
        for target in targets {
            if !self.members[i].eligible()
                || self.members[i]
                    .basic_beam
                    .as_ref()
                    .is_none_or(|beam| beam.end.is_some())
            {
                break;
            }
            if !self.hostile_contact(&beam.source, &target) {
                continue;
            }
            let owner = self.resolve_entity(&target).expect("eligible target");
            events.push((
                None,
                EventV4::BasicBeamContact {
                    action_id: beam.action_id.clone(),
                    source: beam.source.clone(),
                    target,
                    tick: beam.ticks,
                    at_ms: now,
                },
            ));
            self.resolve_native_basic_contact(
                i,
                owner,
                (&beam.source, &beam.action_id, raw),
                c,
                now,
                events,
            );
        }
    }
}
