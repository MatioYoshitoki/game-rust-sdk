use serde::{Deserialize, Serialize};
use std::sync::Arc;
mod area;
mod assets;
mod basic_followup;
mod basic_pursuit;
pub(crate) use basic_pursuit::MAX_BASIC_PURSUIT_MS;
mod member_loot_drop;
pub(crate) use member_loot_drop::MAX_MEMBER_LOOT_DROPS;
mod member_item_use;
pub(crate) use member_item_use::{
    MAX_MEMBER_ITEM_COOLDOWN_MS, MAX_MEMBER_ITEM_QUALITY_BPS, MAX_MEMBER_ITEM_RECOVERY_BPS,
    MAX_MEMBER_ITEM_USES,
};
mod boss;
mod catalog;
mod chain_damage;
mod contact_policy;
mod contact_status;
pub(crate) use chain_damage::ChainDamageRule;
pub(crate) use contact_status::{ContactSampling, ContactStatusRule};
mod periodic_recovery;
mod shield_records;
pub(crate) use periodic_recovery::{PeriodicRecoveryRule, SpiritConversionWhen};
mod status_cleanses;
pub(crate) use status_cleanses::{CleanseAction, CleanseScope, StatusCleanseRule};
mod status_windows;
pub(crate) use status_windows::{MAX_COMBAT_RECORD_AMOUNT, StatusWindowRule};
mod status_operations;
pub(crate) use status_operations::{
    DeathGuard, StatusDisposition, StatusPolicy, blocked as status_blocked,
};
mod triggers;
pub(crate) use shield_records::{RecordRefresh, ShieldRecordRule};
pub(crate) use triggers::{
    CombatFact, CombatTrigger, TriggerCounting, TriggerEffect, TriggerScope, TriggerSubject,
};
mod basic_profiles;
pub(crate) use basic_profiles::{BasicEndTargetStatus, MAX_BASIC_PROFILES};
pub(crate) mod ally_recovery;
mod combat;
mod control_policies;
pub(crate) use combat::{
    AttackRule, CombatProfile, CombatRules, DamageKind, GroundTargetPolicy, LifeStealTiming,
};
pub(crate) use contact_policy::ContactPolicyConfig;
pub(crate) use control_policies::{ControlPolicy, hard_control};
mod encounters;
mod heavy;
#[cfg(all(test, match_sdk_test_data))]
mod legacy;
mod objective;
mod parameters;
mod skill_wire;
use skill_wire::{PlayerSkillMechanism, PlayerSkillWire};
mod cast_movement;
mod channel;
mod recast;
mod recovery_cancel;
pub(crate) use recovery_cancel::RecoveryCancelConfig;
mod resource_limits;
pub(crate) use channel::{MAX_CHANNEL_MS, MAX_CHANNEL_PULSES};
pub(crate) use recast::MAX_RECAST_WINDOW_MS;
pub(crate) use resource_limits::{MAX_BASE_RESOURCE, MAX_SPIRIT_COST};
mod charge;
pub(crate) use charge::{
    ChargeConfig, MAX_AIM_TURN_DEG_PER_SEC, MAX_CHARGE_MS, MIN_AIM_INTERVAL_MS,
};
mod followup;
pub(crate) use followup::MAX_FOLLOWUP_WINDOW_MS;
mod cast_interception;
mod character_build;
mod conditional_activation;
mod delayed_damage;
mod hp_cost;
mod resource_commit;
mod source_gates;
mod stack_counters;
pub(crate) use source_gates::MasteryStatuses;
pub(crate) use source_gates::{ActionFilter, SkillRequirement};
pub(crate) use stack_counters::threshold_leaf as stack_threshold_leaf;
mod status_ends;
mod status_retaliation;
pub(crate) use character_build::*;
mod resource_steal;
pub(crate) use cast_movement::PhaseMovement;
pub(crate) use resource_steal::{ResourceBonusMode, ResourceSteal};
pub(crate) mod moving_enter_area;
pub(crate) mod npc_ground_field;
mod player_skills;
mod skill_loadouts;
pub(crate) mod weapon_equipment;
pub(crate) use player_skills::InstantHeal;
pub(crate) use skill_loadouts::{MAX_SKILL_DEFINITIONS, initial_skill_loadout, valid_loadout};
mod population;
mod reset;
mod scoring;
mod status;
pub(crate) use status::PeriodicStackTick;
pub(crate) mod checkpoint_zones;
pub(crate) mod match_mode;
mod member_cast_anchor;
pub(crate) mod training_targets;
pub(crate) use member_cast_anchor::MAX_ANCHOR_RECAST_COUNT;
mod member_ground_trap;
pub(crate) mod member_summons;
mod support;
mod timed_position;
pub(crate) mod two_members;
mod validate;
pub(crate) use area::*;
pub(crate) use boss::*;
pub use catalog::ConfigCatalog;
pub(crate) use catalog::default_catalog;
pub(crate) use encounters::*;
pub(crate) use heavy::*;
pub(crate) use objective::*;
pub(crate) use parameters::*;
pub(crate) use player_skills::MAX_MEMBER_SKILLS;
pub(crate) use population::*;
pub(crate) use reset::*;
pub(crate) use status::*;
pub(crate) use support::*;
pub(crate) use timed_position::*;
pub(crate) use two_members::{MembersConfig, TwoMembersConfig};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub min: [f64; 2],
    pub max: [f64; 2],
    pub walls: Vec<Rect>,
}

