use super::*;

fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(52);
    let mut skill = value["additional_skills"][1].clone();
    skill["id"] = json!("opener");
    skill["windup_ms"] = json!(30);
    skill["recovery_ms"] = json!(300);
    skill["speed"] = json!(60);
    value["skill"] = skill.clone();
    value["skill"]["recovery_cancel"] = json!({"after_ms":90});
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
fn recovery_cancel_configuration_is_strict_in_both_slots_and_keeps_mechanism_51() {
    assert!(delta_catalog(8, configure).is_ok());
    for invalid in [
        Value::Null,
        json!({}),
        json!({"after_ms":-1}),
        json!({"after_ms":31}),
        json!({"after_ms":300}),
        json!({"after_ms":90,"mode":"always"}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["additional_skills"][1]["recovery_cancel"] = invalid;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["additional_skills"][1]["recovery_cancel"] = json!({"after_ms":0});
            v["skill_followup"] = json!({"window_ms":150});
        })
        .is_ok()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["additional_skills"][1]["recovery_cancel"] = json!({"after_ms":270});
        })
        .is_ok()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(51);
        })
        .is_err()
    );
    for field in ["charge", "channel", "recast"] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["skill"][field] = match field {
                    "charge" => {
                        v["skill"]["windup_ms"] = json!(0);
                        json!({"min_ms":90,"max_ms":450,"min_damage_bps":4000})
                    }
                    "channel" => json!({"duration_ms":360,"interval_ms":90,"spirit_per_pulse":3}),
                    _ => json!({"window_ms":360,"spirit_cost":3}),
                };
            })
            .is_err()
        );
    }
    let mut h = DeltaHarness::new(2, |v| {
        configure(v);
        v["mechanism_version"] = json!(51);
        v["skill_followup"] = json!({"window_ms":150});
        v["skill"]
            .as_object_mut()
            .unwrap()
            .remove("recovery_cancel");
    });
    h.active();
    h.action(0, cast("opener")).unwrap();
    h.ticks(7);
    h.action(0, cast("next")).unwrap();
    h.pump();
    assert!(h.snapshot(0).own.recovery_cancel.is_none());
    assert!(
        h.snapshot(0)
            .own
            .followup
            .as_ref()
            .unwrap()
            .queued
            .is_some()
    );
    h.close();
}

#[test]
fn recovery_cancel_public_reconnect_preserves_deadline_and_replacement_is_idempotent() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    h.action(0, cast("opener")).unwrap();
    h.action(1, cast("opener")).unwrap();
    h.ticks(2);
    let view = h.snapshot(0).own.recovery_cancel.clone().unwrap();
    let end = h.snapshot(0).own.recovery_until_ms;
    assert!(
        !serde_json::to_string(&h.snapshot(1).members)
            .unwrap()
            .contains("recovery_cancel")
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
        id: "recovery-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    reconnect.request_id = "recovery-reconnect".into();
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.recovery_cancel.is_none());
    assert_eq!(h.snapshot(0).own.recovery_until_ms, end);
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    let publication_id = h.replicas[0].last_full_id().unwrap().to_owned();
    h.action(0, Action::CombatReady { publication_id }).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.recovery_cancel.as_ref(), Some(&view));
    h.ticks(2);
    let input = h.input(0, cast("next"));
    let result = h.engine.input(input.clone());
    assert!(result.is_ok());
    assert_eq!(h.engine.input(input.clone()), result);
    h.pump();
    assert!(h.snapshot(0).own.recovery_cancel.is_none());
    assert_eq!(h.snapshot(0).own.recovery_until_ms, view.cancel_at_ms);
    assert_eq!(h.snapshot(1).own.recovery_until_ms, end);
    assert_eq!(h.snapshot(1).own.recovery_cancel.as_ref(), Some(&view));
    h.ticks(2);
    let commits = h.events.iter().filter(|(recipient,event)| recipient.as_deref()==Some("P00") && matches!(event,
        EventV4::Private { event: Event::ActionCommitted { action_id, .. } } if action_id == &input.request_id)).count();
    assert_eq!(commits, 1);
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn recovery_cancel_replica_rejects_invalid_clock_skill_and_phase_atomically() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    h.action(0, cast("opener")).unwrap();
    h.ticks(1);
    let before = h.snapshot(0).clone();
    for mode in 0..4 {
        let mut bad = before.clone();
        let view = bad.own.recovery_cancel.as_mut().unwrap();
        match mode {
            0 => view.cancel_at_ms = bad.own.recovery_until_ms,
            1 => view.skill_id = "missing".into(),
            2 => bad.own.protected = true,
            _ => bad.own.extraction_finishes_ms = Some(900),
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-recovery-{mode}");
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
