//! A valid native commit can consume at most one source-owned gate.
use super::*;
impl World {
    pub(super) fn intercept_cast(
        &mut self,
        attempt: (Owner, &str, Option<&str>),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let (caster, skill, action_id) = attempt;
        let Some(r) = c
            .combat_rules
            .as_ref()
            .filter(|r| !r.cast_interceptions.is_empty())
        else {
            return false;
        };
        if !self.status_operation_eligible(caster) {
            return false;
        }
        let target = self.owner_entity(caster);
        let holder = status_entity(&target);
        let profile = self.combat_profile_id(caster, r);
        let Some((mark, source, effect)) = self
            .statuses
            .entries()
            .iter()
            .filter_map(|mark| {
                if mark.target != holder
                    || now >= mark.expires_at_ms
                    || mark.stacks.unwrap_or(1) != 1
                    || self.statuses.is_derived(&mark.instance_id)
                {
                    return None;
                }
                let rule = r.cast_interceptions.get(&mark.definition_id)?;
                if !rule.action_filter.accepts_skill(skill) {
                    return None;
                }
                let source = self.entity_owner(&mark.source)?;
                if !self.status_operation_eligible(source)
                    || self.status_relation(&mark.source, &holder)
                        != Some(combat::Relation::Hostile)
                {
                    return None;
                }
                Some((
                    mark,
                    source,
                    rule.profile_overrides.get(profile).unwrap_or(&rule.default),
                ))
            })
            .min_by_key(|(m, _, _)| (m.expires_at_ms, m.applied_at_ms, m.instance_id.as_str()))
        else {
            return false;
        };
        let mark_id = mark.instance_id.clone();
        let mark_definition_id = mark.definition_id.clone();
        let original_origin = self.statuses.action_origin(&mark_id).cloned();
        let source_entity = self.owner_entity(source);
        let permanent = self.entity_has_status(caster, StatusAttribute::ControlImmunity, now);
        let first = if permanent {
            None
        } else {
            self.statuses
                .entries()
                .iter()
                .filter(|s| {
                    s.target == holder
                        && now < s.expires_at_ms
                        && s.has_attribute(StatusAttribute::FirstControlImmunity)
                })
                .min_by_key(|s| (s.expires_at_ms, s.applied_at_ms, s.instance_id.as_str()))
                .map(|s| s.instance_id.clone())
        };
        let immune = permanent || first.is_some();
        let followup = (!immune).then(|| effect.followup());
        if let Some(id) = followup {
            let mut preview = self.statuses.clone();
            preview.expire(now, &mut vec![]);
            if !preview.consume(&mark_id, 1, &mut vec![])
                || self
                    .preview_other_status(&preview, (source, caster), id, c, now)
                    .is_err()
            {
                return false;
            }
        }
        self.advance_attack_clocks(c, now);
        let before = self
            .statuses
            .entries()
            .iter()
            .filter(|s| s.instance_id == mark_id || first.as_ref() == Some(&s.instance_id))
            .cloned()
            .collect::<Vec<_>>();
        let mut raw = vec![];
        if !self.statuses.consume(&mark_id, 1, &mut raw) {
            return false;
        }
        if let Some(id) = first {
            self.statuses.remove(
                StatusRemovalReason::Depleted,
                |s| s.instance_id == id,
                &mut raw,
            );
        }
        self.route_status_events(raw, &before, events);
        if let Some(id) = followup {
            let previous_origin = std::mem::replace(&mut self.triggers.origin, original_origin);
            let previous_source = self.triggers.action_source.replace(source_entity.clone());
            let previous_action = self.triggers.action.take();
            let maximum = self
                .entity_max_hp(&target, c, now)
                .expect("validated caster");
            self.apply_owned_statuses(
                source,
                (target.clone(), maximum),
                &[id.to_owned()],
                c,
                now,
                events,
            );
            self.triggers.origin = previous_origin;
            self.triggers.action_source = previous_source;
            self.triggers.action = previous_action;
        }
        let interrupted = !immune && effect.interrupts();
        let outcome = if immune {
            CastInterceptionOutcome::ImmuneAllowed
        } else if interrupted {
            CastInterceptionOutcome::Interrupted
        } else {
            CastInterceptionOutcome::AllowedWithFollowup
        };
        let event = Event::CastInterceptionResolved {
            source: source_entity.clone(),
            target,
            skill_id: skill.to_owned(),
            action_id: action_id.map(str::to_owned),
            mark_instance_id: mark_id,
            mark_definition_id,
            followup_status_id: followup.map(str::to_owned),
            outcome,
        };
        for i in self.status_observers(&status_entity(&source_entity), &holder) {
            private(events, i, event.clone());
        }
        interrupted
    }
}
