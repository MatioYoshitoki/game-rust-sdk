use super::*;
use crate::world::two_members::mechanism_contract_tests::{catalog, input, world_with_seats};
use serde_json::json;

fn room() -> TwoMembers {
    room_with_seats(&["a", "b"], 2)
}

fn room_with_seats(ids: &[&str], observers: u8) -> TwoMembers {
    let catalog = Arc::new(
        catalog(input(), |v| {
            v["pve"].as_object_mut().unwrap().remove("status_rules");
            v["members"]["spectators"] = json!({"capacity":observers,"terminal_retention_ms":600});
            v["members"]["capacity"] = json!(ids.len());
            v["members"]["spawns"] = json!(
                (0..ids.len())
                    .map(|i| [20, 20 + 12 * i])
                    .collect::<Vec<_>>()
            );
            v["output_capacity"] = json!(1024);
        })
        .unwrap(),
    );
    let c = catalog.default_config();
    let request = CreateV4 {
        protocol_version: 6,
        key: MatchKey {
            id: "room".into(),
            incarnation: 1,
        },
        request_id: "create".into(),
        config_id: c.id.clone(),
        config_hash: c.hash(),
        seed: 1,
        participants: ids
            .iter()
            .copied()
            .map(|id| ParticipantSeat {
                participant_id: id.into(),
                authenticated_principal: id.into(),
                connection: Connection {
                    id: id.into(),
                    generation: 1,
                },
                weapon: None,
                basic_profile: None,
                combat_build: None,
                skills: None,
            })
            .collect(),
        publication_mode: PublicationMode::Interactive,
        recovery: None,
    };
    let mut room = TwoMembers::create(c.as_ref().clone(), catalog, request).unwrap();
    room.phase = Phase::Active;
    room.world = Some(world_with_seats(&c, ids));
    room
}

fn connection(id: &str, generation: u32) -> Connection {
    Connection {
        id: id.into(),
        generation,
    }
}

fn control(
    room: &mut TwoMembers,
    slot: u8,
    connection: &Connection,
    action: SpectatorActionV6,
) -> Result<Status> {
    room.spectate(SpectateV6 {
        key: room.create.key.clone(),
        slot,
        connection: connection.clone(),
        action,
    })
}

fn attach(
    room: &mut TwoMembers,
    slot: u8,
    connection: &Connection,
    principal: &str,
) -> Result<Status> {
    control(
        room,
        slot,
        connection,
        SpectatorActionV6::Attach {
            authenticated_principal: principal.into(),
        },
    )
}

fn frames(room: &mut TwoMembers) -> Vec<SpectatorEnvelopeV6> {
    room.drain()
        .iter()
        .filter_map(SpectatorEnvelopeV6::from_output)
        .collect()
}

fn receipt(
    room: &mut TwoMembers,
    frame: &SpectatorEnvelopeV6,
    outcome: TransportOutcome,
) -> Result<Status> {
    control(
        room,
        frame.slot,
        &frame.connection,
        SpectatorActionV6::Transport {
            publication_id: frame.publication_id.clone(),
            outcome,
        },
    )
}

#[test]
fn spectator_receipts_are_isolated_and_slot_reuse_fences_the_previous_connection() {
    let mut room = room();
    let a = connection("viewer-a", 1);
    let b = connection("viewer-b", 1);
    attach(&mut room, 1, &a, "x").unwrap();
    attach(&mut room, 2, &b, "y").unwrap();
    assert_eq!(attach(&mut room, 3, &b, "y"), Err(Error::InvalidMessage));
    let first = frames(&mut room);
    assert_eq!(first.len(), 2);
    assert_eq!(
        control(
            &mut room,
            2,
            &b,
            SpectatorActionV6::Transport {
                publication_id: first[0].publication_id.clone(),
                outcome: TransportOutcome::Sent,
            }
        ),
        Err(Error::StalePublication)
    );
    receipt(&mut room, &first[0], TransportOutcome::Sent).unwrap();
    receipt(&mut room, &first[0], TransportOutcome::Sent).unwrap();
    receipt(&mut room, &first[1], TransportOutcome::Failed).unwrap();
    room.now = 30;
    room.publish_spectators_due(0);
    let next = frames(&mut room);
    assert_eq!(next.len(), 2);
    let left = next.iter().find(|f| f.slot == 1).unwrap();
    let right = next.iter().find(|f| f.slot == 2).unwrap();
    assert!(
        matches!(&left.payload, SpectatorPayloadV6::Delta { base_publication_id, .. } if base_publication_id == &first[0].publication_id)
    );
    assert!(matches!(
        right.payload,
        SpectatorPayloadV6::Full {
            reason: FullReason::Failure,
            ..
        }
    ));
    let b2 = connection("viewer-b2", 2);
    assert_eq!(
        attach(&mut room, 2, &b2, "intruder"),
        Err(Error::Unauthorized)
    );
    attach(&mut room, 2, &b2, "y").unwrap();
    assert_eq!(
        receipt(&mut room, right, TransportOutcome::Sent),
        Err(Error::StaleConnection)
    );
    let reconnected = frames(&mut room).remove(0);
    assert!(matches!(
        reconnected.payload,
        SpectatorPayloadV6::Full {
            reason: FullReason::Lifecycle,
            ..
        }
    ));
    control(&mut room, 2, &b2, SpectatorActionV6::Detach {}).unwrap();
    assert_eq!(attach(&mut room, 2, &b2, "y"), Err(Error::StaleConnection));
    attach(&mut room, 2, &connection("replacement", 3), "another").unwrap();
    assert_eq!(
        receipt(&mut room, &reconnected, TransportOutcome::Sent),
        Err(Error::StaleConnection)
    );
    assert_eq!(room.status().phase, Phase::Active);
}

