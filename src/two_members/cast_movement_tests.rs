use super::*;

pub(super) fn configure(value: &mut Value) {
    super::hp_cost::configure(value);
    value["mechanism_version"] = json!(46);
    value["skill"]["windup_ms"] = json!(600);
    value["skill"]["recovery_ms"] = json!(600);
    value["skill"]["cast_movement"] = json!({
        "preparation": {"kind": "locked"},
        "recovery": {"kind": "locked"}
    });
}

#[test]
fn cast_movement_config_validates_all_slots_and_preserves_mechanism_45() {
    assert!(delta_catalog(8, configure).is_ok());
    assert!(delta_catalog(8, super::hp_cost::configure).is_ok());
    for invalid in [
        Value::Null,
        json!({}),
        json!({"preparation":{"kind":"locked"}}),
        json!({"preparation":{"kind":"free"},"recovery":null}),
        json!({"preparation":{"kind":"unknown"},"recovery":{"kind":"free"}}),
        json!({"preparation":{"kind":"locked","speed_bps":1},"recovery":{"kind":"free"}}),
        json!({"preparation":{"kind":"slow","speed_bps":0},"recovery":{"kind":"free"}}),
        json!({"preparation":{"kind":"slow","speed_bps":10000},"recovery":{"kind":"free"}}),
        json!({"preparation":{"kind":"slow","speed_bps":1.5},"recovery":{"kind":"free"}}),
        json!({"preparation":{"kind":"free"},"recovery":{"kind":"free"},"active":{"kind":"free"}}),
    ] {
        assert!(
            delta_catalog(8, |value| {
                configure(value);
                value["additional_skills"][6]["cast_movement"] = invalid;
            })
            .is_err()
        );
    }
    for invalid_field in ["windup_ms", "recovery_ms"] {
        assert!(
            delta_catalog(8, |value| {
                configure(value);
                value["skill"][invalid_field] = json!(0);
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            value["mechanism_version"] = json!(45);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |value| {
            configure(value);
            value["additional_skills"][6]["cast_movement"] = json!({
                "preparation":{"kind":"slow","speed_bps":1},
                "recovery":{"kind":"slow","speed_bps":9999}
            });
        })
        .is_ok()
    );
}

#[test]
fn cast_movement_reconnect_clears_intent_and_preserves_only_committed_recovery() {
    for committed in [false, true] {
        let mut h = DeltaHarness::new(2, configure);
        h.active();
        let start = h.snapshot(0).own.position;
        let step = h.engine.config.step_ms;
        let skill_id = h.engine.config.skill.id.clone();
        let movement = Action::Move {
            direction: [1., 0.],
        };
        h.action(
            0,
            Action::Cast {
                skill_id,
                direction: [1., 0.],
            },
        )
        .unwrap();
        h.action(0, movement.clone()).unwrap();
        h.ticks(if committed {
            600_u64.div_ceil(step) as usize
        } else {
            2
        });
        assert_eq!(h.snapshot(0).own.position, start);
        let before = h.snapshot(0).own.clone();
        let previous = h.engine.seats[0].connection.clone();
        let stale = h.input(0, movement.clone());
        let mut reconnect = h.input(
            0,
            Action::Reconnect {
                previous_connection: previous,
                authenticated_principal: "principal-P00".into(),
            },
        );
        reconnect.connection = Connection {
            id: "movement-reconnect".into(),
            generation: 2,
        };
        reconnect.sequence = 1;
        h.engine.input(reconnect).unwrap();
        h.sequences[0] = 1;
        assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
        h.pump();
        assert!(h.snapshot(0).own.pending_action.is_none());
        assert_eq!(h.snapshot(0).own.hp, before.hp);
        assert_eq!(
            h.snapshot(0).own.recovery_until_ms,
            before.recovery_until_ms
        );
        assert_eq!(h.snapshot(0).own.player_skills, before.player_skills);
        h.action(0, Action::Foreground {}).unwrap();
        let publication_id = h.replicas[0].last_full_id().unwrap().to_owned();
        h.action(0, Action::CombatReady { publication_id }).unwrap();
        h.ticks(1);
        assert_eq!(h.snapshot(0).own.position, start);
        h.action(0, movement.clone()).unwrap();
        h.ticks(1);
        assert_eq!(h.snapshot(0).own.position == start, committed);
        if committed {
            h.ticks(600_u64.div_ceil(step) as usize);
            assert!(h.snapshot(0).own.position[0] > start[0]);
        }
        let commits = h
            .events
            .iter()
            .filter(|(id, event)| {
                id.as_deref() == Some("P00")
                    && matches!(
                        event,
                        EventV4::Private {
                            event: Event::ActionCommitted {
                                hp_cost: Some(3),
                                ..
                            }
                        }
                    )
            })
            .count();
        assert_eq!(commits, usize::from(committed));
        let peer = h.snapshot(1).own.position;
        h.action(0, Action::Leave {}).unwrap();
        h.action(1, movement).unwrap();
        h.ticks(4);
        assert!(h.snapshot(1).own.position[0] > peer[0]);
        h.close();
        assert_eq!(h.engine.pending(), 0);
    }
}
