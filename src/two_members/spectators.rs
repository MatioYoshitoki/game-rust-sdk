use super::*;
use crate::config::two_members::MAX_SPECTATOR_EVENTS;
use crate::replication::changed;

pub(super) struct SpectatorSlot {
    connection: Option<Connection>,
    principal: Option<String>,
    epoch: u32,
    revision: u64,
    stream: replication::Stream<SpectatorSnapshotV6>,
    publications: VecDeque<Publication>,
    queued: Option<Output>,
    last_publish: u64,
}

impl SpectatorSlot {
    pub(super) fn new() -> Self {
        Self {
            connection: None,
            principal: None,
            epoch: 0,
            revision: 0,
            stream: replication::Stream::new(),
            publications: VecDeque::new(),
            queued: None,
            last_publish: 0,
        }
    }

    fn detach(&mut self) {
        self.principal = None;
        self.stream = replication::Stream::new();
        self.publications.clear();
        self.queued = None;
    }
}

fn diff(a: &SpectatorSnapshotV6, b: &SpectatorSnapshotV6) -> SpectatorDeltaV6 {
    SpectatorDeltaV6 {
        contested_equipment: (a.contested_equipment != b.contested_equipment)
            .then(|| b.contested_equipment.clone())
            .flatten(),
        extraction_pressure: (a.extraction_pressure != b.extraction_pressure)
            .then(|| b.extraction_pressure.clone())
            .flatten(),
        encounter_progress: (a.encounter_progress != b.encounter_progress)
            .then(|| b.encounter_progress.clone())
            .flatten(),
        logical_ms: b.logical_ms,
        members: changed(&a.members, &b.members, |m| m.entity.clone()),
        monsters: changed(&a.monsters, &b.monsters, |m| m.entity.clone()),
        projectiles: changed(&a.projectiles, &b.projectiles, |p| p.id.clone()),
        resources: changed(&a.resources, &b.resources, |r| r.id.clone()),
        areas: changed(&a.areas, &b.areas, |r| r.id.clone()),
        events: (a.events != b.events).then(|| b.events.clone()),
        stage: (a.stage != b.stage).then(|| b.stage.clone()).flatten(),
    }
}

impl TwoMembers {
    pub(crate) fn spectate(&mut self, request: SpectateV6) -> Result<Status> {
        self.check_key(&request.key)?;
        if self.spectators.is_empty() {
            return Err(Error::Unsupported);
        }
        let i = usize::from(request.slot.checked_sub(1).ok_or(Error::InvalidMessage)?);
        if !valid_id(&request.connection.id)
            || request.connection.generation == 0
            || i >= self.spectators.len()
        {
            return Err(Error::InvalidMessage);
        }
        if !self.spectator_available() {
            return Err(if matches!(self.phase, Phase::Closed | Phase::Faulted) {
                Error::Closed
            } else {
                Error::NotReady
            });
        }
        let slot = &mut self.spectators[i];
        if let SpectatorActionV6::Attach {
            authenticated_principal,
        } = request.action
        {
            if !valid_id(&authenticated_principal) {
                return Err(Error::InvalidMessage);
            }
            if slot
                .principal
                .as_ref()
                .is_some_and(|p| p != &authenticated_principal)
            {
                return Err(Error::Unauthorized);
            }
            if slot.connection.as_ref() == Some(&request.connection) && slot.principal.is_some() {
                return Ok(self.status());
            }
            let generation = slot.connection.as_ref().map_or(0, |c| c.generation);
            if generation.checked_add(1) != Some(request.connection.generation) {
                return Err(Error::StaleConnection);
            }
            if self.spectators.iter().enumerate().any(|(j, other)| {
                j != i
                    && other.principal.is_some()
                    && other
                        .connection
                        .as_ref()
                        .is_some_and(|c| c.id == request.connection.id)
            }) {
                return Err(Error::Conflict);
            }
            self.reset_spectator(
                i,
                if generation == 0 {
                    FullReason::Initial
                } else {
                    FullReason::Lifecycle
                },
            )?;
            let slot = &mut self.spectators[i];
            slot.connection = Some(request.connection);
            slot.principal = Some(authenticated_principal);
            slot.publications.clear();
            self.publish_spectator(i);
            return Ok(self.status());
        }
        if slot.connection.as_ref() != Some(&request.connection) {
            return Err(Error::StaleConnection);
        }
        if matches!(request.action, SpectatorActionV6::Detach {}) {
            slot.detach();
            self.discard_spectator_undelivered(i);
            return Ok(self.status());
        }
        if slot.principal.is_none() {
            return Err(Error::StaleConnection);
        }
        match request.action {
            SpectatorActionV6::RequestFull { stream_epoch } => {
                if stream_epoch != slot.epoch {
                    return Err(Error::StalePublication);
                }
                self.reset_spectator(i, FullReason::Resync)?;
            }
            SpectatorActionV6::Transport {
                publication_id,
                outcome,
            } => {
                if !valid_id(&publication_id) {
                    return Err(Error::InvalidMessage);
                }
                if !slot.stream.confirm(
                    &mut slot.publications,
                    slot.epoch,
                    &publication_id,
                    outcome,
                )? {
                    return Ok(self.status());
                }
                if outcome == TransportOutcome::Sent {
                    slot.stream
                        .accept_sent(self.io_elapsed, |s| s.phase == Phase::Closed);
                } else {
                    self.reset_spectator(i, FullReason::Failure)?;
                }
            }
            _ => unreachable!("handled spectator lifecycle"),
        }
        self.publish_spectator(i);
        Ok(self.status())
    }