#[test]
fn a_new_spectator_can_read_frozen_results_until_public_retention_expires() {
    let mut room = room();
    let actor = room.world.as_ref().unwrap().members[0]
        .reference
        .entity
        .clone();
    for amount in 1..=40 {
        room.events(vec![(
            None,
            EventV4::Damage {
                source: actor.clone(),
                target: actor.clone(),
                amount,
                hp: 0,
            },
        )]);
    }
    room.events(vec![(
        Some(0),
        EventV4::Private {
            event: Event::ExtractionStarted,
        },
    )]);
    room.phase = Phase::Closed;
    room.closed_at_io = Some(0);
    room.finish_spectators();
    room.world = None;
    room.now = 300;
    room.io_elapsed = 250;
    let a = connection("late-viewer", 1);
    attach(&mut room, 1, &a, "a").unwrap();
    let frame = frames(&mut room).remove(0);
    let SpectatorPayloadV6::Full { snapshot, .. } = &frame.payload else {
        panic!()
    };
    assert_eq!(
        (snapshot.phase, snapshot.logical_ms, snapshot.events.len()),
        (Phase::Closed, 0, 32)
    );
    assert!(matches!(
        snapshot.events[0].event,
        SpectatorEventV6::Damage { amount: 9, .. }
    ));
    receipt(&mut room, &frame, TransportOutcome::Sent).unwrap();
    room.io_elapsed = 600;
    room.poll_spectators();
    assert!(room.spectator_final.is_none());
    assert_eq!(
        attach(&mut room, 2, &connection("expired", 1), "late"),
        Err(Error::Closed)
    );
    assert!(frames(&mut room).is_empty());
}

#[test]
fn spectator_configuration_rejects_unbounded_slots_and_too_short_terminal_retention() {
    for (capacity, ttl) in [(0, 600), (9, 600), (1, 199)] {
        let err = catalog(input(), |v| {
            v["pve"].as_object_mut().unwrap().remove("status_rules");
            v["members"]["spectators"] = json!({"capacity":capacity,"terminal_retention_ms":ttl});
        })
        .err()
        .unwrap();
        assert!(err.to_string().contains("CONFIG_SPECTATORS"));
    }
}

#[test]
fn eight_spectators_keep_independent_windows_alongside_eight_participants() {
    let mut room = room_with_seats(&["a", "b", "c", "d", "e", "f", "g", "h"], 8);
    frames(&mut room);
    let sequence_before_spectators = room.output_sequence;
    for slot in 1..=8 {
        let viewer = format!("viewer-{slot}");
        attach(&mut room, slot, &connection(&viewer, 1), &viewer).unwrap();
    }
    let first = frames(&mut room);
    assert_eq!(first.len(), 8);
    assert_eq!(room.output_sequence, sequence_before_spectators);
    for frame in &first {
        assert!(
            matches!(&frame.payload, SpectatorPayloadV6::Full { snapshot, .. } if snapshot.members.len() == 8)
        );
        if frame.slot <= 4 {
            receipt(&mut room, frame, TransportOutcome::Sent).unwrap();
        }
    }
    room.now = 30;
    room.publish_spectators_due(0);
    let flowing = frames(&mut room);
    assert_eq!(flowing.len(), 4);
    for frame in &flowing {
        assert!(frame.slot <= 4 && matches!(frame.payload, SpectatorPayloadV6::Delta { .. }));
        receipt(&mut room, frame, TransportOutcome::Sent).unwrap();
    }
    room.io_elapsed = 100;
    room.poll_spectators();
    let retry = frames(&mut room);
    assert_eq!(retry.len(), 4);
    assert!(retry.iter().all(|f| f.slot > 4
        && matches!(
            f.payload,
            SpectatorPayloadV6::Full {
                reason: FullReason::Timeout,
                ..
            }
        )));
    assert_eq!(room.status().phase, Phase::Active);
    assert!(room.world.as_ref().unwrap().members.iter().all(|m| m.ready));
}
