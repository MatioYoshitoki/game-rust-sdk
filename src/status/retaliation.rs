//! Bounded per-instance attacker history follows the native status lifecycle.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(super) struct RetaliationState {
    hits: BTreeMap<String, Vec<(StatusEntity, u64)>>,
    expired: Vec<(StatusView, Vec<StatusEntity>)>,
}

impl RetaliationState {
    pub(super) fn clear(&mut self, id: &str) {
        self.hits.remove(id);
    }

    pub(super) fn remove(&mut self, status: &StatusView, reason: StatusRemovalReason) {
        if let Some(hits) = self.hits.remove(&status.instance_id)
            && reason == StatusRemovalReason::Expired
            && self.expired.len() < 128
        {
            self.expired
                .push((status.clone(), hits.into_iter().map(|(e, _)| e).collect()));
        }
    }
}

impl Statuses {
    pub(crate) fn retaliation_ready(
        &self,
        id: &str,
        attacker: &StatusEntity,
        max: usize,
        now: u64,
    ) -> bool {
        self.retaliation.hits.get(id).is_none_or(|hits| {
            hits.iter()
                .find(|(e, _)| e == attacker)
                .map_or(hits.len() < max, |(_, ready)| now >= *ready)
        })
    }

    pub(crate) fn record_retaliation(&mut self, id: String, attacker: StatusEntity, ready: u64) {
        let hits = self.retaliation.hits.entry(id).or_default();
        if let Some((_, deadline)) = hits.iter_mut().find(|(e, _)| e == &attacker) {
            *deadline = ready;
        } else {
            hits.push((attacker, ready));
        }
    }

    pub(crate) fn take_retaliation_expiries(&mut self) -> Vec<(StatusView, Vec<StatusEntity>)> {
        std::mem::take(&mut self.retaliation.expired)
    }
}
