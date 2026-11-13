use super::*;
fn config() -> Config {
    Config::pve()
}

#[test]
fn result_policy_controls_pve_multitem_loss_and_keeps_consumption_evidence() {
    for policy in [SettlementPolicy::Preserve, SettlementPolicy::Attrition] {
        for trigger in [Outcome::Extraction, Outcome::Death] {
            let mut c = crate::config::default_catalog()
                .active("b3-10")
                .unwrap()
                .as_ref()
                .clone();
            c.schema_version = 4;
            c.asset_contract = Some(crate::asset_contract::AssetContract {
                entry_cost_asset_id: "admission-token".into(),
                reward_asset_id: "progress-xp".into(),
            });
            c.result_contract = Some(crate::result_contract::ResultContract {
                extraction: MatchResult {
                    result_id: "secured".into(),
                    policy,
                },
                death: MatchResult {
                    result_id: "ended".into(),
                    policy,
                },
            });
            let cost = c
                .pve
                .as_ref()
                .unwrap()
                .map_objective
                .as_ref()
                .unwrap()
                .cost
                .clone();
            let other = c
                .pve
                .as_ref()
                .unwrap()
                .items
                .keys()
                .find(|id| *id != &c.item_id && *id != &cost.item_id)
                .unwrap()
                .clone();
            let mut w = world(&c);
            w.looted = 10;
            w.inventory = c.carried_quantity + w.looted;
            let arena = w.pve.as_mut().unwrap();
            arena.attuned = true;
            arena.items.insert(other, 5);
            if cost.item_id == c.item_id {
                w.inventory -= cost.quantity;
            } else {
                arena.items.insert(cost.item_id.clone(), 5);
            }
            w.desired_outcome = Some(trigger);
            let intent = w.settlement(&c);
            let wire = c.encode_business(&intent);
            assert_eq!(c.decode_business(&wire).unwrap(), intent);
            let Business::CommitAssets {
                transaction: AssetTransaction::ResultSettlement { summary, .. },
            } = wire
            else {
                panic!()
            };
            assert_eq!(summary.result.policy, policy);
            assert_eq!(
                summary.map_progress.as_ref().unwrap().consumed,
                vec![cost.clone()]
            );
            assert!(summary.inventory.len() >= 2);
            for row in &summary.inventory {
                let consumed = if row.item_id == cost.item_id {
                    cost.quantity
                } else {
                    0
                };
                assert_eq!(row.carried + row.looted, row.kept + row.dropped + consumed);
            }
            let dropped: u32 = summary.inventory.iter().map(|r| r.dropped).sum();
            if policy == SettlementPolicy::Preserve {
                assert_eq!(dropped, 0);
            } else {
                assert!(dropped > 0);
            }
        }
    }
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
    w.ready = true;
    w.joined = true;
    w.protected = false;
    w.next_regen = 1000000;
    w
}
fn open_config() -> Config {
    let mut c = config();
    c.spawn = [400.0, 200.0];
    c.extraction_point = c.spawn;
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.monster_positions = vec![[200.0, 200.0]];
    c
}
#[test]
fn source_grid_and_all_authored_points_are_valid_and_connected() {
    let c = config();
    let a = Arena::new(c.pve.as_ref().unwrap());
    assert_eq!(a.grid.cells.len(), 256 * 256);
    assert_eq!(a.monsters.len(), 3);
    for p in [c.spawn, c.extraction_point]
        .into_iter()
        .chain(a.monsters.iter().map(|m| m.home))
    {
        assert!(a.grid.clear(p, 16.0, 0), "invalid point {p:?}");
        assert!(!a.grid.route(c.spawn, p, 16.0).is_empty(), "no path {p:?}");
    }
}
#[test]
fn navigation_has_no_corner_cutting_or_overshoot_on_real_forest_route() {
    let c = config();
    let mut w = world(&c);
    let goal = c.pve.as_ref().unwrap().monster_positions[2];
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.hp = 0;
    }
    w.action(&Action::Navigate { destination: goal }, "navigate", &c, 0)
        .unwrap();
    for now in (30..30000).step_by(30) {
        w.tick(&c, now);
        assert!(w.pve.as_ref().unwrap().grid.clear(w.position, 16.0, 0));
        if distance(w.position, goal) < 60.0 {
            return;
        }
    }
    panic!("navigation stuck {:?}", w.position);
}
#[test]
fn water_walkability_projectile_and_vision_are_independent() {
    let mut c = open_config();
    let p = c.pve_mut();
    p.cells_rle = vec![(4, 1024)];
    let grid = Grid::new(&std::sync::Arc::new(p.clone()));
    assert!(!grid.clear([400.0, 400.0], 14.0, 0));
    assert!(grid.clear([400.0, 400.0], 0.0, 1));
    assert!(grid.clear([400.0, 400.0], 0.0, 2));
    p.cells_rle = vec![(2, 1024)];
    let grid = Grid::new(&std::sync::Arc::new(p.clone()));
    assert!(!grid.clear([400.0, 400.0], 0.0, 1));
    assert!(!grid.clear([400.0, 400.0], 0.0, 2));
}
#[test]
fn no_path_and_nonfinite_destinations_reject_without_movement() {
    let mut c = open_config();
    c.pve_mut().cells_rle = vec![(2, 1024)];
    let mut w = world(&c);
    let before = w.position;
    assert!(
        w.action(
            &Action::Navigate {
                destination: [f64::NAN, 0.0]
            },
            "bad",
            &c,
            0
        )
        .is_err()
    );
    assert!(
        w.action(
            &Action::Navigate {
                destination: [500.0, 500.0]
            },
            "wall",
            &c,
            0
        )
        .is_err()
    );
    assert_eq!(w.position, before);
}
#[test]
fn terrain_multiplier_changes_world_speed_and_background_cancels_navigation() {
    let mut c = open_config();
    c.pve_mut().monster_positions.clear();
    c.pve_mut().cells_rle = vec![(8, 1024)];
    let mut w = world(&c);
    w.action(
        &Action::Move {
            direction: [1.0, 0.0],
        },
        "move",
        &c,
        0,
    )
    .unwrap();
    w.tick(&c, 30);
    assert!((w.position[0] - 400.0 - 5.94).abs() < 1e-6);
    w.action(
        &Action::Navigate {
            destination: [700.0, 200.0],
        },
        "go",
        &c,
        30,
    )
    .unwrap();
    w.action(&Action::Background {}, "bg", &c, 30).unwrap();
    assert!(w.pve.as_ref().unwrap().player_path.is_empty());
}
#[test]
fn sword_windup_consumes_once_and_projects_only_after_commit() {
    let c = open_config();
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    a.monsters[0].casting = Some((0, 120));
    w.tick(&c, 90);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 82);
    w.tick(&c, 120);
    let a = w.pve.as_ref().unwrap();
    assert_eq!(a.monsters[0].spirit, 72);
    assert_eq!(a.monsters[0].cooldown[0], 10120);
    assert_eq!(a.shots.len(), 1);
    assert_eq!(w.hp, 1000);
    for now in (150..700).step_by(30) {
        w.tick(&c, now);
    }
    assert!(w.hp < 1000);
    assert_eq!(w.pve.as_ref().unwrap().marks, 1);
}
#[test]
fn slot_order_prefers_eligible_sword_before_low_hp_regrowth() {
    let c = open_config();
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    a.monsters[0].hp = 20;
    a.monsters[0].aggro = true;
    w.tick(&c, 30);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].casting, Some((0, 150)));
}
#[test]
fn regrowth_commits_a_frozen_twenty_hp_budget_in_three_ticks() {
    let c = open_config();
    let mut w = world(&c);
    w.protected = true;
    let a = w.pve.as_mut().unwrap();
    a.monsters[0].hp = 20;
    a.monsters[0].casting = Some((1, 120));
    w.tick(&c, 120);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 57);
    for (now, expected) in [(719, 20), (720, 27), (1320, 34), (1920, 40)] {
        w.tick(&c, now);
        assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, expected);
    }
    assert!(w.pve.as_ref().unwrap().monsters[0].heal.is_none());
}
#[test]
fn lifesteal_uses_actual_hp_damage_and_marks_expire() {
    let c = open_config();
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    a.monsters[0].hp = 20;
    w.hp = 1;
    a.hurt_player(&mut w, &c, 0, (10000, true), 0, &mut vec![]);
    assert_eq!(a.monsters[0].hp, 20);
    assert_eq!(a.marks, 0);
    w.hp = 1000;
    w.desired_outcome = None;
    for _ in 0..5 {
        a.hurt_player(&mut w, &c, 0, (16, true), 30, &mut vec![]);
    }
    assert_eq!(a.marks, 3);
    a.step(&mut w, &c, 24030, &mut vec![]);
    assert_eq!(a.marks, 0);
}
#[test]
fn return_immunity_reactivation_and_home_restoration_are_distinct() {
    let c = open_config();
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    a.monsters[0].returning = true;
    a.monsters[0].hp = 40;
    w.position = [1000.0, 200.0];
    a.monsters[0].position = [1000.0, 200.0];
    a.hurt_monster(&mut w, &c, 0, 10, false, &mut vec![]);
    assert_eq!(a.monsters[0].hp, 40);
    w.position = [400.0, 200.0];
    a.monsters[0].position = [300.0, 200.0];
    std::sync::Arc::make_mut(&mut a.grid.config)
        .stats
        .insert("dodge".into(), 0);
    a.hurt_monster(&mut w, &c, 0, 10, false, &mut vec![]);
    assert!(a.monsters[0].hp < 40 && !a.monsters[0].returning && a.monsters[0].aggro);
    a.monsters[0].returning = true;
    a.monsters[0].aggro = false;
    w.protected = true;
    a.monsters[0].position = a.monsters[0].home;
    a.step(&mut w, &c, 30, &mut vec![]);
    assert_eq!(a.monsters[0].hp, 82);
    assert!(!a.monsters[0].aggro);
}
#[test]
fn a_kill_cancels_uncommitted_monster_actions_and_creates_unique_drops() {
    let c = open_config();
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    std::sync::Arc::make_mut(&mut a.grid.config)
        .stats
        .insert("dodge".into(), 0);
    a.monsters[0].casting = Some((1, 120));
    let mut events = vec![];
    a.hurt_monster(&mut w, &c, 0, 100, false, &mut events);
    assert!(a.monsters[0].casting.is_none() && a.monsters[0].heal.is_none());
    assert_eq!(w.kills, 1);
    assert_eq!(w.resources.iter().filter(|r| r.view.orb).count(), 1);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::MonsterKilled { .. }))
            .count(),
        1
    );
}
#[test]
fn drop_sequences_replay_and_each_probability_uses_source_item_and_quantity_bounds() {
    let c = config();
    let run = |seed| {
        let mut w = world(&c);
        w.rng = seed;
        let mut a = w.pve.take().unwrap();
        for i in 0..3 {
            a.drop(&mut w, &c, i);
        }
        for r in &w.resources {
            assert!(a.grid.clear(r.view.position, 2.0, 0));
            if !r.view.orb {
                let item = &a.resource_items[&r.view.id];
                let max = if item == "xue-zhi" || item == "huo-ling" {
                    2
                } else {
                    1
                };
                assert!((1..=max).contains(&r.view.quantity));
            }
        }
        w.resources.into_iter().map(|r| r.view).collect::<Vec<_>>()
    };
    assert_eq!(run(123), run(123));
    assert_ne!(run(123), run(124));
    for seed in 1..100 {
        run(seed);
    }
}
#[test]
fn full_bag_does_not_consume_non_iron_drop_and_success_keeps_its_identity() {
    let mut c = open_config();
    c.inventory_capacity = 1;
    let mut w = world(&c);
    w.position = [200.0, 200.0];
    let a = w.pve.as_mut().unwrap();
    a.resource_items.insert("material".into(), "xue-zhi".into());
    w.resources.push(Resource {
        view: ResourceView {
            id: "material".into(),
            position: w.position,
            quantity: 2,
            orb: false,
        },
        hp: 0,
        spirit: 0,
    });
    let action = Action::Pickup {
        resource_id: "material".into(),
        expected_revision: 1,
    };
    assert_eq!(w.action(&action, "full", &c, 0), Err(Error::Capacity));
    assert_eq!(w.resources.len(), 1);
    c.inventory_capacity = 24;
    w.action(&action, "ok", &c, 0).unwrap();
    assert_eq!(w.inventory, 5);
    assert_eq!(w.pve.as_ref().unwrap().items["xue-zhi"], 2);
    assert!(w.resources.is_empty());
}
#[test]
fn death_and_extraction_conserve_each_asset_and_count_three_kills() {
    for outcome in [Outcome::Death, Outcome::Extraction] {
        let c = open_config();
        let mut w = world(&c);
        w.pve.as_mut().unwrap().items.insert("xue-zhi".into(), 4);
        w.pve.as_mut().unwrap().items.insert("huo-ling".into(), 3);
        w.kills = 3;
        w.looted = 2;
        w.inventory = 7;
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
        for item in items {
            assert_eq!(item.looted, item.kept + item.dropped);
            if outcome == Outcome::Extraction {
                assert_eq!(item.dropped, 0);
            }
        }
        assert_eq!(
            cultivation,
            if outcome == Outcome::Extraction {
                45
            } else {
                11
            }
        );
    }
}

