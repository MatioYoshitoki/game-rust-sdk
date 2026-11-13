//! Shared status scheduler, resolved against this World's actual entity identities.
use super::*;
#[derive(Default)]
pub(super) struct DamageResult {
    pub hp_damage: u32,
    pub absorbed: u32,
    pub lethal: bool,
}

impl World {
    pub(super) fn owner_entity(&self, owner: Owner) -> EntityRef {
        match owner {
            Owner::Member(i) => self.members[i].reference.entity.clone(),
            Owner::Npc(i) => self.npcs[i].entity.clone(),
            Owner::Summon(i) => self.summons[i].entity.clone(),
        }
    }
    pub(super) fn entity_owner(&self, entity: &StatusEntity) -> Option<Owner> {
        if entity.encounter_round.is_some() {
            return None;
        }
        self.resolve_entity(&EntityRef {
            kind: entity.kind,
            id: entity.id.clone(),
            generation: entity.generation,
        })
    }
    pub(super) fn valid_status_entity(&self, entity: &StatusEntity) -> bool {
        match self.entity_owner(entity) {
            Some(Owner::Member(i)) => self.members[i].living(),
            Some(Owner::Npc(i)) => self.npcs[i].hp > 0 && self.npcs[i].pursuit_can_fight(),
            Some(Owner::Summon(i)) => self.summon_alive(i),
            None => false,
        }
    }
    fn is_member_ally(&self, source: &StatusEntity, target: &StatusEntity) -> bool {
        source.kind == StatusEntityKind::Participant
            && target.kind == StatusEntityKind::Participant
            && self.status_relation(source, target) == Some(super::combat::Relation::Friendly)
    }
    pub(super) fn status_observers(
        &self,
        source: &StatusEntity,
        target: &StatusEntity,
    ) -> Vec<usize> {
        match (self.entity_owner(source), self.entity_owner(target)) {
            (Some(Owner::Member(i)), Some(Owner::Member(j))) if i != j => vec![i, j],
            (_, Some(Owner::Member(i))) => vec![i],
            (Some(Owner::Member(i)), _) => vec![i],
            (_, Some(Owner::Summon(i))) | (Some(Owner::Summon(i)), _) => {
                vec![self.summons[i].member]
            }
            (Some(Owner::Npc(_)), Some(Owner::Npc(_))) if source == target => self
                .members
                .iter()
                .enumerate()
                .filter(|(_, m)| m.living())
                .map(|(i, _)| i)
                .collect(),
            _ => vec![],
        }
    }
    pub(super) fn route_status_events(
        &self,
        raw: Vec<Event>,
        before: &[StatusView],
        events: &mut Events,
    ) {
        let records: BTreeMap<_, _> = before
            .iter()
            .chain(self.statuses.entries())
            .map(|s| (s.instance_id.clone(), s))
            .collect();
        for event in raw {
            if let Event::StatusTransferred {
                previous_target,
                status,
            } = event
            {
                let former_observers = self.status_observers(&status.source, &previous_target);
                let mut recipients = self.status_observers(&status.source, &status.target);
                if let Some(Owner::Member(i)) = self.entity_owner(&previous_target) {
                    recipients.push(i);
                }
                recipients.sort();
                recipients.dedup();
                // Public NPC observers may learn that their old visible instance
                // disappeared, but never receive the new holder's private state.
                for i in former_observers
                    .into_iter()
                    .filter(|i| !recipients.contains(i))
                {
                    private(
                        events,
                        i,
                        Event::StatusRemoved {
                            instance_ids: vec![status.instance_id.clone()],
                            reason: StatusRemovalReason::Transferred,
                        },
                    );
                }
                for i in recipients {
                    private(
                        events,
                        i,
                        Event::StatusTransferred {
                            previous_target: previous_target.clone(),
                            status: status.clone(),
                        },
                    );
                }
                continue;
            }
            if let Event::StatusRemoved {
                instance_ids,
                reason,
            } = event
            {
                let mut recipients: BTreeMap<usize, Vec<String>> = BTreeMap::new();
                for id in instance_ids {
                    if let Some(s) = records.get(&id) {
                        for i in self.status_observers(&s.source, &s.target) {
                            recipients.entry(i).or_default().push(id.clone());
                        }
                    }
                }
                for (i, mut instance_ids) in recipients {
                    instance_ids.sort();
                    private(
                        events,
                        i,
                        Event::StatusRemoved {
                            instance_ids,
                            reason,
                        },
                    );
                }
                continue;
            }
            if let Event::ShieldAbsorbed {
                source,
                target,
                consumptions,
                ..
            } = event
            {
                let mut slices: BTreeMap<usize, Vec<crate::protocol::ShieldConsumption>> =
                    BTreeMap::new();
                for entry in consumptions {
                    if let Some(s) = records.get(&entry.instance_id) {
                        let observers = if let Some(Owner::Member(i)) = self.entity_owner(&s.target)
                        {
                            vec![i]
                        } else {
                            self.status_observers(&s.source, &s.target)
                        };
                        for i in observers {
                            slices.entry(i).or_default().push(entry.clone());
                        }
                    }
                }
                for (i, consumptions) in slices {
                    let amount = consumptions.iter().map(|s| s.absorbed).sum();
                    private(
                        events,
                        i,
                        Event::ShieldAbsorbed {
                            source: source.clone(),
                            target: target.clone(),
                            amount,
                            consumptions,
                        },
                    );
                }
                continue;
            }
            let entities = match &event {
                Event::StatusApplied { status }
                | Event::StatusRefreshed { status }
                | Event::StatusStacked { status, .. } => Some((&status.source, &status.target)),
                Event::StatusIgnored { instance_id } => {
                    records.get(instance_id).map(|s| (&s.source, &s.target))
                }
                Event::StatusConsumed { source, target, .. }
                | Event::ShieldGranted { source, target, .. }
                | Event::StatusRejected { source, target, .. }
                | Event::PeriodicTick { source, target, .. }
                | Event::PeriodicHealTick { source, target, .. }
                | Event::PeriodicSpiritTick { source, target, .. } => Some((source, target)),
                _ => None,
            };
            if let Some((source, target)) = entities {
                for i in self.status_observers(source, target) {
                    private(events, i, event.clone());
                }
            }
        }
    }
    pub(super) fn can_owned_statuses(
        &self,
        owner: Owner,
        target: &EntityRef,
        ids: &[String],
        c: &Config,
    ) -> bool {
        ids.is_empty()
            || self.statuses.can_apply(
                &self.resolve_control_definitions(
                    &status_entity(target),
                    &Self::definitions(c, ids),
                    c,
                ),
                &status_entity(&self.owner_entity(owner)),
                &status_entity(target),
                c.pve.as_ref().unwrap().status_rules.as_ref().unwrap(),
            )
    }
    pub(super) fn apply_owned_statuses(
        &mut self,
        owner: Owner,
        target: (EntityRef, u32),
        ids: &[String],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if ids.is_empty() {
            return;
        }
        let defs = Self::definitions(c, ids);
        self.apply_owned_status_definitions(owner, target.0, &defs, c, now, events);
    }
    pub(super) fn apply_owned_status_definitions(
        &mut self,
        owner: Owner,
        target: EntityRef,
        defs: &[crate::config::StatusDefinition],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let source = status_entity(&self.owner_entity(owner));
        self.apply_status_definitions((source, Some(owner), None), target, defs, c, now, events);
    }

