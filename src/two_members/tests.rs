use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn catalog_with(change: impl FnOnce(&mut Value)) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "kernel-two-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir)?;
    let mut v: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/two-member-host/arena-a.json"
    ))
    .unwrap();
    change(&mut v);
    std::fs::write(dir.join("arena.json"), serde_json::to_vec(&v).unwrap())?;
    std::fs::write(dir.join("catalog.json"),br#"{"schema_version":1,"default_scenario":"pair","active":{"pair":"arena.json"},"archives":[]}"#)?;
    let result = crate::ConfigCatalog::load(dir.join("catalog.json")).map(Arc::new);
    std::fs::remove_dir_all(dir)?;
    result
}
fn create(c: &Config) -> CreateV4 {
    CreateV4 {
        protocol_version: 4,
        key: MatchKey {
            id: "two-test".into(),
            incarnation: 7,
        },
        request_id: "create".into(),
        config_id: c.id.clone(),
        config_hash: c.hash(),
        seed: 7,
        participants: vec![
            ParticipantSeat {
                weapon: None,
                basic_profile: None,
                combat_build: None,
                skills: None,
                participant_id: "A".into(),
                authenticated_principal: "principal-A".into(),
                connection: Connection {
                    id: "conn-A".into(),
                    generation: 1,
                },
            },
            ParticipantSeat {
                weapon: None,
                basic_profile: None,
                combat_build: None,
                skills: None,
                participant_id: "B".into(),
                authenticated_principal: "principal-B".into(),
                connection: Connection {
                    id: "conn-B".into(),
                    generation: 1,
                },
            },
        ],
        publication_mode: PublicationMode::Interactive,
        recovery: None,
    }
}

