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
    c.params.minimum_damage = 0;
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.chest_spawns[0].position = [900.0, 400.0];
    for k in ["atk", "spd", "crit", "dodge"] {
        p.stats.insert(k.into(), 0);
    }
    p.stats.insert("hp".into(), 60);
    p.combat.cast_chance = 0.0;
    for t in p.additional_templates.values_mut() {
        t.stats = p.stats.clone();
        t.combat.cast_chance = 0.0;
        t.area = None;
        if let Some(b) = &mut t.boss {
            for stage in &mut b.phases {
                stage.cast_chance = 0.0;
            }
        }
    }
    for e in &mut p.encounters {
        e.trigger.center = [500.0, 400.0];
        e.trigger.radius = 100.0;
        for (i, m) in e
            .monsters
            .iter_mut()
            .chain(e.waves.iter_mut().flat_map(|w| &mut w.monsters))
            .enumerate()
        {
            m.position = [600.0 + i as f64 * 100.0, 400.0];
        }
    }
    c
}
fn harness(c: Config) -> Harness {
    let mut h = Harness::new();
    h.engine = Engine::new(c.clone());
    let mut r = create_request();
    r.config_id = c.id().into();
    r.config_hash = c.content_hash().into();
    h.engine.create(r).unwrap();
    h.pump().unwrap();
    h.action(Action::Join {}).unwrap();
    h.pump().unwrap();
    h.advance(30).unwrap();
    h
}
fn drive(h: &mut Harness) {
    h.action(Action::Navigate {
        destination: [500.0, 400.0],
    })
    .unwrap();
    h.advance(2000).unwrap();
    for _ in 0..80 {
        if h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .encounters
            .iter()
            .all(|e| e.phase == EncounterPhase::Completed)
        {
            break;
        }
        let target = h.snapshot.as_ref().unwrap().target.clone();
        if let Some(target) = target {
            h.action(Action::Navigate {
                destination: target.position,
            })
            .unwrap();
            h.advance(1500).unwrap();
            h.action(Action::Stop {}).unwrap();
            match h.action(Action::Attack {}) {
                Ok(_) | Err(Error::InvalidState | Error::NoResource | Error::OutOfRange) => {}
                e => panic!("{e:?}"),
            };
            h.advance(900).unwrap();
        } else {
            h.action(Action::Navigate {
                destination: [500.0, 400.0],
            })
            .unwrap();
            h.advance(1500).unwrap();
        }
    }
    assert!(
        h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .encounters
            .iter()
            .all(|e| e.phase == EncounterPhase::Completed),
        "{}",
        serde_json::to_string(h.snapshot.as_ref().unwrap()).unwrap()
    );
    for _ in 0..80 {
        let Some(resource) = h.snapshot.as_ref().unwrap().resources.first().cloned() else {
            break;
        };
        h.action(Action::Navigate {
            destination: resource.position,
        })
        .unwrap();
        h.advance(2000).unwrap();
        h.action(Action::Stop {}).unwrap();
        let revision = h.snapshot.as_ref().unwrap().inventory_revision;
        h.action(Action::Pickup {
            resource_id: resource.id,
            expected_revision: revision,
        })
        .unwrap();
        h.pump().unwrap();
    }
    h.action(Action::Navigate {
        destination: [900.0, 400.0],
    })
    .unwrap();
    h.advance(3000).unwrap();
    h.action(Action::Stop {}).unwrap();
}
fn attune(h: &mut Harness) {
    let revision = h.snapshot.as_ref().unwrap().inventory_revision;
    h.action(Action::AttuneObjective {
        objective_id: "misty-sanctum".into(),
        expected_revision: revision,
    })
    .unwrap();
    h.pump().unwrap();
    h.advance(2100).unwrap();
    assert!(
        h.snapshot
            .as_ref()
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .map_objective
            .as_ref()
            .unwrap()
            .attuned
    );
}
fn checkpoint(h: &mut Harness) -> IoCheckpoint {
    h.action(Action::Leave {}).unwrap();
    h.engine
        .drain()
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone()
}
fn settle(cp: &mut IoCheckpoint) -> &mut Business {
    &mut cp
        .state
        .operations
        .iter_mut()
        .find(|o| matches!(o.request.body, Business::CommitSettlement { .. }))
        .unwrap()
        .request
        .body
}
fn recover(c: Config, mut cp: IoCheckpoint) -> Result<Engine> {
    use sha2::{Digest, Sha256};
    cp.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&cp.state).unwrap())
    );
    let mut r = create_request();
    r.config_id = c.id().into();
    r.config_hash = c.content_hash().into();
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    let mut e = Engine::new(c);
    e.create(r)?;
    assert_eq!(e.live_worlds(), 0);
    Ok(e)
}
#[test]
fn objective_public_inputs_freeze_consumption_and_restore_exact_body() {
    let c = config();
    let mut h = harness(c.clone());
    drive(&mut h);
    attune(&mut h);
    let cp = checkpoint(&mut h);
    let mut copy = cp.clone();
    let b = settle(&mut copy).clone();
    assert!(
        matches!(&b,Business::CommitSettlement{map_progress:Some(s),..}if s.attuned && s.consumed[0].quantity==2)
    );
    let mut e = recover(c, cp).unwrap();
    assert!(
        e.drain()
            .iter()
            .filter_map(|o| o.recovery.as_deref())
            .any(|cp| cp.state.operations.iter().any(|o| o.request.body == b))
    );
}
#[test]
fn objective_recovery_rejects_missing_and_forged_cost_summary() {
    let c = config();
    let mut h = harness(c.clone());
    drive(&mut h);
    attune(&mut h);
    let cp = checkpoint(&mut h);
    for change in 0..6 {
        let mut copy = cp.clone();
        if let Business::CommitSettlement { map_progress, .. } = settle(&mut copy) {
            match change {
                0 => *map_progress = None,
                1 => map_progress.as_mut().unwrap().consumed[0].quantity = 1,
                2 => map_progress.as_mut().unwrap().consumed.clear(),
                3 => map_progress
                    .as_mut()
                    .unwrap()
                    .completed_encounter_ids
                    .reverse(),
                4 => {
                    map_progress.as_mut().unwrap().completed_encounter_ids = vec!["sanctum".into()]
                }
                _ => map_progress.as_mut().unwrap().attuned = false,
            }
        }
        assert!(matches!(
            recover(c.clone(), copy),
            Err(Error::InvalidMessage)
        ));
    }
}
#[test]
fn objective_recovery_rejects_unearned_reward_and_missing_item_row() {
    let c = config();
    let mut h = harness(c.clone());
    drive(&mut h);
    let cp = checkpoint(&mut h);
    let mut copy = cp.clone();
    if let Business::CommitSettlement { map_progress, .. } = settle(&mut copy) {
        map_progress.as_mut().unwrap().reward_opened = true;
    }
    assert!(matches!(
        recover(c.clone(), copy),
        Err(Error::InvalidMessage)
    ));
    let mut h = harness(c.clone());
    drive(&mut h);
    attune(&mut h);
    let mut cp = checkpoint(&mut h);
    if let Business::CommitSettlement { items, .. } = settle(&mut cp) {
        items.retain(|i| i.item_id != "huo-ling");
    }
    assert!(matches!(recover(c, cp), Err(Error::InvalidMessage)));
}
#[test]
fn objective_interrupted_unpaid_state_recovers_without_consumption() {
    let c = config();
    let mut h = harness(c.clone());
    drive(&mut h);
    let revision = h.snapshot.as_ref().unwrap().inventory_revision;
    h.action(Action::AttuneObjective {
        objective_id: "misty-sanctum".into(),
        expected_revision: revision,
    })
    .unwrap();
    let cp = checkpoint(&mut h);
    let mut copy = cp.clone();
    assert!(
        matches!(settle(&mut copy),Business::CommitSettlement{map_progress:Some(s),..}if !s.attuned && s.consumed.is_empty())
    );
    recover(c, cp).unwrap();
}
#[test]
fn objective_recovery_requires_actual_kills_for_claimed_completed_nodes() {
    let c = config();
    let mut h = harness(c.clone());
    let mut cp = checkpoint(&mut h);
    if let Business::CommitSettlement { map_progress, .. } = settle(&mut cp) {
        map_progress.as_mut().unwrap().completed_encounter_ids =
            vec!["ruins".into(), "sanctum".into(), "wetland".into()];
    }
    assert!(matches!(recover(c, cp), Err(Error::InvalidMessage)));
}
#[test]
fn objective_reconnect_cancels_unpaid_cast_and_preserves_paid_cost() {
    let c = config();
    let mut h = harness(c);
    drive(&mut h);
    let revision = h.snapshot.as_ref().unwrap().inventory_revision;
    h.action(Action::AttuneObjective {
        objective_id: "misty-sanctum".into(),
        expected_revision: revision,
    })
    .unwrap();
    h.action(Action::Background {}).unwrap();
    h.pump().unwrap();
    h.advance(2400).unwrap();
    assert!(h.trace.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::AttunementInterrupted { .. }
        }
    )));
    h.action(Action::Foreground {}).unwrap();
    h.pump().unwrap();
    let publication_id = h
        .trace
        .iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap()
        .id
        .clone();
    h.action(Action::CombatReady { publication_id }).unwrap();
    h.advance(30).unwrap();
    attune(&mut h);
    let before = h
        .snapshot
        .as_ref()
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .inventory
        .clone();
    h.action(Action::Background {}).unwrap();
    h.action(Action::Foreground {}).unwrap();
    h.pump().unwrap();
    assert_eq!(
        h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().inventory,
        before
    );
}
#[test]
fn objective_duplicate_public_request_does_not_restart_or_charge_twice() {
    let c = config();
    let mut h = harness(c);
    drive(&mut h);
    let r = create_request();
    h.sequence += 1;
    let input = Input {
        key: r.key,
        participant_id: r.participant_id,
        entity_generation: 1,
        connection: r.connection,
        request_id: "attune-once".into(),
        sequence: h.sequence,
        action: Action::AttuneObjective {
            objective_id: "misty-sanctum".into(),
            expected_revision: h.snapshot.as_ref().unwrap().inventory_revision,
        },
    };
    let first = h.engine.input(input.clone()).unwrap();
    h.pump().unwrap();
    h.advance(2100).unwrap();
    assert_eq!(h.engine.input(input).unwrap(), first);
    h.pump().unwrap();
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::AttunementCompleted { .. }
                }
            ))
            .count(),
        1
    );
    let cp = checkpoint(&mut h);
    recover(config(), cp).unwrap();
}
#[test]
fn objective_root_material_cost_restores_net_inventory_and_original_gross() {
    let mut c = config();
    c.pve_mut().map_objective.as_mut().unwrap().cost = ItemAmount {
        item_id: "xuan-tie".into(),
        quantity: 5,
    };
    let mut h = harness(c.clone());
    drive(&mut h);
    attune(&mut h);
    let cp = checkpoint(&mut h);
    recover(c, cp).unwrap();
}
#[test]
fn objective_free_absent_material_does_not_create_invalid_zero_loot_row() {
    let mut c = config();
    c.pve_mut().map_objective.as_mut().unwrap().cost = ItemAmount {
        item_id: "misty-spirit-wood".into(),
        quantity: 0,
    };
    let mut h = harness(c.clone());
    drive(&mut h);
    attune(&mut h);
    let cp = checkpoint(&mut h);
    let mut copy = cp.clone();
    assert!(
        matches!(settle(&mut copy),Business::CommitSettlement{items,..}if !items.iter().any(|i|i.item_id=="misty-spirit-wood"))
    );
    recover(c, cp).unwrap();
}
