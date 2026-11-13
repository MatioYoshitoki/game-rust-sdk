//! HP, spirit and shields partition one native HOT tick's excess budget.
use super::*;
use crate::config::SpiritConversionWhen;
impl World {
    pub(super) fn periodic_full_hp(&self, target: &StatusEntity, c: &Config, now: u64) -> bool {
        let hp = self.current_hp(self.entity_owner(target).unwrap());
        hp >= self
            .entity_max_hp(&Self::status_reference(target), c, now)
            .unwrap()
    }
    fn restore_periodic_spirit(
        &mut self,
        target: &StatusEntity,
        requested: u32,
        c: &Config,
        now: u64,
    ) -> u32 {
        let maximum = self
            .entity_max_spirit(&Self::status_reference(target), c, now)
            .unwrap();
        match self.entity_owner(target).unwrap() {
            Owner::Member(i) => {
                self.members[i].spirit = self.members[i].spirit.min(maximum);
                let restored = requested.min(self.members[i].spirit_recovery_gap(
                    &self.statuses,
                    now,
                    maximum,
                ));
                self.members[i].spirit += restored;
                restored
            }
            Owner::Npc(i) => {
                self.npcs[i].spirit = self.npcs[i].spirit.min(maximum);
                let restored = requested.min(maximum - self.npcs[i].spirit);
                self.npcs[i].spirit += restored;
                restored
            }
            Owner::Summon(_) => 0,
        }
    }
    pub(super) fn resolve_periodic_spirit(
        &mut self,
        status: &StatusView,
        (raw_spirit, hp_excess, full_before, applied): (u32, u64, bool, bool),
        (due, now, outcome): (u64, u64, PeriodicTickOutcome),
        c: &Config,
    ) -> (u64, Event) {
        let conversion = c.combat_rules.as_ref().unwrap().periodic_recovery[&status.definition_id]
            .overheal_spirit
            .as_ref();
        let conversion_bps = conversion.map_or(0, |v| v.bps);
        let conversion_eligible = applied
            && hp_excess > 0
            && conversion.is_some_and(|v| match v.when {
                SpiritConversionWhen::FullBeforeTick => full_before,
                SpiritConversionWhen::AnyExcess => true,
            });
        let base_restored = if applied {
            self.restore_periodic_spirit(&status.target, raw_spirit, c, now)
        } else {
            0
        };
        let mut converted_spirit = 0;
        let mut spent_heal = 0;
        if conversion_eligible {
            let requested = (u128::from(hp_excess) * u128::from(conversion_bps) / 10000)
                .min(u128::from(u32::MAX)) as u32;
            converted_spirit = self.restore_periodic_spirit(&status.target, requested, c, now);
            spent_heal =
                (u128::from(converted_spirit) * 10000).div_ceil(u128::from(conversion_bps)) as u64;
        }
        let remaining_excess = hp_excess - spent_heal;
        (
            remaining_excess,
            Event::PeriodicSpiritTick {
                stacks: status.stacks,
                instance_id: status.instance_id.clone(),
                definition_id: status.definition_id.clone(),
                source: status.source.clone(),
                target: status.target.clone(),
                tick_index: status.periodic.as_ref().unwrap().ticks_done,
                scheduled_at_ms: due,
                executed_at_ms: now,
                raw_spirit,
                base_restored,
                hp_excess,
                conversion_bps,
                conversion_eligible,
                converted_spirit,
                spent_heal,
                remaining_excess,
                outcome,
            },
        )
    }
}
