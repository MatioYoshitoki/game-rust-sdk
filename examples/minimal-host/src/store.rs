//! Process-local sample account adapter. Apply frozen amounts; never calculate rewards.
//! Production adapters need durable atomic transactions, fencing and authenticated ownership.
use crate::{HostResult, check};
use match_kernel::protocol::{Business, BusinessRequest, ParticipantFacts, ReceiptOutcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub participant_id: String,
    pub seed: u32,
    pub facts: ParticipantFacts,
    pub cultivation: u64,
    pub entry_years: u64,
    pub items: BTreeMap<String, u64>,
}

#[derive(Default, Serialize)]
pub struct Counts {
    pub loads: u32,
    pub admissions: u32,
    pub settlements: u32,
    pub compensations: u32,
    pub queries: u32,
}

pub struct Store {
    pub account: Account,
    pub counts: Counts,
    ledger: BTreeMap<String, (Business, ReceiptOutcome)>,
    pub refuse_admission: bool,
}

impl Store {
    pub fn journal(&self) -> &BTreeMap<String, (Business, ReceiptOutcome)> {
        &self.ledger
    }

    pub fn new(account: Account, refuse_admission: bool) -> Self {
        Self {
            account,
            counts: Counts::default(),
            ledger: BTreeMap::new(),
            refuse_admission,
        }
    }

    pub fn execute(&mut self, request: &BusinessRequest) -> HostResult<ReceiptOutcome> {
        if let Some((body, outcome)) = self.ledger.get(&request.operation_id) {
            check(
                body == &request.body,
                "operation ID reused with a different body",
            )?;
            return Ok(outcome.clone());
        }
        // Stage all balance changes. A validation/overflow failure cannot partly change the account.
        let mut account = self.account.clone();
        let result = match &request.body {
            Business::LoadAssets {} | Business::CommitAssets { .. } => {
                return Err("protocol 2 requires an asset adapter".into());
            }
            Business::AppendFacts { .. } => {
                return Err("settlement facts require a protocol 6 adapter".into());
            }
            Business::LoadParticipant {} => {
                self.counts.loads += 1;
                ReceiptOutcome::Loaded {
                    facts: self.account.facts.clone(),
                }
            }
            Business::CommitAdmission {
                expected_revision,
                quantity,
                entry_years,
                ..
            } => {
                if self.refuse_admission
                    || *expected_revision != account.facts.revision
                    || account.facts.warehouse_quantity < *quantity
                {
                    ReceiptOutcome::Refused {}
                } else {
                    account.facts.warehouse_quantity -= quantity;
                    account.entry_years = account
                        .entry_years
                        .checked_add(u64::from(*entry_years))
                        .ok_or("entry balance overflow")?;
                    let result = Self::commit(&mut account)?;
                    self.counts.admissions += 1;
                    result
                }
            }
            Business::CommitSettlement {
                expected_revision,
                kept,
                cultivation,
                items,
                ..
            } => {
                if *expected_revision != account.facts.revision {
                    ReceiptOutcome::Refused {}
                } else if let Some(warehouse) = account.facts.warehouse_quantity.checked_add(*kept)
                {
                    account.facts.warehouse_quantity = warehouse;
                    account.cultivation = account
                        .cultivation
                        .checked_add(u64::from(*cultivation))
                        .ok_or("cultivation balance overflow")?;
                    for item in items {
                        let quantity = account.items.entry(item.item_id.clone()).or_default();
                        *quantity = quantity
                            .checked_add(u64::from(item.kept))
                            .ok_or("item balance overflow")?;
                    }
                    let result = Self::commit(&mut account)?;
                    self.counts.settlements += 1;
                    result
                } else {
                    ReceiptOutcome::Refused {}
                }
            }
            Business::CompensateAdmission {
                original_operation_id,
                item_id,
                quantity,
                entry_years,
                ..
            } => {
                let original = self.ledger.get(original_operation_id);
                check(
                    matches!(original, Some((Business::CommitAdmission {
                        item_id: original_item, quantity: original_quantity,
                        entry_years: original_years, ..
                    }, ReceiptOutcome::Committed { revision, .. }))
                        if original_item == item_id && original_quantity == quantity
                            && original_years == entry_years
                            && *revision == account.facts.revision),
                    "compensation must match the committed admission",
                )?;
                let warehouse = account.facts.warehouse_quantity.checked_add(*quantity);
                check(warehouse.is_some(), "compensation inventory overflow")?;
                check(
                    account.entry_years >= u64::from(*entry_years),
                    "compensation entry balance is insufficient",
                )?;
                account.facts.warehouse_quantity = warehouse.unwrap();
                account.entry_years -= u64::from(*entry_years);
                let result = Self::commit(&mut account)?;
                self.counts.compensations += 1;
                result
            }
            Business::QueryOperation { operation_id } => {
                self.counts.queries += 1;
                self.ledger
                    .get(operation_id)
                    .map(|(_, outcome)| outcome.clone())
                    .unwrap_or(ReceiptOutcome::Unknown {})
            }
            // An absent journal record is not proof that a delayed write cannot commit.
            // This sample has no authorized reconciliation service.
            Business::ReconcileOperation { .. } => ReceiptOutcome::Unknown {},
        };
        if matches!(result, ReceiptOutcome::Committed { .. }) {
            self.account = account;
        }
        if !matches!(result, ReceiptOutcome::Unknown {}) {
            self.ledger.insert(
                request.operation_id.clone(),
                (request.body.clone(), result.clone()),
            );
        }
        Ok(result)
    }

    fn commit(account: &mut Account) -> HostResult<ReceiptOutcome> {
        account.facts.revision = account
            .facts
            .revision
            .checked_add(1)
            .ok_or("account revision overflow")?;
        Ok(ReceiptOutcome::Committed {
            revision: account.facts.revision,
            commit_id: format!("sample-tx-{}", account.facts.revision),
        })
    }
}
