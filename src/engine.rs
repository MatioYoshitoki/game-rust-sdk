use std::collections::{BTreeMap, VecDeque};
mod ledger;
mod recovery;

use crate::{config::Config, protocol::*, world::World};

#[derive(Clone)]
struct Operation {
    request: BusinessRequest,
    intent: Business,
    terminal: Option<ReceiptOutcome>,
    refused: bool,
    deadline_ms: Option<u64>,
    queried_attempt: Option<u32>,
}

impl Business {
    fn durable_write_label(&self) -> Option<&'static str> {
        match self {
            Self::CommitAdmission { .. } => Some("admit"),
            Self::CommitSettlement { .. } => Some("settle"),
            Self::CompensateAdmission { .. } => Some("compensate"),
            Self::AppendFacts { .. } => Some("facts"),
            _ => None,
        }
    }
}

struct Publication {
    id: String,
    epoch: u32,
    outcome: TransportOutcome,
}

pub(crate) struct Engine {
    // Reuse the frozen financial state machine without ever constructing a World.
    // Only the protocol-4 shared-world coordinator can enable this mode.
    ledger_only: bool,
    pub(crate) config: Config,
    catalog: std::sync::Arc<crate::config::ConfigCatalog>,
    create: Option<Create>,
    connection: Option<Connection>,
    connection_lost: bool,
    resume_until: Option<u64>,
    pub(crate) world: Option<World>,
    facts: Option<ParticipantFacts>,
    phase: Phase,
    now: u64,
    io_ms: u64,
    drain_until: Option<u64>,
    was_active: bool,
    operations: BTreeMap<String, Operation>,
    outputs: VecDeque<Output>,
    output_sequence: u64,
    input_sequence: u32,
    recovery_generation: u32,
    recovery_fingerprint: Option<String>,
    history: VecDeque<(Input, Result<Status>)>,
    publications: VecDeque<Publication>,
    ready_epoch: u32,
    last_full: Option<String>,
    last_publish: u64,
    close_result: Option<Status>,
}

impl Engine {
    #[cfg(all(test, match_sdk_test_data))]
    pub fn new(config: Config) -> Self {
        Self::with_catalog(config, crate::config::default_catalog())
    }
    pub(crate) fn with_catalog(
        config: Config,
        catalog: std::sync::Arc<crate::config::ConfigCatalog>,
    ) -> Self {
        Self {
            ledger_only: false,
            catalog,
            config,
            create: None,
            connection: None,
            connection_lost: false,
            resume_until: None,
            world: None,
            facts: None,
            phase: Phase::Preparing,
            now: 0,
            io_ms: 0,
            drain_until: None,
            was_active: false,
            operations: BTreeMap::new(),
            outputs: VecDeque::new(),
            output_sequence: 0,
            input_sequence: 0,
            recovery_generation: 0,
            recovery_fingerprint: None,
            history: VecDeque::new(),
            publications: VecDeque::new(),
            ready_epoch: 1,
            last_full: None,
            last_publish: 0,
            close_result: None,
        }
    }

    pub fn status(&self) -> Status {
        Status {
            phase: self.phase,
            logical_ms: self.now,
        }
    }

    fn key(&self) -> MatchKey {
        self.create.as_ref().expect("created instance").key.clone()
    }

    fn check_key(&self, key: &MatchKey) -> Result<()> {
        if self.create.as_ref().is_none_or(|c| &c.key != key) {
            Err(Error::StaleMatch)
        } else {
            Ok(())
        }
    }

    fn reserve(&self) -> Result<()> {
        if self.outputs.len() + 16 > self.config.output_capacity {
            Err(Error::QueueFull)
        } else {
            Ok(())
        }
    }

    fn emit(&mut self, body: OutputBody) -> String {
        assert!(
            self.outputs.len() < self.config.output_capacity,
            "output reservation invariant"
        );
        self.output_sequence += 1;
        let key = self.key();
        let id = if self.recovery_generation == 0 {
            format!(
                "{}:{}:out:{}",
                key.id, key.incarnation, self.output_sequence
            )
        } else {
            format!(
                "{}:{}:r{}:out:{}",
                key.id, key.incarnation, self.recovery_generation, self.output_sequence
            )
        };
        let recovery = (!matches!(body, OutputBody::Publication { .. }))
            .then(|| Box::new(self.recovery_checkpoint()));
        self.outputs.push_back(Output {
            key,
            id: id.clone(),
            logical_ms: self.now,
            body,
            recovery,
        });
        id
    }

    fn event(&mut self, event: Event) {
        self.emit(OutputBody::Event { event });
    }

    fn operation_id(&self, suffix: &str) -> String {
        let key = self.key();
        format!("{}:{}:{suffix}", key.id, key.incarnation)
    }

    fn request(&mut self, suffix: &str, body: Business) {
        let id = self.operation_id(suffix);
        if self.operations.contains_key(&id) {
            return;
        }
        let request = BusinessRequest {
            operation_id: id.clone(),
            attempt_id: 1,
            body: self.config.encode_business(&body),
        };
        let queried_attempt = match &request.body {
            Business::QueryOperation { operation_id } => self
                .operations
                .get(operation_id)
                .map(|op| op.request.attempt_id),
            _ => None,
        };
        let deadline_ms = (!matches!(request.body, Business::ReconcileOperation { .. }))
            .then_some(self.io_ms + self.config.params.runtime.io_timeout_ms);
        self.operations.insert(
            id,
            Operation {
                request: request.clone(),
                intent: body,
                terminal: None,
                refused: false,
                deadline_ms,
                queried_attempt,
            },
        );
        self.emit(OutputBody::BusinessRequest { request });
    }

