//! Resolve fresh control applications before the ordinary native status pipeline.
use super::*;
impl World {
    pub(super) fn control_policy<'a>(
        &self,
        target: &StatusEntity,
        c: &'a Config,
    ) -> Option<&'a crate::config::ControlPolicy> {
        let r = c.combat_rules.as_ref()?;
        let profile = self.combat_profile_id(self.entity_owner(target)?, r);
        r.control_policies.get(profile)
    }
    pub(super) fn resolve_control_definitions<'a>(
        &self,
        target: &StatusEntity,
        defs: &'a [crate::config::StatusDefinition],
        c: &Config,
    ) -> std::borrow::Cow<'a, [crate::config::StatusDefinition]> {
        let Some(policy) = self.control_policy(target, c) else {
            return std::borrow::Cow::Borrowed(defs);
        };
        let mut result: Vec<crate::config::StatusDefinition> = vec![];
        for original in defs {
            let mut actual = original.clone();
            if crate::config::hard_control(original) {
                let duration = original
                    .duration_ms
                    .min(policy.max_duration_ms.unwrap_or(u64::MAX));
                if let Some(id) = policy.replacements.get(&original.id) {
                    actual = c
                        .pve
                        .as_ref()
                        .unwrap()
                        .status_definitions
                        .iter()
                        .find(|d| d.id == *id)
                        .unwrap()
                        .clone();
                }
                actual.duration_ms = duration;
            }
            if let Some(prior) = result.iter_mut().find(|d| d.id == actual.id) {
                prior.duration_ms = prior.duration_ms.max(actual.duration_ms);
            } else {
                result.push(actual);
            }
        }
        std::borrow::Cow::Owned(result)
    }
}
