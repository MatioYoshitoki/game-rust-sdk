use super::*;

pub(super) fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(47);
    value["skill"]["windup_ms"] = json!(0);
    value["skill"]["charge"] = json!({"min_ms":90,"max_ms":450,"min_damage_bps":4000});
}

pub(super) fn start(h: &mut DeltaHarness, member: usize) -> ChargeActionView {
    let skill_id = h.engine.config.skill.id.clone();
    h.action(
        member,
        Action::StartCharge {
            skill_id,
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.pump();
    h.snapshot(member)
        .own
        .pending_action
        .as_ref()
        .unwrap()
        .charge
        .clone()
        .unwrap()
}

fn commits(h: &DeltaHarness, member: &str) -> usize {
    h.events
        .iter()
        .filter(|(pid, event)| {
            pid.as_deref() == Some(member)
                && matches!(
                    event,
                    EventV4::Private {
                        event: Event::ActionCommitted {
                            charge: Some(_),
                            ..
                        }
                    }
                )
        })
        .count()
}

#[test]
fn charge_config_and_legacy_inputs_are_versioned_and_strict() {
    assert!(delta_catalog(8, configure).is_ok());
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            let mut skill = value["skill"].clone();
            skill["id"] = json!("charge-last");
            value["additional_skills"][6] = skill;
        })
        .is_ok()
    );
    for invalid in [
        Value::Null,
        json!({}),
        json!({"min_ms":0,"max_ms":450,"min_damage_bps":4000}),
        json!({"min_ms":90,"max_ms":100,"min_damage_bps":4000}),
        json!({"min_ms":90,"max_ms":5001,"min_damage_bps":4000}),
        json!({"min_ms":90,"max_ms":450,"min_damage_bps":0}),
        json!({"min_ms":90,"max_ms":450,"min_damage_bps":10001}),
        json!({"min_ms":90,"max_ms":450,"min_damage_bps":4000,"aim":"editable"}),
    ] {
        assert!(
            delta_catalog(8, |value| {
                configure(value);
                let mut skill = value["skill"].clone();
                skill["id"] = json!("charge-last");
                skill["charge"] = invalid;
                value["additional_skills"][6] = skill;
            })
            .is_err()
        );
    }
    for path in ["/mechanism_version", "/skill/windup_ms", "/schema_version"] {
        assert!(
            delta_catalog(8, |value| {
                configure(value);
                *value.pointer_mut(path).unwrap() = match path {
                    "/mechanism_version" => json!(46),
                    "/schema_version" => json!(6),
                    _ => json!(1),
                };
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            value["additional_skills"][6]["windup_ms"] = json!(0);
            value["additional_skills"][6]["charge"] = value["skill"]["charge"].clone();
        })
        .is_err()
    );
    assert!(
        serde_json::from_value::<Action>(
            json!({"kind":"release_charge","action_id":"a","direction":[1,0],"cost":0})
        )
        .is_err()
    );
    let mut old = DeltaHarness::new(2, |value| {
        super::hp_cost::configure(value);
        value["mechanism_version"] = json!(46);
    });
    old.active();
    let skill_id = old.engine.config.skill.id.clone();
    assert_eq!(
        old.action(
            0,
            Action::StartCharge {
                skill_id,
                direction: [1., 0.]
            }
        ),
        Err(Error::Unsupported)
    );
    old.close();
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let skill_id = h.engine.config.skill.id.clone();
    assert_eq!(
        h.action(
            0,
            Action::Cast {
                skill_id: skill_id.clone(),
                direction: [1., 0.]
            }
        ),
        Err(Error::Unsupported)
    );
    assert_eq!(
        h.action(
            0,
            Action::StartCharge {
                skill_id,
                direction: [0., 0.]
            }
        ),
        Err(Error::InvalidMessage)
    );
    assert!(h.snapshot(0).own.pending_action.is_none());
    h.close();
}

#[test]
fn charge_public_two_members_duplicate_release_and_private_projection() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let first = start(&mut h, 0);
    let second = start(&mut h, 1);
    assert_ne!(first.action_id, second.action_id);
    assert!(
        !serde_json::to_string(&h.snapshot(0).members)
            .unwrap()
            .contains("charge")
    );
    assert_eq!(
        h.action(
            1,
            Action::ReleaseCharge {
                action_id: first.action_id.clone(),
                direction: None,
            }
        ),
        Err(Error::InvalidTarget)
    );
    let steps = 90_u64.div_ceil(h.engine.config.step_ms);
    h.ticks(steps as usize);
    let input = h.input(
        0,
        Action::ReleaseCharge {
            action_id: first.action_id.clone(),
            direction: None,
        },
    );
    let response = h.engine.input(input.clone());
    assert!(response.is_ok());
    assert_eq!(h.engine.input(input), response);
    h.ticks(1);
    assert_eq!(commits(&h, "P00"), 1);
    assert_eq!(commits(&h, "P01"), 0);
    assert_eq!(
        h.snapshot(1)
            .own
            .pending_action
            .as_ref()
            .unwrap()
            .charge
            .as_ref(),
        Some(&second)
    );
    assert_eq!(
        h.action(
            0,
            Action::ReleaseCharge {
                action_id: first.action_id,
                direction: None,
            }
        ),
        Err(Error::InvalidTarget)
    );
    h.ticks(450_u64.div_ceil(h.engine.config.step_ms) as usize);
    assert_eq!(commits(&h, "P00"), 1);
    assert_eq!(commits(&h, "P01"), 1);
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn charge_reconnect_interrupts_queued_release_and_fences_stale_id() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let old = start(&mut h, 0);
    h.ticks(90_u64.div_ceil(h.engine.config.step_ms) as usize);
    h.action(
        0,
        Action::ReleaseCharge {
            action_id: old.action_id.clone(),
            direction: None,
        },
    )
    .unwrap();
    let previous = h.engine.seats[0].connection.clone();
    let stale = h.input(
        0,
        Action::ReleaseCharge {
            action_id: old.action_id.clone(),
            direction: None,
        },
    );
    let mut input = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous,
            authenticated_principal: "principal-P00".into(),
        },
    );
    input.connection = Connection {
        id: "charge-reconnect".into(),
        generation: 2,
    };
    input.sequence = 1;
    h.engine.input(input).unwrap();
    h.sequences[0] = 1;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.pending_action.is_none());
    h.action(0, Action::Foreground {}).unwrap();
    let publication_id = h.replicas[0].last_full_id().unwrap().to_owned();
    h.action(0, Action::CombatReady { publication_id }).unwrap();
    let next = start(&mut h, 0);
    assert_ne!(old.action_id, next.action_id);
    assert_eq!(
        h.action(
            0,
            Action::ReleaseCharge {
                action_id: old.action_id,
                direction: None,
            }
        ),
        Err(Error::InvalidTarget)
    );
    assert_eq!(commits(&h, "P00"), 0);
    h.close();
}

#[test]
fn charge_replica_rejects_invalid_timing_identity_and_target_atomically() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    start(&mut h, 0);
    let before = h.snapshot(0).clone();
    for mode in 0..5 {
        let mut invalid = before.clone();
        let pending = invalid.own.pending_action.as_mut().unwrap();
        let charge = pending.charge.as_mut().unwrap();
        match mode {
            0 => charge.action_id.clear(),
            1 => charge.started_at_ms = invalid.logical_ms + 1,
            2 => charge.min_release_at_ms = charge.auto_commit_at_ms,
            3 => pending.commits_at_ms = charge.min_release_at_ms,
            _ => pending.ground_position = Some([0., 0.]),
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-charge-{mode}");
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