    pub fn create(&mut self, mut create: Create) -> Result<Status> {
        let recovery = create.recovery.take();
        if let Some(previous) = &self.create {
            return if previous == &create
                && (recovery.is_none()
                    || self.recovery_fingerprint == recovery.as_ref().map(recovery::fingerprint))
            {
                Ok(self.status())
            } else {
                Err(Error::Conflict)
            };
        }
        if create.config_id != self.config.id || create.config_hash != self.config.hash() {
            let config = self
                .catalog
                .find(&create.config_id, &create.config_hash)
                .ok_or(Error::ConfigMismatch)?;
            let mut candidate = Self::with_catalog(config.as_ref().clone(), self.catalog.clone());
            candidate.io_ms = self.io_ms;
            candidate.ledger_only = self.ledger_only;
            create.recovery = recovery;
            let status = candidate.create(create)?;
            *self = candidate;
            return Ok(status);
        }
        if create.protocol_version != self.config.protocol_version() {
            return Err(Error::VersionMismatch);
        }
        if create.config_id != self.config.id || create.config_hash != self.config.hash() {
            return Err(Error::ConfigMismatch);
        }
        if !valid_id(&create.key.id)
            || create.key.id.len() > 64
            || create.key.incarnation == 0
            || !valid_id(&create.participant_id)
            || !valid_id(&create.authenticated_principal)
            || !valid_id(&create.request_id)
            || !valid_id(&create.connection.id)
            || create.connection.generation == 0
        {
            return Err(Error::InvalidMessage);
        }
        if let Some(recovery) = recovery {
            return self.bootstrap_recovery(create, recovery);
        }
        self.connection = Some(create.connection.clone());
        self.create = Some(create);
        self.request("load", Business::LoadParticipant {});
        Ok(self.status())
    }

    fn publish(&mut self) {
        let Some(world) = &self.world else {
            return;
        };
        if !world.joined {
            return;
        }
        let snapshot = world.snapshot(&self.config, self.now);
        let create = self.create.as_ref().expect("created");
        let body = OutputBody::Publication {
            recipient_id: create.participant_id.clone(),
            connection: self.connection.as_ref().unwrap().clone(),
            snapshot,
        };
        let id = self.emit(body);
        self.last_full = Some(id.clone());
        self.last_publish = self.now;
        self.publications.push_back(Publication {
            id,
            epoch: self.ready_epoch,
            outcome: TransportOutcome::Enqueued,
        });
        while self.publications.len() > self.config.history_capacity {
            self.publications.pop_front();
        }
    }

    pub fn input(&mut self, input: Input) -> Result<Status> {
        self.check_key(&input.key)?;
        if matches!(self.phase, Phase::Closed | Phase::Faulted) {
            return Err(Error::Closed);
        }
        let create = self.create.as_ref().expect("created");
        if input.participant_id != create.participant_id {
            return Err(Error::Unauthorized);
        }
        if input.entity_generation != 1 {
            return Err(Error::InvalidState);
        }
        let reconnect = matches!(input.action, Action::Reconnect { .. });
        if !reconnect && Some(&input.connection) != self.connection.as_ref() {
            return Err(Error::StaleConnection);
        }
        if !valid_id(&input.request_id) || input.sequence == 0 {
            return Err(Error::InvalidMessage);
        }
        match &input.action {
            Action::StartCharge { .. }
            | Action::CastAtAnchor { .. }
            | Action::RecastAnchor { .. }
            | Action::ReturnAnchor { .. }
            | Action::EndAnchor { .. }
            | Action::ReleaseCharge { .. }
            | Action::UpdateChargeAim { .. }
            | Action::RecastProjectile { .. }
            | Action::AttackUnit { .. }
            | Action::AttackGround { .. }
            | Action::StartBasicCharge { .. }
            | Action::ReleaseBasicCharge { .. }
            | Action::CancelBasicCharge { .. }
            | Action::StartAttackBeam { .. }
            | Action::AimAttackBeam { .. }
            | Action::EndAttackBeam { .. }
            | Action::DropItem { .. }
            | Action::OpenRelic { .. }
            | Action::ClaimRelic { .. }
            | Action::ClaimBoon { .. }
            | Action::CounterCheckpoint { .. }
            | Action::UseItem { .. } => {
                return Err(Error::Unsupported);
            }
            Action::CastGround { skill_id, position }
                if !valid_id(skill_id)
                    || skill_id.len() > 64
                    || !position.iter().all(|v| v.is_finite()) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::CastUnit { skill_id, target }
                if !valid_id(skill_id)
                    || skill_id.len() > 64
                    || !valid_id(&target.id)
                    || target.generation == 0 =>
            {
                return Err(Error::InvalidMessage);
            }

            Action::CastSelf { skill_id } if !valid_id(skill_id) || skill_id.len() > 64 => {
                return Err(Error::InvalidMessage);
            }

            Action::AttuneObjective { objective_id, .. }
                if !valid_id(objective_id) || objective_id.len() > 64 =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::OpenChest {
                chest_id,
                expected_generation,
            }
            | Action::CancelChestUnlock {
                chest_id,
                expected_generation,
            } if !valid_id(chest_id) || chest_id.len() > 64 || *expected_generation == 0 => {
                return Err(Error::InvalidMessage);
            }
            Action::Move { direction } | Action::Cast { direction, .. }
                if direction.iter().any(|v| !v.is_finite()) =>
            {
                return Err(Error::InvalidMessage);
            }
            _ => {}
        }
        if let Some((previous, result)) = self
            .history
            .iter()
            .find(|(old, _)| old.request_id == input.request_id)
        {
            if reconnect && Some(&input.connection) != self.connection.as_ref() {
                return Err(Error::StaleConnection);
            }
            return if previous == &input {
                *result
            } else {
                Err(Error::Conflict)
            };
        }
        if input.sequence <= self.input_sequence {
            return Err(Error::StaleSequence);
        }
        self.reserve()?;
        if reconnect {
            return self.reconnect(input);
        }
        if matches!(input.action, Action::ConnectionLost {}) {
            return self.connection_lost(input);
        }
        if self.connection_lost {
            return Err(Error::NotReady);
        }
        if let Action::CombatReady { publication_id } = &input.action
            && !self.publications.iter().any(|publication| {
                publication.id == *publication_id
                    && publication.epoch == self.ready_epoch
                    && publication.outcome == TransportOutcome::Sent
            })
        {
            return Err(Error::NotReady);
        }
        let entering_protection = matches!(input.action, Action::Background {})
            && self.world.as_ref().is_some_and(|world| world.foreground);
        let next_epoch = if entering_protection {
            self.ready_epoch.checked_add(1).ok_or(Error::Faulted)?
        } else {
            self.ready_epoch
        };
        let result = self
            .world
            .as_mut()
            .ok_or(Error::NotReady)
            .and_then(|world| {
                world.action(&input.action, &input.request_id, &self.config, self.now)
            });
        let result = match result {
            Ok(events) => {
                if matches!(input.action, Action::CombatReady { .. }) {
                    self.resume_until = None;
                }
                if entering_protection {
                    self.ready_epoch = next_epoch;
                    self.last_full = None;
                }
                for event in events {
                    self.event(event);
                }
                self.begin_settlement();
                if matches!(
                    input.action,
                    Action::Join {}
                        | Action::Foreground {}
                        | Action::Background {}
                        | Action::Pickup { .. }
                        | Action::AttuneObjective { .. }
                        | Action::CancelAttunement {}
                        | Action::OpenChest { .. }
                        | Action::CancelChestUnlock { .. }
                        | Action::CancelAction { .. }
                ) {
                    self.publish();
                }
                Ok(self.status())
            }
            Err(error) => Err(error),
        };
        self.input_sequence = input.sequence;
        self.history.push_back((input, result));
        while self.history.len() > self.config.history_capacity {
            self.history.pop_front();
        }
        result
    }

