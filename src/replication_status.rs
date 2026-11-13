//! Shape and clock invariants shared by full and delta replica validation.
use super::*;
fn valid_periodic(s: &StatusView, p: &PeriodicView) -> bool {
    if p.interval_ms == 0
        || p.interval_ms >= (1u64 << 53)
        || p.total_ticks > 128
        || p.ticks_done > p.total_ticks
        || s.attribute.is_some()
        || s.shield.is_some()
        || s.modifier_bps != 0
        || p.tick_on_expiry.is_some_and(|tick| {
            !tick
                || p.kind != PeriodicEffectKind::MaxHpHeal
                || p.per_stack.is_some()
                || p.power_sampling.is_some()
                || p.raw_spirit.is_some()
                || s.expires_at_ms
                    .checked_sub(s.applied_at_ms)
                    .is_none_or(|duration| !duration.is_multiple_of(p.interval_ms))
        })
    {
        return false;
    }
    let first = match (p.per_stack, p.first_tick_at_ms, s.stacks, s.max_stacks) {
        (Some(true), Some(first), Some(n), Some(cap))
            if (1..=32).contains(&cap)
                && (1..=cap).contains(&n)
                && first >= s.applied_at_ms
                && first - s.applied_at_ms <= p.interval_ms =>
        {
            first
        }
        (None, None, None, None) => match s.applied_at_ms.checked_add(p.interval_ms) {
            Some(first) => first,
            None => return false,
        },
        _ => return false,
    };
    if first >= (1u64 << 53) {
        return false;
    }
    let last = s.expires_at_ms - u64::from(p.tick_on_expiry.is_none());
    let total = if first <= last {
        (last - first) / p.interval_ms + 1
    } else {
        0
    };
    let next = (u64::from(p.ticks_done) < total)
        .then(|| first.checked_add(u64::from(p.ticks_done) * p.interval_ms))
        .flatten();
    total == u64::from(p.total_ticks)
        && p.next_tick_at_ms == next
        && match p.kind {
            PeriodicEffectKind::MaxHpHeal => p.raw_damage == 0 && p.raw_heal.is_some(),
            _ => p.raw_heal.is_none() && p.raw_spirit.is_none(),
        }
}
pub(super) fn valid_status(s: &StatusView) -> bool {
    if s.retained_source.is_some_and(|retained| {
        !retained
            || s.source.kind != StatusEntityKind::Monster
            || s.source.generation == 0
            || s.target.kind != StatusEntityKind::Participant
            || s.attribute != Some(StatusAttribute::MovementSlow)
            || !(-10000..=-1).contains(&s.modifier_bps)
            || !s.modifiers.is_empty()
            || !s.base_modifiers.is_empty()
            || !s.flags.is_empty()
            || s.periodic.is_some()
            || s.shield.is_some()
            || s.window.is_some()
            || s.cleanse.is_some()
            || s.stack_counter.is_some()
            || s.stacks.is_some()
            || s.max_stacks.is_some()
    }) || s.expires_at_ms <= s.applied_at_ms
        || s.expires_at_ms >= (1u64 << 53)
        || !(-10000..=100000).contains(&s.modifier_bps)
        || match (s.stacks, s.max_stacks) {
            (None, None) => false,
            (Some(n), Some(cap)) => {
                !(1..=128).contains(&cap)
                    || !(if s.stack_counter == Some(true) { 0 } else { 1 }..=cap).contains(&n)
            }
            _ => true,
        }
        || s.periodic.as_ref().is_some_and(|p| !valid_periodic(s, p))
        || s.modifiers.len() + s.base_modifiers.len() > 8
        || !unique(
            &s.base_modifiers
                .iter()
                .map(|m| m.attribute)
                .collect::<Vec<_>>(),
        )
        || s.stack_counter.is_some_and(|flag| {
            !flag
                || s.stacks.is_none()
                || s.attribute.is_some()
                || s.periodic.is_some()
                || s.shield.is_some()
                || !s.flags.is_empty()
                || s.modifiers.is_empty() && s.base_modifiers.is_empty()
                || s.modifiers
                    .iter()
                    .chain(&s.base_modifiers)
                    .any(|m| m.attribute == StatusAttribute::MovementSlow)
                    && s.max_stacks.is_some_and(|n| n > 32)
        })
        || !s.base_modifiers.is_empty()
            && (s.attribute.is_some()
                || s.periodic.is_some()
                || s.shield.is_some()
                || s.modifier_bps != 0)
        || !unique(&s.modifiers.iter().map(|m| m.attribute).collect::<Vec<_>>())
    {
        return false;
    }
    let scalar_only = s.periodic.is_none()
        && s.shield.is_none()
        && s.modifiers.is_empty()
        && s.base_modifiers.is_empty()
        && s.flags.is_empty();
    if !s.modifiers.is_empty() && (s.attribute.is_some() || s.modifier_bps != 0) {
        return false;
    }
    if let Some(a) = s.attribute {
        use StatusAttribute::*;
        let valid = match a {
            Silence | Root | Stun | ControlImmunity | DamageImmunity | FirstControlImmunity => {
                scalar_only && s.modifier_bps == 0 && s.window.is_none() && s.stacks.is_none()
            }
            SpiritRecoveryLock => {
                scalar_only && s.modifier_bps == -10000 && s.window.is_none() && s.stacks.is_none()
            }
            MovementSlow => {
                scalar_only
                    && (-9999..=-1).contains(&s.modifier_bps)
                    && s.max_stacks.is_none_or(|n| n <= 32)
            }
            MovementHaste => {
                scalar_only && (1..=10000).contains(&s.modifier_bps) && s.stacks.is_none()
            }
            BasicIntervalDelay => {
                scalar_only && (1..=100000).contains(&s.modifier_bps) && s.stacks.is_none()
            }
            _ => scalar_only,
        };
        if !valid {
            return false;
        }
    }
    if let Some(v) = &s.cleanse
        && (!v.on_tick && !v.on_expired
            || s.shield.is_some()
            || s.stacks.is_some() && s.stack_counter != Some(true)
            || v.on_tick
                && s.periodic
                    .as_ref()
                    .is_none_or(|p| p.kind != PeriodicEffectKind::MaxHpHeal)
            || s.periodic
                .as_ref()
                .is_some_and(|p| p.kind != PeriodicEffectKind::MaxHpHeal))
    {
        return false;
    }
    if let Some(shield) = &s.shield
        && (s.periodic.is_some()
            || s.attribute.is_some()
            || s.stacks.is_some()
            || s.modifier_bps != 0
            || !s.flags.is_empty()
            || s.window.is_some()
            || s.cleanse.is_some()
            || !s.modifiers.is_empty()
                && (shield.record.is_none()
                    || s.modifiers.len() != 1
                    || s.modifiers[0].attribute != StatusAttribute::DamageReduction
                    || !(1..=10000).contains(&s.modifiers[0].modifier_bps)))
    {
        return false;
    }
    for m in s.modifiers.iter().chain(&s.base_modifiers) {
        use StatusAttribute::*;
        if !matches!(
            m.attribute,
            MaxHp
                | MaxSpirit
                | Spd
                | AttackSpeed
                | BasicIntervalDelay
                | ElementResistance
                | Atk
                | Def
                | HealingReceived
                | LifeSteal
                | DamageReduction
                | RecoveryStrength
                | CooldownReduction
                | ArmorPenetration
                | CriticalChance
                | CriticalDamageBonus
                | DodgePenalty
                | DodgeChance
                | Accuracy
                | SkillPowerBonus
                | ControlDurationReduction
                | HarmfulDurationReduction
                | MovementSlow
                | MovementHaste
        ) || !(-10000..=100000).contains(&m.modifier_bps)
            || m.modifier_bps < 0
                && !matches!(
                    m.attribute,
                    MaxHp
                        | MaxSpirit
                        | Spd
                        | AttackSpeed
                        | ElementResistance
                        | Atk
                        | Def
                        | HealingReceived
                        | LifeSteal
                        | RecoveryStrength
                        | MovementSlow
                )
            || m.attribute == MovementSlow && !(-9999..=-1).contains(&m.modifier_bps)
            || m.attribute == MovementHaste && !(1..=10000).contains(&m.modifier_bps)
        {
            return false;
        }
    }
    if !s.flags.is_empty() {
        let hostile = s.flags.iter().any(|a| a.is_control_flag());
        if s.flags.len() > 3
            || !unique(&s.flags)
            || s.attribute.is_some()
            || s.modifier_bps != 0
            || s.shield.is_some()
            || s.stacks.is_some()
            || s.window.is_some()
                && s.periodic
                    .as_ref()
                    .is_none_or(|p| p.kind != PeriodicEffectKind::MaxHpHeal)
            || s.flags
                .iter()
                .any(|a| !a.is_control_flag() && !a.is_immunity_flag())
            || hostile && s.flags.iter().any(|a| a.is_immunity_flag())
            || s.periodic
                .as_ref()
                .is_some_and(|p| hostile || p.kind != PeriodicEffectKind::MaxHpHeal)
        {
            return false;
        }
    }
    if let Some(w) = &s.window
        && (s.stacks.is_some() && s.stack_counter != Some(true)
            || s.shield.is_some()
            || s.periodic
                .as_ref()
                .is_some_and(|p| p.kind != PeriodicEffectKind::MaxHpHeal)
            || w.recorded > w.cap
            || (w.recorded == 0) != (w.contacts == 0))
    {
        return false;
    }
    true
}
