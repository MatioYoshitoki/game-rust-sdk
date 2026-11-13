use crate::config::{Config, SkillConfig, distance, normalize};
use crate::protocol::*;
use std::collections::BTreeMap;
mod interaction;
pub(crate) mod pve;
pub(crate) mod two_members;

#[derive(Clone)]
struct PendingAction {
    ground_position: Option<[f64; 2]>,
    unit_target: Option<UnitTarget>,
    selected_skill: Option<SkillConfig>,
    rounds: Vec<u32>,
    accepted_at: u64,
    id: String,
    due_ms: u64,
    skill: bool,
    direction: [f64; 2],
}

struct Projectile {
    position: [f64; 2],
    direction: [f64; 2],
    speed: f64,
    remaining: f64,
    radius: f64,
    damage: u32,
    skill: bool,
    expires_ms: u64,
    created_ms: u64,
}

struct Resource {
    view: ResourceView,
    hp: u32,
    spirit: u32,
}

pub(crate) struct World {
    pve: Option<pve::Arena>,
    pub phase: Phase,
    pub joined: bool,
    pub ready: bool,
    pub protected: bool,
    pub foreground: bool,
    pub active_at: Option<u64>,
    pub position: [f64; 2],
    pub hp: u32,
    pub spirit: u32,
    pub facts: ParticipantFacts,
    pub inventory: u32,
    pub inventory_revision: u32,
    pub looted: u32,
    pub kills: u32,
    pub desired_outcome: Option<Outcome>,
    pub extraction_end: Option<u64>,
    movement: [f64; 2],
    pending: Option<PendingAction>,
    recovery_until: u64,
    skill_ready_at: u64,
    skill_cooldowns: BTreeMap<String, u64>,
    basic_ready_at: u64,
    projectiles: Vec<Projectile>,
    target: TargetView,
    target_next_attack: u64,
    resources: Vec<Resource>,
    next_regen: u64,
    rng: u32,
}

impl World {
    pub(crate) fn close_interaction_event_budget(&self) -> usize {
        self.pve.as_ref().map_or(0, |a| {
            usize::from(a.has_piercing())
                + usize::from(a.has_player_fields())
                + usize::from(a.has_player_areas())
                + usize::from(a.has_unlock())
                + usize::from(!a.statuses.is_empty())
                + usize::from(a.has_area_warning())
                + usize::from(a.has_support_cast())
                + usize::from(a.has_heavy_state())
                + usize::from(a.has_reset_countdown())
                + usize::from(a.has_timed_position_warning())
        })
    }
    pub(crate) fn close_container_interaction(&mut self, now: u64) -> Vec<Event> {
        let mut events = vec![];
        if let Some(a) = &mut self.pve {
            a.cancel_player_areas(PlayerAreaCancelReason::Closed, &mut events);
            a.clear_statuses(&mut events);
            a.cancel_timed_position(&mut events);
            a.cancel_all_reset_countdowns(&mut events);
            a.cancel_all_heavies(HeavyCancelReason::Closed, now, &mut events);
            a.cancel_all_areas(AreaCancelReason::Closed, now, &mut events);
            a.cancel_all_supports(SupportCancelReason::Closed, now, &mut events);
            a.interrupt_unlock(ChestInterruptReason::Closed, &mut events);
        }
        events
    }
    pub fn new(config: &Config, facts: ParticipantFacts, seed: u32) -> Self {
        Self {
            pve: config.pve.as_ref().map(pve::Arena::new),
            phase: Phase::Deploying,
            joined: false,
            ready: false,
            protected: true,
            foreground: true,
            active_at: None,
            position: config.spawn,
            hp: facts.hp_points,
            spirit: facts.spirit_points,
            facts,
            inventory: config.carried_quantity,
            inventory_revision: 1,
            looted: 0,
            kills: 0,
            desired_outcome: None,
            extraction_end: None,
            movement: [0.0, 0.0],
            pending: None,
            recovery_until: 0,
            skill_ready_at: 0,
            skill_cooldowns: BTreeMap::new(),
            basic_ready_at: 0,
            projectiles: Vec::new(),
            target: TargetView {
                id: config.target.id.clone(),
                position: config.target.position,
                hp: config.target.hp,
            },
            target_next_attack: 0,
            resources: Vec::new(),
            next_regen: 0,
            rng: if seed == 0 { 0x9e3779b9 } else { seed },
        }
    }

