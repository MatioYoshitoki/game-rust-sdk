//! Per-recipient stop-and-wait replication; never promotes an unsent publication.
use super::*;
use serde::Serialize;
#[derive(Clone)]
struct Baseline<T> {
    id: String,
    revision: u64,
    snapshot: T,
    sent_at: u64,
}
#[derive(Clone)]
struct Flight<T> {
    id: String,
    revision: u64,
    snapshot: T,
    full: bool,
    drained: bool,
    drained_at: u64,
}
pub(super) struct Stream<T = SnapshotV4> {
    base: Option<Baseline<T>>,
    flight: Option<Flight<T>>,
    force: Option<FullReason>,
    full_sent_at: Option<u64>,
    frozen: Option<T>,
    terminal_sent: bool,
    terminal_started: Option<u64>,
    ready_full: Option<String>,
    notice_pending: Option<Option<String>>,
    pub expired: bool,
}
impl<T> Stream<T> {
    pub fn new() -> Self {
        Self {
            base: None,
            flight: None,
            force: Some(FullReason::Initial),
            full_sent_at: None,
            frozen: None,
            terminal_sent: false,
            terminal_started: None,
            ready_full: None,
            notice_pending: None,
            expired: false,
        }
    }
    pub fn reset(&mut self, reason: FullReason) {
        self.base = None;
        self.flight = None;
        self.force = Some(reason);
        self.full_sent_at = None;
        self.terminal_sent = false;
        self.ready_full = None;
    }
    pub fn ready(&self, id: &str) -> bool {
        self.ready_full.as_deref() == Some(id)
    }
    pub fn mark_drained(&mut self, id: &str, now: u64) {
        if let Some(f) = &mut self.flight
            && f.id == id
        {
            f.drained = true;
            f.drained_at = now;
            self.force = None;
        }
    }

    pub(super) fn issue(&mut self, output: &Output, revision: u64, snapshot: T, full: bool) {
        self.flight = Some(Flight {
            id: output.id.clone(),
            revision,
            snapshot,
            full,
            drained: false,
            drained_at: 0,
        });
    }

    pub(super) fn accept_sent(&mut self, now: u64, is_terminal: impl FnOnce(&T) -> bool) {
        let flight = self.flight.take().expect("validated sent publication");
        if flight.full {
            self.ready_full = Some(flight.id.clone());
            self.full_sent_at = Some(now);
            self.terminal_sent |= is_terminal(&flight.snapshot);
        }
        self.base = Some(Baseline {
            id: flight.id,
            revision: flight.revision,
            snapshot: flight.snapshot,
            sent_at: now,
        });
    }

    pub(super) fn terminal_sent(&self) -> bool {
        self.terminal_sent && self.force.is_none()
    }

    pub(super) fn reset_reason(
        &self,
        now: u64,
        cfg: &crate::config::two_members::ReplicationConfig,
    ) -> Option<FullReason> {
        if self
            .flight
            .as_ref()
            .is_some_and(|f| f.drained && now.saturating_sub(f.drained_at) >= cfg.ack_timeout_ms)
        {
            Some(FullReason::Timeout)
        } else if self
            .base
            .as_ref()
            .is_some_and(|b| now.saturating_sub(b.sent_at) >= cfg.baseline_ttl_ms)
        {
            Some(FullReason::Expired)
        } else {
            None
        }
    }

    pub(super) fn confirm(
        &self,
        publications: &mut VecDeque<Publication>,
        epoch: u32,
        id: &str,
        outcome: TransportOutcome,
    ) -> Result<bool> {
        let publication = publications
            .iter_mut()
            .find(|p| p.id == id && p.epoch == epoch)
            .ok_or(Error::StalePublication)?;
        if publication.outcome == outcome || outcome == TransportOutcome::Enqueued {
            return Ok(false);
        }
        if publication.outcome != TransportOutcome::Enqueued {
            return Err(Error::Conflict);
        }
        if !self
            .flight
            .as_ref()
            .is_some_and(|f| f.id == id && f.drained)
        {
            return Err(Error::StalePublication);
        }
        publication.outcome = outcome;
        Ok(true)
    }
}

