//! Bounded transient status instances. No clocks, I/O, callbacks or permanent facts.
use crate::config::{DuplicatePolicy, StatusDefinition, StatusRules};
use crate::protocol::*;
mod clock;
mod counter;
mod derived;
mod periodic;
mod retaliation;
mod shield;
pub(crate) use shield::ShieldGrant;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PeriodicInputs {
    pub attack: u32,
    pub max_hp: u32,
    pub max_spirit: u32,
}

#[derive(Clone, Default)]
pub(crate) struct Statuses {
    end_context: derived::EndContext,
    retaliation: retaliation::RetaliationState,
    periodic_inputs: std::collections::BTreeMap<String, PeriodicInputs>,
    periodic_heal_caps: std::collections::BTreeMap<String, u32>,
    entries: Vec<StatusView>,
    passive_bonuses: Vec<(
        StatusEntity,
        std::collections::BTreeMap<StatusAttribute, i64>,
    )>,
    remove_zero_counters: std::collections::BTreeSet<String>,
    sequence: u64,
    shield_families: std::collections::BTreeMap<String, String>,
    applying_boon: Option<String>,
    boon_origins: std::collections::BTreeMap<String, String>,
    boon_shields: std::collections::BTreeMap<String, u32>,
    shield_ends: Vec<(StatusView, StatusRemovalReason)>,
    window_ends: Vec<(StatusView, StatusRemovalReason)>,
    cleanse_ends: Vec<(StatusView, StatusRemovalReason)>,
}
impl Statuses {
    pub(crate) fn set_boon_context(&mut self, boon: Option<&str>) {
        self.applying_boon = boon.map(str::to_owned);
    }

    fn same_status_context(&self, instance_id: &str) -> bool {
        !self.boon_shields.contains_key(instance_id)
            && self.boon_origins.get(instance_id) == self.applying_boon.as_ref()
    }

    fn push_status(&mut self, status: StatusView) {
        if let Some(boon) = &self.applying_boon {
            self.boon_origins
                .insert(status.instance_id.clone(), boon.clone());
        }
        self.entries.push(status);
    }

