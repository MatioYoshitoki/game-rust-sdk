use super::*;

const RECALL: &str = "sword_piercing_array";
const RICOCHET: &str = "shield_double_quake";
const RETURN: &str = "shield_double_quake_return";

fn seals(h: &DeltaHarness) -> &[[f64; 2]] {
    h.snapshot(1).members[0].basic_seals.as_deref().unwrap()
}

fn role(shot: &ProjectileViewV4) -> Option<BasicProjectileRole> {
    shot.basic_segment.as_ref().unwrap().role
}

fn flight_ended(h: &DeltaHarness, id: &str, expected: Option<ProjectileEndReasonV4>) -> bool {
    h.events.iter().any(|(_, event)| {
        matches!(event, EventV4::ProjectileEnded {
        projectile_id, reason, ..
    } if projectile_id == id && *reason == expected)
    })
}

fn place_wall(v: &mut Value, column: usize) {
    let mut wall = v["pve"]["palette"][0].clone();
    wall["blocksProjectiles"] = json!(true);
    v["pve"]["palette"].as_array_mut().unwrap().push(wall);
    let cell = 10 * 32 + column;
    v["pve"]["cells_rle"] = json!([[0, cell], [1, 1], [0, 1024 - cell - 1]]);
}

#[test]
fn basic_projectile_paths_six_seals_recall_on_one_action_and_refresh_slow() {
    let mut h = profile_host([Some(RECALL), None], |v| {
        v["pve"]["monster_positions"] = json!([[500, 400], [550, 420], [520, 429]]);
    });
    for count in 1..=6 {
        ready(&mut h, 0);
        h.action(0, attack(&h, 0, 0)).unwrap();
        until(&mut h, |h| seals(h).len() == count);
        assert_eq!(seals(&h), vec![[584., 400.]; count]);
    }
    let sixth = h.snapshot(0).logical_ms;
    let outbound = flights(&h, 0)[5]
        .basic_segment
        .as_ref()
        .unwrap()
        .action_id
        .clone();
    assert_eq!(h.snapshot(0).own.spirit, 110);
    assert_eq!(damage(&h, 0), [100, 40].repeat(6));
    h.ticks(3);
    assert_eq!(flights(&h, 0).len(), 6);
    h.ticks(1);
    assert_eq!(seals(&h).len(), 5);
    assert_eq!(h.snapshot(0).own.spirit, 110);
    until(&mut h, |h| h.snapshot(0).own.spirit == 112);
    let started = flights(&h, 0);
    assert_eq!(started.len(), 12);
    assert!(
        started[..6]
            .iter()
            .all(|shot| role(shot) == Some(BasicProjectileRole::SealOutbound))
    );
    assert!(
        started[6..]
            .iter()
            .all(|shot| role(shot) == Some(BasicProjectileRole::SealRecall))
    );
    let segments = started[6..]
        .iter()
        .map(|shot| shot.basic_segment.as_ref().unwrap())
        .collect::<Vec<_>>();
    assert!(
        segments
            .iter()
            .all(|segment| segment.action_id == outbound && segment.aim_point == [400., 400.])
    );
    assert_eq!(
        segments
            .iter()
            .map(|segment| segment.launched_at_ms - sixth)
            .collect::<Vec<_>>(),
        [120, 180, 210, 270, 330, 390]
    );
    assert_eq!(damage(&h, 0), [[100, 40].repeat(6), vec![18; 12]].concat());
    assert_eq!(h.snapshot(0).monsters[2].hp, 10000);
    for npc in &h.snapshot(0).monsters[..2] {
        let status = &npc.statuses.as_ref().unwrap()[0];
        assert_eq!(status.definition_id, "seal-recall-slow");
        assert_eq!(status.modifier_bps, -2000);
        assert_eq!(status.expires_at_ms - status.applied_at_ms, 2000);
    }
    assert!(h.snapshot(0).projectiles.is_empty());
    assert!(seals(&h).is_empty());
    assert_eq!(
        h.snapshot(0).own.basic_ready_at_ms,
        h.snapshot(0).logical_ms + 700
    );
}

