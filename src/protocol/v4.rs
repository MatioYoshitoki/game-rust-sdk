use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateRequest {
    Replicated(CreateV6),
    Members(CreateV5),
    Single(Create),
    Two(CreateV4),
}
impl From<Create> for CreateRequest {
    fn from(v: Create) -> Self {
        Self::Single(v)
    }
}
impl From<CreateV4> for CreateRequest {
    fn from(v: CreateV4) -> Self {
        Self::Two(v)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantSeat {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub weapon: Option<WeaponInstance>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub basic_profile: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub combat_build: Option<CombatBuild>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub skills: Option<SkillLoadout>,
    pub participant_id: String,
    pub authenticated_principal: String,
    pub connection: Connection,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateV4 {
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateV5 {
    #[serde(deserialize_with = "version_five")]
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
fn version_five<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<u32, D::Error> {
    let version = u32::deserialize(d)?;
    if version == 5 {
        Ok(version)
    } else {
        Err(serde::de::Error::custom("expected protocol 5"))
    }
}
impl From<CreateV5> for CreateRequest {
    fn from(v: CreateV5) -> Self {
        Self::Members(v)
    }
}
impl From<CreateV5> for CreateV4 {
    fn from(v: CreateV5) -> Self {
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BusinessReceiptRequest {
    Single(BusinessReceipt),
    Member(MemberBusinessReceipt),
}
impl From<BusinessReceipt> for BusinessReceiptRequest {
    fn from(v: BusinessReceipt) -> Self {
        Self::Single(v)
    }
}
impl From<MemberBusinessReceipt> for BusinessReceiptRequest {
    fn from(v: MemberBusinessReceipt) -> Self {
        Self::Member(v)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberBusinessReceipt {
    pub key: MatchKey,
    pub participant_id: String,
    pub operation_id: String,
    pub attempt_id: u32,
    pub outcome: ReceiptOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    pub kind: StatusEntityKind,
    pub id: String,
    pub generation: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantRef {
    pub entity: EntityRef,
    pub owner_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberPhase {
    Deploying,
    Active,
    Settling,
    Terminal,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PublicMemberView {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub extraction_channel: Option<ExtractionChannelView>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_seals: Option<Vec<[f64; 2]>>,
    pub entity: EntityRef,
    pub position: [f64; 2],
    pub hp: u32,
    pub max_hp: u32,
    pub phase: MemberPhase,
    pub protected: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemCooldownView {
    pub item_id: String,
    pub ready_at_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateMemberView {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub zone_exposure: Option<ZoneExposureView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub world_weapon_profile: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub training: Option<TrainingView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub mode: Option<ModeView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub boons: Option<BoonView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub checkpoint_progress: Option<Vec<CheckpointProgressView>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub equipment: Option<WeaponEquipmentView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub basic_charge: Option<BasicChargeView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub basic_charge_ready_at_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub basic_profile: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub basic_wave_phase: Option<BasicWavePhase>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub basic_projectile_progress: Option<BasicProjectileProgress>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub skill_powers: Option<std::collections::BTreeMap<String, u32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub skill_loadout_revision: Option<u32>,
    #[serde(
        default,
        deserialize_with = "optional_shield_total",
        skip_serializing_if = "Option::is_none"
    )]
    pub shield_total: Option<u32>,

    pub participant: ParticipantRef,
    pub position: [f64; 2],
    pub hp: u32,
    pub max_hp: u32,
    pub spirit: u32,
    pub max_spirit: u32,
    pub phase: MemberPhase,
    pub protected: bool,
    pub foreground: bool,
    pub ready: bool,
    pub effective_atk: u32,
    pub effective_def: u32,
    pub statuses: Vec<StatusView>,
    pub inventory: Vec<ItemAmount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<ItemInstance>,
    pub inventory_revision: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_cooldowns: Vec<ItemCooldownView>,
    pub player_skills: Vec<PlayerSkillView>,
    pub pending_action: Option<PlayerActionView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::present_value"
    )]
    pub active_channel: Option<ActiveChannelView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::present_value"
    )]
    pub projectile_recast: Option<ProjectileRecastView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::present_value"
    )]
    pub followup: Option<FollowupView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::present_value"
    )]
    pub recovery_cancel: Option<RecoveryCancelView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::present_value"
    )]
    pub basic_followup: Option<BasicFollowupView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::present_value"
    )]
    pub basic_pursuit: Option<BasicPursuitView>,
    pub basic_ready_at_ms: u64,
    pub recovery_until_ms: u64,
    pub extraction_finishes_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicChargeKind {
    StagedWave,
    Guard,
    Dash,
    Flight,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicChargeView {
    pub session_id: String,
    pub kind: BasicChargeKind,
    pub started_at_ms: u64,
    pub max_charge_at_ms: u64,
    pub tier: u8,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonsterViewV4 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub training_owner_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub boss_phase: Option<BossPhaseView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub summon: Option<SummonOwnershipView>,
    #[serde(
        default,
        deserialize_with = "optional_shield_total",
        skip_serializing_if = "Option::is_none"
    )]
    pub shield_total: Option<u32>,

