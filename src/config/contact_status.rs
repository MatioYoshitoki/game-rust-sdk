//! A single bounded status condition on an attack, sampled at native contact.
use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContactStatusRule {
    #[serde(
        default,
        skip_serializing_if = "ContactSampling::contact",
        deserialize_with = "sampling"
    )]
    pub sampling: ContactSampling,
    pub subject: TriggerSubject,
    pub definition_id: String,
    pub at_least: u32,
    pub consume_max: u32,
    pub damage_bonus_bps_per_stack: u32,
    pub armor_penetration_bps_per_stack: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ContactSampling {
    #[default]
    Contact,
    Action,
}
impl ContactSampling {
    fn contact(&self) -> bool {
        *self == Self::Contact
    }
}
fn sampling<'de, D: serde::Deserializer<'de>>(d: D) -> Result<ContactSampling, D::Error> {
    match String::deserialize(d)?.as_str() {
        "contact" => Ok(ContactSampling::Contact),
        "action" => Ok(ContactSampling::Action),
        v => Err(serde::de::Error::unknown_variant(v, &["contact", "action"])),
    }
}
pub(super) fn default_snapshots() -> usize {
    256
}
pub(super) fn default_snapshot_count(n: &usize) -> bool {
    *n == 256
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    if !(1..=1024).contains(&r.max_contact_action_snapshots) {
        return Err("CONFIG_CONTACT_ACTION_BUDGET");
    }
    if r.periodic_damage
        .values()
        .any(|r| r.contact_status.is_some())
    {
        return Err("CONFIG_CONTACT_STATUS_PERIODIC");
    }
    for attack in std::iter::once(&r.basic_attack)
        .chain(r.skills.values())
        .chain(r.npc_attacks.values())
        .chain(r.periodic_damage.values())
    {
        if attack.armor_penetration_cap_bps.is_some_and(|n| n > 10000) {
            return Err("CONFIG_CONTACT_PENETRATION_CAP");
        }
        let Some(rule) = &attack.contact_status else {
            continue;
        };
        let Some(d) = c
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .find(|d| d.id == rule.definition_id)
        else {
            return Err("CONFIG_CONTACT_STATUS_REFERENCE");
        };
        if !d.source_scoped
            || !matches!(
                d.mechanism,
                StatusMechanism::TimedAttributeModifier
                    | StatusMechanism::TimedDefenseModifier
                    | StatusMechanism::TimedCompoundModifier
                    | StatusMechanism::TimedControl
            )
            || r.status_windows.contains_key(&d.id)
            || r.status_cleanses.contains_key(&d.id)
            || rule.at_least == 0
            || rule.at_least > d.maximum_stacks()
            || rule.consume_max == 0
            || rule.consume_max > d.maximum_stacks()
            || rule.damage_bonus_bps_per_stack > 100000
            || rule.armor_penetration_bps_per_stack > 10000
            || rule.sampling == ContactSampling::Action
                && !matches!(rule.subject, TriggerSubject::Owner)
            || rule.sampling == ContactSampling::Contact
                && rule.damage_bonus_bps_per_stack == 0
                && rule.armor_penetration_bps_per_stack == 0
            || u64::from(rule.consume_max) * u64::from(rule.damage_bonus_bps_per_stack) > 1000000
            || u64::from(rule.consume_max) * u64::from(rule.armor_penetration_bps_per_stack) > 10000
            || rule.armor_penetration_bps_per_stack > 0
                && attack.armor_penetration_cap_bps.is_none()
        {
            return Err("CONFIG_CONTACT_STATUS_POLICY");
        }
    }
    Ok(())
}
