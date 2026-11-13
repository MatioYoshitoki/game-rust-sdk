use super::*;
use sha2::{Digest, Sha256};

fn digest(value: &impl serde::Serialize) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("typed checkpoint"))
    )
}
pub(super) fn fingerprint(recovery: &Recovery) -> String {
    digest(recovery)
}

impl Engine {
    pub(super) fn recovery_checkpoint(&self) -> IoCheckpoint {
        let state = IoState {
            schema_version: 1,
            identity_sha256: digest(self.create.as_ref().expect("created")),
            logical_ms: self.now,
            input_sequence: self.input_sequence,
            output_sequence: self.output_sequence,
            recovery_generation: self.recovery_generation,
            operations: self
                .operations
                .values()
                .map(|op| FrozenOperation {
                    request: op.request.clone(),
                    terminal: op.terminal.clone(),
                    refused: op.refused,
                    queried_attempt: op.queried_attempt,
                })
                .collect(),
        };
        IoCheckpoint {
            sha256: digest(&state),
            state,
        }
    }

    pub(super) fn bootstrap_recovery(
        &mut self,
        create: Create,
        recovery: Recovery,
    ) -> Result<Status> {
        let state = &recovery.checkpoint.state;
        if state.schema_version != 1
            || recovery.checkpoint.sha256 != digest(state)
            || state.identity_sha256 != digest(&create)
            || recovery.generation <= state.recovery_generation
            || state.operations.is_empty()
            || state.operations.len() > 24
            || state.logical_ms >= (1_u64 << 53)
            || state.output_sequence >= (1_u64 << 53)
        {
            return Err(Error::InvalidMessage);
        }
        let mut next = Engine::with_catalog(self.config.clone(), self.catalog.clone());
        next.ledger_only = self.ledger_only;
        next.connection = Some(create.connection.clone());
        next.create = Some(create);
        next.phase = Phase::Closed;
        next.now = state.logical_ms;
        next.io_ms = self.io_ms;
        next.input_sequence = state.input_sequence;
        next.output_sequence = state.output_sequence;
        next.recovery_generation = recovery.generation;
        next.recovery_fingerprint = Some(fingerprint(&recovery));
        for op in &state.operations {
            let intent = next.config.decode_business(&op.request.body)?;
            if op.request.attempt_id == 0
                || op.request.attempt_id > self.config.params.runtime.max_attempts
                || !valid_id(&op.request.operation_id)
                || next.operations.contains_key(&op.request.operation_id)
                || (op.refused
                    && (op.terminal.is_some()
                        || !matches!(
                            intent,
                            Business::CommitSettlement { .. }
                                | Business::CompensateAdmission { .. }
                                | Business::AppendFacts { .. }
                        )))
            {
                return Err(Error::InvalidMessage);
            }
            next.operations.insert(
                op.request.operation_id.clone(),
                Operation {
                    request: op.request.clone(),
                    intent,
                    terminal: op.terminal.clone(),
                    refused: op.refused,
                    queried_attempt: op.queried_attempt,
                    deadline_ms: None,
                },
            );
        }
        next.validate_recovered_operations()?;
        // A checkpoint contains only immutable I/O facts. Never construct a World from it.
        let old_roots: Vec<_> = next
            .operations
            .iter()
            .filter(|(_, op)| {
                op.terminal.is_none()
                    && (matches!(op.intent, Business::LoadParticipant {})
                        || op.intent.durable_write_label().is_some())
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &old_roots {
            if matches!(next.operations[id].intent, Business::LoadParticipant {}) {
                next.complete_operation(id, ReceiptOutcome::Unknown {});
            }
        }
        // Clean up a linked read/operator request if its root already has authoritative proof.
        let linked: Vec<_> = next
            .operations
            .iter()
            .filter_map(|(id, op)| {
                if op.terminal.is_some() {
                    return None;
                }
                let (root, manual) = match &op.intent {
                    Business::QueryOperation { operation_id } => (operation_id, false),
                    Business::ReconcileOperation { operation_id } => (operation_id, true),
                    _ => return None,
                };
                next.operations[root].terminal.as_ref().map(|proof| {
                    let proof = match (manual, proof) {
                        (
                            true,
                            ReceiptOutcome::Committed {
                                revision,
                                commit_id,
                            },
                        ) => ReceiptOutcome::Reconciled {
                            revision: *revision,
                            commit_id: commit_id.clone(),
                        },
                        (true, ReceiptOutcome::FactsAppended { .. }) => proof.clone(),
                        (true, _) => ReceiptOutcome::NotCommitted {},
                        (_, proof) => proof.clone(),
                    };
                    (id.clone(), proof)
                })
            })
            .collect();
        for (id, proof) in linked {
            next.complete_operation(&id, proof);
        }

        let compensating_entry = !next.operations.contains_key(&next.operation_id("settle"))
            && matches!(
                next.operations
                    .get(&next.operation_id("admit"))
                    .and_then(|op| op.terminal.as_ref()),
                Some(ReceiptOutcome::Committed { .. })
            );
        if compensating_entry {
            next.compensate_admission();
        }
        for id in old_roots {
            if next.operations[&id].terminal.is_some() {
                continue;
            }
            let manual = next
                .operations
                .iter()
                .find(|(_, op)| {
                    matches!(&op.intent,
                Business::ReconcileOperation { operation_id } if operation_id == &id)
                })
                .map(|(key, _)| key.clone());
            if let Some(manual) = manual {
                if next.operations[&manual].terminal.is_none() {
                    next.emit(OutputBody::BusinessRequest {
                        request: next.operations[&manual].request.clone(),
                    });
                }
                continue;
            }
            if next.operations[&id].refused {
                next.require_reconciliation(&id, Error::ExternalRejected);
                continue;
            }
            let Some(label) = next.operations[&id].intent.durable_write_label() else {
                continue;
            };
            let query_id = next.operation_id(&format!(
                "query-{label}-{}",
                next.operations[&id].request.attempt_id
            ));
            let attempt = next.operations[&id].request.attempt_id;
            if let Some(query) = next.operations.get_mut(&query_id) {
                query.terminal = None;
                query.deadline_ms = Some(next.io_ms + self.config.params.runtime.io_timeout_ms);
                query.queried_attempt = Some(attempt);
                let request = query.request.clone();
                next.emit(OutputBody::BusinessRequest { request });
            } else {
                next.unknown_operation(&id)?;
            }
        }
        next.event(Event::RecoveryRestored {
            generation: recovery.generation,
            compensating_entry,
        });
        next.finish_close();
        let status = next.status();
        *self = next;
        Ok(status)
    }

    fn validate_recovered_operations(&self) -> Result<()> {
        let admit_id = self.operation_id("admit");
        let admit = self.operations.get(&admit_id);
        let admit_revision = admit.and_then(|op| match op.terminal.as_ref() {
            Some(ReceiptOutcome::Committed { revision, .. }) => Some(*revision),
            _ => None,
        });
        for (id, op) in &self.operations {
            let expected_revision = match &op.intent {
                Business::LoadParticipant {} => {
                    if id != &self.operation_id("load") {
                        return Err(Error::InvalidMessage);
                    }
                    if let Some(
                        proof @ (ReceiptOutcome::Loaded { .. }
                        | ReceiptOutcome::AssetsLoaded { .. }),
                    ) = &op.terminal
                    {
                        let facts = self
                            .config
                            .loaded_facts(proof)
                            .map_err(|_| Error::InvalidMessage)?;
                        if facts.principal != self.create.as_ref().unwrap().authenticated_principal
                            || facts.revision == u32::MAX
                        {
                            return Err(Error::InvalidMessage);
                        }
                    }
                    None
                }
                Business::CommitAdmission {
                    expected_revision,
                    item_id,
                    quantity,
                    entry_years,
                    clock,
                } => {
                    let expected_clock = self
                        .loaded_assets()
                        .map(|facts| self.config.progression_entry_clock(facts))
                        .transpose()
                        .map_err(|_| Error::InvalidMessage)?
                        .flatten();
                    if id != &admit_id
                        || item_id != &self.config.item_id
                        || *quantity != self.config.carried_quantity
                        || *entry_years
                            != expected_clock
                                .as_ref()
                                .and_then(EntryClock::charged)
                                .unwrap_or(self.config.params.entry_years)
                        || clock != &expected_clock
                    {
                        return Err(Error::InvalidMessage);
                    }
                    Some(*expected_revision)
                }
                Business::CommitSettlement {
                    outcome,
                    expected_revision,
                    item_id,
                    carried,
                    looted,
                    kept,
                    dropped,
                    cultivation,
                    scoring,
                    map_progress,
                    reset_progress,
                    items,
                    instances,
                    rewards,
                    progression,
                } => {
                    if matches!(outcome, Outcome::Win | Outcome::Loss | Outcome::Draw)
                        && self.config.match_mode.is_none()
                    {
                        return Err(Error::InvalidMessage);
                    }
                    let policy = self.config.settlement_policy_outcome(*outcome);
                    if !self.config.valid_reset_progress(
                        reset_progress.as_ref(),
                        scoring.as_ref(),
                        map_progress.as_ref(),
                        self.now,
                    ) {
                        return Err(Error::InvalidMessage);
                    }
                    if !self
                        .config
                        .valid_map_progress(map_progress.as_ref(), scoring.as_ref())
                    {
                        return Err(Error::InvalidMessage);
                    }
                    let consumed = |id: &str| {
                        map_progress
                            .as_ref()
                            .and_then(|s| s.consumed.iter().find(|r| r.item_id == id))
                            .map_or(0, |r| r.quantity)
                    };
                    let total = carried
                        .checked_add(*looted)
                        .and_then(|q| q.checked_sub(consumed(item_id)))
                        .ok_or(Error::InvalidMessage)?;
                    let frozen_kills = if self.config.scoring_enabled() {
                        let s = scoring
                            .as_ref()
                            .filter(|s| self.config.valid_scoring(s, self.now))
                            .ok_or(Error::InvalidMessage)?;
                        if self
                            .config
                            .cultivation_from_score(u64::from(s.kill_score), *outcome)
                            != *cultivation
                        {
                            return Err(Error::InvalidMessage);
                        }
                        s.credited_kills
                    } else {
                        if scoring.is_some() {
                            return Err(Error::InvalidMessage);
                        }
                        (0..=self.config.max_kills(self.now))
                            .rfind(|&k| self.config.cultivation(k, *outcome) == *cultivation)
                            .ok_or(Error::InvalidMessage)?
                    };
                    if self.settled_reward_balances(&op.intent).as_ref() != Some(rewards) {
                        return Err(Error::InvalidMessage);
                    }
                    if self.config.settlement_progression.is_some() {
                        let facts = self.loaded_assets().ok_or(Error::InvalidMessage)?;
                        if self.config.plan_progression(&op.intent, facts).as_ref()
                            != progression.as_ref()
                        {
                            return Err(Error::InvalidMessage);
                        }
                    } else if progression.is_some() {
                        return Err(Error::InvalidMessage);
                    }
                    let loot_bound = |item: &str| {
                        if self.ledger_only {
                            // A member can pick a public drop produced by the other member.
                            // The coordinator additionally validates aggregate two-member bounds.
                            return self.config.pve_loot_bound(
                                item,
                                self.config.max_kills(self.now),
                                self.now,
                            );
                        }
                        let bound = scoring.as_ref().map_or_else(
                            || self.config.pve_loot_bound(item, frozen_kills, self.now),
                            |s| self.config.scored_loot_bound(item, s, self.now),
                        );
                        if map_progress.as_ref().is_some_and(|s| !s.reward_opened) {
                            bound.saturating_sub(self.config.objective_loot_bound(item))
                        } else {
                            bound
                        }
                    };
                    if self.config.pve.is_some() && *looted > loot_bound(item_id) {
                        return Err(Error::InvalidMessage);
                    }
                    let mut item_ids = std::collections::BTreeSet::new();
                    for item in items {
                        let Some(pve) = self.config.pve.as_ref() else {
                            return Err(Error::InvalidMessage);
                        };
                        if !item_ids.insert(&item.item_id)
                            || item.item_id == self.config.item_id
                            || !pve.items.contains_key(&item.item_id)
                            || self.config.instance_definition(&item.item_id).is_some()
                            || item.looted > loot_bound(&item.item_id)
                            || item.looted == 0
                            || item
                                .kept
                                .checked_add(item.dropped)
                                .and_then(|q| q.checked_add(consumed(&item.item_id)))
                                != Some(item.looted)
                            || (policy == Outcome::Extraction && item.dropped != 0)
                            || item.dropped > item.looted
                        {
                            return Err(Error::InvalidMessage);
                        }
                    }
                    if map_progress.as_ref().is_some_and(|s| {
                        s.consumed.iter().any(|row| {
                            row.item_id != self.config.item_id
                                && !items.iter().any(|i| i.item_id == row.item_id)
                        })
                    }) {
                        return Err(Error::InvalidMessage);
                    }
                    let quantities = std::iter::once((item_id.as_str(), total, *dropped))
                        .chain(items.iter().map(|item| {
                            (
                                item.item_id.as_str(),
                                item.looted - consumed(&item.item_id),
                                item.dropped,
                            )
                        }))
                        .collect::<Vec<_>>();
                    if !instances.is_empty() && !self.ledger_only {
                        return Err(Error::InvalidMessage);
                    }
                    let mut instance_counts = std::collections::BTreeMap::new();
                    for instance in instances {
                        *instance_counts
                            .entry(instance.item_id.as_str())
                            .or_insert(0u32) += 1;
                    }
                    if instance_counts.iter().any(|(id, &n)| {
                        n > loot_bound(id)
                            || *outcome != Outcome::Extraction && self.config.contested_reward(id)
                    }) {
                        return Err(Error::InvalidMessage);
                    }
                    let slots =
                        quantities
                            .iter()
                            .try_fold(instances.len() as u32, |sum, (id, q, _)| {
                                self.config
                                    .item_limit(id)
                                    .and_then(|limit| sum.checked_add(q.div_ceil(limit)))
                            });
                    let loss_valid = if self.config.death_relics().is_some() {
                        quantities.iter().all(|row| row.2 == 0)
                    } else {
                        policy != Outcome::Death || self.config.valid_death_loss(&quantities)
                    };
                    if slots.is_none_or(|n| n > self.config.inventory_capacity) || !loss_valid {
                        return Err(Error::InvalidMessage);
                    }
                    if id != &self.operation_id("settle")
                        || item_id != &self.config.item_id
                        || *carried != self.config.carried_quantity
                        || if self.config.pve.is_some() {
                            *looted > loot_bound(item_id)
                        } else {
                            ![0, self.config.target.drop_quantity].contains(looted)
                        }
                        || *dropped > total
                        || (policy == Outcome::Extraction && *dropped != 0)
                        || kept.checked_add(*dropped) != Some(total)
                        || (*looted > 0 && frozen_kills == 0 && self.config.pve.is_none())
                        || admit_revision != Some(*expected_revision)
                    {
                        return Err(Error::InvalidMessage);
                    }
                    Some(*expected_revision)
                }
                Business::CompensateAdmission {
                    original_operation_id,
                    item_id,
                    quantity,
                    entry_years,
                    clock,
                } => {
                    let original = admit.and_then(|op| match &op.intent {
                        Business::CommitAdmission {
                            entry_years, clock, ..
                        } => Some((*entry_years, clock.as_ref().map(EntryClock::reversed))),
                        _ => None,
                    });
                    if id != &self.operation_id("compensate-admit")
                        || original_operation_id != &admit_id
                        || admit.is_none()
                        || item_id != &self.config.item_id
                        || *quantity != self.config.carried_quantity
                        || original
                            .as_ref()
                            .is_none_or(|(cost, reversed)| cost != entry_years || reversed != clock)
                    {
                        return Err(Error::InvalidMessage);
                    }
                    admit_revision
                }
                Business::AppendFacts { .. } => {
                    if id != &self.operation_id("facts")
                        || op.terminal.as_ref().is_some_and(|proof| {
                            !matches!(proof, ReceiptOutcome::FactsAppended { .. })
                        })
                    {
                        return Err(Error::InvalidMessage);
                    }
                    None
                }
                Business::QueryOperation { operation_id }
                | Business::ReconcileOperation { operation_id } => {
                    let root = self
                        .operations
                        .get(operation_id)
                        .ok_or(Error::InvalidMessage)?;
                    let suffix = root
                        .intent
                        .durable_write_label()
                        .ok_or(Error::InvalidMessage)?;
                    let query = matches!(op.intent, Business::QueryOperation { .. });
                    let valid = if query {
                        op.queried_attempt.is_some_and(|attempt| {
                            attempt <= root.request.attempt_id
                                && id == &self.operation_id(&format!("query-{suffix}-{attempt}"))
                        })
                    } else {
                        id == &self.operation_id(&format!("reconcile-{suffix}"))
                    };
                    if !valid
                        || (query
                            && !op
                                .queried_attempt
                                .is_some_and(|value| (1..=3).contains(&value)))
                        || (!query && op.queried_attempt.is_some())
                        || op.request.attempt_id != 1
                    {
                        return Err(Error::InvalidMessage);
                    }
                    None
                }
                Business::LoadAssets {} | Business::CommitAssets { .. } => {
                    return Err(Error::InvalidMessage);
                }
            };
            if !matches!(op.intent, Business::QueryOperation { .. }) && op.queried_attempt.is_some()
            {
                return Err(Error::InvalidMessage);
            }
            if let Some(proof) = &op.terminal {
                let fact_root = match &op.intent {
                    Business::AppendFacts { .. } => Some(op),
                    Business::QueryOperation { operation_id }
                    | Business::ReconcileOperation { operation_id } => self
                        .operations
                        .get(operation_id)
                        .filter(|root| matches!(root.intent, Business::AppendFacts { .. })),
                    _ => None,
                };
                match proof {
                    ReceiptOutcome::Committed {
                        revision,
                        commit_id,
                    } => {
                        if !valid_id(commit_id)
                            || fact_root.is_some()
                            || (matches!(op.intent, Business::CompensateAdmission { .. })
                                && admit_revision.is_none())
                            || expected_revision
                                .is_some_and(|expected| expected.checked_add(1) != Some(*revision))
                            || matches!(
                                op.intent,
                                Business::LoadParticipant {} | Business::ReconcileOperation { .. }
                            )
                        {
                            return Err(Error::InvalidMessage);
                        }
                    }
                    ReceiptOutcome::Reconciled { commit_id, .. } => {
                        if !valid_id(commit_id)
                            || fact_root.is_some()
                            || !matches!(op.intent, Business::ReconcileOperation { .. })
                        {
                            return Err(Error::InvalidMessage);
                        }
                    }
                    ReceiptOutcome::FactsAppended { commit_id } => {
                        if !valid_id(commit_id)
                            || fact_root.and_then(|root| root.terminal.as_ref()) != Some(proof)
                        {
                            return Err(Error::InvalidMessage);
                        }
                    }
                    ReceiptOutcome::Loaded { .. }
                    | ReceiptOutcome::AssetsLoaded { .. }
                    | ReceiptOutcome::Unknown {} => {
                        if !matches!(op.intent, Business::LoadParticipant {}) {
                            return Err(Error::InvalidMessage);
                        }
                    }
                    ReceiptOutcome::Refused {} | ReceiptOutcome::NotCommitted {} => {}
                }
            }
        }
        if !self.operations.contains_key(&self.operation_id("load")) {
            return Err(Error::InvalidMessage);
        }
        let settle_id = self.operation_id("settle");
        let expected_facts = self
            .operations
            .get(&settle_id)
            .filter(|op| matches!(op.terminal, Some(ReceiptOutcome::Committed { .. })))
            .and_then(|op| self.config.settlement_fact_intent(&settle_id, &op.intent));
        let actual_facts = self
            .operations
            .get(&self.operation_id("facts"))
            .map(|op| &op.intent);
        if expected_facts.as_ref() != actual_facts {
            return Err(Error::InvalidMessage);
        }
        Ok(())
    }
}
