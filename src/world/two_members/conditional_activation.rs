//! A bundle has one condition consumption and a two-end preview, not a script.
use super::*;
impl World {
    pub(super) fn preview_status_bundle(
        &self,
        preview: &Statuses,
        actors: (Owner, Owner),
        lists: (&[String], &[String]),
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let source = self.owner_entity(actors.0);
        let target = self.owner_entity(actors.1);
        if !self.status_operation_eligible(actors.0)
            || !self.status_operation_eligible(actors.1)
            || self.status_relation(&status_entity(&source), &status_entity(&target))
                != Some(combat::Relation::Hostile)
        {
            return Err(Error::InvalidTarget);
        }
        let rules = &c.combat_rules.as_ref().unwrap().status_policies;
        if lists.0.iter().any(|id| {
            crate::config::status_blocked(preview, id, &status_entity(&source), rules, now)
        }) {
            return Err(Error::Immune);
        }
        let own_defs = Self::definitions(c, lists.0);
        if own_defs.iter().any(|d| {
            d.is_hostile_control()
                || d.attribute == Some(StatusAttribute::SpiritRecoveryLock)
                    && source.kind != StatusEntityKind::Participant
        }) {
            return Err(Error::InvalidTarget);
        }
        if !preview.can_apply(
            &own_defs,
            &status_entity(&source),
            &status_entity(&source),
            c.pve.as_ref().unwrap().status_rules.as_ref().unwrap(),
        ) {
            return Err(Error::Capacity);
        }
        for id in lists.1 {
            self.preview_other_status(preview, actors, id, c, now)?;
        }
        let originals = Self::definitions(c, lists.1);
        let resolved = self.resolve_control_definitions(&status_entity(&target), &originals, c);
        if !preview.can_apply(
            &resolved,
            &status_entity(&source),
            &status_entity(&target),
            c.pve.as_ref().unwrap().status_rules.as_ref().unwrap(),
        ) {
            return Err(Error::Capacity);
        }
        Ok(())
    }
    pub(super) fn execute_status_bundle(
        &mut self,
        actors: (Owner, Owner),
        lists: (&[String], &[String]),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if self
            .preview_status_bundle(&self.statuses, actors, lists, c, now)
            .is_err()
        {
            return;
        }
        for (owner, ids) in [(actors.0, lists.0), (actors.1, lists.1)] {
            if ids.is_empty() {
                continue;
            }
            let target = self.owner_entity(owner);
            let maximum = self.entity_max_hp(&target, c, now).unwrap();
            self.apply_owned_statuses(actors.0, (target, maximum), ids, c, now, events);
        }
    }
}
impl World {
    pub(super) fn owned_status_instance<'a>(
        &self,
        statuses: &'a Statuses,
        actors: (Owner, Owner),
        selector: (crate::config::TriggerSubject, &str),
        c: &Config,
        now: u64,
    ) -> Result<&'a StatusView> {
        let target = Self::counter_target(actors, selector.0);
        if !self.status_operation_eligible(actors.0) || !self.status_operation_eligible(target) {
            return Err(Error::InvalidTarget);
        }
        let source = status_entity(&self.owner_entity(actors.0));
        let target = status_entity(&self.owner_entity(target));
        if crate::config::status_blocked(
            statuses,
            selector.1,
            &target,
            &c.combat_rules.as_ref().unwrap().status_policies,
            now,
        ) {
            return Err(Error::Immune);
        }
        statuses
            .entries()
            .iter()
            .find(|s| {
                s.definition_id == selector.1
                    && s.source == source
                    && s.target == target
                    && now < s.expires_at_ms
            })
            .ok_or(Error::InvalidState)
    }
    pub(super) fn plan_status_refresh(
        &self,
        statuses: &Statuses,
        actors: (Owner, Owner),
        operation: (crate::config::TriggerSubject, &str, Option<u64>),
        c: &Config,
        now: u64,
    ) -> Result<(String, u64)> {
        let s = self.owned_status_instance(statuses, actors, (operation.0, operation.1), c, now)?;
        let d = c
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .find(|d| d.id == operation.1)
            .ok_or(Error::InvalidState)?;
        if s.periodic.is_some()
            || s.shield.is_some()
            || !s.flags.is_empty()
            || s.attribute
                .is_some_and(|a| a.is_control_flag() || a.is_immunity_flag())
        {
            return Err(Error::Unsupported);
        }
        let reduction = if d.is_harmful() {
            statuses.attribute(
                &s.target,
                10000,
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
                StatusAttribute::HarmfulDurationReduction,
            ) - 10000
        } else {
            0
        };
        let duration = (u128::from(operation.2.unwrap_or(d.duration_ms))
            * u128::from(10000 - reduction)
            / 10000)
            .max(1) as u64;
        let expires = now
            .checked_add(duration)
            .filter(|ms| *ms < (1u64 << 53))
            .ok_or(Error::InvalidState)?;
        Ok((s.instance_id.clone(), expires))
    }
    pub(super) fn refresh_status_duration(
        &mut self,
        actors: (Owner, Owner),
        operation: (crate::config::TriggerSubject, &str, Option<u64>),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let Ok((id, expires)) = self.plan_status_refresh(&self.statuses, actors, operation, c, now)
        else {
            return;
        };
        self.advance_attack_clocks(c, now);
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        if self.statuses.refresh_deadline(&id, now, expires, &mut raw) {
            self.route_status_events(raw, &before, events);
            self.reconcile_caps(c, now);
        }
    }
}
