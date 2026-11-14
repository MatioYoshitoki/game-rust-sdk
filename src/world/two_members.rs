//! One shared authoritative world. Members own their mutable state; NPCs, terrain,
//! projectiles, status store and resources are each allocated exactly once.
use super::{damage, segment_circle};
use crate::{
    config::{
        BasicConfig, ChargeConfig, Config, MonsterTemplate, PhaseMovement, SkillConfig, distance,
        normalize,
    },
    protocol::*,
    status::Statuses,
};
use std::{collections::BTreeMap, sync::Arc};

#[cfg(test)]
pub(crate) mod mechanism_contract_tests;

#[cfg(all(test, match_sdk_test_data))]
mod ally_heal_tests;
#[cfg(all(test, match_sdk_test_data))]
mod ally_hot_overheal_shield_tests;
#[cfg(all(test, match_sdk_test_data))]
mod ally_hot_tests;
#[cfg(all(test, match_sdk_test_data))]
mod ally_overheal_shield_tests;
mod ally_recovery;
#[cfg(all(test, match_sdk_test_data))]
mod ally_shield_tests;
#[cfg(all(test, match_sdk_test_data))]
mod area_tests;
mod areas;
mod attack_speed;
mod boons;
mod npc_ai;
mod npc_skills;
mod participant_ai;
mod world_loot;
use npc_skills::NpcCast;
mod basic_beam;
mod basic_charge;
use basic_charge::{ChargeAim, ChargedBasic};
mod basic_delivery;
mod basic_flight;
mod basic_followup;
#[cfg(all(test, match_sdk_test_data))]
mod basic_followup_tests;
mod basic_impact;
mod basic_projectiles;
mod basic_pursuit;
mod basic_wave;
mod basic_wave_controls;
mod checkpoint_zones;
mod contested_equipment;
mod encounter_progress;
mod extraction_lifecycle;
mod match_mode;
mod spectators;
mod training_targets;
mod zone_exposure;
use basic_pursuit::BasicPursuit;
#[cfg(all(test, match_sdk_test_data))]
mod basic_pursuit_tests;
mod cast_interception;
#[cfg(all(test, match_sdk_test_data))]
mod cast_interception_tests;
mod cast_movement;
#[cfg(all(test, match_sdk_test_data))]
mod cast_movement_tests;
mod channel;
mod character_build;
#[cfg(all(test, match_sdk_test_data))]
mod character_build_tests;
mod conditional_activation;
#[cfg(all(test, match_sdk_test_data))]
mod conditional_activation_tests;
#[cfg(all(test, match_sdk_test_data))]
mod conditional_clock_tests;
#[cfg(all(test, match_sdk_test_data))]
mod derived_effects_tests;
#[cfg(all(test, match_sdk_test_data))]
mod locked_basic_tests;
#[cfg(all(test, match_sdk_test_data))]
mod paid_resource_tests;
mod recast;
#[cfg(all(test, match_sdk_test_data))]
mod recast_tests;
#[cfg(all(test, match_sdk_test_data))]
mod resource_reservation_tests;
mod resource_steal;
#[cfg(all(test, match_sdk_test_data))]
mod resource_steal_tests;
mod retaliation;
#[cfg(all(test, match_sdk_test_data))]
mod retaliation_tests;
#[cfg(all(test, match_sdk_test_data))]
mod source_cast_scenario_tests;
#[cfg(all(test, match_sdk_test_data))]
mod source_execution_tests;
#[cfg(all(test, match_sdk_test_data))]
mod source_gate_tests;
mod source_gates;
#[cfg(all(test, match_sdk_test_data))]
mod stack_counter_tests;
mod stack_counters;
mod status_ends;
mod summon_copy;
mod summons;
#[cfg(all(test, match_sdk_test_data))]
mod terminal_counter_tests;
#[cfg(all(test, match_sdk_test_data))]
mod unit_effect_tests;
mod unit_effects;
use recast::RecastOpportunity;
#[cfg(all(test, match_sdk_test_data))]
mod accuracy_tests;
mod action_totals;
#[cfg(all(test, match_sdk_test_data))]
mod armor_penetration_tests;
mod chain_damage;
#[cfg(all(test, match_sdk_test_data))]
mod channel_recovery_tests;
#[cfg(all(test, match_sdk_test_data))]
mod channel_tests;
#[cfg(all(test, match_sdk_test_data))]
mod charge_tests;
mod combat;
#[cfg(all(test, match_sdk_test_data))]
mod combat_tests;
mod contact_status;
#[cfg(all(test, match_sdk_test_data))]
mod control_duration_reduction_tests;
#[cfg(all(test, match_sdk_test_data))]
mod control_immunity_tests;
mod control_policies;
#[cfg(all(test, match_sdk_test_data))]
mod cooldown_reduction_tests;
#[cfg(all(test, match_sdk_test_data))]
mod critical_chance_tests;
#[cfg(all(test, match_sdk_test_data))]
mod critical_damage_bonus_tests;
#[cfg(all(test, match_sdk_test_data))]
mod damage_immunity_tests;
#[cfg(all(test, match_sdk_test_data))]
mod direct_ignore_armor_tests;
#[cfg(all(test, match_sdk_test_data))]
mod dodge_chance_tests;
#[cfg(all(test, match_sdk_test_data))]
mod dodge_penalty_tests;
#[cfg(all(test, match_sdk_test_data))]
mod first_control_immunity_tests;
mod followup;
#[cfg(all(test, match_sdk_test_data))]
mod followup_tests;
#[cfg(all(test, match_sdk_test_data))]
mod hp_cost_tests;
#[cfg(all(test, match_sdk_test_data))]
mod instant_self_heal_tests;
#[cfg(all(test, match_sdk_test_data))]
mod item_cleanse_tests;
#[cfg(all(test, match_sdk_test_data))]
mod item_healing_received_tests;
mod item_use;
#[cfg(all(test, match_sdk_test_data))]
mod item_use_tests;
#[cfg(all(test, match_sdk_test_data))]
mod line_tests;
mod loot_drop;
#[cfg(all(test, match_sdk_test_data))]
mod loot_drop_tests;
#[cfg(all(test, match_sdk_test_data))]
mod member_unavoidable_tests;
#[cfg(all(test, match_sdk_test_data))]
mod movement_haste_tests;
#[cfg(all(test, match_sdk_test_data))]
mod npc_hit_tests;
#[cfg(all(test, match_sdk_test_data))]
mod npc_movement_slow_tests;
#[cfg(all(test, match_sdk_test_data))]
mod npc_root_tests;
#[cfg(all(test, match_sdk_test_data))]
mod npc_silence_tests;
#[cfg(all(test, match_sdk_test_data))]
mod npc_spirit_recovery_lock_tests;
#[cfg(all(test, match_sdk_test_data))]
mod npc_stun_tests;
#[cfg(all(test, match_sdk_test_data))]
mod npc_unavoidable_tests;
mod numeric;
#[cfg(all(test, match_sdk_test_data))]
mod numeric_tests;
#[cfg(all(test, match_sdk_test_data))]
mod overheal_shield_tests;
mod periodic;
#[cfg(all(test, match_sdk_test_data))]
mod periodic_damage_reduction_tests;
#[cfg(all(test, match_sdk_test_data))]
mod periodic_healing_received_tests;
#[cfg(all(test, match_sdk_test_data))]
mod periodic_overheal_shield_tests;
mod periodic_resources;
#[cfg(all(test, match_sdk_test_data))]
mod periodic_tests;
mod piercing;
#[cfg(all(test, match_sdk_test_data))]
mod piercing_tests;
#[cfg(all(test, match_sdk_test_data))]
mod reaim_tests;
mod recovery_cancel;
#[cfg(all(test, match_sdk_test_data))]
mod recovery_cancel_tests;
#[cfg(all(test, match_sdk_test_data))]
mod recovery_orb_tests;
#[cfg(all(test, match_sdk_test_data))]
mod recovery_strength_tests;
mod shield_records;
#[cfg(all(test, match_sdk_test_data))]
mod shield_tests;
mod skill_loadouts;
#[cfg(all(test, match_sdk_test_data))]
mod skill_power_tests;
#[cfg(all(test, match_sdk_test_data))]
mod skill_slot_tests;
#[cfg(all(test, match_sdk_test_data))]
mod spirit_recovery_lock_tests;
mod status_cleanses;
mod status_operations;
mod status_windows;
#[cfg(all(test, match_sdk_test_data))]
mod stillness_guard_tests;
mod trigger_status;
mod triggers;
mod weapon_equipment;

pub(crate) type Events = Vec<(Option<usize>, EventV4)>;
fn private(events: &mut Events, i: usize, event: Event) {
    events.push((Some(i), EventV4::Private { event }));
}
fn status_entity(e: &EntityRef) -> StatusEntity {
    StatusEntity {
        kind: e.kind,
        id: e.id.clone(),
        generation: e.generation,
        encounter_round: None,
    }
}

#[derive(Clone)]
struct Pending {
    checkpoint_epoch: u32,
    charged_basic: Option<ChargedBasic>,
    reservation: Option<ResourceReservationView>,
    id: String,
    skill: Option<SkillConfig>,
    direction: [f64; 2],
    target: Option<EntityRef>,
    ground: Option<[f64; 2]>,
    anchor_id: Option<String>,
    commits: u64,
    charge_started_at_ms: Option<u64>,
    aim_updated_at_ms: Option<u64>,
    recovery_replacement: bool,
    pursuit_windup: bool,
    movement_anchor: cast_movement::MovementAnchor,
    charge_released_at: Option<u64>,
    charge_clock: Option<ChargeClock>,
}
#[derive(Clone, Copy)]
struct ChargeClock {
    min_release: u64,
    max_release: u64,
    release_windup: u64,
    held_ms: Option<u64>,
}
impl ChargeClock {
    fn new(definition: ChargeConfig, now: u64, step: u64) -> Self {
        let duration = |value: u64| {
            if definition.release_windup_ms.is_some() {
                value.div_ceil(step) * step
            } else {
                value
            }
        };
        Self {
            min_release: now + duration(definition.min_ms),
            max_release: now + duration(definition.max_ms),
            release_windup: duration(definition.release_windup_ms.unwrap_or(0)),
            held_ms: None,
        }
    }
}
impl Pending {
    fn open_charge(&self, now: u64) -> Result<(u64, ChargeConfig)> {
        let started = self.charge_started_at_ms.ok_or(Error::InvalidTarget)?;
        let charge = self
            .skill
            .as_ref()
            .and_then(|skill| skill.charge)
            .expect("charge definition");
        let clock = self.charge_clock.expect("charge clock");
        if self.commits != clock.max_release + clock.release_windup
            || self.charge_released_at.is_some()
            || now >= clock.max_release
        {
            return Err(Error::InvalidState);
        }
        Ok((started, charge))
    }

    fn set_charge_direction(
        &mut self,
        charge: ChargeConfig,
        direction: [f64; 2],
        now: u64,
    ) -> Result<()> {
        const ANGLE_EPSILON_DEG: f64 = 1e-9;
        let policy = charge.aim_update.ok_or(Error::Unsupported)?;
        let last = self.aim_updated_at_ms.expect("editable charge clock");
        let [x, y] = self.direction;
        let [next_x, next_y] = direction;
        let cross = x * next_y - y * next_x;
        let dot = x * next_x + y * next_y;
        let turn = cross.abs().atan2(dot).to_degrees();
        if turn <= ANGLE_EPSILON_DEG {
            return Ok(());
        }
        let elapsed = now.saturating_sub(last);
        if elapsed < policy.min_interval_ms {
            return Err(Error::InvalidState);
        }
        let budget = f64::from(policy.max_turn_deg_per_sec) * elapsed as f64 / 1000.;
        if turn > budget + ANGLE_EPSILON_DEG {
            return Err(Error::OutOfRange);
        }
        self.direction = direction;
        self.aim_updated_at_ms = Some(now);
        Ok(())
    }
}

fn validated_direction(direction: [f64; 2]) -> Result<[f64; 2]> {
    if !direction.iter().all(|value| value.is_finite()) {
        return Err(Error::InvalidMessage);
    }
    let unit = normalize(direction);
    if unit == [0., 0.] {
        return Err(Error::InvalidMessage);
    }
    Ok(unit)
}

struct ActiveChannel {
    skill: SkillConfig,
    action_id: String,
    direction: [f64; 2],
    raw: u32,
    started_at_ms: u64,
    ends_at_ms: u64,
    next_pulse_at_ms: u64,
    pulses: u32,
    movement_anchor: cast_movement::MovementAnchor,
    recovery_budget: Option<[u32; 2]>,
    steal_budget: Option<[u32; 2]>,
    target: Option<EntityRef>,
}

struct MemberShot<'a> {
    action_id: Option<&'a str>,
    start_position: Option<[f64; 2]>,
    skill: Option<&'a SkillConfig>,
    direction: [f64; 2],
    target: Option<EntityRef>,
    raw: u32,
}

