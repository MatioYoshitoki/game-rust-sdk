use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-3")
        .unwrap()
        .as_ref()
        .clone();
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.chest_spawns[0].position = [400.0, 400.0];
    p.chest_spawns[1].position = [440.0, 400.0];
    c
}
fn world(c: &Config) -> World {
    let mut w = World::new(
        c,
        ParticipantFacts {
            principal: "principal-A".into(),
            revision: 1,
            hp_points: 1000,
            spirit_points: 100,
            atk_points: 100,
            def_points: 10,
            spd_points: 8,
            warehouse_quantity: 95,
        },
        7,
    );
    w.phase = Phase::Active;
    w.active_at = Some(0);
    w.joined = true;
    w.ready = true;
    w.foreground = true;
    w.protected = false;
    w.position = [400.0, 400.0];
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.hp = 0;
    }
    w
}
fn open(g: u32) -> Action {
    Action::OpenChest {
        chest_id: "sealed-cache-1".into(),
        expected_generation: g,
    }
}
fn cancel(g: u32) -> Action {
    Action::CancelChestUnlock {
        chest_id: "sealed-cache-1".into(),
        expected_generation: g,
    }
}
fn seal(w: &World, c: &Config, now: u64) -> SealView {
    w.snapshot(c, now).pve.unwrap().chests[0]
        .seal
        .clone()
        .unwrap()
}
fn unlock(w: &mut World, c: &Config) {
    w.action(&open(1), "begin", c, 0).unwrap();
    w.tick(c, 1200);
    w.tick(c, 2400);
}
fn retention(c: &mut Config, ratio: f64) {
    c.pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .seal
        .as_mut()
        .unwrap()
        .interrupt_retention_ratio = ratio;
}

