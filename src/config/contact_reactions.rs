//! Finite, source-attributed reactions of existing single-instance harmful states.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ContactReactionWhen {
    DirectContact,
    HpDamage,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReactionAttacker {
    SourceOnly,
    AnyHostile,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContactReactionRule {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub source_requirement: Option<super::SkillRequirement>,
    pub when: ContactReactionWhen,
    pub attacker: ReactionAttacker,
    pub fixed: u32,
    pub source_power_bps: u32,
    pub damage_kind: DamageKind,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub element: Option<String>,
    pub ignore_armor: bool,
    pub armor_penetration_cap_bps: u32,
}
impl ContactReactionRule {
    pub(crate) fn amount(&self, power: u32) -> u32 {
        (u128::from(self.fixed) + u128::from(power) * u128::from(self.source_power_bps) / 10000)
            .min(u128::from(u32::MAX)) as u32
    }
}
pub(super) fn hit_default() -> usize {
    1
}
pub(super) fn step_default() -> usize {
    32
}
pub(super) fn hit_is_default(v: &usize) -> bool {
    *v == 1
}
pub(super) fn step_is_default(v: &usize) -> bool {
    *v == 32
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if r.contact_reactions.len() > 16
        || !(1..=4).contains(&r.max_contact_reactions_per_hit)
        || !(1..=128).contains(&r.max_contact_reactions_per_step)
    {
        return Err("CONFIG_CONTACT_REACTION_BUDGET");
    }
    for (id, rule) in &r.contact_reactions {
        let d = c
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .find(|d| d.id == *id)
            .ok_or("CONFIG_CONTACT_REACTION_REFERENCE")?;
        if !d.source_scoped
            || !d.is_harmful()
            || !matches!(
                d.duplicate_policy,
                DuplicatePolicy::Refresh | DuplicatePolicy::Ignore
            )
            || !matches!(
                d.mechanism,
                StatusMechanism::TimedAttributeModifier
                    | StatusMechanism::TimedDefenseModifier
                    | StatusMechanism::TimedCompoundModifier
                    | StatusMechanism::TimedControl
            )
            || d.attribute.is_some_and(|a| a.is_immunity_flag())
            || d.flags.iter().any(|a| !a.is_control_flag())
            || r.status_windows.contains_key(id)
            || r.status_cleanses.contains_key(id)
            || r.status_policies
                .get(id)
                .is_none_or(|p| p.disposition != StatusDisposition::Harmful)
        {
            return Err("CONFIG_CONTACT_REACTION_CARRIER");
        }
        if rule.fixed > 1000000
            || rule.source_power_bps > 100000
            || rule.fixed == 0 && rule.source_power_bps == 0
            || rule.armor_penetration_cap_bps > 10000
            || rule
                .element
                .as_ref()
                .is_some_and(|e| !r.elements.contains(e))
            || rule.damage_kind == DamageKind::Elemental && rule.element.is_none()
            || rule.damage_kind == DamageKind::TrueDamage && rule.element.is_some()
        {
            return Err("CONFIG_CONTACT_REACTION_POLICY");
        }
        if std::iter::once(&r.basic_attack)
            .chain(r.skills.values())
            .chain(r.npc_attacks.values())
            .any(|a| {
                a.contact_status
                    .as_ref()
                    .is_some_and(|s| s.definition_id == *id)
            })
        {
            return Err("CONFIG_CONTACT_REACTION_DOUBLE_CONSUMPTION");
        }
    }
    Ok(())
}
