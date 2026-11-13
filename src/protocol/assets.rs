use super::*;
use sha2::{Digest, Sha224};

pub(crate) const SCOPED_REWARD_PREFIX: &str = "@scope/";

/// The wallet key for a reward channel bound to a trusted weapon instance.
pub fn equipped_weapon_reward_asset_id(channel: &str, instance_id: &str) -> Option<String> {
    if [channel, instance_id]
        .into_iter()
        .any(|id| !valid_id(id) || id.len() > 64)
    {
        return None;
    }
    let mut hash = Sha224::new();
    hash.update(channel.as_bytes());
    hash.update([0]);
    hash.update(instance_id.as_bytes());
    Some(format!("{SCOPED_REWARD_PREFIX}{:x}", hash.finalize()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAmount {
    pub asset_id: String,
    pub quantity: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetParticipantFacts {
    pub principal: String,
    pub revision: u32,
    pub hp_points: u32,
    pub spirit_points: u32,
    pub atk_points: u32,
    pub def_points: u32,
    pub spd_points: u32,
    pub assets: Vec<AssetAmount>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub instance_count: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub warehouse_other_slots: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub location: Option<AccountLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountLocation {
    pub region_id: String,
    pub revision: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BalanceChange {
    pub asset_id: String,
    pub before: u32,
    pub after: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryClock {
    pub remaining: BalanceChange,
    pub elapsed: BalanceChange,
}

impl EntryClock {
    pub(crate) fn charged(&self) -> Option<u32> {
        self.remaining.before.checked_sub(self.remaining.after)
    }

    pub(crate) fn reversed(&self) -> Self {
        let reverse = |row: &BalanceChange| BalanceChange {
            asset_id: row.asset_id.clone(),
            before: row.after,
            after: row.before,
        };
        Self {
            remaining: reverse(&self.remaining),
            elapsed: reverse(&self.elapsed),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageGrowth {
    pub asset_id: String,
    pub points_bps: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageGrowth {
    pub asset_id: String,
    pub before: u32,
    pub after: u32,
    pub stored: u32,
    pub overflow: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceStorage {
    pub before: u32,
    pub after: u32,
    pub stored_ids: Vec<String>,
    pub overflow_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarehouseSlots {
    pub before: u32,
    pub after: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrowthConversion {
    pub rule_id: String,
    pub changes: Vec<BalanceChange>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionTransition {
    pub before: AccountLocation,
    pub after: AccountLocation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackFlow {
    pub item_id: String,
    pub picked_up: u64,
    pub consumed: u64,
    pub released: u64,
    pub relic_lost: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressionSettlement {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub reward_weapon_instance_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub threshold_progress: Option<ThresholdProgress>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub reward_earnings: Option<RewardEarnings>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub stack_flows: Option<Box<[StackFlow]>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub damage: Vec<DamageGrowth>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub storage: Vec<StorageGrowth>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub instance_storage: Option<InstanceStorage>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub warehouse_slots: Option<WarehouseSlots>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub conversion: Option<Box<GrowthConversion>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub conversion_steps: Option<Box<[GrowthConversion]>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub entry_charged: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub region: Option<RegionTransition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetInventory {
    pub item_id: String,
    pub carried: u32,
    pub looted: u32,
    pub kept: u32,
    pub dropped: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemInstance {
    pub instance_id: String,
    pub item_id: String,
    pub quality: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardBalance {
    pub asset_id: String,
    pub base_points: u64,
    pub requested: u32,
    pub awarded: u32,
    pub balance_before: u32,
    pub balance_after: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub adjustment: Option<Box<RewardAdjustment>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardAdjustment {
    pub original_requested: u32,
    pub deferred_payout: u32,
    pub changes: Vec<BalanceChange>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetSettlement {
    pub outcome: Outcome,
    pub inventory: Vec<AssetInventory>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<ItemInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rewards: Vec<RewardBalance>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub progression: Option<ProgressionSettlement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring: Option<ScoreSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_progress: Option<MapProgressSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_progress: Option<ResetProgressSummary>,
}

/// Registered bundles of inventory and score rules. Selected by startup configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementPolicy {
    Preserve,
    Attrition,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchResult {
    pub result_id: String,
    pub policy: SettlementPolicy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultSettlement {
    pub result: MatchResult,
    pub inventory: Vec<AssetInventory>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<ItemInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rewards: Vec<RewardBalance>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub progression: Option<ProgressionSettlement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring: Option<ScoreSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_progress: Option<MapProgressSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_progress: Option<ResetProgressSummary>,
}

/// One atomic account transaction. Amounts are frozen by Rust, never calculated by the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssetTransaction {
    Admission {
        expected_revision: u32,
        debits: Vec<AssetAmount>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        clock: Option<EntryClock>,
    },
    Settlement {
        expected_revision: u32,
        credits: Vec<AssetAmount>,
        summary: AssetSettlement,
    },
    ResultSettlement {
        expected_revision: u32,
        credits: Vec<AssetAmount>,
        summary: ResultSettlement,
    },
    Compensation {
        original_operation_id: String,
        credits: Vec<AssetAmount>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        clock: Option<EntryClock>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdProgress {
    pub stage_index: u32,
    pub total: u32,
    pub current_threshold: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub next_threshold: Option<u32>,
    pub balance_met: bool,
    pub progress: BalanceChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardEarnings {
    pub rates: Vec<RewardEarningRate>,
    pub kills: Vec<RewardKill>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardEarningRate {
    pub asset_id: String,
    pub bps: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardKill {
    pub template_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub basic_profile: Option<String>,
    pub kills: u32,
}
