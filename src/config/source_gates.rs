//! Typed eligibility and classification filters; there are no game-name branches.
use super::*;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SkillSelection {
    Any {},
    Only { ids: Vec<String> },
    Except { ids: Vec<String> },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionFilter {
    pub basic: bool,
    pub skills: SkillSelection,
}
impl ActionFilter {
    pub(crate) fn accepts(&self, origin: Option<&crate::origin::ActionOrigin>) -> bool {
        use crate::origin::ActionOrigin;
        match origin {
            None => false,
            Some(ActionOrigin::Basic) => self.basic,
            Some(ActionOrigin::Skill(id)) => self.accepts_skill(id),
        }
    }
    pub(crate) fn accepts_skill(&self, id: &str) -> bool {
        match &self.skills {
            SkillSelection::Any {} => true,
            SkillSelection::Only { ids } => ids.iter().any(|key| key == id),
            SkillSelection::Except { ids } => ids.iter().all(|key| key != id),
        }
    }
    pub(super) fn valid(&self, c: &Config) -> bool {
        match &self.skills {
            SkillSelection::Any {} => true,
            SkillSelection::Only { ids } | SkillSelection::Except { ids } => {
                ids.len() <= 64
                    && (self.basic
                        || !ids.is_empty()
                        || matches!(self.skills, SkillSelection::Except { .. }))
                    && ids
                        .iter()
                        .enumerate()
                        .all(|(i, id)| !ids[..i].contains(id) && c.known_power_skill(id))
            }
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SkillRequirement {
    pub skill_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub equipped_any_of: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub minimum_mastery: Option<usize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub maximum_mastery: Option<usize>,
}
impl SkillRequirement {
    pub(crate) fn valid(&self, c: &Config) -> bool {
        c.known_power_skill(&self.skill_id)
            && self.equipped_any_of.as_ref().is_none_or(|ids| {
                !ids.is_empty()
                    && ids.len() <= 8
                    && ids
                        .iter()
                        .enumerate()
                        .all(|(i, id)| !ids[..i].contains(id) && c.known_power_skill(id))
            })
            && self
                .minimum_mastery
                .zip(self.maximum_mastery)
                .is_none_or(|(min, max)| min <= max)
            && [self.minimum_mastery, self.maximum_mastery]
                .into_iter()
                .flatten()
                .all(|index| {
                    index < 128
                        && c.build_rules().is_some_and(|r| {
                            r.power_models
                                .values()
                                .any(|m| m.mastery_add_bps.len() > index)
                        })
                })
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MasteryStatuses {
    pub minimum_mastery: usize,
    pub statuses: Vec<String>,
}

impl MasteryStatuses {
    pub(crate) fn select(rows: &[Self], mastery: usize) -> Option<&[String]> {
        rows.iter()
            .rev()
            .find(|row| mastery >= row.minimum_mastery)
            .map(|row| row.statuses.as_slice())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MasteryCooldown {
    pub minimum_mastery: usize,
    pub reduction_bps: u32,
}

pub(super) fn one() -> u32 {
    1
}
pub(super) fn is_one(v: &u32) -> bool {
    *v == 1
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(r) = &c.combat_rules else {
        return Ok(());
    };
    for t in &r.triggers {
        if t.action_filter.as_ref().is_some_and(|f| !f.valid(c))
            || t.owner_skill.as_ref().is_some_and(|g| !g.valid(c))
            || !(1..=8).contains(&t.maximum_per_action)
            || t.maximum_per_action > 1 && t.counting != TriggerCounting::Action
        {
            return Err("CONFIG_COMBAT_SOURCE_GATE");
        }
    }
    if r.contact_reactions
        .values()
        .any(|r| r.source_requirement.as_ref().is_some_and(|g| !g.valid(c)))
    {
        return Err("CONFIG_CONTACT_SOURCE_GATE");
    }
    let mastery_count = c
        .build_rules()
        .and_then(|rules| {
            rules
                .power_models
                .values()
                .map(|m| m.mastery_add_bps.len())
                .max()
        })
        .unwrap_or_default();
    for (id, rows) in &r.self_statuses_by_mastery {
        if !c
            .player_skill(id)
            .is_some_and(|s| s.is_self() && !s.on_commit_self_statuses.is_empty())
            || !valid_mastery_status_rows(rows, mastery_count)
        {
            return Err("CONFIG_SELF_STATUS_MASTERY");
        }
    }
    if !r.skill_cooldowns_by_mastery.is_empty()
        && (c.protocol_version() != 6
            || r.skill_cooldowns_by_mastery.len() > super::MAX_SKILL_DEFINITIONS
            || r.skill_cooldowns_by_mastery.iter().any(|(id, rows)| {
                c.player_skill(id)
                    .is_none_or(|skill| skill.cooldown_ms == 0)
                    || !(1..=16).contains(&rows.len())
                    || rows.iter().any(|row| {
                        row.minimum_mastery >= mastery_count
                            || !(1..=10000).contains(&row.reduction_bps)
                    })
                    || rows
                        .windows(2)
                        .any(|pair| pair[0].minimum_mastery >= pair[1].minimum_mastery)
            }))
    {
        return Err("CONFIG_SKILL_COOLDOWN_MASTERY");
    }
    if r.unit_hit_statuses_by_mastery.len() > super::MAX_SKILL_DEFINITIONS
        || r.unit_hit_statuses_by_mastery.iter().any(|(id, rows)| {
            !c.player_skill(id)
                .is_some_and(|s| s.is_unit_effect() && s.channel.is_none())
                || !valid_mastery_status_rows(rows, mastery_count)
        })
    {
        return Err("CONFIG_UNIT_HIT_STATUS_MASTERY");
    }
    Ok(())
}

fn valid_mastery_status_rows(rows: &[MasteryStatuses], mastery_count: usize) -> bool {
    (1..=16).contains(&rows.len())
        && rows
            .iter()
            .all(|r| r.minimum_mastery < mastery_count && !r.statuses.is_empty())
        && rows
            .windows(2)
            .all(|pair| pair[0].minimum_mastery < pair[1].minimum_mastery)
}
