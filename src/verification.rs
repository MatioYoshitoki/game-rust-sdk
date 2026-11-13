//! Explicit verification-only fixed-clock runner. Not a production World control API.
use crate::{Config, config_hash, engine::Engine, protocol::*};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Default)]
struct Store {
    warehouse: u32,
    cultivation: u32,
    revision: u32,
    years: u32,
    admission_commits: u32,
    settlement_commits: u32,
    journal: BTreeMap<String, (Business, ReceiptOutcome)>,
}

impl Store {
    fn execute(&mut self, request: &BusinessRequest) -> Result<ReceiptOutcome> {
        if let Some((body, outcome)) = self.journal.get(&request.operation_id) {
            return if body == &request.body {
                Ok(outcome.clone())
            } else {
                Err(Error::Conflict)
            };
        }
        let outcome = match &request.body {
            Business::LoadAssets {}
            | Business::CommitAssets { .. }
            | Business::AppendFacts { .. } => {
                return Err(Error::Unsupported);
            }
            Business::LoadParticipant {} => ReceiptOutcome::Loaded {
                facts: ParticipantFacts {
                    principal: "principal-A".into(),
                    revision: self.revision,
                    hp_points: 1000,
                    spirit_points: 100,
                    atk_points: 100,
                    def_points: 10,
                    spd_points: 8,
                    warehouse_quantity: self.warehouse,
                },
            },
            Business::CommitAdmission {
                expected_revision,
                quantity,
                entry_years,
                ..
            } => {
                if *expected_revision != self.revision || self.warehouse < *quantity {
                    self.journal.insert(
                        request.operation_id.clone(),
                        (request.body.clone(), ReceiptOutcome::Refused {}),
                    );
                    return Ok(ReceiptOutcome::Refused {});
                }
                self.warehouse -= quantity;
                self.years += entry_years;
                self.revision += 1;
                self.admission_commits += 1;
                ReceiptOutcome::Committed {
                    revision: self.revision,
                    commit_id: format!("tx-{}", self.revision),
                }
            }
            Business::CommitSettlement {
                expected_revision,
                kept,
                cultivation,
                ..
            } => {
                if *expected_revision != self.revision {
                    self.journal.insert(
                        request.operation_id.clone(),
                        (request.body.clone(), ReceiptOutcome::Refused {}),
                    );
                    return Ok(ReceiptOutcome::Refused {});
                }
                self.warehouse += kept;
                self.cultivation += cultivation;
                self.revision += 1;
                self.settlement_commits += 1;
                ReceiptOutcome::Committed {
                    revision: self.revision,
                    commit_id: format!("tx-{}", self.revision),
                }
            }
            Business::CompensateAdmission {
                quantity,
                entry_years,
                ..
            } => {
                self.warehouse += quantity;
                self.years -= entry_years;
                self.revision += 1;
                ReceiptOutcome::Committed {
                    revision: self.revision,
                    commit_id: format!("tx-{}", self.revision),
                }
            }
            Business::QueryOperation { operation_id } => self
                .journal
                .get(operation_id)
                .map(|(_, result)| result.clone())
                .unwrap_or(ReceiptOutcome::NotCommitted {}),
            Business::ReconcileOperation { operation_id } => {
                let (body, previous) = self
                    .journal
                    .get(operation_id)
                    .ok_or(Error::InvalidMessage)?
                    .clone();
                let result = if matches!(previous, ReceiptOutcome::Committed { .. }) {
                    previous
                } else {
                    // Controlled stand-in for an operator reconciling the frozen original intent.
                    // The production service must fence old attempts and apply this transaction atomically.
                    self.journal.remove(operation_id);
                    self.execute(&BusinessRequest {
                        operation_id: operation_id.clone(),
                        attempt_id: 1,
                        body,
                    })?
                };
                match result {
                    ReceiptOutcome::Committed {
                        revision,
                        commit_id,
                    } => ReceiptOutcome::Reconciled {
                        revision,
                        commit_id,
                    },
                    _ => ReceiptOutcome::Unknown {},
                }
            }
        };
        if !matches!(outcome, ReceiptOutcome::Unknown {}) {
            self.journal.insert(
                request.operation_id.clone(),
                (request.body.clone(), outcome.clone()),
            );
        }
        Ok(outcome)
    }
}

