use super::*;
use crate::replication::{DecodeError, ReplicaV6};
#[path = "checkpoint_zone_tests.rs"]
mod checkpoint_zones;
#[path = "member_cast_anchor_tests.rs"]
mod member_cast_anchor;
#[path = "member_ground_trap_tests.rs"]
mod member_ground_trap;
#[path = "moving_enter_area_tests.rs"]
mod moving_enter_area;
#[path = "npc_ground_field_tests.rs"]
mod npc_ground_field;
#[path = "npc_ground_strike_tests.rs"]
mod npc_ground_strike;
fn delta_catalog(
    n: usize,
    change: impl FnOnce(&mut Value),
) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    capacity_catalog(n, |v| {
        v["schema_version"] = json!(7);
        v["mechanism_version"] = json!(37);
        v["id"] = json!("delta-native");
        v["replication"] = json!({"ack_timeout_ms":600,"baseline_ttl_ms":5000,"full_interval_ms":1500,"terminal_grace_ms":3600});
        change(v);
    })
}
fn delta_request(c: &Config) -> CreateV4 {
    let mut r = request(c);
    r.protocol_version = 6;
    r
}
fn typed6(r: CreateV4) -> CreateV6 {
    serde_json::from_value(serde_json::to_value(r).unwrap()).unwrap()
}
struct DeltaHarness {
    engine: TwoMembers,
    stores: Vec<Store>,
    sequences: Vec<u32>,
    replicas: Vec<ReplicaV6>,
    ack: Vec<bool>,
    frames: Vec<PublicationEnvelopeV6>,
    vectors: Vec<Value>,
    events: Vec<(Option<String>, EventV4)>,
    held: Vec<(usize, BusinessRequest)>,
    hold_settle: Vec<bool>,
    checkpoint: Option<IoCheckpointV4>,
}
impl DeltaHarness {
    fn new(n: usize, change: impl FnOnce(&mut Value)) -> Self {
        let cat = delta_catalog(n, change).unwrap();
        let request = delta_request(&cat.default_config());
        Self::from_request(cat, request)
    }