pub(crate) struct Member {
    boons: Option<boons::BoonState>,
    exposure: Option<zone_exposure::Exposure>,
    ai: Option<participant_ai::ParticipantAiState>,
    checkpoints: Vec<checkpoint_zones::CheckpointCounter>,
    basic_profile: Option<String>,
    world_weapon_profile: Option<String>,
    reserved_world_slots: u32,
    equipment: Option<WeaponEquipmentView>,
    combat_build: Option<CombatBuild>,
    skills: SkillLoadoutState,
    pub reference: ParticipantRef,
    summon_copy_ready: (u32, u64),
    pub facts: ParticipantFacts,
    pub phase: MemberPhase,
    pub joined: bool,
    pub ready: bool,
    pub foreground: bool,
    pub protected: bool,
    pub hp: u32,
    pub spirit: u32,
    pub position: [f64; 2],
    pub desired: Option<Outcome>,
    movement: [f64; 2],
    path: Vec<[f64; 2]>,
    pending: Option<Pending>,
    channel: Option<ActiveChannel>,
    recast: Option<RecastOpportunity>,
    recovery_skill: Option<String>,
    recovery_cancel_at: Option<u64>,
    recovery_cancel_policy: Option<crate::config::RecoveryCancelConfig>,
    queued_cast: Option<QueuedCastView>,
    queued_cast_epoch: u32,
    queued_basic: Option<BasicFollowupView>,
    queued_basic_epoch: u32,
    basic_pursuit: Option<BasicPursuit>,
    cooldowns: BTreeMap<String, u64>,
    item_cooldowns: BTreeMap<String, u64>,
    attack_clock: Option<attack_speed::AttackCooldown>,
    basic_ready: u64,
    basic_delivery: Option<basic_delivery::BasicDelivery>,
    basic_beam: Option<basic_beam::BasicBeam>,
    basic_charge: Option<basic_charge::BasicChargeSession>,
    basic_charge_ready: u64,
    basic_wave_phase: BasicWavePhase,
    basic_wave_target_until: BTreeMap<usize, (u32, u64)>,
    basic_projectile: Option<basic_projectiles::BasicProjectilePlan>,
    basic_projectile_hits: u32,
    basic_seals: Vec<[f64; 2]>,
    basic_ricochet_marks: BTreeMap<usize, (u32, u32, u64)>,
    recovery_until: u64,
    recovery_movement: PhaseMovement,
    inventory: BTreeMap<String, u32>,
    instances: BTreeMap<String, ItemInstance>,
    relic_access: Option<String>,
    looted: BTreeMap<String, u32>,
    stack_removals: Option<BTreeMap<String, loot_drop::StackRemovals>>,
    revision: u32,
    tally: BTreeMap<String, u32>,
    damage_growth: BTreeMap<String, u64>,
    reward_rates: Vec<RewardEarningRate>,
    reward_kills: BTreeMap<(String, Option<String>), u32>,
    extract_end: Option<u64>,
    extraction_channel: Option<ExtractionChannelView>,
}
impl Member {
    fn active_basic_profile(&self) -> Option<&String> {
        self.world_weapon_profile
            .as_ref()
            .or(self.basic_profile.as_ref())
    }
    fn basic<'a>(&self, c: &'a Config) -> &'a BasicConfig {
        match self.active_basic_profile() {
            Some(id) => &c.basic_profiles().expect("validated basic profiles")[id],
            None => &c.basic,
        }
    }
    fn skill_equipped(&self, id: &str, c: &Config) -> bool {
        if self.skills.declared {
            self.skills
                .loadout
                .equipped_skill_ids
                .iter()
                .any(|s| s == id)
        } else {
            c.default_player_skills().any(|s| s.id == id)
        }
    }
    fn equipped_skills(&self, c: &Config) -> Vec<String> {
        if self.skills.declared {
            self.skills.loadout.equipped_skill_ids.clone()
        } else {
            c.default_player_skills().map(|s| s.id.clone()).collect()
        }
    }
    fn eligible(&self) -> bool {
        self.phase == MemberPhase::Active
            && self.hp > 0
            && self.desired.is_none()
            && self.ready
            && !self.protected
    }
    fn phase_movement(&self, now: u64) -> PhaseMovement {
        if self.basic_beam.is_some() || self.basic_charge.is_some() {
            return PhaseMovement::Locked {};
        }
        if let Some(channel) = &self.channel {
            return channel
                .skill
                .cast_movement
                .and_then(|p| p.active)
                .unwrap_or(PhaseMovement::Locked {});
        }
        if let Some(pending) = &self.pending {
            if pending.pursuit_windup
                || matches!(pending.charged_basic, Some(ChargedBasic::Flight(_)))
            {
                return PhaseMovement::Locked {};
            }
            return pending
                .skill
                .as_ref()
                .and_then(|skill| skill.cast_movement)
                .unwrap_or_default()
                .preparation;
        }
        if now < self.recovery_until {
            return self.recovery_movement;
        }
        PhaseMovement::Free {}
    }
    fn living(&self) -> bool {
        self.phase == MemberPhase::Active && self.hp > 0 && self.desired.is_none()
    }

    fn spirit_recovery_gap(&self, statuses: &Statuses, now: u64, max_spirit: u32) -> u32 {
        let gap = max_spirit.saturating_sub(self.spirit);
        if gap > 0
            && statuses.entries().iter().any(|status| {
                status.attribute == Some(StatusAttribute::SpiritRecoveryLock)
                    && now < status.expires_at_ms
                    && status.target == status_entity(&self.reference.entity)
            })
        {
            return 0;
        }
        gap
    }
}
struct Npc {
    entity: EntityRef,
    template_id: String,
    profile: Arc<MonsterTemplate>,
    position: [f64; 2],
    home: [f64; 2],
    hp: u32,
    spirit: u32,
    target: Option<EntityRef>,
    alerted: bool,
    returning: bool,
    attack_clock: Option<attack_speed::AttackCooldown>,
    next_attack: u64,
    next_cast: u64,
    casting: Option<NpcCast>,
    recovery: u64,
    path: Vec<[f64; 2]>,
    repath: u64,
    ai: npc_ai::NpcAiState,
    boss_phase: usize,
    training_owner: Option<usize>,
    training_active: bool,
    runtime_spawned: bool,
    training_reverse: bool,
    pursuit: Option<contested_equipment::Pursuit>,
}
impl Npc {
    fn new(
        entity: EntityRef,
        template_id: String,
        profile: Arc<MonsterTemplate>,
        home: [f64; 2],
    ) -> Self {
        Self {
            entity,
            template_id,
            hp: profile.stats["hp"],
            spirit: profile.stats["spirit"],
            profile,
            position: home,
            home,
            target: None,
            alerted: false,
            returning: false,
            attack_clock: None,
            next_attack: 0,
            next_cast: 0,
            casting: None,
            recovery: 0,
            path: vec![],
            repath: 0,
            ai: Default::default(),
            boss_phase: 0,
            training_owner: None,
            training_active: false,
            runtime_spawned: false,
            training_reverse: false,
            pursuit: None,
        }
    }
    fn visible_training_target(&self) -> bool {
        self.training_owner.is_none() || self.training_active
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Member(usize),
    Npc(usize),
    Summon(usize),
}
struct Shot {
    /// A member shot with end_on_source_invalid survives its source settling.
    end_on_source_invalid: bool,
    flight: Option<Box<basic_flight::FlightContactPlan>>,
    origin: crate::origin::ActionOrigin,
    action_id: String,
    damage_rule: Option<crate::config::AttackRule>,
    ignore_armor: bool,
    unavoidable: bool,
    contact: Option<crate::contact::ContactBudget<(u8, String, u32)>>,
    view: ProjectileViewV4,
    target: Option<EntityRef>,
    owner: Owner,
    direction: [f64; 2],
    remaining: f64,
    speed: f64,
    raw: u32,
    metal: bool,
    hit_statuses: Vec<String>,
    created: u64,
    expires: u64,
}
struct Resource {
    view: ResourceViewV4,
    hp: u32,
    spirit: u32,
    player_drop: bool,
    origin: world_loot::LootOrigin,
}

pub(crate) struct World {
    mode: Option<match_mode::ModeState>,
    contested: Option<contested_equipment::ContestedState>,
    exposure: Option<zone_exposure::ExposureWorld>,
    encounter: Option<encounter_progress::EncounterState>,
    member_conflict: bool,
    training: bool,
    training_shared_seats: Vec<usize>,
    training_runtime: Option<Vec<training_targets::SeatRuntime>>,
    checkpoint_epoch: u32,
    shared_boon_claims: [u8; crate::config::world_boons::MAX_OPPORTUNITIES],
    world_loot: world_loot::LootState,
    status_ends: status_ends::EndState,
    stack_leaves: bool,
    triggers: triggers::TriggerState,
    record_shields: shield_records::ShieldRecordState,
    window_records: status_windows::WindowState,
    contact_reactions: contact_reactions::ContactReactionState,
    status_cleanses: status_cleanses::CleanseState,
    periodic_enabled: bool,
    shields_enabled: bool,
    pub members: Vec<Member>,
    pub active_at: Option<u64>,
    grid: super::pve::Grid,
    terrain_damage_remainders: BTreeMap<triggers::EntityKey, u64>,
    npcs: Vec<Npc>,
    summons: Vec<summons::Summon>,
    shots: Vec<Shot>,
    areas: Vec<areas::AreaPlan>,
    basic_waves: Vec<basic_wave::BasicWavePlan>,
    basic_impacts: Vec<basic_impact::BasicImpactPlan>,
    resources: Vec<Resource>,
    statuses: Statuses,
    rng: u32,
    sequence: u64,
    next_regen: u64,
    extraction_pressure_pulse: u64,
}
impl World {
    pub(crate) fn new(
        c: &Config,
        seats: &[ParticipantSeat],
        facts: Vec<ParticipantFacts>,
        seed: u32,
    ) -> Self {
        let p = c.pve.as_ref().expect("validated dual PVE");
        let mode = c
            .match_mode
            .as_ref()
            .map(|spec| match_mode::ModeState::new(spec, seats.len()));
        let profiles = p.build_profiles();
        let mut npcs = vec![];
        let mut add = |id: String, template_id: String, position: [f64; 2]| {
            let profile = profiles[&template_id].clone();
            let hidden = mode.as_ref().is_some_and(|mode| mode.hides(&id));
            let mut npc = Npc::new(
                EntityRef {
                    kind: StatusEntityKind::Monster,
                    id,
                    generation: 1,
                },
                template_id,
                profile,
                position,
            );
            if let Some(seats) = c.training_targets() {
                let (owner, spec) = seats
                    .iter()
                    .enumerate()
                    .find(|(_, spec)| {
                        spec.presets
                            .values()
                            .any(|ids| ids.contains(&npc.entity.id))
                    })
                    .expect("validated training owner");
                npc.training_owner = Some(owner);
                npc.training_active = spec
                    .initial_preset
                    .as_ref()
                    .is_some_and(|id| spec.presets[id].contains(&npc.entity.id));
            }
            if c.contested_equipment.as_ref().is_some_and(|rule| {
                rule.nodes
                    .iter()
                    .any(|node| node.pursuit_ids.contains(&npc.entity.id))
            }) {
                npc.pursuit = Some(contested_equipment::Pursuit::Dormant);
            }
            if hidden || !npc.visible_training_target() || npc.pursuit.is_some() {
                npc.hp = 0;
            }
            npcs.push(npc);
        };
        for (i, &position) in p.monster_positions.iter().enumerate() {
            add(
                format!("{}-{}", p.template_id, i + 1),
                p.template_id.clone(),
                position,
            );
        }
        for spawn in &p.extra_spawns {
            add(spawn.id.clone(), spawn.template_id.clone(), spawn.position);
        }
        npcs.sort_by(|a, b| a.entity.id.cmp(&b.entity.id));
        let mut world = Self {
            mode,
            exposure: c
                .zone_exposure
                .as_ref()
                .map(zone_exposure::ExposureWorld::new),
            contested: c
                .contested_equipment
                .as_ref()
                .map(contested_equipment::ContestedState::new),
            encounter: c
                .encounter_progress
                .as_ref()
                .map(encounter_progress::EncounterState::new),
            member_conflict: c.member_conflict(),
            training: c.training_targets().is_some(),
            training_shared_seats: c.training_shared_seats(),
            training_runtime: c.training_runtime().map(|r| {
                (0..seats.len())
                    .map(|_| training_targets::SeatRuntime::new(r.metrics))
                    .collect()
            }),
            checkpoint_epoch: 0,
            world_loot: world_loot::LootState::new(c.world_loot.as_ref()),
            shared_boon_claims: [0; crate::config::world_boons::MAX_OPPORTUNITIES],
            status_ends: Default::default(),
            stack_leaves: false,
            periodic_enabled: c.mechanism_version >= 38,
            shields_enabled: c.mechanism_version >= 39,
            members: seats
                .iter()
                .zip(facts)
                .enumerate()
                .map(|(i, (s, facts))| {
                    let (basic_profile, equipment) = match &s.weapon {
                        Some(weapon) => {
                            let (id, weapon) = c.weapon_catalog().unwrap().resolve(weapon).unwrap();
                            (
                                Some(id.to_owned()),
                                Some(WeaponEquipmentView {
                                    revision: 0,
                                    weapon: Some(weapon),
                                }),
                            )
                        }
                        None => (s.basic_profile.clone(), None),
                    };
                    Member {
                        ai: c
                            .participant_ai
                            .contains_key(&s.participant_id)
                            .then(Default::default),
                        checkpoints: vec![Default::default(); c.checkpoint_schedules().len()],
                        basic_profile,
                        world_weapon_profile: None,
                        reserved_world_slots: 0,
                        equipment,
                        combat_build: s.combat_build.clone(),
                        skills: crate::config::initial_skill_loadout(c, s.skills.as_ref()),
                        reference: ParticipantRef {
                            entity: EntityRef {
                                kind: StatusEntityKind::Participant,
                                id: s.participant_id.clone(),
                                generation: 1,
                            },
                            owner_id: s.participant_id.clone(),
                        },
                        summon_copy_ready: (0, 0),
                        hp: c.derived_base(facts.hp_points, StatusAttribute::MaxHp),
                        spirit: c.derived_base(facts.spirit_points, StatusAttribute::MaxSpirit),
                        facts,
                        position: c.member_settings().unwrap().spawns[i],
                        phase: MemberPhase::Deploying,
                        joined: false,
                        ready: false,
                        foreground: true,
                        protected: true,
                        desired: None,
                        movement: [0., 0.],
                        path: vec![],
                        pending: None,
                        channel: None,
                        recast: None,
                        recovery_skill: None,
                        recovery_cancel_at: None,
                        recovery_cancel_policy: None,
                        queued_cast: None,
                        queued_cast_epoch: 0,
                        queued_basic: None,
                        queued_basic_epoch: 0,
                        basic_pursuit: None,
                        cooldowns: BTreeMap::new(),
                        item_cooldowns: BTreeMap::new(),
                        attack_clock: None,
                        basic_ready: 0,
                        basic_delivery: None,
                        basic_beam: None,
                        basic_charge: None,
                        basic_charge_ready: 0,
                        basic_wave_phase: BasicWavePhase::Primary,
                        basic_wave_target_until: BTreeMap::new(),
                        basic_projectile: None,
                        basic_projectile_hits: 0,
                        basic_seals: vec![],
                        basic_ricochet_marks: BTreeMap::new(),
                        recovery_until: 0,
                        recovery_movement: PhaseMovement::Free {},
                        inventory: if c.carried_quantity == 0 {
                            BTreeMap::new()
                        } else {
                            BTreeMap::from([(c.item_id.clone(), c.carried_quantity)])
                        },
                        exposure: c.zone_exposure.as_ref().map(|_| Default::default()),
                        boons: c
                            .world_boons
                            .as_ref()
                            .map(|spec| boons::BoonState::new(spec.capacity)),
                        instances: BTreeMap::new(),
                        relic_access: None,
                        looted: BTreeMap::new(),
                        stack_removals: c
                            .settlement_progression
                            .as_ref()
                            .filter(|rule| rule.stack_flows)
                            .map(|_| BTreeMap::new()),
                        revision: 1,
                        tally: BTreeMap::new(),
                        damage_growth: BTreeMap::new(),
                        reward_rates: Vec::new(),
                        reward_kills: BTreeMap::new(),
                        extract_end: None,
                        extraction_channel: None,
                    }
                })
                .collect(),
            active_at: None,
            grid: super::pve::Grid::new(p),
            terrain_damage_remainders: BTreeMap::new(),
            npcs,
            summons: vec![],
            shots: vec![],
            areas: vec![],
            basic_waves: vec![],
            basic_impacts: vec![],
            resources: vec![],
            statuses: Statuses::default(),
            triggers: Default::default(),
            record_shields: Default::default(),
            window_records: Default::default(),
            contact_reactions: Default::default(),
            status_cleanses: Default::default(),
            rng: if seed == 0 { 0x9e3779b9 } else { seed },
            sequence: 0,
            next_regen: 0,
            extraction_pressure_pulse: 0,
        };
        world.refresh_encounter_stats(c, 0);
        world
    }
    pub(crate) fn npc_ids(c: &Config) -> Vec<String> {
        let p = c.pve.as_ref().unwrap();
        p.monster_positions
            .iter()
            .enumerate()
            .map(|(i, _)| format!("{}-{}", p.template_id, i + 1))
            .chain(p.extra_spawns.iter().map(|m| m.id.clone()))
            .collect()
    }
    pub(crate) fn bind_reward_rates(&mut self, i: usize, rates: Vec<RewardEarningRate>) {
        self.members[i].reward_rates = rates;
    }

    fn random(&mut self) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng
    }
    fn roll(&mut self, p: f64) -> bool {
        p >= 1. || p > 0. && f64::from(self.random()) / 4294967296. < p
    }
    fn next_id(&mut self, kind: &str) -> String {
        self.sequence += 1;
        format!("{kind}-{}", self.sequence)
    }
    fn stats(&self, i: usize, c: &Config, now: u64) -> (u32, u32) {
        let m = &self.members[i];
        let e = status_entity(&m.reference.entity);
        let r = c.pve.as_ref().unwrap().status_rules.as_ref();
        let attack = self.statuses.attribute(
            &e,
            m.base_points(c, StatusAttribute::Atk),
            now,
            r,
            StatusAttribute::Atk,
        );
        let defense = self
            .statuses
            .defense(&e, m.base_points(c, StatusAttribute::Def), now, r);
        (
            m.exposure_points(attack, c, now),
            m.exposure_points(defense, c, now),
        )
    }
    fn definitions(c: &Config, ids: &[String]) -> Vec<crate::config::StatusDefinition> {
        ids.iter()
            .map(|id| {
                c.pve
                    .as_ref()
                    .unwrap()
                    .status_definitions
                    .iter()
                    .find(|d| &d.id == id)
                    .unwrap()
                    .clone()
            })
            .collect()
    }
    fn can_statuses(&self, i: usize, c: &Config, ids: &[String]) -> bool {
        if ids.is_empty() {
            return true;
        }
        let e = status_entity(&self.members[i].reference.entity);
        self.statuses.can_apply(
            &Self::definitions(c, ids),
            &e,
            &e,
            c.pve.as_ref().unwrap().status_rules.as_ref().unwrap(),
        )
    }
    fn apply_statuses(
        &mut self,
        owner: usize,
        target: (EntityRef, u32),
        ids: &[String],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if ids.is_empty() {
            return;
        }
        self.apply_owned_statuses(Owner::Member(owner), target, ids, c, now, events);
    }
    pub(crate) fn activate(&mut self, c: &Config, now: u64, events: &mut Events) {
        assert!(self.active_at.is_none());
        self.active_at = Some(now);
        if let Some(exposure) = &mut self.exposure {
            exposure.activate(now);
        }
        self.next_regen = now + c.params.regen.interval_ms;
        for m in &mut self.members {
            m.phase = MemberPhase::Active;
            m.protected = false;
        }
        for n in &mut self.npcs {
            n.next_attack = now + n.profile.combat.attack_interval_ms;
            n.attack_clock = c.combat_rules.as_ref().map(|_| {
                attack_speed::AttackCooldown::new(now, n.profile.combat.attack_interval_ms)
            });
            n.next_cast = now;
        }
        events.push((None, EventV4::Activated { active_at_ms: now }));
        self.open_mode_stage(0, c, now, events);
    }
    fn skill_silenced(&self, i: usize, now: u64) -> bool {
        self.member_has_status(i, StatusAttribute::Silence, now)
    }

    fn active_member_statuses(&self, i: usize, now: u64) -> impl Iterator<Item = &StatusView> {
        let target = &self.members[i].reference.entity;
        self.statuses.entries().iter().filter(move |status| {
            status.target.kind == target.kind
                && status.target.id == target.id
                && status.target.generation == target.generation
                && now < status.expires_at_ms
        })
    }

    fn member_has_status(&self, i: usize, attribute: StatusAttribute, now: u64) -> bool {
        self.active_member_statuses(i, now)
            .any(|status| status.has_attribute(attribute))
    }