#[derive(Clone)]
struct Store {
    revision: u32,
    principal: String,
    hp: u32,
    spirit: u32,
    balances: BTreeMap<String, u32>,
    requests: BTreeMap<String, Business>,
    proofs: BTreeMap<String, ReceiptOutcome>,
    admissions: u32,
    settlements: u32,
    compensations: u32,
}
impl Store {
    fn new(i: usize, c: &Config) -> Self {
        Self {
            revision: i as u32 * 10,
            principal: format!("principal-{}", if i == 0 { "A" } else { "B" }),
            hp: 1000 + i as u32 * 200,
            spirit: 200,
            balances: BTreeMap::from([
                (c.item_id.clone(), 100),
                (
                    c.asset_contract
                        .as_ref()
                        .unwrap()
                        .entry_cost_asset_id
                        .clone(),
                    100,
                ),
                (
                    c.asset_contract.as_ref().unwrap().reward_asset_id.clone(),
                    0,
                ),
            ]),
            requests: BTreeMap::new(),
            proofs: BTreeMap::new(),
            admissions: 0,
            settlements: 0,
            compensations: 0,
        }
    }
    fn execute(&mut self, r: &BusinessRequest) -> ReceiptOutcome {
        if let Some(old) = self.requests.get(&r.operation_id) {
            assert_eq!(old, &r.body)
        } else {
            self.requests.insert(r.operation_id.clone(), r.body.clone());
        }
        if let Some(p) = self.proofs.get(&r.operation_id) {
            return p.clone();
        }
        let proof = match &r.body {
            Business::LoadAssets {} => ReceiptOutcome::AssetsLoaded {
                facts: AssetParticipantFacts {
                    instance_count: None,
                    warehouse_other_slots: None,
                    location: None,
                    principal: self.principal.clone(),
                    revision: self.revision,
                    hp_points: self.hp,
                    spirit_points: self.spirit,
                    atk_points: 100,
                    def_points: 100,
                    spd_points: 8,
                    assets: self
                        .balances
                        .iter()
                        .map(|(id, &q)| AssetAmount {
                            asset_id: id.clone(),
                            quantity: q,
                        })
                        .collect(),
                },
            },
            Business::CommitAssets { transaction } => {
                let (revision, rows, debit) = match transaction {
                    AssetTransaction::Admission {
                        expected_revision,
                        debits,
                        ..
                    } => (*expected_revision, debits, true),
                    AssetTransaction::ResultSettlement {
                        expected_revision,
                        credits,
                        ..
                    }
                    | AssetTransaction::Settlement {
                        expected_revision,
                        credits,
                        ..
                    } => (*expected_revision, credits, false),
                    AssetTransaction::Compensation {
                        original_operation_id,
                        credits,
                        ..
                    } => {
                        let Some(ReceiptOutcome::Committed { revision, .. }) =
                            self.proofs.get(original_operation_id)
                        else {
                            panic!("refund without debit proof")
                        };
                        let Business::CommitAssets {
                            transaction: AssetTransaction::Admission { debits, .. },
                        } = &self.requests[original_operation_id]
                        else {
                            panic!("bad original")
                        };
                        assert_eq!(credits, debits);
                        (*revision, credits, false)
                    }
                };
                if revision != self.revision
                    || debit
                        && rows.iter().any(|r| {
                            self.balances.get(&r.asset_id).copied().unwrap_or(0) < r.quantity
                        })
                {
                    ReceiptOutcome::Refused {}
                } else {
                    for r in rows {
                        let balance = self.balances.entry(r.asset_id.clone()).or_default();
                        if debit {
                            *balance -= r.quantity
                        } else {
                            *balance += r.quantity
                        }
                    }
                    self.revision += 1;
                    match transaction {
                        AssetTransaction::Admission { .. } => self.admissions += 1,
                        AssetTransaction::Compensation { .. } => self.compensations += 1,
                        _ => self.settlements += 1,
                    }
                    ReceiptOutcome::Committed {
                        revision: self.revision,
                        commit_id: format!("{}-{}", self.principal, self.revision),
                    }
                }
            }
            Business::QueryOperation { operation_id } => self
                .proofs
                .get(operation_id)
                .cloned()
                .unwrap_or(ReceiptOutcome::NotCommitted {}),
            Business::ReconcileOperation { operation_id } => match self.proofs.get(operation_id) {
                Some(ReceiptOutcome::Committed {
                    revision,
                    commit_id,
                }) => ReceiptOutcome::Reconciled {
                    revision: *revision,
                    commit_id: commit_id.clone(),
                },
                _ => ReceiptOutcome::Unknown {},
            },
            _ => panic!("noncanonical account request"),
        };
        if !matches!(proof, ReceiptOutcome::Unknown {}) {
            self.proofs.insert(r.operation_id.clone(), proof.clone());
        }
        proof
    }
}
struct Harness {
    engine: TwoMembers,
    stores: [Store; 2],
    sequences: [u32; 2],
    snapshots: [Option<(String, SnapshotV4)>; 2],
    events: Vec<(Option<String>, EventV4)>,
    held: Vec<(usize, BusinessRequest)>,
    hold_admit: [bool; 2],
    hold_settle: [bool; 2],
    ack: [bool; 2],
    checkpoint: Option<IoCheckpointV4>,
}
impl Harness {
    fn new(change: impl FnOnce(&mut Value)) -> Self {
        let catalog = catalog_with(change).unwrap();
        let c = catalog.default_config().as_ref().clone();
        let engine = TwoMembers::create(c.clone(), catalog, create(&c)).unwrap();
        Self {
            engine,
            stores: [Store::new(0, &c), Store::new(1, &c)],
            sequences: [0; 2],
            snapshots: [None, None],
            events: vec![],
            held: vec![],
            hold_admit: [false; 2],
            hold_settle: [false; 2],
            ack: [true; 2],
            checkpoint: None,
        }
    }
    fn pump(&mut self) {
        for _ in 0..40 {
            let outputs = self.engine.drain();
            if outputs.is_empty() {
                return;
            }
            for o in outputs {
                match o.body {
                    OutputBody::BusinessRequestV4 {
                        participant_id,
                        request,
                        ..
                    } => {
                        let i = usize::from(participant_id == "B");
                        let admission = matches!(
                            request.body,
                            Business::CommitAssets {
                                transaction: AssetTransaction::Admission { .. }
                            }
                        );
                        let settlement = matches!(
                            request.body,
                            Business::CommitAssets {
                                transaction: AssetTransaction::ResultSettlement { .. }
                                    | AssetTransaction::Settlement { .. }
                            } | Business::QueryOperation { .. }
                                | Business::ReconcileOperation { .. }
                        );
                        if self.hold_admit[i] && admission || self.hold_settle[i] && settlement {
                            self.held.push((i, request));
                            continue;
                        }
                        let outcome = self.stores[i].execute(&request);
                        self.receipt(i, &request, outcome).unwrap();
                    }
                    OutputBody::PublicationV4 {
                        recipient_id,
                        connection,
                        snapshot,
                    } => {
                        let i = usize::from(recipient_id == "B");
                        if self.ack[i] {
                            let result = self.engine.transport(TransportReceipt {
                                key: o.key,
                                output_id: o.id.clone(),
                                recipient_id,
                                connection,
                                outcome: TransportOutcome::Sent,
                            });
                            assert!(result.is_ok() || result == Err(Error::Closed));
                        }
                        self.snapshots[i] = Some((o.id, snapshot));
                    }
                    OutputBody::EventV4 {
                        recipient_id,
                        event,
                    } => self.events.push((recipient_id, event)),
                    OutputBody::ResponsibilityV4 { checkpoint } => {
                        self.checkpoint = Some(*checkpoint)
                    }
                    _ => panic!("legacy output escaped v4"),
                }
            }
        }
        panic!("nonquiescent output loop")
    }
    fn receipt(
        &mut self,
        i: usize,
        r: &BusinessRequest,
        outcome: ReceiptOutcome,
    ) -> Result<Status> {
        self.engine.business(MemberBusinessReceipt {
            key: self.engine.create.key.clone(),
            participant_id: self.engine.seats[i].original.participant_id.clone(),
            operation_id: r.operation_id.clone(),
            attempt_id: r.attempt_id,
            outcome,
        })
    }
    fn input(&mut self, i: usize, action: Action) -> Input {
        self.sequences[i] += 1;
        Input {
            key: self.engine.create.key.clone(),
            participant_id: self.engine.seats[i].original.participant_id.clone(),
            entity_generation: 1,
            connection: self.engine.seats[i].connection.clone(),
            request_id: format!("r{}", self.sequences[i]),
            sequence: self.sequences[i],
            action,
        }
    }
    fn action(&mut self, i: usize, action: Action) -> Result<Status> {
        let input = self.input(i, action);
        self.engine.input(input)
    }
    fn join(&mut self) {
        self.pump();
        for i in 0..2 {
            self.action(i, Action::Join {}).unwrap();
            self.pump();
        }
    }
    fn active(&mut self) {
        self.join();
        for i in 0..2 {
            let id = self.snapshots[i].as_ref().unwrap().0.clone();
            self.action(i, Action::CombatReady { publication_id: id })
                .unwrap();
            self.pump();
        }
        assert_eq!(self.engine.phase, Phase::Active);
    }
    fn ticks(&mut self, n: usize) {
        for _ in 0..n {
            self.engine.tick();
            self.pump();
        }
    }
    fn close(&mut self) {
        self.engine
            .close(Close {
                key: self.engine.create.key.clone(),
                request_id: "close".into(),
            })
            .unwrap();
        self.pump();
    }
    fn snapshot(&self, i: usize) -> &SnapshotV4 {
        &self.snapshots[i].as_ref().unwrap().1
    }
    fn fire(&mut self, i: usize) {
        self.action(
            i,
            Action::Cast {
                skill_id: self.engine.config.skill.id.clone(),
                direction: [1., 0.],
            },
        )
        .unwrap();
    }
}

