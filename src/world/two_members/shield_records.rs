//! One consumed end record can produce one bounded, non-recursive release.
use super::*;
#[derive(Default)]
pub(super) struct ShieldRecordState {
    pub(super) executing: bool,
    step: Option<u64>,
    releases: usize,
    hits: usize,
}
impl World {
    /// Prepare the same native record whether the balance comes from a grant or recovery excess.
    pub(super) fn configure_shield_record(
        &self,
        definition: &mut crate::config::StatusDefinition,
        source: Owner,
        c: &Config,
        now: u64,
    ) {
        definition.derived =
            self.record_shields.executing || self.status_ends.executing || self.stack_leaves;
        definition.action_origin = if !definition.derived
            && self.triggers.action_source.as_ref() == Some(&self.owner_entity(source))
        {
            self.triggers.origin.clone()
        } else {
            None
        };
        definition.end_effects = if definition.derived {
            0
        } else {
            c.combat_rules
                .as_ref()
                .and_then(|r| r.status_ends.get(&definition.id))
                .map_or(0, |r| {
                    r.on.iter().fold(0, |mask, reason| {
                        mask | match reason {
                            StatusRemovalReason::Expired => 1,
                            StatusRemovalReason::Depleted => 2,
                            StatusRemovalReason::Consumed => 4,
                            StatusRemovalReason::Cleansed => 8,
                            _ => 0,
                        }
                    })
                })
        };
        let Some(rule) = c
            .combat_rules
            .as_ref()
            .and_then(|r| r.shield_records.get(&definition.id))
        else {
            return;
        };
        definition.shield_record = Some(ShieldRecordView {
            basis: rule.basis,
            recorded: 0,
            cap: rule.cap(
                self.status_source_power(&self.statuses, source, &definition.id, c, now)
                    .unwrap_or_else(|| self.record_power(source, c, now)),
            ),
        });
        definition.shield_record_preserve =
            matches!(rule.refresh, crate::config::RecordRefresh::Preserve);
        definition.shield_record_reduction = rule.damage_reduction_bps;
    }
    pub(super) fn record_power(&self, owner: Owner, c: &Config, now: u64) -> u32 {
        self.record_power_with(&self.statuses, owner, c, now)
    }
    pub(super) fn record_power_with(
        &self,
        statuses: &Statuses,
        owner: Owner,
        c: &Config,
        now: u64,
    ) -> u32 {
        let entity = status_entity(&self.owner_entity(owner));
        let base = self.base_attack(owner, c);
        let attack = statuses.attribute(
            &entity,
            base,
            now,
            c.pve.as_ref().unwrap().status_rules.as_ref(),
            StatusAttribute::Atk,
        );
        if self.has_power_build(owner, c) {
            let (bonus, points) = self.power_multipliers(statuses, owner, c, now);
            return crate::config::AttackRule::default().budget(attack, 10000, bonus, points);
        }
        let bonus = statuses.attribute(
            &entity,
            10000,
            now,
            c.pve.as_ref().unwrap().status_rules.as_ref(),
            StatusAttribute::SkillPowerBonus,
        );
        let points = self.combat_profile(owner, c).power_points_bps;
        crate::config::AttackRule::default().budget(attack, 10000, bonus, points)
    }
    fn record_end_event(
        &self,
        status: &StatusView,
        reason: StatusRemovalReason,
        amounts: (u32, u32),
        suppressed: Option<Error>,
        events: &mut Events,
    ) {
        for i in self.status_observers(&status.source, &status.target) {
            private(
                events,
                i,
                Event::ShieldRecordEnded {
                    instance_id: status.instance_id.clone(),
                    definition_id: status.definition_id.clone(),
                    source: status.source.clone(),
                    target: status.target.clone(),
                    reason,
                    recorded: status
                        .shield
                        .as_ref()
                        .unwrap()
                        .record
                        .as_ref()
                        .unwrap()
                        .recorded,
                    raw_counter: amounts.0,
                    targets_hit: amounts.1,
                    suppressed,
                },
            );
        }
    }
    pub(super) fn finish_shield_records(&mut self, c: &Config, now: u64, events: &mut Events) {
        let ended = self.statuses.take_shield_ends();
        if ended.is_empty() {
            return;
        }
        let Some(rules) = &c.combat_rules else {
            return;
        };
        if self.record_shields.executing || self.triggers.executing {
            for (status, reason) in ended {
                self.record_end_event(&status, reason, (0, 0), Some(Error::InvalidState), events);
            }
            return;
        }
        if self.record_shields.step != Some(now) {
            self.record_shields.step = Some(now);
            self.record_shields.releases = 0;
            self.record_shields.hits = 0;
        }
        self.record_shields.executing = true;
        let prior_trigger_guard = self.triggers.executing;
        self.triggers.executing = true;
        for (status, reason) in ended {
            let Some(rule) = rules.shield_records.get(&status.definition_id) else {
                continue;
            };
            if !rule.releases_on(reason) {
                self.record_end_event(&status, reason, (0, 0), None, events);
                continue;
            }
            if !self.valid_status_entity(&status.source)
                || !self.valid_status_entity(&status.target)
            {
                self.record_end_event(&status, reason, (0, 0), Some(Error::InvalidTarget), events);
                continue;
            }
            if self.periodic_outcome(&status.source, &status.target) != PeriodicTickOutcome::Applied
            {
                self.record_end_event(&status, reason, (0, 0), Some(Error::InvalidState), events);
                continue;
            }
            if self.record_shields.releases >= rules.max_shield_releases_per_step
                || !rules.status_ends.is_empty() && !self.take_status_end_budget(c, now)
            {
                self.record_end_event(&status, reason, (0, 0), Some(Error::Capacity), events);
                continue;
            }
            self.record_shields.releases += 1;
            let source = self.entity_owner(&status.source).unwrap();
            let holder = self.entity_owner(&status.target).unwrap();
            let center = self.actor_position(holder);
            let counter = rule.counter_for(reason);
            let raw = counter.map_or(0, |counter| {
                (u64::from(
                    status
                        .shield
                        .as_ref()
                        .unwrap()
                        .record
                        .as_ref()
                        .unwrap()
                        .recorded,
                ) * u64::from(counter.ratio_bps)
                    / 10000) as u32
            });
            let mut hits = 0;
            let mut suppressed = None;
            if let Some(counter) = counter
                && raw > 0
            {
                let source_ref = Self::status_reference(&status.source);
                let mut candidates = self
                    .members
                    .iter()
                    .enumerate()
                    .map(|(i, m)| (Owner::Member(i), &m.reference.entity, m.position))
                    .chain(
                        self.npcs
                            .iter()
                            .enumerate()
                            .map(|(j, n)| (Owner::Npc(j), &n.entity, n.position)),
                    )
                    .filter(|(_, e, p)| {
                        self.hostile_contact(&source_ref, e)
                            && distance(center, *p) <= counter.radius
                            && self.grid.line(center, *p, 0., 2)
                    })
                    .map(|(o, e, p)| (distance(center, p), e.id.clone(), e.generation, o))
                    .collect::<Vec<_>>();
                candidates
                    .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
                let attack = crate::config::AttackRule {
                    damage_kind: counter.damage_kind,
                    element: counter.element.clone(),
                    ignore_armor: counter.ignore_armor,
                    unavoidable: true,
                    can_crit: false,
                    ..Default::default()
                };
                for (_, _, _, target) in candidates.into_iter().take(counter.max_targets) {
                    if !self.valid_status_entity(&status.source)
                        || !self.valid_status_entity(&status.target)
                    {
                        break;
                    }
                    if self.record_shields.hits >= rules.max_shield_counter_hits_per_step {
                        suppressed = Some(Error::Capacity);
                        break;
                    }
                    // Attempt budget also covers immune contacts; they cannot bypass it.
                    self.record_shields.hits += 1;
                    if self.direct_contact(
                        source,
                        target,
                        combat::DirectContact {
                            origin: None,
                            action_id: None,
                            rule: Some(&attack),
                            ignore_armor: counter.ignore_armor,
                            unavoidable: true,
                            dead_source: false,
                            source: &source_ref,
                            raw,
                            metal: true,
                            hit_statuses: &counter.on_hit_target_statuses,
                        },
                        c,
                        now,
                        events,
                    ) {
                        hits += 1;
                    }
                }
            }
            if self.valid_status_entity(&status.source) && self.valid_status_entity(&status.target)
            {
                let target_ref = Self::status_reference(&status.target);
                let max = self.entity_max_hp(&target_ref, c, now).unwrap();
                self.apply_owned_statuses(
                    source,
                    (target_ref, max),
                    &rule.after_end_target_statuses,
                    c,
                    now,
                    events,
                );
            }
            self.record_end_event(&status, reason, (raw, hits), suppressed, events);
        }
        // Status expiry or depletion produced by the derived release is cleanup only.
        let nested = self.statuses.take_shield_ends();
        for (status, reason) in nested {
            self.record_end_event(&status, reason, (0, 0), Some(Error::InvalidState), events);
        }
        self.triggers.executing = prior_trigger_guard;
        self.record_shields.executing = false;
        self.reconcile_caps(c, now);
    }
}