impl Geometry {
    pub fn walkable(&self, p: [f64; 2], radius: f64) -> bool {
        if (0..2).any(|i| p[i] - radius < self.min[i] || p[i] + radius > self.max[i]) {
            return false;
        }
        !self.walls.iter().any(|wall| {
            let nearest = [
                p[0].clamp(wall.min[0], wall.max[0]),
                p[1].clamp(wall.min[1], wall.max[1]),
            ];
            distance(p, nearest) <= radius
        })
    }

    pub fn path_clear(&self, start: [f64; 2], end: [f64; 2], radius: f64) -> bool {
        let steps = (distance(start, end) / 2.0).ceil().max(1.0) as u32;
        (1..=steps).all(|step| {
            let t = f64::from(step) / f64::from(steps);
            self.walkable(
                [
                    start[0] + (end[0] - start[0]) * t,
                    start[1] + (end[1] - start[1]) * t,
                ],
                radius,
            )
        })
    }
}

pub(crate) fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

pub(crate) fn normalize(v: [f64; 2]) -> [f64; 2] {
    let length = v[0].hypot(v[1]);
    if length <= 1e-9 {
        [0.0, 0.0]
    } else if length.is_infinite() && v.iter().all(|value| value.is_finite()) {
        let scale = v[0].abs().max(v[1].abs());
        let scaled = [v[0] / scale, v[1] / scale];
        let length = scaled[0].hypot(scaled[1]);
        [scaled[0] / length, scaled[1] / length]
    } else {
        [v[0] / length, v[1] / length]
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetConfig {
    pub id: String,
    pub position: [f64; 2],
    pub hp: u32,
    pub atk: u32,
    pub armor: u32,
    pub speed: f64,
    pub radius: f64,
    pub aggro_range: f64,
    pub attack_range: f64,
    pub interval_ms: u64,
    pub drop_quantity: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicConfig {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub stacked_wave: Option<BasicStackedWave>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub charge: Option<BasicChargeConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub effects: Option<basic_profiles::BasicEffects>,
    #[serde(default, skip_serializing_if = "stack_counters::is_false")]
    pub settle_on_end: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub piercing: Option<BasicPiercingConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub wave: Option<BasicWaveConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub impact: Option<BasicImpactConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub beam: Option<BasicBeamConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub projectile_followup: Option<BasicProjectileFollowup>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub unavoidable: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_armor: Option<bool>,
    pub mechanism: ProjectileMechanism,
    pub windup_ms: u64,
    pub interval_ms: u64,
    pub range: f64,
    pub projectile_speed: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicChargeConfig {
    pub max_ms: u64,
    pub behavior: BasicChargeBehavior,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BasicChargeBehavior {
    StagedWave {
        upgrades: Vec<BasicChargeWave>,
    },
    Guard {
        reduction_bps: u32,
        reflect_bps: u32,
        reflect_damage_kind: DamageKind,
    },
    Dash(Box<BasicChargeDash>),
    Flight(Box<BasicChargeFlight>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicChargeFlight {
    pub plan_id: String,
    pub target_cap: usize,
    pub contact_interval_ms: u64,
    pub status_id: String,
    pub contact_stacks: u32,
    pub damage_rule: AttackRule,
    pub stages: Vec<BasicFlightStage>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicFlightStage {
    pub after_ms: u64,
    pub radius: f64,
    pub contact_damage_bps: u32,
    pub max_contacts_per_target: u32,
    pub burst_damage_bps: u32,
    pub burst_radius: f64,
    pub burst_stacks: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicStackedWave {
    pub stack_status: String,
    pub range: f64,
    pub finisher: BasicWaveConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicChargeWave {
    pub after_ms: u64,
    pub range: f64,
    pub wave: BasicWaveConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicChargeDash {
    pub ready_interval_ms: u64,
    pub tap_max_ms: u64,
    pub min_distance: f64,
    pub max_distance: f64,
    pub half_width: f64,
    pub target_cap: usize,
    pub primary_damage_bps: u32,
    pub additional_damage_bps: u32,
    pub knockback_distance: f64,
    pub stun_status: String,
    pub stun_resistant_profiles: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicPiercingConfig {
    pub plan_id: String,
    pub max_hits: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicWaveConfig {
    pub plan_id: String,
    pub geometry: BasicWaveGeometryConfig,
    pub speed: f64,
    pub target_cap: usize,
    pub primary_damage_bps: u32,
    pub additional_damage_bps: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub behavior: Option<BasicWaveBehavior>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub field: Option<BasicWaveField>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicWaveField {
    pub plan_id: String,
    pub trigger: BasicFieldTrigger,
    pub sampling: BasicFieldSampling,
    pub radius: f64,
    pub duration_ms: u64,
    pub interval_ms: u64,
    pub damage_bps: u32,
    pub target_cap: usize,
    pub max_per_source: usize,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub owner_reduction_bps: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_tick_statuses: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub damage_rule: Option<AttackRule>,
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BasicFieldTrigger {
    PrimaryContact {},
    Landing { impact_radius: f64 },
    Ground {},
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BasicFieldSampling {
    Live,
    Frozen,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BasicWaveBehavior {
    RadialPush {
        distance: f64,
        resistant_profiles: Vec<String>,
        reduced_status: String,
    },
    Push {
        distance: f64,
        rehit_immunity_ms: u64,
        resistant_profiles: Vec<String>,
        reduced_status: String,
    },
    Alternate {
        pull_distance: f64,
        resistant_profiles: Vec<String>,
        mark_duration_ms: u64,
        marked_damage_bonus_bps: u32,
        alternate: BasicAlternateWave,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicAlternateWave {
    pub plan_id: String,
    pub range: f64,
    pub speed: f64,
    pub primary_damage_bps: u32,
    pub additional_damage_bps: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BasicWaveGeometryConfig {
    Directional { cone_degrees: f64 },
    Radial {},
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicBeamConfig {
    pub plan_id: String,
    pub interval_ms: u64,
    pub half_width: f64,
    pub target_cap: usize,
    pub damage_bps: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicProjectileFollowup {
    pub plan_id: String,
    pub primary_damage_bps: u32,
    pub pattern: BasicProjectilePattern,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BasicProjectilePattern {
    Companion {
        delay_ms: u64,
        damage_bps: u32,
    },
    Volley {
        hits: u32,
        interval_ms: u64,
        damage_bps: u32,
        attack_speed_bps_per_hit: u32,
    },
    SealRecall(BasicProjectileRecall),
    Ricochet(BasicProjectileRicochet),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicProjectileRecall {
    pub required_seals: u32,
    pub overtravel: f64,
    pub corridor_half_width: f64,
    pub additional_damage_bps: u32,
    pub recall_damage_bps: u32,
    pub settle_delay_ms: u64,
    pub interval_ms: u64,
    pub slow_status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicProjectileRicochet {
    pub max_targets: u32,
    pub search_radius: f64,
    pub damage_bps: u32,
    pub mark_duration_ms: u64,
    pub hits_to_stun: u32,
    pub stun_status: String,
    pub resistant_profiles: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub return_to_owner: Option<BasicProjectileReturn>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicProjectileReturn {
    pub speed_bps: u32,
    pub shield_status: String,
    pub base_max_hp_bps: u32,
    pub per_extra_max_hp_bps: u32,
    pub max_hp_bps: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicImpactConfig {
    pub plan_id: String,
    pub damage_bps: u32,
    pub timing: BasicImpactTiming,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub contact_status: Option<BasicContactStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicContactStatus {
    pub status_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub death_burst: Option<PeriodicDeathBurst>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PeriodicDeathBurst {
    pub radius: f64,
    pub damage_bps_per_stack: u32,
    pub inherited_stack_bps: u32,
    pub target_cap: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BasicImpactTiming {
    Hitscan {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        refraction: Option<BasicRefraction>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        followup: Option<BasicHitscanFollowup>,
    },
    FixedDelay {
        delay_ms: u64,
    },
    GroundField {
        delay_ms: u64,
        field: Box<BasicWaveField>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BasicHitscanFollowup {
    AlternatingRefraction {
        radius: f64,
        damage_bps: u32,
        max_contacts: usize,
        repeat_decay_bps: u32,
        minimum_damage_bps: u32,
    },
    TargetBurst {
        radius: f64,
        damage_bps: u32,
        max_targets: usize,
    },
}

impl BasicConfig {
    pub(crate) fn field(&self) -> Option<&BasicWaveField> {
        self.wave
            .as_ref()
            .and_then(|wave| wave.field.as_ref())
            .or_else(|| self.impact.as_ref().and_then(BasicImpactConfig::field))
    }
}

impl BasicImpactConfig {
    pub(crate) fn field(&self) -> Option<&BasicWaveField> {
        match &self.timing {
            BasicImpactTiming::GroundField { field, .. } => Some(field),
            _ => None,
        }
    }

    pub(crate) fn max_contacts(&self) -> usize {
        match &self.timing {
            BasicImpactTiming::Hitscan {
                followup: Some(followup),
                ..
            } => match followup {
                BasicHitscanFollowup::AlternatingRefraction { max_contacts, .. } => *max_contacts,
                BasicHitscanFollowup::TargetBurst { max_targets, .. } => *max_targets,
            },
            _ => 2,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicRefraction {
    pub radius: f64,
    pub damage_bps: u32,
}

impl BasicConfig {
    pub(crate) fn non_projectile(&self) -> bool {
        self.wave.is_some() || self.impact.is_some() || self.charged_flight().is_some()
    }

    pub(crate) fn charged_flight(&self) -> Option<&BasicChargeFlight> {
        match &self.charge.as_ref()?.behavior {
            BasicChargeBehavior::Flight(flight) => Some(flight),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(from = "PlayerSkillWire", into = "PlayerSkillWire")]
pub(crate) struct SkillConfig {
    pub commit_on_accept: bool,
    pub unavoidable: Option<bool>,
    pub ignore_armor: Option<bool>,
    pub ally_status: Option<String>,
    pub instant_heal: Option<player_skills::InstantHeal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp_cost: Option<hp_cost::HpCost>,
    pub resource_commit: Option<resource_commit::ResourceCommitPolicy>,
    pub resource_steal: Option<ResourceSteal>,
    pub cast_movement: Option<cast_movement::CastMovement>,
    pub charge: Option<charge::ChargeConfig>,
    pub channel: Option<channel::ChannelConfig>,
    pub recast: Option<recast::RecastConfig>,
    pub recovery_cancel: Option<recovery_cancel::RecoveryCancelConfig>,
    pub projectile_repeat_interval_ms: Option<u64>,
    /// Keep a member's in-flight shot alive after the member settles.
    /// Default false: shots end with their source (SourceInvalid).
    pub end_on_source_invalid: bool,
    pub projectile_ttl_ms: u64,
    pub max_hits: u32,
    pub contact_policy: Option<ContactPolicyConfig>,
    pub duration_ms: u64,
    pub tick_interval_ms: u64,
    pub warning_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_commit_self_statuses: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_hit_target_statuses: Vec<String>,
    pub mechanism: PlayerSkillMechanism,
    pub id: String,
    pub windup_ms: u64,
    pub recovery_ms: u64,
    pub cooldown_ms: u64,
    pub cost: u32,
    pub damage_bps: u32,
    pub range: f64,
    pub speed: f64,
    pub radius: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) world_boons: Option<world_boons::WorldBoons>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub(crate) settlement_facts: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub(crate) settlement_rewards:
        std::collections::BTreeMap<String, settlement_rewards::RewardRule>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) settlement_progression: Option<settlement_progression::SettlementProgression>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) world_loot: Option<world_loot::WorldLoot>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub(crate) participant_ai: std::collections::BTreeMap<String, participant_ai::ParticipantAi>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) cast_limits: Option<cast_movement::CastLimits>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) resource_limits: Option<resource_limits::ResourceLimits>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) combat_rules: Option<CombatRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) two_members: Option<TwoMembersConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) members: Option<MembersConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) match_mode: Option<match_mode::MatchMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) encounter_progress: Option<encounter_progress::EncounterProgress>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) extraction_lifecycle: Option<extraction_lifecycle::ExtractionLifecycle>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) contested_equipment: Option<contested_equipment::ContestedEquipment>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(crate) zone_exposure: Option<zone_exposure::ZoneExposure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) replication: Option<two_members::ReplicationConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) skill_followup: Option<followup::FollowupConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) basic_followup: Option<basic_followup::BasicFollowupConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) basic_pursuit: Option<basic_pursuit::BasicPursuitConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) member_loot_drop: Option<member_loot_drop::MemberLootDropConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) member_item_use:
        Option<std::collections::BTreeMap<String, member_item_use::MemberItemUseConfig>>,
    pub(crate) schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) asset_contract: Option<crate::asset_contract::AssetContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) result_contract: Option<crate::result_contract::ResultContract>,
    pub(crate) mechanism_version: u32,
    pub(crate) params: Parameters,
    #[serde(skip)]
    pub(crate) loaded_hash: String,
    #[serde(default)]
    pub(crate) pve: Option<Arc<PveConfig>>,
    pub(crate) id: String,
    pub(crate) source_commit: String,
    pub(crate) purpose: String,
    pub(crate) step_ms: u64,
    pub(crate) geometry: Geometry,
    pub(crate) spawn: [f64; 2],
    pub(crate) extraction_point: [f64; 2],
    pub(crate) target: TargetConfig,
    pub(crate) basic: BasicConfig,
    pub(crate) skill: SkillConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) additional_skills: Vec<SkillConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) skill_library: Vec<SkillConfig>,
    pub(crate) inventory_capacity: u32,
    pub(crate) stack_limit: u32,
    pub(crate) item_id: String,
    pub(crate) carried_quantity: u32,
    pub(crate) extraction_unlock_ms: u64,
    pub(crate) extraction_channel_ms: u64,
    pub(crate) terminal_ms: u64,
    pub(crate) kill_score: u32,
    pub(crate) extraction_score: u32,
    pub(crate) cultivation_coefficient: f64,
    pub(crate) death_multiplier: f64,
    pub(crate) defender_realm_atk: u32,
    pub(crate) defense_cap: f64,
    pub(crate) input_capacity: usize,
    pub(crate) output_capacity: usize,
    pub(crate) history_capacity: usize,
}

impl Config {
    pub(crate) fn pve_loot_bound(&self, item_id: &str, kills: u32, logical_ms: u64) -> u32 {
        let pve = self.pve.as_ref().expect("PVE loot bounds");
        let monster = if pve.additional_templates.is_empty()
            && pve.extra_spawns.is_empty()
            && pve.encounters.is_empty()
            && pve.guardian().is_none()
        {
            self.monster_loot_bound(&pve.template_id, &pve.drop_table, item_id) * u64::from(kills)
        } else {
            let mut budget = u64::from(kills);
            let mut monster = 0u64;
            let mut credited = vec![];
            for (id, count) in pve.population_counts(logical_ms.min(self.terminal_ms)) {
                let template = pve.profile(&id).expect("validated population template");
                let per = self.monster_loot_bound(&id, &template.drop_table, item_id);
                if template.kill_credit {
                    credited.push((per, u64::from(count)));
                } else {
                    monster += per * u64::from(count);
                }
            }
            credited.sort_unstable_by(|a, b| b.cmp(a));
            for (per, count) in credited {
                let used = budget.min(count);
                monster += per * used;
                budget -= used;
            }
            monster
        };
        (monster
            + self.pve_chest_loot_bound(item_id, logical_ms)
            + self.world_loot_bound(item_id, logical_ms))
        .min(u64::from(u32::MAX)) as u32
    }
    pub(crate) fn pve_chest_loot_bound(&self, item_id: &str, logical_ms: u64) -> u64 {
        let pve = self.pve.as_ref().unwrap();
        pve.treasure_rules.as_ref().map_or(0, |rules| {
            let unlock = rules
                .template
                .seal
                .as_ref()
                .map_or(0, |s| u64::from(s.layers) * s.layer_duration_ms);
            let cycles = logical_ms
                .min(self.terminal_ms)
                .checked_sub(unlock)
                .map_or(0, |elapsed| {
                    elapsed / (rules.template.respawn_delay_ms + unlock) + 1
                });
            let per_open = rules
                .loot
                .guaranteed
                .iter()
                .chain(&rules.loot.random)
                .filter(|rule| rule.item_id == item_id && rule.chance != Some(0.0))
                .map(|rule| rule.quantity_max)
                .sum::<u32>();
            let once = u64::from(pve.map_objective.is_some());
            (pve.chest_spawns.len() as u64 * cycles - once * cycles.saturating_sub(1))
                * u64::from(per_open)
        })
    }
    pub(crate) fn cultivation(&self, kills: u32, outcome: crate::protocol::Outcome) -> u32 {
        let score = u64::from(kills) * u64::from(self.kill_score);
        self.cultivation_from_score(score, outcome)
    }
    pub(crate) fn cultivation_from_score(
        &self,
        score: u64,
        outcome: crate::protocol::Outcome,
    ) -> u32 {
        let outcome = self.settlement_policy_outcome(outcome);
        let score = score
            + if outcome == crate::protocol::Outcome::Extraction {
                u64::from(self.extraction_score)
            } else {
                0
            };
        let multiplier = if outcome == crate::protocol::Outcome::Death {
            self.death_multiplier
        } else {
            1.0
        };
        (score as f64 * self.cultivation_coefficient * multiplier).floor() as u32
    }
    pub fn frozen() -> Self {
        default_catalog().default_config().as_ref().clone()
    }
    #[cfg(all(test, match_sdk_test_data))]
    pub(crate) fn pve() -> Self {
        default_catalog()
            .active("b3-1")
            .expect("PVE active configuration")
            .as_ref()
            .clone()
    }
    pub(crate) fn hash(&self) -> String {
        self.loaded_hash.clone()
    }
    pub fn content_hash(&self) -> &str {
        &self.loaded_hash
    }
    #[cfg(all(test, match_sdk_test_data))]
    pub(crate) fn pve_mut(&mut self) -> &mut PveConfig {
        let pve = Arc::make_mut(self.pve.as_mut().unwrap());
        pve.cells = Arc::from([]);
        pve.profiles.clear();
        pve.prepared = Some(Arc::new(self.params.clone()));
        pve
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn source_commit(&self) -> &str {
        &self.source_commit
    }
    pub fn purpose(&self) -> &str {
        &self.purpose
    }
}

pub fn config_hash() -> String {
    default_catalog().default_config().hash()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PveConfig {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub npc_ai: Option<npc_ai::NpcAiConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub world_geometry: Option<world_geometry::WorldGeometry>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub isometric_grid: Option<isometric_grid::IsometricGrid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_definitions: Vec<StatusDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_rules: Option<StatusRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timed_position: Option<TimedPositionConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_objective: Option<MapObjectiveConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub encounters: Vec<EncounterConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kill_score: Option<u32>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub additional_templates: std::collections::BTreeMap<String, MonsterTemplate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_spawns: Vec<MonsterSpawn>,
    #[serde(skip)]
    pub profiles: std::collections::BTreeMap<String, Arc<MonsterTemplate>>,
    #[serde(skip)]
    pub prepared: Option<Arc<Parameters>>,
    pub template_id: String,
    pub combat: MonsterCombat,
    pub drop_table: LootTable,
    #[serde(skip)]
    pub cells: Arc<[usize]>,
    pub map_id: String,
    pub width: usize,
    pub height: usize,
    pub cell_size: f64,
    pub palette: Vec<Terrain>,
    pub cells_rle: Vec<(usize, usize)>,
    pub monster_positions: Vec<[f64; 2]>,
    #[serde(default)]
    pub chest_spawns: Vec<ChestSpawn>,
    #[serde(default)]
    pub treasure_rules: Option<TreasureRules>,
    pub stats: std::collections::BTreeMap<String, u32>,
    pub leash: f64,
    pub help_range: f64,
    pub memory_range: f64,
    pub items: std::collections::BTreeMap<String, Item>,
}

pub(crate) mod contested_equipment;
pub(crate) mod encounter_progress;
pub(crate) mod extraction_lifecycle;
mod isometric_grid;
pub(crate) mod npc_ai;
pub(crate) mod participant_ai;
mod settlement_facts;
pub(crate) mod settlement_progression;
mod settlement_rewards;
pub(crate) mod world_boons;
pub(crate) mod world_geometry;
pub(crate) mod world_loot;
pub(crate) mod zone_exposure;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChestSpawn {
    pub id: String,
    pub position: [f64; 2],
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TreasureRules {
    pub template: ContainerTemplate,
    pub loot: LootTable,
    pub interaction_range: f64,
    pub ground_capacity: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LootTable {
    pub guaranteed: Vec<LootRule>,
    pub random: Vec<LootRule>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LootRule {
    pub item_id: String,
    #[serde(default)]
    pub chance: Option<f64>,
    pub quantity_min: u32,
    pub quantity_max: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Terrain {
    pub terrain: String,
    pub dot_damage_per_second: f64,
    pub elevation: f64,
    pub max_climb_height: f64,
    pub max_drop_height: f64,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub slope: Option<TerrainSlope>,
    pub walkable: bool,
    pub speed_multiplier: f64,
    pub blocks_projectiles: bool,
    pub blocks_vision: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TerrainSlope {
    pub axis: TerrainAxis,
    pub end_elevation: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TerrainAxis {
    X,
    Y,
}

impl Terrain {
    pub(crate) fn damage_milli_per_second(&self) -> u64 {
        (self.dot_damage_per_second * 1000.0).round() as u64
    }
}

mod post_consumption;

mod contact_reactions;
pub(crate) use contact_reactions::{ContactReactionRule, ContactReactionWhen, ReactionAttacker};

mod finishers;
pub(crate) use finishers::{FinishPolicy, FinishRule, ThresholdComparison};

mod finisher_followups;
pub(crate) use finisher_followups::FinisherSelector;
