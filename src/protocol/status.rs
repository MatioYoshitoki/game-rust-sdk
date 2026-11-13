use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusAttribute {
    MaxHp,
    MaxSpirit,
    Spd,
    AttackSpeed,
    BasicIntervalDelay,
    ElementResistance,
    Atk,
    Def,
    HealingReceived,
    DamageReduction,
    RecoveryStrength,
    SpiritRecoveryLock,
    Silence,
    Root,
    Stun,
    MovementSlow,
    MovementHaste,
    CooldownReduction,
    ArmorPenetration,
    CriticalChance,
    CriticalDamageBonus,
    DodgePenalty,
    DodgeChance,
    Accuracy,
    SkillPowerBonus,
    LifeSteal,
    ControlImmunity,
    DamageImmunity,
    FirstControlImmunity,
    ControlDurationReduction,
    HarmfulDurationReduction,
}

impl<'de> Deserialize<'de> for StatusAttribute {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "max_hp" => Self::MaxHp,
            "max_spirit" => Self::MaxSpirit,
            "spd" => Self::Spd,
            "attack_speed" => Self::AttackSpeed,
            "basic_interval_delay" => Self::BasicIntervalDelay,
            "element_resistance" => Self::ElementResistance,
            "atk" => Self::Atk,
            "def" => Self::Def,
            "healing_received" => Self::HealingReceived,
            "damage_reduction" => Self::DamageReduction,
            "recovery_strength" => Self::RecoveryStrength,
            "spirit_recovery_lock" => Self::SpiritRecoveryLock,
            "silence" => Self::Silence,
            "root" => Self::Root,
            "stun" => Self::Stun,
            "movement_slow" => Self::MovementSlow,
            "movement_haste" => Self::MovementHaste,
            "cooldown_reduction" => Self::CooldownReduction,
            "armor_penetration" => Self::ArmorPenetration,
            "critical_chance" => Self::CriticalChance,
            "critical_damage_bonus" => Self::CriticalDamageBonus,
            "dodge_penalty" => Self::DodgePenalty,
            "dodge_chance" => Self::DodgeChance,
            "accuracy" => Self::Accuracy,
            "skill_power_bonus" => Self::SkillPowerBonus,
            "life_steal" => Self::LifeSteal,
            "control_immunity" => Self::ControlImmunity,
            "damage_immunity" => Self::DamageImmunity,
            "first_control_immunity" => Self::FirstControlImmunity,
            "control_duration_reduction" => Self::ControlDurationReduction,
            "harmful_duration_reduction" => Self::HarmfulDurationReduction,
            _ => return Err(serde::de::Error::custom("invalid StatusAttribute")),
        })
    }
}

impl StatusAttribute {
    pub(crate) fn is_control_flag(self) -> bool {
        matches!(self, Self::Silence | Self::Root | Self::Stun)
    }
    pub(crate) fn is_immunity_flag(self) -> bool {
        matches!(
            self,
            Self::ControlImmunity | Self::DamageImmunity | Self::FirstControlImmunity
        )
    }
}
pub(crate) fn nonempty_status_flags<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<StatusAttribute>, D::Error> {
    let flags = Vec::<StatusAttribute>::deserialize(d)?;
    if flags.is_empty() {
        return Err(serde::de::Error::custom("empty status flags"));
    }
    Ok(flags)
}

pub(crate) fn nonempty_status_modifiers<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<StatusModifier>, D::Error> {
    let modifiers = Vec::<StatusModifier>::deserialize(d)?;
    if modifiers.is_empty() {
        return Err(serde::de::Error::custom("empty status modifiers"));
    }
    Ok(modifiers)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusEntityKind {
    Participant,
    Monster,
}

impl<'de> Deserialize<'de> for StatusEntityKind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "participant" => Self::Participant,
            "monster" => Self::Monster,
            _ => return Err(serde::de::Error::custom("invalid StatusEntityKind")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntity {
    pub kind: StatusEntityKind,
    pub id: String,
    pub generation: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter_round: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusModifier {
    pub attribute: StatusAttribute,
    pub modifier_bps: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusView {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub retained_source: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "nonempty_status_modifiers"
    )]
    pub base_modifiers: Vec<StatusModifier>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub stack_counter: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub cleanse: Option<StatusCleanseView>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "nonempty_status_flags"
    )]
    pub flags: Vec<StatusAttribute>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub window: Option<WindowRecordView>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "nonempty_status_modifiers"
    )]
    pub modifiers: Vec<StatusModifier>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub periodic: Option<PeriodicView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub shield: Option<ShieldView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub attribute: Option<StatusAttribute>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub stacks: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub max_stacks: Option<u32>,
    pub instance_id: String,
    pub definition_id: String,
    pub source: StatusEntity,
    pub target: StatusEntity,
    pub applied_at_ms: u64,
    pub expires_at_ms: u64,
    pub modifier_bps: i32,
}

