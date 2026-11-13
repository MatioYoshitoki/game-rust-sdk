//! Raw build facts are immutable; all live outputs use the current native status set.
use super::*;
impl Member {
    pub(super) fn base_points(&self, c: &Config, a: StatusAttribute) -> u32 {
        let raw = match a {
            StatusAttribute::MaxHp => self.facts.hp_points,
            StatusAttribute::MaxSpirit => self.facts.spirit_points,
            StatusAttribute::Atk => self.facts.atk_points,
            StatusAttribute::Def => self.facts.def_points,
            StatusAttribute::Spd => self.facts.spd_points,
            _ => unreachable!("member base axis"),
        };
        c.derived_base(raw, a)
    }
}
impl World {
    pub(super) fn build_for<'a>(&'a self, owner: Owner, c: &'a Config) -> Option<&'a CombatBuild> {
        match owner {
            Owner::Member(i) => self.members[i].combat_build.as_ref(),
            Owner::Npc(j) => c
                .combat_rules
                .as_ref()?
                .npc(&self.npcs[j].template_id)
                .build
                .as_ref(),
            Owner::Summon(_) => None,
        }
    }
    pub(super) fn has_power_build(&self, owner: Owner, c: &Config) -> bool {
        self.build_for(owner, c).is_some()
    }
    fn skill_source_base(&self, owner: Owner, skill: &str, c: &Config) -> Option<u32> {
        let build = self.build_for(owner, c)?;
        let input = build.powers.get(skill)?;
        Some(
            c.build_rules()?
                .power_models
                .get(&input.model_id)?
                .amount(input),
        )
    }
    pub(super) fn power_multipliers(
        &self,
        statuses: &Statuses,
        owner: Owner,
        c: &Config,
        now: u64,
    ) -> (u32, u32) {
        let profile = self.combat_profile(owner, c);
        let build = self.build_for(owner, c);
        let permanent = build.map_or(0, |b| i64::from(b.skill_power_points) * 100);
        let bonus = statuses.attribute_with_bonus(
            &status_entity(&self.owner_entity(owner)),
            10000,
            now,
            c.pve.as_ref().unwrap().status_rules.as_ref(),
            StatusAttribute::SkillPowerBonus,
            permanent,
        );
        let points = build.map_or(profile.power_points_bps, |b| b.power_points * 100);
        (bonus, points)
    }
    pub(super) fn selected_power_with(
        &self,
        statuses: &Statuses,
        owner: Owner,
        skill: &str,
        c: &Config,
        now: u64,
    ) -> u32 {
        let e = status_entity(&self.owner_entity(owner));
        let raw = self.base_attack(owner, c);
        let attack = statuses.attribute(
            &e,
            raw,
            now,
            c.pve.as_ref().unwrap().status_rules.as_ref(),
            StatusAttribute::Atk,
        );
        let attack = match owner {
            Owner::Member(i) => self.members[i].exposure_points(attack, c, now),
            _ => attack,
        };
        let base = self
            .skill_source_base(owner, skill, c)
            .or_else(|| {
                c.combat_rules
                    .as_ref()
                    .unwrap()
                    .skills
                    .get(skill)
                    .and_then(|r| r.base_power)
            })
            .unwrap_or(attack);
        let (bonus, points) = self.power_multipliers(statuses, owner, c, now);
        (u128::from(base) * u128::from(bonus) * u128::from(10000 + points) / 100000000)
            .min(u128::from(u32::MAX)) as u32
    }
    pub(super) fn status_source_power(
        &self,
        statuses: &Statuses,
        owner: Owner,
        definition: &str,
        c: &Config,
        now: u64,
    ) -> Option<u32> {
        let skill = c
            .combat_rules
            .as_ref()?
            .status_power_sources
            .get(definition)?;
        Some(self.selected_power_with(statuses, owner, skill, c, now))
    }
    pub(super) fn recovery_power(&self, owner: Owner, skill: &str, c: &Config, now: u64) -> u32 {
        if self.has_power_build(owner, c) || c.build_rules().is_some() {
            self.selected_power_with(&self.statuses, owner, skill, c, now)
        } else {
            self.entity_attack(&status_entity(&self.owner_entity(owner)), c, now)
                .unwrap()
        }
    }
    pub(super) fn source_element_resistance(&self, owner: Owner, c: &Config) -> Option<u32> {
        let build = self.build_for(owner, c)?;
        Some(c.build_rules()?.conversion.amount(
            build.element_resistance_points,
            StatusAttribute::ElementResistance,
        ))
    }
}
