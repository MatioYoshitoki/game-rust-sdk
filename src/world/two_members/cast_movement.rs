//! New intent is compared with the phase's frozen anchor, so accumulated noise
//! cannot bypass interruption. Stop and invalid input never interrupt a cast.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct MovementAnchor {
    direction: [f64; 2],
    destination: Option<[f64; 2]>,
    origin: [f64; 2],
}

#[derive(Clone, Copy)]
pub(super) enum MoveRequest {
    Direction([f64; 2]),
    Destination([f64; 2]),
}

impl MovementAnchor {
    pub(super) fn capture(member: &Member) -> Self {
        let destination = member.path.last().copied();
        Self {
            origin: member.position,
            direction: destination.map_or(member.movement, |d| {
                normalize([d[0] - member.position[0], d[1] - member.position[1]])
            }),
            destination,
        }
    }

    fn changed(self, request: MoveRequest, policy: PhaseMovement) -> bool {
        let PhaseMovement::InterruptOnNewMove {
            min_turn_degrees,
            min_destination_distance,
        } = policy
        else {
            return false;
        };
        match request {
            MoveRequest::Destination(destination) => {
                distance(self.destination.unwrap_or(self.origin), destination)
                    >= min_destination_distance
            }
            MoveRequest::Direction(direction) => {
                if direction == [0., 0.] {
                    return false;
                }
                if self.direction == [0., 0.] {
                    return true;
                }
                let [x, y] = self.direction;
                let [dx, dy] = direction;
                let angle = (x * dy - y * dx).abs().atan2(x * dx + y * dy).to_degrees();
                angle + 1e-9 >= f64::from(min_turn_degrees)
            }
        }
    }
}

impl Member {
    pub(super) fn stop_movement(&mut self) {
        self.path.clear();
        self.movement = [0., 0.];
        let neutral = MovementAnchor::capture(self);
        if let Some(pending) = self.pending.as_mut() {
            pending.movement_anchor = neutral;
        }
        if let Some(channel) = self.channel.as_mut() {
            channel.movement_anchor = neutral;
        }
    }
}

impl World {
    pub(super) fn interrupt_for_movement(
        &mut self,
        i: usize,
        request: MoveRequest,
        now: u64,
        events: &mut Events,
    ) {
        let member = &self.members[i];
        let anchor = member
            .pending
            .as_ref()
            .map(|p| p.movement_anchor)
            .or_else(|| member.channel.as_ref().map(|p| p.movement_anchor));
        let was_recovering = anchor.is_none();
        if anchor.is_some_and(|anchor| anchor.changed(request, member.phase_movement(now))) {
            if self.members[i].pending.take().is_some() {
                private(events, i, Event::ActionInterrupted);
            }
            self.finish_channel(i, ChannelEndReason::Interrupted, now, events);
        }
        if (matches!(request, MoveRequest::Destination(_))
            || matches!(request,MoveRequest::Direction(d) if d != [0.,0.]))
            && was_recovering
            && self.members[i].can_cancel_recovery_by_move(now)
        {
            self.members[i].end_recovery(now);
            self.members[i].queued_cast = None;
            private(events, i, Event::ActionInterrupted);
        }
    }
}
