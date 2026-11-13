use super::*;
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
fn drive(h: &mut Harness, phase_only: bool) {
    let center = h.engine.config.pve.as_ref().unwrap().encounters[0]
        .trigger
        .center;
    h.action(Action::Navigate {
        destination: center,
    })
    .unwrap();
    h.advance(900).unwrap();
    for _ in 0..180 {
        h.pump().unwrap();
        let s = h.snapshot.as_ref().unwrap();
        if s.pve.as_ref().unwrap().encounters[0].phase == EncounterPhase::Completed
            || phase_only
                && s.pve
                    .as_ref()
                    .unwrap()
                    .monsters
                    .iter()
                    .any(|m| m.boss.as_ref().is_some_and(|b| b.phase_index >= 2) && m.hp > 0)
        {
            return;
        }
        if s.pve.as_ref().unwrap().encounters[0].phase == EncounterPhase::Waiting {
            h.advance(30).unwrap();
            continue;
        }
        let Some(target) = s.target.as_ref().map(|t| t.position) else {
            h.advance(30).unwrap();
            continue;
        };
        h.action(Action::Navigate {
            destination: target,
        })
        .unwrap();
        h.advance(900).unwrap();
        match h.action(Action::Attack {}) {
            Ok(_) => {}
            Err(Error::NoResource | Error::OutOfRange | Error::InvalidState) => {}
            Err(e) => panic!("{e:?}"),
        };
        h.advance(600).unwrap();
    }
    panic!("Boss did not progress")
}
fn config() -> Config {
    crate::config::default_catalog()
        .active("b3-8")
        .unwrap()
        .as_ref()
        .clone()
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
    assert_eq!(e.status().phase, Phase::Closed);
    Ok(e)
}
#[test]
fn public_combat_freezes_weighted_score_and_restores_identical_responsibility() {
    let c = config();
    let mut h = harness(c.clone());
    drive(&mut h, false);
    let view = h
        .snapshot
        .as_ref()
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .scoring
        .as_ref()
        .unwrap();
    assert_eq!(
        (view.kills, view.credited_kills, view.kill_score),
        (3, 3, 140)
    );
    assert_eq!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::KillScored { .. }
                }
            ))
            .count(),
        3
    );
    let cp = checkpoint(&mut h);
    let mut copy = cp.clone();
    let original = settle(&mut copy).clone();
    assert!(
        matches!(&original,Business::CommitSettlement{cultivation:52,scoring:Some(s),..}if s.kill_score==140)
    );
    let mut e = recover(c, cp).unwrap();
    let out = e.drain();
    assert!(out.iter().all(|o| !matches!(
        o.body,
        OutputBody::Event {
            event: Event::KillScored { .. }
        }
    )));
    assert!(out.iter().filter_map(|o| o.recovery.as_deref()).any(|cp| {
        cp.state
            .operations
            .iter()
            .any(|op| op.request.body == original)
    }));
}
#[test]
fn duplicate_public_attack_and_extra_score_data_cannot_award_twice() {
    let mut h = harness(config());
    drive(&mut h, true);
    assert!(
        serde_json::from_value::<Action>(serde_json::json!({"kind":"set_score","score":9999}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<Action>(serde_json::json!({"kind":"attack","kill_score":9999}))
            .is_err()
    );
    h.advance(600).unwrap();
    let r = create_request();
    h.sequence += 1;
    let i = Input {
        key: r.key,
        participant_id: r.participant_id,
        entity_generation: 1,
        connection: r.connection,
        sequence: h.sequence,
        request_id: "scoring-attack-once".into(),
        action: Action::Attack {},
    };
    let first = h.engine.input(i.clone()).unwrap();
    assert_eq!(h.engine.input(i).unwrap(), first);
    h.advance(1500).unwrap();
    assert!(
        h.trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::KillScored { .. }
                }
            ))
            .count()
            <= 3
    );
}
#[test]
fn connection_takeover_preserves_actual_score_and_fences_old_attack() {
    let mut h = harness(config());
    drive(&mut h, true);
    let before = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
        .pve
        .unwrap()
        .scoring;
    let c = create_request();
    h.sequence += 1;
    let mut i = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "scoring-next".into(),
            generation: 2,
        },
        sequence: h.sequence,
        request_id: "scoring-handover".into(),
        action: Action::Reconnect {
            previous_connection: c.connection.clone(),
            authenticated_principal: c.authenticated_principal,
        },
    };
    h.engine.input(i.clone()).unwrap();
    i.connection = c.connection;
    i.sequence += 1;
    i.request_id = "old-score-attack".into();
    i.action = Action::Attack {};
    assert_eq!(h.engine.input(i), Err(Error::StaleConnection));
    assert_eq!(
        before,
        h.engine
            .world
            .as_ref()
            .unwrap()
            .snapshot(&h.engine.config, h.engine.status().logical_ms)
            .pve
            .unwrap()
            .scoring
    );
}
#[test]
fn malformed_missing_or_inconsistent_frozen_tally_is_rejected() {
    let c = config();
    let mut h = harness(c.clone());
    drive(&mut h, false);
    let cp = checkpoint(&mut h);
    for mode in 0..11 {
        let mut candidate = cp.clone();
        if let Business::CommitSettlement {
            scoring,
            cultivation,
            ..
        } = settle(&mut candidate)
        {
            if mode == 0 {
                *scoring = None;
            } else {
                let s = scoring.as_mut().unwrap();
                match mode {
                    1 => s.kills += 1,
                    2 => s.credited_kills -= 1,
                    3 => s.kill_score += 1,
                    4 => s.templates[0].template_id = "unknown".into(),
                    5 => s.templates.push(s.templates[0].clone()),
                    6 => s.templates.reverse(),
                    7 => s.templates[0].kills = 0,
                    8 => s.templates[0].kills += 1,
                    9 => *cultivation += 1,
                    _ => s.templates.clear(),
                }
            }
        }
        assert!(
            matches!(recover(c.clone(), candidate), Err(Error::InvalidMessage)),
            "mode {mode}"
        );
    }
}
#[test]
fn drop_claim_without_actual_template_death_is_rejected_even_with_valid_growth() {
    let c = config();
    let mut h = harness(c.clone());
    let mut cp = checkpoint(&mut h);
    if let Business::CommitSettlement {
        outcome,
        looted,
        kept,
        dropped,
        cultivation,
        ..
    } = settle(&mut cp)
    {
        *outcome = Outcome::Extraction;
        *looted = 1;
        *kept = 6;
        *dropped = 0;
        *cultivation = 22;
    }
    assert!(matches!(recover(c, cp), Err(Error::InvalidMessage)));
}
#[test]
fn zero_points_or_zero_coefficient_still_restore_real_picked_loot() {
    for zero_points in [true, false] {
        let mut c = config();
        if zero_points {
            c.pve_mut().kill_score = Some(0);
            for t in c.pve_mut().additional_templates.values_mut() {
                t.kill_score = Some(0);
            }
        } else {
            c.cultivation_coefficient = 0.0;
        }
        let mut h = harness(c.clone());
        drive(&mut h, false);
        let resource = h
            .snapshot
            .as_ref()
            .unwrap()
            .resources
            .iter()
            .find(|r| !r.orb)
            .unwrap()
            .clone();
        h.action(Action::Navigate {
            destination: resource.position,
        })
        .unwrap();
        for _ in 0..100 {
            h.advance(300).unwrap();
            let position = h.snapshot.as_ref().unwrap().position;
            if crate::config::distance(position, resource.position) < c.params.pickup_range - 1.0 {
                break;
            }
        }
        let revision = h.snapshot.as_ref().unwrap().inventory_revision;
        h.action(Action::Pickup {
            resource_id: resource.id,
            expected_revision: revision,
        })
        .unwrap();
        h.pump().unwrap();
        let mut cp = checkpoint(&mut h);
        assert!(
            matches!(settle(&mut cp),Business::CommitSettlement{scoring:Some(s),..}if s.kills==3)
        );
        recover(c, cp).unwrap();
    }
}
#[test]
fn older_config_responsibility_omits_score_and_cannot_accept_new_metadata() {
    let c = crate::config::default_catalog()
        .active("b3-7")
        .unwrap()
        .as_ref()
        .clone();
    let mut h = harness(c.clone());
    let cp = checkpoint(&mut h);
    assert!(
        !serde_json::to_value(&cp)
            .unwrap()
            .to_string()
            .contains("scoring")
    );
    recover(c.clone(), cp.clone()).unwrap();
    let mut candidate = cp;
    if let Business::CommitSettlement { scoring, .. } = settle(&mut candidate) {
        *scoring = Some(ScoreSummary {
            kills: 0,
            credited_kills: 0,
            kill_score: 0,
            templates: vec![],
        });
    }
    assert!(matches!(recover(c, candidate), Err(Error::InvalidMessage)));
}
#[test]
fn saturated_outputs_freeze_actual_new_score_before_reclaiming_world() {
    let mut h = harness(config());
    drive(&mut h, true);
    loop {
        let s = h.snapshot.as_ref().unwrap();
        let m = s
            .pve
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .find(|m| m.boss.is_some())
            .unwrap();
        if m.hp < 80 {
            break;
        }
        h.advance(900).unwrap();
        h.action(Action::Attack {}).unwrap();
        h.advance(600).unwrap();
    }
    h.engine.config.basic.windup_ms = 0;
    h.engine.config.basic.projectile_speed = 10000.0;
    h.advance(900).unwrap();
    h.action(Action::Attack {}).unwrap();
    h.pump().unwrap();
    h.engine.config.output_capacity = 32;
    let mut held = vec![];
    for _ in 0..2 {
        for _ in 0..14 {
            h.action(Action::Foreground {}).unwrap();
        }
        held.extend(h.engine.drain());
    }
    assert_eq!(held.len(), 28);
    h.engine.restore_undelivered(held);
    h.engine.tick();
    let outputs = h.engine.drain();
    assert!(outputs.len() <= 32);
    assert_eq!(h.engine.live_worlds(), 0);
    assert_eq!(h.engine.status().phase, Phase::Faulted);
    let bodies: Vec<_> = outputs
        .iter()
        .filter_map(|o| match &o.body {
            OutputBody::BusinessRequest { request }
                if matches!(request.body, Business::CommitSettlement { .. }) =>
            {
                Some(&request.body)
            }
            _ => None,
        })
        .collect();
    assert_eq!(bodies.len(), 1);
    assert!(
        matches!(bodies[0],Business::CommitSettlement{scoring:Some(s),cultivation:15,..}if s.kills==2 && s.kill_score==40)
    );
}
