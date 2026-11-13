//! Frozen, game-owned numeric tables. No map, skill-name or elemental-cycle branches.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DamageKind {
    #[default]
    Physical,
    Elemental,
    TrueDamage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LifeStealTiming {
    #[default]
    Contact,
    ActionEnd,
}
impl LifeStealTiming {
    fn contact(&self) -> bool {
        *self == Self::Contact
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum Multiplier {
    BasisPoints(u32),
    Exact(ExactMultiplier),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExactMultiplier {
    pub numerator: u32,
    pub denominator: u32,
}
impl Multiplier {
    fn valid(&self) -> bool {
        match self {
            Self::BasisPoints(n) => *n <= 1000000,
            Self::Exact(r) => {
                r.numerator <= 1000000
                    && (1..=1000000).contains(&r.denominator)
                    && u64::from(r.numerator) <= u64::from(r.denominator) * 100
            }
        }
    }
    pub(crate) fn factor(&self) -> f64 {
        match self {
            Self::BasisPoints(n) => f64::from(*n) / 10000.,
            Self::Exact(r) => f64::from(r.numerator) / f64::from(r.denominator),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CombatProfile {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub build: Option<crate::protocol::CombatBuild>,
    pub tier: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_resistance: Option<u32>,
    pub defense_baseline_attack: u32,
    pub hit_bps: u32,
    pub critical_chance_bps: u32,
    pub dodge_bps: u32,
    pub critical_multiplier_bps: u32,
    pub power_points_bps: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttackRule {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub contact_status: Option<super::ContactStatusRule>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub armor_penetration_cap_bps: Option<u32>,
    pub damage_kind: DamageKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon_element: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_power: Option<u32>,
    pub fixed: u32,
    pub cultivation_bps: u32,
    pub armor_penetration_bps: u32,
    pub life_steal_bps: u32,
    #[serde(default, skip_serializing_if = "LifeStealTiming::contact")]
    pub life_steal_timing: LifeStealTiming,
    pub unavoidable: bool,
    pub ignore_armor: bool,
    pub can_crit: bool,
}
impl Default for AttackRule {
    fn default() -> Self {
        Self {
            contact_status: None,
            armor_penetration_cap_bps: None,
            damage_kind: DamageKind::Physical,
            element: None,
            weapon_element: None,
            base_power: None,
            fixed: 0,
            cultivation_bps: 10000,
            armor_penetration_bps: 0,
            life_steal_bps: 0,
            life_steal_timing: LifeStealTiming::Contact,
            unavoidable: false,
            ignore_armor: false,
            can_crit: true,
        }
    }
}
impl AttackRule {
    pub(crate) fn budget(
        &self,
        attack: u32,
        damage_bps: u32,
        bonus_bps: u32,
        points_bps: u32,
    ) -> u32 {
        self.budget_layers(attack, damage_bps, bonus_bps, points_bps, 1)
    }
    pub(crate) fn budget_layers(
        &self,
        attack: u32,
        damage_bps: u32,
        bonus_bps: u32,
        points_bps: u32,
        layers: u32,
    ) -> u32 {
        let power = u128::from(self.base_power.unwrap_or(attack))
            * u128::from(bonus_bps)
            * u128::from(10000 + points_bps)
            / 100_000_000;
        let base = u128::from(self.fixed) * u128::from(layers)
            + power * u128::from(damage_bps) * u128::from(layers) / 10000;
        (base * u128::from(self.cultivation_bps) / 10000).min(u128::from(u32::MAX)) as u32
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryFormula {
    pub fixed: u32,
    pub source_attack_bps: u32,
    pub target_max_hp_bps: u32,
}
impl RecoveryFormula {
    pub(crate) fn amount(&self, attack: u32, max_hp: u32) -> u32 {
        self.amount_layers(attack, max_hp, 1)
    }
    pub(crate) fn amount_layers(&self, attack: u32, max_hp: u32, layers: u32) -> u32 {
        (u128::from(self.fixed) * u128::from(layers)
            + (u128::from(attack) * u128::from(self.source_attack_bps)
                + u128::from(max_hp) * u128::from(self.target_max_hp_bps))
                * u128::from(layers)
                / 10000)
            .min(u128::from(u32::MAX)) as u32
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GroundTargetPolicy {
    Strict,
    ClampToRange,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CombatRules {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub member_summons: Option<super::member_summons::MemberSummons>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub member_cast_anchors: BTreeMap<String, super::member_cast_anchor::MemberCastAnchor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub member_ground_traps: BTreeMap<String, super::member_ground_trap::MemberGroundTrap>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub npc_enter_areas: BTreeMap<String, super::moving_enter_area::NpcEnterArea>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub npc_ground_fields: BTreeMap<String, super::npc_ground_field::NpcGroundField>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub npc_ground_strikes: BTreeMap<String, super::npc_ground_field::NpcGroundStrike>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub weapon_catalog: Option<super::weapon_equipment::WeaponCatalog>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub basic_profiles: BTreeMap<String, BasicConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub self_statuses_by_mastery: BTreeMap<String, Vec<super::source_gates::MasteryStatuses>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub skill_cooldowns_by_mastery: BTreeMap<String, Vec<super::source_gates::MasteryCooldown>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ally_recovery_by_mastery: BTreeMap<String, Vec<super::ally_recovery::AllyRecovery>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ally_self_target_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unit_hit_statuses_by_mastery: BTreeMap<String, Vec<super::source_gates::MasteryStatuses>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_damage_taken_bps: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_retaliations: BTreeMap<String, super::status_retaliation::RetaliationRule>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cast_interceptions: BTreeMap<String, super::cast_interception::CastInterception>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_ends: BTreeMap<String, super::status_ends::StatusEndRule>,
    #[serde(
        default = "super::status_ends::default_budget",
        skip_serializing_if = "super::status_ends::default_value"
    )]
    pub max_status_end_releases_per_step: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_power_sources: BTreeMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub build_rules: Option<super::BuildRules>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub contact_reactions: BTreeMap<String, super::ContactReactionRule>,
    #[serde(
        default = "super::contact_reactions::hit_default",
        skip_serializing_if = "super::contact_reactions::hit_is_default"
    )]
    pub max_contact_reactions_per_hit: usize,
    #[serde(
        default = "super::contact_reactions::step_default",
        skip_serializing_if = "super::contact_reactions::step_is_default"
    )]
    pub max_contact_reactions_per_step: usize,
    #[serde(
        default = "super::chain_damage::default_budget",
        skip_serializing_if = "super::chain_damage::default_value"
    )]
    pub max_chain_hits_per_step: usize,
    #[serde(
        default = "super::contact_status::default_snapshots",
        skip_serializing_if = "super::contact_status::default_snapshot_count"
    )]
    pub max_contact_action_snapshots: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub control_policies: BTreeMap<String, super::ControlPolicy>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub periodic_recovery: BTreeMap<String, super::PeriodicRecoveryRule>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_cleanses: BTreeMap<String, super::StatusCleanseRule>,
    #[serde(
        default = "super::status_cleanses::default_budget",
        skip_serializing_if = "super::status_cleanses::is_default_budget"
    )]
    pub max_cleanse_operations_per_step: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_windows: BTreeMap<String, super::StatusWindowRule>,
    #[serde(
        default = "super::status_windows::default_release_budget",
        skip_serializing_if = "super::status_windows::is_default_release_budget"
    )]
    pub max_window_releases_per_step: usize,
    #[serde(
        default = "super::triggers::default_target_budget",
        skip_serializing_if = "super::triggers::is_default_target_budget"
    )]
    pub max_trigger_targets_per_rule: usize,
    #[serde(
        default = "super::triggers::default_total_budget",
        skip_serializing_if = "super::triggers::is_default_total_budget"
    )]
    pub max_pending_action_totals: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_policies: BTreeMap<String, super::StatusPolicy>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub death_guards: BTreeMap<String, super::DeathGuard>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_death_guards: BTreeMap<String, super::status_operations::StatusDeathGuard>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shield_records: BTreeMap<String, super::ShieldRecordRule>,
    #[serde(default = "super::shield_records::default_release_budget")]
    pub max_shield_releases_per_step: usize,
    #[serde(default = "super::shield_records::default_hit_budget")]
    pub max_shield_counter_hits_per_step: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub periodic_sampling: BTreeMap<String, crate::protocol::PeriodicSampling>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub dodge_cap_bps: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub critical_multiplier_cap_bps: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub life_steal_cap_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ground_target_policies: BTreeMap<String, GroundTargetPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<super::CombatTrigger>,
    #[serde(default = "super::triggers::default_trigger_budget")]
    pub max_triggers_per_step: usize,
    #[serde(default)]
    pub healing: BTreeMap<String, RecoveryFormula>,
    pub profiles: BTreeMap<String, CombatProfile>,
    pub default_member_profile: String,
    pub default_npc_profile: String,
    pub member_profiles: BTreeMap<String, String>,
    pub npc_profiles: BTreeMap<String, String>,
    pub elements: Vec<String>,
    pub tier_multipliers: BTreeMap<i16, Multiplier>,
    pub element_multipliers_bps: BTreeMap<String, BTreeMap<String, u32>>,
    pub weapon_skill_multipliers_bps: BTreeMap<String, BTreeMap<String, u32>>,
    pub basic_attack: AttackRule,
    pub skills: BTreeMap<String, AttackRule>,
    pub npc_attacks: BTreeMap<String, AttackRule>,
    pub periodic_damage: BTreeMap<String, AttackRule>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub delayed_damage: BTreeMap<String, super::delayed_damage::DelayedDamageRule>,
}
impl CombatRules {
    pub(crate) fn member(&self, id: &str) -> &CombatProfile {
        &self.profiles[self
            .member_profiles
            .get(id)
            .unwrap_or(&self.default_member_profile)]
    }
    pub(crate) fn npc(&self, template: &str) -> &CombatProfile {
        &self.profiles[self
            .npc_profiles
            .get(template)
            .unwrap_or(&self.default_npc_profile)]
    }
    pub(crate) fn element_factor(
        &self,
        attack: Option<&str>,
        defense: Option<&str>,
        weapon: bool,
    ) -> u32 {
        let table = if weapon {
            &self.weapon_skill_multipliers_bps
        } else {
            &self.element_multipliers_bps
        };
        attack
            .zip(defense)
            .and_then(|(a, b)| table.get(a)?.get(b))
            .copied()
            .unwrap_or(10000)
    }
}

