use super::*;
impl World {
    pub(super) fn mastery_cooldown_reduction(&self, member: usize, skill: &str, c: &Config) -> u32 {
        let Some(rows) = c
            .combat_rules
            .as_ref()
            .and_then(|r| r.skill_cooldowns_by_mastery.get(skill))
        else {
            return 0;
        };
        self.members[member]
            .combat_build
            .as_ref()
            .and_then(|build| build.powers.get(skill))
            .and_then(|power| {
                rows.iter()
                    .rev()
                    .find(|row| power.mastery >= row.minimum_mastery)
            })
            .map_or(0, |row| row.reduction_bps)
    }

    pub(super) fn self_statuses_for<'a>(
        &self,
        member: usize,
        skill: &'a SkillConfig,
        c: &'a Config,
    ) -> &'a [String] {
        let selected = c
            .combat_rules
            .as_ref()
            .and_then(|rules| rules.self_statuses_by_mastery.get(&skill.id))
            .zip(self.members[member].combat_build.as_ref())
            .and_then(|(rows, build)| {
                let mastery = build.powers.get(&skill.id)?.mastery;
                crate::config::MasteryStatuses::select(rows, mastery)
            });
        selected.unwrap_or(&skill.on_commit_self_statuses)
    }

    pub(super) fn meets_skill_requirement(
        &self,
        owner: Owner,
        rule: &crate::config::SkillRequirement,
        c: &Config,
    ) -> bool {
        let ids = rule
            .equipped_any_of
            .as_deref()
            .unwrap_or(std::slice::from_ref(&rule.skill_id));
        let installed = ids.iter().any(|id| match owner {
            Owner::Member(i) => self.members[i].skill_equipped(id, c),
            Owner::Npc(j) => {
                let p = &self.npcs[j].profile;
                p.combat.projectile.id == *id || p.combat.heal.id == *id
            }
            Owner::Summon(_) => false,
        });
        installed
            && (rule.minimum_mastery.is_none() && rule.maximum_mastery.is_none() || {
                self.build_for(owner, c)
                    .and_then(|b| b.powers.get(&rule.skill_id))
                    .is_some_and(|g| {
                        rule.minimum_mastery.is_none_or(|min| g.mastery >= min)
                            && rule.maximum_mastery.is_none_or(|max| g.mastery <= max)
                    })
            })
    }
}
