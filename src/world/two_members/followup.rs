//! Buffering owns the intent. Preparation and commit retain their original costs.
use super::*;

impl Member {
    fn followup_release(&self, now: u64, step: u64) -> Option<(&str, u64)> {
        let source = self.recovery_skill.as_deref()?;
        if now >= self.recovery_until
            || !self.eligible()
            || self.pending.is_some()
            || self.channel.is_some()
            || self.extract_end.is_some()
        {
            return None;
        }
        let release = now + (self.recovery_until - now).div_ceil(step) * step;
        Some((source, release))
    }

    pub(super) fn followup_view(&self, now: u64, step: u64) -> Option<FollowupView> {
        let (source, promotes_at_ms) = self.followup_release(now, step)?;
        Some(FollowupView {
            source_skill_id: source.into(),
            promotes_at_ms,
            queued: self.queued_cast.clone(),
        })
    }
}

impl World {
    pub(super) fn queue_followup(
        &mut self,
        i: usize,
        skill: &SkillConfig,
        direction: [f64; 2],
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let member = &self.members[i];
        let (source, release) = member
            .followup_release(now, c.step_ms)
            .ok_or(Error::InvalidState)?;
        let expires_at_ms = now + c.skill_followup.expect("followup enabled").window_ms;
        if !skill.supports_followup()
            || skill.id == source
            || release > expires_at_ms
            || member
                .cooldowns
                .get(&skill.id)
                .is_some_and(|ready| *ready > release)
        {
            return Err(Error::InvalidState);
        }
        self.check_skill_budget(i, skill, c)?;
        self.members[i].queued_cast = Some(QueuedCastView {
            action_id: id.into(),
            skill_id: skill.id.clone(),
            direction,
            expires_at_ms,
        });
        self.members[i].queued_cast_epoch = self.checkpoint_epoch;
        Ok(())
    }

    pub(super) fn promote_followup(&mut self, i: usize, c: &Config, now: u64, events: &mut Events) {
        let member = &mut self.members[i];
        if member
            .queued_cast
            .as_ref()
            .is_none_or(|cast| now < member.recovery_until && now <= cast.expires_at_ms)
        {
            return;
        }
        let cast = member.queued_cast.take().expect("due followup");
        let checkpoint_epoch = member.queued_cast_epoch;
        let result = (|| {
            if now > cast.expires_at_ms {
                return Err(Error::InvalidState);
            }
            let skill = c.player_skill(&cast.skill_id).ok_or(Error::InvalidTarget)?;
            self.projectile_origin(i, skill.radius)?;
            self.prepare(
                i,
                Some(skill.clone()),
                (cast.direction, None),
                &cast.action_id,
                c,
                now,
            )?;
            self.members[i].pending.as_mut().unwrap().checkpoint_epoch = checkpoint_epoch;
            Ok(())
        })();
        if let Err(reason) = result {
            private(
                events,
                i,
                Event::ActionRejected {
                    action_id: cast.action_id,
                    reason,
                },
            );
        }
    }
}
