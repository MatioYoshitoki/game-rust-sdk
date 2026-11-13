use super::*;
use crate::config::world_loot::DeathRelics;

pub(super) struct Relic {
    view: RelicView,
    entries: Vec<RelicEntry>,
}

impl Relic {
    fn update_summary(&mut self, policy: &DeathRelics) {
        self.view.remaining_stacks = self.entries.len();
        self.view.highest_quality = self
            .entries
            .iter()
            .map(|row| {
                row.instance.as_ref().map_or_else(
                    || {
                        policy
                            .stack_qualities
                            .get(&row.item_id)
                            .copied()
                            .unwrap_or(0)
                    },
                    |instance| instance.quality,
                )
            })
            .max();
    }
}

impl World {
    pub(in crate::world::two_members) fn create_death_relic(
        &mut self,
        i: usize,
        policy: &DeathRelics,
        c: &Config,
    ) {
        let m = &self.members[i];
        let position = m.position;
        let deceased = m.reference.entity.clone();
        if self.world_loot.relics.len() >= policy.capacity
            || self
                .world_loot
                .relics
                .iter()
                .any(|relic| relic.view.deceased == deceased)
            || !self.grid.clear(position, c.params.player_radius, 0)
        {
            return;
        }

        let mut eligible = vec![];
        for (id, &quantity) in &m.looted {
            if policy.excluded_items.contains(id) {
                continue;
            }
            let held = m.inventory.get(id).copied().unwrap_or(0).min(quantity);
            let limit = c.item_limit(id).expect("validated inventory item");
            for n in 0..held.div_ceil(limit) {
                eligible.push((id.clone(), (held - n * limit).min(limit), None));
            }
        }
        eligible.extend(
            m.instances
                .values()
                .filter(|instance| {
                    !policy.excluded_items.contains(&instance.item_id)
                        && c.instance_definition(&instance.item_id)
                            .is_some_and(|def| def.relic_eligible)
                })
                .map(|instance| (instance.item_id.clone(), 1, Some(instance.clone()))),
        );

        let mut entries = Vec::new();
        let mut removed = BTreeMap::<String, u32>::new();
        for _ in 0..c.params.death.count(eligible.len()) {
            let index = self.loot_index(eligible.len() as u32) as usize;
            let (item_id, held, instance) = eligible.swap_remove(index);
            let quantity = if instance.is_some() {
                1
            } else {
                c.params.death.amount(held)
            };
            if quantity == 0 {
                continue;
            }
            if instance.is_none() {
                *removed.entry(item_id.clone()).or_default() += quantity;
            }
            entries.push(RelicEntry {
                entry_id: entries.len() as u32 + 1,
                item_id,
                quantity,
                instance,
            });
        }

        let m = &mut self.members[i];
        let Ok(plans) = removed
            .into_iter()
            .map(|(id, q)| {
                let removal = m.plan_acquired_removal(&id, q, m.revision)?;
                Ok::<_, Error>((id, removal))
            })
            .collect::<Result<Vec<_>>>()
        else {
            return;
        };
        if !entries.is_empty() {
            let Some(revision) = m.revision.checked_add(1) else {
                return;
            };
            for (id, plan) in plans {
                m.commit_acquired_removal(&id, plan, super::super::loot_drop::RemovalKind::Relic);
            }
            for instance in entries.iter().filter_map(|row| row.instance.as_ref()) {
                m.instances.remove(&instance.instance_id);
            }
            m.revision = revision;
        }
        let mut relic = Relic {
            view: RelicView {
                id: self.next_id("relic"),
                position,
                deceased,
                revision: 1,
                remaining_stacks: 0,
                highest_quality: None,
            },
            entries,
        };
        relic.update_summary(policy);
        self.world_loot.relics.push(relic);
    }

    pub(in crate::world::two_members) fn relic_views(&self, c: &Config) -> Option<Vec<RelicView>> {
        c.death_relics().map(|_| {
            self.world_loot
                .relics
                .iter()
                .map(|relic| relic.view.clone())
                .collect()
        })
    }

