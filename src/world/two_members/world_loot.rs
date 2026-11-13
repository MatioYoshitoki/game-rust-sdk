use super::*;
use crate::config::world_loot::{ContainerVisibility, LootEntry, LootNode, LootPool, WorldLoot};

mod containers;
mod monster_drops;
mod relics;
use containers::Container;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum LootOrigin {
    #[default]
    Legacy,
    Environment(usize),
    Container,
    PrivateContainer,
    Monster {
        priority_until_ms: u64,
    },
}

#[derive(Default)]
struct Node {
    ready_at: u64,
    live: bool,
    draft: Option<Vec<ItemAmount>>,
    cycle: usize,
}

pub(super) struct LootState {
    nodes: Vec<Node>,
    containers: Vec<Container>,
    monster_drops: monster_drops::MonsterDrops,
    relics: Vec<relics::Relic>,
    instance_scope: String,
}

impl LootState {
    pub(super) fn new(config: Option<&WorldLoot>) -> Self {
        Self {
            nodes: (0..config.map_or(0, |w| w.nodes.len()))
                .map(|_| Node::default())
                .collect(),
            containers: config
                .into_iter()
                .flat_map(|w| &w.containers)
                .map(|definition| Container::new(definition.id.clone()))
                .collect(),
            monster_drops: Default::default(),
            relics: Default::default(),
            instance_scope: String::new(),
        }
    }
}

impl World {
    pub(crate) fn bind_instance_scope(&mut self, scope: String) {
        self.world_loot.instance_scope = scope;
    }

    pub(super) fn advance_world_loot(&mut self, c: &Config, now: u64, events: &mut Events) {
        let Some(config) = &c.world_loot else { return };
        if !self.members.iter().any(Member::living) {
            return;
        }
        self.advance_monster_drops(c, config, now, events);
        self.advance_containers(c, config, now, events);
        let mut count = self
            .resources
            .iter()
            .filter(|r| matches!(r.origin, LootOrigin::Environment(_)))
            .count();
        for (index, node) in config.nodes.iter().enumerate() {
            self.update_node_empty(index, node, now);
            let state = &self.world_loot.nodes[index];
            if state.live || now < state.ready_at {
                continue;
            }
            if state.draft.is_none() {
                let draft = self.draw_loot(&config.pools[&node.pool]);
                self.world_loot.nodes[index].draft = Some(draft);
            }
            let state = &self.world_loot.nodes[index];
            let size = state.draft.as_ref().unwrap().len();
            if size == 0 {
                let state = &mut self.world_loot.nodes[index];
                state.draft = None;
                state.ready_at = now + node.replenish_ms;
                state.cycle += 1;
                continue;
            }
            let point = (count + size <= config.environment_capacity)
                .then(|| self.environment_position(node, config, state.cycle, c))
                .flatten();
            let Some(point) = point else {
                self.world_loot.nodes[index].ready_at = now + c.params.runtime.resource_sweep_ms;
                continue;
            };
            let state = &mut self.world_loot.nodes[index];
            state.live = true;
            state.cycle += 1;
            let items = state.draft.take().unwrap();
            count += items.len();
            self.publish_loot(
                items,
                point,
                now + node.ttl_ms,
                (LootOrigin::Environment(index), None),
                c,
                events,
            );
        }
    }

