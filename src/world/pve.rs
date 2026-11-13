use super::*;
use crate::config::{MonsterTemplate, PveConfig};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::Arc;
mod area;
#[cfg(all(test, match_sdk_test_data))]
mod area_tests;
#[cfg(all(test, match_sdk_test_data))]
mod attribute_tests;
mod boss;
#[cfg(all(test, match_sdk_test_data))]
mod boss_tests;
mod chests;
#[cfg(all(test, match_sdk_test_data))]
mod contact_policy_tests;
#[cfg(test)]
mod drop_ttl_tests;
#[cfg(all(test, match_sdk_test_data))]
mod encounter_tests;
mod encounters;
mod field;
#[cfg(all(test, match_sdk_test_data))]
mod field_skill_tests;
mod ground;
#[cfg(all(test, match_sdk_test_data))]
mod ground_skill_tests;
#[cfg(all(test, match_sdk_test_data))]
mod guardian_tests;
mod healing;
#[cfg(all(test, match_sdk_test_data))]
mod healing_received_tests;
#[cfg(all(test, match_sdk_test_data))]
mod healing_tests;
mod heavy;
#[cfg(all(test, match_sdk_test_data))]
mod heavy_tests;
pub(super) mod line;
#[cfg(all(test, match_sdk_test_data))]
mod line_field_tests;
mod objective;
#[cfg(all(test, match_sdk_test_data))]
mod objective_tests;
#[cfg(all(test, match_sdk_test_data))]
mod parameter_tests;
mod periodic;
#[cfg(all(test, match_sdk_test_data))]
mod periodic_tests;
mod piercing;
#[cfg(all(test, match_sdk_test_data))]
mod piercing_tests;
#[cfg(all(test, match_sdk_test_data))]
mod player_skills_tests;
#[cfg(all(test, match_sdk_test_data))]
mod projectile_repeat_tests;
mod reset;
#[cfg(all(test, match_sdk_test_data))]
mod reset_tests;
#[cfg(all(test, match_sdk_test_data))]
mod scoring_tests;
#[cfg(all(test, match_sdk_test_data))]
mod self_skill_tests;
#[cfg(all(test, match_sdk_test_data))]
mod shield_tests;
#[cfg(all(test, match_sdk_test_data))]
mod stack_tests;
mod status;
#[cfg(all(test, match_sdk_test_data))]
mod status_tests;
mod support;
#[cfg(all(test, match_sdk_test_data))]
mod support_tests;
#[cfg(all(test, match_sdk_test_data))]
mod sword_trial_tests;
mod terrain;
mod timed_position;
#[cfg(all(test, match_sdk_test_data))]
mod timed_position_tests;
#[cfg(all(test, match_sdk_test_data))]
mod unit_skill_tests;
mod unit_target;
#[cfg(all(test, match_sdk_test_data))]
mod wave_tests;

fn scaled_amount(base: u32, ratio_bps: u32, scale_bps: u32) -> u32 {
    (u64::from(base) * u64::from(ratio_bps) * u64::from(scale_bps) / 100_000_000)
        .min(u64::from(u32::MAX)) as u32
}

mod grid_geometry;
mod isometric_grid;

