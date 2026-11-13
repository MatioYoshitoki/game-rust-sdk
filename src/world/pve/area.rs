use super::*;
pub(super) struct AreaCast {
    pub(super) id: u32,
    pub(super) center: [f64; 2],
    pub(super) started: u64,
    pub(super) due: u64,
}
impl Monster {
    pub(super) fn area_view(&self) -> Option<AreaWarningView> {
        let cast = self.area_cast.as_ref()?;
        let a = self.profile.area.as_ref().unwrap();
        Some(AreaWarningView {
            cast_id: cast.id,
            skill_id: a.id.clone(),
            name: a.name.clone(),
            center: cast.center,
            radius: a.radius,
            started_at_ms: cast.started,
            resolves_at_ms: cast.due,
        })
    }
    pub(super) fn remove_area(&mut self, now: u64) -> Option<AreaCastIdentity> {
        let cast = self.area_cast.take()?;
        let a = self.profile.area.as_ref().unwrap();
        self.recovery = self.recovery.max(now + a.recovery_ms);
        Some(AreaCastIdentity {
            monster_id: self.id.clone(),
            cast_id: cast.id,
        })
    }
}
impl Arena {
    pub(crate) fn current_time(&self) -> u64 {
        self.now
    }
    pub(crate) fn has_area_warning(&self) -> bool {
        self.monsters.iter().any(|m| m.area_cast.is_some())
    }
    pub(crate) fn cancel_all_areas(
        &mut self,
        reason: AreaCancelReason,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let casts: Vec<_> = self
            .monsters
            .iter_mut()
            .filter_map(|m| m.remove_area(now))
            .collect();
        if !casts.is_empty() {
            events.push(Event::AreaCancelled { casts, reason });
        }
    }
    pub(super) fn cancel_area(
        &mut self,
        i: usize,
        reason: AreaCancelReason,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        if let Some(cast) = self.monsters[i].remove_area(now) {
            events.push(Event::AreaCancelled {
                casts: vec![cast],
                reason,
            });
        }
    }
    pub(super) fn try_start_area(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        now: u64,
        events: &mut Vec<Event>,
    ) -> bool {
        let profile = self.monsters[i].profile.clone();
        let Some(a) = &profile.area else { return false };
        let m = &self.monsters[i];
        let separation = distance(m.position, w.position);
        if c.mechanism_version < 8
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
            || now < m.area_ready
            || m.spirit < a.cost
            || m.boss_phase + 1 < a.min_phase_index as usize
            || !separation.is_finite()
            || separation > a.range
            || !self.grid.line(m.position, w.position, 0.0, 2)
        {
            return false;
        }
        if w.random() % 10000 >= (a.cast_chance * 10000.0) as u32 {
            return false;
        }
        let m = &mut self.monsters[i];
        m.area_sequence += 1;
        m.spirit -= a.cost;
        m.area_ready = now + a.cooldown_ms;
        m.area_cast = Some(AreaCast {
            id: m.area_sequence,
            center: w.position,
            started: now,
            due: now + a.windup_ms,
        });
        m.path.clear();
        events.push(Event::AreaStarted {
            monster_id: m.id.clone(),
            cast_id: m.area_sequence,
            skill_id: a.id.clone(),
            center: w.position,
            radius: a.radius,
            started_at_ms: now,
            resolves_at_ms: now + a.windup_ms,
        });
        true
    }
    pub(super) fn advance_area(
        &mut self,
        w: &mut World,
        c: &Config,
        i: usize,
        now: u64,
        events: &mut Vec<Event>,
    ) -> bool {
        let Some(cast) = self.monsters[i].area_cast.as_ref() else {
            return false;
        };
        if now < cast.due {
            return false;
        }
        let cast = self.monsters[i].area_cast.take().unwrap();
        let profile = self.monsters[i].profile.clone();
        let a = profile.area.as_ref().unwrap();
        self.monsters[i].recovery = self.monsters[i].recovery.max(now + a.recovery_ms);
        let hit = !w.protected
            && w.hp > 0
            && w.desired_outcome.is_none()
            && self
                .grid
                .circle_contact(cast.center, a.radius, w.position, c.params.player_radius);
        let before = w.hp;
        if hit {
            self.hurt_player(
                w,
                c,
                i,
                (
                    (u64::from(self.monster_attack(i, now)) * u64::from(a.damage_bps) / 10000)
                        as u32,
                    false,
                ),
                now,
                events,
            );
        }
        events.push(Event::AreaResolved {
            monster_id: self.monsters[i].id.clone(),
            cast_id: cast.id,
            skill_id: a.id.clone(),
            hit,
            damage: before - w.hp,
        });
        true
    }
}
