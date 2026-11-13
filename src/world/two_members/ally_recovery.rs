//! Resolve one committed ally cast against the primary and its nearby members.
use super::*;
use crate::config::ally_recovery::{AllyRecovery, RecoveryMark, RecoveryMarkOwner};

impl World {
    pub(super) fn ally_recovery_for<'a>(
        &self,
        member: usize,
        skill_id: &str,
        c: &'a Config,
    ) -> Option<&'a AllyRecovery> {
        let rows = c
            .combat_rules
            .as_ref()?
            .ally_recovery_by_mastery
            .get(skill_id)?;
        let mastery = self.members[member]
            .combat_build
            .as_ref()
            .and_then(|b| b.powers.get(skill_id))
            .map_or(0, |p| p.mastery);
        rows.iter().rev().find(|row| row.minimum_mastery <= mastery)
    }

    pub(super) fn resolve_ally_recovery(
        &mut self,
        (source, primary): (usize, usize),
        (skill_id, row): (&str, &AllyRecovery),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let source_owner = Owner::Member(source);
        let power = self.recovery_power(source_owner, skill_id, c, now);
        let source_entity = status_entity(&self.owner_entity(source_owner));
        let mut recipients = vec![(primary, 10000, 10000)];
        if let Some(area) = &row.splash {
            let center = self.members[primary].position;
            recipients.extend(
                self.members
                    .iter()
                    .enumerate()
                    .filter(|(i, member)| {
                        *i != primary
                            && member.eligible()
                            && distance(member.position, center) <= area.radius
                    })
                    .map(|(i, _)| (i, area.heal_bps, area.hot_bps)),
            );
        }
        for (target, heal_bps, hot_bps) in recipients {
            let entity = self.owner_entity(Owner::Member(target));
            let max_hp = self.entity_max_hp(&entity, c, now).unwrap();
            let base = u64::from(row.heal.amount(power, max_hp));
            let raw = (base * u64::from(heal_bps) / 10000) as u32;
            let excess = self.resolve_skill_heal_amount((source, target), raw, c, now, events);
            if let Some(hot) = &row.hot
                && hot_bps > 0
            {
                let mut defs = Self::definitions(c, std::slice::from_ref(&hot.status_id));
                defs[0].duration_ms = hot.duration_ms;
                defs[0].periodic_tick_on_expiry = true;
                defs[0].periodic_raw_heal = Some(
                    (base * u64::from(hot.tick_bps) * u64::from(hot_bps) / 100_000_000) as u32,
                );
                self.apply_owned_status_definitions(
                    source_owner,
                    entity.clone(),
                    &defs,
                    c,
                    now,
                    events,
                );
            }
            if let Some(conversion) = &row.overheal_shield {
                self.convert_resolved_overheal(
                    c,
                    conversion,
                    (&source_entity, &status_entity(&entity), max_hp),
                    now,
                    excess,
                    events,
                );
            }
        }
        if let Some(mark) = &row.mark {
            self.apply_recovery_mark((source, primary), (mark, power), c, now, events);
        }
    }

    fn apply_recovery_mark(
        &mut self,
        (source, primary): (usize, usize),
        (mark, power): (&RecoveryMark, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let holder = match mark.owner {
            RecoveryMarkOwner::Caster => source,
            RecoveryMarkOwner::PrimaryTarget => primary,
        };
        let target = self.owner_entity(Owner::Member(holder));
        let target_key = status_entity(&target);
        let owner = Owner::Member(source);
        let source_key = status_entity(&self.owner_entity(owner));
        let mut defs = Self::definitions(c, std::slice::from_ref(&mark.status_id));
        defs[0].application_stacks = Some(mark.stacks);
        let start = events.len();
        self.apply_owned_status_definitions(owner, target, &defs, c, now, events);
        let Some(formula) = &mark.at_cap_heal else {
            return;
        };
        let Some((id, stacks)) = events[start..].iter().find_map(|(_, event)| {
            let status = match event {
                EventV4::Private {
                    event: Event::StatusApplied { status },
                } => status,
                EventV4::Private {
                    event:
                        Event::StatusStacked {
                            status,
                            added_stacks,
                        },
                } if *added_stacks > 0 => status,
                _ => return None,
            };
            let stacks = status.stacks?;
            (status.definition_id == mark.status_id
                && status.source == source_key
                && status.target == target_key
                && status.max_stacks == Some(stacks))
            .then(|| (status.instance_id.clone(), stacks))
        }) else {
            return;
        };
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        if !self.statuses.consume(&id, stacks, &mut raw) {
            return;
        }
        self.route_status_events(raw, &before, events);
        self.reconcile_caps(c, now);
        for i in 0..self.members.len() {
            if self.members[i].eligible() {
                let entity = &self.members[i].reference.entity;
                let max_hp = self.entity_max_hp(entity, c, now).unwrap();
                let amount = formula.amount(power, max_hp);
                self.resolve_member_heal(
                    (source, i),
                    (amount, HealKind::Triggered),
                    c,
                    now,
                    events,
                );
            }
        }
    }
}
