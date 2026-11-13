//! Bounded status transactions and a pre-HP lethal gate, shared by all damage.
use super::*;
use crate::config::{DeathGuard, TriggerEffect, TriggerSubject};

fn subject(actors: (Owner, Owner), which: TriggerSubject) -> Owner {
    match which {
        TriggerSubject::Owner => actors.0,
        TriggerSubject::Other => actors.1,
    }
}
pub(super) struct StatusSelection<'a> {
    pub disposition: crate::config::StatusDisposition,
    pub max_count: usize,
    pub transferable: bool,
    pub exclude: Option<&'a str>,
    pub applied_since: Option<u64>,
}
fn transferred_control(s: &StatusView) -> bool {
    s.attribute.is_some_and(StatusAttribute::is_control_flag)
        || s.flags.iter().any(|a| a.is_control_flag())
}
impl World {
    pub(super) fn select_statuses<'a>(
        &self,
        statuses: &'a Statuses,
        target: &StatusEntity,
        selection: StatusSelection<'_>,
        c: &Config,
        now: u64,
    ) -> Vec<&'a StatusView> {
        let policies = &c.combat_rules.as_ref().unwrap().status_policies;
        let mut selected = statuses
            .entries()
            .iter()
            .filter(|s| {
                s.target == *target
                    && now < s.expires_at_ms
                    && selection.exclude.is_none_or(|id| s.instance_id != id)
                    && selection
                        .applied_since
                        .is_none_or(|since| s.applied_at_ms >= since)
                    && if s.retained_source == Some(true) {
                        !selection.transferable
                    } else {
                        self.valid_status_entity(&s.source)
                    }
                    && policies.get(&s.definition_id).is_some_and(|p| {
                        p.dispellable
                            && p.disposition == selection.disposition
                            && (!selection.transferable || p.transferable)
                    })
            })
            .collect::<Vec<_>>();
        selected.sort_by(|a, b| {
            a.expires_at_ms
                .cmp(&b.expires_at_ms)
                .then(a.applied_at_ms.cmp(&b.applied_at_ms))
                .then(a.instance_id.cmp(&b.instance_id))
        });
        selected.truncate(selection.max_count);
        selected
    }
    pub(super) fn plan_status_operation(
        &self,
        statuses: &Statuses,
        actors: (Owner, Owner),
        effect: &TriggerEffect,
        c: &Config,
        now: u64,
    ) -> Result<(Vec<String>, Option<StatusEntity>)> {
        let (which, disposition, max_count, transfer) = match effect {
            TriggerEffect::DispelStatuses {
                subject,
                disposition,
                max_count,
            } => (*subject, *disposition, *max_count, false),
            TriggerEffect::TransferStatuses {
                from,
                disposition,
                max_count,
            } => (*from, *disposition, *max_count, true),
            _ => return Err(Error::Unsupported),
        };
        let from_owner = subject(actors, which);
        let from = status_entity(&self.owner_entity(from_owner));
        if !self.status_operation_eligible(from_owner) {
            return Err(Error::InvalidTarget);
        }
        let policies = &c.combat_rules.as_ref().unwrap().status_policies;
        let selected = self.select_statuses(
            statuses,
            &from,
            StatusSelection {
                disposition,
                max_count,
                transferable: transfer,
                exclude: None,
                applied_since: None,
            },
            c,
            now,
        );
        if selected.is_empty() {
            return Err(Error::InvalidState);
        }
        let target = if transfer {
            let to_owner = match which {
                TriggerSubject::Owner => actors.1,
                TriggerSubject::Other => actors.0,
            };
            let to = status_entity(&self.owner_entity(to_owner));
            if to == from || !self.status_operation_eligible(to_owner) {
                return Err(Error::InvalidTarget);
            }
            let definitions = &c.pve.as_ref().unwrap().status_definitions;
            for s in &selected {
                let lock = s.attribute == Some(StatusAttribute::SpiritRecoveryLock);
                if (transferred_control(s) || lock)
                    && self
                        .entity_owner(&s.source)
                        .is_none_or(|owner| !self.status_operation_eligible(owner))
                {
                    return Err(Error::InvalidTarget);
                }
                if (s
                    .periodic
                    .as_ref()
                    .is_some_and(|p| p.kind != PeriodicEffectKind::MaxHpHeal)
                    || transferred_control(s)
                    || lock)
                    && self.status_relation(&s.source, &to)
                        != Some(super::combat::Relation::Hostile)
                    || lock && to.kind != StatusEntityKind::Participant
                {
                    return Err(Error::InvalidTarget);
                }
                if transferred_control(s)
                    && self.control_policy(&to, c).is_some_and(|p| {
                        p.replacements.contains_key(&s.definition_id)
                            || p.max_duration_ms
                                .is_some_and(|cap| s.expires_at_ms.saturating_sub(now) > cap)
                    })
                {
                    return Err(Error::Immune);
                }
                if crate::config::status_blocked(statuses, &s.definition_id, &to, policies, now) {
                    return Err(Error::Immune);
                }
                let d = definitions
                    .iter()
                    .find(|d| d.id == s.definition_id)
                    .unwrap();
                if statuses.entries().iter().any(|other| {
                    other.target == to
                        && now < other.expires_at_ms
                        && other.definition_id == s.definition_id
                        && (!d.source_scoped || other.source == s.source)
                }) {
                    return Err(Error::InvalidState);
                }
            }
            let used = statuses
                .entries()
                .iter()
                .filter(|s| s.target == to && now < s.expires_at_ms)
                .count();
            if used + selected.len()
                > c.pve
                    .as_ref()
                    .unwrap()
                    .status_rules
                    .as_ref()
                    .unwrap()
                    .max_active_per_entity
            {
                return Err(Error::Capacity);
            }
            if selected.iter().any(|s| transferred_control(s)) {
                let mut preview = statuses.clone();
                let ids = selected
                    .iter()
                    .map(|s| s.instance_id.clone())
                    .collect::<Vec<_>>();
                preview.retarget(&ids, &to, &mut vec![]);
                if preview.entries().iter().any(|s| {
                    s.target == to
                        && now < s.expires_at_ms
                        && (s.has_attribute(StatusAttribute::ControlImmunity)
                            || s.has_attribute(StatusAttribute::FirstControlImmunity))
                }) {
                    return Err(Error::Immune);
                }
            }
            if selected.iter().any(|s| s.shield.is_some()) {
                if selected
                    .iter()
                    .any(|s| s.shield.as_ref().is_some_and(|v| v.remaining == 0))
                {
                    return Err(Error::InvalidState);
                }
                // The preview already includes condition consumption. Resolve the
                // final pool after every numeric modifier in this same transfer.
                let mut preview = statuses.clone();
                let ids = selected
                    .iter()
                    .map(|s| s.instance_id.clone())
                    .collect::<Vec<_>>();
                preview.retarget(&ids, &to, &mut vec![]);
                let base = self.base_hp(to_owner, c);
                let rules = c.pve.as_ref().unwrap().status_rules.as_ref().unwrap();
                let maximum = preview
                    .attribute(&to, base, now, Some(rules), StatusAttribute::MaxHp)
                    .max(1);
                if preview
                    .ordinary_shield_room(&to, now, (maximum, rules.shield_cap_bps.unwrap()), None)
                    .is_none()
                {
                    return Err(Error::Capacity);
                }
            }
            Some(to)
        } else {
            None
        };
        Ok((
            selected.iter().map(|s| s.instance_id.clone()).collect(),
            target,
        ))
    }
    pub(super) fn status_operation_eligible(&self, owner: Owner) -> bool {
        match owner {
            Owner::Member(i) => self.members[i].eligible(),
            Owner::Npc(j) => self.npcs[j].hp > 0 && !self.npcs[j].returning,
            Owner::Summon(i) => self.summon_eligible(i),
        }
    }
    pub(super) fn execute_status_operation(
        &mut self,
        actors: (Owner, Owner),
        effect: &TriggerEffect,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let Ok((ids, target)) = self.plan_status_operation(&self.statuses, actors, effect, c, now)
        else {
            return;
        };
        self.advance_attack_clocks(c, now);
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        let moved = before
            .iter()
            .filter(|s| ids.contains(&s.instance_id))
            .collect::<Vec<_>>();
        let flags = (
            moved
                .iter()
                .any(|s| s.has_attribute(StatusAttribute::Silence)),
            moved.iter().any(|s| s.has_attribute(StatusAttribute::Root)),
            moved.iter().any(|s| s.has_attribute(StatusAttribute::Stun)),
        );
        if let Some(target) = &target {
            self.statuses.retarget(&ids, target, &mut raw);
        } else {
            self.statuses.remove(
                StatusRemovalReason::Cleansed,
                |s| ids.contains(&s.instance_id),
                &mut raw,
            );
        }
        self.route_status_events(raw, &before, events);
        self.reconcile_caps(c, now);
        if let Some(target) = target {
            self.apply_control_consequences(&target, flags, c, now, events);
        }
    }
    pub(super) fn plan_death_guard(
        &mut self,
        target: Owner,
        hp_loss: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<DeathGuard> {
        if !self.status_operation_eligible(target) {
            return None;
        }
        let rules = c.combat_rules.as_ref()?;
        let entity = self.owner_entity(target);
        let key = super::triggers::entity_key(&entity);
        let hp = self.current_hp(target);
        if hp == 0 || hp_loss < hp {
            return None;
        }
        let status_target = status_entity(&entity);
        let deadlines = self.triggers.status_death_ready.get(&key);
        let window = if rules.status_death_guards.is_empty() {
            None
        } else {
            self.statuses
                .entries()
                .iter()
                .filter(|s| {
                    s.target == status_target
                        && s.source == status_target
                        && now < s.expires_at_ms
                        && !self.statuses.is_derived(&s.instance_id)
                })
                .filter_map(|s| {
                    rules
                        .status_death_guards
                        .get(&s.definition_id)
                        .map(|rule| (s, rule))
                })
                .filter(|(_, rule)| {
                    deadlines
                        .and_then(|d| d.get(&rule.cooldown_group))
                        .is_none_or(|ready| now >= *ready)
                })
                .min_by_key(|(s, _)| (s.expires_at_ms, s.applied_at_ms, s.instance_id.as_str()))
                .map(|(_, rule)| rule)
        };
        let guard = if let Some(rule) = window {
            &rule.guard
        } else {
            if matches!(target, Owner::Summon(_)) || self.triggers.death_used.contains(&key) {
                return None;
            }
            let profile = self.combat_profile_id(target, rules);
            rules.death_guards.get(profile)?
        };
        let mut preview = self.statuses.clone();
        preview.expire(now, &mut vec![]);
        let reason = if guard.self_statuses.iter().any(|id| {
            crate::config::status_blocked(
                &self.statuses,
                id,
                &status_target,
                &rules.status_policies,
                now,
            )
        }) {
            Some(Error::Immune)
        } else if !preview.can_apply(
            &Self::definitions(c, &guard.self_statuses),
            &status_target,
            &status_target,
            c.pve.as_ref().unwrap().status_rules.as_ref().unwrap(),
        ) {
            Some(Error::Capacity)
        } else {
            None
        };
        if let Some(reason) = reason {
            self.suppress_trigger(&entity, "death_guard", reason, events);
            return None;
        }
        if let Some(rule) = window {
            self.triggers
                .status_death_ready
                .entry(key)
                .or_default()
                .insert(
                    rule.cooldown_group.clone(),
                    now.saturating_add(rule.cooldown_ms),
                );
        } else {
            self.triggers.death_used.insert(key);
        }
        Some(guard.clone())
    }
    pub(super) fn finish_death_guard(
        &mut self,
        target: Owner,
        source: Option<&EntityRef>,
        (guard, prevented): (DeathGuard, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let entity = self.owner_entity(target);
        let max = self.entity_max_hp(&entity, c, now).unwrap();
        self.ratio_self_heal(
            &entity,
            u64::from(max),
            (guard.heal_max_hp_bps, HealKind::Triggered),
            c,
            now,
            events,
        );
        self.apply_owned_statuses(
            target,
            (entity.clone(), max),
            &guard.self_statuses,
            c,
            now,
            events,
        );
        let hp = self.current_hp(target);
        for i in self.status_observers(&status_entity(&entity), &status_entity(&entity)) {
            private(
                events,
                i,
                match source {
                    Some(source) => Event::DeathGuardActivated {
                        source: status_entity(source),
                        target: status_entity(&entity),
                        prevented_hp_loss: prevented,
                        hp,
                    },
                    None => Event::TerrainDeathGuardActivated {
                        target: status_entity(&entity),
                        prevented_hp_loss: prevented,
                        hp,
                    },
                },
            );
        }
    }
}
