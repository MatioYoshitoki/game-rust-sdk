use super::*;

const FAN: &str = "fan_wide_gale";
const GATE: &str = "tower_guard_domain";

fn strikes(h: &DeltaHarness, i: usize) -> Vec<u32> {
    let participant = &h.snapshot(i).own.participant.entity;
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::Damage { source, amount, .. } if source == participant => Some(*amount),
            _ => None,
        })
        .collect()
}

fn ready(h: &mut DeltaHarness, i: usize) {
    while h.snapshot(i).logical_ms < h.snapshot(i).own.basic_ready_at_ms {
        h.ticks(1);
    }
}

#[test]
fn basic_wave_controls_push_stops_at_walk_wall_and_uses_independent_source_windows() {
    let mut h = profile_host([Some(FAN), Some(FAN)], |v| {
        v["pve"]["monster_positions"] = json!([[460, 400]]);
        v["pve"]["combat"]["return_distance"] = json!(500);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["walkable"] = json!(false);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let index = 10 * 32 + 14;
        v["pve"]["cells_rle"] = json!([[0, index], [1, 1], [0, 1024 - index - 1]]);
    });
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.ticks(5);
    let plan = &h.snapshot(0).basic_waves.as_ref().unwrap()[0];
    assert_eq!((&*plan.plan_id, plan.range, plan.speed), (FAN, 280., 500.));
    assert_eq!(
        plan.geometry,
        BasicWaveGeometry::Directional {
            direction: [1., 0.],
            cone_degrees: 116.,
        }
    );
    h.ticks(4);
    assert_eq!(strikes(&h, 0), [70]);
    let position = h.snapshot(0).monsters[0].position;
    assert!((position[0] - 546.).abs() < 1e-5);
    assert_eq!(position[1], 400.);
    assert_eq!(h.snapshot(1).monsters[0].position, position);
    assert_eq!(h.snapshot(0).own.spirit, 102);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1070);

    ready(&mut h, 0);
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(0, [460., 400.]);
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.ticks(9);
    assert_eq!(strikes(&h, 0), [70, 70]);
    assert_eq!(h.snapshot(0).monsters[0].position, [460., 400.]);

    // B can move the same target while A's previous push is still on cooldown.
    h.action(1, attack(&h, 1, 0)).unwrap();
    h.ticks(12);
    assert_eq!(strikes(&h, 1), [70]);
    assert_eq!(h.snapshot(0).monsters[0].position, [544., 288.]);

    ready(&mut h, 0);
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(0, [460., 400.]);
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.ticks(9);
    assert_eq!(strikes(&h, 0), [70, 70, 70]);
    assert_eq!(h.snapshot(0).monsters[0].position, position);
}

#[test]
fn basic_wave_controls_resistant_npcs_refresh_slow_and_keep_position() {
    for profile in ["elite", "boss"] {
        let mut h = profile_host([Some(FAN), Some(FAN)], |v| {
            v["pve"]["monster_positions"] = json!([[460, 400]]);
            v["combat_rules"]["default_npc_profile"] = json!(profile);
        });
        h.action(0, attack(&h, 0, 0)).unwrap();
        h.action(1, attack(&h, 1, 0)).unwrap();
        h.ticks(12);
        assert_eq!(h.snapshot(0).monsters[0].position, [460., 400.]);
        let effect = h.snapshot(0).monsters[0].statuses.as_ref().unwrap()[0].clone();
        for (i, participant) in ["A", "B"].into_iter().enumerate() {
            let statuses = h.snapshot(i).monsters[0].statuses.as_ref().unwrap();
            assert_eq!(statuses.len(), 1);
            let slow = &statuses[0];
            assert_eq!(
                (&*slow.definition_id, &*slow.source.id),
                ("wide-gale-slow", participant)
            );
            assert_eq!(slow.modifier_bps, -3000);
            assert_eq!(slow.expires_at_ms - slow.applied_at_ms, 1250);
        }
        ready(&mut h, 0);
        h.action(0, attack(&h, 0, 0)).unwrap();
        h.ticks(9);
        let statuses = h.snapshot(0).monsters[0].statuses.as_ref().unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].instance_id, effect.instance_id);
        assert!(statuses[0].expires_at_ms > effect.expires_at_ms);
    }
}

