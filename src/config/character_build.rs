//! Values and growth tables belong to configuration; arithmetic is shared native code.
use super::*;
use crate::protocol::{CombatBuild, PowerGrowth, StatusAttribute};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BaseConversion {
    pub hp_bps: u32,
    pub spirit_bps: u32,
    pub atk_bps: u32,
    pub def_bps: u32,
    pub spd_bps: u32,
    pub element_resistance_bps: u32,
}
impl BaseConversion {
    pub(crate) fn amount(&self, points: u32, a: StatusAttribute) -> u32 {
        let bps = match a {
            StatusAttribute::MaxHp => self.hp_bps,
            StatusAttribute::MaxSpirit => self.spirit_bps,
            StatusAttribute::Atk => self.atk_bps,
            StatusAttribute::Def => self.def_bps,
            StatusAttribute::Spd => self.spd_bps,
            StatusAttribute::ElementResistance => self.element_resistance_bps,
            _ => unreachable!("base conversion axis"),
        };
        (u64::from(points) * u64::from(bps) / 10000).clamp(1, u64::from(u32::MAX)) as u32
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PowerModel {
    pub rank_base: Vec<u32>,
    pub quality_add_bps: Vec<i32>,
    pub level_add_bps: Vec<i32>,
    pub mastery_add_bps: Vec<i32>,
    pub external_bps: u32,
    pub maximum_power: u32,
}
impl PowerModel {
    pub(crate) fn valid_input(&self, p: &PowerGrowth) -> bool {
        p.rank < self.rank_base.len()
            && p.quality < self.quality_add_bps.len()
            && p.level < self.level_add_bps.len()
            && p.mastery < self.mastery_add_bps.len()
    }
    pub(crate) fn amount(&self, p: &PowerGrowth) -> u32 {
        let added = (10000i64
            + i64::from(self.quality_add_bps[p.quality])
            + i64::from(self.level_add_bps[p.level])
            + i64::from(self.mastery_add_bps[p.mastery]))
        .max(0) as u128;
        let grown = u128::from(self.rank_base[p.rank]) * added / 10000;
        (grown * u128::from(self.external_bps) / 10000).min(u128::from(self.maximum_power)) as u32
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildRules {
    pub conversion: BaseConversion,
    pub maximum_input_points: u32,
    pub power_models: BTreeMap<String, PowerModel>,
}
impl Config {
    pub(crate) fn build_rules(&self) -> Option<&BuildRules> {
        self.combat_rules.as_ref()?.build_rules.as_ref()
    }
    pub(crate) fn known_power_skill(&self, skill: &str) -> bool {
        self.player_skill(skill).is_some()
            || self.pve.as_ref().is_some_and(|v| {
                v.build_profiles()
                    .values()
                    .any(|t| t.combat.projectile.id == skill || t.combat.heal.id == skill)
            })
    }
    pub(crate) fn valid_combat_build(&self, b: &CombatBuild) -> bool {
        let Some(r) = self.build_rules() else {
            return false;
        };
        [
            b.power_points,
            b.skill_power_points,
            b.element_resistance_points,
        ]
        .iter()
        .all(|n| *n <= r.maximum_input_points)
            && b.powers.len() <= 64
            && b.powers.iter().all(|(skill, p)| {
                let known = self.known_power_skill(skill);
                known
                    && r.power_models
                        .get(&p.model_id)
                        .is_some_and(|m| m.valid_input(p))
                    && self
                        .combat_rules
                        .as_ref()
                        .unwrap()
                        .skills
                        .get(skill)
                        .is_none_or(|rule| rule.base_power.is_none())
            })
    }
    pub(crate) fn derived_base(&self, points: u32, a: StatusAttribute) -> u32 {
        self.build_rules()
            .map_or(points, |r| r.conversion.amount(points, a))
    }
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let combat = c.combat_rules.as_ref().unwrap();
    if std::iter::once(&combat.default_member_profile)
        .chain(combat.member_profiles.values())
        .any(|id| combat.profiles.get(id).is_some_and(|p| p.build.is_some()))
    {
        return Err("CONFIG_MEMBER_BUILD_REQUIRES_TRUSTED_SEAT");
    }
    for (template, p) in c.pve.as_ref().unwrap().build_profiles() {
        let profile = combat
            .npc_profiles
            .get(&template)
            .unwrap_or(&combat.default_npc_profile);
        if combat
            .profiles
            .get(profile)
            .and_then(|p| p.build.as_ref())
            .is_some_and(|b| b.powers.contains_key(&p.combat.projectile.id))
            && combat
                .npc_attacks
                .get(&template)
                .is_some_and(|r| r.base_power.is_some())
        {
            return Err("CONFIG_NPC_AMBIGUOUS_POWER_SOURCE");
        }
    }

    if combat.status_power_sources.len() > 16
        || combat.status_power_sources.iter().any(|(id, skill)| {
            !c.known_power_skill(skill)
                || !c.pve.as_ref().unwrap().status_definitions.iter().any(|d| {
                    d.id == *id
                        && (combat.shield_records.contains_key(id)
                            || combat.contact_reactions.contains_key(id)
                            || d.is_periodic() && (!d.is_heal() || combat.healing.contains_key(id)))
                })
                || combat
                    .periodic_damage
                    .get(id)
                    .is_some_and(|r| r.base_power.is_some())
                || combat.periodic_sampling.get(id).is_some_and(|s| s.target())
        })
    {
        return Err("CONFIG_STATUS_POWER_SOURCE");
    }
    if combat.triggers.iter().any(|t| match &t.effect {
        TriggerEffect::ChainDamage { rule } => rule
            .source_skill_id
            .as_ref()
            .is_some_and(|id| !c.known_power_skill(id)),
        TriggerEffect::FinishOther { rule } => rule
            .source_skill_id
            .as_ref()
            .is_some_and(|id| !c.known_power_skill(id)),
        _ => false,
    }) {
        return Err("CONFIG_EFFECT_POWER_SOURCE");
    }
    let Some(r) = c.build_rules() else {
        if c.combat_rules
            .as_ref()
            .is_some_and(|r| r.profiles.values().any(|p| p.build.is_some()))
        {
            return Err("CONFIG_BUILD_RULES_REQUIRED");
        }
        return Ok(());
    };
    let x = &r.conversion;
    if [
        x.hp_bps,
        x.spirit_bps,
        x.atk_bps,
        x.def_bps,
        x.spd_bps,
        x.element_resistance_bps,
    ]
    .iter()
    .any(|n| *n == 0 || *n > 1000000)
        || r.maximum_input_points > 1000000
        || r.power_models.is_empty()
        || r.power_models.len() > 64
    {
        return Err("CONFIG_BUILD_RULES_BOUNDS");
    }
    for (id, m) in &r.power_models {
        let valid_id = !id.is_empty() && id.len() <= 64 && !id.chars().any(char::is_control);
        if !valid_id
            || m.rank_base.is_empty()
            || m.rank_base.len() > 16
            || m.rank_base.iter().any(|n| *n > super::MAX_BASE_RESOURCE)
            || m.external_bps > 1000000
            || m.maximum_power == 0
            || m.maximum_power > super::MAX_BASE_RESOURCE
            || [&m.quality_add_bps, &m.level_add_bps, &m.mastery_add_bps]
                .iter()
                .any(|v| {
                    v.is_empty()
                        || v.len() > 128
                        || v.iter().any(|n| !(-10000..=1000000).contains(n))
                })
        {
            return Err("CONFIG_POWER_MODEL_BOUNDS");
        }
    }
    if c.combat_rules
        .as_ref()
        .unwrap()
        .profiles
        .values()
        .any(|p| p.build.as_ref().is_some_and(|b| !c.valid_combat_build(b)))
    {
        return Err("CONFIG_BUILD_INPUT");
    }
    Ok(())
}
