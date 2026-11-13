//! Versioned account boundary. Existing gameplay rules keep their internal accounting intent;
//! exactly one frozen wire request is journaled, queried, retried and recovered.
use crate::{config::Config, protocol::*};
use serde::{Deserialize, Serialize};
#[cfg(all(test, match_sdk_test_data))]
#[path = "asset_contract_tests.rs"]
pub(crate) mod tests;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssetContract {
    pub entry_cost_asset_id: String,
    pub reward_asset_id: String,
}

fn amount(asset_id: &str, quantity: u32) -> AssetAmount {
    AssetAmount {
        asset_id: asset_id.into(),
        quantity,
    }
}
fn canonical(mut rows: Vec<AssetAmount>) -> Vec<AssetAmount> {
    rows.retain(|r| r.quantity != 0);
    rows.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    rows
}

impl Config {
    pub fn protocol_version(&self) -> u32 {
        if self.replication.is_some() {
            6
        } else if self.members.is_some() {
            5
        } else if self.two_members.is_some() {
            4
        } else if self.result_contract.is_some() {
            3
        } else if self.asset_contract.is_some() {
            2
        } else {
            1
        }
    }

    pub(crate) fn encode_business(&self, intent: &Business) -> Business {
        let Some(binding) = &self.asset_contract else {
            return intent.clone();
        };
        let transaction = match intent {
            Business::LoadParticipant {} => return Business::LoadAssets {},
            Business::CommitAdmission {
                expected_revision,
                item_id,
                quantity,
                entry_years,
                clock,
            } => AssetTransaction::Admission {
                expected_revision: *expected_revision,
                debits: canonical(vec![
                    amount(item_id, *quantity),
                    amount(&binding.entry_cost_asset_id, *entry_years),
                ]),
                clock: clock.clone(),
            },
            Business::CompensateAdmission {
                original_operation_id,
                item_id,
                quantity,
                entry_years,
                clock,
            } => AssetTransaction::Compensation {
                original_operation_id: original_operation_id.clone(),
                credits: canonical(vec![
                    amount(item_id, *quantity),
                    amount(&binding.entry_cost_asset_id, *entry_years),
                ]),
                clock: clock.clone(),
            },
            Business::CommitSettlement {
                expected_revision,
                outcome,
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
                let stored = |id: &str, kept| {
                    progression
                        .as_ref()
                        .and_then(|p| p.storage.iter().find(|r| r.asset_id == id))
                        .map_or(kept, |row| row.stored)
                };
                let mut credits = vec![
                    amount(item_id, stored(item_id, *kept)),
                    amount(&binding.reward_asset_id, *cultivation),
                ];
                let mut inventory = vec![AssetInventory {
                    item_id: item_id.clone(),
                    carried: *carried,
                    looted: *looted,
                    kept: *kept,
                    dropped: *dropped,
                }];
                for item in items {
                    credits.push(amount(&item.item_id, stored(&item.item_id, item.kept)));
                    inventory.push(AssetInventory {
                        item_id: item.item_id.clone(),
                        carried: 0,
                        looted: item.looted,
                        kept: item.kept,
                        dropped: item.dropped,
                    });
                }
                credits.extend(rewards.iter().map(|row| amount(&row.asset_id, row.awarded)));
                inventory.sort_by(|a, b| a.item_id.cmp(&b.item_id));
                AssetTransaction::Settlement {
                    expected_revision: *expected_revision,
                    credits: canonical(credits),
                    summary: AssetSettlement {
                        outcome: *outcome,
                        inventory,
                        instances: instances.clone(),
                        rewards: rewards.clone(),
                        progression: progression.clone(),
                        scoring: scoring.clone(),
                        map_progress: map_progress.clone(),
                        reset_progress: reset_progress.clone(),
                    },
                }
            }
            _ => return intent.clone(),
        };
        let transaction = match transaction {
            AssetTransaction::Settlement {
                expected_revision,
                credits,
                summary,
            } if self.result_contract.is_some() => {
                let result = self
                    .result_binding(summary.outcome)
                    .expect("validated result contract")
                    .clone();
                AssetTransaction::ResultSettlement {
                    expected_revision,
                    credits,
                    summary: ResultSettlement::from_asset(summary, result),
                }
            }
            other => other,
        };
        Business::CommitAssets { transaction }
    }

