use super::{Config, DuplicatePolicy, MAX_DAMAGE_REDUCTION_BPS, OverhealShield, StatusMechanism};
use crate::protocol::StatusAttribute;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) const MAX_MEMBER_ITEM_USES: usize = 16;
pub(crate) const MAX_MEMBER_ITEM_COOLDOWN_MS: u64 = 60_000;
pub(crate) const MAX_MEMBER_ITEM_RECOVERY_BPS: u32 = 10_000;
pub(crate) const MAX_MEMBER_ITEM_QUALITY_BPS: u32 = 40_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ItemStatusOverride {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<crate::protocol::StatusModifier>,
    pub modifier_bps: i32,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberItemUseConfig {
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub grant_statuses_without_recovery: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub heal_hp: Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub heal_max_hp_bps: Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub restore_spirit: Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub restore_max_spirit_bps: Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub quality_multiplier_bps: Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub apply_healing_received: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub overheal_shield: Option<OverhealShield>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub on_use_self_statuses: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub on_use_self_status_overrides: Option<BTreeMap<String, ItemStatusOverride>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub cleanse_self_statuses: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub cooldown_ms: Option<u64>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub cooldown_group: Option<String>,
}

pub(super) fn fields(document: &Value) -> Result<(), &'static str> {
    if let Some(definition) = document.get("member_item_use") {
        if document["schema_version"] != 7
            || !matches!(document["mechanism_version"].as_u64(), Some(58..=107))
        {
            return Err("CONFIG_MEMBER_ITEM_USE_CAPABILITY");
        }
        let Some(definitions) = definition.as_object() else {
            return Err("CONFIG_MEMBER_ITEM_USE_SHAPE");
        };
        if document["mechanism_version"] == 58
            && definitions
                .values()
                .any(|entry| entry.get("cooldown_ms").is_some())
        {
            return Err("CONFIG_MEMBER_ITEM_COOLDOWN_CAPABILITY");
        }
        if !matches!(document["mechanism_version"].as_u64(), Some(60..=107))
            && definitions
                .values()
                .any(|entry| entry.get("restore_spirit").is_some())
        {
            return Err("CONFIG_MEMBER_ITEM_SPIRIT_CAPABILITY");
        }
        if !matches!(document["mechanism_version"].as_u64(), Some(61..=107))
            && definitions
                .values()
                .any(|entry| entry.get("cooldown_group").is_some())
        {
            return Err("CONFIG_MEMBER_ITEM_COOLDOWN_GROUP_CAPABILITY");
        }
        if !matches!(document["mechanism_version"].as_u64(), Some(63..=107))
            && definitions.values().any(|entry| {
                entry.get("heal_max_hp_bps").is_some()
                    || entry.get("restore_max_spirit_bps").is_some()
            })
        {
            return Err("CONFIG_MEMBER_ITEM_RATIO_RECOVERY_CAPABILITY");
        }
        if !matches!(document["mechanism_version"].as_u64(), Some(64..=107))
            && definitions
                .values()
                .any(|entry| entry.get("quality_multiplier_bps").is_some())
        {
            return Err("CONFIG_MEMBER_ITEM_QUALITY_RECOVERY_CAPABILITY");
        }
        if !matches!(document["mechanism_version"].as_u64(), Some(65..=107))
            && definitions
                .values()
                .any(|entry| entry.get("on_use_self_statuses").is_some())
        {
            return Err("CONFIG_MEMBER_ITEM_SELF_STATUS_CAPABILITY");
        }
        if !matches!(document["mechanism_version"].as_u64(), Some(66..=107))
            && definitions
                .values()
                .any(|entry| entry.get("on_use_self_status_overrides").is_some())
        {
            return Err("CONFIG_MEMBER_ITEM_STATUS_QUALITY_CAPABILITY");
        }
        if !matches!(document["mechanism_version"].as_u64(), Some(70..=107))
            && definitions
                .values()
                .any(|entry| entry.get("apply_healing_received").is_some())
        {
            return Err("CONFIG_MEMBER_ITEM_HEALING_RECEIVED_CAPABILITY");
        }
        if !matches!(document["mechanism_version"].as_u64(), Some(75..=107))
            && definitions
                .values()
                .any(|entry| entry.get("overheal_shield").is_some())
        {
            return Err("CONFIG_MEMBER_OVERHEAL_SHIELD_CAPABILITY");
        }
        if document["mechanism_version"]
            .as_u64()
            .is_none_or(|version| version < 87)
            && definitions
                .values()
                .any(|entry| entry.get("cleanse_self_statuses").is_some())
        {
            return Err("CONFIG_MEMBER_ITEM_CLEANSE_CAPABILITY");
        }
    }
    Ok(())
}

