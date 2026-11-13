use super::*;
use crate::protocol::{
    AssetParticipantFacts, DamageGrowth, Outcome, ProgressionSettlement, RewardBalance,
    RewardEarningRate, RewardEarnings, ScoreSummary,
};
use std::collections::{BTreeMap, BTreeSet};

mod adjustments;

const MAX_CHANNELS: usize = 8;
const MAX_EARNING_RATES: usize = 16;
const MAX_EARNING_RATE_BPS: u32 = 100_000;
const NEUTRAL_RATE_BPS: u32 = 10_000;
const MAX_POINTS: u32 = 1_000_000;
const MAX_RATE_BPS: u32 = 100_000;
const MAX_CHANNEL_RATE_BPS: u32 = 5_000_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RewardRule {
    kill_points: BTreeMap<String, u32>,
    rate_bps: u32,
    balance_cap: u32,
    by_result: BTreeMap<String, ResultReward>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    multiply_by_entry_cost: bool,
    #[serde(default, skip_serializing_if = "EntryCostBasis::is_standard")]
    entry_cost_basis: EntryCostBasis,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    equipped_weapon: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    earning_scale: Option<EarningScale>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub(super) damage: Option<DamageReward>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    adjustments: Vec<adjustments::AdjustmentRule>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum EntryCostBasis {
    #[default]
    Standard,
    Charged,
}

impl EntryCostBasis {
    fn is_standard(&self) -> bool {
        matches!(self, Self::Standard)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EarningScale {
    default_asset_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    by_basic_profile: BTreeMap<String, String>,
}

impl EarningScale {
    fn asset_ids(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.default_asset_id.as_str())
            .chain(self.by_basic_profile.values().map(String::as_str))
    }

    fn rate(&self, rates: &[RewardEarningRate], profile: Option<&str>) -> u32 {
        let id = profile
            .and_then(|id| self.by_basic_profile.get(id))
            .unwrap_or(&self.default_asset_id);
        rates
            .iter()
            .find(|rate| &rate.asset_id == id)
            .expect("validated earning rate")
            .bps
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DamageReward {
    rate_bps: u32,
    per_hit_cap_points: u32,
    match_cap_points: u32,
    #[serde(default, skip_serializing_if = "DamageRounding::is_accumulated")]
    rounding: DamageRounding,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum DamageRounding {
    #[default]
    Accumulated,
    PerContact,
}

impl DamageRounding {
    fn is_accumulated(&self) -> bool {
        *self == Self::Accumulated
    }
}

impl DamageReward {
    fn maximum(&self) -> u64 {
        u64::from(self.match_cap_points) * 10_000
    }

    fn contact_points(&self, hp_damage: u32, rate_bps: u32) -> u64 {
        let points = (u128::from(hp_damage) * u128::from(self.rate_bps) * u128::from(rate_bps)
            / 10_000)
            .min(u128::from(self.per_hit_cap_points) * 10_000) as u64;
        match self.rounding {
            DamageRounding::Accumulated => points,
            DamageRounding::PerContact => points / 10_000 * 10_000,
        }
    }

    fn accepts(&self, points_bps: u64) -> bool {
        points_bps <= self.maximum()
            && (self.rounding.is_accumulated() || points_bps.is_multiple_of(10_000))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultReward {
    bonus_points: u32,
    multiplier_bps: u32,
}

impl RewardRule {
    fn requested(
        &self,
        points: u64,
        damage_points_bps: u64,
        multiplier_bps: u32,
        entry_cost: u32,
    ) -> Option<u32> {
        u32::try_from(
            (u128::from(points) * 10_000 + u128::from(damage_points_bps))
                * u128::from(self.rate_bps)
                * u128::from(multiplier_bps)
                * u128::from(if self.multiply_by_entry_cost {
                    entry_cost
                } else {
                    1
                })
                / 1_000_000_000_000,
        )
        .ok()
    }
}

impl Config {
    pub(crate) fn has_equipped_weapon_rewards(&self) -> bool {
        self.settlement_rewards
            .values()
            .any(|rule| rule.equipped_weapon)
    }

    pub(crate) fn earning_rate_assets(&self) -> BTreeSet<&str> {
        self.settlement_rewards
            .values()
            .filter_map(|rule| rule.earning_scale.as_ref())
            .flat_map(EarningScale::asset_ids)
            .collect()
    }

    pub(crate) fn loaded_earning_rates(
        &self,
        facts: &AssetParticipantFacts,
    ) -> Option<Vec<RewardEarningRate>> {
        self.earning_rate_assets()
            .into_iter()
            .map(|id| {
                let bps = super::settlement_progression::balance(facts, id)?;
                (bps <= MAX_EARNING_RATE_BPS).then(|| RewardEarningRate {
                    asset_id: id.into(),
                    bps,
                })
            })
            .collect()
    }

    fn valid_reward_earnings(
        &self,
        earnings: Option<&RewardEarnings>,
        scoring: Option<&ScoreSummary>,
    ) -> bool {
        let assets = self.earning_rate_assets();
        let Some(earnings) = earnings else {
            return assets.is_empty();
        };
        if assets.is_empty()
            || !assets
                .into_iter()
                .eq(earnings.rates.iter().map(|rate| rate.asset_id.as_str()))
            || earnings
                .rates
                .iter()
                .any(|rate| rate.bps > MAX_EARNING_RATE_BPS)
            || earnings.kills.len() > 17 * (super::MAX_BASIC_PROFILES + 1)
        {
            return false;
        }
        let Some(scoring) = scoring else { return false };
        let mut previous = None;
        let mut totals = BTreeMap::new();
        for row in &earnings.kills {
            let key = (row.template_id.as_str(), row.basic_profile.as_deref());
            if row.kills == 0
                || previous.is_some_and(|last| last >= key)
                || row.basic_profile.as_ref().is_some_and(|profile| {
                    self.basic_profiles()
                        .is_none_or(|profiles| !profiles.contains_key(profile))
                })
            {
                return false;
            }
            previous = Some(key);
            *totals.entry(key.0).or_insert(0u64) += u64::from(row.kills);
        }
        totals.into_iter().eq(scoring
            .templates
            .iter()
            .map(|row| (row.template_id.as_str(), u64::from(row.kills))))
    }

    pub(crate) fn record_damage_growth(
        &self,
        tally: &mut BTreeMap<String, u64>,
        hp_damage: u32,
        rates: &[RewardEarningRate],
        profile: Option<&str>,
    ) {
        for (id, rule) in &self.settlement_rewards {
            if let Some(damage) = &rule.damage {
                let rate = rule
                    .earning_scale
                    .as_ref()
                    .map_or(NEUTRAL_RATE_BPS, |scale| scale.rate(rates, profile));
                let total = tally.entry(id.clone()).or_default();
                *total = (*total + damage.contact_points(hp_damage, rate)).min(damage.maximum());
            }
        }
    }

    pub(crate) fn valid_damage_growth(&self, rows: &[DamageGrowth]) -> bool {
        let rules = self
            .settlement_rewards
            .iter()
            .filter_map(|(id, rule)| Some((id, rule.damage.as_ref()?)));
        let expected = rules.clone().count();
        rows.len() == expected
            && rows
                .iter()
                .zip(rules)
                .all(|(row, (id, rule))| &row.asset_id == id && rule.accepts(row.points_bps))
    }

    #[cfg(test)]
    pub(crate) fn settlement_reward_plan(
        &self,
        outcome: Outcome,
        scoring: Option<&ScoreSummary>,
        balance: impl Fn(&str) -> u32,
    ) -> Option<Vec<RewardBalance>> {
        self.reward_plan_with_damage(outcome, scoring, None, balance)
    }

    pub(crate) fn reward_plan_with_damage(
        &self,
        outcome: Outcome,
        scoring: Option<&ScoreSummary>,
        progression: Option<&ProgressionSettlement>,
        balance: impl Fn(&str) -> u32,
    ) -> Option<Vec<RewardBalance>> {
        let damage = progression.map_or(&[][..], |p| p.damage.as_slice());
        let earnings = progression.and_then(|p| p.reward_earnings.as_ref());
        if !self.valid_damage_growth(damage) || !self.valid_reward_earnings(earnings, scoring) {
            return None;
        }
        if self.settlement_rewards.is_empty() {
            return Some(vec![]);
        }
        let result = &self.result_binding(outcome)?.result_id;
        let entry_charge = self.wire_entry_charge(progression.and_then(|p| p.entry_charged))?;
        let weapon_instance = progression.and_then(|p| p.reward_weapon_instance_id.as_deref());
        let scoring = scoring?;
        let mut rows = Vec::with_capacity(self.settlement_rewards.len());
        for (id, rule) in &self.settlement_rewards {
            let asset_id = if rule.equipped_weapon {
                let Some(weapon) = weapon_instance else {
                    continue;
                };
                crate::protocol::equipped_weapon_reward_asset_id(id, weapon)?
            } else {
                id.clone()
            };
            let result_rule = rule.by_result.get(result)?;
            let kill_points =
                |template: &str| u64::from(rule.kill_points.get(template).copied().unwrap_or(0));
            let kill_points =
                if let Some((scale, earnings)) = rule.earning_scale.as_ref().zip(earnings) {
                    earnings
                        .kills
                        .iter()
                        .map(|row| {
                            let rate = scale.rate(&earnings.rates, row.basic_profile.as_deref());
                            (kill_points(&row.template_id) * u64::from(rate) / 10_000)
                                * u64::from(row.kills)
                        })
                        .sum::<u64>()
                } else {
                    scoring
                        .templates
                        .iter()
                        .map(|row| kill_points(&row.template_id) * u64::from(row.kills))
                        .sum::<u64>()
                };
            let base_points = u64::from(result_rule.bonus_points) + kill_points;
            let damage_points_bps = damage
                .iter()
                .find(|r| &r.asset_id == id)
                .map_or(0, |r| r.points_bps);
            let requested = rule.requested(
                base_points,
                damage_points_bps,
                result_rule.multiplier_bps,
                if rule.entry_cost_basis.is_standard() {
                    self.params.entry_years
                } else {
                    entry_charge
                },
            )?;
            let (requested, adjustment) = rule.adjusted_reward(requested, result, &balance)?;
            let before = balance(&asset_id);
            let awarded = requested.min(rule.balance_cap.saturating_sub(before));
            rows.push(RewardBalance {
                asset_id,
                base_points,
                requested,
                awarded,
                balance_before: before,
                balance_after: before + awarded,
                adjustment,
            });
        }
        rows.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        Some(rows)
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    if c.settlement_rewards.is_empty() {
        return Ok(());
    }
    let (Some(assets), Some(_), Some(pve)) = (&c.asset_contract, &c.result_contract, &c.pve) else {
        return Err("CONFIG_SETTLEMENT_REWARDS_CAPABILITY");
    };
    if c.protocol_version() != 6 || c.mechanism_version < 105 || c.cultivation_coefficient != 0.0 {
        return Err("CONFIG_SETTLEMENT_REWARDS_CAPABILITY");
    }
    if c.has_equipped_weapon_rewards()
        && (c.members.is_none()
            || c.weapon_catalog().is_none()
            || c.settlement_progression.is_none()
            || std::iter::once(&c.item_id)
                .chain([&assets.entry_cost_asset_id, &assets.reward_asset_id])
                .chain(pve.items.keys())
                .chain(c.settlement_rewards.keys())
                .any(|id| id.starts_with(crate::protocol::SCOPED_REWARD_PREFIX)))
    {
        return Err("CONFIG_SETTLEMENT_REWARDS_WEAPON");
    }
    let rate_assets = c.earning_rate_assets();
    adjustments::validate(c, assets, pve, &rate_assets)?;
    if !rate_assets.is_empty()
        && (c.members.is_none()
            || c.settlement_progression.is_none()
            || rate_assets.len() > MAX_EARNING_RATES
            || rate_assets.iter().any(|id| {
                !crate::protocol::valid_id(id)
                    || id.len() > 64
                    || id.starts_with(crate::protocol::SCOPED_REWARD_PREFIX)
                    || *id == c.item_id
                    || *id == assets.entry_cost_asset_id
                    || *id == assets.reward_asset_id
                    || pve.items.contains_key(*id)
                    || c.settlement_rewards.contains_key(*id)
            })
            || c.settlement_rewards
                .values()
                .filter_map(|rule| rule.earning_scale.as_ref())
                .any(|scale| {
                    scale.by_basic_profile.keys().any(|id| {
                        c.basic_profiles()
                            .is_none_or(|profiles| !profiles.contains_key(id))
                    })
                }))
    {
        return Err("CONFIG_SETTLEMENT_REWARDS_EARNING_SCALE");
    }
    let item_rows = pve.items.len() + usize::from(!pve.items.contains_key(&c.item_id));
    if c.settlement_rewards.len() > MAX_CHANNELS || item_rows + c.settlement_rewards.len() > 65 {
        return Err("CONFIG_SETTLEMENT_REWARDS_BOUNDS");
    }
    let profiles = pve.build_profiles();
    let population = pve.population_counts(c.terminal_ms);
    for (id, rule) in &c.settlement_rewards {
        if id.len() > 64
            || !crate::protocol::valid_id(id)
            || id == &assets.entry_cost_asset_id
            || id == &c.item_id
            || pve.items.contains_key(id)
            || !(1..=MAX_CHANNEL_RATE_BPS).contains(&rule.rate_bps)
            || !rule.entry_cost_basis.is_standard() && !rule.multiply_by_entry_cost
            || rule.damage.as_ref().is_some_and(|r| {
                c.settlement_progression.is_none()
                    || !(1..=MAX_RATE_BPS).contains(&r.rate_bps)
                    || !(1..=MAX_POINTS).contains(&r.match_cap_points)
                    || !(1..=r.match_cap_points).contains(&r.per_hit_cap_points)
            })
            || rule.balance_cap == 0
            || rule
                .kill_points
                .iter()
                .any(|(template, points)| !profiles.contains_key(template) || *points > MAX_POINTS)
            || rule.by_result.len() != c.result_bindings().count()
            || c.result_bindings()
                .any(|(_, result)| !rule.by_result.contains_key(&result.result_id))
        {
            return Err("CONFIG_SETTLEMENT_REWARDS_RULE");
        }
        let kill_bound: u64 = rule
            .kill_points
            .iter()
            .map(|(id, &points)| {
                u64::from(points) * u64::from(population.get(id).copied().unwrap_or(0))
            })
            .sum();
        if rule.by_result.values().any(|r| {
            r.bonus_points > MAX_POINTS
                || r.multiplier_bps > MAX_RATE_BPS
                || rule
                    .requested(
                        (if rule.earning_scale.is_some() {
                            kill_bound * u64::from(MAX_EARNING_RATE_BPS / NEUTRAL_RATE_BPS)
                        } else {
                            kill_bound
                        }) + u64::from(r.bonus_points),
                        rule.damage.as_ref().map_or(0, DamageReward::maximum),
                        r.multiplier_bps,
                        c.params.entry_years,
                    )
                    .is_none_or(|original| !rule.valid_adjusted_bound(original))
        }) {
            return Err("CONFIG_SETTLEMENT_REWARDS_BOUNDS");
        }
    }
    Ok(())
}
