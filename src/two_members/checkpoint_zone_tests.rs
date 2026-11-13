use super::*;

fn configure(v: &mut Value) {
    *v = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/mastery-stages.json"
    ))
    .unwrap();
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/checkpoint-zones.json"
    ))
    .unwrap();
    v["members"]["checkpoint_schedules"] = sample["checkpoint_schedules"].clone();
    v["pve"]["monster_positions"] = json!([[1050, 1050]]);
    v["pve"]["combat"]["cast_chance"] = json!(0);
}

fn host(change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let mut h = DeltaHarness::new(2, |v| {
        configure(v);
        change(v);
    });
    h.active();
    h
}

fn go_to(h: &mut DeltaHarness, since_active: u64) {
    let time = h.snapshot(0).active_at_ms.unwrap() + since_active;
    let n = time
        .saturating_sub(h.engine.now)
        .div_ceil(h.engine.config.step_ms);
    h.ticks(n as usize);
}

fn place(h: &mut DeltaHarness, i: usize, position: [f64; 2]) {
    h.engine.world.as_mut().unwrap().members[i].position = position;
}

fn progress(h: &DeltaHarness, i: usize) -> &CheckpointProgressView {
    &h.snapshot(i).own.checkpoint_progress.as_ref().unwrap()[0]
}

fn windows(h: &DeltaHarness, i: usize) -> &[CheckpointZoneView] {
    h.snapshot(i).checkpoint_zones.as_deref().unwrap()
}

fn reached(h: &DeltaHarness) -> Vec<(&str, u32, u64)> {
    h.events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event: Event::CheckpointReached { progress, at_ms },
            } => Some((
                recipient.as_deref().expect("checkpoint recipient"),
                progress.last_window_index,
                *at_ms,
            )),
            _ => None,
        })
        .collect()
}

fn compact_schedule(v: &mut Value) {
    v["members"]["checkpoint_schedules"][0] = json!({
        "id":"checkpoint","first_delay_ms":90,"interval_ms":120,"window_ms":60,
        "radius":32,"required":2,"centers":[[400,400],[400,400]]
    });
}

#[test]
fn checkpoint_zones_wind_eye_schedule_counts_three_once_and_keeps_seats_independent() {
    let mut h = host(|_| {});
    let active = h.snapshot(0).active_at_ms.unwrap();
    assert!(windows(&h, 0).is_empty());
    assert_eq!(
        (progress(&h, 0).required, progress(&h, 0).total_windows),
        (3, 3)
    );
    for (index, center) in [[650., 400.], [400., 650.], [150., 400.]]
        .into_iter()
        .enumerate()
    {
        place(&mut h, 0, center);
        if index == 1 {
            place(&mut h, 1, center);
        } else {
            place(&mut h, 1, [400., 480.]);
        }
        let opens = 8000 * (index as u64 + 1);
        go_to(&mut h, opens);
        assert_eq!(windows(&h, 0), windows(&h, 1));
        let z = &windows(&h, 0)[0];
        assert_eq!(
            (z.window_index, z.center, z.radius),
            (index as u32 + 1, center, 150.)
        );
        assert_eq!(
            (z.opens_at_ms, z.expires_at_ms),
            (active + opens, active + opens + 3000)
        );
        assert_eq!(progress(&h, 0).collected, index as u32 + 1);
        place(&mut h, 0, [400., 400.]);
        h.ticks(1);
        place(&mut h, 0, center);
        h.ticks(1);
        assert_eq!(progress(&h, 0).collected, index as u32 + 1);
        go_to(&mut h, opens + 3000);
        assert!(windows(&h, 0).is_empty());
    }
    assert_eq!(
        (progress(&h, 1).collected, progress(&h, 1).last_window_index),
        (1, 2)
    );
    assert_eq!(
        reached(&h)
            .iter()
            .map(|&(recipient, index, _)| (recipient, index))
            .collect::<Vec<_>>(),
        vec![("P00", 1), ("P00", 2), ("P01", 2), ("P00", 3)]
    );
}