    fn remember(&mut self, input: Input, result: Result<Status>) -> Result<Status> {
        self.input_sequence = input.sequence;
        self.history.push_back((input, result));
        while self.history.len() > self.config.history_capacity {
            self.history.pop_front();
        }
        result
    }

    fn reconnect(&mut self, input: Input) -> Result<Status> {
        let Action::Reconnect {
            previous_connection,
            authenticated_principal,
        } = &input.action
        else {
            unreachable!()
        };
        if !matches!(
            self.phase,
            Phase::Preparing | Phase::Deploying | Phase::Active
        ) {
            return Err(Error::InvalidState);
        }
        if authenticated_principal != &self.create.as_ref().unwrap().authenticated_principal {
            return Err(Error::Unauthorized);
        }
        if Some(previous_connection) != self.connection.as_ref() {
            return Err(Error::StaleConnection);
        }
        if !valid_id(&input.connection.id)
            || previous_connection.generation.checked_add(1) != Some(input.connection.generation)
        {
            return Err(Error::InvalidMessage);
        }
        let epoch = self.ready_epoch.checked_add(1).ok_or(Error::Faulted)?;
        let events = if let Some(world) = self.world.as_mut() {
            world.action(
                &Action::Background {},
                &input.request_id,
                &self.config,
                self.now,
            )?
        } else {
            Vec::new()
        };
        self.ready_epoch = epoch;
        self.connection = Some(input.connection.clone());
        self.connection_lost = false;
        self.resume_until = self
            .world
            .is_some()
            .then_some(self.now + self.config.params.runtime.reconnect_grace_ms);
        self.publications.clear();
        self.last_full = None;
        for event in events {
            self.event(event);
        }
        self.event(Event::ConnectionChanged {
            generation: input.connection.generation,
        });
        self.publish();
        self.remember(input, Ok(self.status()))
    }

    fn connection_lost(&mut self, input: Input) -> Result<Status> {
        if !matches!(
            self.phase,
            Phase::Preparing | Phase::Deploying | Phase::Active
        ) {
            return Err(Error::InvalidState);
        }
        if !self.connection_lost {
            let epoch = self.ready_epoch.checked_add(1).ok_or(Error::Faulted)?;
            let events = if let Some(world) = self.world.as_mut() {
                world.action(
                    &Action::Background {},
                    &input.request_id,
                    &self.config,
                    self.now,
                )?
            } else {
                Vec::new()
            };
            self.ready_epoch = epoch;
            self.connection_lost = true;
            self.resume_until = self
                .world
                .is_some()
                .then_some(self.now + self.config.params.runtime.reconnect_grace_ms);
            self.publications.clear();
            self.last_full = None;
            for event in events {
                self.event(event);
            }
            self.event(Event::ConnectionLost {
                generation: input.connection.generation,
                resume_until_ms: self.resume_until,
            });
            self.publish();
        }
        self.remember(input, Ok(self.status()))
    }

