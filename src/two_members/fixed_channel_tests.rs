use super::*;

fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(49);
    let mut skill = value["additional_skills"][1].clone();
    skill["id"] = json!("fixed-channel");
    skill["windup_ms"] = json!(60);
    skill["recovery_ms"] = json!(120);
    skill["channel"] = json!({"duration_ms":360,"interval_ms":90,"spirit_per_pulse":3});
    value["skill"] = skill;
}

#[test]
fn channel_config_is_strict_and_preserves_mechanism_48_charge() {
    assert!(delta_catalog(8, configure).is_ok());
    for invalid in [
        Value::Null,
        json!({}),
        json!({"duration_ms":360,"interval_ms":0,"spirit_per_pulse":3}),
        json!({"duration_ms":361,"interval_ms":90,"spirit_per_pulse":3}),
        json!({"duration_ms":360,"interval_ms":91,"spirit_per_pulse":3}),
        json!({"duration_ms":90,"interval_ms":90,"spirit_per_pulse":3}),
        json!({"duration_ms":30030,"interval_ms":30,"spirit_per_pulse":3}),
        json!({"duration_ms":3900,"interval_ms":30,"spirit_per_pulse":3}),
        json!({"duration_ms":360,"interval_ms":90,"spirit_per_pulse":1000001}),
        json!({"duration_ms":360,"interval_ms":90,"spirit_per_pulse":3,"active":"moving"}),
    ] {
        assert!(
            delta_catalog(8, |value| {
                configure(value);
                let mut skill = value["skill"].clone();
                skill["id"] = json!("last-channel");
                skill["channel"] = invalid;
                value["additional_skills"][6] = skill;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            let mut skill = value["skill"].clone();
            skill["id"] = json!("last-channel");
            skill["channel"] = json!({"duration_ms":3870,"interval_ms":30,"spirit_per_pulse":0});
            value["additional_skills"][6] = skill;
        })
        .is_ok()
    );
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            value["mechanism_version"] = json!(48);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            value["additional_skills"][6]["channel"] = value["skill"]["channel"].clone();
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            value["skill"]["windup_ms"] = json!(0);
            value["skill"]["charge"] = json!({"min_ms":90,"max_ms":450,"min_damage_bps":4000});
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |value| {
            super::charge::configure(value);
            value["mechanism_version"] = json!(48);
            value["skill"]["charge"]["aim_update"] =
                json!({"min_interval_ms":90,"max_turn_deg_per_sec":720});
        })
        .is_ok()
    );
}

fn cast(h: &DeltaHarness) -> Action {
    Action::Cast {
        skill_id: h.engine.config.skill.id.clone(),
        direction: [1., 0.],
    }
}

fn pulses(h: &DeltaHarness, member: &str) -> usize {
    h.events
        .iter()
        .filter(|(pid, event)| {
            pid.as_deref() == Some(member)
                && matches!(
                    event,
                    EventV4::Private {
                        event: Event::ChannelPulse { .. }
                    }
                )
        })
        .count()
}

#[test]
fn channel_public_duplicate_commit_reconnect_and_peer_are_isolated() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let action = cast(&h);
    let first = h.input(0, action.clone());
    let result = h.engine.input(first.clone());
    assert!(result.is_ok());
    assert_eq!(h.engine.input(first), result);
    h.action(1, action).unwrap();
    h.ticks(5);
    assert_eq!((pulses(&h, "P00"), pulses(&h, "P01")), (1, 1));
    let own = h.snapshot(0).own.active_channel.clone().unwrap();
    let peer = h.snapshot(1).own.active_channel.clone().unwrap();
    assert_ne!(own.action_id, peer.action_id);
    assert!(
        !serde_json::to_string(&h.snapshot(1).members)
            .unwrap()
            .contains(&own.action_id)
    );
    let previous = h.engine.seats[0].connection.clone();
    let stale = h.input(0, Action::CancelAction {});
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous,
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "channel-reconnect".into(),
        generation: 2,
    };
    reconnect.request_id = "channel-reconnect-request".into();
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.active_channel.is_none());
    assert_eq!(h.snapshot(0).own.recovery_until_ms, h.engine.now + 120);
    assert!(h.events.iter().any(|(pid,event)|pid.as_deref()==Some("P00") &&
        matches!(event,EventV4::Private{event:Event::ChannelEnded{action_id,reason:ChannelEndReason::Interrupted}}
            if action_id==&own.action_id)));
    assert_eq!(
        h.snapshot(1).own.active_channel.as_ref().unwrap().action_id,
        peer.action_id
    );
    h.ticks(3);
    assert_eq!((pulses(&h, "P00"), pulses(&h, "P01")), (1, 2));
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn channel_invalid_private_projection_is_rejected_atomically() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let action = cast(&h);
    h.action(0, action).unwrap();
    h.ticks(2);
    let before = h.snapshot(0).clone();
    let mut missing = serde_json::to_value(before.own.active_channel.as_ref().unwrap()).unwrap();
    missing.as_object_mut().unwrap().remove("next_pulse_at_ms");
    assert!(serde_json::from_value::<ActiveChannelView>(missing).is_err());
    for mode in 0..5 {
        let mut invalid = before.clone();
        let channel = invalid.own.active_channel.as_mut().unwrap();
        match mode {
            0 => channel.direction = [2., 0.],
            1 => channel.next_pulse_at_ms = Some(invalid.logical_ms),
            2 => channel.pulses = 129,
            3 => channel.skill_id = "missing".into(),
            _ => invalid.own.protected = true,
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-channel-{mode}");
        envelope.payload = PublicationPayloadV6::Full {
            snapshot: Box::new(invalid),
            reason: FullReason::Resync,
        };
        assert_eq!(
            h.replicas[0].apply(&envelope),
            Err(DecodeError::InvalidFrame)
        );
        assert_eq!(h.snapshot(0), &before);
    }
    h.close();
}