#[test]
fn source_fixture_uses_explicit_layered_capability_and_rare_loot() {
    let c = config();
    let r = c.pve.as_ref().unwrap().treasure_rules.as_ref().unwrap();
    let s = r.template.seal.as_ref().unwrap();
    assert_eq!(c.mechanism_version, 2);
    assert_eq!((s.layers, s.layer_duration_ms), (2, 1200));
    assert_eq!(s.interrupt_retention_ratio, 0.0);
    assert_eq!(r.loot.guaranteed[0].item_id, "han-tan");
    assert_ne!(r.template.id, "treasure-jade-box");
}
#[test]
fn layer_deadlines_unlock_without_reward_and_require_explicit_open() {
    let c = config();
    let mut w = world(&c);
    w.action(&open(1), "start", &c, 0).unwrap();
    assert_eq!(seal(&w, &c, 0).layer_progress_ms, 0);
    w.tick(&c, 1199);
    assert_eq!(seal(&w, &c, 1199).completed_layers, 0);
    let e = w.tick(&c, 1200);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ChestLayerCompleted {
            completed_layers: 1,
            ..
        }
    )));
    w.tick(&c, 2399);
    assert_eq!(seal(&w, &c, 2399).completed_layers, 1);
    let e = w.tick(&c, 2400);
    assert!(e.iter().any(|e| matches!(e, Event::ChestUnlocked { .. })));
    assert_eq!(seal(&w, &c, 2400).completed_layers, 2);
    assert!(!seal(&w, &c, 2400).unlocking);
    assert!(w.resources.is_empty());
    assert_eq!(w.inventory, 5);
    assert_eq!(w.kills, 0);
    w.action(&open(1), "claim", &c, 2400).unwrap();
    let rng = w.rng;
    let n = w.resources.len();
    assert!(n > 0);
    assert_eq!(
        w.action(&open(1), "again", &c, 2400),
        Err(Error::NoResource)
    );
    assert_eq!((w.resources.len(), w.rng), (n, rng));
}
#[test]
fn duplicate_start_does_not_restart_and_other_container_cannot_replace_owner() {
    let c = config();
    let mut w = world(&c);
    w.action(&open(1), "start", &c, 0).unwrap();
    w.tick(&c, 600);
    assert_eq!(
        w.action(&open(1), "other", &c, 600),
        Err(Error::InvalidState)
    );
    assert_eq!(
        w.action(
            &Action::OpenChest {
                chest_id: "sealed-cache-2".into(),
                expected_generation: 1
            },
            "second",
            &c,
            600
        ),
        Err(Error::InvalidState)
    );
    w.tick(&c, 1200);
    assert_eq!(seal(&w, &c, 1200).completed_layers, 1);
}
#[test]
fn configured_interrupt_retention_preserves_completed_layers_and_fraction_of_current() {
    for (ratio, expected) in [(0.0, 0), (0.5, 300), (1.0, 600)] {
        let mut c = config();
        retention(&mut c, ratio);
        let mut w = world(&c);
        w.action(&open(1), "start", &c, 0).unwrap();
        w.tick(&c, 1800);
        w.action(&cancel(1), "cancel", &c, 1800).unwrap();
        let s = seal(&w, &c, 1800);
        assert_eq!((s.completed_layers, s.layer_progress_ms), (1, expected));
        assert_eq!(s.interruption_reason, Some(ChestInterruptReason::Cancelled));
        w.tick(&c, 3000);
        assert_eq!(seal(&w, &c, 3000), s);
        w.action(&open(1), "resume", &c, 3000).unwrap();
        w.tick(&c, 4200 - expected);
        assert_eq!(seal(&w, &c, 4200 - expected).completed_layers, 2);
    }
}
#[test]
fn invalid_navigation_keeps_progress_but_valid_movement_interrupts() {
    let c = config();
    let mut w = world(&c);
    w.action(&open(1), "start", &c, 0).unwrap();
    w.tick(&c, 600);
    assert_eq!(
        w.action(
            &Action::Navigate {
                destination: [-1.0, -1.0]
            },
            "invalid",
            &c,
            600
        ),
        Err(Error::OutOfRange)
    );
    assert!(seal(&w, &c, 600).unlocking);
    w.action(&Action::Stop {}, "stop", &c, 600).unwrap();
    assert!(seal(&w, &c, 600).unlocking);
    w.action(
        &Action::Move {
            direction: [1.0, 0.0],
        },
        "move",
        &c,
        600,
    )
    .unwrap();
    assert_eq!(
        seal(&w, &c, 600).interruption_reason,
        Some(ChestInterruptReason::Movement)
    );
    w.action(&open(1), "resume", &c, 600).unwrap();
    w.action(
        &Action::Navigate {
            destination: [500.0, 400.0],
        },
        "nav",
        &c,
        600,
    )
    .unwrap();
    assert!(!seal(&w, &c, 600).unlocking);
}
#[test]
fn range_and_wall_gate_unlock_without_state_or_rng_side_effects() {
    let mut c = config();
    let mut w = world(&c);
    w.position = [324.99, 400.0];
    let rng = w.rng;
    assert_eq!(w.action(&open(1), "far", &c, 0), Err(Error::OutOfRange));
    assert_eq!(w.rng, rng);
    w.position = [325.0, 400.0];
    w.action(&open(1), "edge", &c, 0).unwrap();
    c.pve_mut().cells_rle = (0..1024)
        .map(|i| (usize::from(i % 32 == 11) * 2, 1))
        .collect();
    let mut w = world(&c);
    w.position = [330.0, 400.0];
    assert_eq!(w.action(&open(1), "wall", &c, 0), Err(Error::OutOfRange));
    assert!(!seal(&w, &c, 0).unlocking);
}
#[test]
fn lifecycle_and_cast_or_extraction_gates_prevent_start() {
    let c = config();
    for mode in 0..7 {
        let mut w = world(&c);
        match mode {
            0 => w.ready = false,
            1 => w.protected = true,
            2 => w.hp = 0,
            3 => w.phase = Phase::Closed,
            4 => {
                w.pending = Some(PendingAction {
                    ground_position: None,
                    unit_target: None,
                    selected_skill: None,
                    rounds: vec![],
                    accepted_at: 0,
                    id: "cast".into(),
                    due_ms: 900,
                    skill: true,
                    direction: [1.0, 0.0],
                })
            }
            5 => w.extraction_end = Some(5000),
            _ => {
                w.action(&Action::Background {}, "bg", &c, 0).unwrap();
            }
        }
        assert!(w.action(&open(1), "bad", &c, 0).is_err());
        assert!(!seal(&w, &c, 0).unlocking);
        assert!(w.resources.is_empty());
    }
}
#[test]
fn cracking_excludes_attack_skill_pickup_and_extraction() {
    let c = config();
    let mut w = world(&c);
    w.action(&open(1), "start", &c, 0).unwrap();
    for a in [
        Action::Attack {},
        Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1.0, 0.0],
        },
        Action::Pickup {
            resource_id: "none".into(),
            expected_revision: 1,
        },
        Action::ExtractStart {},
    ] {
        assert_eq!(w.action(&a, "mutex", &c, 0), Err(Error::InvalidState));
    }
    assert!(seal(&w, &c, 0).unlocking);
    assert_eq!(w.spirit, 100);
}
fn enemy(c: &mut Config, w: &mut World, damage: u32) {
    let p = c.pve_mut();
    p.combat.cast_chance = 0.0;
    p.stats.insert("atk".into(), damage);
    p.stats.insert("crit".into(), 0);
    p.stats.insert("spd".into(), 0);
    let mut fresh = world(c);
    let m = &mut fresh.pve.as_mut().unwrap().monsters[0];
    m.hp = 100;
    m.position = [440.0, 400.0];
    m.home = m.position;
    m.aggro = true;
    m.next_attack = 1200;
    *w = fresh;
}
#[test]
fn same_tick_positive_damage_wins_over_layer_completion() {
    let mut c = config();
    let mut w = world(&c);
    enemy(&mut c, &mut w, 10);
    w.action(&open(1), "start", &c, 0).unwrap();
    w.tick(&c, 1199);
    let e = w.tick(&c, 1200);
    assert!(
        e.iter()
            .any(|e| matches!(e,Event::Damage{amount,..}if *amount>0))
    );
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::ChestLayerCompleted { .. }))
    );
    assert_eq!(seal(&w, &c, 1200).completed_layers, 0);
    assert_eq!(
        seal(&w, &c, 1200).interruption_reason,
        Some(ChestInterruptReason::Damage)
    );
}
#[test]
fn zero_actual_damage_does_not_interrupt_unlock() {
    let mut c = config();
    c.params.minimum_damage = 0;
    let mut w = world(&c);
    enemy(&mut c, &mut w, 0);
    w.action(&open(1), "start", &c, 0).unwrap();
    w.tick(&c, 1200);
    assert_eq!(seal(&w, &c, 1200).completed_layers, 1);
    assert!(seal(&w, &c, 1200).unlocking);
}
#[test]
fn background_keeps_completed_layers_but_does_not_automatically_resume() {
    let c = config();
    let mut w = world(&c);
    w.action(&open(1), "start", &c, 0).unwrap();
    w.tick(&c, 1800);
    w.action(&Action::Background {}, "bg", &c, 1800).unwrap();
    let s = seal(&w, &c, 1800);
    assert_eq!((s.completed_layers, s.layer_progress_ms), (1, 0));
    assert_eq!(
        s.interruption_reason,
        Some(ChestInterruptReason::Background)
    );
    w.action(&Action::Foreground {}, "fg", &c, 1800).unwrap();
    w.action(
        &Action::CombatReady {
            publication_id: "test".into(),
        },
        "ready",
        &c,
        1800,
    )
    .unwrap();
    w.tick(&c, 5000);
    assert_eq!(seal(&w, &c, 5000), s);
}
#[test]
fn leave_and_absolute_terminal_never_complete_or_reward_active_interaction() {
    for terminal in [false, true] {
        let mut c = config();
        if terminal {
            c.terminal_ms = 1800;
        }
        let mut w = world(&c);
        w.action(&open(1), "start", &c, 0).unwrap();
        w.tick(&c, 1200);
        if terminal {
            w.tick(&c, 1800);
        } else {
            w.action(&Action::Leave {}, "leave", &c, 1800).unwrap();
        }
        let s = seal(&w, &c, 1800);
        assert!(!s.unlocking);
        w.tick(&c, 600000);
        assert_eq!(seal(&w, &c, 600000), s);
        assert!(w.resources.is_empty());
    }
}
#[test]
fn failed_ground_commit_retains_unlock_and_rng_then_retries_once() {
    let c = config();
    let mut w = world(&c);
    unlock(&mut w, &c);
    for i in 0..50 {
        w.resources.push(Resource {
            view: ResourceView {
                id: format!("old-{i}"),
                position: w.position,
                quantity: 1,
                orb: false,
            },
            hp: 0,
            spirit: 0,
        });
    }
    let rng = w.rng;
    assert_eq!(w.action(&open(1), "full", &c, 2400), Err(Error::Capacity));
    assert_eq!(w.rng, rng);
    assert_eq!(seal(&w, &c, 2400).completed_layers, 2);
    w.resources.clear();
    w.action(&open(1), "retry", &c, 2400).unwrap();
    assert!(w.snapshot(&c, 2400).pve.unwrap().chests[0].opened);
}
#[test]
fn respawn_resets_progress_and_stale_cancel_cannot_release_new_generation() {
    let mut c = config();
    c.pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .respawn_delay_ms = 1000;
    let mut w = world(&c);
    unlock(&mut w, &c);
    w.action(&open(1), "claim", &c, 2400).unwrap();
    w.tick(&c, 3399);
    assert!(w.snapshot(&c, 3399).pve.unwrap().chests[0].opened);
    w.tick(&c, 3400);
    assert_eq!(seal(&w, &c, 3400).completed_layers, 0);
    w.action(&open(2), "new", &c, 3400).unwrap();
    assert_eq!(
        w.action(&cancel(1), "old-cancel", &c, 3400),
        Err(Error::RevisionConflict)
    );
    assert!(seal(&w, &c, 3400).unlocking);
    assert_eq!(
        w.action(&open(1), "old-start", &c, 3400),
        Err(Error::RevisionConflict)
    );
}
#[test]
fn recovery_yield_bound_includes_first_and_repeated_unlock_duration() {
    let c = config();
    assert_eq!(c.pve_loot_bound("han-tan", 0, 2399), 0);
    assert_eq!(c.pve_loot_bound("han-tan", 0, 2400), 6);
    assert_eq!(c.pve_loot_bound("han-tan", 0, 604799), 6);
    assert_eq!(c.pve_loot_bound("han-tan", 0, 604800), 12);
}
#[test]
fn configured_layer_counts_and_durations_use_the_same_clock() {
    for layers in [1, 3, 5] {
        let mut c = config();
        let s = c
            .pve_mut()
            .treasure_rules
            .as_mut()
            .unwrap()
            .template
            .seal
            .as_mut()
            .unwrap();
        s.layers = layers;
        s.layer_duration_ms = 90;
        let mut w = world(&c);
        w.action(&open(1), "begin", &c, 0).unwrap();
        for count in 1..=layers {
            w.tick(&c, u64::from(count) * 90);
            assert_eq!(seal(&w, &c, u64::from(count) * 90).completed_layers, count);
        }
        assert!(!seal(&w, &c, u64::from(layers) * 90).unlocking);
        assert!(w.resources.is_empty());
    }
}
