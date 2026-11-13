//! This control path is host trusted and is never a player Action variant.
use super::*;
const MAX_CONTROL_OPERATIONS: usize = 128;
fn initial(c: &Config, r: &CreateV4) -> SkillControlCheckpoint {
    SkillControlCheckpoint {
        members: r
            .participants
            .iter()
            .map(|s| {
                (
                    s.participant_id.clone(),
                    crate::config::initial_skill_loadout(c, s.skills.as_ref()),
                )
            })
            .collect(),
        operations: vec![],
    }
}
fn valid_request(c: &Config, r: &CreateV4, request: &SetSkillLoadout) -> bool {
    request.key == r.key
        && request.entity_generation == 1
        && !request.operation_id.is_empty()
        && request.operation_id.len() <= 64
        && valid_id(&request.operation_id)
        && r.participants
            .iter()
            .any(|s| s.participant_id == request.participant_id)
        && crate::config::valid_loadout(c, &request.loadout)
}
impl TwoMembers {
    pub(super) fn initial_skill_control(
        c: &Config,
        r: &CreateV4,
        recovery: Option<&RecoveryV4>,
    ) -> Result<SkillControlCheckpoint> {
        for s in &r.participants {
            if let Some(loadout) = &s.skills {
                if r.protocol_version != 6 || c.mechanism_version < 105 {
                    return Err(Error::Unsupported);
                }
                if !crate::config::valid_loadout(c, loadout) {
                    return Err(Error::InvalidMessage);
                }
            }
        }
        let mut state = initial(c, r);
        if let Some(recovery) = recovery {
            let audit = &recovery.checkpoint.state.skill_control;
            if let Some(audit) = audit {
                if c.mechanism_version < 105 || audit.operations.len() > MAX_CONTROL_OPERATIONS {
                    return Err(Error::InvalidMessage);
                }
                let mut seen = std::collections::BTreeSet::new();
                for operation in &audit.operations {
                    let q = &operation.request;
                    let receipt = &operation.receipt;
                    if !valid_request(c, r, q) || !seen.insert(&q.operation_id) {
                        return Err(Error::InvalidMessage);
                    }
                    let m = state
                        .members
                        .get_mut(&q.participant_id)
                        .ok_or(Error::InvalidMessage)?;
                    let revision = m.revision.checked_add(1).ok_or(Error::InvalidMessage)?;
                    if q.expected_revision != m.revision
                        || receipt.operation_id != q.operation_id
                        || receipt.revision != revision
                        || receipt.equipped_skill_ids != q.loadout.equipped_skill_ids
                    {
                        return Err(Error::InvalidMessage);
                    }
                    *m = SkillLoadoutState {
                        loadout: q.loadout.clone(),
                        revision,
                        declared: true,
                    };
                    state.operations.push(operation.clone());
                }
                if state.members != audit.members {
                    return Err(Error::InvalidMessage);
                }
            } else if r.participants.iter().any(|s| s.skills.is_some()) {
                return Err(Error::InvalidMessage);
            }
        }
        Ok(state)
    }
    pub(crate) fn set_skill_loadout(
        &mut self,
        request: SetSkillLoadout,
    ) -> Result<SkillLoadoutReceipt> {
        if request.key != self.create.key {
            return Err(Error::StaleMatch);
        }
        if self.create.protocol_version != 6 || self.config.mechanism_version < 105 {
            return Err(Error::Unsupported);
        }
        if let Some(old) = self
            .skill_control
            .operations
            .iter()
            .find(|op| op.request.operation_id == request.operation_id)
        {
            return if old.request == request {
                Ok(old.receipt.clone())
            } else {
                Err(Error::Conflict)
            };
        }
        if matches!(self.phase, Phase::Closed | Phase::Faulted) {
            return Err(Error::Closed);
        }
        if !valid_request(&self.config, &self.create, &request) {
            return Err(Error::InvalidMessage);
        }
        let i = self
            .seats
            .iter()
            .position(|s| s.original.participant_id == request.participant_id)
            .ok_or(Error::Unauthorized)?;
        let before = &self.skill_control.members[&request.participant_id];
        if request.expected_revision != before.revision {
            return Err(Error::RevisionConflict);
        }
        if self.skill_control.operations.len() >= MAX_CONTROL_OPERATIONS {
            return Err(Error::Capacity);
        }
        let revision = before.revision.checked_add(1).ok_or(Error::Capacity)?;
        self.reserve()?;
        let world = self.world.as_mut().ok_or(Error::NotReady)?;
        if !world.loadout_mutable(i) {
            return Err(Error::InvalidState);
        }
        let next = SkillLoadoutState {
            loadout: request.loadout.clone(),
            revision,
            declared: true,
        };
        let events = world.replace_skill_loadout(i, next.clone(), self.now);
        self.skill_control
            .members
            .insert(request.participant_id.clone(), next);
        let receipt = SkillLoadoutReceipt {
            operation_id: request.operation_id.clone(),
            revision,
            equipped_skill_ids: request.loadout.equipped_skill_ids.clone(),
        };
        self.skill_control.operations.push(SkillLoadoutOperation {
            request,
            receipt: receipt.clone(),
        });
        self.events(events);
        self.dirty = true;
        Ok(receipt)
    }
}
