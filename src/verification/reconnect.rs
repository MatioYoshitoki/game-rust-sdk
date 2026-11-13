use super::*;

fn view(h: &Harness) -> Snapshot {
    h.engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
}

fn command(h: &mut Harness, connection: &Connection, action: Action) -> Input {
    h.sequence += 1;
    Input {
        key: create_request().key,
        participant_id: "A".into(),
        entity_generation: 1,
        connection: connection.clone(),
        request_id: format!("link-{}", h.sequence),
        sequence: h.sequence,
        action,
    }
}

fn replacement(h: &mut Harness, previous: &Connection) -> Input {
    command(
        h,
        &Connection {
            id: format!("conn-{}", previous.generation + 1),
            generation: previous.generation + 1,
        },
        Action::Reconnect {
            previous_connection: previous.clone(),
            authenticated_principal: "principal-A".into(),
        },
    )
}

fn full(h: &mut Harness) -> Output {
    h.engine
        .drain()
        .into_iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap()
}

fn sent(h: &mut Harness, output: &Output, connection: &Connection) -> Result<Status> {
    h.engine.transport(TransportReceipt {
        key: create_request().key,
        output_id: output.id.clone(),
        recipient_id: "A".into(),
        connection: connection.clone(),
        outcome: TransportOutcome::Sent,
    })
}

fn ready(h: &mut Harness, connection: &Connection) {
    let foreground = command(h, connection, Action::Foreground {});
    h.engine.input(foreground).unwrap();
    let output = full(h);
    sent(h, &output, connection).unwrap();
    let input = command(
        h,
        connection,
        Action::CombatReady {
            publication_id: output.id,
        },
    );
    h.engine.input(input).unwrap();
}

#[test]
fn state_cooldowns_assets_and_create_identity_survive_rebind() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    let before = view(&h);
    let input = replacement(&mut h, &create_request().connection);
    h.engine.input(input.clone()).unwrap();
    let after = view(&h);
    assert_eq!(after.position, before.position);
    assert_eq!((after.hp, after.spirit), (before.hp, before.spirit));
    assert_eq!(
        (after.inventory_quantity, after.inventory_revision),
        (7, before.inventory_revision)
    );
    assert_eq!(
        (after.target, after.resources),
        (before.target, before.resources)
    );
    assert_eq!(
        (
            after.skill_ready_at_ms,
            after.basic_ready_at_ms,
            after.active_at_ms
        ),
        (
            before.skill_ready_at_ms,
            before.basic_ready_at_ms,
            before.active_at_ms
        )
    );
    assert!(after.protected);
    h.engine.input(input.clone()).unwrap();
    assert_eq!(
        h.engine
            .drain()
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::ConnectionChanged { .. }
                }
            ))
            .count(),
        1
    );
    h.engine.create(create_request()).unwrap();
    ready(&mut h, &input.connection);
    assert!(!view(&h).protected);
    let cast = command(
        &mut h,
        &input.connection,
        Action::Cast {
            skill_id: "duanjin-shou".into(),
            direction: [1.0, 0.0],
        },
    );
    assert_eq!(h.engine.input(cast), Err(Error::InvalidState));
    assert_eq!((h.store.admission_commits, h.store.warehouse), (1, 95));
}

#[test]
fn old_connections_and_old_fulls_cannot_control_the_replacement() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Foreground {}).unwrap();
    let old = full(&mut h);
    let input = replacement(&mut h, &create_request().connection);
    h.engine.input(input.clone()).unwrap();
    assert_eq!(
        sent(&mut h, &old, &create_request().connection),
        Err(Error::StaleConnection)
    );
    let stale = command(
        &mut h,
        &create_request().connection,
        Action::Move {
            direction: [1.0, 0.0],
        },
    );
    assert_eq!(h.engine.input(stale), Err(Error::StaleConnection));
    let stale_ready = command(
        &mut h,
        &input.connection,
        Action::CombatReady {
            publication_id: old.id,
        },
    );
    assert_eq!(h.engine.input(stale_ready), Err(Error::NotReady));
    let current = full(&mut h);
    sent(&mut h, &current, &input.connection).unwrap();
    let no_foreground = command(
        &mut h,
        &input.connection,
        Action::CombatReady {
            publication_id: current.id,
        },
    );
    assert_eq!(h.engine.input(no_foreground), Err(Error::NotReady));
    ready(&mut h, &input.connection);
    assert!(!view(&h).protected);
}

