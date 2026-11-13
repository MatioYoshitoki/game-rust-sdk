use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContactPolicyConfig {
    pub max_targets_per_tick: u32,
    pub max_total_hits: u32,
    pub repeat_interval_ms: u64,
}
impl ContactPolicyConfig {
    pub(super) fn valid(&self) -> bool {
        (1..=32).contains(&self.max_targets_per_tick)
            && (1..=128).contains(&self.max_total_hits)
            && self.repeat_interval_ms <= 60000
    }
}
pub(super) fn present<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<ContactPolicyConfig>, D::Error> {
    ContactPolicyConfig::deserialize(d).map(Some)
}
