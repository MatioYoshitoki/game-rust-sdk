use super::*;

impl World {
    pub(crate) fn equipment_revision(&self, i: usize) -> u32 {
        self.members[i]
            .equipment
            .as_ref()
            .map_or(0, |view| view.revision)
    }

    pub(crate) fn equip_weapon(
        &mut self,
        i: usize,
        receipt: &WeaponEquipmentReceipt,
        c: &Config,
        now: u64,
    ) -> Result<Events> {
        if !self.loadout_mutable(i) {
            return Err(Error::InvalidState);
        }
        let next = &receipt.equipment.weapon;
        if let Some(weapon) = next
            && self.members.iter().enumerate().any(|(other, m)| {
                other != i
                    && m.equipment
                        .as_ref()
                        .and_then(|v| v.weapon.as_ref())
                        .is_some_and(|w| w.instance_id == weapon.instance_id)
            })
        {
            return Err(Error::Conflict);
        }
        let m = &self.members[i];
        let current = m.equipment.as_ref().and_then(|v| v.weapon.as_ref());
        let changing = current != next.as_ref() || m.basic_profile != receipt.basic_profile;
        let mut events = vec![];
        if changing && m.world_weapon_profile.is_none() {
            self.prepare_basic_switch(i, c, now, &mut events)?;
        }
        let m = &mut self.members[i];
        m.equipment = Some(receipt.equipment.clone());
        m.basic_profile.clone_from(&receipt.basic_profile);
        private(
            &mut events,
            i,
            Event::WeaponEquipmentChanged {
                equipment: receipt.equipment.clone(),
                basic_profile: receipt.basic_profile.clone(),
            },
        );
        Ok(events)
    }

    pub(super) fn prepare_basic_switch(
        &mut self,
        i: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Result<()> {
        let m = &self.members[i];
        if m.basic_delivery.is_some() && m.basic_beam.is_none()
            || m.basic_projectile.is_some()
            || self.shots.iter().any(|s| {
                s.owner == Owner::Member(i) && s.origin == crate::origin::ActionOrigin::Basic
            })
        {
            return Err(Error::InvalidState);
        }
        let floor = m.basic(c).stacked_wave.as_ref().and_then(|upgrade| {
            let source = status_entity(&m.reference.entity);
            self.statuses
                .entries()
                .iter()
                .find(|s| {
                    s.source == source
                        && s.target == source
                        && s.definition_id == upgrade.stack_status
                })
                .map(|s| s.instance_id.clone())
        });
        if m.basic_beam.is_some() {
            self.end_basic_beam(i, BasicBeamEndReason::EquipmentChanged, events);
            self.finish_basic_delivery(i, c, now, events);
        }
        let m = &mut self.members[i];
        let pending = m.pending.take_if(|p| p.skill.is_none()).is_some();
        let pursuit = m.basic_pursuit.take().is_some();
        let queued = m.queued_basic.take().is_some();
        let charge = m.basic_charge.take().is_some();
        if pending || pursuit || queued || charge {
            private(events, i, Event::ActionInterrupted);
        }
        m.basic_wave_phase = BasicWavePhase::Primary;
        m.basic_wave_target_until.clear();
        m.basic_projectile_hits = 0;
        m.basic_seals.clear();
        m.basic_ricochet_marks.clear();
        if let Some(floor) = floor {
            self.remove_basic_stack(&floor, StatusRemovalReason::Reset, events);
        }
        Ok(())
    }
}
