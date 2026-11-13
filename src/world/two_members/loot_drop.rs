use super::*;

pub(super) struct AcquiredRemoval {
    quantity: u32,
    held_after: u32,
    looted_after: u32,
    revision: u32,
}

pub(super) enum RemovalKind {
    Consumed,
    Released,
    Relic,
}

#[derive(Default)]
pub(super) struct StackRemovals {
    consumed: u64,
    released: u64,
    relic_lost: u64,
}

impl Member {
    pub(super) fn stack_flows(&self) -> Option<Box<[StackFlow]>> {
        let removals = self.stack_removals.as_ref()?;
        let ids = self
            .looted
            .keys()
            .chain(removals.keys())
            .collect::<std::collections::BTreeSet<_>>();
        Some(
            ids.into_iter()
                .map(|id| {
                    let net = u64::from(self.looted.get(id).copied().unwrap_or(0));
                    let empty = StackRemovals::default();
                    let spent = removals.get(id).unwrap_or(&empty);
                    StackFlow {
                        item_id: id.clone(),
                        picked_up: net + spent.consumed + spent.released + spent.relic_lost,
                        consumed: spent.consumed,
                        released: spent.released,
                        relic_lost: spent.relic_lost,
                    }
                })
                .collect(),
        )
    }

    pub(super) fn free_inventory_slots(&self, c: &Config) -> u64 {
        let used = self
            .inventory
            .iter()
            .map(|(id, q)| {
                u64::from(q.div_ceil(c.item_limit(id).expect("validated inventory item")))
            })
            .sum::<u64>()
            + self.instances.len() as u64
            + u64::from(self.reserved_world_slots);
        u64::from(c.inventory_capacity).saturating_sub(used)
    }

    pub(super) fn stack_credit_capacity(&self, item: &str, c: &Config) -> u32 {
        let previous = self.inventory.get(item).copied().unwrap_or(0);
        let limit = c.item_limit(item).expect("validated inventory item");
        let stack_space =
            u64::from(previous.div_ceil(limit)) * u64::from(limit) - u64::from(previous);
        (self.free_inventory_slots(c) * u64::from(limit) + stack_space)
            .min(u64::from(u32::MAX - previous)) as u32
    }

    pub(super) fn credit_stack(
        &mut self,
        item: &str,
        quantity: u32,
        partial: bool,
        c: &Config,
    ) -> Option<u32> {
        let capacity = self.stack_credit_capacity(item, c);
        let amount = if partial {
            quantity.min(capacity)
        } else {
            quantity
        };
        if amount == 0 || amount > capacity {
            return None;
        }
        let net = self
            .looted
            .get(item)
            .copied()
            .unwrap_or(0)
            .checked_add(amount)?;
        *self.inventory.entry(item.into()).or_default() += amount;
        self.looted.insert(item.into(), net);
        Some(amount)
    }

    pub(super) fn credit_instance(&mut self, item: &ItemInstance, c: &Config) -> bool {
        if self.free_inventory_slots(c) == 0 || self.instances.contains_key(&item.instance_id) {
            return false;
        }
        self.instances
            .insert(item.instance_id.clone(), item.clone());
        true
    }

    pub(super) fn plan_acquired_removal(
        &self,
        item_id: &str,
        quantity: u32,
        expected_revision: u32,
    ) -> Result<AcquiredRemoval> {
        if self.revision != expected_revision {
            return Err(Error::RevisionConflict);
        }
        let net_loot = self.looted.get(item_id).copied().unwrap_or(0);
        let held = self.inventory.get(item_id).copied().unwrap_or(0);
        if quantity == 0 || quantity > net_loot || quantity > held {
            return Err(Error::InvalidTarget);
        }
        Ok(AcquiredRemoval {
            quantity,
            held_after: held - quantity,
            looted_after: net_loot - quantity,
            revision: self.revision.checked_add(1).ok_or(Error::Capacity)?,
        })
    }

    pub(super) fn commit_acquired_removal(
        &mut self,
        item_id: &str,
        removal: AcquiredRemoval,
        kind: RemovalKind,
    ) {
        if removal.held_after == 0 {
            self.inventory.remove(item_id);
        } else {
            self.inventory.insert(item_id.into(), removal.held_after);
        }
        // Financial recovery audits net loot against the original world supply.
        if removal.looted_after == 0 {
            self.looted.remove(item_id);
        } else {
            self.looted.insert(item_id.into(), removal.looted_after);
        }
        if let Some(flows) = &mut self.stack_removals {
            let row = flows.entry(item_id.into()).or_default();
            let counter = match kind {
                RemovalKind::Consumed => &mut row.consumed,
                RemovalKind::Released => &mut row.released,
                RemovalKind::Relic => &mut row.relic_lost,
            };
            *counter += u64::from(removal.quantity);
        }
        self.revision = removal.revision;
    }
}

impl World {
    pub(super) fn stack_credit_capacity(&self, i: usize, item: &str, c: &Config) -> u32 {
        self.members[i].stack_credit_capacity(item, c)
    }

    pub(super) fn drop_loot(
        &mut self,
        i: usize,
        item_id: &str,
        quantity: u32,
        expected_revision: u32,
        c: &Config,
        now: u64,
    ) -> Result<ResourceViewV4> {
        let policy = c.member_loot_drop.ok_or(Error::Unsupported)?;
        let member = &self.members[i];
        let removal = member.plan_acquired_removal(item_id, quantity, expected_revision)?;
        let position = member.position;
        if !self.grid.clear(position, c.params.player_radius, 0) {
            return Err(Error::InvalidTarget);
        }
        let active = self
            .resources
            .iter()
            .filter(|r| r.player_drop && now < r.view.expires_at_ms)
            .count();
        if active >= policy.max_active {
            return Err(Error::Capacity);
        }
        let view = ResourceViewV4 {
            id: self.next_id("player-drop"),
            position,
            quantity,
            item_id: Some(item_id.into()),
            owner: None,
            expires_at_ms: now + c.params.resource_ttl_ms,
            instance: None,
        };
        self.resources.push(Resource {
            view: view.clone(),
            hp: 0,
            spirit: 0,
            player_drop: true,
            origin: Default::default(),
        });
        self.members[i].commit_acquired_removal(item_id, removal, RemovalKind::Released);
        Ok(view)
    }
}
