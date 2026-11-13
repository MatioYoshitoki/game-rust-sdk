//! A separate consumer crate. Only public SDK types are used; no replay clock or World access.
mod store;

use match_kernel::{Client, ConfigCatalog, Runtime, protocol::*};
use std::{
    collections::BTreeSet,
    env, fs,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use store::{Account, Store};

type HostResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn check(condition: bool, message: &str) -> HostResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    UnknownSettlement,
    RefusedAdmission,
    CloseBeforeReady,
    FailedPublication,
    ConfigMismatch,
}

impl Mode {
    fn parse(value: &str) -> HostResult<Self> {
        match value {
            "normal" => Ok(Self::Normal),
            "unknown-settlement" => Ok(Self::UnknownSettlement),
            "refused-admission" => Ok(Self::RefusedAdmission),
            "close-before-ready" => Ok(Self::CloseBeforeReady),
            "failed-publication" => Ok(Self::FailedPublication),
            "config-mismatch" => Ok(Self::ConfigMismatch),
            _ => Err(format!("unknown mode: {value}").into()),
        }
    }
}

struct Host {
    client: Client,
    key: MatchKey,
    participant_id: String,
    connection: Connection,
    store: Store,
    mode: Mode,
    sequence: u32,
    phase: Phase,
    // The local scripted player is the transport receiver for this example.
    received: Option<(String, Snapshot)>,
    pending: BTreeSet<String>,
    resolved: BTreeSet<String>,
    closed_events: u32,
    settlement_events: u32,
    failed_publications: u32,
    unknown_injected: bool,
    active_seen: bool,
    closed_with_pending: bool,
}

impl Host {
    fn input(&mut self, action: Action) -> HostResult<()> {
        self.sequence += 1;
        let request = Input {
            key: self.key.clone(),
            participant_id: self.participant_id.clone(),
            entity_generation: 1,
            connection: self.connection.clone(),
            request_id: format!("input-{}", self.sequence),
            sequence: self.sequence,
            action,
        };
        self.phase = self.client.submit_input(request.clone())?.phase;
        self.client.submit_input(request)?; // Same ID, sequence and body: safe duplicate.
        Ok(())
    }

    fn close(&mut self) -> HostResult<()> {
        let request = Close {
            key: self.key.clone(),
            request_id: "close".into(),
        };
        let first = self.client.close(request.clone())?;
        check(self.client.close(request)? == first, "close result changed")?;
        self.phase = first.phase;
        Ok(())
    }

    fn pump(&mut self) -> HostResult<()> {
        // A durable host must persist this batch's latest checkpoint, intents and
        // unique events before executing I/O or delivering publications (see guide).
        for output in self.client.drain_outputs()? {
            check(output.key == self.key, "wrong match output")?;
            match output.body {
                OutputBody::BusinessRequest { request } => {
                    self.pending.insert(request.operation_id.clone());
                    let stored = self.store.execute(&request)?;
                    let account = self.store.account.clone();
                    check(
                        self.store.execute(&request)? == stored,
                        "store receipt changed",
                    )?;
                    check(
                        self.store.account == account,
                        "duplicate business mutated account",
                    )?;
                    let outcome = if self.mode == Mode::UnknownSettlement
                        && !self.unknown_injected
                        && matches!(request.body, Business::CommitSettlement { .. })
                    {
                        check(
                            matches!(stored, ReceiptOutcome::Committed { .. }),
                            "commit required",
                        )?;
                        self.unknown_injected = true;
                        ReceiptOutcome::Unknown {}
                    } else {
                        stored
                    };
                    let receipt = BusinessReceipt {
                        key: self.key.clone(),
                        operation_id: request.operation_id,
                        attempt_id: request.attempt_id,
                        outcome,
                    };
                    self.phase = self.client.submit_business_receipt(receipt.clone())?.phase;
                    self.client.submit_business_receipt(receipt)?;
                }
                OutputBody::Publication {
                    recipient_id,
                    connection,
                    snapshot,
                } => {
                    check(
                        recipient_id == self.participant_id && connection == self.connection,
                        "publication recipient mismatch",
                    )?;
                    let failed = self.mode == Mode::FailedPublication
                        && self.failed_publications == 0
                        && snapshot.phase == Phase::Deploying
                        && snapshot.protected;
                    let outcome = if failed {
                        self.failed_publications += 1;
                        TransportOutcome::Failed
                    } else {
                        // Delivery boundary: the local player now owns the immutable snapshot.
                        self.active_seen |= snapshot.phase == Phase::Active;
                        self.received = Some((output.id.clone(), snapshot));
                        TransportOutcome::Sent
                    };
                    let receipt = TransportReceipt {
                        key: self.key.clone(),
                        output_id: output.id,
                        recipient_id,
                        connection,
                        outcome,
                    };
                    match self.client.submit_transport_receipt(receipt.clone()) {
                        Ok(status) => {
                            self.phase = status.phase;
                            self.client.submit_transport_receipt(receipt)?;
                        }
                        Err(Error::Closed) => {} // Late publication ACK cannot reopen a closed World.
                        Err(error) => return Err(error.into()),
                    }
                }
                OutputBody::Event { event } => match event {
                    Event::OperationResolved { operation_id } => {
                        self.pending.remove(&operation_id);
                        self.resolved.insert(operation_id);
                    }
                    Event::SettlementApplied { .. } => self.settlement_events += 1,
                    Event::Closed {
                        pending_operations,
                        live_worlds,
                    } => {
                        check(live_worlds == 0, "World was not released")?;
                        self.closed_events += 1;
                        self.closed_with_pending |= !pending_operations.is_empty();
                        for id in pending_operations {
                            if !self.resolved.contains(&id) {
                                self.pending.insert(id);
                            }
                        }
                    }
                    Event::Fault { code } => return Err(code.into()),
                    _ => {}
                },
                OutputBody::SpectatorPublicationV6 { .. }
                | OutputBody::PublicationV6 { .. }
                | OutputBody::BusinessRequestV4 { .. }
                | OutputBody::PublicationV4 { .. }
                | OutputBody::EventV4 { .. }
                | OutputBody::ResponsibilityV4 { .. } => {
                    return Err("unexpected multi-member output in the protocol-1 consumer".into());
                }
            }
        }
        Ok(())
    }