    fn member_movement_budget(&self, i: usize, c: &Config, now: u64, dt: f64) -> f64 {
        let member = &self.members[i];
        let mut speed = c
            .params
            .movement
            .speed(self.effective_spd(Owner::Member(i), c, now));
        if c.mechanism_version >= 90 {
            speed *= f64::from(
                self.statuses
                    .movement_factor_bps(&status_entity(&member.reference.entity), now),
            ) / 10000.;
        }
        speed * self.grid.speed(member.position) * dt
    }

    fn interrupt_silenced_skill(&mut self, i: usize, now: u64, events: &mut Events) {
        let member = &mut self.members[i];
        let pending = member
            .pending
            .as_ref()
            .is_some_and(|action| action.skill.is_some());
        if pending {
            member.pending = None;
        }
        let queued = member.queued_cast.take().is_some();
        if pending || queued {
            private(events, i, Event::ActionInterrupted);
        }
        self.finish_channel(i, ChannelEndReason::Interrupted, now, events);
    }

    fn interrupt_rooted_movement(&mut self, i: usize, events: &mut Events) {
        let member = &mut self.members[i];
        member.movement = [0., 0.];
        member.path.clear();
        if member.basic_pursuit.take().is_some() {
            private(events, i, Event::ActionInterrupted);
        }
    }

    fn interrupt(&mut self, i: usize, events: &mut Events) {
        let m = &mut self.members[i];
        let pending = m.pending.take().is_some();
        let queued = m.queued_cast.take().is_some();
        let basic = m.queued_basic.take().is_some();
        let pursuit = m.basic_pursuit.take().is_some();
        let charge = m.basic_charge.take().is_some();
        if pending || queued || basic || pursuit || charge {
            private(events, i, Event::ActionInterrupted);
        }
        m.movement = [0., 0.];
        m.path.clear();
        self.end_extraction_channel(i, ExtractionChannelState::Interrupted, events);
    }
    pub(crate) fn protect(&mut self, i: usize, now: u64, events: &mut Events) {
        self.clear_checkpoint_warnings(i);
        self.cancel_boon_claim(i, Error::NotReady, events);
        self.release_member_containers(i, events);
        self.forget_npc_enemy(&self.members[i].reference.entity.clone(), events);
        self.interrupt_owned_copies(i, events);
        self.cancel_owner_totals(&self.members[i].reference.entity.clone());
        self.schedule_basic_beam_end(i, BasicBeamEndReason::Protected);
        self.finish_channel(i, ChannelEndReason::Interrupted, now, events);
        self.interrupt(i, events);
        let m = &mut self.members[i];
        if let Some(delivery) = &mut m.basic_delivery {
            delivery.cancel_reward();
        }
        m.ready = false;
        m.protected = true;
        m.foreground = false;
        m.relic_access = None;
    }
    pub(crate) fn terminal_intent(&mut self, i: usize, outcome: Outcome, events: &mut Events) {
        if matches!(
            self.members[i].phase,
            MemberPhase::Settling | MemberPhase::Terminal
        ) {
            return;
        }
        self.release_contested_equipment(i, outcome, events);
        self.members[i].exposure = None;
        self.clear_mode_respawn(i);
        if let Some(seats) = &mut self.training_runtime {
            seats[i].respawn_at_ms = None;
            seats[i].clear_defeated_life = false;
        }
        if outcome == Outcome::Extraction && self.members[i].extraction_channel.is_some() {
            self.end_extraction_channel(i, ExtractionChannelState::Completed, events);
        }
        self.clear_member_combat(i, Some(outcome), events);
        if self.training {
            self.replace_training_npcs(i, &[], events);
        }
    }

