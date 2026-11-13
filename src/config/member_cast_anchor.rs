use super::*;
use crate::protocol::AnchorRecastBehavior;
use std::collections::BTreeSet;

pub(crate) const MAX_ANCHOR_RECAST_COUNT: u32 = 8;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberCastAnchor {
    pub duration_ms: u64,
    pub max_use_distance: f64,
    pub consumer_skill_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub following_field_skill_ids: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub recast: Option<AnchorRecast>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnchorRecast {
    pub window_ms: u64,
    pub max_count: u32,
    pub spirit_cost: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub behaviors: Option<Vec<AnchorRecastBehavior>>,
}

impl AnchorRecast {
    pub(crate) fn permits(&self, behavior: AnchorRecastBehavior) -> bool {
        self.behaviors
            .as_ref()
            .map_or(behavior == AnchorRecastBehavior::Relocate, |enabled| {
                enabled.contains(&behavior)
            })
    }
}

impl Config {
    pub(crate) fn member_cast_anchor(&self, skill_id: &str) -> Option<&MemberCastAnchor> {
        self.combat_rules
            .as_ref()?
            .member_cast_anchors
            .get(skill_id)
    }
}

pub(super) fn validate(c: &Config, rules: &CombatRules) -> Result<(), &'static str> {
    let anchors = &rules.member_cast_anchors;
    if anchors.len() > 16
        || anchors.iter().any(|(id, anchor)| {
            c.player_skill(id).is_none_or(|s| {
                s.mechanism != PlayerSkillMechanism::GroundCircle
                    || s.damage_bps != 0
                    || s.warning_ms != 0
                    || s.charge.is_some()
                    || s.channel.is_some()
                    || !s.on_hit_target_statuses.is_empty()
            }) || rules.skills.contains_key(id)
                || rules.member_ground_traps.contains_key(id)
                || !(c.step_ms..=60000).contains(&anchor.duration_ms)
                || !(0.1..=10000.).contains(&anchor.max_use_distance)
                || anchor.recast.as_ref().is_some_and(|recast| {
                    c.protocol_version() != 6
                        || !(c.step_ms..=anchor.duration_ms.min(MAX_RECAST_WINDOW_MS))
                            .contains(&recast.window_ms)
                        || !recast.window_ms.is_multiple_of(c.step_ms)
                        || !(1..=MAX_ANCHOR_RECAST_COUNT).contains(&recast.max_count)
                        || recast.spirit_cost > c.resource_bounds().max_spirit_cost
                        || recast
                            .behaviors
                            .as_ref()
                            .is_some_and(|enabled| !AnchorRecastBehavior::valid_set(enabled))
                })
                || !(1..=8).contains(&anchor.consumer_skill_ids.len())
                || !valid_following_fields(c, anchor)
                || anchor
                    .consumer_skill_ids
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != anchor.consumer_skill_ids.len()
                || anchor.consumer_skill_ids.iter().any(|id| {
                    anchors.contains_key(id)
                        || rules.member_ground_traps.contains_key(id)
                        || c.player_skill(id).is_none_or(|s| {
                            !matches!(
                                s.mechanism,
                                PlayerSkillMechanism::Projectile
                                    | PlayerSkillMechanism::GroundCircle
                                    | PlayerSkillMechanism::GroundField
                            ) || s.charge.is_some()
                                || s.channel.is_some()
                                || s.recast.is_some()
                        })
                })
        })
    {
        return Err("CONFIG_MEMBER_CAST_ANCHOR");
    }
    Ok(())
}

fn valid_following_fields(c: &Config, anchor: &MemberCastAnchor) -> bool {
    let following = &anchor.following_field_skill_ids;
    following.is_empty()
        || c.protocol_version() == 6
            && following.iter().collect::<BTreeSet<_>>().len() == following.len()
            && following.iter().all(|id| {
                anchor.consumer_skill_ids.contains(id)
                    && c.player_skill(id)
                        .is_some_and(|skill| skill.mechanism == PlayerSkillMechanism::GroundField)
            })
}
