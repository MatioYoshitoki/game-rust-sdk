//! Frozen, typed rules for a single shield instance's bounded observation ledger.
use super::*;
use crate::protocol::{ShieldRecordBasis, StatusRemovalReason};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordRefresh {
    Reset,
    Preserve,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShieldCounter {
    pub ratio_bps: u32,
    pub damage_kind: DamageKind,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub element: Option<String>,
    pub ignore_armor: bool,
    pub radius: f64,
    pub max_targets: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_hit_target_statuses: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShieldRecordRule {
    pub basis: ShieldRecordBasis,
    pub cap_fixed: u32,
    pub cap_power_bps: u32,
    pub damage_reduction_bps: u32,
    pub refresh: RecordRefresh,
    pub on_depleted: bool,
    pub on_expired: bool,
    pub on_consumed: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub counter: Option<ShieldCounter>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub counter_on: Option<Vec<StatusRemovalReason>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub after_end_target_statuses: Vec<String>,
}
impl ShieldRecordRule {
    pub(crate) fn releases_on(&self, reason: StatusRemovalReason) -> bool {
        match reason {
            StatusRemovalReason::Depleted => self.on_depleted,
            StatusRemovalReason::Expired => self.on_expired,
            StatusRemovalReason::Consumed => self.on_consumed,
            _ => false,
        }
    }

    pub(crate) fn counter_for(&self, reason: StatusRemovalReason) -> Option<&ShieldCounter> {
        self.counter.as_ref().filter(|_| {
            self.counter_on
                .as_ref()
                .is_none_or(|reasons| reasons.contains(&reason))
        })
    }

    pub(crate) fn cap(&self, power: u32) -> u32 {
        (u128::from(self.cap_fixed) + u128::from(power) * u128::from(self.cap_power_bps) / 10000)
            .min(u128::from(u32::MAX)) as u32
    }
}
pub(super) fn default_release_budget() -> usize {
    32
}
pub(super) fn default_hit_budget() -> usize {
    128
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    for d in c.pve.iter().flat_map(|p| &p.status_definitions) {
        if let Some(family) = &d.shield_family
            && (c.protocol_version() != 6
                || !d.is_shield()
                || !d.source_scoped
                || !matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                || family.is_empty()
                || family.len() > 64
                || !family
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
                || c.combat_rules.as_ref().is_none_or(|r| {
                    !r.shield_records.contains_key(&d.id)
                        || r.status_policies.get(&d.id).is_none_or(|p| {
                            p.transferable || p.disposition != StatusDisposition::Beneficial
                        })
                }))
        {
            return Err("CONFIG_SHIELD_FAMILY");
        }
    }
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.shield_records.len() > 16
        || !(1..=128).contains(&r.max_shield_releases_per_step)
        || !(1..=128).contains(&r.max_shield_counter_hits_per_step)
    {
        return Err("CONFIG_SHIELD_RECORD_BUDGET");
    }
    let definitions = &c.pve.as_ref().unwrap().status_definitions;
    let refs = |ids: &[String], beneficial: bool| {
        ids.len() <= 2
            && ids.iter().collect::<std::collections::BTreeSet<_>>().len() == ids.len()
            && ids.iter().all(|id| {
                definitions
                    .iter()
                    .any(|d| d.id == *id && super::status_ends::derived_leaf(c, id, beneficial))
            })
    };
    for (id, s) in &r.shield_records {
        if !definitions.iter().any(|d| d.id == *id && d.is_shield())
            || s.cap_fixed > 1000000
            || s.cap_power_bps > 100000
            || s.damage_reduction_bps > 10000
            || !refs(&s.after_end_target_statuses, true)
            || s.counter_on.as_ref().is_some_and(|reasons| {
                s.counter.is_none()
                    || !(1..=3).contains(&reasons.len())
                    || reasons
                        .iter()
                        .enumerate()
                        .any(|(i, reason)| !s.releases_on(*reason) || reasons[..i].contains(reason))
            })
        {
            return Err("CONFIG_SHIELD_RECORD_POLICY");
        }
        if let Some(counter) = &s.counter
            && (counter.ratio_bps > 10000
                || !counter.radius.is_finite()
                || !(0.0..=2000.).contains(&counter.radius)
                || counter.radius == 0.
                || !(1..=32).contains(&counter.max_targets)
                || counter
                    .element
                    .as_ref()
                    .is_some_and(|e| !r.elements.contains(e))
                || counter.damage_kind == DamageKind::Elemental && counter.element.is_none()
                || counter.damage_kind != DamageKind::Elemental && counter.element.is_some()
                || !refs(&counter.on_hit_target_statuses, false))
        {
            return Err("CONFIG_SHIELD_COUNTER_POLICY");
        }
    }
    Ok(())
}