fn create_request() -> Create {
    Create {
        protocol_version: 1,
        key: MatchKey {
            id: "b1".into(),
            incarnation: 1,
        },
        request_id: "create-1".into(),
        config_id: Config::frozen().id().into(),
        config_hash: config_hash(),
        seed: 7,
        participant_id: "A".into(),
        authenticated_principal: "principal-A".into(),
        connection: Connection {
            id: "conn-A".into(),
            generation: 1,
        },
        publication_mode: PublicationMode::default(),
        recovery: None,
    }
}

struct Harness {
    engine: Engine,
    store: Store,
    sequence: u32,
    snapshot: Option<Snapshot>,
    trace: Vec<Output>,
}

impl Harness {
    fn new() -> Self {
        Self {
            engine: Engine::new(Config::frozen()),
            store: Store {
                warehouse: 100,
                ..Store::default()
            },
            sequence: 0,
            snapshot: None,
            trace: Vec::new(),
        }
    }

    fn action(&mut self, action: Action) -> Result<Status> {
        self.sequence += 1;
        let create = create_request();
        self.engine.input(Input {
            key: create.key,
            participant_id: "A".into(),
            entity_generation: 1,
            connection: create.connection,
            request_id: format!("r-{}", self.sequence),
            sequence: self.sequence,
            action,
        })
    }

    fn pump(&mut self) -> Result<()> {
        for _ in 0..32 {
            let outputs = self.engine.drain();
            if outputs.is_empty() {
                return Ok(());
            }
            for output in outputs {
                match &output.body {
                    OutputBody::BusinessRequest { request } => {
                        let outcome = self.store.execute(request)?;
                        self.engine.business(BusinessReceipt {
                            key: output.key.clone(),
                            operation_id: request.operation_id.clone(),
                            attempt_id: request.attempt_id,
                            outcome,
                        })?;
                    }
                    OutputBody::Publication {
                        recipient_id,
                        connection,
                        snapshot,
                    } => {
                        self.snapshot = Some(snapshot.clone());
                        let transport = self.engine.transport(TransportReceipt {
                            key: output.key.clone(),
                            output_id: output.id.clone(),
                            recipient_id: recipient_id.clone(),
                            connection: connection.clone(),
                            outcome: TransportOutcome::Sent,
                        });
                        if !matches!(
                            transport,
                            Ok(_) | Err(Error::Closed | Error::StalePublication)
                        ) {
                            transport?;
                        }
                        if snapshot.phase == Phase::Deploying && snapshot.protected {
                            self.action(Action::CombatReady {
                                publication_id: output.id.clone(),
                            })?;
                        }
                    }
                    _ => {}
                }
                self.trace.push(output);
            }
        }
        Err(Error::Faulted)
    }

    fn start(&mut self) -> Result<()> {
        self.engine.create(create_request())?;
        self.pump()?;
        self.action(Action::Join {})?;
        self.pump()?;
        self.advance(30)
    }

    fn advance(&mut self, milliseconds: u64) -> Result<()> {
        let target = self.engine.status().logical_ms + milliseconds;
        while self.engine.status().logical_ms < target
            && !matches!(self.engine.status().phase, Phase::Closed | Phase::Faulted)
        {
            self.engine.tick();
            self.pump()?;
        }
        Ok(())
    }

