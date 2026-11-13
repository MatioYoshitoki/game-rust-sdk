use super::*;
#[path = "spectators.rs"]
mod spectators;
pub use spectators::*;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateV6 {
    #[serde(deserialize_with = "version_six")]
    pub protocol_version: u32,
    pub key: MatchKey,
    pub request_id: String,
    pub config_id: String,
    pub config_hash: String,
    pub seed: u32,
    pub participants: Vec<ParticipantSeat>,
    #[serde(default)]
    pub publication_mode: PublicationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryV4>,
}
fn version_six<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<u32, D::Error> {
    let n = u32::deserialize(d)?;
    if n == 6 {
        Ok(n)
    } else {
        Err(serde::de::Error::custom("expected protocol 6"))
    }
}
impl From<CreateV6> for CreateRequest {
    fn from(v: CreateV6) -> Self {
        Self::Replicated(v)
    }
}
impl From<CreateV6> for CreateV4 {
    fn from(v: CreateV6) -> Self {
        Self {
            protocol_version: v.protocol_version,
            key: v.key,
            request_id: v.request_id,
            config_id: v.config_id,
            config_hash: v.config_hash,
            seed: v.seed,
            participants: v.participants,
            publication_mode: v.publication_mode,
            recovery: v.recovery,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullReason {
    Initial,
    Periodic,
    Expired,
    Failure,
    Timeout,
    Resync,
    Lifecycle,
    Terminal,
    Size,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotStateV6 {
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
    pub active_at_ms: Option<u64>,
    pub extraction_point: [f64; 2],
    pub extraction_open: bool,
}
fn non_null<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListDelta<T, K> {
    pub upsert: Vec<T>,
    pub removed: Vec<K>,
    pub order: Vec<K>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDeltaV6 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub relics: Option<Vec<RelicView>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "non_null"
    )]
    pub containers: Option<Vec<ContainerViewV6>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub active_blockers: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub checkpoint_zones: Option<Vec<CheckpointZoneView>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_waves: Option<Vec<BasicWaveView>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_impacts: Option<Vec<BasicImpactView>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_beams: Option<Vec<BasicBeamView>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub areas: Option<Vec<AreaPlanViewV4>>,
    pub logical_ms: u64,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub state: Option<SnapshotStateV6>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub own: Option<PrivateMemberView>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub members: Option<ListDelta<PublicMemberView, EntityRef>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub monsters: Option<ListDelta<MonsterViewV4, EntityRef>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub resources: Option<ListDelta<ResourceViewV4, String>>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub projectiles: Option<ListDelta<ProjectileViewV4, String>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplicationPayloadV6<S, D> {
    Full {
        snapshot: Box<S>,
        reason: FullReason,
    },
    Delta {
        base_publication_id: String,
        base_revision: u64,
        delta: Box<D>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PublicationPayloadV6 {
    Full {
        snapshot: Box<SnapshotV4>,
        reason: FullReason,
    },
    Delta {
        base_publication_id: String,
        base_revision: u64,
        delta: Box<SnapshotDeltaV6>,
    },
}

impl From<ReplicationPayloadV6<SnapshotV4, SnapshotDeltaV6>> for PublicationPayloadV6 {
    fn from(payload: ReplicationPayloadV6<SnapshotV4, SnapshotDeltaV6>) -> Self {
        match payload {
            ReplicationPayloadV6::Full { snapshot, reason } => Self::Full { snapshot, reason },
            ReplicationPayloadV6::Delta {
                base_publication_id,
                base_revision,
                delta,
            } => Self::Delta {
                base_publication_id,
                base_revision,
                delta,
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationEnvelopeV6 {
    pub key: MatchKey,
    pub publication_id: String,
    pub recipient_id: String,
    pub connection: Connection,
    pub stream_epoch: u32,
    pub revision: u64,
    pub payload: PublicationPayloadV6,
}
impl PublicationEnvelopeV6 {
    pub fn from_output(o: &Output) -> Option<Self> {
        if let OutputBody::PublicationV6 {
            recipient_id,
            connection,
            stream_epoch,
            revision,
            payload,
        } = &o.body
        {
            Some(Self {
                key: o.key.clone(),
                publication_id: o.id.clone(),
                recipient_id: recipient_id.clone(),
                connection: connection.clone(),
                stream_epoch: *stream_epoch,
                revision: *revision,
                payload: (**payload).clone(),
            })
        } else {
            None
        }
    }
}
