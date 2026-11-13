//! Bounded counters, typed conditions and atomic status consumption.
use super::stack_counters::TriggerOutcome;
use super::*;
use crate::config::{
    CombatFact, CombatTrigger, TriggerCounting, TriggerEffect, TriggerScope, TriggerSubject,
};
pub(super) type EntityKey = (u8, String, u32);
pub(super) type TriggerKey = (EntityKey, String, Option<EntityKey>);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ActionKey {
    pub(super) source: EntityKey,
    pub(super) id: String,
}
pub(super) fn entity_key(e: &EntityRef) -> EntityKey {
    (
        u8::from(e.kind == StatusEntityKind::Monster),
        e.id.clone(),
        e.generation,
    )
}
impl ActionKey {
    pub(super) fn source_ref(&self) -> EntityRef {
        EntityRef {
            kind: if self.source.0 == 0 {
                StatusEntityKind::Participant
            } else {
                StatusEntityKind::Monster
            },
            id: self.source.1.clone(),
            generation: self.source.2,
        }
    }
    pub(super) fn new(source: &EntityRef, id: &str) -> Self {
        Self {
            source: entity_key(source),
            id: id.into(),
        }
    }
}
pub(super) struct TriggerInput {
    pub(super) origin: Option<crate::origin::ActionOrigin>,
    pub(super) actors: (Owner, Owner),
    pub(super) other: EntityRef,
    pub(super) amount: u64,
    pub(super) action: Option<ActionKey>,
    pub(super) contact: Option<super::contact_status::ContactAugment>,
}
#[derive(Default)]
struct Counter {
    seen_actions: BTreeMap<ActionKey, u32>,
    count: u32,
    ready: u64,
}
#[derive(Default)]
pub(super) struct TriggerState {
    pub(super) origin: Option<crate::origin::ActionOrigin>,
    pub(super) contacts: super::contact_status::ContactActions,
    pub(super) checkpoint_actions: BTreeMap<ActionKey, u32>,
    pub(super) death_used: std::collections::BTreeSet<EntityKey>,
    pub(super) status_death_ready: BTreeMap<EntityKey, BTreeMap<String, u64>>,
    pub(super) action: Option<String>,
    counters: BTreeMap<TriggerKey, Counter>,
    pub(super) action_source: Option<EntityRef>,
    pub(super) totals: super::action_totals::Totals,
    pub(super) executing: bool,
    step: Option<u64>,
    used: usize,
    pub(super) chain_used: usize,
    budget_reported: Vec<EntityRef>,
}
impl TriggerState {
    pub(super) fn clear(&mut self, e: &EntityRef) {
        let key = entity_key(e);
        self.death_used.remove(&key);
        self.status_death_ready.remove(&key);
        self.counters.retain(|(owner, _, _), _| *owner != key);
        self.totals.clear_owner(&key);
        self.contacts.clear(&key);
        self.checkpoint_actions
            .retain(|action, _| action.source != key);
    }
}
impl World {
    pub(super) fn active_trigger_actions(&self) -> std::collections::BTreeSet<ActionKey> {
        self.shots
            .iter()
            .map(|s| ActionKey::new(&s.view.source, &s.action_id))
            .chain(
                self.members
                    .iter()
                    .filter_map(|m| m.basic_projectile.as_ref().map(|plan| plan.action.clone())),
            )
            .chain(
                self.areas.iter().filter_map(|p| {
                    Some(ActionKey::new(p.trigger_source(), p.trigger_action_id()?))
                }),
            )
            .chain(
                self.basic_waves
                    .iter()
                    .map(super::basic_wave::BasicWavePlan::action),
            )
            .chain(self.basic_impacts.iter().map(|plan| plan.action.clone()))
            .chain(self.members.iter().filter_map(|m| {
                m.basic_beam
                    .as_ref()
                    .map(super::basic_beam::BasicBeam::action)
            }))
            .chain(self.members.iter().filter_map(|m| {
                m.channel
                    .as_ref()
                    .map(|c| ActionKey::new(&m.reference.entity, &c.action_id))
            }))
            .collect()
    }
    pub(super) fn prune_trigger_actions(&mut self) {
        if self.triggers.counters.is_empty()
            && self.triggers.contacts.empty()
            && self.triggers.checkpoint_actions.is_empty()
        {
            return;
        }
        let active = self.active_trigger_actions();
        self.triggers.contacts.prune(&active);
        self.triggers
            .checkpoint_actions
            .retain(|action, _| active.contains(action));
        for counter in self.triggers.counters.values_mut() {
            counter.seen_actions.retain(|id, _| active.contains(id));
        }
        self.prune_trigger_targets();
    }
    pub(super) fn prune_trigger_targets(&mut self) {
        let held = self.triggers.totals.held_counters();
        let live = self
            .members
            .iter()
            .filter(|m| m.living())
            .map(|m| entity_key(&m.reference.entity))
            .chain(
                self.npcs
                    .iter()
                    .filter(|n| n.hp > 0 && !n.returning)
                    .map(|n| entity_key(&n.entity)),
            )
            .collect::<std::collections::BTreeSet<_>>();
        self.triggers.counters.retain(|key, _| {
            live.contains(&key.0)
                && (key.2.as_ref().is_none_or(|target| live.contains(target)) || held.contains(key))
        });
    }
    pub(super) fn trigger_step(&mut self, now: u64) {
        if self.triggers.step != Some(now) {
            self.triggers.step = Some(now);
            self.triggers.used = 0;
            self.triggers.chain_used = 0;
            self.triggers.budget_reported.clear();
        }
    }
    fn trigger_condition(
        &self,
        t: &CombatTrigger,
        actors: (Owner, Owner),
        amount: u64,
        c: &Config,
        now: u64,
    ) -> Option<Option<(String, u32)>> {
        let (owner, other) = actors;
        if t.owner_skill
            .as_ref()
            .is_some_and(|r| !self.meets_skill_requirement(owner, r, c))
        {
            return None;
        }
        if amount < u64::from(t.conditions.minimum_amount) {
            return None;
        }
        for (actor, threshold) in [
            (owner, t.conditions.owner_hp_at_most_bps),
            (other, t.conditions.other_hp_at_most_bps),
        ] {
            if let Some(threshold) = threshold {
                let e = self.owner_entity(actor);
                let max = self.entity_max_hp(&e, c, now)?;
                let hp = self.current_hp(actor);
                if u128::from(hp) * 10000 > u128::from(max) * u128::from(threshold) {
                    return None;
                }
            }
        }
        let Some(requirement) = &t.conditions.status else {
            return Some(None);
        };
        let source = status_entity(&self.owner_entity(owner));
        let target = status_entity(&self.owner_entity(match requirement.subject {
            TriggerSubject::Owner => owner,
            TriggerSubject::Other => other,
        }));
        let instance = self.statuses.entries().iter().find(|s| {
            s.source == source
                && s.target == target
                && s.definition_id == requirement.definition_id
                && now < s.expires_at_ms
                && s.stacks.unwrap_or(1) >= requirement.at_least
        })?;
        Some((requirement.consume > 0).then(|| (instance.instance_id.clone(), requirement.consume)))
    }
    pub(super) fn suppress_trigger(
        &mut self,
        e: &EntityRef,
        id: &str,
        reason: Error,
        events: &mut Events,
    ) {
        if self.triggers.budget_reported.contains(e) {
            return;
        }
        self.triggers.budget_reported.push(e.clone());
        for i in self.status_observers(&status_entity(e), &status_entity(e)) {
            private(
                events,
                i,
                Event::CombatTriggerSuppressed {
                    trigger_id: id.into(),
                    source: status_entity(e),
                    reason,
                },
            );
        }
    }
    fn trigger_has_capacity(
        &self,
        actors: (Owner, Owner),
        effect: (
            &TriggerEffect,
            Option<&super::contact_status::ContactAugment>,
            Option<&str>,
        ),
        consume: Option<&(String, u32)>,
        amount: u64,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let (effect, contact, boon) = effect;
        if matches!(effect, TriggerEffect::StealOther { .. }) {
            let source = self.owner_entity(actors.0);
            let target = self.owner_entity(actors.1);
            return if self.hostile_contact(&source, &target)
                && self.valid_status_entity(&status_entity(&source))
                && self.valid_status_entity(&status_entity(&target))
            {
                Ok(())
            } else {
                Err(Error::InvalidTarget)
            };
        }

        if let TriggerEffect::ReduceSkillCooldowns {
            skill_ids,
            remaining_bps,
            fixed_ms,
        } = effect
        {
            return if self
                .plan_cooldown_reduction(actors.0, skill_ids, *remaining_bps, *fixed_ms, now)
                .is_empty()
            {
                Err(Error::InvalidState)
            } else {
                Ok(())
            };
        }
        if !matches!(
            effect,
            TriggerEffect::ApplySelfStatus { .. }
                | TriggerEffect::ApplyStatusBundle { .. }
                | TriggerEffect::RefreshStatusDuration { .. }
                | TriggerEffect::AddStatusStacks { .. }
                | TriggerEffect::ApplyOtherStatus { .. }
                | TriggerEffect::DispelStatuses { .. }
                | TriggerEffect::TransferStatuses { .. }
                | TriggerEffect::ChainDamage { .. }
                | TriggerEffect::FinishOther { .. }
        ) {
            return Ok(());
        }
        let e = self.owner_entity(actors.0);
        let mut preview = self.statuses.clone();
        preview.set_boon_context(boon);
        let mut ignored = vec![];
        preview.expire(now, &mut ignored);
        if let Some((id, count)) = consume
            && !preview.consume(id, *count, &mut ignored)
        {
            return Err(Error::InvalidState);
        }
        if let TriggerEffect::RefreshStatusDuration {
            subject,
            status_id,
            duration_ms,
        } = effect
        {
            return self
                .plan_status_refresh(
                    &preview,
                    actors,
                    (*subject, status_id, *duration_ms),
                    c,
                    now,
                )
                .map(|_| ());
        }
        if let TriggerEffect::ApplyStatusBundle {
            owner_statuses,
            other_statuses,
        } = effect
        {
            return self.preview_status_bundle(
                &preview,
                actors,
                (owner_statuses, other_statuses),
                c,
                now,
            );
        }
        if let TriggerEffect::AddStatusStacks {
            subject, status_id, ..
        } = effect
        {
            return self
                .plan_counter_adjustment(&preview, actors, *subject, status_id, c, now)
                .map(|_| ());
        }
        if let TriggerEffect::ApplyOtherStatus { status_id } = effect {
            return self.preview_other_status(&preview, actors, status_id, c, now);
        }
        if let TriggerEffect::FinishOther { rule } = effect {
            return self
                .plan_finish_other(&preview, actors, rule, c, now, contact)
                .map(|_| ());
        }
        if let TriggerEffect::ChainDamage { rule } = effect {
            return self.preview_chain_damage(&preview, actors, amount, rule, c, now);
        }
        let TriggerEffect::ApplySelfStatus { status_id } = effect else {
            return if matches!(
                effect,
                TriggerEffect::DispelStatuses { .. } | TriggerEffect::TransferStatuses { .. }
            ) {
                self.plan_status_operation(&preview, actors, effect, c, now)
                    .map(|_| ())
            } else {
                Ok(())
            };
        };
        if c.combat_rules.as_ref().is_some_and(|r| {
            crate::config::status_blocked(
                &preview,
                status_id,
                &status_entity(&e),
                &r.status_policies,
                now,
            )
        }) {
            return Err(Error::Immune);
        }
        if preview.can_apply(
            &Self::definitions(c, std::slice::from_ref(status_id)),
            &status_entity(&e),
            &status_entity(&e),
            c.pve.as_ref().unwrap().status_rules.as_ref().unwrap(),
        ) {
            Ok(())
        } else {
            Err(Error::Capacity)
        }
    }
    pub(super) fn combat_fact(
        &mut self,
        fact: CombatFact,
        actors: (Owner, Owner),
        amount: (u32, bool),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        self.combat_fact_with_consumption((fact, None), actors, amount, c, now, events);
    }
    pub(super) fn combat_fact_with_consumption(
        &mut self,
        fact: (CombatFact, Option<(&str, u32)>),
        actors: (Owner, Owner),
        amount: (u32, bool),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        self.combat_fact_with_context((fact.0, fact.1, None), actors, amount, c, now, events);
    }
    pub(super) fn combat_fact_with_context(
        &mut self,
        fact: (
            CombatFact,
            Option<(&str, u32)>,
            Option<&super::contact_status::ContactAugment>,
        ),
        actors: (Owner, Owner),
        amount: (u32, bool),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let (fact, consumed, contact) = fact;
        if self.triggers.executing {
            return;
        }
        let Some(r) = &c.combat_rules else {
            return;
        };
        self.reconcile_caps(c, now);
        let (damage, periodic) = amount;
        let (owner, other) = actors;
        if matches!(owner, Owner::Summon(_)) {
            return;
        }
        let e = self.owner_entity(owner);
        if !self.valid_status_entity(&status_entity(&e)) {
            return;
        }
        let profile = self.combat_profile_id(owner, r);
        let mut definitions = r
            .triggers
            .iter()
            .filter(|t| {
                t.when == fact
                    && t.action_filter
                        .as_ref()
                        .is_none_or(|f| f.accepts(self.triggers.origin.as_ref()))
                    && t.when != CombatFact::FinisherKill
                    && t.when != CombatFact::StatusStacksReached
                    && t.profiles.iter().any(|id| id == profile)
                    && (!periodic || t.include_periodic)
                    && t.contact_consumption.as_ref().is_none_or(|f| {
                        consumed
                            .is_some_and(|(id, count)| id == f.definition_id && count >= f.at_least)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        if definitions.is_empty() {
            return;
        }
        definitions.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.id.cmp(&b.id)));
        self.trigger_step(now);
        let origin = self.triggers.action_source.clone().unwrap_or_else(|| {
            self.owner_entity(
                if matches!(fact, CombatFact::DamageTaken | CombatFact::ShieldAbsorbed) {
                    other
                } else {
                    owner
                },
            )
        });
        let action = self
            .triggers
            .action
            .as_ref()
            .map(|id| ActionKey::new(&origin, id));
        let mut outcomes = vec![];
        self.triggers.executing = true;
        for definition in definitions {
            if !self.valid_status_entity(&status_entity(&e)) {
                break;
            }
            let input = TriggerInput {
                origin: self.triggers.origin.clone(),
                actors,
                other: self.owner_entity(other),
                amount: u64::from(damage),
                action: action.clone(),
                contact: contact.cloned(),
            };
            if definition.counting == TriggerCounting::ActionTotal {
                self.collect_action_total(definition, input, &origin, c, events);
            } else {
                self.run_combat_trigger(&definition, input, c, now, events, &mut outcomes);
            }
        }
        self.triggers.executing = false;
        self.reconcile_caps(c, now);
        self.dispatch_trigger_outcomes(outcomes, c, now, events);
    }
    pub(super) fn run_combat_trigger(
        &mut self,
        definition: &CombatTrigger,
        input: TriggerInput,
        c: &Config,
        now: u64,
        events: &mut Events,
        outcomes: &mut Vec<TriggerOutcome>,
    ) -> bool {
        if definition
            .action_filter
            .as_ref()
            .is_some_and(|f| !f.accepts(input.origin.as_ref()))
        {
            return false;
        }
        let action = input.action.clone();
        let e = self.owner_entity(input.actors.0);
        let boon = definition
            .requires_boon
            .as_ref()
            .map(|gate| gate.id.as_str());
        let effect = if let Some(gate) = &definition.requires_boon {
            let (Owner::Member(i), other) = input.actors else {
                return false;
            };
            if !self.members[i].eligible()
                || matches!(other, Owner::Summon(_))
                || gate.npc_only && !matches!(other, Owner::Npc(_))
                || self.record_shields.executing
                || self.window_records.executing
                || self.status_ends.executing
            {
                return false;
            }
            let Some(level) = self.boon_level(i, &gate.id) else {
                return false;
            };
            usize::from(level)
                .checked_sub(2)
                .and_then(|index| gate.higher_level_effects.get(index))
                .unwrap_or(&definition.effect)
        } else {
            &definition.effect
        };
        let Some(consume) = self.trigger_condition(definition, input.actors, input.amount, c, now)
        else {
            return false;
        };
        let r = c.combat_rules.as_ref().unwrap();
        if self.triggers.used >= r.max_triggers_per_step {
            self.suppress_trigger(&e, &definition.id, Error::Capacity, events);
            return false;
        }
        let key = (
            entity_key(&e),
            definition.id.clone(),
            (definition.counter_scope == TriggerScope::Target).then(|| entity_key(&input.other)),
        );
        if key.2.is_some() && !self.triggers.counters.contains_key(&key) {
            self.prune_trigger_targets();
        }
        if !self.triggers.counters.contains_key(&key)
            && key.2.is_some()
            && self
                .triggers
                .counters
                .keys()
                .filter(|k| k.0 == key.0 && k.1 == key.1)
                .count()
                >= r.max_trigger_targets_per_rule
        {
            self.suppress_trigger(&e, &definition.id, Error::Capacity, events);
            return false;
        }
        if definition.maximum_per_action > 1 && input.action.is_none() {
            return false;
        }
        let count = self.triggers.counters.entry(key.clone()).or_default();
        if definition.counting == TriggerCounting::Action
            && let Some(action) = input.action
        {
            if count.seen_actions.get(&action).copied().unwrap_or(0)
                >= definition.maximum_per_action
            {
                return false;
            }
            if count.seen_actions.len() >= 256 && !count.seen_actions.contains_key(&action) {
                self.suppress_trigger(&e, &definition.id, Error::Capacity, events);
                return false;
            }
            *count.seen_actions.entry(action).or_default() += 1;
        }
        count.count = count.count.saturating_add(1).min(definition.every);
        if count.count < definition.every || now < count.ready {
            return false;
        }
        if let Err(reason) = self.trigger_has_capacity(
            input.actors,
            (effect, input.contact.as_ref(), boon),
            consume.as_ref(),
            input.amount,
            c,
            now,
        ) {
            self.suppress_trigger(&e, &definition.id, reason, events);
            return false;
        }
        if definition
            .chance_bps
            .is_some_and(|p| !self.roll(f64::from(p) / 10000.))
        {
            return false;
        }
        let steal_budget = if let TriggerEffect::StealOther { steal } = effect {
            self.resource_budget(
                &self.owner_entity(input.actors.0),
                &self.owner_entity(input.actors.1),
                steal,
                c,
                now,
            )
        } else {
            None
        };
        if let Some((id, stacks)) = consume {
            let before = self.statuses.entries().to_vec();
            let mut raw = vec![];
            if !self.statuses.consume(&id, stacks, &mut raw) {
                return false;
            }
            self.route_status_events(raw, &before, events);
        }
        let count = self.triggers.counters.get_mut(&key).unwrap();
        count.count = 0;
        count.ready = now + definition.cooldown_ms;
        self.triggers.used += 1;
        for i in self.status_observers(&status_entity(&e), &status_entity(&e)) {
            private(
                events,
                i,
                Event::CombatTriggerFired {
                    trigger_id: definition.id.clone(),
                    source: status_entity(&e),
                },
            );
        }
        if let Some(mut outcome) = self.execute_trigger_effect(
            (&definition.id, effect.clone(), input.contact.as_ref(), boon),
            input.actors,
            (input.amount, steal_budget),
            c,
            now,
            events,
        ) {
            match &mut outcome {
                TriggerOutcome::Finisher(f) => {
                    f.action = action;
                    f.origin = input.origin.clone();
                }
                TriggerOutcome::Stacks(f) => {
                    f.action = action;
                    f.origin = input.origin.clone();
                }
            }
            outcomes.push(outcome);
        }
        true
    }
    fn execute_trigger_effect(
        &mut self,
        effect: (
            &str,
            TriggerEffect,
            Option<&super::contact_status::ContactAugment>,
            Option<&str>,
        ),
        actors: (Owner, Owner),
        facts: (u64, Option<[u32; 2]>),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<TriggerOutcome> {
        let (damage, steal_budget) = facts;
        let (owner, other) = actors;
        let (trigger_id, effect, contact, boon) = effect;
        let e = self.owner_entity(owner);
        match effect {
            TriggerEffect::RestorePaidSpirit { bps } => {
                let Owner::Member(i) = owner else { return None };
                let requested =
                    (u128::from(damage) * u128::from(bps) / 10000).min(u128::from(u32::MAX)) as u32;
                let gap = self.members[i].spirit_recovery_gap(
                    &self.statuses,
                    now,
                    self.member_max_spirit(i, c, now),
                );
                let restored = requested.min(gap);
                self.members[i].spirit += restored;
                private(
                    events,
                    i,
                    Event::PaidSpiritRestored {
                        source: e,
                        trigger_id: trigger_id.to_owned(),
                        action_id: self.triggers.action.clone(),
                        paid_spirit: damage as u32,
                        requested,
                        restored,
                        excess: requested - restored,
                    },
                );
            }
            TriggerEffect::RefreshStatusDuration {
                subject,
                status_id,
                duration_ms,
            } => self.refresh_status_duration(
                actors,
                (subject, &status_id, duration_ms),
                c,
                now,
                events,
            ),
            TriggerEffect::ApplyStatusBundle {
                owner_statuses,
                other_statuses,
            } => self.execute_status_bundle(
                actors,
                (&owner_statuses, &other_statuses),
                c,
                now,
                events,
            ),
            TriggerEffect::AddStatusStacks {
                subject,
                status_id,
                amount,
                refresh_duration,
            } => {
                return self
                    .add_status_stacks(
                        actors,
                        (subject, &status_id, amount, refresh_duration),
                        c,
                        now,
                        events,
                    )
                    .map(TriggerOutcome::Stacks);
            }
            TriggerEffect::StealOther { steal } => {
                let target = self.owner_entity(other);
                if let Some(budget) = steal_budget {
                    let action = self.triggers.action.clone();
                    self.steal_resources(
                        (owner, &target, action.as_deref()),
                        &steal,
                        budget,
                        c,
                        now,
                        events,
                    );
                }
            }
            TriggerEffect::FinishOther { rule } => {
                return self
                    .execute_finisher(trigger_id, actors, (&rule, contact), c, now, events)
                    .map(TriggerOutcome::Finisher);
            }
            TriggerEffect::ReduceSkillCooldowns {
                skill_ids,
                remaining_bps,
                fixed_ms,
            } => {
                self.execute_cooldown_reduction(
                    trigger_id,
                    owner,
                    &skill_ids,
                    (remaining_bps, fixed_ms),
                    now,
                    events,
                );
            }
            TriggerEffect::ChainDamage { rule } => {
                self.execute_chain_damage((trigger_id, actors, damage), &rule, c, now, events);
            }
            TriggerEffect::ApplyOtherStatus { status_id } => {
                let target = self.owner_entity(other);
                if let Some(boon) = boon {
                    self.apply_boon_status(owner, target, (boon, status_id), c, now, events);
                    return None;
                }
                let max = self
                    .entity_max_hp(&target, c, now)
                    .expect("preflight target");
                self.apply_owned_statuses(owner, (target, max), &[status_id], c, now, events);
            }
            effect @ (TriggerEffect::DispelStatuses { .. }
            | TriggerEffect::TransferStatuses { .. }) => {
                self.execute_status_operation(actors, &effect, c, now, events);
            }
            TriggerEffect::HealFromDamage { bps } => {
                self.ratio_self_heal(&e, damage, (bps, HealKind::Triggered), c, now, events)
            }
            TriggerEffect::RestoreSpirit {
                amount,
                max_spirit_bps,
            } => {
                let maximum = self.entity_max_spirit(&e, c, now).unwrap();
                let amount = (u128::from(amount)
                    + u128::from(maximum) * u128::from(max_spirit_bps) / 10000)
                    .min(u128::from(u32::MAX)) as u32;
                match owner {
                    Owner::Member(i) => {
                        let gap = self.members[i].spirit_recovery_gap(
                            &self.statuses,
                            now,
                            self.member_max_spirit(i, c, now),
                        );
                        self.members[i].spirit += amount.min(gap);
                    }
                    Owner::Npc(j) => {
                        let gap = self
                            .entity_max_spirit(&self.npcs[j].entity, c, now)
                            .unwrap()
                            .saturating_sub(self.npcs[j].spirit);
                        self.npcs[j].spirit += amount.min(gap);
                    }
                    Owner::Summon(_) => {}
                }
            }
            TriggerEffect::ApplySelfStatus { status_id } => {
                if let Some(boon) = boon {
                    self.apply_boon_status(owner, e, (boon, status_id), c, now, events);
                    return None;
                }
                let max = self.entity_max_hp(&e, c, now).unwrap();
                self.apply_owned_statuses(owner, (e, max), &[status_id], c, now, events);
            }
            TriggerEffect::ReflectDamage {
                bps,
                damage_kind,
                element,
            } => {
                let raw =
                    (u128::from(damage) * u128::from(bps) / 10000).min(u128::from(u32::MAX)) as u32;
                if raw > 0 {
                    self.resolve_damage_with_rule(
                        combat::DamageIntent {
                            ignore_armor: false,
                            source: e,
                            target: self.owner_entity(other),
                            raw,
                            direct: false,
                        },
                        Some(&crate::config::AttackRule {
                            damage_kind,
                            element,
                            can_crit: false,
                            ..Default::default()
                        }),
                        c,
                        now,
                        events,
                    );
                }
            }
        }
        None
    }
}
