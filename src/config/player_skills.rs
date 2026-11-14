use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstantHeal {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub amount: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub max_hp_bps: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub overheal_shield: Option<OverhealShield>,
}

impl InstantHeal {
    pub fn base_amount(&self, max_hp: u32) -> u32 {
        self.amount.unwrap_or_else(|| {
            (u64::from(max_hp) * u64::from(self.max_hp_bps.unwrap()) / 10_000) as u32
        })
    }
}

pub(super) fn validate_skill_effects(c: &Config) -> Result<(), &'static str> {
    if c.player_skills().any(|s| {
        s.commit_on_accept
            && (c.mechanism_version < 105
                || c.combat_rules.is_none()
                || !s.is_self()
                || s.windup_ms != 0
                || s.channel.is_some()
                || s.charge.is_some())
    }) {
        return Err("CONFIG_SELF_SKILL_COMMIT_TIMING");
    }
    if (c.basic.unavoidable.is_some() || c.player_skills().any(|s| s.unavoidable.is_some()))
        && (c.schema_version != 7 || c.mechanism_version < 102)
    {
        return Err("CONFIG_MEMBER_UNAVOIDABLE_CAPABILITY");
    }
    if (c.basic.ignore_armor.is_some() || c.player_skills().any(|s| s.ignore_armor.is_some()))
        && (c.schema_version != 7 || c.mechanism_version < 103)
    {
        return Err("CONFIG_DIRECT_IGNORE_ARMOR_CAPABILITY");
    }
    for skill in c.player_skills() {
        if let Some(id) = &skill.ally_status {
            let shield = skill.is_ally_shield();
            let (minimum, capability_error, binding_error) = if shield {
                (
                    80,
                    "CONFIG_ALLY_SHIELD_CAPABILITY",
                    "CONFIG_ALLY_SHIELD_BINDING",
                )
            } else {
                (79, "CONFIG_ALLY_HOT_CAPABILITY", "CONFIG_ALLY_HOT_BINDING")
            };
            if c.schema_version != 7 || c.mechanism_version < minimum {
                return Err(capability_error);
            }
            if skill.instant_heal.is_some()
                || !c.pve.as_ref().is_some_and(|pve| {
                    pve.status_definitions.iter().any(|definition| {
                        definition.id == *id
                            && definition.source_scoped
                            && matches!(definition.duplicate_policy, DuplicatePolicy::Refresh)
                            && if shield {
                                definition.is_shield()
                            } else {
                                definition.is_heal()
                                    && definition.overheal_shield.as_ref().is_none_or(
                                        |conversion| {
                                            c.mechanism_version >= 82
                                                && pve
                                                    .overheal_shield_definition(conversion)
                                                    .is_some_and(|shield| shield.source_scoped)
                                        },
                                    )
                            }
                    })
                })
            {
                return Err(binding_error);
            }
        } else if skill.is_ally_heal()
            && skill.instant_heal.is_none()
            && skill.channel.as_ref().is_none_or(|c| c.recovery.is_none())
            && c.combat_rules
                .as_ref()
                .is_none_or(|r| !r.ally_recovery_by_mastery.contains_key(&skill.id))
        {
            return Err("CONFIG_ALLY_HEAL_BOUNDS");
        }
        let Some(heal) = &skill.instant_heal else {
            continue;
        };
        let ally = skill.is_ally_heal();
        let (minimum, capability_error, bounds_error) = if ally {
            (78, "CONFIG_ALLY_HEAL_CAPABILITY", "CONFIG_ALLY_HEAL_BOUNDS")
        } else {
            (77, "CONFIG_SELF_HEAL_CAPABILITY", "CONFIG_SELF_HEAL_BOUNDS")
        };
        if c.schema_version != 7 || c.mechanism_version < minimum {
            return Err(capability_error);
        }
        if !skill.on_commit_self_statuses.is_empty()
            || !match (heal.amount, heal.max_hp_bps) {
                (Some(amount), None) => (1..=1_000_000).contains(&amount),
                (None, Some(bps)) => (1..=10_000).contains(&bps),
                _ => false,
            }
            || heal.overheal_shield.as_ref().is_some_and(|conversion| {
                c.pve
                    .as_ref()
                    .and_then(|pve| pve.overheal_shield_definition(conversion))
                    .is_none_or(|shield| {
                        ally && (c.mechanism_version < 81 || !shield.source_scoped)
                    })
            })
        {
            return Err(bounds_error);
        }
    }
    Ok(())
}