    fn environment_position(
        &self,
        node: &LootNode,
        config: &WorldLoot,
        cycle: usize,
        c: &Config,
    ) -> Option<[f64; 2]> {
        let shared = || {
            config
                .containers
                .iter()
                .filter(|row| matches!(row.visibility, ContainerVisibility::Shared))
        };
        let preferred = node.hotspot.as_ref().and_then(|hotspot| {
            let anchors: Vec<_> = shared()
                .filter(|row| row.hotspot_group.as_deref() == Some(&hotspot.group))
                .collect();
            (!anchors.is_empty()).then(|| (anchors[cycle % anchors.len()].position, hotspot.radius))
        });
        let points =
            || (0..node.points.len()).map(|i| node.points[(i + cycle) % node.points.len()]);
        let legal = |p: &[f64; 2]| {
            self.grid.clear(*p, c.params.player_radius, 0)
                && shared().all(|row| distance(row.position, *p) >= config.shared_clearance)
                && c.member_settings().unwrap().spawns.iter().any(|&spawn| {
                    !self
                        .grid
                        .route(spawn, *p, c.params.player_radius)
                        .is_empty()
                })
        };
        preferred
            .and_then(|(anchor, radius)| {
                points()
                    .filter(|p| distance(anchor, *p) <= radius)
                    .find(legal)
            })
            .or_else(|| points().find(legal))
    }

    fn update_node_empty(&mut self, index: usize, node: &LootNode, now: u64) {
        let state = &mut self.world_loot.nodes[index];
        if state.live
            && !self
                .resources
                .iter()
                .any(|r| r.origin == LootOrigin::Environment(index))
        {
            state.live = false;
            state.ready_at = now + node.replenish_ms;
        }
    }

    fn draw_loot(&mut self, pool: &LootPool) -> Vec<ItemAmount> {
        let mut items: Vec<_> = pool
            .guaranteed
            .iter()
            .map(|entry| self.draw_loot_entry(entry))
            .collect();
        for roll in &pool.independent {
            if self.loot_chance(roll.chance_bps) {
                items.push(self.draw_loot_entry(&roll.entry));
            }
        }
        for group in &pool.groups {
            if !self.loot_chance(group.chance_bps) {
                continue;
            }
            let count = group.count_min
                + self.loot_index((group.count_max - group.count_min + 1) as u32) as usize;
            let mut available: Vec<_> = group.entries.iter().collect();
            for _ in 0..count {
                let index = self.loot_index(available.len() as u32) as usize;
                items.push(self.draw_loot_entry(available.swap_remove(index)));
            }
        }
        items
    }

    fn loot_chance(&mut self, bps: u32) -> bool {
        bps == 10000 || bps > 0 && self.loot_index(10000) < bps
    }

    fn loot_index(&mut self, width: u32) -> u32 {
        if width == 1 {
            return 0;
        }
        // The World PRNG cycles through the nonzero u32 values.
        let bound = u32::MAX - u32::MAX % width;
        loop {
            let value = self.random() - 1;
            if value < bound {
                return value % width;
            }
        }
    }

    fn draw_loot_entry(&mut self, entry: &LootEntry) -> ItemAmount {
        ItemAmount {
            item_id: entry.item_id.clone(),
            quantity: entry.quantity_min
                + self.loot_index(entry.quantity_max - entry.quantity_min + 1),
        }
    }

    pub(super) fn loot_instance(&self, item_id: &str, c: &Config) -> Option<ItemInstance> {
        let definition = c.instance_definition(item_id)?;
        Some(ItemInstance {
            instance_id: format!("{}-{:016x}", self.world_loot.instance_scope, self.sequence),
            item_id: item_id.into(),
            quality: definition.quality,
        })
    }

    fn publish_loot(
        &mut self,
        items: Vec<ItemAmount>,
        position: [f64; 2],
        expires_at_ms: u64,
        (origin, owner): (LootOrigin, Option<usize>),
        c: &Config,
        events: &mut Events,
    ) {
        for item in items {
            let id = self.next_id("resource");
            let instance = self.loot_instance(&item.item_id, c);
            let view = ResourceViewV4 {
                id,
                position,
                quantity: item.quantity,
                item_id: Some(item.item_id),
                owner: owner.map(|i| self.members[i].reference.entity.clone()),
                expires_at_ms,
                instance,
            };
            events.push((
                owner.filter(|_| origin == LootOrigin::PrivateContainer),
                EventV4::ResourceSpawned {
                    resource: view.clone(),
                },
            ));
            self.resources.push(Resource {
                view,
                hp: 0,
                spirit: 0,
                player_drop: false,
                origin,
            });
        }
    }

