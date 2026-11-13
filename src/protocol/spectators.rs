use super::*;

/// Host-authenticated control of a bounded, read-only slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectateV6 {
    pub key: MatchKey,
    pub slot: u8,
    pub connection: Connection,
    pub action: SpectatorActionV6,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpectatorActionV6 {
    Attach {
        authenticated_principal: String,
    },
    Detach {},
    RequestFull {
        stream_epoch: u32,
    },
    Transport {
        publication_id: String,
        outcome: TransportOutcome,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorMemberV6 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub extraction_channel: Option<ExtractionChannelView>,
    pub entity: EntityRef,
    pub position: [f64; 2],
    pub hp: u32,
    pub max_hp: u32,
    pub phase: MemberPhase,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub outcome: Option<Outcome>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorMonsterV6 {
    pub entity: EntityRef,
    pub template_id: String,
    pub position: [f64; 2],
    pub hp: u32,
    pub max_hp: u32,
    pub radius: f64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub summon_owner: Option<EntityRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorProjectileV6 {
    pub id: String,
    pub source: EntityRef,
    pub position: [f64; 2],
    pub radius: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorObjectV6 {
    pub id: String,
    pub position: [f64; 2],
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorAreaV6 {
    pub id: String,
    pub center: [f64; 2],
    pub radius: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorStageV6 {
    pub id: String,
    pub index: u8,
    pub objective: ModeObjective,
    pub opens_at_ms: u64,
    pub expires_at_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub wave: Option<ModeWaveView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub progress: Option<ModeProgressView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorFactV6 {
    pub sequence: u64,
    pub logical_ms: u64,
    pub event: SpectatorEventV6,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpectatorEventV6 {
    ExtractionChannelChanged(ExtractionChannelChange),
    Damage {
        source: EntityRef,
        target: EntityRef,
        amount: u32,
        hp: u32,
    },
    MemberChanged {
        entity: EntityRef,
        phase: MemberPhase,
    },
    Respawned {
        entity: EntityRef,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorSnapshotV6 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub contested_equipment: Option<Vec<WorldEquipmentView>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub extraction_pressure: Option<ExtractionPressureView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub encounter_progress: Option<EncounterProgressView>,
    pub phase: Phase,
    pub logical_ms: u64,
    pub active_at_ms: u64,
    pub members: Vec<SpectatorMemberV6>,
    pub monsters: Vec<SpectatorMonsterV6>,
    pub projectiles: Vec<SpectatorProjectileV6>,
    pub resources: Vec<SpectatorObjectV6>,
    pub areas: Vec<SpectatorAreaV6>,
    pub events: Vec<SpectatorFactV6>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub stage: Option<SpectatorStageV6>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorDeltaV6 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub contested_equipment: Option<Vec<WorldEquipmentView>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub extraction_pressure: Option<ExtractionPressureView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub encounter_progress: Option<EncounterProgressView>,
    pub logical_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub members: Option<ListDelta<SpectatorMemberV6, EntityRef>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub monsters: Option<ListDelta<SpectatorMonsterV6, EntityRef>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub projectiles: Option<ListDelta<SpectatorProjectileV6, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub resources: Option<ListDelta<SpectatorObjectV6, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub areas: Option<ListDelta<SpectatorAreaV6, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub events: Option<Vec<SpectatorFactV6>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub stage: Option<SpectatorStageV6>,
}

pub type SpectatorPayloadV6 = ReplicationPayloadV6<SpectatorSnapshotV6, SpectatorDeltaV6>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectatorEnvelopeV6 {
    pub key: MatchKey,
    pub publication_id: String,
    pub slot: u8,
    pub connection: Connection,
    pub stream_epoch: u32,
    pub revision: u64,
    pub payload: SpectatorPayloadV6,
}

impl SpectatorEnvelopeV6 {
    pub fn from_output(output: &Output) -> Option<Self> {
        let OutputBody::SpectatorPublicationV6 {
            slot,
            connection,
            stream_epoch,
            revision,
            payload,
        } = &output.body
        else {
            return None;
        };
        Some(Self {
            key: output.key.clone(),
            publication_id: output.id.clone(),
            slot: *slot,
            connection: connection.clone(),
            stream_epoch: *stream_epoch,
            revision: *revision,
            payload: (**payload).clone(),
        })
    }
}
