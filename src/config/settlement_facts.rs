use super::*;
use crate::protocol::{Business, valid_id};
use std::collections::BTreeSet;

impl Config {
    pub(crate) fn settlement_fact_intent(&self, id: &str, intent: &Business) -> Option<Business> {
        let Business::CommitSettlement { outcome, .. } = intent else {
            return None;
        };
        let result = self.result_binding(*outcome)?;
        let mut fact_ids = self.settlement_facts.get(&result.result_id)?.clone();
        fact_ids.sort_unstable();
        Some(Business::AppendFacts {
            settlement_operation_id: id.to_owned(),
            result_id: result.result_id.clone(),
            fact_ids,
        })
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    if c.settlement_facts.is_empty() {
        return Ok(());
    }
    if c.result_contract.is_none() {
        return Err("CONFIG_SETTLEMENT_FACTS_CAPABILITY");
    }
    if c.protocol_version() != 6 || c.mechanism_version < 105 {
        return Err("CONFIG_SETTLEMENT_FACTS_CAPABILITY");
    }
    if c.settlement_facts.iter().any(|(result, ids)| {
        !c.result_bindings()
            .any(|(_, binding)| &binding.result_id == result)
            || !(1..=8).contains(&ids.len())
            || ids.iter().any(|id| id.len() > 64 || !valid_id(id))
            || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    }) {
        return Err("CONFIG_SETTLEMENT_FACTS_RULE");
    }
    Ok(())
}
