use serde::{Deserialize, Serialize};
mod assets;
mod boons;
pub use boons::*;
mod character_build;
pub use character_build::*;
mod skill_loadouts;
pub use skill_loadouts::*;
mod training_tools;
pub use training_tools::*;
mod training_spawn;
pub use training_spawn::*;
mod weapon_equipment;
pub use weapon_equipment::*;
mod status;
mod v4;
mod v6;
pub use assets::*;
pub use status::*;
pub use v4::*;
pub use v6::*;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Error {
    InvalidMessage,
    VersionMismatch,
    ConfigMismatch,
    Unsupported,
    StaleMatch,
    StaleConnection,
    StalePublication,
    Unauthorized,
    NotReady,
    InvalidState,
    InvalidTarget,
    OutOfRange,
    NoResource,
    GuardianPending,
    GuardianAlive,
    EncounterLocked,
    Capacity,
    Immune,
    RevisionConflict,
    QueueFull,
    ExternalRejected,
    ExternalUnknown,
    Closed,
    Faulted,
    Conflict,
    StaleSequence,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchKey {
    pub id: String,
    pub incarnation: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub generation: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub protocol_version: u32,
    pub key: MatchKey,
    pub request_id: String,
    pub config_id: String,
    pub config_hash: String,
    pub seed: u32,
    pub participant_id: String,
    pub authenticated_principal: String,
    pub connection: Connection,
    #[serde(default)]
    pub publication_mode: PublicationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Recovery>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenOperation {
    pub request: BusinessRequest,
    pub terminal: Option<ReceiptOutcome>,
    pub refused: bool,
    pub queried_attempt: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IoState {
    pub schema_version: u32,
    pub identity_sha256: String,
    pub logical_ms: u64,
    pub input_sequence: u32,
    pub output_sequence: u64,
    pub recovery_generation: u32,
    pub operations: Vec<FrozenOperation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IoCheckpoint {
    pub state: IoState,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recovery {
    pub generation: u32,
    pub checkpoint: IoCheckpoint,
}

/// Fixed for the lifetime of a match; affects output cadence, never simulation time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationMode {
    #[default]
    Batched,
    Interactive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Preparing,
    Deploying,
    Active,
    Draining,
    Closed,
    Faulted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub key: MatchKey,
    pub participant_id: String,
    pub entity_generation: u32,
    pub connection: Connection,
    pub request_id: String,
    pub sequence: u32,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Trusted host lifecycle input; never accepted as a browser gameplay action.
    Reconnect {
        previous_connection: Connection,
        authenticated_principal: String,
    },
    RequestFull {
        stream_epoch: u32,
    },
    ConnectionLost {},
    Join {},
    CombatReady {
        publication_id: String,
    },
    Move {
        direction: [f64; 2],
    },
    Stop {},
    Navigate {
        destination: [f64; 2],
    },
    Attack {},
    UseExposureNode {
        node_id: String,
        expected_revision: u32,
        choice: ExposureNodeChoice,
    },
    ClaimWorldEquipment {
        node_id: String,
        expected_revision: u32,
    },
    CompleteModeCheckpoint {
        stage_id: String,
    },
    SetOwnedTargets {
        preset_id: String,
    },
    ClearOwnedTargets {},
    ResetTrainingMetrics {},
    ResetTrainingCooldowns(crate::protocol::training_tools::ResetTrainingCooldowns),
    RestoreTrainingMember(crate::protocol::training_tools::RestoreTrainingMember),
    AttackUnit {
        target: UnitTarget,
    },
    AttackGround {
        position: [f64; 2],
    },
    StartBasicCharge {},
    ReleaseBasicCharge {
        session_id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        target: Option<UnitTarget>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        direction: Option<[f64; 2]>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        position: Option<[f64; 2]>,
    },
    CancelBasicCharge {
        session_id: String,
    },
    StartAttackBeam {
        direction: [f64; 2],
    },
    AimAttackBeam {
        action_id: String,
        direction: [f64; 2],
    },
    EndAttackBeam {
        action_id: String,
    },
    Cast {
        skill_id: String,
        direction: [f64; 2],
    },
    RecastProjectile {
        action_id: String,
        recast_index: u32,
        direction: [f64; 2],
    },
    StartCharge {
        skill_id: String,
        direction: [f64; 2],
    },
    UpdateChargeAim {
        action_id: String,
        direction: [f64; 2],
    },
    ReleaseCharge {
        action_id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        direction: Option<[f64; 2]>,
    },
    CastSelf {
        skill_id: String,
    },
    CastUnit {
        skill_id: String,
        target: UnitTarget,
    },
    CastGround {
        skill_id: String,
        position: [f64; 2],
    },
    CastAtAnchor {
        skill_id: String,
        anchor_id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        direction: Option<[f64; 2]>,
    },
    RecastAnchor {
        anchor_id: String,
        action_id: String,
        recast_index: u32,
        position: [f64; 2],
    },
    ReturnAnchor {
        anchor_id: String,
        action_id: String,
        recast_index: u32,
    },
    EndAnchor {
        anchor_id: String,
        action_id: String,
        recast_index: u32,
    },
    CancelAction {},
    Pickup {
        resource_id: String,
        expected_revision: u32,
    },
    ClaimBoon {
        opportunity_id: String,
        boon_id: String,
        expected_revision: u32,
        expected_inventory_revision: u32,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        replace_slot: Option<u8>,
    },
    CounterCheckpoint {
        schedule_id: String,
        window_index: u32,
        target: UnitTarget,
    },
    OpenRelic {
        relic_id: String,
    },
    ClaimRelic {
        relic_id: String,
        expected_revision: u32,
        expected_inventory_revision: u32,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        entry_id: Option<u32>,
    },
    DropItem {
        item_id: String,
        quantity: u32,
        expected_revision: u32,
    },
    UseItem {
        item_id: String,
        expected_revision: u32,
    },
    AttuneObjective {
        objective_id: String,
        expected_revision: u32,
    },
    CancelAttunement {},
    OpenChest {
        chest_id: String,
        expected_generation: u32,
    },
    CancelChestUnlock {
        chest_id: String,
        expected_generation: u32,
    },
    ExtractStart {},
    ExtractCancel {},
    Background {},
    Foreground {},
    Leave {},
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantFacts {
    pub principal: String,
    pub revision: u32,
    pub hp_points: u32,
    pub spirit_points: u32,
    pub atk_points: u32,
    pub def_points: u32,
    pub spd_points: u32,
    pub warehouse_quantity: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Extraction,
    Death,
    Win,
    Loss,
    Draw,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Business {
    LoadParticipant {},
    LoadAssets {},
    CommitAssets {
        transaction: AssetTransaction,
    },
    CommitAdmission {
        expected_revision: u32,
        item_id: String,
        quantity: u32,
        entry_years: u32,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        clock: Option<EntryClock>,
    },
    CommitSettlement {
        outcome: Outcome,
        expected_revision: u32,
        item_id: String,
        carried: u32,
        looted: u32,
        kept: u32,
        dropped: u32,
        cultivation: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scoring: Option<ScoreSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map_progress: Option<MapProgressSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reset_progress: Option<ResetProgressSummary>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        items: Vec<ItemSettlement>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        instances: Vec<ItemInstance>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rewards: Vec<RewardBalance>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        progression: Option<ProgressionSettlement>,
    },
    CompensateAdmission {
        original_operation_id: String,
        item_id: String,
        quantity: u32,
        entry_years: u32,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        clock: Option<EntryClock>,
    },
    AppendFacts {
        settlement_operation_id: String,
        result_id: String,
        fact_ids: Vec<String>,
    },
    QueryOperation {
        operation_id: String,
    },
    ReconcileOperation {
        operation_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusinessRequest {
    pub operation_id: String,
    pub attempt_id: u32,
    pub body: Business,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReceiptOutcome {
    Loaded { facts: ParticipantFacts },
    AssetsLoaded { facts: AssetParticipantFacts },
    Committed { revision: u32, commit_id: String },
    FactsAppended { commit_id: String },
    Refused {},
    NotCommitted {},
    Unknown {},
    Reconciled { revision: u32, commit_id: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusinessReceipt {
    pub key: MatchKey,
    pub operation_id: String,
    pub attempt_id: u32,
    pub outcome: ReceiptOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportOutcome {
    Enqueued,
    Sent,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportReceipt {
    pub key: MatchKey,
    pub output_id: String,
    pub recipient_id: String,
    pub connection: Connection,
    pub outcome: TransportOutcome,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Close {
    pub key: MatchKey,
    pub request_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub phase: Phase,
    pub logical_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSettlement {
    pub item_id: String,
    pub looted: u32,
    pub kept: u32,
    pub dropped: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateKills {
    pub template_id: String,
    pub kills: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreSummary {
    pub kills: u32,
    pub credited_kills: u32,
    pub kill_score: u32,
    pub templates: Vec<TemplateKills>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoreView {
    pub kills: u32,
    pub credited_kills: u32,
    pub kill_score: u32,
    pub extraction_cultivation: u32,
    pub death_cultivation: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PveView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piercing_projectiles: Option<Vec<PiercingProjectileView>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_fields: Option<Vec<PlayerFieldView>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_area_warnings: Option<Vec<PlayerAreaWarningView>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timed_position: Option<TimedPositionView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<CounterView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_objective: Option<MapObjectiveView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring: Option<ScoreView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub encounters: Vec<EncounterView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chests: Vec<ChestView>,
    pub monsters: Vec<MonsterView>,
    pub inventory: Vec<(String, u32)>,
    pub resource_items: Vec<(String, String)>,
    pub kills: u32,
    pub sword_marks: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimedPositionView {
    pub id: String,
    pub name: String,
    pub encounter_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_target_id: Option<String>,
    pub warning_bonus_active: bool,
    pub failure_scale_active: bool,
    pub successes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stabilized_sites: Option<u8>,
    pub warning: Option<TimedPositionWarningView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimedPositionWarningView {
    pub round: u32,
    pub cycle: u32,
    pub started_at_ms: u64,
    pub resolves_at_ms: u64,
    pub failure_raw_damage: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub miss_heal: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_confirmed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<TimedPositionSite>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedPositionSite {
    pub id: String,
    pub name: String,
    pub position: [f64; 2],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChestView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<EncounterChestView>,
    pub id: String,
    pub type_id: String,
    pub position: [f64; 2],
    pub generation: u32,
    pub opened: bool,
    pub respawn_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seal: Option<SealView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardians: Option<GuardianView>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuardianBinding {
    pub chest_id: String,
    pub generation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterPhase {
    Dormant,
    Waiting,
    Spawning,
    Active,
    Completed,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset: Option<EncounterResetView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wave: Option<EncounterWaveView>,
    pub id: String,
    pub phase: EncounterPhase,
    pub monster_ids: Vec<String>,
    pub required_monster_ids: Vec<String>,
    pub defeated_required: u32,
    pub reward_chest_ids: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterWaveView {
    pub id: String,
    pub index: u32,
    pub total: u32,
    pub completed_waves: u32,
    pub ready_at_ms: Option<u64>,
    pub monster_ids: Vec<String>,
    pub required_monster_ids: Vec<String>,
    pub defeated_required: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterChestView {
    pub encounter_id: String,
    pub completed: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuardianView {
    pub pending: bool,
    pub planned_count: Option<u32>,
    pub alive: u32,
    pub monster_ids: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SealView {
    pub layers: u32,
    pub completed_layers: u32,
    pub layer_progress_ms: u64,
    pub layer_duration_ms: u64,
    pub unlocking: bool,
    pub interruption_reason: Option<ChestInterruptReason>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChestInterruptReason {
    Cancelled,
    Movement,
    Damage,
    Background,
    Closed,
    OutOfRange,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonsterView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_target: Option<UnitTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield_total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_atk: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healing_received_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<StatusView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_def: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter_round: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heavy: Option<HeavyView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support: Option<SupportView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area_warning: Option<AreaWarningView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kill_score: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss: Option<BossPhaseView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wave_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardian: Option<GuardianBinding>,
    pub id: String,
    pub position: [f64; 2],
    pub hp: u32,
    pub max_hp: u32,
    pub spirit: u32,
    pub state: String,
    pub cast: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaWarningView {
    pub cast_id: u32,
    pub skill_id: String,
    pub name: String,
    pub center: [f64; 2],
    pub radius: f64,
    pub started_at_ms: u64,
    pub resolves_at_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaCastIdentity {
    pub monster_id: String,
    pub cast_id: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreaCancelReason {
    Background,
    Returning,
    OwnerDied,
    Closed,
    Replaced,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BossPhaseView {
    pub phase_id: String,
    pub phase_name: String,
    pub phase_index: u32,
    pub phase_count: u32,
    pub epoch: u32,
    pub attack_interval_ms: u64,
    pub cast_chance: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetView {
    pub id: String,
    pub position: [f64; 2],
    pub hp: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceView {
    pub id: String,
    pub position: [f64; 2],
    pub quantity: u32,
    pub orb: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield_total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_atk: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healing_received_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<StatusView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_def: Option<u32>,
    #[serde(default)]
    pub max_hp: u32,
    #[serde(default)]
    pub max_spirit: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pve: Option<PveView>,
    pub phase: Phase,
    pub logical_ms: u64,
    pub active_at_ms: Option<u64>,
    pub position: [f64; 2],
    pub hp: u32,
    pub spirit: u32,
    pub protected: bool,
    pub basic_ready_at_ms: u64,
    pub skill_ready_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_skills: Option<Vec<PlayerSkillView>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_action: Option<PlayerActionView>,
    pub recovery_until_ms: u64,
    pub inventory_quantity: u32,
    pub inventory_revision: u32,
    pub target: Option<TargetView>,
    pub resources: Vec<ResourceView>,
    pub extraction_open: bool,
    pub extraction_finishes_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    TrainingCooldownsReset {
        participant_id: String,
    },
    TrainingMemberRestored {
        participant_id: String,
        hp: u32,
        spirit: u32,
        cleansed: Vec<String>,
    },
    BoonClaimStarted {
        claim: BoonPendingClaimView,
    },
    BoonClaimInterrupted {
        opportunity_id: String,
        reason: Error,
    },
    BoonClaimed {
        opportunity_id: String,
        slot: BoonSlotView,
        revision: u32,
    },
    RelicOpened {
        relic_id: String,
        revision: u32,
        contents: Vec<RelicEntry>,
    },
    RelicClaimed {
        relic_id: String,
        revision: u32,
        items: Vec<RelicEntry>,
    },
    CheckpointReached {
        progress: CheckpointProgressView,
        at_ms: u64,
    },
    CheckpointAvoided {
        progress: CheckpointProgressView,
        at_ms: u64,
    },
    CheckpointCounterResolved {
        schedule_id: String,
        window_index: u32,
        target: EntityRef,
        raw_damage: u32,
        absorbed: u32,
        hp_damage: u32,
    },
    SkillLoadoutChanged {
        revision: u32,
        equipped_skill_ids: Vec<String>,
    },
    WeaponEquipmentChanged {
        equipment: WeaponEquipmentView,
        #[serde(skip_serializing_if = "Option::is_none")]
        basic_profile: Option<String>,
    },
    WindowRecordEnded {
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        reason: StatusRemovalReason,
        recorded: u64,
        contacts: u32,
        raw_hp: u32,
        hp_restored: u32,
        raw_spirit: u32,
        spirit_restored: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppressed: Option<Error>,
    },
    CombatActionTotal {
        action_id: String,
        action_source: StatusEntity,
        trigger_id: String,
        source: StatusEntity,
        other: StatusEntity,
        total_amount: u64,
        contacts: u32,
        fired: bool,
    },
    StatusTransferred {
        previous_target: StatusEntity,
        status: StatusView,
    },
    DeathGuardActivated {
        source: StatusEntity,
        target: StatusEntity,
        prevented_hp_loss: u32,
        hp: u32,
    },
    TerrainDeathGuardActivated {
        target: StatusEntity,
        prevented_hp_loss: u32,
        hp: u32,
    },
    CombatTriggerFired {
        trigger_id: String,
        source: StatusEntity,
    },
    CombatTriggerSuppressed {
        trigger_id: String,
        source: StatusEntity,
        reason: Error,
    },
    HealResolved {
        source: StatusEntity,
        target: StatusEntity,
        heal_kind: HealKind,
        raw_heal: u32,
        multiplier_bps: u32,
        adjusted_heal: u64,
        hp_restored: u32,
    },
    PeriodicHealTick {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        stacks: Option<u32>,
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        tick_index: u32,
        scheduled_at_ms: u64,
        executed_at_ms: u64,
        raw_heal: u32,
        hp_restored: u32,
        outcome: PeriodicTickOutcome,
    },
    PeriodicTick {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        stacks: Option<u32>,
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        tick_index: u32,
        scheduled_at_ms: u64,
        executed_at_ms: u64,
        raw_damage: u32,
        absorbed: u32,
        hp_damage: u32,
        outcome: PeriodicTickOutcome,
    },
    ShieldGranted {
        source: StatusEntity,
        target: StatusEntity,
        definition_id: String,
        instance_id: Option<String>,
        requested: u32,
        granted: u32,
        remaining: u32,
    },
    ShieldAbsorbed {
        source: String,
        target: StatusEntity,
        amount: u32,
        consumptions: Vec<ShieldConsumption>,
    },
    StatusApplied {
        status: StatusView,
    },
    StatusRefreshed {
        status: StatusView,
    },
    StatusStacked {
        status: StatusView,
        added_stacks: u32,
    },
    StatusIgnored {
        instance_id: String,
    },
    StatusRejected {
        source: StatusEntity,
        target: StatusEntity,
        definition_ids: Vec<String>,
        reason: Error,
    },
    StatusRemoved {
        instance_ids: Vec<String>,
        reason: StatusRemovalReason,
    },
    ActionRejected {
        action_id: String,
        reason: Error,
    },
    TimedPositionStarted {
        mechanic_id: String,
        warning: TimedPositionWarningView,
    },
    TimedPositionResolved {
        mechanic_id: String,
        round: u32,
        cycle: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        site_id: Option<String>,
        target_id: Option<String>,
        damage: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spirit_restored: Option<u32>,
    },
    TimedPositionCancelled {
        mechanic_id: String,
        round: u32,
        cycle: u32,
    },
    TimedHitTriggered {
        mechanic_id: String,
        round: u32,
        cycle: u32,
        target_id: String,
        damage: u32,
    },
    TimedHitMissed {
        mechanic_id: String,
        round: u32,
        cycle: u32,
        target_id: String,
        healed: u32,
    },
    RecoveryRestored {
        generation: u32,
        compensating_entry: bool,
    },
    IoTimedOut {
        operation_id: String,
        attempt_id: u32,
    },
    IoDrainExpired {},
    ConnectionChanged {
        generation: u32,
    },
    ConnectionLost {
        generation: u32,
        resume_until_ms: Option<u64>,
    },
    ConnectionExpired {
        generation: u32,
    },
    Joined,
    Activated,
    PiercingStarted {
        projectile: PiercingProjectileView,
    },
    PiercingHit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hit_at_ms: Option<u64>,
        action_id: String,
        skill_id: String,
        target: UnitTarget,
        position: [f64; 2],
        remaining_hits: u32,
    },
    PiercingEnded {
        action_ids: Vec<String>,
        reason: PiercingEndReason,
    },
    PlayerFieldStarted {
        field: PlayerFieldView,
    },
    PlayerFieldTick {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hit_targets: Option<Vec<UnitTarget>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remaining_hits: Option<u32>,
        action_id: String,
        skill_id: String,
        tick_index: u32,
        scheduled_at_ms: u64,
        hits: u32,
    },
    PlayerFieldEnded {
        action_ids: Vec<String>,
        reason: PlayerFieldEndReason,
    },
    PlayerAreaStarted {
        warning: PlayerAreaWarningView,
    },
    PlayerAreaResolved {
        action_id: String,
        skill_id: String,
        hits: u32,
    },
    PlayerAreaCancelled {
        action_ids: Vec<String>,
        reason: PlayerAreaCancelReason,
    },
    StatusConsumed {
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        consumed_stacks: u32,
        remaining_stacks: u32,
    },
    FinisherResolved {
        trigger_id: String,
        source: StatusEntity,
        target: StatusEntity,
        mode: FinisherMode,
        raw_damage: u32,
        hp_damage: u32,
        absorbed: u32,
        lethal: bool,
    },
    ContactReactionResolved {
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        provoker: StatusEntity,
        raw_damage: u32,
        hp_damage: u32,
        absorbed: u32,
    },
    SkillCooldownsReduced {
        at_ms: u64,
        trigger_id: String,
        source: StatusEntity,
        changes: Vec<SkillCooldownChange>,
    },
    ChainDamageResolved {
        trigger_id: String,
        source: StatusEntity,
        origin: StatusEntity,
        initial_raw: u32,
        hops: Vec<ChainHopView>,
        reason: ChainEndReason,
    },
    PeriodicSpiritTick {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        stacks: Option<u32>,
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        tick_index: u32,
        scheduled_at_ms: u64,
        executed_at_ms: u64,
        raw_spirit: u32,
        base_restored: u32,
        hp_excess: u64,
        conversion_bps: u32,
        conversion_eligible: bool,
        converted_spirit: u32,
        spent_heal: u64,
        remaining_excess: u64,
        outcome: PeriodicTickOutcome,
    },
    StatusCleanseResolved {
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        phase: StatusCleansePhase,
        #[serde(skip_serializing_if = "Option::is_none")]
        tick_index: Option<u32>,
        scheduled_at_ms: u64,
        executed_at_ms: u64,
        removed_count: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        suppressed: Option<Error>,
    },
    StatusEndResolved {
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        reason: StatusRemovalReason,
        active_definition_ids: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        failure: Option<Error>,
    },
    StatusEndsSuppressed {
        at_ms: u64,
        count: u32,
    },
    ShieldRecordEnded {
        instance_id: String,
        definition_id: String,
        source: StatusEntity,
        target: StatusEntity,
        reason: StatusRemovalReason,
        recorded: u32,
        raw_counter: u32,
        targets_hit: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppressed: Option<Error>,
    },
    ActionCommitted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ground_position: Option<[f64; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit_target: Option<UnitTarget>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skill_id: Option<String>,
        action_id: String,
        skill: bool,
        spirit_cost: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hp_cost: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charge: Option<ChargeCommitView>,
    },
    ActionInterrupted,
    ProjectileRecast {
        action_id: String,
        projectile_id: String,
        recast_index: u32,
        direction: [f64; 2],
        spirit_cost: u32,
    },
    ChannelPulse {
        action_id: String,
        pulse: u32,
        spirit_cost: u32,
        projectile_id: String,
    },
    CastInterceptionResolved {
        source: EntityRef,
        target: EntityRef,
        skill_id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        action_id: Option<String>,
        mark_instance_id: String,
        mark_definition_id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        followup_status_id: Option<String>,
        outcome: CastInterceptionOutcome,
    },
    PaidSpiritRestored {
        source: EntityRef,
        trigger_id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        action_id: Option<String>,
        paid_spirit: u32,
        requested: u32,
        restored: u32,
        excess: u32,
    },
    ResourceStolen {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        action_id: Option<String>,
        source: EntityRef,
        target: EntityRef,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_value"
        )]
        hp_policy: Option<ResourceHpPolicy>,
        requested_hp: u32,
        requested_spirit: u32,
        hp_removed: u32,
        spirit_removed: u32,
        hp_multiplier_bps: u32,
        adjusted_hp_return: u64,
        hp_restored: u32,
        spirit_restored: u32,
        hp_excess: u64,
        spirit_excess: u32,
    },
    UnitChannelPulse {
        action_id: String,
        target: EntityRef,
        pulse: u32,
        spirit_cost: u32,
        raw_damage: u32,
        scheduled_at_ms: u64,
        executed_at_ms: u64,
    },
    ChannelRecoveryPulse {
        action_id: String,
        pulse: u32,
        spirit_cost: u32,
        raw_hp: u32,
        raw_spirit: u32,
        hp_restored: u32,
        spirit_restored: u32,
        scheduled_at_ms: u64,
        executed_at_ms: u64,
    },
    ChannelEnded {
        action_id: String,
        reason: ChannelEndReason,
    },
    Damage {
        source: String,
        target: String,
        amount: u32,
    },
    TargetKilled,
    MonsterKilled {
        monster_id: String,
    },
    KillScored {
        monster_id: String,
        template_id: String,
        score: u32,
        total_score: u32,
        kills: u32,
    },
    MonsterSkill {
        monster_id: String,
        skill_id: String,
    },
    MonsterHealed {
        monster_id: String,
        amount: u32,
    },
    PickedUp {
        resource_id: String,
        quantity: u32,
    },
    ItemUsed {
        action_id: String,
        item_id: String,
        hp_restored: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spirit_restored: Option<u32>,
    },
    AttunementStarted {
        objective_id: String,
        finishes_at_ms: u64,
    },
    AttunementInterrupted {
        objective_id: String,
        reason: ChestInterruptReason,
    },
    AttunementCompleted {
        objective_id: String,
        consumed: Vec<ItemAmount>,
    },
    HeavyStarted {
        monster_id: String,
        cast_id: u32,
        skill_id: String,
        commits_at_ms: u64,
    },
    HeavyCommitted {
        cast: HeavyCastIdentity,
    },
    HeavyResolved {
        cast: HeavyCastIdentity,
        contacted: bool,
        damage: u32,
        blocked: bool,
        counter_awarded: bool,
    },
    HeavyCancelled {
        casts: Vec<HeavyCastIdentity>,
        counter: Option<HeavyCastIdentity>,
        reason: HeavyCancelReason,
    },
    CounterOpened {
        window: CounterView,
    },
    CounterTriggered {
        cast: HeavyCastIdentity,
        damage: u32,
    },
    SupportStarted {
        monster_id: String,
        cast_id: u32,
        target_id: String,
        commits_at_ms: u64,
        interrupt_until_ms: u64,
    },
    SupportPulse {
        monster_id: String,
        cast_id: u32,
        target_id: String,
        tick: u32,
        amount: u32,
    },
    SupportCompleted {
        monster_id: String,
        cast_id: u32,
        target_id: String,
    },
    SupportInterrupted {
        cast: SupportCastIdentity,
    },
    SupportCancelled {
        casts: Vec<SupportCastIdentity>,
        reason: SupportCancelReason,
    },
    ChestOpened {
        chest_id: String,
        generation: u32,
        resource_ids: Vec<String>,
    },
    ChestRespawned {
        chest_id: String,
        generation: u32,
    },
    ChestUnlockStarted {
        chest_id: String,
        generation: u32,
    },
    ChestUnlockInterrupted {
        chest_id: String,
        generation: u32,
        reason: ChestInterruptReason,
    },
    ChestLayerCompleted {
        chest_id: String,
        generation: u32,
        completed_layers: u32,
    },
    ChestUnlocked {
        chest_id: String,
        generation: u32,
    },
    GuardiansReady {
        chest_id: String,
        generation: u32,
        monster_ids: Vec<String>,
    },
    GuardianSpawnBlocked {
        chest_id: String,
        generation: u32,
    },
    EncounterResetScheduled {
        encounter_id: String,
        round: u32,
        resets_at_ms: u64,
    },
    EncounterResetCancelled {
        encounter_ids: Vec<String>,
    },
    EncounterReset {
        encounter_id: String,
        round: u32,
    },
    EncounterTriggered {
        encounter_id: String,
    },
    EncounterSpawnBlocked {
        encounter_id: String,
    },
    EncounterStarted {
        encounter_id: String,
        monster_ids: Vec<String>,
    },
    EncounterCompleted {
        encounter_id: String,
        reward_chest_ids: Vec<String>,
    },
    EncounterWaveScheduled {
        encounter_id: String,
        wave_id: String,
        wave_index: u32,
        ready_at_ms: u64,
    },
    EncounterWaveStarted {
        encounter_id: String,
        wave_id: String,
        wave_index: u32,
        monster_ids: Vec<String>,
    },
    EncounterWaveSpawnBlocked {
        encounter_id: String,
        wave_id: String,
        wave_index: u32,
    },
    EncounterWaveCompleted {
        encounter_id: String,
        wave_id: String,
        wave_index: u32,
    },
    BossPhaseChanged {
        monster_id: String,
        epoch: u32,
        from_index: u32,
        phase_id: String,
        phase_name: String,
        phase_index: u32,
    },
    BossReset {
        monster_id: String,
        epoch: u32,
        phase_id: String,
        phase_name: String,
    },
    AreaStarted {
        monster_id: String,
        cast_id: u32,
        skill_id: String,
        center: [f64; 2],
        radius: f64,
        started_at_ms: u64,
        resolves_at_ms: u64,
    },
    AreaResolved {
        monster_id: String,
        cast_id: u32,
        skill_id: String,
        hit: bool,
        damage: u32,
    },
    AreaCancelled {
        casts: Vec<AreaCastIdentity>,
        reason: AreaCancelReason,
    },
    ExtractionStarted,
    ExtractionInterrupted,
    SettlementApplied {
        outcome: Outcome,
        kept: u32,
        dropped: u32,
        cultivation: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scoring: Option<ScoreSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map_progress: Option<MapProgressSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reset_progress: Option<ResetProgressSummary>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        items: Vec<ItemSettlement>,
    },
    AssetSettlementApplied {
        credits: Vec<AssetAmount>,
        summary: AssetSettlement,
    },
    ResultSettlementApplied {
        credits: Vec<AssetAmount>,
        summary: ResultSettlement,
    },
    OperationNeedsReconciliation {
        operation_id: String,
        reason: Error,
    },
    OperationResolved {
        operation_id: String,
    },
    Closed {
        pending_operations: Vec<String>,
        live_worlds: u32,
    },
    Fault {
        code: Error,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(
    clippy::large_enum_variant,
    reason = "Keep the public inline Snapshot type; bounded output queues cap variant storage."
)]
pub enum OutputBody {
    SpectatorPublicationV6 {
        slot: u8,
        connection: Connection,
        stream_epoch: u32,
        revision: u64,
        payload: Box<SpectatorPayloadV6>,
    },
    PublicationV6 {
        recipient_id: String,
        connection: Connection,
        stream_epoch: u32,
        revision: u64,
        payload: Box<PublicationPayloadV6>,
    },
    BusinessRequestV4 {
        participant_id: String,
        principal: String,
        request: BusinessRequest,
    },
    PublicationV4 {
        recipient_id: String,
        connection: Connection,
        snapshot: SnapshotV4,
    },
    EventV4 {
        recipient_id: Option<String>,
        event: EventV4,
    },
    ResponsibilityV4 {
        checkpoint: Box<IoCheckpointV4>,
    },
    BusinessRequest {
        request: BusinessRequest,
    },
    Publication {
        recipient_id: String,
        connection: Connection,
        snapshot: Snapshot,
    },
    Event {
        event: Event,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub key: MatchKey,
    pub id: String,
    pub logical_ms: u64,
    pub body: OutputBody,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Box<IoCheckpoint>>,
}

pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemAmount {
    pub item_id: String,
    pub quantity: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapProgressSummary {
    pub completed_encounter_ids: Vec<String>,
    pub attuned: bool,
    pub reward_opened: bool,
    pub consumed: Vec<ItemAmount>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapObjectiveView {
    pub id: String,
    pub name: String,
    pub required_encounter_ids: Vec<String>,
    pub completed_encounter_ids: Vec<String>,
    pub reward_chest_id: String,
    pub available: bool,
    pub attuned: bool,
    pub reward_opened: bool,
    pub finishes_at_ms: Option<u64>,
    pub duration_ms: u64,
    pub cost: ItemAmount,
    pub interruption_reason: Option<ChestInterruptReason>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SupportView {
    pub skill_id: String,
    pub name: String,
    pub remaining_casts: u32,
    pub max_casts: u32,
    pub ready_at_ms: u64,
    pub interrupt_ready_at_ms: u64,
    pub cast: Option<SupportCastView>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SupportCastView {
    pub cast_id: u32,
    pub target_id: String,
    pub committed: bool,
    pub started_at_ms: u64,
    pub commits_at_ms: u64,
    pub next_tick_at_ms: u64,
    pub interrupt_until_ms: u64,
    pub completed_ticks: u32,
    pub total_ticks: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SupportCastIdentity {
    pub monster_id: String,
    pub cast_id: u32,
    pub target_id: String,
    pub committed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportCancelReason {
    Background,
    Returning,
    MemberDied,
    TargetInvalid,
    Resource,
    Closed,
    Replaced,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeavyView {
    pub skill_id: String,
    pub name: String,
    pub ready_at_ms: u64,
    pub cast: Option<HeavyCastView>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeavyCastView {
    pub cast_id: u32,
    pub origin: [f64; 2],
    pub end: [f64; 2],
    pub front: [f64; 2],
    pub half_width: f64,
    pub started_at_ms: u64,
    pub commits_at_ms: u64,
    pub ends_at_ms: u64,
    pub committed: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeavyCastIdentity {
    pub monster_id: String,
    pub cast_id: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeavyCancelReason {
    Background,
    Returning,
    OwnerDied,
    Closed,
    Replaced,
    Resource,
    Expired,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CounterView {
    pub monster_id: String,
    pub cast_id: u32,
    pub started_at_ms: u64,
    pub ends_at_ms: u64,
    pub raw_damage: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetProgressSummary {
    pub actual_kills: u32,
    pub encounters: Vec<EncounterRoundSummary>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterRoundSummary {
    pub encounter_id: String,
    pub round: u32,
    pub completed: bool,
    pub defeats: Vec<SpawnDefeatSummary>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnDefeatSummary {
    pub monster_id: String,
    pub deaths: u32,
    pub last_round: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterResetView {
    pub round: u32,
    pub reset_at_ms: Option<u64>,
    pub delay_ms: u64,
    pub pursuit_radius: f64,
    pub actual_defeats: u32,
    pub rewarded_defeats: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerSkillView {
    pub skill_id: String,
    pub ready_at_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceReservationView {
    pub spirit: u32,
    pub hp: u32,
    pub minimum_hp_after_cost: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerActionView {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub action_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub resource_reservation: Option<ResourceReservationView>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub charge: Option<ChargeActionView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_position: Option<[f64; 2]>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub unit_target: Option<UnitTarget>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub ally_target: Option<UnitTarget>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub ally_target_is_self: Option<bool>,
    pub skill_id: Option<String>,
    pub commits_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeActionView {
    pub action_id: String,
    pub started_at_ms: u64,
    pub min_release_at_ms: u64,
    pub auto_commit_at_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub released_at_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub release_windup_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub aim: Option<ChargeAimView>,
}

pub(crate) fn present_value<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeAimView {
    pub direction: [f64; 2],
    pub updated_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeCommitView {
    pub held_ms: u64,
    pub damage_factor_bps: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub direction: Option<[f64; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelEndReason {
    InvalidTarget,
    OutOfRange,
    Completed,
    Cancelled,
    Interrupted,
    SourceInvalid,
    NoResource,
    Capacity,
    InvalidOrigin,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveChannelView {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub target: Option<EntityRef>,
    pub action_id: String,
    pub skill_id: String,
    pub direction: [f64; 2],
    pub started_at_ms: u64,
    pub ends_at_ms: u64,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub next_pulse_at_ms: Option<u64>,
    pub pulses: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub total_pulses: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub recovery: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectileRecastView {
    pub action_id: String,
    pub skill_id: String,
    pub projectile_id: String,
    pub expires_at_ms: u64,
    pub next_index: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuedCastView {
    pub action_id: String,
    pub skill_id: String,
    pub direction: [f64; 2],
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FollowupView {
    pub source_skill_id: String,
    pub promotes_at_ms: u64,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub queued: Option<QueuedCastView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCancelView {
    pub skill_id: String,
    pub cancel_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicFollowupView {
    pub action_id: String,
    pub target: UnitTarget,
    pub promotes_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicPursuitView {
    pub action_id: String,
    pub target: UnitTarget,
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
}

/// A unit identity scoped to one World. The action and skill determine the allowed target kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitTarget {
    pub id: String,
    pub generation: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerAreaWarningView {
    pub action_id: String,
    pub skill_id: String,
    pub center: [f64; 2],
    pub radius: f64,
    pub started_at_ms: u64,
    pub resolves_at_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerAreaCancelReason {
    OwnerDied,
    Terminal,
    Closed,
    EncounterReset,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerFieldView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineFieldView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_policy: Option<FieldContactPolicyView>,
    pub action_id: String,
    pub skill_id: String,
    pub center: [f64; 2],
    pub radius: f64,
    pub started_at_ms: u64,
    pub activates_at_ms: u64,
    pub expires_at_ms: u64,
    pub next_tick_at_ms: Option<u64>,
    pub completed_ticks: u32,
    pub total_ticks: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerFieldEndReason {
    HitBudgetExhausted,
    Expired,
    OwnerDied,
    Terminal,
    Closed,
    EncounterReset,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldContactPolicyView {
    pub max_targets_per_tick: u32,
    pub max_total_hits: u32,
    pub repeat_interval_ms: u64,
    pub used_hits: u32,
    pub remaining_hits: u32,
    pub contacts: Vec<FieldContactView>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldContactView {
    pub target: UnitTarget,
    pub last_hit_at_ms: u64,
    pub next_eligible_at_ms: Option<u64>,
}

/// Directional flight published by Rust, scoped to one World.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PiercingProjectileView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_interval_ms: Option<u64>,
    pub action_id: String,
    pub skill_id: String,
    pub position: [f64; 2],
    pub direction: [f64; 2],
    pub radius: f64,
    pub speed: f64,
    pub remaining_range: f64,
    pub expires_at_ms: u64,
    pub max_hits: u32,
    pub used_hits: u32,
    pub remaining_hits: u32,
    pub contacts: Vec<PiercingContactView>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiercingContactView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_eligible_at_ms: Option<u64>,
    pub target: UnitTarget,
    pub hit_at_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PiercingEndReason {
    HitBudgetExhausted,
    Wall,
    RangeExhausted,
    Expired,
    OwnerDied,
    Terminal,
    Closed,
    EncounterReset,
}

/// Fixed world capsule, clipped against projectile obstacles at commit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineFieldView {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub width: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainHopView {
    pub from: StatusEntity,
    pub target: StatusEntity,
    pub raw_damage: u32,
    pub applied: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainEndReason {
    Complete,
    NoTarget,
    ZeroBudget,
    SourceInvalid,
    Capacity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillCooldownChange {
    pub skill_id: String,
    pub previous_ready_at_ms: u64,
    pub ready_at_ms: u64,
    pub reduced_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinisherMode {
    Execute,
    PowerDamage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum ResourceHpPolicy {
    DirectTransfer,
    Damage,
}
impl TryFrom<String> for ResourceHpPolicy {
    type Error = &'static str;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        match value.as_str() {
            "direct_transfer" => Ok(Self::DirectTransfer),
            "damage" => Ok(Self::Damage),
            _ => Err("CONFIG_RESOURCE_HP_POLICY"),
        }
    }
}
impl From<ResourceHpPolicy> for String {
    fn from(value: ResourceHpPolicy) -> Self {
        match value {
            ResourceHpPolicy::DirectTransfer => "direct_transfer",
            ResourceHpPolicy::Damage => "damage",
        }
        .into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CastInterceptionOutcome {
    ImmuneAllowed,
    Interrupted,
    AllowedWithFollowup,
}
