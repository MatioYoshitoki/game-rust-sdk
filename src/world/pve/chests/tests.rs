use super::*;
use crate::config::default_catalog;

fn config() -> Config {
    default_catalog().active("b3-2").unwrap().as_ref().clone()
}
fn world(c: &Config, seed: u32) -> World {
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
        seed,
    );
    w.phase = Phase::Active;
    w.active_at = Some(0);
    w.ready = true;
    w.joined = true;
    w.protected = false;
    w.position = c.pve.as_ref().unwrap().chest_spawns[0].position;
    for monster in &mut w.pve.as_mut().unwrap().monsters {
        monster.hp = 0;
    }
    w
}
fn flat() -> Config {
    let mut c = config();
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.chest_spawns[0].position = [400.0, 400.0];
    p.chest_spawns[1].position = [600.0, 400.0];
    c
}
fn action(c: &Config, generation: u32) -> Action {
    Action::OpenChest {
        chest_id: c.pve.as_ref().unwrap().chest_spawns[0].id.clone(),
        expected_generation: generation,
    }
}
fn drops(w: &World) -> Vec<ResourceView> {
    w.resources.iter().map(|r| r.view.clone()).collect()
}

#[test]
fn source_pouches_have_ordinary_rules_and_reachable_authored_positions() {
    let c = config();
    let p = c.pve.as_ref().unwrap();
    assert_eq!(p.chest_spawns.len(), 2);
    assert_eq!(p.chest_spawns[0].position, [4176.0, 2640.0]);
    assert_eq!(p.chest_spawns[1].position, [5936.0, 2800.0]);
    let rules = p.treasure_rules.as_ref().unwrap();
    assert!(matches!(
        rules.template.mechanism,
        crate::config::ContainerMechanism::ImmediateGroundLoot
    ));
    assert_eq!(rules.template.respawn_delay_ms, 300000);
    assert_eq!(rules.interaction_range, 75.0);
    assert_eq!(rules.loot.guaranteed[0].item_id, "huo-ling");
    assert_eq!(rules.loot.random[0].chance, Some(0.3));
    assert!(validate_map(p, c.spawn, c.extraction_point));
}

#[test]
fn opening_is_immediate_ground_loot_without_inventory_or_score_credit() {
    let c = config();
    let mut w = world(&c, 7);
    let inventory = w.inventory;
    let revision = w.inventory_revision;
    let events = w.action(&action(&c, 1), "open", &c, 30).unwrap();
    assert!(matches!(
        &events[..],
        [Event::ChestOpened { generation: 1, .. }]
    ));
    assert_eq!(w.inventory, inventory);
    assert_eq!(w.inventory_revision, revision);
    assert_eq!(w.kills, 0);
    assert!(w.pve.as_ref().unwrap().items.is_empty());
    assert!(!w.resources.is_empty());
    assert_eq!(
        w.snapshot(&c, 30).pve.unwrap().chests[0].respawn_at_ms,
        Some(300030)
    );
}

#[test]
fn repeated_opening_has_no_second_loot_or_rng_side_effect() {
    let c = config();
    let mut w = world(&c, 7);
    w.action(&action(&c, 1), "open", &c, 30).unwrap();
    let before = drops(&w);
    let rng = w.rng;
    for id in ["open", "retry", "different"] {
        assert_eq!(w.action(&action(&c, 1), id, &c, 60), Err(Error::NoResource));
    }
    assert_eq!(drops(&w), before);
    assert_eq!(w.rng, rng);
}

#[test]
fn interaction_range_and_wall_occlusion_are_authoritative() {
    let c = flat();
    let mut w = world(&c, 7);
    w.position = [324.999, 400.0];
    assert_eq!(
        w.action(&action(&c, 1), "far", &c, 30),
        Err(Error::OutOfRange)
    );
    assert!(w.resources.is_empty());
    w.position = [325.0, 400.0];
    w.action(&action(&c, 1), "edge", &c, 30).unwrap();
    let mut c = flat();
    c.pve_mut().cells_rle = (0..1024)
        .map(|i| (usize::from(i % 32 == 11) * 2, 1))
        .collect();
    let mut w = world(&c, 7);
    w.position = [330.0, 400.0];
    assert_eq!(
        w.action(&action(&c, 1), "wall", &c, 30),
        Err(Error::OutOfRange)
    );
}

#[test]
fn readiness_background_death_and_terminal_states_cannot_open() {
    let c = config();
    for state in 0..6 {
        let mut w = world(&c, 7);
        match state {
            0 => w.ready = false,
            1 => w.protected = true,
            2 => {
                w.action(&Action::Background {}, "background", &c, 0)
                    .unwrap();
            }
            3 => w.hp = 0,
            4 => w.phase = Phase::Deploying,
            _ => w.phase = Phase::Closed,
        }
        assert!(w.action(&action(&c, 1), "open", &c, 30).is_err());
        assert!(w.resources.is_empty());
        assert!(!w.snapshot(&c, 30).pve.unwrap().chests[0].opened);
    }
}

