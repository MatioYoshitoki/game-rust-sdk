//! Protocol-4 coordinator: one World and bounded financial-only member ledgers.
use crate::{
    config::Config,
    engine::Engine,
    protocol::*,
    world::two_members::{Events, World},
};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, sync::Arc};
mod replication;
#[cfg(all(test, match_sdk_test_data))]
mod source_scenario_tests;
#[cfg(test)]
mod spectator_tests;
mod spectators;
#[cfg(all(test, match_sdk_test_data))]
mod tests;

fn digest(v: &impl serde::Serialize) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(v).expect("typed protocol"))
    )
}
// The JSONL envelope serializes through Value, which sorts object keys. V4's
// aggregate checksum uses that explicit canonical representation across hosts.
fn state_digest(state: &IoStateV4) -> String {
    digest(&serde_json::to_value(state).expect("typed checkpoint"))
}
struct Publication {
    id: String,
    epoch: u32,
    connection: Connection,
    outcome: TransportOutcome,
    full: bool,
}
struct Seat {
    original: ParticipantSeat,
    connection: Connection,
    lost: bool,
    resume_until: Option<u64>,
    mode_forfeit_at: Option<u64>,
    epoch: u32,
    sequence: u32,
    history: VecDeque<(Input, Result<Status>)>,
    publications: VecDeque<Publication>,
    queue: VecDeque<Output>,
    ledger: Engine,
    settlement_draft: Option<Business>,
    last_view: Option<SnapshotV4>,
    last_publish: u64,
    last_full: Option<String>,
    terminal_published: bool,
    stream: Option<replication::Stream>,
}
mod skill_loadouts;
mod training_tools;
mod weapon_equipment;
pub(crate) struct TwoMembers {
    skill_control: SkillControlCheckpoint,
    training_tool_requests: Vec<String>,
    weapon_operations: Vec<WeaponEquipmentOperation>,
    pub config: Config,
    create: CreateV4,
    seats: Vec<Seat>,
    spectators: Vec<spectators::SpectatorSlot>,
    spectator_facts: VecDeque<SpectatorFactV6>,
    spectator_event_sequence: u64,
    spectator_final: Option<SpectatorSnapshotV6>,
    pub(crate) world: Option<World>,
    phase: Phase,
    now: u64,
    io_origin: u64,
    io_elapsed: u64,
    closed_at_io: Option<u64>,
    output_sequence: u64,
    generation: u32,
    recovery_fingerprint: Option<String>,
    outputs: VecDeque<Output>,
    undelivered: Vec<Output>,
    dirty: bool,
    closed_emitted: bool,
}
impl TwoMembers {
    pub(crate) fn create(
        config: Config,
        catalog: Arc<crate::ConfigCatalog>,
        mut create: CreateV4,
    ) -> Result<Self> {
        let recovery = create.recovery.take();
        if !matches!(create.protocol_version, 4..=6)
            || config.protocol_version() != create.protocol_version
        {
            return Err(Error::VersionMismatch);
        }
        if create.config_id != config.id || create.config_hash != config.hash() {
            return Err(Error::ConfigMismatch);
        }
        if create.participants.len() != config.member_settings().unwrap().capacity
            || !valid_id(&create.key.id)
            || create.key.id.len() > 64
            || create.key.incarnation == 0
            || !valid_id(&create.request_id)
        {
            return Err(Error::InvalidMessage);
        }
        let npc_ids = World::npc_ids(&config);
        for s in &create.participants {
            if !valid_id(&s.participant_id)
                || !valid_id(&s.authenticated_principal)
                || !valid_id(&s.connection.id)
                || s.connection.generation == 0
                || npc_ids.contains(&s.participant_id)
            {
                return Err(Error::InvalidMessage);
            }
        }
        for (i, a) in create.participants.iter().enumerate() {
            if create.participants[..i].iter().any(|b| {
                a.participant_id == b.participant_id
                    || a.authenticated_principal == b.authenticated_principal
                    || a.connection.id == b.connection.id
            }) {
                return Err(Error::Conflict);
            }
        }
        if config.participant_ai.keys().any(|id| {
            !create
                .participants
                .iter()
                .any(|seat| &seat.participant_id == id)
        }) {
            return Err(Error::InvalidMessage);
        }
        if config.member_conflict()
            || config
                .match_mode
                .as_ref()
                .is_some_and(|mode| mode.opposed())
        {
            let targets = World::npc_ids(&config);
            if create
                .participants
                .iter()
                .any(|seat| targets.contains(&seat.participant_id))
            {
                return Err(Error::InvalidMessage);
            }
        }
        if config.world_loot.as_ref().is_some_and(|world| world.containers.iter().any(|container| {
            matches!(&container.visibility, crate::config::world_loot::ContainerVisibility::Personal { participant_id }
                if !create.participants.iter().any(|seat| seat.participant_id == *participant_id))
        })) {
            return Err(Error::InvalidMessage);
        }
        for seat in &create.participants {
            if let Some(id) = &seat.basic_profile {
                if create.protocol_version != 6 || config.mechanism_version < 105 {
                    return Err(Error::Unsupported);
                }
                if !config
                    .basic_profiles()
                    .is_some_and(|profiles| profiles.contains_key(id))
                {
                    return Err(Error::InvalidMessage);
                }
            }
            if let Some(build) = &seat.combat_build {
                if create.protocol_version != 6 || config.mechanism_version < 105 {
                    return Err(Error::Unsupported);
                }
                if !config.valid_combat_build(build) {
                    return Err(Error::InvalidMessage);
                }
            }
        }
        let skill_control = Self::initial_skill_control(&config, &create, recovery.as_ref())?;
        let weapon_operations =
            Self::initial_weapon_operations(&config, &create, recovery.as_ref())?;
        let mut sorted = create.participants.clone();
        sorted.sort_by(|a, b| a.participant_id.cmp(&b.participant_id));
        let mut generation = 0;
        let mut now = 0;
        let mut output_sequence = 0;
        if let Some(r) = &recovery {
            let state = &r.checkpoint.state;
            if r.generation == 0
                || r.generation <= state.recovery_generation
                || state.schema_version != 1
                || r.checkpoint.sha256 != state_digest(state)
                || state.identity_sha256 != digest(&create)
                || state.logical_ms >= (1u64 << 53)
                || state.output_sequence >= (1u64 << 53)
                || state.members.len() != config.member_settings().unwrap().capacity
                || !state
                    .members
                    .iter()
                    .zip(&sorted)
                    .all(|(m, s)| m.participant_id == s.participant_id)
            {
                return Err(Error::InvalidMessage);
            }
            now = state.logical_ms;
            output_sequence = state.output_sequence;
            generation = r.generation;
            Self::validate_aggregate(&config, state, &digest(&create.key), &create)?;
        }
        let mut seats = vec![];
        for (i, s) in sorted.into_iter().enumerate() {
            let mut ledger =
                Engine::new_member_ledger(config.single_member_validation(), catalog.clone());
            let financial_key = MatchKey {
                id: digest(&(&create.key, &s.participant_id)),
                incarnation: create.key.incarnation,
            };
            ledger.create(Create {
                protocol_version: 3,
                key: financial_key,
                request_id: create.request_id.clone(),
                config_id: config.id.clone(),
                config_hash: config.hash(),
                seed: create.seed,
                participant_id: s.participant_id.clone(),
                authenticated_principal: s.authenticated_principal.clone(),
                connection: s.connection.clone(),
                publication_mode: create.publication_mode,
                recovery: recovery.as_ref().map(|r| Recovery {
                    generation: r.generation,
                    checkpoint: r.checkpoint.state.members[i].checkpoint.clone(),
                }),
            })?;
            assert!(ledger.world.is_none());
            seats.push(Seat {
                connection: s.connection.clone(),
                original: s,
                lost: false,
                resume_until: None,
                mode_forfeit_at: None,
                epoch: 1,
                sequence: 0,
                history: VecDeque::new(),
                publications: VecDeque::new(),
                queue: VecDeque::new(),
                ledger,
                settlement_draft: None,
                last_view: None,
                last_publish: 0,
                last_full: None,
                terminal_published: false,
                stream: (create.protocol_version == 6).then(replication::Stream::new),
            });
        }
        let restored = recovery.is_some();
        let spectators = (0..config.spectators().map_or(0, |s| s.capacity))
            .map(|_| spectators::SpectatorSlot::new())
            .collect();
        let mut next = Self {
            skill_control,
            training_tool_requests: Vec::new(),
            weapon_operations,
            config,
            create,
            seats,
            spectators,
            spectator_facts: VecDeque::new(),
            spectator_event_sequence: 0,
            spectator_final: None,
            world: None,
            phase: if restored {
                Phase::Closed
            } else {
                Phase::Preparing
            },
            now,
            io_origin: 0,
            io_elapsed: 0,
            closed_at_io: None,
            output_sequence,
            generation,
            recovery_fingerprint: recovery.as_ref().map(digest),
            outputs: VecDeque::new(),
            undelivered: vec![],
            dirty: true,
            closed_emitted: false,
        };
        next.harvest();
        if restored {
            next.emit_closed();
        }
        Ok(next)
    }
    pub(crate) fn repeat_create(&self, mut create: CreateV4) -> Result<Status> {
        let recovery = create.recovery.take();
        if create == self.create
            && (recovery.is_none() || recovery.as_ref().map(digest) == self.recovery_fingerprint)
        {
            Ok(self.status())
        } else {
            Err(Error::Conflict)
        }
    }
    fn validate_aggregate(
        c: &Config,
        state: &IoStateV4,
        instance_scope: &str,
        create: &CreateV4,
    ) -> Result<()> {
        let mut loot = std::collections::BTreeMap::<String, u64>::new();
        let mut kills = std::collections::BTreeMap::<String, u64>::new();
        let mut instances = std::collections::BTreeSet::new();
        for member in &state.members {
            let seat = create
                .participants
                .iter()
                .find(|seat| seat.participant_id == member.participant_id)
                .ok_or(Error::InvalidMessage)?;
            let weapon = state
                .weapon_operations
                .iter()
                .rev()
                .find(|op| op.request.participant_id == member.participant_id)
                .map_or(seat.weapon.as_ref(), |op| {
                    op.receipt.equipment.weapon.as_ref()
                });
            if member.checkpoint.state.logical_ms != state.logical_ms {
                return Err(Error::InvalidMessage);
            }
            for op in &member.checkpoint.state.operations {
                if c.protocol_version() >= 5
                    && matches!(&op.terminal,Some(ReceiptOutcome::AssetsLoaded{facts}) if facts.assets.len()>32)
                {
                    return Err(Error::InvalidMessage);
                }

                if let Business::CommitSettlement {
                    item_id,
                    looted,
                    items,
                    instances: kept_instances,
                    scoring,
                    progression,
                    ..
                } = c.decode_business(&op.request.body)?
                {
                    if c.has_equipped_weapon_rewards()
                        && progression
                            .as_ref()
                            .and_then(|p| p.reward_weapon_instance_id.as_deref())
                            != weapon.map(|w| w.instance_id.as_str())
                    {
                        return Err(Error::InvalidMessage);
                    }
                    *loot.entry(item_id).or_default() += u64::from(looted);
                    for item in items {
                        *loot.entry(item.item_id).or_default() += u64::from(item.looted);
                    }
                    if let Some(flows) = progression.and_then(|plan| plan.stack_flows) {
                        for row in flows {
                            *loot.entry(row.item_id).or_default() += row.consumed;
                        }
                    }
                    for item in kept_instances {
                        let serial = item
                            .instance_id
                            .strip_prefix(instance_scope)
                            .and_then(|tail| tail.strip_prefix('-'))
                            .filter(|tail| tail.len() == 16);
                        if !serial.is_some_and(|tail| {
                            u64::from_str_radix(tail, 16)
                                .is_ok_and(|n| n > 0 && format!("{n:016x}") == tail)
                        }) || !instances.insert(item.instance_id)
                        {
                            return Err(Error::InvalidMessage);
                        }
                        *loot.entry(item.item_id).or_default() += 1;
                    }
                    for t in scoring.ok_or(Error::InvalidMessage)?.templates {
                        *kills.entry(t.template_id).or_default() += u64::from(t.kills);
                    }
                }
            }
        }
        let population = c.pve.as_ref().unwrap().population_counts(state.logical_ms);
        if c.training_targets().is_some()
            && (kills.values().any(|&n| n != 0) || loot.values().any(|&n| n != 0))
            || kills
                .iter()
                .any(|(id, &n)| n > u64::from(population.get(id).copied().unwrap_or(0)))
            || loot.iter().any(|(id, &n)| {
                n > u64::from(c.pve_loot_bound(id, c.max_kills(state.logical_ms), state.logical_ms))
            })
        {
            return Err(Error::InvalidMessage);
        }
        Ok(())
    }
    pub(crate) fn status(&self) -> Status {
        Status {
            phase: self.phase,
            logical_ms: self.now,
        }
    }
    pub(crate) fn align_service_clock(&mut self, origin: u64) {
        self.io_origin = origin;
    }
    fn check_key(&self, key: &MatchKey) -> Result<()> {
        if *key == self.create.key {
            Ok(())
        } else {
            Err(Error::StaleMatch)
        }
    }
    fn index(&self, id: &str) -> Result<usize> {
        self.seats
            .iter()
            .position(|s| s.original.participant_id == id)
            .ok_or(Error::Unauthorized)
    }
    fn reserve_margin(&self) -> usize {
        if self.create.protocol_version >= 5 {
            16 * self.seats.len()
        } else {
            16
        }
    }
    fn reserve(&self) -> Result<()> {
        if self.outputs.len() + self.undelivered.len() + self.reserve_margin()
            > self.config.output_capacity
        {
            Err(Error::QueueFull)
        } else {
            Ok(())
        }
    }
    fn output(&mut self, body: OutputBody) -> Output {
        self.output_sequence += 1;
        Output {
            key: self.create.key.clone(),
            id: format!(
                "{}:{}:r{}:out:{:020}",
                self.create.key.id,
                self.create.key.incarnation,
                self.generation,
                self.output_sequence
            ),
            logical_ms: self.now,
            body,
            recovery: None,
        }
    }
    fn emit(&mut self, body: OutputBody) {
        let o = self.output(body);
        self.outputs.push_back(o);
    }
    fn events(&mut self, events: Events) {
        for (i, event) in events {
            if i.is_none() {
                self.record_spectator_event(&event);
            }
            let recipient_id = i.map(|i| self.seats[i].original.participant_id.clone());
            self.emit(OutputBody::EventV4 {
                recipient_id,
                event,
            });
        }
    }
    fn harvest(&mut self) {
        for i in 0..self.seats.len() {
            // Keep the original bounded ledger queue owned by that ledger until it fits.
            if self.outputs.len() + self.seats[i].ledger.ledger_queued()
                > self.config.output_capacity
            {
                continue;
            }
            let outputs = self.seats[i].ledger.drain();
            if !outputs.is_empty() {
                self.dirty = true;
            }
            for output in outputs {
                match output.body {
                    OutputBody::BusinessRequest { request } => {
                        self.emit(OutputBody::BusinessRequestV4 {
                            participant_id: self.seats[i].original.participant_id.clone(),
                            principal: self.seats[i].original.authenticated_principal.clone(),
                            request,
                        })
                    }
                    OutputBody::Event { event } => self.emit(OutputBody::EventV4 {
                        recipient_id: Some(self.seats[i].original.participant_id.clone()),
                        event: EventV4::Private { event },
                    }),
                    _ => unreachable!("financial ledger never publishes a World"),
                }
            }
        }
    }
    fn checkpoint(&mut self) -> IoCheckpointV4 {
        for s in &mut self.seats {
            s.ledger.ledger_clock(self.now);
        }
        let state = IoStateV4 {
            weapon_operations: self.weapon_operations.clone(),
            skill_control: (self.skill_control.members.values().any(|s| s.declared))
                .then(|| self.skill_control.clone()),
            schema_version: 1,
            identity_sha256: digest(&self.create),
            logical_ms: self.now,
            output_sequence: self.output_sequence,
            recovery_generation: self.generation,
            members: self
                .seats
                .iter()
                .map(|s| MemberCheckpoint {
                    participant_id: s.original.participant_id.clone(),
                    checkpoint: s.ledger.ledger_checkpoint(),
                })
                .collect(),
        };
        IoCheckpointV4 {
            sha256: state_digest(&state),
            state,
        }
    }
    fn publish(&mut self, i: usize) {
        if self.create.protocol_version == 6 {
            self.publish_v6(i);
            return;
        }
        if self.seats[i].lost {
            return;
        }
        let snapshot = if let Some(w) = &self.world {
            if !w.members[i].joined {
                return;
            }
            if w.members[i].phase == MemberPhase::Terminal && self.seats[i].terminal_published {
                return;
            }
            w.snapshot(i, &self.config, self.phase, self.now)
        } else if let Some(mut last) = self.seats[i].last_view.clone() {
            last.phase = self.phase;
            last.logical_ms = self.now;
            if self.seats[i].ledger.ledger_settled() {
                last.own.phase = MemberPhase::Terminal;
            }
            last
        } else {
            return;
        };
        self.seats[i].terminal_published = snapshot.own.phase == MemberPhase::Terminal;
        self.seats[i].last_view = Some(snapshot.clone());
        self.seats[i].last_publish = self.now;
        let o = self.output(OutputBody::PublicationV4 {
            recipient_id: self.seats[i].original.participant_id.clone(),
            connection: self.seats[i].connection.clone(),
            snapshot,
        });
        let seat = &mut self.seats[i];
        seat.last_full = Some(o.id.clone());
        seat.queue.push_back(o);
        while seat.queue.len()
            > self
                .config
                .member_settings()
                .unwrap()
                .publication_queue_capacity
        {
            seat.queue.pop_front();
        }
    }
    fn publish_all(&mut self) {
        for i in 0..self.seats.len() {
            self.publish(i)
        }
    }
    fn sync(&mut self) -> Result<()> {
        if self.phase == Phase::Preparing && self.world.is_none() {
            if self
                .seats
                .iter()
                .any(|s| matches!(s.ledger.status().phase, Phase::Closed | Phase::Faulted))
            {
                self.abort()?;
                return Ok(());
            }
            if let Some(facts) = self
                .seats
                .iter()
                .map(|s| s.ledger.ledger_admitted_facts())
                .collect::<Option<Vec<_>>>()
            {
                let seats = self
                    .seats
                    .iter()
                    .map(|s| s.original.clone())
                    .collect::<Vec<_>>();
                let mut world = World::new(&self.config, &seats, facts, self.create.seed);
                if !self.config.earning_rate_assets().is_empty() {
                    for (i, seat) in self.seats.iter().enumerate() {
                        world.bind_reward_rates(
                            i,
                            seat.ledger
                                .ledger_reward_rates()
                                .ok_or(Error::InvalidMessage)?,
                        );
                    }
                }
                if self
                    .config
                    .world_loot
                    .as_ref()
                    .is_some_and(|loot| !loot.instances.is_empty())
                {
                    world.bind_instance_scope(digest(&self.create.key));
                }
                self.world = Some(world);
                self.phase = Phase::Deploying;
            }
        }
        if let Some(w) = &mut self.world {
            for i in 0..self.seats.len() {
                let seat = &mut self.seats[i];
                if w.members[i].desired.is_some() && !seat.ledger.ledger_settlement_started() {
                    let body = seat
                        .settlement_draft
                        .get_or_insert_with(|| w.settlement(i, &self.config));
                    seat.ledger.ledger_clock(self.now);
                    seat.ledger.ledger_settle(body.clone())?;
                    seat.settlement_draft = None;
                }
                if self.seats[i].ledger.ledger_settled()
                    && w.members[i].phase != MemberPhase::Terminal
                {
                    w.members[i].phase = MemberPhase::Terminal;
                    let entity = w.members[i].reference.entity.clone();
                    self.outputs.push_back(Output {
                        key: self.create.key.clone(),
                        id: String::new(),
                        logical_ms: self.now,
                        body: OutputBody::EventV4 {
                            recipient_id: None,
                            event: EventV4::MemberChanged {
                                entity,
                                phase: MemberPhase::Terminal,
                            },
                        },
                        recovery: None,
                    });
                }
            }
        }
        // Assign IDs to member transition outputs after releasing the World borrow.
        for o in &mut self.outputs {
            if o.id.is_empty() {
                self.output_sequence += 1;
                o.id = format!(
                    "{}:{}:r{}:out:{:020}",
                    self.create.key.id,
                    self.create.key.incarnation,
                    self.generation,
                    self.output_sequence
                );
            }
        }
        self.harvest();
        if self.world.as_ref().is_some_and(|w| {
            w.active_at.is_some()
                && w.members
                    .iter()
                    .all(|m| matches!(m.phase, MemberPhase::Settling | MemberPhase::Terminal))
        }) {
            self.finish_close()?;
        }
        Ok(())
    }
    pub(crate) fn business(&mut self, r: MemberBusinessReceipt) -> Result<Status> {
        self.check_key(&r.key)?;
        if self.create.protocol_version >= 5
            && matches!(&r.outcome,ReceiptOutcome::AssetsLoaded{facts} if facts.assets.len()>32)
        {
            return Err(Error::InvalidMessage);
        }
        self.reserve()?;
        let i = self.index(&r.participant_id)?;
        self.seats[i].ledger.ledger_clock(self.now);
        let key = self.seats[i].ledger.ledger_key();
        self.seats[i].ledger.business(BusinessReceipt {
            key,
            operation_id: r.operation_id,
            attempt_id: r.attempt_id,
            outcome: r.outcome,
        })?;
        self.dirty = true;
        self.sync()?;
        if self.world.is_some() {
            self.publish_all();
        } else {
            self.publish(i);
        }
        Ok(self.status())
    }
    fn remember(&mut self, i: usize, input: Input, result: Result<Status>) -> Result<Status> {
        let s = &mut self.seats[i];
        s.sequence = input.sequence;
        s.history.push_back((input, result));
        while s.history.len() > self.config.history_capacity {
            s.history.pop_front();
        }
        result
    }
    pub(crate) fn input(&mut self, input: Input) -> Result<Status> {
        self.check_key(&input.key)?;
        if matches!(self.phase, Phase::Closed | Phase::Faulted)
            && !(self.create.protocol_version == 6
                && matches!(input.action, Action::RequestFull { .. }))
        {
            return Err(Error::Closed);
        }
        let i = self.index(&input.participant_id)?;
        let reconnect = matches!(input.action, Action::Reconnect { .. });
        if input.entity_generation != 1 {
            return Err(Error::InvalidState);
        }
        if !reconnect && input.connection != self.seats[i].connection {
            return Err(Error::StaleConnection);
        }
        if !valid_id(&input.request_id) || input.sequence == 0 {
            return Err(Error::InvalidMessage);
        }
        if self
            .config
            .participant_ai
            .contains_key(&input.participant_id)
            && !matches!(
                input.action,
                Action::Join {}
                    | Action::CombatReady { .. }
                    | Action::Leave {}
                    | Action::Reconnect { .. }
                    | Action::ConnectionLost {}
                    | Action::RequestFull { .. }
                    | Action::Foreground {}
                    | Action::Background {}
            )
        {
            return Err(Error::Unauthorized);
        }
        match &input.action {
            Action::RecastProjectile { .. } if self.config.mechanism_version < 50 => {
                return Err(Error::Unsupported);
            }
            Action::RecastProjectile {
                action_id,
                recast_index,
                direction,
            } if !valid_id(action_id)
                || *recast_index != 1
                || !direction.iter().all(|v| v.is_finite()) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::StartCharge { .. } | Action::ReleaseCharge { .. }
                if self.config.mechanism_version < 47 =>
            {
                return Err(Error::Unsupported);
            }
            Action::UpdateChargeAim { .. }
            | Action::ReleaseCharge {
                direction: Some(_), ..
            } if self.config.mechanism_version < 48 => {
                return Err(Error::Unsupported);
            }
            Action::ReleaseCharge { action_id, .. } | Action::UpdateChargeAim { action_id, .. }
                if !valid_id(action_id) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::Move { direction }
            | Action::Cast { direction, .. }
            | Action::StartCharge { direction, .. }
            | Action::UpdateChargeAim { direction, .. }
            | Action::CastAtAnchor {
                direction: Some(direction),
                ..
            }
            | Action::ReleaseCharge {
                direction: Some(direction),
                ..
            } if !direction.iter().all(|v| v.is_finite()) => {
                return Err(Error::InvalidMessage);
            }
            Action::Navigate { destination } if !destination.iter().all(|v| v.is_finite()) => {
                return Err(Error::InvalidMessage);
            }
            Action::Cast { skill_id, .. }
            | Action::StartCharge { skill_id, .. }
            | Action::CastSelf { skill_id }
            | Action::CastUnit { skill_id, .. }
            | Action::CastGround { skill_id, .. }
            | Action::CastAtAnchor { skill_id, .. }
                if !valid_id(skill_id) || skill_id.len() > 64 =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::Pickup { resource_id, .. } if !valid_id(resource_id) => {
                return Err(Error::InvalidMessage);
            }
            Action::UseExposureNode { .. }
                if self
                    .config
                    .zone_exposure
                    .as_ref()
                    .is_none_or(|rule| rule.interaction.is_none()) =>
            {
                return Err(Error::Unsupported);
            }
            Action::UseExposureNode {
                node_id,
                expected_revision,
                ..
            } if !valid_id(node_id) || node_id.len() > 64 || *expected_revision == 0 => {
                return Err(Error::InvalidMessage);
            }
            Action::ClaimWorldEquipment { .. } if self.config.contested_equipment.is_none() => {
                return Err(Error::Unsupported);
            }
            Action::ClaimWorldEquipment {
                node_id,
                expected_revision,
            } if !valid_id(node_id) || node_id.len() > 64 || *expected_revision == 0 => {
                return Err(Error::InvalidMessage);
            }
            Action::ClaimBoon { .. } if self.config.world_boons.is_none() => {
                return Err(Error::Unsupported);
            }
            Action::CompleteModeCheckpoint { .. } if self.config.match_mode.is_none() => {
                return Err(Error::Unsupported);
            }
            Action::SetOwnedTargets { .. } | Action::ClearOwnedTargets {}
                if self.config.training_targets().is_none() =>
            {
                return Err(Error::Unsupported);
            }
            Action::SetOwnedTargets { preset_id }
                if !valid_id(preset_id) || preset_id.len() > 64 =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::ResetTrainingMetrics {}
                if self
                    .config
                    .training_runtime()
                    .is_none_or(|r| r.metrics.is_none()) =>
            {
                return Err(Error::Unsupported);
            }
            Action::CompleteModeCheckpoint { stage_id }
                if !valid_id(stage_id) || stage_id.len() > 64 =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::CounterCheckpoint {
                schedule_id,
                window_index,
                target,
            } if !valid_id(schedule_id)
                || !(1..=crate::config::checkpoint_zones::MAX_CHECKPOINT_WINDOWS as u32)
                    .contains(window_index)
                || !valid_id(&target.id)
                || target.generation == 0 =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::ClaimBoon {
                opportunity_id,
                boon_id,
                expected_revision,
                replace_slot,
                ..
            } if !valid_id(opportunity_id)
                || opportunity_id.len() > 64
                || !valid_id(boon_id)
                || boon_id.len() > 64
                || *expected_revision == 0
                || replace_slot.is_some_and(|slot| {
                    !(1..=crate::config::world_boons::MAX_SLOTS).contains(&slot)
                }) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::DropItem { .. } if self.config.member_loot_drop.is_none() => {
                return Err(Error::Unsupported);
            }
            Action::UseItem { .. } if self.config.member_item_use.is_none() => {
                return Err(Error::Unsupported);
            }
            Action::DropItem {
                item_id, quantity, ..
            } if !valid_id(item_id) || item_id.len() > 64 || *quantity == 0 => {
                return Err(Error::InvalidMessage);
            }
            Action::UseItem { item_id, .. } if !valid_id(item_id) || item_id.len() > 64 => {
                return Err(Error::InvalidMessage);
            }
            Action::CombatReady { publication_id } if !valid_id(publication_id) => {
                return Err(Error::InvalidMessage);
            }
            Action::CastUnit { .. } if self.config.mechanism_version < 40 => {
                return Err(Error::Unsupported);
            }
            Action::StartAttackBeam { .. }
            | Action::AimAttackBeam { .. }
            | Action::EndAttackBeam { .. }
                if self.create.protocol_version != 6 || self.config.mechanism_version < 105 =>
            {
                return Err(Error::Unsupported);
            }
            Action::AimAttackBeam { action_id, .. } | Action::EndAttackBeam { action_id }
                if !valid_id(action_id) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::AttackUnit { .. } if self.config.mechanism_version < 54 => {
                return Err(Error::Unsupported);
            }
            Action::CastUnit { target, .. } | Action::AttackUnit { target }
                if !valid_id(&target.id) || target.generation == 0 =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::CastGround { .. } if self.config.mechanism_version < 41 => {
                return Err(Error::Unsupported);
            }
            Action::CastAtAnchor { .. } if self.config.mechanism_version < 105 => {
                return Err(Error::Unsupported);
            }
            Action::CastAtAnchor { anchor_id, .. } if !valid_id(anchor_id) => {
                return Err(Error::InvalidMessage);
            }
            Action::RecastAnchor { .. }
            | Action::ReturnAnchor { .. }
            | Action::EndAnchor { .. }
                if self.create.protocol_version != 6 || self.config.mechanism_version < 105 =>
            {
                return Err(Error::Unsupported);
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
            } if !valid_id(anchor_id)
                || !valid_id(action_id)
                || !(1..=crate::config::MAX_ANCHOR_RECAST_COUNT).contains(recast_index) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::RecastAnchor { position, .. } if !position.iter().all(|v| v.is_finite()) => {
                return Err(Error::InvalidMessage);
            }
            Action::AttackGround { .. }
            | Action::StartBasicCharge { .. }
            | Action::ReleaseBasicCharge { .. }
            | Action::CancelBasicCharge { .. }
                if self.create.protocol_version != 6 || self.config.mechanism_version < 105 =>
            {
                return Err(Error::Unsupported);
            }
            Action::CastGround { position, .. } | Action::AttackGround { position }
                if !position.iter().all(|v| v.is_finite()) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::ReleaseBasicCharge { session_id, .. }
            | Action::CancelBasicCharge { session_id }
                if !valid_id(session_id) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::OpenChest { .. } | Action::CancelChestUnlock { .. }
                if self.config.world_loot.is_none() =>
            {
                return Err(Error::Unsupported);
            }
            Action::OpenRelic { .. } | Action::ClaimRelic { .. }
                if self.config.death_relics().is_none() =>
            {
                return Err(Error::Unsupported);
            }
            Action::OpenRelic { relic_id } | Action::ClaimRelic { relic_id, .. }
                if !valid_id(relic_id) =>
            {
                return Err(Error::InvalidMessage);
            }
            Action::ClaimRelic {
                expected_revision,
                entry_id,
                ..
            } if *expected_revision == 0 || entry_id.is_some_and(|id| !(1..=128).contains(&id)) => {
                return Err(Error::InvalidMessage);
            }
            Action::OpenChest {
                chest_id,
                expected_generation,
            }
            | Action::CancelChestUnlock {
                chest_id,
                expected_generation,
            } if !valid_id(chest_id) || *expected_generation == 0 => {
                return Err(Error::InvalidMessage);
            }
            Action::AttuneObjective { .. } | Action::CancelAttunement {} => {
                return Err(Error::Unsupported);
            }
            _ => {}
        }
        if let Some((old, result)) = self.seats[i]
            .history
            .iter()
            .find(|(v, _)| v.request_id == input.request_id)
        {
            if input.connection != self.seats[i].connection {
                return Err(Error::StaleConnection);
            }
            return if *old == input {
                *result
            } else {
                Err(Error::Conflict)
            };
        }
        if let Action::RequestFull { stream_epoch } = input.action {
            if self.create.protocol_version != 6 {
                return Err(Error::Unsupported);
            }
            if input.sequence <= self.seats[i].sequence {
                return Err(Error::StaleSequence);
            }
            if self.seats[i].lost {
                return Err(Error::NotReady);
            }
            if stream_epoch != self.seats[i].epoch {
                return Err(Error::StalePublication);
            }
            if self.seats[i].stream.as_ref().unwrap().expired {
                return Err(Error::Closed);
            }
            if self.seats[i].last_view.is_none() {
                return Err(if matches!(self.phase, Phase::Closed | Phase::Faulted) {
                    Error::Closed
                } else {
                    Error::NotReady
                });
            }
            self.reserve()?;
            self.reset_stream(i, FullReason::Resync)?;
            self.publish(i);
            return self.remember(i, input, Ok(self.status()));
        }
        if self.world.as_ref().is_some_and(|w| {
            matches!(
                w.members[i].phase,
                MemberPhase::Settling | MemberPhase::Terminal
            )
        }) {
            return Err(Error::Closed);
        }
        self.reserve()?;
        if reconnect {
            let Action::Reconnect {
                previous_connection,
                authenticated_principal,
            } = &input.action
            else {
                unreachable!()
            };
            let s = &self.seats[i];
            if authenticated_principal != &s.original.authenticated_principal {
                return Err(Error::Unauthorized);
            }
            if previous_connection != &s.connection
                || input.connection.generation
                    != s.connection
                        .generation
                        .checked_add(1)
                        .ok_or(Error::Faulted)?
            {
                return Err(Error::StaleConnection);
            }
            if !valid_id(&input.connection.id)
                || self
                    .seats
                    .iter()
                    .enumerate()
                    .any(|(j, s)| j != i && s.connection.id == input.connection.id)
            {
                return Err(Error::InvalidMessage);
            }
            if self.world.as_ref().is_some_and(|w| {
                matches!(
                    w.members[i].phase,
                    MemberPhase::Settling | MemberPhase::Terminal
                )
            }) {
                return Err(Error::Closed);
            }
            self.seats[i].connection = input.connection.clone();
            self.seats[i].lost = false;
            self.seats[i].history.clear();
            self.seats[i].sequence = 0;
            self.seats[i].epoch = self.seats[i].epoch.checked_add(1).ok_or(Error::Faulted)?;
            self.seats[i].queue.clear();
            if let Some(s) = &mut self.seats[i].stream {
                s.reset(FullReason::Lifecycle);
            }
            self.seats[i]
                .resume_until
                .get_or_insert(self.now + self.config.params.runtime.reconnect_grace_ms);
            let events = self
                .world
                .as_mut()
                .map(|w| {
                    let mut e = vec![];
                    w.protect(i, self.now, &mut e);
                    e
                })
                .unwrap_or_default();
            self.events(events);
            self.events(vec![(
                Some(i),
                EventV4::Private {
                    event: Event::ConnectionChanged {
                        generation: input.connection.generation,
                    },
                },
            )]);
            self.publish(i);
            return self.remember(i, input, Ok(self.status()));
        }
        if input.sequence <= self.seats[i].sequence {
            return Err(Error::StaleSequence);
        }
        if matches!(input.action, Action::ConnectionLost {}) {
            if !self.seats[i].lost {
                self.seats[i].lost = true;
                self.seats[i].resume_until =
                    Some(self.now + self.config.params.runtime.reconnect_grace_ms);
                if self.phase == Phase::Active
                    && let Some(delay) = self
                        .config
                        .match_mode
                        .as_ref()
                        .and_then(|mode| mode.disconnect_forfeit_ms)
                {
                    self.seats[i]
                        .mode_forfeit_at
                        .get_or_insert(self.now + delay);
                }
                self.seats[i].epoch = self.seats[i].epoch.checked_add(1).ok_or(Error::Faulted)?;
                self.seats[i].queue.clear();
                if let Some(s) = &mut self.seats[i].stream {
                    s.reset(FullReason::Lifecycle);
                }
                let events = self
                    .world
                    .as_mut()
                    .map(|w| {
                        let mut e = vec![];
                        w.protect(i, self.now, &mut e);
                        e
                    })
                    .unwrap_or_default();
                self.events(events);
                self.events(vec![(
                    Some(i),
                    EventV4::Private {
                        event: Event::ConnectionLost {
                            generation: input.connection.generation,
                            resume_until_ms: self.seats[i].resume_until,
                        },
                    },
                )]);
            }
            return self.remember(i, input, Ok(self.status()));
        }
        if self.seats[i].lost {
            return Err(Error::NotReady);
        }
        if let Action::CombatReady { publication_id } = &input.action
            && !(self.seats[i]
                .stream
                .as_ref()
                .is_some_and(|s| s.ready(publication_id))
                || self.seats[i].publications.iter().any(|p| {
                    p.id == *publication_id
                        && p.epoch == self.seats[i].epoch
                        && p.connection == input.connection
                        && p.outcome == TransportOutcome::Sent
                        && (self.create.protocol_version != 6 || p.full)
                }))
        {
            return Err(Error::NotReady);
        }
        if matches!(input.action, Action::Leave {})
            && matches!(self.phase, Phase::Preparing | Phase::Deploying)
        {
            self.abort()?;
            return self.remember(i, input, Ok(self.status()));
        }
        let before_background = self.world.as_ref().is_some_and(|w| w.members[i].foreground);
        let result =
            self.world.as_mut().ok_or(Error::NotReady).and_then(|w| {
                w.action(i, &input.action, &input.request_id, &self.config, self.now)
            });
        let result = match result {
            Err(e) => {
                // A matching terminal input consumes its charge even when aim fails.
                if matches!(input.action, Action::ReleaseBasicCharge { .. }) {
                    self.publish(i);
                }
                Err(e)
            }
            Ok(events) => {
                self.events(events);
                if matches!(input.action, Action::Background {}) && before_background {
                    self.seats[i].epoch =
                        self.seats[i].epoch.checked_add(1).ok_or(Error::Faulted)?;
                    self.seats[i].queue.clear();
                    if let Some(s) = &mut self.seats[i].stream {
                        s.reset(FullReason::Lifecycle);
                    }
                }
                if matches!(input.action, Action::CombatReady { .. }) {
                    self.seats[i].resume_until = None;
                    self.seats[i].mode_forfeit_at = None;
                }
                let activate = self.world.as_ref().is_some_and(|w| {
                    w.active_at.is_none() && w.members.iter().all(|m| m.joined && m.ready)
                });
                if activate {
                    let mut events = vec![];
                    self.world
                        .as_mut()
                        .unwrap()
                        .activate(&self.config, self.now, &mut events);
                    self.phase = Phase::Active;
                    for s in &mut self.seats {
                        s.ledger.ledger_activate();
                    }
                    self.events(events);
                }
                self.sync()?;
                self.publish_all();
                Ok(self.status())
            }
        };
        self.remember(i, input, result)
    }
    pub(crate) fn transport(&mut self, r: TransportReceipt) -> Result<Status> {
        self.check_key(&r.key)?;
        let i = self.index(&r.recipient_id)?;
        if r.connection != self.seats[i].connection {
            return Err(Error::StaleConnection);
        }
        if matches!(self.phase, Phase::Closed | Phase::Faulted) && self.create.protocol_version != 6
        {
            return Err(Error::Closed);
        }
        if self.create.protocol_version == 6 {
            return self.transport_v6(r, i);
        }
        let epoch = self.seats[i].epoch;
        let p = self.seats[i]
            .publications
            .iter_mut()
            .find(|p| p.id == r.output_id)
            .ok_or(Error::StalePublication)?;
        if p.epoch != epoch {
            return Err(Error::StalePublication);
        }
        if p.outcome == r.outcome || r.outcome == TransportOutcome::Enqueued {
            return Ok(self.status());
        }
        if p.outcome != TransportOutcome::Enqueued {
            return Err(Error::Conflict);
        }
        p.outcome = r.outcome;
        if matches!(
            r.outcome,
            TransportOutcome::Failed | TransportOutcome::Cancelled
        ) && self.seats[i].last_full.as_ref() == Some(&r.output_id)
        {
            self.publish(i);
        }
        Ok(self.status())
    }
    pub(crate) fn poll_io(&mut self, elapsed: u64) {
        let elapsed = elapsed.saturating_sub(self.io_origin);
        self.io_elapsed = self.io_elapsed.max(elapsed);
        if self.create.protocol_version == 6 {
            self.poll_publications();
            self.poll_spectators();
        }
        for s in &mut self.seats {
            s.ledger.ledger_clock(self.now);
            s.ledger.poll_io(elapsed);
        }
        if self.sync().is_err() {
            self.fault(Error::Faulted)
        }
    }
    pub(crate) fn tick(&mut self) {
        if matches!(self.phase, Phase::Closed | Phase::Faulted) {
            return;
        }
        if self.reserve().is_err() {
            self.fault(Error::QueueFull);
            return;
        }
        self.now += self.config.step_ms;
        if matches!(self.phase, Phase::Preparing | Phase::Deploying)
            && self.now >= self.config.member_settings().unwrap().deployment_timeout_ms
        {
            let _ = self.abort();
            return;
        }
        let expired: Vec<_> = self
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.resume_until
                    .into_iter()
                    .chain(s.mode_forfeit_at)
                    .any(|at| self.now >= at)
            })
            .map(|(i, _)| i)
            .collect();
        if !expired.is_empty() && matches!(self.phase, Phase::Preparing | Phase::Deploying) {
            let _ = self.abort();
            return;
        }
        for i in expired {
            self.seats[i].resume_until = None;
            self.seats[i].mode_forfeit_at = None;
            if let Some(w) = &mut self.world {
                let mut e = vec![];
                w.set_contested_clock(self.now);
                w.withdraw_member(i, &self.config, &mut e);
                self.events(e);
            }
        }
        if let Some(w) = &mut self.world {
            let events = w.tick(&self.config, self.now);
            // Gameplay events are finite under the configured entity/projectile budgets.
            if self.outputs.len() + events.len() + self.reserve_margin()
                > self.config.output_capacity
            {
                self.fault(Error::QueueFull);
                return;
            }
            self.events(events);
        }
        if self.sync().is_err() {
            self.fault(Error::Faulted);
            return;
        }
        let interval = match self.create.publication_mode {
            PublicationMode::Batched => self.config.params.runtime.publish_batched_ms,
            PublicationMode::Interactive => self.config.step_ms,
        };
        for i in 0..self.seats.len() {
            if self.now >= self.seats[i].last_publish + interval {
                self.publish(i);
            }
        }
        self.publish_spectators_due(interval);
    }
    fn emit_closed(&mut self) {
        if self.closed_emitted {
            return;
        }
        self.closed_emitted = true;
        let pending = self.seats.iter().map(|s| s.ledger.ledger_pending()).sum();
        self.events(vec![(
            None,
            EventV4::Closed {
                live_worlds: 0,
                pending_operations: pending,
            },
        )]);
        self.dirty = true;
    }
    fn finish_close(&mut self) -> Result<()> {
        if self.closed_emitted {
            return Ok(());
        }
        self.phase = Phase::Closed;
        self.closed_at_io = Some(self.io_elapsed);
        self.finish_spectators();
        self.publish_all();
        self.world = None;
        for s in &mut self.seats {
            let _ = s.ledger.ledger_close()?;
        }
        self.harvest();
        self.emit_closed();
        Ok(())
    }
    fn abort(&mut self) -> Result<()> {
        self.world = None;
        self.finish_close()
    }
    pub(crate) fn close(&mut self, r: Close) -> Result<Status> {
        self.check_key(&r.key)?;
        if !valid_id(&r.request_id) {
            return Err(Error::InvalidMessage);
        }
        if self.closed_emitted {
            return Ok(self.status());
        }
        self.reserve()?;
        if let Some(w) = &mut self.world
            && w.active_at.is_some()
        {
            let mut e = vec![];
            for i in 0..w.members.len() {
                w.terminal_intent(i, Outcome::Death, &mut e);
            }
            w.expire_contested_equipment(&mut e);
            self.events(e);
            self.sync()?;
        }
        self.finish_close()?;
        Ok(self.status())
    }
    pub(crate) fn fault(&mut self, code: Error) {
        if self.closed_emitted {
            return;
        }
        // Financial reservations remain in each bounded ledger even if gameplay output is full.
        if let Some(w) = &mut self.world
            && w.active_at.is_some()
        {
            let mut ignored = vec![];
            for i in 0..w.members.len() {
                w.terminal_intent(i, Outcome::Death, &mut ignored);
            }
            w.expire_contested_equipment(&mut ignored);
        }
        let _ = self.sync();
        let _ = self.finish_close();
        self.phase = Phase::Faulted;
        if self.outputs.len() < self.config.output_capacity {
            self.events(vec![(None, EventV4::Fault { code })]);
        }
    }
    pub(crate) fn drain(&mut self) -> Vec<Output> {
        if !self.undelivered.is_empty() {
            return std::mem::take(&mut self.undelivered);
        }
        self.harvest();
        let mut outputs = self.outputs.drain(..).collect::<Vec<_>>();
        // Harvest member queues after freeing the coordinator queue as well.
        if self.create.protocol_version == 6 {
            self.flush_publication_notices();
        }
        self.harvest();
        outputs.extend(self.outputs.drain(..));
        if self.dirty {
            self.output_sequence += 1;
            let checkpoint = self.checkpoint();
            let id = format!(
                "{}:{}:r{}:out:{:020}",
                self.create.key.id,
                self.create.key.incarnation,
                self.generation,
                self.output_sequence
            );
            outputs.push(Output {
                key: self.create.key.clone(),
                id,
                logical_ms: self.now,
                body: OutputBody::ResponsibilityV4 {
                    checkpoint: Box::new(checkpoint),
                },
                recovery: None,
            });
            self.dirty = false;
        }
        for seat in &mut self.seats {
            for o in seat.queue.drain(..) {
                if let Some(s) = &mut seat.stream {
                    s.mark_drained(&o.id, self.io_elapsed);
                }
                seat.publications.push_back(Publication {
                    id: o.id.clone(),
                    epoch: seat.epoch,
                    connection: seat.connection.clone(),
                    outcome: TransportOutcome::Enqueued,
                    full:!matches!(&o.body,OutputBody::PublicationV6{payload,..} if matches!(payload.as_ref(),PublicationPayloadV6::Delta{..})),
                });
                outputs.push(o);
            }
            while seat.publications.len() > self.config.history_capacity {
                seat.publications.pop_front();
            }
        }
        self.drain_spectators(&mut outputs);
        outputs.sort_by(|a, b| a.id.cmp(&b.id));
        outputs
    }
    pub(crate) fn restore_undelivered(&mut self, outputs: Vec<Output>) {
        assert!(self.undelivered.is_empty());
        self.undelivered = outputs;
    }
    #[cfg(all(test, match_sdk_test_data))]
    pub(crate) fn pending(&self) -> u32 {
        self.seats.iter().map(|s| s.ledger.ledger_pending()).sum()
    }
}