pub(super) struct Grid {
    pub config: Arc<PveConfig>,
    cells: Arc<[usize]>,
    nav_budget: usize,
    geometry: Option<grid_geometry::Geometry>,
    isometric_tiles: Option<isometric_grid::IsometricTiles>,
    terrain_height_enabled: bool,
    pub(super) terrain_damage_enabled: bool,
}
impl Grid {
    pub(super) fn new(config: &Arc<PveConfig>) -> Self {
        let cells = if config.cells.is_empty() {
            // Unit fixtures may construct tiny maps directly. Production catalog entries are prepared.
            #[cfg(test)]
            {
                config
                    .cells_rle
                    .iter()
                    .flat_map(|(i, n)| std::iter::repeat_n(*i, *n))
                    .collect::<Vec<_>>()
                    .into()
            }
            #[cfg(not(test))]
            {
                panic!("unprepared configuration")
            }
        } else {
            config.cells.clone()
        };
        let advanced_terrain = config.world_geometry.is_some() || config.isometric_grid.is_some();
        Self {
            nav_budget: config.prepared.as_ref().unwrap().runtime.nav_budget,
            config: config.clone(),
            cells,
            geometry: config
                .world_geometry
                .as_ref()
                .map(grid_geometry::Geometry::new),
            isometric_tiles: isometric_grid::IsometricTiles::new(config),
            terrain_height_enabled: advanced_terrain
                && config.palette.iter().any(|tile| {
                    tile.slope.is_some() || tile.elevation != config.palette[0].elevation
                }),
            terrain_damage_enabled: advanced_terrain
                && config
                    .palette
                    .iter()
                    .any(|tile| tile.damage_milli_per_second() > 0),
        }
    }
    pub(in crate::world) fn index(&self, p: [f64; 2]) -> Option<usize> {
        let p = self.config.to_tile(p);
        if !(p[0] >= 0.0 && p[1] >= 0.0) {
            return None;
        }
        let (x, y) = (p[0] as usize, p[1] as usize);
        (x < self.config.width && y < self.config.height).then(|| y * self.config.width + x)
    }
    fn center(&self, i: usize) -> [f64; 2] {
        let tile = [
            (i % self.config.width) as f64,
            (i / self.config.width) as f64,
        ];
        if self.isometric_tiles.is_some() {
            self.config
                .to_world(tile.map(|coordinate| coordinate + 0.5))
        } else {
            let size = self.config.cell_size;
            tile.map(|coordinate| coordinate * size + size / 2.)
        }
    }
    pub(super) fn clear(&self, p: [f64; 2], r: f64, kind: u8) -> bool {
        self.clear_tiles(p, r, kind) && self.geometry_contact(p, p, r, kind).is_none()
    }
    fn clear_tiles(&self, p: [f64; 2], r: f64, kind: u8) -> bool {
        if self.isometric_tiles.is_some() {
            return self.clear_isometric_tiles(p, r, kind);
        }
        let s = self.config.cell_size;
        if p[0] - r < 0.0
            || p[1] - r < 0.0
            || p[0] + r > self.config.width as f64 * s
            || p[1] + r > self.config.height as f64 * s
        {
            return false;
        }
        for y in ((p[1] - r) / s).floor() as usize
            ..=((p[1] + r) / s)
                .floor()
                .min(self.config.height as f64 - 1.0) as usize
        {
            for x in ((p[0] - r) / s).floor() as usize
                ..=((p[0] + r) / s).floor().min(self.config.width as f64 - 1.0) as usize
            {
                let tile = &self.config.palette[self.cells[y * self.config.width + x]];
                let blocked = match kind {
                    1 => tile.blocks_projectiles,
                    2 => tile.blocks_vision,
                    _ => !tile.walkable,
                };
                let nearest = [
                    p[0].clamp(x as f64 * s, (x + 1) as f64 * s),
                    p[1].clamp(y as f64 * s, (y + 1) as f64 * s),
                ];
                if blocked && distance(p, nearest) < r.max(1e-7) {
                    return false;
                }
            }
        }
        true
    }
    pub(super) fn line(&self, a: [f64; 2], b: [f64; 2], r: f64, kind: u8) -> bool {
        if self.geometry_contact(a, b, r, kind).is_some()
            || kind == 0 && self.terrain_step_contact(a, b).is_some()
        {
            return false;
        }
        if self.isometric_tiles.is_some() {
            return self.clear_tiles(a, r, kind)
                && self.clear_tiles(b, r, kind)
                && self.isometric_tile_contact(a, b, r, kind).is_none();
        }
        let steps = distance(a, b).ceil().max(1.0) as u32;
        (0..=steps).all(|i| {
            let t = f64::from(i) / f64::from(steps);
            self.clear_tiles(
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t],
                r,
                kind,
            )
        })
    }
    pub(super) fn speed(&self, p: [f64; 2]) -> f64 {
        self.index(p)
            .map_or(0.0, |i| self.config.palette[self.cells[i]].speed_multiplier)
    }
    pub(super) fn route(&self, a: [f64; 2], b: [f64; 2], radius: f64) -> Vec<[f64; 2]> {
        self.route_avoiding(a, b, radius, &[])
    }
    fn route_avoiding(
        &self,
        a: [f64; 2],
        b: [f64; 2],
        radius: f64,
        obstacles: &[([f64; 2], f64)],
    ) -> Vec<[f64; 2]> {
        let clear_line = |from: [f64; 2], to: [f64; 2]| {
            self.line(from, to, radius, 0)
                && obstacles.iter().all(|(center, r)| {
                    let d = [to[0] - from[0], to[1] - from[1]];
                    let length = d[0] * d[0] + d[1] * d[1];
                    let t = if length == 0.0 {
                        0.0
                    } else {
                        (((center[0] - from[0]) * d[0] + (center[1] - from[1]) * d[1]) / length)
                            .clamp(0.0, 1.0)
                    };
                    distance([from[0] + t * d[0], from[1] + t * d[1]], *center) + 1e-7 >= radius + r
                })
        };
        if clear_line(a, b) {
            return vec![b];
        }
        let (Some(start), Some(goal)) = (self.index(a), self.index(b)) else {
            return vec![];
        };
        let w = self.config.width;
        let heuristic = |i: usize| ((i % w).abs_diff(goal % w) + (i / w).abs_diff(goal / w)) as u32;
        let mut queue = BinaryHeap::new();
        let mut costs = vec![u32::MAX; self.cells.len()];
        let mut parent = vec![usize::MAX; self.cells.len()];
        costs[start] = 0;
        queue.push(Reverse((heuristic(start), start)));
        for _ in 0..self.nav_budget {
            let Some(Reverse((_, i))) = queue.pop() else {
                break;
            };
            if i == goal {
                if (!obstacles.is_empty()
                    || self.geometry.is_some()
                    || self.isometric_tiles.is_some())
                    && !clear_line(if i == start { a } else { self.center(i) }, b)
                {
                    continue;
                }
                let mut path = vec![b];
                let mut at = i;
                while at != start {
                    path.push(self.center(at));
                    at = parent[at];
                }
                path.reverse();
                return path;
            }
            for (dx, dy) in [(0_i32, -1), (-1, 0), (1, 0), (0, 1)] {
                let x = (i % w) as i32 + dx;
                let y = (i / w) as i32 + dy;
                if x < 0 || y < 0 || x >= w as i32 || y >= self.config.height as i32 {
                    continue;
                }
                let next = y as usize * w + x as usize;
                let point = self.center(next);
                if costs[i] + 1 >= costs[next]
                    || !clear_line(if i == start { a } else { self.center(i) }, point)
                {
                    continue;
                }
                costs[next] = costs[i] + 1;
                parent[next] = i;
                queue.push(Reverse((costs[next] + heuristic(next), next)));
            }
        }
        vec![]
    }
}
struct Monster {
    status_generation: u32,
    heavy_cast: Option<heavy::HeavyCast>,
    heavy_ready: u64,
    heavy_sequence: u32,
    support_cast: Option<support::SupportCast>,
    support_used: u32,
    support_ready: u64,
    support_sequence: u32,
    area_cast: Option<area::AreaCast>,
    area_ready: u64,
    area_sequence: u32,
    boss_phase: usize,
    boss_epoch: u32,
    encounter_index: Option<usize>,
    encounter_id: Option<String>,
    wave_id: Option<String>,
    id: String,
    template_id: String,
    profile: Arc<MonsterTemplate>,
    enabled: bool,
    binding: Option<GuardianBinding>,
    position: [f64; 2],
    home: [f64; 2],
    hp: u32,
    spirit: u32,
    aggro: bool,
    returning: bool,
    next_attack: u64,
    next_decision: u64,
    cooldown: [u64; 2],
    casting: Option<(usize, u64)>,
    heal: Option<(u64, u32, u32)>,
    recovery: u64,
    path: Vec<[f64; 2]>,
    repath: u64,
}
struct Shot {
    strict_target: Option<UnitTarget>,
    status_source: Option<StatusEntity>,
    hit_statuses: Vec<String>,
    rounds: Vec<u32>,
    manual_started: Option<u64>,
    position: [f64; 2],
    direction: [f64; 2],
    remaining: f64,
    speed: f64,
    radius: f64,
    raw: u32,
    owner: Option<usize>,
    metal: bool,
    mark: bool,
    expires: u64,
    created: u64,
}
impl Monster {
    fn new(
        id: String,
        template_id: String,
        profile: Arc<MonsterTemplate>,
        position: [f64; 2],
        binding: Option<GuardianBinding>,
    ) -> Self {
        Self {
            heavy_cast: None,
            heavy_ready: 0,
            heavy_sequence: 0,
            status_generation: 1,
            support_cast: None,
            support_used: 0,
            support_ready: 0,
            support_sequence: 0,
            area_cast: None,
            area_ready: 0,
            area_sequence: 0,
            boss_phase: 0,
            boss_epoch: 1,
            encounter_index: None,
            encounter_id: None,
            wave_id: None,
            id,
            template_id,
            hp: profile.stats["hp"],
            spirit: profile.stats["spirit"],
            next_attack: profile
                .boss
                .as_ref()
                .map_or(profile.combat.attack_interval_ms, |b| {
                    b.phases[0].attack_interval_ms
                }),
            profile,
            enabled: true,
            binding,
            position,
            home: position,
            aggro: false,
            returning: false,
            next_decision: 0,
            cooldown: [0, 0],
            casting: None,
            heal: None,
            recovery: 0,
            path: vec![],
            repath: 0,
        }
    }
}
pub(super) struct Arena {
    piercing_projectiles: Vec<piercing::PiercingProjectile>,
    player_areas: Vec<ground::PlayerArea>,
    player_fields: Vec<field::PlayerField>,
    pub(super) statuses: crate::status::Statuses,
    timed_position: Option<timed_position::TimedPositionState>,
    counter: Option<heavy::CounterWindow>,
    support_interrupt_ready: u64,
    attuned: bool,
    attune_finishes: Option<u64>,
    attune_interruption: Option<ChestInterruptReason>,
    score_tally: BTreeMap<String, u32>,
    encounters: Vec<encounters::Encounter>,
    chests: Vec<chests::Chest>,
    grid: Grid,
    monsters: Vec<Monster>,
    items: BTreeMap<String, u32>,
    resource_items: BTreeMap<String, String>,
    shots: Vec<Shot>,
    locked: Option<usize>,
    marks: u32,
    mark_sources: Vec<usize>,
    marks_until: u64,
    player_path: Vec<[f64; 2]>,
    resource_expiry: BTreeMap<String, u64>,
    now: u64,
    next_sweep: u64,
}
impl Arena {
    pub(super) fn stop_navigation(&mut self) {
        self.player_path.clear();
    }
    pub fn new(c: &Arc<PveConfig>) -> Self {
        let profiles = if c.profiles.is_empty() {
            c.build_profiles()
        } else {
            c.profiles.clone()
        };
        let mut monsters: Vec<_> = c
            .monster_positions
            .iter()
            .enumerate()
            .map(|(i, &position)| {
                Monster::new(
                    format!("{}-{}", c.template_id, i + 1),
                    c.template_id.clone(),
                    profiles[&c.template_id].clone(),
                    position,
                    None,
                )
            })
            .collect();
        for spawn in &c.extra_spawns {
            monsters.push(Monster::new(
                spawn.id.clone(),
                spawn.template_id.clone(),
                profiles[&spawn.template_id].clone(),
                spawn.position,
                None,
            ));
        }
        let mut chests = vec![];
        for spawn in &c.chest_spawns {
            let mut chest = chests::Chest::new(spawn);
            if let Some(g) = c.guardian() {
                for i in 0..g.max_count {
                    let mut m = Monster::new(
                        format!("guardian-{}-g1-{}", spawn.id, i + 1),
                        g.template_id.clone(),
                        profiles[&g.template_id].clone(),
                        spawn.position,
                        Some(GuardianBinding {
                            chest_id: spawn.id.clone(),
                            generation: 1,
                        }),
                    );
                    m.enabled = false;
                    m.hp = 0;
                    chest.guardian_slots.push(monsters.len());
                    monsters.push(m);
                }
            }
            chests.push(chest);
        }
        let encounters = encounters::reserve(c, &profiles, &mut monsters);
        let timed_position = timed_position::TimedPositionState::new(c, &monsters);
        Self {
            piercing_projectiles: vec![],
            player_areas: vec![],
            player_fields: vec![],
            statuses: crate::status::Statuses::default(),
            timed_position,
            counter: None,
            support_interrupt_ready: 0,
            attuned: false,
            attune_finishes: None,
            attune_interruption: None,
            score_tally: BTreeMap::new(),
            encounters,
            chests,
            grid: Grid::new(c),
            monsters,
            items: BTreeMap::new(),
            resource_items: BTreeMap::new(),
            shots: vec![],
            locked: None,
            marks: 0,
            mark_sources: vec![],
            marks_until: 0,
            player_path: vec![],
            resource_expiry: BTreeMap::new(),
            now: 0,
            next_sweep: c.prepared.as_ref().unwrap().runtime.resource_sweep_ms,
        }
    }
    fn id(&self, i: usize) -> String {
        self.monsters[i].id.clone()
    }
    fn nearest(&self, p: [f64; 2], range: f64) -> Option<usize> {
        self.monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| m.hp > 0 && distance(m.position, p) <= range)
            .min_by(|(ia, a), (ib, b)| {
                distance(a.position, p)
                    .total_cmp(&distance(b.position, p))
                    .then(ia.cmp(ib))
            })
            .map(|(i, _)| i)
    }
    fn inventory(&self, w: &World, c: &Config) -> BTreeMap<String, u32> {
        let mut result = self.items.clone();
        result.insert(c.item_id.clone(), w.inventory);
        result
    }
    fn limit(&self, id: &str, c: &Config) -> u32 {
        if id == c.item_id {
            c.stack_limit
        } else {
            self.grid.config.items[id].stack_limit
        }
    }
    fn step(&mut self, w: &mut World, c: &Config, now: u64, events: &mut Vec<Event>) {
        // Ground-loot expiry (and status/cancel bookkeeping below) is stamped from
        // this clock; it must track the world clock or drops die the moment they land.
        self.now = now;
        self.prepare_statuses(w, now, events);
        self.advance_periodic(w, c, now, events);
        if w.hp == 0 || w.desired_outcome.is_some() {
            return;
        }
        self.refresh_chests(now, events);
        self.prepare_guardians(w, now, events);
        if now >= self.next_sweep {
            self.next_sweep = now + c.params.runtime.resource_sweep_ms;
            w.resources.retain(|r| {
                self.resource_expiry
                    .get(&r.view.id)
                    .is_none_or(|expiry| *expiry > now)
            });
        }
        self.resource_items
            .retain(|id, _| w.resources.iter().any(|r| &r.view.id == id));
        self.resource_expiry
            .retain(|id, _| w.resources.iter().any(|r| &r.view.id == id));
        let dt = c.step_ms as f64 / 1000.0;
        if now >= self.marks_until {
            self.marks = 0;
            self.mark_sources.clear();
        }
        if !w.protected {
            let navigation_goal = self.player_path.first().copied();
            if let Some(goal) = navigation_goal {
                w.movement = normalize([goal[0] - w.position[0], goal[1] - w.position[1]]);
            }
            let budget =
                c.params.movement.speed(w.facts.spd_points) * self.grid.speed(w.position) * dt;
            let step =
                navigation_goal.map_or(budget, |goal| budget.min(distance(w.position, goal)));
            let end = [
                w.position[0] + w.movement[0] * step,
                w.position[1] + w.movement[1] * step,
            ];
            if self.grid.line(w.position, end, c.params.player_radius, 0)
                && self.monsters.iter().all(|m| {
                    m.hp == 0
                        || distance(end, m.position)
                            >= c.params.player_radius + m.profile.combat.radius
                })
            {
                w.position = end;
                if navigation_goal.is_some_and(|goal| distance(end, goal) < 1e-6) {
                    self.player_path.remove(0);
                    if self.player_path.is_empty() {
                        w.movement = [0.0, 0.0];
                    }
                }
            }
        }
        if w.extraction_end.is_some()
            && distance(w.position, c.extraction_point) > c.params.extraction_range
        {
            w.interrupt_extraction(events);
        }
        self.advance_encounter_resets(w, now, events);
        // A new round invalidates an uncommitted mechanism-26 action. Already
        // launched projectiles retain their original per-target round stamp.
        if c.mechanism_version >= 26
            && w.pending
                .as_ref()
                .is_some_and(|a| a.rounds != self.round_stamp())
        {
            w.interrupt(events);
        }
        if self
            .player_areas
            .iter()
            .any(|p| p.rounds != self.round_stamp())
            || self
                .player_fields
                .iter()
                .any(|p| p.area.rounds != self.round_stamp())
        {
            self.cancel_player_areas(PlayerAreaCancelReason::EncounterReset, events);
        }
        self.prepare_encounters(w, now, events);
        for i in 0..self.monsters.len() {
            let profile = self.monsters[i].profile.clone();
            let monster_id = self.id(i);
            let max_hp = profile.stats["hp"];
            if self.monsters[i].hp == 0 {
                continue;
            }
            let separation = distance(self.monsters[i].position, w.position);
            let group_managed = self.group_managed(i);
            if !group_managed
                && self.monsters[i].aggro
                && (distance(self.monsters[i].home, w.position) > profile.leash
                    || distance(self.monsters[i].position, self.monsters[i].home) > profile.leash
                    || separation > profile.memory_range)
            {
                self.cancel_supports_for(i, SupportCancelReason::Returning, now, events);
                self.cancel_area(i, AreaCancelReason::Returning, now, events);
                self.cancel_heavy_for(i, HeavyCancelReason::Returning, now, events);
                let m = &mut self.monsters[i];
                m.returning = true;
                m.aggro = false;
                m.casting = None;
                m.heal = None;
            }
            if !w.protected
                && !self.monsters[i].returning
                && !self.monsters[i].aggro
                && separation <= profile.combat.aggro_range
                && self
                    .grid
                    .line(self.monsters[i].position, w.position, 0.0, 2)
            {
                self.monsters[i].aggro = true;
                self.monsters[i].next_attack = now + self.monsters[i].attack_interval();
            }
            if !group_managed
                && self.monsters[i].returning
                && distance(self.monsters[i].position, self.monsters[i].home)
                    <= profile.combat.return_distance
            {
                let m = &mut self.monsters[i];
                m.returning = false;
                m.hp = max_hp;
                m.aggro = false;
                m.path.clear();
                self.reset_boss(i, now, events);
            }
            let area_finished = self.advance_area(w, c, i, now, events);
            if w.hp == 0 {
                return;
            }
            if let Some((due, tick, budget)) = self.monsters[i].heal
                && now >= due
            {
                let share = budget / profile.combat.heal.ticks
                    + u32::from(tick < budget % profile.combat.heal.ticks);
                let target = self.status_entity(i);
                let amount = self.resolve_heal(
                    w,
                    c,
                    (&target, &target),
                    (HealKind::SelfChannel, share),
                    now,
                    events,
                );
                let m = &mut self.monsters[i];
                m.hp += amount;
                events.push(Event::MonsterHealed {
                    monster_id: monster_id.clone(),
                    amount,
                });
                m.heal = if tick + 1 == profile.combat.heal.ticks {
                    m.recovery = now + profile.combat.heal.recovery_ms;
                    None
                } else {
                    Some((due + profile.combat.heal.interval_ms, tick + 1, budget))
                };
            }
            if let Some((slot, due)) = self.monsters[i].casting
                && now >= due
            {
                let source = self.status_entity(i);
                let self_statuses = if slot == 0 {
                    &profile.combat.projectile.on_commit_self_statuses[..]
                } else {
                    &[]
                };
                let allowed = !self.monsters[i].returning
                    && self.monsters[i].spirit
                        >= if slot == 0 {
                            profile.combat.projectile.cost
                        } else {
                            profile.combat.heal.cost
                        };
                let status_allowed =
                    !allowed || self.commit_statuses(w, self_statuses, &source, now, events);
                let attack = self.monster_attack(i, now);
                let m = &mut self.monsters[i];
                m.casting = None;
                let cost = if slot == 0 {
                    profile.combat.projectile.cost
                } else {
                    profile.combat.heal.cost
                };
                if allowed && status_allowed {
                    m.spirit -= cost;
                    m.cooldown[slot] = now
                        + if slot == 0 {
                            profile.combat.projectile.cooldown_ms
                        } else {
                            profile.combat.heal.cooldown_ms
                        };
                    events.push(Event::MonsterSkill {
                        monster_id: monster_id.clone(),
                        skill_id: if slot == 0 {
                            profile.combat.projectile.id.as_str()
                        } else {
                            profile.combat.heal.id.as_str()
                        }
                        .into(),
                    });
                    if slot == 0 {
                        m.recovery = now + profile.combat.projectile.recovery_ms;
                        self.shots.push(Shot {
                            strict_target: None,
                            status_source: Some(source),
                            hit_statuses: profile.combat.projectile.on_hit_target_statuses.clone(),
                            manual_started: None,
                            rounds: vec![],
                            position: m.position,
                            direction: normalize([
                                w.position[0] - m.position[0],
                                w.position[1] - m.position[1],
                            ]),
                            remaining: profile.combat.projectile.range,
                            speed: profile.combat.projectile.speed,
                            radius: profile.combat.projectile.radius,
                            raw: (u64::from(attack)
                                * u64::from(profile.combat.projectile.damage_bps)
                                / 10000) as u32,
                            owner: Some(i),
                            metal: true,
                            mark: true,
                            expires: now + profile.combat.projectile.ttl_ms,
                            created: now,
                        });
                    } else {
                        m.heal = Some((
                            now + profile.combat.heal.interval_ms,
                            0,
                            (u64::from(max_hp) * u64::from(profile.combat.heal.heal_bps) / 10000)
                                as u32,
                        ));
                    }
                }
            }
            if !w.protected
                && self.monsters[i].aggro
                && !self.monsters[i].returning
                && self.monsters[i].heavy_cast.is_none()
                && self.monsters[i].support_cast.is_none()
                && self.monsters[i].area_cast.is_none()
                && !area_finished
                && self.monsters[i].casting.is_none()
                && self.monsters[i].heal.is_none()
                && now >= self.monsters[i].recovery
            {
                if now >= self.monsters[i].next_decision {
                    self.monsters[i].next_decision = now + c.params.runtime.ai_decision_ms;
                    if !self.try_start_support(w, c, i, now, events)
                        && !self.try_start_heavy(w, c, i, now, events)
                        && !self.try_start_area(w, c, i, now, events)
                    {
                        for slot in 0..2 {
                            let m = &self.monsters[i];
                            let eligible = now >= m.cooldown[slot]
                                && m.spirit
                                    >= if slot == 0 {
                                        profile.combat.projectile.cost
                                    } else {
                                        profile.combat.heal.cost
                                    }
                                && if slot == 0 {
                                    separation <= profile.combat.projectile.range
                                } else {
                                    f64::from(m.hp)
                                        < f64::from(max_hp) * profile.combat.heal.hp_threshold
                                };
                            if eligible
                                && w.random() % 10000
                                    < (self.monsters[i].cast_chance() * 10000.0) as u32
                            {
                                self.monsters[i].casting = Some((
                                    slot,
                                    now + if slot == 0 {
                                        profile.combat.projectile.windup_ms
                                    } else {
                                        profile.combat.heal.windup_ms
                                    },
                                ));
                                break;
                            }
                        }
                    }
                }
                if self.monsters[i].casting.is_none()
                    && self.monsters[i].heavy_cast.is_none()
                    && self.monsters[i].support_cast.is_none()
                    && self.monsters[i].area_cast.is_none()
                    && separation <= profile.combat.melee_range
                    && now >= self.monsters[i].next_attack
                    && self
                        .grid
                        .line(self.monsters[i].position, w.position, 0.0, 2)
                {
                    self.monsters[i].next_attack = now + self.monsters[i].attack_interval();
                    self.hurt_player(w, c, i, (self.monster_attack(i, now), false), now, events);
                    if w.hp == 0 {
                        return;
                    }
                }
            }
            let can_move = !area_finished
                && self.monsters[i].heavy_cast.is_none()
                && self.monsters[i].support_cast.is_none()
                && self.monsters[i].area_cast.is_none()
                && self.monsters[i].heal.is_none()
                && self.monsters[i].casting.is_none_or(|(slot, _)| slot != 1);
            let goal = if self.monsters[i].returning {
                Some(self.monsters[i].home)
            } else if !w.protected
                && self.monsters[i].aggro
                && separation > profile.combat.melee_range
            {
                Some(w.position)
            } else {
                None
            };
            if can_move && let Some(goal) = goal {
                if now >= self.monsters[i].repath {
                    self.monsters[i].path =
                        self.grid
                            .route(self.monsters[i].position, goal, profile.combat.radius);
                    self.monsters[i].repath = now + c.params.runtime.nav_repath_ms;
                }
                if let Some(&point) = self.monsters[i].path.first() {
                    let m = &self.monsters[i];
                    let d = distance(m.position, point);
                    let step = (c.params.movement.speed(profile.stats["spd"])
                        * self.grid.speed(m.position)
                        * dt)
                        .min(d);
                    let dir = normalize([point[0] - m.position[0], point[1] - m.position[1]]);
                    let end = [m.position[0] + dir[0] * step, m.position[1] + dir[1] * step];
                    if self.grid.line(m.position, end, profile.combat.radius, 0)
                        && distance(end, w.position)
                            >= c.params.player_radius + profile.combat.radius
                        && self.monsters.iter().enumerate().all(|(j, m)| {
                            i == j
                                || m.hp == 0
                                || distance(end, m.position)
                                    >= profile.combat.radius + m.profile.combat.radius
                        })
                    {
                        self.monsters[i].position = end;
                        if d <= step + 0.01 {
                            self.monsters[i].path.remove(0);
                        }
                    }
                }
            }
        }
        if self
            .piercing_projectiles
            .iter()
            .any(|p| p.rounds != self.round_stamp())
        {
            self.cancel_piercing(PiercingEndReason::EncounterReset, events);
        }
        if w.pending.as_ref().is_some_and(|a| a.due_ms <= now) {
            let a = w.pending.take().unwrap();
            let chosen = a.selected_skill.as_ref().unwrap_or(&c.skill);
            let target = self.locked.filter(|&i| {
                self.monsters[i].hp > 0
                    && distance(w.position, self.monsters[i].position) <= c.basic.range
            });
            let unit_aim = a
                .unit_target
                .as_ref()
                .map(|t| self.unit_aim(w, chosen, t))
                .transpose();
            let ground_check = a
                .ground_position
                .map(|p| self.ground_target(w, chosen, p))
                .transpose();
            let line_check = (a.skill && chosen.is_line())
                .then(|| self.line_plan(w, chosen, a.direction))
                .transpose();
            if let Err(reason) = ground_check.and(line_check.as_ref().map(|_| ()).map_err(|e| *e)) {
                events.push(Event::ActionRejected {
                    action_id: a.id,
                    reason,
                });
            } else if a.skill
                && ((chosen.is_piercing() && self.piercing_projectiles.len() >= 2)
                    || ((chosen.is_ground() || chosen.is_line()) && self.ground_plan_count() >= 2))
            {
                events.push(Event::ActionRejected {
                    action_id: a.id,
                    reason: Error::Capacity,
                });
            } else if let Err(reason) = unit_aim {
                events.push(Event::ActionRejected {
                    action_id: a.id,
                    reason,
                });
            } else if c.mechanism_version >= 26 && a.skill && w.spirit < chosen.cost {
                events.push(Event::ActionRejected {
                    action_id: a.id,
                    reason: Error::NoResource,
                });
            } else if a.skill
                && !self.commit_statuses(
                    w,
                    &chosen.on_commit_self_statuses,
                    &status::participant(),
                    now,
                    events,
                )
            {
                events.push(Event::ActionRejected {
                    action_id: a.id,
                    reason: Error::Capacity,
                });
            } else if !a.skill && target.is_none() {
                events.push(Event::ActionInterrupted);
            } else {
                if a.skill {
                    w.spirit -= chosen.cost;
                    w.set_skill_ready(c, &chosen.id, now + chosen.cooldown_ms);
                    w.recovery_until = now + chosen.recovery_ms;
                }
                if (chosen.is_ground() || chosen.is_line()) && a.skill {
                    let warning = PlayerAreaWarningView {
                        action_id: a.id.clone(),
                        skill_id: chosen.id.clone(),
                        center: a.ground_position.unwrap_or(w.position),
                        radius: chosen.radius,
                        started_at_ms: now,
                        resolves_at_ms: now + chosen.warning_ms,
                    };
                    let area = ground::PlayerArea {
                        line: line_check.unwrap(),
                        warning: warning.clone(),
                        rounds: a.rounds.clone(),
                        raw: (u64::from(self.player_attack(w, now)) * u64::from(chosen.damage_bps)
                            / 10000) as u32,
                        accepted_at: a.accepted_at,
                        hit_statuses: chosen.on_hit_target_statuses.clone(),
                    };
                    if chosen.is_field() {
                        let mut f = field::PlayerField::new(
                            area,
                            chosen.duration_ms,
                            chosen.tick_interval_ms,
                        );
                        f.contact = chosen
                            .contact_policy
                            .clone()
                            .map(crate::contact::ContactBudget::new);
                        events.push(Event::PlayerFieldStarted { field: f.view() });
                        self.player_fields.push(f);
                    } else {
                        self.player_areas.push(area);
                        events.push(Event::PlayerAreaStarted { warning });
                    }
                } else if a.skill && chosen.is_piercing() {
                    let p = piercing::PiercingProjectile::new(
                        &a,
                        chosen,
                        w.position,
                        self.player_attack(w, now),
                        now,
                    );
                    events.push(Event::PiercingStarted {
                        projectile: p.view(),
                    });
                    self.piercing_projectiles.push(p);
                } else if !a.skill || !chosen.is_self() {
                    let (dir, speed, range, radius, raw) = if a.skill {
                        (
                            unit_aim.unwrap().unwrap_or(a.direction),
                            chosen.speed,
                            chosen.range,
                            chosen.radius,
                            (u64::from(self.player_attack(w, now)) * u64::from(chosen.damage_bps)
                                / 10000) as u32,
                        )
                    } else {
                        let p = self.monsters[target.unwrap()].position;
                        (
                            normalize([p[0] - w.position[0], p[1] - w.position[1]]),
                            c.basic.projectile_speed,
                            c.basic.range,
                            c.params.basic_projectile_radius,
                            self.player_attack(w, now),
                        )
                    };
                    self.shots.push(Shot {
                        strict_target: a.unit_target.clone(),
                        status_source: a.skill.then(status::participant),
                        hit_statuses: if a.skill {
                            chosen.on_hit_target_statuses.clone()
                        } else {
                            vec![]
                        },
                        manual_started: Some(a.accepted_at),
                        rounds: a.rounds,
                        position: w.position,
                        direction: dir,
                        remaining: range,
                        speed,
                        radius,
                        raw,
                        owner: None,
                        metal: a.skill,
                        mark: false,
                        expires: now + c.params.player_projectile_ttl_ms,
                        created: now,
                    });
                }
                events.push(Event::ActionCommitted {
                    ground_position: a.ground_position,
                    unit_target: a.unit_target.clone(),
                    skill_id: (c.mechanism_version >= 26 && a.skill).then(|| chosen.id.clone()),
                    action_id: a.id,
                    skill: a.skill,
                    spirit_cost: if a.skill { chosen.cost } else { 0 },
                    hp_cost: None,
                    charge: None,
                });
            }
        }
        let shots = std::mem::take(&mut self.shots);
        for mut shot in shots {
            if shot
                .strict_target
                .as_ref()
                .is_some_and(|t| self.unit_index(t).is_err())
            {
                continue;
            }

            if now >= shot.expires {
                continue;
            }
            if shot.created == now {
                self.shots.push(shot);
                continue;
            }
            let travel = (shot.speed * dt).min(shot.remaining);
            let end = [
                shot.position[0] + shot.direction[0] * travel,
                shot.position[1] + shot.direction[1] * travel,
            ];
            let steps = travel.ceil().max(1.0) as u32;
            let wall = (0..=steps)
                .find(|&j| {
                    let t = f64::from(j) / f64::from(steps);
                    !self.grid.clear(
                        [
                            shot.position[0] + (end[0] - shot.position[0]) * t,
                            shot.position[1] + (end[1] - shot.position[1]) * t,
                        ],
                        shot.radius,
                        1,
                    )
                })
                .map(|j| f64::from(j) / f64::from(steps));
            let hit = if shot.owner.is_some() {
                (!w.protected)
                    .then(|| {
                        segment_circle(
                            shot.position,
                            end,
                            w.position,
                            c.params.player_radius + shot.radius,
                        )
                    })
                    .flatten()
                    .map(|t| (t, usize::MAX))
            } else {
                self.monsters
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| {
                        m.hp > 0
                            && self.accepts_round(m, &shot.rounds)
                            && (shot.strict_target.is_none() || m.enabled)
                    })
                    .filter_map(|(i, m)| {
                        segment_circle(
                            shot.position,
                            end,
                            m.position,
                            m.profile.combat.radius + shot.radius,
                        )
                        .map(|t| (t, i))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            };
            if let Some((t, index)) = hit.filter(|(t, _)| wall.is_none_or(|wall| *t < wall)) {
                let _ = t;
                // A strict unit spell is consumed by intervening units, without retargeting its damage/effects.
                if shot.strict_target.as_ref().is_some_and(|target| {
                    self.monsters[index].id != target.id
                        || self.monsters[index].status_generation != target.generation
                }) {
                    continue;
                }

                if let Some(owner) = shot.owner {
                    self.hurt_player(w, c, owner, (shot.raw, shot.mark), now, events);
                    if let Some(source) = &shot.status_source {
                        self.hit_statuses(
                            w,
                            &shot.hit_statuses,
                            source,
                            &status::participant(),
                            now,
                            events,
                        );
                    }
                } else {
                    let contact_events = events.len();
                    if self.hurt_monster_contact(
                        w,
                        c,
                        index,
                        (shot.raw, shot.metal, shot.manual_started),
                        events,
                    ) > 0
                        && !shot.metal
                    {
                        w.spirit =
                            (w.spirit + c.params.basic_spirit_on_hit).min(w.facts.spirit_points);
                    }
                    // Damage events distinguish a legal zero-damage contact from dodge/immunity.
                    let contacted = events[contact_events..].iter().any(|e| matches!(e, Event::Damage { source, target, .. } if source=="participant" && target==&self.monsters[index].id));
                    if contacted && let Some(source) = &shot.status_source {
                        self.hit_statuses(
                            w,
                            &shot.hit_statuses,
                            source,
                            &self.status_entity(index),
                            now,
                            events,
                        );
                    }
                }
            } else if wall.is_none() && shot.remaining > travel {
                shot.position = end;
                shot.remaining -= travel;
                self.shots.push(shot);
            }
            if w.hp == 0 {
                return;
            }
        }
        self.advance_piercing(w, c, now, dt, events);
        if w.hp == 0 {
            return;
        }
        self.advance_player_areas(w, c, now, events);
        self.advance_player_fields(w, c, now, events);
        self.advance_heavies(w, c, now, events);
        if w.hp == 0 {
            return;
        }
        self.advance_supports(w, c, now, events);
        if now >= w.next_regen {
            w.next_regen += c.params.regen.interval_ms;
            for m in &mut self.monsters {
                if m.hp > 0
                    && m.encounter_index
                        .is_none_or(|i| self.encounters[i].reset_at.is_none())
                {
                    let max_hp = m.profile.stats["hp"];
                    let ratio = if distance(m.position, w.position) <= c.params.regen.near_radius {
                        c.params.regen.near_ratio
                    } else {
                        c.params.regen.far_ratio
                    };
                    m.hp = (m.hp + (f64::from(max_hp) * ratio).floor() as u32).min(max_hp);
                    let max_mp = m.profile.stats["spirit"];
                    m.spirit = (m.spirit + (f64::from(max_mp) * ratio).floor() as u32).min(max_mp);
                }
            }
            let ratio = if self
                .nearest(w.position, c.params.regen.near_radius)
                .is_some()
            {
                c.params.regen.near_ratio
            } else {
                c.params.regen.far_ratio
            };
            w.hp = (w.hp + (f64::from(w.facts.hp_points) * ratio).floor() as u32)
                .min(w.facts.hp_points);
            w.spirit = (w.spirit + (f64::from(w.facts.spirit_points) * ratio).floor() as u32)
                .min(w.facts.spirit_points);
        }
        self.advance_encounters(w, events);
        self.advance_timed_position(w, c, now, events);
        if w.hp == 0 {
            return;
        }
        self.advance_attunement(w, c, now, events);
        self.advance_unlock(w, now, events);
        if w.extraction_end.is_some_and(|end| now >= end) {
            self.clear_statuses(events);
            self.cancel_timed_position(events);
            self.cancel_all_reset_countdowns(events);
            self.cancel_all_heavies(HeavyCancelReason::Closed, now, events);
            self.cancel_player_areas(
                if w.hp == 0 {
                    PlayerAreaCancelReason::OwnerDied
                } else {
                    PlayerAreaCancelReason::Terminal
                },
                events,
            );
            self.cancel_all_areas(AreaCancelReason::Closed, now, events);
            self.cancel_all_supports(SupportCancelReason::Closed, now, events);
            w.desired_outcome = Some(Outcome::Extraction);
        }
    }
    fn hurt_player(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        hit: (u32, bool),
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let id = self.monsters[i].id.clone();
        self.apply_player_damage(w, c, (&id, Some(i)), (hit.0, hit.1, false), now, events);
    }
    fn apply_player_damage(
        &mut self,
        w: &mut World,
        c: &Config,
        origin: (&str, Option<usize>),
        hit: (u32, bool, bool),
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let (source, index) = origin;
        let profile = index.map(|i| self.monsters[i].profile.clone());
        if w.protected || w.hp == 0 {
            return;
        }
        let (raw, mark, periodic) = hit;
        let defense = f64::from(self.player_defense(w, now));
        let crit = !periodic && w.random() % 100 < profile.as_ref().unwrap().stats["crit"];
        let amount = ((f64::from(raw)
            * (1.0
                - (defense / (defense + c.params.defense_scale * f64::from(c.defender_realm_atk)))
                    .min(c.defense_cap))
            * if crit {
                c.params.critical_multiplier
            } else {
                1.0
            })
        .floor() as u32)
            .max(c.params.minimum_damage);
        let amount = self
            .statuses
            .absorb(&status::participant(), amount, now, source, events)
            .min(w.hp);
        w.hp -= amount;
        if amount > 0 {
            self.interrupt_unlock(ChestInterruptReason::Damage, events);
        }
        events.push(Event::Damage {
            source: source.into(),
            target: "participant".into(),
            amount,
        });
        if !periodic || amount > 0 {
            w.interrupt_extraction(events);
        }
        if !periodic
            && let Some(i) = index
            && self.monsters[i].hp > 0
        {
            let profile = profile.as_ref().unwrap();
            self.monsters[i].hp = (self.monsters[i].hp
                + (f64::from(amount) * profile.combat.lifesteal_ratio).floor() as u32)
                .min(profile.stats["hp"]);
        }
        if !periodic
            && mark
            && w.hp > 0
            && let Some(i) = index
            && let Some(profile) = &profile
            && profile.combat.mark_max > 0
        {
            self.marks = (self.marks + 1).min(profile.combat.mark_max);
            self.mark_sources.push(i);
            if self.mark_sources.len() > self.marks as usize {
                self.mark_sources
                    .drain(..self.mark_sources.len() - self.marks as usize);
            }
            self.marks_until = now + profile.combat.mark_duration_ms;
        }
        if w.hp == 0 {
            self.clear_status_entity(
                &status::participant(),
                StatusRemovalReason::TargetInvalid,
                events,
            );
            self.cancel_timed_position(events);
            self.cancel_all_reset_countdowns(events);
            self.cancel_all_heavies(HeavyCancelReason::Closed, now, events);
            self.cancel_player_areas(
                if w.hp == 0 {
                    PlayerAreaCancelReason::OwnerDied
                } else {
                    PlayerAreaCancelReason::Terminal
                },
                events,
            );
            self.cancel_all_areas(AreaCancelReason::Closed, now, events);
            self.cancel_all_supports(SupportCancelReason::Closed, now, events);
            w.die(events);
        }
    }
    #[cfg(all(test, match_sdk_test_data))]
    fn hurt_monster(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        raw: u32,
        metal: bool,
        events: &mut Vec<Event>,
    ) -> u32 {
        self.hurt_monster_contact(w, c, i, (raw, metal, None), events)
    }
    fn hurt_monster_contact(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        hit: (u32, bool, Option<u64>),
        events: &mut Vec<Event>,
    ) -> u32 {
        let (raw, metal, manual_started) = hit;
        if self.monsters[i].hp == 0 || !self.monsters[i].enabled {
            return 0;
        }
        let profile = self.monsters[i].profile.clone();
        if self.monsters[i].returning
            && distance(self.monsters[i].position, self.monsters[i].home) > profile.leash
        {
            return 0;
        }
        if w.random() % 100 < profile.stats["dodge"] {
            return 0;
        }
        let counter = self.take_counter(i, manual_started, self.now);
        let timed_hit = self.take_timed_hit(w, i, manual_started);
        let amount =
            self.apply_monster_damage(w, c, i, (raw, metal, manual_started, "participant"), events);
        if let Some(contact) = timed_hit {
            self.apply_timed_hit(w, c, i, contact, events);
        }
        if let Some(window) = counter {
            let damage = if self.monsters[i].hp > 0 {
                self.apply_monster_damage(
                    w,
                    c,
                    i,
                    (window.raw_damage, false, None, "counter"),
                    events,
                )
            } else {
                0
            };
            events.push(Event::CounterTriggered {
                cast: HeavyCastIdentity {
                    monster_id: window.monster_id,
                    cast_id: window.cast_id,
                },
                damage,
            });
        }
        amount
    }
    fn apply_monster_damage(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        hit: (u32, bool, Option<u64>, &'static str),
        events: &mut Vec<Event>,
    ) -> u32 {
        let (raw, metal, manual_started, source) = hit;
        let profile = self.monsters[i].profile.clone();
        let armor = f64::from(self.monster_defense(i, self.now));
        let amount = (f64::from(raw)
            * if metal {
                c.params.metal_multiplier
            } else {
                1.0
            }
            * (1.0
                - (armor / (armor + c.params.defense_scale * f64::from(c.defender_realm_atk)))
                    .min(c.defense_cap)))
        .floor()
        .max(f64::from(c.params.minimum_damage)) as u32;
        let amount = self
            .statuses
            .absorb(&self.status_entity(i), amount, self.now, source, events)
            .min(self.monsters[i].hp);
        self.monsters[i].hp -= amount;
        self.monsters[i].returning = false;
        self.monsters[i].aggro = true;
        let home = self.monsters[i].position;
        for m in &mut self.monsters {
            if m.hp > 0 && distance(m.position, home) <= profile.help_range {
                m.aggro = true;
            }
        }
        events.push(Event::Damage {
            source: source.into(),
            target: self.monsters[i].id.clone(),
            amount,
        });
        if self.monsters[i].hp > 0 {
            self.interrupt_support_on_contact(i, manual_started, self.now, events);
        }
        if amount > 0 && self.monsters[i].hp > 0 {
            self.advance_boss(i, events);
        }
        if self.monsters[i].hp == 0 {
            self.clear_status_entity(
                &self.status_entity(i),
                StatusRemovalReason::TargetInvalid,
                events,
            );
            self.cancel_supports_for(i, SupportCancelReason::MemberDied, self.now, events);
            self.cancel_area(i, AreaCancelReason::OwnerDied, self.now, events);
            self.cancel_heavy_for(i, HeavyCancelReason::OwnerDied, self.now, events);
            self.monsters[i].casting = None;
            self.monsters[i].heal = None;
            let first = self.record_defeat(i);
            if first && profile.kill_credit {
                w.kills += 1;
            }
            if c.scoring_enabled() {
                if first {
                    *self
                        .score_tally
                        .entry(self.monsters[i].template_id.clone())
                        .or_default() += 1;
                }
                let summary = c.score_summary(&self.score_tally);
                events.push(Event::KillScored {
                    monster_id: self.monsters[i].id.clone(),
                    template_id: self.monsters[i].template_id.clone(),
                    score: if first { profile.effective_score(c) } else { 0 },
                    total_score: summary.kill_score,
                    kills: self.actual_kills(c),
                });
            }
            events.push(Event::TargetKilled);
            events.push(Event::MonsterKilled {
                monster_id: self.monsters[i].id.clone(),
            });
            self.drop_reward(w, c, i, first);
        }
        amount
    }
    #[cfg(all(test, match_sdk_test_data))]
    fn drop(&mut self, w: &mut World, c: &Config, i: usize) {
        self.drop_reward(w, c, i, true);
    }
    fn drop_reward(&mut self, w: &mut World, c: &Config, i: usize, first: bool) {
        let table = &self.monsters[i].profile.drop_table;
        let rules = table
            .guaranteed
            .iter()
            .map(|r| (r.clone(), true))
            .chain(table.random.iter().map(|r| (r.clone(), false)))
            .collect::<Vec<_>>();
        let origin = self.monsters[i].position;
        for (n, (rule, guaranteed)) in rules.iter().enumerate().filter(|_| first) {
            if !*guaranteed
                && (rule.chance == Some(0.0)
                    || (rule.chance.unwrap() < 1.0
                        && f64::from(w.random()) / (f64::from(u32::MAX) + 1.0)
                            >= rule.chance.unwrap()))
            {
                continue;
            }
            let lo = rule.quantity_min;
            let hi = rule.quantity_max;
            let quantity = lo + w.random() % (hi - lo + 1);
            if quantity == 0 {
                continue;
            }
            let angle = f64::from(w.random()) / f64::from(u32::MAX) * std::f64::consts::TAU;
            let r = f64::from(
                c.params.drop_min_radius
                    + w.random() % (c.params.drop_max_radius - c.params.drop_min_radius + 1),
            );
            let candidate = [origin[0] + angle.cos() * r, origin[1] + angle.sin() * r];
            let position = if self.grid.line(origin, candidate, c.params.player_radius, 0) {
                candidate
            } else {
                origin
            };
            let id = if i < self.grid.config.monster_positions.len() {
                format!("wolf-{}-drop-{n}", i + 1)
            } else {
                format!("monster-{}-drop-{n}", self.id(i))
            };
            self.resource_expiry
                .insert(id.clone(), self.now + c.params.resource_ttl_ms);
            self.resource_items.insert(id.clone(), rule.item_id.clone());
            w.resources.push(Resource {
                view: ResourceView {
                    id,
                    position,
                    quantity,
                    orb: false,
                },
                hp: 0,
                spirit: 0,
            });
        }
        if !self.monsters[i].profile.drop_orb {
            return;
        }
        let orb_id = if self.monsters[i]
            .encounter_index
            .is_some_and(|index| self.grid.config.encounters[index].reset.is_some())
        {
            format!(
                "monster-{}-r{}-orb",
                self.id(i),
                self.encounters[self.monsters[i].encounter_index.unwrap()].round
            )
        } else if i < self.grid.config.monster_positions.len() {
            format!("wolf-{}-orb", i + 1)
        } else {
            format!("monster-{}-orb", self.id(i))
        };
        self.resource_expiry
            .insert(orb_id.clone(), self.now + c.params.resource_ttl_ms);
        w.resources.push(Resource {
            view: ResourceView {
                id: orb_id,
                position: origin,
                quantity: 0,
                orb: true,
            },
            hp: (f64::from(w.facts.hp_points) * c.params.orb_hp_ratio).floor() as u32,
            spirit: (f64::from(w.facts.spirit_points) * c.params.orb_spirit_ratio).floor() as u32,
        });
        let _ = c;
    }
}
impl World {
    pub(super) fn pve_action(
        &mut self,
        action: &Action,
        id: &str,
        c: &Config,
        now: u64,
    ) -> Result<Vec<Event>> {
        let mut arena = self.pve.take().unwrap();
        let mut result = (|| {
            let mut events = vec![];
            if arena.has_unlock()
                && matches!(
                    action,
                    Action::Attack {}
                        | Action::Cast { .. }
                        | Action::CastSelf { .. }
                        | Action::CastGround { .. }
                        | Action::CastUnit { .. }
                        | Action::Pickup { .. }
                        | Action::ExtractStart {}
                )
            {
                return Err(Error::InvalidState);
            }
            match action {
                Action::AttuneObjective {
                    objective_id,
                    expected_revision,
                } => {
                    if self.pending.is_some() || self.extraction_end.is_some() {
                        return Err(Error::InvalidState);
                    }
                    arena.start_attunement(
                        self,
                        c,
                        objective_id,
                        *expected_revision,
                        now,
                        &mut events,
                    )?;
                }
                Action::CancelAttunement {} => {
                    if arena.attune_finishes.is_none() {
                        return Err(Error::InvalidState);
                    }
                    arena.interrupt_unlock(ChestInterruptReason::Cancelled, &mut events);
                }
                Action::CancelChestUnlock {
                    chest_id,
                    expected_generation,
                } => {
                    arena.cancel_unlock(chest_id, *expected_generation, &mut events)?;
                }
                Action::OpenChest {
                    chest_id,
                    expected_generation,
                } => {
                    if arena.attune_finishes.is_some()
                        || self.pending.is_some()
                        || self.extraction_end.is_some()
                    {
                        return Err(Error::InvalidState);
                    }
                    arena.open_chest(self, chest_id, *expected_generation, now, &mut events)?;
                }
                Action::Move { direction } => {
                    arena.interrupt_unlock(ChestInterruptReason::Movement, &mut events);
                    arena.player_path.clear();
                    self.movement = normalize(*direction);
                    if c.mechanism_version < 26 {
                        self.recovery_until = now;
                    }
                }
                Action::Stop {} => {
                    arena.player_path.clear();
                    self.movement = [0.0, 0.0]
                }
                Action::Navigate { destination } => {
                    if !destination.iter().all(|v| v.is_finite())
                        || !arena.grid.clear(*destination, c.params.player_radius, 0)
                    {
                        return Err(Error::OutOfRange);
                    }
                    // New capability avoids intermediate live actors. A goal inside a target
                    // retains the established approach-until-collision semantics.
                    let obstacles: Vec<_> = if c.mechanism_version >= 10 {
                        arena
                            .monsters
                            .iter()
                            .filter(|m| {
                                m.enabled
                                    && m.hp > 0
                                    && distance(*destination, m.position)
                                        >= c.params.player_radius + m.profile.combat.radius
                            })
                            .map(|m| (m.position, m.profile.combat.radius))
                            .collect()
                    } else {
                        vec![]
                    };
                    let mut path = arena.grid.route_avoiding(
                        self.position,
                        *destination,
                        c.params.player_radius,
                        &obstacles,
                    );
                    if path.is_empty() && !obstacles.is_empty() {
                        // Moving targets can leave the sampled destination behind a crowd.
                        // Retain the prior approach path; movement still enforces all footprints.
                        path =
                            arena
                                .grid
                                .route(self.position, *destination, c.params.player_radius);
                    }
                    if path.is_empty() {
                        return Err(Error::OutOfRange);
                    }
                    arena.interrupt_unlock(ChestInterruptReason::Movement, &mut events);
                    arena.player_path = path;
                }
                Action::Attack {}
                | Action::Cast { .. }
                | Action::CastSelf { .. }
                | Action::CastGround { .. }
                | Action::CastUnit { .. } => {
                    if matches!(action, Action::CastGround { .. }) && c.mechanism_version < 29 {
                        return Err(Error::Unsupported);
                    }
                    if matches!(action, Action::CastUnit { .. }) && c.mechanism_version < 28 {
                        return Err(Error::Unsupported);
                    }
                    if matches!(action, Action::CastSelf { .. }) && c.mechanism_version < 27 {
                        return Err(Error::Unsupported);
                    }

                    if self.pending.is_some()
                        || c.mechanism_version >= 26 && now < self.recovery_until
                    {
                        return Err(Error::InvalidState);
                    }
                    let (skill, direction, windup, selected_skill, unit_target, ground_position) =
                        if let Action::Cast { skill_id, .. }
                        | Action::CastSelf { skill_id }
                        | Action::CastUnit { skill_id, .. }
                        | Action::CastGround { skill_id, .. } = action
                        {
                            let selected = c.player_skill(skill_id).ok_or(Error::Unsupported)?;
                            if selected.is_line() && c.mechanism_version < 34 {
                                return Err(Error::Unsupported);
                            }
                            if selected.projectile_repeat_interval_ms.is_some()
                                && c.mechanism_version < 33
                            {
                                return Err(Error::Unsupported);
                            }
                            if selected.is_piercing() && c.mechanism_version < 32 {
                                return Err(Error::InvalidMessage);
                            }
                            if selected.contact_policy.is_some() && c.mechanism_version < 31 {
                                return Err(Error::Unsupported);
                            }
                            if selected.is_field() && c.mechanism_version < 30 {
                                return Err(Error::Unsupported);
                            }
                            let (aim, unit_target, ground_position) = match action {
                                Action::CastSelf { .. } if selected.is_self() => (None, None, None),
                                Action::CastUnit { target, .. } if selected.is_unit() => {
                                    (None, Some(target.clone()), None)
                                }
                                Action::CastGround { position, .. } if selected.is_ground() => {
                                    (None, None, Some(*position))
                                }
                                Action::Cast { direction, .. }
                                    if !selected.is_self()
                                        && !selected.is_unit()
                                        && !selected.is_ground() =>
                                {
                                    (Some(*direction), None, None)
                                }
                                _ => return Err(Error::InvalidMessage),
                            };
                            if c.mechanism_version >= 26
                                && aim.is_some_and(|d| !d.iter().all(|v| v.is_finite()))
                            {
                                return Err(Error::InvalidMessage);
                            }
                            if now < self.skill_ready(c, skill_id) {
                                return Err(Error::InvalidState);
                            }
                            if self.spirit < selected.cost {
                                return Err(Error::NoResource);
                            }
                            if let Some(p) = ground_position {
                                arena.ground_target(self, selected, p)?;
                            }
                            let direction = if let Some(t) = &unit_target {
                                arena.unit_aim(self, selected, t)?
                            } else if let Some(d) = aim {
                                let d = normalize(d);
                                if d == [0., 0.] {
                                    return Err(Error::InvalidMessage);
                                }
                                d
                            } else {
                                [0., 0.]
                            };
                            if selected.is_line() {
                                arena.line_origin(self, selected)?;
                            }
                            (
                                true,
                                direction,
                                selected.windup_ms,
                                Some(selected.clone()),
                                unit_target,
                                ground_position,
                            )
                        } else {
                            if now < self.basic_ready_at {
                                return Err(Error::InvalidState);
                            }
                            arena.locked = arena.nearest(self.position, c.basic.range);
                            if arena.locked.is_none() {
                                return Err(Error::NoResource);
                            }
                            self.basic_ready_at = now + c.basic.interval_ms;
                            (false, [0.0, 0.0], c.basic.windup_ms, None, None, None)
                        };
                    if c.mechanism_version < 26 {
                        self.recovery_until = now;
                    }
                    self.pending = Some(PendingAction {
                        ground_position,
                        unit_target,
                        selected_skill,
                        rounds: arena.round_stamp(),
                        accepted_at: now,
                        id: id.into(),
                        due_ms: now + windup,
                        skill,
                        direction,
                    });
                }
                Action::CancelAction {} => {
                    self.interrupt(&mut events);
                    arena.interrupt_unlock(ChestInterruptReason::Cancelled, &mut events);
                }
                Action::Pickup {
                    resource_id,
                    expected_revision,
                } => {
                    if *expected_revision != self.inventory_revision {
                        return Err(Error::RevisionConflict);
                    }
                    let i = self
                        .resources
                        .iter()
                        .position(|r| r.view.id == *resource_id)
                        .ok_or(Error::NoResource)?;
                    let r = &self.resources[i];
                    if distance(self.position, r.view.position) > c.params.pickup_range
                        || !arena.grid.line(self.position, r.view.position, 0.0, 0)
                    {
                        return Err(Error::OutOfRange);
                    }
                    let quantity = r.view.quantity;
                    if r.view.orb {
                        self.hp = (self.hp + r.hp).min(self.facts.hp_points);
                        self.spirit = (self.spirit + r.spirit).min(self.facts.spirit_points);
                    } else {
                        let item = arena.resource_items[resource_id].clone();
                        let mut bag = arena.inventory(self, c);
                        *bag.entry(item.clone()).or_default() += quantity;
                        if bag
                            .iter()
                            .map(|(id, q)| q.div_ceil(arena.limit(id, c)))
                            .sum::<u32>()
                            > c.inventory_capacity
                        {
                            return Err(Error::Capacity);
                        }
                        if item == c.item_id {
                            self.inventory += quantity;
                            self.looted += quantity;
                        } else {
                            *arena.items.entry(item).or_default() += quantity;
                        }
                        self.inventory_revision += 1;
                    }
                    self.resources.remove(i);
                    arena.resource_items.remove(resource_id);
                    arena.resource_expiry.remove(resource_id);
                    events.push(Event::PickedUp {
                        resource_id: resource_id.clone(),
                        quantity,
                    });
                }
                Action::ExtractStart {} => {
                    if now < self.active_at.unwrap() + c.extraction_unlock_ms
                        || self.extraction_end.is_some()
                    {
                        return Err(Error::InvalidState);
                    }
                    if distance(self.position, c.extraction_point) > c.params.extraction_range {
                        return Err(Error::OutOfRange);
                    }
                    self.extraction_end = Some(now + c.extraction_channel_ms);
                    events.push(Event::ExtractionStarted);
                }
                Action::ExtractCancel {} => self.interrupt_extraction(&mut events),
                Action::Leave {} => {
                    arena.cancel_timed_position(&mut events);
                    arena.cancel_all_reset_countdowns(&mut events);
                    arena.cancel_all_heavies(HeavyCancelReason::Closed, now, &mut events);
                    arena.cancel_player_areas(PlayerAreaCancelReason::Terminal, &mut events);
                    arena.cancel_all_areas(AreaCancelReason::Closed, now, &mut events);
                    arena.cancel_all_supports(SupportCancelReason::Closed, now, &mut events);
                    arena.interrupt_unlock(ChestInterruptReason::Closed, &mut events);
                    self.die(&mut events);
                }
                _ => return Err(Error::InvalidState),
            }
            Ok(events)
        })();
        if self.desired_outcome.is_some()
            && let Ok(events) = &mut result
        {
            arena.clear_statuses(events);
        }
        self.pve = Some(arena);
        result
    }
    pub(super) fn pve_tick(&mut self, c: &Config, now: u64, mut events: Vec<Event>) -> Vec<Event> {
        let mut arena = self.pve.take().unwrap();
        arena.step(self, c, now, &mut events);
        if self.desired_outcome.is_some() {
            arena.cancel_player_areas(PlayerAreaCancelReason::Terminal, &mut events);
            arena.clear_statuses(&mut events);
        }
        self.pve = Some(arena);
        events
    }
    pub(super) fn pve_snapshot(&self, c: &Config, now: u64) -> Snapshot {
        let a = self.pve.as_ref().unwrap();
        let monsters = a
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| m.enabled)
            .map(|(i, m)| {
                let support = a.support_view(m);
                let support_cast = support
                    .as_ref()
                    .filter(|s| s.cast.is_some())
                    .map(|s| s.skill_id.clone());
                MonsterView {
                    unit_target: if c.mechanism_version >= 28 {
                        a.unit_reference(i)
                    } else {
                        None
                    },
                    shield_total: (c.mechanism_version >= 22)
                        .then(|| a.statuses.shield_total(&a.status_entity(i), now)),
                    effective_atk: (c.mechanism_version >= 21).then(|| a.monster_attack(i, now)),
                    healing_received_bps: (c.mechanism_version >= 25).then(|| {
                        a.statuses.healing_received_bps(
                            &a.status_entity(i),
                            now,
                            a.grid.config.status_rules.as_ref(),
                        )
                    }),
                    statuses: a.statuses.views(&a.status_entity(i), now),
                    effective_def: (c.mechanism_version >= 19).then(|| a.monster_defense(i, now)),
                    encounter_round: m
                        .encounter_index
                        .filter(|&i| a.grid.config.encounters[i].reset.is_some())
                        .map(|i| a.encounters[i].round),
                    heavy: m.heavy_view(),
                    support,
                    area_warning: m.area_view(),
                    boss: m.boss_view(),
                    kill_score: c.scoring_enabled().then(|| m.profile.effective_score(c)),
                    encounter_id: m.encounter_id.clone(),
                    wave_id: m.wave_id.clone(),
                    template_id: (c.mechanism_version >= 3).then(|| m.template_id.clone()),
                    radius: (c.mechanism_version >= 3).then_some(m.profile.combat.radius),
                    guardian: m.binding.clone(),
                    id: a.id(i),
                    position: m.position,
                    hp: m.hp,
                    max_hp: m.profile.stats["hp"],
                    spirit: m.spirit,
                    state: if m.hp == 0 {
                        "dead"
                    } else if m.returning {
                        "returning"
                    } else if m.aggro {
                        "combat"
                    } else {
                        "idle"
                    }
                    .into(),
                    cast: support_cast
                        .or_else(|| {
                            m.heavy_cast
                                .as_ref()
                                .map(|_| m.profile.heavy.as_ref().unwrap().id.clone())
                        })
                        .or_else(|| m.area_view().map(|a| a.skill_id))
                        .or_else(|| {
                            m.casting.map(|(slot, _)| {
                                if slot == 0 {
                                    m.profile.combat.projectile.id.as_str()
                                } else {
                                    m.profile.combat.heal.id.as_str()
                                }
                                .into()
                            })
                        }),
                }
            })
            .collect();
        Snapshot {
            shield_total: (c.mechanism_version >= 22)
                .then(|| a.statuses.shield_total(&status::participant(), now)),
            effective_atk: (c.mechanism_version >= 21).then(|| a.player_attack(self, now)),
            healing_received_bps: (c.mechanism_version >= 25).then(|| {
                a.statuses.healing_received_bps(
                    &status::participant(),
                    now,
                    a.grid.config.status_rules.as_ref(),
                )
            }),
            statuses: a.statuses.views(&status::participant(), now),
            effective_def: (c.mechanism_version >= 19).then(|| a.player_defense(self, now)),
            max_hp: self.facts.hp_points,
            max_spirit: self.facts.spirit_points,
            pve: Some(PveView {
                piercing_projectiles: (c.mechanism_version >= 32)
                    .then(|| a.piercing_projectiles.iter().map(|p| p.view()).collect()),
                player_fields: (c.mechanism_version >= 30)
                    .then(|| a.player_fields.iter().map(|f| f.view()).collect()),
                player_area_warnings: (c.mechanism_version >= 29)
                    .then(|| a.player_areas.iter().map(|p| p.warning.clone()).collect()),
                timed_position: a.timed_position_view(),
                counter: a.counter.as_ref().map(|v| v.view.clone()),
                map_objective: a.objective_view(),
                scoring: c.scoring_enabled().then(|| {
                    let summary = c.score_summary(&a.score_tally);
                    ScoreView {
                        kills: a.actual_kills(c),
                        credited_kills: summary.credited_kills,
                        kill_score: summary.kill_score,
                        extraction_cultivation: c.cultivation_from_score(
                            u64::from(summary.kill_score),
                            Outcome::Extraction,
                        ),
                        death_cultivation: c
                            .cultivation_from_score(u64::from(summary.kill_score), Outcome::Death),
                    }
                }),
                encounters: a.encounter_views(),
                chests: a.chest_views(),
                monsters,
                inventory: a.inventory(self, c).into_iter().collect(),
                resource_items: a
                    .resource_items
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                kills: self.kills,
                sword_marks: if now < a.marks_until { a.marks } else { 0 },
            }),
            phase: self.phase,
            logical_ms: now,
            active_at_ms: self.active_at,
            position: self.position,
            hp: self.hp,
            spirit: self.spirit,
            protected: self.protected,
            basic_ready_at_ms: self.basic_ready_at,
            skill_ready_at_ms: self.skill_ready_at,
            player_skills: self.skill_views(c),
            pending_action: self.pending_view(c),
            recovery_until_ms: self.recovery_until,
            inventory_quantity: self.inventory + a.items.values().sum::<u32>(),
            inventory_revision: self.inventory_revision,
            target: a.nearest(self.position, f64::MAX).map(|i| TargetView {
                id: a.id(i),
                position: a.monsters[i].position,
                hp: a.monsters[i].hp,
            }),
            resources: self.resources.iter().map(|r| r.view.clone()).collect(),
            extraction_open: self
                .active_at
                .is_some_and(|at| now >= at + c.extraction_unlock_ms),
            extraction_finishes_ms: self.extraction_end,
        }
    }
    pub(super) fn pve_settlement(&mut self, c: &Config) -> Business {
        let mut arena = self.pve.take().unwrap();
        // Terminal state is authoritative even when the output queue cannot fit cancel events.
        for m in &mut arena.monsters {
            m.area_cast = None;
            m.support_cast = None;
            m.heavy_cast = None;
        }
        arena.counter = None;
        arena.timed_position = None;
        for e in &mut arena.encounters {
            e.reset_at = None;
        }
        arena.attune_finishes = None;
        let outcome = self.desired_outcome.unwrap();
        let inventory = arena.inventory(self, c);
        let mut drops: BTreeMap<String, u32> = BTreeMap::new();
        if c.settlement_policy_outcome(outcome) == Outcome::Death {
            let mut slots = vec![];
            for (id, &q) in &inventory {
                let limit = arena.limit(id, c);
                for n in 0..q.div_ceil(limit) {
                    slots.push((id.clone(), (q - n * limit).min(limit)));
                }
            }
            let count = c.params.death.count(slots.len());
            for _ in 0..count {
                let i = self.random() as usize % slots.len();
                let (id, q) = slots.swap_remove(i);
                *drops.entry(id).or_default() += c.params.death.amount(q);
            }
        }
        let dropped = drops.get(&c.item_id).copied().unwrap_or(0);
        let consumed = arena.consumed();
        let mut received = arena.items.clone();
        for row in &consumed {
            *received.entry(row.item_id.clone()).or_default() += row.quantity;
        }
        let items = received
            .iter()
            .filter(|(id, quantity)| *id != &c.item_id && **quantity > 0)
            .map(|(id, &q)| ItemSettlement {
                item_id: id.clone(),
                looted: q,
                kept: arena.items.get(id).copied().unwrap_or(0)
                    - drops.get(id).copied().unwrap_or(0),
                dropped: drops.get(id).copied().unwrap_or(0),
            })
            .collect();
        self.phase = Phase::Draining;
        self.protected = true;
        self.movement = [0.0, 0.0];
        self.pending = None;
        self.projectiles.clear();
        self.extraction_end = None;
        let scoring = c
            .scoring_enabled()
            .then(|| c.score_summary(&arena.score_tally));
        let map_progress = arena.map_summary();
        let reset_progress = arena.reset_summary(c);
        self.pve = Some(arena);
        let cultivation = scoring.as_ref().map_or_else(
            || c.cultivation(self.kills, outcome),
            |s| c.cultivation_from_score(u64::from(s.kill_score), outcome),
        );
        Business::CommitSettlement {
            reset_progress,
            map_progress,
            scoring,
            outcome,
            expected_revision: self.facts.revision,
            item_id: c.item_id.clone(),
            carried: c.carried_quantity,
            looted: self.looted,
            kept: self.inventory - dropped,
            dropped,
            cultivation,
            items,
            instances: Vec::new(),
            rewards: Vec::new(),
            progression: None,
        }
    }
}

