use super::*;

const BPS: u64 = 10_000;

fn recovery_amount(fixed: Option<u32>, bps: Option<u32>, max: u32, multiplier: u64) -> Option<u64> {
    Some(if let Some(amount) = fixed {
        u64::from(amount) * multiplier / BPS
    } else {
        u64::from(max) * u64::from(bps?) * multiplier / (BPS * BPS)
    })
}

impl World {
    pub(super) fn use_item(
        &mut self,
        i: usize,
        item_id: &str,
        expected_revision: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<(u32, Option<u32>)> {
        let definitions = c.member_item_use.as_ref().ok_or(Error::Unsupported)?;
        let definition = definitions.get(item_id).ok_or(Error::InvalidTarget)?;
        let member = &self.members[i];
        let removal = member.plan_acquired_removal(item_id, 1, expected_revision)?;
        if member
            .item_cooldowns
            .get(item_id)
            .is_some_and(|&end| now < end)
        {
            return Err(Error::InvalidState);
        }
        let cleanse = if let Some(ids) = &definition.cleanse_self_statuses {
            let target = status_entity(&member.reference.entity);
            let matches = move |status: &StatusView| {
                status.target == target
                    && now < status.expires_at_ms
                    && ids.contains(&status.definition_id)
            };
            let before: Vec<_> = self
                .statuses
                .entries()
                .iter()
                .filter(|s| matches(s))
                .cloned()
                .collect();
            if before.is_empty() {
                return Err(Error::InvalidState);
            }
            Some((matches, before))
        } else {
            None
        };
        let multiplier = definition.quality_multiplier_bps.map_or(BPS, u64::from);
        let max_spirit = self.member_max_spirit(i, c, now);
        let spirit = recovery_amount(
            definition.restore_spirit,
            definition.restore_max_spirit_bps,
            max_spirit,
            multiplier,
        )
        .map(|requested| {
            requested.min(u64::from(member.spirit_recovery_gap(
                &self.statuses,
                now,
                max_spirit,
            ))) as u32
        });
        let max_hp = self
            .entity_max_hp(&member.reference.entity, c, now)
            .unwrap();
        let hp = recovery_amount(
            definition.heal_hp,
            definition.heal_max_hp_bps,
            max_hp,
            multiplier,
        )
        .unwrap_or(0);
        let hp = if definition.apply_healing_received == Some(true) {
            self.adjusted_heal(&status_entity(&member.reference.entity), hp, c, now)
        } else {
            hp
        };
        let hp_restored = hp.min(u64::from(max_hp.saturating_sub(member.hp))) as u32;
        let target = member.reference.entity.clone();
        let converted_shield = if let Some(conversion) = &definition.overheal_shield {
            let entity = status_entity(&target);
            self.plan_overheal_conversion(
                c,
                conversion,
                (&entity, &entity, max_hp),
                now,
                hp - u64::from(hp_restored),
            )?
            .map(|plan| (plan, entity))
        } else {
            None
        };
        if hp_restored == 0
            && spirit.unwrap_or(0) == 0
            && converted_shield.is_none()
            && cleanse.is_none()
            && definition.grant_statuses_without_recovery != Some(true)
        {
            return Err(Error::InvalidState);
        }
        let statuses = definition
            .on_use_self_statuses
            .as_deref()
            .unwrap_or_default();
        if !self.can_statuses(i, c, statuses) {
            return Err(Error::Capacity);
        }
        let hp = self.restore_hp(&target, hp_restored, c, now);
        let member = &mut self.members[i];
        member.spirit += spirit.unwrap_or(0);
        member.commit_acquired_removal(item_id, removal, super::loot_drop::RemovalKind::Consumed);
        if let Some(duration) = definition.cooldown_ms {
            let ready_at = now + duration;
            if let Some(group) = &definition.cooldown_group {
                for (id, sibling) in definitions {
                    if sibling.cooldown_group.as_ref() == Some(group) {
                        member.item_cooldowns.insert(id.clone(), ready_at);
                    }
                }
            } else {
                member.item_cooldowns.insert(item_id.into(), ready_at);
            }
        }
        if let Some((matches, before)) = cleanse {
            let mut raw = vec![];
            self.statuses
                .remove(StatusRemovalReason::Cleansed, matches, &mut raw);
            self.route_status_events(raw, &before, events);
        } else if let Some((plan, entity)) = converted_shield {
            self.apply_overheal_conversion(plan, &entity, &entity, now, events);
        } else if let Some(overrides) = &definition.on_use_self_status_overrides {
            let mut defs = Self::definitions(c, statuses);
            for status in &mut defs {
                if let Some(quality) = overrides.get(&status.id) {
                    status.modifier_bps = quality.modifier_bps;
                    status.duration_ms = quality.duration_ms;
                    if !quality.modifiers.is_empty() {
                        status.modifiers = quality.modifiers.clone();
                    }
                }
            }
            self.apply_owned_status_definitions(Owner::Member(i), target, &defs, c, now, events);
        } else {
            self.apply_statuses(i, (target, max_hp), statuses, c, now, events);
        }
        Ok((hp, spirit))
    }
}
