//! Private lineage and bounded end notifications; no client can declare these flags.
use super::*;
use std::collections::BTreeMap;
const MAX_END_NOTICES: usize = 1024;
type OverflowKey = (u8, String, u32, Option<u32>, u8, String, u32, Option<u32>);
#[derive(Clone, Default)]
pub(super) struct EndContext {
    pub(super) suppressed_cause: bool,
    pub(super) enabled: std::collections::BTreeMap<String, u8>,
    pub(super) origins: BTreeMap<String, crate::origin::ActionOrigin>,
    pub(super) derived: std::collections::BTreeSet<String>,
    pub(super) ends: Vec<(StatusView, StatusRemovalReason)>,
    pub(super) overflow: BTreeMap<OverflowKey, (StatusEntity, StatusEntity, u32)>,
}
impl EndContext {
    pub(super) fn remove(&mut self, s: &StatusView, reason: StatusRemovalReason) {
        if reason == StatusRemovalReason::Transferred {
            return;
        }
        self.origins.remove(&s.instance_id);
        let enabled = self.enabled.remove(&s.instance_id).unwrap_or(0);
        let derived = self.derived.remove(&s.instance_id);
        let mask = match reason {
            StatusRemovalReason::Expired => 1,
            StatusRemovalReason::Depleted => 2,
            StatusRemovalReason::Consumed => 4,
            StatusRemovalReason::Cleansed => 8,
            _ => 0,
        };
        if enabled & mask == 0
            || derived
            || self.suppressed_cause && reason != StatusRemovalReason::Expired
        {
            return;
        }
        if self.ends.len() < MAX_END_NOTICES {
            self.ends.push((s.clone(), reason));
            return;
        }
        let tag = |e: &StatusEntity| {
            if e.kind == StatusEntityKind::Participant {
                0
            } else {
                1
            }
        };
        let key = (
            tag(&s.source),
            s.source.id.clone(),
            s.source.generation,
            s.source.encounter_round,
            tag(&s.target),
            s.target.id.clone(),
            s.target.generation,
            s.target.encounter_round,
        );
        // A World has at most 40 identities, so fewer than 2048 distinct pairs.
        if self.overflow.contains_key(&key) || self.overflow.len() < 2048 {
            let row = self
                .overflow
                .entry(key)
                .or_insert((s.source.clone(), s.target.clone(), 0));
            row.2 = row.2.saturating_add(1);
        }
    }
}
impl Statuses {
    pub(super) fn register_context(&mut self, id: &str, d: &StatusDefinition) {
        if let Some(origin) = &d.action_origin {
            self.end_context.origins.insert(id.into(), origin.clone());
        } else {
            self.end_context.origins.remove(id);
        }
        if d.end_effects > 0 && !d.derived {
            self.end_context.enabled.insert(id.into(), d.end_effects);
        } else {
            self.end_context.enabled.remove(id);
        }
        if d.derived {
            self.end_context.derived.insert(id.into());
        } else if !matches!(d.duplicate_policy, DuplicatePolicy::Stack) {
            self.end_context.derived.remove(id);
        }
    }
    pub(crate) fn end_cause_scope(&mut self, suppressed: bool) -> bool {
        std::mem::replace(&mut self.end_context.suppressed_cause, suppressed)
    }
    pub(crate) fn action_origin(&self, id: &str) -> Option<&crate::origin::ActionOrigin> {
        self.end_context.origins.get(id)
    }
    pub(crate) fn is_derived(&self, id: &str) -> bool {
        self.end_context.derived.contains(id)
    }
    pub(crate) fn take_status_ends(&mut self) -> Vec<(StatusView, StatusRemovalReason)> {
        std::mem::take(&mut self.end_context.ends)
    }
    pub(crate) fn take_end_overflow(&mut self) -> Vec<(StatusEntity, StatusEntity, u32)> {
        std::mem::take(&mut self.end_context.overflow)
            .into_values()
            .collect()
    }
}
