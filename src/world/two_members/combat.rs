//! Entity identity, PVE relation policy and one shared effect resolver.
//! Skill plans select contact; this module alone applies combat HP changes.
use super::periodic::DamageResult;
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Relation {
    SelfTarget,
    Friendly,
    Hostile,
}
pub(super) struct DirectContact<'a> {
    pub origin: Option<crate::origin::ActionOrigin>,
    pub action_id: Option<&'a str>,
    pub rule: Option<&'a crate::config::AttackRule>,
    pub ignore_armor: bool,
    pub unavoidable: bool,
    /// end_on_source_invalid shot: settle the frozen payload even though the
    /// firing member has settled (the shot itself stays the authority).
    pub dead_source: bool,
    pub source: &'a EntityRef,
    pub raw: u32,
    pub metal: bool,
    pub hit_statuses: &'a [String],
}
pub(super) struct DamageIntent {
    pub ignore_armor: bool,
    pub source: EntityRef,
    pub target: EntityRef,
    pub raw: u32,
    pub direct: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DamageMode<'a> {
    DetachedShot,
    Ordinary,
    Periodic,
    Transfer,
    BasicField,
    GuardReflection,
    DetachedArea,
    SummonDeath,
    Terrain(&'a str),
    Deadline,
}
pub(super) fn dodge_probability(dodge_bps: u64, penalty_bps: u32, hit_bps: u64) -> f64 {
    let remaining_bps = dodge_bps.saturating_sub(u64::from(penalty_bps));
    let denominator = 10000 + hit_bps;
    (remaining_bps as f64 / denominator as f64)
        .min(f64::from(crate::config::MAX_DODGE_BPS) / 10000.0)
}

impl World {
    pub(super) fn resolve_deadline_damage(
        &mut self,
        entity: EntityRef,
        raw: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        self.resolve_damage_delivery(
            DamageIntent {
                source: entity.clone(),
                target: entity,
                raw,
                direct: false,
                ignore_armor: true,
            },
            None,
            DamageMode::Deadline,
            c,
            now,
            events,
        );
    }

    pub(super) fn advance_terrain_damage(&mut self, c: &Config, now: u64, events: &mut Events) {
        // Rate is milli-HP per second, time is milliseconds, reduction is in basis points.
        const DAMAGE_UNITS_PER_HP: u64 = 10_000_000_000;
        if !self.grid.terrain_damage_enabled {
            return;
        }
        let mut previous = std::mem::take(&mut self.terrain_damage_remainders);
        let actors = (0..self.members.len())
            .map(Owner::Member)
            .chain((0..self.npcs.len()).map(Owner::Npc))
            .chain((0..self.summons.len()).map(Owner::Summon));
        for actor in actors {
            let position = match actor {
                Owner::Member(i) => self.members[i].position,
                Owner::Npc(i) => self.npcs[i].position,
                Owner::Summon(i) => self.summons[i].position,
            };
            let Some(tile) = self.grid.terrain_at(position) else {
                continue;
            };
            let rate = tile.damage_milli_per_second();
            if rate == 0
                || !self.contact_eligible(actor)
                || self.damage_immune(actor, c, now)
                || matches!(actor, Owner::Npc(i) if self.npcs[i].returning)
            {
                continue;
            }
            let target = self.owner_entity(actor);
            let key = super::triggers::entity_key(&target);
            let reduction = self.effective_damage_reduction(actor, &target, c, now);
            let damage_bps = u64::from(10000u32.saturating_sub(reduction));
            let units = rate * c.step_ms * damage_bps + previous.remove(&key).unwrap_or(0);
            let raw = (units / DAMAGE_UNITS_PER_HP) as u32;
            if raw > 0 {
                let terrain = tile.terrain.clone();
                self.resolve_damage_delivery(
                    DamageIntent {
                        // Environmental delivery has no combat source. The shared intent
                        // stores the target here only to keep other delivery call sites intact.
                        source: target.clone(),
                        target,
                        raw,
                        direct: false,
                        ignore_armor: true,
                    },
                    None,
                    DamageMode::Terrain(&terrain),
                    c,
                    now,
                    events,
                );
            }
            let remainder = units % DAMAGE_UNITS_PER_HP;
            if remainder > 0 && self.contact_eligible(actor) {
                self.terrain_damage_remainders.insert(key, remainder);
            }
        }
    }

    fn damage_immune(&self, target: Owner, c: &Config, now: u64) -> bool {
        c.combat_rules.is_some()
            && self.entity_has_status(target, StatusAttribute::DamageImmunity, now)
            || matches!(target, Owner::Member(i) if self.member_damage_immune(i, c, now))
    }
    pub(super) fn resolve_entity(&self, e: &EntityRef) -> Option<Owner> {
        match e.kind {
            StatusEntityKind::Participant => self
                .members
                .iter()
                .position(|m| m.reference.entity == *e)
                .map(Owner::Member),
            StatusEntityKind::Monster => self
                .npcs
                .iter()
                .position(|n| n.entity == *e)
                .map(Owner::Npc)
                .or_else(|| {
                    self.summons
                        .iter()
                        .position(|s| s.entity == *e)
                        .map(Owner::Summon)
                }),
        }
    }
    pub(super) fn relation(&self, source: &EntityRef, target: &EntityRef) -> Option<Relation> {
        let a = self.resolve_entity(source)?;
        let b = self.resolve_entity(target)?;
        if !self.same_training_scope(a, b) {
            return None;
        }
        let seat = |owner| match owner {
            Owner::Member(i) => Some(i),
            Owner::Summon(i) => Some(self.summons[i].member),
            Owner::Npc(_) => None,
        };
        if let Owner::Npc(j) = a
            && !self.pursuit_allows_target(j, b)
        {
            return None;
        }
        Some(if a == b {
            Relation::SelfTarget
        } else if (self.member_conflict || self.mode_opposed())
            && let (Some(a), Some(b)) = (seat(a), seat(b))
        {
            if a == b {
                Relation::Friendly
            } else {
                Relation::Hostile
            }
        } else if matches!(a, Owner::Npc(_)) == matches!(b, Owner::Npc(_)) {
            Relation::Friendly
        } else {
            Relation::Hostile
        })
    }