    fn clear_member_combat(&mut self, i: usize, outcome: Option<Outcome>, events: &mut Events) {
        self.cancel_boon_claim(i, Error::InvalidState, events);
        self.clear_checkpoint_counters(i);
        self.end_basic_beam(i, BasicBeamEndReason::SourceInvalid, events);
        self.release_member_containers(i, events);
        self.clear_basic_projectile(i, events);
        self.interrupt(i, events);
        let m = &mut self.members[i];
        if let Some(channel) = m.channel.take() {
            private(
                events,
                i,
                Event::ChannelEnded {
                    action_id: channel.action_id,
                    reason: ChannelEndReason::SourceInvalid,
                },
            );
        }
        m.recast = None;
        m.basic_delivery = None;
        m.basic_wave_target_until.clear();
        m.item_cooldowns.clear();
        m.recovery_cancel_at = None;
        m.recovery_skill = None;
        if let Some(outcome) = outcome {
            m.desired = Some(outcome);
            m.phase = MemberPhase::Settling;
            m.protected = true;
            m.ready = false;
        }
        m.relic_access = None;
        let entity = m.reference.entity.clone();
        self.clear_boons(i, events);
        self.release_monster_priority(&entity);
        self.retire_member_summons(&entity, events);
        self.clear_owned_statuses(&entity, StatusRemovalReason::TargetInvalid, events);
        self.clear_areas(&entity, events);
        if outcome.is_some()
            && self.members.iter().all(|member| {
                matches!(member.phase, MemberPhase::Settling | MemberPhase::Terminal)
                    || self.mode.is_none() && !member.living()
            })
        {
            self.cancel_ground_areas(events);
        }
        self.clear_basic_waves(&entity, events);
        self.clear_basic_impacts(&entity, events);
        self.forget_npc_enemy(&entity, events);
        self.shots.retain(|s| {
            if matches!(s.owner,Owner::Member(j)if j==i) {
                // end_on_source_invalid keeps the frozen payload in flight.
                if s.end_on_source_invalid {
                    return true;
                }
                if s.contact.is_some() || s.flight.is_some() {
                    events.push((
                        None,
                        EventV4::ProjectileEnded {
                            projectile_id: s.view.id.clone(),
                            reason: Some(ProjectileEndReasonV4::SourceInvalid),
                        },
                    ));
                }
                false
            } else {
                true
            }
        });
        for n in &mut self.npcs {
            if n.target.as_ref() == Some(&entity) {
                n.target = None;
                n.casting = None;
            }
        }
        if outcome.is_some() {
            events.push((
                None,
                EventV4::MemberChanged {
                    entity,
                    phase: MemberPhase::Settling,
                },
            ));
        }
    }
    pub(crate) fn action(
        &mut self,
        i: usize,
        action: &Action,
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<Events> {
        let training_preset = match action {
            Action::SetOwnedTargets { preset_id } => Some(Some(preset_id.as_str())),
            Action::ClearOwnedTargets {} => Some(None),
            _ => None,
        };
        if matches!(action, Action::ResetTrainingMetrics {}) {
            self.reset_training_metrics(i, now)?;
            return Ok(vec![]);
        }
        if let Some(preset) = training_preset {
            if !self.members[i].eligible() {
                return Err(Error::NotReady);
            }
            let mut events = vec![];
            self.change_training_targets(i, preset, c, now, &mut events)?;
            return Ok(events);
        }
        self.reconcile_caps(c, now);
        let mut events = vec![];
        match action {
            Action::Join {} => {
                if self.members[i].phase != MemberPhase::Deploying || self.members[i].joined {
                    return Err(Error::InvalidState);
                }
                self.members[i].joined = true;
                private(&mut events, i, Event::Joined);
                return Ok(events);
            }
            Action::CombatReady { .. } => {
                let m = &mut self.members[i];
                if !m.joined
                    || !m.foreground
                    || matches!(m.phase, MemberPhase::Settling | MemberPhase::Terminal)
                {
                    return Err(Error::InvalidState);
                }
                m.ready = true;
                if self.active_at.is_some() {
                    m.protected = false;
                }
                return Ok(events);
            }
            Action::Background {} => {
                self.protect(i, now, &mut events);
                return Ok(events);
            }
            Action::Foreground {} => {
                if matches!(
                    self.members[i].phase,
                    MemberPhase::Settling | MemberPhase::Terminal
                ) {
                    return Err(Error::InvalidState);
                }
                self.members[i].foreground = true;
                return Ok(events);
            }
            Action::Leave {}
                if (self.mode.is_some() || self.training) && self.active_at.is_some() =>
            {
                self.withdraw_member(i, c, &mut events);
                return Ok(events);
            }
            _ => {}
        }
        self.expire_basic_charge(i, c, now);
        if !self.members[i].eligible() {
            return Err(Error::NotReady);
        }
        if self.mode_waiting(c)
            || (self.mode.is_some() || self.training)
                && matches!(action, Action::ExtractStart {} | Action::ExtractCancel {})
        {
            return Err(Error::InvalidState);
        }
        if c.combat_rules.is_some()
            && matches!(
                action,
                Action::Attack {}
                    | Action::AttackUnit { .. }
                    | Action::AttackGround { .. }
                    | Action::Cast { .. }
                    | Action::CastSelf { .. }
                    | Action::CastUnit { .. }
                    | Action::CastGround { .. }
                    | Action::CastAtAnchor { .. }
                    | Action::StartCharge { .. }
            )
        {
            let m = &self.members[i];
            let source = &m.reference.entity;
            if m.pending.as_ref().is_some_and(|p| p.id == id)
                || m.basic_delivery.as_ref().is_some_and(|p| p.is_action(id))
                || m.queued_cast.as_ref().is_some_and(|p| p.action_id == id)
                || m.queued_basic.as_ref().is_some_and(|p| p.action_id == id)
                || m.basic_pursuit
                    .as_ref()
                    .is_some_and(|p| p.view.action_id == id)
                || m.channel.as_ref().is_some_and(|p| p.action_id == id)
                || self
                    .shots
                    .iter()
                    .any(|s| s.view.source == *source && s.action_id == id)
                || self
                    .areas
                    .iter()
                    .any(|a| a.trigger_source() == source && a.trigger_action_id() == Some(id))
                || self.triggers.totals.action_open(source, id)
            {
                return Err(Error::InvalidState);
            }
        }
        if (self.members[i].skills.declared || !c.skill_library.is_empty())
            && let Action::Cast { skill_id, .. }
            | Action::StartCharge { skill_id, .. }
            | Action::CastSelf { skill_id }
            | Action::CastUnit { skill_id, .. }
            | Action::CastGround { skill_id, .. }
            | Action::CastAtAnchor { skill_id, .. } = action
            && !self.members[i].skill_equipped(skill_id, c)
        {
            return Err(Error::InvalidTarget);
        }
        let skill_action = matches!(
            action,
            Action::Cast { .. }
                | Action::CastSelf { .. }
                | Action::CastUnit { .. }
                | Action::CastGround { .. }
                | Action::CastAtAnchor { .. }
                | Action::StartCharge { .. }
                | Action::UpdateChargeAim { .. }
                | Action::ReleaseCharge { .. }
                | Action::RecastProjectile { .. }
                | Action::RecastAnchor { .. }
                | Action::ReturnAnchor { .. }
                | Action::EndAnchor { .. }
        );
        if self.members[i].basic_charge.is_some()
            && (skill_action
                || matches!(
                    action,
                    Action::Attack {}
                        | Action::AttackUnit { .. }
                        | Action::AttackGround { .. }
                        | Action::StartAttackBeam { .. }
                        | Action::ExtractStart {}
                        | Action::CancelAction {}
                ))
        {
            return Err(Error::InvalidState);
        }
        if self.members[i].basic_beam.is_some()
            && (skill_action
                || matches!(
                    action,
                    Action::Attack {}
                        | Action::AttackUnit { .. }
                        | Action::AttackGround { .. }
                        | Action::ExtractStart {}
                ))
        {
            return Err(Error::InvalidState);
        }
        if c.mechanism_version >= 89
            && (skill_action
                || matches!(
                    action,
                    Action::Attack { .. }
                        | Action::AttackUnit { .. }
                        | Action::AttackGround { .. }
                        | Action::StartAttackBeam { .. }
                        | Action::StartBasicCharge {}
                        | Action::Move { .. }
                        | Action::Navigate { .. }
                        | Action::ExtractStart { .. }
                ))
            && self.member_has_status(i, StatusAttribute::Stun, now)
        {
            return Err(Error::InvalidState);
        }
        if c.mechanism_version >= 83 && skill_action && self.skill_silenced(i, now) {
            return Err(Error::InvalidState);
        }
        if c.mechanism_version >= 88
            && matches!(action, Action::Move { .. } | Action::Navigate { .. })
            && self.member_has_status(i, StatusAttribute::Root, now)
        {
            return Err(Error::InvalidState);
        }
        match action {
            Action::Move { direction } => {
                if !direction.iter().all(|v| v.is_finite()) {
                    return Err(Error::InvalidMessage);
                }
                self.interrupt_for_movement(
                    i,
                    cast_movement::MoveRequest::Direction(normalize(*direction)),
                    now,
                    &mut events,
                );
                self.end_extraction_channel(i, ExtractionChannelState::Interrupted, &mut events);
                self.members[i].path.clear();
                self.members[i].movement = normalize(*direction);
                if self.members[i].movement == [0., 0.] {
                    self.members[i].stop_movement();
                }
            }
            Action::Navigate { destination } => {
                if !destination.iter().all(|v| v.is_finite()) {
                    return Err(Error::InvalidMessage);
                }
                if !self.grid.clear(*destination, c.params.player_radius, 0) {
                    return Err(Error::InvalidTarget);
                }
                let path = self.grid.route(
                    self.members[i].position,
                    *destination,
                    c.params.player_radius,
                );
                if path.is_empty() {
                    return Err(Error::InvalidTarget);
                }
                self.interrupt_for_movement(
                    i,
                    cast_movement::MoveRequest::Destination(*destination),
                    now,
                    &mut events,
                );
                self.end_extraction_channel(i, ExtractionChannelState::Interrupted, &mut events);
                self.members[i].path = path;
            }
            Action::Stop {} => {
                self.members[i].stop_movement();
            }
            Action::CancelAction {} => {
                self.schedule_basic_beam_end(i, BasicBeamEndReason::Cancelled);
                let channel_cancelled = self.members[i].channel.is_some();
                self.finish_channel(i, ChannelEndReason::Cancelled, now, &mut events);
                self.members[i].recast = None;
                let member = &mut self.members[i];
                let pending = member.pending.take().is_some();
                let queued = member.queued_cast.take().is_some();
                let basic = member.queued_basic.take().is_some();
                let pursuit = member.basic_pursuit.take().is_some();
                let recovery_cancelled = !channel_cancelled && member.can_cancel_recovery(now);
                if recovery_cancelled {
                    member.end_recovery(now);
                }
                if pending || queued || basic || pursuit || recovery_cancelled {
                    private(&mut events, i, Event::ActionInterrupted)
                }
            }
            Action::RecastProjectile {
                action_id,
                recast_index: 1,
                direction,
            } => {
                self.redirect_projectile(i, action_id, *direction, now, &mut events)?;
            }
            Action::RecastAnchor {
                anchor_id,
                action_id,
                recast_index,
                ..
            }
            | Action::ReturnAnchor {
                anchor_id,
                action_id,
                recast_index,
            }
            | Action::EndAnchor {
                anchor_id,
                action_id,
                recast_index,
            } => {
                let intent = match action {
                    Action::RecastAnchor { position, .. } => {
                        areas::AnchorRecastIntent::Relocate(*position)
                    }
                    Action::ReturnAnchor { .. } => areas::AnchorRecastIntent::ReturnToSource,
                    _ => areas::AnchorRecastIntent::End,
                };
                self.recast_anchor(
                    i,
                    (anchor_id, action_id, *recast_index),
                    intent,
                    c,
                    now,
                    &mut events,
                )?;
            }
            Action::UpdateChargeAim {
                action_id,
                direction,
            } => {
                let pending = self.members[i]
                    .pending
                    .as_mut()
                    .filter(|pending| pending.id == *action_id)
                    .ok_or(Error::InvalidTarget)?;
                let (_, charge) = pending.open_charge(now)?;
                pending.set_charge_direction(charge, validated_direction(*direction)?, now)?;
            }
            Action::ReleaseCharge {
                action_id,
                direction,
            } => {
                self.release_charge(i, action_id, *direction, now, &mut events)?;
            }
            Action::Cast {
                skill_id,
                direction,
            }
            | Action::StartCharge {
                skill_id,
                direction,
            } => {
                let skill = c.player_skill(skill_id).ok_or(Error::InvalidTarget)?;
                if matches!(action, Action::StartCharge { .. }) != skill.charge.is_some() {
                    return Err(Error::Unsupported);
                }
                if skill.is_self() || skill.is_unit_cast() || skill.is_ground() {
                    return Err(Error::InvalidTarget);
                }
                let direction = validated_direction(*direction)?;
                if skill.is_piercing() && c.mechanism_version < 43 {
                    return Err(Error::Unsupported);
                }
                if skill.is_piercing()
                    || skill.charge.is_some()
                    || skill.channel.is_some()
                    || skill.recast.is_some()
                    || skill.recovery_cancel.is_some()
                    || c.skill_followup.is_some() && skill.supports_followup()
                {
                    self.projectile_origin(i, skill.radius)?;
                }
                if skill.is_line() {
                    if c.mechanism_version < 42 {
                        return Err(Error::Unsupported);
                    }
                    self.line_plan(i, skill, direction)?;
                }
                let member = &self.members[i];
                if c.skill_followup.is_some()
                    && now < member.recovery_until
                    && !member.can_replace_recovery(skill, now)
                {
                    self.queue_followup(i, skill, direction, id, c, now)?;
                } else {
                    self.prepare(i, Some(skill.clone()), (direction, None), id, c, now)?;
                }
            }
            Action::CastSelf { skill_id } => {
                let skill = c.player_skill(skill_id).ok_or(Error::InvalidTarget)?;
                if !skill.is_self() {
                    return Err(Error::InvalidTarget);
                }
                self.prepare(i, Some(skill.clone()), ([0., 0.], None), id, c, now)?;
                if skill.commit_on_accept {
                    let prepared = self.members[i].pending.take().expect("accepted self skill");
                    self.commit(i, prepared, c, now, &mut events);
                }
            }
            Action::CastUnit { skill_id, target } => {
                if c.mechanism_version < 40 {
                    return Err(Error::Unsupported);
                }
                let skill = c.player_skill(skill_id).ok_or(Error::InvalidTarget)?;
                if !skill.is_unit_cast() {
                    return Err(Error::InvalidTarget);
                }
                if !crate::protocol::valid_id(&target.id) || target.generation == 0 {
                    return Err(Error::InvalidMessage);
                }
                let target = if skill.targets_ally() {
                    EntityRef {
                        kind: StatusEntityKind::Participant,
                        id: target.id.clone(),
                        generation: target.generation,
                    }
                } else {
                    self.hostile_unit_ref(target)
                };
                let direction = if skill.targets_ally() {
                    self.ally_skill_target(i, skill, &target, c)?;
                    [0., 0.]
                } else {
                    self.unit_aim(i, skill, &target)?
                };
                self.prepare(
                    i,
                    Some(skill.clone()),
                    (direction, Some(target)),
                    id,
                    c,
                    now,
                )?;
            }
            Action::CastGround { skill_id, position } => {
                if c.mechanism_version < 41 {
                    return Err(Error::Unsupported);
                }
                let skill = c.player_skill(skill_id).ok_or(Error::InvalidTarget)?;
                if !skill.is_ground() {
                    return Err(Error::InvalidTarget);
                }
                let point = self.configured_ground_target(i, skill, *position, c)?;
                self.prepare(i, Some(skill.clone()), ([0., 0.], None), id, c, now)?;
                self.members[i].pending.as_mut().unwrap().ground = Some(point);
            }
            Action::CastAtAnchor {
                skill_id,
                anchor_id,
                direction,
            } => {
                if c.mechanism_version < 105 {
                    return Err(Error::Unsupported);
                }
                let skill = c.player_skill(skill_id).ok_or(Error::InvalidTarget)?;
                self.cast_anchor_target(i, skill, anchor_id, c, now)?;
                let direction = match (skill.is_ground(), *direction) {
                    (true, None) => [0., 0.],
                    (false, Some(direction)) => validated_direction(direction)?,
                    _ => return Err(Error::InvalidMessage),
                };
                if now < self.members[i].recovery_until {
                    return Err(Error::InvalidState);
                }
                self.prepare(i, Some(skill.clone()), (direction, None), id, c, now)?;
                self.members[i].pending.as_mut().unwrap().anchor_id = Some(anchor_id.clone());
            }
            Action::StartAttackBeam { direction } => {
                self.start_basic_beam(i, *direction, c, now, &mut events)?
            }
            Action::AimAttackBeam {
                action_id,
                direction,
            } => self.aim_basic_beam(i, action_id, *direction)?,
            Action::EndAttackBeam { action_id } => self.release_basic_beam(i, action_id)?,
            Action::Attack {} => self.attack(i, None, id, c, now)?,
            Action::AttackGround { position } => self.attack_ground(i, *position, id, c, now)?,
            Action::StartBasicCharge {} => self.start_basic_charge(i, c, now)?,
            Action::ReleaseBasicCharge {
                session_id,
                target,
                direction,
                position,
            } => self.release_basic_charge(
                i,
                session_id,
                ChargeAim {
                    target: target.as_ref(),
                    direction: *direction,
                    position: *position,
                },
                id,
                c,
                now,
            )?,
            Action::CancelBasicCharge { session_id } => self.cancel_basic_charge(i, session_id)?,
            Action::AttackUnit { target } => {
                self.attack(i, Some(self.hostile_unit_ref(target)), id, c, now)?
            }
            Action::CompleteModeCheckpoint { stage_id } => {
                self.submit_mode_checkpoint(i, stage_id, c, now, &mut events)?;
            }
            Action::Pickup {
                resource_id,
                expected_revision,
            } => {
                self.pickup_resource(i, (resource_id, *expected_revision), c, now, &mut events)?;
            }
            Action::UseExposureNode {
                node_id,
                expected_revision,
                choice,
            } => {
                self.use_exposure_node(i, (node_id, *expected_revision, *choice), c, now)?;
            }
            Action::ClaimWorldEquipment {
                node_id,
                expected_revision,
            } => {
                self.claim_world_equipment(i, (node_id, *expected_revision), c, now, &mut events)?;
            }
            Action::ClaimBoon {
                opportunity_id,
                boon_id,
                expected_revision,
                expected_inventory_revision,
                replace_slot,
            } => {
                self.claim_boon(
                    i,
                    boons::BoonClaim {
                        opportunity_id,
                        boon_id,
                        expected_revision: *expected_revision,
                        expected_inventory_revision: *expected_inventory_revision,
                        replace_slot: *replace_slot,
                    },
                    c,
                    now,
                    &mut events,
                )?;
            }
            Action::CounterCheckpoint {
                schedule_id,
                window_index,
                target,
            } => {
                self.counter_checkpoint(
                    i,
                    (schedule_id, *window_index, target),
                    c,
                    now,
                    &mut events,
                )?;
            }
            Action::OpenRelic { relic_id } => {
                self.open_relic(i, relic_id, c, &mut events)?;
            }
            Action::ClaimRelic {
                relic_id,
                expected_revision,
                expected_inventory_revision,
                entry_id,
            } => {
                self.claim_relic(
                    i,
                    (
                        relic_id,
                        *expected_revision,
                        *expected_inventory_revision,
                        *entry_id,
                    ),
                    c,
                    &mut events,
                )?;
            }
            Action::OpenChest {
                chest_id,
                expected_generation,
            } => {
                self.open_container(i, (chest_id, *expected_generation), c, now, &mut events)?;
            }
            Action::CancelChestUnlock {
                chest_id,
                expected_generation,
            } => {
                self.cancel_container(i, (chest_id, *expected_generation), c, &mut events)?;
            }
            Action::DropItem {
                item_id,
                quantity,
                expected_revision,
            } => {
                let resource = self.drop_loot(i, item_id, *quantity, *expected_revision, c, now)?;
                events.push((None, EventV4::ResourceSpawned { resource }));
            }
            Action::UseItem {
                item_id,
                expected_revision,
            } => {
                let (hp_restored, spirit_restored) =
                    self.use_item(i, item_id, *expected_revision, c, now, &mut events)?;
                private(
                    &mut events,
                    i,
                    Event::ItemUsed {
                        action_id: id.into(),
                        item_id: item_id.clone(),
                        hp_restored,
                        spirit_restored,
                    },
                );
            }
            Action::ExtractStart {} => {
                if self.has_container_reservation(i) {
                    return Err(Error::InvalidState);
                }
                let m = &mut self.members[i];
                if now < self.active_at.unwrap() + c.extraction_unlock_ms
                    || m.extract_end.is_some()
                    || m.pending.is_some()
                    || m.channel.is_some()
                    || m.queued_cast.is_some()
                    || m.queued_basic.is_some()
                    || m.basic_pursuit.is_some()
                {
                    return Err(Error::InvalidState);
                }
                if distance(m.position, c.extraction_point) > c.params.extraction_range {
                    return Err(Error::OutOfRange);
                }
                m.movement = [0., 0.];
                m.path.clear();
                self.start_extraction_channel(i, c, now, &mut events);
            }
            Action::ExtractCancel {} => {
                self.end_extraction_channel(i, ExtractionChannelState::Interrupted, &mut events);
            }
            Action::Leave {} => self.terminal_intent(i, Outcome::Death, &mut events),
            _ => return Err(Error::Unsupported),
        }
        if skill_action
            || matches!(
                action,
                Action::CancelAction {}
                    | Action::ExtractStart {}
                    | Action::OpenChest { .. }
                    | Action::OpenRelic { .. }
                    | Action::Attack {}
                    | Action::AttackUnit { .. }
                    | Action::AttackGround { .. }
                    | Action::StartAttackBeam { .. }
                    | Action::StartBasicCharge {}
            )
        {
            self.cancel_boon_claim(i, Error::InvalidState, &mut events);
        }
        self.finish_shield_records(c, now, &mut events);
        self.finish_status_cleanses(c, now, &mut events);
        self.finish_window_records(c, now, &mut events);
        self.finish_status_ends(c, now, &mut events);
        self.reconcile_caps(c, now);
        self.finish_training_defeats(now, &mut events);
        Ok(events)
    }
    fn prepare(
        &mut self,
        i: usize,
        skill: Option<SkillConfig>,
        aim: ([f64; 2], Option<EntityRef>),
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let m = &self.members[i];
        let basic = m.basic(c);
        let recovery_replacement = m.can_replace_recovery_action(skill.as_ref(), now);
        if m.pending.is_some()
            || m.basic_pursuit.is_some()
            || m.channel.is_some()
            || m.extract_end.is_some()
            || skill.is_none() && m.basic_delivery.is_some()
            || now < m.recovery_until && !recovery_replacement
        {
            return Err(Error::InvalidState);
        }
        let ready = skill.as_ref().map_or(m.basic_ready, |s| {
            m.cooldowns.get(&s.id).copied().unwrap_or(0)
        });
        if now < ready {
            return Err(Error::InvalidState);
        }
        if let Some(skill) = &skill {
            self.check_skill_budget(i, skill, c)?;
            if recovery_replacement
                && !skill.is_self()
                && !skill.is_ground()
                && !skill.is_line()
                && !skill.targets_ally()
                && !skill.is_unit_effect()
            {
                self.projectile_origin(i, skill.radius)?;
            }
        } else if basic.piercing.is_some()
            || m.active_basic_profile().is_some() && !basic.non_projectile()
        {
            self.projectile_origin(i, c.params.basic_projectile_radius)?;
        }
        let charge = skill.as_ref().and_then(|skill| skill.charge);
        let charge_clock = charge.map(|definition| ChargeClock::new(definition, now, c.step_ms));
        let windup = charge.map_or_else(
            || skill.as_ref().map_or(basic.windup_ms, |s| s.windup_ms),
            |_| charge_clock.unwrap().max_release + charge_clock.unwrap().release_windup - now,
        );
        let id = if charge.is_some() {
            self.next_id("charge")
        } else if skill.as_ref().is_some_and(|skill| skill.channel.is_some()) {
            self.next_id("channel")
        } else if skill.as_ref().is_some_and(|skill| skill.recast.is_some()) {
            self.next_id("recast")
        } else {
            id.into()
        };
        let reservation = skill
            .as_ref()
            .filter(|s| s.resource_commit.is_some_and(|p| p.reserves()))
            .map(|s| ResourceReservationView {
                spirit: self.members[i].effective_spirit_cost(s.cost),
                hp: s
                    .hp_cost
                    .as_ref()
                    .map_or(0, |d| d.evaluate(self.members[i].hp)),
                minimum_hp_after_cost: s.hp_cost.as_ref().map_or(1, |d| d.minimum()),
            });
        if reservation.is_some_and(|r| self.members[i].hp < r.minimum_hp_after_cost) {
            return Err(Error::NoResource);
        }
        let member = &mut self.members[i];
        if recovery_replacement {
            member.end_recovery(now);
            member.queued_cast = None;
        }
        member.queued_basic = None;
        member.pending = Some(Pending {
            checkpoint_epoch: self.checkpoint_epoch,
            charged_basic: None,
            reservation,
            id,
            skill,
            direction: aim.0,
            target: aim.1,
            ground: None,
            anchor_id: None,
            commits: now + windup,
            charge_started_at_ms: charge.map(|_| now),
            aim_updated_at_ms: charge.and_then(|charge| charge.aim_update).map(|_| now),
            recovery_replacement,
            pursuit_windup: false,
            movement_anchor: cast_movement::MovementAnchor::capture(member),
            charge_released_at: None,
            charge_clock,
        });
        Ok(())
    }
    fn check_skill_budget(&self, i: usize, skill: &SkillConfig, c: &Config) -> Result<()> {
        if self.members[i].spirit < self.members[i].effective_spirit_cost(skill.cost) {
            return Err(Error::NoResource);
        }
        if !self.can_statuses(i, c, self.self_statuses_for(i, skill, c)) {
            return Err(Error::Capacity);
        }
        Ok(())
    }

    fn release_charge(
        &mut self,
        i: usize,
        action_id: &str,
        direction: Option<[f64; 2]>,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let member = &mut self.members[i];
        let pending = member
            .pending
            .as_mut()
            .filter(|p| p.id == action_id)
            .ok_or(Error::InvalidTarget)?;
        let (started, charge) = pending.open_charge(now)?;
        let clock = pending.charge_clock.unwrap();
        let direction = direction.map(validated_direction).transpose()?;
        if direction.is_some() && charge.aim_update.is_none() {
            return Err(Error::Unsupported);
        }
        if now < clock.min_release {
            if charge.early_release.is_some() {
                if let Some(direction) = direction {
                    pending.set_charge_direction(charge, direction, now)?;
                }
                pending.commits = clock.min_release + clock.release_windup;
                pending.charge_released_at = Some(now);
                pending.charge_clock.as_mut().unwrap().held_ms = Some(charge.min_ms);
            } else {
                member.pending = None;
                private(events, i, Event::ActionInterrupted);
            }
        } else {
            if let Some(direction) = direction {
                pending.set_charge_direction(charge, direction, now)?;
            }
            pending.commits = now + clock.release_windup;
            pending.charge_released_at = charge.release_windup_ms.map(|_| now);
            pending.charge_clock.as_mut().unwrap().held_ms =
                Some((now - started).min(charge.max_ms));
        }
        Ok(())
    }

    fn commit(&mut self, i: usize, p: Pending, c: &Config, now: u64, events: &mut Events) {
        if c.combat_rules.is_none() {
            self.commit_inner(i, p, c, now, events);
            return;
        }
        let previous = self.triggers.action.replace(p.id.clone());
        let previous_source = self
            .triggers
            .action_source
            .replace(self.members[i].reference.entity.clone());
        let previous_origin = self.triggers.origin.replace(
            p.skill
                .as_ref()
                .map_or(crate::origin::ActionOrigin::Basic, |s| {
                    crate::origin::ActionOrigin::Skill(s.id.clone())
                }),
        );
        self.commit_inner(i, p, c, now, events);
        if !self.triggers.checkpoint_actions.is_empty() {
            self.retain_committed_checkpoint_action(i);
        }
        self.triggers.origin = previous_origin;
        self.triggers.action = previous;
        self.triggers.action_source = previous_source;
    }
    fn commit_inner(
        &mut self,
        i: usize,
        mut p: Pending,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let basic = self.members[i].basic(c);
        let has_basic_profile = self.members[i].active_basic_profile().is_some();
        self.reconcile_caps(c, now);
        if p.skill.is_some() && c.mechanism_version >= 83 && self.skill_silenced(i, now) {
            private(
                events,
                i,
                Event::ActionRejected {
                    action_id: p.id,
                    reason: Error::InvalidState,
                },
            );
            return;
        }
        if p.skill.as_ref().is_some_and(|s| {
            s.resource_commit.is_some()
                && self.members[i]
                    .cooldowns
                    .get(&s.id)
                    .is_some_and(|ready| now < *ready)
        }) {
            private(
                events,
                i,
                Event::ActionRejected {
                    action_id: p.id,
                    reason: Error::InvalidState,
                },
            );
            return;
        }
        let mut ally_target = i;
        if let Some(target) = &p.target {
            let aim = match &p.skill {
                Some(skill) if skill.targets_ally() => {
                    self.ally_skill_target(i, skill, target, c).map(|j| {
                        ally_target = j;
                        [0., 0.]
                    })
                }
                Some(skill) => self.unit_aim(i, skill, target),
                None => match p.charged_basic {
                    Some(ChargedBasic::Wave(stage)) => self.charged_wave_aim(i, target, stage, c),
                    _ => self.basic_target_aim(i, target, c, now),
                },
            };
            match aim {
                Ok(direction) => p.direction = direction,
                Err(reason) => {
                    private(
                        events,
                        i,
                        Event::ActionRejected {
                            action_id: p.id,
                            reason,
                        },
                    );
                    return;
                }
            }
        }
        let (anchor_origin, follows_anchor) = if let Some(anchor_id) = &p.anchor_id {
            let skill = p.skill.as_ref().expect("anchor casts carry a skill");
            match self.cast_anchor_target(i, skill, anchor_id, c, now) {
                Ok((point, follows)) => {
                    if skill.is_ground() {
                        p.ground = Some(point);
                    }
                    (Some(point), follows)
                }
                Err(reason) => {
                    private(
                        events,
                        i,
                        Event::ActionRejected {
                            action_id: p.id,
                            reason,
                        },
                    );
                    return;
                }
            }
        } else {
            (None, false)
        };
        if anchor_origin.is_none()
            && let Some(center) = p.ground
            && let Err(reason) = match p.skill.as_ref() {
                Some(skill) => self.ground_target(i, skill, center),
                None => self.basic_ground_target(i, center, c),
            }
        {
            private(
                events,
                i,
                Event::ActionRejected {
                    action_id: p.id,
                    reason,
                },
            );
            return;
        }
        let projectile_radius = p
            .skill
            .as_ref()
            .filter(|s| {
                !s.is_self()
                    && !s.is_ground()
                    && !s.is_line()
                    && !s.targets_ally()
                    && !s.is_unit_effect()
                    && (s.is_piercing()
                        || s.charge.is_some()
                        || s.channel
                            .clone()
                            .is_some_and(|channel| channel.recovery.is_none())
                        || s.recast.is_some()
                        || s.recovery_cancel.is_some() && s.supports_followup()
                        || p.recovery_replacement
                        || c.skill_followup.is_some() && s.supports_followup())
            })
            .map(|skill| skill.radius)
            .or_else(|| {
                (p.skill.is_none()
                    && (basic.piercing.is_some()
                        || basic.projectile_followup.is_some()
                        || has_basic_profile && !basic.non_projectile()))
                .then_some(c.params.basic_projectile_radius)
            });
        if anchor_origin.is_none()
            && let Some(radius) = projectile_radius
            && let Err(reason) = self.projectile_origin(i, radius)
        {
            private(
                events,
                i,
                Event::ActionRejected {
                    action_id: p.id,
                    reason,
                },
            );
            return;
        }
        let line = if let Some(skill) = p.skill.as_ref().filter(|s| s.is_line()) {
            match self.line_plan(i, skill, p.direction) {
                Ok(v) => Some(v),
                Err(reason) => {
                    private(
                        events,
                        i,
                        Event::ActionRejected {
                            action_id: p.id,
                            reason,
                        },
                    );
                    return;
                }
            }
        } else {
            None
        };
        let channel = p.skill.as_ref().and_then(|skill| skill.channel.clone());
        let summon = p.skill.as_ref().and_then(|skill| c.summon_cast(&skill.id));
        let summon_profile = if let Some(summon) = summon {
            match self.plan_summons(i, summon, c) {
                Ok(profile) => Some(profile),
                Err(reason) => {
                    private(
                        events,
                        i,
                        Event::ActionRejected {
                            action_id: p.id,
                            reason,
                        },
                    );
                    return;
                }
            }
        } else {
            None
        };
        let immediate = p
            .skill
            .as_ref()
            .is_some_and(|s| s.is_self() || s.targets_ally() || s.is_unit_effect());
        let ground = p.skill.is_some() && (p.ground.is_some() || line.is_some());
        let replaces_anchor = p.skill.as_ref().is_some_and(|s| {
            c.member_cast_anchor(&s.id).is_some() && self.member_anchor_index(i, &s.id).is_some()
        });
        let ally_status = p
            .skill
            .as_ref()
            .and_then(|skill| skill.ally_status.as_ref());
        let release_status = basic
            .effects
            .as_ref()
            .and_then(|effects| effects.on_release_self_status.as_ref());
        let self_statuses = p.skill.as_ref().map_or_else(
            || release_status.map_or(&[][..], std::slice::from_ref),
            |skill| self.self_statuses_for(i, skill, c),
        );
        let native_basic = if p.skill.is_none() {
            match self.plan_native_basic(i, &p, c, now) {
                Ok(plan) => plan,
                Err(reason) => {
                    private(
                        events,
                        i,
                        Event::ActionRejected {
                            action_id: p.id,
                            reason,
                        },
                    );
                    return;
                }
            }
        } else {
            None
        };
        let spirit_cost = p.skill.as_ref().map_or(0, |s| {
            p.reservation.map_or_else(
                || self.members[i].effective_spirit_cost(s.cost),
                |r| r.spirit,
            )
        });
        let failed = if self.members[i].spirit < spirit_cost
            || p.reservation.is_some_and(|r| {
                self.members[i]
                    .hp
                    .checked_sub(r.hp)
                    .is_none_or(|left| left < r.minimum_hp_after_cost)
            }) {
            Some(Error::NoResource)
        } else if !self.can_statuses(i, c, self_statuses)
            || ally_status.is_some_and(|id| {
                !self.can_owned_statuses(
                    Owner::Member(i),
                    &self.members[ally_target].reference.entity,
                    std::slice::from_ref(id),
                    c,
                )
            })
            || ground
                && !replaces_anchor
                && self.areas.iter().filter(|area| !area.is_basic()).count()
                    >= c.members.as_ref().unwrap().area_capacity.unwrap()
            || !ground
                && !immediate
                && channel.is_none()
                && self.projectile_slots_used()
                    + native_basic
                        .as_ref()
                        .map_or(1, |plan| plan.projectile_slots())
                    > c.member_settings().unwrap().projectile_capacity
        {
            Some(Error::Capacity)
        } else {
            None
        };
        if let Some(reason) = failed {
            private(
                events,
                i,
                Event::ActionRejected {
                    action_id: p.id,
                    reason,
                },
            );
            return;
        }
        if let Some(skill) = &p.skill
            && self.intercept_cast((Owner::Member(i), &skill.id, Some(&p.id)), c, now, events)
        {
            private(events, i, Event::ActionInterrupted);
            return;
        }
        let charge = p.charge_started_at_ms.map(|started| {
            let definition = p
                .skill
                .as_ref()
                .and_then(|skill| skill.charge)
                .expect("charge definition");
            let held_ms = p.charge_clock.unwrap().held_ms.unwrap_or_else(|| {
                if definition.release_windup_ms.is_some() {
                    definition.max_ms
                } else {
                    p.commits - started
                }
            });
            ChargeCommitView {
                held_ms,
                damage_factor_bps: definition.damage_factor(held_ms),
                direction: p.aim_updated_at_ms.map(|_| p.direction),
            }
        });
        let steal_budget = p
            .skill
            .as_ref()
            .and_then(|s| s.resource_steal.as_ref())
            .zip(p.target.as_ref())
            .and_then(|(d, t)| {
                self.resource_budget(&self.members[i].reference.entity, t, d, c, now)
            });
        let mut cost = 0;
        let hp_cost = p
            .skill
            .as_ref()
            .and_then(|s| s.hp_cost.as_ref())
            .map(|definition| definition.evaluate(self.members[i].hp));
        let hp_cost = p.reservation.map_or(hp_cost, |r| {
            p.skill.as_ref().unwrap().hp_cost.as_ref().map(|_| r.hp)
        });
        if let Some(s) = &p.skill {
            self.members[i].hp -= hp_cost.unwrap_or(0);
            cost = spirit_cost;
            self.members[i].spirit -= cost;
            let cooldown = if c.mechanism_version >= 92 && s.cooldown_ms > 0 {
                let reduction = self.statuses.attribute(
                    &status_entity(&self.members[i].reference.entity),
                    10000,
                    now,
                    c.pve.as_ref().and_then(|p| p.status_rules.as_ref()),
                    StatusAttribute::CooldownReduction,
                ) - 10000;
                let mastery = self.mastery_cooldown_reduction(i, &s.id, c);
                let scaled = u128::from(s.cooldown_ms)
                    * u128::from(10000 - reduction)
                    * u128::from(10000 - mastery)
                    / 100_000_000;
                (scaled as u64).max(s.cooldown_ms.min(c.step_ms))
            } else {
                s.cooldown_ms
            };
            self.members[i]
                .cooldowns
                .insert(s.id.clone(), now + cooldown);
            self.members[i].recovery_skill = (s.recovery_cancel.is_some()
                || c.skill_followup.is_some() && s.supports_followup())
            .then(|| s.id.clone());
            self.members[i].recovery_cancel_at = s
                .recovery_cancel
                .map(|definition| now + definition.after_ms);
            self.members[i].recovery_cancel_policy = s.recovery_cancel;
            if channel.is_none() {
                self.members[i].recovery_until = now + s.recovery_ms;
                self.members[i].recovery_movement = s.cast_movement.unwrap_or_default().recovery;
            }
            self.apply_statuses(
                i,
                (
                    self.members[i].reference.entity.clone(),
                    self.members[i].base_points(c, StatusAttribute::MaxHp),
                ),
                self_statuses,
                c,
                now,
                events,
            );
        } else if basic.settle_on_end {
            let deadline = native_basic.as_ref().map_or_else(
                || now.saturating_add(c.params.player_projectile_ttl_ms),
                basic_delivery::NativeBasicPlan::deadline,
            );
            let finisher_status = self
                .ready_stacked_wave(i, c, now)
                .map(|(_, status)| status.instance_id.clone());
            self.start_basic_delivery(i, &p.id, deadline, finisher_status);
        } else {
            self.start_attack_clock(Owner::Member(i), basic.interval_ms, c, now);
        }
        if p.skill.is_some() {
            self.combat_fact(
                crate::config::CombatFact::SkillCommitted,
                (Owner::Member(i), Owner::Member(i)),
                (0, false),
                c,
                now,
                events,
            );
        }
        // A separate fact preserves the existing self context of SkillCommitted.
        if let Some(target) = p.target.as_ref().filter(|t| {
            p.skill.is_some()
                && self.status_relation(
                    &status_entity(&self.members[i].reference.entity),
                    &status_entity(t),
                ) == Some(combat::Relation::Hostile)
        }) && let Some(other) = self.resolve_entity(target)
        {
            self.combat_fact(
                crate::config::CombatFact::UnitSkillCommitted,
                (Owner::Member(i), other),
                (0, false),
                c,
                now,
                events,
            );
        }
        if p.skill.is_some() {
            self.combat_fact(
                crate::config::CombatFact::SkillCostCommitted,
                (Owner::Member(i), Owner::Member(i)),
                (cost, false),
                c,
                now,
                events,
            );
        }
        if let Some(definition) = &channel {
            let skill = p.skill.as_ref().expect("channel skill");
            self.members[i].channel = Some(ActiveChannel {
                skill: skill.clone(),
                target: p.target.clone(),
                steal_budget,
                action_id: p.id.clone(),
                direction: p.direction,
                raw: self.configured_budget(i, Some(skill), c, now),
                started_at_ms: now,
                ends_at_ms: now + definition.duration_ms,
                next_pulse_at_ms: now + definition.interval_ms,
                pulses: 0,
                movement_anchor: cast_movement::MovementAnchor::capture(&self.members[i]),
                recovery_budget: definition.recovery.map(|recovery| {
                    let attack = self.recovery_power(Owner::Member(i), &skill.id, c, now);
                    [
                        recovery.hp.map_or(0, |f| {
                            f.amount(
                                attack,
                                self.entity_max_hp(
                                    &self.members[ally_target].reference.entity,
                                    c,
                                    now,
                                )
                                .unwrap(),
                            )
                        }),
                        recovery.spirit.map_or(0, |f| {
                            f.amount(attack, self.member_max_spirit(ally_target, c, now))
                        }),
                    ]
                }),
            });
        }
        let recast_action = p.skill.as_ref().and_then(|skill| {
            skill
                .recast
                .map(|recast| (p.id.clone(), skill.id.clone(), recast))
        });
        if p.skill.as_ref().is_none_or(|skill| {
            !skill.is_self()
                && !skill.targets_ally()
                && !skill.is_ground()
                && !skill.is_line()
                && skill.channel.is_none()
        }) {
            self.register_checkpoint_action(i, &p.id, p.checkpoint_epoch, now);
        }
        private(
            events,
            i,
            Event::ActionCommitted {
                ground_position: p.ground,
                unit_target: p
                    .target
                    .as_ref()
                    .filter(|_| p.skill.is_some() || c.mechanism_version >= 54)
                    .map(Self::unit_wire),
                skill_id: p.skill.as_ref().map(|s| s.id.clone()),
                action_id: p.id.clone(),
                skill: p.skill.is_some(),
                spirit_cost: cost,
                hp_cost,
                charge,
            },
        );
        if let Some((summon, profile)) = summon.zip(summon_profile) {
            self.start_summons(i, (&p.skill.as_ref().unwrap().id, summon, profile), c, now);
        }
        if let Some(skill) = p
            .skill
            .as_ref()
            .filter(|s| s.is_unit_effect() && channel.is_none())
        {
            self.unit_effect(
                (i, p.target.as_ref().unwrap(), skill, &p.id),
                self.configured_budget(i, Some(skill), c, now),
                c,
                now,
                events,
            );
        }
        if channel.is_none()
            && let (Some(definition), Some(budget), Some(target)) = (
                p.skill.as_ref().and_then(|s| s.resource_steal.as_ref()),
                steal_budget,
                p.target.as_ref(),
            )
        {
            self.steal_resources(
                (Owner::Member(i), target, Some(&p.id)),
                definition,
                budget,
                c,
                now,
                events,
            );
        }
        if let Some(skill) = &p.skill {
            if let Some(row) = self.ally_recovery_for(i, &skill.id, c) {
                self.resolve_ally_recovery((i, ally_target), (&skill.id, row), c, now, events);
            } else if let Some(heal) = &skill.instant_heal {
                self.resolve_skill_heal((i, ally_target), heal, &skill.id, c, now, events);
            }
        }
        if let Some(id) = ally_status {
            let member = &self.members[ally_target];
            self.apply_owned_statuses(
                Owner::Member(i),
                (
                    member.reference.entity.clone(),
                    member.base_points(c, StatusAttribute::MaxHp),
                ),
                std::slice::from_ref(id),
                c,
                now,
                events,
            );
        }
        if p.skill.as_ref().is_some_and(|s| s.targets_ally()) {
            self.combat_fact(
                crate::config::CombatFact::AllySkillCommitted,
                (Owner::Member(i), Owner::Member(ally_target)),
                (0, false),
                c,
                now,
                events,
            );
        }
        if immediate || channel.is_some() {
            return;
        }
        if let Some(plan) = native_basic {
            match plan {
                basic_delivery::NativeBasicPlan::Wave(wave) => {
                    self.start_basic_wave(i, wave, events);
                    self.apply_basic_self_status(i, release_status, c, now, events);
                }
                basic_delivery::NativeBasicPlan::Impact(impact) => {
                    self.start_basic_impact(i, impact, release_status, c, now, events);
                }
                basic_delivery::NativeBasicPlan::Projectile(projectile) => {
                    self.apply_basic_self_status(i, release_status, c, now, events);
                    self.start_basic_projectile(i, projectile, events);
                }
                basic_delivery::NativeBasicPlan::Dash(dash) => {
                    self.apply_basic_self_status(i, release_status, c, now, events);
                    self.start_basic_dash(i, &p, dash, c, now, events);
                }
                basic_delivery::NativeBasicPlan::Flight(shot) => {
                    self.apply_basic_self_status(i, release_status, c, now, events);
                    self.start_charged_flight(*shot, events);
                }
            }
            return;
        }
        if let Some(center) = p.ground {
            let attached_anchor_id = p.anchor_id.filter(|_| follows_anchor);
            self.start_area(
                i,
                p.skill.as_ref().unwrap(),
                (center, None, attached_anchor_id),
                c,
                now,
                events,
            );
            return;
        }
        if let Some(line) = line {
            self.start_area(
                i,
                p.skill.as_ref().unwrap(),
                (line.start, Some(line), None),
                c,
                now,
                events,
            );
            return;
        }
        let raw = self.configured_budget(i, p.skill.as_ref(), c, now);
        if p.skill.is_none() {
            self.apply_basic_self_status(i, release_status, c, now, events);
        }
        let raw = charge.map_or(raw, |charge| {
            (u64::from(raw) * u64::from(charge.damage_factor_bps) / 10_000) as u32
        });
        let projectile_id = self.spawn_member_shot(
            i,
            MemberShot {
                action_id: None,
                start_position: anchor_origin,
                skill: p.skill.as_ref(),
                direction: p.direction,
                target: p
                    .target
                    .filter(|_| p.skill.is_some() || has_basic_profile && basic.piercing.is_none()),
                raw,
            },
            c,
            now,
            events,
        );
        if let Some((action_id, skill_id, recast)) = recast_action {
            self.members[i].recast = Some(RecastOpportunity {
                view: ProjectileRecastView {
                    action_id,
                    skill_id,
                    projectile_id: projectile_id.to_owned(),
                    expires_at_ms: now + recast.window_ms,
                    next_index: 1,
                },
                spirit_cost: recast.spirit_cost,
            });
        }
    }

    fn member_raw_attack(&self, i: usize, bps: u32, is_skill: bool, c: &Config, now: u64) -> u32 {
        let mut power = self.stats(i, c, now).0;
        if is_skill && c.mechanism_version >= 104 {
            let factor = self.statuses.attribute(
                &status_entity(&self.members[i].reference.entity),
                10000,
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
                StatusAttribute::SkillPowerBonus,
            );
            power = (u64::from(power) * u64::from(factor) / 10000).min(u64::from(u32::MAX)) as u32;
        }
        (u64::from(power) * u64::from(bps) / 10000).min(u64::from(u32::MAX)) as u32
    }

    fn spawn_member_shot(
        &mut self,
        i: usize,
        plan: MemberShot<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> &str {
        let basic = self.members[i].basic(c);
        let (range, speed, radius, ttl) = plan.skill.map_or(
            (
                basic.range,
                basic.projectile_speed,
                c.params.basic_projectile_radius,
                c.params.player_projectile_ttl_ms,
            ),
            |s| {
                (
                    s.range,
                    s.speed,
                    s.radius,
                    if s.is_piercing() {
                        s.projectile_ttl_ms
                    } else {
                        c.params.player_projectile_ttl_ms
                    },
                )
            },
        );
        let piercing = match plan.skill {
            Some(s) if s.is_piercing() => Some((
                s.id.as_str(),
                s.max_hits,
                s.projectile_repeat_interval_ms.unwrap_or(0),
            )),
            None => basic
                .piercing
                .as_ref()
                .map(|p| (p.plan_id.as_str(), p.max_hits, 0)),
            _ => None,
        };
        let contact = piercing.map(|(_, max_hits, repeat)| {
            crate::contact::ContactBudget::projectile(max_hits, repeat)
        });
        let view = ProjectileViewV4 {
            basic_segment: None,
            piercing: piercing.map(|(id, max_hits, repeat_interval_ms)| PiercingPlanViewV4 {
                skill_id: id.to_owned(),
                direction: plan.direction,
                speed,
                remaining_range: range,
                created_at_ms: now,
                expires_at_ms: now + ttl,
                max_hits,
                used_hits: 0,
                remaining_hits: max_hits,
                repeat_interval_ms,
                contacts: vec![],
            }),
            unit_target: plan.target.as_ref().map(Self::unit_wire),
            id: self.next_id("shot"),
            source: self.members[i].reference.entity.clone(),
            position: plan.start_position.unwrap_or(self.members[i].position),
            radius,
        };
        events.push((
            None,
            EventV4::ProjectileStarted {
                projectile: view.clone(),
            },
        ));
        self.shots.push(Shot {
            end_on_source_invalid: plan.skill.is_some_and(|s| s.end_on_source_invalid),
            flight: None,
            origin: plan.skill.map_or(crate::origin::ActionOrigin::Basic, |s| {
                crate::origin::ActionOrigin::Skill(s.id.clone())
            }),
            action_id: plan
                .action_id
                .map(str::to_owned)
                .or_else(|| {
                    self.triggers
                        .action
                        .clone()
                        .filter(|_| self.triggers.action_source.as_ref() == Some(&view.source))
                })
                .unwrap_or_else(|| view.id.clone()),
            damage_rule: self.attack_rule(Owner::Member(i), plan.skill.map(|s| s.id.as_str()), c),
            ignore_armor: c.mechanism_version >= 103
                && plan.skill.map_or(basic.ignore_armor, |s| s.ignore_armor) == Some(true),
            unavoidable: c.mechanism_version >= 102
                && plan.skill.map_or(basic.unavoidable, |s| s.unavoidable) == Some(true),
            contact,
            view,
            target: plan.target,
            owner: Owner::Member(i),
            direction: plan.direction,
            remaining: range,
            speed,
            raw: plan.raw,
            metal: plan.skill.is_some(),
            hit_statuses: plan
                .skill
                .map_or_else(Vec::new, |skill| skill.on_hit_target_statuses.clone()),
            created: now,
            expires: now + ttl,
        });
        &self.shots.last().expect("new projectile").view.id
    }
    pub(crate) fn tick(&mut self, c: &Config, now: u64) -> Events {
        self.prune_trigger_actions();
        self.reconcile_caps(c, now);
        let mut events = vec![];
        let Some(active) = self.active_at else {
            return events;
        };
        self.set_contested_clock(now);
        self.advance_contested_equipment(now);
        self.advance_encounter_progress(c, now);
        if self.mode_before_step(c, now, &mut events) {
            return events;
        }
        let previous = self.areas.iter().any(areas::AreaPlan::is_moving).then(|| {
            self.members
                .iter()
                .map(|member| member.position)
                .collect::<Vec<_>>()
        });
        if now >= active + c.terminal_ms {
            if c.extraction_lifecycle
                .as_ref()
                .is_some_and(|rule| rule.pressure.is_some())
            {
                self.force_extraction_deadline(&mut events);
            } else {
                for i in 0..self.members.len() {
                    if self.members[i].living() || self.training_respawn_at(i).is_some() {
                        self.terminal_intent(i, Outcome::Death, &mut events)
                    }
                }
            }
            self.expire_contested_equipment(&mut events);
            self.finish_status_cleanses(c, now, &mut events);
            self.finish_window_records(c, now, &mut events);
            self.finish_status_ends(c, now, &mut events);
            return events;
        }
        self.resources.retain(|r| now < r.view.expires_at_ms);
        self.expire_summons(now, &mut events);
        self.advance_world_geometry(c, now.saturating_sub(active));
        self.advance_training_lifecycle(c, now, &mut events);
        self.advance_zone_exposure(c, now);
        if self.periodic_enabled {
            self.prepare_periodic(now, &mut events);
            self.finish_shield_records(c, now, &mut events);
            self.finish_status_cleanses(c, now, &mut events);
            self.finish_window_records(c, now, &mut events);
            self.finish_status_ends(c, now, &mut events);
            self.reconcile_caps(c, now);
            if c.world_boons.is_some() {
                for i in 0..self.members.len() {
                    self.rebuild_boon_shield(i, c, now, &mut events);
                }
            }
            self.advance_periodic(c, now, &mut events);
        } else {
            self.statuses.expire(now, &mut vec![]);
        }
        self.advance_terrain_damage(c, now, &mut events);
        self.advance_extraction_pressure(c, active, now, &mut events);
        self.settle_pending_basic_beams(&mut events);
        self.advance_participant_ai(c, now, &mut events);
        let dt = c.step_ms as f64 / 1000.;
        for i in 0..self.members.len() {
            self.expire_basic_charge(i, c, now);
            if !self.members[i].eligible()
                || (c.mechanism_version >= 89
                    && self.member_has_status(i, StatusAttribute::Stun, now))
            {
                continue;
            }
            if self.members[i]
                .channel
                .as_ref()
                .is_none_or(|p| p.target.is_none())
            {
                self.advance_channel(i, c, now, &mut events);
            }
            self.promote_followup(i, c, now, &mut events);
            self.promote_basic(i, c, now, &mut events);
            if self.members[i].basic_pursuit.is_some() {
                self.advance_basic_pursuit(i, c, now, dt, &mut events);
            } else if c.mechanism_version < 88
                || !self.member_has_status(i, StatusAttribute::Root, now)
            {
                let speed_factor = self.members[i].phase_movement(now).speed_factor();
                if speed_factor > 0.0 {
                    let budget = self.member_movement_budget(i, c, now, dt) * speed_factor;
                    let m = &mut self.members[i];
                    let goal = m.path.first().copied();
                    if let Some(goal) = goal {
                        m.movement = normalize([goal[0] - m.position[0], goal[1] - m.position[1]]);
                    }
                    let step = goal.map_or(budget, |g| budget.min(distance(m.position, g)));
                    let end = [
                        m.position[0] + m.movement[0] * step,
                        m.position[1] + m.movement[1] * step,
                    ];
                    if self.grid.line(m.position, end, c.params.player_radius, 0) {
                        m.position = end;
                    }
                    if goal.is_some_and(|g| distance(g, m.position) < 1e-6) {
                        m.path.remove(0);
                        if m.path.is_empty() {
                            m.movement = [0., 0.];
                        }
                    }
                }
            }
            let m = &mut self.members[i];
            if m.pending.as_ref().is_some_and(|p| {
                p.commits <= now
                    && !p.skill.as_ref().is_some_and(|s| {
                        s.is_unit_effect() || s.targets_ally() && s.channel.is_some()
                    })
            }) {
                let p = m.pending.take().unwrap();
                self.commit(i, p, c, now, &mut events);
            }
        }
        self.finish_shield_records(c, now, &mut events);
        self.finish_status_cleanses(c, now, &mut events);
        self.finish_window_records(c, now, &mut events);
        self.finish_status_ends(c, now, &mut events);
        self.advance_summons(c, now, &mut events);
        self.advance_npcs(c, now, dt, &mut events);
        self.advance_shots(c, now, dt, &mut events);
        self.advance_basic_projectiles(c, now, dt, &mut events);
        self.prune_recasts(now);
        self.advance_npc_areas(previous.as_deref(), c, now, &mut events);
        self.advance_areas(c, now, &mut events);
        self.advance_basic_waves(c, now, &mut events);
        self.advance_basic_impacts(c, now, &mut events);
        // Strict unit plans observe this step's completed movement and controls.
        // Legacy projectile/recovery plans retain their established ordering.
        for i in 0..self.members.len() {
            if self.members[i].eligible()
                && !self.member_has_status(i, StatusAttribute::Stun, now)
                && !self.member_has_status(i, StatusAttribute::Silence, now)
                && self.members[i].pending.as_ref().is_some_and(|p| {
                    p.commits <= now
                        && p.skill.as_ref().is_some_and(|s| {
                            s.is_unit_effect() || s.targets_ally() && s.channel.is_some()
                        })
                })
            {
                let pending = self.members[i].pending.take().unwrap();
                self.commit(i, pending, c, now, &mut events);
            }
            if self.members[i]
                .channel
                .as_ref()
                .is_some_and(|p| p.target.is_some())
            {
                self.advance_channel(i, c, now, &mut events);
            }
        }
        self.advance_basic_beams(c, now, &mut events);
        if now >= self.next_regen {
            self.next_regen = now + c.params.regen.interval_ms;
            let rules = c.pve.as_ref().and_then(|p| p.status_rules.as_ref());
            let member_limits = (0..self.members.len())
                .map(|i| {
                    (
                        self.entity_max_hp(&self.members[i].reference.entity, c, now)
                            .unwrap(),
                        self.member_max_spirit(i, c, now),
                    )
                })
                .collect::<Vec<_>>();
            let npc_limits = (0..self.npcs.len())
                .map(|j| {
                    (
                        self.entity_max_hp(&self.npcs[j].entity, c, now).unwrap(),
                        self.entity_max_spirit(&self.npcs[j].entity, c, now)
                            .unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            for (index, m) in self.members.iter_mut().enumerate() {
                if !m.living() {
                    continue;
                }
                let shared_near = self.training_shared_seats.iter().any(|&owner| {
                    self.npcs.iter().any(|n| {
                        n.hp > 0
                            && n.training_owner == Some(owner)
                            && distance(n.position, m.position) <= c.params.regen.near_radius
                    })
                });
                let near = self.npcs.iter().any(|n| {
                    n.hp > 0
                        && (n.training_owner.is_none_or(|owner| owner == index) || shared_near)
                        && distance(n.position, m.position) <= c.params.regen.near_radius
                });
                let ratio = if near {
                    c.params.regen.near_ratio
                } else {
                    c.params.regen.far_ratio
                };
                let strength = if c.mechanism_version >= 72 {
                    f64::from(self.statuses.attribute(
                        &status_entity(&m.reference.entity),
                        10000,
                        now,
                        rules,
                        StatusAttribute::RecoveryStrength,
                    )) / 10000.0
                } else {
                    1.0
                };
                m.hp =
                    m.hp.saturating_add(
                        (f64::from(member_limits[index].0) * ratio * strength).floor() as u32,
                    )
                    .min(member_limits[index].0);
                let spirit = (f64::from(member_limits[index].1) * ratio * strength).floor() as u32;
                m.spirit +=
                    spirit.min(m.spirit_recovery_gap(&self.statuses, now, member_limits[index].1));
            }
            for (index, n) in self.npcs.iter_mut().enumerate() {
                if n.hp == 0 {
                    continue;
                }
                let near = self.members.iter().enumerate().any(|(i, m)| {
                    m.living()
                        && n.training_owner.is_none_or(|owner| owner == i)
                        && distance(n.position, m.position) <= c.params.regen.near_radius
                });
                let ratio = if near {
                    c.params.regen.near_ratio
                } else {
                    c.params.regen.far_ratio
                };
                n.hp =
                    n.hp.saturating_add((f64::from(npc_limits[index].0) * ratio).floor() as u32)
                        .min(npc_limits[index].0);
                n.spirit = n
                    .spirit
                    .saturating_add((f64::from(npc_limits[index].1) * ratio).floor() as u32)
                    .min(npc_limits[index].1);
            }
        }
        for i in 0..self.members.len() {
            if self.training
                && self.members[i].hp == 0
                && self.members[i].phase == MemberPhase::Active
            {
                self.handle_member_defeat(i, c, now, &mut events);
            }
            if self.members[i].eligible()
                && self.members[i].extract_end.is_some_and(|end| now >= end)
            {
                self.terminal_intent(i, Outcome::Extraction, &mut events)
            }
        }
        self.finish_trigger_actions(c, now, &mut events);
        self.finish_shield_records(c, now, &mut events);
        self.finish_status_cleanses(c, now, &mut events);
        self.finish_window_records(c, now, &mut events);
        self.finish_status_ends(c, now, &mut events);
        self.advance_checkpoints(c, active, now, &mut events);
        self.advance_world_loot(c, now, &mut events);
        self.advance_boon_claims(c, now, &mut events);
        self.mode_after_step(c, now, &mut events);
        self.finish_training_defeats(now, &mut events);
        self.sample_training_metrics(now);
        events
    }
    fn advance_world_geometry(&mut self, c: &Config, elapsed_ms: u64) {
        if !self
            .grid
            .config
            .world_geometry
            .as_ref()
            .is_some_and(|geometry| {
                geometry
                    .blockers
                    .iter()
                    .any(|blocker| blocker.active_window.is_some())
            })
        {
            return;
        }
        let occupants: Vec<_> = self
            .members
            .iter()
            .filter(|m| m.living())
            .map(|m| (m.position, c.params.player_radius))
            .chain(
                self.npcs
                    .iter()
                    .filter(|n| n.hp > 0 && n.training_owner.is_none())
                    .map(|n| (n.position, n.profile.combat.radius)),
            )
            .chain(
                self.summons
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| self.summon_alive(*i))
                    .map(|(_, summon)| (summon.position, summon.profile.combat.radius)),
            )
            .collect();
        if !self.grid.advance_blockers(elapsed_ms, &occupants) {
            return;
        }
        for member in &mut self.members {
            if let Some(&destination) = member.path.last() {
                let route = self
                    .grid
                    .route(member.position, destination, c.params.player_radius);
                if !route.is_empty() {
                    member.path = route;
                }
            }
        }
        for npc in &mut self.npcs {
            npc.path.clear();
            npc.repath = 0;
        }
    }

    fn advance_npcs(&mut self, c: &Config, now: u64, dt: f64, events: &mut Events) {
        let ai = c.pve.as_ref().unwrap().npc_ai.as_ref();
        let help = self.npc_help_signals();
        for j in 0..self.npcs.len() {
            if self.npcs[j].hp == 0
                || !self.mode_npc_enabled(&self.npcs[j].entity.id)
                || !self.npcs[j].pursuit_can_fight()
            {
                continue;
            }
            if self.npcs[j].training_owner.is_some()
                && !matches!(
                    self.training_target_behavior(j, c),
                    Some(crate::config::training_targets::TrainingTargetBehavior::Combat {})
                )
            {
                self.move_training_dummy(j, c, now, dt);
                continue;
            }
            self.advance_npc_boss(j, c, now, events);
            let stunned = c.combat_rules.is_some()
                && self.entity_has_status(Owner::Npc(j), StatusAttribute::Stun, now);
            if stunned && ai.is_none() {
                continue;
            }
            let pursuit = self.npcs[j].pursuit;
            let n_target = if let Some(contested_equipment::Pursuit::Tracking(i)) = pursuit {
                self.select_pursuit_target(j, i)
            } else if let Some(ai) = ai {
                self.select_npc_ai_target(j, ai, &help, now)
            } else {
                self.select_legacy_npc_target(j)
            };
            let next_target = n_target.as_ref().map(|t| &t.1);
            if self.npcs[j].target.as_ref() != next_target {
                let n = &mut self.npcs[j];
                n.target = next_target.cloned();
                if ai.is_some() && !n.returning {
                    n.path.clear();
                    n.repath = 0;
                    n.ai.decide_at = 0;
                    n.ai.retreating = false;
                }
            }
            if stunned {
                continue;
            }
            let npc_silenced = c.combat_rules.is_some()
                && self.entity_has_status(Owner::Npc(j), StatusAttribute::Silence, now);
            let movement_factor = self.npc_movement_factor(j, c, now);
            let effective_spd = self.effective_spd(Owner::Npc(j), c, now);
            let Some((target_owner, target_entity, target, d)) = n_target else {
                if pursuit.is_some() {
                    let n = &mut self.npcs[j];
                    n.path.clear();
                    n.repath = 0;
                    if n.casting.take().is_some() {
                        let source = n.entity.clone();
                        self.stop_npc_areas(&source, AreaEndReason::Cancelled, events);
                    }
                    continue;
                }
                let n = &self.npcs[j];
                let returning = n.returning;
                let geometry_enabled = self.grid.config.world_geometry.is_some() || ai.is_some();
                let at_home = distance(n.position, n.home) <= n.profile.combat.return_distance
                    && (!geometry_enabled
                        || self
                            .grid
                            .line(n.position, n.home, n.profile.combat.radius, 0));
                if at_home && !returning && self.npc_ai_profile(j, c).is_some() {
                    if n.casting.is_some() {
                        self.advance_npc_cast(j, c, now, events);
                        continue;
                    }
                    if !npc_silenced && self.try_npc_skill(j, None, c, now, events) {
                        continue;
                    }
                }
                if self.periodic_enabled && returning && at_home {
                    let old = self.npcs[j].entity.clone();
                    self.clear_owned_statuses(&old, StatusRemovalReason::Reset, events);
                    self.stop_npc_areas(&old, AreaEndReason::SourceInvalid, events);
                    self.npcs[j].entity.generation += 1;
                    self.shots.retain(|s| s.view.source != old);
                }
                if returning && at_home {
                    self.clear_monster_contribution(j);
                }
                if ai.is_some() && self.npcs[j].casting.is_some() {
                    let entity = self.npcs[j].entity.clone();
                    self.stop_npc_areas(&entity, AreaEndReason::Cancelled, events);
                }
                let n = &mut self.npcs[j];
                n.casting = None;
                if !at_home {
                    if geometry_enabled && !n.returning {
                        n.path.clear();
                        n.repath = 0;
                    }
                    n.returning = true;
                    if geometry_enabled {
                        let budget = c.params.movement.speed(effective_spd)
                            * self.grid.speed(n.position)
                            * dt
                            * movement_factor;
                        Self::follow_npc_path(
                            &self.grid,
                            n,
                            n.home,
                            budget,
                            now,
                            c.params.runtime.nav_repath_ms,
                        );
                    } else {
                        let budget = c.params.movement.speed(effective_spd) * dt * movement_factor;
                        let d = normalize([n.home[0] - n.position[0], n.home[1] - n.position[1]]);
                        let step = budget.min(distance(n.position, n.home));
                        let end = [n.position[0] + d[0] * step, n.position[1] + d[1] * step];
                        if self.grid.line(n.position, end, n.profile.combat.radius, 0) {
                            n.position = end;
                        }
                    }
                } else if n.returning {
                    if geometry_enabled {
                        n.path.clear();
                        n.repath = 0;
                    }
                    n.position = n.home;
                    n.returning = false;
                    n.alerted = false;
                    n.hp = n.profile.stats["hp"];
                    n.spirit = n.profile.stats["spirit"];
                    if ai.is_some() {
                        n.ai = Default::default();
                        n.next_attack = 0;
                        n.next_cast = 0;
                        n.attack_clock = None;
                        n.recovery = 0;
                    }
                    n.boss_phase = 0;
                    if let Some(phase) = n.boss_view() {
                        events.push((
                            None,
                            EventV4::BossReset {
                                entity: n.entity.clone(),
                                phase,
                            },
                        ));
                    }
                } else {
                    n.alerted = false;
                }
                continue;
            };
            if self.npcs[j].casting.is_some() {
                self.advance_npc_cast(j, c, now, events);
                continue;
            }
            if now < self.npcs[j].recovery {
                continue;
            }
            let profile = self.npcs[j].profile.clone();
            let ai_profile = self.npc_ai_profile(j, c);
            if let Some(ai_profile) = ai_profile {
                let movement = c.params.movement.speed(effective_spd)
                    * self.grid.speed(self.npcs[j].position)
                    * dt
                    * movement_factor;
                if self.retreat_npc(j, ai_profile, (target, d), movement) {
                    continue;
                }
                if !npc_silenced
                    && self.try_npc_skill(j, Some((&target_entity, target, d)), c, now, events)
                {
                    continue;
                }
            } else if matches!(target_owner, Owner::Member(_))
                && !npc_silenced
                && now >= self.npcs[j].next_cast
                && d <= profile.combat.projectile.range
                && self.npcs[j].spirit >= profile.combat.projectile.cost
                && self.grid.line(self.npcs[j].position, target, 0., 2)
                && self.roll(profile.combat.cast_chance)
            {
                let direction = normalize([
                    target[0] - self.npcs[j].position[0],
                    target[1] - self.npcs[j].position[1],
                ]);
                let skill_id = &profile.combat.projectile.id;
                let prepared = if let Some(delivery) = c.npc_ground_delivery(skill_id) {
                    self.prepare_ground_area(j, target, delivery, c, now, events)
                } else {
                    direction != [0., 0.]
                        && c.npc_enter_area(skill_id).is_none_or(|rule| {
                            self.prepare_enter_area(j, (target, direction), rule, c, now, events)
                        })
                };
                if prepared {
                    self.npcs[j].casting = Some(NpcCast::legacy(
                        now + profile.combat.projectile.windup_ms,
                        direction,
                    ));
                    continue;
                }
            }
            if d <= profile.combat.melee_range
                && self.grid.line(self.npcs[j].position, target, 0., 1)
            {
                if now >= self.npcs[j].next_attack {
                    self.start_attack_clock(
                        Owner::Npc(j),
                        self.npcs[j]
                            .boss_phase()
                            .map_or(profile.combat.attack_interval_ms, |phase| {
                                phase.attack_interval_ms
                            }),
                        c,
                        now,
                    );
                    if let Owner::Member(i) = target_owner
                        && (self.member_damage_immune(i, c, now)
                            || self.member_dodges_npc_contact(i, j, c, now))
                    {
                        continue;
                    }
                    let e = status_entity(&self.npcs[j].entity);
                    let mut raw = self.statuses.attribute(
                        &e,
                        profile.stats["atk"],
                        now,
                        c.pve.as_ref().unwrap().status_rules.as_ref(),
                        StatusAttribute::Atk,
                    );
                    if c.combat_rules.is_none()
                        && self
                            .roll(f64::from(profile.stats.get("crit").copied().unwrap_or(0)) / 100.)
                    {
                        raw = (f64::from(raw) * c.params.critical_multiplier)
                            .floor()
                            .min(f64::from(u32::MAX)) as u32;
                    }
                    let raw = self.npc_attack_budget(j, raw, 10000, false, c, now);
                    let source = self.npcs[j].entity.clone();
                    if let Owner::Member(i) = target_owner {
                        self.hurt_member(i, source, raw, c, now, events);
                    } else {
                        self.direct_contact(
                            Owner::Npc(j),
                            target_owner,
                            combat::DirectContact {
                                origin: Some(crate::origin::ActionOrigin::Basic),
                                action_id: None,
                                rule: None,
                                ignore_armor: false,
                                unavoidable: false,
                                dead_source: false,
                                source: &source,
                                raw,
                                metal: false,
                                hit_statuses: &[],
                            },
                            c,
                            now,
                            events,
                        );
                    }
                }
            } else {
                let n = &mut self.npcs[j];
                let budget = c.params.movement.speed(effective_spd)
                    * self.grid.speed(n.position)
                    * dt
                    * movement_factor;
                Self::follow_npc_path(
                    &self.grid,
                    n,
                    target,
                    budget,
                    now,
                    c.params.runtime.nav_repath_ms,
                );
            }
        }
    }
    fn select_legacy_npc_target(&mut self, j: usize) -> Option<npc_ai::NpcTarget> {
        let candidates = || {
            self.members
                .iter()
                .enumerate()
                .filter(|(_, m)| m.eligible())
                .map(|(i, m)| (Owner::Member(i), &m.reference.entity, m.position))
                .chain(
                    self.summons
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| self.summon_eligible(*i))
                        .map(|(i, summon)| (Owner::Summon(i), &summon.entity, summon.position)),
                )
                .filter(|(owner, _, _)| self.same_training_scope(Owner::Npc(j), *owner))
        };
        let alerted = self.npcs[j].alerted
            || candidates().any(|(_, _, p)| {
                distance(p, self.npcs[j].position) <= self.npcs[j].profile.combat.aggro_range
            })
            || self.npcs.iter().enumerate().any(|(k, n)| {
                k != j
                    && self.same_training_scope(Owner::Npc(j), Owner::Npc(k))
                    && n.hp > 0
                    && n.alerted
                    && !n.returning
                    && distance(n.position, self.npcs[j].position)
                        <= self.npcs[j].profile.help_range
            });
        let range = if alerted {
            self.npcs[j].profile.memory_range
        } else {
            self.npcs[j].profile.combat.aggro_range
        };
        let return_home = distance(self.npcs[j].position, self.npcs[j].home)
            > self.npcs[j].profile.leash
            || self.npcs[j].returning;
        let n_target = if return_home {
            None
        } else {
            candidates()
                .map(|(owner, entity, p)| (owner, entity, p, distance(p, self.npcs[j].position)))
                .filter(|(_, _, _, d)| *d <= range)
                .min_by(|a, b| a.3.total_cmp(&b.3).then(a.1.id.cmp(&b.1.id)))
                .map(|(owner, entity, p, d)| (owner, entity.clone(), p, d))
        };
        self.npcs[j].alerted = alerted;
        n_target
    }
    fn follow_npc_path(
        grid: &super::pve::Grid,
        npc: &mut Npc,
        destination: [f64; 2],
        budget: f64,
        now: u64,
        repath_ms: u64,
    ) {
        let radius = npc.profile.combat.radius;
        if now >= npc.repath || npc.path.is_empty() {
            npc.path = grid.route(npc.position, destination, radius);
            npc.repath = now + repath_ms;
        }
        if let Some(goal) = npc.path.first().copied() {
            let dir = normalize([goal[0] - npc.position[0], goal[1] - npc.position[1]]);
            let step = budget.min(distance(npc.position, goal));
            let end = [
                npc.position[0] + dir[0] * step,
                npc.position[1] + dir[1] * step,
            ];
            if grid.line(npc.position, end, radius, 0) {
                npc.position = end;
            }
            if distance(npc.position, goal) < 1e-6 {
                npc.path.remove(0);
            }
        }
    }
    fn advance_shots(&mut self, c: &Config, now: u64, dt: f64, events: &mut Events) {
        for mut s in std::mem::take(&mut self.shots) {
            let is_copy = matches!(s.owner, Owner::Summon(_));
            let valid = match s.owner {
                // end_on_source_invalid keeps the frozen payload flying.
                Owner::Member(i) => s.end_on_source_invalid || self.members[i].living(),
                Owner::Npc(i) => {
                    self.npcs[i].hp > 0
                        && self.npcs[i].pursuit_can_fight()
                        && (!self.periodic_enabled || self.npcs[i].entity == s.view.source)
                }
                Owner::Summon(_) => {
                    if let Some(owner @ Owner::Summon(i)) = self.resolve_entity(&s.view.source) {
                        s.owner = owner;
                        self.summon_eligible(i)
                    } else {
                        false
                    }
                }
            };
            if !valid
                || now >= s.expires
                || s.target
                    .as_ref()
                    .is_some_and(|target| !self.hostile_contact(&s.view.source, target))
            {
                events.push((
                    None,
                    EventV4::ProjectileEnded {
                        reason: (s.contact.is_some() || s.flight.is_some() || is_copy).then_some({
                            if !valid {
                                ProjectileEndReasonV4::SourceInvalid
                            } else if is_copy && now < s.expires {
                                ProjectileEndReasonV4::TargetInvalid
                            } else {
                                ProjectileEndReasonV4::Expired
                            }
                        }),
                        projectile_id: s.view.id,
                    },
                ));
                continue;
            }
            if now <= s.created {
                self.shots.push(s);
                continue;
            }
            if s.flight.is_some() {
                self.advance_charged_flight(s, c, now, dt, events);
                continue;
            }
            if s.contact.is_some() {
                self.advance_piercing_shot(s, c, now, dt, events);
                continue;
            }
            let travel = (s.speed * dt).min(s.remaining);
            let end = [
                s.view.position[0] + s.direction[0] * travel,
                s.view.position[1] + s.direction[1] * travel,
            ];
            let wall = self.grid.piercing_wall(s.view.position, end, s.view.radius);
            let hit = self.first_hostile_contact(
                &s.view.source,
                (s.view.position, end),
                s.view.radius,
                c,
            );
            if let Some((t, target)) = hit.filter(|(t, _)| wall.is_none_or(|w| *t < w)) {
                let _contact = t;
                let target_ref = self.owner_entity(target);
                if s.target
                    .as_ref()
                    .is_some_and(|intended| *intended != target_ref)
                {
                    events.push((
                        None,
                        EventV4::ProjectileEnded {
                            reason: is_copy.then_some(ProjectileEndReasonV4::TargetInvalid),
                            projectile_id: s.view.id,
                        },
                    ));
                    continue;
                }
                self.direct_contact(
                    s.owner,
                    target,
                    combat::DirectContact {
                        origin: Some(s.origin.clone()),
                        action_id: Some(&s.action_id),
                        rule: s.damage_rule.as_ref(),
                        ignore_armor: s.ignore_armor,
                        unavoidable: s.unavoidable,
                        dead_source: s.end_on_source_invalid,
                        source: &s.view.source,
                        raw: s.raw,
                        metal: s.metal,
                        hit_statuses: &s.hit_statuses,
                    },
                    c,
                    now,
                    events,
                );
                events.push((
                    None,
                    EventV4::ProjectileEnded {
                        reason: None,
                        projectile_id: s.view.id,
                    },
                ));
            } else if wall.is_some() || s.remaining <= travel {
                events.push((
                    None,
                    EventV4::ProjectileEnded {
                        reason: is_copy.then_some(if wall.is_some() {
                            ProjectileEndReasonV4::Wall
                        } else {
                            ProjectileEndReasonV4::RangeExhausted
                        }),
                        projectile_id: s.view.id,
                    },
                ));
            } else {
                s.view.position = end;
                s.remaining -= travel;
                self.shots.push(s);
            }
        }
    }
    fn hurt_member(
        &mut self,
        i: usize,
        source: EntityRef,
        raw: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let previous_origin = self
            .triggers
            .origin
            .replace(crate::origin::ActionOrigin::Basic);
        let previous_source = self.triggers.action_source.replace(source.clone());
        let needs_identity = c
            .combat_rules
            .as_ref()
            .is_some_and(|r| r.triggers.iter().any(|t| t.maximum_per_action > 1));
        let action_id = needs_identity.then(|| self.next_id("npc-basic"));
        let previous_action = std::mem::replace(&mut self.triggers.action, action_id);
        self.hurt_member_effect(i, source, (raw, true), c, now, events);
        self.triggers.origin = previous_origin;
        self.triggers.action_source = previous_source;
        self.triggers.action = previous_action;
    }
    fn defeated(&mut self, j: usize, i: usize, c: &Config, now: u64, events: &mut Events) {
        let profile = self.npcs[j].profile.clone();
        self.npcs[j].casting = None;
        self.npcs[j].target = None;
        let pursuit = self.npcs[j].pursuit.is_some();
        if !self.training && !pursuit {
            let member = &mut self.members[i];
            let template = &self.npcs[j].template_id;
            *member.tally.entry(template.clone()).or_default() += 1;
            if !member.reward_rates.is_empty() {
                let profile = member.active_basic_profile().cloned();
                *member
                    .reward_kills
                    .entry((template.clone(), profile))
                    .or_default() += 1;
            }
        }
        if !self.rebuilds_training_dummy(j, c) {
            events.push((
                None,
                EventV4::MonsterDefeated {
                    entity: self.npcs[j].entity.clone(),
                    credited_to: self.members[i].reference.entity.clone(),
                },
            ));
        }
        if pursuit {
            return;
        }
        self.clear_owned_statuses(
            &self.npcs[j].entity.clone(),
            StatusRemovalReason::TargetInvalid,
            events,
        );
        if self.training {
            return;
        }
        if !self.queue_monster_loot(j, c, now) {
            self.drop_legacy_monster_loot(j, &profile, c, now, events);
        }
        if profile.drop_orb {
            self.drop_monster_orb(j, i, &profile, c, now, events);
        }
        self.record_encounter_kill(j, c, now);
        self.record_mode_kill(j, c);
    }

    fn drop_legacy_monster_loot(
        &mut self,
        j: usize,
        profile: &MonsterTemplate,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        for rule in profile
            .drop_table
            .guaranteed
            .iter()
            .chain(&profile.drop_table.random)
        {
            if rule.chance.is_some_and(|p| !self.roll(p)) {
                continue;
            }
            let quantity =
                rule.quantity_min + self.random() % (rule.quantity_max - rule.quantity_min + 1);
            if quantity == 0 {
                continue;
            }
            let angle = f64::from(self.random()) / f64::from(u32::MAX) * std::f64::consts::TAU;
            let radius = c.params.drop_min_radius
                + self.random() % (c.params.drop_max_radius - c.params.drop_min_radius + 1);
            let candidate = [
                self.npcs[j].position[0] + angle.cos() * f64::from(radius),
                self.npcs[j].position[1] + angle.sin() * f64::from(radius),
            ];
            let position = if self.grid.clear(candidate, 0., 0) {
                candidate
            } else {
                self.npcs[j].position
            };
            let view = ResourceViewV4 {
                id: self.next_id("drop"),
                position,
                quantity,
                item_id: Some(rule.item_id.clone()),
                owner: None,
                expires_at_ms: now + c.params.resource_ttl_ms,
                instance: None,
            };
            events.push((
                None,
                EventV4::ResourceSpawned {
                    resource: view.clone(),
                },
            ));
            self.resources.push(Resource {
                view,
                hp: 0,
                spirit: 0,
                player_drop: false,
                origin: Default::default(),
            });
        }
    }

    fn drop_monster_orb(
        &mut self,
        j: usize,
        i: usize,
        profile: &MonsterTemplate,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let view = ResourceViewV4 {
            id: self.next_id("orb"),
            position: self.npcs[j].position,
            quantity: 0,
            item_id: None,
            owner: Some(self.members[i].reference.entity.clone()),
            expires_at_ms: now + c.params.resource_ttl_ms,
            instance: None,
        };
        let max_hp = self
            .entity_max_hp(&self.members[i].reference.entity, c, now)
            .unwrap();
        let max_spirit = self.member_max_spirit(i, c, now);
        let (hp, spirit) = if let Some(orb) = profile.recovery_orb {
            (
                (u64::from(max_hp) * u64::from(orb.hp_bps) / 10_000) as u32,
                (u64::from(max_spirit) * u64::from(orb.spirit_bps) / 10_000) as u32,
            )
        } else {
            (
                (f64::from(max_hp) * c.params.orb_hp_ratio).floor() as u32,
                (f64::from(max_spirit) * c.params.orb_spirit_ratio).floor() as u32,
            )
        };
        events.push((
            None,
            EventV4::ResourceSpawned {
                resource: view.clone(),
            },
        ));
        self.resources.push(Resource {
            view,
            hp,
            spirit,
            player_drop: false,
            origin: Default::default(),
        });
    }
    fn stack_limit(&self, c: &Config, id: &str) -> u32 {
        if id == c.item_id {
            c.stack_limit
        } else {
            c.pve.as_ref().unwrap().items[id].stack_limit
        }
    }
    pub(crate) fn settlement(&mut self, i: usize, c: &Config) -> Business {
        let outcome = self.members[i].desired.unwrap();
        let mut drops = BTreeMap::<String, u32>::new();
        if outcome == Outcome::Death
            && let Some(policy) = c.death_relics()
        {
            self.create_death_relic(i, policy, c);
        } else if c.settlement_policy_outcome(outcome) == Outcome::Death {
            let mut slots = vec![];
            for (id, &q) in &self.members[i].inventory {
                let limit = self.stack_limit(c, id);
                for n in 0..q.div_ceil(limit) {
                    slots.push((id.clone(), (q - n * limit).min(limit)));
                }
            }
            for _ in 0..c.params.death.count(slots.len()) {
                let at = self.random() as usize % slots.len();
                let (id, q) = slots.swap_remove(at);
                *drops.entry(id).or_default() += c.params.death.amount(q);
            }
        }
        let m = &self.members[i];
        let scoring = c.score_summary(&m.tally);
        let dropped = drops.get(&c.item_id).copied().unwrap_or(0);
        let items = m
            .looted
            .iter()
            .filter(|(id, _)| *id != &c.item_id)
            .map(|(id, &q)| ItemSettlement {
                item_id: id.clone(),
                looted: q,
                kept: m.inventory.get(id).copied().unwrap_or(0)
                    - drops.get(id).copied().unwrap_or(0),
                dropped: drops.get(id).copied().unwrap_or(0),
            })
            .collect();
        let mut progression = c.progression_draft(&m.damage_growth);
        if let Some(draft) = &mut progression {
            draft.stack_flows = m.stack_flows();
        }
        if !m.reward_rates.is_empty() {
            progression.as_mut().unwrap().reward_earnings = Some(RewardEarnings {
                rates: m.reward_rates.clone(),
                kills: m
                    .reward_kills
                    .iter()
                    .map(|((template, profile), &kills)| RewardKill {
                        template_id: template.clone(),
                        basic_profile: profile.clone(),
                        kills,
                    })
                    .collect(),
            });
        }
        if c.has_equipped_weapon_rewards() {
            progression.as_mut().unwrap().reward_weapon_instance_id = m
                .equipment
                .as_ref()
                .and_then(|view| view.weapon.as_ref())
                .map(|weapon| weapon.instance_id.clone());
        }
        Business::CommitSettlement {
            outcome,
            expected_revision: m.facts.revision,
            item_id: c.item_id.clone(),
            carried: c.carried_quantity,
            looted: m.looted.get(&c.item_id).copied().unwrap_or(0),
            kept: m.inventory.get(&c.item_id).copied().unwrap_or(0) - dropped,
            dropped,
            cultivation: c.cultivation_from_score(u64::from(scoring.kill_score), outcome),
            scoring: Some(scoring),
            map_progress: None,
            reset_progress: None,
            items,
            instances: m.instances.values().cloned().collect(),
            rewards: Vec::new(),
            progression,
        }
    }
    pub(crate) fn snapshot(&self, i: usize, c: &Config, phase: Phase, now: u64) -> SnapshotV4 {
        let m = &self.members[i];
        let (effective_atk, effective_def) = self.stats(i, c, now);
        SnapshotV4 {
            contested_equipment: self.contested_views(),
            extraction_pressure: self.extraction_pressure_view(c, now),
            encounter_progress: self.encounter_view(),
            member_conflict: self.member_conflict,
            relics: self.relic_views(c),
            containers: self.container_views(i, c),
            active_blockers: self.grid.active_blockers(),
            checkpoint_zones: self.checkpoint_zones(i, c, now),
            basic_beams: c
                .basic_configs()
                .any(|basic| basic.beam.is_some())
                .then(|| {
                    self.members
                        .iter()
                        .filter_map(|m| m.basic_beam.as_ref()?.view().cloned())
                        .collect()
                }),
            basic_impacts: c
                .basic_configs()
                .any(|basic| basic.impact.is_some())
                .then(|| {
                    self.basic_impacts
                        .iter()
                        .map(|plan| plan.view.clone())
                        .collect()
                }),
            basic_waves: c
                .basic_configs()
                .any(|basic| basic.wave.is_some())
                .then(|| {
                    self.basic_waves
                        .iter()
                        .map(basic_wave::BasicWavePlan::view)
                        .collect()
                }),
            areas: (c.mechanism_version >= 41).then(|| {
                let settling = matches!(m.phase, MemberPhase::Settling | MemberPhase::Terminal);
                self.areas
                    .iter()
                    .filter(|p| {
                        !settling
                            || !p.is_npc_area()
                                && !p.is_trap()
                                && !p.is_anchor()
                                && !p.follows_anchor()
                    })
                    .map(areas::AreaPlan::view)
                    .collect()
            }),
            phase,
            logical_ms: now,
            active_at_ms: self.active_at,
            own: PrivateMemberView {
                zone_exposure: self.zone_exposure_view(i, c, now),
                training: self.training_view(i),
                mode: self.mode_view(i, c),
                boons: self.boon_view(i, c),
                checkpoint_progress: self.checkpoint_progress(i, c, now),
                equipment: m.equipment.clone(),
                world_weapon_profile: m.world_weapon_profile.clone(),
                basic_charge: m.charge_view(c, now),
                basic_charge_ready_at_ms: m.dash_ready_at(c),
                basic_profile: m.basic_profile.clone(),
                basic_projectile_progress: m.projectile_progress(c),
                basic_wave_phase: matches!(m.phase, MemberPhase::Deploying | MemberPhase::Active)
                    .then(|| m.wave_phase(c))
                    .flatten(),
                skill_powers: m.combat_build.as_ref().map(|_| {
                    m.equipped_skills(c)
                        .iter()
                        .map(|id| {
                            (
                                id.clone(),
                                self.selected_power_with(
                                    &self.statuses,
                                    Owner::Member(i),
                                    id,
                                    c,
                                    now,
                                ),
                            )
                        })
                        .collect()
                }),
                shield_total: self.shields_enabled.then(|| {
                    self.statuses
                        .shield_total(&status_entity(&m.reference.entity), now)
                }),
                participant: m.reference.clone(),
                instances: m.instances.values().cloned().collect(),
                position: m.position,
                hp: m
                    .hp
                    .min(self.entity_max_hp(&m.reference.entity, c, now).unwrap()),
                max_hp: self.entity_max_hp(&m.reference.entity, c, now).unwrap(),
                spirit: m.spirit.min(self.member_max_spirit(i, c, now)),
                max_spirit: self.member_max_spirit(i, c, now),
                phase: m.phase,
                protected: m.protected,
                foreground: m.foreground,
                ready: m.ready,
                effective_atk,
                effective_def,
                statuses: self
                    .statuses
                    .views(&status_entity(&m.reference.entity), now),
                inventory: m
                    .inventory
                    .iter()
                    .filter(|(_, q)| **q > 0)
                    .map(|(id, &q)| ItemAmount {
                        item_id: id.clone(),
                        quantity: q,
                    })
                    .collect(),
                inventory_revision: m.revision,
                item_cooldowns: m
                    .item_cooldowns
                    .iter()
                    .filter(|&(_, &ready_at_ms)| ready_at_ms > now)
                    .map(|(item_id, &ready_at_ms)| ItemCooldownView {
                        item_id: item_id.clone(),
                        ready_at_ms,
                    })
                    .collect(),
                skill_loadout_revision: m.skills.declared.then_some(m.skills.revision),
                player_skills: m
                    .equipped_skills(c)
                    .iter()
                    .map(|id| PlayerSkillView {
                        skill_id: id.clone(),
                        ready_at_ms: m.cooldowns.get(id).copied().unwrap_or(0),
                    })
                    .collect(),
                active_channel: m.channel.as_ref().map(|channel| ActiveChannelView {
                    action_id: channel.action_id.clone(),
                    skill_id: channel.skill.id.clone(),
                    direction: channel.direction,
                    started_at_ms: channel.started_at_ms,
                    ends_at_ms: channel.ends_at_ms,
                    next_pulse_at_ms: (channel.next_pulse_at_ms < channel.ends_at_ms
                        || channel.skill.channel.clone().unwrap().pulse_count.is_some())
                    .then_some(channel.next_pulse_at_ms),
                    pulses: channel.pulses,
                    total_pulses: channel.skill.channel.clone().unwrap().pulse_count,
                    recovery: channel.recovery_budget.map(|_| true),
                    target: channel.target.clone(),
                }),
                pending_action: m.pending.as_ref().map(|p| PlayerActionView {
                    action_id: p
                        .skill
                        .as_ref()
                        .filter(|s| s.resource_commit.is_some())
                        .map(|_| p.id.clone()),
                    resource_reservation: p.reservation,
                    charge: p.charge_started_at_ms.map(|started_at_ms| {
                        let definition = p
                            .skill
                            .as_ref()
                            .and_then(|skill| skill.charge)
                            .expect("charge definition");
                        let clock = p.charge_clock.unwrap();
                        ChargeActionView {
                            action_id: p.id.clone(),
                            started_at_ms,
                            min_release_at_ms: clock.min_release,
                            auto_commit_at_ms: clock.max_release + clock.release_windup,
                            released_at_ms: p.charge_released_at.or_else(|| {
                                definition
                                    .release_windup_ms
                                    .filter(|_| now >= clock.max_release)
                                    .map(|_| clock.max_release)
                            }),
                            release_windup_ms: definition
                                .release_windup_ms
                                .map(|_| clock.release_windup),
                            aim: p.aim_updated_at_ms.map(|updated_at_ms| ChargeAimView {
                                direction: p.direction,
                                updated_at_ms,
                            }),
                        }
                    }),
                    skill_id: p.skill.as_ref().map(|s| s.id.clone()),
                    ground_position: p.ground,
                    ally_target: p
                        .target
                        .as_ref()
                        .filter(|target| {
                            target.kind == StatusEntityKind::Participant
                                && p.skill.as_ref().is_some_and(|skill| skill.targets_ally())
                        })
                        .map(Self::unit_wire),
                    ally_target_is_self: (p.target.as_ref() == Some(&m.reference.entity))
                        .then_some(true),
                    unit_target: p
                        .target
                        .as_ref()
                        .filter(|target| {
                            !p.skill.as_ref().is_some_and(|skill| skill.targets_ally())
                                && (p.skill.is_some()
                                    || c.mechanism_version >= 54
                                        && self.resolve_entity(target).is_some())
                        })
                        .map(Self::unit_wire),
                    commits_at_ms: p.commits,
                }),
                projectile_recast: m.recast.as_ref().map(|recast| recast.view.clone()),
                followup: c
                    .skill_followup
                    .and_then(|_| m.followup_view(now, c.step_ms)),
                recovery_cancel: m.recovery_cancel_view(now),
                basic_followup: m.queued_basic.clone(),
                basic_pursuit: m.basic_pursuit.as_ref().map(|plan| plan.view.clone()),
                basic_ready_at_ms: self.member_basic_deadline(i, c),
                recovery_until_ms: m.recovery_until,
                extraction_finishes_ms: m.extract_end,
            },
            members: self
                .members
                .iter()
                .map(|m| PublicMemberView {
                    extraction_channel: self.members[i]
                        .living()
                        .then(|| m.extraction_channel.clone())
                        .flatten(),
                    basic_seals: m.projectile_seals(c),
                    entity: m.reference.entity.clone(),
                    position: m.position,
                    hp: m
                        .hp
                        .min(self.entity_max_hp(&m.reference.entity, c, now).unwrap()),
                    max_hp: self.entity_max_hp(&m.reference.entity, c, now).unwrap(),
                    phase: m.phase,
                    protected: m.protected,
                })
                .collect(),
            monsters: self.monster_views(i, c, now),
            resources: self
                .resources
                .iter()
                .filter(|r| r.visible_to(&m.reference.entity))
                .map(|r| r.view_at(now))
                .collect(),
            projectiles: self
                .shots
                .iter()
                .filter(|shot| self.members[i].living() || !matches!(shot.owner, Owner::Summon(_)))
                .map(Shot::view)
                .chain(self.basic_projectile_views())
                .collect(),
            extraction_point: c.extraction_point,
            extraction_open: self.mode.is_none()
                && !self.training
                && self
                    .active_at
                    .is_some_and(|at| now >= at + c.extraction_unlock_ms),
        }
    }

    fn monster_views(&self, i: usize, c: &Config, now: u64) -> Vec<MonsterViewV4> {
        let include_summons = self.members[i].living();
        let mut monsters = self
            .npcs
            .iter()
            .filter(|npc| {
                npc.visible_training_target()
                    && npc.pursuit_visible()
                    && self.mode_npc_visible(&npc.entity.id)
            })
            .map(|n| MonsterViewV4 {
                training_owner_id: n
                    .training_owner
                    .map(|i| self.members[i].reference.entity.id.clone()),
                boss_phase: n.boss_view(),
                summon: None,
                shield_total: self
                    .shields_enabled
                    .then(|| self.statuses.shield_total(&status_entity(&n.entity), now)),
                statuses: self
                    .periodic_enabled
                    .then(|| self.visible_npc_statuses(i, &n.entity, now)),
                entity: n.entity.clone(),
                template_id: n.template_id.clone(),
                position: n.position,
                hp: n.hp.min(self.entity_max_hp(&n.entity, c, now).unwrap()),
                max_hp: self.entity_max_hp(&n.entity, c, now).unwrap(),
                radius: n.profile.combat.radius,
                returning: n.returning,
                target: n
                    .target
                    .as_ref()
                    .filter(|target| {
                        include_summons || target.kind == StatusEntityKind::Participant
                    })
                    .cloned(),
            })
            .collect::<Vec<_>>();
        if include_summons && !self.summons.is_empty() {
            monsters.extend(self.summon_views(i, c, now));
            monsters.sort_by(|a, b| a.entity.id.cmp(&b.entity.id));
        }
        monsters
    }
}

mod cooldown_returns;

mod contact_reactions;

mod finishers;

mod finisher_followups;
