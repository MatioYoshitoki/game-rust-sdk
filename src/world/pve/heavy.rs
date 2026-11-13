use super::*;

pub(super) struct HeavyCast {
    id: u32,
    epoch: u32,
    origin: [f64; 2],
    end: [f64; 2],
    front: [f64; 2],
    target_start: [f64; 2],
    eligible: bool,
    escaped: bool,
    started: u64,
    commits: u64,
    released: Option<u64>,
    raw: u32,
}
pub(super) struct CounterWindow {
    target: usize,
    epoch: u32,
    pub(super) view: CounterView,
}
fn inside(origin: [f64; 2], end: [f64; 2], point: [f64; 2], radius: f64) -> bool {
    let d = [end[0] - origin[0], end[1] - origin[1]];
    let length = d[0] * d[0] + d[1] * d[1];
    let t = if length == 0.0 {
        0.0
    } else {
        (((point[0] - origin[0]) * d[0] + (point[1] - origin[1]) * d[1]) / length).clamp(0.0, 1.0)
    };
    distance([origin[0] + t * d[0], origin[1] + t * d[1]], point) <= radius
}
impl Monster {
    pub(super) fn heavy_view(&self) -> Option<HeavyView> {
        let h = self.profile.heavy.as_ref()?;
        Some(HeavyView {
            skill_id: h.id.clone(),
            name: h.name.clone(),
            ready_at_ms: self.heavy_ready,
            cast: self.heavy_cast.as_ref().map(|a| HeavyCastView {
                cast_id: a.id,
                origin: a.origin,
                end: a.end,
                front: a.front,
                half_width: h.half_width,
                started_at_ms: a.started,
                commits_at_ms: a.commits,
                ends_at_ms: a.released.unwrap_or(a.commits) + h.travel_ms,
                committed: a.released.is_some(),
            }),
        })
    }
    pub(super) fn take_heavy(&mut self, now: u64) -> Option<HeavyCastIdentity> {
        let cast = self.heavy_cast.take()?;
        self.recovery = self
            .recovery
            .max(now + self.profile.heavy.as_ref().unwrap().recovery_ms);
        Some(HeavyCastIdentity {
            monster_id: self.id.clone(),
            cast_id: cast.id,
        })
    }
}
impl Arena {
    pub(crate) fn has_heavy_state(&self) -> bool {
        self.counter.is_some() || self.monsters.iter().any(|m| m.heavy_cast.is_some())
    }
    pub(crate) fn cancel_all_heavies(
        &mut self,
        reason: HeavyCancelReason,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let casts: Vec<_> = self
            .monsters
            .iter_mut()
            .filter_map(|m| m.take_heavy(now))
            .collect();
        let counter = self.counter.take().map(|v| HeavyCastIdentity {
            monster_id: v.view.monster_id,
            cast_id: v.view.cast_id,
        });
        if !casts.is_empty() || counter.is_some() {
            events.push(Event::HeavyCancelled {
                casts,
                counter,
                reason,
            });
        }
    }
    pub(super) fn cancel_heavy_for(
        &mut self,
        i: usize,
        reason: HeavyCancelReason,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        self.cancel_heavies_for_members(&[i], reason, now, events);
    }
    pub(super) fn cancel_heavies_for_members(
        &mut self,
        members: &[usize],
        reason: HeavyCancelReason,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let casts: Vec<_> = members
            .iter()
            .filter_map(|&i| self.monsters[i].take_heavy(now))
            .collect();
        let counter = if self
            .counter
            .as_ref()
            .is_some_and(|v| members.contains(&v.target))
        {
            self.counter.take().map(|v| HeavyCastIdentity {
                monster_id: v.view.monster_id,
                cast_id: v.view.cast_id,
            })
        } else {
            None
        };
        if !casts.is_empty() || counter.is_some() {
            events.push(Event::HeavyCancelled {
                casts,
                counter,
                reason,
            });
        }
    }
    pub(super) fn take_counter(
        &mut self,
        i: usize,
        manual_started: Option<u64>,
        now: u64,
    ) -> Option<CounterView> {
        let v = self.counter.as_ref()?;
        let started = manual_started?;
        if self.monsters[i].returning
            || started > now
            || v.target != i
            || self.monsters[i].id != v.view.monster_id
            || self.monsters[i].boss_epoch != v.epoch
            || started < v.view.started_at_ms
            || now > v.view.ends_at_ms
        {
            return None;
        }
        self.counter.take().map(|v| v.view)
    }
    pub(super) fn try_start_heavy(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        now: u64,
        events: &mut Vec<Event>,
    ) -> bool {
        let m = &self.monsters[i];
        let Some(h) = &m.profile.heavy else {
            return false;
        };
        let separation = distance(m.position, w.position);
        if c.mechanism_version < 11
            || w.phase != Phase::Active
            || !w.ready
            || !w.foreground
            || w.protected
            || w.hp == 0
            || w.desired_outcome.is_some()
            || !m.enabled
            || m.hp == 0
            || !m.aggro
            || m.returning
            || m.heavy_cast.is_some()
            || m.support_cast.is_some()
            || m.area_cast.is_some()
            || m.casting.is_some()
            || m.heal.is_some()
            || now < m.recovery
            || now < m.heavy_ready
            || m.spirit < h.cost
            || m.boss_phase + 1 < h.min_phase_index as usize
            || separation <= 1e-9
            || !separation.is_finite()
            || separation > h.range
            || h.cast_chance == 0.0
            || !self.grid.line(m.position, w.position, 0.0, 2)
        {
            return false;
        }
        if h.cast_chance < 1.0
            && f64::from(w.random()) / (f64::from(u32::MAX) + 1.0) >= h.cast_chance
        {
            return false;
        }
        let attack = self.monster_attack(i, now);
        let m = &mut self.monsters[i];
        let h = m.profile.heavy.as_ref().unwrap();
        let direction = normalize([w.position[0] - m.position[0], w.position[1] - m.position[1]]);
        let end = [
            m.position[0] + direction[0] * h.range,
            m.position[1] + direction[1] * h.range,
        ];
        m.heavy_sequence += 1;
        m.heavy_cast = Some(HeavyCast {
            id: m.heavy_sequence,
            epoch: m.boss_epoch,
            origin: m.position,
            end,
            front: m.position,
            target_start: w.position,
            eligible: inside(
                m.position,
                end,
                w.position,
                h.half_width + c.params.player_radius,
            ) && self.grid.line(m.position, w.position, 0.0, 1),
            escaped: false,
            started: now,
            commits: now + h.windup_ms,
            released: None,
            raw: (u64::from(attack) * u64::from(h.damage_bps) / 10000) as u32,
        });
        m.path.clear();
        events.push(Event::HeavyStarted {
            monster_id: m.id.clone(),
            cast_id: m.heavy_sequence,
            skill_id: h.id.clone(),
            commits_at_ms: now + h.windup_ms,
        });
        true
    }
    pub(super) fn advance_heavies(
        &mut self,
        w: &mut World,
        c: &Config,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        if w.phase != Phase::Active || w.hp == 0 || w.desired_outcome.is_some() {
            self.cancel_all_heavies(HeavyCancelReason::Closed, now, events);
            return;
        }
        if w.protected || !w.ready || !w.foreground {
            self.cancel_all_heavies(HeavyCancelReason::Background, now, events);
            return;
        }
        if let Some(v) = &self.counter {
            let reason = if now > v.view.ends_at_ms {
                Some(HeavyCancelReason::Expired)
            } else if self.monsters[v.target].id != v.view.monster_id
                || self.monsters[v.target].boss_epoch != v.epoch
                || self.monsters[v.target].hp == 0
                || !self.monsters[v.target].enabled
                || self.monsters[v.target].returning
            {
                Some(HeavyCancelReason::Replaced)
            } else {
                None
            };
            if let Some(reason) = reason {
                let view = self.counter.take().unwrap().view;
                events.push(Event::HeavyCancelled {
                    casts: vec![],
                    counter: Some(HeavyCastIdentity {
                        monster_id: view.monster_id,
                        cast_id: view.cast_id,
                    }),
                    reason,
                });
            }
        }
        let mut order: Vec<_> = (0..self.monsters.len()).collect();
        order.sort_by(|&a, &b| self.monsters[a].id.cmp(&self.monsters[b].id));
        let mut replaced = vec![];
        let mut resource = vec![];
        for i in order {
            let Some(mut cast) = self.monsters[i].heavy_cast.take() else {
                continue;
            };
            let h = self.monsters[i].profile.heavy.as_ref().unwrap().clone();
            let identity = HeavyCastIdentity {
                monster_id: self.id(i),
                cast_id: cast.id,
            };
            if self.monsters[i].hp == 0
                || !self.monsters[i].enabled
                || self.monsters[i].returning
                || self.monsters[i].boss_epoch != cast.epoch
            {
                self.monsters[i].recovery = self.monsters[i].recovery.max(now + h.recovery_ms);
                replaced.push(identity);
                continue;
            }
            let outside = !inside(
                cast.origin,
                cast.end,
                w.position,
                h.half_width + c.params.player_radius,
            );
            if outside && distance(w.position, cast.target_start) > 1e-6 {
                cast.escaped = true;
            }
            let Some(released) = cast.released else {
                if now >= cast.commits {
                    if self.monsters[i].spirit < h.cost {
                        self.monsters[i].recovery =
                            self.monsters[i].recovery.max(now + h.recovery_ms);
                        resource.push(identity);
                        continue;
                    }
                    self.monsters[i].spirit -= h.cost;
                    self.monsters[i].heavy_ready = now + h.cooldown_ms;
                    cast.released = Some(now);
                    events.push(Event::HeavyCommitted { cast: identity });
                }
                self.monsters[i].heavy_cast = Some(cast);
                continue;
            };
            let progress = (now.saturating_sub(released) as f64 / h.travel_ms as f64).min(1.0);
            let end = [
                cast.origin[0] + (cast.end[0] - cast.origin[0]) * progress,
                cast.origin[1] + (cast.end[1] - cast.origin[1]) * progress,
            ];
            let steps = distance(cast.front, end).ceil().max(1.0) as u32;
            let wall = (0..=steps)
                .find(|&j| {
                    let t = f64::from(j) / f64::from(steps);
                    !self.grid.clear(
                        [
                            cast.front[0] + (end[0] - cast.front[0]) * t,
                            cast.front[1] + (end[1] - cast.front[1]) * t,
                        ],
                        h.half_width,
                        1,
                    )
                })
                .map(|j| {
                    let mut lo = f64::from(j.saturating_sub(1)) / f64::from(steps);
                    let mut hi = f64::from(j) / f64::from(steps);
                    for _ in 0..32 {
                        let t = (lo + hi) / 2.0;
                        if self.grid.clear(
                            [
                                cast.front[0] + (end[0] - cast.front[0]) * t,
                                cast.front[1] + (end[1] - cast.front[1]) * t,
                            ],
                            h.half_width,
                            1,
                        ) {
                            lo = t;
                        } else {
                            hi = t;
                        }
                    }
                    hi
                });
            let hit = segment_circle(
                cast.front,
                end,
                w.position,
                h.half_width + c.params.player_radius,
            );
            let contacted = hit.is_some_and(|t| wall.is_none_or(|wall| t + 1e-9 < wall));
            let blocked = wall.is_some() && !contacted;
            if contacted || blocked || progress >= 1.0 {
                self.monsters[i].recovery = self.monsters[i].recovery.max(now + h.recovery_ms);
                let before = w.hp;
                if contacted {
                    self.hurt_player(w, c, i, (cast.raw, false), now, events);
                }
                let awarded = !contacted
                    && !blocked
                    && cast.eligible
                    && cast.escaped
                    && outside
                    && self.counter.is_none()
                    && w.hp > 0
                    && w.desired_outcome.is_none();
                if awarded {
                    let bonus = self
                        .completed_value(h.counter_window_bonus_ms.as_ref())
                        .unwrap_or(0);
                    let scale = self
                        .completed_value(h.counter_damage_scale_bps.as_ref())
                        .unwrap_or(10000);
                    let view = CounterView {
                        monster_id: self.id(i),
                        cast_id: cast.id,
                        started_at_ms: now,
                        ends_at_ms: now + h.counter_window_ms + bonus,
                        raw_damage: scaled_amount(
                            self.player_attack(w, now),
                            h.counter_damage_bps,
                            scale,
                        ),
                    };
                    self.counter = Some(CounterWindow {
                        target: i,
                        epoch: cast.epoch,
                        view: view.clone(),
                    });
                    events.push(Event::CounterOpened { window: view });
                }
                events.push(Event::HeavyResolved {
                    cast: identity,
                    contacted,
                    damage: before - w.hp,
                    blocked,
                    counter_awarded: awarded,
                });
                if w.hp == 0 {
                    break;
                }
            } else {
                cast.front = end;
                self.monsters[i].heavy_cast = Some(cast);
            }
        }
        if !replaced.is_empty() {
            events.push(Event::HeavyCancelled {
                casts: replaced,
                counter: None,
                reason: HeavyCancelReason::Replaced,
            });
        }
        if !resource.is_empty() {
            events.push(Event::HeavyCancelled {
                casts: resource,
                counter: None,
                reason: HeavyCancelReason::Resource,
            });
        }
    }
}
