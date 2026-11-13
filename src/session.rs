//! Version dispatch at the public boundaries; the single-member engine stays isolated.
use crate::{config::Config, engine::Engine, protocol::*, two_members::TwoMembers};
use std::sync::Arc;
pub(crate) struct Session {
    single: Engine,
    two: Option<TwoMembers>,
    catalog: Arc<crate::ConfigCatalog>,
    single_created: bool,
    io_ms: u64,
}
impl Session {
    pub(crate) fn new(config: Config, catalog: Arc<crate::ConfigCatalog>) -> Self {
        Self {
            single: Engine::with_catalog(config, catalog.clone()),
            catalog,
            two: None,
            single_created: false,
            io_ms: 0,
        }
    }
    pub(crate) fn config(&self) -> &Config {
        self.two.as_ref().map_or(&self.single.config, |t| &t.config)
    }
    pub(crate) fn create(&mut self, r: CreateRequest) -> Result<Status> {
        match r {
            CreateRequest::Single(r) => {
                if self.two.is_some() {
                    return Err(Error::Conflict);
                }
                if r.protocol_version >= 4
                    || self
                        .catalog
                        .find(&r.config_id, &r.config_hash)
                        .is_some_and(|c| c.protocol_version() >= 4)
                {
                    return Err(Error::VersionMismatch);
                }
                let status = self.single.create(r)?;
                self.single_created = true;
                Ok(status)
            }
            CreateRequest::Replicated(r) => {
                if r.protocol_version != 6 {
                    return Err(Error::VersionMismatch);
                }
                self.create_members(r.into())
            }
            CreateRequest::Two(r) => {
                if r.protocol_version != 4 {
                    return Err(Error::VersionMismatch);
                }
                self.create_members(r)
            }
            CreateRequest::Members(r) => {
                if r.protocol_version != 5 {
                    return Err(Error::VersionMismatch);
                }
                self.create_members(r.into())
            }
        }
    }
    fn create_members(&mut self, r: CreateV4) -> Result<Status> {
        if self.single_created {
            return Err(Error::Conflict);
        }
        if let Some(two) = &self.two {
            return two.repeat_create(r);
        }
        let config = self
            .catalog
            .find(&r.config_id, &r.config_hash)
            .ok_or(Error::ConfigMismatch)?;
        let mut next = TwoMembers::create(config.as_ref().clone(), self.catalog.clone(), r)?;
        next.align_service_clock(self.io_ms);
        let status = next.status();
        self.two = Some(next);
        Ok(status)
    }
    pub(crate) fn input(&mut self, r: Input) -> Result<Status> {
        match &mut self.two {
            Some(t) => t.input(r),
            None => self.single.input(r),
        }
    }
    pub(crate) fn business(&mut self, r: BusinessReceiptRequest) -> Result<Status> {
        match (&mut self.two, r) {
            (Some(t), BusinessReceiptRequest::Member(r)) => t.business(r),
            (None, BusinessReceiptRequest::Single(r)) => self.single.business(r),
            _ => Err(Error::VersionMismatch),
        }
    }
    pub(crate) fn set_skill_loadout(
        &mut self,
        request: SetSkillLoadout,
    ) -> Result<SkillLoadoutReceipt> {
        self.two
            .as_mut()
            .ok_or(Error::Unsupported)?
            .set_skill_loadout(request)
    }
    pub(crate) fn set_equipped_weapon(
        &mut self,
        request: SetEquippedWeapon,
    ) -> Result<WeaponEquipmentReceipt> {
        self.two
            .as_mut()
            .ok_or(Error::Unsupported)?
            .set_equipped_weapon(request)
    }
    pub(crate) fn transport(&mut self, r: TransportReceipt) -> Result<Status> {
        match &mut self.two {
            Some(t) => t.transport(r),
            None => self.single.transport(r),
        }
    }
    pub(crate) fn spectate(&mut self, request: SpectateV6) -> Result<Status> {
        self.two
            .as_mut()
            .ok_or(Error::Unsupported)?
            .spectate(request)
    }
    pub(crate) fn close(&mut self, r: Close) -> Result<Status> {
        match &mut self.two {
            Some(t) => t.close(r),
            None => self.single.close(r),
        }
    }
    pub(crate) fn drain(&mut self) -> Vec<Output> {
        match &mut self.two {
            Some(t) => t.drain(),
            None => self.single.drain(),
        }
    }
    pub(crate) fn restore_undelivered(&mut self, v: Vec<Output>) {
        match &mut self.two {
            Some(t) => t.restore_undelivered(v),
            None => self.single.restore_undelivered(v),
        }
    }
    pub(crate) fn tick(&mut self) {
        match &mut self.two {
            Some(t) => t.tick(),
            None => self.single.tick(),
        }
    }
    pub(crate) fn poll_io(&mut self, ms: u64) {
        self.io_ms = self.io_ms.max(ms);
        match &mut self.two {
            Some(t) => t.poll_io(ms),
            None => self.single.poll_io(ms),
        }
    }
    pub(crate) fn fault(&mut self, e: Error) {
        match &mut self.two {
            Some(t) => t.fault(e),
            None => self.single.fault(e),
        }
    }
}