#[test]
fn basic_projectile_paths_walls_gate_seals_and_each_recall_uses_its_launch_position() {
    let mut blocked = profile_host([Some(RECALL), None], |v| {
        v["pve"]["monster_positions"] = json!([[500, 400]]);
        place_wall(v, 12);
    });
    blocked.action(0, attack(&blocked, 0, 0)).unwrap();
    blocked.ticks(14);
    assert!(seals(&blocked).is_empty());
    assert!(damage(&blocked, 0).is_empty());
    let first = flights(&blocked, 0)[0];
    assert!(flight_ended(
        &blocked,
        &first.id,
        Some(ProjectileEndReasonV4::Wall)
    ));

    let mut h = profile_host([Some(RECALL), None], |v| {
        v["pve"]["monster_positions"] = json!([[500, 400], [600, 400]]);
        v["combat_rules"]["basic_profiles"][RECALL]["projectile_followup"]["pattern"]["required_seals"] =
            json!(2);
        place_wall(v, 13);
    });
    for count in 1..=2 {
        ready(&mut h, 0);
        h.action(0, attack(&h, 0, 0)).unwrap();
        until(&mut h, |h| seals(h).len() == count);
        assert!(
            seals(&h)
                .iter()
                .all(|point| (point[0] - 520.).abs() < 10. && point[0] < 520.)
        );
    }
    until(&mut h, |h| flights(h, 0).len() == 3);
    h.engine.world.as_mut().unwrap().members[0].position = [650., 400.];
    until(&mut h, |h| h.snapshot(0).own.spirit == 104);
    assert_eq!(damage(&h, 0), [100, 100, 18]);
    let flight = flights(&h, 0);
    assert_eq!(
        flight[2].basic_segment.as_ref().unwrap().aim_point,
        [400., 400.]
    );
    assert_eq!(
        flight[3].basic_segment.as_ref().unwrap().aim_point,
        [650., 400.]
    );
    assert!(flight_ended(
        &h,
        &flight[2].id,
        Some(ProjectileEndReasonV4::RangeExhausted)
    ));
    assert!(flight_ended(
        &h,
        &flight[3].id,
        Some(ProjectileEndReasonV4::Wall)
    ));
    assert!(seals(&h).is_empty());
}

#[test]
fn basic_projectile_paths_ricochet_uses_distinct_targets_and_stops_at_configured_cap() {
    let mut h = profile_host([Some(RICOCHET), None], |v| {
        v["pve"]["monster_positions"] =
            json!([[500, 400], [550, 400], [600, 400], [650, 400], [700, 400]]);
    });
    h.action(0, attack(&h, 0, 0)).unwrap();
    until(&mut h, |h| h.snapshot(0).own.spirit == 102);
    assert_eq!(damage(&h, 0), [72, 56, 56, 56]);
    let shots = flights(&h, 0);
    let action = &shots[0].basic_segment.as_ref().unwrap().action_id;
    for (i, shot) in shots.iter().enumerate() {
        let segment = shot.basic_segment.as_ref().unwrap();
        assert_eq!(segment.index as usize, i);
        assert_eq!(&segment.action_id, action);
        assert_eq!(role(shot), Some(BasicProjectileRole::Ricochet));
        assert_eq!(
            shot.unit_target.as_ref().unwrap().id,
            h.snapshot(0).monsters[i].entity.id
        );
    }
    assert_eq!(shots.len(), 4);
    assert_eq!(h.snapshot(0).monsters[4].hp, 10000);
    assert_eq!(
        h.snapshot(0).own.basic_ready_at_ms,
        contacts(&h, 0).last().unwrap().1 + 1000
    );
}

#[test]
fn basic_projectile_paths_marks_count_by_source_and_resist_control_for_elite() {
    for profile in ["npc", "elite"] {
        let mut h = profile_host([Some(RICOCHET), Some(RICOCHET)], |v| {
            v["pve"]["monster_positions"] = json!([[480, 420]]);
            v["combat_rules"]["default_npc_profile"] = json!(profile);
        });
        let strikes = [0, 1, 1, 0, 0];
        for i in strikes {
            ready(&mut h, i);
            let next = contacts(&h, i).len() + 1;
            h.action(i, attack(&h, i, 0)).unwrap();
            until(&mut h, |h| contacts(h, i).len() == next);
            let statuses = h.snapshot(0).monsters[0].statuses.as_ref().unwrap();
            assert_eq!(
                statuses.len(),
                usize::from(i == 0 && next == 3 && profile == "npc")
            );
            if let Some(stun) = statuses.first() {
                assert_eq!(stun.definition_id, "ricochet-stun");
                assert_eq!(stun.source.id, "A");
                assert_eq!(stun.expires_at_ms - stun.applied_at_ms, 800);
            }
        }
    }
}

