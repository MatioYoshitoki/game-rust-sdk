use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BoonOffer {
    Grant { choices: Vec<String> },
    Upgrade {},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoonSlotView {
    pub slot: u8,
    pub boon_id: String,
    pub level: u8,
    pub max_level: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoonOpportunityView {
    pub id: String,
    pub position: [f64; 2],
    pub offer: BoonOffer,
    pub available: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub shared_remaining: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoonView {
    pub capacity: u8,
    pub revision: u32,
    pub slots: Vec<BoonSlotView>,
    pub opportunities: Vec<BoonOpportunityView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub pending_claim: Option<BoonPendingClaimView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoonPendingClaimView {
    pub opportunity_id: String,
    pub boon_id: String,
    pub completes_at_ms: u64,
}
