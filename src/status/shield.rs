use super::*;

pub(crate) struct ShieldGrant {
    requested: u32,
    remaining: u32,
}

fn shield_limit(max_hp: u32, bps: u32) -> u32 {
    (u64::from(max_hp) * u64::from(bps) / 10000) as u32
}

impl Statuses {
    pub(crate) fn ordinary_shield_room(
        &self,
        target: &StatusEntity,
        now: u64,
        (max_hp, ordinary_bps): (u32, u32),
        except: Option<usize>,
    ) -> Option<u32> {
        let mut ordinary = 0u64;
        let mut special = 0u64;
        let mut shared_bps = ordinary_bps;
        for (i, s) in self.entries.iter().enumerate() {
            if Some(i) == except || s.target != *target || now >= s.expires_at_ms {
                continue;
            }
            let Some(shield) = &s.shield else { continue };
            if let Some(bps) = self.boon_shields.get(&s.instance_id) {
                special += u64::from(shield.remaining);
                shared_bps = shared_bps.max(*bps);
            } else {
                ordinary += u64::from(shield.remaining);
            }
        }
        let ordinary_room = u64::from(shield_limit(max_hp, ordinary_bps)).checked_sub(ordinary)?;
        let shared_room =
            u64::from(shield_limit(max_hp, shared_bps)).checked_sub(ordinary + special)?;
        Some(ordinary_room.min(shared_room) as u32)
    }

    pub(crate) fn has_boon_shield(&self, target: &StatusEntity, now: u64) -> bool {
        self.entries.iter().any(|s| {
            s.target == *target
                && now < s.expires_at_ms
                && self.boon_shields.contains_key(&s.instance_id)
        })
    }

    pub(crate) fn grant_boon_shield(
        &mut self,
        target: &StatusEntity,
        boon: &str,
        (max_hp, bps): (u32, u32),
        (now, end): (u64, u64),
        rules: &StatusRules,
        events: &mut Vec<Event>,
    ) -> bool {
        if now >= end
            || self.has_boon_shield(target, now)
            || self
                .entries
                .iter()
                .filter(|s| s.target == *target && now < s.expires_at_ms)
                .count()
                >= rules.max_active_per_entity
        {
            return false;
        }
        let requested = shield_limit(max_hp, bps);
        let shared_limit = shield_limit(max_hp, bps.max(rules.shield_cap_bps.unwrap()));
        let remaining = requested.min(shared_limit.saturating_sub(self.shield_total(target, now)));
        if remaining == 0 {
            return false;
        }
        self.sequence += 1;
        let id = format!("status-{}", self.sequence);
        let status = StatusView {
            instance_id: id.clone(),
            definition_id: boon.to_owned(),
            source: target.clone(),
            target: target.clone(),
            retained_source: None,
            base_modifiers: vec![],
            stack_counter: None,
            cleanse: None,
            flags: vec![],
            window: None,
            modifiers: vec![],
            periodic: None,
            attribute: None,
            stacks: None,
            max_stacks: None,
            modifier_bps: 0,
            applied_at_ms: now,
            expires_at_ms: end,
            shield: Some(ShieldView {
                record: None,
                granted: remaining,
                remaining,
            }),
        };
        self.boon_shields.insert(id.clone(), bps);
        events.push(Event::StatusApplied {
            status: status.clone(),
        });
        self.entries.push(status);
        events.push(Event::ShieldGranted {
            source: target.clone(),
            target: target.clone(),
            definition_id: boon.to_owned(),
            instance_id: Some(id),
            requested,
            granted: remaining,
            remaining,
        });
        true
    }

    pub(super) fn family_shield(
        &self,
        family: &str,
        source: &StatusEntity,
        target: &StatusEntity,
    ) -> Option<usize> {
        self.entries.iter().position(|s| {
            s.source == *source
                && s.target == *target
                && self.same_status_context(&s.instance_id)
                && self
                    .shield_families
                    .get(&s.instance_id)
                    .is_some_and(|key| key == family)
        })
    }