#[test]
fn basic_projectile_paths_visual_return_grants_shared_shield_only_after_arrival() {
    let mut h = profile_host([Some(RETURN), None], |v| {
        v["pve"]["monster_positions"] = json!([[500, 400], [550, 400]]);
        v["pve"]["status_rules"]["shield_cap_bps"] = json!(450);
    });
    h.action(0, attack(&h, 0, 0)).unwrap();
    until(&mut h, |h| {
        flights(h, 0)
            .last()
            .is_some_and(|shot| role(shot) == Some(BasicProjectileRole::ReturnToOwner))
    });
    let last = (*flights(&h, 0).last().unwrap()).clone();
    assert_eq!(last.basic_segment.as_ref().unwrap().aim_point, [400., 400.]);
    assert!(last.unit_target.is_none());
    assert_eq!(h.snapshot(0).own.shield_total, Some(0));
    assert_eq!(h.snapshot(0).own.spirit, 100);
    until(&mut h, |h| h.snapshot(0).own.spirit == 102);
    assert_eq!(damage(&h, 0), [72, 84]);
    assert_eq!(contacts(&h, 0).len(), 2);
    assert_eq!(h.snapshot(0).own.shield_total, Some(45));
    assert!(flight_ended(&h, &last.id, None));
    let status = own_status(&h, 0, "ricochet-return-shield").unwrap().clone();
    assert_eq!(status.expires_at_ms - status.applied_at_ms, 1800);

    ready(&mut h, 0);
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(1, [800., 700.]);
    h.action(0, attack(&h, 0, 0)).unwrap();
    until(&mut h, |h| h.snapshot(0).own.spirit == 104);
    assert_eq!(damage(&h, 0), [72, 84, 72]);
    assert_eq!(h.snapshot(0).own.shield_total, Some(45));
    let refreshed = own_status(&h, 0, "ricochet-return-shield").unwrap();
    assert_eq!(refreshed.instance_id, status.instance_id);
    assert!(refreshed.expires_at_ms > status.expires_at_ms);
}

#[test]
fn basic_projectile_paths_zero_actual_damage_can_return_without_shield_or_seal() {
    let mut h = profile_host([Some(RECALL), Some(RETURN)], |v| {
        v["pve"]["monster_positions"] = json!([[500, 400]]);
        v["combat_rules"]["basic_attack"]["base_power"] = json!(0);
        v["params"]["minimum_damage"] = json!(0);
    });
    for i in 0..2 {
        h.action(i, attack(&h, i, 0)).unwrap();
    }
    until(&mut h, |h| h.snapshot(1).own.spirit == 102);
    assert_eq!(contacts(&h, 0).len(), 1);
    assert_eq!(contacts(&h, 1).len(), 1);
    assert_eq!(h.snapshot(1).monsters[0].hp, 10000);
    assert!(seals(&h).is_empty());
    assert_eq!(h.snapshot(1).own.shield_total, Some(0));
    assert_eq!(flights(&h, 1).len(), 2);
    assert_eq!(
        role(flights(&h, 1)[1]),
        Some(BasicProjectileRole::ReturnToOwner)
    );
}

#[test]
fn basic_projectile_paths_reject_invalid_profiles_and_projectile_budgets() {
    let load = |change: fn(&mut Value)| profile_catalog_with_paths(true, change);
    assert!(load(|_| {}).is_ok());
    assert!(load(|v| v["members"]["projectile_capacity"] = json!(5)).is_err());
    assert!(load(|v| {
        v["combat_rules"]["basic_profiles"][RECALL]["projectile_followup"]["pattern"]["slow_status"] =
            json!("ricochet-stun");
    }).is_err());
    assert!(load(|v| {
        v["combat_rules"]["basic_profiles"][RETURN]["projectile_followup"]["pattern"]["return_to_owner"]["speed_bps"] =
            json!(0);
    }).is_err());
    assert!(load(|v| {
        v["combat_rules"]["basic_profiles"][RICOCHET]["projectile_followup"]["pattern"]["resistant_profiles"] =
            json!(["elite", "elite"]);
    }).is_err());
}