    fn from_request(cat: Arc<crate::ConfigCatalog>, request: CreateV4) -> Self {
        let c = cat.default_config().as_ref().clone();
        let key = request.key.clone();
        let engine = TwoMembers::create(c.clone(), cat, request).unwrap();
        let n = engine.seats.len();
        let stores = engine
            .seats
            .iter()
            .enumerate()
            .map(|(i, seat)| {
                let mut store = Store::new(i, &c);
                store.principal = seat.original.authenticated_principal.clone();
                store
            })
            .collect();
        let replicas = engine
            .seats
            .iter()
            .map(|seat| ReplicaV6::new(key.clone(), seat.original.participant_id.clone()))
            .collect();
        Self {
            engine,
            stores,
            sequences: vec![0; n],
            replicas,
            ack: vec![true; n],
            frames: vec![],
            vectors: vec![],
            events: vec![],
            held: vec![],
            hold_settle: vec![false; n],
            checkpoint: None,
        }
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
    fn transport(
        &mut self,
        e: &PublicationEnvelopeV6,
        outcome: TransportOutcome,
    ) -> Result<Status> {
        self.engine.transport(TransportReceipt {
            key: e.key.clone(),
            output_id: e.publication_id.clone(),
            recipient_id: e.recipient_id.clone(),
            connection: e.connection.clone(),
            outcome,
        })
    }
    fn pump(&mut self) {
        for _ in 0..64 {
            let out = self.engine.drain();
            if out.is_empty() {
                return;
            }
            for o in out {
                match &o.body {
                    OutputBody::BusinessRequestV4 {
                        participant_id,
                        request,
                        ..
                    } => {
                        let i = self.engine.index(participant_id).unwrap();
                        if self.hold_settle[i]
                            && matches!(
                                request.body,
                                Business::CommitAssets {
                                    transaction: AssetTransaction::ResultSettlement { .. }
                                } | Business::QueryOperation { .. }
                                    | Business::ReconcileOperation { .. }
                            )
                        {
                            self.held.push((i, request.clone()));
                            continue;
                        }
                        let outcome = self.stores[i].execute(request);
                        self.receipt(i, request, outcome).unwrap();
                    }
                    OutputBody::PublicationV6 { recipient_id, .. } => {
                        let i = self.engine.index(recipient_id).unwrap();
                        let e = PublicationEnvelopeV6::from_output(&o).unwrap();
                        let expected = self.engine.target_v6(i, &o.id).unwrap();
                        assert_eq!(self.replicas[i].apply(&e).unwrap(), &expected);
                        self.vectors.push(json!({"envelope":e,"snapshot":expected}));
                        self.frames.push(e.clone());
                        if self.ack[i] {
                            self.transport(&e, TransportOutcome::Sent).unwrap();
                        }
                    }
                    OutputBody::ResponsibilityV4 { checkpoint } => {
                        self.checkpoint = Some((**checkpoint).clone())
                    }
                    OutputBody::EventV4 {
                        recipient_id,
                        event,
                    } => self.events.push((recipient_id.clone(), event.clone())),
                    _ => panic!("legacy publication escaped protocol 6"),
                }
            }
        }
        panic!("nonquiescent replication");
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
        let r = self.input(i, action);
        self.engine.input(r)
    }
    fn join(&mut self) {
        self.pump();
        for i in 0..self.stores.len() {
            self.action(i, Action::Join {}).unwrap();
            self.pump();
        }
    }
    fn active(&mut self) {
        self.join();
        for i in 0..self.stores.len() {
            let publication_id = self.replicas[i].last_full_id().unwrap().to_string();
            self.action(i, Action::CombatReady { publication_id })
                .unwrap();
            self.pump();
        }
        assert_eq!(self.engine.phase, Phase::Active);
    }
    fn ticks(&mut self, n: usize) {
        for _ in 0..n {
            self.engine.tick();
            self.engine.poll_io(self.engine.now);
            self.pump();
        }
    }
    fn latest(&self, i: usize) -> PublicationEnvelopeV6 {
        self.frames
            .iter()
            .rev()
            .find(|f| f.recipient_id == self.engine.seats[i].original.participant_id)
            .unwrap()
            .clone()
    }
    fn snapshot(&self, i: usize) -> &SnapshotV4 {
        self.replicas[i].snapshot().unwrap()
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
}
#[test]
fn delta_configuration_and_typed_create_are_versioned_and_strict() {
    for n in [1, 2, 4, 8] {
        let mut h = DeltaHarness::new(n, |_| {});
        h.active();
        h.ticks(4);
        assert!(
            h.frames
                .iter()
                .any(|f| matches!(f.payload, PublicationPayloadV6::Delta { .. }))
        );
        h.close();
        assert_eq!(h.engine.pending(), 0);
    }
    for (path, v) in [
        ("/replication/ack_timeout_ms", json!(0)),
        ("/replication/baseline_ttl_ms", json!(300)),
        ("/replication/full_interval_ms", json!(6000)),
        ("/replication/terminal_grace_ms", json!(600)),
        ("/schema_version", json!(6)),
        ("/mechanism_version", json!(36)),
    ] {
        assert!(delta_catalog(8, |d| *d.pointer_mut(path).unwrap() = v).is_err());
    }
    assert!(delta_catalog(8, |v| v["replication"]["unknown"] = json!(1)).is_err());
    assert!(capacity_catalog(8, |v| v["replication"] = json!({})).is_err());
    let c = delta_catalog(8, |_| {}).unwrap();
    let cfg = c.default_config().as_ref().clone();
    let mut session = crate::session::Session::new(cfg.clone(), c);
    assert_eq!(
        session.create(CreateRequest::Members({
            let mut wrong = delta_request(&cfg);
            wrong.protocol_version = 5;
            typed(wrong)
        })),
        Err(Error::VersionMismatch)
    );
    assert!(session.drain().is_empty());
    session.create(typed6(delta_request(&cfg)).into()).unwrap();
}
#[test]
fn delta_sent_base_advances_only_for_exact_current_drained_recipient() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.ack[3] = false;
    h.join();
    let e = h.latest(3);
    h.transport(&e, TransportOutcome::Enqueued).unwrap();
    assert_eq!(h.engine.stream_cache_counts(3), (0, 1));
    let mut wrong = e.clone();
    wrong.recipient_id = "P04".into();
    assert_eq!(
        h.transport(&wrong, TransportOutcome::Sent),
        Err(Error::StaleConnection)
    );
    h.transport(&e, TransportOutcome::Sent).unwrap();
    h.transport(&e, TransportOutcome::Sent).unwrap();
    assert_eq!(
        h.transport(&e, TransportOutcome::Failed),
        Err(Error::Conflict)
    );
    assert_eq!(h.engine.stream_cache_counts(3), (1, 0));
}
#[test]
fn delta_enqueued_full_and_sent_delta_cannot_be_used_as_ready() {
    let mut h = DeltaHarness::new(2, |_| {});
    h.ack[0] = false;
    h.join();
    let f = h.latest(0);
    assert_eq!(
        h.action(
            0,
            Action::CombatReady {
                publication_id: f.publication_id.clone()
            }
        ),
        Err(Error::NotReady)
    );
    h.transport(&f, TransportOutcome::Sent).unwrap();
    h.ack[0] = true;
    h.ticks(2);
    let d = h.latest(0);
    assert!(matches!(d.payload, PublicationPayloadV6::Delta { .. }));
    assert_eq!(
        h.action(
            0,
            Action::CombatReady {
                publication_id: d.publication_id
            }
        ),
        Err(Error::NotReady)
    );
    h.action(
        0,
        Action::CombatReady {
            publication_id: f.publication_id,
        },
    )
    .unwrap();
}
#[test]
fn delta_ready_full_is_pinned_when_sent_history_rolls_over() {
    let mut h = DeltaHarness::new(2, |v| v["history_capacity"] = json!(16));
    h.join();
    let f = h.replicas[0].last_full_id().unwrap().to_string();
    h.ticks(40);
    assert!(!h.engine.seats[0].publications.iter().any(|p| p.id == f));
    h.action(0, Action::CombatReady { publication_id: f })
        .unwrap();
}
#[test]
fn delta_undrained_output_is_not_a_sent_baseline() {
    let mut h = DeltaHarness::new(2, |_| {});
    h.pump();
    h.action(0, Action::Join {}).unwrap();
    let o = h.engine.seats[0].queue.back().unwrap().clone();
    let e = PublicationEnvelopeV6::from_output(&o).unwrap();
    assert_eq!(
        h.transport(&e, TransportOutcome::Sent),
        Err(Error::StalePublication)
    );
    assert_eq!(h.engine.stream_cache_counts(0), (0, 1));
}
fn failed(kind: TransportOutcome, delta: bool) {
    let mut h = DeltaHarness::new(8, |_| {});
    if delta {
        h.active();
        h.ack[3] = false;
        h.ticks(1);
    } else {
        h.ack[3] = false;
        h.join();
    }
    let old = h.latest(3);
    h.transport(&old, kind).unwrap();
    assert_eq!(
        h.transport(&old, TransportOutcome::Sent),
        Err(Error::StalePublication)
    );
    h.ack[3] = true;
    // A normal tick can coalesce the replacement before the host drains it.
    h.engine.tick();
    h.pump();
    let full = h.latest(3);
    assert!(full.stream_epoch > old.stream_epoch);
    assert!(matches!(
        full.payload,
        PublicationPayloadV6::Full {
            reason: FullReason::Failure,
            ..
        }
    ));
    assert!(h.snapshot(7).logical_ms >= h.snapshot(3).logical_ms);
}
#[test]
fn delta_failed_initial_full_gets_replacement_full() {
    failed(TransportOutcome::Failed, false);
}
#[test]
fn delta_failed_delta_never_advances_base() {
    failed(TransportOutcome::Failed, true);
}
#[test]
fn delta_cancelled_delta_never_advances_base() {
    failed(TransportOutcome::Cancelled, true);
}
#[test]
fn delta_timeout_replaces_own_stream_while_seven_peers_keep_progressing() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    h.ack[3] = false;
    h.action(
        7,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(1);
    let old = h.latest(3);
    let before = h.snapshot(7).own.position;
    h.ticks(21);
    assert!(h.snapshot(7).own.position[1] > before[1]);
    let replacement = h.latest(3);
    assert!(replacement.stream_epoch > old.stream_epoch);
    assert!(matches!(
        replacement.payload,
        PublicationPayloadV6::Full {
            reason: FullReason::Timeout,
            ..
        }
    ));
    assert_eq!(
        h.transport(&old, TransportOutcome::Sent),
        Err(Error::StalePublication)
    );
    assert!(
        h.stores
            .iter()
            .all(|s| s.admissions == 1 && s.settlements == 0)
    );
}
#[test]
fn delta_unsent_coalescing_and_inflight_storage_are_bounded() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    for _ in 0..80 {
        h.engine.tick();
    }
    for i in 0..8 {
        assert!(h.engine.seats[i].queue.len() <= 1);
        assert!(h.engine.stream_cache_counts(i).0 <= 1);
        assert!(h.engine.stream_cache_counts(i).1 <= 1);
    }
    h.pump();
    h.ack[3] = false;
    h.ticks(80);
    assert_eq!(h.engine.stream_cache_counts(3).1, 1);
    assert_eq!(h.snapshot(7).logical_ms, h.engine.now);
}
#[test]
fn delta_periodic_full_refreshes_without_changing_epoch() {
    let mut h = DeltaHarness::new(2, |v| v["replication"]["full_interval_ms"] = json!(300));
    h.active();
    let epoch = h.latest(0).stream_epoch;
    h.ticks(12);
    assert!(h.frames.iter().any(|f| f.recipient_id == "P00"
        && matches!(
            f.payload,
            PublicationPayloadV6::Full {
                reason: FullReason::Periodic,
                ..
            }
        )));
    assert_eq!(h.latest(0).stream_epoch, epoch);
}
#[test]
fn delta_expired_sent_baseline_starts_new_epoch_full() {
    let mut h = DeltaHarness::new(2, |_| {});
    h.active();
    let old = h.latest(0);
    h.engine.poll_io(5001);
    h.pump();
    let next = h.latest(0);
    assert!(next.stream_epoch > old.stream_epoch);
    assert!(matches!(
        next.payload,
        PublicationPayloadV6::Full {
            reason: FullReason::Expired,
            ..
        }
    ));
}
#[test]
fn delta_client_cache_loss_and_resync_are_atomic_and_idempotent() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    h.ticks(2);
    let mut client = ReplicaV6::new(h.engine.create.key.clone(), "P03".into());
    let old = h.latest(3);
    assert_eq!(client.apply(&old), Err(DecodeError::BaselineMissing));
    let request = h.input(
        3,
        Action::RequestFull {
            stream_epoch: old.stream_epoch,
        },
    );
    let epoch = h.engine.seats[3].epoch;
    h.engine.input(request.clone()).unwrap();
    h.engine.input(request).unwrap();
    assert_eq!(h.engine.seats[3].epoch, epoch + 1);
    h.pump();
    let full = h.latest(3);
    assert!(matches!(
        full.payload,
        PublicationPayloadV6::Full {
            reason: FullReason::Resync,
            ..
        }
    ));
    client.apply(&full).unwrap();
    h.ticks(1);
    client.apply(&h.latest(3)).unwrap();
    assert_eq!(client.snapshot(), Some(h.snapshot(3)));
    assert_eq!(
        h.action(
            3,
            Action::RequestFull {
                stream_epoch: old.stream_epoch
            }
        ),
        Err(Error::StalePublication)
    );
}
#[test]
fn delta_request_full_cannot_reuse_an_old_connection_or_change_peer() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    let old = h.input(
        0,
        Action::RequestFull {
            stream_epoch: h.latest(0).stream_epoch,
        },
    );
    let peer = h.latest(7).stream_epoch;
    let prev = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: prev,
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "replacement".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    assert_eq!(h.engine.input(old), Err(Error::StaleConnection));
    h.pump();
    assert!(matches!(
        h.latest(0).payload,
        PublicationPayloadV6::Full { .. }
    ));
    assert_eq!(h.latest(7).stream_epoch, peer);
}
#[test]
fn delta_background_foreground_requires_new_sent_full_and_preserves_costs() {
    let mut h = DeltaHarness::new(2, |_| {});
    h.active();
    let old = h.latest(0);
    let spirit = h.snapshot(0).own.spirit;
    h.action(0, Action::Background {}).unwrap();
    h.pump();
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    assert_eq!(
        h.action(
            0,
            Action::CombatReady {
                publication_id: old.publication_id
            }
        ),
        Err(Error::NotReady)
    );
    let publication_id = h.replicas[0].last_full_id().unwrap().to_string();
    h.action(0, Action::CombatReady { publication_id }).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.spirit, spirit);
    assert!(h.snapshot(0).own.ready);
}
fn full_fixture() -> (DeltaHarness, PublicationEnvelopeV6) {
    let mut h = DeltaHarness::new(2, |_| {});
    h.active();
    let f = h
        .frames
        .iter()
        .rev()
        .find(|f| f.recipient_id == "P00" && matches!(f.payload, PublicationPayloadV6::Full { .. }))
        .unwrap()
        .clone();
    (h, f)
}
#[test]
fn delta_decoder_rejects_duplicate_conflicts_and_old_frames_without_mutating() {
    let (_, f) = full_fixture();
    let mut r = ReplicaV6::new(f.key.clone(), f.recipient_id.clone());
    let snapshot = r.apply(&f).unwrap().clone();
    r.apply(&f).unwrap();
    let mut wrong = f.clone();
    if let PublicationPayloadV6::Full { snapshot, .. } = &mut wrong.payload {
        snapshot.own.spirit -= 1;
    }
    assert_eq!(r.apply(&wrong), Err(DecodeError::Conflict));
    assert_eq!(r.snapshot(), Some(&snapshot));
    let mut wrong = f.clone();
    wrong.revision -= 1;
    assert_eq!(r.apply(&wrong), Err(DecodeError::StaleFrame));
}
#[test]
fn delta_decoder_rejects_wrong_key_owner_epoch_base_and_missing_cache() {
    let (mut h, f) = full_fixture();
    let mut r = ReplicaV6::new(f.key.clone(), "P00".into());
    r.apply(&f).unwrap();
    let before = r.snapshot().unwrap().clone();
    let mut wrong = f.clone();
    wrong.recipient_id = "P01".into();
    assert_eq!(r.apply(&wrong), Err(DecodeError::WrongRecipient));
    h.ticks(2);
    let mut d = h.latest(0);
    if let PublicationPayloadV6::Delta { base_revision, .. } = &mut d.payload {
        *base_revision += 1;
    }
    assert_eq!(r.apply(&d), Err(DecodeError::BaselineMissing));
    assert_eq!(r.snapshot(), Some(&before));
    r.forget_baseline();
    assert_eq!(r.apply(&h.latest(0)), Err(DecodeError::BaselineMissing));
}
#[test]
fn delta_collection_diff_preserves_removal_reorder_null_and_empty() {
    let (h, f) = full_fixture();
    let base = h.snapshot(0).clone();
    let mut next = base.clone();
    next.logical_ms += 30;
    next.members.reverse();
    next.monsters.clear();
    next.resources.push(ResourceViewV4 {
        id: "test-resource".into(),
        position: [400., 400.],
        quantity: 1,
        item_id: Some("xuan-tie".into()),
        owner: None,
        expires_at_ms: 2000,
        instance: None,
    });
    next.own.pending_action = None;
    next.own.statuses.clear();
    next.own.inventory.clear();
    next.active_at_ms = None;
    let mut initial = f.clone();
    initial.payload = PublicationPayloadV6::Full {
        snapshot: Box::new(base),
        reason: FullReason::Initial,
    };
    let mut r = ReplicaV6::new(f.key.clone(), "P00".into());
    r.apply(&initial).unwrap();
    let d = PublicationEnvelopeV6 {
        publication_id: "delta-test".into(),
        revision: f.revision + 1,
        payload: PublicationPayloadV6::Delta {
            base_publication_id: f.publication_id.clone(),
            base_revision: f.revision,
            delta: Box::new(crate::replication::diff(r.snapshot().unwrap(), &next)),
        },
        ..f
    };
    assert_eq!(r.apply(&d).unwrap(), &next);
}
#[test]
fn delta_bad_collection_order_or_remove_does_not_partially_apply() {
    let (h, f) = full_fixture();
    let base = h.snapshot(0).clone();
    for mode in 0..3 {
        let mut full = f.clone();
        full.payload = PublicationPayloadV6::Full {
            snapshot: Box::new(base.clone()),
            reason: FullReason::Initial,
        };
        let mut r = ReplicaV6::new(f.key.clone(), "P00".into());
        r.apply(&full).unwrap();
        let mut target = base.clone();
        target.logical_ms += 30;
        target.own.spirit -= 1;
        target.members.reverse();
        let mut patch = crate::replication::diff(&base, &target);
        let m = patch.members.as_mut().unwrap();
        match mode {
            0 => m.order.push(m.order[0].clone()),
            1 => m.removed.push(EntityRef {
                kind: StatusEntityKind::Participant,
                id: "missing".into(),
                generation: 1,
            }),
            _ => {
                m.order.pop();
            }
        }
        let e = PublicationEnvelopeV6 {
            publication_id: "bad-list".into(),
            revision: f.revision + 1,
            payload: PublicationPayloadV6::Delta {
                base_publication_id: f.publication_id.clone(),
                base_revision: f.revision,
                delta: Box::new(patch),
            },
            ..f.clone()
        };
        assert_eq!(r.apply(&e), Err(DecodeError::InvalidFrame));
        assert_eq!(r.snapshot(), Some(&base));
    }
}
#[test]
fn delta_dto_rejects_unknown_and_null_patch_fields() {
    let wire = json!({"logical_ms":30,"own":null});
    assert!(serde_json::from_value::<SnapshotDeltaV6>(wire).is_err());
    assert!(
        serde_json::from_value::<SnapshotDeltaV6>(json!({"logical_ms":30,"world_patch":{}}))
            .is_err()
    );
}
#[test]
fn delta_large_patch_uses_full_instead_of_growing_payload() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    for i in 0..8 {
        h.action(
            i,
            Action::Move {
                direction: [0., 1.],
            },
        )
        .unwrap();
    }
    h.ticks(3);
    assert!(h.frames.iter().any(|f| matches!(
        f.payload,
        PublicationPayloadV6::Full {
            reason: FullReason::Size,
            ..
        }
    )));
}
#[test]
fn delta_terminal_full_failure_can_recover_after_world_closed() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    h.ack[3] = false;
    h.close();
    assert!(h.engine.world.is_none());
    assert_eq!(h.engine.pending(), 0);
    let f = h.latest(3);
    assert!(matches!(f.payload, PublicationPayloadV6::Full { .. }));
    h.transport(&f, TransportOutcome::Failed).unwrap();
    h.ack[3] = true;
    h.pump();
    assert_eq!(h.snapshot(3).own.phase, MemberPhase::Terminal);
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}
#[test]
fn delta_closed_grace_expiry_is_explicit_and_does_not_undo_assets() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    h.ack[3] = false;
    h.close();
    h.engine.poll_io(3601);
    h.pump();
    assert!(
        h.events.iter().any(|(id, e)| id.as_deref() == Some("P03")
            && matches!(e, EventV4::PublicationExpired { .. }))
    );
    assert_eq!(h.engine.stream_cache_counts(3), (0, 0));
    assert_eq!(
        h.action(
            3,
            Action::RequestFull {
                stream_epoch: h.engine.seats[3].epoch
            }
        ),
        Err(Error::Closed)
    );
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}
#[test]
fn delta_terminal_resync_uses_frozen_view_not_survivor_positions() {
    let mut h = DeltaHarness::new(2, |_| {});
    h.active();
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Settling);
    let frozen = h.snapshot(0).members.clone();
    h.action(
        1,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(3);
    assert_ne!(h.snapshot(1).members, frozen);
    h.action(
        0,
        Action::RequestFull {
            stream_epoch: h.engine.seats[0].epoch,
        },
    )
    .unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).members, frozen);
    assert_eq!(h.engine.phase, Phase::Active);
    h.hold_settle[0] = false;
    for (i, r) in std::mem::take(&mut h.held) {
        let proof = h.stores[i].execute(&r);
        h.receipt(i, &r, proof).unwrap();
    }
    h.pump();
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Terminal);
    assert_eq!(h.snapshot(0).members[1].position, frozen[1].position);
    assert_eq!(h.snapshot(0).members[0].phase, MemberPhase::Terminal);
}
#[test]
fn delta_unknown_terminal_core_continues_after_publication_expiry() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    h.hold_settle[3] = true;
    h.close();
    h.engine.poll_io(3601);
    h.pump();
    assert!(h.engine.world.is_none());
    assert!(h.engine.pending() > 0);
    h.hold_settle[3] = false;
    let held = std::mem::take(&mut h.held);
    for (i, r) in held {
        let proof = h.stores[i].execute(&r);
        h.receipt(i, &r, proof).unwrap();
    }
    h.pump();
    assert_eq!(h.engine.pending(), 0);
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}
#[test]
fn delta_frozen_financial_recovery_has_no_baselines_or_world() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    h.ticks(8);
    h.hold_settle.fill(true);
    h.close();
    let cp = h.checkpoint.clone().unwrap();
    let original = h.engine.create.clone();
    for generation in [1, 2] {
        let mut r = original.clone();
        r.recovery = Some(RecoveryV4 {
            generation,
            checkpoint: cp.clone(),
        });
        h.engine = TwoMembers::create(
            h.engine.config.clone(),
            delta_catalog(8, |_| {}).unwrap(),
            r,
        )
        .unwrap();
        h.hold_settle.fill(false);
        h.pump();
        assert!(h.engine.world.is_none());
        assert_eq!(h.engine.pending(), 0);
        for i in 0..8 {
            assert_eq!(h.engine.stream_cache_counts(i), (0, 0));
        }
    }
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}
#[test]
fn delta_real_frames_interoperate_with_client_and_emit_smaller_deltas() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    for step in 0..70 {
        if step == 0 || step == 28 {
            h.action(
                0,
                Action::Cast {
                    skill_id: h.engine.config.skill.id.clone(),
                    direction: [1., 0.],
                },
            )
            .unwrap();
        }
        if step == 38
            && let Some(r) = h
                .snapshot(0)
                .resources
                .iter()
                .find(|r| r.item_id.is_some())
                .cloned()
        {
            h.action(
                0,
                Action::Navigate {
                    destination: r.position,
                },
            )
            .unwrap();
        }
        if (40..70).contains(&step)
            && let Some(r) = h
                .snapshot(0)
                .resources
                .iter()
                .find(|r| r.item_id.is_some())
                .cloned()
            && crate::config::distance(h.snapshot(0).own.position, r.position)
                <= h.engine.config.params.pickup_range
        {
            h.action(
                0,
                Action::Pickup {
                    resource_id: r.id,
                    expected_revision: h.snapshot(0).own.inventory_revision,
                },
            )
            .unwrap();
        }
        if step == 3 {
            h.action(
                7,
                Action::Move {
                    direction: [0., 1.],
                },
            )
            .unwrap();
        }
        if step == 8 {
            h.action(7, Action::Stop {}).unwrap();
        }
        if step == 12 {
            h.action(
                3,
                Action::CastSelf {
                    skill_id: h.engine.config.additional_skills[0].id.clone(),
                },
            )
            .unwrap();
        }
        h.ticks(1);
    }
    h.close();
    assert!(
        h.frames
            .iter()
            .any(|f| matches!(f.payload, PublicationPayloadV6::Delta { .. }))
    );
    for v in &h.vectors {
        let e: PublicationEnvelopeV6 = serde_json::from_value(v["envelope"].clone()).unwrap();
        if matches!(e.payload, PublicationPayloadV6::Delta { .. }) {
            let target: SnapshotV4 = serde_json::from_value(v["snapshot"].clone()).unwrap();
            let full = PublicationPayloadV6::Full {
                snapshot: Box::new(target),
                reason: FullReason::Size,
            };
            assert!(
                serde_json::to_vec(&e.payload).unwrap().len()
                    < serde_json::to_vec(&full).unwrap().len()
            );
        }
    }
    if let Ok(path) = std::env::var("SDK520_INTEROP_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&h.vectors).unwrap()).unwrap();
    }
}

