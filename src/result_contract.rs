//! Result identity is a configured binding of a registered terminal trigger to a settlement policy.
//! It never grants the host or client authority to select or submit a terminal result.
use crate::{config::Config, protocol::*};
use serde::{Deserialize, Serialize};
#[cfg(all(test, match_sdk_test_data))]
#[path = "result_contract_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResultContract {
    pub extraction: MatchResult,
    pub death: MatchResult,
}

impl ResultSettlement {
    pub(crate) fn from_asset(summary: AssetSettlement, result: MatchResult) -> Self {
        Self {
            result,
            inventory: summary.inventory,
            instances: summary.instances,
            rewards: summary.rewards,
            progression: summary.progression,
            scoring: summary.scoring,
            map_progress: summary.map_progress,
            reset_progress: summary.reset_progress,
        }
    }
    pub(crate) fn to_asset(&self, trigger: Outcome) -> AssetSettlement {
        AssetSettlement {
            outcome: trigger,
            inventory: self.inventory.clone(),
            instances: self.instances.clone(),
            rewards: self.rewards.clone(),
            progression: self.progression.clone(),
            scoring: self.scoring.clone(),
            map_progress: self.map_progress.clone(),
            reset_progress: self.reset_progress.clone(),
        }
    }
}

impl Config {
    pub(crate) fn result_binding(&self, trigger: Outcome) -> Option<&MatchResult> {
        match trigger {
            Outcome::Extraction => Some(&self.result_contract.as_ref()?.extraction),
            Outcome::Death => Some(&self.result_contract.as_ref()?.death),
            Outcome::Win => Some(&self.match_mode.as_ref()?.results.win),
            Outcome::Loss => Some(&self.match_mode.as_ref()?.results.loss),
            Outcome::Draw => Some(&self.match_mode.as_ref()?.results.draw),
        }
    }

    pub(crate) fn result_bindings(&self) -> impl Iterator<Item = (Outcome, &MatchResult)> {
        [
            Outcome::Extraction,
            Outcome::Death,
            Outcome::Win,
            Outcome::Loss,
            Outcome::Draw,
        ]
        .into_iter()
        .filter_map(|outcome| Some((outcome, self.result_binding(outcome)?)))
    }

    pub(crate) fn result_trigger(&self, result: &MatchResult) -> Result<Outcome> {
        self.result_bindings()
            .find_map(|(outcome, binding)| (binding == result).then_some(outcome))
            .ok_or(Error::InvalidMessage)
    }

    // Adapt the two registered rule bundles to the existing, tested inventory and score engines.
    // The stored gameplay intent retains its original trigger; policy is never inferred from it.
    pub(crate) fn settlement_policy_outcome(&self, trigger: Outcome) -> Outcome {
        self.result_binding(trigger)
            .map_or(trigger, |result| match result.policy {
                SettlementPolicy::Preserve => Outcome::Extraction,
                SettlementPolicy::Attrition => Outcome::Death,
            })
    }
}
