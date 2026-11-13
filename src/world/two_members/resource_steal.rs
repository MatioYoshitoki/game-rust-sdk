//! Current-resource budgets and one shared native settlement, with no synthetic hit.
use super::*;
use crate::config::{AttackRule, DamageKind, ResourceSteal};
impl World {
    fn spirit_value(&self, o: Owner) -> u32 {
        match o {
            Owner::Member(i) => self.members[i].spirit,
            Owner::Npc(j) => self.npcs[j].spirit,
            Owner::Summon(_) => 0,
        }
    }
    pub(super) fn resource_budget(
        &self,
        source: &EntityRef,
        target: &EntityRef,
        d: &ResourceSteal,
        c: &Config,
        now: u64,
    ) -> Option<[u32; 2]> {
        let owner = self.resolve_entity(target)?;
        let hp = self.current_hp(owner);
        let bonus = d.status_bonus.as_ref().map_or(0, |b| {
            if b.qualification.as_ref().is_some_and(|q| {
                self.resolve_entity(source)
                    .is_none_or(|owner| !self.meets_skill_requirement(owner, q, c))
            }) {
                return 0;
            }
            let holder = if matches!(b.subject, crate::config::TriggerSubject::Owner) {
                source
            } else {
                target
            };
            self.statuses
                .entries()
                .iter()
                .find(|s| {
                    s.source == status_entity(source)
                        && s.target == status_entity(holder)
                        && s.definition_id == b.definition_id
                        && now < s.expires_at_ms
                })
                .map_or(0, |s| {
                    (u64::from(s.stacks.unwrap_or(1)) * u64::from(b.per_stack_bps))
                        .min(u64::from(b.maximum_bonus_bps))
                })
        });
        let amount = |value: u32, ratio: u32| {
            if ratio == 0 {
                return 0;
            }
            let numerator = match d.status_bonus.as_ref().map(|b| b.mode) {
                Some(crate::config::ResourceBonusMode::AddCurrentBps) => {
                    ((u128::from(ratio) + u128::from(bonus)) * 10000).min(100000000)
                }
                _ => (u128::from(ratio) * u128::from(10000 + bonus)).min(100000000),
            };
            (u128::from(value) * numerator / 100000000) as u32
        };
        Some([
            amount(hp, d.hp_current_bps),
            amount(self.spirit_value(owner), d.spirit_current_bps),
        ])
    }
    fn take_spirit(&mut self, o: Owner, amount: u32) -> u32 {
        let value = match o {
            Owner::Member(i) => &mut self.members[i].spirit,
            Owner::Npc(j) => &mut self.npcs[j].spirit,
            Owner::Summon(_) => return 0,
        };
        let taken = amount.min(*value);
        *value -= taken;
        taken
    }
    fn return_spirit(&mut self, o: Owner, amount: u32, c: &Config, now: u64) -> u32 {
        let e = self.owner_entity(o);
        let max = self.entity_max_spirit(&e, c, now).unwrap();
        if self.entity_has_status(o, StatusAttribute::SpiritRecoveryLock, now) {
            return 0;
        }
        let value = match o {
            Owner::Member(i) => &mut self.members[i].spirit,
            Owner::Npc(j) => &mut self.npcs[j].spirit,
            Owner::Summon(_) => return 0,
        };
        *value = (*value).min(max);
        let restored = amount.min(max - *value);
        *value += restored;
        restored
    }
    pub(super) fn steal_resources(
        &mut self,
        (owner, target, action_id): (Owner, &EntityRef, Option<&str>),
        d: &ResourceSteal,
        budget: [u32; 2],
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        self.reconcile_caps(c, now);
        let source = self.owner_entity(owner);
        let source_status = status_entity(&source);
        let target_status = status_entity(target);
        let mut removed_hp = 0;
        let mut removed_spirit = 0;
        let mut restored_spirit = 0;
        if self.hostile_contact(&source, target)
            && self.valid_status_entity(&source_status)
            && self.valid_status_entity(&target_status)
        {
            let other = self.resolve_entity(target).unwrap();
            // Both resources belong to this one legal effect. Take SP before HP death cleanup.
            removed_spirit = self.take_spirit(other, budget[1]);
            restored_spirit = self.return_spirit(owner, removed_spirit, c, now);
            if budget[0] > 0 {
                let previous = self.triggers.action.clone();
                let previous_source = self.triggers.action_source.clone();
                if let Some(id) = action_id {
                    self.triggers.action = Some(id.into());
                    self.triggers.action_source = Some(source.clone());
                }
                let rule = AttackRule {
                    damage_kind: DamageKind::Physical,
                    ignore_armor: false,
                    unavoidable: true,
                    can_crit: false,
                    ..Default::default()
                };
                removed_hp = self
                    .resolve_damage_mode(
                        combat::DamageIntent {
                            source: source.clone(),
                            target: target.clone(),
                            raw: budget[0],
                            ignore_armor: false,
                            direct: true,
                        },
                        Some(&rule),
                        d.hp_policy == Some(ResourceHpPolicy::DirectTransfer),
                        c,
                        now,
                        events,
                    )
                    .hp_damage;
                self.triggers.action = previous;
                self.triggers.action_source = previous_source;
            }
        }
        let multiplier = self.statuses.healing_received_bps(
            &source_status,
            now,
            c.pve.as_ref().and_then(|p| p.status_rules.as_ref()),
        );
        let adjusted = u64::from(removed_hp) * u64::from(multiplier) / 10000;
        let restored_hp = if removed_hp > 0 {
            self.restore_hp(&source, adjusted.min(u64::from(u32::MAX)) as u32, c, now)
        } else {
            0
        };
        if removed_hp > 0 {
            let event = Event::HealResolved {
                source: source_status.clone(),
                target: source_status.clone(),
                heal_kind: HealKind::ResourceSteal,
                raw_heal: removed_hp,
                multiplier_bps: multiplier,
                adjusted_heal: adjusted,
                hp_restored: restored_hp,
            };
            for i in self.status_observers(&source_status, &source_status) {
                private(events, i, event.clone());
            }
        }
        let event = Event::ResourceStolen {
            action_id: action_id.map(str::to_owned),
            source,
            target: target.clone(),
            hp_policy: d.hp_policy,
            requested_hp: budget[0],
            requested_spirit: budget[1],
            hp_removed: removed_hp,
            spirit_removed: removed_spirit,
            hp_multiplier_bps: multiplier,
            adjusted_hp_return: adjusted,
            hp_restored: restored_hp,
            spirit_restored: restored_spirit,
            hp_excess: adjusted - u64::from(restored_hp),
            spirit_excess: removed_spirit - restored_spirit,
        };
        for i in self.status_observers(&source_status, &target_status) {
            private(events, i, event.clone());
        }
    }
}
