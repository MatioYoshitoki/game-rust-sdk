//! A trusted grant changes qualification, never already committed plans or debts.
use super::*;
impl World {
    pub(crate) fn loadout_mutable(&self, i: usize) -> bool {
        let m = &self.members[i];
        m.hp > 0
            && m.desired.is_none()
            && matches!(m.phase, MemberPhase::Deploying | MemberPhase::Active)
    }
    pub(crate) fn replace_skill_loadout(
        &mut self,
        i: usize,
        state: SkillLoadoutState,
        now: u64,
    ) -> Events {
        let equipped = &state.loadout.equipped_skill_ids;
        let m = &mut self.members[i];
        let removed_source = m
            .recovery_skill
            .as_ref()
            .is_some_and(|id| !equipped.contains(id));
        let pending = m
            .pending
            .as_ref()
            .and_then(|p| p.skill.as_ref())
            .is_some_and(|s| !equipped.contains(&s.id));
        let queued = m
            .queued_cast
            .as_ref()
            .is_some_and(|q| !equipped.contains(&q.skill_id) || removed_source);
        let basic = m.queued_basic.is_some() && removed_source;
        if pending {
            m.pending = None;
        }
        if queued {
            m.queued_cast = None;
        }
        if basic {
            m.queued_basic = None;
        }
        if m.recast
            .as_ref()
            .is_some_and(|r| !equipped.contains(&r.view.skill_id))
        {
            m.recast = None;
        }
        let channel = m
            .channel
            .as_ref()
            .is_some_and(|p| !equipped.contains(&p.skill.id));
        let mut events = vec![];
        if pending || queued || basic {
            private(&mut events, i, Event::ActionInterrupted);
        }
        if channel {
            self.finish_channel(i, ChannelEndReason::Interrupted, now, &mut events);
        }
        // The elapsed recovery debt survives; removed skills no longer offer new
        // followup or cancellation opportunities that reference an unequipped slot.
        let member = &mut self.members[i];
        if member
            .recovery_skill
            .as_ref()
            .is_some_and(|id| !equipped.contains(id))
        {
            member.recovery_skill = None;
            member.recovery_cancel_at = None;
            member.recovery_cancel_policy = None;
        }
        self.members[i].skills = state;
        private(
            &mut events,
            i,
            Event::SkillLoadoutChanged {
                revision: self.members[i].skills.revision,
                equipped_skill_ids: self.members[i].skills.loadout.equipped_skill_ids.clone(),
            },
        );
        events
    }
}