#[test]
fn invalid_rebinds_and_competing_successors_have_no_partial_effects() {
    let mut h = Harness::new();
    h.start().unwrap();
    let valid = replacement(&mut h, &create_request().connection);
    let mut wrong = valid.clone();
    let Action::Reconnect {
        authenticated_principal,
        ..
    } = &mut wrong.action
    else {
        unreachable!()
    };
    *authenticated_principal = "other-owner".into();
    assert_eq!(h.engine.input(wrong), Err(Error::Unauthorized));
    let mut skipped = valid.clone();
    skipped.connection.generation = 4;
    assert_eq!(h.engine.input(skipped), Err(Error::InvalidMessage));
    assert!(!view(&h).protected);
    h.engine.input(valid.clone()).unwrap();
    let mut competing = valid.clone();
    competing.request_id = "competitor".into();
    competing.sequence += 1;
    competing.connection.id = "other-successor".into();
    assert_eq!(h.engine.input(competing), Err(Error::StaleConnection));
    let next = replacement(&mut h, &valid.connection);
    h.engine.input(next).unwrap();
    assert_eq!(h.engine.input(valid), Err(Error::StaleConnection));
}

#[test]
fn disconnect_cancels_uncommitted_actions_and_clears_movement() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::Move {
        direction: [0.0, -1.0],
    })
    .unwrap();
    h.advance(90).unwrap();
    let position = view(&h).position;
    h.action(Action::Cast {
        skill_id: "duanjin-shou".into(),
        direction: [1.0, 0.0],
    })
    .unwrap();
    h.action(Action::ConnectionLost {}).unwrap();
    h.advance(300).unwrap();
    assert_eq!(view(&h).position, position);
    assert_eq!(view(&h).spirit, 100);
    assert_eq!(h.action(Action::Foreground {}), Err(Error::NotReady));
    let input = replacement(&mut h, &create_request().connection);
    h.engine.input(input.clone()).unwrap();
    ready(&mut h, &input.connection);
    for _ in 0..10 {
        h.engine.tick();
        h.engine.drain();
    }
    assert_eq!(view(&h).position, position);
    assert_eq!(view(&h).skill_ready_at_ms, 0);
}

#[test]
fn disconnect_and_rebind_interrupt_extraction_without_settlement() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.fight_and_pickup().unwrap();
    h.advance(180000).unwrap();
    h.action(Action::ExtractStart {}).unwrap();
    h.advance(4980).unwrap();
    let active_at = view(&h).active_at_ms;
    h.action(Action::ConnectionLost {}).unwrap();
    let input = replacement(&mut h, &create_request().connection);
    h.engine.input(input.clone()).unwrap();
    ready(&mut h, &input.connection);
    assert_eq!(view(&h).extraction_finishes_ms, None);
    assert_eq!(view(&h).active_at_ms, active_at);
    assert_eq!(h.store.settlement_commits, 0);
}

#[test]
fn repeated_disconnect_does_not_extend_the_grace_and_expiry_closes_once() {
    let mut h = Harness::new();
    h.start().unwrap();
    h.action(Action::ConnectionLost {}).unwrap();
    h.advance(15000).unwrap();
    h.action(Action::ConnectionLost {}).unwrap();
    h.advance(14970).unwrap();
    assert_eq!(h.engine.status().phase, Phase::Active);
    h.advance(30).unwrap();
    assert_eq!(
        (
            h.engine.status().phase,
            h.engine.live_worlds(),
            h.store.settlement_commits,
            h.store.warehouse
        ),
        (Phase::Closed, 0, 1, 98)
    );
    h.engine
        .close(Close {
            key: create_request().key,
            request_id: "again".into(),
        })
        .unwrap();
    assert_eq!(h.store.settlement_commits, 1);
    let input = replacement(&mut h, &create_request().connection);
    assert_eq!(h.engine.input(input), Err(Error::Closed));
}

#[test]
fn replacement_without_a_ready_handshake_expires_and_refunds_unactivated_admission() {
    let mut h = Harness::new();
    h.engine.create(create_request()).unwrap();
    h.pump().unwrap();
    let input = replacement(&mut h, &create_request().connection);
    h.engine.input(input).unwrap();
    h.advance(30000).unwrap();
    assert_eq!(
        (
            h.engine.status().phase,
            h.engine.live_worlds(),
            h.store.warehouse,
            h.store.years
        ),
        (Phase::Closed, 0, 100, 0)
    );
    assert_eq!(h.store.settlement_commits, 0);
}

#[test]
fn confirmed_ready_removes_the_reconnect_expiry() {
    let mut h = Harness::new();
    h.start().unwrap();
    let input = replacement(&mut h, &create_request().connection);
    h.engine.input(input.clone()).unwrap();
    ready(&mut h, &input.connection);
    for _ in 0..1001 {
        h.engine.tick();
        h.engine.drain();
    }
    assert_eq!(h.engine.status().phase, Phase::Active);
}