#[test]
fn two_configurations_normalize_protocol4_and_preserve_legacy_protocols() {
    let catalog = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/two-member-host/catalog.json"),
    )
    .unwrap();
    for alias in ["pair-a", "pair-b"] {
        let c = catalog.active(alias).unwrap();
        assert_eq!(c.protocol_version(), 4);
        assert_eq!(c.schema_version, 5);
        assert_eq!(c.mechanism_version, 35);
    }
    assert_eq!(crate::Config::frozen().protocol_version(), 1);
}
#[test]
fn two_create_strict_counts_identity_namespace_and_atomic_rejection() {
    let cat = catalog_with(|_| {}).unwrap();
    let c = cat.default_config().as_ref().clone();
    let good = create(&c);
    for count in [0, 1, 3] {
        let mut r = good.clone();
        r.participants = vec![good.participants[0].clone(); count];
        assert!(TwoMembers::create(c.clone(), cat.clone(), r).is_err());
    }
    for field in 0..4 {
        let mut r = good.clone();
        match field {
            0 => r.participants[1].participant_id = r.participants[0].participant_id.clone(),
            1 => {
                r.participants[1].authenticated_principal =
                    r.participants[0].authenticated_principal.clone()
            }
            2 => r.participants[1].connection = r.participants[0].connection.clone(),
            _ => r.participants[0].participant_id = World::npc_ids(&c)[0].clone(),
        };
        assert!(TwoMembers::create(c.clone(), cat.clone(), r).is_err());
    }
    let mut wire = serde_json::to_value(good).unwrap();
    wire["participant_id"] = json!("legacy");
    assert!(serde_json::from_value::<CreateRequest>(wire).is_err());
}
#[test]
fn two_configuration_bounds_and_dormant_capabilities_are_rejected() {
    for path in [
        "/two_members/projectile_capacity",
        "/two_members/publication_queue_capacity",
        "/two_members/deployment_timeout_ms",
    ] {
        assert!(catalog_with(|v| *v.pointer_mut(path).unwrap() = json!(0)).is_err());
    }
    assert!(catalog_with(|v| v["two_members"]["spawns"][1] = v["spawn"].clone()).is_err());
    assert!(catalog_with(|v| v["two_members"]["spawns"][1] = json!([-1., -1.])).is_err());
    assert!(catalog_with(|v| v["pve"]["combat"]["heal"]["hp_threshold"] = json!(0.3)).is_err());
    assert!(catalog_with(|v|v["pve"]["status_definitions"][0]["attribute"]=json!("healing_received")).is_err());
    assert!(catalog_with(|v| v["schema_version"] = json!(4)).is_err());
    assert!(
        catalog_with(|v| {
            v.as_object_mut().unwrap().remove("two_members");
        })
        .is_err()
    );
}

