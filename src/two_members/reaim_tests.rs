use super::*;

fn configure(value: &mut Value) {
    super::charge::configure(value);
    value["mechanism_version"] = json!(48);
    value["skill"]["charge"]["aim_update"] =
        json!({"min_interval_ms":90,"max_turn_deg_per_sec":720});
}

fn update(charge: &ChargeActionView, direction: [f64; 2]) -> Action {
    Action::UpdateChargeAim {
        action_id: charge.action_id.clone(),
        direction,
    }
}

fn observed(h: &DeltaHarness, member: usize) -> ChargeAimView {
    h.snapshot(member)
        .own
        .pending_action
        .as_ref()
        .unwrap()
        .charge
        .as_ref()
        .unwrap()
        .aim
        .unwrap()
}

#[test]
fn reaim_config_and_optional_final_direction_are_version_gated() {
    assert!(delta_catalog(8, configure).is_ok());
    assert!(delta_catalog(8, super::charge::configure).is_ok());
    for invalid in [
        Value::Null,
        json!({}),
        json!({"min_interval_ms":49,"max_turn_deg_per_sec":720}),
        json!({"min_interval_ms":450,"max_turn_deg_per_sec":720}),
        json!({"min_interval_ms":50,"max_turn_deg_per_sec":0}),
        json!({"min_interval_ms":50,"max_turn_deg_per_sec":721}),
        json!({"min_interval_ms":50,"max_turn_deg_per_sec":720,"snap":true}),
    ] {
        assert!(
            delta_catalog(8, |value| {
                configure(value);
                let mut skill = value["skill"].clone();
                skill["id"] = json!("last-reaim");
                skill["charge"]["aim_update"] = invalid;
                value["additional_skills"][6] = skill;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            let mut skill = value["skill"].clone();
            skill["id"] = json!("last-reaim");
            skill["charge"]["aim_update"]["min_interval_ms"] = json!(420);
            value["additional_skills"][6] = skill;
        })
        .is_ok()
    );
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            value["mechanism_version"] = json!(47);
        })
        .is_err()
    );
    assert!(
        serde_json::from_value::<Action>(json!({
            "kind":"release_charge","action_id":"charge-1","direction":null
        }))
        .is_err()
    );

    let mut old = DeltaHarness::new(2, super::charge::configure);
    old.active();
    let charge = super::charge::start(&mut old, 0);
    assert!(charge.aim.is_none());
    assert_eq!(
        old.action(0, update(&charge, [1., 1.])),
        Err(Error::Unsupported)
    );
    assert_eq!(
        old.action(
            0,
            Action::ReleaseCharge {
                action_id: charge.action_id,
                direction: Some([1., 1.])
            }
        ),
        Err(Error::Unsupported)
    );
    old.close();
}

#[test]
fn reaim_public_update_duplicate_and_peer_identity_stay_isolated() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let actor = super::charge::start(&mut h, 0);
    let peer = super::charge::start(&mut h, 1);
    let baseline = observed(&h, 1);
    h.ticks(90_u64.div_ceil(h.engine.config.step_ms) as usize);
    assert_eq!(
        h.action(1, update(&actor, [1., 1.])),
        Err(Error::InvalidTarget)
    );
    let input = h.input(0, update(&actor, [1., 1.]));
    let status = h.engine.input(input.clone());
    assert!(status.is_ok());
    assert_eq!(h.engine.input(input), status);
    h.pump();
    let accepted = observed(&h, 0);
    assert!(accepted.direction[1] > 0.7);
    assert_eq!(observed(&h, 1), baseline);
    assert!(
        !serde_json::to_string(&h.snapshot(1).members)
            .unwrap()
            .contains(&actor.action_id)
    );
    h.ticks(3);
    h.action(0, update(&actor, [2., 2.])).unwrap();
    h.pump();
    assert_eq!(observed(&h, 0), accepted);
    assert_eq!(
        h.snapshot(1)
            .own
            .pending_action
            .as_ref()
            .unwrap()
            .charge
            .as_ref()
            .unwrap()
            .action_id,
        peer.action_id
    );
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn reaim_reconnect_clears_latest_aim_and_fences_late_update() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let old = super::charge::start(&mut h, 0);
    h.ticks(90_u64.div_ceil(h.engine.config.step_ms) as usize);
    h.action(0, update(&old, [1., 1.])).unwrap();
    let previous = h.engine.seats[0].connection.clone();
    let stale = h.input(0, update(&old, [1., 0.]));
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous,
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "reaim-reconnect".into(),
        generation: 2,
    };
    reconnect.request_id = "reaim-reconnect-request".into();
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert!(h.snapshot(0).own.pending_action.is_none());
    h.action(0, Action::Foreground {}).unwrap();
    let publication_id = h.replicas[0].last_full_id().unwrap().to_owned();
    h.action(0, Action::CombatReady { publication_id }).unwrap();
    let next = super::charge::start(&mut h, 0);
    assert_ne!(next.action_id, old.action_id);
    assert_eq!(observed(&h, 0).direction, [1., 0.]);
    assert_eq!(
        h.action(0, update(&old, [1., 1.])),
        Err(Error::InvalidTarget)
    );
    h.action(0, Action::CancelAction {}).unwrap();
    assert_eq!(
        h.action(0, update(&next, [1., 1.])),
        Err(Error::InvalidTarget)
    );
    h.close();
}

#[test]
fn reaim_bad_direction_and_update_clock_are_atomic_in_rust_replica() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    super::charge::start(&mut h, 0);
    let before = h.snapshot(0).clone();
    for mode in 0..3 {
        let mut invalid = before.clone();
        let charge = invalid
            .own
            .pending_action
            .as_mut()
            .unwrap()
            .charge
            .as_mut()
            .unwrap();
        let aim = charge.aim.as_mut().unwrap();
        match mode {
            0 => aim.direction = [2., 0.],
            1 => aim.updated_at_ms = invalid.logical_ms + 1,
            _ => {
                charge.started_at_ms += 1;
                invalid.logical_ms += 1;
            }
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-reaim-{mode}");
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
