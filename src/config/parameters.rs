use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Parameters {
    pub player_radius: f64,
    pub pickup_range: f64,
    pub extraction_range: f64,
    pub movement: Movement,
    pub regen: Regeneration,
    pub basic_spirit_on_hit: u32,
    pub orb_hp_ratio: f64,
    pub orb_spirit_ratio: f64,
    pub defense_scale: f64,
    pub minimum_damage: u32,
    pub critical_multiplier: f64,
    pub metal_multiplier: f64,
    pub resource_ttl_ms: u64,
    pub drop_min_radius: u32,
    pub drop_max_radius: u32,
    pub basic_projectile_radius: f64,
    pub player_projectile_ttl_ms: u64,
    pub entry_years: u32,
    pub death: Death,
    pub initial_player: InitialPlayer,
    pub runtime: RuntimeParameters,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Movement {
    pub base: f64,
    pub bonus: f64,
    pub reference: f64,
    pub log_base: f64,
}
impl Movement {
    pub fn speed(&self, points: u32) -> f64 {
        if points == 0 {
            0.0
        } else {
            self.base
                + self.bonus
                    * ((f64::from(points) / self.reference).ln() / self.log_base.ln())
                        .clamp(0.0, 1.0)
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Regeneration {
    pub interval_ms: u64,
    pub near_radius: f64,
    pub near_ratio: f64,
    pub far_ratio: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Death {
    pub slot_ratio: f64,
    pub quantity_ratio: f64,
    pub minimum_quantity: u32,
}
impl Death {
    pub fn count(&self, slots: usize) -> usize {
        ((slots as f64 * self.slot_ratio).ceil() as usize).min(slots)
    }
    pub fn amount(&self, quantity: u32) -> u32 {
        ((f64::from(quantity) * self.quantity_ratio).floor() as u32)
            .max(self.minimum_quantity)
            .min(quantity)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InitialPlayer {
    pub hp_points: u32,
    pub spirit_points: u32,
    pub atk_points: u32,
    pub def_points: u32,
    pub spd_points: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeParameters {
    pub io_timeout_ms: u64,
    pub drain_timeout_ms: u64,
    pub reconnect_grace_ms: u64,
    pub publish_batched_ms: u64,
    pub nav_budget: usize,
    pub nav_repath_ms: u64,
    pub ai_decision_ms: u64,
    pub resource_sweep_ms: u64,
    pub call_timeout_ms: u64,
    pub max_attempts: u32,
    pub max_catchup_steps: u32,
    pub max_lag_steps: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MonsterCombat {
    pub radius: f64,
    pub attack_interval_ms: u64,
    pub aggro_range: f64,
    pub melee_range: f64,
    pub cast_chance: f64,
    pub return_distance: f64,
    pub lifesteal_ratio: f64,
    pub mark_max: u32,
    pub mark_duration_ms: u64,
    pub projectile: ProjectileAbility,
    pub heal: HealAbility,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectileAbility {
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_commit_self_statuses: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_hit_target_statuses: Vec<String>,
    pub mechanism: ProjectileMechanism,
    pub id: String,
    pub cost: u32,
    pub windup_ms: u64,
    pub cooldown_ms: u64,
    pub recovery_ms: u64,
    pub range: f64,
    pub speed: f64,
    pub radius: f64,
    pub ttl_ms: u64,
    pub damage_bps: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ProjectileMechanism {
    #[serde(rename = "projectile")]
    Projectile,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HealAbility {
    pub mechanism: HealMechanism,
    pub id: String,
    pub cost: u32,
    pub windup_ms: u64,
    pub cooldown_ms: u64,
    pub recovery_ms: u64,
    pub hp_threshold: f64,
    pub heal_bps: u32,
    pub interval_ms: u64,
    pub ticks: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum HealMechanism {
    #[serde(rename = "budget_heal_channel")]
    BudgetHealChannel,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Item {
    pub name: String,
    #[serde(rename = "stackLimit")]
    pub stack_limit: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContainerTemplate {
    pub id: String,
    pub name: String,
    pub respawn_delay_ms: u64,
    pub mechanism: ContainerMechanism,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seal: Option<SealConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardian: Option<GuardianConfig>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GuardianConfig {
    pub template_id: String,
    pub chance: f64,
    pub min_count: u32,
    pub max_count: u32,
    pub spawn_offsets: Vec<[f64; 2]>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SealConfig {
    pub layers: u32,
    pub layer_duration_ms: u64,
    pub interrupt_retention_ratio: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ContainerMechanism {
    #[serde(rename = "immediate_ground_loot")]
    ImmediateGroundLoot,
    #[serde(rename = "layered_seal_ground_loot")]
    LayeredSealGroundLoot,
}
