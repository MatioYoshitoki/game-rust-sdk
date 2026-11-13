//! An opportunity references an existing shot and owns only the redirect cost.
use super::*;

pub(super) struct RecastOpportunity {
    pub view: ProjectileRecastView,
    pub spirit_cost: u32,
}

impl World {
    pub(super) fn redirect_projectile(
        &mut self,
        i: usize,
        action_id: &str,
        direction: [f64; 2],
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let member = &mut self.members[i];
        let opportunity = member
            .recast
            .as_ref()
            .filter(|recast| recast.view.action_id == action_id && now < recast.view.expires_at_ms)
            .ok_or(Error::InvalidTarget)?;
        let shot = self
            .shots
            .iter_mut()
            .find(|shot| {
                shot.view.id == opportunity.view.projectile_id
                    && matches!(shot.owner, Owner::Member(owner) if owner == i)
                    && now < shot.expires
            })
            .ok_or(Error::InvalidTarget)?;
        let direction = validated_direction(direction)?;
        let cost = member.effective_spirit_cost(opportunity.spirit_cost);
        if member.spirit < cost {
            return Err(Error::NoResource);
        }
        let opportunity = member.recast.take().expect("validated recast");
        member.spirit -= cost;
        shot.direction = direction;
        private(
            events,
            i,
            Event::ProjectileRecast {
                action_id: opportunity.view.action_id,
                projectile_id: opportunity.view.projectile_id,
                recast_index: 1,
                direction,
                spirit_cost: cost,
            },
        );
        Ok(())
    }

    pub(super) fn prune_recasts(&mut self, now: u64) {
        for member in &mut self.members {
            if member.recast.as_ref().is_some_and(|recast| {
                now >= recast.view.expires_at_ms
                    || !self
                        .shots
                        .iter()
                        .any(|shot| shot.view.id == recast.view.projectile_id)
            }) {
                member.recast = None;
            }
        }
    }
}