pub(crate) const MAX_MEMBER_SKILLS: usize = 8;
pub(super) fn field_limit(v: &serde_json::Value) -> usize {
    if v["schema_version"] == 7 && matches!(v["mechanism_version"].as_u64(), Some(44..=107)) {
        MAX_MEMBER_SKILLS
    } else {
        2
    }
}
impl Config {
    pub(crate) fn default_player_skills(&self) -> impl Iterator<Item = &SkillConfig> {
        std::iter::once(&self.skill).chain(self.additional_skills.iter())
    }
    pub(crate) fn player_skills(&self) -> impl Iterator<Item = &SkillConfig> {
        self.default_player_skills()
            .chain(self.skill_library.iter())
    }
    pub(crate) fn player_skill(&self, id: &str) -> Option<&SkillConfig> {
        self.player_skills().find(|s| s.id == id)
    }
}
pub(super) fn raw(v: &serde_json::Value) -> impl Iterator<Item = &serde_json::Value> {
    std::iter::once(&v["skill"])
        .chain(v["additional_skills"].as_array().into_iter().flatten())
        .chain(v["skill_library"].as_array().into_iter().flatten())
}
pub(super) fn fields(v: &serde_json::Value, skill_limit: usize) -> Result<(), &'static str> {
    for skill in std::iter::once(&v["skill"])
        .chain(v["additional_skills"].as_array().into_iter().flatten())
        .chain(v["skill_library"].as_array().into_iter().flatten())
    {
        if skill["mechanism"] == "line_field"
            && (v["mechanism_version"].as_u64().is_none_or(|m| m < 34)
                || v["schema_version"] != 4
                || !v["pve"].is_object())
        {
            return Err("CONFIG_LINE_FIELD_CAPABILITY");
        }
        if skill.get("repeat_interval_ms").is_some()
            && (skill["mechanism"] != "piercing_projectile"
                || v["mechanism_version"].as_u64().is_none_or(|m| m < 33)
                || v["schema_version"] != 4
                || !v["pve"].is_object())
        {
            return Err("CONFIG_PROJECTILE_REPEAT_CAPABILITY");
        }
        if skill["mechanism"] == "piercing_projectile"
            && (v["mechanism_version"].as_u64().is_none_or(|m| m < 32)
                || v["schema_version"] != 4
                || !v["pve"].is_object())
        {
            return Err("CONFIG_PIERCING_CAPABILITY");
        }
        if skill.get("contact_policy").is_some()
            && (!matches!(
                skill["mechanism"].as_str(),
                Some("ground_field" | "line_field")
            ) || v["mechanism_version"].as_u64().is_none_or(|m| m < 31)
                || v["schema_version"] != 4
                || !v["pve"].is_object())
        {
            return Err("CONFIG_CONTACT_POLICY_CAPABILITY");
        }
        if skill["mechanism"] == "ground_field"
            && (v["mechanism_version"].as_u64().is_none_or(|m| m < 30)
                || v["schema_version"] != 4
                || !v["pve"].is_object())
        {
            return Err("CONFIG_FIELD_CAPABILITY");
        }
        if skill["mechanism"] == "ground_circle"
            && (v["mechanism_version"].as_u64().is_none_or(|m| m < 29)
                || v["schema_version"] != 4
                || !v["pve"].is_object())
        {
            return Err("CONFIG_GROUND_SKILL_CAPABILITY");
        }
        if skill["mechanism"] == "unit_projectile"
            && (v["mechanism_version"].as_u64().is_none_or(|m| m < 28)
                || v["schema_version"] != 4
                || !v["pve"].is_object())
        {
            return Err("CONFIG_UNIT_SKILL_CAPABILITY");
        }
        if skill["mechanism"] == "self_status"
            && (v["mechanism_version"].as_u64().is_none_or(|m| m < 27)
                || v["schema_version"] != 4
                || !v["pve"].is_object())
        {
            return Err("CONFIG_SELF_SKILL_CAPABILITY");
        }
    }
    if let Some(rows) = v.get("additional_skills") {
        if v["mechanism_version"].as_u64().is_none_or(|n| n < 26)
            || v["schema_version"] != 4
            || !v["pve"].is_object()
        {
            return Err("CONFIG_PLAYER_SKILLS_CAPABILITY");
        }
        if rows.as_array().is_none_or(|a| {
            (if skill_limit == 2 {
                a.len() != 1
            } else {
                a.len() >= skill_limit
            }) || a.iter().any(|s| !s.is_object())
        }) {
            return Err("CONFIG_PLAYER_SKILLS_COUNT");
        }
    }
    Ok(())
}
pub(super) fn validate(c: &Config, skill_limit: usize) -> Result<(), &'static str> {
    if c.mechanism_version >= 26 && (c.schema_version != 4 || c.pve.is_none()) {
        return Err("CONFIG_PLAYER_SKILLS_CAPABILITY");
    }
    if c.additional_skills.is_empty() && c.mechanism_version < 27 {
        return Ok(());
    }
    if c.mechanism_version < 26
        || c.schema_version != 4
        || c.pve.is_none()
        || c.additional_skills.len() >= skill_limit
    {
        return Err("CONFIG_PLAYER_SKILLS_COUNT");
    }
    if c.player_skills().count() > super::skill_loadouts::MAX_SKILL_DEFINITIONS {
        return Err("CONFIG_SKILL_LIBRARY_BOUNDS");
    }
    let mut ids = std::collections::BTreeSet::new();
    for s in c.player_skills() {
        if s.is_line()
            && (c.mechanism_version < 34
                || s.contact_policy.is_none()
                || !(0.1..=128.).contains(&s.radius))
        {
            return Err("CONFIG_LINE_FIELD_BOUNDS");
        }
        if let Some(interval) = s.projectile_repeat_interval_ms
            && (!s.is_piercing() || c.mechanism_version < 33 || interval > 60000)
        {
            return Err("CONFIG_PROJECTILE_REPEAT_BOUNDS");
        }
        if s.is_piercing()
            && (c.mechanism_version < 32
                || !(1..=128).contains(&s.max_hits)
                || s.projectile_ttl_ms < c.step_ms.saturating_mul(2)
                || s.projectile_ttl_ms > 60000)
        {
            return Err("CONFIG_PIERCING_BOUNDS");
        }
        if let Some(p) = &s.contact_policy
            && (!s.is_field() || c.mechanism_version < 31 || !p.valid())
        {
            return Err("CONFIG_CONTACT_POLICY_BOUNDS");
        }
        if s.is_field()
            && (c.mechanism_version < 30
                || s.duration_ms == 0
                || s.duration_ms > 60000
                || s.tick_interval_ms == 0
                || s.tick_interval_ms < c.step_ms
                || s.tick_interval_ms > 60000
                || s.duration_ms <= s.tick_interval_ms
                || (s.duration_ms - 1) / s.tick_interval_ms > 128)
        {
            return Err("CONFIG_FIELD_BOUNDS");
        }
        if (s.is_ground() || s.is_line())
            && (c.mechanism_version < 29
                || s.warning_ms > 60000
                || !(0.1..=128.).contains(&s.radius))
        {
            return Err("CONFIG_GROUND_SKILL_BOUNDS");
        }
        if s.is_unit() && c.mechanism_version < 28 {
            return Err("CONFIG_UNIT_SKILL_CAPABILITY");
        }
        if s.id.is_empty()
            || s.id.len() > 64
            || s.id.chars().any(char::is_control)
            || !ids.insert(&s.id)
            || [s.windup_ms, s.recovery_ms, s.cooldown_ms]
                .iter()
                .any(|t| *t > 3600000)
            || s.cost > c.resource_bounds().max_spirit_cost
        {
            return Err("CONFIG_PLAYER_SKILL_BOUNDS");
        }
        if s.is_self() {
            if c.mechanism_version < 27
                || (s.on_commit_self_statuses.is_empty()
                    && s.instant_heal.is_none()
                    && c.summon_cast(&s.id).is_none()
                    && s.channel
                        .clone()
                        .is_none_or(|channel| channel.recovery.is_none()))
                || s.on_commit_self_statuses.len() > 2
                || !s.on_hit_target_statuses.is_empty()
            {
                return Err("CONFIG_SELF_SKILL_BINDINGS");
            }
        } else if s.damage_bps > 100000
            || !s.range.is_finite()
            || !(0.1..=10000.).contains(&s.range)
            || (!s.is_ground()
                && !s.is_line()
                && !s.targets_ally()
                && !s.is_unit_effect()
                && (!s.speed.is_finite() || !(0.1..=10000.).contains(&s.speed)))
            || !s.radius.is_finite()
            || !(0.0..=128.).contains(&s.radius)
        {
            return Err("CONFIG_PLAYER_SKILL_BOUNDS");
        }
    }
    Ok(())
}
#[cfg(all(test, match_sdk_test_data))]
#[path = "player_skills_tests.rs"]
mod tests;