    pub(crate) fn remove_boon_statuses(
        &mut self,
        source: &StatusEntity,
        boon: Option<&str>,
        events: &mut Vec<Event>,
    ) {
        let ids = self
            .entries
            .iter()
            .filter(|s| {
                s.source == *source
                    && (self
                        .boon_origins
                        .get(&s.instance_id)
                        .is_some_and(|id| boon.is_none_or(|boon| boon == id))
                        || self.boon_shields.contains_key(&s.instance_id)
                            && boon.is_none_or(|boon| boon == s.definition_id))
            })
            .map(|s| s.instance_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let reason = if boon.is_some() {
            StatusRemovalReason::Reset
        } else {
            StatusRemovalReason::SourceInvalid
        };
        self.remove(reason, |s| ids.contains(&s.instance_id), events);
    }

    pub(crate) fn set_passive_bonuses(
        &mut self,
        target: StatusEntity,
        modifiers: std::collections::BTreeMap<StatusAttribute, i64>,
    ) {
        self.clear_passive_bonuses(&target);
        if !modifiers.is_empty() {
            self.passive_bonuses.push((target, modifiers));
        }
    }

    pub(crate) fn clear_passive_bonuses(&mut self, target: &StatusEntity) {
        self.passive_bonuses.retain(|(entity, _)| entity != target);
    }

    fn existing(
        &self,
        d: &StatusDefinition,
        source: &StatusEntity,
        target: &StatusEntity,
    ) -> Option<usize> {
        self.entries.iter().position(|s| {
            s.target == *target
                && s.definition_id == d.id
                && (!d.source_scoped || s.source == *source)
                && self.same_status_context(&s.instance_id)
        })
    }
    pub fn can_apply(
        &self,
        defs: &[StatusDefinition],
        source: &StatusEntity,
        target: &StatusEntity,
        rules: &StatusRules,
    ) -> bool {
        let used = self.entries.iter().filter(|s| s.target == *target).count();
        let mut families = std::collections::BTreeSet::new();
        let new = defs
            .iter()
            .filter(|d| match &d.shield_family {
                Some(family) => {
                    self.family_shield(family, source, target).is_none() && families.insert(family)
                }
                None => self.existing(d, source, target).is_none(),
            })
            .count();
        used + new <= rules.max_active_per_entity
    }
    pub fn apply(
        &mut self,
        defs: &[StatusDefinition],
        source: (&StatusEntity, u32),
        target: (&StatusEntity, u32),
        now: u64,
        rules: &StatusRules,
        events: &mut Vec<Event>,
    ) -> bool {
        let (source_entity, source_attack) = source;
        let source = source_entity;
        let (target_entity, max_hp) = target;
        let target = target_entity;
        if defs.is_empty() {
            return true;
        }
        self.expire(now, events);
        if !self.can_apply(defs, source, target, rules) {
            events.push(Event::StatusRejected {
                source: source.clone(),
                target: target.clone(),
                definition_ids: defs.iter().map(|d| d.id.clone()).collect(),
                reason: Error::Capacity,
            });
            return false;
        }
        for d in defs {
            if d.is_periodic() {
                self.apply_periodic(d, (source, source_attack), (target, max_hp), now, events);
                continue;
            }
            if d.is_shield() {
                self.apply_shield(d, source, (target, max_hp), now, rules, events);
                continue;
            }
            if let Some(i) = self.existing(d, source, target) {
                if matches!(d.duplicate_policy, DuplicatePolicy::Ignore) {
                    events.push(Event::StatusIgnored {
                        instance_id: self.entries[i].instance_id.clone(),
                    });
                    continue;
                }
                let s = &mut self.entries[i];
                Self::refresh_window(s, d);
                let previous_stacks = s.stacks.unwrap_or(1);
                if matches!(d.duplicate_policy, DuplicatePolicy::Stack) {
                    let cap = d.max_stacks.expect("validated stack definition");
                    s.stacks = Some(
                        previous_stacks
                            .saturating_add(d.application_stacks.unwrap_or(1))
                            .min(cap),
                    );
                    s.max_stacks = Some(cap);
                }
                if let Some(counter) = &d.stack_counter {
                    s.stacks = Some(counter.initial);
                    s.max_stacks = Some(counter.maximum);
                }
                if d.stack_counter
                    .as_ref()
                    .is_some_and(|v| v.removes_at_zero())
                {
                    self.remove_zero_counters.insert(s.instance_id.clone());
                } else {
                    self.remove_zero_counters.remove(&s.instance_id);
                }
                s.stack_counter = d.stack_counter.as_ref().map(|_| true);
                s.base_modifiers = d.base_modifiers.clone();
                s.source = source.clone();
                s.retained_source = d.retained_source.then_some(true);
                s.applied_at_ms = now;
                s.expires_at_ms = now + d.duration_ms;
                s.modifier_bps = d.modifier_bps;
                s.modifiers = d.modifiers.clone();
                s.cleanse = d.cleanse.clone();
                s.flags = d.flags.clone();
                if matches!(d.duplicate_policy, DuplicatePolicy::Stack) {
                    events.push(Event::StatusStacked {
                        status: s.clone(),
                        added_stacks: s.stacks.unwrap() - previous_stacks,
                    });
                } else {
                    events.push(Event::StatusRefreshed { status: s.clone() });
                }
            } else {
                self.sequence += 1;
                let s = StatusView {
                    retained_source: d.retained_source.then_some(true),
                    base_modifiers: d.base_modifiers.clone(),
                    stack_counter: d.stack_counter.as_ref().map(|_| true),
                    cleanse: d.cleanse.clone(),
                    flags: d.flags.clone(),
                    window: d.window.clone(),
                    modifiers: d.modifiers.clone(),
                    periodic: None,
                    shield: None,
                    attribute: d.attribute,
                    stacks: d.stack_counter.as_ref().map(|v| v.initial).or_else(|| {
                        matches!(d.duplicate_policy, DuplicatePolicy::Stack)
                            .then(|| d.application_stacks.unwrap_or(1).min(d.max_stacks.unwrap()))
                    }),
                    max_stacks: d.stack_counter.as_ref().map(|v| v.maximum).or(d.max_stacks),
                    instance_id: format!("status-{}", self.sequence),
                    definition_id: d.id.clone(),
                    source: source.clone(),
                    target: target.clone(),
                    applied_at_ms: now,
                    expires_at_ms: now + d.duration_ms,
                    modifier_bps: d.modifier_bps,
                };
                if d.stack_counter
                    .as_ref()
                    .is_some_and(|v| v.removes_at_zero())
                {
                    self.remove_zero_counters.insert(s.instance_id.clone());
                }
                events.push(Event::StatusApplied { status: s.clone() });
                self.push_status(s);
            }
            if let Some(i) = self.existing(d, source, target) {
                let id = self.entries[i].instance_id.clone();
                self.retaliation.clear(&id);
                self.register_context(&id, d);
            }
        }
        true
    }
    pub fn remove(
        &mut self,
        reason: StatusRemovalReason,
        predicate: impl Fn(&StatusView) -> bool,
        events: &mut Vec<Event>,
    ) {
        let mut instance_ids = vec![];
        let ends = &mut self.shield_ends;
        let shield_families = &mut self.shield_families;
        let boon_origins = &mut self.boon_origins;
        let boon_shields = &mut self.boon_shields;
        let window_ends = &mut self.window_ends;
        let cleanse_ends = &mut self.cleanse_ends;
        let periodic_inputs = &mut self.periodic_inputs;
        let periodic_heal_caps = &mut self.periodic_heal_caps;
        let remove_zero_counters = &mut self.remove_zero_counters;
        let end_context = &mut self.end_context;
        let retaliation = &mut self.retaliation;
        self.entries.retain(|s| {
            if predicate(s) {
                end_context.remove(s, reason);
                retaliation.remove(s, reason);
                shield_families.remove(&s.instance_id);
                boon_origins.remove(&s.instance_id);
                boon_shields.remove(&s.instance_id);
                periodic_inputs.remove(&s.instance_id);
                periodic_heal_caps.remove(&s.instance_id);
                remove_zero_counters.remove(&s.instance_id);
                if s.cleanse.as_ref().is_some_and(|v| v.on_expired) {
                    cleanse_ends.push((s.clone(), reason));
                }
                if s.window.is_some() {
                    window_ends.push((s.clone(), reason));
                }
                if s.shield.as_ref().is_some_and(|v| v.record.is_some()) {
                    ends.push((s.clone(), reason));
                }
                instance_ids.push(s.instance_id.clone());
                false
            } else {
                true
            }
        });
        if !instance_ids.is_empty() {
            instance_ids.sort();
            events.push(Event::StatusRemoved {
                instance_ids,
                reason,
            });
        }
    }
    pub fn expire(&mut self, now: u64, events: &mut Vec<Event>) {
        self.remove(
            StatusRemovalReason::Expired,
            |s| {
                s.expires_at_ms <= now
                    && !s
                        .periodic
                        .as_ref()
                        .is_some_and(|p| p.due_on_expiry(s.expires_at_ms))
            },
            events,
        );
    }
    pub fn restore_shield_ends(&mut self, mut prior: Vec<(StatusView, StatusRemovalReason)>) {
        prior.append(&mut self.shield_ends);
        self.shield_ends = prior;
    }
    pub fn take_shield_ends(&mut self) -> Vec<(StatusView, StatusRemovalReason)> {
        std::mem::take(&mut self.shield_ends)
    }
    pub fn take_cleanse_ends(&mut self) -> Vec<(StatusView, StatusRemovalReason)> {
        std::mem::take(&mut self.cleanse_ends)
    }
    pub fn restore_cleanse_ends(&mut self, mut prior: Vec<(StatusView, StatusRemovalReason)>) {
        prior.append(&mut self.cleanse_ends);
        self.cleanse_ends = prior;
    }
    pub fn take_window_ends(&mut self) -> Vec<(StatusView, StatusRemovalReason)> {
        std::mem::take(&mut self.window_ends)
    }
    pub fn restore_window_ends(&mut self, mut prior: Vec<(StatusView, StatusRemovalReason)>) {
        prior.append(&mut self.window_ends);
        self.window_ends = prior;
    }
    fn refresh_window(status: &mut StatusView, definition: &StatusDefinition) {
        if !definition.window_preserve || status.window.is_none() {
            status.window = definition.window.clone();
        }
    }
    pub fn record_windows(
        &mut self,
        (source, target): (&StatusEntity, &StatusEntity),
        (hp, absorbed, periodic): (u32, u32, bool),
        now: u64,
        rules: &std::collections::BTreeMap<String, crate::config::StatusWindowRule>,
    ) {
        for s in &mut self.entries {
            if now >= s.expires_at_ms {
                continue;
            }
            let Some(rule) = rules.get(&s.definition_id) else {
                continue;
            };
            if periodic && !rule.include_periodic {
                continue;
            }
            let Some(record) = &mut s.window else {
                continue;
            };
            let amount = match record.basis {
                WindowBasis::DamageDealt if s.target == *source => hp,
                WindowBasis::DamageTaken if s.target == *target => hp,
                WindowBasis::ShieldAbsorbed if s.target == *target => absorbed,
                _ => 0,
            };
            if amount > 0 {
                record.recorded = record.cap.min(record.recorded + u64::from(amount));
                record.contacts = record.contacts.saturating_add(1);
            }
        }
    }
    pub fn consume(&mut self, id: &str, amount: u32, events: &mut Vec<Event>) -> bool {
        let Some(s) = self.entries.iter_mut().find(|s| s.instance_id == id) else {
            return false;
        };
        let stacks = s.stacks.unwrap_or(1);
        if amount == 0 || amount > stacks {
            return false;
        }
        let remaining = stacks - amount;
        events.push(Event::StatusConsumed {
            instance_id: s.instance_id.clone(),
            definition_id: s.definition_id.clone(),
            source: s.source.clone(),
            target: s.target.clone(),
            consumed_stacks: amount,
            remaining_stacks: remaining,
        });
        if remaining == 0
            && (s.stack_counter != Some(true) || self.remove_zero_counters.contains(id))
        {
            self.remove(
                StatusRemovalReason::Consumed,
                |s| s.instance_id == id,
                events,
            );
        } else {
            s.stacks = Some(remaining);
        }
        true
    }
    pub fn entries(&self) -> &[StatusView] {
        &self.entries
    }
    /// Caller has preflighted every selected identity, collision and target slot.
    pub fn retarget(&mut self, ids: &[String], target: &StatusEntity, events: &mut Vec<Event>) {
        for id in ids {
            let s = self
                .entries
                .iter_mut()
                .find(|s| s.instance_id == *id)
                .expect("preflighted transfer");
            let previous_target = std::mem::replace(&mut s.target, target.clone());
            events.push(Event::StatusTransferred {
                previous_target,
                status: s.clone(),
            });
        }
    }
    pub fn views(&self, target: &StatusEntity, now: u64) -> Vec<StatusView> {
        let mut v: Vec<_> = self
            .entries
            .iter()
            .filter(|s| s.target == *target && now < s.expires_at_ms)
            .cloned()
            .collect();
        v.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));
        v
    }
    pub fn defense(
        &self,
        target: &StatusEntity,
        base: u32,
        now: u64,
        rules: Option<&StatusRules>,
    ) -> u32 {
        self.attribute(target, base, now, rules, StatusAttribute::Def)
    }
    pub fn attribute(
        &self,
        target: &StatusEntity,
        base: u32,
        now: u64,
        rules: Option<&StatusRules>,
        attribute: StatusAttribute,
    ) -> u32 {
        self.attribute_with_bonus(target, base, now, rules, attribute, 0)
    }
    pub(crate) fn attribute_with_bonus(
        &self,
        target: &StatusEntity,
        base: u32,
        now: u64,
        rules: Option<&StatusRules>,
        attribute: StatusAttribute,
        extra: i64,
    ) -> u32 {
        let Some(rules) = rules else {
            return base;
        };
        let mut capped_periodic = std::collections::BTreeMap::new();
        let passive = self
            .passive_bonuses
            .iter()
            .find(|(entity, _)| entity == target)
            .and_then(|(_, modifiers)| modifiers.get(&attribute))
            .copied()
            .unwrap_or(0);
        let healing = attribute == StatusAttribute::HealingReceived;
        let mut sum = extra + passive;
        let mut ordinary_healing_penalty = 0i64;
        let mut boon_healing_penalty = 0i64;
        for s in self
            .entries
            .iter()
            .filter(|s| s.target == *target && now < s.expires_at_ms)
        {
            let own =
                if s.shield.is_none() && s.attribute.unwrap_or(StatusAttribute::Def) == attribute {
                    i64::from(s.modifier_bps)
                } else {
                    0
                };
            let modifiers = |rows: &[StatusModifier]| {
                rows.iter()
                    .filter(|m| m.attribute == attribute)
                    .map(|m| i64::from(m.modifier_bps))
                    .sum::<i64>()
            };
            let contribution = modifiers(&s.base_modifiers)
                + (own + modifiers(&s.modifiers)) * i64::from(s.stacks.unwrap_or(1));
            if healing {
                let cap = self.periodic_heal_caps.get(&s.instance_id).copied();
                if contribution < 0 && self.boon_origins.contains_key(&s.instance_id) {
                    let penalty = cap.map_or(contribution, |cap| contribution.max(-i64::from(cap)));
                    boon_healing_penalty = boon_healing_penalty.min(penalty);
                    continue;
                }
                if let Some(cap) = cap {
                    let (total, _) = capped_periodic.entry(&s.definition_id).or_insert((0, cap));
                    *total += contribution;
                    continue;
                }
                ordinary_healing_penalty += contribution.min(0);
            }
            sum += contribution;
        }
        for (total, cap) in capped_periodic.into_values() {
            let contribution = total.max(-i64::from(cap));
            sum += contribution;
            ordinary_healing_penalty += contribution.min(0);
        }
        sum += (boon_healing_penalty - ordinary_healing_penalty).min(0);
        let cap = |fallback| rules.cap(attribute, fallback);
        let bounded = match attribute {
            StatusAttribute::MaxHp
            | StatusAttribute::MaxSpirit
            | StatusAttribute::Spd
            | StatusAttribute::AttackSpeed => sum.clamp(-10000, cap(10000)),
            StatusAttribute::Atk => sum.clamp(
                -10000,
                cap(i64::from(rules.attack_bonus_cap_bps.unwrap_or(0))),
            ),
            StatusAttribute::Def | StatusAttribute::ElementResistance => {
                sum.clamp(-10000, cap(i64::from(rules.defense_bonus_cap_bps)))
            }
            StatusAttribute::HealingReceived
            | StatusAttribute::RecoveryStrength
            | StatusAttribute::SpiritRecoveryLock
            | StatusAttribute::LifeSteal => sum.clamp(-10000, cap(i64::MAX)),
            StatusAttribute::SkillPowerBonus => sum.clamp(0, cap(i64::MAX)),
            StatusAttribute::HarmfulDurationReduction => sum.clamp(0, cap(10000)),
            StatusAttribute::BasicIntervalDelay => sum.clamp(0, cap(100000)),
            StatusAttribute::ControlDurationReduction => sum.clamp(
                0,
                cap(i64::from(crate::config::MAX_CONTROL_DURATION_REDUCTION_BPS)),
            ),
            StatusAttribute::CooldownReduction => {
                sum.clamp(0, cap(i64::from(crate::config::MAX_COOLDOWN_REDUCTION_BPS)))
            }
            StatusAttribute::DamageReduction => {
                sum.clamp(0, cap(i64::from(crate::config::MAX_DAMAGE_REDUCTION_BPS)))
            }
            StatusAttribute::ArmorPenetration
            | StatusAttribute::CriticalChance
            | StatusAttribute::DodgePenalty => sum.clamp(0, cap(10000)),
            StatusAttribute::DodgeChance => {
                sum.clamp(0, cap(i64::from(crate::config::MAX_DODGE_BPS)))
            }
            StatusAttribute::Accuracy => {
                sum.clamp(0, cap(i64::from(crate::config::MAX_ACCURACY_BPS)))
            }
            StatusAttribute::CriticalDamageBonus => sum.clamp(
                0,
                cap(i64::from(crate::config::MAX_CRITICAL_DAMAGE_BONUS_BPS)),
            ),
            StatusAttribute::Silence
            | StatusAttribute::Root
            | StatusAttribute::Stun
            | StatusAttribute::MovementSlow
            | StatusAttribute::MovementHaste
            | StatusAttribute::ControlImmunity
            | StatusAttribute::DamageImmunity
            | StatusAttribute::FirstControlImmunity => 0,
        };
        (u128::from(base) * (10000 + bounded) as u128 / 10000).min(u128::from(u32::MAX)) as u32
    }
    pub fn movement_factor_bps(&self, target: &StatusEntity, now: u64) -> u32 {
        let mut haste = 0;
        let mut slow = 0;
        for s in &self.entries {
            if s.target != *target || now >= s.expires_at_ms {
                continue;
            }
            for axis in [
                StatusAttribute::MovementSlow,
                StatusAttribute::MovementHaste,
            ] {
                let own = if s.attribute == Some(axis) {
                    i64::from(s.modifier_bps)
                } else {
                    0
                };
                let layered = own
                    + s.modifiers
                        .iter()
                        .filter(|m| m.attribute == axis)
                        .map(|m| i64::from(m.modifier_bps))
                        .sum::<i64>();
                let fixed = s
                    .base_modifiers
                    .iter()
                    .filter(|m| m.attribute == axis)
                    .map(|m| i64::from(m.modifier_bps))
                    .sum::<i64>();
                let value = fixed + layered * i64::from(s.stacks.unwrap_or(1));
                if axis == StatusAttribute::MovementSlow {
                    slow = slow.min(value.max(-9999) as i32)
                } else {
                    haste = haste.max(value.min(10000) as i32)
                }
            }
        }
        (10000 + haste + slow) as u32
    }
    pub fn healing_received_bps(
        &self,
        target: &StatusEntity,
        now: u64,
        rules: Option<&StatusRules>,
    ) -> u32 {
        self.attribute(target, 10000, now, rules, StatusAttribute::HealingReceived)
    }

    pub fn damage_reduction_bps(
        &self,
        target: &StatusEntity,
        now: u64,
        rules: Option<&StatusRules>,
    ) -> u32 {
        self.attribute(target, 10000, now, rules, StatusAttribute::DamageReduction) - 10000
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