#[test]
fn one_time_drops_expire_after_five_minutes_and_do_not_respawn() {
    let c = open_config();
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    a.drop(&mut w, &c, 0);
    let count = w.resources.len();
    assert!(count > 0);
    w.protected = true;
    a.step(&mut w, &c, 299999, &mut vec![]);
    assert_eq!(w.resources.len(), count);
    a.step(&mut w, &c, 305000, &mut vec![]);
    assert!(w.resources.is_empty());
    a.step(&mut w, &c, 610000, &mut vec![]);
    assert!(w.resources.is_empty());
}
#[test]
fn profile_selection_preserves_recovery_and_rejects_bad_config_without_initialization() {
    use crate::engine::Engine;
    let c = config();
    let create:Create=serde_json::from_value(serde_json::json!({"protocol_version":1,"key":{"id":"pve","incarnation":1},"request_id":"create","config_id":c.id,"config_hash":c.hash(),"seed":7,"participant_id":"A","authenticated_principal":"principal-A","connection":{"id":"web","generation":1}})).unwrap();
    let mut engine = Engine::new(Config::frozen());
    let mut bad = create.clone();
    bad.config_hash = "bad".into();
    assert_eq!(engine.create(bad), Err(Error::ConfigMismatch));
    assert!(engine.drain().is_empty());
    engine.create(create.clone()).unwrap();
    let checkpoint = engine.drain().pop().unwrap().recovery.unwrap();
    let mut restored = Engine::new(Config::frozen());
    let mut request = create;
    request.recovery = Some(Recovery {
        generation: 1,
        checkpoint: *checkpoint,
    });
    assert_eq!(restored.create(request).unwrap().phase, Phase::Closed);
    assert_eq!(restored.live_worlds(), 0);
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn recovered_assets_are_bounded_by_source_drop_quantities_and_frozen_kill_score() {
    use crate::engine::Engine;
    use sha2::{Digest, Sha256};
    let c = config();
    let create:Create=serde_json::from_value(serde_json::json!({"protocol_version":1,"key":{"id":"pve-assets","incarnation":1},"request_id":"create","config_id":c.id,"config_hash":c.hash(),"seed":7,"participant_id":"A","authenticated_principal":"principal-A","connection":{"id":"web","generation":1}})).unwrap();
    let mut engine = Engine::new(Config::frozen());
    engine.create(create.clone()).unwrap();
    let mut checkpoint = *engine.drain().remove(0).recovery.unwrap();
    let mut facts = world(&c).facts;
    facts.revision = 0;
    checkpoint.state.operations[0].terminal = Some(ReceiptOutcome::Loaded { facts });
    checkpoint.state.operations.push(FrozenOperation {
        request: BusinessRequest {
            operation_id: "pve-assets:1:admit".into(),
            attempt_id: 1,
            body: Business::CommitAdmission {
                clock: None,
                expected_revision: 0,
                item_id: c.item_id.clone(),
                quantity: 5,
                entry_years: 1,
            },
        },
        terminal: Some(ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "tx-1".into(),
        }),
        refused: false,
        queried_attempt: None,
    });
    checkpoint.state.operations.push(FrozenOperation {
        request: BusinessRequest {
            operation_id: "pve-assets:1:settle".into(),
            attempt_id: 1,
            body: Business::CommitSettlement {
                reset_progress: None,
                scoring: None,
                map_progress: None,
                outcome: Outcome::Extraction,
                expected_revision: 1,
                item_id: c.item_id.clone(),
                carried: 5,
                looted: 0,
                kept: 5,
                dropped: 0,
                cultivation: 45,
                instances: vec![],
                rewards: vec![],
                progression: None,
                items: vec![ItemSettlement {
                    item_id: "pill_healing".into(),
                    looted: 3,
                    kept: 3,
                    dropped: 0,
                }],
            },
        },
        terminal: None,
        refused: false,
        queried_attempt: None,
    });
    for forged in [false, true] {
        let mut cp = checkpoint.clone();
        if forged
            && let Business::CommitSettlement { items, .. } =
                &mut cp.state.operations[2].request.body
        {
            items[0].looted = 4;
            items[0].kept = 4;
        }
        cp.sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&cp.state).unwrap())
        );
        let mut request = create.clone();
        request.recovery = Some(Recovery {
            generation: 1,
            checkpoint: cp,
        });
        let mut restored = Engine::new(Config::frozen());
        if forged {
            assert_eq!(restored.create(request), Err(Error::InvalidMessage));
            assert!(restored.drain().is_empty());
        } else {
            assert_eq!(restored.create(request).unwrap().phase, Phase::Closed);
            assert_eq!(restored.live_worlds(), 0);
        }
    }
}