    pub fn transport(&mut self, receipt: TransportReceipt) -> Result<Status> {
        self.check_key(&receipt.key)?;
        let create = self.create.as_ref().expect("created");
        if Some(&receipt.connection) != self.connection.as_ref() {
            return Err(Error::StaleConnection);
        }
        if receipt.recipient_id != create.participant_id {
            return Err(Error::Unauthorized);
        }
        if matches!(self.phase, Phase::Closed | Phase::Faulted) {
            return Err(Error::Closed);
        }
        let index = self
            .publications
            .iter()
            .position(|publication| publication.id == receipt.output_id)
            .ok_or(Error::InvalidMessage)?;
        let entry = &self.publications[index];
        if entry.epoch != self.ready_epoch {
            return Err(Error::StalePublication);
        }
        if entry.outcome == receipt.outcome || receipt.outcome == TransportOutcome::Enqueued {
            return Ok(self.status());
        }
        if entry.outcome != TransportOutcome::Enqueued {
            return Err(Error::Conflict);
        }
        let replace = matches!(
            receipt.outcome,
            TransportOutcome::Failed | TransportOutcome::Cancelled
        ) && self.last_full.as_ref() == Some(&receipt.output_id);
        if replace {
            self.reserve()?;
        }
        self.publications[index].outcome = receipt.outcome;
        if replace {
            self.publish();
        }
        Ok(self.status())
    }

    pub fn business(&mut self, receipt: BusinessReceipt) -> Result<Status> {
        self.check_key(&receipt.key)?;
        let operation = self
            .operations
            .get(&receipt.operation_id)
            .ok_or(Error::InvalidMessage)?
            .clone();
        if receipt.attempt_id == 0 || receipt.attempt_id > operation.request.attempt_id {
            return Err(Error::InvalidMessage);
        }
        if let Some(previous) = &operation.terminal {
            return if previous == &receipt.outcome {
                Ok(self.status())
            } else {
                Err(Error::Conflict)
            };
        }
        if receipt.attempt_id < operation.request.attempt_id
            && !matches!(
                receipt.outcome,
                ReceiptOutcome::Committed { .. } | ReceiptOutcome::FactsAppended { .. }
            )
        {
            return Err(Error::InvalidMessage);
        }
        self.reserve()?;
        if let Business::QueryOperation { operation_id } = &operation.intent {
            match &receipt.outcome {
                ReceiptOutcome::Committed {
                    revision,
                    commit_id,
                } => self.apply_commit(operation_id, *revision, commit_id, false)?,
                ReceiptOutcome::FactsAppended { commit_id } => {
                    self.apply_facts(operation_id, commit_id, false)?
                }
                ReceiptOutcome::NotCommitted {} => {
                    self.complete_operation(&receipt.operation_id, ReceiptOutcome::NotCommitted {});
                    if !self.has_reconciliation(operation_id)
                        && self.operations.get(operation_id).is_some_and(|root| {
                            Some(root.request.attempt_id) == operation.queried_attempt
                        })
                    {
                        self.retry_or_reconcile(operation_id)?;
                    }
                }
                ReceiptOutcome::Refused {} => {
                    if !self.has_reconciliation(operation_id)
                        && self.operations.get(operation_id).is_some_and(|root| {
                            Some(root.request.attempt_id) == operation.queried_attempt
                        })
                    {
                        self.refuse_operation(operation_id)?;
                    }
                    self.complete_operation(&receipt.operation_id, ReceiptOutcome::Refused {});
                }
                ReceiptOutcome::Unknown {} => {
                    self.require_reconciliation(operation_id, Error::ExternalUnknown)
                }
                _ => return Err(Error::InvalidMessage),
            }
            self.operations
                .get_mut(&receipt.operation_id)
                .unwrap()
                .deadline_ms = None;
            return Ok(self.status());
        }
        if let Business::ReconcileOperation { operation_id } = &operation.intent {
            match &receipt.outcome {
                ReceiptOutcome::Reconciled {
                    revision,
                    commit_id,
                } => self.apply_commit(operation_id, *revision, commit_id, true)?,
                ReceiptOutcome::FactsAppended { commit_id } => {
                    self.apply_facts(operation_id, commit_id, true)?
                }
                ReceiptOutcome::Unknown {} => {} // An operator has not established the durable result yet.
                ReceiptOutcome::NotCommitted {} => {
                    let original = &self
                        .operations
                        .get(operation_id)
                        .ok_or(Error::InvalidMessage)?
                        .request
                        .body;
                    if !matches!(original, Business::CommitAdmission { .. }) {
                        // Proving no commit cannot cancel an inventory/progression obligation.
                        return Err(Error::ExternalRejected);
                    }
                    self.complete_operation(operation_id, ReceiptOutcome::NotCommitted {});
                    self.finish_close();
                }
                _ => return Err(Error::InvalidMessage),
            }
            return Ok(self.status());
        }
        if operation.refused && !matches!(receipt.outcome, ReceiptOutcome::Refused {}) {
            return Err(Error::Conflict);
        }
        match &receipt.outcome {
            ReceiptOutcome::Loaded { .. } | ReceiptOutcome::AssetsLoaded { .. } => {
                let facts = self.config.loaded_facts(&receipt.outcome)?;
                if !matches!(operation.intent, Business::LoadParticipant {}) {
                    return Err(Error::InvalidMessage);
                }
                if facts.principal
                    != self
                        .create
                        .as_ref()
                        .expect("created")
                        .authenticated_principal
                {
                    return Err(Error::Unauthorized);
                }
                if facts.revision == u32::MAX {
                    return Err(Error::InvalidMessage);
                }
                if [facts.atk_points, facts.def_points, facts.spd_points]
                    .iter()
                    .any(|v| *v > 1_000_000)
                    || self
                        .config
                        .derived_base(facts.hp_points, StatusAttribute::MaxHp)
                        > self.config.resource_bounds().max_hp
                    || self
                        .config
                        .derived_base(facts.spirit_points, StatusAttribute::MaxSpirit)
                        > self.config.resource_bounds().max_spirit
                    || self.config.build_rules().is_none() && facts.hp_points == 0
                    || facts.warehouse_quantity < self.config.carried_quantity
                {
                    return Err(Error::NoResource);
                }
                if !matches!(self.phase, Phase::Closed | Phase::Faulted) {
                    let clock = match &receipt.outcome {
                        ReceiptOutcome::AssetsLoaded { facts } => {
                            self.config.progression_entry_clock(facts)?
                        }
                        _ => None,
                    };
                    self.facts = Some(facts.clone());
                    self.request(
                        "admit",
                        Business::CommitAdmission {
                            expected_revision: facts.revision,
                            item_id: self.config.item_id.clone(),
                            quantity: self.config.carried_quantity,
                            entry_years: clock
                                .as_ref()
                                .and_then(EntryClock::charged)
                                .unwrap_or(self.config.params.entry_years),
                            clock,
                        },
                    );
                }
                self.complete_operation(&receipt.operation_id, receipt.outcome);
            }
            ReceiptOutcome::Committed {
                revision,
                commit_id,
            } => {
                self.apply_commit(&receipt.operation_id, *revision, commit_id, false)?;
            }
            ReceiptOutcome::FactsAppended { commit_id } => {
                self.apply_facts(&receipt.operation_id, commit_id, false)?;
            }
            ReceiptOutcome::Unknown {} => {
                self.unknown_operation(&receipt.operation_id)?;
            }
            ReceiptOutcome::NotCommitted {} => self.retry_or_reconcile(&receipt.operation_id)?,
            ReceiptOutcome::Refused {} => self.refuse_operation(&receipt.operation_id)?,
            ReceiptOutcome::Reconciled { .. } => return Err(Error::InvalidMessage),
        }
        Ok(self.status())
    }