impl StatusView {
    pub fn has_attribute(&self, attribute: StatusAttribute) -> bool {
        self.attribute == Some(attribute) || self.flags.contains(&attribute)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowBasis {
    None,
    DamageDealt,
    DamageTaken,
    ShieldAbsorbed,
}

impl<'de> Deserialize<'de> for WindowBasis {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "none" => Self::None,
            "damage_dealt" => Self::DamageDealt,
            "damage_taken" => Self::DamageTaken,
            "shield_absorbed" => Self::ShieldAbsorbed,
            _ => return Err(serde::de::Error::custom("invalid WindowBasis")),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowMaximumSampling {
    Snapshot,
    Live,
}

impl<'de> Deserialize<'de> for WindowMaximumSampling {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "snapshot" => Self::Snapshot,
            "live" => Self::Live,
            _ => return Err(serde::de::Error::custom("invalid WindowMaximumSampling")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowRecordView {
    pub basis: WindowBasis,
    pub recorded: u64,
    pub cap: u64,
    pub contacts: u32,
    pub maximum_sampling: WindowMaximumSampling,
    pub max_hp_at_apply: u32,
    pub max_spirit_at_apply: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusRemovalReason {
    Expired,
    Depleted,
    Cleansed,
    Consumed,
    Transferred,
    TargetInvalid,
    SourceInvalid,
    Reset,
    Closed,
}

impl<'de> Deserialize<'de> for StatusRemovalReason {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "expired" => Self::Expired,
            "depleted" => Self::Depleted,
            "cleansed" => Self::Cleansed,
            "consumed" => Self::Consumed,
            "transferred" => Self::Transferred,
            "target_invalid" => Self::TargetInvalid,
            "source_invalid" => Self::SourceInvalid,
            "reset" => Self::Reset,
            "closed" => Self::Closed,
            _ => return Err(serde::de::Error::custom("invalid StatusRemovalReason")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<ShieldRecordView>,
    pub granted: u32,
    pub remaining: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShieldRecordBasis {
    Incoming,
    OwnAbsorbed,
}

impl<'de> Deserialize<'de> for ShieldRecordBasis {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "incoming" => Self::Incoming,
            "own_absorbed" => Self::OwnAbsorbed,
            _ => return Err(serde::de::Error::custom("invalid ShieldRecordBasis")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldRecordView {
    pub basis: ShieldRecordBasis,
    pub recorded: u32,
    pub cap: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldConsumption {
    pub instance_id: String,
    pub absorbed: u32,
    pub remaining: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodicEffectKind {
    ElementalDamage,
    TrueDamage,
    PhysicalDamage,
    MaxHpHeal,
}

impl<'de> Deserialize<'de> for PeriodicEffectKind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "elemental_damage" => Self::ElementalDamage,
            "true_damage" => Self::TrueDamage,
            "physical_damage" => Self::PhysicalDamage,
            "max_hp_heal" => Self::MaxHpHeal,
            _ => return Err(serde::de::Error::custom("invalid PeriodicEffectKind")),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodicTickOutcome {
    Applied,
    Protected,
    Returning,
}

impl<'de> Deserialize<'de> for PeriodicTickOutcome {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "applied" => Self::Applied,
            "protected" => Self::Protected,
            "returning" => Self::Returning,
            _ => return Err(serde::de::Error::custom("invalid PeriodicTickOutcome")),
        })
    }
}

/// Selects whose ATK is sampled and when. Live sampling occurs at execution time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodicSampling {
    SourceSnapshot,
    TargetSnapshot,
    SourceLive,
    TargetLive,
}

impl<'de> Deserialize<'de> for PeriodicSampling {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Ok(match value.as_str() {
            "source_snapshot" => Self::SourceSnapshot,
            "target_snapshot" => Self::TargetSnapshot,
            "source_live" => Self::SourceLive,
            "target_live" => Self::TargetLive,
            _ => return Err(serde::de::Error::custom("invalid PeriodicSampling")),
        })
    }
}

impl PeriodicSampling {
    pub(crate) fn live(self) -> bool {
        matches!(self, Self::SourceLive | Self::TargetLive)
    }
    pub(crate) fn target(self) -> bool {
        matches!(self, Self::TargetSnapshot | Self::TargetLive)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeriodicView {
    pub tick_on_expiry: Option<bool>,
    pub per_stack: Option<bool>,
    pub first_tick_at_ms: Option<u64>,
    pub raw_spirit: Option<u32>,
    pub power_sampling: Option<PeriodicSampling>,
    pub kind: PeriodicEffectKind,
    pub interval_ms: u64,
    pub next_tick_at_ms: Option<u64>,
    pub ticks_done: u32,
    pub total_ticks: u32,
    pub raw_damage: u32,
    pub raw_heal: Option<u32>,
}

impl PeriodicView {
    pub(crate) fn due_on_expiry(&self, expires: u64) -> bool {
        self.tick_on_expiry == Some(true) && self.next_tick_at_ms == Some(expires)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PeriodicWire {
    #[serde(default, deserialize_with = "present_value")]
    pub tick_on_expiry: Option<bool>,
    #[serde(default, deserialize_with = "present_value")]
    pub per_stack: Option<bool>,
    #[serde(default, deserialize_with = "present_value")]
    pub first_tick_at_ms: Option<u64>,
    #[serde(default, deserialize_with = "present_value")]
    pub raw_spirit: Option<u32>,
    #[serde(default, deserialize_with = "present_value")]
    pub power_sampling: Option<PeriodicSampling>,
    pub kind: PeriodicEffectKind,
    pub interval_ms: u64,
    #[serde(deserialize_with = "nullable_periodic_due")]
    pub next_tick_at_ms: Option<u64>,
    pub ticks_done: u32,
    pub total_ticks: u32,
    #[serde(default, deserialize_with = "present_value")]
    pub raw_damage: Option<u32>,
    #[serde(default, deserialize_with = "present_value")]
    pub raw_heal: Option<u32>,
}

fn nullable_periodic_due<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<u64>, D::Error> {
    Option::<u64>::deserialize(d)
}
impl<'de> Deserialize<'de> for PeriodicView {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let p = PeriodicWire::deserialize(d)?;
        if match p.kind {
            PeriodicEffectKind::MaxHpHeal => p.raw_damage.is_some() || p.raw_heal.is_none(),
            _ => p.raw_damage.is_none() || p.raw_heal.is_some() || p.raw_spirit.is_some(),
        } {
            return Err(serde::de::Error::custom(
                "conflicting or missing periodic amount",
            ));
        }
        Ok(Self {
            tick_on_expiry: p.tick_on_expiry,
            per_stack: p.per_stack,
            first_tick_at_ms: p.first_tick_at_ms,
            raw_spirit: p.raw_spirit,
            power_sampling: p.power_sampling,
            kind: p.kind,
            interval_ms: p.interval_ms,
            next_tick_at_ms: p.next_tick_at_ms,
            ticks_done: p.ticks_done,
            total_ticks: p.total_ticks,
            raw_damage: p.raw_damage.unwrap_or(0),
            raw_heal: p.raw_heal,
        })
    }
}

// Preserve the mechanism-23 wire shape, including zero-damage DOT. HOT exposes
// its own amount rather than an unrelated zero raw_damage field.
impl Serialize for PeriodicView {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(
            6 + usize::from(self.power_sampling.is_some())
                + usize::from(self.raw_spirit.is_some())
                + usize::from(self.per_stack.is_some())
                + usize::from(self.tick_on_expiry.is_some())
                + usize::from(self.first_tick_at_ms.is_some()),
        ))?;
        map.serialize_entry("kind", &self.kind)?;
        if let Some(tick_on_expiry) = self.tick_on_expiry {
            map.serialize_entry("tick_on_expiry", &tick_on_expiry)?;
        }
        if let Some(per_stack) = self.per_stack {
            map.serialize_entry("per_stack", &per_stack)?;
        }
        if let Some(first) = self.first_tick_at_ms {
            map.serialize_entry("first_tick_at_ms", &first)?;
        }
        if let Some(sampling) = self.power_sampling {
            map.serialize_entry("power_sampling", &sampling)?;
        }
        map.serialize_entry("interval_ms", &self.interval_ms)?;
        map.serialize_entry("next_tick_at_ms", &self.next_tick_at_ms)?;
        map.serialize_entry("ticks_done", &self.ticks_done)?;
        map.serialize_entry("total_ticks", &self.total_ticks)?;
        match self.kind {
            PeriodicEffectKind::PhysicalDamage
            | PeriodicEffectKind::ElementalDamage
            | PeriodicEffectKind::TrueDamage => {
                map.serialize_entry("raw_damage", &self.raw_damage)?
            }
            PeriodicEffectKind::MaxHpHeal => map.serialize_entry("raw_heal", &self.raw_heal)?,
        }
        if let Some(amount) = self.raw_spirit {
            map.serialize_entry("raw_spirit", &amount)?;
        }
        map.end()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealKind {
    BasicAttack,
    ResourceSteal,
    WindowRecovery,
    Triggered,
    LifeSteal,
    Hot,
    SelfChannel,
    SelfSkill,
    AllySkill,
    SupportChannel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusCleanseView {
    pub on_tick: bool,
    pub on_expired: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusCleansePhase {
    Tick,
    Expired,
}
