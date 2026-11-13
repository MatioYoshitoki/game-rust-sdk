use super::*;
use crate::config::world_loot::LootContainer;

#[derive(Clone, Copy)]
struct Reservation {
    member: usize,
    completes: u64,
    inventory_revision: u32,
}

enum State {
    Ready,
    Reserved(Reservation),
    Cooldown(u64),
}

pub(super) struct Container {
    id: String,
    generation: u32,
    state: State,
    draft: Option<Vec<ItemAmount>>,
}

impl Container {
    pub(super) fn new(id: String) -> Self {
        Self {
            id,
            generation: 1,
            state: State::Ready,
            draft: None,
        }
    }
}

impl World {
    pub(in crate::world::two_members) fn open_container(
        &mut self,
        i: usize,
        (id, generation): (&str, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let world = c.world_loot.as_ref().ok_or(Error::Unsupported)?;
        let (index, definition) = self.find_visible_container(world, i, id)?;
        let container = &self.world_loot.containers[index];
        if generation != container.generation {
            return Err(Error::RevisionConflict);
        }
        if !matches!(container.state, State::Ready) || self.has_container_reservation(i) {
            return Err(Error::Conflict);
        }
        self.container_access(i, world, definition, c, now)?;
        let inventory_revision = self.members[i].revision;
        if let Some(cost) = &definition.cost {
            self.members[i].plan_acquired_removal(
                &cost.item_id,
                cost.quantity,
                inventory_revision,
            )?;
        }
        let completes = now + definition.unlock_ms;
        self.world_loot.containers[index].state = State::Reserved(Reservation {
            member: i,
            completes,
            inventory_revision,
        });
        events.push((
            Some(i),
            EventV4::ContainerReserved {
                container_id: id.into(),
                generation,
                finishes_at_ms: completes,
            },
        ));
        Ok(())
    }

    pub(in crate::world::two_members) fn cancel_container(
        &mut self,
        i: usize,
        (id, generation): (&str, u32),
        c: &Config,
        events: &mut Events,
    ) -> Result<()> {
        let world = c.world_loot.as_ref().ok_or(Error::Unsupported)?;
        let (index, _) = self.find_visible_container(world, i, id)?;
        let state = &self.world_loot.containers[index];
        if generation != state.generation {
            return Err(Error::RevisionConflict);
        }
        if !matches!(state.state, State::Reserved(r) if r.member == i) {
            return Err(Error::InvalidState);
        }
        self.release_container(index, events);
        Ok(())
    }

    pub(in crate::world::two_members) fn release_member_containers(
        &mut self,
        i: usize,
        events: &mut Events,
    ) {
        for index in 0..self.world_loot.containers.len() {
            if matches!(self.world_loot.containers[index].state, State::Reserved(r) if r.member == i)
            {
                self.release_container(index, events);
            }
        }
    }

    fn release_container(&mut self, index: usize, events: &mut Events) {
        let container = &mut self.world_loot.containers[index];
        if let State::Reserved(r) = std::mem::replace(&mut container.state, State::Ready) {
            events.push((
                Some(r.member),
                EventV4::ContainerReleased {
                    container_id: container.id.clone(),
                    generation: container.generation,
                },
            ));
        }
    }

    pub(super) fn advance_containers(
        &mut self,
        c: &Config,
        world: &WorldLoot,
        now: u64,
        events: &mut Events,
    ) {
        let mut active = self
            .resources
            .iter()
            .filter(|r| {
                matches!(
                    r.origin,
                    LootOrigin::Container | LootOrigin::PrivateContainer
                )
            })
            .count();
        for (index, definition) in world.containers.iter().enumerate() {
            let container = &mut self.world_loot.containers[index];
            let reservation = match container.state {
                State::Ready => continue,
                State::Cooldown(at) => {
                    if now >= at {
                        container.generation += 1;
                        container.state = State::Ready;
                    }
                    continue;
                }
                State::Reserved(reservation) => reservation,
            };
            if self
                .container_access(reservation.member, world, definition, c, now)
                .is_err()
            {
                self.release_container(index, events);
                continue;
            }
            if now < reservation.completes {
                continue;
            }
            match self.commit_container(
                index,
                definition,
                reservation,
                (c, world, active),
                now,
                events,
            ) {
                Ok(count) => active += count,
                Err(_) => self.release_container(index, events),
            }
        }
    }

    fn commit_container(
        &mut self,
        index: usize,
        definition: &LootContainer,
        reservation: Reservation,
        (c, world, active): (&Config, &WorldLoot, usize),
        now: u64,
        events: &mut Events,
    ) -> Result<usize> {
        let i = reservation.member;
        let removal = definition
            .cost
            .as_ref()
            .map(|cost| {
                self.members[i]
                    .plan_acquired_removal(
                        &cost.item_id,
                        cost.quantity,
                        reservation.inventory_revision,
                    )
                    .map(|removal| (&cost.item_id, removal))
            })
            .transpose()?;
        if self.world_loot.containers[index].draft.is_none() {
            let items = self.draw_loot(&world.pools[&definition.pool]);
            self.world_loot.containers[index].draft = Some(items);
        }
        let count = self.world_loot.containers[index]
            .draft
            .as_ref()
            .unwrap()
            .len();
        if active + count > world.container_capacity {
            return Err(Error::Capacity);
        }
        let point = if count == 0 {
            definition.position
        } else {
            let points = if definition.drop_points.is_empty() {
                std::slice::from_ref(&definition.position)
            } else {
                &definition.drop_points
            };
            points
                .iter()
                .copied()
                .find(|&point| {
                    self.grid.clear(point, c.params.player_radius, 0)
                        && self
                            .grid
                            .line(definition.position, point, c.params.player_radius, 0)
                })
                .ok_or(Error::InvalidTarget)?
        };
        let container = &mut self.world_loot.containers[index];
        let generation = container.generation;
        let items = container.draft.take().unwrap();
        container.state = State::Cooldown(now + definition.replenish_ms);
        if let Some((item, removal)) = removal {
            self.members[i].commit_acquired_removal(
                item,
                removal,
                super::super::loot_drop::RemovalKind::Consumed,
            );
        }
        let private = !matches!(definition.visibility, ContainerVisibility::Shared);
        let origin = if private {
            LootOrigin::PrivateContainer
        } else {
            LootOrigin::Container
        };
        self.publish_loot(
            items,
            point,
            now + definition.ttl_ms,
            (origin, private.then_some(i)),
            c,
            events,
        );
        events.push((
            Some(i),
            EventV4::ContainerOpened {
                container_id: definition.id.clone(),
                generation,
            },
        ));
        Ok(count)
    }

    fn container_access(
        &self,
        i: usize,
        world: &WorldLoot,
        definition: &LootContainer,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let m = &self.members[i];
        if !m.eligible()
            || m.extract_end.is_some()
            || self.member_has_status(i, StatusAttribute::Stun, now)
        {
            return Err(Error::InvalidState);
        }
        if distance(m.position, definition.position) > world.interaction_range {
            return Err(Error::OutOfRange);
        }
        if !self.container_visible(i, definition)
            || !self
                .grid
                .line(m.position, definition.position, c.params.player_radius, 0)
            || self.container_guarded(definition)
        {
            return Err(Error::InvalidTarget);
        }
        Ok(())
    }

    fn container_guarded(&self, definition: &LootContainer) -> bool {
        self.npcs.iter().any(|npc| {
            (npc.hp > 0 || !self.mode_npc_visible(&npc.entity.id))
                && definition.guardians.contains(&npc.entity.id)
        })
    }

    fn container_visible(&self, i: usize, definition: &LootContainer) -> bool {
        let member = &self.members[i];
        match &definition.visibility {
            ContainerVisibility::Shared => true,
            ContainerVisibility::Personal { participant_id } => {
                *participant_id == member.reference.entity.id
            }
            ContainerVisibility::Hidden { reveal_radius } => {
                member.living()
                    && distance(member.position, definition.position) <= *reveal_radius
                    && self.grid.line(member.position, definition.position, 0., 2)
            }
        }
    }

    fn find_visible_container<'a>(
        &self,
        world: &'a WorldLoot,
        i: usize,
        id: &str,
    ) -> Result<(usize, &'a LootContainer)> {
        world
            .containers
            .iter()
            .enumerate()
            .find(|(_, c)| c.id == id && self.container_visible(i, c))
            .ok_or(Error::InvalidTarget)
    }

