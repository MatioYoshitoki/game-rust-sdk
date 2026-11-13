use super::*;

const COMPANION: &str = "sword_dual_strike";
const VOLLEY: &str = "sword_triple_chase";

#[path = "basic_projectile_paths_tests.rs"]
mod paths;

fn flights(h: &DeltaHarness, i: usize) -> Vec<&ProjectileViewV4> {
    let source = &h.snapshot(i).own.participant.entity;
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::ProjectileStarted { projectile }
                if &projectile.source == source && projectile.basic_segment.is_some() =>
            {
                Some(projectile)
            }
            _ => None,
        })
        .collect()
}

fn contacts(h: &DeltaHarness, i: usize) -> Vec<(&EntityRef, u64)> {
    let source = &h.snapshot(i).own.participant.entity;
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::ProjectileContact {
                source: actor,
                target,
                hit_at_ms,
                ..
            } if actor == source => Some((target, *hit_at_ms)),
            _ => None,
        })
        .collect()
}

fn damage(h: &DeltaHarness, i: usize) -> Vec<u32> {
    let source = &h.snapshot(i).own.participant.entity;
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::Damage {
                source: actor,
                amount,
                ..
            } if actor == source => Some(*amount),
            _ => None,
        })
        .collect()
}

fn until(h: &mut DeltaHarness, done: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..200 {
        if done(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("wave did not reach its expected state");
}

fn ready(h: &mut DeltaHarness, i: usize) {
    until(h, |h| {
        h.snapshot(i).logical_ms >= h.snapshot(i).own.basic_ready_at_ms
    });
}

fn progress(h: &DeltaHarness, i: usize) -> &BasicProjectileProgress {
    h.snapshot(i)
        .own
        .basic_projectile_progress
        .as_ref()
        .unwrap()
}

#[test]
fn basic_projectile_followups_companion_uses_one_action_and_terminal_reward() {
    let mut h = profile_host([Some(COMPANION), None], |v| {
        v["pve"]["monster_positions"] = json!([[490, 400]]);
        v["combat_rules"]["basic_attack"]["life_steal_bps"] = json!(5000);
    });
    h.engine.world.as_mut().unwrap().members[0].hp = 500;
    h.action(0, attack(&h, 0, 0)).unwrap();
    until(&mut h, |h| contacts(h, 0).len() == 1);
    let hit_at = contacts(&h, 0)[0].1;
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert_eq!(h.snapshot(0).own.hp, 500);
    assert!(h.snapshot(1).projectiles.is_empty());
    h.ticks(2);
    assert_eq!(flights(&h, 0).len(), 1);
    h.ticks(1);
    let flight = &h.snapshot(1).projectiles[0];
    assert_eq!(flight, &h.snapshot(0).projectiles[0]);
    let main = flights(&h, 0)[0].basic_segment.as_ref().unwrap();
    let child = flight.basic_segment.as_ref().unwrap();
    assert_eq!(
        (&*child.plan_id, child.index, child.launched_at_ms),
        (COMPANION, 1, hit_at + 90)
    );
    assert_eq!(child.action_id, main.action_id);
    assert_eq!(flight.unit_target, flights(&h, 0)[0].unit_target);
    until(&mut h, |h| h.snapshot(0).own.spirit == 102);
    assert_eq!(damage(&h, 0), [105, 35]);
    assert_eq!(h.snapshot(0).own.hp, 570);
    assert_eq!(
        h.snapshot(0).own.basic_ready_at_ms,
        contacts(&h, 0)[1].1 + 700
    );
    assert!(h.snapshot(1).projectiles.is_empty());
}

#[test]
fn basic_projectile_followups_companion_retargets_nearest_legal_identity_once() {
    let mut h = profile_host([Some(COMPANION), None], |v| {
        v["pve"]["monster_positions"] = json!([[510, 400], [400, 510]]);
        let pve = &v["pve"];
        let mut stats = pve["stats"].clone();
        stats["hp"] = json!(100);
        v["pve"]["additional_templates"]["fragile"] = json!({
            "name":"fragile","mechanism":"melee_projectile_heal","stats":stats,
            "combat":pve["combat"],"drop_table":pve["drop_table"],"leash":pve["leash"],
            "help_range":pve["help_range"],"memory_range":pve["memory_range"],"kill_credit":false,"drop_orb":false
        });
        v["pve"]["extra_spawns"] =
            json!([{"id":"fragile-1","template_id":"fragile","position":[450,400]}]);
    });
    h.action(0, attack(&h, 0, 2)).unwrap();
    until(&mut h, |h| h.snapshot(0).own.spirit == 102);
    assert_eq!(damage(&h, 0), [100, 35]);
    assert_eq!(contacts(&h, 0)[0].0.id, "fragile-1");
    assert_eq!(contacts(&h, 0)[1].0, &h.snapshot(0).monsters[0].entity);
    assert_eq!(flights(&h, 0).len(), 2);
}

#[test]
fn basic_projectile_followups_six_hits_feed_clock_and_one_overlapping_volley() {
    let mut h = profile_host([Some(VOLLEY), None], |v| {
        v["pve"]["monster_positions"] = json!([[590, 400]]);
        v["combat_rules"]["basic_attack"]["life_steal_bps"] = json!(5000);
    });
    h.engine.world.as_mut().unwrap().members[0].hp = 500;
    for hits in 1..=6 {
        ready(&mut h, 0);
        h.action(0, attack(&h, 0, 0)).unwrap();
        until(&mut h, |h| progress(h, 0).stored == hits);
        assert_eq!(progress(&h, 0).attack_speed_bonus_bps, hits * 800);
        if hits < 6 {
            let at = contacts(&h, 0).last().unwrap().1;
            let cooldown = 7_000_000u64.div_ceil(10000 + u64::from(hits) * 800);
            assert_eq!(h.snapshot(0).own.basic_ready_at_ms, at + cooldown);
        }
    }
    let sixth_hit = contacts(&h, 0)[5].1;
    assert_eq!(h.snapshot(0).own.spirit, 110);
    assert_eq!(h.snapshot(0).own.hp, 750);
    h.ticks(4);
    assert!(h.snapshot(1).projectiles.len() >= 2);
    until(&mut h, |h| progress(h, 0).stored == 0);
    let projectiles = flights(&h, 0);
    let segments = projectiles
        .iter()
        .map(|shot| shot.basic_segment.as_ref().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(segments.len(), 12);
    assert_eq!(
        segments[5..].iter().map(|s| s.index).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6]
    );
    assert!(
        segments[5..]
            .iter()
            .all(|s| s.action_id == segments[5].action_id && s.aim_point == [590., 400.])
    );
    assert_eq!(
        segments[6..]
            .iter()
            .map(|s| s.launched_at_ms - sixth_hit)
            .collect::<Vec<_>>(),
        [0, 60, 120, 180, 240, 300]
    );
    assert_eq!(
        damage(&h, 0),
        [100, 100, 100, 100, 100, 100, 35, 35, 35, 35, 35, 35]
    );
    assert_eq!(h.snapshot(0).own.spirit, 112);
    assert_eq!(h.snapshot(0).own.hp, 905);
    assert_eq!(
        h.snapshot(0).own.basic_ready_at_ms,
        contacts(&h, 0).last().unwrap().1 + 700
    );
}

#[test]
fn basic_projectile_followups_volley_replacement_never_takes_old_generation_damage() {
    let mut h = profile_host([Some(VOLLEY), None], |v| {
        v["pve"]["monster_positions"] = json!([[590, 400], [495, 440]]);
    });
    for hits in 1..=6 {
        ready(&mut h, 0);
        h.action(0, attack(&h, 0, 0)).unwrap();
        until(&mut h, |h| progress(h, 0).stored == hits);
    }
    let world = h.engine.world.as_mut().unwrap();
    world.replace_wave_test_npc(0);
    world.place_wave_test_npc(1, [495., 400.]);
    h.ticks(1);
    assert!(h.snapshot(1).projectiles[0].unit_target.is_none());
    until(&mut h, |h| progress(h, 0).stored == 0);
    let projectiles = flights(&h, 0);
    assert_eq!(projectiles.len(), 12);
    assert!(
        projectiles[7..]
            .iter()
            .all(|shot| shot.unit_target.is_none())
    );
    assert_eq!(damage(&h, 0), [100; 6]);
    assert_eq!(h.snapshot(0).monsters[1].hp, 10000);
    assert_eq!(h.snapshot(0).monsters[0].entity.generation, 2);
}

#[test]
fn basic_projectile_followups_wall_and_capacity_leave_unchanged_progress() {
    let mut wall = profile_host([Some(VOLLEY), None], |v| {
        v["pve"]["monster_positions"] = json!([[590, 400]]);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        wall["blocksVision"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let cell = 10 * 32 + 12;
        v["pve"]["cells_rle"] = json!([[0, cell], [1, 1], [0, 1024 - cell - 1]]);
    });
    wall.action(0, attack(&wall, 0, 0)).unwrap();
    wall.ticks(10);
    assert!(wall.events.iter().any(|(_, e)| matches!(
        e,
        EventV4::ProjectileEnded {
            reason: Some(ProjectileEndReasonV4::Wall),
            ..
        }
    )));
    assert_eq!(progress(&wall, 0).stored, 0);
    assert_eq!(wall.snapshot(0).own.spirit, 100);

    let mut h = profile_host([Some(VOLLEY), Some(VOLLEY)], |v| {
        v["pve"]["monster_positions"] = json!([[490, 440]]);
        v["members"]["projectile_capacity"] = json!(6);
    });
    for hits in 1..=5 {
        for i in 0..2 {
            ready(&mut h, i);
            h.action(i, attack(&h, i, 0)).unwrap();
        }
        until(&mut h, |h| {
            [0, 1].iter().all(|i| progress(h, *i).stored == hits)
        });
    }
    for i in 0..2 {
        ready(&mut h, i);
        h.action(i, attack(&h, i, 0)).unwrap();
    }
    h.ticks(4);
    assert_eq!(flights(&h, 1).len(), 5);
    assert_eq!(progress(&h, 1).stored, 5);
    assert!(
        h.events
            .iter()
            .any(|(recipient, e)| recipient.as_deref() == Some("B")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionRejected {
                            reason: Error::Capacity,
                            ..
                        }
                    }
                ))
    );
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(h.snapshot(1).projectiles.is_empty());
    h.action(1, attack(&h, 1, 0)).unwrap();
    until(&mut h, |h| progress(h, 1).stored == 0);
    assert_eq!(flights(&h, 1).len(), 12);
    assert!(h.snapshot(1).projectiles.is_empty());
}

#[test]
fn basic_projectile_followups_reject_invalid_pattern_and_capacity_configurations() {
    for (profile, key, value) in [
        (COMPANION, "delay_ms", json!(0)),
        (COMPANION, "damage_bps", json!(100001)),
        (VOLLEY, "hits", json!(9)),
        (VOLLEY, "interval_ms", json!(0)),
        (VOLLEY, "attack_speed_bps_per_hit", json!(1667)),
    ] {
        assert!(
            profile_catalog(|v| {
                v["combat_rules"]["basic_profiles"][profile]["projectile_followup"]["pattern"]
                    [key] = value;
            })
            .is_err(),
            "{profile} {key}"
        );
    }
    assert!(profile_catalog(|v| v["members"]["projectile_capacity"] = json!(5)).is_err());
    assert!(
        profile_catalog(|v| {
            v["combat_rules"]["basic_profiles"][VOLLEY]["piercing"] =
                json!({"plan_id":"invalid-combo","max_hits":2});
        })
        .is_err()
    );
}
