use super::*;

fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(53);
    value["basic_followup"] = json!({"grace_ms":150});
    value["basic"]["windup_ms"] = json!(30);
    value["basic"]["interval_ms"] = json!(180);
    value["basic"]["range"] = json!(1000);
    value["basic"]["projectile_speed"] = json!(15);
}

#[test]
fn basic_followup_configuration_gates_versions_and_preserves_original_attack() {
    assert!(delta_catalog(8, configure).is_ok());
    for bad in [
        Value::Null,
        json!({}),
        json!({"grace_ms":-1}),
        json!({"grace_ms":151}),
        json!({"grace_ms":1020}),
        json!({"grace_ms":0,"limit":2}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["basic_followup"] = bad;
            })
            .is_err()
        );
    }
    for grace in [0, 990] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["basic_followup"]["grace_ms"] = json!(grace);
            })
            .is_ok()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(52);
        })
        .is_err()
    );
    for version in [52, 53] {
        let mut h = DeltaHarness::new(2, |v| {
            configure(v);
            v["mechanism_version"] = json!(version);
            v.as_object_mut().unwrap().remove("basic_followup");
        });
        h.active();
        h.action(0, Action::Attack {}).unwrap();
        h.ticks(1);
        assert_eq!(h.action(0, Action::Attack {}), Err(Error::InvalidState));
        assert!(h.snapshot(0).own.basic_followup.is_none());
        h.close();
    }
}

#[test]
fn basic_followup_duplicate_and_reconnect_do_not_consume_the_peer_queue() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    for member in 0..2 {
        h.action(member, Action::Attack {}).unwrap();
    }
    h.ticks(2);
    let first = h.input(0, Action::Attack {});
    let result = h.engine.input(first.clone());
    assert!(result.is_ok());
    let latest = h.input(0, Action::Attack {});
    h.engine.input(latest.clone()).unwrap();
    assert_eq!(h.engine.input(first), result);
    let peer = h.input(1, Action::Attack {});
    h.engine.input(peer.clone()).unwrap();
    h.pump();
    assert_eq!(
        h.snapshot(0).own.basic_followup.as_ref().unwrap().action_id,
        latest.request_id
    );
    assert!(
        !serde_json::to_string(&h.snapshot(1).members)
            .unwrap()
            .contains("basic_followup")
    );
    let stale = h.input(0, Action::Attack {});
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: stale.connection.clone(),
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "basic-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    reconnect.request_id = "basic-reconnect".into();
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.basic_followup.is_none());
    assert!(h.snapshot(1).own.basic_followup.is_some());
    h.ticks(6);
    for (pid, id, expected) in [("P00", latest.request_id, 0), ("P01", peer.request_id, 1)] {
        let count=h.events.iter().filter(|(recipient,event)| recipient.as_deref()==Some(pid) && matches!(event,
            EventV4::Private { event: Event::ActionCommitted {action_id, skill: false, ..} } if action_id==&id)).count();
        assert_eq!(count, expected);
    }
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn basic_followup_replica_rejects_bad_identity_deadline_and_phase_atomically() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(1);
    h.action(0, Action::Attack {}).unwrap();
    h.pump();
    let before = h.snapshot(0).clone();
    for mode in 0..4 {
        let mut bad = before.clone();
        let queue = bad.own.basic_followup.as_mut().unwrap();
        match mode {
            0 => queue.target.generation = 0,
            1 => queue.expires_at_ms = queue.promotes_at_ms - 1,
            2 => queue.promotes_at_ms = bad.own.basic_ready_at_ms - 1,
            _ => bad.own.protected = true,
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-basic-{mode}");
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
