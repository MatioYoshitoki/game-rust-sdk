//! Configured instant healing with an optional frozen periodic follow-up.
use super::combat::RecoveryFormula;
use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryHot {
    pub status_id: String,
    pub duration_ms: u64,
    pub tick_bps: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoverySplash {
    pub radius: f64,
    pub heal_bps: u32,
    pub hot_bps: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecoveryMarkOwner {
    Caster,
    PrimaryTarget,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryMark {
    pub status_id: String,
    pub owner: RecoveryMarkOwner,
    pub stacks: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub at_cap_heal: Option<RecoveryFormula>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AllyRecovery {
    pub minimum_mastery: usize,
    pub heal: RecoveryFormula,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub hot: Option<RecoveryHot>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub splash: Option<RecoverySplash>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub overheal_shield: Option<OverhealShield>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub mark: Option<RecoveryMark>,
}

fn valid_heal(heal: &RecoveryFormula) -> bool {
    heal.fixed <= 1_000_000
        && heal.source_attack_bps <= 100000
        && heal.target_max_hp_bps <= 10000
        && heal.amount(10000, 10000) > 0
}

impl Config {
    pub(crate) fn allows_ally_self_target(&self, skill: &str) -> bool {
        self.combat_rules
            .as_ref()
            .is_some_and(|r| r.ally_self_target_skills.iter().any(|id| id == skill))
    }
}

pub(super) fn validate(c: &Config, r: &CombatRules) -> Result<(), &'static str> {
    let self_targets = &r.ally_self_target_skills;
    if !self_targets.is_empty()
        && (c.protocol_version() != 6
            || self_targets.len() > super::MAX_SKILL_DEFINITIONS
            || self_targets.iter().enumerate().any(|(i, id)| {
                self_targets[..i].contains(id)
                    || c.player_skill(id)
                        .is_none_or(|s| !s.is_ally_heal() || s.channel.is_some())
            }))
    {
        return Err("CONFIG_ALLY_SELF_TARGET");
    }
    let table = &r.ally_recovery_by_mastery;
    if table.is_empty() {
        return Ok(());
    }
    let p = c.pve.as_ref().ok_or("CONFIG_COMBAT_PVE")?;
    let mark_valid = |mark: &RecoveryMark| {
        mark.at_cap_heal.as_ref().is_none_or(valid_heal)
            && !r.status_ends.contains_key(&mark.status_id)
            && r.status_policies
                .get(&mark.status_id)
                .is_some_and(|p| p.disposition == StatusDisposition::Beneficial && !p.transferable)
            && p.status_definitions.iter().any(|d| {
                d.id == mark.status_id
                    && d.source_scoped
                    && matches!(d.mechanism, StatusMechanism::TimedAttributeModifier)
                    && matches!(d.duplicate_policy, DuplicatePolicy::Stack)
                    && !d.is_harmful()
                    && d.max_stacks
                        .is_some_and(|cap| (1..=cap).contains(&mark.stacks))
            })
    };
    let hot_valid = |hot: &RecoveryHot| {
        (1..=10000).contains(&hot.tick_bps)
            && !r.healing.contains_key(&hot.status_id)
            && !r.periodic_sampling.contains_key(&hot.status_id)
            && !r.periodic_recovery.contains_key(&hot.status_id)
            && p.status_definitions.iter().any(|d| {
                d.id == hot.status_id
                    && d.is_heal()
                    && d.source_scoped
                    && d.periodic_stack.is_none()
                    && d.overheal_shield.is_none()
                    && matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    && d.tick_interval_ms.is_some_and(|interval| {
                        interval > 0
                            && interval >= c.step_ms
                            && (interval..=60000).contains(&hot.duration_ms)
                            && hot.duration_ms.is_multiple_of(interval)
                            && hot.duration_ms / interval <= 128
                    })
            })
    };
    if c.protocol_version() != 6
        || table.len() > super::MAX_SKILL_DEFINITIONS
        || table.iter().any(|(id, rows)| {
            !c.player_skill(id).is_some_and(|s| {
                s.is_ally_heal()
                    && s.channel.is_none()
                    && s.instant_heal.is_none()
                    && s.ally_status.is_none()
                    && s.on_commit_self_statuses.is_empty()
            }) || !(1..=16).contains(&rows.len())
                || rows[0].minimum_mastery != 0
                || rows
                    .windows(2)
                    .any(|pair| pair[0].minimum_mastery >= pair[1].minimum_mastery)
                || rows.iter().any(|row| {
                    row.minimum_mastery > 0
                        && c.build_rules().is_none_or(|build| {
                            !build
                                .power_models
                                .values()
                                .any(|model| model.mastery_add_bps.len() > row.minimum_mastery)
                        })
                        || !valid_heal(&row.heal)
                        || row.mark.as_ref().is_some_and(|mark| !mark_valid(mark))
                        || row.hot.as_ref().is_some_and(|hot| !hot_valid(hot))
                        || row.splash.as_ref().is_some_and(|area| {
                            !(0.1..=512.).contains(&area.radius)
                                || !(1..=10000).contains(&area.heal_bps)
                                || area.hot_bps > 10000
                                || row.hot.is_none() && area.hot_bps != 0
                        })
                        || row.overheal_shield.as_ref().is_some_and(|conversion| {
                            p.overheal_shield_definition(conversion)
                                .is_none_or(|d| !d.source_scoped)
                        })
                })
        })
    {
        return Err("CONFIG_ALLY_RECOVERY_MASTERY");
    }
    Ok(())
}
