use super::*;

fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(50);
    let mut skill = value["additional_skills"][1].clone();
    skill["id"] = json!("redirect");
    skill["windup_ms"] = json!(30);
    skill["speed"] = json!(60);
    skill["recast"] = json!({"window_ms":600,"spirit_cost":7});
    value["skill"] = skill;
}

#[test]
fn recast_config_gates_mechanism_shapes_and_bounds_in_all_slots() {
    assert!(delta_catalog(8, configure).is_ok());
    for definition in [
        Value::Null,
        json!({}),
        json!({"window_ms":0,"spirit_cost":0}),
        json!({"window_ms":61,"spirit_cost":0}),
        json!({"window_ms":30030,"spirit_cost":0}),
        json!({"window_ms":600,"spirit_cost":1000001}),
        json!({"window_ms":600,"spirit_cost":0,"max":2}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                let mut skill = v["skill"].clone();
                skill["id"] = json!("eighth");
                skill["recast"] = definition;
                v["additional_skills"][6] = skill;
            })
            .is_err()
        );
    }
    for field in ["charge", "channel"] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                if field == "charge" {
                    v["skill"]["windup_ms"] = json!(0);
                    v["skill"][field] = json!({"min_ms":90,"max_ms":450,"min_damage_bps":4000});
                } else {
                    v["skill"][field] =
                        json!({"duration_ms":360,"interval_ms":90,"spirit_per_pulse":3});
                }
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["skill"]["recast"]["window_ms"] = json!(6000);
            v["params"]["player_projectile_ttl_ms"] = json!(3000);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(49);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["additional_skills"][6]["recast"] = v["skill"]["recast"].clone();
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            let mut skill = v["skill"].clone();
            skill["id"] = json!("eighth");
            skill["recast"] = json!({"window_ms":30,"spirit_cost":0});
            v["additional_skills"][6] = skill;
        })
        .is_ok()
    );
}

fn start(h: &mut DeltaHarness) -> ProjectileRecastView {
    h.active();
    h.action(
        0,
        Action::Cast {
            skill_id: "redirect".into(),
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(1);
    h.snapshot(0).own.projectile_recast.clone().unwrap()
}

#[test]
fn recast_public_uses_current_connection_and_is_idempotent_and_private() {
    let mut h = DeltaHarness::new(2, configure);
    let recast = start(&mut h);
    let action = Action::RecastProjectile {
        action_id: recast.action_id.clone(),
        recast_index: 1,
        direction: [0., 1.],
    };
    assert!(h.snapshot(1).own.projectile_recast.is_none());
    assert_eq!(h.action(1, action.clone()), Err(Error::InvalidTarget));
    let stale = h.input(0, action.clone());
    let previous = stale.connection.clone();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous,
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "redirect-current".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    reconnect.request_id = "redirect-reconnect".into();
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    h.pump();
    assert_eq!(h.snapshot(0).own.projectile_recast.as_ref(), Some(&recast));
    assert_eq!(h.action(0, action.clone()), Err(Error::NotReady));
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    let publication_id = h.replicas[0].last_full_id().unwrap().to_owned();
    h.action(0, Action::CombatReady { publication_id }).unwrap();
    h.pump();
    let spirit = h.snapshot(0).own.spirit;
    let input = h.input(0, action.clone());
    let result = h.engine.input(input.clone());
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(h.engine.input(input), result);
    assert_eq!(h.action(0, action), Err(Error::InvalidTarget));
    h.pump();
    assert_eq!(h.snapshot(0).own.spirit, spirit - 7);
    assert!(h.snapshot(0).own.projectile_recast.is_none());
    let events: Vec<_> = h
        .events
        .iter()
        .filter(|(_, event)| {
            matches!(
                event,
                EventV4::Private {
                    event: Event::ProjectileRecast { .. }
                }
            )
        })
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0.as_deref(), Some("P00"));
    h.close();
    assert_eq!(h.engine.pending(), 0);
}

#[test]
fn recast_decoder_rejects_cross_reference_index_and_deadline_atomically() {
    let mut h = DeltaHarness::new(2, configure);
    start(&mut h);
    let before = h.snapshot(0).clone();
    for mode in 0..5 {
        let mut bad = before.clone();
        let recast = bad.own.projectile_recast.as_mut().unwrap();
        match mode {
            0 => recast.next_index = 2,
            1 => recast.expires_at_ms = bad.logical_ms,
            2 => recast.skill_id = "missing".into(),
            3 => recast.projectile_id = "other".into(),
            _ => bad.projectiles[0].source = h.snapshot(1).own.participant.entity.clone(),
        }
        let mut envelope = h.latest(0);
        envelope.revision += 100;
        envelope.publication_id = format!("bad-recast-{mode}");
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
