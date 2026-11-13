use super::*;
use crate::config::contested_equipment::ContestedEquipment;

#[derive(Clone, Copy)]
pub(super) enum Pursuit {
    Dormant,
    Tracking(usize),
    Retiring(u64),
}

impl Npc {
    pub(super) fn pursuit_visible(&self) -> bool {
        !matches!(self.pursuit, Some(Pursuit::Dormant))
    }

    pub(super) fn pursuit_can_fight(&self) -> bool {
        matches!(self.pursuit, None | Some(Pursuit::Tracking(_)))
    }
}

struct Claim {
    view: WorldEquipmentView,
    instance: Option<ItemInstance>,
}

pub(super) struct ContestedState {
    nodes: Vec<Claim>,
    range: f64,
    retire_delay: u64,
    now: u64,
}

impl ContestedState {
    pub(super) fn new(rule: &ContestedEquipment) -> Self {
        Self {
            nodes: rule
                .nodes
                .iter()
                .map(|node| Claim {
                    view: WorldEquipmentView {
                        id: node.id.clone(),
                        position: node.position,
                        revision: 1,
                        state: WorldEquipmentState::Available,
                        holder: None,
                    },
                    instance: None,
                })
                .collect(),
            range: rule.pursuit_range,
            retire_delay: rule.retire_delay_ms,
            now: 0,
        }
    }
}

impl World {
    pub(crate) fn set_contested_clock(&mut self, now: u64) {
        if let Some(state) = &mut self.contested {
            state.now = now;
        }
    }

    pub(super) fn contested_views(&self) -> Option<Vec<WorldEquipmentView>> {
        Some(
            self.contested
                .as_ref()?
                .nodes
                .iter()
                .map(|node| node.view.clone())
                .collect(),
        )
    }