    pub(super) fn hostile_unit_ref(&self, target: &UnitTarget) -> EntityRef {
        let participant = (self.member_conflict || self.mode_opposed())
            && self
                .members
                .iter()
                .any(|m| m.reference.entity.id == target.id);
        EntityRef {
            kind: if participant {
                StatusEntityKind::Participant
            } else {
                StatusEntityKind::Monster
            },
            id: target.id.clone(),
            generation: target.generation,
        }
    }

    pub(super) fn hostile_unit_position(&self, i: usize, target: &EntityRef) -> Result<[f64; 2]> {
        if !self.hostile_contact(&self.members[i].reference.entity, target) {
            return Err(Error::InvalidTarget);
        }
        match self.resolve_entity(target) {
            Some(owner @ (Owner::Npc(_) | Owner::Member(_))) => Ok(self.actor_position(owner)),
            _ => Err(Error::InvalidTarget),
        }
    }
    pub(super) fn status_reference(e: &StatusEntity) -> EntityRef {
        EntityRef {
            kind: e.kind,
            id: e.id.clone(),
            generation: e.generation,
        }
    }
    pub(super) fn status_relation(&self, a: &StatusEntity, b: &StatusEntity) -> Option<Relation> {
        if a.encounter_round.is_some() || b.encounter_round.is_some() {
            return None;
        }
        self.relation(&Self::status_reference(a), &Self::status_reference(b))
    }
    pub(super) fn actor_position(&self, owner: Owner) -> [f64; 2] {
        match owner {
            Owner::Member(i) => self.members[i].position,
            Owner::Npc(i) => self.npcs[i].position,
            Owner::Summon(i) => self.summons[i].position,
        }
    }
    pub(super) fn current_hp(&self, owner: Owner) -> u32 {
        match owner {
            Owner::Member(i) => self.members[i].hp,
            Owner::Npc(i) => self.npcs[i].hp,
            Owner::Summon(i) => self.summons[i].hp,
        }
    }
    pub(super) fn base_hp(&self, owner: Owner, c: &Config) -> u32 {
        match owner {
            Owner::Member(i) => self.members[i].base_points(c, StatusAttribute::MaxHp),
            Owner::Npc(i) => self.npcs[i].profile.stats["hp"],
            Owner::Summon(i) => self.summons[i].base_hp,
        }
    }
    pub(super) fn base_attack(&self, owner: Owner, c: &Config) -> u32 {
        match owner {
            Owner::Member(i) => self.members[i].base_points(c, StatusAttribute::Atk),
            Owner::Npc(i) => self.npcs[i].profile.stats["atk"],
            Owner::Summon(i) => self.summons[i].base_atk,
        }
    }
    pub(super) fn combat_profile_id<'a>(
        &self,
        owner: Owner,
        rules: &'a crate::config::CombatRules,
    ) -> &'a str {
        let (profiles, id, fallback) = match owner {
            Owner::Member(i) => (
                &rules.member_profiles,
                &self.members[i].reference.owner_id,
                &rules.default_member_profile,
            ),
            Owner::Npc(i) => (
                &rules.npc_profiles,
                &self.npcs[i].template_id,
                &rules.default_npc_profile,
            ),
            Owner::Summon(i) => (
                &rules.npc_profiles,
                &self.summons[i].template_id,
                &rules.default_npc_profile,
            ),
        };
        profiles.get(id).unwrap_or(fallback)
    }
    pub(super) fn entity_max_hp(&self, e: &EntityRef, c: &Config, now: u64) -> Option<u32> {
        self.resolve_entity(e).map(|owner| {
            let base = self.base_hp(owner, c);
            if c.combat_rules.is_none() {
                base
            } else {
                self.statuses
                    .attribute(
                        &status_entity(e),
                        base,
                        now,
                        c.pve.as_ref().unwrap().status_rules.as_ref(),
                        StatusAttribute::MaxHp,
                    )
                    .max(1)
            }
        })
    }
    pub(super) fn entity_max_spirit(&self, e: &EntityRef, c: &Config, now: u64) -> Option<u32> {
        self.resolve_entity(e).map(|o| {
            let base = match o {
                Owner::Member(i) => self.members[i].base_points(c, StatusAttribute::MaxSpirit),
                Owner::Npc(j) => self.npcs[j].profile.stats["spirit"],
                Owner::Summon(_) => return 0,
            };
            if c.combat_rules.is_none() {
                base
            } else {
                self.statuses.attribute(
                    &status_entity(e),
                    base,
                    now,
                    c.pve.as_ref().unwrap().status_rules.as_ref(),
                    StatusAttribute::MaxSpirit,
                )
            }
        })
    }
    pub(super) fn member_max_spirit(&self, i: usize, c: &Config, now: u64) -> u32 {
        self.entity_max_spirit(&self.members[i].reference.entity, c, now)
            .unwrap()
    }
    pub(super) fn effective_spd(&self, owner: Owner, c: &Config, now: u64) -> u32 {
        let e = self.owner_entity(owner);
        let base = match owner {
            Owner::Member(i) => self.members[i].base_points(c, StatusAttribute::Spd),
            Owner::Npc(i) => self.npcs[i].profile.stats["spd"],
            Owner::Summon(i) => self.summons[i].profile.stats["spd"],
        };
        if c.combat_rules.is_none() {
            base
        } else {
            self.statuses.attribute(
                &status_entity(&e),
                base,
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
                StatusAttribute::Spd,
            )
        }
    }
    pub(super) fn reconcile_caps(&mut self, c: &Config, now: u64) {
        self.advance_attack_clocks(c, now);
        if c.combat_rules.is_none() {
            return;
        }
        for i in 0..self.members.len() {
            let hp = self
                .entity_max_hp(&self.members[i].reference.entity, c, now)
                .unwrap();
            let spirit = self.member_max_spirit(i, c, now);
            self.members[i].hp = self.members[i].hp.min(hp);
            self.members[i].spirit = self.members[i].spirit.min(spirit);
        }
        for j in 0..self.npcs.len() {
            let hp = self.entity_max_hp(&self.npcs[j].entity, c, now).unwrap();
            let spirit = self
                .entity_max_spirit(&self.npcs[j].entity, c, now)
                .unwrap();
            self.npcs[j].hp = self.npcs[j].hp.min(hp);
            self.npcs[j].spirit = self.npcs[j].spirit.min(spirit);
        }
        for j in 0..self.summons.len() {
            let hp = self.entity_max_hp(&self.summons[j].entity, c, now).unwrap();
            self.summons[j].hp = self.summons[j].hp.min(hp);
        }
    }
    fn contact_eligible(&self, o: Owner) -> bool {
        match o {
            Owner::Member(i) => self.members[i].eligible(),
            Owner::Npc(i) => self.npc_can_be_attacked(i),
            Owner::Summon(i) => self.summon_eligible(i),
        }
    }
    pub(super) fn hostile_contact(&self, source: &EntityRef, target: &EntityRef) -> bool {
        self.relation(source, target) == Some(Relation::Hostile)
            && self
                .resolve_entity(target)
                .is_some_and(|o| self.contact_eligible(o))
    }
    pub(super) fn projectile_origin(&self, i: usize, radius: f64) -> Result<()> {
        let p = self.members[i].position;
        if !p.iter().all(|v| v.is_finite())
            || self.grid.index(p).is_none()
            || !self.grid.clear(p, radius, 1)
        {
            Err(Error::InvalidTarget)
        } else {
            Ok(())
        }
    }
    pub(super) fn hostile_sweeps<'a>(
        &'a self,
        source: &'a EntityRef,
        segment: ([f64; 2], [f64; 2]),
        radius: f64,
        c: &'a Config,
    ) -> impl Iterator<Item = (f64, &'a EntityRef, Owner)> + 'a {
        let members = self.members.iter().enumerate().map(move |(i, m)| {
            (
                Owner::Member(i),
                &m.reference.entity,
                m.position,
                c.params.player_radius,
            )
        });
        let npcs = self.npcs.iter().enumerate().map(move |(i, n)| {
            (
                Owner::Npc(i),
                &n.entity,
                n.position,
                n.profile.combat.radius,
            )
        });
        members
            .chain(npcs)
            .chain(self.summons.iter().enumerate().map(|(i, s)| {
                (
                    Owner::Summon(i),
                    &s.entity,
                    s.position,
                    s.profile.combat.radius,
                )
            }))
            .filter(move |(_, e, _, _)| self.hostile_contact(source, e))
            .filter_map(move |(o, e, p, r)| {
                segment_circle(segment.0, segment.1, p, radius + r).map(|t| (t, e, o))
            })
    }
    pub(super) fn first_hostile_contact(
        &self,
        source: &EntityRef,
        segment: ([f64; 2], [f64; 2]),
        radius: f64,
        c: &Config,
    ) -> Option<(f64, Owner)> {
        self.hostile_sweeps(source, segment, radius, c)
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)))
            .map(|(t, _, o)| (t, o))
    }
    pub(super) fn unit_wire(e: &EntityRef) -> UnitTarget {
        UnitTarget {
            id: e.id.clone(),
            generation: e.generation,
        }
    }
    pub(super) fn unit_aim(
        &self,
        i: usize,
        skill: &SkillConfig,
        target: &EntityRef,
    ) -> Result<[f64; 2]> {
        let from = self.members[i].position;
        let to = self.hostile_unit_position(i, target)?;
        if distance(from, to) > skill.range || !self.grid.line(from, to, 0., 2) {
            return Err(Error::OutOfRange);
        }
        let direction = normalize([to[0] - from[0], to[1] - from[1]]);
        if direction == [0., 0.] && !skill.is_unit_effect() {
            return Err(Error::OutOfRange);
        }
        Ok(direction)
    }
    pub(super) fn ally_skill_target(
        &self,
        i: usize,
        skill: &SkillConfig,
        target: &EntityRef,
        c: &Config,
    ) -> Result<usize> {
        let Some(Owner::Member(j)) = self.resolve_entity(target) else {
            return Err(Error::InvalidTarget);
        };
        if !self.members[j].eligible() {
            return Err(Error::InvalidTarget);
        }
        if i == j {
            return c
                .allows_ally_self_target(&skill.id)
                .then_some(j)
                .ok_or(Error::InvalidTarget);
        }
        if self.relation(&self.members[i].reference.entity, target) != Some(Relation::Friendly) {
            return Err(Error::InvalidTarget);
        }
        let from = self.members[i].position;
        let to = self.members[j].position;
        if distance(from, to) > skill.range || !self.grid.line(from, to, 0., 2) {
            return Err(Error::OutOfRange);
        }
        Ok(j)
    }

    pub(super) fn adjusted_heal(
        &self,
        target: &StatusEntity,
        raw: u64,
        c: &Config,
        now: u64,
    ) -> u64 {
        raw * u64::from(self.healing_received_bps(target, c, now)) / 10_000
    }

    fn healing_received_bps(&self, target: &StatusEntity, c: &Config, now: u64) -> u32 {
        self.statuses.healing_received_bps(
            target,
            now,
            c.pve.as_ref().and_then(|p| p.status_rules.as_ref()),
        )
    }

    pub(super) fn resolve_skill_heal(
        &mut self,
        (source_index, target_index): (usize, usize),
        heal: &crate::config::InstantHeal,
        skill_id: &str,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let member = &self.members[target_index];
        let entity = member.reference.entity.clone();
        let target = status_entity(&entity);
        let max_hp = self.entity_max_hp(&entity, c, now).unwrap();
        let raw_heal = c
            .combat_rules
            .as_ref()
            .and_then(|r| r.healing.get(skill_id))
            .map_or_else(
                || heal.base_amount(max_hp),
                |f| {
                    f.amount(
                        self.recovery_power(Owner::Member(source_index), skill_id, c, now),
                        max_hp,
                    )
                },
            );
        let excess =
            self.resolve_skill_heal_amount((source_index, target_index), raw_heal, c, now, events);
        if let Some(conversion) = &heal.overheal_shield {
            let source = status_entity(&self.members[source_index].reference.entity);
            self.convert_resolved_overheal(
                c,
                conversion,
                (&source, &target, max_hp),
                now,
                excess,
                events,
            );
        }
    }

    pub(super) fn resolve_skill_heal_amount(
        &mut self,
        (source_index, target_index): (usize, usize),
        raw_heal: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> u64 {
        let kind = if source_index == target_index {
            HealKind::SelfSkill
        } else {
            HealKind::AllySkill
        };
        self.resolve_member_heal(
            (source_index, target_index),
            (raw_heal, kind),
            c,
            now,
            events,
        )
    }

    pub(super) fn resolve_member_heal(
        &mut self,
        (source_index, target_index): (usize, usize),
        (raw_heal, heal_kind): (u32, HealKind),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> u64 {
        let entity = self.owner_entity(Owner::Member(target_index));
        let target = status_entity(&entity);
        let multiplier_bps = self.healing_received_bps(&target, c, now);
        let adjusted_heal = u64::from(raw_heal) * u64::from(multiplier_bps) / 10_000;
        let hp_restored = self.restore_hp(
            &entity,
            u32::try_from(adjusted_heal).unwrap_or(u32::MAX),
            c,
            now,
        );
        let self_cast = source_index == target_index;
        let source = status_entity(&self.members[source_index].reference.entity);
        let event = Event::HealResolved {
            source: source.clone(),
            target: target.clone(),
            heal_kind,
            raw_heal,
            multiplier_bps,
            adjusted_heal,
            hp_restored,
        };
        if !self_cast {
            private(events, source_index, event.clone());
        }
        private(events, target_index, event);
        adjusted_heal - u64::from(hp_restored)
    }

    pub(super) fn plan_overheal_conversion(
        &self,
        c: &Config,
        conversion: &crate::config::OverhealShield,
        recipient: (&StatusEntity, &StatusEntity, u32),
        now: u64,
        excess: u64,
    ) -> Result<Option<(crate::config::StatusDefinition, crate::status::ShieldGrant)>> {
        let requested = (excess * u64::from(conversion.conversion_bps) / 10_000)
            .min(u64::from(u32::MAX)) as u32;
        if requested == 0 {
            return Ok(None);
        }
        let pve = c.pve.as_ref().unwrap();
        let shield = pve.overheal_shield_definition(conversion).unwrap();
        let Some(grant) = self.statuses.plan_overheal_shield(
            shield,
            recipient,
            now,
            pve.status_rules.as_ref().unwrap(),
            requested,
        )?
        else {
            return Ok(None);
        };
        if let Some(rules) = &c.combat_rules
            && crate::config::status_blocked(
                &self.statuses,
                &shield.id,
                recipient.1,
                &rules.status_policies,
                now,
            )
        {
            return Err(Error::Immune);
        }
        let source = self
            .entity_owner(recipient.0)
            .filter(|_| self.valid_status_entity(recipient.0))
            .ok_or(Error::InvalidTarget)?;
        let mut shield = shield.clone();
        self.configure_shield_record(&mut shield, source, c, now);
        Ok(Some((shield, grant)))
    }

    pub(super) fn apply_overheal_conversion(
        &mut self,
        planned: (crate::config::StatusDefinition, crate::status::ShieldGrant),
        source: &StatusEntity,
        target: &StatusEntity,
        now: u64,
        events: &mut Events,
    ) {
        let before = self.statuses.entries().to_vec();
        let mut raw = vec![];
        self.statuses
            .apply_overheal_shield(&planned.0, source, target, now, planned.1, &mut raw);
        self.route_status_events(raw, &before, events);
    }

    pub(super) fn convert_resolved_overheal(
        &mut self,
        c: &Config,
        conversion: &crate::config::OverhealShield,
        (source, target, max_hp): (&StatusEntity, &StatusEntity, u32),
        now: u64,
        excess: u64,
        events: &mut Events,
    ) {
        match self.plan_overheal_conversion(c, conversion, (source, target, max_hp), now, excess) {
            Ok(Some(plan)) => self.apply_overheal_conversion(plan, source, target, now, events),
            Ok(None) => {}
            Err(reason) => self.route_status_events(
                vec![Event::StatusRejected {
                    source: source.clone(),
                    target: target.clone(),
                    definition_ids: vec![conversion.status_id.clone()],
                    reason,
                }],
                &[],
                events,
            ),
        }
    }

    pub(super) fn restore_hp(
        &mut self,
        target: &EntityRef,
        requested: u32,
        c: &Config,
        now: u64,
    ) -> u32 {
        let Some(o) = self.resolve_entity(target) else {
            return 0;
        };
        let max = self.entity_max_hp(target, c, now).unwrap();
        let hp = match o {
            Owner::Member(i) => &mut self.members[i].hp,
            Owner::Npc(i) => &mut self.npcs[i].hp,
            Owner::Summon(i) => &mut self.summons[i].hp,
        };
        if *hp == 0 {
            return 0;
        }
        if c.combat_rules.is_some() {
            *hp = (*hp).min(max);
        }
        let restored = requested.min(max.saturating_sub(*hp));
        *hp += restored;
        restored
    }
    pub(super) fn member_damage_immune(&self, i: usize, c: &Config, now: u64) -> bool {
        c.mechanism_version >= 100
            && self.statuses.entries().iter().any(|status| {
                status.target == status_entity(&self.members[i].reference.entity)
                    && status.has_attribute(StatusAttribute::DamageImmunity)
                    && now < status.expires_at_ms
            })
    }

    pub(super) fn member_dodges_npc_contact(
        &mut self,
        i: usize,
        npc: usize,
        c: &Config,
        now: u64,
    ) -> bool {
        if let Some(rules) = &c.combat_rules {
            if rules
                .npc_attacks
                .get(&self.npcs[npc].template_id)
                .is_some_and(|r| r.unavoidable)
            {
                return false;
            }
            return self.configured_dodge(Owner::Npc(npc), Owner::Member(i), c, now);
        }
        if c.mechanism_version < 97 {
            return false;
        }
        let chance = self.statuses.attribute(
            &status_entity(&self.members[i].reference.entity),
            10000,
            now,
            c.pve.as_ref().and_then(|p| p.status_rules.as_ref()),
            StatusAttribute::DodgeChance,
        ) - 10000;
        let hit = if c.mechanism_version >= 99 {
            self.npcs[npc]
                .profile
                .stats
                .get("hit")
                .copied()
                .unwrap_or(0)
        } else {
            0
        };
        self.roll(dodge_probability(
            u64::from(chance),
            0,
            u64::from(hit) * 100,
        ))
    }

    pub(super) fn direct_contact(
        &mut self,
        owner: Owner,
        target: Owner,
        hit: DirectContact<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        self.direct_contact_damage(owner, target, hit, c, now, events)
            .is_some()
    }
    pub(super) fn direct_contact_damage(
        &mut self,
        owner: Owner,
        target: Owner,
        hit: DirectContact<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<u32> {
        if c.combat_rules.is_none() {
            return self.direct_contact_inner(owner, target, hit, c, now, events);
        }
        let previous_origin = std::mem::replace(&mut self.triggers.origin, hit.origin.clone());
        let previous = self.triggers.action.take();
        let previous_source = self.triggers.action_source.replace(hit.source.clone());
        self.triggers.action = hit.action_id.map(str::to_owned);
        let result = self.direct_contact_inner(owner, target, hit, c, now, events);
        self.triggers.action = previous;
        self.triggers.action_source = previous_source;
        self.triggers.origin = previous_origin;
        result
    }
    pub(super) fn direct_contact_eligible(
        &mut self,
        owner: Owner,
        target: Owner,
        source: &EntityRef,
        (unavoidable, dead_source): (bool, bool),
        c: &Config,
        now: u64,
    ) -> bool {
        let target_ref = self.owner_entity(target);
        if self.owner_entity(owner) != *source
            || !dead_source && !self.valid_status_entity(&status_entity(source))
            || !self.hostile_contact(source, &target_ref)
        {
            return false;
        }
        if c.combat_rules.is_some()
            && self.entity_has_status(target, StatusAttribute::DamageImmunity, now)
        {
            return false;
        }
        if let Owner::Member(i) = target
            && self.member_damage_immune(i, c, now)
        {
            return false;
        }
        let dodged = if c.combat_rules.is_some() {
            if unavoidable {
                false
            } else {
                self.configured_dodge(owner, target, c, now)
            }
        } else if c.mechanism_version >= 101 && unavoidable {
            false
        } else if let Owner::Npc(j) = target {
            let dodge = self.npcs[j]
                .profile
                .stats
                .get("dodge")
                .copied()
                .unwrap_or(0);
            let probability = if c.mechanism_version >= 96 && matches!(owner, Owner::Member(_)) {
                let attacker = status_entity(source);
                let rules = c.pve.as_ref().unwrap().status_rules.as_ref();
                let penalty = self.statuses.attribute(
                    &attacker,
                    10000,
                    now,
                    rules,
                    StatusAttribute::DodgePenalty,
                ) - 10000;
                let accuracy = if c.mechanism_version >= 98 {
                    self.statuses
                        .attribute(&attacker, 10000, now, rules, StatusAttribute::Accuracy)
                        - 10000
                } else {
                    0
                };
                dodge_probability(u64::from(dodge) * 100, penalty, u64::from(accuracy))
            } else {
                f64::from(dodge) / 100.0
            };
            self.roll(probability)
        } else if let (Owner::Npc(j), Owner::Member(i)) = (owner, target) {
            self.member_dodges_npc_contact(i, j, c, now)
        } else {
            false
        };
        !dodged
    }

    fn direct_contact_inner(
        &mut self,
        owner: Owner,
        target: Owner,
        hit: DirectContact<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<u32> {
        if !self.direct_contact_eligible(
            owner,
            target,
            hit.source,
            (
                hit.unavoidable
                    || c.combat_rules.is_some() && hit.rule.is_some_and(|r| r.unavoidable),
                hit.dead_source,
            ),
            c,
            now,
        ) {
            return None;
        }
        let checkpoint = hit
            .origin
            .as_ref()
            .and_then(|_| self.take_checkpoint_contact((owner, target), hit.action_id, c, now));
        let target_ref = self.owner_entity(target);
        let raw = if hit.metal && c.combat_rules.is_none() {
            (f64::from(hit.raw) * c.params.metal_multiplier)
                .floor()
                .min(f64::from(u32::MAX)) as u32
        } else {
            hit.raw
        };
        let fallback = if hit.rule.is_none() {
            self.attack_rule(owner, None, c)
        } else {
            None
        };
        let attack = hit.rule.or(fallback.as_ref());
        let modified = attack.and_then(|r| {
            self.contact_status_modifier((owner, target, hit.action_id), raw, r, c, now, events)
        });
        let (raw, attack) = modified
            .as_ref()
            .map_or((raw, attack), |m| (m.raw, Some(&m.rule)));
        let reactions = self.capture_contact_reactions(hit.source, &target_ref, c, now);
        let intent = DamageIntent {
            ignore_armor: hit.ignore_armor,
            source: hit.source.clone(),
            target: target_ref.clone(),
            raw,
            direct: true,
        };
        let result = if hit.dead_source {
            self.resolve_detached_damage(intent, attack, c, now, events)
        } else {
            self.resolve_damage_with_rule(intent, attack, c, now, events)
        };
        if let Owner::Member(i) = owner
            && self.members[i].eligible()
            && !self.triggers.executing
            && !self.record_shields.executing
            && !self.window_records.executing
            && !self.status_ends.executing
            && let Some(rule) =
                attack.filter(|r| r.life_steal_timing == crate::config::LifeStealTiming::ActionEnd)
            && let (Some(id), Some(origin)) = (hit.action_id, hit.origin.as_ref())
        {
            self.collect_action_life_steal(
                super::triggers::ActionKey::new(hit.source, id),
                origin,
                result.hp_damage,
                rule.life_steal_bps,
                c,
            );
        }
        self.resolve_contact_reactions(
            (hit.source, &target_ref),
            reactions,
            result.hp_damage,
            c,
            now,
            events,
        );
        if !hit.metal
            && let Owner::Member(i) = owner
        {
            self.basic_contact(
                i,
                hit.action_id,
                (result.hp_damage > 0).then_some(&target_ref),
                c,
                now,
            );
        }
        if (self.periodic_enabled || matches!(owner, Owner::Member(_)))
            && self.valid_status_entity(&status_entity(&target_ref))
        {
            let max = self.entity_max_hp(&target_ref, c, now).unwrap();
            self.apply_owned_statuses(owner, (target_ref, max), hit.hit_statuses, c, now, events);
        }
        self.combat_fact_with_context(
            (
                crate::config::CombatFact::DirectHit,
                None,
                modified.as_ref().and_then(|m| m.augment.as_ref()),
            ),
            (owner, target),
            (result.hp_damage, false),
            c,
            now,
            events,
        );
        if let Some(m) = &modified
            && m.consumed > 0
        {
            self.combat_fact_with_consumption(
                (
                    crate::config::CombatFact::ContactStatusConsumed,
                    Some((
                        &m.rule.contact_status.as_ref().unwrap().definition_id,
                        m.consumed,
                    )),
                ),
                (owner, target),
                (raw, false),
                c,
                now,
                events,
            );
        }
        if !hit.metal && matches!(owner, Owner::Member(_)) {
            self.combat_fact(
                crate::config::CombatFact::BasicHit,
                (owner, target),
                (result.hp_damage, false),
                c,
                now,
                events,
            );
        }
        if let Some(checkpoint) = checkpoint {
            self.finish_checkpoint_contact(checkpoint, c, now, events);
        }
        Some(result.hp_damage)
    }
    pub(super) fn resolve_damage(
        &mut self,
        hit: DamageIntent,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        self.resolve_damage_with_rule(hit, None, c, now, events)
    }
    pub(super) fn resolve_damage_with_rule(
        &mut self,
        hit: DamageIntent,
        rule: Option<&crate::config::AttackRule>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        self.resolve_damage_mode(hit, rule, false, c, now, events)
    }
    pub(super) fn resolve_detached_damage(
        &mut self,
        hit: DamageIntent,
        rule: Option<&crate::config::AttackRule>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        self.resolve_damage_delivery(hit, rule, DamageMode::DetachedShot, c, now, events)
    }
    pub(super) fn resolve_periodic_damage(
        &mut self,
        hit: DamageIntent,
        rule: Option<&crate::config::AttackRule>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        self.resolve_damage_delivery(hit, rule, DamageMode::Periodic, c, now, events)
    }
    pub(super) fn resolve_damage_mode(
        &mut self,
        hit: DamageIntent,
        rule: Option<&crate::config::AttackRule>,
        transfer: bool,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        let mode = if transfer {
            DamageMode::Transfer
        } else {
            DamageMode::Ordinary
        };
        self.resolve_damage_delivery(hit, rule, mode, c, now, events)
    }

    pub(super) fn resolve_basic_field_damage(
        &mut self,
        hit: DamageIntent,
        rule: &crate::config::AttackRule,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        self.resolve_damage_delivery(hit, Some(rule), DamageMode::BasicField, c, now, events);
    }

    pub(super) fn detached_area_contact(
        &mut self,
        (i, source, raw): (usize, &EntityRef, u32),
        accuracy: Option<super::numeric::AttackAccuracy>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        if !self.members[i].eligible()
            || self.member_damage_immune(i, c, now)
            || accuracy
                .is_some_and(|hit| self.dodges_configured_attack(Owner::Member(i), hit, c, now))
        {
            return false;
        }
        self.resolve_damage_delivery(
            DamageIntent {
                ignore_armor: false,
                source: source.clone(),
                target: self.owner_entity(Owner::Member(i)),
                raw,
                direct: true,
            },
            None,
            DamageMode::DetachedArea,
            c,
            now,
            events,
        );
        true
    }

    pub(super) fn resolve_summon_burst_damage(
        &mut self,
        hit: DamageIntent,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let rule = crate::config::AttackRule {
            can_crit: false,
            ..Default::default()
        };
        let prior = std::mem::replace(&mut self.triggers.executing, true);
        self.resolve_damage_delivery(hit, Some(&rule), DamageMode::SummonDeath, c, now, events);
        self.triggers.executing = prior;
    }

    fn resolve_damage_delivery(
        &mut self,
        hit: DamageIntent,
        rule: Option<&crate::config::AttackRule>,
        mode: DamageMode<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        let prior_end_scope = self.statuses.end_cause_scope(
            self.record_shields.executing
                || self.triggers.executing
                || self.window_records.executing
                || self.status_ends.executing,
        );
        let resolved = if mode != DamageMode::DetachedArea
            && self.triggers.action.is_none()
            && hit.direct
            && c.combat_rules.as_ref().is_some_and(|r| {
                r.triggers
                    .iter()
                    .any(|t| t.counting == crate::config::TriggerCounting::ActionTotal)
            }) {
            let id = self.next_id("contact");
            self.triggers.action = Some(id);
            let previous_source = self.triggers.action_source.replace(hit.source.clone());
            let result = self.resolve_damage_with_rule_inner(hit, rule, mode, c, now, events);
            self.triggers.action = None;
            self.triggers.action_source = previous_source;
            result
        } else {
            self.resolve_damage_with_rule_inner(hit, rule, mode, c, now, events)
        };
        self.statuses.end_cause_scope(prior_end_scope);
        resolved
    }
    fn resolve_damage_with_rule_inner(
        &mut self,
        hit: DamageIntent,
        rule: Option<&crate::config::AttackRule>,
        mode: DamageMode<'_>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> DamageResult {
        let transfer = mode == DamageMode::Transfer;
        let basic_field = mode == DamageMode::BasicField;
        let terrain = if let DamageMode::Terrain(name) = mode {
            Some(name)
        } else {
            None
        };
        let deadline = mode == DamageMode::Deadline;
        let environmental = terrain.is_some() || deadline;
        self.reconcile_caps(c, now);
        let Some(target) = self.resolve_entity(&hit.target) else {
            return DamageResult::default();
        };
        let source = if environmental {
            let eligible = if deadline {
                matches!(target, Owner::Member(i) if self.members[i].living())
            } else {
                self.contact_eligible(target)
            };
            if !eligible {
                return DamageResult::default();
            }
            None
        } else if mode == DamageMode::DetachedArea {
            if hit.source.kind != StatusEntityKind::Monster
                || !self.pursuit_allows_detached(&hit.source, target)
                || !matches!(target, Owner::Member(_))
                || !self.contact_eligible(target)
                || self.training
                    && self
                        .resolve_entity(&hit.source)
                        .is_none_or(|source| !self.same_training_scope(source, target))
            {
                return DamageResult::default();
            }
            None
        } else {
            if !self.hostile_contact(&hit.source, &hit.target) {
                return DamageResult::default();
            }
            let source = self.resolve_entity(&hit.source);
            let valid = if mode == DamageMode::SummonDeath {
                matches!(source, Some(Owner::Summon(i)) if self.summons[i].hp == 0
                    && self.members[self.summons[i].member].eligible()
                    && self.members[self.summons[i].member].reference.entity == self.summons[i].owner)
            } else if mode == DamageMode::DetachedShot {
                // The frozen payload settles even after its member settled.
                true
            } else {
                self.valid_status_entity(&status_entity(&hit.source))
            };
            if !valid {
                return DamageResult::default();
            }
            source
        };
        let ordinary = matches!(mode, DamageMode::Ordinary | DamageMode::Periodic);
        let guard_reflect = match target {
            Owner::Member(i) if ordinary && !self.triggers.executing => {
                self.members[i].active_guard(c, now)
            }
            _ => None,
        };
        if self.damage_immune(target, c, now) {
            return DamageResult::default();
        }
        let owned_rule = if c.combat_rules.is_some()
            && rule.is_none()
            && let Some(source) = source
        {
            if hit.direct {
                self.attack_rule(source, None, c)
            } else {
                Some(crate::config::AttackRule::default())
            }
        } else {
            None
        };
        let resolved_rule = if c.combat_rules.is_some() {
            rule.or(owned_rule.as_ref())
        } else {
            None
        };
        let (incoming, critical) = match (source, resolved_rule) {
            _ if transfer => (hit.raw, false),
            (None, _) => (hit.raw, false),
            (Some(source), Some(rule)) => self.configured_incoming(
                &hit,
                rule,
                (source, target),
                hit.direct || basic_field,
                c,
                now,
            ),
            (Some(source), None) => (self.legacy_incoming(&hit, source, target, c, now), false),
        };
        let prior_shield_ends = self.statuses.take_shield_ends();
        let prior_window_ends = self.statuses.take_window_ends();
        let prior_cleanse_ends = self.statuses.take_cleanse_ends();
        if !transfer && !self.record_shields.executing && !self.triggers.executing {
            self.statuses
                .record_incoming(&status_entity(&hit.target), incoming, now);
        }
        let (left, absorbed) = if transfer {
            (incoming, 0)
        } else {
            let label = terrain.map(|name| format!("terrain:{name}"));
            let native_npc =
                matches!(source, Some(Owner::Npc(_))) || mode == DamageMode::DetachedArea;
            let result = self.absorb_damage(
                &hit.target,
                (
                    label
                        .as_deref()
                        .unwrap_or(if deadline { "deadline" } else { &hit.source.id }),
                    native_npc,
                ),
                incoming,
                now,
                events,
            );
            if native_npc
                && result.1 > 0
                && let Owner::Member(i) = target
            {
                self.note_boon_shield_depletion(i, c, now);
            }
            result
        };
        let guard = self.plan_death_guard(target, left, c, now, events);
        let hp = match target {
            Owner::Member(i) => &mut self.members[i].hp,
            Owner::Npc(i) => &mut self.npcs[i].hp,
            Owner::Summon(i) => &mut self.summons[i].hp,
        };
        let would_lose = left.min(*hp);
        let hp_damage = guard
            .as_ref()
            .map_or(would_lose, |g| hp.saturating_sub(g.retain_hp.min(*hp)));
        *hp -= hp_damage;
        let remaining = *hp;
        if hp_damage > 0
            && let Owner::Npc(j) = target
            && let Some(i) = self.npcs[j].training_owner
            && self.damage_member(source, mode) == Some(i)
            && let Some((_, metrics)) = self
                .training_runtime
                .as_mut()
                .and_then(|seats| seats[i].metrics.as_mut())
        {
            use super::training_targets::DamageSource;
            let kind = match (source, mode, self.triggers.origin.as_ref()) {
                (Some(Owner::Summon(_)), _, _) => DamageSource::Summon,
                (_, DamageMode::Periodic, _) => DamageSource::Periodic,
                (_, DamageMode::BasicField, _)
                | (_, DamageMode::Ordinary, Some(crate::origin::ActionOrigin::Basic))
                    if hit.direct || basic_field =>
                {
                    DamageSource::Basic
                }
                (_, DamageMode::Ordinary, Some(crate::origin::ActionOrigin::Skill(_)))
                    if hit.direct =>
                {
                    DamageSource::Skill
                }
                _ => DamageSource::Other,
            };
            metrics.record(hp_damage, critical, kind, now);
        }
        if c.settlement_progression.is_some()
            && hp_damage > 0
            && hit.direct
            && mode == DamageMode::Ordinary
            && !self.triggers.executing
            && !self.record_shields.executing
            && !self.window_records.executing
            && !self.status_ends.executing
            && let (Some(Owner::Member(i)), Owner::Npc(_)) = (source, target)
        {
            let m = &mut self.members[i];
            let profile = m
                .world_weapon_profile
                .as_deref()
                .or(m.basic_profile.as_deref());
            c.record_damage_growth(&mut m.damage_growth, hp_damage, &m.reward_rates, profile);
        }
        if let Owner::Npc(i) = target
            && source.is_some()
        {
            self.npc_ai_hurt(i, &hit.source, hp_damage);
            if let Some(member) = self.damage_member(source, mode) {
                self.record_monster_contribution(i, member, hp_damage, c);
            }
        }
        if !transfer
            && !environmental
            && !self.record_shields.executing
            && !self.triggers.executing
            && !self.window_records.executing
            && let Some(rules) = &c.combat_rules
        {
            self.statuses.record_windows(
                (&status_entity(&hit.source), &status_entity(&hit.target)),
                (hp_damage, absorbed, !hit.direct),
                now,
                &rules.status_windows,
            );
        }
        if deadline {
            events.push((
                None,
                EventV4::DeadlineDamage {
                    target: hit.target.clone(),
                    amount: hp_damage,
                    absorbed,
                    hp: remaining,
                },
            ));
        } else if let Some(terrain) = terrain {
            events.push((
                None,
                EventV4::TerrainDamage {
                    terrain: terrain.into(),
                    target: hit.target.clone(),
                    amount: hp_damage,
                    absorbed,
                    hp: remaining,
                },
            ));
        } else if !transfer {
            events.push((
                None,
                EventV4::Damage {
                    source: hit.source.clone(),
                    target: hit.target.clone(),
                    amount: hp_damage,
                    hp: remaining,
                },
            ));
        }
        if let Owner::Npc(j) = target
            && hp_damage > 0
            && remaining > 0
        {
            self.advance_npc_boss(j, c, now, events);
        }
        if let Some(guard) = guard {
            self.finish_death_guard(
                target,
                (!environmental).then_some(&hit.source),
                (guard, would_lose - hp_damage),
                c,
                now,
                events,
            );
        }
        if mode == DamageMode::Ordinary
            && hit.direct
            && remaining > 0
            && (hp_damage > 0 || absorbed > 0)
            && !self.triggers.executing
            && !self.record_shields.executing
            && !self.window_records.executing
            && !self.status_ends.executing
            && let Some(source) = source
        {
            self.retaliate(target, source, c, now, events);
        }
        if !transfer
            && !basic_field
            && let Some(rule) = resolved_rule
                .filter(|r| r.life_steal_timing == crate::config::LifeStealTiming::Contact)
        {
            self.life_steal(&hit.source, (hp_damage, hit.direct), rule, c, now, events);
        }
        if let Some(source) = source
            && absorbed > 0
        {
            self.combat_fact(
                crate::config::CombatFact::ShieldAbsorbed,
                (target, source),
                (absorbed, !hit.direct),
                c,
                now,
                events,
            );
        }
        if let Some(source) = source
            && hp_damage > 0
        {
            if !transfer {
                if ordinary {
                    self.combat_fact(
                        crate::config::CombatFact::DamageDealt,
                        (source, target),
                        (hp_damage, !hit.direct),
                        c,
                        now,
                        events,
                    );
                }
                self.combat_fact(
                    crate::config::CombatFact::DamageTaken,
                    (target, source),
                    (hp_damage, !hit.direct),
                    c,
                    now,
                    events,
                );
            }
            if remaining == 0 {
                self.combat_fact(
                    crate::config::CombatFact::Kill,
                    (source, target),
                    (hp_damage, !hit.direct),
                    c,
                    now,
                    events,
                );
            }
        }
        match target {
            Owner::Member(i) => {
                if hp_damage > 0 || absorbed > 0 {
                    self.cancel_boon_claim(i, Error::InvalidState, events);
                }
                if hp_damage > 0 {
                    self.release_member_containers(i, events);
                }
                if hp_damage > 0 || self.shields_enabled && hit.direct {
                    self.end_extraction_channel(i, ExtractionChannelState::Interrupted, events);
                }
                if remaining == 0 && self.mode.is_none() {
                    self.handle_member_defeat(i, c, now, events);
                }
            }
            Owner::Npc(j) => {
                if remaining == 0 {
                    self.stop_npc_areas(&hit.target, AreaEndReason::SourceInvalid, events);
                    let credited = self.damage_member(source, mode);
                    if self.npcs[j].pursuit.is_some() {
                        self.clear_pursuit_combat(j, events);
                    }
                    if let Some(i) = credited {
                        self.defeated(j, i, c, now, events);
                    } else if terrain.is_some() && self.npcs[j].pursuit.is_none() {
                        self.clear_monster_contribution(j);
                        self.npcs[j].casting = None;
                        self.npcs[j].target = None;
                        self.npcs[j].ai = Default::default();
                        self.clear_owned_statuses(
                            &hit.target,
                            StatusRemovalReason::TargetInvalid,
                            events,
                        );
                    }
                }
            }
            Owner::Summon(i) => {
                if remaining == 0 && hp_damage > 0 {
                    self.retire_summon(i, events);
                    self.summon_death_burst(i, c, now, events);
                }
            }
        }
        self.finish_shield_records(c, now, events);
        self.finish_status_cleanses(c, now, events);
        self.finish_window_records(c, now, events);
        self.statuses.restore_shield_ends(prior_shield_ends);
        self.statuses.restore_window_ends(prior_window_ends);
        self.statuses.restore_cleanse_ends(prior_cleanse_ends);
        self.reconcile_caps(c, now);
        if let Some((_, bps, damage_kind)) = guard_reflect
            && remaining > 0
        {
            let raw = (u64::from(hp_damage) * u64::from(bps) / 10000) as u32;
            if raw > 0 {
                let rule = crate::config::AttackRule {
                    damage_kind,
                    can_crit: false,
                    ..Default::default()
                };
                self.resolve_damage_delivery(
                    DamageIntent {
                        source: hit.target,
                        target: hit.source,
                        raw,
                        direct: false,
                        ignore_armor: false,
                    },
                    Some(&rule),
                    DamageMode::GuardReflection,
                    c,
                    now,
                    events,
                );
            }
        }
        DamageResult {
            hp_damage,
            absorbed,
            lethal: hp_damage > 0 && remaining == 0,
        }
    }

    fn damage_member(&self, source: Option<Owner>, mode: DamageMode<'_>) -> Option<usize> {
        match source {
            Some(Owner::Member(i)) => Some(i),
            Some(Owner::Summon(i)) if mode == DamageMode::SummonDeath || self.summon_alive(i) => {
                Some(self.summons[i].member)
            }
            _ => None,
        }
    }
    fn legacy_incoming(
        &mut self,
        hit: &DamageIntent,
        source: Owner,
        target: Owner,
        c: &Config,
        now: u64,
    ) -> u32 {
        let mut armor = f64::from(match target {
            Owner::Member(i) => self.stats(i, c, now).1,
            Owner::Npc(i) => self.statuses.defense(
                &status_entity(&hit.target),
                self.npcs[i].profile.stats["def"],
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
            ),
            Owner::Summon(i) => self.summons[i].base_def,
        });
        let mut multiplier = 1.0;
        if c.mechanism_version >= 93
            && hit.direct
            && matches!((source, target), (Owner::Member(_), Owner::Npc(_)))
        {
            let source = status_entity(&hit.source);
            let rules = c.pve.as_ref().unwrap().status_rules.as_ref();
            let penetration = self.statuses.attribute(
                &source,
                10000,
                now,
                rules,
                StatusAttribute::ArmorPenetration,
            ) - 10000;
            armor *= f64::from(10000 - penetration) / 10000.0;
            if c.mechanism_version >= 94 {
                let chance = self.statuses.attribute(
                    &source,
                    10000,
                    now,
                    rules,
                    StatusAttribute::CriticalChance,
                ) - 10000;
                if self.roll(f64::from(chance) / 10000.0) {
                    multiplier = c.params.critical_multiplier;
                    if c.mechanism_version >= 95 {
                        let bonus = self.statuses.attribute(
                            &source,
                            10000,
                            now,
                            rules,
                            StatusAttribute::CriticalDamageBonus,
                        ) - 10000;
                        multiplier += f64::from(bonus) / 10000.0;
                    }
                }
            }
        }
        if c.mechanism_version >= 103 && hit.direct && hit.ignore_armor {
            armor = 0.0;
        }
        let mut incoming = damage(hit.raw, armor, multiplier, c);
        // For periodic hits, `now` is the actual execution time of the tick.
        if matches!(target, Owner::Member(_))
            && (c.mechanism_version >= 68 || c.mechanism_version == 67 && hit.direct)
        {
            let reduction = self.effective_damage_reduction(target, &hit.target, c, now);
            let scaled = u64::from(incoming) * u64::from(10000 - reduction);
            let remainder = scaled % 10000;
            incoming = (scaled / 10000) as u32;
            if remainder != 0 && self.roll(remainder as f64 / 10000.) {
                incoming += 1;
            }
        }
        incoming
    }
}
