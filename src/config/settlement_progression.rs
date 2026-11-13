use super::*;
use crate::protocol::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SettlementProgression {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) stack_flows: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) partial_entry_charge: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    age_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    storage_caps: BTreeMap<String, u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    instance_capacity: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub(crate) warehouse_slots: Option<WarehouseLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    conversions: Vec<ConversionRule>,
    #[serde(default, skip_serializing_if = "ConversionMode::is_first")]
    conversion_mode: ConversionMode,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    destinations: BTreeMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    threshold_progress: Option<ThresholdProgressRule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ThresholdProgressRule {
    stage_asset_id: String,
    total_asset_id: String,
    progress_asset_id: String,
    thresholds: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WarehouseLayout {
    capacity: u32,
    stack_sizes: BTreeMap<String, u32>,
}

impl WarehouseLayout {
    fn occupied(
        &self,
        facts: &AssetParticipantFacts,
        quantity: impl Fn(&str) -> Option<u32>,
    ) -> Option<u32> {
        self.stack_sizes.iter().try_fold(
            facts
                .warehouse_other_slots?
                .checked_add(facts.instance_count?)?,
            |used, (id, &size)| used.checked_add(quantity(id)?.div_ceil(size)),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConversionRule {
    id: String,
    when: Threshold,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    and: Vec<Threshold>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    result_ids: BTreeSet<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    clock_cost: Option<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    changes: BTreeMap<String, Conversion>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ConversionMode {
    #[default]
    First,
    Ordered,
}

impl ConversionMode {
    fn is_first(&self) -> bool {
        *self == Self::First
    }
}

impl SettlementProgression {
    fn terminal_clock_limit(&self) -> u64 {
        let costs = self
            .conversions
            .iter()
            .filter_map(|rule| rule.clock_cost)
            .map(u64::from);
        if self.conversion_mode.is_first() {
            costs.max().unwrap_or(0)
        } else {
            costs.sum()
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Threshold {
    AtMost { asset_id: String, quantity: u32 },
    AtLeast { asset_id: String, quantity: u32 },
}
impl Threshold {
    fn asset(&self) -> &str {
        match self {
            Self::AtMost { asset_id, .. } | Self::AtLeast { asset_id, .. } => asset_id,
        }
    }
    fn matches(&self, balance: u32) -> bool {
        match self {
            Self::AtMost { quantity, .. } => balance <= *quantity,
            Self::AtLeast { quantity, .. } => balance >= *quantity,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Conversion {
    Set { quantity: u32 },
    Scale { bps: u32 },
    AddCapped { quantity: u32, cap: u32 },
}
impl Conversion {
    fn apply(&self, before: u32) -> u32 {
        match self {
            Self::Set { quantity } => *quantity,
            Self::Scale { bps } => (u64::from(before) * u64::from(*bps) / 10000) as u32,
            Self::AddCapped { quantity, cap } => before + quantity.min(&cap.saturating_sub(before)),
        }
    }
}

pub(crate) fn balance(facts: &AssetParticipantFacts, id: &str) -> Option<u32> {
    facts
        .assets
        .iter()
        .find(|r| r.asset_id == id)
        .map(|r| r.quantity)
}

impl Config {
    pub(crate) fn allows_partial_entry(&self) -> bool {
        self.settlement_progression
            .as_ref()
            .is_some_and(|rule| rule.partial_entry_charge)
    }

    pub(crate) fn entry_charge(&self, available: u32) -> Result<u32> {
        let standard = self.params.entry_years;
        if available >= standard || self.allows_partial_entry() && available > 0 {
            Ok(standard.min(available))
        } else {
            Err(Error::NoResource)
        }
    }

    pub(crate) fn wire_entry_charge(&self, partial: Option<u32>) -> Option<u32> {
        if self.allows_partial_entry() {
            partial.filter(|cost| (1..=self.params.entry_years).contains(cost))
        } else {
            partial.is_none().then_some(self.params.entry_years)
        }
    }

    pub(crate) fn valid_conversion_shape(&self, plan: &ProgressionSettlement) -> bool {
        let valid_step = |step: &GrowthConversion| (1..=8).contains(&step.changes.len());
        let Some(rule) = &self.settlement_progression else {
            return false;
        };
        if rule.conversion_mode.is_first() {
            plan.conversion_steps.is_none() && plan.conversion.as_deref().is_none_or(valid_step)
        } else {
            plan.conversion.is_none()
                && plan.conversion_steps.as_ref().is_some_and(|steps| {
                    steps.len() <= rule.conversions.len() && steps.iter().all(valid_step)
                })
        }
    }

    pub(crate) fn progression_entry_clock(
        &self,
        facts: &AssetParticipantFacts,
    ) -> Result<Option<EntryClock>> {
        let Some(rule) = &self.settlement_progression else {
            return Ok(None);
        };
        if facts.revision.checked_add(2).is_none()
            || rule.threshold_progress.as_ref().is_some_and(|stage| {
                balance(facts, &stage.stage_asset_id)
                    .is_none_or(|index| index as usize >= stage.thresholds.len())
            })
            || rule.instance_capacity.is_some() && facts.instance_count.is_none()
            || rule.warehouse_slots.as_ref().is_some_and(|layout| {
                layout
                    .occupied(facts, |id| Some(balance(facts, id).unwrap_or(0)))
                    .is_none()
            })
            || !rule.destinations.is_empty()
                && facts.location.as_ref().is_none_or(|location| {
                    !short_id(&location.region_id) || location.revision == u32::MAX
                })
        {
            return Err(Error::InvalidMessage);
        }
        let terminal_limit = rule.terminal_clock_limit();
        let Some(age) = rule
            .age_asset_id
            .as_ref()
            .filter(|_| self.params.entry_years > 0 || terminal_limit > 0)
        else {
            return Ok(None);
        };
        let entry = &self.asset_contract.as_ref().unwrap().entry_cost_asset_id;
        let elapsed = balance(facts, age).ok_or(Error::InvalidMessage)?;
        let remaining = balance(facts, entry).unwrap_or(0);
        let cost = self.entry_charge(remaining)?;
        let remaining_after = remaining - cost;
        let elapsed_after = elapsed.checked_add(cost).ok_or(Error::NoResource)?;
        let terminal_max = terminal_limit.min(u64::from(remaining_after));
        if u64::from(elapsed_after) + terminal_max > u64::from(u32::MAX) {
            return Err(Error::NoResource);
        }
        Ok((cost > 0).then(|| EntryClock {
            elapsed: BalanceChange {
                asset_id: age.clone(),
                before: elapsed,
                after: elapsed_after,
            },
            remaining: BalanceChange {
                asset_id: entry.clone(),
                before: remaining,
                after: remaining_after,
            },
        }))
    }

    pub(crate) fn progression_draft(
        &self,
        damage: &BTreeMap<String, u64>,
    ) -> Option<ProgressionSettlement> {
        self.settlement_progression.as_ref()?;
        Some(ProgressionSettlement {
            damage: self
                .settlement_rewards
                .iter()
                .filter(|(_, r)| r.damage.is_some())
                .map(|(id, _)| DamageGrowth {
                    asset_id: id.clone(),
                    points_bps: damage.get(id).copied().unwrap_or(0),
                })
                .collect(),
            ..Default::default()
        })
    }

    pub(crate) fn plan_progression(
        &self,
        intent: &Business,
        facts: &AssetParticipantFacts,
    ) -> Option<ProgressionSettlement> {
        let rule = self.settlement_progression.as_ref()?;
        let Business::CommitSettlement {
            outcome,
            looted,
            kept,
            items,
            instances,
            progression,
            rewards,
            ..
        } = intent
        else {
            return None;
        };
        let draft = progression.as_ref()?;
        if !self.valid_stack_flows(*outcome, *looted, items, draft.stack_flows.as_deref()) {
            return None;
        }
        let rates = self.loaded_earning_rates(facts)?;
        match &draft.reward_earnings {
            Some(earnings) if !rates.is_empty() && earnings.rates == rates => {}
            None if rates.is_empty() => {}
            _ => return None,
        }
        let damage = draft.damage.clone();
        let entry = &self.asset_contract.as_ref()?.entry_cost_asset_id;
        let clock = self.progression_entry_clock(facts).ok()?;
        let charge = self.entry_charge(balance(facts, entry).unwrap_or(0)).ok()?;
        let entry_charged = rule.partial_entry_charge.then_some(charge);
        if draft.entry_charged != entry_charged {
            return None;
        }
        let after_admission = |id: &str| {
            let original = balance(facts, id).unwrap_or(0);
            if id == self.item_id {
                original.checked_sub(self.carried_quantity)
            } else if id == entry {
                original.checked_sub(charge)
            } else if let Some(clock) = &clock
                && id == clock.elapsed.asset_id
            {
                Some(clock.elapsed.after)
            } else {
                Some(original)
            }
        };
        let mut warehouse_slots = if let Some(layout) = &rule.warehouse_slots {
            let before = layout.occupied(facts, after_admission)?;
            Some(WarehouseSlots {
                before,
                after: before,
            })
        } else {
            None
        };
        let mut retained: Vec<_> = std::iter::once((self.item_id.as_str(), *kept))
            .chain(items.iter().map(|i| (i.item_id.as_str(), i.kept)))
            .collect();
        retained.sort_by_key(|&(id, _)| id);
        let mut storage = Vec::new();
        for (id, kept) in retained {
            let cap = rule.storage_caps.get(id).copied();
            if cap.is_none() && warehouse_slots.is_none() {
                continue;
            }
            let before = after_admission(id)?;
            let mut stored = kept.min(cap.unwrap_or(u32::MAX).saturating_sub(before));
            if let Some((layout, slots)) =
                rule.warehouse_slots.as_ref().zip(warehouse_slots.as_mut())
            {
                let size = *layout.stack_sizes.get(id)?;
                let existing = before.div_ceil(size);
                let free = layout.capacity.saturating_sub(slots.after);
                let room =
                    (u64::from(existing) + u64::from(free)) * u64::from(size) - u64::from(before);
                stored = u64::from(stored).min(room) as u32;
                slots.after += (before + stored).div_ceil(size) - existing;
            }
            storage.push(StorageGrowth {
                asset_id: id.into(),
                before,
                after: before + stored,
                stored,
                overflow: kept - stored,
            });
        }
        let instance_storage = if rule.instance_capacity.is_some() || warehouse_slots.is_some() {
            let before = facts.instance_count?;
            let mut stored = instances.len().min(
                rule.instance_capacity
                    .unwrap_or(u32::MAX)
                    .saturating_sub(before) as usize,
            );
            if let Some((layout, slots)) =
                rule.warehouse_slots.as_ref().zip(warehouse_slots.as_mut())
            {
                stored = stored.min(layout.capacity.saturating_sub(slots.after) as usize);
                slots.after += stored as u32;
            }
            let (stored, overflow) = instances.split_at(stored);
            Some(InstanceStorage {
                before,
                after: before + stored.len() as u32,
                stored_ids: stored.iter().map(|i| i.instance_id.clone()).collect(),
                overflow_ids: overflow.iter().map(|i| i.instance_id.clone()).collect(),
            })
        } else {
            None
        };
        let after_rewards = |id: &str| {
            if let Some(reward) = rewards.iter().find(|r| r.asset_id == id) {
                Some(reward.balance_after)
            } else {
                after_admission(id)
            }
        };
        let result = &self.result_binding(*outcome)?.result_id;
        let mut converted = BTreeMap::new();
        let mut steps = Vec::new();
        for row in &rule.conversions {
            let current = |id: &str| converted.get(id).copied().or_else(|| after_rewards(id));
            if !row.result_ids.is_empty() && !row.result_ids.contains(result)
                || !std::iter::once(&row.when).chain(&row.and).all(|condition| {
                    current(condition.asset()).is_some_and(|quantity| condition.matches(quantity))
                })
            {
                continue;
            }
            let mut changes = row
                .changes
                .iter()
                .map(|(id, operation)| {
                    let before = current(id)?;
                    Some(BalanceChange {
                        asset_id: id.clone(),
                        before,
                        after: operation.apply(before),
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            if let Some(limit) = row.clock_cost {
                let age = rule.age_asset_id.as_ref()?;
                let remaining = current(entry)?;
                let elapsed = current(age)?;
                let deducted = remaining.min(limit);
                changes.extend([
                    BalanceChange {
                        asset_id: entry.clone(),
                        before: remaining,
                        after: remaining - deducted,
                    },
                    BalanceChange {
                        asset_id: age.clone(),
                        before: elapsed,
                        after: elapsed.checked_add(deducted)?,
                    },
                ]);
                changes.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
            }
            for change in &changes {
                converted.insert(change.asset_id.clone(), change.after);
            }
            steps.push(GrowthConversion {
                rule_id: row.id.clone(),
                changes,
            });
            if rule.conversion_mode.is_first() {
                break;
            }
        }
        let (conversion, conversion_steps) = if rule.conversion_mode.is_first() {
            (steps.pop().map(Box::new), None)
        } else {
            (None, Some(steps.into_boxed_slice()))
        };
        let threshold_progress = if let Some(rule) = &rule.threshold_progress {
            let stage_index = balance(facts, &rule.stage_asset_id)?;
            let index = stage_index as usize;
            let total = converted
                .get(&rule.total_asset_id)
                .copied()
                .or_else(|| after_rewards(&rule.total_asset_id))?;
            let current_threshold = *rule.thresholds.get(index)?;
            let next_threshold = rule.thresholds.get(index + 1).copied();
            Some(ThresholdProgress {
                stage_index,
                total,
                current_threshold,
                next_threshold,
                balance_met: next_threshold.is_some_and(|threshold| total >= threshold),
                progress: BalanceChange {
                    asset_id: rule.progress_asset_id.clone(),
                    before: balance(facts, &rule.progress_asset_id).unwrap_or(0),
                    after: total.saturating_sub(current_threshold),
                },
            })
        } else {
            None
        };
        let region = if let Some(destination) = rule.destinations.get(result) {
            let before = facts.location.as_ref()?.clone();
            let after = AccountLocation {
                region_id: destination.clone(),
                revision: before.revision.checked_add(1)?,
            };
            Some(RegionTransition { before, after })
        } else {
            None
        };
        Some(ProgressionSettlement {
            reward_weapon_instance_id: draft.reward_weapon_instance_id.clone(),
            reward_earnings: draft.reward_earnings.clone(),
            stack_flows: draft.stack_flows.clone(),
            threshold_progress,
            damage,
            storage,
            instance_storage,
            warehouse_slots,
            conversion,
            conversion_steps,
            entry_charged,
            region,
        })
    }

    pub(crate) fn valid_stack_flows(
        &self,
        outcome: Outcome,
        looted: u32,
        items: &[ItemSettlement],
        flows: Option<&[StackFlow]>,
    ) -> bool {
        let enabled = self
            .settlement_progression
            .as_ref()
            .is_some_and(|rule| rule.stack_flows);
        let Some(flows) = flows else { return !enabled };
        if !enabled || flows.len() > 65 || !flows.windows(2).all(|w| w[0].item_id < w[1].item_id) {
            return false;
        }
        let mut expected: BTreeMap<_, _> = items
            .iter()
            .map(|row| (row.item_id.as_str(), row.looted))
            .collect();
        if looted > 0 {
            expected.insert(self.item_id.as_str(), looted);
        }
        for row in flows {
            let id = row.item_id.as_str();
            let Some(limit) = self.item_limit(id) else {
                return false;
            };
            let capacity = u64::from(self.inventory_capacity) * u64::from(limit);
            let net = u64::from(expected.remove(id).unwrap_or(0));
            if self.instance_definition(id).is_some()
                || row.picked_up == 0
                || row.picked_up > capacity * u64::from(u32::MAX)
                || row.consumed > u64::from(u32::MAX)
                || row.released > 0 && self.member_loot_drop.is_none()
                || row.relic_lost > 0
                    && (outcome != Outcome::Death
                        || row.relic_lost > capacity
                        || self
                            .death_relics()
                            .is_none_or(|r| r.excluded_items.contains(id)))
                || row
                    .consumed
                    .checked_add(row.released)
                    .and_then(|q| q.checked_add(row.relic_lost))
                    .and_then(|q| q.checked_add(net))
                    != Some(row.picked_up)
            {
                return false;
            }
        }
        expected.is_empty()
    }
}

fn short_id(id: &str) -> bool {
    id.len() <= 64 && valid_id(id)
}

pub(super) fn validate(c: &Config) -> std::result::Result<(), &'static str> {
    let Some(rule) = &c.settlement_progression else {
        return Ok(());
    };
    let (Some(asset), Some(pve), Some(_)) = (&c.asset_contract, &c.pve, &c.result_contract) else {
        return Err("CONFIG_PROGRESSION_CAPABILITY");
    };
    if c.protocol_version() != 6 || c.mechanism_version != 105 || c.cultivation_coefficient != 0.0 {
        return Err("CONFIG_PROGRESSION_CAPABILITY");
    }
    if rule.stack_flows && c.members.is_none() {
        return Err("CONFIG_PROGRESSION_CAPABILITY");
    }
    if rule.partial_entry_charge && (rule.age_asset_id.is_none() || c.params.entry_years == 0) {
        return Err("CONFIG_PROGRESSION_ENTRY_CHARGE");
    }
    let stack = |id: &str| id == c.item_id || pve.items.contains_key(id);
    let rate_assets = c.earning_rate_assets();
    let adjustment_assets = c.reward_adjustment_assets().collect::<BTreeSet<_>>();
    let growth_asset = |id: &str| {
        short_id(id)
            && !stack(id)
            && !rate_assets.contains(id)
            && !adjustment_assets.contains(id)
            && (!c.has_equipped_weapon_rewards() || !id.starts_with(SCOPED_REWARD_PREFIX))
    };
    if rule.age_asset_id.as_ref().is_some_and(|id| {
        !growth_asset(id)
            || id == &asset.entry_cost_asset_id
            || c.settlement_rewards.contains_key(id)
    }) || rule
        .storage_caps
        .iter()
        .any(|(id, _)| !short_id(id) || !stack(id) || c.instance_definition(id).is_some())
        || rule.storage_caps.len() > 17
        || rule.destinations.iter().any(|(id, destination)| {
            !short_id(destination) || !c.result_bindings().any(|(_, r)| r.result_id == *id)
        })
    {
        return Err("CONFIG_PROGRESSION_BINDING");
    }
    if let Some(stage) = &rule.threshold_progress {
        let ids = [
            &stage.stage_asset_id,
            &stage.total_asset_id,
            &stage.progress_asset_id,
        ];
        if ids.iter().any(|id| !growth_asset(id))
            || ids.into_iter().collect::<BTreeSet<_>>().len() != ids.len()
            || !(1..=32).contains(&stage.thresholds.len())
            || stage.thresholds.first() != Some(&0)
            || stage.thresholds.windows(2).any(|pair| pair[0] >= pair[1])
            || [&stage.stage_asset_id, &stage.progress_asset_id]
                .into_iter()
                .any(|id| {
                    id == &asset.entry_cost_asset_id
                        || id == &asset.reward_asset_id
                        || rule.age_asset_id.as_ref() == Some(id)
                        || c.settlement_rewards.contains_key(id)
                        || rule
                            .conversions
                            .iter()
                            .any(|conversion| conversion.changes.contains_key(id))
                })
        {
            return Err("CONFIG_PROGRESSION_THRESHOLD");
        }
    }
    if let Some(layout) = &rule.warehouse_slots {
        let expected: BTreeSet<_> = std::iter::once(c.item_id.as_str())
            .chain(pve.items.keys().map(String::as_str))
            .filter(|id| c.instance_definition(id).is_none())
            .collect();
        if !expected
            .into_iter()
            .eq(layout.stack_sizes.keys().map(String::as_str))
            || layout
                .stack_sizes
                .iter()
                .any(|(id, size)| !short_id(id) || !(1..=10000).contains(size))
        {
            return Err("CONFIG_PROGRESSION_WAREHOUSE");
        }
    }
    let mut ids = BTreeSet::new();
    let clock_costs = rule.terminal_clock_limit() > 0;
    if rule.conversions.len() > 8 || clock_costs && rule.age_asset_id.is_none() || rule.conversions.iter().any(|row| {
        !short_id(&row.id) || !ids.insert(&row.id) || !growth_asset(row.when.asset())
            || row.and.len() > 7 || row.and.iter().any(|condition| !growth_asset(condition.asset()))
            || row.result_ids.iter().any(|id| !c.result_bindings().any(|(_, result)| &result.result_id == id))
            || row.clock_cost == Some(0)
            || !(1..=8).contains(&(row.changes.len() + usize::from(row.clock_cost.is_some()) * 2))
            || row.changes.iter().any(|(id, change)| {
                !growth_asset(id)
                    || clock_costs && (id == &asset.entry_cost_asset_id || rule.age_asset_id.as_ref() == Some(id))
                    || matches!(change, Conversion::Scale { bps } if *bps > 10000)
                    || matches!(change, Conversion::AddCapped { quantity, cap } if *quantity == 0 || *cap == 0)
            })
    }) {
        return Err("CONFIG_PROGRESSION_CONVERSION");
    }
    Ok(())
}
