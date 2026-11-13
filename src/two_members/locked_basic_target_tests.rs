use super::*;

fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(54);
    value["basic_followup"] = json!({"grace_ms":150});
    value["basic"]["windup_ms"] = json!(60);
    value["basic"]["interval_ms"] = json!(180);
    value["basic"]["range"] = json!(1000);
    value["basic"]["projectile_speed"] = json!(15);
}

fn locked(h: &DeltaHarness, index: usize) -> Action {
    let entity = &h.snapshot(0).monsters[index].entity;
    Action::AttackUnit {
        target: UnitTarget {
            id: entity.id.clone(),
            generation: entity.generation,
        },
    }
}

#[test]
fn locked_basic_shape_and_version_are_strict_and_legacy_followup_still_works() {
    assert!(delta_catalog(8, configure).is_ok());
    let mut h = DeltaHarness::new(2, |v| {
        configure(v);
        v["mechanism_version"] = json!(53);
    });
    h.active();
    assert_eq!(h.action(0, locked(&h, 0)), Err(Error::Unsupported));
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(2);
    h.action(0, Action::Attack {}).unwrap();
    h.pump();
    assert!(h.snapshot(0).own.basic_followup.is_some());
    h.close();

    let mut h = DeltaHarness::new(2, configure);
    h.active();
    for target in [
        json!({"id":"npc","generation":1,"extra":true}),
        json!({"id":"npc"}),
        Value::Null,
    ] {
        assert!(
            serde_json::from_value::<Action>(json!({"kind":"attack_unit","target":target}))
                .is_err()
        );
    }
    assert_eq!(
        h.action(
            0,
            Action::AttackUnit {
                target: UnitTarget {
                    id: "npc".into(),
                    generation: 0
                }
            }
        ),
        Err(Error::InvalidMessage)
    );
    assert_eq!(
        h.action(
            0,
            Action::AttackUnit {
                target: UnitTarget {
                    id: "P01".into(),
                    generation: 1
                }
            }
        ),
        Err(Error::InvalidTarget)
    );
    h.close();
}

#[test]
fn locked_basic_duplicate_and_reconnect_preserve_peer_attack() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let action = locked(&h, 0);
    let first = h.input(0, action.clone());
    let accepted = h.engine.input(first.clone());
    assert!(accepted.is_ok());
    assert_eq!(h.engine.input(first), accepted);
    h.action(1, action.clone()).unwrap();
    h.pump();
    assert!(
        h.snapshot(0)
            .own
            .pending_action
            .as_ref()
            .unwrap()
            .unit_target
            .is_some()
    );
    assert!(
        !serde_json::to_string(&h.snapshot(1).members)
            .unwrap()
            .contains("unit_target")
    );
    let stale = h.input(0, action);
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: stale.connection.clone(),
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "basic-target-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    reconnect.request_id = "basic-target-reconnect".into();
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.pending_action.is_none());
    assert!(h.snapshot(1).own.pending_action.is_some());
    h.ticks(2);
    assert!(
        h.events
            .iter()
            .any(|(recipient, event)| recipient.as_deref() == Some("P01")
                && matches!(
                    event,
                    EventV4::Private {
                        event: Event::ActionCommitted {
                            unit_target: Some(_),
                            skill: false,
                            ..
                        }
                    }
                ))
    );
    assert!(
        !h.events
            .iter()
            .any(|(recipient, event)| recipient.as_deref() == Some("P00")
                && matches!(
                    event,
                    EventV4::Private {
                        event: Event::ActionCommitted { skill: false, .. }
                    }
                ))
    );
    h.close();
}

#[test]
fn locked_basic_replica_rejects_bad_identity_and_mixed_aim_atomically() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    h.action(0, locked(&h, 0)).unwrap();
    h.pump();
    let before = h.snapshot(0).clone();
    for mode in 0..4 {
        let mut bad = before.clone();
        let p = bad.own.pending_action.as_mut().unwrap();
        let unit = p.unit_target.as_mut().unwrap();
        match mode {
            0 => unit.generation += 1,
            1 => unit.generation = 0,
            2 => unit.id = "missing".into(),
            _ => p.ground_position = Some([0., 0.]),
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-lock-{mode}");
        envelope.payload = PublicationPayloadV6::Full {
            snapshot: Box::new(bad),
            reason: FullReason::Resync,
        };
        assert_eq!(
            h.replicas[0].apply(&envelope),
            Err(DecodeError::InvalidFrame)
        );
        assert_eq!(h.replicas[0].snapshot(), Some(&before));
    }
}