#[test]
fn attack_windup_and_extraction_channel_reject_open_without_cancelling_them() {
    let c = config();
    let mut w = world(&c, 7);
    w.pending = Some(PendingAction {
        ground_position: None,
        unit_target: None,
        selected_skill: None,
        rounds: vec![],
        accepted_at: 0,
        id: "cast".into(),
        due_ms: 200,
        skill: true,
        direction: [1.0, 0.0],
    });
    assert_eq!(
        w.action(&action(&c, 1), "open", &c, 30),
        Err(Error::InvalidState)
    );
    assert!(w.pending.is_some());
    w.pending = None;
    w.extraction_end = Some(5000);
    assert_eq!(
        w.action(&action(&c, 1), "open", &c, 30),
        Err(Error::InvalidState)
    );
    assert_eq!(w.extraction_end, Some(5000));
    assert!(w.resources.is_empty());
}

#[test]
fn nonexistent_and_stale_generation_do_not_consume_rng() {
    let c = config();
    let mut w = world(&c, 7);
    let rng = w.rng;
    assert_eq!(
        w.action(
            &Action::OpenChest {
                chest_id: "missing".into(),
                expected_generation: 1
            },
            "none",
            &c,
            30
        ),
        Err(Error::NoResource)
    );
    assert_eq!(
        w.action(&action(&c, 2), "stale", &c, 30),
        Err(Error::RevisionConflict)
    );
    assert_eq!(w.rng, rng);
    assert!(w.resources.is_empty());
}

#[test]
fn chance_zero_one_and_quantity_bounds_replay_deterministically() {
    for chance in [0.0, 0.3, 1.0] {
        let mut c = config();
        c.pve_mut().treasure_rules.as_mut().unwrap().loot.random[0].chance = Some(chance);
        let mut bounds = std::collections::BTreeSet::new();
        let mut seen_random = false;
        let mut seen_none = false;
        for seed in 1..=256 {
            let mut a = world(&c, seed);
            let mut b = world(&c, seed);
            a.action(&action(&c, 1), "open", &c, 30).unwrap();
            b.action(&action(&c, 1), "open", &c, 30).unwrap();
            assert_eq!(drops(&a), drops(&b));
            assert!(
                a.resources
                    .iter()
                    .all(|r| (1..=2).contains(&r.view.quantity))
            );
            bounds.insert(a.resources[0].view.quantity);
            seen_random |= a.resources.len() == 2;
            seen_none |= a.resources.len() == 1;
            if chance == 0.0 {
                assert_eq!(a.resources.len(), 1);
            }
            if chance == 1.0 {
                assert_eq!(a.resources.len(), 2);
            }
        }
        assert_eq!(bounds.len(), 2);
        if chance == 0.3 {
            assert!(seen_random && seen_none);
        }
    }
}

#[test]
fn ground_drops_use_full_footprint_and_nearby_fallback() {
    let mut c = flat();
    c.pve_mut().chest_spawns[0].position = [16.0, 16.0];
    let mut used_fallback = false;
    for seed in 1..=64 {
        let mut w = world(&c, seed);
        w.action(&action(&c, 1), "open", &c, 30).unwrap();
        for r in &w.resources {
            assert!(w.pve.as_ref().unwrap().grid.clear(r.view.position, 16.0, 0));
            assert!(distance(w.position, r.view.position) <= 75.0);
            used_fallback |= r.view.position == w.position;
        }
    }
    assert!(used_fallback);
}

#[test]
fn full_ground_rolls_back_box_rng_and_all_drops_before_retry() {
    let c = config();
    let mut w = world(&c, 7);
    let mut fresh = world(&c, 7);
    for i in 0..50 {
        w.resources.push(Resource {
            view: ResourceView {
                id: format!("existing-{i}"),
                position: w.position,
                quantity: 1,
                orb: false,
            },
            hp: 0,
            spirit: 0,
        });
    }
    let rng = w.rng;
    assert_eq!(
        w.action(&action(&c, 1), "full", &c, 30),
        Err(Error::Capacity)
    );
    assert_eq!(w.rng, rng);
    assert_eq!(w.resources.len(), 50);
    assert!(!w.snapshot(&c, 30).pve.unwrap().chests[0].opened);
    w.resources.clear();
    w.action(&action(&c, 1), "retry", &c, 60).unwrap();
    fresh.action(&action(&c, 1), "open", &c, 60).unwrap();
    assert_eq!(drops(&w), drops(&fresh));
}

