use super::*;

fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(55);
    value["basic_pursuit"] = json!({"max_range":750,"duration_ms":1500});
    value["basic"]["range"] = json!(10);
    value["basic"]["windup_ms"] = json!(60);
}

fn attack(h: &DeltaHarness) -> Action {
    let npc = &h.snapshot(0).monsters[0].entity;
    Action::AttackUnit {
        target: UnitTarget {
            id: npc.id.clone(),
            generation: npc.generation,
        },
    }
}

#[test]
fn basic_pursuit_config_validates_optional_bounds_and_keeps_54() {
    assert!(delta_catalog(8, configure).is_ok());
    for bad in [
        Value::Null,
        json!({}),
        json!({"max_range":10,"duration_ms":1500}),
        json!({"max_range":10001,"duration_ms":1500}),
        json!({"max_range":750,"duration_ms":0}),
        json!({"max_range":750,"duration_ms":151}),
        json!({"max_range":750,"duration_ms":1830}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["basic_pursuit"] = bad;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["basic_pursuit"]["duration_ms"] = json!(1800);
        })
        .is_ok()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(54);
        })
        .is_err()
    );
    for version in [54, 55] {
        let mut h = DeltaHarness::new(2, |v| {
            configure(v);
            v["mechanism_version"] = json!(version);
            v.as_object_mut().unwrap().remove("basic_pursuit");
        });
        h.active();
        assert_eq!(h.action(0, attack(&h)), Err(Error::InvalidTarget));
        assert!(h.snapshot(0).own.basic_pursuit.is_none());
        h.close();
    }
}

#[test]
fn basic_pursuit_duplicate_and_reconnect_clear_only_current_actor() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let action = attack(&h);
    let first = h.input(0, action.clone());
    let result = h.engine.input(first.clone());
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(h.engine.input(first.clone()), result);
    h.action(1, action.clone()).unwrap();
    h.pump();
    assert_eq!(
        h.snapshot(0).own.basic_pursuit.as_ref().unwrap().action_id,
        first.request_id
    );
    let peer = h.snapshot(1).own.basic_pursuit.clone();
    assert!(peer.is_some());
    assert!(
        !serde_json::to_string(&h.snapshot(1).members)
            .unwrap()
            .contains("basic_pursuit")
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
        id: "pursuit-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    reconnect.request_id = "pursuit-reconnect".into();
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.basic_pursuit.is_none());
    assert_eq!(h.snapshot(1).own.basic_pursuit, peer);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 0);
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn basic_pursuit_replica_rejects_bad_clock_and_occupied_phase_atomically() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    h.action(0, attack(&h)).unwrap();
    h.pump();
    let before = h.snapshot(0).clone();
    for mode in 0..4 {
        let mut bad = before.clone();
        let plan = bad.own.basic_pursuit.as_mut().unwrap();
        match mode {
            0 => plan.target.generation = 0,
            1 => plan.expires_at_ms = bad.logical_ms,
            2 => plan.started_at_ms = bad.logical_ms + 1,
            _ => bad.own.basic_ready_at_ms = bad.logical_ms + 1,
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-pursuit-{mode}");
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

fn configure_auto(value: &mut Value) {
    configure(value);
    value["mechanism_version"] = json!(56);
    value["basic_pursuit"]["auto_max_range"] = json!(750);
}

#[test]
fn auto_basic_pursuit_config_gates_opt_in_and_validates_both_ranges() {
    assert!(delta_catalog(8, configure_auto).is_ok());
    for bad in [
        json!(null),
        json!("750"),
        json!(10),
        json!(751),
        json!(false),
    ] {
        assert!(
            delta_catalog(8, |value| {
                configure_auto(value);
                value["basic_pursuit"]["auto_max_range"] = bad;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |value| {
            configure_auto(value);
            value["mechanism_version"] = json!(55);
        })
        .is_err()
    );
    for version in [55, 56] {
        let mut h = DeltaHarness::new(2, |v| {
            configure(v);
            v["mechanism_version"] = json!(version);
        });
        h.active();
        assert_eq!(h.action(0, Action::Attack {}), Err(Error::InvalidTarget));
        h.action(0, attack(&h)).unwrap();
        h.pump();
        assert!(h.snapshot(0).own.basic_pursuit.is_some());
        h.close();
    }
}

#[test]
fn auto_basic_pursuit_public_idempotency_reconnect_and_private_state() {
    let mut h = DeltaHarness::new(2, configure_auto);
    h.active();
    let first = h.input(0, Action::Attack {});
    let accepted = h.engine.input(first.clone()).unwrap();
    assert_eq!(h.engine.input(first.clone()).unwrap(), accepted);
    h.action(1, Action::Attack {}).unwrap();
    h.pump();
    let pursuit = h.snapshot(0).own.basic_pursuit.as_ref().unwrap();
    assert_eq!(pursuit.action_id, first.request_id);
    assert!(
        h.snapshot(0)
            .monsters
            .iter()
            .any(|npc| npc.entity.id == pursuit.target.id
                && npc.entity.generation == pursuit.target.generation)
    );
    let peer = h.snapshot(1).own.basic_pursuit.clone().unwrap();
    assert!(
        !serde_json::to_string(&h.snapshot(1).members)
            .unwrap()
            .contains("basic_pursuit")
    );

    let stale = h.input(0, Action::CancelAction {});
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: stale.connection.clone(),
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "auto-pursuit-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    reconnect.request_id = "auto-pursuit-reconnect".into();
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.basic_pursuit.is_none());
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 0);
    assert_eq!(h.snapshot(1).own.basic_pursuit.as_ref(), Some(&peer));
    h.action(1, Action::CancelAction {}).unwrap();
    h.pump();
    assert!(h.snapshot(1).own.basic_pursuit.is_none());
    assert_eq!(h.snapshot(1).own.basic_ready_at_ms, 0);
    h.close();
    assert_eq!(h.engine.pending(), 0);
}