    /// Decode only canonical configured wire intents; deep gameplay proofs are then checked by
    /// the shared recovery validator. No alternate or partially applied accounting is accepted.
    pub(crate) fn decode_business(&self, body: &Business) -> Result<Business> {
        let Some(_) = &self.asset_contract else {
            return match body {
                Business::LoadAssets {} | Business::CommitAssets { .. } => {
                    Err(Error::InvalidMessage)
                }
                _ => Ok(body.clone()),
            };
        };
        let intent = match body {
            Business::LoadAssets {} => Business::LoadParticipant {},
            Business::CommitAssets { transaction } => match transaction {
                AssetTransaction::Admission {
                    expected_revision,
                    clock,
                    ..
                } => Business::CommitAdmission {
                    expected_revision: *expected_revision,
                    item_id: self.item_id.clone(),
                    quantity: self.carried_quantity,
                    entry_years: self
                        .wire_entry_charge(
                            self.allows_partial_entry()
                                .then(|| clock.as_ref().and_then(EntryClock::charged))
                                .flatten(),
                        )
                        .ok_or(Error::InvalidMessage)?,
                    clock: clock.clone(),
                },
                AssetTransaction::Compensation {
                    original_operation_id,
                    clock,
                    ..
                } => Business::CompensateAdmission {
                    original_operation_id: original_operation_id.clone(),
                    item_id: self.item_id.clone(),
                    quantity: self.carried_quantity,
                    entry_years: self
                        .wire_entry_charge(
                            self.allows_partial_entry()
                                .then(|| {
                                    let row = &clock.as_ref()?.remaining;
                                    row.after.checked_sub(row.before)
                                })
                                .flatten(),
                        )
                        .ok_or(Error::InvalidMessage)?,
                    clock: clock.clone(),
                },
                AssetTransaction::Settlement {
                    expected_revision,
                    credits,
                    summary,
                } => self.decode_asset_settlement(*expected_revision, credits, summary)?,
                AssetTransaction::ResultSettlement {
                    expected_revision,
                    credits,
                    summary,
                } => {
                    let trigger = self.result_trigger(&summary.result)?;
                    self.decode_asset_settlement(
                        *expected_revision,
                        credits,
                        &summary.to_asset(trigger),
                    )?
                }
            },
            Business::QueryOperation { .. } | Business::ReconcileOperation { .. } => body.clone(),
            Business::AppendFacts { .. } if !self.settlement_facts.is_empty() => body.clone(),
            _ => return Err(Error::InvalidMessage),
        };
        if self.encode_business(&intent) != *body {
            return Err(Error::InvalidMessage);
        }
        Ok(intent)
    }

