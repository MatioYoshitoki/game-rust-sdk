//! Equipment assignments come only from the supervising host.
use super::*;
use crate::config::weapon_equipment::{WeaponCatalog, weapon_id};
use std::collections::{BTreeMap, BTreeSet};

const MAX_WEAPON_OPERATIONS: usize = 128;

fn equipment_receipt(
    catalog: &WeaponCatalog,
    request: &SetEquippedWeapon,
) -> Result<WeaponEquipmentReceipt> {
    if request.entity_generation != 1 || !weapon_id(&request.operation_id) {
        return Err(Error::InvalidMessage);
    }
    let (basic_profile, weapon) = if let Some(instance) = &request.weapon {
        let (profile, equipped) = catalog.resolve(instance).ok_or(Error::InvalidMessage)?;
        (Some(profile.to_owned()), Some(equipped))
    } else {
        (None, None)
    };
    let revision = request
        .expected_revision
        .checked_add(1)
        .ok_or(Error::Capacity)?;
    Ok(WeaponEquipmentReceipt {
        operation_id: request.operation_id.clone(),
        equipment: WeaponEquipmentView { revision, weapon },
        basic_profile,
    })
}

fn unique_instance(
    slots: &BTreeMap<&str, (u32, Option<&WeaponInstance>)>,
    participant: &str,
    weapon: Option<&WeaponInstance>,
) -> bool {
    weapon.is_none_or(|w| {
        slots.iter().all(|(&pid, (_, equipped))| {
            pid == participant || equipped.is_none_or(|other| other.instance_id != w.instance_id)
        })
    })
}

impl TwoMembers {
    pub(super) fn initial_weapon_operations(
        c: &Config,
        create: &CreateV4,
        recovery: Option<&RecoveryV4>,
    ) -> Result<Vec<WeaponEquipmentOperation>> {
        let operations =
            recovery.map_or(&[][..], |r| r.checkpoint.state.weapon_operations.as_slice());
        if operations.is_empty() && create.participants.iter().all(|seat| seat.weapon.is_none()) {
            return Ok(vec![]);
        }
        if create.protocol_version != 6 || c.mechanism_version < 105 {
            return Err(Error::Unsupported);
        }
        let catalog = c.weapon_catalog().ok_or(Error::Unsupported)?;
        if operations.len() > MAX_WEAPON_OPERATIONS {
            return Err(Error::InvalidMessage);
        }
        let mut slots = BTreeMap::new();
        for seat in &create.participants {
            if let Some(weapon) = &seat.weapon {
                if seat.basic_profile.is_some() || catalog.resolve(weapon).is_none() {
                    return Err(Error::InvalidMessage);
                }
                if !unique_instance(&slots, &seat.participant_id, Some(weapon)) {
                    return Err(Error::Conflict);
                }
            }
            slots.insert(seat.participant_id.as_str(), (0, seat.weapon.as_ref()));
        }
        let mut seen = BTreeSet::new();
        for op in operations {
            let request = &op.request;
            let participant = request.participant_id.as_str();
            let before = slots.get(participant).ok_or(Error::InvalidMessage)?;
            if request.key != create.key
                || !seen.insert(request.operation_id.as_str())
                || request.expected_revision != before.0
                || !unique_instance(&slots, participant, request.weapon.as_ref())
                || equipment_receipt(catalog, request) != Ok(op.receipt.clone())
            {
                return Err(Error::InvalidMessage);
            }
            slots.insert(
                participant,
                (op.receipt.equipment.revision, request.weapon.as_ref()),
            );
        }
        Ok(operations.to_vec())
    }

    pub(crate) fn set_equipped_weapon(
        &mut self,
        request: SetEquippedWeapon,
    ) -> Result<WeaponEquipmentReceipt> {
        if request.key != self.create.key {
            return Err(Error::StaleMatch);
        }
        let catalog = self.config.weapon_catalog().ok_or(Error::Unsupported)?;
        if self.create.protocol_version != 6 || self.config.mechanism_version < 105 {
            return Err(Error::Unsupported);
        }
        if let Some(op) = self
            .weapon_operations
            .iter()
            .find(|op| op.request.operation_id == request.operation_id)
        {
            return if op.request == request {
                Ok(op.receipt.clone())
            } else {
                Err(Error::Conflict)
            };
        }
        if matches!(self.phase, Phase::Closed | Phase::Faulted) {
            return Err(Error::Closed);
        }
        let i = self
            .seats
            .iter()
            .position(|s| s.original.participant_id == request.participant_id)
            .ok_or(Error::Unauthorized)?;
        self.reserve()?;
        let world = self.world.as_mut().ok_or(Error::NotReady)?;
        if world.equipment_revision(i) != request.expected_revision {
            return Err(Error::RevisionConflict);
        }
        if self.weapon_operations.len() >= MAX_WEAPON_OPERATIONS {
            return Err(Error::Capacity);
        }
        let receipt = equipment_receipt(catalog, &request)?;
        let events = world.equip_weapon(i, &receipt, &self.config, self.now)?;
        self.weapon_operations.push(WeaponEquipmentOperation {
            request,
            receipt: receipt.clone(),
        });
        self.events(events);
        self.dirty = true;
        Ok(receipt)
    }
}