#[test]
fn two_delayed_create_uses_its_own_service_clock_deadlines() {
    let catalog = catalog_with(|_| {}).unwrap();
    let config = catalog.default_config().as_ref().clone();
    let mut session = crate::session::Session::new(config.clone(), catalog);
    session.poll_io(100_000);
    session.create(CreateRequest::Two(create(&config))).unwrap();
    session.poll_io(100_001);
    let outputs = session.drain();
    assert_eq!(
        outputs
            .iter()
            .filter(|o| matches!(o.body, OutputBody::BusinessRequestV4 { .. }))
            .count(),
        2
    );
    assert!(!outputs.iter().any(|o| matches!(
        o.body,
        OutputBody::EventV4 {
            event: EventV4::Closed { .. },
            ..
        }
    )));
    session.poll_io(105_001);
    let outputs = session.drain();
    assert!(outputs.iter().any(|o| matches!(
        o.body,
        OutputBody::EventV4 {
            event: EventV4::Closed { .. },
            ..
        }
    )));
}
#[test]
fn two_admissions_are_independent_and_financial_ledgers_never_own_worlds() {
    let mut h = Harness::new(|_| {});
    h.hold_admit[1] = true;
    h.pump();
    assert!(h.engine.world.is_none());
    assert_eq!(h.stores[0].admissions, 1);
    assert_eq!(h.stores[1].admissions, 0);
    assert!(h.engine.seats.iter().all(|s| s.ledger.world.is_none()));
    let (i, r) = h.held.pop().unwrap();
    let result = h.stores[i].execute(&r);
    h.receipt(i, &r, result).unwrap();
    h.pump();
    let w = h.engine.world.as_ref().unwrap();
    assert_eq!(w.members.len(), 2);
    assert_ne!(w.members[0].facts.revision, w.members[1].facts.revision);
    assert_eq!(h.stores[1].admissions, 1);
}
#[test]
fn two_activation_waits_for_both_sent_and_ready_and_is_exactly_once() {
    let mut h = Harness::new(|_| {});
    h.ack[1] = false;
    h.join();
    let a = h.snapshots[0].as_ref().unwrap().0.clone();
    let b = h.snapshots[1].as_ref().unwrap().0.clone();
    h.action(0, Action::CombatReady { publication_id: a })
        .unwrap();
    assert_eq!(h.engine.phase, Phase::Deploying);
    assert_eq!(
        h.action(
            1,
            Action::CombatReady {
                publication_id: b.clone()
            }
        ),
        Err(Error::NotReady)
    );
    h.engine
        .transport(TransportReceipt {
            key: h.engine.create.key.clone(),
            output_id: b,
            recipient_id: "B".into(),
            connection: h.engine.seats[1].connection.clone(),
            outcome: TransportOutcome::Failed,
        })
        .unwrap();
    h.ack[1] = true;
    h.pump();
    let b = h.snapshots[1].as_ref().unwrap().0.clone();
    let input = h.input(1, Action::CombatReady { publication_id: b });
    h.engine.input(input.clone()).unwrap();
    h.engine.input(input).unwrap();
    h.pump();
    assert_eq!(
        h.events
            .iter()
            .filter(|(_, e)| matches!(e, EventV4::Activated { .. }))
            .count(),
        1
    );
    assert!(
        h.engine
            .world
            .as_ref()
            .unwrap()
            .members
            .iter()
            .all(|m| m.ready)
    );
}
#[test]
fn two_refused_required_member_compensates_only_successful_admission() {
    let mut h = Harness::new(|_| {});
    h.hold_admit[1] = true;
    h.pump();
    let (i, r) = h.held.pop().unwrap();
    h.receipt(i, &r, ReceiptOutcome::Refused {}).unwrap();
    h.pump();
    assert!(h.engine.world.is_none());
    assert_eq!(h.stores[0].compensations, 1);
    assert_eq!(h.stores[1].compensations, 0);
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn two_close_unknown_admission_queries_then_compensates_once() {
    let mut h = Harness::new(|_| {});
    h.hold_admit[1] = true;
    h.pump();
    let (i, r) = h.held.pop().unwrap();
    let committed = h.stores[i].execute(&r);
    h.receipt(i, &r, ReceiptOutcome::Unknown {}).unwrap();
    h.close();
    h.pump();
    h.receipt(i, &r, committed).unwrap();
    h.pump();
    assert_eq!(h.stores[0].compensations, 1);
    assert_eq!(h.stores[1].compensations, 1);
    assert_eq!(h.engine.pending(), 0);
    assert!(h.engine.world.is_none());
}
#[test]
fn two_missing_ready_times_out_without_activation_or_orphaned_cost() {
    let mut h = Harness::new(|v| v["two_members"]["deployment_timeout_ms"] = json!(300));
    h.join();
    h.ticks(10);
    assert_eq!(h.engine.phase, Phase::Closed);
    assert!(h.engine.world.is_none());
    assert!(
        h.stores
            .iter()
            .all(|s| s.admissions == 1 && s.compensations == 1)
    );
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn two_equal_sequence_and_request_ids_do_not_cross_members() {
    let mut h = Harness::new(|_| {});
    h.active();
    let a = h.input(
        0,
        Action::Move {
            direction: [1., 0.],
        },
    );
    let b = h.input(
        1,
        Action::Move {
            direction: [0., 1.],
        },
    );
    assert_eq!(a.sequence, b.sequence);
    assert_eq!(a.request_id, b.request_id);
    h.engine.input(a).unwrap();
    h.engine.input(b).unwrap();
    h.ticks(1);
    assert_eq!(h.snapshot(0).own.position, [405.4, 400.]);
    assert_eq!(h.snapshot(1).own.position, [400., 485.4]);
}
#[test]
fn two_duplicate_cast_commits_cost_and_action_once_without_early_travel() {
    let mut h = Harness::new(|_| {});
    h.active();
    let input = h.input(
        0,
        Action::Cast {
            skill_id: "arc-bolt".into(),
            direction: [1., 0.],
        },
    );
    h.engine.input(input.clone()).unwrap();
    h.engine.input(input).unwrap();
    h.ticks(3);
    assert_eq!(h.snapshot(0).own.spirit, 200);
    h.ticks(1);
    assert_eq!(h.snapshot(0).own.spirit, 188);
    assert_eq!(h.snapshot(1).own.spirit, 200);
    assert_eq!(
        h.events
            .iter()
            .filter(|(r, e)| r.as_deref() == Some("A")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionCommitted { .. }
                    }
                ))
            .count(),
        1
    );
}
#[test]
fn two_cancel_before_commit_and_nonfinite_inputs_do_not_debit() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.fire(0);
    h.action(0, Action::CancelAction {}).unwrap();
    h.ticks(6);
    let position = h.snapshot(0).own.position;
    assert_eq!(
        h.action(
            0,
            Action::Move {
                direction: [f64::NAN, 0.]
            }
        ),
        Err(Error::InvalidMessage)
    );
    h.ticks(1);
    assert_eq!(h.snapshot(0).own.position, position);
    assert_eq!(h.snapshot(0).own.spirit, 200);
}
#[test]
fn two_self_statuses_use_real_member_ids_and_leave_does_not_clear_survivor() {
    let mut h = Harness::new(|_| {});
    h.active();
    for i in 0..2 {
        h.action(
            i,
            Action::CastSelf {
                skill_id: "self-resolve".into(),
            },
        )
        .unwrap();
    }
    h.ticks(4);
    assert_eq!(h.snapshot(0).own.effective_atk, 125);
    assert_eq!(h.snapshot(1).own.effective_atk, 125);
    assert_eq!(h.snapshot(0).own.statuses[0].source.id, "A");
    assert_eq!(h.snapshot(1).own.statuses[0].source.id, "B");
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    h.ticks(2);
    assert_eq!(h.engine.phase, Phase::Active);
    assert_eq!(h.snapshot(1).own.statuses.len(), 1);
    assert_eq!(h.stores[0].settlements, 1);
    assert_eq!(h.stores[1].settlements, 0);
}
#[test]
fn two_npcs_select_and_damage_both_members_in_the_single_shared_world() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.ticks(40);
    assert!(h.snapshot(0).own.hp < 1000);
    assert!(h.snapshot(1).own.hp < 1200);
    assert_eq!(h.snapshot(0).monsters, h.snapshot(1).monsters);
    assert!(
        h.events
            .iter()
            .any(|(_, e)| matches!(e,EventV4::Damage{target,..}if target.id=="A"))
    );
    assert!(
        h.events
            .iter()
            .any(|(_, e)| matches!(e,EventV4::Damage{target,..}if target.id=="B"))
    );
}
#[test]
fn two_npc_projectiles_use_shared_target_eligibility() {
    let mut h = Harness::new(|v| {
        v["pve"]["combat"]["cast_chance"] = json!(1.);
        v["pve"]["combat"]["melee_range"] = json!(1.);
    });
    h.active();
    h.ticks(15);
    assert!(h.snapshot(0).own.hp < 1000);
    assert!(h.snapshot(1).own.hp < 1200);
    assert!(h.events.iter().any(|(_,e)|matches!(e,EventV4::ProjectileStarted{projectile}if projectile.source.kind==StatusEntityKind::Monster)));
}
fn kill_and_resource(h: &mut Harness) -> ResourceViewV4 {
    h.fire(0);
    h.ticks(25);
    h.fire(0);
    h.ticks(6);
    h.snapshot(0)
        .resources
        .iter()
        .find(|r| r.item_id.is_some())
        .unwrap()
        .clone()
}
#[test]
fn two_same_step_resource_competition_grants_one_copy_and_preserves_other_inventory() {
    let mut h = Harness::new(|_| {});
    h.active();
    let resource = kill_and_resource(&mut h);
    {
        let w = h.engine.world.as_mut().unwrap();
        for m in &mut w.members {
            m.position = resource.position;
        }
    }
    let a = h.input(
        0,
        Action::Pickup {
            resource_id: resource.id.clone(),
            expected_revision: 1,
        },
    );
    let b = h.input(
        1,
        Action::Pickup {
            resource_id: resource.id.clone(),
            expected_revision: 1,
        },
    );
    h.engine.input(a.clone()).unwrap();
    h.engine.input(a).unwrap();
    assert_eq!(h.engine.input(b), Err(Error::InvalidTarget));
    h.pump();
    assert_eq!(
        h.snapshot(0)
            .own
            .inventory
            .iter()
            .find(|r| r.item_id == "xuan-tie")
            .unwrap()
            .quantity,
        h.engine.config.carried_quantity + 2
    );
    assert_eq!(h.snapshot(1).own.inventory_revision, 1);
    assert!(!h.snapshot(0).resources.iter().any(|r| r.id == resource.id));
}
#[test]
fn two_full_inventory_refusal_does_not_consume_shared_resource() {
    let mut h = Harness::new(|v| {
        v["inventory_capacity"] = json!(1);
        v["stack_limit"] = json!(1);
        v["carried_quantity"] = json!(1);
        v["pve"]["items"]["xuan-tie"]["stackLimit"] = json!(1);
    });
    h.active();
    let resource = kill_and_resource(&mut h);
    h.engine.world.as_mut().unwrap().members[0].position = resource.position;
    assert_eq!(
        h.action(
            0,
            Action::Pickup {
                resource_id: resource.id.clone(),
                expected_revision: 1
            }
        ),
        Err(Error::Capacity)
    );
    h.ticks(1);
    assert!(h.snapshot(0).resources.iter().any(|r| r.id == resource.id));
    assert_eq!(h.snapshot(0).own.inventory_revision, 1);
}
#[test]
fn two_kill_orb_is_frozen_for_killer_and_other_member_cannot_take_it() {
    let mut h = Harness::new(|_| {});
    h.active();
    let _ = kill_and_resource(&mut h);
    let orb = h
        .snapshot(0)
        .resources
        .iter()
        .find(|r| r.item_id.is_none())
        .unwrap()
        .clone();
    {
        let w = h.engine.world.as_mut().unwrap();
        for m in &mut w.members {
            m.position = orb.position;
        }
        w.members[0].hp = 500;
    }
    assert_eq!(
        h.action(
            1,
            Action::Pickup {
                resource_id: orb.id.clone(),
                expected_revision: 1
            }
        ),
        Err(Error::Unauthorized)
    );
    h.action(
        0,
        Action::Pickup {
            resource_id: orb.id,
            expected_revision: 1,
        },
    )
    .unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.hp, 530);
}
#[test]
fn two_core_unknown_holds_only_one_member_while_survivor_continues() {
    let mut h = Harness::new(|v| v["pve"]["combat"]["aggro_range"] = json!(1.));
    h.active();
    h.ticks(34);
    h.hold_settle[0] = true;
    h.action(0, Action::ExtractStart {}).unwrap();
    h.ticks(20);
    assert_eq!(
        h.engine.world.as_ref().unwrap().members[0].phase,
        MemberPhase::Settling
    );
    assert_eq!(h.engine.phase, Phase::Active);
    h.action(
        1,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    let old = h.snapshot(1).own.position;
    h.ticks(3);
    assert!(h.snapshot(1).own.position[1] > old[1]);
    assert_eq!(h.stores[0].settlements, 0);
    let (i, r) = h.held.pop().unwrap();
    let proof = h.stores[i].execute(&r);
    h.receipt(i, &r, proof.clone()).unwrap();
    h.receipt(i, &r, proof).unwrap();
    h.pump();
    assert_eq!(h.stores[0].settlements, 1);
    assert_eq!(h.engine.phase, Phase::Active);
    assert_eq!(
        h.engine.world.as_ref().unwrap().members[0].phase,
        MemberPhase::Terminal
    );
}
#[test]
fn two_background_and_reconnect_fence_only_the_current_member() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.action(
        0,
        Action::Move {
            direction: [1., 0.],
        },
    )
    .unwrap();
    let old_input = h.input(0, Action::Stop {});
    let previous = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    h.action(
        1,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(3);
    assert_eq!(h.snapshot(0).own.position, [400., 400.]);
    assert!(h.snapshot(1).own.position[1] > 480.);
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous,
            authenticated_principal: "principal-A".into(),
        },
    );
    reconnect.connection = Connection {
        id: "A-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    reconnect.request_id = "reconnect".into();
    h.engine.input(reconnect.clone()).unwrap();
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    assert_eq!(h.engine.input(old_input), Err(Error::StaleConnection));
    h.pump();
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    let id = h.snapshots[0].as_ref().unwrap().0.clone();
    h.action(0, Action::CombatReady { publication_id: id })
        .unwrap();
    h.ticks(2);
    assert_eq!(h.snapshot(0).own.position, [400., 400.]);
    assert!(h.snapshot(1).own.position[1] > 496.);
}
#[test]
fn two_disconnect_expiry_does_not_extend_or_close_survivor() {
    let mut h = Harness::new(|v| v["params"]["runtime"]["reconnect_grace_ms"] = json!(300));
    h.active();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let deadline = h.engine.seats[0].resume_until;
    h.ticks(2);
    h.action(0, Action::ConnectionLost {}).unwrap();
    assert_eq!(h.engine.seats[0].resume_until, deadline);
    h.ticks(8);
    assert_eq!(h.stores[0].settlements, 1);
    assert_eq!(h.stores[1].settlements, 0);
    assert_eq!(h.engine.phase, Phase::Active);
}
#[test]
fn two_public_members_and_events_do_not_contain_private_account_state() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.fire(0);
    h.ticks(4);
    let members = serde_json::to_string(&h.snapshot(1).members).unwrap();
    for key in [
        "spirit",
        "inventory",
        "principal",
        "operation_id",
        "cooldown",
        "ready_at",
        "revision",
    ] {
        assert!(!members.contains(key), "{key}");
    }
    for (recipient, event) in &h.events {
        if matches!(event, EventV4::Private { .. }) {
            assert!(recipient.is_some());
        }
    }
    assert_eq!(h.snapshot(0).own.participant.owner_id, "A");
    assert_eq!(h.snapshot(1).own.participant.owner_id, "B");
}
#[test]
fn two_failed_or_wrong_recipient_transport_cannot_advance_another_baseline() {
    let mut h = Harness::new(|_| {});
    h.ack = [false; 2];
    h.join();
    let a = h.snapshots[0].as_ref().unwrap().0.clone();
    assert_eq!(
        h.engine.transport(TransportReceipt {
            key: h.engine.create.key.clone(),
            output_id: a.clone(),
            recipient_id: "B".into(),
            connection: h.engine.seats[1].connection.clone(),
            outcome: TransportOutcome::Sent
        }),
        Err(Error::StalePublication)
    );
    h.engine
        .transport(TransportReceipt {
            key: h.engine.create.key.clone(),
            output_id: a.clone(),
            recipient_id: "A".into(),
            connection: h.engine.seats[0].connection.clone(),
            outcome: TransportOutcome::Enqueued,
        })
        .unwrap();
    assert_eq!(
        h.action(0, Action::CombatReady { publication_id: a }),
        Err(Error::NotReady)
    );
}
#[test]
fn two_slow_member_publications_remain_bounded_and_do_not_hold_peer() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.ack[0] = false;
    h.action(
        1,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(80);
    assert!(h.snapshot(1).logical_ms >= 2400);
    assert!(h.snapshot(1).own.position[1] > 480.);
    assert!(h.engine.seats[0].publications.len() <= h.engine.config.history_capacity);
    assert!(h.engine.seats.iter().all(|s| s.queue.len() <= 2));
    assert_eq!(h.engine.phase, Phase::Active);
}
#[test]
fn two_absolute_terminal_ignores_protection_and_closes_shared_world_once() {
    let mut h = Harness::new(|v| {
        v["extraction_unlock_ms"] = json!(0);
        v["extraction_channel_ms"] = json!(90);
        v["terminal_ms"] = json!(300);
        for d in v["pve"]["status_definitions"].as_array_mut().unwrap() {
            d["duration_ms"] = json!(300);
        }
    });
    h.active();
    h.action(0, Action::Background {}).unwrap();
    h.ticks(10);
    assert!(h.engine.world.is_none());
    assert_eq!(h.engine.phase, Phase::Closed);
    assert!(h.stores.iter().all(|s| s.settlements == 1));
    h.close();
    assert_eq!(
        h.events
            .iter()
            .filter(|(_, e)| matches!(e, EventV4::Closed { .. }))
            .count(),
        1
    );
}
#[test]
fn two_member_operation_cannot_be_applied_to_the_other_ledger() {
    let mut h = Harness::new(|_| {});
    h.hold_admit = [true; 2];
    h.pump();
    let (i, r) = h.held[0].clone();
    let proof = h.stores[i].execute(&r);
    assert_eq!(
        h.receipt(1 - i, &r, proof.clone()),
        Err(Error::InvalidMessage)
    );
    h.receipt(i, &r, proof).unwrap();
    assert!(h.engine.world.is_none());
    assert_eq!(h.stores[1 - i].admissions, 0);
}
#[test]
fn two_recovery_before_activation_refunds_each_original_debit_without_world() {
    let mut h = Harness::new(|_| {});
    h.hold_admit[1] = true;
    h.pump();
    let checkpoint = h.checkpoint.clone().unwrap();
    for generation in [1, 2] {
        let mut r = h.engine.create.clone();
        r.recovery = Some(RecoveryV4 {
            generation,
            checkpoint: checkpoint.clone(),
        });
        let catalog = catalog_with(|_| {}).unwrap();
        h.engine = TwoMembers::create(h.engine.config.clone(), catalog, r).unwrap();
        h.hold_admit = [false; 2];
        h.pump();
        assert!(h.engine.world.is_none());
        assert_eq!(h.engine.phase, Phase::Closed);
        assert_eq!(h.engine.pending(), 0);
    }
    assert_eq!(h.stores[0].admissions, 1);
    assert_eq!(h.stores[0].compensations, 1);
    assert_eq!(h.stores[1].admissions, 0);
}
#[test]
fn two_recovery_retains_frozen_settlements_across_two_restarts() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.hold_settle = [true; 2];
    h.close();
    let checkpoint = h.checkpoint.clone().unwrap();
    let original = h.engine.create.clone();
    for generation in [1, 2] {
        let mut r = original.clone();
        r.recovery = Some(RecoveryV4 {
            generation,
            checkpoint: checkpoint.clone(),
        });
        let catalog = catalog_with(|_| {}).unwrap();
        h.engine = TwoMembers::create(h.engine.config.clone(), catalog, r).unwrap();
        h.hold_settle = [false; 2];
        h.pump();
        assert!(h.engine.world.is_none());
        assert_eq!(h.engine.pending(), 0);
    }
    assert!(
        h.stores
            .iter()
            .all(|s| s.admissions == 1 && s.settlements == 1 && s.compensations == 0)
    );
}
#[test]
fn two_recovery_rejects_corrupt_hash_and_member_cross_binding() {
    let mut h = Harness::new(|_| {});
    h.pump();
    let checkpoint = h.checkpoint.clone().unwrap();
    let catalog = catalog_with(|_| {}).unwrap();
    for mode in 0..3 {
        let mut r = h.engine.create.clone();
        let mut checkpoint = checkpoint.clone();
        match mode {
            0 => checkpoint.sha256 = "0".repeat(64),
            1 => {
                checkpoint.state.members.swap(0, 1);
                checkpoint.sha256 = state_digest(&checkpoint.state)
            }
            _ => {
                checkpoint.state.identity_sha256 = "x".into();
                checkpoint.sha256 = state_digest(&checkpoint.state)
            }
        }
        r.recovery = Some(RecoveryV4 {
            generation: 1,
            checkpoint,
        });
        assert!(TwoMembers::create(h.engine.config.clone(), catalog.clone(), r).is_err());
    }
}