    pub(super) fn pickup_resource(
        &mut self,
        i: usize,
        (id, expected_revision): (&str, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let m = &self.members[i];
        if m.revision != expected_revision {
            return Err(Error::RevisionConflict);
        }
        let index = self
            .resources
            .iter()
            .position(|r| {
                r.view.id == id && now < r.view.expires_at_ms && r.visible_to(&m.reference.entity)
            })
            .ok_or(Error::InvalidTarget)?;
        let r = &self.resources[index];
        if distance(m.position, r.view.position) > c.params.pickup_range {
            return Err(Error::OutOfRange);
        }
        if r.requires_reachable_pickup()
            && !self
                .grid
                .line(m.position, r.view.position, c.params.player_radius, 0)
        {
            return Err(Error::InvalidTarget);
        }
        if r.owner_at(now)
            .is_some_and(|owner| owner != &m.reference.entity)
        {
            return Err(Error::Unauthorized);
        }
        let revision = m.revision.checked_add(1).ok_or(Error::Capacity)?;
        let view = r.view_at(now);
        let origin = r.origin;
        let recipient = (origin == LootOrigin::PrivateContainer).then_some(i);
        let amount = if let Some(instance) = &view.instance {
            if !self.members[i].credit_instance(instance, c) {
                return Err(Error::Capacity);
            }
            1
        } else if let Some(item) = &view.item_id {
            self.members[i]
                .credit_stack(
                    item,
                    view.quantity,
                    c.world_loot.as_ref().is_some_and(|w| w.partial_pickup),
                    c,
                )
                .ok_or(Error::Capacity)?
        } else {
            let max_hp = self.entity_max_hp(&m.reference.entity, c, now).unwrap();
            let max_spirit = self.member_max_spirit(i, c, now);
            let (hp, spirit) = (r.hp, r.spirit);
            let m = &mut self.members[i];
            m.hp = m.hp.saturating_add(hp).min(max_hp);
            m.spirit += spirit.min(m.spirit_recovery_gap(&self.statuses, now, max_spirit));
            view.quantity
        };
        let member = &mut self.members[i];
        member.revision = revision;
        let participant = member.reference.entity.clone();
        private(
            events,
            i,
            Event::PickedUp {
                resource_id: view.id.clone(),
                quantity: amount,
            },
        );
        if amount == view.quantity {
            self.resources.remove(index);
            if let LootOrigin::Environment(node) = origin {
                self.update_node_empty(node, &c.world_loot.as_ref().unwrap().nodes[node], now);
            }
            events.push((
                recipient,
                EventV4::ResourceTaken {
                    resource_id: view.id,
                    participant,
                },
            ));
        } else {
            let remainder = &mut self.resources[index].view;
            remainder.quantity -= amount;
            remainder.owner = view.owner;
            events.push((
                recipient,
                EventV4::ResourceUpdated {
                    resource: remainder.clone(),
                    participant,
                },
            ));
        }
        Ok(())
    }
}

impl Resource {
    pub(super) fn owner_at(&self, now: u64) -> Option<&EntityRef> {
        if matches!(self.origin, LootOrigin::Monster { priority_until_ms } if now >= priority_until_ms)
        {
            return None;
        }
        self.view.owner.as_ref()
    }

    pub(super) fn view_at(&self, now: u64) -> ResourceViewV4 {
        let mut view = self.view.clone();
        if self.owner_at(now).is_none() {
            view.owner = None;
        }
        view
    }

    pub(super) fn visible_to(&self, entity: &EntityRef) -> bool {
        self.origin != LootOrigin::PrivateContainer || self.view.owner.as_ref() == Some(entity)
    }

    pub(super) fn requires_reachable_pickup(&self) -> bool {
        self.player_drop || self.origin != LootOrigin::Legacy
    }
}