#[test]
fn full_inventory_and_stale_revision_keep_the_original_ground_stack() {
    let mut c = config();
    c.inventory_capacity = 1;
    let mut w = world(&c, 7);
    w.action(&action(&c, 1), "open", &c, 30).unwrap();
    let original = drops(&w);
    let id = w.resources[0].view.id.clone();
    let pick = |revision| Action::Pickup {
        resource_id: id.clone(),
        expected_revision: revision,
    };
    assert_eq!(
        w.action(&pick(0), "old", &c, 30),
        Err(Error::RevisionConflict)
    );
    assert_eq!(w.action(&pick(1), "full", &c, 30), Err(Error::Capacity));
    assert_eq!(drops(&w), original);
    c.inventory_capacity = 2;
    w.action(&pick(1), "pick", &c, 30).unwrap();
    assert_eq!(w.action(&pick(2), "again", &c, 30), Err(Error::NoResource));
    assert_eq!(
        w.pve.as_ref().unwrap().items["huo-ling"],
        original[0].quantity
    );
}

#[test]
fn only_picked_items_enter_extraction_or_death_settlement() {
    let c = config();
    for outcome in [Outcome::Extraction, Outcome::Death] {
        let mut w = world(&c, 7);
        w.kills = 3;
        w.action(&action(&c, 1), "open", &c, 30).unwrap();
        let expected = w.resources[0].view.quantity;
        let id = w.resources[0].view.id.clone();
        w.action(
            &Action::Pickup {
                resource_id: id,
                expected_revision: 1,
            },
            "pick",
            &c,
            30,
        )
        .unwrap();
        w.desired_outcome = Some(outcome);
        let Business::CommitSettlement {
            carried,
            looted,
            kept,
            dropped,
            cultivation,
            items,
            ..
        } = w.settlement(&c)
        else {
            panic!()
        };
        assert_eq!(carried + looted, kept + dropped);
        assert_eq!(
            cultivation,
            if outcome == Outcome::Extraction {
                45
            } else {
                11
            }
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].looted, expected);
        assert_eq!(items[0].kept + items[0].dropped, expected);
    }
    let mut w = world(&c, 7);
    w.action(&action(&c, 1), "open", &c, 30).unwrap();
    w.desired_outcome = Some(Outcome::Extraction);
    let Business::CommitSettlement { items, looted, .. } = w.settlement(&c) else {
        panic!()
    };
    assert!(items.is_empty());
    assert_eq!(looted, 0);
}

#[test]
fn respawn_waits_full_delay_and_new_generation_rejects_old_requests() {
    let c = config();
    let mut w = world(&c, 7);
    w.action(&action(&c, 1), "open", &c, 30).unwrap();
    let old = drops(&w);
    assert!(
        !w.tick(&c, 300029)
            .iter()
            .any(|e| matches!(e, Event::ChestRespawned { .. }))
    );
    assert!(w.snapshot(&c, 300029).pve.unwrap().chests[0].opened);
    let events = w.tick(&c, 300030);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::ChestRespawned { .. }))
            .count(),
        1
    );
    assert!(
        w.tick(&c, 300031)
            .iter()
            .all(|e| !matches!(e, Event::ChestRespawned { .. }))
    );
    assert_eq!(
        w.action(&action(&c, 1), "stale", &c, 300031),
        Err(Error::RevisionConflict)
    );
    let events = w.action(&action(&c, 2), "new", &c, 300031).unwrap();
    let Event::ChestOpened { resource_ids, .. } = &events[0] else {
        panic!()
    };
    assert!(
        resource_ids
            .iter()
            .all(|id| old.iter().all(|prior| &prior.id != id))
    );
    let ids: std::collections::BTreeSet<_> = w.resources.iter().map(|r| &r.view.id).collect();
    assert_eq!(ids.len(), w.resources.len());
    // The existing five-second sweep may retain old ground stacks briefly across respawn.
    w.tick(&c, 305030);
    assert!(
        w.resources
            .iter()
            .all(|r| old.iter().all(|prior| prior.id != r.view.id))
    );
    assert_eq!(
        w.snapshot(&c, 300031).pve.unwrap().chests[0].respawn_at_ms,
        Some(600031)
    );
}

#[test]
fn terminal_world_does_not_respawn_or_generate_loot() {
    let c = config();
    let mut w = world(&c, 7);
    w.action(&action(&c, 1), "open", &c, 30).unwrap();
    w.action(&Action::Leave {}, "leave", &c, 60).unwrap();
    let original = drops(&w);
    assert!(w.tick(&c, 600000).is_empty());
    assert_eq!(drops(&w), original);
    assert_eq!(w.snapshot(&c, 600000).pve.unwrap().chests[0].generation, 1);
    assert!(w.action(&action(&c, 2), "closed", &c, 600000).is_err());
}

#[test]
fn opening_stops_existing_navigation_and_movement() {
    let c = flat();
    let mut w = world(&c, 7);
    w.action(
        &Action::Navigate {
            destination: [600.0, 400.0],
        },
        "walk",
        &c,
        0,
    )
    .unwrap();
    w.movement = [1.0, 0.0];
    w.action(&action(&c, 1), "open", &c, 30).unwrap();
    let position = w.position;
    w.tick(&c, 60);
    assert_eq!(w.position, position);
    assert!(w.pve.as_ref().unwrap().player_path.is_empty());
}