impl<T: Clone + PartialEq + Serialize> Stream<T> {
    pub(super) fn payload<D: Serialize>(
        &self,
        snapshot: &T,
        terminal: bool,
        now: u64,
        cfg: &crate::config::two_members::ReplicationConfig,
        diff: impl FnOnce(&T, &T) -> D,
    ) -> Option<ReplicationPayloadV6<T, D>> {
        if self.flight.as_ref().is_some_and(|f| f.drained) {
            return None;
        }
        let periodic = self
            .full_sent_at
            .is_some_and(|at| now.saturating_sub(at) >= cfg.full_interval_ms);
        let forced = self.force;
        if forced.is_none()
            && !periodic
            && self.base.as_ref().is_some_and(|b| &b.snapshot == snapshot)
        {
            return None;
        }
        if forced.is_none()
            && !periodic
            && !terminal
            && let Some(base) = &self.base
        {
            let delta = ReplicationPayloadV6::Delta {
                base_publication_id: base.id.clone(),
                base_revision: base.revision,
                delta: Box::new(diff(&base.snapshot, snapshot)),
            };
            let full = ReplicationPayloadV6::Full {
                snapshot: Box::new(snapshot.clone()),
                reason: FullReason::Size,
            };
            return Some(
                if serde_json::to_vec(&delta).unwrap().len()
                    < serde_json::to_vec(&full).unwrap().len()
                {
                    delta
                } else {
                    full
                },
            );
        }
        let reason = forced.unwrap_or(if terminal {
            FullReason::Terminal
        } else if periodic {
            FullReason::Periodic
        } else {
            FullReason::Initial
        });
        Some(ReplicationPayloadV6::Full {
            snapshot: Box::new(snapshot.clone()),
            reason,
        })
    }
}
impl TwoMembers {
    pub(super) fn reset_stream(&mut self, i: usize, reason: FullReason) -> Result<()> {
        let epoch = self.seats[i].epoch.checked_add(1).ok_or(Error::Faulted)?;
        self.seats[i].epoch = epoch;
        self.seats[i].queue.clear();
        self.seats[i].stream.as_mut().unwrap().reset(reason);
        Ok(())
    }
    pub(super) fn publish_v6(&mut self, i: usize) {
        if self.seats[i].lost || self.seats[i].stream.as_ref().unwrap().expired {
            return;
        }
        let stream = self.seats[i].stream.as_ref().unwrap();
        if stream.terminal_sent && stream.force.is_none() {
            return;
        }
        let mut snapshot = if let Some(frozen) = &stream.frozen {
            let mut s = frozen.clone();
            s.phase = self.phase;
            s.logical_ms = self.now;
            if self.seats[i].ledger.ledger_settled() {
                s.own.phase = MemberPhase::Terminal;
                if let Some(own) = s
                    .members
                    .iter_mut()
                    .find(|m| m.entity == s.own.participant.entity)
                {
                    own.phase = MemberPhase::Terminal;
                }
            }
            s
        } else if let Some(w) = &self.world {
            if !w.members[i].joined {
                return;
            }
            w.snapshot(i, &self.config, self.phase, self.now)
        } else if let Some(mut s) = self.seats[i].last_view.clone() {
            s.phase = self.phase;
            s.logical_ms = self.now;
            if self.seats[i].ledger.ledger_settled() {
                s.own.phase = MemberPhase::Terminal;
            }
            s
        } else {
            return;
        };
        if let Some(mode) = &mut snapshot.own.mode
            && mode.outcome.is_none()
        {
            mode.forfeit_at_ms = self.seats[i].mode_forfeit_at;
        }
        self.seats[i].last_view = Some(snapshot.clone());
        self.seats[i].last_publish = self.now;
        let s = self.seats[i].stream.as_mut().unwrap();
        if matches!(
            snapshot.own.phase,
            MemberPhase::Settling | MemberPhase::Terminal
        ) {
            s.terminal_started.get_or_insert(self.io_elapsed);
        }
        if matches!(
            snapshot.own.phase,
            MemberPhase::Settling | MemberPhase::Terminal
        ) && s.frozen.is_none()
        {
            s.frozen = Some(snapshot.clone());
        }
        let terminal = matches!(
            snapshot.own.phase,
            MemberPhase::Settling | MemberPhase::Terminal
        ) || matches!(snapshot.phase, Phase::Closed | Phase::Faulted);
        let Some(payload) = s.payload(
            &snapshot,
            terminal,
            self.io_elapsed,
            self.config.replication.as_ref().unwrap(),
            crate::replication::diff,
        ) else {
            return;
        };
        let full = matches!(payload, ReplicationPayloadV6::Full { .. });
        let revision = self.output_sequence + 1;
        let output = self.output(OutputBody::PublicationV6 {
            recipient_id: self.seats[i].original.participant_id.clone(),
            connection: self.seats[i].connection.clone(),
            stream_epoch: self.seats[i].epoch,
            revision,
            payload: Box::new(payload.into()),
        });
        let seat = &mut self.seats[i];
        seat.last_full = full.then(|| output.id.clone());
        seat.queue.clear();
        seat.queue.push_back(output.clone());
        seat.stream
            .as_mut()
            .unwrap()
            .issue(&output, revision, snapshot, full);
    }
    pub(super) fn transport_v6(&mut self, r: TransportReceipt, i: usize) -> Result<Status> {
        if self.seats[i].stream.as_ref().unwrap().expired {
            return Err(Error::Closed);
        }
        let seat = &mut self.seats[i];
        let stream = seat.stream.as_mut().unwrap();
        if !stream.confirm(&mut seat.publications, seat.epoch, &r.output_id, r.outcome)? {
            return Ok(self.status());
        }
        if r.outcome == TransportOutcome::Sent {
            stream.accept_sent(self.io_elapsed, |s| s.own.phase == MemberPhase::Terminal);
        } else {
            self.reset_stream(i, FullReason::Failure)?;
        }
        self.publish(i);
        Ok(self.status())
    }
    pub(super) fn flush_publication_notices(&mut self) {
        for i in 0..self.seats.len() {
            if self.outputs.len() >= self.config.output_capacity {
                return;
            }
            if let Some(publication_id) =
                self.seats[i].stream.as_mut().unwrap().notice_pending.take()
            {
                self.events(vec![(
                    Some(i),
                    EventV4::PublicationExpired {
                        stream_epoch: self.seats[i].epoch,
                        publication_id,
                    },
                )]);
            }
        }
    }
    pub(super) fn poll_publications(&mut self) {
        let cfg = self.config.replication.as_ref().unwrap().clone();
        for i in 0..self.seats.len() {
            if self.seats[i].stream.as_ref().unwrap().expired {
                continue;
            }
            let terminal_at = self.seats[i].stream.as_ref().unwrap().terminal_started;
            let closed_at = match (self.closed_at_io, terminal_at) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            if closed_at
                .is_some_and(|at| self.io_elapsed.saturating_sub(at) >= cfg.terminal_grace_ms)
            {
                let has_view = self.seats[i].last_view.is_some();
                let stream = self.seats[i].stream.as_mut().unwrap();
                let abandoned = !stream.terminal_sent && has_view;
                let publication_id = stream.flight.as_ref().map(|f| f.id.clone());
                stream.expired = true;
                stream.base = None;
                stream.flight = None;
                stream.frozen = None;
                stream.ready_full = None;
                self.seats[i].queue.clear();
                self.seats[i].last_view = None;
                self.seats[i].publications.clear();
                if abandoned {
                    self.seats[i].stream.as_mut().unwrap().notice_pending = Some(publication_id);
                }
                self.flush_publication_notices();
                continue;
            }
            let stream = self.seats[i].stream.as_ref().unwrap();
            let reason = stream.reset_reason(self.io_elapsed, &cfg);
            if let Some(reason) = reason {
                if self.reset_stream(i, reason).is_err() {
                    self.fault(Error::Faulted);
                    return;
                }
                self.publish(i);
            }
        }
    }
}

#[cfg(all(test, match_sdk_test_data))]
impl TwoMembers {
    pub(crate) fn target_v6(&self, i: usize, id: &str) -> Option<SnapshotV4> {
        self.seats[i]
            .stream
            .as_ref()?
            .flight
            .as_ref()
            .filter(|f| f.id == id)
            .map(|f| f.snapshot.clone())
    }
    pub(crate) fn stream_cache_counts(&self, i: usize) -> (usize, usize) {
        let s = self.seats[i].stream.as_ref().unwrap();
        (
            usize::from(s.base.is_some()),
            usize::from(s.flight.is_some()),
        )
    }
}
