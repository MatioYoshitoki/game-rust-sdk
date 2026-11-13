use super::loot_drop::AcquiredRemoval;
use super::*;
use crate::config::world_boons::{BoonOpportunity, BoonScope, BoonShield, WorldBoons};

#[derive(Clone)]
struct Slot {
    id: String,
    level: u8,
}

pub(super) struct BoonState {
    slots: Vec<Option<Slot>>,
    revision: u32,
    used: u32,
    spirit_cost_bps: u32,
    shield_ready_at: Option<u64>,
    pending: Option<PendingClaim>,
}

impl BoonState {
    pub(super) fn new(capacity: u8) -> Self {
        Self {
            slots: vec![None; usize::from(capacity)],
            revision: 1,
            used: 0,
            spirit_cost_bps: 10000,
            shield_ready_at: None,
            pending: None,
        }
    }
}

impl Member {
    pub(super) fn pending_boon_claim(&self) -> bool {
        self.boons
            .as_ref()
            .is_some_and(|boons| boons.pending.is_some())
    }

    pub(super) fn effective_spirit_cost(&self, base: u32) -> u32 {
        let Some(state) = &self.boons else {
            return base;
        };
        if base == 0 {
            return 0;
        }
        (u64::from(base) * u64::from(state.spirit_cost_bps) / 10000).max(1) as u32
    }
}

#[derive(Clone, Copy)]
pub(super) struct BoonClaim<'a> {
    pub opportunity_id: &'a str,
    pub boon_id: &'a str,
    pub expected_revision: u32,
    pub expected_inventory_revision: u32,
    pub replace_slot: Option<u8>,
}

struct ClaimPlan<'a> {
    source: &'a BoonOpportunity,
    source_index: usize,
    slot: usize,
    new_slot: Slot,
    cost: Option<AcquiredRemoval>,
}

struct PendingClaim {
    view: BoonPendingClaimView,
    source_index: usize,
    expected_revision: u32,
    expected_inventory_revision: u32,
    replace_slot: Option<u8>,
}

impl PendingClaim {
    fn input(&self) -> BoonClaim<'_> {
        BoonClaim {
            opportunity_id: &self.view.opportunity_id,
            boon_id: &self.view.boon_id,
            expected_revision: self.expected_revision,
            expected_inventory_revision: self.expected_inventory_revision,
            replace_slot: self.replace_slot,
        }
    }
}

impl Slot {
    fn view(&self, index: usize, spec: &WorldBoons) -> BoonSlotView {
        BoonSlotView {
            slot: index as u8 + 1,
            boon_id: self.id.clone(),
            level: self.level,
            max_level: spec.definitions[&self.id].levels.len() as u8,
        }
    }
}

impl World {
    pub(super) fn boon_level(&self, i: usize, id: &str) -> Option<u8> {
        self.members[i]
            .boons
            .as_ref()?
            .slots
            .iter()
            .flatten()
            .find(|slot| slot.id == id)
            .map(|slot| slot.level)
    }

    fn boon_source_available(
        &self,
        i: usize,
        index: usize,
        source: &BoonOpportunity,
        c: &Config,
    ) -> Result<()> {
        let m = &self.members[i];
        if source.human_only && m.ai.is_some() {
            return Err(Error::Unauthorized);
        }
        if m.boons.as_ref().unwrap().used & (1 << index) != 0
            || matches!(source.scope, BoonScope::Shared)
                && self.shared_boon_claims[index] >= source.shared_uses.unwrap_or(1)
        {
            return Err(Error::Conflict);
        }
        let owns_reservation = |m: &Member| {
            m.boons
                .as_ref()
                .and_then(|b| b.pending.as_ref())
                .is_some_and(|pending| pending.source_index == index)
        };
        if owns_reservation(m)
            || matches!(source.scope, BoonScope::Shared)
                && source.interaction_ms.is_some()
                && self.members.iter().any(owns_reservation)
        {
            return Err(Error::Conflict);
        }
        if source
            .requires_checkpoint
            .as_ref()
            .is_some_and(|id| !m.checkpoint_completed(c, id))
        {
            return Err(Error::InvalidState);
        }
        if self.npcs.iter().any(|npc| {
            (npc.hp > 0 || !self.mode_npc_visible(&npc.entity.id))
                && source.guardians.contains(&npc.entity.id)
        }) {
            return Err(Error::GuardianAlive);
        }
        Ok(())
    }