fn valid_recovery_amount(fixed: Option<u32>, bps: Option<u32>) -> bool {
    matches!(
        (fixed, bps),
        (None, None) | (Some(1..=1_000_000), None) | (None, Some(1..=MAX_MEMBER_ITEM_RECOVERY_BPS))
    )
}

pub(super) fn validate(config: &Config) -> Result<(), &'static str> {
    if config.member_item_use.as_ref().is_some_and(|definitions| {
        !(1..=MAX_MEMBER_ITEM_USES).contains(&definitions.len())
            || definitions.iter().any(|(item_id, definition)| {
                let heals = definition.heal_hp.is_some() || definition.heal_max_hp_bps.is_some();
                let restores_spirit = definition.restore_spirit.is_some()
                    || definition.restore_max_spirit_bps.is_some();
                config.item_limit(item_id).is_none()
                    || !(heals
                        || restores_spirit
                        || definition.cleanse_self_statuses.is_some()
                        || definition.grant_statuses_without_recovery == Some(true))
                    || (definition.apply_healing_received.is_some() && !heals)
                    || (heals && restores_spirit && config.mechanism_version < 62)
                    || !valid_recovery_amount(definition.heal_hp, definition.heal_max_hp_bps)
                    || !valid_recovery_amount(
                        definition.restore_spirit,
                        definition.restore_max_spirit_bps,
                    )
                    || definition
                        .quality_multiplier_bps
                        .is_some_and(|bps| !(1..=MAX_MEMBER_ITEM_QUALITY_BPS).contains(&bps))
                    || definition.cooldown_ms.is_some_and(|ms| {
                        !(1..=MAX_MEMBER_ITEM_COOLDOWN_MS).contains(&ms)
                            || config.step_ms == 0
                            || !ms.is_multiple_of(config.step_ms)
                    })
            })
    }) {
        return Err("CONFIG_MEMBER_ITEM_USE_BOUNDS");
    }
    let mut group_durations = BTreeMap::new();
    for definition in config
        .member_item_use
        .iter()
        .flat_map(|items| items.values())
    {
        if definition.grant_statuses_without_recovery.is_some()
            && (config.mechanism_version < 105
                || config.combat_rules.is_none()
                || definition
                    .on_use_self_statuses
                    .as_ref()
                    .is_none_or(Vec::is_empty))
        {
            return Err("CONFIG_MEMBER_ITEM_STATUS_ONLY_CAPABILITY");
        }
        if let Some(ids) = &definition.cleanse_self_statuses {
            let error = "CONFIG_MEMBER_ITEM_CLEANSE_BOUNDS";
            let pve = config.pve.as_ref().ok_or(error)?;
            if !(1..=2).contains(&ids.len())
                || (ids.len() == 2 && ids[0] == ids[1])
                || ids.iter().any(|id| {
                    !pve.status_definitions
                        .iter()
                        .any(|d| d.id == *id && d.is_hostile_control())
                })
                || definition.heal_hp.is_some()
                || definition.heal_max_hp_bps.is_some()
                || definition.restore_spirit.is_some()
                || definition.restore_max_spirit_bps.is_some()
                || definition.apply_healing_received.is_some()
                || definition.quality_multiplier_bps.is_some()
                || definition.overheal_shield.is_some()
                || definition.on_use_self_statuses.is_some()
                || definition.on_use_self_status_overrides.is_some()
            {
                return Err(error);
            }
        }
        if let Some(conversion) = &definition.overheal_shield {
            let error = "CONFIG_MEMBER_OVERHEAL_SHIELD_BOUNDS";
            let pve = config.pve.as_ref().ok_or(error)?;
            if (definition.heal_hp.is_none() && definition.heal_max_hp_bps.is_none())
                || definition.on_use_self_statuses.is_some()
                || pve.overheal_shield_definition(conversion).is_none()
            {
                return Err(error);
            }
        }
        if let Some(ids) = &definition.on_use_self_statuses {
            let pve = config
                .pve
                .as_ref()
                .ok_or("CONFIG_MEMBER_ITEM_SELF_STATUS_REFERENCE")?;
            if !(1..=2).contains(&ids.len())
                || (ids.len() == 2 && ids[0] == ids[1])
                || pve.status_rules.is_none()
                || ids.iter().any(|id| {
                    !pve.status_definitions.iter().any(|d| {
                        d.id == *id
                            && (!d.is_periodic() || d.is_heal())
                            && (d.flags.is_empty() || !d.is_hostile_control())
                            && (config.combat_rules.is_some() && !d.is_hostile_control()
                                || !matches!(
                                    d.attribute,
                                    Some(
                                        StatusAttribute::Silence
                                            | StatusAttribute::Root
                                            | StatusAttribute::Stun
                                            | StatusAttribute::MovementSlow
                                            | StatusAttribute::MovementHaste
                                            | StatusAttribute::CooldownReduction
                                            | StatusAttribute::ArmorPenetration
                                            | StatusAttribute::CriticalChance
                                            | StatusAttribute::CriticalDamageBonus
                                            | StatusAttribute::DodgePenalty
                                            | StatusAttribute::DodgeChance
                                            | StatusAttribute::Accuracy
                                            | StatusAttribute::SkillPowerBonus
                                            | StatusAttribute::LifeSteal
                                            | StatusAttribute::ControlImmunity
                                            | StatusAttribute::DamageImmunity
                                            | StatusAttribute::FirstControlImmunity
                                            | StatusAttribute::ControlDurationReduction
                                            | StatusAttribute::HarmfulDurationReduction
                                    )
                                ))
                    })
                })
            {
                return Err("CONFIG_MEMBER_ITEM_SELF_STATUS_REFERENCE");
            }
        }
        if let Some(overrides) = &definition.on_use_self_status_overrides {
            let error = "CONFIG_MEMBER_ITEM_STATUS_QUALITY_BOUNDS";
            let ids = definition.on_use_self_statuses.as_ref().ok_or(error)?;
            let pve = config.pve.as_ref().ok_or(error)?;
            if !(1..=2).contains(&overrides.len())
                || overrides.iter().any(|(id, quality)| {
                    !ids.contains(id)
                        || !(-10000..=100000).contains(&quality.modifier_bps)
                        || !(config.step_ms..=config.terminal_ms.min(3600000))
                            .contains(&quality.duration_ms)
                        || !pve.status_definitions.iter().any(|d| {
                            if config.combat_rules.is_some() && d.id == *id {
                                let mut candidate = config.clone();
                                let changed =
                                    std::sync::Arc::make_mut(candidate.pve.as_mut().unwrap())
                                        .status_definitions
                                        .iter_mut()
                                        .find(|d| d.id == *id)
                                        .unwrap();
                                changed.modifier_bps = quality.modifier_bps;
                                changed.duration_ms = quality.duration_ms;
                                if !quality.modifiers.is_empty() {
                                    changed.modifiers = quality.modifiers.clone();
                                }
                                return super::status::validate_member_status_capabilities(
                                    &candidate,
                                )
                                .is_ok()
                                    && super::status::validate(
                                        &candidate.single_member_validation(),
                                    )
                                    .is_ok();
                            }
                            quality.modifiers.is_empty()
                                && d.id == *id
                                && matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                                && matches!(
                                    d.mechanism,
                                    StatusMechanism::TimedDefenseModifier
                                        | StatusMechanism::TimedAttributeModifier
                                )
                                && match d.effective_attribute() {
                                    StatusAttribute::Atk | StatusAttribute::Def => true,
                                    StatusAttribute::DamageReduction => (1
                                        ..=MAX_DAMAGE_REDUCTION_BPS)
                                        .contains(&quality.modifier_bps),
                                    StatusAttribute::MaxHp
                                    | StatusAttribute::MaxSpirit
                                    | StatusAttribute::Spd
                                    | StatusAttribute::AttackSpeed
                                    | StatusAttribute::BasicIntervalDelay
                                    | StatusAttribute::ElementResistance
                                    | StatusAttribute::HealingReceived
                                    | StatusAttribute::RecoveryStrength
                                    | StatusAttribute::SpiritRecoveryLock
                                    | StatusAttribute::Silence
                                    | StatusAttribute::Root
                                    | StatusAttribute::Stun
                                    | StatusAttribute::MovementSlow
                                    | StatusAttribute::MovementHaste
                                    | StatusAttribute::CooldownReduction
                                    | StatusAttribute::ArmorPenetration
                                    | StatusAttribute::CriticalChance
                                    | StatusAttribute::CriticalDamageBonus
                                    | StatusAttribute::DodgePenalty
                                    | StatusAttribute::DodgeChance
                                    | StatusAttribute::Accuracy
                                    | StatusAttribute::SkillPowerBonus
                                    | StatusAttribute::LifeSteal
                                    | StatusAttribute::ControlImmunity
                                    | StatusAttribute::DamageImmunity
                                    | StatusAttribute::FirstControlImmunity
                                    | StatusAttribute::ControlDurationReduction
                                    | StatusAttribute::HarmfulDurationReduction => false,
                                }
                        })
                })
            {
                return Err(error);
            }
        }
        let Some(group) = definition.cooldown_group.as_deref() else {
            continue;
        };
        let duration = definition
            .cooldown_ms
            .ok_or("CONFIG_MEMBER_ITEM_COOLDOWN_GROUP_BOUNDS")?;
        if !crate::protocol::valid_id(group)
            || group.len() > 64
            || group_durations
                .insert(group, duration)
                .is_some_and(|previous| previous != duration)
        {
            return Err("CONFIG_MEMBER_ITEM_COOLDOWN_GROUP_BOUNDS");
        }
    }
    Ok(())
}
