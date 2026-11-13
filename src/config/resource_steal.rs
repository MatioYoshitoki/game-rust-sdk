//! Frozen current-resource budgets. The explicit HP policy selects settlement.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResourceBonusMode {
    MultiplyBase,
    AddCurrentBps,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceStatusBonus {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub qualification: Option<super::SkillRequirement>,
    pub mode: ResourceBonusMode,
    pub subject: super::TriggerSubject,
    pub definition_id: String,
    pub per_stack_bps: u32,
    pub maximum_bonus_bps: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceSteal {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub status_bonus: Option<ResourceStatusBonus>,
    pub hp_current_bps: u32,
    pub spirit_current_bps: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub hp_policy: Option<crate::protocol::ResourceHpPolicy>,
}
impl ResourceSteal {
    pub(crate) fn valid(&self, c: &super::Config) -> bool {
        self.hp_current_bps <= 10000
            && self.spirit_current_bps <= 10000
            && (self.hp_current_bps > 0 || self.spirit_current_bps > 0)
            && (self.hp_current_bps > 0) == self.hp_policy.is_some()
            && self.status_bonus.as_ref().is_none_or(|b| {
                b.qualification.as_ref().is_none_or(|q| q.valid(c))
                    && (1..=100000).contains(&b.per_stack_bps)
                    && (1..=100000).contains(&b.maximum_bonus_bps)
                    && c.pve.as_ref().is_some_and(|p| {
                        p.status_definitions.iter().any(|d| {
                            d.id == b.definition_id && d.source_scoped && {
                                let maximum = (u64::from(d.maximum_stacks())
                                    * u64::from(b.per_stack_bps))
                                .min(u64::from(b.maximum_bonus_bps));
                                [self.hp_current_bps, self.spirit_current_bps]
                                    .into_iter()
                                    .all(|ratio| {
                                        ratio == 0
                                            || match b.mode {
                                                ResourceBonusMode::MultiplyBase => {
                                                    u64::from(ratio) * (10000 + maximum)
                                                        <= 100000000
                                                }
                                                ResourceBonusMode::AddCurrentBps => {
                                                    u64::from(ratio) + maximum <= 10000
                                                }
                                            }
                                    })
                            }
                        })
                    })
            })
    }
}
pub(super) fn fields(v: &serde_json::Value) -> Result<(), &'static str> {
    for s in super::player_skills::raw(v) {
        if let Some(value) = s.get("resource_steal") {
            if v["schema_version"] != 7
                || v["mechanism_version"] != 105
                || !v["members"].is_object()
            {
                return Err("CONFIG_RESOURCE_STEAL_CAPABILITY");
            }
            if !value.is_object() {
                return Err("CONFIG_RESOURCE_STEAL_SHAPE");
            }
        }
    }
    Ok(())
}
pub(super) fn validate(c: &super::Config) -> Result<(), &'static str> {
    if c.player_skills().any(|s| {
        s.resource_steal.as_ref().is_some_and(|r| {
            !r.valid(c) || !s.is_unit_effect() || r.hp_current_bps > 0 && c.combat_rules.is_none()
        })
    }) {
        return Err("CONFIG_RESOURCE_STEAL_BOUNDS");
    }
    Ok(())
}
