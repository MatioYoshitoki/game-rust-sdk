use super::*;
impl Statuses {
    pub(super) fn apply_periodic(
        &mut self,
        d: &StatusDefinition,
        source: (&StatusEntity, u32),
        target: (&StatusEntity, u32),
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let (source, attack) = source;
        let (target, max_hp) = target;
        let existing = self.existing(d, source, target);
        if let Some(i) = existing
            && matches!(d.duplicate_policy, DuplicatePolicy::Ignore)
        {
            events.push(Event::StatusIgnored {
                instance_id: self.entries[i].instance_id.clone(),
            });
            return;
        }
        let interval = d.tick_interval_ms.unwrap();
        let expires = now + d.duration_ms;
        let stacked = d.periodic_stack.is_some();
        let added = if d.periodic_added_stacks > 0 {
            d.periodic_added_stacks
        } else {
            d.periodic_stack
                .and_then(|policy| policy.stacks_per_application)
                .unwrap_or(1)
        };
        let old_next = existing
            .and_then(|i| self.entries[i].periodic.as_ref())
            .and_then(|p| p.next_tick_at_ms);
        let first = match d.periodic_stack.map(|p| p.tick_policy) {
            Some(crate::config::PeriodicStackTick::Preserve) => {
                old_next.map_or(now + interval, |due| {
                    if due < now {
                        due + (now - due).div_ceil(interval) * interval
                    } else {
                        due
                    }
                })
            }
            _ => now + interval,
        };
        let last = expires.saturating_sub(u64::from(!d.periodic_tick_on_expiry));
        let total = if first <= last {
            (last - first) / interval + 1
        } else {
            0
        };
        let periodic = PeriodicView {
            tick_on_expiry: d.periodic_tick_on_expiry.then_some(true),
            per_stack: stacked.then_some(true),
            first_tick_at_ms: stacked.then_some(first),
            raw_spirit: d.periodic_raw_spirit,
            power_sampling: d.periodic_sampling,
            kind: if d.is_heal() {
                PeriodicEffectKind::MaxHpHeal
            } else {
                match d.periodic_damage_kind.unwrap_or_default() {
                    crate::config::DamageKind::Physical => PeriodicEffectKind::PhysicalDamage,
                    crate::config::DamageKind::Elemental => PeriodicEffectKind::ElementalDamage,
                    crate::config::DamageKind::TrueDamage => PeriodicEffectKind::TrueDamage,
                }
            },
            interval_ms: interval,
            next_tick_at_ms: (total > 0).then_some(first),
            ticks_done: 0,
            total_ticks: total as u32,
            raw_damage: if d.is_heal() {
                0
            } else {
                d.periodic_raw_damage.unwrap_or_else(|| {
                    (u64::from(attack) * u64::from(d.damage_bps.unwrap()) / 10000)
                        .min(u64::from(u32::MAX)) as u32
                })
            },
            raw_heal: d.is_heal().then(|| {
                d.periodic_raw_heal.unwrap_or_else(|| {
                    (u64::from(max_hp) * u64::from(d.heal_bps.unwrap()) / 10000)
                        .min(u64::from(u32::MAX)) as u32
                })
            }),
        };
        if let Some(i) = existing {
            let state = &mut self.entries[i];
            Self::refresh_window(state, d);
            let before = state.stacks.unwrap_or(1);
            state.stacks = d
                .max_stacks
                .map(|cap| before.saturating_add(added).min(cap));
            state.max_stacks = d.max_stacks;
            state.cleanse = d.cleanse.clone();
            state.source = source.clone();
            state.applied_at_ms = now;
            state.expires_at_ms = expires;
            state.periodic = Some(periodic);
            state.modifiers = d.modifiers.clone();
            state.flags = d.flags.clone();
            if let Some(inputs) = d.periodic_inputs {
                self.periodic_inputs
                    .insert(state.instance_id.clone(), inputs);
            } else {
                self.periodic_inputs.remove(&state.instance_id);
            }
            if stacked {
                events.push(Event::StatusStacked {
                    status: state.clone(),
                    added_stacks: state.stacks.unwrap() - before,
                });
            } else {
                events.push(Event::StatusRefreshed {
                    status: state.clone(),
                });
            }
        } else {
            self.sequence += 1;
            let state = StatusView {
                retained_source: None,
                base_modifiers: vec![],
                stack_counter: None,
                cleanse: d.cleanse.clone(),
                flags: d.flags.clone(),
                window: d.window.clone(),
                modifiers: d.modifiers.clone(),
                periodic: Some(periodic),
                shield: None,
                attribute: None,
                stacks: d.max_stacks.map(|cap| added.min(cap)),
                max_stacks: d.max_stacks,
                instance_id: format!("status-{}", self.sequence),
                definition_id: d.id.clone(),
                source: source.clone(),
                target: target.clone(),
                applied_at_ms: now,
                expires_at_ms: expires,
                modifier_bps: 0,
            };
            if let Some(inputs) = d.periodic_inputs {
                self.periodic_inputs
                    .insert(state.instance_id.clone(), inputs);
            }
            events.push(Event::StatusApplied {
                status: state.clone(),
            });
            self.push_status(state);
        }
        if let Some(i) = self.existing(d, source, target) {
            let id = self.entries[i].instance_id.clone();
            if let Some(cap) = d
                .periodic_stack
                .and_then(|policy| policy.healing_reduction_cap_bps)
            {
                self.periodic_heal_caps.insert(id.clone(), cap);
            } else {
                self.periodic_heal_caps.remove(&id);
            }
            self.register_context(&id, d);
        }
    }
    pub(crate) fn periodic_inputs(&self, id: &str) -> Option<PeriodicInputs> {
        self.periodic_inputs.get(id).copied()
    }
    pub fn record_periodic_sample(
        &mut self,
        id: &str,
        raw_damage: u32,
        raw_heal: Option<u32>,
        raw_spirit: Option<u32>,
    ) {
        if let Some(p) = self
            .entries
            .iter_mut()
            .find(|s| s.instance_id == id)
            .and_then(|s| s.periodic.as_mut())
        {
            p.raw_damage = raw_damage;
            p.raw_heal = raw_heal;
            p.raw_spirit = raw_spirit;
        }
    }
    pub fn take_periodic_due(&mut self, now: u64) -> Option<(StatusView, u64)> {
        let key = |s: &StatusView| {
            let p = s.periodic.as_ref().unwrap();
            (
                p.next_tick_at_ms,
                match p.kind {
                    PeriodicEffectKind::PhysicalDamage
                    | PeriodicEffectKind::ElementalDamage
                    | PeriodicEffectKind::TrueDamage => 0,
                    PeriodicEffectKind::MaxHpHeal => 1,
                },
            )
        };
        let index = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.periodic.as_ref().is_some_and(|p| {
                    (now < s.expires_at_ms || p.due_on_expiry(s.expires_at_ms))
                        && p.next_tick_at_ms.is_some_and(|t| t <= now)
                })
            })
            .min_by(|(_, a), (_, b)| {
                key(a)
                    .cmp(&key(b))
                    .then_with(|| a.instance_id.cmp(&b.instance_id))
            })
            .map(|(i, _)| i)?;
        let s = &mut self.entries[index];
        let periodic = s.periodic.as_mut().unwrap();
        let due = periodic.next_tick_at_ms.unwrap();
        periodic.ticks_done += 1;
        let next = due + periodic.interval_ms;
        periodic.next_tick_at_ms = (next < s.expires_at_ms
            || periodic.tick_on_expiry == Some(true) && next == s.expires_at_ms)
            .then_some(next);
        Some((s.clone(), due))
    }
}
