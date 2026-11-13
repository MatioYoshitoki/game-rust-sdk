use super::*;
fn harness() -> Harness {
    let c = crate::config::default_catalog().active("b3-7").unwrap();
    let mut h = Harness::new();
    h.engine = Engine::new(c.as_ref().clone());
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
fn open() -> Action {
    Action::OpenChest {
        chest_id: "boss-cache-1".into(),
        expected_generation: 1,
    }
}
fn restore(h: &mut Harness) -> Engine {
    h.action(Action::Leave {}).unwrap();
    let cp = h
        .engine
        .drain()
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone();
    let raw = serde_json::to_value(&cp).unwrap();
    assert!(!raw.to_string().contains("boss_phase"));
    let c = crate::config::default_catalog().active("b3-7").unwrap();
    let mut r = create_request();
    r.config_id = c.id().into();
    r.config_hash = c.content_hash().into();
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    let mut e = Engine::new(Config::frozen());
    e.create(r).unwrap();
    assert_eq!(e.live_worlds(), 0);
    assert_eq!(e.status().phase, Phase::Closed);
    e
}
#[test]
fn public_inputs_cannot_set_boss_phase_and_nonlethal_transition_cannot_claim_reward() {
    let mut h = harness();
    drive(&mut h, true);
    assert_eq!(h.action(open()), Err(Error::EncounterLocked));
    // Extra data cannot become an authoritative state-writing action.
    assert!(
        serde_json::from_value::<Action>(
            serde_json::json!({"kind":"attack","boss_phase":2,"hp":1})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<Action>(serde_json::json!({"kind":"set_boss_phase","index":2}))
            .is_err()
    );
}
#[test]
fn public_combat_changes_phase_then_kills_one_boss_without_phase_rewards() {
    let mut h = harness();
    drive(&mut h, false);
    assert_eq!(h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().kills, 3);
    let changes: Vec<_> = h
        .trace
        .iter()
        .filter_map(|o| match &o.body {
            OutputBody::Event {
                event:
                    Event::BossPhaseChanged {
                        monster_id,
                        phase_index,
                        ..
                    },
            } => Some((monster_id.clone(), *phase_index)),
            _ => None,
        })
        .collect();
    assert_eq!(changes, vec![("forest-phase-boss-1".into(), 2)]);
    assert_eq!(h.trace.iter().filter(|o|matches!(&o.body,OutputBody::Event{event:Event::MonsterKilled{monster_id}}if monster_id=="forest-phase-boss-1")).count(),1);
    assert!(!h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().chests[0].opened);
    assert_eq!(h.store.warehouse, 95);
}
#[test]
fn handover_keeps_boss_phase_epoch_and_rejects_old_attack() {
    let mut h = harness();
    drive(&mut h, true);
    h.pump().unwrap();
    let before = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
        .pve
        .unwrap()
        .monsters
        .into_iter()
        .find(|m| m.boss.is_some())
        .unwrap();
    let c = create_request();
    h.sequence += 1;
    let i = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "boss-next".into(),
            generation: 2,
        },
        sequence: h.sequence,
        request_id: "boss-handover".into(),
        action: Action::Reconnect {
            previous_connection: c.connection.clone(),
            authenticated_principal: c.authenticated_principal,
        },
    };
    h.engine.input(i.clone()).unwrap();
    let mut old = i;
    old.connection = c.connection;
    old.sequence += 1;
    old.request_id = "old-boss-attack".into();
    old.action = Action::Attack {};
    assert_eq!(h.engine.input(old), Err(Error::StaleConnection));
    let after = h
        .engine
        .world
        .as_ref()
        .unwrap()
        .snapshot(&h.engine.config, h.engine.status().logical_ms)
        .pve
        .unwrap()
        .monsters
        .into_iter()
        .find(|m| m.boss.is_some())
        .unwrap();
    assert_eq!(before, after);
}
#[test]
fn partial_boss_phase_settlement_recovers_closed_with_only_actual_kills() {
    let mut h = harness();
    drive(&mut h, true);
    assert_eq!(h.snapshot.as_ref().unwrap().pve.as_ref().unwrap().kills, 2);
    let mut e = restore(&mut h);
    assert!(e.drain().iter().all(|o| !matches!(
        &o.body,
        OutputBody::Event {
            event: Event::BossPhaseChanged { .. } | Event::BossReset { .. }
        }
    )));
}
#[test]
fn completed_boss_settlement_recovers_without_replaying_phase_or_kill() {
    let mut h = harness();
    drive(&mut h, false);
    let mut e = restore(&mut h);
    assert!(e.drain().iter().all(|o| !matches!(
        &o.body,
        OutputBody::Event {
            event: Event::BossPhaseChanged { .. }
                | Event::BossReset { .. }
                | Event::MonsterKilled { .. }
        }
    )));
}
