use super::*;
use crate::asset_contract::AssetContract;
use crate::protocol::{BalanceChange, RewardAdjustment};

const BPS: u128 = 10_000;
pub(crate) const MAX_ADJUSTMENT_ASSETS: usize = 16;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AdjustmentRule {
    uses_asset_id: String,
    effect: AdjustmentEffect,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum AdjustmentEffect {
    Scale {
        bps: u32,
    },
    AddOriginal {
        bps: u32,
    },
    Defer {
        escrow_asset_id: String,
        cap: u32,
        divert_bps: u32,
        release_bps: u32,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        forfeit_results: BTreeSet<String>,
    },
}

impl AdjustmentRule {
    fn state_ids(&self) -> impl Iterator<Item = &str> {
        let escrow = match &self.effect {
            AdjustmentEffect::Defer {
                escrow_asset_id, ..
            } => Some(escrow_asset_id.as_str()),
            _ => None,
        };
        std::iter::once(self.uses_asset_id.as_str()).chain(escrow)
    }
}

fn scaled(quantity: u128, bps: u32) -> u128 {
    quantity * u128::from(bps) / BPS
}

impl RewardRule {
    pub(super) fn adjusted_reward(
        &self,
        original: u32,
        result: &str,
        balance: impl Fn(&str) -> u32,
    ) -> Option<(u32, Option<Box<RewardAdjustment>>)> {
        if original == 0 || self.adjustments.is_empty() {
            return Some((original, None));
        }
        let original_amount = u128::from(original);
        let mut immediate = original_amount;
        let mut payout = 0;
        let mut changes = Vec::new();
        for rule in &self.adjustments {
            let uses = balance(&rule.uses_asset_id);
            if uses == 0 {
                continue;
            }
            let mut uses_after = uses - 1;
            match &rule.effect {
                AdjustmentEffect::Scale { bps } => immediate = scaled(immediate, *bps),
                AdjustmentEffect::AddOriginal { bps } => immediate += scaled(original_amount, *bps),
                AdjustmentEffect::Defer {
                    escrow_asset_id,
                    cap,
                    divert_bps,
                    release_bps,
                    forfeit_results,
                } => {
                    let before = balance(escrow_asset_id);
                    let available = cap.checked_sub(before)?;
                    let diverted = scaled(immediate, *divert_bps).min(u128::from(available));
                    immediate -= diverted;
                    let mut after = before + diverted as u32;
                    if uses_after == 0 {
                        payout += scaled(u128::from(after), *release_bps);
                        after = 0;
                    } else if forfeit_results.contains(result) {
                        uses_after = 0;
                        after = 0;
                    }
                    changes.push(BalanceChange {
                        asset_id: escrow_asset_id.clone(),
                        before,
                        after,
                    });
                }
            }
            changes.push(BalanceChange {
                asset_id: rule.uses_asset_id.clone(),
                before: uses,
                after: uses_after,
            });
        }
        if changes.is_empty() {
            return Some((original, None));
        }
        changes.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        Some((
            u32::try_from(immediate + payout).ok()?,
            Some(Box::new(RewardAdjustment {
                original_requested: original,
                deferred_payout: u32::try_from(payout).ok()?,
                changes,
            })),
        ))
    }

    pub(super) fn valid_adjusted_bound(&self, original: u32) -> bool {
        if original == 0 {
            return true;
        }
        let original = u128::from(original);
        let mut immediate = original;
        let mut payout = 0;
        for rule in &self.adjustments {
            match rule.effect {
                AdjustmentEffect::Scale { bps } => {
                    immediate = immediate.max(scaled(immediate, bps))
                }
                AdjustmentEffect::AddOriginal { bps } => immediate += scaled(original, bps),
                AdjustmentEffect::Defer {
                    cap, release_bps, ..
                } => payout += scaled(u128::from(cap), release_bps),
            }
        }
        immediate + payout <= u128::from(u32::MAX)
    }
}

impl Config {
    pub(crate) fn reward_adjustment_assets(&self) -> impl Iterator<Item = &str> {
        self.settlement_rewards
            .values()
            .flat_map(|rule| &rule.adjustments)
            .flat_map(AdjustmentRule::state_ids)
    }

    pub(crate) fn valid_reward_adjustment_baseline(&self, facts: &AssetParticipantFacts) -> bool {
        let balance = |id| super::super::settlement_progression::balance(facts, id).unwrap_or(0);
        self.settlement_rewards
            .values()
            .flat_map(|rule| &rule.adjustments)
            .all(|rule| {
                let AdjustmentEffect::Defer {
                    escrow_asset_id,
                    cap,
                    ..
                } = &rule.effect
                else {
                    return true;
                };
                let escrow = balance(escrow_asset_id);
                escrow <= *cap && (escrow == 0 || balance(&rule.uses_asset_id) > 0)
            })
    }
}

pub(super) fn validate(
    c: &Config,
    assets: &AssetContract,
    pve: &PveConfig,
    rate_assets: &BTreeSet<&str>,
) -> std::result::Result<(), &'static str> {
    let error = "CONFIG_SETTLEMENT_REWARDS_ADJUSTMENTS";
    let mut state = BTreeSet::new();
    for id in c.reward_adjustment_assets() {
        if !state.insert(id)
            || state.len() + rate_assets.len() > MAX_ADJUSTMENT_ASSETS
            || !crate::protocol::valid_id(id)
            || id.len() > 64
            || id.starts_with(crate::protocol::SCOPED_REWARD_PREFIX)
            || id == c.item_id
            || id == assets.entry_cost_asset_id
            || id == assets.reward_asset_id
            || pve.items.contains_key(id)
            || c.settlement_rewards.contains_key(id)
            || rate_assets.contains(id)
        {
            return Err(error);
        }
    }
    if !state.is_empty() && (c.members.is_none() || c.settlement_progression.is_none()) {
        return Err(error);
    }
    for rule in c
        .settlement_rewards
        .values()
        .flat_map(|rule| &rule.adjustments)
    {
        let valid = match &rule.effect {
            AdjustmentEffect::Scale { bps } | AdjustmentEffect::AddOriginal { bps } => {
                *bps <= MAX_RATE_BPS
            }
            AdjustmentEffect::Defer {
                cap,
                divert_bps,
                release_bps,
                forfeit_results,
                ..
            } => {
                *cap > 0
                    && (1..=10_000).contains(divert_bps)
                    && *release_bps <= MAX_RATE_BPS
                    && forfeit_results.iter().all(|id| {
                        c.result_bindings()
                            .any(|(_, result)| result.result_id == *id)
                    })
            }
        };
        if !valid {
            return Err(error);
        }
    }
    Ok(())
}
