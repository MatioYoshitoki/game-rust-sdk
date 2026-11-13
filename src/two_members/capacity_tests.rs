use super::*;
fn capacity_catalog(
    n: usize,
    change: impl FnOnce(&mut Value),
) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    catalog_with(|v| {
        v["schema_version"] = json!(6);
        v["mechanism_version"] = json!(36);
        v["id"] = json!("capacity-native");
        v["output_capacity"] = json!(1024);
        let mut block = v.as_object_mut().unwrap().remove("two_members").unwrap();
        block["capacity"] = json!(n);
        block["projectile_capacity"] = json!(64);
        block["spawns"] = json!(
            (0..n)
                .map(|i| [400. - (i / 2) as f64 * 64., 400. + (i % 2) as f64 * 80.])
                .collect::<Vec<_>>()
        );
        v["members"] = block;
        change(v);
    })
}
fn request(c: &Config) -> CreateV4 {
    let mut r = create(c);
    r.protocol_version = 5;
    r.participants = (0..c.member_settings().unwrap().capacity)
        .map(|i| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            combat_build: None,
            skills: None,
            participant_id: format!("P{i:02}"),
            authenticated_principal: format!("principal-P{i:02}"),
            connection: Connection {
                id: format!("connection-{i}"),
                generation: 1,
            },
        })
        .collect();
    r
}
fn typed(r: CreateV4) -> CreateV5 {
    serde_json::from_value(serde_json::to_value(r).unwrap()).unwrap()
}
struct MultiHarness {
    engine: TwoMembers,
    stores: Vec<Store>,
    sequences: Vec<u32>,
    snapshots: Vec<Option<(String, SnapshotV4)>>,
    events: Vec<(Option<String>, EventV4)>,
    held: Vec<(usize, BusinessRequest)>,
    hold_admit: Vec<bool>,
    hold_load: Vec<bool>,
    hold_settle: Vec<bool>,
    ack: Vec<bool>,
    checkpoint: Option<IoCheckpointV4>,
}
impl MultiHarness {
    fn new(n: usize, change: impl FnOnce(&mut Value)) -> Self {
        let catalog = capacity_catalog(n, change).unwrap();
        let c = catalog.default_config().as_ref().clone();
        let engine = TwoMembers::create(c.clone(), catalog, request(&c)).unwrap();
        Self {
            engine,
            stores: (0..n)
                .map(|i| {
                    let mut store = Store::new(i, &c);
                    store.principal = format!("principal-P{i:02}");
                    store
                })
                .collect(),
            sequences: vec![0; n],
            snapshots: vec![None; n],
            events: vec![],
            held: vec![],
            hold_admit: vec![false; n],
            hold_load: vec![false; n],
            hold_settle: vec![false; n],
            ack: vec![true; n],
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
                        let i = self.engine.index(&participant_id).unwrap();
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
                        if self.hold_load[i] && matches!(request.body, Business::LoadAssets {})
                            || self.hold_admit[i] && admission
                            || self.hold_settle[i] && settlement
                        {
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
                        let i = self.engine.index(&recipient_id).unwrap();
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
        for i in 0..self.stores.len() {
            self.action(i, Action::Join {}).unwrap();
            self.pump();
        }
    }
    fn active(&mut self) {
        self.join();
        for i in 0..self.stores.len() {
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
fn member_capacity_one_two_four_eight_have_real_independent_world_members() {
    for n in [1, 2, 4, 8] {
        let mut h = MultiHarness::new(n, |_| {});
        h.active();
        let w = h.engine.world.as_ref().unwrap();
        assert_eq!(w.members.len(), n);
        for i in 0..n {
            assert_eq!(h.snapshot(i).own.participant.owner_id, format!("P{i:02}"));
            assert_eq!(h.snapshot(i).own.hp, 1000 + i as u32 * 200);
            assert_eq!(h.stores[i].revision, i as u32 * 10 + 1);
            assert!(h.engine.seats[i].ledger.world.is_none());
        }
        h.close();
        assert!(h.engine.world.is_none());
        assert_eq!(h.engine.pending(), 0);
        assert!(
            h.stores
                .iter()
                .all(|s| s.admissions == 1 && s.settlements == 1)
        );
    }
}
#[test]
fn member_capacity_rejects_invalid_or_mixed_configuration_and_spawn_pairs() {
    for n in [0, 9] {
        assert!(capacity_catalog(n, |_| {}).is_err());
    }
    for path in [
        "/members/capacity",
        "/members/spawns",
        "/members/deployment_timeout_ms",
        "/members/publication_queue_capacity",
        "/members/projectile_capacity",
    ] {
        assert!(
            capacity_catalog(8, |v| {
                v.pointer_mut(path).unwrap().take();
                v.as_object_mut();
            })
            .is_err()
        );
    }
    assert!(capacity_catalog(8, |v| v["members"]["capacity"] = json!(2.5)).is_err());
    assert!(
        capacity_catalog(8, |v| v["members"]["spawns"][7] =
            v["members"]["spawns"][3].clone())
        .is_err()
    );
    assert!(capacity_catalog(8, |v| v["members"]["spawns"][7] = json!([-1., 400.])).is_err());
    assert!(capacity_catalog(8, |v| v["members"]["extra"] = json!(1)).is_err());
    assert!(capacity_catalog(8, |v| v["two_members"] = v["members"].clone()).is_err());
    assert!(capacity_catalog(8, |v| v["schema_version"] = json!(5)).is_err());
    assert!(capacity_catalog(8, |v| v["output_capacity"] = json!(511)).is_err());
}
#[test]
fn member_capacity_create_count_and_every_identity_are_checked_before_io() {
    let cat = capacity_catalog(8, |_| {}).unwrap();
    let c = cat.default_config().as_ref().clone();
    let good = request(&c);
    for count in [0, 1, 7, 9] {
        let mut r = good.clone();
        r.participants.resize(count, good.participants[0].clone());
        assert!(TwoMembers::create(c.clone(), cat.clone(), r).is_err());
    }
    for field in 0..3 {
        let mut r = good.clone();
        match field {
            0 => r.participants[7].participant_id = r.participants[3].participant_id.clone(),
            1 => {
                r.participants[7].authenticated_principal =
                    r.participants[3].authenticated_principal.clone()
            }
            _ => r.participants[7].connection = r.participants[3].connection.clone(),
        };
        assert!(TwoMembers::create(c.clone(), cat.clone(), r).is_err());
    }
    let mut session = crate::session::Session::new(c.clone(), cat);
    assert_eq!(
        session.create(CreateRequest::Two(good.clone())),
        Err(Error::VersionMismatch)
    );
    assert!(session.drain().is_empty());
    session.create(typed(good).into()).unwrap();
    assert_eq!(
        session
            .drain()
            .iter()
            .filter(|o| matches!(o.body, OutputBody::BusinessRequestV4 { .. }))
            .count(),
        8
    );
    let mut wire = serde_json::to_value(typed(request(&c))).unwrap();
    wire["participant_id"] = json!("mixed");
    assert!(serde_json::from_value::<CreateRequest>(wire).is_err());
}
#[test]
fn member_capacity_protocol4_stays_exactly_two() {
    let cat = catalog_with(|_| {}).unwrap();
    let c = cat.default_config().as_ref().clone();
    for count in [1, 3, 8] {
        let mut r = create(&c);
        r.participants.resize(count, r.participants[0].clone());
        assert!(TwoMembers::create(c.clone(), cat.clone(), r).is_err());
    }
    assert!(serde_json::from_value::<CreateV5>(serde_json::to_value(create(&c)).unwrap()).is_err());
    let info = crate::sdk_info();
    assert_eq!(info["members"]["maximum"], 8);
    assert_eq!(info["twoMembers"]["participants"], 2);
}
#[test]
fn member_capacity_last_seat_sent_and_ready_gates_one_common_activation() {
    let mut h = MultiHarness::new(8, |_| {});
    h.ack[7] = false;
    h.join();
    for i in 0..7 {
        let publication_id = h.snapshots[i].as_ref().unwrap().0.clone();
        h.action(i, Action::CombatReady { publication_id }).unwrap();
    }
    let publication_id = h.snapshots[7].as_ref().unwrap().0.clone();
    assert_eq!(
        h.action(
            7,
            Action::CombatReady {
                publication_id: publication_id.clone()
            }
        ),
        Err(Error::NotReady)
    );
    assert_eq!(h.engine.phase, Phase::Deploying);
    h.engine
        .transport(TransportReceipt {
            key: h.engine.create.key.clone(),
            output_id: publication_id.clone(),
            recipient_id: "P07".into(),
            connection: h.engine.seats[7].connection.clone(),
            outcome: TransportOutcome::Sent,
        })
        .unwrap();
    h.action(7, Action::CombatReady { publication_id }).unwrap();
    h.pump();
    let active_at = h.snapshot(0).active_at_ms;
    assert!(active_at.is_some());
    assert!(
        h.snapshots
            .iter()
            .all(|s| s.as_ref().unwrap().1.active_at_ms == active_at)
    );
    assert_eq!(h.engine.phase, Phase::Active);
}
#[test]
fn member_capacity_reverse_admission_receipts_never_cross_revisions() {
    let mut h = MultiHarness::new(8, |_| {});
    h.hold_admit.fill(true);
    h.pump();
    assert_eq!(h.held.len(), 8);
    let mut held = std::mem::take(&mut h.held);
    held.reverse();
    for (i, r) in held {
        let proof = h.stores[i].execute(&r);
        assert_eq!(
            h.receipt((i + 1) % 8, &r, proof.clone()),
            Err(Error::InvalidMessage)
        );
        h.receipt(i, &r, proof.clone()).unwrap();
        h.receipt(i, &r, proof).unwrap();
    }
    h.hold_admit.fill(false);
    h.active();
    assert!(
        h.stores
            .iter()
            .enumerate()
            .all(|(i, s)| s.admissions == 1 && s.revision == i as u32 * 10 + 1)
    );
}
#[test]
fn member_capacity_required_refusal_first_middle_last_refunds_other_commits_once() {
    for refused in [0, 3, 7] {
        let mut h = MultiHarness::new(8, |_| {});
        h.hold_admit.fill(true);
        h.pump();
        let held = std::mem::take(&mut h.held);
        for (i, r) in held.iter().filter(|(i, _)| *i != refused) {
            let proof = h.stores[*i].execute(r);
            h.receipt(*i, r, proof).unwrap();
        }
        let (_, r) = held.iter().find(|(i, _)| *i == refused).unwrap();
        h.receipt(refused, r, ReceiptOutcome::Refused {}).unwrap();
        h.hold_admit.fill(false);
        h.pump();
        assert!(h.engine.world.is_none());
        assert_eq!(h.engine.pending(), 0);
        assert!(h.stores.iter().enumerate().all(|(i, s)| if i == refused {
            s.admissions == 0 && s.compensations == 0
        } else {
            s.admissions == 1 && s.compensations == 1
        }));
        h.close();
    }
}
#[test]
fn member_capacity_missing_last_ready_aborts_and_refunds_all_eight() {
    let mut h = MultiHarness::new(8, |v| v["members"]["deployment_timeout_ms"] = json!(300));
    h.join();
    for i in 0..7 {
        let publication_id = h.snapshots[i].as_ref().unwrap().0.clone();
        h.action(i, Action::CombatReady { publication_id }).unwrap();
    }
    h.ticks(10);
    assert!(h.engine.world.is_none());
    assert!(
        h.stores
            .iter()
            .all(|s| s.compensations == 1 && s.settlements == 0)
    );
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn member_capacity_unknown_admission_is_queried_before_refund() {
    let mut h = MultiHarness::new(8, |_| {});
    h.hold_admit[3] = true;
    h.pump();
    let (_, r) = h.held.pop().unwrap();
    let proof = h.stores[3].execute(&r);
    h.receipt(3, &r, ReceiptOutcome::Unknown {}).unwrap();
    assert_eq!(h.stores[3].compensations, 0);
    h.hold_admit[3] = false;
    h.close();
    h.receipt(3, &r, proof).unwrap();
    h.pump();
    assert!(h.stores.iter().all(|s| s.compensations == 1));
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn member_capacity_same_input_ids_and_self_status_are_independent_for_eight() {
    let mut h = MultiHarness::new(8, |_| {});
    h.active();
    let before: Vec<_> = h
        .snapshots
        .iter()
        .map(|s| s.as_ref().unwrap().1.own.position)
        .collect();
    for i in 0..8 {
        h.action(
            i,
            Action::Move {
                direction: [0., 1.],
            },
        )
        .unwrap();
    }
    h.ticks(2);
    for (i, p) in before.iter().enumerate() {
        assert!(h.snapshot(i).own.position[1] > p[1]);
    }
    let skill_id = h.engine.config.additional_skills[0].id.clone();
    let input = h.input(3, Action::CastSelf { skill_id });
    h.engine.input(input.clone()).unwrap();
    h.engine.input(input).unwrap();
    h.ticks(6);
    assert!(!h.snapshot(3).own.statuses.is_empty());
    assert!(
        h.snapshots
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 3)
            .all(|(_, s)| s.as_ref().unwrap().1.own.statuses.is_empty())
    );
    assert!(h.snapshot(3).own.spirit < 200);
}
#[test]
fn member_capacity_shared_npcs_damage_distinct_members_and_allies_are_ignored() {
    let mut h = MultiHarness::new(8, |_| {});
    h.active();
    h.fire(6);
    h.ticks(40);
    let damaged: Vec<_> = h
        .snapshots
        .iter()
        .filter(|s| {
            let s = &s.as_ref().unwrap().1.own;
            s.hp < s.max_hp
        })
        .collect();
    assert!(damaged.len() >= 2);
    assert_eq!(h.snapshot(2).own.hp, h.snapshot(2).own.max_hp);
    assert!(h.snapshot(0).monsters.iter().any(|m| m.hp < m.max_hp));
}
fn resource(h: &mut MultiHarness) -> ResourceViewV4 {
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
fn member_capacity_eight_compete_for_one_resource_and_duplicate_does_not_repeat() {
    let mut h = MultiHarness::new(8, |_| {});
    h.active();
    let r = resource(&mut h);
    for m in &mut h.engine.world.as_mut().unwrap().members {
        m.position = r.position;
    }
    let intent = h.input(
        7,
        Action::Pickup {
            resource_id: r.id.clone(),
            expected_revision: 1,
        },
    );
    h.engine.input(intent.clone()).unwrap();
    h.engine.input(intent).unwrap();
    for i in 0..7 {
        assert_eq!(
            h.action(
                i,
                Action::Pickup {
                    resource_id: r.id.clone(),
                    expected_revision: 1
                }
            ),
            Err(Error::InvalidTarget)
        );
    }
    h.pump();
    for i in 0..8 {
        assert_eq!(
            h.snapshot(i).own.inventory_revision,
            if i == 7 { 2 } else { 1 }
        );
    }
}
#[test]
fn member_capacity_full_member_leaves_resource_for_next_member() {
    let mut h = MultiHarness::new(8, |v| {
        v["inventory_capacity"] = json!(1);
        v["stack_limit"] = json!(7);
        v["pve"]["items"]["xuan-tie"]["stackLimit"] = json!(7);
    });
    h.active();
    let first = resource(&mut h);
    h.fire(1);
    h.ticks(25);
    h.fire(1);
    h.ticks(6);
    let second = h
        .snapshot(0)
        .resources
        .iter()
        .find(|r| r.item_id.is_some() && r.id != first.id)
        .unwrap()
        .clone();
    h.engine.world.as_mut().unwrap().members[7].position = first.position;
    h.action(
        7,
        Action::Pickup {
            resource_id: first.id,
            expected_revision: 1,
        },
    )
    .unwrap();
    h.engine.world.as_mut().unwrap().members[7].position = second.position;
    assert_eq!(
        h.action(
            7,
            Action::Pickup {
                resource_id: second.id.clone(),
                expected_revision: 2
            }
        ),
        Err(Error::Capacity)
    );
    h.engine.world.as_mut().unwrap().members[0].position = second.position;
    h.action(
        0,
        Action::Pickup {
            resource_id: second.id,
            expected_revision: 1,
        },
    )
    .unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.inventory_revision, 2);
    assert_eq!(h.snapshot(7).own.inventory_revision, 2);
}
#[test]
fn member_capacity_private_publications_and_wrong_recipient_acks_stay_isolated() {
    let mut h = MultiHarness::new(8, |_| {});
    h.active();
    for i in 0..8 {
        let value = serde_json::to_value(h.snapshot(i)).unwrap();
        for m in value["members"].as_array().unwrap() {
            let text = m.to_string();
            for private in [
                "spirit",
                "inventory",
                "revision",
                "principal",
                "operation_id",
                "player_skills",
            ] {
                assert!(!text.contains(private));
            }
        }
    }
    h.ack[3] = false;
    h.ticks(8);
    let id = h.snapshots[3].as_ref().unwrap().0.clone();
    assert_eq!(
        h.engine.transport(TransportReceipt {
            key: h.engine.create.key.clone(),
            output_id: id,
            recipient_id: "P04".into(),
            connection: h.engine.seats[4].connection.clone(),
            outcome: TransportOutcome::Sent
        }),
        Err(Error::StalePublication)
    );
    assert!(h.engine.seats.iter().all(|s| s.queue.len() <= 2));
    assert_eq!(h.snapshot(7).logical_ms, h.engine.now);
}
#[test]
fn member_capacity_reconnect_first_middle_last_fences_old_input_only() {
    for i in [0, 3, 7] {
        let mut h = MultiHarness::new(8, |_| {});
        h.active();
        let stale = h.input(i, Action::Stop {});
        let previous = h.engine.seats[i].connection.clone();
        h.action(i, Action::ConnectionLost {}).unwrap();
        let peer = (i + 1) % 8;
        h.action(
            peer,
            Action::Move {
                direction: [0., 1.],
            },
        )
        .unwrap();
        let before = h.snapshot(peer).own.position;
        h.ticks(2);
        assert!(h.snapshot(peer).own.position[1] > before[1]);
        let mut reconnect = h.input(
            i,
            Action::Reconnect {
                previous_connection: previous,
                authenticated_principal: format!("principal-P{i:02}"),
            },
        );
        reconnect.connection = Connection {
            id: format!("replacement-{i}"),
            generation: 2,
        };
        reconnect.sequence = 1;
        h.engine.input(reconnect).unwrap();
        h.sequences[i] = 1;
        assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
        h.pump();
        h.action(i, Action::Foreground {}).unwrap();
        h.pump();
        let publication_id = h.snapshots[i].as_ref().unwrap().0.clone();
        h.action(i, Action::CombatReady { publication_id }).unwrap();
        h.pump();
        assert!(h.snapshot(i).own.ready);
        assert_eq!(h.engine.phase, Phase::Active);
    }
}
#[test]
fn member_capacity_disconnect_timeout_terminates_only_one_of_eight() {
    let mut h = MultiHarness::new(8, |v| {
        v["params"]["runtime"]["reconnect_grace_ms"] = json!(300)
    });
    h.active();
    h.action(3, Action::ConnectionLost {}).unwrap();
    h.ticks(10);
    assert_eq!(h.stores[3].settlements, 1);
    assert_eq!(h.engine.phase, Phase::Active);
    assert_eq!(h.stores.iter().filter(|s| s.settlements == 0).count(), 7);
}
#[test]
fn member_capacity_unknown_core_and_each_member_exit_leave_survivor_active() {
    let mut h = MultiHarness::new(8, |v| v["pve"]["combat"]["aggro_range"] = json!(1));
    h.active();
    h.ticks(34);
    h.hold_settle[3] = true;
    h.engine.world.as_mut().unwrap().members[3].position = h.engine.config.extraction_point;
    h.action(3, Action::ExtractStart {}).unwrap();
    h.ticks(20);
    assert_eq!(h.stores[3].settlements, 0);
    assert_eq!(h.snapshot(3).own.phase, MemberPhase::Settling);
    h.action(
        7,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    let before = h.snapshot(7).own.position;
    h.ticks(2);
    assert!(h.snapshot(7).own.position[1] > before[1]);
    for i in 0..7 {
        if i != 3 {
            h.action(i, Action::Leave {}).unwrap();
            h.pump();
            assert_eq!(h.engine.phase, Phase::Active);
        }
    }
    h.hold_settle[3] = false;
    let held = std::mem::take(&mut h.held);
    for (i, r) in held {
        let outcome = h.stores[i].execute(&r);
        h.receipt(i, &r, outcome).unwrap();
    }
    h.pump();
    h.action(7, Action::Leave {}).unwrap();
    h.pump();
    assert!(h.stores.iter().all(|s| s.settlements == 1));
    assert!(h.engine.world.is_none());
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn member_capacity_absolute_terminal_and_repeated_close_release_single_world() {
    let mut h = MultiHarness::new(8, |v| {
        v["terminal_ms"] = json!(300);
        v["extraction_unlock_ms"] = json!(0);
        v["extraction_channel_ms"] = json!(30);
        for d in v["pve"]["status_definitions"].as_array_mut().unwrap() {
            d["duration_ms"] = json!(300);
        }
    });
    h.active();
    h.ticks(10);
    h.close();
    h.close();
    assert!(h.engine.world.is_none());
    assert_eq!(h.engine.pending(), 0);
    assert_eq!(
        h.events
            .iter()
            .filter(|(_, e)| matches!(e, EventV4::Closed { live_worlds: 0, .. }))
            .count(),
        1
    );
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}
#[test]
fn member_capacity_eight_frozen_settlements_recover_twice_without_world() {
    let mut h = MultiHarness::new(8, |_| {});
    h.active();
    h.hold_settle.fill(true);
    h.close();
    let checkpoint = h.checkpoint.clone().unwrap();
    let original = h.engine.create.clone();
    for generation in [1, 2] {
        let mut r = original.clone();
        r.recovery = Some(RecoveryV4 {
            generation,
            checkpoint: checkpoint.clone(),
        });
        let cat = capacity_catalog(8, |_| {}).unwrap();
        h.engine = TwoMembers::create(h.engine.config.clone(), cat, r).unwrap();
        h.hold_settle.fill(false);
        h.pump();
        assert!(h.engine.world.is_none());
        assert_eq!(h.engine.pending(), 0);
    }
    assert!(
        h.stores
            .iter()
            .all(|s| s.settlements == 1 && s.compensations == 0)
    );
}
#[test]
fn member_capacity_recovery_rejects_missing_or_reordered_or_rebound_members() {
    let mut h = MultiHarness::new(8, |_| {});
    h.pump();
    let original = h.engine.create.clone();
    for mode in 0..4 {
        let mut cp = h.checkpoint.clone().unwrap();
        match mode {
            0 => {
                cp.state.members.pop();
            }
            1 => cp.state.members.swap(3, 7),
            2 => cp.state.members[7].participant_id = "P03".into(),
            _ => cp.state.members[7].checkpoint = cp.state.members[3].checkpoint.clone(),
        };
        cp.sha256 = state_digest(&cp.state);
        let mut r = original.clone();
        r.recovery = Some(RecoveryV4 {
            generation: 1,
            checkpoint: cp,
        });
        assert!(
            TwoMembers::create(
                h.engine.config.clone(),
                capacity_catalog(8, |_| {}).unwrap(),
                r
            )
            .is_err()
        );
    }
}
#[test]
fn member_capacity_asset_fact_budget_rejects_before_application_and_fits_jsonl_recovery() {
    let mut h = MultiHarness::new(8, |_| {});
    let outputs = h.engine.drain();
    for o in outputs {
        if let OutputBody::BusinessRequestV4 {
            participant_id,
            request,
            ..
        } = o.body
        {
            let i = h.engine.index(&participant_id).unwrap();
            let mut proof = h.stores[i].execute(&request);
            if let ReceiptOutcome::AssetsLoaded { facts } = &mut proof {
                for k in 0..30 {
                    facts.assets.push(AssetAmount {
                        asset_id: format!("extra-{k:02}"),
                        quantity: u32::MAX,
                    });
                }
                facts.assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
            }
            assert_eq!(h.receipt(i, &request, proof), Err(Error::InvalidMessage));
        }
    }
    assert!(h.engine.world.is_none());
    assert!(h.stores.iter().all(|s| s.admissions == 0));
    let mut h = MultiHarness::new(8, |_| {});
    for store in &mut h.stores {
        for k in 0..29 {
            store
                .balances
                .insert(format!("{k:02}{}", "\\".repeat(62)), u32::MAX);
        }
    }
    h.active();
    h.hold_settle.fill(true);
    h.close();
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: h.checkpoint.clone().unwrap(),
    });
    let wire =
        serde_json::to_vec(&serde_json::json!({"type":"create","request":typed(r)})).unwrap();
    assert!(wire.len() < crate::JSONL_LINE_LIMIT, "{}", wire.len());
}

#[test]
fn member_capacity_public_runtime_hundred_eight_member_cycles_release_threads_and_debts() {
    let cat = capacity_catalog(8, |_| {}).unwrap();
    let c = cat.default_config().as_ref().clone();
    for cycle in 0..100 {
        let runtime = crate::Runtime::with_catalog(cat.clone());
        let client = runtime.client();
        let mut request = request(&c);
        request.key.id = format!("runtime-cycle-{cycle}");
        let key = request.key.clone();
        client.create(typed(request.clone())).unwrap();
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
                            OutputBody::PublicationV4 {
                                recipient_id,
                                connection,
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

#[test]
fn member_capacity_first_middle_last_npc_death_does_not_close_survivors() {
    for i in [0, 3, 7] {
        let mut h = MultiHarness::new(8, |v| {
            v["pve"]["monster_positions"][0] = v["members"]["spawns"][i].clone()
        });
        h.stores[i].hp = 10;
        h.active();
        h.ticks(40);
        assert_eq!(h.snapshot(i).own.hp, 0);
        assert_eq!(h.stores[i].settlements, 1);
        assert_eq!(h.engine.phase, Phase::Active);
        assert_eq!(h.stores.iter().filter(|s| s.settlements == 0).count(), 7);
        h.close();
        assert_eq!(h.engine.pending(), 0);
    }
}

#[test]
fn member_capacity_refused_facts_never_admit_that_member_and_compensate_others() {
    for refused in [0, 3, 7] {
        let mut h = MultiHarness::new(8, |_| {});
        h.hold_load[refused] = true;
        h.pump();
        assert!(h.engine.world.is_none());
        assert_eq!(h.stores[refused].admissions, 0);
        let (_, r) = h.held.pop().unwrap();
        h.receipt(refused, &r, ReceiptOutcome::Refused {}).unwrap();
        h.hold_load.fill(false);
        h.pump();
        assert_eq!(h.engine.phase, Phase::Closed);
        assert_eq!(h.engine.pending(), 0);
        assert!(h.stores.iter().enumerate().all(|(i, s)| if i == refused {
            s.admissions == 0 && s.compensations == 0
        } else {
            s.admissions == 1 && s.compensations == 1
        }));
    }
}
#[test]
fn member_capacity_output_pressure_rejects_before_mutation_and_can_close_after_drain() {
    let mut h = MultiHarness::new(8, |_| {});
    h.active();
    let entity = h.snapshot(0).own.participant.entity.clone();
    while h.engine.outputs.len() + h.engine.reserve_margin() <= h.engine.config.output_capacity {
        h.engine.emit(OutputBody::EventV4 {
            recipient_id: None,
            event: EventV4::MemberChanged {
                entity: entity.clone(),
                phase: MemberPhase::Active,
            },
        });
    }
    assert_eq!(h.action(0, Action::Leave {}), Err(Error::QueueFull));
    assert_eq!(
        h.engine.world.as_ref().unwrap().members[0].phase,
        MemberPhase::Active
    );
    assert!(h.stores.iter().all(|s| s.settlements == 0));
    h.pump();
    h.close();
    assert!(h.engine.world.is_none());
    assert_eq!(h.engine.pending(), 0);
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}

#[path = "delta_tests.rs"]
mod delta;
