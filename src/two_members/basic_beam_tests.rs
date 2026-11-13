use super::*;

const BEAM: &str = "mirror_world_beam";

fn start(h: &mut DeltaHarness, i: usize, direction: [f64; 2]) -> String {
    h.action(i, Action::StartAttackBeam { direction }).unwrap();
    h.pump();
    h.snapshot(i)
        .basic_beams
        .as_ref()
        .unwrap()
        .iter()
        .find(|beam| beam.source == h.snapshot(i).own.participant.entity)
        .unwrap()
        .action_id
        .clone()
}

fn beam_contacts(h: &DeltaHarness) -> Vec<(&str, u32, u64)> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::BasicBeamContact {
                target,
                tick,
                at_ms,
                ..
            } => Some((target.id.as_str(), *tick, *at_ms)),
            _ => None,
        })
        .collect()
}

fn terminal(h: &DeltaHarness) -> Vec<(BasicBeamEndReason, u32)> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::BasicBeamEnded { reason, ticks, .. } => Some((*reason, *ticks)),
            _ => None,
        })
        .collect()
}

#[test]
fn basic_beam_open_session_repeats_turns_locks_movement_and_settles_once() {
    let mut h = profile_host([Some(BEAM), None], |v| {
        v["pve"]["monster_positions"] = json!([[490, 400], [400, 520]]);
        v["combat_rules"]["basic_attack"]["life_steal_bps"] = json!(5000);
    });
    h.engine.world.as_mut().unwrap().members[0].hp = 500;
    let ids = h
        .snapshot(0)
        .monsters
        .iter()
        .map(|npc| npc.entity.id.clone())
        .collect::<Vec<_>>();
    let id = start(&mut h, 0, [5., 0.]);
    let started = &h.snapshot(0).basic_beams.as_ref().unwrap()[0];
    assert_eq!(
        (
            started.direction,
            started.interval_ms,
            started.next_tick_at_ms
        ),
        ([1., 0.], 150, 150)
    );
    assert_eq!(started.action_id, id);
    h.action(
        0,
        Action::Move {
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(10);
    assert_eq!(
        beam_contacts(&h),
        [(ids[0].as_str(), 1, 150), (ids[0].as_str(), 2, 300)]
    );
    assert_eq!(h.snapshot(0).own.position, [400., 400.]);
    h.action(
        0,
        Action::AimAttackBeam {
            action_id: id.clone(),
            direction: [0., 12.],
        },
    )
    .unwrap();
    h.action(
        0,
        Action::Move {
            direction: [0., -1.],
        },
    )
    .unwrap();
    assert_eq!(h.action(0, Action::Attack {}), Err(Error::InvalidState));
    assert_eq!(
        h.action(
            0,
            Action::CastSelf {
                skill_id: "tag-debuff".into()
            }
        ),
        Err(Error::InvalidState)
    );
    assert_eq!(
        h.action(0, Action::ExtractStart {}),
        Err(Error::InvalidState)
    );
    h.ticks(100);
    assert_eq!(h.snapshot(0).own.position, [400., 400.]);
    assert_eq!(h.snapshot(1).basic_beams.as_ref().unwrap()[0].ticks, 22);
    assert_eq!(h.snapshot(0).own.hp, 500);
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert!(terminal(&h).is_empty());
    assert_eq!(beam_contacts(&h).len(), 22);
    assert!(
        beam_contacts(&h)[2..]
            .iter()
            .enumerate()
            .all(|(i, (target, tick, at))| *target == ids[1]
                && *tick == i as u32 + 3
                && *at == u64::from(*tick) * 150)
    );
    h.action(0, Action::EndAttackBeam { action_id: id })
        .unwrap();
    h.ticks(1);
    assert_eq!(terminal(&h), [(BasicBeamEndReason::Released, 22)]);
    assert_eq!(beam_contacts(&h).len(), 22);
    assert_eq!(h.snapshot(0).own.hp, 698);
    assert_eq!(h.snapshot(0).own.spirit, 102);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 4230);
    assert!(h.snapshot(0).own.position[1] < 400.);
    assert!(h.snapshot(0).basic_beams.as_ref().unwrap().is_empty());
    assert_eq!(
        h.snapshot(0)
            .monsters
            .iter()
            .map(|npc| npc.hp)
            .collect::<Vec<_>>(),
        [9964, 9640]
    );
    assert_eq!(
        h.events
            .iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::HealResolved {
                        heal_kind: HealKind::LifeSteal,
                        ..
                    }
                }
            ))
            .count(),
        1
    );
}