    pub(super) fn apply_shield(
        &mut self,
        d: &StatusDefinition,
        source: &StatusEntity,
        target: (&StatusEntity, u32),
        now: u64,
        rules: &StatusRules,
        events: &mut Vec<Event>,
    ) {
        let (target, max_hp) = target;
        let grouped = d
            .shield_family
            .as_ref()
            .and_then(|family| self.family_shield(family, source, target));
        let existing = grouped.or_else(|| self.existing(d, source, target));
        if let Some(i) = existing
            && matches!(d.duplicate_policy, DuplicatePolicy::Ignore)
        {
            events.push(Event::StatusIgnored {
                instance_id: self.entries[i].instance_id.clone(),
            });
            return;
        }
        let requested = shield_limit(max_hp, d.shield_bps.unwrap());
        let room = self
            .ordinary_shield_room(
                target,
                now,
                (max_hp, rules.shield_cap_bps.unwrap()),
                existing,
            )
            .unwrap_or(0);
        let previous = existing
            .filter(|_| d.shield_preserve_higher)
            .map_or(0, |i| self.entries[i].shield.as_ref().unwrap().remaining);
        let remaining = requested.max(previous).min(room);
        if let Some(i) = grouped {
            let shield = &self.entries[i];
            let previous = shield.shield.as_ref().unwrap().remaining;
            if remaining <= previous {
                events.push(Event::StatusIgnored {
                    instance_id: shield.instance_id.clone(),
                });
                return;
            }
            if shield.definition_id != d.id {
                let id = shield.instance_id.clone();
                self.remove(StatusRemovalReason::Reset, |s| s.instance_id == id, events);
            }
        }
        self.write_shield(
            d,
            source,
            target,
            now,
            ShieldGrant {
                requested,
                remaining,
            },
            events,
        );
    }

    pub(crate) fn plan_overheal_shield(
        &self,
        definition: &StatusDefinition,
        (source, target, max_hp): (&StatusEntity, &StatusEntity, u32),
        now: u64,
        rules: &StatusRules,
        requested: u32,
    ) -> Result<Option<ShieldGrant>> {
        let existing = self
            .existing(definition, source, target)
            .filter(|&i| self.entries[i].expires_at_ms > now);
        let balance = existing.map_or(0, |i| self.entries[i].shield.as_ref().unwrap().remaining);
        let own_limit = shield_limit(max_hp, definition.shield_bps.unwrap());
        let room = self
            .ordinary_shield_room(target, now, (max_hp, rules.shield_cap_bps.unwrap()), None)
            .unwrap_or(0);
        let increase = requested.min(own_limit.saturating_sub(balance)).min(room);
        if increase == 0 {
            return Ok(None);
        }
        if existing.is_none()
            && self
                .entries
                .iter()
                .filter(|s| s.target == *target && now < s.expires_at_ms)
                .count()
                >= rules.max_active_per_entity
        {
            return Err(Error::Capacity);
        }
        Ok(Some(ShieldGrant {
            requested,
            remaining: balance + increase,
        }))
    }

    pub(crate) fn apply_overheal_shield(
        &mut self,
        definition: &StatusDefinition,
        source: &StatusEntity,
        target: &StatusEntity,
        now: u64,
        grant: ShieldGrant,
        events: &mut Vec<Event>,
    ) {
        self.expire(now, events);
        self.write_shield(definition, source, target, now, grant, events);
    }