pub(super) fn validate_fields(v: &serde_json::Value) -> Result<(), &'static str> {
    let Some(table) = v
        .get("combat_rules")
        .and_then(|r| r.get("tier_multipliers"))
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(());
    };
    if table.keys().any(|key| {
        key.parse::<i16>()
            .map_or(true, |value| value.to_string() != *key)
    }) {
        return Err("CONFIG_COMBAT_TIER_KEY");
    }
    Ok(())
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    super::basic_profiles::validate(c)?;
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if c.schema_version != 7 || c.mechanism_version < 105 || c.member_settings().is_none() {
        return Err("CONFIG_COMBAT_RULES_CAPABILITY");
    }
    super::moving_enter_area::validate(c, r)?;
    super::npc_ground_field::validate(c, r)?;
    super::member_ground_trap::validate(c, r)?;
    super::member_cast_anchor::validate(c, r)?;
    super::member_summons::validate(c, r)?;
    if r.life_steal_cap_bps.is_some_and(|n| n > 1000000)
        || r.dodge_cap_bps.is_some_and(|n| n > 10000)
        || r.critical_multiplier_cap_bps
            .is_some_and(|n| !(10000..=1000000).contains(&n))
    {
        return Err("CONFIG_COMBAT_PROBABILITY_CAPS");
    }
    super::character_build::validate(c)?;
    super::stack_counters::validate_triggers(c)?;
    super::conditional_activation::validate(c)?;
    super::cast_interception::validate(c)?;
    super::source_gates::validate(c)?;
    super::ally_recovery::validate(c, r)?;
    if r.status_damage_taken_bps.len() > 16
        || r.status_damage_taken_bps.iter().any(|(id, bps)| {
            !(1..=10000).contains(bps)
                || r.status_policies
                    .get(id)
                    .is_none_or(|p| p.disposition != super::StatusDisposition::Harmful)
                || !c.pve.as_ref().is_some_and(|p| {
                    p.status_definitions.iter().any(|d| {
                        &d.id == id
                            && matches!(
                                d.mechanism,
                                super::StatusMechanism::TimedAttributeModifier
                                    | super::StatusMechanism::TimedCompoundModifier
                            )
                    })
                })
        })
    {
        return Err("CONFIG_STATUS_DAMAGE_TAKEN");
    }
    super::status_ends::validate(c)?;
    super::status_retaliation::validate(c, r)?;
    super::delayed_damage::validate(c)?;
    let id = |s: &str| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control);
    let element = |s: &Option<String>| s.as_ref().is_none_or(|s| r.elements.contains(s));
    if r.elements.len() > 16
        || r.elements.iter().any(|s| !id(s))
        || r.elements
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != r.elements.len()
        || r.profiles.is_empty()
        || r.profiles.len() > 64
        || !r.profiles.contains_key(&r.default_member_profile)
        || !r.profiles.contains_key(&r.default_npc_profile)
        || r.member_profiles.len() > 8
        || r.npc_profiles.len() > 64
        || r.member_profiles
            .iter()
            .chain(&r.npc_profiles)
            .any(|(k, v)| !id(k) || !r.profiles.contains_key(v))
    {
        return Err("CONFIG_COMBAT_PROFILE_REFERENCE");
    }
    if r.profiles.iter().any(|(key, p)| {
        !id(key)
            || p.tier > 31
            || !element(&p.element)
            || p.element_resistance.is_some_and(|n| n > 1_000_000)
            || !(1..=1_000_000).contains(&p.defense_baseline_attack)
            || p.hit_bps > 100_000_000
            || p.critical_chance_bps > 10000
            || p.dodge_bps > 100_000_000
            || !(10000..=r.critical_multiplier_cap_bps.unwrap_or(30000))
                .contains(&p.critical_multiplier_bps)
            || p.power_points_bps > 1_000_000
    }) {
        return Err("CONFIG_COMBAT_PROFILE_BOUNDS");
    }
    if r.tier_multipliers.len() > 63
        || r.tier_multipliers
            .iter()
            .any(|(d, n)| !(-31..=31).contains(d) || !n.valid())
        || r.profiles.values().any(|a| {
            r.profiles.values().any(|b| {
                !r.tier_multipliers
                    .contains_key(&(i16::from(a.tier) - i16::from(b.tier)))
            })
        })
    {
        return Err("CONFIG_COMBAT_TIER_TABLE");
    }
    for table in [&r.element_multipliers_bps, &r.weapon_skill_multipliers_bps] {
        if table.len() > 16
            || table.iter().any(|(a, rows)| {
                !r.elements.contains(a)
                    || rows.len() > 16
                    || rows
                        .iter()
                        .any(|(b, n)| !r.elements.contains(b) || *n > 100000)
            })
        {
            return Err("CONFIG_COMBAT_ELEMENT_TABLE");
        }
    }
    if r.ground_target_policies
        .keys()
        .any(|id| c.player_skill(id).is_none_or(|s| !s.is_ground()))
    {
        return Err("CONFIG_COMBAT_GROUND_POLICY");
    }
    let pve = c.pve.as_ref().ok_or("CONFIG_COMBAT_PVE")?;
    if r.periodic_sampling.len() > 16
        || r.periodic_sampling.keys().any(|id| {
            !pve.status_definitions
                .iter()
                .any(|d| d.id == *id && d.is_periodic())
        })
    {
        return Err("CONFIG_COMBAT_PERIODIC_SAMPLING");
    }
    let npc_ids: std::collections::BTreeSet<_> = pve
        .additional_templates
        .keys()
        .map(String::as_str)
        .chain(std::iter::once(pve.template_id.as_str()))
        .collect();
    if r.npc_profiles
        .keys()
        .chain(r.npc_attacks.keys())
        .any(|s| !npc_ids.contains(s.as_str()))
        || r.skills.keys().any(|s| {
            c.player_skill(s)
                .is_none_or(|s| s.is_self() || s.is_ally_heal() || s.is_ally_shield())
        })
        || r.periodic_damage.keys().any(|s| {
            !pve.status_definitions
                .iter()
                .any(|d| d.id == *s && d.is_periodic() && !d.is_heal())
        })
    {
        return Err("CONFIG_COMBAT_EFFECT_REFERENCE");
    }
    if r.healing.len() > 128
        || r.healing.iter().any(|(id, f)| {
            c.player_skill(id).is_none_or(|s| s.instant_heal.is_none())
                && !pve
                    .status_definitions
                    .iter()
                    .any(|d| d.id == *id && d.is_heal())
                || f.fixed > 1_000_000
                || f.source_attack_bps > 100_000
                || f.target_max_hp_bps > 10000
        })
    {
        return Err("CONFIG_COMBAT_HEALING_FORMULA");
    }
    if r.npc_attacks
        .values()
        .chain(r.periodic_damage.values())
        .any(|rule| rule.life_steal_timing == LifeStealTiming::ActionEnd)
    {
        return Err("CONFIG_COMBAT_LIFE_STEAL_TIMING");
    }
    for rule in std::iter::once(&r.basic_attack)
        .chain(r.skills.values())
        .chain(r.npc_attacks.values())
        .chain(r.periodic_damage.values())
        .chain(
            c.basic_configs()
                .filter_map(|basic| basic.field()?.damage_rule.as_ref()),
        )
        .chain(
            c.basic_configs()
                .filter_map(|basic| Some(&basic.charged_flight()?.damage_rule)),
        )
    {
        if !element(&rule.element)
            || !element(&rule.weapon_element)
            || rule.base_power.is_some_and(|n| n > 1_000_000)
            || rule.fixed > 1_000_000
            || rule.cultivation_bps > 1_000_000
            || rule.armor_penetration_bps > 1_000_000
            || rule.life_steal_bps > 10000
            || rule.damage_kind == DamageKind::Elemental && rule.element.is_none()
            || rule.damage_kind == DamageKind::TrueDamage
                && (rule.can_crit || rule.element.is_some() || rule.weapon_element.is_some())
        {
            return Err("CONFIG_COMBAT_EFFECT_BOUNDS");
        }
    }
    Ok(())
}