#[test]
fn two_public_loot_from_another_members_kill_survives_frozen_recovery() {
    let mut h = Harness::new(|v| v["pve"]["combat"]["aggro_range"] = json!(1.));
    h.active();
    let resource = kill_and_resource(&mut h);
    h.action(
        0,
        Action::Navigate {
            destination: [400., 480.],
        },
    )
    .unwrap();
    h.ticks(19);
    h.fire(0);
    h.ticks(25);
    h.fire(0);
    h.ticks(8);
    assert!(h.snapshot(0).monsters.iter().all(|n| n.hp == 0));
    h.engine.world.as_mut().unwrap().members[1].position = resource.position;
    h.action(
        1,
        Action::Pickup {
            resource_id: resource.id,
            expected_revision: 1,
        },
    )
    .unwrap();
    h.pump();
    h.hold_settle = [true; 2];
    h.close();
    let checkpoint = h.checkpoint.clone().unwrap();
    let summaries: Vec<_> = checkpoint
        .state
        .members
        .iter()
        .flat_map(|m| m.checkpoint.state.operations.iter())
        .filter_map(
            |o| match h.engine.config.decode_business(&o.request.body).unwrap() {
                Business::CommitSettlement {
                    scoring, looted, ..
                } => Some((scoring.unwrap().credited_kills, looted)),
                _ => None,
            },
        )
        .collect();
    assert_eq!(summaries, vec![(2, 0), (0, 2)]);
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint,
    });
    h.engine = TwoMembers::create(
        h.engine.config.clone(),
        catalog_with(|v| v["pve"]["combat"]["aggro_range"] = json!(1.)).unwrap(),
        r,
    )
    .unwrap();
    h.hold_settle = [false; 2];
    h.pump();
    assert!(h.engine.world.is_none());
    assert_eq!(h.engine.pending(), 0);
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}
#[test]
fn two_one_hundred_closes_release_world_and_all_external_responsibilities() {
    for _ in 0..100 {
        let mut h = Harness::new(|_| {});
        h.active();
        h.close();
        assert!(h.engine.world.is_none());
        assert_eq!(h.engine.pending(), 0);
        assert!(h.engine.drain().is_empty());
        assert!(
            h.stores
                .iter()
                .all(|s| s.admissions == 1 && s.settlements == 1)
        );
    }
}

