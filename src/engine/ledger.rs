//! Financial-only use of the already verified admission/settlement/recovery state machine.
use super::*;

impl Engine {
    pub(crate) fn new_member_ledger(
        config: Config,
        catalog: std::sync::Arc<crate::ConfigCatalog>,
    ) -> Self {
        let mut next = Self::with_catalog(config, catalog);
        next.ledger_only = true;
        next
    }
    pub(crate) fn ledger_key(&self) -> MatchKey {
        self.key()
    }
    pub(crate) fn ledger_clock(&mut self, now: u64) {
        self.now = now;
    }
    pub(crate) fn ledger_admitted_facts(&self) -> Option<ParticipantFacts> {
        let mut facts = self
            .operations
            .get(&self.operation_id("load"))
            .and_then(|o| o.terminal.as_ref())
            .and_then(|proof| self.config.loaded_facts(proof).ok())?;
        let ReceiptOutcome::Committed { revision, .. } = self
            .operations
            .get(&self.operation_id("admit"))?
            .terminal
            .as_ref()?
        else {
            return None;
        };
        facts.revision = *revision;
        Some(facts)
    }
    pub(crate) fn ledger_activate(&mut self) {
        self.was_active = true;
    }
    pub(super) fn loaded_assets(&self) -> Option<&AssetParticipantFacts> {
        match self
            .operations
            .get(&self.operation_id("load"))?
            .terminal
            .as_ref()?
        {
            ReceiptOutcome::AssetsLoaded { facts } => Some(facts),
            _ => None,
        }
    }

    pub(crate) fn ledger_reward_rates(&self) -> Option<Vec<RewardEarningRate>> {
        self.config.loaded_earning_rates(self.loaded_assets()?)
    }

    pub(super) fn settled_reward_balances(&self, intent: &Business) -> Option<Vec<RewardBalance>> {
        let Business::CommitSettlement {
            outcome,
            scoring,
            progression,
            ..
        } = intent
        else {
            return None;
        };
        if self.config.settlement_rewards.is_empty() {
            return Some(vec![]);
        }
        // Reward assets cannot be debited by admission, so their baseline is the
        // original loaded balance protected by the ledger's expected revision.
        let facts = self.loaded_assets()?;
        self.config.reward_plan_with_damage(
            *outcome,
            scoring.as_ref(),
            progression.as_ref(),
            |id| crate::config::settlement_progression::balance(facts, id).unwrap_or(0),
        )
    }

    pub(crate) fn ledger_settle(&mut self, mut intent: Business) -> Result<()> {
        assert!(self.ledger_only && self.world.is_none());
        self.reserve()?;
        if self.config.allows_partial_entry() {
            let Some(Operation {
                intent: Business::CommitAdmission { entry_years, .. },
                ..
            }) = self.operations.get(&self.operation_id("admit"))
            else {
                return Err(Error::InvalidMessage);
            };
            let Business::CommitSettlement {
                progression: Some(draft),
                ..
            } = &mut intent
            else {
                return Err(Error::InvalidMessage);
            };
            draft.entry_charged = Some(*entry_years);
        }
        let computed = self
            .settled_reward_balances(&intent)
            .ok_or(Error::InvalidMessage)?;
        let Business::CommitSettlement { rewards, .. } = &mut intent else {
            return Err(Error::InvalidMessage);
        };
        *rewards = computed;
        if self.config.settlement_progression.is_some() {
            let plan = self
                .config
                .plan_progression(&intent, self.loaded_assets().ok_or(Error::InvalidMessage)?)
                .ok_or(Error::InvalidMessage)?;
            let Business::CommitSettlement { progression, .. } = &mut intent else {
                unreachable!()
            };
            *progression = Some(plan);
        }
        self.phase = Phase::Draining;
        self.drain_until = Some(self.io_ms + self.config.params.runtime.drain_timeout_ms);
        self.request("settle", intent);
        Ok(())
    }
    pub(crate) fn ledger_settled(&self) -> bool {
        self.operations
            .get(&self.operation_id("settle"))
            .is_some_and(|o| matches!(o.terminal, Some(ReceiptOutcome::Committed { .. })))
    }
    pub(crate) fn ledger_settlement_started(&self) -> bool {
        self.operations.contains_key(&self.operation_id("settle"))
    }
    pub(crate) fn ledger_checkpoint(&self) -> IoCheckpoint {
        self.recovery_checkpoint()
    }
    pub(crate) fn ledger_pending(&self) -> u32 {
        self.operations
            .values()
            .filter(|o| o.terminal.is_none())
            .count() as u32
    }
    pub(crate) fn ledger_queued(&self) -> usize {
        self.outputs.len()
    }
    pub(crate) fn ledger_close(&mut self) -> Result<Status> {
        self.close(Close {
            key: self.key(),
            request_id: "member-ledger-close".into(),
        })
    }
}
