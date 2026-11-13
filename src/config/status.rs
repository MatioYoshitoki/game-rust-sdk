use super::*;
#[cfg(all(test, match_sdk_test_data))]
#[path = "attribute_tests.rs"]
mod attribute_tests;
#[cfg(all(test, match_sdk_test_data))]
#[path = "healing_received_tests.rs"]
mod healing_received_tests;
#[cfg(all(test, match_sdk_test_data))]
#[path = "healing_tests.rs"]
mod healing_tests;
#[cfg(all(test, match_sdk_test_data))]
#[path = "periodic_tests.rs"]
mod periodic_tests;
#[cfg(all(test, match_sdk_test_data))]
#[path = "shield_tests.rs"]
mod shield_tests;
use crate::protocol::StatusAttribute;
use std::collections::BTreeSet;
#[cfg(all(test, match_sdk_test_data))]
#[path = "stack_tests.rs"]
mod stack_tests;
#[cfg(all(test, match_sdk_test_data))]
#[path = "status_tests.rs"]
mod tests;

pub(crate) const MAX_DAMAGE_REDUCTION_BPS: i32 = 5000;
pub(crate) const MAX_CONTROL_DURATION_REDUCTION_BPS: i32 = 4000;
pub(crate) const MAX_COOLDOWN_REDUCTION_BPS: i32 = 4000;
pub(crate) const MAX_CRITICAL_DAMAGE_BONUS_BPS: i32 = 15000;
pub(crate) const MAX_DODGE_BPS: u32 = 5000;
pub(crate) const MAX_ACCURACY_BPS: u32 = 10000;

pub(super) fn definition_limit(c: &Config) -> usize {
    // The legacy validation view keeps the combat table from the real config.
    if c.combat_rules.is_some() { 32 } else { 16 }
}