    fn spectator_available(&self) -> bool {
        self.world.as_ref().is_some_and(|w| w.active_at.is_some())
            || self.spectator_final.is_some()
                && self.closed_at_io.is_some_and(|at| {
                    self.io_elapsed.saturating_sub(at)
                        < self.config.spectators().unwrap().terminal_retention_ms
                })
    }

    fn discard_spectator_undelivered(&mut self, i: usize) {
        self.undelivered.retain(|output| !matches!(output.body, OutputBody::SpectatorPublicationV6 { slot, .. } if usize::from(slot) == i + 1));
    }

    fn reset_spectator(&mut self, i: usize, reason: FullReason) -> Result<()> {
        let slot = &mut self.spectators[i];
        slot.epoch = slot.epoch.checked_add(1).ok_or(Error::Faulted)?;
        slot.queued = None;
        slot.stream.reset(reason);
        self.discard_spectator_undelivered(i);
        Ok(())
    }

    pub(super) fn record_spectator_event(&mut self, event: &EventV4) {
        if self.spectators.is_empty() || self.phase != Phase::Active {
            return;
        }
        let event = match event {
            EventV4::ExtractionChannelChanged(change) => {
                SpectatorEventV6::ExtractionChannelChanged(change.clone())
            }
            EventV4::Damage {
                source,
                target,
                amount,
                hp,
            } => SpectatorEventV6::Damage {
                source: source.clone(),
                target: target.clone(),
                amount: *amount,
                hp: *hp,
            },
            EventV4::MemberChanged { entity, phase } => SpectatorEventV6::MemberChanged {
                entity: entity.clone(),
                phase: *phase,
            },
            EventV4::ModeRespawned { entity } => SpectatorEventV6::Respawned {
                entity: entity.clone(),
            },
            _ => return,
        };
        if self.spectator_facts.len() == MAX_SPECTATOR_EVENTS {
            self.spectator_facts.pop_front();
        }
        self.spectator_event_sequence += 1;
        self.spectator_facts.push_back(SpectatorFactV6 {
            sequence: self.spectator_event_sequence,
            logical_ms: self.now,
            event,
        });
    }

    fn live_spectator_snapshot(&self) -> Option<SpectatorSnapshotV6> {
        let mut view =
            self.world
                .as_ref()?
                .spectator_snapshot(&self.config, self.phase, self.now)?;
        view.events = self.spectator_facts.iter().cloned().collect();
        Some(view)
    }

