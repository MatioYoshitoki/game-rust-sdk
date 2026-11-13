use super::*;

impl Monster {
    fn phase(&self) -> Option<&crate::config::BossPhase> {
        self.profile
            .boss
            .as_ref()
            .map(|b| &b.phases[self.boss_phase])
    }
    pub(super) fn attack_interval(&self) -> u64 {
        self.phase()
            .map_or(self.profile.combat.attack_interval_ms, |p| {
                p.attack_interval_ms
            })
    }
    pub(super) fn cast_chance(&self) -> f64 {
        self.phase()
            .map_or(self.profile.combat.cast_chance, |p| p.cast_chance)
    }
    pub(super) fn boss_view(&self) -> Option<BossPhaseView> {
        self.phase().map(|p| BossPhaseView {
            phase_id: p.id.clone(),
            phase_name: p.name.clone(),
            phase_index: self.boss_phase as u32 + 1,
            phase_count: self.profile.boss.as_ref().unwrap().phases.len() as u32,
            epoch: self.boss_epoch,
            attack_interval_ms: p.attack_interval_ms,
            cast_chance: p.cast_chance,
        })
    }
}
impl Arena {
    pub(super) fn advance_boss(&mut self, index: usize, events: &mut Vec<Event>) {
        let m = &mut self.monsters[index];
        let Some(b) = &m.profile.boss else {
            return;
        };
        let next = b
            .phases
            .iter()
            .enumerate()
            .skip(m.boss_phase + 1)
            .filter(|(_, p)| {
                u64::from(m.hp) * 10000
                    <= u64::from(m.profile.stats["hp"]) * u64::from(p.hp_threshold_bps)
            })
            .map(|(i, _)| i)
            .next_back();
        if let Some(next) = next {
            let previous = m.boss_phase;
            m.boss_phase = next;
            let p = &b.phases[next];
            events.push(Event::BossPhaseChanged {
                monster_id: m.id.clone(),
                epoch: m.boss_epoch,
                from_index: previous as u32 + 1,
                phase_id: p.id.clone(),
                phase_name: p.name.clone(),
                phase_index: next as u32 + 1,
            });
        }
    }
    pub(super) fn reset_boss(&mut self, index: usize, now: u64, events: &mut Vec<Event>) {
        self.clear_status_entity(
            &self.status_entity(index),
            StatusRemovalReason::Reset,
            events,
        );
        self.monsters[index].status_generation += 1;
        let m = &mut self.monsters[index];
        if m.profile.boss.is_none() {
            return;
        }
        m.boss_phase = 0;
        m.boss_epoch += 1;
        m.next_attack = now + m.attack_interval();
        let p = m.phase().unwrap();
        events.push(Event::BossReset {
            monster_id: m.id.clone(),
            epoch: m.boss_epoch,
            phase_id: p.id.clone(),
            phase_name: p.name.clone(),
        });
    }
}