    fn boon_plan<'a>(
        &self,
        i: usize,
        input: BoonClaim<'_>,
        spec: &'a WorldBoons,
        c: &Config,
        now: u64,
    ) -> Result<ClaimPlan<'a>> {
        let m = &self.members[i];
        let state = m.boons.as_ref().expect("enabled at deployment");
        if !m.eligible()
            || state.pending.is_some()
            || m.extract_end.is_some()
            || m.pending.is_some()
            || m.channel.is_some()
            || m.basic_beam.is_some()
            || m.basic_charge.is_some()
            || m.basic_pursuit.is_some()
            || self.has_container_reservation(i)
            || self.member_has_status(i, StatusAttribute::Stun, now)
        {
            return Err(Error::InvalidState);
        }
        if state.revision != input.expected_revision
            || m.revision != input.expected_inventory_revision
        {
            return Err(Error::RevisionConflict);
        }
        let (index, source) = spec
            .opportunities
            .iter()
            .enumerate()
            .find(|(_, source)| source.id == input.opportunity_id)
            .ok_or(Error::InvalidTarget)?;
        if source.interaction_ms.is_some() && (m.queued_cast.is_some() || m.queued_basic.is_some())
        {
            return Err(Error::InvalidState);
        }
        if distance(m.position, source.position) > spec.interaction_range {
            return Err(Error::OutOfRange);
        }
        if !self
            .grid
            .line(m.position, source.position, c.params.player_radius, 0)
            || !self.grid.line(m.position, source.position, 0., 2)
        {
            return Err(Error::InvalidTarget);
        }
        self.boon_source_available(i, index, source, c)?;
        let held = state
            .slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|s| s.id == input.boon_id));
        let (slot, level) = match &source.offer {
            BoonOffer::Grant { choices } => {
                if !choices.iter().any(|id| id == input.boon_id) {
                    return Err(Error::InvalidTarget);
                }
                if held.is_some() {
                    return Err(Error::Conflict);
                }
                let slot = match (
                    state.slots.iter().position(Option::is_none),
                    input.replace_slot,
                ) {
                    (Some(slot), None) => slot,
                    (None, Some(slot)) if (1..=spec.capacity).contains(&slot) => {
                        usize::from(slot - 1)
                    }
                    (None, None) => return Err(Error::Capacity),
                    _ => return Err(Error::InvalidTarget),
                };
                (slot, 1)
            }
            BoonOffer::Upgrade {} => {
                if input.replace_slot.is_some() {
                    return Err(Error::InvalidTarget);
                }
                let slot = held.ok_or(Error::InvalidTarget)?;
                let current = state.slots[slot].as_ref().unwrap();
                if usize::from(current.level) >= spec.definitions[&current.id].levels.len() {
                    return Err(Error::Capacity);
                }
                (slot, current.level + 1)
            }
        };
        if spec.definitions[input.boon_id].shield.is_some()
            && state.slots.iter().enumerate().any(|(i, row)| {
                i != slot
                    && row
                        .as_ref()
                        .is_some_and(|row| spec.definitions[&row.id].shield.is_some())
            })
        {
            return Err(Error::Capacity);
        }
        let cost = source
            .cost
            .as_ref()
            .map(|cost| {
                m.plan_acquired_removal(
                    &cost.item_id,
                    cost.quantity,
                    input.expected_inventory_revision,
                )
                .map_err(|error| {
                    if error == Error::InvalidTarget {
                        Error::NoResource
                    } else {
                        error
                    }
                })
            })
            .transpose()?;
        Ok(ClaimPlan {
            source,
            source_index: index,
            slot,
            cost,
            new_slot: Slot {
                id: input.boon_id.to_owned(),
                level,
            },
        })
    }

    pub(super) fn claim_boon(
        &mut self,
        i: usize,
        input: BoonClaim<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let spec = c.world_boons.as_ref().ok_or(Error::Unsupported)?;
        let plan = self.boon_plan(i, input, spec, c, now)?;
        if let Some(duration) = plan.source.interaction_ms {
            let completes_at_ms = now.saturating_add(duration);
            if completes_at_ms >= self.active_at.unwrap() + c.terminal_ms {
                return Err(Error::InvalidState);
            }
            let view = BoonPendingClaimView {
                opportunity_id: input.opportunity_id.to_owned(),
                boon_id: input.boon_id.to_owned(),
                completes_at_ms,
            };
            self.members[i].boons.as_mut().unwrap().pending = Some(PendingClaim {
                view: view.clone(),
                source_index: plan.source_index,
                expected_revision: input.expected_revision,
                expected_inventory_revision: input.expected_inventory_revision,
                replace_slot: input.replace_slot,
            });
            private(events, i, Event::BoonClaimStarted { claim: view });
        } else {
            self.commit_boon(i, plan, c, now, events);
        }
        Ok(())
    }

    pub(super) fn advance_boon_claims(&mut self, c: &Config, now: u64, events: &mut Events) {
        let Some(spec) = &c.world_boons else { return };
        for i in 0..self.members.len() {
            let Some(pending) = self.members[i]
                .boons
                .as_mut()
                .and_then(|b| b.pending.take())
            else {
                continue;
            };
            match self.boon_plan(i, pending.input(), spec, c, now) {
                Ok(plan) if now >= pending.view.completes_at_ms => {
                    self.commit_boon(i, plan, c, now, events)
                }
                Ok(_) => self.members[i].boons.as_mut().unwrap().pending = Some(pending),
                Err(reason) => private(
                    events,
                    i,
                    Event::BoonClaimInterrupted {
                        opportunity_id: pending.view.opportunity_id,
                        reason,
                    },
                ),
            }
        }
    }

    pub(super) fn cancel_boon_claim(&mut self, i: usize, reason: Error, events: &mut Events) {
        if let Some(pending) = self.members[i]
            .boons
            .as_mut()
            .and_then(|b| b.pending.take())
        {
            private(
                events,
                i,
                Event::BoonClaimInterrupted {
                    opportunity_id: pending.view.opportunity_id,
                    reason,
                },
            );
        }
    }

    fn commit_boon(
        &mut self,
        i: usize,
        plan: ClaimPlan<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let spec = c.world_boons.as_ref().unwrap();
        self.advance_attack_clocks(c, now);
        let m = &mut self.members[i];
        if let Some(cost) = plan.cost {
            m.commit_acquired_removal(
                &plan.source.cost.as_ref().unwrap().item_id,
                cost,
                super::loot_drop::RemovalKind::Consumed,
            );
        }
        let state = m.boons.as_mut().unwrap();
        state.used |= 1 << plan.source_index;
        if matches!(plan.source.scope, BoonScope::Shared) {
            self.shared_boon_claims[plan.source_index] += 1;
        }
        let view = plan.new_slot.view(plan.slot, spec);
        let old = state.slots[plan.slot]
            .replace(plan.new_slot)
            .filter(|old| old.id != view.boon_id);
        if spec.definitions[&view.boon_id].shield.is_some() && view.level == 1 {
            state.shield_ready_at = Some(now);
        } else if old
            .as_ref()
            .is_some_and(|old| spec.definitions[&old.id].shield.is_some())
        {
            state.shield_ready_at = None;
        }
        state.revision += 1;
        state.spirit_cost_bps = state
            .slots
            .iter()
            .flatten()
            .filter_map(|slot| {
                spec.definitions[&slot.id]
                    .spirit_cost_bps
                    .get(usize::from(slot.level - 1))
                    .copied()
            })
            .min()
            .unwrap_or(10000);
        let revision = state.revision;
        let mut bonuses = BTreeMap::new();
        for slot in state.slots.iter().flatten() {
            for modifier in &spec.definitions[&slot.id].levels[usize::from(slot.level - 1)] {
                *bonuses.entry(modifier.attribute).or_default() += i64::from(modifier.modifier_bps);
            }
        }
        let entity = status_entity(&m.reference.entity);
        self.statuses.set_passive_bonuses(entity.clone(), bonuses);
        if let Some(old) = old {
            self.remove_boon_effects(&entity, Some(&old.id), events);
        }
        self.reconcile_caps(c, now);
        private(
            events,
            i,
            Event::BoonClaimed {
                opportunity_id: plan.source.id.clone(),
                slot: view,
                revision,
            },
        );
        self.rebuild_boon_shield(i, c, now, events);
    }

    fn remove_boon_effects(
        &mut self,
        source: &StatusEntity,
        id: Option<&str>,
        events: &mut Events,
    ) {
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        self.statuses.remove_boon_statuses(source, id, &mut raw);
        self.route_status_events(raw, &before, events);
    }

    pub(super) fn clear_boons(&mut self, i: usize, events: &mut Events) {
        let m = &mut self.members[i];
        if let Some(state) = &mut m.boons
            && state.slots.iter().any(Option::is_some)
        {
            state.slots.fill(None);
            state.revision += 1;
            state.spirit_cost_bps = 10000;
            state.shield_ready_at = None;
            let source = status_entity(&m.reference.entity);
            self.statuses.clear_passive_bonuses(&source);
            self.remove_boon_effects(&source, None, events);
        }
    }

    fn boon_shield<'a>(&'a self, i: usize, c: &'a Config) -> Option<(&'a Slot, &'a BoonShield)> {
        let definitions = &c.world_boons.as_ref()?.definitions;
        self.members[i]
            .boons
            .as_ref()?
            .slots
            .iter()
            .flatten()
            .find_map(|slot| Some((slot, definitions[&slot.id].shield.as_ref()?)))
    }

    pub(super) fn rebuild_boon_shield(
        &mut self,
        i: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let member = &self.members[i];
        if !member.eligible()
            || member
                .boons
                .as_ref()
                .and_then(|b| b.shield_ready_at)
                .is_none_or(|at| now < at)
        {
            return;
        }
        let Some((slot, shield)) = self.boon_shield(i, c) else {
            return;
        };
        let entity = &member.reference.entity;
        let maximum = self.entity_max_hp(entity, c, now).unwrap();
        let target = status_entity(entity);
        let id = slot.id.clone();
        let bps = shield.capacity_bps[usize::from(slot.level - 1)];
        let rules = c.pve.as_ref().unwrap().status_rules.as_ref().unwrap();
        let mut raw = vec![];
        if self.statuses.grant_boon_shield(
            &target,
            &id,
            (maximum, bps),
            (now, self.active_at.unwrap() + c.terminal_ms),
            rules,
            &mut raw,
        ) {
            self.members[i].boons.as_mut().unwrap().shield_ready_at = None;
            self.route_status_events(raw, &[], events);
        }
    }

    pub(super) fn note_boon_shield_depletion(&mut self, i: usize, c: &Config, now: u64) {
        let Some((_, shield)) = self.boon_shield(i, c) else {
            return;
        };
        if !self
            .statuses
            .has_boon_shield(&status_entity(&self.members[i].reference.entity), now)
        {
            let ready = now.saturating_add(shield.rebuild_ms);
            self.members[i]
                .boons
                .as_mut()
                .unwrap()
                .shield_ready_at
                .get_or_insert(ready);
        }
    }

    pub(super) fn boon_view(&self, i: usize, c: &Config) -> Option<BoonView> {
        let spec = c.world_boons.as_ref()?;
        let m = &self.members[i];
        let state = m.boons.as_ref()?;
        let opportunities = if m.living() {
            spec.opportunities
                .iter()
                .enumerate()
                .filter(|(_, source)| {
                    (!source.human_only || m.ai.is_none())
                        && distance(m.position, source.position) <= spec.interaction_range
                        && self.grid.line(m.position, source.position, 0., 2)
                })
                .map(|(index, source)| BoonOpportunityView {
                    id: source.id.clone(),
                    position: source.position,
                    offer: source.offer.clone(),
                    available: self.boon_source_available(i, index, source, c).is_ok(),
                    shared_remaining: source
                        .shared_uses
                        .map(|uses| uses - self.shared_boon_claims[index]),
                })
                .collect()
        } else {
            vec![]
        };
        Some(BoonView {
            capacity: spec.capacity,
            revision: state.revision,
            slots: state
                .slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.as_ref().map(|s| s.view(i, spec)))
                .collect(),
            opportunities,
            pending_claim: state.pending.as_ref().map(|p| p.view.clone()),
        })
    }

    pub(super) fn participant_ai_boon(
        &self,
        i: usize,
        c: &Config,
        radius: f64,
        now: u64,
    ) -> Option<Action> {
        let spec = c.world_boons.as_ref()?;
        let m = &self.members[i];
        let state = m.boons.as_ref()?;
        if radius == 0. {
            return None;
        }
        for source in &spec.opportunities {
            if source.human_only || distance(m.position, source.position) > radius {
                continue;
            }
            let eligible = |id: &str| {
                self.boon_plan(
                    i,
                    BoonClaim {
                        opportunity_id: &source.id,
                        boon_id: id,
                        expected_revision: state.revision,
                        expected_inventory_revision: m.revision,
                        replace_slot: None,
                    },
                    spec,
                    c,
                    now,
                )
                .is_ok()
            };
            let selected = match &source.offer {
                BoonOffer::Grant { choices } => choices.iter().find(|id| eligible(id)),
                BoonOffer::Upgrade {} => state
                    .slots
                    .iter()
                    .flatten()
                    .map(|slot| &slot.id)
                    .find(|id| eligible(id)),
            };
            if let Some(boon_id) = selected {
                return Some(Action::ClaimBoon {
                    opportunity_id: source.id.clone(),
                    boon_id: boon_id.clone(),
                    expected_revision: state.revision,
                    expected_inventory_revision: m.revision,
                    replace_slot: None,
                });
            }
        }
        None
    }
}
