//! A finite set of native combat facts and effects, not an expression language.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CombatFact {
    DamageDealt,
    DamageTaken,
    ShieldAbsorbed,
    Kill,
    SkillCommitted,
    UnitSkillCommitted,
    SkillCostCommitted,
    AllySkillCommitted,
    DirectHit,
    BasicHit,
    ContactStatusConsumed,
    FinisherKill,
    StatusStacksReached,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum TriggerEffect {
    RestorePaidSpirit {
        bps: u32,
    },
    RefreshStatusDuration {
        subject: TriggerSubject,
        status_id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        duration_ms: Option<u64>,
    },
    ApplyStatusBundle {
        owner_statuses: Vec<String>,
        other_statuses: Vec<String>,
    },
    AddStatusStacks {
        subject: TriggerSubject,
        status_id: String,
        amount: u32,
        #[serde(default, skip_serializing_if = "super::stack_counters::is_false")]
        refresh_duration: bool,
    },
    StealOther {
        steal: super::ResourceSteal,
    },
    FinishOther {
        rule: super::FinishRule,
    },
    ReduceSkillCooldowns {
        skill_ids: Vec<String>,
        remaining_bps: u32,
        fixed_ms: u64,
    },
    ChainDamage {
        rule: super::ChainDamageRule,
    },
    ApplyOtherStatus {
        status_id: String,
    },
    DispelStatuses {
        subject: TriggerSubject,
        disposition: super::StatusDisposition,
        max_count: usize,
    },
    TransferStatuses {
        from: TriggerSubject,
        disposition: super::StatusDisposition,
        max_count: usize,
    },
    HealFromDamage {
        bps: u32,
    },
    RestoreSpirit {
        amount: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        max_spirit_bps: u32,
    },
    ApplySelfStatus {
        status_id: String,
    },
    ReflectDamage {
        bps: u32,
        damage_kind: DamageKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        element: Option<String>,
    },
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TriggerCounting {
    #[default]
    Contact,
    Action,
    ActionTotal,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TriggerScope {
    #[default]
    Owner,
    Target,
}
impl TriggerScope {
    fn owner(&self) -> bool {
        *self == Self::Owner
    }
}
impl TriggerCounting {
    fn contact(&self) -> bool {
        *self == Self::Contact
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TriggerSubject {
    Owner,
    Other,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusRequirement {
    pub subject: TriggerSubject,
    pub definition_id: String,
    pub at_least: u32,
    pub consume: u32,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TriggerConditions {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub minimum_amount: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub owner_hp_at_most_bps: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub other_hp_at_most_bps: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub status: Option<StatusRequirement>,
}
fn is_zero(n: &u32) -> bool {
    *n == 0
}
impl TriggerConditions {
    fn empty(&self) -> bool {
        self.minimum_amount == 0
            && self.owner_hp_at_most_bps.is_none()
            && self.other_hp_at_most_bps.is_none()
            && self.status.is_none()
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoonTrigger {
    pub id: String,
    #[serde(default, skip_serializing_if = "super::stack_counters::is_false")]
    pub npc_only: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub higher_level_effects: Vec<TriggerEffect>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CombatTrigger {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub requires_boon: Option<BoonTrigger>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub stack_change: Option<super::stack_counters::StackThreshold>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub action_filter: Option<super::ActionFilter>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub owner_skill: Option<super::SkillRequirement>,
    #[serde(
        default = "super::source_gates::one",
        skip_serializing_if = "super::source_gates::is_one"
    )]
    pub maximum_per_action: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub finisher: Option<super::FinisherSelector>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub contact_consumption: Option<super::post_consumption::ContactConsumption>,
    #[serde(default, skip_serializing_if = "TriggerScope::owner")]
    pub counter_scope: TriggerScope,
    #[serde(default, skip_serializing_if = "TriggerConditions::empty")]
    pub conditions: TriggerConditions,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub chance_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "TriggerCounting::contact")]
    pub counting: TriggerCounting,
    pub id: String,
    pub profiles: Vec<String>,
    pub when: CombatFact,
    pub include_periodic: bool,
    pub every: u32,
    pub cooldown_ms: u64,
    pub priority: u16,
    pub effect: TriggerEffect,
}

fn valid_boon_trigger(c: &Config, t: &CombatTrigger, gate: &BoonTrigger) -> bool {
    let Some(definition) = c
        .world_boons
        .as_ref()
        .and_then(|boons| boons.definitions.get(&gate.id))
    else {
        return false;
    };
    matches!(
        t.when,
        CombatFact::DirectHit
            | CombatFact::BasicHit
            | CombatFact::DamageDealt
            | CombatFact::DamageTaken
            | CombatFact::ShieldAbsorbed
            | CombatFact::Kill
    ) && !t.include_periodic
        && t.counting != TriggerCounting::ActionTotal
        && matches!(
            t.effect,
            TriggerEffect::ApplyOtherStatus { .. }
                | TriggerEffect::ApplySelfStatus { .. }
                | TriggerEffect::HealFromDamage { .. }
                | TriggerEffect::RestoreSpirit { .. }
        )
        && (gate.higher_level_effects.is_empty()
            || gate.higher_level_effects.len() + 1 == definition.levels.len()
                && gate.higher_level_effects.iter().all(|effect| {
                    std::mem::discriminant(effect) == std::mem::discriminant(&t.effect)
                }))
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.triggers.len() > 32
        || !(1..=128).contains(&r.max_triggers_per_step)
        || !(1..=128).contains(&r.max_trigger_targets_per_rule)
        || !(1..=1024).contains(&r.max_pending_action_totals)
    {
        return Err("CONFIG_COMBAT_TRIGGER_BUDGET");
    }
    let mut ids = std::collections::BTreeSet::new();
    for t in &r.triggers {
        if t.id.is_empty()
            || t.id.len() > 64
            || t.id.chars().any(char::is_control)
            || !ids.insert(&t.id)
            || t.profiles.is_empty()
            || t.profiles.len() > 64
            || t.profiles.iter().any(|p| !r.profiles.contains_key(p))
            || !(1..=1000000).contains(&t.every)
            || t.cooldown_ms > 3600000
            || t.counting != TriggerCounting::Contact && t.include_periodic
        {
            return Err("CONFIG_COMBAT_TRIGGER_POLICY");
        }
        if t.requires_boon
            .as_ref()
            .is_some_and(|gate| !valid_boon_trigger(c, t, gate))
        {
            return Err("CONFIG_BOON_TRIGGER_BINDING");
        }
        if t.when == CombatFact::SkillCostCommitted
            && (t.include_periodic
                || !matches!(
                    t.counting,
                    TriggerCounting::Contact | TriggerCounting::Action
                )
                || t.counter_scope != TriggerScope::Owner
                || t.conditions.other_hp_at_most_bps.is_some()
                || t.conditions
                    .status
                    .as_ref()
                    .is_some_and(|s| matches!(s.subject, TriggerSubject::Other))
                || !matches!(t.effect, TriggerEffect::RestorePaidSpirit { .. }))
            || matches!(t.effect, TriggerEffect::RestorePaidSpirit {bps} if !(1..=10000).contains(&bps) || t.when != CombatFact::SkillCostCommitted)
        {
            return Err("CONFIG_PAID_SPIRIT_TRIGGER");
        }
        if t.counting == TriggerCounting::ActionTotal {
            let allowed = matches!(
                t.effect,
                TriggerEffect::HealFromDamage { .. }
                    | TriggerEffect::RestoreSpirit { .. }
                    | TriggerEffect::ApplySelfStatus { .. }
            ) || t.when == CombatFact::DamageDealt
                && t.counter_scope == TriggerScope::Owner
                && matches!(
                    t.effect,
                    TriggerEffect::AddStatusStacks {
                        subject: TriggerSubject::Owner,
                        ..
                    }
                );
            if !allowed
                || !matches!(
                    t.when,
                    CombatFact::DamageDealt | CombatFact::DamageTaken | CombatFact::ShieldAbsorbed
                )
                || t.conditions.other_hp_at_most_bps.is_some()
                || t.conditions
                    .status
                    .as_ref()
                    .is_some_and(|s| matches!(s.subject, TriggerSubject::Other))
            {
                return Err("CONFIG_ACTION_TOTAL_POLICY");
            }
        }
        if t.chance_bps.is_some_and(|n| n > 10000)
            || t.conditions.owner_hp_at_most_bps.is_some_and(|n| n > 10000)
            || t.conditions.other_hp_at_most_bps.is_some_and(|n| n > 10000)
        {
            return Err("CONFIG_TRIGGER_CONDITION_BOUNDS");
        }
        if let Some(requirement) = &t.conditions.status
            && !c.pve.as_ref().unwrap().status_definitions.iter().any(|d| {
                d.id == requirement.definition_id
                    && (1..=d.maximum_stacks()).contains(&requirement.at_least)
                    && requirement.consume <= requirement.at_least
            })
        {
            return Err("CONFIG_TRIGGER_STATUS_REQUIREMENT");
        }
        if let TriggerEffect::StealOther { steal } = &t.effect
            && (!steal.valid(c)
                || !matches!(t.when, CombatFact::DirectHit | CombatFact::BasicHit)
                || t.include_periodic)
        {
            return Err("CONFIG_RESOURCE_STEAL_TRIGGER");
        }
        let higher_effects = t
            .requires_boon
            .iter()
            .flat_map(|gate| &gate.higher_level_effects);
        for effect in std::iter::once(&t.effect).chain(higher_effects) {
            match effect {
                TriggerEffect::ApplyOtherStatus { status_id } => {
                    let d = c
                        .pve
                        .as_ref()
                        .unwrap()
                        .status_definitions
                        .iter()
                        .find(|d| d.id == *status_id);
                    if !matches!(
                        t.when,
                        CombatFact::DirectHit
                            | CombatFact::DamageDealt
                            | CombatFact::DamageTaken
                            | CombatFact::ShieldAbsorbed
                            | CombatFact::UnitSkillCommitted
                    ) || d.is_none_or(|d| {
                        !d.source_scoped
                            || !d.is_harmful()
                            || d.attribute.is_some_and(|a| a.is_immunity_flag())
                            || !matches!(
                                d.mechanism,
                                StatusMechanism::TimedAttributeModifier
                                    | StatusMechanism::TimedCompoundModifier
                                    | StatusMechanism::TimedControl
                                    | StatusMechanism::PeriodicDamage
                            )
                            || d.flags.iter().any(|a| !a.is_control_flag())
                            || r.status_windows.contains_key(status_id)
                            || r.status_cleanses.contains_key(status_id)
                    }) || r
                        .status_policies
                        .get(status_id)
                        .is_none_or(|p| p.disposition != super::StatusDisposition::Harmful)
                    {
                        return Err("CONFIG_TRIGGER_TARGET_STATUS");
                    }
                }
                TriggerEffect::DispelStatuses { max_count, .. }
                | TriggerEffect::TransferStatuses { max_count, .. }
                    if !(1..=16).contains(max_count) =>
                {
                    return Err("CONFIG_STATUS_OPERATION_BUDGET");
                }
                TriggerEffect::HealFromDamage { bps }
                | TriggerEffect::ReflectDamage { bps, .. }
                    if *bps == 0 || *bps > 10000 =>
                {
                    return Err("CONFIG_COMBAT_TRIGGER_AMOUNT");
                }
                TriggerEffect::RestoreSpirit {
                    amount,
                    max_spirit_bps,
                } if *amount > 1000000
                    || *max_spirit_bps > 10000
                    || *amount == 0 && *max_spirit_bps == 0 =>
                {
                    return Err("CONFIG_COMBAT_TRIGGER_AMOUNT");
                }
                TriggerEffect::ReflectDamage {
                    damage_kind,
                    element,
                    ..
                } if element.as_ref().is_some_and(|id| !r.elements.contains(id))
                    || *damage_kind == DamageKind::Elemental && element.is_none()
                    || *damage_kind == DamageKind::TrueDamage && element.is_some() =>
                {
                    return Err("CONFIG_COMBAT_TRIGGER_ELEMENT");
                }
                TriggerEffect::ApplySelfStatus { status_id }
                    if !c.pve.as_ref().unwrap().status_definitions.iter().any(|d| {
                        d.id == *status_id
                            && !d.is_hostile_control()
                            && (!d.is_periodic() || d.is_heal())
                    }) =>
                {
                    return Err("CONFIG_COMBAT_TRIGGER_STATUS");
                }
                _ => {}
            }
        }
        if matches!(
            t.when,
            CombatFact::AllySkillCommitted | CombatFact::DirectHit | CombatFact::BasicHit
        ) && t.include_periodic
        {
            return Err("CONFIG_COMBAT_TRIGGER_FACT");
        }
        if matches!(
            t.when,
            CombatFact::SkillCommitted | CombatFact::AllySkillCommitted
        ) && matches!(
            t.effect,
            TriggerEffect::HealFromDamage { .. } | TriggerEffect::ReflectDamage { .. }
        ) || matches!(t.effect, TriggerEffect::ReflectDamage { .. })
            && !matches!(t.when, CombatFact::DamageTaken | CombatFact::ShieldAbsorbed)
        {
            return Err("CONFIG_COMBAT_TRIGGER_FACT");
        }
    }
    Ok(())
}
pub(super) fn default_trigger_budget() -> usize {
    32
}
pub(super) fn default_target_budget() -> usize {
    32
}
pub(super) fn is_default_target_budget(n: &usize) -> bool {
    *n == default_target_budget()
}
pub(super) fn default_total_budget() -> usize {
    256
}
pub(super) fn is_default_total_budget(n: &usize) -> bool {
    *n == default_total_budget()
}