#[test]
fn checkpoint_zones_are_open_inclusive_and_expiry_exclusive() {
    let mut h = host(compact_schedule);
    go_to(&mut h, 60);
    assert_eq!(progress(&h, 0).collected, 0);
    go_to(&mut h, 90);
    let first = windows(&h, 0)[0].clone();
    assert_eq!(first.opens_at_ms, h.snapshot(0).logical_ms);
    assert_eq!(reached(&h), vec![("P00", 1, first.opens_at_ms)]);
    go_to(&mut h, 120);
    place(&mut h, 1, [400., 400.]);
    go_to(&mut h, 150);
    assert_eq!(h.snapshot(1).logical_ms, first.expires_at_ms);
    assert!(windows(&h, 1).is_empty());
    assert_eq!(progress(&h, 1).collected, 0);
    go_to(&mut h, 210);
    assert_eq!(
        (progress(&h, 1).collected, progress(&h, 1).last_window_index),
        (1, 2)
    );
}

#[test]
fn checkpoint_zones_reconnect_preserves_progress_and_protection_prevents_collection() {
    let mut h = host(|v| {
        compact_schedule(v);
        v["members"]["checkpoint_schedules"][0]["window_ms"] = json!(180);
        v["members"]["checkpoint_schedules"][0]["interval_ms"] = json!(240);
    });
    h.action(1, Action::Background {}).unwrap();
    place(&mut h, 1, [420., 400.]);
    go_to(&mut h, 90);
    let original = windows(&h, 0)[0].clone();
    assert_eq!(
        (progress(&h, 0).collected, progress(&h, 1).collected),
        (1, 0)
    );
    let previous = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous.clone(),
            authenticated_principal: h.stores[0].principal.clone(),
        },
    );
    reconnect.connection = Connection {
        id: "checkpoints-reconnect".into(),
        generation: previous.generation + 1,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.pump();
    assert_eq!(windows(&h, 0), &[original]);
    assert_eq!(progress(&h, 0).collected, 1);
    assert!(h.snapshot(0).own.protected);
    for i in 0..2 {
        h.action(i, Action::Foreground {}).unwrap();
        h.pump();
        let publication_id = h.replicas[i].last_full_id().unwrap().to_string();
        h.action(i, Action::CombatReady { publication_id }).unwrap();
        h.pump();
    }
    h.ticks(1);
    assert_eq!(
        (progress(&h, 0).collected, progress(&h, 1).collected),
        (1, 1)
    );
    assert_eq!(reached(&h).len(), 2);
}

#[test]
fn checkpoint_zones_respect_movement_walls() {
    let mut h = host(|v| {
        compact_schedule(v);
        v["members"]["checkpoint_schedules"][0]["radius"] = json!(150);
        v["members"]["checkpoint_schedules"][0]["centers"] = json!([[405, 410], [405, 410]]);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["walkable"] = json!(false);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let first = 10 * 32 + 11;
        v["pve"]["cells_rle"] = json!([[0, first], [1, 1], [0, 1024 - first - 1]]);
    });
    place(&mut h, 1, [495., 410.]);
    go_to(&mut h, 90);
    assert_eq!(
        (progress(&h, 0).collected, progress(&h, 1).collected),
        (1, 0)
    );
    place(&mut h, 1, [420., 410.]);
    go_to(&mut h, 120);
    assert_eq!(progress(&h, 1).collected, 1);
}

#[test]
fn checkpoint_zones_keep_departed_progress_without_freezing_world_windows() {
    let mut h = host(compact_schedule);
    go_to(&mut h, 90);
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.phase, MemberPhase::Settling);
    assert_eq!(progress(&h, 0).collected, 1);
    assert!(windows(&h, 0).is_empty());
    assert_eq!(windows(&h, 1)[0].window_index, 1);
    place(&mut h, 1, [400., 400.]);
    go_to(&mut h, 210);
    assert_eq!(progress(&h, 0).collected, 1);
    assert_eq!(progress(&h, 1).collected, 2);
    h.close();
    assert!(windows(&h, 1).is_empty());
}

#[test]
fn checkpoint_zones_reject_bad_geometry_and_unbounded_schedules() {
    for (key, invalid) in [
        ("id", json!("")),
        ("first_delay_ms", json!(u64::MAX)),
        ("interval_ms", json!(2000)),
        ("window_ms", json!(1)),
        ("radius", json!(513)),
        ("required", json!(4)),
        ("centers", json!([])),
        ("centers", json!([[0, 0]])),
    ] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                v["members"]["checkpoint_schedules"][0][key] = invalid;
            })
            .is_err(),
            "{key}"
        );
    }
    assert!(
        delta_catalog(2, |v| {
            configure(v);
            let schedules = v["members"]["checkpoint_schedules"].as_array_mut().unwrap();
            schedules.push(schedules[0].clone());
        })
        .is_err()
    );
}