#[test]
fn delta_public_runtime_hundred_cycles_deliver_terminal_full_and_release_threads() {
    // This is a resource-lifecycle check under the parallel full suite. Timeout
    // recovery has separate deterministic tests using the normal 600ms budget.
    let cat = delta_catalog(8, |v| {
        v["replication"] = json!({"ack_timeout_ms":30000,"baseline_ttl_ms":60000,"full_interval_ms":60000,"terminal_grace_ms":60000});
    }).unwrap();
    let c = cat.default_config().as_ref().clone();
    for cycle in 0..100 {
        let runtime = crate::Runtime::with_catalog(cat.clone());
        let client = runtime.client();
        let mut request = delta_request(&c);
        request.key.id = format!("runtime-cycle-{cycle}");
        let key = request.key.clone();
        client.create(typed6(request.clone())).unwrap();
        if cycle == 0 {
            for _ in 0..3 {
                client.abandon_drain_for_test();
            }
        }
        let mut stores: Vec<_> = (0..8)
            .map(|i| {
                let mut s = Store::new(i, &c);
                s.principal = format!("principal-P{i:02}");
                s
            })
            .collect();
        let mut snapshots: Vec<Option<String>> = vec![None; 8];
        let mut checkpoint = None;
        let mut terminal_seen = [false; 8];
        let participants = request.participants.clone();
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
                                let i = participants
                                    .iter()
                                    .position(|p| p.participant_id == participant_id)
                                    .unwrap();
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
                            OutputBody::PublicationV6 {
                                recipient_id,
                                connection,
                                payload,
                                ..
                            } => {
                                let i = participants
                                    .iter()
                                    .position(|p| p.participant_id == recipient_id)
                                    .unwrap();
                                let result = client.submit_transport_receipt(TransportReceipt {
                                    key: key.clone(),
                                    output_id: o.id.clone(),
                                    recipient_id,
                                    connection,
                                    outcome: TransportOutcome::Sent,
                                });
                                assert!(result.is_ok() || result == Err(Error::Closed));
                                if matches!(payload.as_ref(), PublicationPayloadV6::Full { .. }) {
                                    snapshots[i] = Some(o.id);
                                }
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
                        let i = participants
                            .iter()
                            .position(|p| p.participant_id == participant_id)
                            .unwrap();
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
                    OutputBody::PublicationV6 {
                        recipient_id,
                        connection,
                        payload,
                        ..
                    } => {
                        let i = participants
                            .iter()
                            .position(|p| p.participant_id == recipient_id)
                            .unwrap();
                        if matches!(payload.as_ref(),PublicationPayloadV6::Full{snapshot,..} if snapshot.own.phase==MemberPhase::Terminal)
                        {
                            terminal_seen[i] = true;
                        }
                        client
                            .submit_transport_receipt(TransportReceipt {
                                key: key.clone(),
                                recipient_id,
                                connection,
                                output_id: o.id,
                                outcome: TransportOutcome::Sent,
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
        assert!(terminal_seen.iter().all(|v| *v));
        assert!(client.drain_outputs().unwrap().is_empty());
        drop(runtime);
        assert_eq!(client.drain_outputs(), Err(Error::Closed));
    }
}

#[test]
fn delta_member_terminal_delivery_expires_without_stopping_other_members() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    h.ack[0] = false;
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    h.engine.poll_io(3601);
    h.pump();
    assert!(
        h.events.iter().any(|(id, e)| id.as_deref() == Some("P00")
            && matches!(e, EventV4::PublicationExpired { .. }))
    );
    assert_eq!(h.engine.phase, Phase::Active);
    assert_eq!(h.stores[0].settlements, 1);
    let before = h.snapshot(7).own.position;
    h.action(
        7,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(2);
    assert!(h.snapshot(7).own.position[1] > before[1]);
}

#[test]
fn delta_request_full_requires_a_real_view_and_cannot_hot_restore_financial_recovery() {
    let mut h = DeltaHarness::new(2, |_| {});
    assert_eq!(
        h.action(0, Action::RequestFull { stream_epoch: 1 }),
        Err(Error::NotReady)
    );
    h.active();
    h.close();
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: h.checkpoint.clone().unwrap(),
    });
    h.engine = TwoMembers::create(
        h.engine.config.clone(),
        delta_catalog(2, |_| {}).unwrap(),
        r,
    )
    .unwrap();
    assert_eq!(
        h.action(0, Action::RequestFull { stream_epoch: 1 }),
        Err(Error::Closed)
    );
    assert!(h.engine.world.is_none());
    assert!(
        h.engine
            .drain()
            .iter()
            .all(|o| !matches!(o.body, OutputBody::PublicationV6 { .. }))
    );
}

#[test]
fn delta_expiry_notice_survives_output_backpressure() {
    let mut h = DeltaHarness::new(8, |_| {});
    h.active();
    h.ack[3] = false;
    h.close();
    let entity = h.snapshot(0).own.participant.entity.clone();
    while h.engine.outputs.len() < h.engine.config.output_capacity {
        h.engine.emit(OutputBody::EventV4 {
            recipient_id: None,
            event: EventV4::MemberChanged {
                entity: entity.clone(),
                phase: MemberPhase::Terminal,
            },
        });
    }
    h.engine.poll_io(3601);
    assert!(h.engine.outputs.len() <= h.engine.config.output_capacity);
    h.pump();
    assert!(
        h.events.iter().any(|(id, e)| id.as_deref() == Some("P03")
            && matches!(e, EventV4::PublicationExpired { .. }))
    );
    assert_eq!(h.engine.pending(), 0);
}

#[path = "periodic_tests.rs"]
mod periodic;

#[path = "shield_tests.rs"]
mod shields;

#[path = "combat_tests.rs"]
mod combat;

#[path = "area_tests.rs"]
mod areas;

#[path = "line_tests.rs"]
mod lines;

#[path = "piercing_tests.rs"]
mod flights;

#[path = "skill_slot_tests.rs"]
mod slots;

#[path = "hp_cost_tests.rs"]
mod hp_cost;

#[path = "cast_movement_tests.rs"]
mod cast_movement;

#[path = "charge_tests.rs"]
mod charge;

#[path = "reaim_tests.rs"]
mod reaim;

#[path = "fixed_channel_tests.rs"]
mod fixed_channel;

#[path = "projectile_recast_tests.rs"]
mod projectile_recast;

#[path = "recovery_followup_tests.rs"]
mod recovery_followup;

#[path = "cancelable_recovery_tests.rs"]
mod cancelable_recovery;

#[path = "basic_attack_followup_tests.rs"]
mod basic_attack_followup;

#[path = "locked_basic_target_tests.rs"]
mod locked_basic_target;

#[path = "basic_pursuit_tests.rs"]
mod basic_pursuit;

#[path = "member_loot_drop_tests.rs"]
mod member_loot_drop;

#[path = "member_item_use_tests.rs"]
mod member_item_use;

#[path = "member_item_cooldown_tests.rs"]
mod member_item_cooldown;

#[path = "member_item_spirit_tests.rs"]
mod member_item_spirit;

#[path = "member_item_cooldown_group_tests.rs"]
mod member_item_cooldown_group;

#[path = "member_item_dual_recovery_tests.rs"]
mod member_item_dual_recovery;

#[path = "member_item_ratio_recovery_tests.rs"]
mod member_item_ratio_recovery;

#[path = "member_item_quality_recovery_tests.rs"]
mod member_item_quality_recovery;

#[path = "member_item_self_status_tests.rs"]
mod member_item_self_status;

#[path = "member_item_status_quality_tests.rs"]
mod member_item_status_quality;

#[path = "member_item_damage_reduction_tests.rs"]
mod member_item_damage_reduction;

#[path = "cast_lifecycle_tests.rs"]
mod cast_lifecycle;
#[path = "skill_loadout_tests.rs"]
mod skill_loadouts;

#[path = "source_terminal_settlement_tests.rs"]
mod source_terminal_settlement;

#[path = "source_mastery_tests.rs"]
mod source_mastery;

#[path = "source_time_current_tests.rs"]
mod source_time_current;

#[path = "source_ally_recovery_tests.rs"]
mod source_ally_recovery;
#[path = "source_ally_self_and_marks_tests.rs"]
mod source_ally_self_and_marks;
#[path = "source_guard_family_tests.rs"]
mod source_guard_family;
#[path = "source_primordial_seal_tests.rs"]
mod source_primordial_seal;
#[path = "source_retaliation_tests.rs"]
mod source_retaliation;
#[path = "source_stillness_guard_tests.rs"]
mod source_stillness_guard;

#[path = "basic_profile_tests.rs"]
mod basic_profiles;
