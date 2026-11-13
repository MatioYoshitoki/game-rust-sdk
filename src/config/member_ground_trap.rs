use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberGroundTrap {
    pub duration_ms: u64,
    pub contact_policy: ContactPolicyConfig,
}

impl Config {
    pub(crate) fn member_ground_trap(&self, id: &str) -> Option<&MemberGroundTrap> {
        self.combat_rules.as_ref()?.member_ground_traps.get(id)
    }
}

pub(super) fn validate(c: &Config, rules: &CombatRules) -> Result<(), &'static str> {
    if rules.member_ground_traps.len() > 16
        || rules.member_ground_traps.iter().any(|(id, trap)| {
            c.player_skill(id).is_none_or(|skill| {
                skill.mechanism != PlayerSkillMechanism::GroundCircle
                    || skill.charge.is_some()
                    || skill.channel.is_some()
            }) || !(c.step_ms..=60000).contains(&trap.duration_ms)
                || !trap.contact_policy.valid()
        })
    {
        return Err("CONFIG_MEMBER_GROUND_TRAP");
    }
    Ok(())
}