    pub(super) fn claim_world_equipment(
        &mut self,
        seat: usize,
        (id, revision): (&str, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let rule = c.contested_equipment.as_ref().ok_or(Error::Unsupported)?;
        let state = self.contested.as_ref().unwrap();
        let index = state
            .nodes
            .iter()
            .position(|node| node.view.id == id)
            .ok_or(Error::InvalidTarget)?;
        let node = &state.nodes[index];
        let spec = &rule.nodes[index];
        let member = &self.members[seat];
        if node.view.revision != revision {
            return Err(Error::RevisionConflict);
        }
        if !matches!(
            node.view.state,
            WorldEquipmentState::Available | WorldEquipmentState::Dropped
        ) {
            return Err(Error::InvalidState);
        }
        if distance(node.view.position, member.position) > rule.interaction_range
            || !self.grid.line(member.position, node.view.position, 0., 0)
        {
            return Err(Error::InvalidTarget);
        }
        if member.free_inventory_slots(c) == 0 {
            return Err(Error::Capacity);
        }
        let hunters: Vec<_> = self
            .npcs
            .iter()
            .enumerate()
            .filter(|(_, npc)| spec.pursuit_ids.contains(&npc.entity.id))
            .map(|(j, _)| j)
            .collect();
        for &j in &hunters {
            let npc = &self.npcs[j];
            if !self.grid.clear(npc.home, npc.profile.combat.radius, 0) {
                return Err(Error::InvalidTarget);
            }
        }
        let first = node.instance.is_none();
        let holder = member.reference.entity.clone();
        if member.active_basic_profile() != Some(&spec.basic_profile) {
            self.prepare_basic_switch(seat, c, now, events)?;
        }
        let instance = first.then(|| {
            self.next_id("world-equipment");
            self.loot_instance(&spec.secured_item_id, c).unwrap()
        });
        self.set_contested_clock(now);
        let node = &mut self.contested.as_mut().unwrap().nodes[index];
        if first {
            node.instance = instance;
        }
        node.view.revision += 1;
        node.view.state = WorldEquipmentState::Held;
        node.view.holder = Some(holder);
        let member = &mut self.members[seat];
        member.reserved_world_slots += 1;
        member.world_weapon_profile = Some(spec.basic_profile.clone());
        for j in hunters {
            let npc = &self.npcs[j];
            let mut next = Npc::new(
                EntityRef {
                    generation: npc.entity.generation + 1,
                    ..npc.entity.clone()
                },
                npc.template_id.clone(),
                npc.profile.clone(),
                npc.home,
            );
            next.pursuit = Some(Pursuit::Tracking(seat));
            next.next_attack = now + next.profile.combat.attack_interval_ms;
            next.attack_clock = Some(attack_speed::AttackCooldown::new(
                now,
                next.profile.combat.attack_interval_ms,
            ));
            next.next_cast = now;
            self.npcs[j] = next;
        }
        Ok(())
    }

    pub(super) fn release_contested_equipment(
        &mut self,
        seat: usize,
        outcome: Outcome,
        events: &mut Events,
    ) {
        let Some(state) = &mut self.contested else {
            return;
        };
        let member = &mut self.members[seat];
        if member.reserved_world_slots == 0 {
            return;
        }
        member.world_weapon_profile = None;
        member.reserved_world_slots = 0;
        for node in &mut state.nodes {
            if node.view.holder.as_ref() != Some(&member.reference.entity) {
                continue;
            }
            node.view.revision += 1;
            node.view.holder = None;
            node.view.state = if outcome == Outcome::Extraction {
                let item = node.instance.take().unwrap();
                member.instances.insert(item.instance_id.clone(), item);
                WorldEquipmentState::Secured
            } else {
                node.view.position = member.position;
                WorldEquipmentState::Dropped
            };
        }
        let expires_at = state.now + state.retire_delay;
        for j in 0..self.npcs.len() {
            if matches!(self.npcs[j].pursuit, Some(Pursuit::Tracking(i)) if i == seat) {
                self.npcs[j].pursuit = Some(Pursuit::Retiring(expires_at));
                self.clear_pursuit_combat(j, events);
            }
        }
    }

    pub(super) fn clear_pursuit_combat(&mut self, j: usize, events: &mut Events) {
        let npc = &mut self.npcs[j];
        npc.target = None;
        npc.casting = None;
        npc.path.clear();
        npc.ai = Default::default();
        let source = npc.entity.clone();
        self.clear_owned_statuses(&source, StatusRemovalReason::Reset, events);
        let before = self.statuses.entries().to_vec();
        let mut removed = vec![];
        self.statuses.remove(
            StatusRemovalReason::Reset,
            |s| s.source == status_entity(&source),
            &mut removed,
        );
        self.route_status_events(removed, &before, events);
        self.clear_areas(&source, events);
        self.clear_monster_contribution(j);
        self.terrain_damage_remainders
            .remove(&triggers::entity_key(&source));
        self.shots.retain(|shot| shot.view.source != source);
    }

    pub(super) fn advance_contested_equipment(&mut self, now: u64) {
        if self.contested.is_none() {
            return;
        }
        for npc in &mut self.npcs {
            if matches!(npc.pursuit, Some(Pursuit::Retiring(until)) if now >= until) {
                npc.pursuit = Some(Pursuit::Dormant);
                npc.hp = 0;
            }
        }
    }

    pub(crate) fn expire_contested_equipment(&mut self, events: &mut Events) {
        let Some(state) = &mut self.contested else {
            return;
        };
        for node in &mut state.nodes {
            if matches!(
                node.view.state,
                WorldEquipmentState::Available | WorldEquipmentState::Dropped
            ) {
                node.view.revision += 1;
                node.view.state = WorldEquipmentState::Lost;
                node.instance = None;
            }
        }
        for j in 0..self.npcs.len() {
            if self.npcs[j].pursuit.is_some() && self.npcs[j].pursuit_visible() {
                self.clear_pursuit_combat(j, events);
                self.npcs[j].pursuit = Some(Pursuit::Dormant);
                self.npcs[j].hp = 0;
            }
        }
    }

    pub(super) fn select_pursuit_target(&self, j: usize, seat: usize) -> Option<npc_ai::NpcTarget> {
        let m = &self.members[seat];
        let d = distance(self.npcs[j].position, m.position);
        (m.eligible() && d <= self.contested.as_ref()?.range).then(|| {
            (
                Owner::Member(seat),
                m.reference.entity.clone(),
                m.position,
                d,
            )
        })
    }

    pub(super) fn pursuit_allows_target(&self, j: usize, target: Owner) -> bool {
        match self.npcs[j].pursuit {
            None => true,
            Some(Pursuit::Tracking(seat)) => {
                matches!(target, Owner::Npc(_)) || target == Owner::Member(seat)
            }
            Some(Pursuit::Dormant | Pursuit::Retiring(_)) => false,
        }
    }

    pub(super) fn pursuit_allows_detached(&self, source: &EntityRef, target: Owner) -> bool {
        self.contested.is_none()
            || self
                .npcs
                .iter()
                .position(|npc| npc.pursuit.is_some() && npc.entity.id == source.id)
                .is_none_or(|j| {
                    self.npcs[j].entity == *source
                        && self.npcs[j].hp > 0
                        && self.pursuit_allows_target(j, target)
                })
    }
}