#[test]
fn basic_wave_controls_gate_clips_pull_at_combined_radii_and_alternates_at_release() {
    let mut h = profile_host([Some(GATE), None], |v| {
        v["pve"]["monster_positions"] = json!([[450, 400], [610, 400]]);
        v["pve"]["combat"]["return_distance"] = json!(500);
    });
    assert_eq!(
        h.snapshot(0).own.basic_wave_phase,
        Some(BasicWavePhase::Primary)
    );
    assert_eq!(h.snapshot(1).own.basic_wave_phase, None);
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.ticks(5);
    assert_eq!(
        h.snapshot(0).own.basic_wave_phase,
        Some(BasicWavePhase::Primary)
    );
    h.ticks(1);
    assert_eq!(
        h.snapshot(0).own.basic_wave_phase,
        Some(BasicWavePhase::Alternate)
    );
    let plan = &h.snapshot(0).basic_waves.as_ref().unwrap()[0];
    assert_eq!(
        (&*plan.plan_id, plan.range, plan.speed),
        ("tower_gate_open", 230., 720.)
    );
    assert!(matches!(
        plan.geometry,
        BasicWaveGeometry::Directional {
            cone_degrees: 90.,
            ..
        }
    ));
    assert!(own_status(&h, 0, "tower-reduction").is_none());
    h.ticks(3);
    assert_eq!(h.snapshot(0).monsters[0].position, [430., 400.]);
    h.ticks(7);
    assert_eq!(h.snapshot(0).monsters[1].position, [540., 400.]);
    assert_eq!(strikes(&h, 0), [50, 44]);
    assert_eq!(h.snapshot(0).own.spirit, 102);

    ready(&mut h, 0);
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(1, [610., 400.]);
    assert_eq!(h.action(0, attack(&h, 0, 1)), Err(Error::InvalidTarget));
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.action(0, Action::CancelAction {}).unwrap();
    h.ticks(6);
    assert_eq!(
        h.snapshot(0).own.basic_wave_phase,
        Some(BasicWavePhase::Alternate)
    );
    assert!(h.snapshot(0).basic_waves.as_ref().unwrap().is_empty());

    h.action(0, Action::Attack {}).unwrap();
    h.ticks(6);
    let plan = &h.snapshot(0).basic_waves.as_ref().unwrap()[0];
    assert_eq!(
        (&*plan.plan_id, plan.range, plan.speed),
        ("tower_gate_close", 190., 1200.)
    );
    assert_eq!(plan.geometry, BasicWaveGeometry::Radial {});
    assert_eq!(
        h.snapshot(0).own.basic_wave_phase,
        Some(BasicWavePhase::Primary)
    );
    h.ticks(1);
    assert_eq!(strikes(&h, 0), [50, 44, 87]);
    assert_eq!(h.snapshot(0).own.spirit, 104);
}

