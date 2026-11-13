//! Typed contact consumption predicates and bounded skill deadline edits.
use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContactConsumption {
    pub definition_id: String,
    pub at_least: u32,
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    for t in &r.triggers {
        if t.when == CombatFact::ContactStatusConsumed {
            let Some(f) = &t.contact_consumption else {
                return Err("CONFIG_CONTACT_CONSUMPTION_FILTER");
            };
            if t.include_periodic
                || t.counting == TriggerCounting::ActionTotal
                || f.at_least == 0
                || !std::iter::once(&r.basic_attack)
                    .chain(r.skills.values())
                    .chain(r.npc_attacks.values())
                    .any(|a| {
                        a.contact_status.as_ref().is_some_and(|s| {
                            s.definition_id == f.definition_id && f.at_least <= s.consume_max
                        })
                    })
            {
                return Err("CONFIG_CONTACT_CONSUMPTION_POLICY");
            }
        } else if t.contact_consumption.is_some() {
            return Err("CONFIG_CONTACT_CONSUMPTION_FACT");
        }
        if let TriggerEffect::ReduceSkillCooldowns {
            skill_ids,
            remaining_bps,
            fixed_ms,
        } = &t.effect
            && (skill_ids.is_empty()
                || skill_ids.len() > 8
                || skill_ids
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != skill_ids.len()
                || skill_ids.iter().any(|id| c.player_skill(id).is_none())
                || *remaining_bps > 10000
                || *fixed_ms > 3600000
                || *remaining_bps == 0 && *fixed_ms == 0)
        {
            return Err("CONFIG_COOLDOWN_RETURN_POLICY");
        }
    }
    Ok(())
}
