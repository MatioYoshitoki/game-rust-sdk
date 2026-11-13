//! Bounded identity/time accounting; callers own geometry and report real contact success.
use crate::config::ContactPolicyConfig;
use std::collections::BTreeMap;
pub(crate) struct ContactBudget<K> {
    policy: ContactPolicyConfig,
    used: u32,
    history: BTreeMap<K, u64>,
}
impl<K: Ord> ContactBudget<K> {
    pub(crate) fn once(max_hits: u32) -> Self {
        Self::projectile(max_hits, 0)
    }
    pub(crate) fn projectile(max_hits: u32, repeat_interval_ms: u64) -> Self {
        Self::new(ContactPolicyConfig {
            max_targets_per_tick: 32,
            max_total_hits: max_hits,
            repeat_interval_ms,
        })
    }
    pub(crate) fn new(policy: ContactPolicyConfig) -> Self {
        Self {
            policy,
            used: 0,
            history: BTreeMap::new(),
        }
    }
    pub(crate) fn policy(&self) -> &ContactPolicyConfig {
        &self.policy
    }
    pub(crate) fn used(&self) -> u32 {
        self.used
    }
    pub(crate) fn remaining(&self) -> u32 {
        self.policy.max_total_hits - self.used
    }
    pub(crate) fn history(&self) -> impl Iterator<Item = (&K, &u64)> {
        self.history.iter()
    }
    pub(crate) fn allows(&self, key: &K, at: u64) -> bool {
        self.remaining() > 0
            && self.history.get(key).is_none_or(|last| {
                self.policy.repeat_interval_ms > 0
                    && at >= last.saturating_add(self.policy.repeat_interval_ms)
            })
    }
    pub(crate) fn record(&mut self, key: K, at: u64) -> bool {
        if !self.allows(&key, at) {
            return false;
        }
        self.used += 1;
        self.history.insert(key, at);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy(repeat: u64, total: u32) -> ContactPolicyConfig {
        ContactPolicyConfig {
            max_targets_per_tick: 1,
            max_total_hits: total,
            repeat_interval_ms: repeat,
        }
    }
    #[test]
    fn contact_budget_once_uses_full_identity_and_never_resets_on_probe() {
        let mut b = ContactBudget::new(policy(0, 3));
        assert!(b.record(("enemy", 1), 100));
        assert!(!b.allows(&("enemy", 1), 99999));
        assert!(!b.record(("enemy", 1), 99999));
        assert!(b.record(("enemy", 2), 200));
        assert_eq!(b.used(), 2);
        assert_eq!(b.history().count(), 2);
    }
    #[test]
    fn contact_budget_positive_interval_is_inclusive_and_uses_supplied_schedule() {
        let mut b = ContactBudget::new(policy(50, 3));
        assert!(b.record(1, 100));
        assert!(!b.record(1, 149));
        assert!(!b.record(1, 99));
        assert!(b.record(1, 150));
        assert!(!b.record(1, 150));
        assert!(b.record(1, 200));
        assert_eq!(b.remaining(), 0);
    }
    #[test]
    fn contact_budget_exhaustion_cannot_underflow_or_add_history() {
        let mut b = ContactBudget::new(policy(1, 2));
        assert!(b.record(1, 1));
        assert!(b.record(1, 2));
        assert!(!b.record(2, 3));
        assert_eq!(b.remaining(), 0);
        assert_eq!(b.history().count(), 1);
    }
    #[test]
    fn contact_budget_128_successes_bound_distinct_history() {
        let mut b = ContactBudget::new(policy(0, 128));
        for k in 0..128 {
            assert!(b.record(k, k));
        }
        assert!(!b.record(129, 129));
        assert_eq!(b.history().count(), 128);
        assert_eq!(b.used(), 128);
    }
    #[test]
    fn contact_budget_instances_and_frozen_configuration_are_independent() {
        let mut p = policy(100, 2);
        let mut a = ContactBudget::new(p.clone());
        let b = ContactBudget::new(p.clone());
        p.max_total_hits = 99;
        assert_eq!(p.max_total_hits, 99);
        assert!(a.record("x", 100));
        assert!(b.allows(&"x", 100));
        assert_eq!(a.policy().max_total_hits, 2);
        assert_eq!(b.used(), 0);
    }
}