#[test]
fn basic_wave_controls_gate_marks_are_private_to_source_and_survive_in_match_reconnect() {
    let mut h = profile_host([Some(GATE), Some(GATE)], |v| {
        v["pve"]["monster_positions"] = json!([[490, 400]]);
        v["combat_rules"]["default_npc_profile"] = json!("elite");
    });
    for i in 0..2 {
        h.action(i, attack(&h, i, 0)).unwrap();
    }
    h.ticks(6);
    let connection = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut input = h.input(
        0,
        Action::Reconnect {
            previous_connection: connection,
            authenticated_principal: "principal-A".into(),
        },
    );
    input.connection = Connection {
        id: "gate-reconnected".into(),
        generation: 2,
    };
    input.request_id = "gate-reconnected".into();
    input.sequence = 1;
    h.engine.input(input).unwrap();
    h.sequences[0] = 1;
    h.pump();
    assert!(matches!(
        h.latest(0).payload,
        PublicationPayloadV6::Full { .. }
    ));
    assert_eq!(
        h.snapshot(0).own.basic_wave_phase,
        Some(BasicWavePhase::Alternate)
    );
    assert_eq!(h.snapshot(0).basic_waves.as_ref().unwrap().len(), 2);
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    h.action(
        0,
        Action::CombatReady {
            publication_id: h.replicas[0].last_full_id().unwrap().into(),
        },
    )
    .unwrap();
    h.ticks(6);
    assert_eq!(h.snapshot(1).monsters[0].position, [490., 400.]);
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert_eq!(h.snapshot(1).own.spirit, 102);
    for i in [1, 0] {
        ready(&mut h, i);
        h.action(i, attack(&h, i, 0)).unwrap();
        h.ticks(10);
        assert_eq!(strikes(&h, i), [50, 87]);
    }
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.basic_wave_phase, None);
    assert_eq!(
        h.snapshot(1).own.basic_wave_phase,
        Some(BasicWavePhase::Primary)
    );
    h.close();
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}

#[test]
fn basic_wave_controls_gate_consumes_each_mark_only_once() {
    let mut h = profile_host([Some(GATE), None], |v| {
        v["pve"]["monster_positions"] = json!([[490, 400], [310, 400]]);
        v["combat_rules"]["default_npc_profile"] = json!("elite");
    });
    for target in [0, 0, 1, 0] {
        ready(&mut h, 0);
        h.action(0, attack(&h, 0, target)).unwrap();
        h.ticks(11);
    }
    assert_eq!(strikes(&h, 0), [50, 87, 65, 50, 72, 79]);
}

#[test]
fn basic_wave_controls_gate_ignores_expired_and_replaced_target_marks() {
    for replace in [false, true] {
        let mut h = profile_host([Some(GATE), None], |v| {
            v["pve"]["monster_positions"] = json!([[490, 400]]);
            v["combat_rules"]["default_npc_profile"] = json!("boss");
        });
        h.action(0, attack(&h, 0, 0)).unwrap();
        h.ticks(11);
        ready(&mut h, 0);
        if replace {
            h.engine.world.as_mut().unwrap().replace_wave_test_npc(0);
            h.ticks(1);
        } else {
            h.ticks(150);
        }
        h.action(0, attack(&h, 0, 0)).unwrap();
        h.ticks(9);
        assert_eq!(strikes(&h, 0), [50, 72]);
    }
}

#[test]
fn basic_wave_controls_reject_malformed_and_incompatible_profiles() {
    for (profile, field, value) in [
        (FAN, "distance", json!(0)),
        (FAN, "rehit_immunity_ms", json!(60001)),
        (FAN, "reduced_status", json!("tower-attack")),
        (FAN, "resistant_profiles", json!(["missing"])),
        (FAN, "resistant_profiles", json!(["elite", "elite"])),
        (GATE, "marked_damage_bonus_bps", json!(10001)),
        (GATE, "mark_duration_ms", json!(0)),
        (GATE, "alternate", json!(null)),
    ] {
        assert!(
            profile_catalog(|v| {
                v["combat_rules"]["basic_profiles"][profile]["wave"]["behavior"][field] = value;
            })
            .is_err(),
            "{profile} {field}"
        );
    }
    assert!(
        profile_catalog(|v| {
            v["combat_rules"]["basic_profiles"][GATE]["wave"]["behavior"]["alternate"]["plan_id"] =
                json!("tower_gate_open");
        })
        .is_err()
    );
    assert!(
        profile_catalog(|v| {
            v["combat_rules"]["basic_profiles"][GATE]["wave"]["geometry"] =
                json!({"kind":"radial"});
        })
        .is_err()
    );
}