    fn skill_ready(&self, c: &Config, id: &str) -> u64 {
        if id == c.skill.id {
            self.skill_ready_at
        } else {
            self.skill_cooldowns.get(id).copied().unwrap_or(0)
        }
    }
    fn set_skill_ready(&mut self, c: &Config, id: &str, deadline: u64) {
        if id == c.skill.id {
            self.skill_ready_at = deadline;
        } else {
            self.skill_cooldowns.insert(id.into(), deadline);
        }
    }
    fn skill_views(&self, c: &Config) -> Option<Vec<PlayerSkillView>> {
        (c.mechanism_version >= 26).then(|| {
            c.player_skills()
                .map(|s| PlayerSkillView {
                    skill_id: s.id.clone(),
                    ready_at_ms: self.skill_ready(c, &s.id),
                })
                .collect()
        })
    }
    fn pending_view(&self, c: &Config) -> Option<PlayerActionView> {
        if c.mechanism_version < 26 {
            return None;
        }
        self.pending.as_ref().map(|a| PlayerActionView {
            action_id: None,
            resource_reservation: None,
            charge: None,
            ally_target: None,
            ally_target_is_self: None,
            ground_position: if c.mechanism_version >= 29 {
                a.ground_position
            } else {
                None
            },
            unit_target: if c.mechanism_version >= 28 {
                a.unit_target.clone()
            } else {
                None
            },
            skill_id: a
                .skill
                .then(|| a.selected_skill.as_ref().unwrap_or(&c.skill).id.clone()),
            commits_at_ms: a.due_ms,
        })
    }
    pub fn snapshot(&self, config: &Config, now: u64) -> Snapshot {
        if self.pve.is_some() {
            return self.pve_snapshot(config, now);
        }
        Snapshot {
            shield_total: None,
            effective_atk: None,
            healing_received_bps: None,
            statuses: vec![],
            effective_def: None,
            max_hp: self.facts.hp_points,
            max_spirit: self.facts.spirit_points,
            pve: None,
            phase: self.phase,
            logical_ms: now,
            active_at_ms: self.active_at,
            position: self.position,
            hp: self.hp,
            spirit: self.spirit,
            protected: self.protected,
            basic_ready_at_ms: self.basic_ready_at,
            skill_ready_at_ms: self.skill_ready_at,
            player_skills: None,
            pending_action: None,
            recovery_until_ms: self.recovery_until,
            inventory_quantity: self.inventory,
            inventory_revision: self.inventory_revision,
            target: (self.target.hp > 0).then(|| self.target.clone()),
            resources: self.resources.iter().map(|r| r.view.clone()).collect(),
            extraction_open: self
                .active_at
                .is_some_and(|at| now >= at + config.extraction_unlock_ms),
            extraction_finishes_ms: self.extraction_end,
        }
    }

