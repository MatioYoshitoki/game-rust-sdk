use super::*;

const FAN: &str = "fan_four_winds";
const GUARD: &str = "shield_cross_wall";
const DASH: &str = "shield_breaker_wall";

fn install(v: &mut Value) {
    let kit: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/basic-charge-profiles.json"
    ))
    .unwrap();
    v["combat_rules"]["basic_profiles"] = kit["basic_profiles"].clone();
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .extend(
            kit["status_definitions"]
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
    v["pve"]["combat"]["aggro_range"] = json!(10);
}

fn host(profiles: [Option<&str>; 2], adjust: impl FnOnce(&mut Value)) -> DeltaHarness {
    profile_host(profiles, |v| {
        install(v);
        adjust(v);
    })
}

fn until(h: &mut DeltaHarness, f: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..300 {
        if f(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("expected charge state was not published");
}

fn ready(h: &mut DeltaHarness, i: usize) {
    until(h, |h| {
        h.snapshot(i).logical_ms >= h.snapshot(i).own.basic_ready_at_ms
            && h.snapshot(i).own.pending_action.is_none()
    });
}

fn start(h: &mut DeltaHarness, i: usize) -> BasicChargeView {
    h.action(i, Action::StartBasicCharge {}).unwrap();
    h.pump();
    h.snapshot(i).own.basic_charge.clone().unwrap()
}

fn target_release(h: &DeltaHarness, i: usize, session: &BasicChargeView, target: usize) -> Action {
    let e = &h.snapshot(i).monsters[target].entity;
    Action::ReleaseBasicCharge {
        session_id: session.session_id.clone(),
        position: None,
        target: Some(UnitTarget {
            id: e.id.clone(),
            generation: e.generation,
        }),
        direction: None,
    }
}

fn directional_release(session: &BasicChargeView, direction: [f64; 2]) -> Action {
    Action::ReleaseBasicCharge {
        session_id: session.session_id.clone(),
        position: None,
        target: None,
        direction: Some(direction),
    }
}

#[test]
fn basic_charge_freezes_wave_stage_and_resumes_latest_move_after_terminal_input() {
    let mut h = host([Some(FAN), Some(GUARD)], |v| {
        v["pve"]["monster_positions"] = json!([[650, 400], [250, 400]]);
    });
    let initial = start(&mut h, 0);
    let other = start(&mut h, 1);
    assert_ne!(initial.session_id, other.session_id);
    assert_eq!(h.action(0, Action::Attack {}), Err(Error::InvalidState));
    h.action(
        0,
        Action::Move {
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(21);
    assert_eq!(h.snapshot(0).logical_ms, 630);
    assert_eq!(h.snapshot(0).own.position, [400., 400.]);
    h.action(0, target_release(&h, 0, &initial, 0)).unwrap();
    h.ticks(1);
    assert!(h.snapshot(0).own.position[0] > 400.);
    h.action(0, Action::Stop {}).unwrap();
    until(&mut h, |h| {
        h.snapshot(0)
            .basic_waves
            .as_ref()
            .is_some_and(|waves| !waves.is_empty())
    });
    let wave = &h.snapshot(0).basic_waves.as_ref().unwrap()[0];
    assert_eq!(
        (&*wave.plan_id, wave.range, wave.pending.len()),
        ("fan-charged-wave-1", 300., 1)
    );
    ready(&mut h, 0);

    let second = start(&mut h, 0);
    h.ticks(22);
    assert_eq!(h.snapshot(0).own.basic_charge.as_ref().unwrap().tier, 2);
    assert_eq!(
        h.action(0, target_release(&h, 0, &initial, 0)),
        Err(Error::InvalidTarget)
    );
    h.pump();
    assert_eq!(
        h.snapshot(0).own.basic_charge.as_ref().unwrap().session_id,
        second.session_id
    );
    assert_eq!(
        h.action(0, target_release(&h, 0, &second, 0)),
        Err(Error::InvalidTarget)
    );
    h.pump();
    assert!(h.snapshot(0).own.basic_charge.is_none());

    let third = start(&mut h, 0);
    h.ticks(45);
    h.action(0, target_release(&h, 0, &third, 0)).unwrap();
    until(&mut h, |h| {
        h.snapshot(0)
            .basic_waves
            .as_ref()
            .is_some_and(|w| w.iter().any(|w| w.plan_id == "fan-charged-wave-3"))
    });
    let wave = &h.snapshot(0).basic_waves.as_ref().unwrap()[0];
    assert_eq!(
        (wave.range, wave.pending.len(), wave.geometry.clone()),
        (310., 2, BasicWaveGeometry::Radial {})
    );
    until(&mut h, |h| h.snapshot(0).monsters[0].position[0] > 700.);
    assert_eq!(h.snapshot(0).monsters[0].position, [740., 400.]);
    assert_eq!(h.snapshot(0).monsters[1].position, [160., 400.]);
    assert!(h.snapshot(1).own.basic_charge.is_none());
    h.close();
}

#[test]
fn basic_charge_guard_uses_post_shield_loss_and_disconnect_clears_only_that_member() {
    let mut h = host([Some(GUARD), Some(FAN)], |v| {
        v["pve"]["monster_positions"] = json!([[455, 400]])
    });
    let session = start(&mut h, 0);
    h.action(0, target_release(&h, 0, &session, 0)).unwrap();
    until(&mut h, |h| h.snapshot(0).own.shield_total == Some(70));
    assert!(
        h.snapshot(0)
            .own
            .statuses
            .iter()
            .any(|s| s.definition_id == "charged-guard-shield"
                && s.expires_at_ms - s.applied_at_ms == 1800)
    );
    ready(&mut h, 0);
    start(&mut h, 0);
    let other = start(&mut h, 1);
    let now = h.snapshot(0).logical_ms;
    let events = h
        .engine
        .world
        .as_mut()
        .unwrap()
        .charge_test_hit(0, 0, &h.engine.config, now);
    let damage: Vec<_> = events
        .iter()
        .filter_map(|(_, e)| match e {
            EventV4::Damage { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(damage, [30, 18]);
    h.action(0, Action::Background {}).unwrap();
    h.pump();
    assert!(h.snapshot(0).own.basic_charge.is_none());
    assert_eq!(
        h.snapshot(1).own.basic_charge.as_ref().unwrap().session_id,
        other.session_id
    );
    h.close();
}

#[test]
fn basic_charge_dash_clips_actual_path_and_preserves_normal_attack_while_waiting() {
    let mut h = host([None, Some(DASH)], |v| {
        v["pve"]["monster_positions"] = json!([[490, 490], [530, 510], [700, 480]]);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["walkable"] = json!(false);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let index = 12 * 32 + 16;
        v["pve"]["cells_rle"] = json!([[0, index], [1, 1], [0, 1024 - index - 1]]);
    });
    let first = start(&mut h, 1);
    h.action(1, directional_release(&first, [2., 0.])).unwrap();
    until(&mut h, |h| h.snapshot(1).own.position[0] == 580.);
    assert_eq!(
        h.snapshot(1)
            .monsters
            .iter()
            .map(|n| n.hp)
            .collect::<Vec<_>>(),
        [9910, 9928, 10000]
    );
    assert_eq!(h.snapshot(1).monsters[0].position, [580., 490.]);
    assert!(
        h.snapshot(1).monsters[0]
            .statuses
            .as_ref()
            .unwrap()
            .iter()
            .any(|s| s.definition_id == "charged-dash-stun"
                && s.expires_at_ms - s.applied_at_ms == 900)
    );
    let dash_ready = h.snapshot(1).own.basic_charge_ready_at_ms.unwrap();
    assert_eq!(dash_ready, h.snapshot(1).logical_ms + 6000);
    ready(&mut h, 1);
    assert_eq!(
        h.action(1, Action::StartBasicCharge {}),
        Err(Error::InvalidState)
    );
    h.action(1, attack(&h, 1, 0)).unwrap();
    until(&mut h, |h| h.snapshot(1).monsters[0].hp < 9910);
    assert_eq!(h.snapshot(1).own.basic_charge_ready_at_ms, Some(dash_ready));

    until(&mut h, |h| h.snapshot(1).logical_ms >= dash_ready);
    let second = start(&mut h, 1);
    h.ticks(102);
    h.action(1, directional_release(&second, [1., 0.])).unwrap();
    until(&mut h, |h| h.snapshot(1).own.position[0] > 600.);
    let x = h.snapshot(1).own.position[0];
    assert!((x - (640. - h.engine.config.params.player_radius)).abs() < 1e-5);
    assert_eq!(h.snapshot(1).monsters[2].hp, 10000);
    h.close();
}

#[test]
fn basic_charge_loader_rejects_duplicate_thresholds_and_wrong_control() {
    assert!(profile_catalog(install).is_ok());
    assert!(profile_catalog(|v| {
        install(v);
        v["combat_rules"]["basic_profiles"][FAN]["charge"]["behavior"]["upgrades"][1]["after_ms"] = json!(650);
    }).is_err());
    assert!(
        profile_catalog(|v| {
            install(v);
            v["combat_rules"]["basic_profiles"][DASH]["charge"]["behavior"]["stun_status"] =
                json!("charged-wave-slow");
        })
        .is_err()
    );
}

#[test]
fn basic_charge_pursuit_keeps_tier_two_after_crossing_the_tier_three_threshold() {
    let mut h = host([Some(FAN), None], |v| {
        v["pve"]["monster_positions"] = json!([[645, 400]]);
        v["basic_pursuit"] = json!({"max_range":340,"duration_ms":900});
    });
    let session = start(&mut h, 0);
    h.ticks(44);
    assert_eq!(h.snapshot(0).own.basic_charge.as_ref().unwrap().tier, 2);
    h.action(0, target_release(&h, 0, &session, 0)).unwrap();
    h.pump();
    assert!(h.snapshot(0).own.basic_pursuit.is_some());
    until(&mut h, |h| {
        h.snapshot(0)
            .basic_waves
            .as_ref()
            .is_some_and(|waves| !waves.is_empty())
    });
    let wave = &h.snapshot(0).basic_waves.as_ref().unwrap()[0];
    assert_eq!((&*wave.plan_id, wave.range), ("fan-charged-wave-2", 220.));
    assert!(wave.released_at_ms > session.started_at_ms + 1350);
    h.close();
}

#[test]
fn basic_charge_elite_takes_radial_slow_and_dash_push_without_stun() {
    let mut h = host([Some(FAN), Some(DASH)], |v| {
        v["pve"]["monster_positions"] = json!([[480, 440]]);
        v["combat_rules"]["npc_profiles"]["burn-source"] = json!("elite");
    });
    let session = start(&mut h, 0);
    h.ticks(45);
    h.action(0, target_release(&h, 0, &session, 0)).unwrap();
    until(&mut h, |h| {
        h.snapshot(0).monsters[0]
            .statuses
            .as_ref()
            .unwrap()
            .iter()
            .any(|s| {
                s.definition_id == "charged-wave-slow"
                    && s.modifier_bps == -3500
                    && s.expires_at_ms - s.applied_at_ms == 1500
            })
    });
    assert_eq!(h.snapshot(0).monsters[0].position, [480., 440.]);
    let dash = start(&mut h, 1);
    h.action(1, directional_release(&dash, [1., 0.])).unwrap();
    until(&mut h, |h| h.snapshot(1).own.position[0] == 580.);
    assert_eq!(h.snapshot(1).monsters[0].position, [570., 440.]);
    assert!(
        h.snapshot(1).monsters[0]
            .statuses
            .as_ref()
            .unwrap()
            .iter()
            .all(|s| s.definition_id != "charged-dash-stun")
    );
    h.close();
}
