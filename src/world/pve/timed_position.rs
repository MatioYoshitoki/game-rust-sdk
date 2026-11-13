use super::*;
use crate::config::{PositionCondition, TimedPositionConfig};

struct Warning {
    view: TimedPositionWarningView,
    success: Success,
}

enum Success {
    Damage(u32),
    RestoreSpirit(u32),
    DirectHit(u32),
}

pub(super) struct TimedHit {
    round: u32,
    cycle: u32,
    raw_damage: u32,
}

pub(super) struct TimedPositionState {
    encounter: usize,
    source: usize,
    round: u32,
    cycle: u32,
    successes: u32,
    next_site: usize,
    next_at: Option<u64>,
    warning: Option<Warning>,
}

impl TimedPositionState {
    pub(super) fn new(config: &PveConfig, monsters: &[Monster]) -> Option<Self> {
        let check = config.timed_position.as_ref()?;
        Some(Self {
            encounter: config
                .encounters
                .iter()
                .position(|e| e.id == check.encounter_id)?,
            source: monsters.iter().position(|m| m.id == check.source_id)?,
            round: 1,
            cycle: 0,
            successes: 0,
            next_site: 0,
            next_at: None,
            warning: None,
        })
    }
}

impl Arena {
    pub(super) fn timed_position_view(&self) -> Option<TimedPositionView> {
        let config = self.grid.config.timed_position.as_ref()?;
        let state = self.timed_position.as_ref()?;
        Some(TimedPositionView {
            id: config.id.clone(),
            name: config.name.clone(),
            encounter_id: config.encounter_id.clone(),
            radius: config.position.as_ref().map(PositionCondition::radius),
            contact_target_id: config.direct_hit.as_ref().map(|_| config.source_id.clone()),
            warning_bonus_active: self
                .completed_value(config.warning_bonus_ms.as_ref())
                .is_some(),
            failure_scale_active: self
                .completed_value(config.failure_scale_bps.as_ref())
                .is_some(),
            successes: state.successes,
            stabilized_sites: (!config.skip_sites_after.is_empty()).then(|| {
                config
                    .skip_sites_after
                    .values()
                    .filter(|id| self.encounter_completed(id))
                    .count() as u8
            }),
            warning: state.warning.as_ref().map(|warning| warning.view.clone()),
        })
    }

    pub(crate) fn has_timed_position_warning(&self) -> bool {
        self.timed_position
            .as_ref()
            .is_some_and(|s| s.warning.is_some())
    }

    pub(crate) fn cancel_timed_position(&mut self, events: &mut Vec<Event>) {
        let Some(state) = &mut self.timed_position else {
            return;
        };
        state.next_at = None;
        if let Some(warning) = state.warning.take() {
            events.push(Event::TimedPositionCancelled {
                mechanic_id: self.grid.config.timed_position.as_ref().unwrap().id.clone(),
                round: warning.view.round,
                cycle: warning.view.cycle,
            });
        }
    }

    fn timed_position_eligible(&self, w: &World, state: &TimedPositionState) -> bool {
        let encounter = &self.encounters[state.encounter];
        let trigger = &self.grid.config.encounters[state.encounter].trigger;
        w.phase == Phase::Active
            && w.hp > 0
            && w.desired_outcome.is_none()
            && w.ready
            && w.foreground
            && !w.protected
            && encounter.phase == EncounterPhase::Active
            && encounter.reset_at.is_none()
            && self.monsters[state.source].hp > 0
            && distance(w.position, trigger.center) <= trigger.radius
    }

    pub(super) fn take_timed_hit(
        &mut self,
        w: &World,
        index: usize,
        manual_started: Option<u64>,
    ) -> Option<TimedHit> {
        let started = manual_started?;
        let state = self.timed_position.as_ref()?;
        let warning = state.warning.as_ref()?;
        let Success::DirectHit(raw_damage) = warning.success else {
            return None;
        };
        if index != state.source
            || state.round != self.encounters[state.encounter].round
            || warning.view.hit_confirmed == Some(true)
            || started < warning.view.started_at_ms
            || started > self.now
            || self.now >= warning.view.resolves_at_ms
            || !self.timed_position_eligible(w, state)
        {
            return None;
        }
        let contact = TimedHit {
            round: state.round,
            cycle: state.cycle,
            raw_damage,
        };
        let state = self.timed_position.as_mut().unwrap();
        state.warning.as_mut().unwrap().view.hit_confirmed = Some(true);
        state.successes += 1;
        Some(contact)
    }

    pub(super) fn apply_timed_hit(
        &mut self,
        w: &mut World,
        c: &Config,
        target: usize,
        contact: TimedHit,
        events: &mut Vec<Event>,
    ) {
        let damage = if self.monsters[target].hp > 0 {
            self.apply_monster_damage(
                w,
                c,
                target,
                (contact.raw_damage, false, None, "timed_hit"),
                events,
            )
        } else {
            0
        };
        events.push(Event::TimedHitTriggered {
            mechanic_id: self.grid.config.timed_position.as_ref().unwrap().id.clone(),
            round: contact.round,
            cycle: contact.cycle,
            target_id: self.monsters[target].id.clone(),
            damage,
        });
    }