pub(super) fn validate_member_status_capabilities(c: &Config) -> Result<(), &'static str> {
    super::stack_counters::validate(c)?;
    if let Some(r) = c.pve.as_ref().and_then(|p| p.status_rules.as_ref()) {
        if !r.attribute_caps_bps.is_empty()
            && (c.schema_version != 7 || c.mechanism_version < 105 || c.combat_rules.is_none())
        {
            return Err("CONFIG_ATTRIBUTE_CAPS_CAPABILITY");
        }
        for (a, n) in &r.attribute_caps_bps {
            if !matches!(
                a,
                StatusAttribute::MaxHp
                    | StatusAttribute::MaxSpirit
                    | StatusAttribute::Spd
                    | StatusAttribute::AttackSpeed
                    | StatusAttribute::BasicIntervalDelay
                    | StatusAttribute::Atk
                    | StatusAttribute::Def
                    | StatusAttribute::ElementResistance
                    | StatusAttribute::HealingReceived
                    | StatusAttribute::LifeSteal
                    | StatusAttribute::RecoveryStrength
                    | StatusAttribute::DamageReduction
                    | StatusAttribute::CooldownReduction
                    | StatusAttribute::ArmorPenetration
                    | StatusAttribute::CriticalChance
                    | StatusAttribute::CriticalDamageBonus
                    | StatusAttribute::DodgePenalty
                    | StatusAttribute::DodgeChance
                    | StatusAttribute::Accuracy
                    | StatusAttribute::SkillPowerBonus
                    | StatusAttribute::ControlDurationReduction
                    | StatusAttribute::HarmfulDurationReduction
            ) || *n > 1000000
                || matches!(
                    a,
                    StatusAttribute::DamageReduction
                        | StatusAttribute::CooldownReduction
                        | StatusAttribute::ControlDurationReduction
                        | StatusAttribute::CriticalChance
                        | StatusAttribute::HarmfulDurationReduction
                ) && *n > 10000
            {
                return Err("CONFIG_ATTRIBUTE_CAPS_POLICY");
            }
        }
    }
    for definition in c.pve.iter().flat_map(|p| &p.status_definitions) {
        if definition.attribute == Some(StatusAttribute::MovementSlow)
            && matches!(definition.duplicate_policy, DuplicatePolicy::Stack)
            && (c.schema_version != 7
                || c.mechanism_version < 105
                || c.combat_rules.is_none()
                || definition.max_stacks.is_none_or(|n| !(1..=32).contains(&n)))
        {
            return Err("CONFIG_MOVEMENT_SLOW_STACK_CAPABILITY");
        }
        let compound = matches!(definition.mechanism, StatusMechanism::TimedCompoundModifier);
        if (definition.harmful.is_some()
            || definition.attribute == Some(StatusAttribute::HarmfulDurationReduction))
            && (c.schema_version != 7 || c.mechanism_version < 105 || c.combat_rules.is_none())
        {
            return Err("CONFIG_HARMFUL_DURATION_CAPABILITY");
        }
        if definition.attribute == Some(StatusAttribute::HarmfulDurationReduction)
            && !(1..=100000).contains(&definition.modifier_bps)
        {
            return Err("CONFIG_HARMFUL_DURATION_POLICY");
        }
        if matches!(
            definition.attribute,
            Some(
                StatusAttribute::ElementResistance
                    | StatusAttribute::MaxHp
                    | StatusAttribute::MaxSpirit
                    | StatusAttribute::Spd
                    | StatusAttribute::AttackSpeed
                    | StatusAttribute::BasicIntervalDelay
                    | StatusAttribute::LifeSteal
            )
        ) && (c.schema_version != 7 || c.mechanism_version < 105 || c.combat_rules.is_none())
        {
            return Err("CONFIG_ELEMENT_RESISTANCE_CAPABILITY");
        }
        let periodic_composition = definition.is_periodic()
            && (!definition.modifiers.is_empty()
                || !definition.flags.is_empty()
                || definition.periodic_stack.is_some());
        if (compound || periodic_composition)
            && (c.schema_version != 7 || c.mechanism_version < 105 || c.combat_rules.is_none())
        {
            return Err("CONFIG_COMPOUND_STATUS_CAPABILITY");
        }
        if definition.periodic_stack.is_some()
            && (!definition.is_periodic()
                || !definition.source_scoped
                || !matches!(definition.duplicate_policy, DuplicatePolicy::Stack)
                || definition.max_stacks.is_none_or(|n| !(1..=32).contains(&n))
                || definition
                    .periodic_stack
                    .and_then(|policy| policy.stacks_per_application)
                    .is_some_and(|n| n == 0 || n > definition.max_stacks.unwrap_or(0))
                || definition
                    .periodic_stack
                    .and_then(|policy| policy.healing_reduction_cap_bps)
                    .is_some_and(|cap| {
                        definition.is_heal()
                            || !(1..=10000).contains(&cap)
                            || !definition.modifiers.iter().any(|m| {
                                m.attribute == StatusAttribute::HealingReceived
                                    && m.modifier_bps < 0
                            })
                    }))
        {
            return Err("CONFIG_PERIODIC_STACK_POLICY");
        }
        if compound || periodic_composition {
            let allowed = |a| {
                matches!(
                    a,
                    StatusAttribute::MaxHp
                        | StatusAttribute::MaxSpirit
                        | StatusAttribute::Spd
                        | StatusAttribute::AttackSpeed
                        | StatusAttribute::BasicIntervalDelay
                        | StatusAttribute::MovementSlow
                        | StatusAttribute::MovementHaste
                        | StatusAttribute::ElementResistance
                        | StatusAttribute::Atk
                        | StatusAttribute::Def
                        | StatusAttribute::HealingReceived
                        | StatusAttribute::LifeSteal
                        | StatusAttribute::RecoveryStrength
                        | StatusAttribute::DamageReduction
                        | StatusAttribute::CooldownReduction
                        | StatusAttribute::ArmorPenetration
                        | StatusAttribute::CriticalChance
                        | StatusAttribute::CriticalDamageBonus
                        | StatusAttribute::DodgePenalty
                        | StatusAttribute::DodgeChance
                        | StatusAttribute::Accuracy
                        | StatusAttribute::SkillPowerBonus
                        | StatusAttribute::ControlDurationReduction
                        | StatusAttribute::HarmfulDurationReduction
                )
            };
            let mut attributes = std::collections::BTreeSet::new();
            if compound
                && definition.modifiers.is_empty()
                && definition.base_modifiers.is_empty()
                && definition.flags.is_empty()
                || definition.modifiers.len() + definition.base_modifiers.len() > 8
                || definition
                    .modifiers
                    .iter()
                    .chain(&definition.base_modifiers)
                    .any(|m| {
                        matches!(
                            m.attribute,
                            StatusAttribute::MovementSlow | StatusAttribute::MovementHaste
                        )
                    })
                    && !definition.source_scoped
                || definition.attribute.is_some()
                || definition.modifier_bps != 0
                || definition
                    .modifiers
                    .iter()
                    .map(|m| (false, m))
                    .chain(definition.base_modifiers.iter().map(|m| (true, m)))
                    .any(|(fixed, m)| {
                        !allowed(m.attribute)
                            || !(-10000..=100000).contains(&m.modifier_bps)
                            || m.modifier_bps < 0
                                && !matches!(
                                    m.attribute,
                                    StatusAttribute::MaxHp
                                        | StatusAttribute::MaxSpirit
                                        | StatusAttribute::Spd
                                        | StatusAttribute::AttackSpeed
                                        | StatusAttribute::MovementSlow
                                        | StatusAttribute::ElementResistance
                                        | StatusAttribute::Atk
                                        | StatusAttribute::Def
                                        | StatusAttribute::HealingReceived
                                        | StatusAttribute::LifeSteal
                                        | StatusAttribute::RecoveryStrength
                                )
                            || m.attribute == StatusAttribute::MovementSlow
                                && !(-9999..=-1).contains(&m.modifier_bps)
                            || m.attribute == StatusAttribute::MovementHaste
                                && !(1..=10000).contains(&m.modifier_bps)
                            || !attributes.insert((fixed, format!("{:?}", m.attribute)))
                    })
            {
                return Err("CONFIG_COMPOUND_STATUS_POLICY");
            }
        } else if !definition.modifiers.is_empty() {
            return Err("CONFIG_COMPOUND_STATUS_POLICY");
        }
        if !definition.flags.is_empty() {
            let hostile = definition.flags.iter().any(|a| a.is_control_flag());
            if (!compound && !definition.is_heal())
                || definition.is_periodic() && hostile
                || definition.flags.len() > 3
                || definition
                    .flags
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != definition.flags.len()
                || definition
                    .flags
                    .iter()
                    .any(|a| !a.is_control_flag() && !a.is_immunity_flag())
                || hostile && definition.flags.iter().any(|a| a.is_immunity_flag())
                || !definition.source_scoped
                || !(matches!(definition.duplicate_policy, DuplicatePolicy::Refresh)
                    || definition.is_heal()
                        && matches!(definition.duplicate_policy, DuplicatePolicy::Ignore))
                || definition.max_stacks.is_some()
            {
                return Err("CONFIG_COMPOUND_FLAGS_POLICY");
            }
        }
        if definition.overheal_shield.is_some()
            && (c.schema_version != 7 || c.mechanism_version < 76)
        {
            return Err("CONFIG_PERIODIC_OVERHEAL_SHIELD_CAPABILITY");
        }
        match definition.attribute {
            Some(StatusAttribute::DamageReduction)
                if c.schema_version != 7 || c.mechanism_version < 67 =>
            {
                return Err("CONFIG_DAMAGE_REDUCTION_CAPABILITY");
            }
            Some(StatusAttribute::RecoveryStrength)
                if c.schema_version != 7 || c.mechanism_version < 72 =>
            {
                return Err("CONFIG_RECOVERY_STRENGTH_CAPABILITY");
            }
            Some(StatusAttribute::SpiritRecoveryLock)
                if c.schema_version != 7 || c.mechanism_version < 73 =>
            {
                return Err("CONFIG_SPIRIT_RECOVERY_LOCK_CAPABILITY");
            }
            Some(StatusAttribute::Silence) if c.schema_version != 7 || c.mechanism_version < 83 => {
                return Err("CONFIG_SILENCE_CAPABILITY");
            }
            Some(StatusAttribute::ControlImmunity)
                if c.schema_version != 7 || c.mechanism_version < 84 =>
            {
                return Err("CONFIG_CONTROL_IMMUNITY_CAPABILITY");
            }
            Some(StatusAttribute::DamageImmunity)
                if c.schema_version != 7 || c.mechanism_version < 100 =>
            {
                return Err("CONFIG_DAMAGE_IMMUNITY_CAPABILITY");
            }
            Some(
                StatusAttribute::ControlDurationReduction
                | StatusAttribute::HarmfulDurationReduction,
            ) if c.schema_version != 7 || c.mechanism_version < 85 => {
                return Err("CONFIG_CONTROL_DURATION_REDUCTION_CAPABILITY");
            }
            Some(StatusAttribute::FirstControlImmunity)
                if c.schema_version != 7 || c.mechanism_version < 86 =>
            {
                return Err("CONFIG_FIRST_CONTROL_IMMUNITY_CAPABILITY");
            }
            Some(StatusAttribute::Root) if c.schema_version != 7 || c.mechanism_version < 88 => {
                return Err("CONFIG_NPC_ROOT_CAPABILITY");
            }
            Some(StatusAttribute::Stun) if c.schema_version != 7 || c.mechanism_version < 89 => {
                return Err("CONFIG_NPC_STUN_CAPABILITY");
            }
            Some(StatusAttribute::MovementSlow)
                if c.schema_version != 7 || c.mechanism_version < 90 =>
            {
                return Err("CONFIG_NPC_MOVEMENT_SLOW_CAPABILITY");
            }
            Some(StatusAttribute::MovementHaste)
                if c.schema_version != 7 || c.mechanism_version < 91 =>
            {
                return Err("CONFIG_MEMBER_MOVEMENT_HASTE_CAPABILITY");
            }
            Some(StatusAttribute::CooldownReduction)
                if c.schema_version != 7 || c.mechanism_version < 92 =>
            {
                return Err("CONFIG_MEMBER_COOLDOWN_REDUCTION_CAPABILITY");
            }
            Some(StatusAttribute::ArmorPenetration)
                if c.schema_version != 7 || c.mechanism_version < 93 =>
            {
                return Err("CONFIG_MEMBER_ARMOR_PENETRATION_CAPABILITY");
            }
            Some(StatusAttribute::CriticalChance)
                if c.schema_version != 7 || c.mechanism_version < 94 =>
            {
                return Err("CONFIG_MEMBER_CRITICAL_CHANCE_CAPABILITY");
            }
            Some(StatusAttribute::CriticalDamageBonus)
                if c.schema_version != 7 || c.mechanism_version < 95 =>
            {
                return Err("CONFIG_MEMBER_CRITICAL_DAMAGE_BONUS_CAPABILITY");
            }
            Some(StatusAttribute::DodgePenalty)
                if c.schema_version != 7 || c.mechanism_version < 96 =>
            {
                return Err("CONFIG_MEMBER_DODGE_PENALTY_CAPABILITY");
            }
            Some(StatusAttribute::DodgeChance)
                if c.schema_version != 7 || c.mechanism_version < 97 =>
            {
                return Err("CONFIG_MEMBER_DODGE_CHANCE_CAPABILITY");
            }
            Some(StatusAttribute::Accuracy)
                if c.schema_version != 7 || c.mechanism_version < 98 =>
            {
                return Err("CONFIG_MEMBER_ACCURACY_CAPABILITY");
            }
            Some(StatusAttribute::SkillPowerBonus)
                if c.schema_version != 7 || c.mechanism_version < 104 =>
            {
                return Err("CONFIG_MEMBER_SKILL_POWER_CAPABILITY");
            }
            _ => {}
        }
    }
    Ok(())
}