    #[serde(
        default,
        deserialize_with = "optional_status_views",
        skip_serializing_if = "Option::is_none"
    )]
    pub statuses: Option<Vec<StatusView>>,
    pub entity: EntityRef,
    pub template_id: String,
    pub position: [f64; 2],
    pub hp: u32,
    pub max_hp: u32,
    pub radius: f64,
    pub returning: bool,
    pub target: Option<EntityRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummonOwnershipView {
    pub owner: EntityRef,
    pub expires_at_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceViewV4 {
    pub id: String,
    pub position: [f64; 2],
    pub quantity: u32,
    pub item_id: Option<String>,
    pub owner: Option<EntityRef>,
    pub expires_at_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub instance: Option<ItemInstance>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelicEntry {
    pub entry_id: u32,
    pub item_id: String,
    pub quantity: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub instance: Option<ItemInstance>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelicView {
    pub id: String,
    pub position: [f64; 2],
    pub deceased: EntityRef,
    pub revision: u32,
    pub remaining_stacks: usize,
    pub highest_quality: Option<u16>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerStateV6 {
    Ready,
    Reserved,
    Cooldown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerViewV6 {
    pub id: String,
    pub generation: u32,
    pub position: [f64; 2],
    pub state: ContainerStateV6,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectileViewV4 {
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_segment: Option<BasicProjectileSegment>,
    #[serde(
        default,
        deserialize_with = "optional_piercing",
        skip_serializing_if = "Option::is_none"
    )]
    pub piercing: Option<PiercingPlanViewV4>,
    #[serde(
        default,
        deserialize_with = "optional_unit_target",
        skip_serializing_if = "Option::is_none"
    )]
    pub unit_target: Option<UnitTarget>,
    pub id: String,
    pub source: EntityRef,
    pub position: [f64; 2],
    pub radius: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicProjectileProgress {
    pub stored: u32,
    pub maximum: u32,
    pub attack_speed_bonus_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicProjectileSegment {
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub tier: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub role: Option<BasicProjectileRole>,
    pub action_id: String,
    pub plan_id: String,
    pub index: u32,
    pub launched_at_ms: u64,
    pub aim_point: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicProjectileRole {
    SealOutbound,
    SealRecall,
    Ricochet,
    ReturnToOwner,
    ChargedFlight,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotV4 {
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub member_conflict: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub relics: Option<Vec<RelicView>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub containers: Option<Vec<ContainerViewV6>>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub active_blockers: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub checkpoint_zones: Option<Vec<CheckpointZoneView>>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_waves: Option<Vec<BasicWaveView>>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_impacts: Option<Vec<BasicImpactView>>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_beams: Option<Vec<BasicBeamView>>,
    #[serde(
        default,
        deserialize_with = "optional_areas",
        skip_serializing_if = "Option::is_none"
    )]
    pub areas: Option<Vec<AreaPlanViewV4>>,
    pub phase: Phase,
    pub logical_ms: u64,
    pub active_at_ms: Option<u64>,
    #[serde(rename = "self")]
    pub own: PrivateMemberView,
    pub members: Vec<PublicMemberView>,
    pub monsters: Vec<MonsterViewV4>,
    pub resources: Vec<ResourceViewV4>,
    pub projectiles: Vec<ProjectileViewV4>,
    pub extraction_point: [f64; 2],
    pub extraction_open: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointZoneView {
    pub schedule_id: String,
    pub window_index: u32,
    pub center: [f64; 2],
    pub radius: f64,
    pub opens_at_ms: u64,
    pub expires_at_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub avoid: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointProgressView {
    pub schedule_id: String,
    pub collected: u32,
    pub required: u32,
    pub total_windows: u32,
    pub last_window_index: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub counter: Option<CheckpointCounterView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointCounterView {
    pub window_index: u32,
    pub target: UnitTarget,
    pub opens_at_ms: u64,
    pub expires_at_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub on_contact: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeObjective {
    Wait,
    Checkpoint,
    ClearTargets,
    Survive,
    Waves,
    KillQuota,
    Contest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeWaveView {
    pub index: u8,
    pub total: u8,
    pub opens_at_ms: u64,
    pub opened: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeProgressView {
    pub current: u32,
    pub required: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterProgressView {
    pub initial: u32,
    pub remaining: u32,
    pub stats_bps: u32,
    pub surge_started: bool,
    #[serde(deserialize_with = "nullable_time")]
    pub surge_ends_at_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeView {
    pub stage_id: String,
    pub stage_index: u8,
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
    pub submitted: bool,
    pub respawns_left: u8,
    pub respawn_at_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub forfeit_at_ms: Option<u64>,
    pub outcome: Option<Outcome>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingView {
    #[serde(deserialize_with = "nullable_time")]
    pub respawn_at_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub metrics: Option<TrainingMetricsView>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingMetricsView {
    pub sampled_at_ms: u64,
    pub damage: u64,
    pub hits: u64,
    pub critical_hits: u64,
    pub highest_hit: u32,
    pub rolling_dps: u64,
    pub average_dps: u64,
    pub by_source: TrainingDamageBySource,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingDamageBySource {
    pub basic: u64,
    pub skill: u64,
    pub periodic: u64,
    pub summon: u64,
    pub other: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionPressurePhase {
    Exploring,
    Grace,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionWarningView {
    pub id: String,
    pub index: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionPressureView {
    pub grace_at_ms: u64,
    pub ends_at_ms: u64,
    pub phase: ExtractionPressurePhase,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub warning: Option<ExtractionWarningView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionChannelView {
    pub position: [f64; 2],
    pub started_at_ms: u64,
    pub ends_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionChannelState {
    Started,
    Interrupted,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionChannelChange {
    pub entity: EntityRef,
    pub channel: ExtractionChannelView,
    pub state: ExtractionChannelState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventV4 {
    TrainingTargetsSpawned {
        participant_id: String,
        ids: Vec<String>,
    },
    TrainingTargetsDespawning {
        ids: Vec<String>,
    },
    ExtractionChannelChanged(ExtractionChannelChange),
    DeadlineDamage {
        target: EntityRef,
        amount: u32,
        absorbed: u32,
        hp: u32,
    },
    ModeStageOpened {
        stage_id: String,
        stage_index: u8,
        opens_at_ms: u64,
        expires_at_ms: u64,
    },
    ModeRespawned {
        entity: EntityRef,
    },
    ModeConcluded {
        outcome: Outcome,
        result: MatchResult,
    },
    ResourceUpdated {
        resource: ResourceViewV4,
        participant: EntityRef,
    },
    ContainerReserved {
        container_id: String,
        generation: u32,
        finishes_at_ms: u64,
    },
    ContainerOpened {
        container_id: String,
        generation: u32,
    },
    ContainerReleased {
        container_id: String,
        generation: u32,
    },
    BossPhaseChanged {
        entity: EntityRef,
        from_index: u32,
        phase: BossPhaseView,
    },
    BossReset {
        entity: EntityRef,
        phase: BossPhaseView,
    },
    NpcSkillCommitted {
        source: EntityRef,
        target: EntityRef,
        skill_id: String,
    },
    NpcHealTick {
        source: EntityRef,
        target: EntityRef,
        skill_id: String,
        amount: u32,
        hp: u32,
    },
    SummonSkillCopied {
        source: EntityRef,
        skill_id: String,
        target: EntityRef,
    },
    BasicBeamStarted {
        beam: BasicBeamView,
    },
    BasicBeamPulse {
        action_id: String,
        source: EntityRef,
        origin: [f64; 2],
        direction: [f64; 2],
        tick: u32,
        at_ms: u64,
    },
    BasicBeamContact {
        action_id: String,
        source: EntityRef,
        target: EntityRef,
        tick: u32,
        at_ms: u64,
    },
    BasicBeamEnded {
        action_id: String,
        source: EntityRef,
        reason: BasicBeamEndReason,
        ticks: u32,
    },
    BasicImpactStarted {
        impact: BasicImpactView,
    },
    BasicImpactContact {
        impact_id: String,
        source: EntityRef,
        target: EntityRef,
        scheduled_at_ms: u64,
        #[serde(
            default,
            deserialize_with = "present_value",
            skip_serializing_if = "Option::is_none"
        )]
        segment: Option<BasicImpactSegment>,
    },
    BasicImpactEnded {
        impact_id: String,
        reason: BasicImpactEndReason,
        #[serde(
            default,
            deserialize_with = "present_value",
            skip_serializing_if = "Option::is_none"
        )]
        segments: Option<Vec<BasicImpactSegment>>,
    },
    BasicWaveStarted {
        wave: BasicWaveView,
    },
    BasicWaveContact {
        wave_id: String,
        source: EntityRef,
        target: EntityRef,
        scheduled_at_ms: u64,
    },
    BasicWaveEnded {
        wave_id: String,
        reason: BasicWaveEndReason,
    },
    AreaStarted {
        area: AreaPlanViewV4,
    },
    AnchorRelocated {
        area_id: String,
        action_id: String,
        source: EntityRef,
        recast_index: u32,
        center: [f64; 2],
    },
    AnchorRecastEnded {
        area_id: String,
        action_id: String,
        source: EntityRef,
        recast_index: u32,
    },
    AreaEntered {
        area_id: String,
        source: EntityRef,
        target: EntityRef,
        at_ms: u64,
    },
    ForcedDisplacement {
        area_id: String,
        source: EntityRef,
        target: EntityRef,
        from: [f64; 2],
        to: [f64; 2],
        slide_speed_multiplier: f64,
    },
    AreaTick {
        area_id: String,
        tick_index: u32,
        scheduled_at_ms: u64,
        targets: Vec<EntityRef>,
    },
    AreaEnded {
        area_id: String,
        reason: AreaEndReason,
    },
    PublicationExpired {
        stream_epoch: u32,
        publication_id: Option<String>,
    },
    Activated {
        active_at_ms: u64,
    },
    Private {
        event: Event,
    },
    Damage {
        source: EntityRef,
        target: EntityRef,
        amount: u32,
        hp: u32,
    },
    TerrainDamage {
        terrain: String,
        target: EntityRef,
        amount: u32,
        absorbed: u32,
        hp: u32,
    },
    MonsterDefeated {
        entity: EntityRef,
        credited_to: EntityRef,
    },
    ResourceSpawned {
        resource: ResourceViewV4,
    },
    ResourceTaken {
        resource_id: String,
        participant: EntityRef,
    },
    ProjectileStarted {
        projectile: ProjectileViewV4,
    },
    ProjectileContact {
        projectile_id: String,
        source: EntityRef,
        target: EntityRef,
        position: [f64; 2],
        hit_at_ms: u64,
        remaining_hits: u32,
    },
    ProjectileEnded {
        #[serde(
            default,
            deserialize_with = "optional_projectile_reason",
            skip_serializing_if = "Option::is_none"
        )]
        reason: Option<ProjectileEndReasonV4>,
        projectile_id: String,
    },
    BasicFlightContact {
        projectile_id: String,
        source: EntityRef,
        target: EntityRef,
        position: [f64; 2],
        hit_at_ms: u64,
        target_hits: u32,
    },
    BasicFlightBurst {
        projectile_id: String,
        source: EntityRef,
        center: [f64; 2],
        radius: f64,
        targets: Vec<EntityRef>,
    },
    BasicAttackSwordLanded {
        source: EntityRef,
        action_id: String,
        position: [f64; 2],
        stored: u32,
    },
    MemberChanged {
        entity: EntityRef,
        phase: MemberPhase,
    },
    Closed {
        live_worlds: u32,
        pending_operations: u32,
    },
    Fault {
        code: Error,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicBeamView {
    pub action_id: String,
    pub plan_id: String,
    pub source: EntityRef,
    pub direction: [f64; 2],
    pub range: f64,
    pub half_width: f64,
    pub interval_ms: u64,
    pub started_at_ms: u64,
    pub next_tick_at_ms: u64,
    pub ticks: u32,
    pub released: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicBeamEndReason {
    EquipmentChanged,
    Released,
    Cancelled,
    Interrupted,
    Protected,
    SourceInvalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicImpactSegmentRole {
    Primary,
    Secondary,
    Return,
    Blast,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicImpactSegment {
    pub index: u8,
    pub role: BasicImpactSegmentRole,
    pub origin: [f64; 2],
    pub target: UnitTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicImpactKind {
    Hitscan,
    FixedDelay,
    GroundField,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicImpactEndReason {
    Resolved,
    SourceInvalid,
    TargetInvalid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicImpactView {
    pub id: String,
    pub plan_id: String,
    pub source: EntityRef,
    pub kind: BasicImpactKind,
    pub origin: [f64; 2],
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub target: Option<UnitTarget>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub position: Option<[f64; 2]>,
    pub released_at_ms: u64,
    pub impact_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BasicWaveGeometry {
    Directional {
        direction: [f64; 2],
        cone_degrees: f64,
    },
    Radial {},
    Landing {
        from: [f64; 2],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicWaveEndReason {
    Resolved,
    SourceInvalid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicWaveTargetView {
    pub target: UnitTarget,
    pub at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicWavePhase {
    Primary,
    Alternate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicWaveView {
    pub id: String,
    pub plan_id: String,
    pub source: EntityRef,
    pub origin: [f64; 2],
    pub geometry: BasicWaveGeometry,
    pub range: f64,
    pub speed: f64,
    pub released_at_ms: u64,
    pub pending: Vec<BasicWaveTargetView>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberCheckpoint {
    pub participant_id: String,
    pub checkpoint: IoCheckpoint,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IoStateV4 {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapon_operations: Vec<WeaponEquipmentOperation>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub skill_control: Option<SkillControlCheckpoint>,
    pub schema_version: u32,
    pub identity_sha256: String,
    pub logical_ms: u64,
    pub output_sequence: u64,
    pub recovery_generation: u32,
    pub members: Vec<MemberCheckpoint>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IoCheckpointV4 {
    pub state: IoStateV4,
    pub sha256: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryV4 {
    pub generation: u32,
    pub checkpoint: IoCheckpointV4,
}

fn optional_status_views<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Vec<StatusView>>, D::Error> {
    Vec::<StatusView>::deserialize(d).map(Some)
}

fn optional_shield_total<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<u32>, D::Error> {
    u32::deserialize(d).map(Some)
}

fn optional_unit_target<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<UnitTarget>, D::Error> {
    UnitTarget::deserialize(d).map(Some)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreaPlanKind {
    Circle,
    Field,
    LineField,
    BasicField,
    MovingEnter,
    NpcGroundField,
    NpcGroundStrike,
    MemberGroundTrap,
    MemberCastAnchor,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreaEndReason {
    Resolved,
    Expired,
    HitBudgetExhausted,
    SourceInvalid,
    Replaced,
    Cancelled,
    AnchorInvalid,
    RecastConsumed,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaPlanViewV4 {
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub attached_anchor_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub anchor_recast: Option<AnchorRecastView>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub movement: Option<AreaMovementView>,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_action_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "optional_line",
        skip_serializing_if = "Option::is_none"
    )]
    pub line: Option<LineFieldView>,
    pub id: String,
    pub source: EntityRef,
    pub skill_id: String,
    pub mechanism: AreaPlanKind,
    pub center: [f64; 2],
    pub radius: f64,
    pub created_at_ms: u64,
    pub activates_at_ms: u64,
    pub expires_at_ms: u64,
    pub interval_ms: u64,
    pub next_tick_at_ms: Option<u64>,
    pub completed_ticks: u32,
    pub total_ticks: u32,
    pub contact_policy: Option<FieldContactPolicyView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorRecastView {
    pub action_id: String,
    pub expires_at_ms: u64,
    pub max_count: u32,
    pub used_count: u32,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub behaviors: Option<Vec<AnchorRecastBehavior>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorRecastBehavior {
    Relocate,
    ReturnToSource,
    End,
}

impl AnchorRecastBehavior {
    pub(crate) fn valid_set(behaviors: &[Self]) -> bool {
        (1..=3).contains(&behaviors.len())
            && behaviors
                .iter()
                .enumerate()
                .all(|(i, behavior)| !behaviors[..i].contains(behavior))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaMovementView {
    pub origin: [f64; 2],
    pub direction: [f64; 2],
    pub speed: f64,
    pub distance: f64,
    pub committed: bool,
}
fn optional_areas<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Vec<AreaPlanViewV4>>, D::Error> {
    Vec::<AreaPlanViewV4>::deserialize(d).map(Some)
}

fn optional_line<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<LineFieldView>, D::Error> {
    LineFieldView::deserialize(d).map(Some)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PiercingPlanViewV4 {
    pub skill_id: String,
    pub direction: [f64; 2],
    pub speed: f64,
    pub remaining_range: f64,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub max_hits: u32,
    pub used_hits: u32,
    pub remaining_hits: u32,
    pub repeat_interval_ms: u64,
    pub contacts: Vec<ProjectileContactViewV4>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectileContactViewV4 {
    pub target: UnitTarget,
    pub hit_at_ms: u64,
    #[serde(deserialize_with = "nullable_time")]
    pub next_eligible_at_ms: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileEndReasonV4 {
    Wall,
    RangeExhausted,
    Expired,
    HitBudgetExhausted,
    SourceInvalid,
    TargetInvalid,
}
fn optional_piercing<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<PiercingPlanViewV4>, D::Error> {
    PiercingPlanViewV4::deserialize(d).map(Some)
}
fn optional_projectile_reason<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<ProjectileEndReasonV4>, D::Error> {
    ProjectileEndReasonV4::deserialize(d).map(Some)
}
fn nullable_time<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<u64>, D::Error> {
    Option::<u64>::deserialize(d)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldEquipmentView {
    pub id: String,
    pub position: [f64; 2],
    pub revision: u32,
    pub state: WorldEquipmentState,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub holder: Option<EntityRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldEquipmentState {
    Available,
    Held,
    Dropped,
    Secured,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExposureNodeChoice {
    Reduce,
    Boost,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneExposureView {
    pub band_id: String,
    pub stacks: u8,
    pub max_stacks: u8,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub boost_ends_at_ms: Option<u64>,
    pub nodes: Vec<ExposureNodeView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExposureNodeView {
    pub id: String,
    pub position: [f64; 2],
    pub revision: u32,
    pub ready_at_ms: u64,
}
