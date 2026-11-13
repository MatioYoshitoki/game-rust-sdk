//! A successful replacement owns a new preparation. The earlier commit remains final.
use super::*;

impl Member {
    fn cancel_window(&self, now: u64) -> Option<u64> {
        self.recovery_cancel_at.filter(|_| {
            now < self.recovery_until
                && self.eligible()
                && self.pending.is_none()
                && self.channel.is_none()
                && self.extract_end.is_none()
        })
    }

    pub(super) fn recovery_cancel_view(&self, now: u64) -> Option<RecoveryCancelView> {
        Some(RecoveryCancelView {
            skill_id: self.recovery_skill.as_ref()?.clone(),
            cancel_at_ms: self.cancel_window(now)?,
        })
    }

    pub(super) fn can_cancel_recovery(&self, now: u64) -> bool {
        self.cancel_window(now).is_some_and(|opens| now >= opens)
            && self
                .recovery_cancel_policy
                .and_then(|p| p.by_explicit_action)
                .unwrap_or(true)
    }

    pub(super) fn can_replace_recovery(&self, skill: &SkillConfig, now: u64) -> bool {
        self.can_replace_recovery_action(Some(skill), now)
    }
    pub(super) fn can_replace_recovery_action(
        &self,
        skill: Option<&SkillConfig>,
        now: u64,
    ) -> bool {
        self.can_cancel_recovery(now)
            && (self.recovery_cancel_policy.is_some_and(|p| p.expanded())
                || skill.is_some_and(|skill| {
                    skill.supports_followup()
                        && self
                            .recovery_skill
                            .as_deref()
                            .is_some_and(|source| source != skill.id)
                }))
    }
    pub(super) fn can_cancel_recovery_by_move(&self, now: u64) -> bool {
        self.cancel_window(now).is_some_and(|opens| now >= opens)
            && self.recovery_cancel_policy.and_then(|p| p.by_move) == Some(true)
    }

    pub(super) fn end_recovery(&mut self, now: u64) {
        self.recovery_until = now;
        self.recovery_cancel_at = None;
        self.recovery_skill = None;
        self.recovery_cancel_policy = None;
    }
}