    pub(super) fn advance_timed_position(
        &mut self,
        w: &mut World,
        c: &Config,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let Some(state) = &self.timed_position else {
            return;
        };
        let source = state.source;
        let round = self.encounters[state.encounter].round;
        let eligible = self.timed_position_eligible(w, state);
        if state.round != round {
            self.cancel_timed_position(events);
            let state = self.timed_position.as_mut().unwrap();
            state.round = round;
            state.cycle = 0;
            state.successes = 0;
            state.next_site = 0;
        }
        if !eligible {
            self.cancel_timed_position(events);
            return;
        }

        let config = Arc::clone(&self.grid.config);
        let check = config.timed_position.as_ref().unwrap();
        let state = self.timed_position.as_mut().unwrap();
        if let Some(warning) = &state.warning {
            if now >= warning.view.resolves_at_ms {
                let warning = state.warning.take().unwrap();
                self.resolve_timed_position(w, c, check, warning, now, events);
            }
            return;
        }
        let next_at = state.next_at.get_or_insert(now + check.interval_ms);
        if now < *next_at {
            return;
        }
        let next_site = state.next_site;

        let bonus = self
            .completed_value(check.warning_bonus_ms.as_ref())
            .unwrap_or(0);
        let scale = self
            .completed_value(check.failure_scale_bps.as_ref())
            .unwrap_or(10000);
        let stats = &self.monsters[source].profile.stats;
        let failure_raw_damage = check.failure_damage_bps.map_or(0, |bps| {
            scaled_amount(self.monster_attack(source, now), bps, scale)
        });
        let miss_heal = check
            .direct_hit
            .as_ref()
            .map(|hit| scaled_amount(stats["hp"], hit.miss_heal_bps, scale));
        let sites = check.sites();
        let selected = (0..sites.len())
            .map(|offset| (next_site + offset) % sites.len())
            .find(|&index| {
                check
                    .skip_sites_after
                    .get(&sites[index].id)
                    .is_none_or(|side| !self.encounter_completed(side))
            });
        let player_attack = self.player_attack(w, now);
        let state = self.timed_position.as_mut().unwrap();
        state.cycle += 1;
        state.next_at = Some(now + check.interval_ms);
        let site = selected.map(|index| {
            state.next_site = index + 1;
            sites[index].clone()
        });
        let view = TimedPositionWarningView {
            round,
            cycle: state.cycle,
            started_at_ms: now,
            resolves_at_ms: now + check.warning_ms + bonus,
            failure_raw_damage,
            miss_heal,
            hit_confirmed: miss_heal.map(|_| false),
            site,
        };
        let success = if let Some(bps) = check.success_spirit_restore_bps {
            Success::RestoreSpirit(scaled_amount(w.facts.spirit_points, bps, 10000))
        } else {
            let damage = scaled_amount(player_attack, check.success_damage_bps.unwrap(), 10000);
            if check.direct_hit.is_some() {
                Success::DirectHit(damage)
            } else {
                Success::Damage(damage)
            }
        };
        state.warning = Some(Warning {
            view: view.clone(),
            success,
        });
        events.push(Event::TimedPositionStarted {
            mechanic_id: check.id.clone(),
            warning: view,
        });
    }

    fn resolve_timed_position(
        &mut self,
        w: &mut World,
        c: &Config,
        check: &TimedPositionConfig,
        warning: Warning,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        let state = self.timed_position.as_ref().unwrap();
        let source = state.source;
        if let Some(heal) = warning.view.miss_heal {
            if warning.view.hit_confirmed == Some(false) {
                let core = &mut self.monsters[source];
                let healed = heal.min(core.profile.stats["hp"].saturating_sub(core.hp));
                core.hp += healed;
                events.push(Event::TimedHitMissed {
                    mechanic_id: check.id.clone(),
                    round: warning.view.round,
                    cycle: warning.view.cycle,
                    target_id: core.id.clone(),
                    healed,
                });
            }
            return;
        }
        let radius = check.position.as_ref().unwrap().radius();
        let site = warning.view.site.as_ref();
        let origin = site.map_or(w.position, |site| site.position);
        let player_in_site = site.is_none_or(|site| distance(w.position, site.position) <= radius);
        let target = if player_in_site && matches!(warning.success, Success::Damage(_)) {
            self.encounters[state.encounter]
                .slots
                .iter()
                .copied()
                .filter_map(|i| {
                    let m = &self.monsters[i];
                    let range = distance(origin, m.position);
                    (m.enabled && m.hp > 0 && (site.is_some() || range <= radius))
                        .then_some((i, range))
                })
                .min_by(|(a, da), (b, db)| {
                    da.total_cmp(db)
                        .then_with(|| self.monsters[*a].id.cmp(&self.monsters[*b].id))
                })
                .map(|(i, _)| i)
        } else {
            None
        };
        let (target_id, damage, spirit_restored) = match (warning.success, target) {
            (Success::Damage(amount), Some(target)) => {
                let damage = self.hurt_monster_contact(w, c, target, (amount, false, None), events);
                self.advance_encounters(w, events);
                (Some(self.id(target)), damage, None)
            }
            (Success::RestoreSpirit(amount), _) if player_in_site => {
                let restored = amount.min(w.facts.spirit_points.saturating_sub(w.spirit));
                w.spirit += restored;
                (None, 0, Some(restored))
            }
            _ => {
                let before = w.hp;
                self.hurt_player(
                    w,
                    c,
                    source,
                    (warning.view.failure_raw_damage, false),
                    now,
                    events,
                );
                (None, before - w.hp, None)
            }
        };
        if target_id.is_some() || spirit_restored.is_some() {
            self.timed_position.as_mut().unwrap().successes += 1;
        }
        events.push(Event::TimedPositionResolved {
            mechanic_id: check.id.clone(),
            round: warning.view.round,
            cycle: warning.view.cycle,
            site_id: warning.view.site.map(|site| site.id),
            target_id,
            damage,
            spirit_restored,
        });
    }
}
