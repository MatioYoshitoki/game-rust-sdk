use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberSummons {
    pub max_active_per_member: usize,
    pub skills: BTreeMap<String, SummonCast>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SummonCast {
    pub template_id: String,
    pub count: usize,
    pub duration_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub inherit_attributes_bps: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub skill_copy: Option<SummonSkillCopy>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub death_burst: Option<SummonDeathBurst>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub owner_while_alive: Option<SummonOwnerBonus>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SummonOwnerBonus {
    pub damage_bonus_bps: u32,
    pub damage_reduction_bps: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SummonSkillCopy {
    pub skill_ids: Vec<String>,
    pub shared_interval_ms: u64,
    pub damage_bps: u32,
    pub max_circle_targets: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SummonDeathBurst {
    pub radius: f64,
    pub max_targets: usize,
    pub damage_bps: u32,
}

impl MemberSummons {
    pub(super) fn contact_reserve(
        &self,
        c: &Config,
        members: usize,
        projectile_capacity: usize,
    ) -> usize {
        let burst_targets = self
            .skills
            .values()
            .filter_map(|rule| rule.death_burst)
            .map(|burst| burst.max_targets)
            .max()
            .unwrap_or(0);
        let copies = self
            .skills
            .values()
            .filter_map(|rule| rule.skill_copy.as_ref());
        let copy_targets = copies
            .clone()
            .map(|copy| {
                if copy
                    .skill_ids
                    .iter()
                    .any(|id| c.player_skill(id).is_some_and(SkillConfig::is_ground))
                {
                    copy.max_circle_targets
                } else {
                    1
                }
            })
            .max()
            .unwrap_or(0);
        members * (self.max_active_per_member * (1 + burst_targets) + copy_targets)
            + usize::from(copies.flat_map(|copy| &copy.skill_ids).any(|id| {
                c.player_skill(id)
                    .is_some_and(SkillConfig::is_copy_projectile)
            })) * projectile_capacity
    }
}

impl Config {
    pub(crate) fn member_summons(&self) -> Option<&MemberSummons> {
        self.combat_rules.as_ref()?.member_summons.as_ref()
    }

    pub(crate) fn summon_cast(&self, skill_id: &str) -> Option<&SummonCast> {
        self.member_summons()?.skills.get(skill_id)
    }
}

pub(super) fn validate(c: &Config, rules: &CombatRules) -> Result<(), &'static str> {
    let Some(summons) = &rules.member_summons else {
        return Ok(());
    };
    if c.protocol_version() != 6 {
        return Err("CONFIG_MEMBER_SUMMON_CAPABILITY");
    }
    let pve = c.pve.as_ref().ok_or("CONFIG_MEMBER_SUMMON_CAPABILITY")?;
    if !(1..=4).contains(&summons.max_active_per_member)
        || summons.skills.is_empty()
        || summons.skills.len() > super::MAX_MEMBER_SKILLS
        || summons.skills.iter().any(|(id, summon)| {
            c.player_skill(id)
                .is_none_or(|s| !s.is_self() || s.channel.is_some() || s.charge.is_some())
                || summon.template_id != pve.template_id
                    && !pve.additional_templates.contains_key(&summon.template_id)
                || !(1..=summons.max_active_per_member).contains(&summon.count)
                || !(c.step_ms.saturating_mul(2)..=60000).contains(&summon.duration_ms)
        })
    {
        return Err("CONFIG_MEMBER_SUMMON_BINDING");
    }
    for cast in summons.skills.values() {
        if cast
            .inherit_attributes_bps
            .is_some_and(|bps| !(1000..=10000).contains(&bps))
            || cast.death_burst.is_some_and(|burst| {
                !(0.1..=128.).contains(&burst.radius)
                    || !(1..=8).contains(&burst.max_targets)
                    || !(1..=100000).contains(&burst.damage_bps)
            })
            || cast.owner_while_alive.is_some_and(|bonus| {
                c.mechanism_version < 105
                    || bonus.damage_bonus_bps > 10000
                    || bonus.damage_reduction_bps > 10000
                    || bonus.damage_bonus_bps == 0 && bonus.damage_reduction_bps == 0
                    || bonus.damage_reduction_bps > 0 && pve.status_rules.is_none()
            })
        {
            return Err("CONFIG_MEMBER_SUMMON_EFFECT");
        }
        if let Some(copy) = &cast.skill_copy {
            let unique = copy
                .skill_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>();
            if unique.len() != copy.skill_ids.len()
                || !(1..=super::MAX_MEMBER_SKILLS).contains(&unique.len())
                || !(c.step_ms..=60000).contains(&copy.shared_interval_ms)
                || !(1..=10000).contains(&copy.damage_bps)
                || !(1..=8).contains(&copy.max_circle_targets)
                || copy.skill_ids.iter().any(|id| {
                    c.player_skill(id)
                        .is_none_or(|skill| summons.skills.contains_key(id) || !copyable(c, skill))
                })
            {
                return Err("CONFIG_MEMBER_SUMMON_COPY");
            }
        }
    }
    Ok(())
}

impl SkillConfig {
    pub(crate) fn is_copy_projectile(&self) -> bool {
        matches!(
            self.mechanism,
            PlayerSkillMechanism::Projectile | PlayerSkillMechanism::UnitProjectile
        )
    }
}

fn copyable(c: &Config, skill: &SkillConfig) -> bool {
    (skill.is_copy_projectile()
        || skill.is_unit_effect()
        || (skill.mechanism == PlayerSkillMechanism::GroundCircle
            && skill.warning_ms == 0
            && c.member_ground_trap(&skill.id).is_none()
            && c.member_cast_anchor(&skill.id).is_none()))
        && skill.windup_ms <= 60000
        && skill.channel.is_none()
        && skill.charge.is_none()
        && skill.recast.is_none()
        && (skill.damage_bps > 0
            || c.combat_rules
                .as_ref()
                .unwrap()
                .skills
                .get(&skill.id)
                .is_some_and(|rule| rule.fixed > 0))
}