// Preserve presence information before serde defaults erase an explicit empty list.
pub(super) fn validate_fields(v: &serde_json::Value) -> Result<(), &'static str> {
    let bound = |a: &serde_json::Value| {
        a.get("on_commit_self_statuses").is_some() || a.get("on_hit_target_statuses").is_some()
    };
    let p = &v["pve"];
    let present = p.get("status_definitions").is_some()
        || p.get("status_rules").is_some()
        || bound(&v["skill"])
        || v["additional_skills"]
            .as_array()
            .is_some_and(|skills| skills.iter().any(bound))
        || bound(&p["combat"]["projectile"])
        || p["additional_templates"]
            .as_object()
            .is_some_and(|ts| ts.values().any(|t| bound(&t["combat"]["projectile"])));
    if present
        && (v["schema_version"] != 4
            || v["mechanism_version"].as_u64().is_none_or(|m| m < 19)
            || !p.is_object())
    {
        return Err("CONFIG_STATUS_CAPABILITY");
    }
    for d in p["status_definitions"].as_array().into_iter().flatten() {
        let heal = d["mechanism"] == "periodic_heal";
        if (heal || d.get("heal_bps").is_some())
            && v["mechanism_version"].as_u64().is_none_or(|m| m < 24)
        {
            return Err("CONFIG_PERIODIC_HEAL_CAPABILITY");
        }
        let periodic = heal || d["mechanism"] == "periodic_damage";
        let periodic_fields = d.get("tick_interval_ms").is_some()
            || d.get("damage_bps").is_some()
            || d.get("heal_bps").is_some();
        if (periodic || periodic_fields) && v["mechanism_version"].as_u64().is_none_or(|m| m < 23) {
            return Err("CONFIG_PERIODIC_CAPABILITY");
        }
        if periodic {
            if (heal && d.get("damage_bps").is_some()) || (!heal && d.get("heal_bps").is_some()) {
                return Err("CONFIG_PERIODIC_POLICY");
            }
            let stacked = d.get("periodic_stack").is_some();
            if ["attribute", "modifier_bps", "shield_bps"]
                .iter()
                .any(|key| d.get(*key).is_some())
                || if stacked {
                    d["duplicate_policy"] != "stack"
                        || d["max_stacks"]
                            .as_u64()
                            .is_none_or(|n| !(1..=32).contains(&n))
                } else {
                    d.get("max_stacks").is_some()
                        || !matches!(d["duplicate_policy"].as_str(), Some("refresh" | "ignore"))
                }
            {
                return Err("CONFIG_PERIODIC_POLICY");
            }
        } else if periodic_fields {
            return Err("CONFIG_PERIODIC_POLICY");
        }
        let control = d["mechanism"] == "timed_control";
        let control_attribute = matches!(
            d["attribute"].as_str(),
            Some(
                "silence"
                    | "root"
                    | "stun"
                    | "damage_immunity"
                    | "control_immunity"
                    | "first_control_immunity"
            )
        );
        if (control || control_attribute)
            && (!control
                || !control_attribute
                || d.get("modifier_bps").is_some()
                || d["duplicate_policy"] != "refresh"
                || d["source_scoped"] != true
                || d.get("max_stacks").is_some())
        {
            return Err(match d["attribute"].as_str() {
                Some("stun") => "CONFIG_NPC_STUN_POLICY",
                Some("root") => "CONFIG_NPC_ROOT_POLICY",
                Some("control_immunity") => "CONFIG_CONTROL_IMMUNITY_POLICY",
                Some("damage_immunity") => "CONFIG_DAMAGE_IMMUNITY_POLICY",
                Some("first_control_immunity") => "CONFIG_FIRST_CONTROL_IMMUNITY_POLICY",
                _ => "CONFIG_SILENCE_POLICY",
            });
        }
        let shield = d["mechanism"] == "absorption_shield";
        if (shield || d.get("shield_bps").is_some())
            && v["mechanism_version"].as_u64().is_none_or(|m| m < 22)
        {
            return Err("CONFIG_SHIELD_CAPABILITY");
        }
        if shield {
            if ["attribute", "modifier_bps", "max_stacks"]
                .iter()
                .any(|key| d.get(*key).is_some())
                || !matches!(d["duplicate_policy"].as_str(), Some("refresh" | "ignore"))
            {
                return Err("CONFIG_SHIELD_POLICY");
            }
        } else if !periodic
            && (d.get("shield_bps").is_some()
                || !control
                    && d["mechanism"] != "timed_compound_modifier"
                    && d["modifier_bps"].as_i64().is_none())
        {
            return Err("CONFIG_STATUS_DEFINITION");
        }

        if d["attribute"] == "healing_received"
            && (v["mechanism_version"].as_u64().is_none_or(|m| m < 25)
                || !matches!(d["duplicate_policy"].as_str(), Some("refresh" | "ignore"))
                || d.get("max_stacks").is_some())
        {
            return Err("CONFIG_HEALING_RECEIVED_POLICY");
        }
        if d["attribute"] == "recovery_strength"
            && (!matches!(d["duplicate_policy"].as_str(), Some("refresh" | "ignore"))
                || d.get("max_stacks").is_some())
        {
            return Err("CONFIG_RECOVERY_STRENGTH_POLICY");
        }
        if d["attribute"] == "spirit_recovery_lock"
            && (d["modifier_bps"] != -10000
                || !matches!(d["duplicate_policy"].as_str(), Some("refresh" | "ignore"))
                || d.get("max_stacks").is_some())
        {
            return Err("CONFIG_SPIRIT_RECOVERY_LOCK_POLICY");
        }
        if (d.get("attribute").is_some() || d["mechanism"] == "timed_attribute_modifier")
            && v["mechanism_version"].as_u64().is_none_or(|m| m < 21)
        {
            return Err("CONFIG_STATUS_ATTRIBUTE_CAPABILITY");
        }
        if d.get("attribute").is_some() && d["mechanism"] != "timed_attribute_modifier" && !control
        {
            return Err("CONFIG_STATUS_ATTRIBUTE_POLICY");
        }
        if (d.get("max_stacks").is_some() || d["duplicate_policy"] == "stack")
            && v["mechanism_version"].as_u64().is_none_or(|m| m < 20)
        {
            return Err("CONFIG_STATUS_STACK_CAPABILITY");
        }
        if d.get("max_stacks").is_some() && d["duplicate_policy"] != "stack" {
            return Err("CONFIG_STATUS_STACK_POLICY");
        }
    }
    if let Some(cap) = p["status_rules"].get("attack_bonus_cap_bps") {
        if v["mechanism_version"].as_u64().is_none_or(|m| m < 21) {
            return Err("CONFIG_STATUS_ATTRIBUTE_CAPABILITY");
        }
        if cap.as_i64().is_none_or(|n| !(0..=100000).contains(&n)) {
            return Err("CONFIG_STATUS_ATTACK_CAP");
        }
    }
    if let Some(cap) = p["status_rules"].get("shield_cap_bps") {
        if v["mechanism_version"].as_u64().is_none_or(|m| m < 22) {
            return Err("CONFIG_SHIELD_CAPABILITY");
        }
        if cap.as_u64().is_none_or(|n| n > 10000) {
            return Err("CONFIG_SHIELD_CAP");
        }
    }
    if present
        && p["status_definitions"]
            .as_array()
            .is_none_or(|d| d.is_empty())
    {
        return Err("CONFIG_STATUS_DEFINITION");
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusDefinition {
    #[serde(skip)]
    pub retained_source: bool,
    #[serde(
        default,
        deserialize_with = "crate::protocol::nonempty_status_modifiers"
    )]
    pub base_modifiers: Vec<crate::protocol::StatusModifier>,
    #[serde(default, deserialize_with = "crate::protocol::present_value")]
    pub stack_counter: Option<super::stack_counters::StackCounterConfig>,
    #[serde(skip)]
    pub end_effects: u8,
    #[serde(skip)]
    pub action_origin: Option<crate::origin::ActionOrigin>,
    #[serde(skip)]
    pub derived: bool,
    #[serde(skip)]
    pub periodic_inputs: Option<crate::status::PeriodicInputs>,
    #[serde(default, deserialize_with = "crate::protocol::present_value")]
    pub periodic_stack: Option<PeriodicStackPolicy>,
    #[serde(skip)]
    pub periodic_added_stacks: u32,
    #[serde(skip)]
    pub application_stacks: Option<u32>,
    #[serde(skip)]
    pub periodic_raw_spirit: Option<u32>,
    #[serde(skip)]
    pub cleanse: Option<crate::protocol::StatusCleanseView>,
    #[serde(default, deserialize_with = "crate::protocol::nonempty_status_flags")]
    pub flags: Vec<StatusAttribute>,
    #[serde(skip)]
    pub window: Option<crate::protocol::WindowRecordView>,
    #[serde(skip)]
    pub window_preserve: bool,
    #[serde(skip)]
    pub shield_record: Option<crate::protocol::ShieldRecordView>,
    #[serde(skip)]
    pub shield_record_preserve: bool,
    #[serde(skip)]
    pub shield_preserve_higher: bool,
    #[serde(skip)]
    pub shield_record_reduction: u32,
    #[serde(default, deserialize_with = "crate::protocol::present_value")]
    pub harmful: Option<bool>,
    #[serde(skip)]
    pub periodic_sampling: Option<crate::protocol::PeriodicSampling>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::nonempty_status_modifiers"
    )]
    pub modifiers: Vec<crate::protocol::StatusModifier>,
    #[serde(skip)]
    pub periodic_damage_kind: Option<super::DamageKind>,
    #[serde(skip)]
    pub periodic_raw_damage: Option<u32>,
    #[serde(skip)]
    pub periodic_raw_heal: Option<u32>,
    #[serde(skip)]
    pub periodic_tick_on_expiry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<StatusAttribute>,
    #[serde(default)]
    pub shield_bps: Option<u32>,
    #[serde(default, deserialize_with = "crate::protocol::present_value")]
    pub shield_family: Option<String>,
    #[serde(default)]
    pub tick_interval_ms: Option<u64>,
    #[serde(default)]
    pub damage_bps: Option<u32>,
    #[serde(default)]
    pub heal_bps: Option<u32>,
    #[serde(default, deserialize_with = "crate::protocol::present_value")]
    pub overheal_shield: Option<OverhealShield>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stacks: Option<u32>,
    pub id: String,
    pub mechanism: StatusMechanism,
    pub duration_ms: u64,
    #[serde(default)]
    pub modifier_bps: i32,
    pub duplicate_policy: DuplicatePolicy,
    pub source_scoped: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PeriodicStackPolicy {
    pub tick_policy: PeriodicStackTick,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub healing_reduction_cap_bps: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub stacks_per_application: Option<u32>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) enum PeriodicStackTick {
    Preserve,
    Restart,
}
impl TryFrom<String> for PeriodicStackTick {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "preserve" => Ok(Self::Preserve),
            "restart" => Ok(Self::Restart),
            _ => Err("CONFIG_PERIODIC_STACK_TICK"),
        }
    }
}
impl From<PeriodicStackTick> for String {
    fn from(value: PeriodicStackTick) -> Self {
        match value {
            PeriodicStackTick::Preserve => "preserve",
            PeriodicStackTick::Restart => "restart",
        }
        .into()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OverhealShield {
    pub status_id: String,
    pub conversion_bps: u32,
}

impl PveConfig {
    pub fn overheal_shield_definition(
        &self,
        conversion: &OverhealShield,
    ) -> Option<&StatusDefinition> {
        if !(1..=10000).contains(&conversion.conversion_bps) {
            return None;
        }
        self.status_rules.as_ref()?.shield_cap_bps?;
        self.status_definitions.iter().find(|d| {
            d.id == conversion.status_id
                && d.is_shield()
                && d.shield_family.is_none()
                && matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
        })
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StatusMechanism {
    TimedCompoundModifier,
    TimedDefenseModifier,
    TimedAttributeModifier,
    TimedControl,
    AbsorptionShield,
    PeriodicDamage,
    PeriodicHeal,
}
impl StatusDefinition {
    pub(crate) fn maximum_stacks(&self) -> u32 {
        self.stack_counter
            .as_ref()
            .map(|c| c.maximum)
            .or(self.max_stacks)
            .unwrap_or(1)
    }
    pub fn is_hostile_control(&self) -> bool {
        self.flags.iter().any(|a| a.is_control_flag())
            || matches!(
                self.attribute,
                Some(
                    StatusAttribute::Silence
                        | StatusAttribute::Root
                        | StatusAttribute::Stun
                        | StatusAttribute::MovementSlow
                )
            )
    }

    pub fn is_harmful(&self) -> bool {
        self.harmful.unwrap_or_else(|| {
            self.is_hostile_control()
                || self.attribute == Some(StatusAttribute::BasicIntervalDelay)
                || self.modifiers.iter().chain(&self.base_modifiers).any(|m| {
                    m.attribute == StatusAttribute::BasicIntervalDelay && m.modifier_bps > 0
                })
                || self.is_periodic() && !self.is_heal()
                || self.modifier_bps < 0
                || self
                    .modifiers
                    .iter()
                    .chain(&self.base_modifiers)
                    .any(|m| m.modifier_bps < 0)
        })
    }
    pub fn is_periodic(&self) -> bool {
        matches!(
            self.mechanism,
            StatusMechanism::PeriodicDamage | StatusMechanism::PeriodicHeal
        )
    }
    pub fn is_heal(&self) -> bool {
        matches!(self.mechanism, StatusMechanism::PeriodicHeal)
    }
    pub fn is_shield(&self) -> bool {
        matches!(self.mechanism, StatusMechanism::AbsorptionShield)
    }
    pub fn effective_attribute(&self) -> StatusAttribute {
        self.attribute.unwrap_or(StatusAttribute::Def)
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DuplicatePolicy {
    Refresh,
    Ignore,
    Stack,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusRules {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub attribute_caps_bps: std::collections::BTreeMap<StatusAttribute, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield_cap_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_bonus_cap_bps: Option<i32>,
    pub max_active_per_entity: usize,
    pub defense_bonus_cap_bps: i32,
}

impl StatusRules {
    pub(crate) fn cap(&self, a: StatusAttribute, fallback: i64) -> i64 {
        self.attribute_caps_bps
            .get(&a)
            .map_or(fallback, |n| i64::from(*n))
    }
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let mut hit_bindings: Vec<_> = c
        .player_skills()
        .map(|s| &s.on_hit_target_statuses)
        .collect();
    let mut self_bindings: Vec<_> = c
        .player_skills()
        .map(|s| &s.on_commit_self_statuses)
        .collect();
    if let Some(rules) = &c.combat_rules {
        self_bindings.extend(
            rules
                .self_statuses_by_mastery
                .values()
                .flatten()
                .map(|row| &row.statuses),
        );
        hit_bindings.extend(
            rules
                .unit_hit_statuses_by_mastery
                .values()
                .flatten()
                .map(|row| &row.statuses),
        );
    }
    let mut bindings: Vec<_> = self_bindings
        .iter()
        .chain(hit_bindings.iter())
        .copied()
        .collect();
    if let Some(p) = &c.pve {
        for ability in std::iter::once(&p.combat.projectile).chain(
            p.additional_templates
                .values()
                .map(|t| &t.combat.projectile),
        ) {
            self_bindings.push(&ability.on_commit_self_statuses);
            hit_bindings.push(&ability.on_hit_target_statuses);
            bindings.extend([
                &ability.on_commit_self_statuses,
                &ability.on_hit_target_statuses,
            ]);
        }
    }
    let present = c
        .pve
        .as_ref()
        .is_some_and(|p| !p.status_definitions.is_empty() || p.status_rules.is_some())
        || bindings.iter().any(|b| !b.is_empty());
    if !present {
        return Ok(());
    }
    if c.schema_version != 4 || c.mechanism_version < 19 {
        return Err("CONFIG_STATUS_CAPABILITY");
    }
    let p = c.pve.as_ref().ok_or("CONFIG_STATUS_PVE_REQUIRED")?;
    let rules = p.status_rules.as_ref().ok_or("CONFIG_STATUS_RULES")?;
    if p.status_definitions.is_empty()
        || p.status_definitions.len() > definition_limit(c)
        || !(1..=32).contains(&rules.max_active_per_entity)
        || !(0..=100000).contains(&rules.defense_bonus_cap_bps)
    {
        return Err("CONFIG_STATUS_BOUNDS");
    }
    if rules
        .attack_bonus_cap_bps
        .is_some_and(|n| c.mechanism_version < 21 || !(0..=100000).contains(&n))
    {
        return Err("CONFIG_STATUS_ATTACK_CAP");
    }
    if rules
        .shield_cap_bps
        .is_some_and(|n| c.mechanism_version < 22 || n > 10000)
    {
        return Err("CONFIG_SHIELD_CAP");
    }
    let numeric_limit = |legacy: i32| {
        if c.combat_rules.is_some() {
            100000
        } else {
            legacy
        }
    };
    let mut ids = BTreeSet::new();
    let mut item_only_ids = BTreeSet::new();
    for d in &p.status_definitions {
        if let Some(conversion) = &d.overheal_shield
            && (!d.is_heal() || p.overheal_shield_definition(conversion).is_none())
        {
            return Err("CONFIG_PERIODIC_OVERHEAL_SHIELD_BOUNDS");
        }
        match (d.mechanism, d.attribute) {
            (StatusMechanism::PeriodicDamage | StatusMechanism::PeriodicHeal, None)
                if c.mechanism_version >= if d.is_heal() { 24 } else { 23 } =>
            {
                let interval = d.tick_interval_ms.ok_or("CONFIG_PERIODIC_INTERVAL")?;
                let invalid_amount = if d.is_heal() {
                    d.heal_bps.is_none_or(|n| n > 10000) || d.damage_bps.is_some()
                } else {
                    d.damage_bps.is_none_or(|n| n > 100000) || d.heal_bps.is_some()
                };
                if interval < c.step_ms
                    || interval >= d.duration_ms
                    || (d.duration_ms - 1) / interval > 128
                    || invalid_amount
                    || d.modifier_bps != 0
                    || d.shield_bps.is_some()
                    || if d.periodic_stack.is_some() {
                        !d.source_scoped
                            || !matches!(d.duplicate_policy, DuplicatePolicy::Stack)
                            || d.max_stacks.is_none_or(|n| !(1..=32).contains(&n))
                            || d.duration_ms.div_ceil(interval) > 128
                    } else {
                        d.max_stacks.is_some()
                            || matches!(d.duplicate_policy, DuplicatePolicy::Stack)
                    }
                {
                    return Err("CONFIG_PERIODIC_POLICY");
                }
            }
            (StatusMechanism::AbsorptionShield, None) if c.mechanism_version >= 22 => {
                if d.shield_bps.is_none_or(|n| !(1..=10000).contains(&n))
                    || rules.shield_cap_bps.is_none()
                    || d.modifier_bps != 0
                    || matches!(d.duplicate_policy, DuplicatePolicy::Stack)
                    || d.max_stacks.is_some()
                {
                    return Err("CONFIG_SHIELD_POLICY");
                }
            }
            (
                StatusMechanism::TimedDefenseModifier | StatusMechanism::TimedCompoundModifier,
                None,
            ) => {}
            (
                StatusMechanism::TimedAttributeModifier,
                Some(
                    StatusAttribute::ElementResistance
                    | StatusAttribute::MaxHp
                    | StatusAttribute::MaxSpirit
                    | StatusAttribute::Spd,
                ),
            ) => {}
            (
                StatusMechanism::TimedAttributeModifier,
                Some(StatusAttribute::BasicIntervalDelay),
            ) => {
                if c.combat_rules.is_none()
                    || !(1..=100000).contains(&d.modifier_bps)
                    || !d.source_scoped
                    || d.max_stacks.is_some()
                    || !matches!(
                        d.duplicate_policy,
                        DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
                    )
                {
                    return Err("CONFIG_BASIC_INTERVAL_DELAY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::HealingReceived)) => {
                if c.mechanism_version < 25
                    || !matches!(
                        d.duplicate_policy,
                        DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
                    )
                    || d.max_stacks.is_some()
                {
                    return Err("CONFIG_HEALING_RECEIVED_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::DamageReduction)) => {
                if !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=numeric_limit(MAX_DAMAGE_REDUCTION_BPS)).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_DAMAGE_REDUCTION_POLICY");
                }
                if c.combat_rules.is_none() {
                    item_only_ids.insert(d.id.as_str());
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::RecoveryStrength)) => {
                if matches!(d.duplicate_policy, DuplicatePolicy::Stack) || d.max_stacks.is_some() {
                    return Err("CONFIG_RECOVERY_STRENGTH_POLICY");
                }
            }
            (
                StatusMechanism::TimedAttributeModifier,
                Some(StatusAttribute::SpiritRecoveryLock),
            ) => {
                if d.modifier_bps != -10000
                    || matches!(d.duplicate_policy, DuplicatePolicy::Stack)
                    || d.max_stacks.is_some()
                {
                    return Err("CONFIG_SPIRIT_RECOVERY_LOCK_POLICY");
                }
            }
            (
                StatusMechanism::TimedControl,
                Some(
                    attribute @ (StatusAttribute::Silence
                    | StatusAttribute::Root
                    | StatusAttribute::Stun
                    | StatusAttribute::ControlImmunity
                    | StatusAttribute::DamageImmunity
                    | StatusAttribute::FirstControlImmunity),
                ),
            ) => {
                if d.modifier_bps != 0
                    || !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || d.max_stacks.is_some()
                {
                    return Err(match attribute {
                        StatusAttribute::Stun => "CONFIG_NPC_STUN_POLICY",
                        StatusAttribute::Root => "CONFIG_NPC_ROOT_POLICY",
                        StatusAttribute::ControlImmunity => "CONFIG_CONTROL_IMMUNITY_POLICY",
                        StatusAttribute::DamageImmunity => "CONFIG_DAMAGE_IMMUNITY_POLICY",
                        StatusAttribute::FirstControlImmunity => {
                            "CONFIG_FIRST_CONTROL_IMMUNITY_POLICY"
                        }
                        _ => "CONFIG_SILENCE_POLICY",
                    });
                }
            }
            (
                StatusMechanism::TimedAttributeModifier,
                Some(
                    StatusAttribute::ControlDurationReduction
                    | StatusAttribute::HarmfulDurationReduction,
                ),
            ) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=numeric_limit(MAX_CONTROL_DURATION_REDUCTION_BPS))
                        .contains(&d.modifier_bps)
                {
                    return Err("CONFIG_CONTROL_DURATION_REDUCTION_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::MovementSlow)) => {
                let source_refresh =
                    d.source_scoped && matches!(d.duplicate_policy, DuplicatePolicy::Refresh);
                let configured_stack = c.combat_rules.is_some()
                    && matches!(d.duplicate_policy, DuplicatePolicy::Stack);
                if !(source_refresh || configured_stack) || !(-9999..=-1).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_NPC_MOVEMENT_SLOW_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::MovementHaste)) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=10000).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_MOVEMENT_HASTE_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::CooldownReduction)) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=numeric_limit(MAX_COOLDOWN_REDUCTION_BPS)).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_COOLDOWN_REDUCTION_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::ArmorPenetration)) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=10000).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_ARMOR_PENETRATION_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::CriticalChance)) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=10000).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_CRITICAL_CHANCE_POLICY");
                }
            }
            (
                StatusMechanism::TimedAttributeModifier,
                Some(StatusAttribute::CriticalDamageBonus),
            ) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=numeric_limit(MAX_CRITICAL_DAMAGE_BONUS_BPS)).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_CRITICAL_DAMAGE_BONUS_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::DodgePenalty)) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=10000).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_DODGE_PENALTY_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::DodgeChance)) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=numeric_limit(MAX_DODGE_BPS as i32)).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_DODGE_CHANCE_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::Accuracy)) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=numeric_limit(MAX_ACCURACY_BPS as i32)).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_ACCURACY_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::LifeSteal)) => {
                if !(-10000..=100000).contains(&d.modifier_bps) {
                    return Err("CONFIG_LIFE_STEAL_MODIFIER");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(StatusAttribute::SkillPowerBonus)) => {
                if !d.source_scoped
                    || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    || !(1..=100000).contains(&d.modifier_bps)
                {
                    return Err("CONFIG_MEMBER_SKILL_POWER_POLICY");
                }
            }
            (StatusMechanism::TimedAttributeModifier, Some(attribute))
                if c.mechanism_version >= 21
                    && !matches!(
                        attribute,
                        StatusAttribute::Silence
                            | StatusAttribute::Root
                            | StatusAttribute::Stun
                            | StatusAttribute::ControlImmunity
                            | StatusAttribute::DamageImmunity
                            | StatusAttribute::FirstControlImmunity
                    ) => {}
            _ => return Err("CONFIG_STATUS_ATTRIBUTE_POLICY"),
        }
        if !d.is_periodic()
            && (d.tick_interval_ms.is_some()
                || d.damage_bps.is_some()
                || d.heal_bps.is_some()
                || d.periodic_stack.is_some())
        {
            return Err("CONFIG_PERIODIC_POLICY");
        }
        if !d.is_shield() && d.shield_bps.is_some() {
            return Err("CONFIG_SHIELD_POLICY");
        }
        if d.effective_attribute() == StatusAttribute::Atk && rules.attack_bonus_cap_bps.is_none() {
            return Err("CONFIG_STATUS_ATTACK_CAP");
        }
        match (d.duplicate_policy, d.max_stacks) {
            (DuplicatePolicy::Stack, Some(1..=128)) if c.mechanism_version >= 20 => {}
            (DuplicatePolicy::Refresh | DuplicatePolicy::Ignore, None) => {}
            _ => return Err("CONFIG_STATUS_STACK_POLICY"),
        }
        if d.id.is_empty()
            || d.id.len() > 64
            || d.id.chars().any(char::is_control)
            || !ids.insert(d.id.as_str())
            || !(c.step_ms..=c.terminal_ms.min(3600000)).contains(&d.duration_ms)
            || !(-10000..=100000).contains(&d.modifier_bps)
        {
            return Err("CONFIG_STATUS_DEFINITION");
        }
    }
    for binding in bindings {
        if binding.iter().any(|id| item_only_ids.contains(id.as_str())) {
            return Err("CONFIG_DAMAGE_REDUCTION_BINDING");
        }
        let unique: BTreeSet<_> = binding.iter().collect();
        if binding.len() > 2
            || unique.len() != binding.len()
            || binding.iter().any(|id| !ids.contains(id.as_str()))
        {
            return Err("CONFIG_STATUS_REFERENCE");
        }
    }
    if self_bindings.iter().any(|b| {
        b.iter().any(|id| {
            p.status_definitions
                .iter()
                .any(|d| &d.id == id && d.is_periodic() && !d.is_heal())
        })
    }) {
        return Err("CONFIG_PERIODIC_BINDING");
    }
    if hit_bindings.iter().any(|b| {
        b.iter().any(|id| {
            p.status_definitions
                .iter()
                .any(|d| &d.id == id && d.is_heal())
        })
    }) {
        return Err("CONFIG_PERIODIC_HEAL_BINDING");
    }
    if c.combat_rules.is_some() {
        if self_bindings.iter().any(|b| {
            b.iter().any(|id| {
                p.status_definitions
                    .iter()
                    .any(|d| d.id == *id && !d.flags.is_empty() && d.is_hostile_control())
            })
        }) || hit_bindings.iter().any(|b| {
            b.iter().any(|id| {
                p.status_definitions
                    .iter()
                    .any(|d| d.id == *id && !d.flags.is_empty() && !d.is_hostile_control())
            })
        }) {
            return Err("CONFIG_COMPOUND_FLAGS_BINDING");
        }
        return Ok(());
    }
    if hit_bindings.into_iter().flatten().any(|id| {
        p.status_definitions
            .iter()
            .any(|d| &d.id == id && d.attribute == Some(StatusAttribute::RecoveryStrength))
    }) {
        return Err("CONFIG_RECOVERY_STRENGTH_BINDING");
    }
    if c.player_skills()
        .flat_map(|skill| &skill.on_hit_target_statuses)
        .any(|id| {
            p.status_definitions
                .iter()
                .any(|d| &d.id == id && d.attribute == Some(StatusAttribute::SpiritRecoveryLock))
        })
    {
        return Err("CONFIG_SPIRIT_RECOVERY_LOCK_BINDING");
    }
    if let Some(control) = self_bindings
        .iter()
        .copied()
        .chain(c.player_skills().map(|skill| &skill.on_hit_target_statuses))
        .flatten()
        .find_map(|id| {
            p.status_definitions
                .iter()
                .find(|d| &d.id == id && d.is_hostile_control())
        })
    {
        return Err(match control.attribute {
            Some(StatusAttribute::Root) => "CONFIG_NPC_ROOT_BINDING",
            Some(StatusAttribute::Stun) => "CONFIG_NPC_STUN_BINDING",
            Some(StatusAttribute::MovementSlow) => "CONFIG_NPC_MOVEMENT_SLOW_BINDING",
            _ => "CONFIG_SILENCE_BINDING",
        });
    }
    for (attribute, error) in [
        (
            StatusAttribute::DodgeChance,
            "CONFIG_MEMBER_DODGE_CHANCE_BINDING",
        ),
        (StatusAttribute::Accuracy, "CONFIG_MEMBER_ACCURACY_BINDING"),
        (
            StatusAttribute::SkillPowerBonus,
            "CONFIG_MEMBER_SKILL_POWER_BINDING",
        ),
        (
            StatusAttribute::DodgePenalty,
            "CONFIG_MEMBER_DODGE_PENALTY_BINDING",
        ),
        (
            StatusAttribute::CriticalDamageBonus,
            "CONFIG_MEMBER_CRITICAL_DAMAGE_BONUS_BINDING",
        ),
        (
            StatusAttribute::CriticalChance,
            "CONFIG_MEMBER_CRITICAL_CHANCE_BINDING",
        ),
        (
            StatusAttribute::ArmorPenetration,
            "CONFIG_MEMBER_ARMOR_PENETRATION_BINDING",
        ),
        (
            StatusAttribute::CooldownReduction,
            "CONFIG_MEMBER_COOLDOWN_REDUCTION_BINDING",
        ),
        (
            StatusAttribute::MovementHaste,
            "CONFIG_MEMBER_MOVEMENT_HASTE_BINDING",
        ),
        (
            StatusAttribute::FirstControlImmunity,
            "CONFIG_FIRST_CONTROL_IMMUNITY_BINDING",
        ),
        (
            StatusAttribute::ControlImmunity,
            "CONFIG_CONTROL_IMMUNITY_BINDING",
        ),
        (
            StatusAttribute::DamageImmunity,
            "CONFIG_DAMAGE_IMMUNITY_BINDING",
        ),
        (
            StatusAttribute::ControlDurationReduction,
            "CONFIG_CONTROL_DURATION_REDUCTION_BINDING",
        ),
    ] {
        let grants = |id: &String| {
            p.status_definitions
                .iter()
                .any(|d| &d.id == id && d.attribute == Some(attribute))
        };
        if c.player_skills().any(|skill| {
            !skill.is_self() && skill.on_commit_self_statuses.iter().any(grants)
                || skill.on_hit_target_statuses.iter().any(grants)
        }) {
            return Err(error);
        }
    }
    let actors = 1
        + p.monster_positions.len()
        + p.extra_spawns.len()
        + p.encounters
            .iter()
            .map(|e| e.all_monsters().count())
            .sum::<usize>()
        + p.guardian()
            .map_or(0, |g| p.chest_spawns.len() * g.max_count as usize);
    // Conservative wire budget: at most 128 status instances, each < 768 bytes.
    // This leaves space for the existing full snapshot in a 256KiB JSONL response.
    if actors * rules.max_active_per_entity > 128 {
        return Err("CONFIG_STATUS_PUBLICATION_BUDGET");
    }
    Ok(())
}