    fn decode_asset_settlement(
        &self,
        expected_revision: u32,
        credits: &[AssetAmount],
        summary: &AssetSettlement,
    ) -> Result<Business> {
        let binding = self.asset_contract.as_ref().ok_or(Error::InvalidMessage)?;
        if summary.inventory.is_empty()
            || summary.inventory.len() > 65
            || credits.len() > 65
            || summary.rewards.len() > self.settlement_rewards.len()
            || summary.rewards.iter().any(|row| {
                row.adjustment
                    .as_ref()
                    .is_some_and(|plan| plan.changes.len() > 16)
            })
            || summary.progression.is_some() != self.settlement_progression.is_some()
            || summary.progression.as_ref().is_some_and(|p| {
                let shared = self
                    .settlement_progression
                    .as_ref()
                    .is_some_and(|rule| rule.warehouse_slots.is_some());
                !self.valid_damage_growth(&p.damage)
                    || p.reward_weapon_instance_id.as_ref().is_some_and(|id| {
                        !self.has_equipped_weapon_rewards()
                            || !crate::config::weapon_equipment::weapon_id(id)
                    })
                    || p.warehouse_slots.is_some() != shared
                    || p.storage.len() > if shared { 65 } else { 17 }
                    || p.instance_storage.as_ref().is_some_and(|row| {
                        row.stored_ids.len().saturating_add(row.overflow_ids.len())
                            != summary.instances.len()
                    })
                    || !self.valid_conversion_shape(p)
                    || self.wire_entry_charge(p.entry_charged).is_none()
            })
            || summary.instances.len() > self.inventory_capacity as usize
            || !summary
                .instances
                .windows(2)
                .all(|rows| rows[0].instance_id < rows[1].instance_id)
            || summary.instances.iter().any(|item| {
                !valid_id(&item.instance_id)
                    || self
                        .instance_definition(&item.item_id)
                        .is_none_or(|definition| definition.quality != item.quality)
            })
            || !summary
                .inventory
                .windows(2)
                .all(|w| w[0].item_id < w[1].item_id)
        {
            return Err(Error::InvalidMessage);
        }
        if !self.settlement_rewards.is_empty()
            && (summary
                .scoring
                .as_ref()
                .is_none_or(|s| s.templates.len() > 17)
                || self
                    .reward_plan_with_damage(
                        summary.outcome,
                        summary.scoring.as_ref(),
                        summary.progression.as_ref(),
                        |id| {
                            summary
                                .rewards
                                .iter()
                                .find(|row| row.asset_id == id)
                                .map(|row| row.balance_before)
                                .or_else(|| {
                                    summary
                                        .rewards
                                        .iter()
                                        .filter_map(|row| row.adjustment.as_deref())
                                        .flat_map(|plan| &plan.changes)
                                        .find(|change| change.asset_id == id)
                                        .map(|change| change.before)
                                })
                                .unwrap_or(0)
                        },
                    )
                    .as_ref()
                    != Some(&summary.rewards))
        {
            return Err(Error::InvalidMessage);
        }
        let primary = summary
            .inventory
            .iter()
            .find(|r| r.item_id == self.item_id)
            .ok_or(Error::InvalidMessage)?;
        let mut items = Vec::new();
        for row in summary
            .inventory
            .iter()
            .filter(|r| r.item_id != self.item_id)
        {
            if row.carried != 0 {
                return Err(Error::InvalidMessage);
            }
            items.push(ItemSettlement {
                item_id: row.item_id.clone(),
                looted: row.looted,
                kept: row.kept,
                dropped: row.dropped,
            });
        }
        if !self.valid_stack_flows(
            summary.outcome,
            primary.looted,
            &items,
            summary
                .progression
                .as_ref()
                .and_then(|p| p.stack_flows.as_deref()),
        ) {
            return Err(Error::InvalidMessage);
        }
        Ok(Business::CommitSettlement {
            expected_revision,
            outcome: summary.outcome,
            item_id: primary.item_id.clone(),
            carried: primary.carried,
            looted: primary.looted,
            kept: primary.kept,
            dropped: primary.dropped,
            cultivation: if self.settlement_rewards.is_empty() {
                credits
                    .iter()
                    .find(|r| r.asset_id == binding.reward_asset_id)
                    .map_or(0, |r| r.quantity)
            } else {
                0
            },
            items,
            instances: summary.instances.clone(),
            rewards: summary.rewards.clone(),
            progression: summary.progression.clone(),
            scoring: summary.scoring.clone(),
            map_progress: summary.map_progress.clone(),
            reset_progress: summary.reset_progress.clone(),
        })
    }

    pub(crate) fn loaded_facts(&self, outcome: &ReceiptOutcome) -> Result<ParticipantFacts> {
        match (self.asset_contract.as_ref(), outcome) {
            (None, ReceiptOutcome::Loaded { facts }) => Ok(facts.clone()),
            (Some(binding), ReceiptOutcome::AssetsLoaded { facts }) => {
                if facts.assets.len() > 128
                    || !facts
                        .assets
                        .windows(2)
                        .all(|w| w[0].asset_id < w[1].asset_id)
                    || facts.assets.iter().any(|r| {
                        r.asset_id.is_empty()
                            || r.asset_id.len() > 64
                            || r.asset_id.chars().any(char::is_control)
                    })
                {
                    return Err(Error::InvalidMessage);
                }
                let balance = |id: &str| {
                    facts
                        .assets
                        .iter()
                        .find(|r| r.asset_id == id)
                        .map(|r| r.quantity)
                };
                let cost = self.entry_charge(balance(&binding.entry_cost_asset_id).unwrap_or(0))?;
                if self.loaded_earning_rates(facts).is_none()
                    || !self.valid_reward_adjustment_baseline(facts)
                {
                    return Err(Error::InvalidMessage);
                }
                self.progression_entry_clock(facts)?;
                Ok(ParticipantFacts {
                    principal: facts.principal.clone(),
                    revision: facts.revision,
                    hp_points: facts.hp_points,
                    spirit_points: facts.spirit_points,
                    atk_points: facts.atk_points,
                    def_points: facts.def_points,
                    spd_points: facts.spd_points,
                    warehouse_quantity: balance(&self.item_id)
                        .or((cost == 0 && self.carried_quantity == 0).then_some(0))
                        .ok_or(Error::NoResource)?,
                })
            }
            _ => Err(Error::InvalidMessage),
        }
    }
}