    fn complete_operation(&mut self, id: &str, result: ReceiptOutcome) {
        let operation = self.operations.get_mut(id).expect("known responsibility");
        if operation.terminal.is_some() {
            return;
        }
        operation.terminal = Some(result.clone());
        operation.refused = false;
        operation.deadline_ms = None;
        self.event(Event::OperationResolved {
            operation_id: id.to_owned(),
        });
        // Resolve outstanding queries/reconciliation with the same authoritative proof.
        // A late negative query must never schedule another write after a known commit.
        let linked: Vec<_> = self
            .operations
            .iter()
            .filter_map(|(key, op)| {
                if op.terminal.is_some() {
                    return None;
                }
                let resolved = match (&op.intent, &result) {
                    (Business::QueryOperation { operation_id }, _) if operation_id == id => {
                        Some(result.clone())
                    }
                    (
                        Business::ReconcileOperation { operation_id },
                        ReceiptOutcome::Committed {
                            revision,
                            commit_id,
                        },
                    ) if operation_id == id => Some(ReceiptOutcome::Reconciled {
                        revision: *revision,
                        commit_id: commit_id.clone(),
                    }),
                    (
                        Business::ReconcileOperation { operation_id },
                        ReceiptOutcome::FactsAppended { .. },
                    ) if operation_id == id => Some(result.clone()),
                    (
                        Business::ReconcileOperation { operation_id },
                        ReceiptOutcome::NotCommitted {} | ReceiptOutcome::Refused {},
                    ) if operation_id == id => Some(ReceiptOutcome::NotCommitted {}),
                    _ => None,
                };
                resolved.map(|value| (key.clone(), value))
            })
            .collect();
        for (key, value) in linked {
            let operation = self.operations.get_mut(&key).expect("linked operation");
            operation.terminal = Some(value);
            operation.deadline_ms = None;
            self.event(Event::OperationResolved { operation_id: key });
        }
    }