#[test]
fn two_positive_damage_interrupts_only_the_affected_members_extraction() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.ticks(34);
    h.action(0, Action::ExtractStart {}).unwrap();
    h.ticks(6);
    assert!(h.snapshot(0).own.extraction_finishes_ms.is_none());
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Active);
    assert_eq!(h.stores[0].settlements, 0);
    assert!(h.events.iter().any(|(id, e)| id.as_deref() == Some("A")
        && matches!(
            e,
            EventV4::Private {
                event: Event::ExtractionInterrupted
            }
        )));
}
#[test]
fn two_death_and_survivor_extraction_settle_independently() {
    let mut h = Harness::new(|_| {});
    h.stores[0].hp = 10;
    h.active();
    h.ticks(40);
    assert_eq!(h.stores[0].settlements, 1);
    assert_eq!(h.engine.phase, Phase::Active);
    h.action(
        1,
        Action::Navigate {
            destination: h.engine.config.extraction_point,
        },
    )
    .unwrap();
    h.ticks(10);
    h.action(1, Action::Stop {}).unwrap();
    h.action(1, Action::ExtractStart {}).unwrap();
    h.ticks(20);
    assert_eq!(h.stores[1].settlements, 1);
    assert_eq!(h.engine.phase, Phase::Closed);
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn two_self_status_capacity_and_malformed_payloads_are_atomic() {
    let mut h = Harness::new(|v| {
        v["pve"]["status_rules"]["max_active_per_entity"] = json!(1);
        v["skill"]["on_commit_self_statuses"] = json!(["guard"]);
    });
    h.active();
    h.fire(0);
    h.ticks(10);
    let spirit = h.snapshot(0).own.spirit;
    assert_eq!(
        h.action(
            0,
            Action::CastSelf {
                skill_id: "self-resolve".into()
            }
        ),
        Err(Error::Capacity)
    );
    let old = h.engine.seats[0].sequence;
    let bad = h.input(
        0,
        Action::Move {
            direction: [f64::INFINITY, 0.],
        },
    );
    assert_eq!(h.engine.input(bad), Err(Error::InvalidMessage));
    assert_eq!(h.engine.seats[0].sequence, old);
    h.ticks(1);
    assert_eq!(h.snapshot(0).own.spirit, spirit);
    assert!(h.snapshot(0).own.pending_action.is_none());
}
#[test]
fn two_shared_projectiles_stop_at_the_first_wall() {
    let mut h = Harness::new(|v| {
        let p = &mut v["pve"];
        let mut wall = p["palette"][0].clone();
        wall["walkable"] = json!(false);
        wall["blocksProjectiles"] = json!(true);
        wall["blocksVision"] = json!(true);
        let i = p["palette"].as_array().unwrap().len();
        p["palette"].as_array_mut().unwrap().push(wall);
        let mut cells = vec![];
        for y in 0..32 {
            for x in 0..32 {
                cells.push(json!([
                    if x == 12 && (9..=11).contains(&y) {
                        i
                    } else {
                        0
                    },
                    1
                ]));
            }
        }
        p["cells_rle"] = json!(cells);
        p["monster_positions"] = json!([[600., 400.], [600., 480.]]);
        p["combat"]["aggro_range"] = json!(1.);
    });
    h.active();
    h.fire(0);
    h.ticks(12);
    assert!(h.snapshot(0).monsters.iter().all(|m| m.hp == m.max_hp));
    assert!(
        h.events
            .iter()
            .any(|(_, e)| matches!(e, EventV4::ProjectileEnded { .. }))
    );
}
#[test]
fn two_financial_timeout_does_not_pause_or_close_the_other_members_world() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    h.engine.poll_io(5001);
    h.pump();
    let before = h.engine.now;
    h.action(
        1,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(3);
    assert!(h.engine.now > before);
    assert_eq!(h.engine.phase, Phase::Active);
    assert!(h.snapshot(1).own.position[1] > 480.);
    assert!(h.engine.pending() > 0);
}
#[test]
fn two_source_ownership_and_aggregate_recovery_reject_reassigned_loot() {
    let mut h = Harness::new(|_| {});
    h.active();
    h.hold_settle = [true; 2];
    h.close();
    let mut checkpoint = h.checkpoint.clone().unwrap();
    let c = &h.engine.config;
    // Each member's two kills is individually legal for the two-NPC fixture;
    // assigning all of those same kills to both members must fail aggregate validation.
    let scoring = c.score_summary(&BTreeMap::from([(
        c.pve.as_ref().unwrap().template_id.clone(),
        2,
    )]));
    assert!(c.valid_scoring(&scoring, checkpoint.state.logical_ms));
    for member in &mut checkpoint.state.members {
        for op in &mut member.checkpoint.state.operations {
            let mut intent = c.decode_business(&op.request.body).unwrap();
            if let Business::CommitSettlement {
                scoring: score,
                cultivation,
                outcome,
                ..
            } = &mut intent
            {
                *score = Some(scoring.clone());
                *cultivation = c.cultivation_from_score(u64::from(scoring.kill_score), *outcome);
                op.request.body = c.encode_business(&intent);
            }
        }
        member.checkpoint.sha256 = digest(&member.checkpoint.state);
    }
    checkpoint.sha256 = state_digest(&checkpoint.state);
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint,
    });
    let cat = catalog_with(|_| {}).unwrap();
    assert!(TwoMembers::create(h.engine.config.clone(), cat, r).is_err());
}