    pub(super) fn apply_boon_status(
        &mut self,
        owner: Owner,
        target: EntityRef,
        (boon, id): (&str, String),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let source = status_entity(&self.owner_entity(owner));
        let defs = Self::definitions(c, &[id]);
        self.apply_status_definitions(
            (source, Some(owner), Some(boon)),
            target,
            &defs,
            c,
            now,
            events,
        );
    }

    pub(super) fn apply_detached_area_statuses(
        &mut self,
        source: &EntityRef,
        target: EntityRef,
        defs: &[crate::config::StatusDefinition],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if defs.is_empty()
            || source.kind != StatusEntityKind::Monster
            || target.kind != StatusEntityKind::Participant
            || defs.iter().any(|d| !d.retained_source)
        {
            return;
        }
        self.apply_status_definitions(
            (status_entity(source), None, None),
            target,
            defs,
            c,
            now,
            events,
        );
    }

    fn apply_status_definitions(
        &mut self,
        (source, owner, boon): (StatusEntity, Option<Owner>, Option<&str>),
        target: EntityRef,
        defs: &[crate::config::StatusDefinition],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        self.advance_attack_clocks(c, now);
        let Some(max_hp) = self.entity_max_hp(&target, c, now) else {
            return;
        };
        let target = status_entity(&target);
        if self.periodic_enabled
            && (owner.is_some() && !self.valid_status_entity(&source)
                || !self.valid_status_entity(&target)
                || defs.iter().any(|d| {
                    if c.combat_rules.is_some() {
                        let invalid_flags = !d.flags.is_empty()
                            && if d.is_hostile_control() {
                                self.status_relation(&source, &target)
                                    != Some(super::combat::Relation::Hostile)
                            } else {
                                source != target
                                    && self.status_relation(&source, &target)
                                        != Some(super::combat::Relation::Friendly)
                            };
                        return invalid_flags
                            || d.is_heal()
                                && source != target
                                && !self.is_member_ally(&source, &target)
                            || d.is_periodic()
                                && !d.is_heal()
                                && self.status_relation(&source, &target)
                                    != Some(super::combat::Relation::Hostile)
                            || d.attribute == Some(StatusAttribute::SpiritRecoveryLock)
                                && target.kind != StatusEntityKind::Participant;
                    }
                    d.is_heal()
                        && source != target
                        && (c.mechanism_version < 79
                            || !self.is_member_ally(&source, &target)
                            || d.overheal_shield.is_some() && c.mechanism_version < 82)
                        || d.is_shield()
                            && self.status_relation(&source, &target)
                                == Some(super::combat::Relation::Friendly)
                            && !(c.mechanism_version >= 80 && self.is_member_ally(&source, &target))
                        || d.is_periodic()
                            && !d.is_heal()
                            && self.status_relation(&source, &target)
                                != Some(super::combat::Relation::Hostile)
                        || d.attribute == Some(StatusAttribute::SpiritRecoveryLock)
                            && (target.kind != StatusEntityKind::Participant
                                || source != target
                                    && (c.mechanism_version < 74
                                        || source.kind != StatusEntityKind::Monster))
                        || d.attribute == Some(StatusAttribute::Silence)
                            && (c.mechanism_version < 83
                                || source.kind != StatusEntityKind::Monster
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::Root)
                            && (c.mechanism_version < 88
                                || source.kind != StatusEntityKind::Monster
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::Stun)
                            && (c.mechanism_version < 89
                                || source.kind != StatusEntityKind::Monster
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::MovementSlow)
                            && (c.mechanism_version < 90
                                || source.kind != StatusEntityKind::Monster
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::MovementHaste)
                            && (c.mechanism_version < 91
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::CooldownReduction)
                            && (c.mechanism_version < 92
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::ArmorPenetration)
                            && (c.mechanism_version < 93
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::CriticalChance)
                            && (c.mechanism_version < 94
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::CriticalDamageBonus)
                            && (c.mechanism_version < 95
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::DodgePenalty)
                            && (c.mechanism_version < 96
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::DodgeChance)
                            && (c.mechanism_version < 97
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::Accuracy)
                            && (c.mechanism_version < 98
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::SkillPowerBonus)
                            && (c.mechanism_version < 104
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::ControlImmunity)
                            && (c.mechanism_version < 84
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::DamageImmunity)
                            && (c.mechanism_version < 100
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::ControlDurationReduction)
                            && (c.mechanism_version < 85
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                        || d.attribute == Some(StatusAttribute::FirstControlImmunity)
                            && (c.mechanism_version < 86
                                || source != target
                                || target.kind != StatusEntityKind::Participant)
                }))
        {
            return;
        }
        if c.combat_rules.is_some()
            && defs.iter().any(crate::config::hard_control)
            && self.status_relation(&source, &target) != Some(super::combat::Relation::Hostile)
        {
            return;
        }
        let resolved = self.resolve_control_definitions(&target, defs, c);
        let defs = resolved.as_ref();
        let attack = owner
            .and_then(|_| self.entity_attack(&source, c, now))
            .unwrap_or(0);
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        let rules = c.pve.as_ref().unwrap().status_rules.as_ref().unwrap();
        let unblocked;
        let defs = if let Some(combat) = &c.combat_rules {
            let (blocked, allowed): (Vec<_>, Vec<_>) = defs.iter().cloned().partition(|d| {
                crate::config::status_blocked(
                    &self.statuses,
                    &d.id,
                    &target,
                    &combat.status_policies,
                    now,
                )
            });
            if !blocked.is_empty() {
                raw.push(Event::StatusRejected {
                    source: source.clone(),
                    target: target.clone(),
                    definition_ids: blocked.into_iter().map(|d| d.id).collect(),
                    reason: Error::Immune,
                });
            }
            unblocked = allowed;
            &unblocked
        } else {
            defs
        };
        let has_control = c.mechanism_version >= 84
            && defs
                .iter()
                .any(crate::config::StatusDefinition::is_hostile_control);
        let immune = has_control
            && self.statuses.entries().iter().any(|s| {
                s.target == target
                    && s.has_attribute(StatusAttribute::ControlImmunity)
                    && now < s.expires_at_ms
            });
        let first_immunity = if has_control && !immune && c.mechanism_version >= 86 {
            self.statuses
                .entries()
                .iter()
                .filter(|s| {
                    s.target == target
                        && s.has_attribute(StatusAttribute::FirstControlImmunity)
                        && now < s.expires_at_ms
                })
                .min_by_key(|s| (s.expires_at_ms, s.applied_at_ms, s.instance_id.as_str()))
                .map(|s| s.instance_id.clone())
        } else {
            None
        };
        let consumed_first = first_immunity.is_some();
        let applicable;
        let defs = if immune || first_immunity.is_some() {
            let (blocked, allowed): (Vec<_>, Vec<_>) = defs
                .iter()
                .cloned()
                .partition(crate::config::StatusDefinition::is_hostile_control);
            raw.push(Event::StatusRejected {
                source: source.clone(),
                target: target.clone(),
                definition_ids: blocked.into_iter().map(|d| d.id).collect(),
                reason: Error::Immune,
            });
            if let Some(instance_id) = first_immunity {
                self.statuses.remove(
                    StatusRemovalReason::Depleted,
                    |s| s.instance_id == instance_id,
                    &mut raw,
                );
            }
            applicable = allowed;
            &applicable
        } else {
            defs
        };
        // A compound first-immunity instance can carry temporary maxima. Remove
        // that contribution before sampling the remaining shield/HOT definitions.
        let max_hp = if consumed_first && c.combat_rules.is_some() {
            self.reconcile_caps(c, now);
            self.entity_max_hp(&Self::status_reference(&target), c, now)
                .unwrap()
        } else {
            max_hp
        };
        // Control immunity rejects the control definition; duration scaling only
        // sees the remaining definitions. The two independent factors multiply.
        let shortened;
        let defs = if c.combat_rules.is_some() || has_control && c.mechanism_version >= 85 {
            let modifier = |a| self.statuses.attribute(&target, 10000, now, Some(rules), a) - 10000;
            let control = modifier(StatusAttribute::ControlDurationReduction);
            let harmful = if c.combat_rules.is_some() {
                modifier(StatusAttribute::HarmfulDurationReduction)
            } else {
                0
            };
            shortened = defs
                .iter()
                .cloned()
                .map(|mut d| {
                    let a = if d.is_hostile_control() { control } else { 0 };
                    let b = if d.is_harmful() { harmful } else { 0 };
                    d.duration_ms =
                        (u128::from(d.duration_ms) * u128::from(10000 - a) * u128::from(10000 - b)
                            / 100_000_000)
                            .max(1) as u64;
                    d
                })
                .collect::<Vec<_>>();
            &shortened
        } else {
            defs
        };
        let configured;
        let defs = if let (Some(combat), Some(owner)) = (&c.combat_rules, owner) {
            configured = defs
                .iter()
                .cloned()
                .map(|mut d| {
                    if let Some(rule) = combat.status_cleanses.get(&d.id) {
                        d.cleanse = Some(StatusCleanseView {
                            on_tick: rule.on_tick.is_some(),
                            on_expired: rule.on_expired.is_some(),
                        });
                    }
                    if let Some(rule) = combat.status_windows.get(&d.id) {
                        d.window = Some(WindowRecordView {
                            basis: rule.basis,
                            recorded: 0,
                            cap: rule.record_cap,
                            contacts: 0,
                            maximum_sampling: rule.maximum_sampling,
                            max_hp_at_apply: max_hp,
                            max_spirit_at_apply: self
                                .entity_max_spirit(&Self::status_reference(&target), c, now)
                                .unwrap(),
                        });
                        d.window_preserve =
                            matches!(rule.refresh, crate::config::RecordRefresh::Preserve);
                    }
                    self.configure_shield_record(&mut d, owner, c, now);
                    d.periodic_sampling = combat.periodic_sampling.get(&d.id).copied();
                    let sampled_attack =
                        if d.periodic_sampling.is_some_and(PeriodicSampling::target) {
                            self.entity_attack(&target, c, now).unwrap()
                        } else {
                            self.status_source_power(&self.statuses, owner, &d.id, c, now)
                                .unwrap_or(attack)
                        };
                    if d.periodic_stack.is_some() || combat.delayed_damage.contains_key(&d.id) {
                        d.periodic_inputs = Some(crate::status::PeriodicInputs {
                            attack: sampled_attack,
                            max_hp,
                            max_spirit: self
                                .entity_max_spirit(&Self::status_reference(&target), c, now)
                                .unwrap(),
                        });
                    }
                    if d.is_periodic() && !d.is_heal() {
                        d.periodic_raw_damage = Some(
                            (u64::from(sampled_attack) * u64::from(d.damage_bps.unwrap()) / 10000)
                                .min(u64::from(u32::MAX)) as u32,
                        );
                    }
                    if let Some(rule) = combat.periodic_damage.get(&d.id) {
                        d.periodic_damage_kind = Some(rule.damage_kind);
                        d.periodic_raw_damage =
                            Some(rule.budget(sampled_attack, d.damage_bps.unwrap(), 10000, 0));
                    }
                    // This view reports the last evaluated amount; no target deficit is frozen.
                    if combat.delayed_damage.contains_key(&d.id) {
                        d.periodic_raw_damage = Some(0);
                    }
                    if let Some(rule) = combat.periodic_recovery.get(&d.id) {
                        let maximum = self
                            .entity_max_spirit(&Self::status_reference(&target), c, now)
                            .unwrap();
                        d.periodic_raw_spirit = Some(rule.amount(sampled_attack, maximum));
                    }
                    if let Some(formula) = combat.healing.get(&d.id) {
                        d.periodic_raw_heal = Some(formula.amount(sampled_attack, max_hp));
                    }
                    d
                })
                .collect::<Vec<_>>();
            &configured
        } else {
            defs
        };
        self.statuses.set_boon_context(boon);
        self.statuses.apply(
            defs,
            (&source, attack),
            (&target, max_hp),
            now,
            rules,
            &mut raw,
        );
        self.statuses.set_boon_context(None);
        self.reconcile_caps(c, now);
        if self.periodic_enabled {
            let applied_control = |attribute| {
                raw.iter().any(|event| {
                    matches!(event,
                        Event::StatusApplied { status } | Event::StatusRefreshed { status }
                            if status.has_attribute(attribute)
                    )
                })
            };
            let silenced = applied_control(StatusAttribute::Silence);
            let rooted = c.mechanism_version >= 88 && applied_control(StatusAttribute::Root);
            let stunned = c.mechanism_version >= 89 && applied_control(StatusAttribute::Stun);
            self.route_status_events(raw, &before, events);
            self.apply_control_consequences(&target, (silenced, rooted, stunned), c, now, events);
        } else if let Some(Owner::Member(i)) = owner {
            for event in raw {
                private(events, i, event);
            }
        }
    }
    pub(super) fn apply_control_consequences(
        &mut self,
        target: &StatusEntity,
        (silenced, rooted, stunned): (bool, bool, bool),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if c.combat_rules.is_some()
            && (silenced || stunned)
            && let Some(Owner::Npc(j)) = self.entity_owner(target)
        {
            self.npcs[j].casting = None;
            self.stop_npc_areas(
                &Self::status_reference(target),
                AreaEndReason::Cancelled,
                events,
            );
        }
        if (silenced || rooted || stunned)
            && let Some(Owner::Member(i)) = self.entity_owner(target)
        {
            if stunned {
                self.schedule_basic_beam_end(i, BasicBeamEndReason::Interrupted);
                self.finish_channel(i, ChannelEndReason::Interrupted, now, events);
                self.interrupt(i, events);
            } else {
                if silenced {
                    self.interrupt_silenced_skill(i, now, events);
                }
                if rooted {
                    self.interrupt_rooted_movement(i, events);
                }
            }
        }
    }
    pub(super) fn clear_owned_statuses(
        &mut self,
        entity: &EntityRef,
        reason: StatusRemovalReason,
        events: &mut Events,
    ) {
        self.triggers.clear(entity);
        let entity = status_entity(entity);
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        if self.periodic_enabled {
            self.statuses
                .remove(reason, |s| s.target == entity, &mut raw);
            self.statuses.remove(
                if reason == StatusRemovalReason::Reset {
                    reason
                } else {
                    StatusRemovalReason::SourceInvalid
                },
                |s| s.source == entity && s.retained_source.is_none(),
                &mut raw,
            );
            self.route_status_events(raw, &before, events);
        } else {
            self.statuses.remove(
                reason,
                |s| s.target == entity || s.source == entity,
                &mut raw,
            );
        }
    }
    pub(super) fn prepare_periodic(&mut self, now: u64, events: &mut Events) {
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        self.statuses.expire(now, &mut raw);
        let mut live = vec![];
        live.extend(
            self.members
                .iter()
                .filter(|m| m.living())
                .map(|m| status_entity(&m.reference.entity)),
        );
        live.extend(
            self.npcs
                .iter()
                .filter(|n| n.hp > 0)
                .map(|n| status_entity(&n.entity)),
        );
        live.extend(
            self.summons
                .iter()
                .enumerate()
                .filter(|(i, _)| self.summon_alive(*i))
                .map(|(_, s)| status_entity(&s.entity)),
        );
        self.statuses.remove(
            StatusRemovalReason::TargetInvalid,
            |s| !live.contains(&s.target),
            &mut raw,
        );
        self.statuses.remove(
            StatusRemovalReason::SourceInvalid,
            |s| !live.contains(&s.source) && s.retained_source.is_none(),
            &mut raw,
        );
        self.route_status_events(raw, &before, events);
    }
    pub(super) fn periodic_outcome(
        &self,
        source: &StatusEntity,
        target: &StatusEntity,
    ) -> PeriodicTickOutcome {
        self.periodic_target_eligibility(source, target, false)
    }
    fn periodic_target_eligibility(
        &self,
        source: &StatusEntity,
        target: &StatusEntity,
        deals_damage: bool,
    ) -> PeriodicTickOutcome {
        let entities = [self.entity_owner(source), self.entity_owner(target)];
        if entities.iter().any(|entity| match entity {
            Some(Owner::Member(i)) => !self.members[*i].eligible(),
            Some(Owner::Summon(i)) => !self.summon_eligible(*i),
            _ => false,
        }) {
            PeriodicTickOutcome::Protected
        } else if entities.iter().enumerate().any(|(role, e)| {
            matches!(e,Some(Owner::Npc(i)) if if self.grid.config.npc_ai.is_some() {
                !self.npc_can_be_attacked(*i) || self.npcs[*i].returning && (role == 0 || !deals_damage)
            } else { self.npcs[*i].returning })
        }) {
            PeriodicTickOutcome::Returning
        } else {
            PeriodicTickOutcome::Applied
        }
    }
    fn periodic_sample(
        &self,
        status: &StatusView,
        c: &Config,
        now: u64,
        layers: u32,
    ) -> (u32, Option<u32>, Option<u32>) {
        let periodic = status.periodic.as_ref().unwrap();
        let live = periodic.power_sampling.is_some_and(|s| s.live());
        if let Some(r) = &c.combat_rules
            && let Some(formula) = r.delayed_damage.get(&status.definition_id)
        {
            let power = if live {
                let owner = self.entity_owner(&status.source).unwrap();
                self.status_source_power(&self.statuses, owner, &status.definition_id, c, now)
                    .unwrap_or_else(|| self.entity_attack(&status.source, c, now).unwrap())
            } else {
                self.statuses
                    .periodic_inputs(&status.instance_id)
                    .expect("delayed source snapshot retained")
                    .attack
            };
            let reference = Self::status_reference(&status.target);
            let maximum = self.entity_max_hp(&reference, c, now).unwrap();
            let owner = self.entity_owner(&status.target).unwrap();
            let hp = self.current_hp(owner);
            let profile = self.combat_profile_id(owner, r);
            return (
                formula.amount(power, maximum.saturating_sub(hp), profile),
                None,
                None,
            );
        }

        if !live && periodic.per_stack.is_none() {
            return (periodic.raw_damage, periodic.raw_heal, periodic.raw_spirit);
        }
        let inputs = if live {
            let sampled = if periodic.power_sampling.unwrap().target() {
                &status.target
            } else {
                &status.source
            };
            crate::status::PeriodicInputs {
                attack: self
                    .entity_owner(sampled)
                    .and_then(|o| {
                        self.status_source_power(&self.statuses, o, &status.definition_id, c, now)
                    })
                    .unwrap_or_else(|| self.entity_attack(sampled, c, now).unwrap()),
                max_hp: self
                    .entity_max_hp(&Self::status_reference(&status.target), c, now)
                    .unwrap(),
                max_spirit: self
                    .entity_max_spirit(&Self::status_reference(&status.target), c, now)
                    .unwrap(),
            }
        } else {
            self.statuses
                .periodic_inputs(&status.instance_id)
                .expect("live layered status retains its private inputs")
        };
        let combat = c
            .combat_rules
            .as_ref()
            .expect("extended periodic sampling requires combat rules");
        let definition = c
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .iter()
            .find(|d| d.id == status.definition_id)
            .unwrap();
        let scaled = |base: u32, bps: u32| {
            ((u128::from(base) * u128::from(bps) * u128::from(layers)) / 10000)
                .min(u128::from(u32::MAX)) as u32
        };
        if definition.is_heal() {
            let amount = combat.healing.get(&definition.id).map_or_else(
                || scaled(inputs.max_hp, definition.heal_bps.unwrap()),
                |f| f.amount_layers(inputs.attack, inputs.max_hp, layers),
            );
            let spirit = combat.periodic_recovery.get(&definition.id).map(|rule| {
                rule.spirit.as_ref().map_or(0, |f| {
                    f.amount_layers(inputs.attack, inputs.max_spirit, layers)
                })
            });
            (0, Some(amount), spirit)
        } else {
            let amount = combat.periodic_damage.get(&definition.id).map_or_else(
                || scaled(inputs.attack, definition.damage_bps.unwrap()),
                |rule| {
                    rule.budget_layers(
                        inputs.attack,
                        definition.damage_bps.unwrap(),
                        10000,
                        0,
                        layers,
                    )
                },
            );
            (amount, None, None)
        }
    }
    pub(super) fn advance_periodic(&mut self, c: &Config, now: u64, events: &mut Events) {
        let mut reached_inclusive_end = false;
        while let Some((status, due)) = self.statuses.take_periodic_due(now) {
            let p = status.periodic.as_ref().unwrap();
            reached_inclusive_end |= p.tick_on_expiry == Some(true) && due == status.expires_at_ms;
            if !self.valid_status_entity(&status.source)
                || !self.valid_status_entity(&status.target)
            {
                continue;
            }
            let derived = self.statuses.is_derived(&status.instance_id);
            let previous_origin = std::mem::replace(
                &mut self.triggers.origin,
                self.statuses.action_origin(&status.instance_id).cloned(),
            );
            let previous_source = self
                .triggers
                .action_source
                .replace(Self::status_reference(&status.source));
            let prior_guards = (
                self.triggers.executing,
                self.record_shields.executing,
                self.window_records.executing,
            );
            if derived {
                self.triggers.executing = true;
                self.record_shields.executing = true;
                self.window_records.executing = true;
            }
            let base = self.periodic_sample(&status, c, now, 1);
            let (raw_damage, raw_heal, raw_spirit) = if let Some(layers) = status.stacks {
                self.periodic_sample(&status, c, now, layers)
            } else {
                base
            };
            self.statuses
                .record_periodic_sample(&status.instance_id, base.0, base.1, base.2);
            let outcome = self.periodic_target_eligibility(
                &status.source,
                &status.target,
                p.kind != PeriodicEffectKind::MaxHpHeal,
            );
            let mut raw = vec![];
            let mut overheal = None;
            let mut death_burst = None;
            match p.kind {
                PeriodicEffectKind::PhysicalDamage
                | PeriodicEffectKind::ElementalDamage
                | PeriodicEffectKind::TrueDamage => {
                    let mut result = DamageResult::default();
                    if outcome == PeriodicTickOutcome::Applied {
                        let inputs = self.statuses.periodic_inputs(&status.instance_id);
                        result = self.resolve_periodic_damage(
                            super::combat::DamageIntent {
                                ignore_armor: false,
                                source: Self::status_reference(&status.source),
                                target: Self::status_reference(&status.target),
                                raw: raw_damage,
                                direct: false,
                            },
                            c.combat_rules
                                .as_ref()
                                .and_then(|r| r.periodic_damage.get(&status.definition_id)),
                            c,
                            now,
                            events,
                        );
                        if result.lethal {
                            death_burst = inputs;
                        }
                    }
                    raw.push(Event::PeriodicTick {
                        stacks: status.stacks,
                        instance_id: status.instance_id.clone(),
                        definition_id: status.definition_id.clone(),
                        source: status.source.clone(),
                        target: status.target.clone(),
                        tick_index: p.ticks_done,
                        scheduled_at_ms: due,
                        executed_at_ms: now,
                        raw_damage,
                        absorbed: result.absorbed,
                        hp_damage: result.hp_damage,
                        outcome,
                    });
                }
                PeriodicEffectKind::MaxHpHeal => {
                    let raw_heal = raw_heal.unwrap();
                    let mut hp_restored = 0;
                    let mut spirit_event = None;
                    let self_heal = status.source == status.target;
                    if outcome == PeriodicTickOutcome::Applied
                        && (self_heal
                            || c.mechanism_version >= 79
                                && self.is_member_ally(&status.source, &status.target)
                            || c.combat_rules.as_ref().is_some_and(|r| {
                                r.status_policies
                                    .get(&status.definition_id)
                                    .is_some_and(|p| p.transferable)
                            }))
                    {
                        let requested = if c.mechanism_version >= 69
                            && (c.combat_rules.is_some()
                                || status.target.kind == StatusEntityKind::Participant)
                        {
                            self.adjusted_heal(&status.target, u64::from(raw_heal), c, now)
                        } else {
                            u64::from(raw_heal)
                        };
                        let full_before =
                            raw_spirit.is_some() && self.periodic_full_hp(&status.target, c, now);
                        hp_restored = self.restore_hp(
                            &Self::status_reference(&status.target),
                            u32::try_from(requested).unwrap_or(u32::MAX),
                            c,
                            now,
                        );
                        let mut excess = requested - u64::from(hp_restored);
                        if let Some(amount) = raw_spirit {
                            let (remaining, event) = self.resolve_periodic_spirit(
                                &status,
                                (amount, excess, full_before, true),
                                (due, now, outcome),
                                c,
                            );
                            excess = remaining;
                            spirit_event = Some(event);
                        }
                        if c.mechanism_version >= 76
                            && (self_heal || c.mechanism_version >= 82)
                            && excess > 0
                            && status.target.kind == StatusEntityKind::Participant
                            && let Some(conversion) = c
                                .pve
                                .as_ref()
                                .unwrap()
                                .status_definitions
                                .iter()
                                .find(|d| d.id == status.definition_id)
                                .and_then(|d| d.overheal_shield.as_ref())
                        {
                            overheal = Some((conversion, excess));
                        }
                    }
                    raw.push(Event::PeriodicHealTick {
                        stacks: status.stacks,
                        instance_id: status.instance_id.clone(),
                        definition_id: status.definition_id.clone(),
                        source: status.source.clone(),
                        target: status.target.clone(),
                        tick_index: p.ticks_done,
                        scheduled_at_ms: due,
                        executed_at_ms: now,
                        raw_heal,
                        hp_restored,
                        outcome,
                    });
                    if let Some(event) = spirit_event {
                        raw.push(event);
                    } else if let Some(amount) = raw_spirit {
                        let (_, event) = self.resolve_periodic_spirit(
                            &status,
                            (amount, 0, false, false),
                            (due, now, outcome),
                            c,
                        );
                        raw.push(event);
                    }
                }
            }
            self.route_status_events(raw, &[], events);
            if let Some(inputs) = death_burst {
                self.periodic_death_burst(&status, inputs.attack, c, now, events);
            }
            if let Some((conversion, excess)) = overheal {
                let target = &status.target;
                let max_hp = self
                    .entity_max_hp(&Self::status_reference(target), c, now)
                    .unwrap();
                self.convert_resolved_overheal(
                    c,
                    conversion,
                    (&status.source, target, max_hp),
                    now,
                    excess,
                    events,
                );
            }
            if matches!(p.kind, PeriodicEffectKind::MaxHpHeal) {
                self.cleanse_status_tick(&status, due, c, now, events);
            }
            (
                self.triggers.executing,
                self.record_shields.executing,
                self.window_records.executing,
            ) = prior_guards;
            self.triggers.origin = previous_origin;
            self.triggers.action_source = previous_source;
            if !self.members.iter().any(Member::living) {
                break;
            }
        }
        if reached_inclusive_end {
            // Inclusive carriers finish their final tick before expiry. Drain their end
            // effects now, before a same-step cast or movement can observe stale state.
            let before = self.statuses.entries().to_vec();
            let mut removed = vec![];
            self.statuses.expire(now, &mut removed);
            self.route_status_events(removed, &before, events);
            self.finish_status_cleanses(c, now, events);
            self.finish_window_records(c, now, events);
            self.finish_status_ends(c, now, events);
            self.reconcile_caps(c, now);
        }
    }
    #[cfg(all(test, match_sdk_test_data))]
    pub(super) fn hurt_npc(
        &mut self,
        j: usize,
        i: usize,
        raw: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> u32 {
        self.hurt_npc_effect(j, i, raw, c, now, events).hp_damage
    }
    pub(super) fn absorb_damage(
        &mut self,
        target: &EntityRef,
        (source, allow_boon_shield): (&str, bool),
        amount: u32,
        now: u64,
        events: &mut Events,
    ) -> (u32, u32) {
        if !self.shields_enabled {
            return (amount, 0);
        }
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        let left = self.statuses.absorb_recording(
            &status_entity(target),
            amount,
            now,
            (
                source,
                !self.record_shields.executing && !self.triggers.executing,
                allow_boon_shield,
            ),
            &mut raw,
        );
        self.route_status_events(raw, &before, events);
        (left, amount - left)
    }
    pub(super) fn hurt_member_effect(
        &mut self,
        i: usize,
        source: EntityRef,
        hit: (u32, bool),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        self.resolve_damage(
            super::combat::DamageIntent {
                ignore_armor: false,
                source,
                target: self.members[i].reference.entity.clone(),
                raw: hit.0,
                direct: hit.1,
            },
            c,
            now,
            events,
        )
    }
    #[cfg(all(test, match_sdk_test_data))]
    pub(super) fn hurt_npc_effect(
        &mut self,
        j: usize,
        i: usize,
        raw: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        self.resolve_damage(
            super::combat::DamageIntent {
                ignore_armor: false,
                source: self.members[i].reference.entity.clone(),
                target: self.npcs[j].entity.clone(),
                raw,
                direct: true,
            },
            c,
            now,
            events,
        )
    }
    pub(super) fn visible_npc_statuses(
        &self,
        i: usize,
        target: &EntityRef,
        now: u64,
    ) -> Vec<StatusView> {
        let owner = status_entity(&self.members[i].reference.entity);
        self.statuses
            .views(&status_entity(target), now)
            .into_iter()
            .filter(|s| {
                s.source == owner
                    || s.source == s.target
                    || matches!(self.resolve_entity(target), Some(Owner::Summon(j)) if
                        self.summons[j].member == i
                            && matches!(self.entity_owner(&s.source), Some(Owner::Npc(_))))
            })
            .collect()
    }
}
