//! Strict wire alternatives: self effects do not carry dormant projectile parameters.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlayerSkillMechanism {
    PiercingProjectile,
    Projectile,
    UnitProjectile,
    UnitEffect,
    SelfStatus,
    AllyHeal,
    AllyShield,
    GroundCircle,
    GroundField,
    LineField,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "mechanism", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum PlayerSkillWire {
    PiercingProjectile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        unavoidable: Option<bool>,
        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        ignore_armor: Option<bool>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_repeat"
        )]
        repeat_interval_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        end_on_source_invalid: bool,
        ttl_ms: u64,
        max_hits: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charge: Option<super::charge::ChargeConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<super::channel::ChannelConfig>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        damage_bps: u32,
        range: f64,
        speed: f64,
        radius: f64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_commit_self_statuses: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_hit_target_statuses: Vec<String>,
    },
    Projectile {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        end_on_source_invalid: bool,
        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        unavoidable: Option<bool>,
        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        ignore_armor: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charge: Option<super::charge::ChargeConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<super::channel::ChannelConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recast: Option<super::recast::RecastConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        damage_bps: u32,
        range: f64,
        speed: f64,
        radius: f64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_commit_self_statuses: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_hit_target_statuses: Vec<String>,
    },
    UnitProjectile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        unavoidable: Option<bool>,
        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        ignore_armor: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charge: Option<super::charge::ChargeConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<super::channel::ChannelConfig>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        damage_bps: u32,
        range: f64,
        speed: f64,
        radius: f64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_commit_self_statuses: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_hit_target_statuses: Vec<String>,
    },
    UnitEffect {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        unavoidable: Option<bool>,
        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        ignore_armor: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_steal: Option<super::ResourceSteal>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<super::channel::ChannelConfig>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        damage_bps: u32,
        range: f64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_commit_self_statuses: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_hit_target_statuses: Vec<String>,
    },
    GroundCircle {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        unavoidable: Option<bool>,
        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        ignore_armor: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charge: Option<super::charge::ChargeConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<super::channel::ChannelConfig>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        damage_bps: u32,
        range: f64,
        radius: f64,
        warning_ms: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_commit_self_statuses: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_hit_target_statuses: Vec<String>,
    },
    GroundField {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        unavoidable: Option<bool>,
        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        ignore_armor: Option<bool>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "super::contact_policy::present"
        )]
        contact_policy: Option<ContactPolicyConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charge: Option<super::charge::ChargeConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<super::channel::ChannelConfig>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        damage_bps: u32,
        range: f64,
        radius: f64,
        warning_ms: u64,
        duration_ms: u64,
        tick_interval_ms: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_commit_self_statuses: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_hit_target_statuses: Vec<String>,
    },
    LineField {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        unavoidable: Option<bool>,
        #[serde(
            default,
            deserialize_with = "crate::protocol::present_value",
            skip_serializing_if = "Option::is_none"
        )]
        ignore_armor: Option<bool>,
        contact_policy: ContactPolicyConfig,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charge: Option<super::charge::ChargeConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<super::channel::ChannelConfig>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        damage_bps: u32,
        length: f64,
        width: f64,
        warning_ms: u64,
        duration_ms: u64,
        tick_interval_ms: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_commit_self_statuses: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_hit_target_statuses: Vec<String>,
    },
    AllyHeal {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        channel: Option<super::channel::ChannelConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        range: f64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        heal: Option<super::InstantHeal>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        hot_status: Option<String>,
    },
    AllyShield {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        range: f64,
        shield_status: String,
    },
    SelfStatus {
        #[serde(default, skip_serializing_if = "super::stack_counters::is_false")]
        commit_on_accept: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery_cancel: Option<super::recovery_cancel::RecoveryCancelConfig>,

        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        self_heal: Option<super::InstantHeal>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<super::hp_cost::HpCost>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        resource_commit: Option<super::resource_commit::ResourceCommitPolicy>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cast_movement: Option<super::cast_movement::CastMovement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charge: Option<super::charge::ChargeConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<super::channel::ChannelConfig>,
        id: String,
        cost: u32,
        windup_ms: u64,
        recovery_ms: u64,
        cooldown_ms: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_commit_self_statuses: Vec<String>,
    },
}
impl From<PlayerSkillWire> for SkillConfig {
    fn from(w: PlayerSkillWire) -> Self {
        match w {
            PlayerSkillWire::PiercingProjectile {
                unavoidable,
                ignore_armor,
                repeat_interval_ms,
                end_on_source_invalid,
                ttl_ms,
                max_hits,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                speed,
                radius,
                on_commit_self_statuses,
                on_hit_target_statuses,
                recovery_cancel,
            } => Self {
                commit_on_accept: false,
                resource_steal: None,
                unavoidable,
                ignore_armor,
                ally_status: None,
                instant_heal: None,
                projectile_repeat_interval_ms: repeat_interval_ms,
                end_on_source_invalid,
                projectile_ttl_ms: ttl_ms,
                max_hits,
                contact_policy: None,
                duration_ms: 0,
                tick_interval_ms: 0,
                warning_ms: 0,
                mechanism: PlayerSkillMechanism::PiercingProjectile,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast: None,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                speed,
                radius,
                on_commit_self_statuses,
                on_hit_target_statuses,
            },
            PlayerSkillWire::Projectile {
                end_on_source_invalid,
                unavoidable,
                ignore_armor,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                speed,
                radius,
                on_commit_self_statuses,
                on_hit_target_statuses,
            } => Self {
                commit_on_accept: false,
                resource_steal: None,
                unavoidable,
                ignore_armor,
                ally_status: None,
                instant_heal: None,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy: None,
                duration_ms: 0,
                tick_interval_ms: 0,
                warning_ms: 0,
                mechanism: PlayerSkillMechanism::Projectile,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                speed,
                radius,
                on_commit_self_statuses,
                on_hit_target_statuses,
            },
            PlayerSkillWire::UnitProjectile {
                unavoidable,
                ignore_armor,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                speed,
                radius,
                on_commit_self_statuses,
                on_hit_target_statuses,
                recovery_cancel,
            } => Self {
                commit_on_accept: false,
                resource_steal: None,
                unavoidable,
                ignore_armor,
                ally_status: None,
                instant_heal: None,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid: false,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy: None,
                duration_ms: 0,
                tick_interval_ms: 0,
                warning_ms: 0,
                mechanism: PlayerSkillMechanism::UnitProjectile,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast: None,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                speed,
                radius,
                on_commit_self_statuses,
                on_hit_target_statuses,
            },
            PlayerSkillWire::UnitEffect {
                unavoidable,
                ignore_armor,
                hp_cost,
                resource_commit,
                resource_steal,
                cast_movement,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                on_commit_self_statuses,
                on_hit_target_statuses,
                recovery_cancel,
            } => Self {
                commit_on_accept: false,
                resource_steal,
                unavoidable,
                ignore_armor,
                ally_status: None,
                instant_heal: None,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid: false,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy: None,
                duration_ms: 0,
                tick_interval_ms: 0,
                warning_ms: 0,
                mechanism: PlayerSkillMechanism::UnitEffect,
                hp_cost,
                resource_commit,
                cast_movement,
                channel,
                recast: None,
                charge: None,
                speed: 0.,
                radius: 0.,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                on_commit_self_statuses,
                on_hit_target_statuses,
            },
            PlayerSkillWire::GroundCircle {
                unavoidable,
                ignore_armor,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                radius,
                warning_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                recovery_cancel,
            } => Self {
                commit_on_accept: false,
                resource_steal: None,
                unavoidable,
                ignore_armor,
                ally_status: None,
                instant_heal: None,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid: false,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy: None,
                duration_ms: 0,
                tick_interval_ms: 0,
                mechanism: PlayerSkillMechanism::GroundCircle,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast: None,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                radius,
                warning_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                speed: 0.,
            },
            PlayerSkillWire::GroundField {
                unavoidable,
                ignore_armor,
                contact_policy,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                radius,
                warning_ms,
                duration_ms,
                tick_interval_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                recovery_cancel,
            } => Self {
                commit_on_accept: false,
                resource_steal: None,
                unavoidable,
                ignore_armor,
                ally_status: None,
                instant_heal: None,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid: false,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy,
                mechanism: PlayerSkillMechanism::GroundField,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast: None,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                radius,
                warning_ms,
                duration_ms,
                tick_interval_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                speed: 0.,
            },
            PlayerSkillWire::LineField {
                unavoidable,
                ignore_armor,
                contact_policy,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                length,
                width,
                warning_ms,
                duration_ms,
                tick_interval_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                recovery_cancel,
            } => Self {
                commit_on_accept: false,
                resource_steal: None,
                unavoidable,
                ignore_armor,
                ally_status: None,
                instant_heal: None,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid: false,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy: Some(contact_policy),
                mechanism: PlayerSkillMechanism::LineField,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast: None,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range: length,
                radius: width / 2.,
                warning_ms,
                duration_ms,
                tick_interval_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                speed: 0.,
            },
            PlayerSkillWire::AllyHeal {
                channel,
                hp_cost,
                resource_commit,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                range,
                heal,
                hot_status,
                cast_movement,
                recovery_cancel,
            } => Self {
                commit_on_accept: false,
                resource_steal: None,
                unavoidable: None,
                ignore_armor: None,
                ally_status: hot_status,
                instant_heal: heal,
                mechanism: PlayerSkillMechanism::AllyHeal,
                id,
                hp_cost,
                resource_commit,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                range,
                damage_bps: 0,
                speed: 0.,
                radius: 0.,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid: false,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy: None,
                duration_ms: 0,
                tick_interval_ms: 0,
                warning_ms: 0,
                cast_movement,
                charge: None,
                channel,
                recast: None,
                recovery_cancel,
                on_commit_self_statuses: vec![],
                on_hit_target_statuses: vec![],
            },
            PlayerSkillWire::AllyShield {
                hp_cost,
                resource_commit,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                range,
                shield_status,
                cast_movement,
                recovery_cancel,
            } => Self {
                commit_on_accept: false,
                resource_steal: None,
                unavoidable: None,
                ignore_armor: None,
                ally_status: Some(shield_status),
                instant_heal: None,
                mechanism: PlayerSkillMechanism::AllyShield,
                id,
                hp_cost,
                resource_commit,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                range,
                damage_bps: 0,
                speed: 0.,
                radius: 0.,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid: false,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy: None,
                duration_ms: 0,
                tick_interval_ms: 0,
                warning_ms: 0,
                cast_movement,
                charge: None,
                channel: None,
                recast: None,
                recovery_cancel,
                on_commit_self_statuses: vec![],
                on_hit_target_statuses: vec![],
            },
            PlayerSkillWire::SelfStatus {
                commit_on_accept,
                self_heal,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                on_commit_self_statuses,
                recovery_cancel,
            } => Self {
                commit_on_accept,
                resource_steal: None,
                unavoidable: None,
                ignore_armor: None,
                ally_status: None,
                instant_heal: self_heal,
                projectile_repeat_interval_ms: None,
                end_on_source_invalid: false,
                projectile_ttl_ms: 0,
                max_hits: 0,
                contact_policy: None,
                duration_ms: 0,
                tick_interval_ms: 0,
                warning_ms: 0,
                mechanism: PlayerSkillMechanism::SelfStatus,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast: None,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                on_commit_self_statuses,
                on_hit_target_statuses: vec![],
                damage_bps: 0,
                range: 0.,
                speed: 0.,
                radius: 0.,
            },
        }
    }
}
impl From<SkillConfig> for PlayerSkillWire {
    fn from(s: SkillConfig) -> Self {
        let SkillConfig {
            commit_on_accept,
            unavoidable,
            ignore_armor,
            end_on_source_invalid,
            ally_status,
            instant_heal,
            projectile_repeat_interval_ms,
            projectile_ttl_ms,
            max_hits,
            contact_policy,
            duration_ms,
            tick_interval_ms,
            warning_ms,
            mechanism,
            hp_cost,
            resource_commit,
            resource_steal,
            cast_movement,
            charge,
            channel,
            recast,
            recovery_cancel,
            id,
            cost,
            windup_ms,
            recovery_ms,
            cooldown_ms,
            on_commit_self_statuses,
            on_hit_target_statuses,
            damage_bps,
            range,
            speed,
            radius,
        } = s;
        match mechanism {
            PlayerSkillMechanism::PiercingProjectile => Self::PiercingProjectile {
                end_on_source_invalid,
                recovery_cancel,
                unavoidable,
                ignore_armor,
                repeat_interval_ms: projectile_repeat_interval_ms,
                ttl_ms: projectile_ttl_ms,
                max_hits,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                damage_bps,
                range,
                speed,
                radius,
            },
            PlayerSkillMechanism::Projectile => Self::Projectile {
                end_on_source_invalid,
                unavoidable,
                ignore_armor,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                recast,
                recovery_cancel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                damage_bps,
                range,
                speed,
                radius,
            },
            PlayerSkillMechanism::UnitProjectile => Self::UnitProjectile {
                recovery_cancel,
                unavoidable,
                ignore_armor,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                damage_bps,
                range,
                speed,
                radius,
            },
            PlayerSkillMechanism::UnitEffect => Self::UnitEffect {
                recovery_cancel,
                unavoidable,
                ignore_armor,
                hp_cost,
                resource_commit,
                resource_steal,
                cast_movement,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
                damage_bps,
                range,
            },
            PlayerSkillMechanism::GroundCircle => Self::GroundCircle {
                recovery_cancel,
                unavoidable,
                ignore_armor,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                radius,
                warning_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
            },
            PlayerSkillMechanism::GroundField => Self::GroundField {
                recovery_cancel,
                unavoidable,
                ignore_armor,
                contact_policy,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                range,
                radius,
                warning_ms,
                duration_ms,
                tick_interval_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
            },
            PlayerSkillMechanism::LineField => Self::LineField {
                recovery_cancel,
                unavoidable,
                ignore_armor,
                contact_policy: contact_policy.expect("validated line contact policy"),
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                damage_bps,
                length: range,
                width: radius * 2.,
                warning_ms,
                duration_ms,
                tick_interval_ms,
                on_commit_self_statuses,
                on_hit_target_statuses,
            },
            PlayerSkillMechanism::AllyHeal => Self::AllyHeal {
                channel,
                recovery_cancel,
                cast_movement,
                hp_cost,
                resource_commit,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                range,
                heal: instant_heal,
                hot_status: ally_status,
            },
            PlayerSkillMechanism::AllyShield => Self::AllyShield {
                recovery_cancel,
                cast_movement,
                hp_cost,
                resource_commit,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                range,
                shield_status: ally_status.expect("ally shield status"),
            },
            PlayerSkillMechanism::SelfStatus => Self::SelfStatus {
                commit_on_accept,
                recovery_cancel,
                self_heal: instant_heal,
                hp_cost,
                resource_commit,
                cast_movement,
                charge,
                channel,
                id,
                cost,
                windup_ms,
                recovery_ms,
                cooldown_ms,
                on_commit_self_statuses,
            },
        }
    }
}
impl SkillConfig {
    pub(crate) fn is_piercing(&self) -> bool {
        self.mechanism == PlayerSkillMechanism::PiercingProjectile
    }
    pub(crate) fn is_line(&self) -> bool {
        self.mechanism == PlayerSkillMechanism::LineField
    }
    pub(crate) fn is_field(&self) -> bool {
        self.mechanism == PlayerSkillMechanism::GroundField || self.is_line()
    }
    pub(crate) fn is_ground(&self) -> bool {
        matches!(
            self.mechanism,
            PlayerSkillMechanism::GroundCircle | PlayerSkillMechanism::GroundField
        )
    }
    pub(crate) fn is_ally_heal(&self) -> bool {
        self.mechanism == PlayerSkillMechanism::AllyHeal
    }
    pub(crate) fn is_ally_shield(&self) -> bool {
        self.mechanism == PlayerSkillMechanism::AllyShield
    }
    pub(crate) fn targets_ally(&self) -> bool {
        self.is_ally_heal() || self.is_ally_shield()
    }
    pub(crate) fn is_unit_cast(&self) -> bool {
        self.is_unit() || self.targets_ally()
    }
    pub(crate) fn is_unit(&self) -> bool {
        matches!(
            self.mechanism,
            PlayerSkillMechanism::UnitProjectile | PlayerSkillMechanism::UnitEffect
        )
    }
    pub(crate) fn is_unit_effect(&self) -> bool {
        self.mechanism == PlayerSkillMechanism::UnitEffect
    }
    pub(crate) fn is_self(&self) -> bool {
        self.mechanism == PlayerSkillMechanism::SelfStatus
    }
}

#[cfg(all(test, match_sdk_test_data))]
#[path = "self_skill_tests.rs"]
mod tests;

#[cfg(all(test, match_sdk_test_data))]
#[path = "unit_skill_tests.rs"]
mod unit_tests;

#[cfg(all(test, match_sdk_test_data))]
#[path = "ground_skill_tests.rs"]
mod ground_tests;

#[cfg(all(test, match_sdk_test_data))]
#[path = "field_skill_tests.rs"]
mod field_tests;

#[cfg(all(test, match_sdk_test_data))]
#[path = "contact_policy_tests.rs"]
mod contact_tests;

#[cfg(all(test, match_sdk_test_data))]
#[path = "piercing_skill_tests.rs"]
mod piercing_tests;

fn present_repeat<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<u64>, D::Error> {
    u64::deserialize(d).map(Some)
}
#[cfg(all(test, match_sdk_test_data))]
#[path = "projectile_repeat_tests.rs"]
mod repeat_tests;

#[cfg(all(test, match_sdk_test_data))]
#[path = "line_field_tests.rs"]
mod line_tests;