    pub(super) fn finish_spectators(&mut self) {
        if self.spectators.is_empty() {
            return;
        }
        self.spectator_final = self.live_spectator_snapshot();
        self.spectator_facts.clear();
        self.publish_spectators_due(0);
    }

    fn publish_spectator(&mut self, i: usize) {
        if let Some(snapshot) = self
            .spectator_final
            .clone()
            .or_else(|| self.live_spectator_snapshot())
        {
            self.queue_spectator(i, &snapshot);
        }
    }

    pub(super) fn publish_spectators_due(&mut self, interval: u64) {
        let due = |slot: &SpectatorSlot| {
            slot.principal.is_some() && self.now >= slot.last_publish + interval
        };
        if !self.spectators.iter().any(due) {
            return;
        }
        if let Some(snapshot) = self
            .spectator_final
            .clone()
            .or_else(|| self.live_spectator_snapshot())
        {
            for i in 0..self.spectators.len() {
                if self.spectators[i].principal.is_some()
                    && self.now >= self.spectators[i].last_publish + interval
                {
                    self.queue_spectator(i, &snapshot);
                }
            }
        }
    }

    fn queue_spectator(&mut self, i: usize, snapshot: &SpectatorSnapshotV6) {
        let slot = &mut self.spectators[i];
        if slot.stream.terminal_sent() {
            return;
        }
        slot.last_publish = self.now;
        let Some(payload) = slot.stream.payload(
            snapshot,
            snapshot.phase == Phase::Closed,
            self.io_elapsed,
            self.config.replication.as_ref().unwrap(),
            diff,
        ) else {
            return;
        };
        slot.revision += 1;
        let revision = slot.revision;
        let connection = slot.connection.as_ref().unwrap().clone();
        let stream_epoch = slot.epoch;
        let slot_id = i as u8 + 1;
        let full = matches!(payload, SpectatorPayloadV6::Full { .. });
        let output = Output {
            key: self.create.key.clone(),
            id: format!(
                "{}:{}:r{}:spectator:{slot_id}:{revision:020}",
                self.create.key.id, self.create.key.incarnation, self.generation
            ),
            logical_ms: self.now,
            body: OutputBody::SpectatorPublicationV6 {
                slot: slot_id,
                connection,
                stream_epoch,
                revision,
                payload: Box::new(payload),
            },
            recovery: None,
        };
        let slot = &mut self.spectators[i];
        slot.stream.issue(&output, revision, snapshot.clone(), full);
        slot.queued = Some(output);
    }

    pub(super) fn poll_spectators(&mut self) {
        if self.spectator_final.is_some() && !self.spectator_available() {
            self.spectator_final = None;
            for i in 0..self.spectators.len() {
                self.spectators[i].detach();
                self.discard_spectator_undelivered(i);
            }
            return;
        }
        for i in 0..self.spectators.len() {
            let slot = &self.spectators[i];
            if slot.principal.is_none() || slot.stream.terminal_sent() {
                continue;
            }
            if let Some(reason) = slot
                .stream
                .reset_reason(self.io_elapsed, self.config.replication.as_ref().unwrap())
            {
                if self.reset_spectator(i, reason).is_ok() {
                    self.publish_spectator(i);
                } else {
                    self.spectators[i].detach();
                }
            }
        }
    }

    pub(super) fn drain_spectators(&mut self, outputs: &mut Vec<Output>) {
        for slot in &mut self.spectators {
            if let Some(output) = slot.queued.take() {
                slot.stream.mark_drained(&output.id, self.io_elapsed);
                let OutputBody::SpectatorPublicationV6 { payload, .. } = &output.body else {
                    unreachable!()
                };
                slot.publications.push_back(Publication {
                    id: output.id.clone(),
                    epoch: slot.epoch,
                    connection: slot.connection.as_ref().unwrap().clone(),
                    outcome: TransportOutcome::Enqueued,
                    full: matches!(payload.as_ref(), SpectatorPayloadV6::Full { .. }),
                });
                if slot.publications.len() > self.config.history_capacity {
                    slot.publications.pop_front();
                }
                outputs.push(output);
            }
        }
    }
}