    pub(in crate::world::two_members) fn has_container_reservation(&self, i: usize) -> bool {
        self.world_loot
            .containers
            .iter()
            .any(|container| matches!(container.state, State::Reserved(r) if r.member == i))
    }

    pub(in crate::world::two_members) fn container_views(
        &self,
        i: usize,
        c: &Config,
    ) -> Option<Vec<ContainerViewV6>> {
        Some(
            c.world_loot
                .as_ref()?
                .containers
                .iter()
                .zip(&self.world_loot.containers)
                .filter(|(definition, _)| self.container_visible(i, definition))
                .map(|(definition, container)| ContainerViewV6 {
                    id: definition.id.clone(),
                    generation: container.generation,
                    position: definition.position,
                    state: match container.state {
                        State::Ready => ContainerStateV6::Ready,
                        State::Reserved(_) => ContainerStateV6::Reserved,
                        State::Cooldown(_) => ContainerStateV6::Cooldown,
                    },
                })
                .collect(),
        )
    }

    pub(in crate::world::two_members) fn container_for_ai<'a>(
        &self,
        i: usize,
        radius: f64,
        c: &'a Config,
    ) -> Option<(&'a LootContainer, u32)> {
        let world = c.world_loot.as_ref()?;
        let m = &self.members[i];
        world
            .containers
            .iter()
            .zip(&self.world_loot.containers)
            .filter_map(|(definition, container)| {
                let d = distance(m.position, definition.position);
                (matches!(container.state, State::Ready)
                    && d <= radius
                    && self.container_visible(i, definition)
                    && !self.container_guarded(definition)
                    && definition.cost.as_ref().is_none_or(|cost| {
                        m.plan_acquired_removal(&cost.item_id, cost.quantity, m.revision)
                            .is_ok()
                    }))
                .then_some((definition, container.generation, d))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.id.cmp(&b.0.id)))
            .map(|(definition, generation, _)| (definition, generation))
    }
}
