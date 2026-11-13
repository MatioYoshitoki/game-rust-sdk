//! The configured combat pipeline. Contact selection and financial state stay elsewhere.
use super::*;
use crate::config::{AttackRule, CombatProfile, DamageKind};

#[derive(Clone, Copy)]
pub(super) struct AttackAccuracy {
    hit: u64,
    penalty: u32,
}

impl World {
    pub(super) fn entity_attack(&self, e: &StatusEntity, c: &Config, now: u64) -> Option<u32> {
        self.entity_owner(e).map(|owner| match owner {
            Owner::Member(i) => self.stats(i, c, now).0,
            Owner::Npc(_) | Owner::Summon(_) => self.statuses.attribute(
                e,
                self.base_attack(owner, c),
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
                StatusAttribute::Atk,
            ),
        })
    }
    pub(super) fn combat_profile<'a>(&self, owner: Owner, c: &'a Config) -> &'a CombatProfile {
        let rules = c.combat_rules.as_ref().expect("configured combat");
        match owner {
            Owner::Member(i) => rules.member(&self.members[i].reference.owner_id),
            Owner::Npc(i) => rules.npc(&self.npcs[i].template_id),
            Owner::Summon(i) => rules.npc(&self.summons[i].template_id),
        }
    }
    pub(super) fn entity_has_status(
        &self,
        owner: Owner,
        attribute: StatusAttribute,
        now: u64,
    ) -> bool {
        let e = status_entity(&self.owner_entity(owner));
        self.statuses
            .entries()
            .iter()
            .any(|s| s.target == e && s.has_attribute(attribute) && now < s.expires_at_ms)
    }
    pub(super) fn npc_movement_factor(&self, j: usize, c: &Config, now: u64) -> f64 {
        if c.combat_rules.is_none() {
            return 1.0;
        }
        if self.entity_has_status(Owner::Npc(j), StatusAttribute::Root, now) {
            return 0.0;
        }
        let target = status_entity(&self.npcs[j].entity);
        f64::from(self.statuses.movement_factor_bps(&target, now)) / 10000.0
    }
    pub(super) fn attack_rule(
        &self,
        owner: Owner,
        skill: Option<&str>,
        c: &Config,
    ) -> Option<AttackRule> {
        let rules = c.combat_rules.as_ref()?;
        Some(match owner {
            Owner::Member(_) => skill.map_or_else(
                || rules.basic_attack.clone(),
                |id| rules.skills.get(id).cloned().unwrap_or_default(),
            ),
            Owner::Npc(i) => rules
                .npc_attacks
                .get(&self.npcs[i].template_id)
                .cloned()
                .unwrap_or_default(),
            Owner::Summon(_) => AttackRule::default(),
        })
    }
    pub(super) fn configured_budget(
        &self,
        i: usize,
        skill: Option<&SkillConfig>,
        c: &Config,
        now: u64,
    ) -> u32 {
        let bps = skill.map_or(10000, |s| s.damage_bps);
        let Some(mut rule) = self.attack_rule(Owner::Member(i), skill.map(|s| s.id.as_str()), c)
        else {
            return self.member_raw_attack(i, bps, skill.is_some(), c, now);
        };
        if let Some(skill) = skill
            && self.has_power_build(Owner::Member(i), c)
        {
            rule.base_power =
                Some(self.selected_power_with(&self.statuses, Owner::Member(i), &skill.id, c, now));
            return rule.budget(self.stats(i, c, now).0, bps, 10000, 0);
        }
        let bonus = if skill.is_some() {
            self.statuses.attribute(
                &status_entity(&self.members[i].reference.entity),
                10000,
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
                StatusAttribute::SkillPowerBonus,
            )
        } else {
            10000
        };
        let points = if skill.is_some() {
            self.combat_profile(Owner::Member(i), c).power_points_bps
        } else {
            0
        };
        rule.budget(self.stats(i, c, now).0, bps, bonus, points)
    }
    pub(super) fn npc_attack_budget(
        &self,
        j: usize,
        attack: u32,
        bps: u32,
        is_skill: bool,
        c: &Config,
        now: u64,
    ) -> u32 {
        let Some(mut rule) = self.attack_rule(Owner::Npc(j), None, c) else {
            return (u64::from(attack) * u64::from(bps) / 10000).min(u64::from(u32::MAX)) as u32;
        };
        if is_skill && self.has_power_build(Owner::Npc(j), c) {
            rule.base_power = Some(self.selected_power_with(
                &self.statuses,
                Owner::Npc(j),
                &self.npcs[j].profile.combat.projectile.id,
                c,
                now,
            ));
            return rule.budget(attack, bps, 10000, 0);
        }
        let bonus = if is_skill {
            self.statuses.attribute(
                &status_entity(&self.npcs[j].entity),
                10000,
                now,
                c.pve.as_ref().unwrap().status_rules.as_ref(),
                StatusAttribute::SkillPowerBonus,
            )
        } else {
            10000
        };
        let points = if is_skill {
            self.combat_profile(Owner::Npc(j), c).power_points_bps
        } else {
            0
        };
        rule.budget(attack, bps, bonus, points)
    }
    pub(super) fn configured_accuracy(
        &self,
        source: Owner,
        c: &Config,
        now: u64,
    ) -> AttackAccuracy {
        let profile = self.combat_profile(source, c);
        let source = status_entity(&self.owner_entity(source));
        let rules = c.pve.as_ref().unwrap().status_rules.as_ref();
        let bonus = |attr| self.statuses.attribute(&source, 10000, now, rules, attr) - 10000;
        AttackAccuracy {
            hit: u64::from(profile.hit_bps) + u64::from(bonus(StatusAttribute::Accuracy)),
            penalty: bonus(StatusAttribute::DodgePenalty),
        }
    }

    pub(super) fn configured_dodge(
        &mut self,
        source: Owner,
        target: Owner,
        c: &Config,
        now: u64,
    ) -> bool {
        let accuracy = self.configured_accuracy(source, c, now);
        self.dodges_configured_attack(target, accuracy, c, now)
    }

    pub(super) fn dodges_configured_attack(
        &mut self,
        target: Owner,
        accuracy: AttackAccuracy,
        c: &Config,
        now: u64,
    ) -> bool {
        let b = self.combat_profile(target, c);
        let target = status_entity(&self.owner_entity(target));
        let rules = c.pve.as_ref().unwrap().status_rules.as_ref();
        let bonus =
            self.statuses
                .attribute(&target, 10000, now, rules, StatusAttribute::DodgeChance)
                - 10000;
        let dodge = u64::from(b.dodge_bps) + u64::from(bonus);
        let remaining = dodge.saturating_sub(u64::from(accuracy.penalty));
        let cap = c
            .combat_rules
            .as_ref()
            .unwrap()
            .dodge_cap_bps
            .unwrap_or(crate::config::MAX_DODGE_BPS);
        self.roll((remaining as f64 / (10000 + accuracy.hit) as f64).min(f64::from(cap) / 10000.0))
    }
    pub(super) fn configured_incoming(
        &mut self,
        hit: &combat::DamageIntent,
        rule: &AttackRule,
        (source, target): (Owner, Owner),
        allow_critical: bool,
        c: &Config,
        now: u64,
    ) -> (u32, bool) {
        let attacker = self.combat_profile(source, c);
        let defender = self.combat_profile(target, c);
        let rules = c.combat_rules.as_ref().unwrap();
        let status_rules = c.pve.as_ref().unwrap().status_rules.as_ref();
        let entity = status_entity(&hit.source);
        let bonus = |attribute| {
            self.statuses
                .attribute(&entity, 10000, now, status_rules, attribute)
                - 10000
        };
        let penetration = rule
            .armor_penetration_bps
            .saturating_add(bonus(StatusAttribute::ArmorPenetration))
            .min(rule.armor_penetration_cap_bps.unwrap_or(u32::MAX));
        let critical_chance = attacker
            .critical_chance_bps
            .saturating_add(bonus(StatusAttribute::CriticalChance))
            .min(10000);
        let critical_bps = attacker
            .critical_multiplier_bps
            .saturating_add(bonus(StatusAttribute::CriticalDamageBonus))
            .min(rules.critical_multiplier_cap_bps.unwrap_or(30000));
        let reduction = self.effective_damage_reduction(target, &hit.target, c, now);
        let damage_taken_bonus_bps = if rules.status_damage_taken_bps.is_empty() {
            0
        } else {
            let target = status_entity(&hit.target);
            self.statuses
                .entries()
                .iter()
                .filter(|s| s.target == target && now < s.expires_at_ms)
                .filter_map(|s| rules.status_damage_taken_bps.get(&s.definition_id).copied())
                .max()
                .unwrap_or(0)
        };
        let defense = match target {
            Owner::Member(i) => self.stats(i, c, now).1,
            Owner::Npc(i) => self.statuses.defense(
                &status_entity(&hit.target),
                self.npcs[i].profile.stats["def"],
                now,
                status_rules,
            ),
            Owner::Summon(i) => self.statuses.defense(
                &status_entity(&hit.target),
                self.summons[i].base_def,
                now,
                status_rules,
            ),
        };
        let armor = if rule.damage_kind == DamageKind::Elemental {
            self.statuses.attribute(
                &status_entity(&hit.target),
                self.source_element_resistance(target, c)
                    .or(defender.element_resistance)
                    .unwrap_or(defense),
                now,
                status_rules,
                StatusAttribute::ElementResistance,
            )
        } else {
            defense
        };
        let critical = allow_critical
            && rule.can_crit
            && rule.damage_kind != DamageKind::TrueDamage
            && self.roll(f64::from(critical_chance) / 10000.0);
        let amount = crate::combat_math::damage(crate::combat_math::DamageFactors {
            raw: hit.raw,
            kind: rule.damage_kind,
            tier_multiplier: rules.tier_multipliers
                [&(i16::from(attacker.tier) - i16::from(defender.tier))]
                .factor(),
            element_bps: rules.element_factor(
                rule.element.as_deref(),
                defender.element.as_deref(),
                false,
            ),
            weapon_bps: rules.element_factor(
                rule.weapon_element.as_deref(),
                rule.element.as_deref(),
                true,
            ),
            armor,
            baseline_attack: defender.defense_baseline_attack,
            penetration_bps: penetration,
            ignore_armor: hit.ignore_armor || rule.ignore_armor,
            defense_scale: c.params.defense_scale,
            defense_cap: c.defense_cap,
            reduction_bps: reduction,
            damage_bonus_bps: if hit.direct
                && !self.triggers.executing
                && !self.record_shields.executing
                && !self.window_records.executing
                && !self.status_ends.executing
            {
                self.summon_owner_bonus(source, now).damage_bonus_bps
            } else {
                0
            },
            damage_taken_bonus_bps,
            critical_bps: if critical { critical_bps } else { 10000 },
            minimum: c.params.minimum_damage,
        });
        (amount, critical)
    }
    pub(super) fn life_steal(
        &mut self,
        source: &EntityRef,
        (hp_damage, direct): (u32, bool),
        rule: &AttackRule,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let bps = self.life_steal_rate(source, rule.life_steal_bps, direct, c, now);
        self.ratio_self_heal(
            source,
            u64::from(hp_damage),
            (bps, HealKind::LifeSteal),
            c,
            now,
            events,
        );
    }
    pub(super) fn life_steal_rate(
        &self,
        source: &EntityRef,
        base_bps: u32,
        direct: bool,
        c: &Config,
        now: u64,
    ) -> u32 {
        let bonus = if direct {
            i64::from(self.statuses.attribute(
                &status_entity(source),
                10000,
                now,
                c.pve.as_ref().and_then(|p| p.status_rules.as_ref()),
                StatusAttribute::LifeSteal,
            )) - 10000
        } else {
            0
        };
        let cap = c
            .combat_rules
            .as_ref()
            .and_then(|r| r.life_steal_cap_bps)
            .unwrap_or(10000);
        (i64::from(base_bps) + bonus).clamp(0, i64::from(cap)) as u32
    }
    pub(super) fn ratio_self_heal(
        &mut self,
        source: &EntityRef,
        basis: u64,
        (bps, heal_kind): (u32, HealKind),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if basis == 0 || bps == 0 || !self.valid_status_entity(&status_entity(source)) {
            return;
        }
        let raw = (u128::from(basis) * u128::from(bps) / 10000).min(u128::from(u32::MAX)) as u32;
        let entity = status_entity(source);
        let multiplier_bps = self.statuses.healing_received_bps(
            &entity,
            now,
            c.pve.as_ref().unwrap().status_rules.as_ref(),
        );
        let adjusted = u64::from(raw) * u64::from(multiplier_bps) / 10000;
        let restored = self.restore_hp(source, adjusted.min(u64::from(u32::MAX)) as u32, c, now);
        let event = Event::HealResolved {
            source: entity.clone(),
            target: entity.clone(),
            heal_kind,
            raw_heal: raw,
            multiplier_bps,
            adjusted_heal: adjusted,
            hp_restored: restored,
        };
        for i in self.status_observers(&entity, &entity) {
            private(events, i, event.clone());
        }
    }
}
