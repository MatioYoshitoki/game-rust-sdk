//! Strict unit plans reuse the shared direct-contact resolver without a projectile.
use super::*;
impl World {
    pub(super) fn unit_effect(
        &mut self,
        (i, target, skill, action_id): (usize, &EntityRef, &SkillConfig, &str),
        raw: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let Some(owner) = self.resolve_entity(target) else {
            return;
        };
        let source = self.members[i].reference.entity.clone();
        let rule = self.attack_rule(Owner::Member(i), Some(&skill.id), c);
        let hit_statuses = c
            .combat_rules
            .as_ref()
            .and_then(|r| r.unit_hit_statuses_by_mastery.get(&skill.id))
            .and_then(|rows| {
                let mastery = self.members[i]
                    .combat_build
                    .as_ref()?
                    .powers
                    .get(&skill.id)?
                    .mastery;
                crate::config::MasteryStatuses::select(rows, mastery)
            })
            .unwrap_or(&skill.on_hit_target_statuses);
        self.direct_contact(
            Owner::Member(i),
            owner,
            combat::DirectContact {
                origin: Some(crate::origin::ActionOrigin::Skill(skill.id.clone())),
                action_id: Some(action_id),
                rule: rule.as_ref(),
                ignore_armor: skill.ignore_armor.unwrap_or(false),
                unavoidable: skill.unavoidable.unwrap_or(false),
                dead_source: false,
                source: &source,
                raw,
                metal: true,
                hit_statuses,
            },
            c,
            now,
            events,
        );
    }
}