    fn write_shield(
        &mut self,
        d: &StatusDefinition,
        source: &StatusEntity,
        target: &StatusEntity,
        now: u64,
        grant: ShieldGrant,
        events: &mut Vec<Event>,
    ) {
        let ShieldGrant {
            requested,
            remaining,
        } = grant;
        let existing = self.existing(d, source, target);
        let mut record = d.shield_record.clone();
        if d.shield_record_preserve
            && let (Some(r), Some(i)) = (&mut record, existing)
            && let Some(old) = self.entries[i]
                .shield
                .as_ref()
                .and_then(|v| v.record.as_ref())
        {
            r.recorded = old.recorded.min(r.cap);
        }
        let modifiers = if d.shield_record_reduction > 0 {
            vec![StatusModifier {
                attribute: StatusAttribute::DamageReduction,
                modifier_bps: d.shield_record_reduction as i32,
            }]
        } else {
            vec![]
        };
        if record.is_some()
            && remaining == 0
            && let Some(i) = existing
        {
            let id = self.entries[i].instance_id.clone();
            self.remove(StatusRemovalReason::Reset, |s| s.instance_id == id, events);
            events.push(Event::ShieldGranted {
                source: source.clone(),
                target: target.clone(),
                definition_id: d.id.clone(),
                instance_id: None,
                requested,
                granted: 0,
                remaining: 0,
            });
            return;
        }
        let mut old = 0;
        let instance_id = if let Some(i) = existing {
            let s = &mut self.entries[i];
            old = s.shield.as_ref().unwrap().remaining;
            s.source = source.clone();
            s.applied_at_ms = now;
            s.expires_at_ms = now + d.duration_ms;
            s.modifiers = modifiers;
            s.shield = Some(ShieldView {
                record,
                granted: remaining,
                remaining,
            });
            events.push(Event::StatusRefreshed { status: s.clone() });
            Some(s.instance_id.clone())
        } else if remaining > 0 {
            self.sequence += 1;
            let s = StatusView {
                retained_source: None,
                base_modifiers: vec![],
                stack_counter: None,
                cleanse: d.cleanse.clone(),
                flags: d.flags.clone(),
                window: None,
                modifiers,
                periodic: None,
                shield: Some(ShieldView {
                    record,
                    granted: remaining,
                    remaining,
                }),
                attribute: None,
                stacks: None,
                max_stacks: None,
                instance_id: format!("status-{}", self.sequence),
                definition_id: d.id.clone(),
                source: source.clone(),
                target: target.clone(),
                applied_at_ms: now,
                expires_at_ms: now + d.duration_ms,
                modifier_bps: 0,
            };
            events.push(Event::StatusApplied { status: s.clone() });
            let id = s.instance_id.clone();
            self.push_status(s);
            Some(id)
        } else {
            None
        };
        if let Some(id) = &instance_id {
            if let Some(family) = &d.shield_family {
                self.shield_families.insert(id.clone(), family.clone());
            }
            self.register_context(id, d);
        }
        events.push(Event::ShieldGranted {
            source: source.clone(),
            target: target.clone(),
            definition_id: d.id.clone(),
            instance_id,
            requested,
            granted: remaining.saturating_sub(old),
            remaining,
        });
    }
    pub fn shield_total(&self, target: &StatusEntity, now: u64) -> u32 {
        self.entries
            .iter()
            .filter(|s| s.target == *target && now < s.expires_at_ms)
            .filter_map(|s| s.shield.as_ref().map(|v| v.remaining))
            .sum()
    }
    pub fn absorb(
        &mut self,
        target: &StatusEntity,
        amount: u32,
        now: u64,
        source: &str,
        events: &mut Vec<Event>,
    ) -> u32 {
        self.absorb_recording(target, amount, now, (source, true, false), events)
    }
    pub fn record_incoming(&mut self, target: &StatusEntity, amount: u32, now: u64) {
        for s in &mut self.entries {
            if s.target == *target
                && now < s.expires_at_ms
                && let Some(r) = s.shield.as_mut().and_then(|v| v.record.as_mut())
                && r.basis == ShieldRecordBasis::Incoming
            {
                r.recorded = r.recorded.saturating_add(amount).min(r.cap);
            }
        }
    }
    pub fn absorb_recording(
        &mut self,
        target: &StatusEntity,
        amount: u32,
        now: u64,
        source: (&str, bool, bool),
        events: &mut Vec<Event>,
    ) -> u32 {
        let (source, recording, allow_boon_shield) = source;
        if amount == 0 || self.entries.is_empty() {
            return amount;
        }
        self.expire(now, events);
        let mut order: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.target == *target
                    && s.shield.is_some()
                    && (allow_boon_shield || !self.boon_shields.contains_key(&s.instance_id))
            })
            .map(|(i, _)| i)
            .collect();
        order.sort_by(|&a, &b| {
            let a = &self.entries[a];
            let b = &self.entries[b];
            (
                self.boon_shields.contains_key(&a.instance_id),
                a.expires_at_ms,
                a.applied_at_ms,
                &a.instance_id,
            )
                .cmp(&(
                    self.boon_shields.contains_key(&b.instance_id),
                    b.expires_at_ms,
                    b.applied_at_ms,
                    &b.instance_id,
                ))
        });
        let mut left = amount;
        let mut consumptions = vec![];
        for i in order {
            if left == 0 {
                break;
            }
            let s = &mut self.entries[i];
            let shield = s.shield.as_mut().unwrap();
            let absorbed = left.min(shield.remaining);
            left -= absorbed;
            shield.remaining -= absorbed;
            if recording
                && let Some(r) = &mut shield.record
                && r.basis == ShieldRecordBasis::OwnAbsorbed
            {
                r.recorded = r.recorded.saturating_add(absorbed).min(r.cap);
            }
            if absorbed > 0 {
                consumptions.push(ShieldConsumption {
                    instance_id: s.instance_id.clone(),
                    absorbed,
                    remaining: shield.remaining,
                });
            }
        }
        if left < amount {
            self.remove(
                StatusRemovalReason::Depleted,
                |s| s.target == *target && s.shield.as_ref().is_some_and(|v| v.remaining == 0),
                events,
            );
            events.push(Event::ShieldAbsorbed {
                source: source.into(),
                target: target.clone(),
                amount: amount - left,
                consumptions,
            });
        }
        left
    }
}
