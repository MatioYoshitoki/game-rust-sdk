use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParticipantAi {
    pub decision_interval_ms: u64,
    pub loot_radius: f64,
    pub threat_radius: f64,
    pub withdraw_hp_bps: u32,
    pub extract_after_looted: u32,
    pub extract_before_timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search_points: Vec<[f64; 2]>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub combat_skill_id: Option<String>,
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    if c.participant_ai.is_empty() {
        return Ok(());
    }
    if c.mechanism_version < 105 || c.protocol_version() != 6 {
        return Err("CONFIG_PARTICIPANT_AI_CAPABILITY");
    }
    if c.participant_ai.len() > c.member_settings().unwrap().capacity
        || c.participant_ai.iter().any(|(seat, policy)| {
            !crate::protocol::valid_id(seat)
                || !(c.step_ms..=60000).contains(&policy.decision_interval_ms)
                || !(0.0..=10000.0).contains(&policy.loot_radius)
                || !(0.0..=10000.0).contains(&policy.threat_radius)
                || policy.withdraw_hp_bps > 10000
                || policy.extract_after_looted > 1_000_000
                || policy.extract_before_timeout_ms > c.terminal_ms
                || policy.search_points.len() > 16
                || policy
                    .search_points
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite())
                || policy.combat_skill_id.as_ref().is_some_and(|id| {
                    c.player_skill(id)
                        .is_none_or(|skill| !skill.is_unit_effect())
                })
        })
    {
        return Err("CONFIG_PARTICIPANT_AI_POLICY");
    }
    Ok(())
}
