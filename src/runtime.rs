use crate::{protocol::*, session::Session};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

enum Command {
    Create(CreateRequest),
    Input(Input),
    SetSkillLoadout(SetSkillLoadout),
    SetEquippedWeapon(SetEquippedWeapon),
    Business(BusinessReceiptRequest),
    Transport(TransportReceipt),
    Spectate(SpectateV6),
    Drain,
    Close(Close),
}

enum Response {
    Status(Status),
    Outputs(Vec<Output>),
    SkillLoadout(SkillLoadoutReceipt),
    WeaponEquipment(WeaponEquipmentReceipt),
}
type Envelope = (Command, mpsc::SyncSender<Result<Response>>);

/// A data-only client. Calls enter one bounded ordered mailbox; no World references escape.
#[derive(Clone)]
pub struct Client {
    sender: mpsc::SyncSender<Envelope>,
    call_timeout: Duration,
}

impl Client {
    fn call(&self, command: Command) -> Result<Response> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send((command, sender))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => Error::QueueFull,
                mpsc::TrySendError::Disconnected(_) => Error::Closed,
            })?;
        receiver
            .recv_timeout(self.call_timeout)
            .map_err(|_| Error::ExternalUnknown)?
    }

    fn status_call(&self, command: Command) -> Result<Status> {
        match self.call(command)? {
            Response::Status(status) => Ok(status),
            _ => Err(Error::Faulted),
        }
    }

    pub fn create(&self, request: impl Into<CreateRequest>) -> Result<Status> {
        self.status_call(Command::Create(request.into()))
    }
    /// Host-trusted grant/update. Do not expose this method as an ordinary player action.
    pub fn set_skill_loadout(&self, request: SetSkillLoadout) -> Result<SkillLoadoutReceipt> {
        match self.call(Command::SetSkillLoadout(request))? {
            Response::SkillLoadout(receipt) => Ok(receipt),
            _ => Err(Error::Faulted),
        }
    }
    /// Updates the match weapon from a host-authenticated equipment assignment.
    pub fn set_equipped_weapon(
        &self,
        request: SetEquippedWeapon,
    ) -> Result<WeaponEquipmentReceipt> {
        match self.call(Command::SetEquippedWeapon(request))? {
            Response::WeaponEquipment(receipt) => Ok(receipt),
            _ => Err(Error::Faulted),
        }
    }
    pub fn submit_input(&self, request: Input) -> Result<Status> {
        self.status_call(Command::Input(request))
    }
    pub fn submit_business_receipt(
        &self,
        request: impl Into<BusinessReceiptRequest>,
    ) -> Result<Status> {
        self.status_call(Command::Business(request.into()))
    }
    pub fn submit_transport_receipt(&self, request: TransportReceipt) -> Result<Status> {
        self.status_call(Command::Transport(request))
    }
    /// Bind and control a read-only slot after the host authenticates the viewer.
    pub fn spectate(&self, request: SpectateV6) -> Result<Status> {
        self.status_call(Command::Spectate(request))
    }
    pub fn close(&self, request: Close) -> Result<Status> {
        self.status_call(Command::Close(request))
    }
    pub fn drain_outputs(&self) -> Result<Vec<Output>> {
        match self.call(Command::Drain)? {
            Response::Outputs(outputs) => Ok(outputs),
            _ => Err(Error::Faulted),
        }
    }

    #[cfg(all(test, match_sdk_test_data))]
    pub(crate) fn abandon_drain_for_test(&self) {
        let (reply, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        self.sender
            .try_send((Command::Drain, reply))
            .unwrap_or_else(|_| panic!("test mailbox full"));
    }
}