    fn apply_commit(
        &mut self,
        id: &str,
        revision: u32,
        commit_id: &str,
        reconciled: bool,
    ) -> Result<()> {
        if !valid_id(commit_id) {
            return Err(Error::InvalidMessage);
        }
        let operation = self
            .operations
            .get(id)
            .ok_or(Error::InvalidMessage)?
            .clone();
        let result = ReceiptOutcome::Committed {
            revision,
            commit_id: commit_id.to_owned(),
        };
        if let Some(previous) = &operation.terminal {
            return if previous == &result {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        if operation.refused && !reconciled {
            return Err(Error::Conflict);
        }
        let expected_revision = match &operation.intent {
            Business::CommitAdmission {
                expected_revision, ..
            }
            | Business::CommitSettlement {
                expected_revision, ..
            } => *expected_revision,
            Business::CompensateAdmission {
                original_operation_id,
                ..
            } => {
                match self
                    .operations
                    .get(original_operation_id)
                    .and_then(|original| original.terminal.as_ref())
                {
                    Some(ReceiptOutcome::Committed { revision, .. }) => *revision,
                    _ => return Err(Error::InvalidState),
                }
            }
            _ => return Err(Error::InvalidMessage),
        };
        if revision
            != expected_revision
                .checked_add(1)
                .ok_or(Error::InvalidMessage)?
        {
            return Err(Error::RevisionConflict);
        }
        let mut close = false;
        match &operation.intent {
            Business::CommitAdmission { .. } => {
                if matches!(self.phase, Phase::Closed | Phase::Faulted) {
                    self.compensate_admission();
                } else if self.ledger_only {
                    self.phase = Phase::Deploying;
                } else {
                    let mut facts = self.facts.clone().ok_or(Error::InvalidState)?;
                    facts.revision = revision;
                    let mut world = World::new(
                        &self.config,
                        facts,
                        self.create.as_ref().expect("created").seed,
                    );
                    self.resume_until =
                        Some(self.now + self.config.params.runtime.reconnect_grace_ms);
                    if self.connection_lost
                        || self.connection.as_ref()
                            != self.create.as_ref().map(|create| &create.connection)
                    {
                        world.action(
                            &Action::Background {},
                            "admission-connection-fence",
                            &self.config,
                            self.now,
                        )?;
                    }
                    self.world = Some(world);
                    self.phase = Phase::Deploying;
                }
            }
            Business::CommitSettlement {
                outcome,
                kept,
                dropped,
                cultivation,
                scoring,
                map_progress,
                reset_progress,
                items,
                ..
            } => {
                let event = match &operation.request.body {
                    Business::CommitAssets {
                        transaction:
                            AssetTransaction::Settlement {
                                credits, summary, ..
                            },
                    } => Event::AssetSettlementApplied {
                        credits: credits.clone(),
                        summary: summary.clone(),
                    },
                    Business::CommitAssets {
                        transaction:
                            AssetTransaction::ResultSettlement {
                                credits, summary, ..
                            },
                    } => Event::ResultSettlementApplied {
                        credits: credits.clone(),
                        summary: summary.clone(),
                    },
                    _ => Event::SettlementApplied {
                        outcome: *outcome,
                        kept: *kept,
                        dropped: *dropped,
                        cultivation: *cultivation,
                        scoring: scoring.clone(),
                        map_progress: map_progress.clone(),
                        reset_progress: reset_progress.clone(),
                        items: items.clone(),
                    },
                };
                self.event(event);
                close = self.world.is_some();
            }
            Business::CompensateAdmission { .. } => {}
            _ => unreachable!("validated original write"),
        }
        self.complete_operation(id, result);
        if let Some(facts) = self.config.settlement_fact_intent(id, &operation.intent) {
            self.request("facts", facts);
        }
        if close {
            self.finish_close();
        }
        Ok(())
    }

    fn apply_facts(&mut self, id: &str, commit_id: &str, reconciled: bool) -> Result<()> {
        let operation = self.operations.get(id).ok_or(Error::InvalidMessage)?;
        if !valid_id(commit_id) || !matches!(operation.intent, Business::AppendFacts { .. }) {
            return Err(Error::InvalidMessage);
        }
        let proof = ReceiptOutcome::FactsAppended {
            commit_id: commit_id.to_owned(),
        };
        if let Some(previous) = &operation.terminal {
            return if *previous == proof {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        if operation.refused && !reconciled {
            return Err(Error::Conflict);
        }
        self.complete_operation(id, proof);
        Ok(())
    }

    fn require_reconciliation(&mut self, id: &str, reason: Error) {
        let operation = self.operations.get(id).expect("known responsibility");
        if operation.terminal.is_some() {
            return;
        }
        let Some(label) = operation.intent.durable_write_label() else {
            return;
        };
        let suffix = format!("reconcile-{label}");
        let admission = matches!(operation.intent, Business::CommitAdmission { .. });
        self.operations.get_mut(id).unwrap().deadline_ms = None;
        if self.operations.contains_key(&self.operation_id(&suffix)) {
            return;
        }
        self.event(Event::OperationNeedsReconciliation {
            operation_id: id.to_owned(),
            reason,
        });
        self.request(
            &suffix,
            Business::ReconcileOperation {
                operation_id: id.to_owned(),
            },
        );
        if admission && self.phase != Phase::Faulted {
            self.finish_close();
        }
    }

    fn has_reconciliation(&self, id: &str) -> bool {
        self.operations.values().any(|op| {
            matches!(&op.intent,
            Business::ReconcileOperation { operation_id } if operation_id == id)
        })
    }

    fn unknown_operation(&mut self, id: &str) -> Result<()> {
        let operation = self
            .operations
            .get(id)
            .ok_or(Error::InvalidMessage)?
            .clone();
        self.operations.get_mut(id).unwrap().deadline_ms = None;
        if self.has_reconciliation(id) {
            return Ok(());
        }
        if matches!(operation.intent, Business::LoadParticipant {}) {
            return self.retry_or_reconcile(id);
        }
        let label = operation
            .intent
            .durable_write_label()
            .ok_or(Error::InvalidMessage)?;
        self.request(
            &format!("query-{label}-{}", operation.request.attempt_id),
            Business::QueryOperation {
                operation_id: id.to_owned(),
            },
        );
        Ok(())
    }

    /// Internal monotonic service clock. It never advances gameplay and is not a Client API.
    pub(crate) fn poll_io(&mut self, elapsed_ms: u64) {
        self.io_ms = self.io_ms.max(elapsed_ms);
        let due: Vec<_> = self
            .operations
            .iter()
            .filter_map(|(id, op)| {
                (op.terminal.is_none() && op.deadline_ms.is_some_and(|at| at <= self.io_ms))
                    .then_some(id.clone())
            })
            .collect();
        for id in due {
            if self.reserve().is_err() {
                // Gameplay lifetime must not depend on the host draining receipts.
                // close has its own reserved capacity; the due I/O remains armed.
                self.close_for_io();
                return;
            }
            let operation = self.operations.get(&id).unwrap().clone();
            self.operations.get_mut(&id).unwrap().deadline_ms = None;
            self.event(Event::IoTimedOut {
                operation_id: id.clone(),
                attempt_id: operation.request.attempt_id,
            });
            match operation.intent {
                Business::LoadParticipant {} => {
                    // Retire a read with no facts; this is not proof about a money write.
                    self.complete_operation(&id, ReceiptOutcome::Unknown {});
                    if self.phase != Phase::Faulted {
                        self.finish_close();
                    }
                }
                Business::QueryOperation { operation_id } => {
                    let facts = matches!(
                        self.operations[&operation_id].intent,
                        Business::AppendFacts { .. }
                    );
                    self.require_reconciliation(&operation_id, Error::ExternalUnknown);
                    if !facts {
                        self.close_for_io();
                    }
                }
                Business::AppendFacts { .. } => {
                    let _ = self.unknown_operation(&id);
                }
                Business::CommitAdmission { .. }
                | Business::CommitSettlement { .. }
                | Business::CompensateAdmission { .. } => {
                    let _ = self.unknown_operation(&id);
                    self.close_for_io();
                }
                Business::ReconcileOperation { .. } => {
                    unreachable!("manual work has no automatic deadline")
                }
                Business::LoadAssets {} | Business::CommitAssets { .. } => {
                    self.fault(Error::Faulted)
                }
            }
        }
        if self.world.is_some() && self.drain_until.is_some_and(|at| at <= self.io_ms) {
            if self.outputs.len() + 5 <= self.config.output_capacity {
                self.event(Event::IoDrainExpired {});
            }
            self.close_for_io();
        }
    }

    fn close_for_io(&mut self) {
        if matches!(self.phase, Phase::Closed | Phase::Faulted) {
            return;
        }
        let _ = self.close(Close {
            key: self.key(),
            request_id: "io-deadline-close".into(),
        });
    }

    fn refuse_operation(&mut self, id: &str) -> Result<()> {
        if self.has_reconciliation(id) {
            return Ok(());
        }
        let operation = self
            .operations
            .get(id)
            .ok_or(Error::InvalidMessage)?
            .clone();
        match operation.intent {
            Business::LoadParticipant {} | Business::CommitAdmission { .. } => {
                self.complete_operation(id, ReceiptOutcome::Refused {});
                self.finish_close();
            }
            Business::CommitSettlement { .. }
            | Business::CompensateAdmission { .. }
            | Business::AppendFacts { .. } => {
                self.operations
                    .get_mut(id)
                    .expect("known responsibility")
                    .refused = true;
                self.require_reconciliation(id, Error::ExternalRejected);
            }
            _ => return Err(Error::InvalidMessage),
        }
        Ok(())
    }

    fn retry_or_reconcile(&mut self, id: &str) -> Result<()> {
        let operation = self
            .operations
            .get(id)
            .ok_or(Error::InvalidMessage)?
            .clone();
        if operation.terminal.is_some() {
            return Err(Error::Conflict);
        }
        if self.has_reconciliation(id) {
            return Ok(());
        }
        if operation.refused {
            self.require_reconciliation(id, Error::ExternalRejected);
            return Ok(());
        }
        let admission = matches!(
            operation.intent,
            Business::LoadParticipant {} | Business::CommitAdmission { .. }
        );
        if admission
            && (matches!(self.phase, Phase::Closed | Phase::Faulted)
                || operation.request.attempt_id >= self.config.params.runtime.max_attempts)
        {
            // A definitive fenced no-commit after close cannot start a fresh debit.
            self.complete_operation(id, ReceiptOutcome::NotCommitted {});
            self.finish_close();
        } else if operation.request.attempt_id >= self.config.params.runtime.max_attempts {
            self.require_reconciliation(id, Error::ExternalUnknown);
        } else {
            let mut request = operation.request;
            request.attempt_id += 1;
            self.operations
                .get_mut(id)
                .expect("known responsibility")
                .request = request.clone();
            self.operations.get_mut(id).unwrap().deadline_ms =
                Some(self.io_ms + self.config.params.runtime.io_timeout_ms);
            self.emit(OutputBody::BusinessRequest { request });
        }
        Ok(())
    }

    fn compensate_admission(&mut self) {
        let original_operation_id = self.operation_id("admit");
        let (entry_years, clock) = self
            .operations
            .get(&original_operation_id)
            .and_then(|op| match &op.intent {
                Business::CommitAdmission {
                    entry_years, clock, ..
                } => Some((*entry_years, clock.as_ref().map(EntryClock::reversed))),
                _ => None,
            })
            .expect("compensate original admission");
        self.request(
            "compensate-admit",
            Business::CompensateAdmission {
                original_operation_id,
                item_id: self.config.item_id.clone(),
                quantity: self.config.carried_quantity,
                entry_years,
                clock,
            },
        );
    }

    fn begin_settlement(&mut self) {
        if self.operations.contains_key(&self.operation_id("settle")) {
            return;
        }
        let Some(world) = self.world.as_mut() else {
            return;
        };
        if world.desired_outcome.is_none() {
            return;
        }
        let body = world.settlement(&self.config);
        self.phase = Phase::Draining;
        self.drain_until = Some(self.io_ms + self.config.params.runtime.drain_timeout_ms);
        self.request("settle", body);
        self.publish();
    }

    pub fn tick(&mut self) {
        if self.create.is_none()
            || matches!(self.phase, Phase::Closed | Phase::Faulted | Phase::Draining)
        {
            return;
        }
        if self.reserve().is_err() {
            self.fault(Error::QueueFull);
            return;
        }
        self.now += self.config.step_ms;
        if self
            .resume_until
            .is_some_and(|deadline| self.now >= deadline)
        {
            self.event(Event::ConnectionExpired {
                generation: self.connection.as_ref().unwrap().generation,
            });
            let _ = self.close(Close {
                key: self.key(),
                request_id: "connection-grace-expired".into(),
            });
            return;
        }
        let publication_interval = match self.create.as_ref().unwrap().publication_mode {
            PublicationMode::Batched => self.config.params.runtime.publish_batched_ms,
            PublicationMode::Interactive => self.config.step_ms,
        };
        if let Some(world) = self.world.as_mut() {
            let events = world.tick(&self.config, self.now);
            self.phase = world.phase;
            self.was_active |= self.phase == Phase::Active;
            let changed = !events.is_empty();
            // A world step can produce a burst larger than the minimum reservation
            // (for example several encounters activating together). Keep room for
            // a publication and the frozen terminal request before emitting any of it.
            if self.outputs.len() + events.len() + 4 > self.config.output_capacity {
                self.fault(Error::QueueFull);
                return;
            }
            for event in events {
                self.event(event);
            }
            self.begin_settlement();
            if changed
                || (!self.connection_lost && self.now >= self.last_publish + publication_interval)
            {
                self.publish();
            }
        }
    }

    pub fn close(&mut self, close: Close) -> Result<Status> {
        self.check_key(&close.key)?;
        if !valid_id(&close.request_id) {
            return Err(Error::InvalidMessage);
        }
        if let Some(result) = &self.close_result {
            return Ok(*result);
        }
        let interrupting = self
            .world
            .as_ref()
            .map_or(0, |w| w.close_interaction_event_budget());
        if self.outputs.len() + 4 + interrupting > self.config.output_capacity {
            return Err(Error::QueueFull);
        }
        if self.was_active {
            let events = self
                .world
                .as_mut()
                .map_or_else(Vec::new, |w| w.close_container_interaction(self.now));
            for event in events {
                self.event(event);
            }
            if let Some(world) = self.world.as_mut()
                && world.desired_outcome.is_none()
            {
                world.desired_outcome = Some(Outcome::Death);
            }
            self.begin_settlement();
        } else if self.world.is_some() || self.ledger_only && self.ledger_admitted_facts().is_some()
        {
            self.compensate_admission();
        }
        self.finish_close();
        Ok(self.status())
    }

    fn finish_close(&mut self) {
        if self.close_result.is_some() {
            return;
        }
        self.world = None;
        self.facts = None;
        self.phase = Phase::Closed;
        self.drain_until = None;
        self.publications.clear();
        self.last_full = None;
        // The currently applying receipt may not yet be recorded. The ledger remains outside World.
        let pending_operations = self.operations.iter().filter(|(_, op)| {
            op.terminal.is_none() && !matches!(&op.intent, Business::QueryOperation { operation_id } | Business::ReconcileOperation { operation_id }
                if self.operations.get(operation_id).is_some_and(|original| original.terminal.is_some()))
        }).map(|(id, _)| id.clone()).collect();
        self.event(Event::Closed {
            pending_operations,
            live_worlds: 0,
        });
        self.close_result = Some(self.status());
    }

    pub fn drain(&mut self) -> Vec<Output> {
        self.outputs.drain(..).collect()
    }

    // The worker still owns a drained batch until the caller has accepted its reply.
    pub(crate) fn restore_undelivered(&mut self, outputs: Vec<Output>) {
        assert!(self.outputs.is_empty());
        assert!(outputs.len() <= self.config.output_capacity);
        self.outputs.extend(outputs);
    }

    pub fn fault(&mut self, code: Error) {
        if self.create.is_none() || matches!(self.phase, Phase::Closed | Phase::Faulted) {
            return;
        }
        if self.outputs.len() + 4 <= self.config.output_capacity {
            if self.was_active {
                if let Some(world) = self.world.as_mut()
                    && world.desired_outcome.is_none()
                {
                    world.desired_outcome = Some(Outcome::Death);
                }
                self.begin_settlement();
            } else if self.world.is_some() {
                self.compensate_admission();
            }
        }
        self.world = None;
        self.phase = Phase::Faulted;
        if self.create.is_some() && self.outputs.len() < self.config.output_capacity {
            self.event(Event::Fault { code });
        }
    }

    #[cfg(all(test, match_sdk_test_data))]
    pub fn live_worlds(&self) -> u32 {
        u32::from(self.world.is_some())
    }

    #[cfg(all(test, match_sdk_test_data))]
    pub fn pending_count(&self) -> u32 {
        self.operations
            .values()
            .filter(|operation| operation.terminal.is_none())
            .count() as u32
    }

    #[cfg(all(test, match_sdk_test_data))]
    pub fn queued_output_count(&self) -> u32 {
        self.outputs.len() as u32
    }

    #[cfg(all(test, match_sdk_test_data))]
    pub fn history_for_test(&self) -> Input {
        self.history.back().expect("input history").0.clone()
    }

    #[cfg(all(test, match_sdk_test_data))]
    pub fn pending_for_test(&self) -> Vec<String> {
        self.operations
            .iter()
            .filter(|(_, op)| op.terminal.is_none())
            .map(|(id, _)| id.clone())
            .collect()
    }
}