    pub fn action(
        &mut self,
        action: &Action,
        id: &str,
        config: &Config,
        now: u64,
    ) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        if self.desired_outcome.is_some() || self.phase == Phase::Draining {
            return Err(Error::InvalidState);
        }
        match action {
            Action::Join {} => {
                if !self.joined {
                    self.joined = true;
                    events.push(Event::Joined);
                }
                return Ok(events);
            }
            Action::CombatReady { .. } => {
                if !self.joined || self.hp == 0 || self.desired_outcome.is_some() {
                    return Err(Error::InvalidState);
                }
                if !self.foreground {
                    return Err(Error::NotReady);
                }
                self.ready = true;
                self.protected = false;
                return Ok(events);
            }
            Action::Background {} => {
                if let Some(arena) = &mut self.pve {
                    arena.stop_navigation();
                    arena.cancel_timed_position(&mut events);
                    arena.cancel_all_heavies(HeavyCancelReason::Background, now, &mut events);
                    arena.cancel_all_areas(AreaCancelReason::Background, now, &mut events);
                    arena.cancel_all_supports(SupportCancelReason::Background, now, &mut events);
                    arena.interrupt_unlock(ChestInterruptReason::Background, &mut events);
                }
                self.foreground = false;
                self.movement = [0.0, 0.0];
                self.protected = true;
                self.ready = false;
                self.interrupt(&mut events);
                self.interrupt_extraction(&mut events);
                return Ok(events);
            }
            Action::Foreground {} => {
                self.foreground = true;
                return Ok(events);
            }
            _ => {}
        }
        if self.phase != Phase::Active || self.desired_outcome.is_some() {
            return Err(Error::InvalidState);
        }
        if !self.ready || self.protected || self.hp == 0 {
            return Err(Error::NotReady);
        }
        if self.pve.is_some() {
            return self.pve_action(action, id, config, now);
        }
        match action {
            Action::AttuneObjective { .. }
            | Action::CancelAttunement {}
            | Action::OpenChest { .. }
            | Action::CancelChestUnlock { .. } => {
                return Err(Error::Unsupported);
            }
            Action::Move { direction } => {
                self.movement = normalize(*direction);
                self.recovery_until = now;
            }
            Action::Stop {} => self.movement = [0.0, 0.0],
            Action::Attack {} | Action::Cast { .. } => {
                if self.pending.is_some() {
                    return Err(Error::InvalidState);
                }
                let (skill, direction, windup) = match action {
                    Action::Cast {
                        skill_id,
                        direction,
                    } => {
                        if skill_id != &config.skill.id {
                            return Err(Error::Unsupported);
                        }
                        if now < self.skill_ready_at {
                            return Err(Error::InvalidState);
                        }
                        if self.spirit < config.skill.cost {
                            return Err(Error::NoResource);
                        }
                        let direction = normalize(*direction);
                        if direction == [0.0, 0.0] {
                            return Err(Error::InvalidMessage);
                        }
                        (true, direction, config.skill.windup_ms)
                    }
                    _ => {
                        if now < self.basic_ready_at {
                            return Err(Error::InvalidState);
                        }
                        if self.target.hp == 0 {
                            return Err(Error::NoResource);
                        }
                        if distance(self.position, self.target.position) > config.basic.range {
                            return Err(Error::OutOfRange);
                        }
                        self.basic_ready_at = now + config.basic.interval_ms;
                        (false, [0.0, 0.0], config.basic.windup_ms)
                    }
                };
                // This profile permits explicitly cancelling recovery with a new action.
                self.recovery_until = now;
                self.pending = Some(PendingAction {
                    ground_position: None,
                    unit_target: None,
                    selected_skill: None,
                    rounds: vec![],
                    accepted_at: now,
                    id: id.to_owned(),
                    due_ms: now + windup,
                    skill,
                    direction,
                });
            }
            Action::CancelAction {} => self.interrupt(&mut events),
            Action::Pickup {
                resource_id,
                expected_revision,
            } => {
                if *expected_revision != self.inventory_revision {
                    return Err(Error::RevisionConflict);
                }
                let index = self
                    .resources
                    .iter()
                    .position(|r| &r.view.id == resource_id)
                    .ok_or(Error::NoResource)?;
                let resource = &self.resources[index];
                if distance(self.position, resource.view.position) > config.params.pickup_range
                    || !config
                        .geometry
                        .path_clear(self.position, resource.view.position, 0.0)
                {
                    return Err(Error::OutOfRange);
                }
                let quantity = resource.view.quantity;
                if !resource.view.orb
                    && self.inventory + quantity > config.inventory_capacity * config.stack_limit
                {
                    return Err(Error::Capacity);
                }
                if resource.view.orb {
                    self.hp = (self.hp + resource.hp).min(self.facts.hp_points);
                    self.spirit = (self.spirit + resource.spirit).min(self.facts.spirit_points);
                } else {
                    self.inventory += quantity;
                    self.looted += quantity;
                    self.inventory_revision += 1;
                }
                self.resources.remove(index);
                events.push(Event::PickedUp {
                    resource_id: resource_id.clone(),
                    quantity,
                });
            }
            Action::ExtractStart {} => {
                let active_at = self.active_at.ok_or(Error::InvalidState)?;
                if now < active_at + config.extraction_unlock_ms || self.extraction_end.is_some() {
                    return Err(Error::InvalidState);
                }
                if distance(self.position, config.extraction_point) > config.params.extraction_range
                {
                    return Err(Error::OutOfRange);
                }
                self.extraction_end = Some(now + config.extraction_channel_ms);
                events.push(Event::ExtractionStarted);
            }
            Action::ExtractCancel {} => self.interrupt_extraction(&mut events),
            Action::Leave {} => self.die(&mut events),
            _ => return Err(Error::InvalidState),
        }
        Ok(events)
    }

    fn interrupt(&mut self, events: &mut Vec<Event>) {
        if self.pending.take().is_some() {
            events.push(Event::ActionInterrupted);
        }
    }

    fn interrupt_extraction(&mut self, events: &mut Vec<Event>) {
        if self.extraction_end.take().is_some() {
            events.push(Event::ExtractionInterrupted);
        }
    }

    fn die(&mut self, events: &mut Vec<Event>) {
        if let Some(arena) = &mut self.pve {
            arena.clear_statuses(events);
            arena.cancel_timed_position(events);
            arena.cancel_all_reset_countdowns(events);
            arena.cancel_all_heavies(HeavyCancelReason::Closed, arena.current_time(), events);
            arena.cancel_player_areas(PlayerAreaCancelReason::OwnerDied, events);
            arena.cancel_all_areas(AreaCancelReason::Closed, arena.current_time(), events);
            arena.cancel_all_supports(SupportCancelReason::Closed, arena.current_time(), events);
            arena.interrupt_unlock(ChestInterruptReason::Closed, events);
        }
        self.hp = 0;
        self.desired_outcome = Some(Outcome::Death);
        self.movement = [0.0, 0.0];
        self.interrupt(events);
        self.interrupt_extraction(events);
    }

    pub fn tick(&mut self, config: &Config, now: u64) -> Vec<Event> {
        let mut events = Vec::new();
        if self.phase == Phase::Deploying && self.joined && self.ready {
            self.phase = Phase::Active;
            self.active_at = Some(now);
            self.target_next_attack = now + config.target.interval_ms;
            self.next_regen = now + config.params.regen.interval_ms;
            events.push(Event::Activated);
        }
        if self.phase != Phase::Active || self.desired_outcome.is_some() {
            return events;
        }
        let active_at = self.active_at.expect("active has clock");
        if now >= active_at + config.terminal_ms {
            self.die(&mut events);
            return events;
        }

        if self.pve.is_some() {
            return self.pve_tick(config, now, events);
        }
        let dt = config.step_ms as f64 / 1000.0;
        if !self.protected {
            let speed = config.params.movement.speed(self.facts.spd_points);
            let destination = [
                self.position[0] + self.movement[0] * speed * dt,
                self.position[1] + self.movement[1] * speed * dt,
            ];
            if config
                .geometry
                .path_clear(self.position, destination, config.params.player_radius)
                && (self.target.hp == 0
                    || distance(destination, self.target.position)
                        >= config.params.player_radius + config.target.radius)
            {
                self.position = destination;
            }
        }
        if self.extraction_end.is_some()
            && distance(self.position, config.extraction_point) > config.params.extraction_range
        {
            self.interrupt_extraction(&mut events);
        }

        if self.target.hp > 0 && !self.protected {
            let separation = distance(self.position, self.target.position);
            if separation <= config.target.aggro_range {
                if separation > config.target.attack_range {
                    let direction = normalize([
                        self.position[0] - self.target.position[0],
                        self.position[1] - self.target.position[1],
                    ]);
                    let budget = (config.target.speed * dt)
                        .min((separation - config.target.attack_range).max(0.0));
                    let end = [
                        self.target.position[0] + direction[0] * budget,
                        self.target.position[1] + direction[1] * budget,
                    ];
                    if config
                        .geometry
                        .path_clear(self.target.position, end, config.target.radius)
                    {
                        self.target.position = end;
                    }
                } else if now >= self.target_next_attack
                    && config
                        .geometry
                        .path_clear(self.target.position, self.position, 0.0)
                {
                    self.target_next_attack = now + config.target.interval_ms;
                    let amount = damage(
                        config.target.atk,
                        f64::from(self.facts.def_points),
                        1.0,
                        config,
                    )
                    .min(self.hp);
                    self.hp = self.hp.saturating_sub(amount);
                    events.push(Event::Damage {
                        source: config.target.id.clone(),
                        target: "participant".into(),
                        amount,
                    });
                    self.interrupt_extraction(&mut events);
                    if self.hp == 0 {
                        self.die(&mut events);
                        return events;
                    }
                }
            }
        }

        if self.pending.as_ref().is_some_and(|a| a.due_ms <= now) {
            let action = self.pending.take().expect("checked action");
            if !action.skill
                && (self.target.hp == 0
                    || distance(self.position, self.target.position) > config.basic.range)
            {
                events.push(Event::ActionInterrupted);
            } else {
                let (direction, speed, range, radius, raw, cost) = if action.skill {
                    self.spirit -= config.skill.cost;
                    self.skill_ready_at = now + config.skill.cooldown_ms;
                    self.recovery_until = now + config.skill.recovery_ms;
                    (
                        action.direction,
                        config.skill.speed,
                        config.skill.range,
                        config.skill.radius,
                        (u64::from(self.facts.atk_points) * u64::from(config.skill.damage_bps)
                            / 10000) as u32,
                        config.skill.cost,
                    )
                } else {
                    (
                        normalize([
                            self.target.position[0] - self.position[0],
                            self.target.position[1] - self.position[1],
                        ]),
                        config.basic.projectile_speed,
                        config.basic.range,
                        config.params.basic_projectile_radius,
                        self.facts.atk_points,
                        0,
                    )
                };
                self.projectiles.push(Projectile {
                    position: self.position,
                    direction,
                    speed,
                    remaining: range,
                    radius,
                    damage: damage(raw, f64::from(config.target.armor), 1.0, config),
                    skill: action.skill,
                    expires_ms: now + config.params.player_projectile_ttl_ms,
                    created_ms: now,
                });
                events.push(Event::ActionCommitted {
                    ground_position: None,
                    unit_target: None,
                    skill_id: None,
                    action_id: action.id,
                    skill: action.skill,
                    spirit_cost: cost,
                    hp_cost: None,
                    charge: None,
                });
            }
        }

        let mut projectiles = std::mem::take(&mut self.projectiles);
        for mut projectile in projectiles.drain(..) {
            if now >= projectile.expires_ms {
                continue;
            }
            if projectile.created_ms == now {
                self.projectiles.push(projectile);
                continue;
            }
            let travel = (projectile.speed * dt).min(projectile.remaining);
            let end = [
                projectile.position[0] + projectile.direction[0] * travel,
                projectile.position[1] + projectile.direction[1] * travel,
            ];
            let wall_t = config
                .geometry
                .walls
                .iter()
                .filter_map(|wall| {
                    segment_rect(
                        projectile.position,
                        end,
                        wall.min,
                        wall.max,
                        projectile.radius,
                    )
                })
                .min_by(f64::total_cmp);
            let hit_t = (self.target.hp > 0)
                .then(|| {
                    segment_circle(
                        projectile.position,
                        end,
                        self.target.position,
                        config.target.radius + projectile.radius,
                    )
                })
                .flatten();
            if hit_t.is_some_and(|t| wall_t.is_none_or(|w| t < w)) {
                let amount = projectile.damage.min(self.target.hp);
                self.target.hp -= amount;
                if !projectile.skill {
                    self.spirit = (self.spirit + config.params.basic_spirit_on_hit)
                        .min(self.facts.spirit_points);
                }
                events.push(Event::Damage {
                    source: "participant".into(),
                    target: self.target.id.clone(),
                    amount,
                });
                if self.target.hp == 0 {
                    self.killed_target(config, &mut events);
                }
            } else if wall_t.is_none()
                && config.geometry.walkable(end, projectile.radius)
                && projectile.remaining > travel
            {
                projectile.position = end;
                projectile.remaining -= travel;
                self.projectiles.push(projectile);
            }
        }

        if now >= self.next_regen {
            self.next_regen += config.params.regen.interval_ms;
            let ratio = if self.target.hp > 0
                && distance(self.position, self.target.position) <= config.params.regen.near_radius
            {
                config.params.regen.near_ratio
            } else {
                config.params.regen.far_ratio
            };
            self.hp = (self.hp + (f64::from(self.facts.hp_points) * ratio).floor() as u32)
                .min(self.facts.hp_points);
            self.spirit = (self.spirit
                + (f64::from(self.facts.spirit_points) * ratio).floor() as u32)
                .min(self.facts.spirit_points);
        }
        if self.extraction_end.is_some_and(|deadline| now >= deadline) {
            self.desired_outcome = Some(Outcome::Extraction);
        }
        events
    }

    fn random(&mut self) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng
    }

    fn killed_target(&mut self, config: &Config, events: &mut Vec<Event>) {
        self.kills += 1;
        let angle = f64::from(self.random()) / f64::from(u32::MAX) * std::f64::consts::TAU;
        let radius = f64::from(
            config.params.drop_min_radius
                + self.random()
                    % (config.params.drop_max_radius - config.params.drop_min_radius + 1),
        );
        let candidate = [
            self.target.position[0] + angle.cos() * radius,
            self.target.position[1] + angle.sin() * radius,
        ];
        let position = if config
            .geometry
            .path_clear(self.target.position, candidate, 2.0)
        {
            candidate
        } else {
            self.target.position
        };
        self.resources.push(Resource {
            view: ResourceView {
                id: "drop-1".into(),
                position,
                quantity: config.target.drop_quantity,
                orb: false,
            },
            hp: 0,
            spirit: 0,
        });
        self.resources.push(Resource {
            view: ResourceView {
                id: "orb-1".into(),
                position: self.target.position,
                quantity: 0,
                orb: true,
            },
            hp: (f64::from(self.facts.hp_points) * config.params.orb_hp_ratio).floor() as u32,
            spirit: (f64::from(self.facts.spirit_points) * config.params.orb_spirit_ratio).floor()
                as u32,
        });
        events.push(Event::TargetKilled);
    }

    pub fn settlement(&mut self, config: &Config) -> Business {
        if self.pve.is_some() {
            return self.pve_settlement(config);
        }
        let outcome = self.desired_outcome.expect("terminal outcome");
        self.phase = Phase::Draining;
        self.protected = true;
        self.movement = [0.0, 0.0];
        self.pending = None;
        self.projectiles.clear();
        self.extraction_end = None;
        // B1 contains one material namespace; each stack is one eligible slot.
        let dropped = if config.settlement_policy_outcome(outcome) == Outcome::Death {
            let quantities: Vec<u32> = (0..self.inventory.div_ceil(config.stack_limit))
                .map(|i| (self.inventory - i * config.stack_limit).min(config.stack_limit))
                .collect();
            let count = config.params.death.count(quantities.len());
            let mut slots = quantities;
            let mut sum = 0;
            for _ in 0..count {
                let index = self.random() as usize % slots.len();
                sum += config.params.death.amount(slots.swap_remove(index));
            }
            sum
        } else {
            0
        };
        let cultivation = config.cultivation(self.kills, outcome);
        Business::CommitSettlement {
            scoring: None,
            map_progress: None,
            reset_progress: None,
            outcome,
            expected_revision: self.facts.revision,
            item_id: config.item_id.clone(),
            carried: config.carried_quantity,
            looted: self.looted,
            kept: self.inventory - dropped,
            dropped,
            cultivation,
            items: Vec::new(),
            instances: Vec::new(),
            rewards: Vec::new(),
            progression: None,
        }
    }
}