#[test]
fn basic_beam_quick_release_keeps_last_direction_and_fences_old_ids() {
    let mut h = profile_host([Some(BEAM), Some(BEAM)], |v| {
        v["pve"]["monster_positions"] = json!([[490, 400], [400, 520]]);
    });
    h.action(
        0,
        Action::Move {
            direction: [1., 0.],
        },
    )
    .unwrap();
    let id = start(&mut h, 0, [1., 0.]);
    assert_eq!(
        h.action(
            1,
            Action::EndAttackBeam {
                action_id: id.clone()
            }
        ),
        Err(Error::InvalidTarget)
    );
    assert_eq!(
        h.action(
            0,
            Action::AimAttackBeam {
                action_id: id.clone(),
                direction: [0., 0.]
            }
        ),
        Err(Error::InvalidMessage)
    );
    h.action(
        0,
        Action::AimAttackBeam {
            action_id: id.clone(),
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.action(0, Action::Stop {}).unwrap();
    h.action(
        0,
        Action::EndAttackBeam {
            action_id: id.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        h.action(
            0,
            Action::AimAttackBeam {
                action_id: id.clone(),
                direction: [1., 0.]
            }
        ),
        Err(Error::InvalidTarget)
    );
    h.ticks(4);
    assert!(h.snapshot(0).basic_beams.as_ref().unwrap()[0].released);
    assert!(beam_contacts(&h).is_empty());
    h.ticks(1);
    assert_eq!(terminal(&h), [(BasicBeamEndReason::Released, 1)]);
    assert_eq!(h.snapshot(0).monsters[0].hp, 10000);
    assert_eq!(h.snapshot(0).monsters[1].hp, 9982);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1050);
    h.ticks(30);
    assert_eq!(h.snapshot(0).own.position, [400., 400.]);
    let next = start(&mut h, 0, [1., 0.]);
    assert_ne!(id, next);
    assert_eq!(
        h.action(0, Action::EndAttackBeam { action_id: id }),
        Err(Error::InvalidTarget)
    );
    h.ticks(5);
    assert_eq!(
        h.snapshot(0).basic_beams.as_ref().unwrap()[0].action_id,
        next
    );
    assert_eq!(beam_contacts(&h).len(), 2);
}

#[test]
fn basic_beam_uses_circle_rectangle_geometry_and_exact_projectile_sight() {
    let mut h = profile_host([Some(BEAM), None], |v| {
        v["pve"]["monster_positions"] = json!([
            [386, 400],
            [385, 400],
            [500, 435],
            [845, 430],
            [851, 432],
            [520, 369]
        ]);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let index = 9 * 32 + 12;
        v["pve"]["cells_rle"] = json!([[0, index], [1, 1], [0, 1024 - index - 1]]);
    });
    start(&mut h, 0, [1., 0.]);
    h.ticks(5);
    let ids = h
        .snapshot(0)
        .monsters
        .iter()
        .map(|npc| npc.entity.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        beam_contacts(&h),
        [(ids[0], 1, 150), (ids[2], 1, 150), (ids[3], 1, 150)]
    );
    assert_eq!(
        h.snapshot(0)
            .monsters
            .iter()
            .map(|npc| npc.hp)
            .collect::<Vec<_>>(),
        [9982, 10000, 9982, 9982, 10000, 10000]
    );
}

#[test]
fn basic_beam_target_budget_is_per_tick_and_sorted_by_projection() {
    let mut h = profile_host([Some(BEAM), None], |v| {
        v["pve"]["monster_positions"] = json!([[550, 400], [500, 405], [500, 390], [490, 400]]);
        v["combat_rules"]["basic_profiles"][BEAM]["beam"]["target_cap"] = json!(2);
    });
    start(&mut h, 0, [1., 0.]);
    h.ticks(10);
    let ids = h
        .snapshot(0)
        .monsters
        .iter()
        .map(|npc| npc.entity.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        beam_contacts(&h),
        [
            (ids[3], 1, 150),
            (ids[1], 1, 150),
            (ids[3], 2, 300),
            (ids[1], 2, 300)
        ]
    );
}

#[test]
fn basic_beam_protection_hides_session_before_tick_and_revokes_rewards() {
    let mut h = profile_host([Some(BEAM), None], |v| {
        v["combat_rules"]["basic_attack"]["life_steal_bps"] = json!(5000);
    });
    h.engine.world.as_mut().unwrap().members[0].hp = 500;
    let id = start(&mut h, 0, [1., 0.]);
    h.ticks(5);
    let contacts = beam_contacts(&h).len();
    assert!(contacts > 0);
    let previous_connection = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut input = h.input(
        0,
        Action::Reconnect {
            previous_connection,
            authenticated_principal: "principal-A".into(),
        },
    );
    input.connection = Connection {
        id: "beam-reconnect".into(),
        generation: 2,
    };
    input.sequence = 1;
    h.engine.input(input).unwrap();
    h.sequences[0] = 1;
    h.pump();
    assert!(matches!(
        h.latest(0).payload,
        PublicationPayloadV6::Full { .. }
    ));
    assert!(h.snapshot(0).basic_beams.as_ref().unwrap().is_empty());
    assert!(terminal(&h).is_empty());
    h.ticks(1);
    assert_eq!(terminal(&h), [(BasicBeamEndReason::Protected, 1)]);
    h.ticks(8);
    assert_eq!(beam_contacts(&h).len(), contacts);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1080);
    assert_eq!(h.snapshot(0).own.hp, 500);
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert!(h.snapshot(1).basic_beams.as_ref().unwrap().is_empty());
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    h.action(
        0,
        Action::CombatReady {
            publication_id: h.replicas[0].last_full_id().unwrap().into(),
        },
    )
    .unwrap();
    assert_eq!(
        h.action(0, Action::EndAttackBeam { action_id: id }),
        Err(Error::InvalidTarget)
    );
}

#[test]
fn basic_beam_stun_and_departure_stop_the_next_pulse() {
    for stun in [true, false] {
        let mut h = profile_host([Some(BEAM), None], |v| {
            v["pve"]["monster_positions"] = json!([[500, 400]]);
            if stun {
                v["pve"]["combat"]["aggro_range"] = json!(120);
                v["pve"]["combat"]["cast_chance"] = json!(1);
                v["pve"]["combat"]["projectile"]["windup_ms"] = json!(30);
                v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!(["beam-stop"]);
                v["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({
                        "mechanism":"timed_control","attribute":"stun","id":"beam-stop",
                        "duration_ms":900,"duplicate_policy":"refresh","source_scoped":true
                    }));
            }
        });
        start(&mut h, 0, [1., 0.]);
        if !stun {
            h.action(0, Action::Leave {}).unwrap();
        }
        h.ticks(8);
        assert_eq!(
            terminal(&h),
            [(
                if stun {
                    BasicBeamEndReason::Interrupted
                } else {
                    BasicBeamEndReason::SourceInvalid
                },
                0
            )]
        );
        assert!(beam_contacts(&h).is_empty());
        assert!(h.snapshot(1).basic_beams.as_ref().unwrap().is_empty());
    }
}