#[test]
fn two_public_runtime_hundred_cycles_join_threads_and_preserve_abandoned_drains() {
    let cat = catalog_with(|_| {}).unwrap();
    let c = cat.default_config().as_ref().clone();
    for cycle in 0..100 {
        let runtime = crate::Runtime::with_catalog(cat.clone());
        let client = runtime.client();
        let mut request = create(&c);
        request.key.id = format!("runtime-cycle-{cycle}");
        let key = request.key.clone();
        client.create(request.clone()).unwrap();
        if cycle == 0 {
            for _ in 0..3 {
                client.abandon_drain_for_test();
            }
        }
        let mut stores = [Store::new(0, &c), Store::new(1, &c)];
        let mut snapshots: [Option<String>; 2] = [None, None];
        let mut checkpoint = None;
        {
            let mut pump = |client: &crate::Client| {
                for _ in 0..20 {
                    let outputs = client.drain_outputs().unwrap();
                    if outputs.is_empty() {
                        break;
                    }
                    for o in outputs {
                        match o.body {
                            OutputBody::BusinessRequestV4 {
                                participant_id,
                                request,
                                ..
                            } => {
                                let i = usize::from(participant_id == "B");
                                let outcome = stores[i].execute(&request);
                                client
                                    .submit_business_receipt(MemberBusinessReceipt {
                                        key: key.clone(),
                                        participant_id,
                                        operation_id: request.operation_id,
                                        attempt_id: request.attempt_id,
                                        outcome,
                                    })
                                    .unwrap();
                            }
                            OutputBody::PublicationV4 {
                                recipient_id,
                                connection,
                                ..
                            } => {
                                let i = usize::from(recipient_id == "B");
                                let result = client.submit_transport_receipt(TransportReceipt {
                                    key: key.clone(),
                                    output_id: o.id.clone(),
                                    recipient_id,
                                    connection,
                                    outcome: TransportOutcome::Sent,
                                });
                                assert!(result.is_ok() || result == Err(Error::Closed));
                                snapshots[i] = Some(o.id);
                            }
                            OutputBody::ResponsibilityV4 { checkpoint: cp } => {
                                checkpoint = Some(*cp)
                            }
                            _ => {}
                        }
                    }
                }
            };
            pump(&client);
            for s in &request.participants {
                client
                    .submit_input(Input {
                        key: key.clone(),
                        participant_id: s.participant_id.clone(),
                        entity_generation: 1,
                        connection: s.connection.clone(),
                        sequence: 1,
                        request_id: "join".into(),
                        action: Action::Join {},
                    })
                    .unwrap();
            }
            pump(&client);
        }
        for (i, s) in request.participants.iter().enumerate() {
            client
                .submit_input(Input {
                    key: key.clone(),
                    participant_id: s.participant_id.clone(),
                    entity_generation: 1,
                    connection: s.connection.clone(),
                    sequence: 2,
                    request_id: "ready".into(),
                    action: Action::CombatReady {
                        publication_id: snapshots[i].clone().unwrap(),
                    },
                })
                .unwrap();
        }
        client
            .close(Close {
                key: key.clone(),
                request_id: "close".into(),
            })
            .unwrap();
        for _ in 0..20 {
            let outputs = client.drain_outputs().unwrap();
            if outputs.is_empty() {
                break;
            }
            for o in outputs {
                match o.body {
                    OutputBody::BusinessRequestV4 {
                        participant_id,
                        request,
                        ..
                    } => {
                        let i = usize::from(participant_id == "B");
                        let outcome = stores[i].execute(&request);
                        client
                            .submit_business_receipt(MemberBusinessReceipt {
                                key: key.clone(),
                                participant_id,
                                operation_id: request.operation_id,
                                attempt_id: request.attempt_id,
                                outcome,
                            })
                            .unwrap();
                    }
                    OutputBody::ResponsibilityV4 { checkpoint: cp } => checkpoint = Some(*cp),
                    _ => {}
                }
            }
        }
        assert!(checkpoint.unwrap().state.members.iter().all(|m| {
            m.checkpoint
                .state
                .operations
                .iter()
                .all(|o| o.terminal.is_some())
        }));
        assert!(
            stores
                .iter()
                .all(|s| s.admissions == 1 && s.settlements == 1)
        );
        drop(runtime);
        assert_eq!(client.drain_outputs(), Err(Error::Closed));
    }
}

#[path = "capacity_tests.rs"]
mod capacity;

#[path = "source_instant_cast_tests.rs"]
mod source_instant_cast;
