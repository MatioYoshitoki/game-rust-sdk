use super::*;

fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(51);
    value["skill_followup"] = json!({"window_ms":150});
    let mut skill = value["additional_skills"][1].clone();
    skill["id"] = json!("opener");
    skill["windup_ms"] = json!(30);
    skill["recovery_ms"] = json!(180);
    skill["speed"] = json!(60);
    value["skill"] = skill.clone();
    skill["id"] = json!("next");
    value["additional_skills"][1] = skill;
}

fn cast(skill_id: &str) -> Action {
    Action::Cast {
        skill_id: skill_id.into(),
        direction: [1., 0.],
    }
}

#[test]
fn followup_config_is_bounded_and_disabled_for_old_or_omitted_config() {
    assert!(delta_catalog(8, configure).is_ok());
    for invalid in [
        Value::Null,
        json!({}),
        json!({"window_ms":0}),
        json!({"window_ms":20}),
        json!({"window_ms":151}),
        json!({"window_ms":1020}),
        json!({"window_ms":150,"slots":2}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["skill_followup"] = invalid;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["skill_followup"]["window_ms"] = json!(990);
        })
        .is_ok()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(50);
        })
        .is_err()
    );
    for version in [50, 51] {
        let mut h = DeltaHarness::new(2, |v| {
            configure(v);
            v["mechanism_version"] = json!(version);
            v.as_object_mut().unwrap().remove("skill_followup");
        });
        h.active();
        h.action(0, cast("opener")).unwrap();
        h.ticks(2);
        assert!(h.snapshot(0).own.followup.is_none());
        assert_eq!(h.action(0, cast("next")), Err(Error::InvalidState));
        h.close();
    }
}

#[test]
fn followup_public_duplicates_and_reconnect_are_scoped_to_each_member() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    h.action(0, cast("opener")).unwrap();
    h.action(1, cast("opener")).unwrap();
    h.ticks(2);
    let first = h.input(0, cast("next"));
    let result = h.engine.input(first.clone());
    assert!(result.is_ok());
    let latest = h.input(
        0,
        Action::Cast {
            skill_id: "next".into(),
            direction: [0., 1.],
        },
    );
    h.engine.input(latest.clone()).unwrap();
    assert_eq!(h.engine.input(first), result);
    let peer = h.input(1, cast("next"));
    h.engine.input(peer.clone()).unwrap();
    h.pump();
    let own = h
        .snapshot(0)
        .own
        .followup
        .as_ref()
        .unwrap()
        .queued
        .as_ref()
        .unwrap();
    assert_eq!(own.action_id, latest.request_id);
    assert_eq!(own.direction, [0., 1.]);
    assert!(
        !serde_json::to_string(&h.snapshot(1).members)
            .unwrap()
            .contains("followup")
    );
    let stale = h.input(0, cast("next"));
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: stale.connection.clone(),
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "followup-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    reconnect.request_id = "followup-reconnect".into();
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.followup.is_none());
    assert!(
        h.snapshot(1)
            .own
            .followup
            .as_ref()
            .unwrap()
            .queued
            .is_some()
    );
    h.ticks(7);
    assert!(h.events.iter().any(|(recipient,event)| recipient.as_deref() == Some("P01") && matches!(event,
        EventV4::Private { event: Event::ActionCommitted { action_id, .. } } if action_id == &peer.request_id)));
    assert!(!h.events.iter().any(|(recipient,event)| recipient.as_deref() == Some("P00") && matches!(event,
        EventV4::Private { event: Event::ActionCommitted { action_id, .. } } if action_id == &latest.request_id)));
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn followup_replica_rejects_invalid_phase_target_direction_and_deadline_atomically() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    h.action(0, cast("opener")).unwrap();
    h.ticks(2);
    h.action(0, cast("next")).unwrap();
    h.pump();
    let before = h.snapshot(0).clone();
    for mode in 0..5 {
        let mut bad = before.clone();
        let followup = bad.own.followup.as_mut().unwrap();
        let queued = followup.queued.as_mut().unwrap();
        match mode {
            0 => queued.direction = [2., 0.],
            1 => queued.expires_at_ms = followup.promotes_at_ms - 1,
            2 => queued.skill_id = followup.source_skill_id.clone(),
            3 => bad.own.protected = true,
            _ => followup.promotes_at_ms = bad.own.recovery_until_ms - 1,
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-followup-{mode}");
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