#[test]
fn basic_beam_cold_validation_rejects_conflicts_and_non_aligned_intervals() {
    for change in [
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][BEAM]["beam"]["interval_ms"] = json!(151)
        },
        |v: &mut Value| v["combat_rules"]["basic_profiles"][BEAM]["beam"]["target_cap"] = json!(9),
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][BEAM]["beam"]["max_duration_ms"] = json!(900)
        },
        |v: &mut Value| v["combat_rules"]["basic_profiles"][BEAM]["beam"]["half_width"] = json!(0),
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][BEAM]["impact"] = json!({
                "plan_id":"conflict","damage_bps":1000,"timing":{"kind":"hitscan"}
            })
        },
    ] {
        assert!(profile_catalog(change).is_err());
    }
    let mut h = profile_host([Some(BEAM), Some("mirror")], |_| {});
    assert_eq!(
        h.action(
            1,
            Action::StartAttackBeam {
                direction: [1., 0.]
            }
        ),
        Err(Error::Unsupported)
    );
    assert_eq!(h.action(0, Action::Attack {}), Err(Error::Unsupported));
    assert_eq!(
        h.action(
            0,
            Action::StartAttackBeam {
                direction: [f64::NAN, 0.]
            }
        ),
        Err(Error::InvalidMessage)
    );
    assert!(
        serde_json::from_value::<Action>(
            json!({"kind":"end_attack_beam","action_id":"beam","direction":[1,0]})
        )
        .is_err()
    );
}