fn damage(raw: u32, armor: f64, multiplier: f64, config: &Config) -> u32 {
    let reduction = (armor
        / (armor + config.params.defense_scale * f64::from(config.defender_realm_atk)))
    .min(config.defense_cap);
    (f64::from(raw) * (1.0 - reduction) * multiplier)
        .floor()
        .max(f64::from(config.params.minimum_damage)) as u32
}

fn segment_circle(start: [f64; 2], end: [f64; 2], center: [f64; 2], radius: f64) -> Option<f64> {
    let d = [end[0] - start[0], end[1] - start[1]];
    let f = [start[0] - center[0], start[1] - center[1]];
    let c = f[0] * f[0] + f[1] * f[1] - radius * radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    let a = d[0] * d[0] + d[1] * d[1];
    if a <= f64::EPSILON {
        return None;
    }
    let b = 2.0 * (f[0] * d[0] + f[1] * d[1]);
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let t = (-b - discriminant.sqrt()) / (2.0 * a);
    (0.0..=1.0).contains(&t).then_some(t)
}

fn segment_rect(
    start: [f64; 2],
    end: [f64; 2],
    min: [f64; 2],
    max: [f64; 2],
    radius: f64,
) -> Option<f64> {
    let (mut enter, mut exit): (f64, f64) = (0.0, 1.0);
    for axis in 0..2 {
        let delta = end[axis] - start[axis];
        if delta.abs() < 1e-12 {
            if start[axis] < min[axis] - radius || start[axis] > max[axis] + radius {
                return None;
            }
        } else {
            let a = (min[axis] - radius - start[axis]) / delta;
            let b = (max[axis] + radius - start[axis]) / delta;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
            if enter > exit {
                return None;
            }
        }
    }
    Some(enter)
}