#[cfg(all(test, match_sdk_test_data))]
#[path = "pve_tests.rs"]
mod tests;

pub(crate) fn validate_walkable_centers(
    config: &Arc<crate::config::PveConfig>,
    mut centers: impl Iterator<Item = [f64; 2]>,
    radius: f64,
) -> bool {
    let grid = Grid::new(config);
    centers.all(|center| grid.clear(center, radius, 0))
}

pub(crate) fn validate_walkable_segment(
    config: &Arc<crate::config::PveConfig>,
    start: [f64; 2],
    end: [f64; 2],
    radius: f64,
) -> bool {
    let grid = Grid::new(config);
    grid.clear(end, radius, 0) && grid.line(start, end, radius, 0)
}

pub(crate) fn validate_map(
    config: &Arc<crate::config::PveConfig>,
    spawn: [f64; 2],
    extraction: [f64; 2],
) -> bool {
    let grid = Grid::new(config);
    [spawn, extraction]
        .into_iter()
        .chain(config.monster_positions.iter().copied())
        .chain(config.extra_spawns.iter().map(|m| m.position))
        .chain(config.encounters.iter().map(|e| e.trigger.center))
        .chain(
            config
                .encounters
                .iter()
                .flat_map(|e| e.all_monsters().map(|m| m.position)),
        )
        .chain(config.chest_spawns.iter().map(|chest| chest.position))
        .chain(
            config
                .timed_position
                .iter()
                .flat_map(crate::config::TimedPositionConfig::sites)
                .map(|site| site.position),
        )
        .all(|point| {
            grid.clear(point, config.prepared.as_ref().unwrap().player_radius, 0)
                && !grid
                    .route(
                        spawn,
                        point,
                        config.prepared.as_ref().unwrap().player_radius,
                    )
                    .is_empty()
        })
        && config
            .monster_positions
            .iter()
            .all(|point| grid.clear(*point, config.combat.radius, 0))
        && {
            let profiles = if config.profiles.is_empty() {
                config.build_profiles()
            } else {
                config.profiles.clone()
            };
            config
                .extra_spawns
                .iter()
                .chain(config.encounters.iter().flat_map(|e| e.all_monsters()))
                .all(|m| {
                    m.position
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 32768.0)
                        && grid.clear(m.position, profiles[&m.template_id].combat.radius, 0)
                })
                && config.guardian().is_none_or(|g| {
                    config.chest_spawns.iter().all(|chest| {
                        g.spawn_offsets.iter().all(|offset| {
                            let p = [chest.position[0] + offset[0], chest.position[1] + offset[1]];
                            let r = profiles[&g.template_id].combat.radius;
                            grid.line(chest.position, p, r, 0)
                                && grid.clear(p, config.prepared.as_ref().unwrap().player_radius, 0)
                                && !grid
                                    .route(
                                        spawn,
                                        p,
                                        config.prepared.as_ref().unwrap().player_radius,
                                    )
                                    .is_empty()
                        })
                    })
                })
        }
}