    fn checked_relic(&self, i: usize, id: &str, policy: &DeathRelics, c: &Config) -> Result<usize> {
        let index = self
            .world_loot
            .relics
            .iter()
            .position(|relic| relic.view.id == id)
            .ok_or(Error::InvalidTarget)?;
        let from = self.members[i].position;
        let to = self.world_loot.relics[index].view.position;
        if distance(from, to) > policy.interaction_range {
            return Err(Error::OutOfRange);
        }
        if !self.grid.line(from, to, c.params.player_radius, 0) {
            return Err(Error::InvalidTarget);
        }
        Ok(index)
    }

    pub(in crate::world::two_members) fn open_relic(
        &mut self,
        i: usize,
        id: &str,
        c: &Config,
        events: &mut Events,
    ) -> Result<()> {
        let policy = c.death_relics().ok_or(Error::Unsupported)?;
        let index = self.checked_relic(i, id, policy, c)?;
        let relic = &self.world_loot.relics[index];
        self.members[i].relic_access = Some(id.to_owned());
        private(
            events,
            i,
            Event::RelicOpened {
                relic_id: id.to_owned(),
                revision: relic.view.revision,
                contents: relic.entries.clone(),
            },
        );
        Ok(())
    }

    pub(in crate::world::two_members) fn claim_relic(
        &mut self,
        i: usize,
        (id, revision, inventory_revision, entry): (&str, u32, u32, Option<u32>),
        c: &Config,
        events: &mut Events,
    ) -> Result<()> {
        let policy = c.death_relics().ok_or(Error::Unsupported)?;
        let index = self.checked_relic(i, id, policy, c)?;
        let m = &mut self.members[i];
        if m.relic_access.as_deref() != Some(id) {
            return Err(Error::Unauthorized);
        }
        let relic = &mut self.world_loot.relics[index];
        if relic.view.revision != revision || m.revision != inventory_revision {
            return Err(Error::RevisionConflict);
        }
        if relic.entries.is_empty()
            || entry.is_some_and(|id| !relic.entries.iter().any(|row| row.entry_id == id))
        {
            return Err(Error::InvalidTarget);
        }
        let next_revision = revision.checked_add(1).ok_or(Error::Capacity)?;
        let next_inventory_revision = inventory_revision.checked_add(1).ok_or(Error::Capacity)?;
        let mut claimed = vec![];
        for row in relic
            .entries
            .iter_mut()
            .filter(|row| entry.is_none_or(|id| row.entry_id == id))
        {
            let amount = if let Some(instance) = &row.instance {
                u32::from(m.credit_instance(instance, c))
            } else {
                m.credit_stack(&row.item_id, row.quantity, true, c)
                    .unwrap_or(0)
            };
            if amount > 0 {
                claimed.push(RelicEntry {
                    quantity: amount,
                    ..row.clone()
                });
                row.quantity -= amount;
            }
        }
        if claimed.is_empty() {
            return Err(Error::Capacity);
        }
        relic.entries.retain(|row| row.quantity > 0);
        relic.view.revision = next_revision;
        relic.update_summary(policy);
        m.revision = next_inventory_revision;
        private(
            events,
            i,
            Event::RelicClaimed {
                relic_id: id.to_owned(),
                revision: next_revision,
                items: claimed,
            },
        );
        Ok(())
    }

    pub(in crate::world::two_members) fn participant_ai_relic(
        &self,
        i: usize,
        c: &Config,
        radius: f64,
    ) -> Option<Action> {
        let policy = c.death_relics()?;
        let m = &self.members[i];
        let relic = self.world_loot.relics.iter().find(|relic| {
            distance(m.position, relic.view.position) <= radius
                && self.checked_relic(i, &relic.view.id, policy, c).is_ok()
                && relic.entries.iter().any(|row| {
                    if row.instance.is_some() {
                        m.free_inventory_slots(c) > 0
                    } else {
                        m.stack_credit_capacity(&row.item_id, c) > 0
                    }
                })
        })?;
        let relic_id = relic.view.id.clone();
        if m.relic_access.as_deref() == Some(&relic_id) {
            Some(Action::ClaimRelic {
                relic_id,
                expected_revision: relic.view.revision,
                expected_inventory_revision: m.revision,
                entry_id: None,
            })
        } else {
            Some(Action::OpenRelic { relic_id })
        }
    }
}