// Keep old normalized definitions byte-for-byte equivalent as JSON values while
// giving shields their own parameters instead of exposing an unused modifier.
impl Serialize for StatusDefinition {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("mechanism", &self.mechanism)?;
        map.serialize_entry("duration_ms", &self.duration_ms)?;
        map.serialize_entry("duplicate_policy", &self.duplicate_policy)?;
        map.serialize_entry("source_scoped", &self.source_scoped)?;
        if let Some(harmful) = self.harmful {
            map.serialize_entry("harmful", &harmful)?;
        }
        if !self.modifiers.is_empty() {
            map.serialize_entry("modifiers", &self.modifiers)?;
        }
        if !self.base_modifiers.is_empty() {
            map.serialize_entry("base_modifiers", &self.base_modifiers)?;
        }
        if let Some(counter) = &self.stack_counter {
            map.serialize_entry("stack_counter", counter)?;
        }
        if !self.flags.is_empty() {
            map.serialize_entry("flags", &self.flags)?;
        }
        if self.is_periodic() {
            if let Some(policy) = self.periodic_stack {
                map.serialize_entry("periodic_stack", &policy)?;
            }
            map.serialize_entry("tick_interval_ms", &self.tick_interval_ms)?;
            if self.is_heal() {
                map.serialize_entry("heal_bps", &self.heal_bps)?;
            } else {
                map.serialize_entry("damage_bps", &self.damage_bps)?;
            }
        } else if self.is_shield() {
            map.serialize_entry("shield_bps", &self.shield_bps)?;
            if let Some(family) = &self.shield_family {
                map.serialize_entry("shield_family", family)?;
            }
        } else if !matches!(
            self.mechanism,
            StatusMechanism::TimedControl | StatusMechanism::TimedCompoundModifier
        ) {
            map.serialize_entry("modifier_bps", &self.modifier_bps)?;
        }
        if let Some(attribute) = self.attribute {
            map.serialize_entry("attribute", &attribute)?;
        }
        if let Some(cap) = self.max_stacks {
            map.serialize_entry("max_stacks", &cap)?;
        }
        if let Some(conversion) = &self.overheal_shield {
            map.serialize_entry("overheal_shield", conversion)?;
        }
        map.end()
    }
}