    fn until(&mut self, ready: impl Fn(&Self) -> bool) -> HostResult<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready(self) {
            check(
                Instant::now() < deadline,
                "sample deadline reached; unresolved operations require investigation",
            )?;
            self.pump()?;
            thread::sleep(Duration::from_millis(3));
        }
        Ok(())
    }
}

fn main() -> HostResult<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    check(
        (2..=4).contains(&args.len()),
        "usage: minimal-match-host <catalog.json> <account.json> [mode] [scenario]",
    )?;
    let mode_name = args.get(2).map(String::as_str).unwrap_or("normal");
    let mode = Mode::parse(mode_name)?;
    // Explicit loading returns configuration errors before a worker is started.
    let catalog = Arc::new(ConfigCatalog::load(&args[0])?);
    let config = match args.get(3) {
        Some(alias) => catalog.active(alias).ok_or("unknown scenario alias")?,
        None => catalog.default_config(),
    };
    let initial: Account = serde_json::from_slice(&fs::read(&args[1])?)?;
    // Keep the Runtime owner alive until World closure AND all business obligations resolve.
    let runtime = Runtime::with_catalog(catalog);
    let mut host = Host {
        client: runtime.client(),
        key: MatchKey {
            id: "sdk-2-example".into(),
            incarnation: 1,
        },
        participant_id: initial.participant_id.clone(),
        connection: Connection {
            id: "local-player".into(),
            generation: 1,
        },
        store: Store::new(initial.clone(), mode == Mode::RefusedAdmission),
        mode,
        sequence: 0,
        phase: Phase::Preparing,
        received: None,
        pending: BTreeSet::new(),
        resolved: BTreeSet::new(),
        closed_events: 0,
        settlement_events: 0,
        failed_publications: 0,
        unknown_injected: false,
        active_seen: false,
        closed_with_pending: false,
    };
    let create = Create {
        protocol_version: 1,
        key: host.key.clone(),
        request_id: "create".into(),
        config_id: config.id().into(),
        config_hash: config.content_hash().into(),
        seed: initial.seed,
        participant_id: initial.participant_id.clone(),
        authenticated_principal: initial.facts.principal.clone(),
        connection: host.connection.clone(),
        publication_mode: PublicationMode::Batched,
        recovery: None,
    };
    if mode == Mode::ConfigMismatch {
        let mut invalid = create.clone();
        invalid.config_hash = "0".repeat(64);
        check(
            host.client.create(invalid) == Err(Error::ConfigMismatch),
            "wrong hash accepted",
        )?;
        check(
            host.client.drain_outputs()?.is_empty(),
            "invalid create emitted business output",
        )?;
    }
    host.client.create(create.clone())?;
    host.client.create(create)?;
    if mode == Mode::RefusedAdmission {
        host.until(|h| h.closed_events == 1 && h.pending.is_empty())?;
    } else {
        host.until(|h| h.phase == Phase::Deploying)?;
        host.input(Action::Join {})?;
        host.until(|h| h.received.is_some())?;
        if mode == Mode::CloseBeforeReady {
            host.close()?;
        } else {
            let publication_id = host.received.as_ref().unwrap().0.clone();
            host.input(Action::CombatReady { publication_id })?;
            host.until(|h| h.active_seen)?;
            // Scripted player intent; Rust chooses death semantics and frozen settlement amounts.
            host.input(Action::Leave {})?;
        }
        host.until(|h| h.closed_events == 1 && h.pending.is_empty())?;
    }
    host.close()?;
    host.pump()?;
    check(
        host.phase == Phase::Closed && host.closed_events == 1 && host.pending.is_empty(),
        "match closed with unresolved obligations",
    )?;
    println!(
        "{}",
        serde_json::json!({
            "mode": mode_name, "clock": "ordinary", "configId": config.id(),
            "configHash": config.content_hash(), "initial": initial, "account": host.store.account,
            "counts": host.store.counts, "activeSeen": host.active_seen,
        "operations": host.store.journal(),
            "closedEvents": host.closed_events, "settlementEvents": host.settlement_events,
            "closedWithPending": host.closed_with_pending, "pendingOperations": host.pending.len(),
            "failedPublications": host.failed_publications, "unknownInjected": host.unknown_injected,
            "liveWorlds": 0, "phase": "closed"
        })
    );
    drop(runtime);
    Ok(())
}
