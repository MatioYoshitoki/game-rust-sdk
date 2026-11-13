//! Conditional target grants share the native status entry and its final preview.
use super::*;
impl World {
    pub(super) fn preview_other_status(
        &self,
        preview: &Statuses,
        actors: (Owner, Owner),
        status_id: &str,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let source = status_entity(&self.owner_entity(actors.0));
        let target = status_entity(&self.owner_entity(actors.1));
        if !self.valid_status_entity(&source)
            || !self.valid_status_entity(&target)
            || self.status_relation(&source, &target) != Some(combat::Relation::Hostile)
        {
            return Err(Error::InvalidTarget);
        }
        let originals = Self::definitions(c, &[status_id.to_owned()]);
        let resolved = self.resolve_control_definitions(&target, &originals, c);
        let r = c.combat_rules.as_ref().unwrap();
        if resolved.iter().any(|d| {
            crate::config::status_blocked(preview, &d.id, &target, &r.status_policies, now)
                || d.attribute == Some(StatusAttribute::SpiritRecoveryLock)
                    && target.kind != StatusEntityKind::Participant
        }) {
            return Err(Error::Immune);
        }
        if resolved
            .iter()
            .any(crate::config::StatusDefinition::is_hostile_control)
            && preview.entries().iter().any(|s| {
                s.target == target
                    && now < s.expires_at_ms
                    && (s.has_attribute(StatusAttribute::ControlImmunity)
                        || s.has_attribute(StatusAttribute::FirstControlImmunity))
            })
        {
            return Err(Error::Immune);
        }
        if !preview.can_apply(
            &resolved,
            &source,
            &target,
            c.pve.as_ref().unwrap().status_rules.as_ref().unwrap(),
        ) {
            return Err(Error::Capacity);
        }
        Ok(())
    }
}