/// Owns the scheduling thread. Dropping it disconnects clients and joins the thread.
pub struct Runtime {
    client: Option<Client>,
    thread: Option<thread::JoinHandle<()>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Runtime {
    /// Uses a host-supplied catalog from MATCH_CONFIG_MANIFEST or --configs.
    /// Prefer with_catalog to handle configuration errors before starting the runtime.
    pub fn spawn() -> Self {
        Self::with_catalog(crate::config::default_catalog())
    }

    /// Explicitly accelerated verification clock; absent from ordinary builds.
    #[cfg(feature = "replay")]
    pub fn spawn_verification() -> Self {
        Self::verification_with_catalog(crate::config::default_catalog())
    }

    pub fn with_catalog(catalog: std::sync::Arc<crate::config::ConfigCatalog>) -> Self {
        Self::start(false, catalog)
    }
    #[cfg(feature = "replay")]
    pub fn verification_with_catalog(
        catalog: std::sync::Arc<crate::config::ConfigCatalog>,
    ) -> Self {
        Self::start(true, catalog)
    }
    fn start(verification: bool, catalog: std::sync::Arc<crate::config::ConfigCatalog>) -> Self {
        let config = catalog.default_config().as_ref().clone();
        let mut interval = Duration::from_millis(if verification { 1 } else { config.step_ms });
        let (sender, receiver) = mpsc::sync_channel::<Envelope>(config.input_capacity);
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_stop = stop.clone();
        let call_timeout = Duration::from_millis(config.params.runtime.call_timeout_ms);
        let handle = thread::Builder::new()
            .name("match-kernel".into())
            .spawn(move || {
                let mut engine = Session::new(config, catalog);
                let service_started = Instant::now();
                let mut deadline = Instant::now() + interval;
                while !thread_stop.load(std::sync::atomic::Ordering::Acquire) {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    let incoming = receiver.recv_timeout(remaining);
                    if catch_unwind(AssertUnwindSafe(|| {
                        engine.poll_io(
                            service_started
                                .elapsed()
                                .as_millis()
                                .min(u128::from(u64::MAX)) as u64,
                        )
                    }))
                    .is_err()
                    {
                        engine.fault(Error::Faulted);
                    }
                    match incoming {
                        Ok((command, reply)) => {
                            let result = catch_unwind(AssertUnwindSafe(|| match command {
                                Command::Create(request) => {
                                    engine.create(request).map(Response::Status)
                                }
                                Command::SetSkillLoadout(request) => engine
                                    .set_skill_loadout(request)
                                    .map(Response::SkillLoadout),
                                Command::SetEquippedWeapon(request) => engine
                                    .set_equipped_weapon(request)
                                    .map(Response::WeaponEquipment),
                                Command::Input(request) => {
                                    engine.input(request).map(Response::Status)
                                }
                                Command::Business(request) => {
                                    engine.business(request).map(Response::Status)
                                }
                                Command::Transport(request) => {
                                    engine.transport(request).map(Response::Status)
                                }
                                Command::Spectate(request) => {
                                    engine.spectate(request).map(Response::Status)
                                }
                                Command::Close(request) => {
                                    engine.close(request).map(Response::Status)
                                }
                                Command::Drain => Ok(Response::Outputs(engine.drain())),
                            }));
                            let result = result.unwrap_or_else(|_| {
                                engine.fault(Error::Faulted);
                                Err(Error::Faulted)
                            });
                            if let Err(mpsc::SendError(Ok(Response::Outputs(outputs)))) =
                                reply.send(result)
                            {
                                engine.restore_undelivered(outputs);
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    let next_interval = Duration::from_millis(if verification {
                        1
                    } else {
                        engine.config().step_ms
                    });
                    if next_interval != interval {
                        interval = next_interval;
                        deadline = Instant::now() + interval;
                    }
                    if Instant::now() >= deadline {
                        let lag = Instant::now().saturating_duration_since(deadline);
                        if !verification
                            && lag > interval * engine.config().params.runtime.max_lag_steps
                        {
                            engine.fault(Error::Faulted);
                            deadline = Instant::now() + interval;
                        } else {
                            let count = if verification {
                                1
                            } else {
                                (lag.as_nanos() / interval.as_nanos() + 1).min(u128::from(
                                    engine.config().params.runtime.max_catchup_steps,
                                )) as u32
                            };
                            for _ in 0..count {
                                if catch_unwind(AssertUnwindSafe(|| engine.tick())).is_err() {
                                    engine.fault(Error::Faulted);
                                    break;
                                }
                            }
                            deadline = if verification {
                                Instant::now() + interval
                            } else {
                                deadline + interval * count
                            };
                        }
                    }
                }
            })
            .expect("create kernel worker");
        Self {
            client: Some(Client {
                sender,
                call_timeout,
            }),
            thread: Some(handle),
            stop,
        }
    }

    pub fn client(&self) -> Client {
        self.client.as_ref().expect("live runtime").clone()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        self.client.take();
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}
