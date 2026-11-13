use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-10")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [200.0, 400.0];
    c.extraction_point = c.spawn;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    let p = c.pve_mut();
    p.width = 64;
    p.height = 64;
    p.cells_rle = vec![(0, 4096)];
    p.chest_spawns[0].position = [1200.0, 400.0];
    for (index, e) in p.encounters.iter_mut().enumerate() {
        e.trigger.center = [400.0 + index as f64 * 300.0, 400.0];
        e.trigger.radius = 80.0;
        for (i, m) in e
            .monsters
            .iter_mut()
            .chain(e.waves.iter_mut().flat_map(|w| &mut w.monsters))
            .enumerate()
        {
            m.position = [400.0 + index as f64 * 300.0 + i as f64 * 60.0, 500.0];
        }
    }
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
    w.ready = true;
    w.foreground = true;
    w.joined = true;
    w.protected = false;
    w.active_at = Some(0);
    w
}
fn finish(w: &mut World, c: &Config) {
    let mut a = w.pve.take().unwrap();
    for index in 0..a.encounters.len() {
        w.position = a.grid.config.encounters[index].trigger.center;
        for n in 0..a.grid.config.encounters[index].group_count() {
            let now = 100 + index as u64 * 10000 + n as u64 * 2000;
            a.prepare_encounters(w, now, &mut vec![]);
            let ids = a.grid.config.encounters[index].group(n).1.to_vec();
            for id in ids {
                let i = a.monsters.iter().position(|m| m.id == id).unwrap();
                assert!(a.monsters[i].enabled);
                a.hurt_monster(w, c, i, 100000, false, &mut vec![]);
            }
            a.now = now;
            a.advance_encounters(w, &mut vec![]);
        }
    }
    w.position = a.chests[0].position;
    a.items.insert("huo-ling".into(), 2);
    w.pve = Some(a);
}
fn start(w: &mut World, c: &Config, now: u64) -> Result<Vec<Event>> {
    w.action(
        &Action::AttuneObjective {
            objective_id: "misty-sanctum".into(),
            expected_revision: w.inventory_revision,
        },
        "attune",
        c,
        now,
    )
}
fn progress(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut events = vec![];
    a.advance_attunement(w, c, now, &mut events);
    w.pve = Some(a);
    events
}
fn summary(w: &World) -> MapProgressSummary {
    w.pve.as_ref().unwrap().map_summary().unwrap()
}
#[test]
fn objective_locked_dependencies_do_not_trigger() {
    let c = config();
    let mut w = world(&c);
    w.position = [1000.0, 400.0];
    let mut a = w.pve.take().unwrap();
    a.prepare_encounters(&w, 30, &mut vec![]);
    assert!(
        a.encounters
            .iter()
            .all(|e| e.phase == EncounterPhase::Dormant)
    );
    assert_eq!(a.encounter_views()[2].blocked_by, vec!["ruins"]);
}
#[test]
fn objective_all_dependencies_and_forward_order() {
    let mut c = config();
    let p = c.pve_mut();
    p.encounters[2].requires = vec!["wetland".into(), "ruins".into()];
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    a.encounters[0].phase = EncounterPhase::Completed;
    assert_eq!(a.blocked_by(&a.grid.config.encounters[2]), vec!["ruins"]);
    a.encounters[1].phase = EncounterPhase::Completed;
    assert!(a.blocked_by(&a.grid.config.encounters[2]).is_empty());
}
#[test]
fn objective_chain_completes_real_required_deaths_and_waves() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    assert_eq!(
        summary(&w).completed_encounter_ids,
        vec!["ruins", "sanctum", "wetland"]
    );
    assert!(!summary(&w).attuned);
    assert_eq!(w.kills, 7);
    assert!(c.valid_map_progress(
        Some(&summary(&w)),
        Some(&c.score_summary(&w.pve.as_ref().unwrap().score_tally))
    ));
}
#[test]
fn objective_start_requires_complete_chain() {
    let c = config();
    let mut w = world(&c);
    assert_eq!(start(&mut w, &c, 30), Err(Error::EncounterLocked));
}
#[test]
fn objective_revision_and_identity_rejected() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    assert_eq!(
        w.action(
            &Action::AttuneObjective {
                objective_id: "bad".into(),
                expected_revision: 1
            },
            "x",
            &c,
            30
        ),
        Err(Error::NoResource)
    );
    assert_eq!(
        w.action(
            &Action::AttuneObjective {
                objective_id: "misty-sanctum".into(),
                expected_revision: 99
            },
            "x",
            &c,
            30
        ),
        Err(Error::RevisionConflict)
    );
}
#[test]
fn objective_insufficient_cost_and_distance() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    w.pve.as_mut().unwrap().items.insert("huo-ling".into(), 1);
    assert_eq!(start(&mut w, &c, 30), Err(Error::NoResource));
    w.position = [200.0, 200.0];
    assert_eq!(start(&mut w, &c, 30), Err(Error::OutOfRange));
}
#[test]
fn objective_success_deducts_only_at_deadline_once() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    let revision = w.inventory_revision;
    start(&mut w, &c, 100).unwrap();
    assert!(progress(&mut w, &c, 2099).is_empty());
    assert_eq!(w.pve.as_ref().unwrap().items["huo-ling"], 2);
    let e = progress(&mut w, &c, 2100);
    assert!(matches!(&e[..], [Event::AttunementCompleted { .. }]));
    assert_eq!(w.inventory_revision, revision + 1);
    assert_eq!(w.pve.as_ref().unwrap().items["huo-ling"], 0);
    assert!(progress(&mut w, &c, 10000).is_empty());
    assert_eq!(start(&mut w, &c, 10000), Err(Error::InvalidState));
}
#[test]
fn objective_free_and_root_item_costs() {
    for (item, q) in [("huo-ling", 0), ("xuan-tie", 5)] {
        let mut c = config();
        c.pve_mut().map_objective.as_mut().unwrap().cost = ItemAmount {
            item_id: item.into(),
            quantity: q,
        };
        let mut w = world(&c);
        finish(&mut w, &c);
        start(&mut w, &c, 100).unwrap();
        progress(&mut w, &c, 2100);
        assert_eq!(
            summary(&w).consumed,
            if q == 0 {
                vec![]
            } else {
                vec![ItemAmount {
                    item_id: item.into(),
                    quantity: q,
                }]
            }
        );
        if q > 0 {
            assert_eq!(w.inventory, 0);
        }
    }
}
#[test]
fn objective_duplicate_start_and_conflicting_actions() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    start(&mut w, &c, 100).unwrap();
    assert_eq!(start(&mut w, &c, 200), Err(Error::InvalidState));
    for action in [
        Action::Attack {},
        Action::ExtractStart {},
        Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1.0, 0.0],
        },
        Action::Pickup {
            resource_id: "x".into(),
            expected_revision: 1,
        },
        Action::OpenChest {
            chest_id: "sanctum-cache".into(),
            expected_generation: 1,
        },
    ] {
        assert_eq!(
            w.action(&action, "blocked", &c, 300),
            Err(Error::InvalidState)
        );
    }
}
#[test]
fn objective_movement_navigation_cancel_and_stop() {
    for action in [
        Action::Move {
            direction: [1.0, 0.0],
        },
        Action::Navigate {
            destination: [1100.0, 400.0],
        },
        Action::CancelAttunement {},
        Action::CancelAction {},
    ] {
        let c = config();
        let mut w = world(&c);
        finish(&mut w, &c);
        start(&mut w, &c, 100).unwrap();
        let e = w.action(&action, "cancel", &c, 200).unwrap();
        assert!(
            e.iter()
                .any(|e| matches!(e, Event::AttunementInterrupted { .. }))
        );
        progress(&mut w, &c, 2200);
        assert!(!summary(&w).attuned);
        assert_eq!(w.pve.as_ref().unwrap().items["huo-ling"], 2);
    }
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    start(&mut w, &c, 100).unwrap();
    w.action(&Action::Stop {}, "stop", &c, 200).unwrap();
    progress(&mut w, &c, 2100);
    assert!(summary(&w).attuned);
}
#[test]
fn objective_damage_before_due_cancels_but_zero_damage_does_not() {
    for raw in [0, 50] {
        let mut c = config();
        c.params.minimum_damage = 0;
        let mut w = world(&c);
        finish(&mut w, &c);
        start(&mut w, &c, 100).unwrap();
        let mut a = w.pve.take().unwrap();
        a.hurt_player(&mut w, &c, 0, (raw, false), 2100, &mut vec![]);
        w.pve = Some(a);
        progress(&mut w, &c, 2100);
        assert_eq!(summary(&w).attuned, raw == 0);
    }
}
#[test]
fn objective_background_death_and_close_cancel_unpaid() {
    for reason in [
        ChestInterruptReason::Background,
        ChestInterruptReason::Closed,
    ] {
        let c = config();
        let mut w = world(&c);
        finish(&mut w, &c);
        start(&mut w, &c, 100).unwrap();
        let mut events = vec![];
        w.pve
            .as_mut()
            .unwrap()
            .interrupt_unlock(reason, &mut events);
        progress(&mut w, &c, 2100);
        assert!(!summary(&w).attuned);
        assert_eq!(events.len(), 1);
    }
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    start(&mut w, &c, 100).unwrap();
    assert_eq!(w.close_interaction_event_budget(), 1);
    let events = w.close_container_interaction(300);
    assert!(matches!(
        &events[..],
        [Event::AttunementInterrupted {
            reason: ChestInterruptReason::Closed,
            ..
        }]
    ));
}
#[test]
fn objective_protected_and_leaving_range_recheck() {
    for protected in [true, false] {
        let c = config();
        let mut w = world(&c);
        finish(&mut w, &c);
        start(&mut w, &c, 100).unwrap();
        if protected {
            w.protected = true;
        } else {
            w.position = [100.0, 100.0];
        }
        let e = progress(&mut w, &c, 2100);
        assert!(matches!(&e[..], [Event::AttunementInterrupted { .. }]));
        assert!(!summary(&w).attuned);
    }
}
#[test]
fn objective_missing_material_at_commit_no_partial_deduction() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    start(&mut w, &c, 100).unwrap();
    w.pve.as_mut().unwrap().items.insert("huo-ling".into(), 1);
    progress(&mut w, &c, 2100);
    assert_eq!(w.pve.as_ref().unwrap().items["huo-ling"], 1);
    assert!(!summary(&w).attuned);
}
#[test]
fn objective_open_is_locked_until_attuned_then_explicit_seal() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    let open = Action::OpenChest {
        chest_id: "sanctum-cache".into(),
        expected_generation: 1,
    };
    assert_eq!(w.action(&open, "open", &c, 50), Err(Error::EncounterLocked));
    start(&mut w, &c, 100).unwrap();
    progress(&mut w, &c, 2100);
    w.action(&open, "seal", &c, 2200).unwrap();
    assert!(!summary(&w).reward_opened);
    let mut a = w.pve.take().unwrap();
    a.advance_unlock(&w, 4200, &mut vec![]);
    w.pve = Some(a);
    w.action(&open, "open", &c, 4300).unwrap();
    assert!(summary(&w).reward_opened);
    assert_eq!(w.action(&open, "again", &c, 4400), Err(Error::NoResource));
    w.pve.as_mut().unwrap().refresh_chests(900000, &mut vec![]);
    assert!(summary(&w).reward_opened);
    assert_eq!(
        w.snapshot(&c, 900000).pve.unwrap().chests[0].respawn_at_ms,
        None
    );
}
#[test]
fn objective_consumed_only_item_preserved_in_extraction_and_death() {
    for outcome in [Outcome::Extraction, Outcome::Death] {
        let c = config();
        let mut w = world(&c);
        finish(&mut w, &c);
        start(&mut w, &c, 100).unwrap();
        progress(&mut w, &c, 2100);
        w.desired_outcome = Some(outcome);
        let Business::CommitSettlement {
            map_progress,
            items,
            ..
        } = w.settlement(&c)
        else {
            panic!()
        };
        let fire = items.iter().find(|i| i.item_id == "huo-ling").unwrap();
        assert_eq!((fire.looted, fire.kept, fire.dropped), (2, 0, 0));
        assert_eq!(map_progress.unwrap().consumed[0].quantity, 2);
    }
}
#[test]
fn objective_remaining_items_only_used_for_death_loss() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    w.pve.as_mut().unwrap().items.insert("huo-ling".into(), 7);
    start(&mut w, &c, 100).unwrap();
    progress(&mut w, &c, 2100);
    w.desired_outcome = Some(Outcome::Death);
    let Business::CommitSettlement {
        carried,
        looted,
        kept,
        dropped,
        items,
        ..
    } = w.settlement(&c)
    else {
        panic!()
    };
    assert_eq!(carried + looted, kept + dropped);
    let fire = items.iter().find(|i| i.item_id == "huo-ling").unwrap();
    assert_eq!(fire.looted, 7);
    assert_eq!(fire.kept + fire.dropped + 2, 7);
    assert!(c.valid_death_loss(&[("xuan-tie", 5, dropped), ("huo-ling", 5, fire.dropped)]));
}
#[test]
fn objective_root_cost_conservation() {
    let mut c = config();
    c.pve_mut().map_objective.as_mut().unwrap().cost = ItemAmount {
        item_id: c.item_id.clone(),
        quantity: 3,
    };
    let mut w = world(&c);
    finish(&mut w, &c);
    start(&mut w, &c, 100).unwrap();
    progress(&mut w, &c, 2100);
    w.desired_outcome = Some(Outcome::Extraction);
    let Business::CommitSettlement {
        carried,
        looted,
        kept,
        dropped,
        map_progress,
        ..
    } = w.settlement(&c)
    else {
        panic!()
    };
    assert_eq!(
        carried + looted,
        kept + dropped + map_progress.unwrap().consumed[0].quantity
    );
}
#[test]
fn objective_early_exit_summary_does_not_claim_completion() {
    let c = config();
    let mut w = world(&c);
    w.desired_outcome = Some(Outcome::Death);
    let Business::CommitSettlement {
        map_progress,
        scoring,
        ..
    } = w.settlement(&c)
    else {
        panic!()
    };
    let s = map_progress.unwrap();
    assert!(s.completed_encounter_ids.is_empty());
    assert!(s.consumed.is_empty());
    assert!(c.valid_map_progress(Some(&s), scoring.as_ref()));
}
#[test]
fn objective_cancel_retry_and_snapshot_countdown() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    start(&mut w, &c, 100).unwrap();
    assert_eq!(
        w.snapshot(&c, 200)
            .pve
            .unwrap()
            .map_objective
            .unwrap()
            .finishes_at_ms,
        Some(2100)
    );
    w.action(&Action::CancelAttunement {}, "cancel", &c, 300)
        .unwrap();
    start(&mut w, &c, 400).unwrap();
    progress(&mut w, &c, 2399);
    assert!(!summary(&w).attuned);
    progress(&mut w, &c, 2400);
    assert!(summary(&w).attuned);
}
#[test]
fn objective_attunement_range_boundary_and_wall_are_authoritative() {
    let mut c = config();
    let range = c
        .pve
        .as_ref()
        .unwrap()
        .treasure_rules
        .as_ref()
        .unwrap()
        .interaction_range;
    let mut w = world(&c);
    finish(&mut w, &c);
    w.position = [1200.0 - range, 400.0];
    start(&mut w, &c, 100).unwrap();
    w.action(&Action::CancelAttunement {}, "cancel", &c, 200)
        .unwrap();
    w.position[0] -= 0.01;
    assert_eq!(start(&mut w, &c, 300), Err(Error::OutOfRange));
    let p = c.pve_mut();
    let mut blocked = p.palette[0].clone();
    blocked.walkable = false;
    let tile = p.palette.len();
    p.palette.push(blocked);
    let mut cells = vec![0; 4096];
    cells[12 * 64 + 35] = tile;
    p.cells_rle = cells.into_iter().map(|v| (v, 1)).collect();
    let mut w = world(&c);
    finish(&mut w, &c);
    w.position = [1100.0, 400.0];
    assert_eq!(start(&mut w, &c, 100), Err(Error::OutOfRange));
}
#[test]
fn objective_actual_simulation_damage_wins_at_same_attune_deadline() {
    let c = config();
    let mut w = world(&c);
    finish(&mut w, &c);
    start(&mut w, &c, 100).unwrap();
    let mut a = w.pve.take().unwrap();
    let m = &mut a.monsters[0];
    m.hp = 10;
    m.position = [1160.0, 400.0];
    m.home = m.position;
    m.aggro = true;
    m.next_attack = 2100;
    m.next_decision = 100000;
    let mut events = vec![];
    a.step(&mut w, &c, 2100, &mut events);
    assert!(w.hp < 1000);
    assert!(!a.attuned);
    assert_eq!(a.items["huo-ling"], 2);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::AttunementInterrupted {
            reason: ChestInterruptReason::Damage,
            ..
        }
    )));
}
#[test]
fn objective_full_ground_does_not_refund_or_recharge_attunement() {
    let mut c = config();
    c.pve_mut().treasure_rules.as_mut().unwrap().ground_capacity = 1;
    let mut w = world(&c);
    finish(&mut w, &c);
    start(&mut w, &c, 100).unwrap();
    progress(&mut w, &c, 2100);
    let open = Action::OpenChest {
        chest_id: "sanctum-cache".into(),
        expected_generation: 1,
    };
    w.action(&open, "seal", &c, 2200).unwrap();
    let mut a = w.pve.take().unwrap();
    a.advance_unlock(&w, 4200, &mut vec![]);
    w.pve = Some(a);
    let rng = w.rng;
    assert_eq!(w.action(&open, "full", &c, 4300), Err(Error::Capacity));
    assert_eq!(w.rng, rng);
    assert!(summary(&w).attuned);
    assert!(!summary(&w).reward_opened);
    assert_eq!(w.pve.as_ref().unwrap().items["huo-ling"], 0);
}