    fn fight_and_pickup(&mut self) -> Result<()> {
        self.action(Action::Move {
            direction: [1.0, 0.0],
        })?;
        self.advance(30)?;
        self.action(Action::Stop {})?;
        self.action(Action::Attack {})?;
        self.advance(400)?;
        self.action(Action::Cast {
            skill_id: "duanjin-shou".into(),
            direction: [1.0, 0.0],
        })?;
        self.advance(1000)?;
        let snapshot = self.snapshot.as_ref().ok_or(Error::Faulted)?;
        if snapshot.target.is_some() {
            return Err(Error::Faulted);
        }
        let revision = snapshot.inventory_revision;
        self.action(Action::Pickup {
            resource_id: "drop-1".into(),
            expected_revision: revision,
        })?;
        self.pump()?;
        let revision = self.snapshot.as_ref().expect("snapshot").inventory_revision;
        self.action(Action::Pickup {
            resource_id: "orb-1".into(),
            expected_revision: revision,
        })?;
        self.pump()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Report {
    pub evidence: &'static str,
    pub outcome: Outcome,
    pub config_hash: String,
    pub warehouse: u32,
    pub cultivation: u32,
    pub admission_commits: u32,
    pub settlement_commits: u32,
    pub charged_entry_years: u32,
    pub live_worlds: u32,
    pub pending_operations: u32,
    pub queued_outputs: u32,
    pub phase: Phase,
    pub logical_ms: u64,
    pub output_count: usize,
}

pub fn run(outcome: Outcome) -> Result<Report> {
    let mut harness = Harness::new();
    harness.start()?;
    harness.fight_and_pickup()?;
    match outcome {
        Outcome::Extraction => {
            let active_at = harness
                .snapshot
                .as_ref()
                .expect("active")
                .active_at_ms
                .expect("active clock");
            let until_open =
                (active_at + 180000).saturating_sub(harness.engine.status().logical_ms);
            harness.advance(until_open)?;
            harness.action(Action::ExtractStart {})?;
            harness.advance(5100)?;
        }
        Outcome::Death => {
            harness.action(Action::Leave {})?;
            harness.pump()?;
        }
        _ => return Err(Error::Unsupported),
    }
    let report = Report {
        evidence: "controlled-memory-storage; verification-fixture",
        outcome,
        config_hash: config_hash(),
        warehouse: harness.store.warehouse,
        cultivation: harness.store.cultivation,
        admission_commits: harness.store.admission_commits,
        settlement_commits: harness.store.settlement_commits,
        charged_entry_years: harness.store.years,
        live_worlds: harness.engine.live_worlds(),
        pending_operations: harness.engine.pending_count(),
        queued_outputs: harness.engine.queued_output_count(),
        phase: harness.engine.status().phase,
        logical_ms: harness.engine.status().logical_ms,
        output_count: harness.trace.len(),
    };
    let (warehouse, cultivation) = match outcome {
        Outcome::Extraction => (102, 30),
        Outcome::Death => (99, 3),
        _ => return Err(Error::Unsupported),
    };
    if report.warehouse != warehouse
        || report.cultivation != cultivation
        || report.admission_commits != 1
        || report.settlement_commits != 1
        || report.live_worlds != 0
        || report.pending_operations != 0
        || report.queued_outputs != 0
        || report.phase != Phase::Closed
    {
        return Err(Error::Faulted);
    }
    Ok(report)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod boundaries;

#[cfg(test)]
mod reconnect;

#[cfg(test)]
mod timeouts;

#[cfg(test)]
mod durability;

#[cfg(test)]
mod concurrency;

#[cfg(test)]
mod chests;
#[cfg(test)]
mod encounters;
#[cfg(test)]
mod guardians;
#[cfg(test)]
mod sealed;

#[cfg(test)]
mod waves;

#[cfg(test)]
mod bosses;

#[cfg(test)]
mod scoring;

#[cfg(test)]
mod area;

#[cfg(test)]
mod objective;

#[cfg(test)]
mod support;

#[cfg(test)]
mod heavy;

#[cfg(test)]
mod reset;
