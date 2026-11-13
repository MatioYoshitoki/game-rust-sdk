use super::*;

fn counter_input(v: &mut Value) {
    v["pve"]["combat"]["aggro_range"] = json!(2);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["status_definitions"] = json!([
        {"id":"ward","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":300,
         "duplicate_policy":"refresh","source_scoped":true}
    ]);
    v["members"]["checkpoint_schedules"] = json!([{
        "id":"route","first_delay_ms":30,"interval_ms":300,"window_ms":150,"radius":6,"required":1,
        "centers":[[20,20],[20,40],[20,40]],
        "counter":{"activation":"manual","target_ids":["npc-1","npc-1","npc-1"],"range":60,"base_damage":40,
            "window_ms":120,"window_extension":{"id":"bonus","by_level":[60,120]},
            "damage_bonus":{"id":"bonus","by_level":[2500,5000]}}
    }]);
    v["world_boons"] = json!({"capacity":1,"interaction_range":20,
    "definitions":{"bonus":{"levels":[[],[]]}},"opportunities":[
        {"id":"gain","scope":"per_member","position":[20,30],"offer":{"kind":"grant","choices":["bonus"]}},
        {"id":"raise","scope":"per_member","position":[20,30],"offer":{"kind":"upgrade"}}
    ]});
}

fn grant(w: &mut World, i: usize, opportunity_id: &str, c: &Config, now: u64) {
    let action = Action::ClaimBoon {
        opportunity_id: opportunity_id.into(),
        boon_id: "bonus".into(),
        replace_slot: None,
        expected_revision: w.boon_view(i, c).unwrap().revision,
        expected_inventory_revision: w.members[i].revision,
    };
    w.action(i, &action, opportunity_id, c, now).unwrap();
}

fn intent(window_index: u32, target: UnitTarget) -> Action {
    Action::CounterCheckpoint {
        schedule_id: "route".into(),
        window_index,
        target,
    }
}

#[test]
fn manual_counter_freezes_window_and_uses_current_rank_through_native_mitigation() {
    let c = catalog(input(), |v| {
        counter_input(v);
        v["pve"]["stats"]["def"] = json!(10);
        v["pve"]["world_geometry"] = json!({"boundary":[[0,0],[128,0],[128,128],[0,128]],"blockers":[{
            "id":"barrier","vertices":[[30,16],[32,16],[32,24],[30,24]],
            "blocks_movement":false,"blocks_projectiles":true,"blocks_vision":true,
            "active_window":{"start_ms":45,"end_ms":75}
        }]});
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let target = w.npcs[0].entity.clone();
    let action = intent(1, World::unit_wire(&target));
    let window = |w: &World, i, now| {
        w.snapshot(i, &c, Phase::Active, now)
            .own
            .checkpoint_progress
            .unwrap()[0]
            .counter
            .clone()
    };
    assert_eq!(
        w.action(0, &action, "early", &c, 0).unwrap_err(),
        Error::InvalidState
    );
    grant(&mut w, 0, "gain", &c, 0);
    w.tick(&c, 30);
    let opened = window(&w, 0, 30).unwrap();
    assert_eq!((opened.opens_at_ms, opened.expires_at_ms), (30, 210));
    assert!(window(&w, 1, 30).is_none());
    assert_eq!(
        w.action(1, &action, "wrong-seat", &c, 30).unwrap_err(),
        Error::InvalidState
    );
    grant(&mut w, 0, "raise", &c, 60);
    w.tick(&c, 60);
    w.protect(0, 60, &mut vec![]);
    assert_eq!(window(&w, 0, 60), Some(opened.clone()));
    assert_eq!(
        w.action(0, &action, "protected", &c, 60).unwrap_err(),
        Error::NotReady
    );
    w.action(0, &Action::Foreground {}, "fg", &c, 60).unwrap();
    w.action(
        0,
        &Action::CombatReady {
            publication_id: "full".into(),
        },
        "ready",
        &c,
        60,
    )
    .unwrap();
    assert_eq!(window(&w, 0, 60), Some(opened.clone()));

    let mut wrong = opened.target.clone();
    wrong.generation += 1;
    assert_eq!(
        w.action(0, &intent(1, wrong), "wrong-generation", &c, 60)
            .unwrap_err(),
        Error::InvalidTarget
    );
    w.members[0].position = [100., 100.];
    assert_eq!(
        w.action(0, &action, "far", &c, 60).unwrap_err(),
        Error::OutOfRange
    );
    w.members[0].position = [20., 20.];
    assert_eq!(
        w.action(0, &action, "blocked", &c, 60).unwrap_err(),
        Error::InvalidTarget
    );
    assert_eq!(window(&w, 0, 60), Some(opened.clone()));
    w.tick(&c, 90);
    w.apply_owned_statuses(
        Owner::Npc(0),
        (target.clone(), 100),
        &["ward".into()],
        &c,
        90,
        &mut vec![],
    );
    let events = w.action(0, &action, "use", &c, 90).unwrap();
    assert_eq!(w.npcs[0].hp, 80);
    assert!(events.iter().any(|(recipient,event)| *recipient == Some(0) && matches!(event, EventV4::Private {
        event:Event::CheckpointCounterResolved { target: hit, raw_damage:60, absorbed:10, hp_damage:20, .. }
    } if hit == &target)));
    assert!(window(&w, 0, 90).is_none());
    assert_eq!(
        w.action(0, &action, "repeat", &c, 90).unwrap_err(),
        Error::InvalidState
    );
    w.action(
        0,
        &Action::CastUnit {
            skill_id: "skill".into(),
            target: opened.target.clone(),
        },
        "skill",
        &c,
        120,
    )
    .unwrap();
    let events = w.tick(&c, 150);
    assert_eq!(w.npcs[0].hp, 70);
    assert!(events.iter().all(|(_, event)| !matches!(
        event,
        EventV4::Private {
            event: Event::CheckpointCounterResolved { .. }
        }
    )));

    w.tick(&c, 330);
    assert_eq!(window(&w, 1, 330).unwrap().expires_at_ms, 450);
    w.npcs[0].entity.generation += 1;
    assert_eq!(
        w.action(1, &intent(2, opened.target), "retired-target", &c, 360)
            .unwrap_err(),
        Error::InvalidTarget
    );
    assert!(window(&w, 1, 360).is_none());
    w.tick(&c, 630);
    let last = window(&w, 1, 630).unwrap();
    assert_eq!(last.expires_at_ms, 750);
    assert_eq!(
        w.action(1, &intent(3, last.target), "expired", &c, 750)
            .unwrap_err(),
        Error::InvalidState
    );
}

#[test]
fn on_reach_counter_uses_the_same_damage_and_credits_the_actual_killer_once() {
    let c = catalog(input(), |v| {
        counter_input(v);
        let schedule = &mut v["members"]["checkpoint_schedules"][0];
        schedule["radius"] = json!(15);
        schedule["centers"] = json!([[20,30]]);
        schedule["counter"] = json!({"activation":"on_reach","target_ids":["npc-1"],"range":60,"base_damage":40,
            "damage_bonus":{"id":"bonus","by_level":[2500,5000]}});
        v["combat_rules"]["triggers"] = json!([{"id":"normal","profiles":["test"],"when":"damage_dealt",
            "include_periodic":true,"every":1,"cooldown_ms":0,"priority":0,"effect":{"kind":"restore_spirit","amount":10}}]);
    }).unwrap().default_config();
    let mut w = world(&c);
    grant(&mut w, 1, "gain", &c, 0);
    w.npcs[0].hp = 65;
    w.members[1].spirit = 50;
    let events = w.tick(&c, 30);
    let damage = events
        .iter()
        .filter_map(|(i, e)| match e {
            EventV4::Private {
                event:
                    Event::CheckpointCounterResolved {
                        raw_damage,
                        hp_damage,
                        ..
                    },
            } => Some((*i, *raw_damage, *hp_damage)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(damage, [(Some(0), 40, 40), (Some(1), 50, 25)]);
    assert_eq!(w.npcs[0].hp, 0);
    assert!(w.members[0].tally.is_empty());
    assert_eq!(w.members[1].tally["npc"], 1);
    assert_eq!(w.members[1].spirit, 50);
    assert!(w.tick(&c, 60).iter().all(|(_, e)| !matches!(
        e,
        EventV4::Private {
            event: Event::CheckpointCounterResolved { .. }
        }
    )));
    let target = World::unit_wire(&w.npcs[0].entity);
    assert_eq!(
        w.action(1, &intent(1, target), "manual-bypass", &c, 60)
            .unwrap_err(),
        Error::Unsupported
    );
}

#[test]
fn counter_catalog_rejects_unknown_targets_unbounded_bonus_and_automatic_windows() {
    for (field, value) in [
        ("/target_ids/0", json!("unknown")),
        ("/window_ms", json!(29)),
        ("/damage_bonus/by_level/1", json!(10001)),
        ("/window_extension/id", json!("missing")),
        ("/activation", json!("on_reach")),
    ] {
        let config = catalog(input(), |v| {
            counter_input(v);
            *v["members"]["checkpoint_schedules"][0]["counter"]
                .pointer_mut(field)
                .unwrap() = value;
        });
        assert!(
            config
                .err()
                .unwrap()
                .to_string()
                .contains("CONFIG_CHECKPOINT_COUNTER")
        );
    }
}

fn warning_input(v: &mut Value) {
    counter_input(v);
    let schedule = &mut v["members"]["checkpoint_schedules"][0];
    schedule["qualification"] = json!("avoid");
    schedule["centers"] = json!([[20, 20]]);
    schedule["window_ms"] = json!(120);
    schedule["counter"]["activation"] = json!("first_contact");
    schedule["counter"]["target_ids"] = json!(["npc-1"]);
    schedule["counter"]["window_ms"] = json!(360);
}

fn progress(w: &World, i: usize, c: &Config, now: u64) -> CheckpointProgressView {
    w.checkpoint_progress(i, c, now).unwrap().remove(0)
}

#[test]
fn warning_only_credits_initial_exposure_and_an_eligible_exit_by_the_deadline() {
    let c = catalog(input(), warning_input).unwrap().default_config();
    let mut w = world(&c);
    assert_eq!(w.checkpoint_zones(0, &c, 29), Some(vec![]));
    w.tick(&c, 30);
    assert_eq!(w.checkpoint_zones(0, &c, 30).unwrap()[0].avoid, Some(true));
    w.members[1].position = [20., 20.];
    w.tick(&c, 60);
    w.members[1].position = [20., 27.];
    w.members[0].position = [20., 26.];
    let events = w.tick(&c, 150);
    assert!(events.iter().all(|(_, event)| !matches!(
        event,
        EventV4::Private {
            event: Event::CheckpointAvoided { .. }
        }
    )));
    w.members[0].position = [20., 27.];
    w.tick(&c, 180);
    assert_eq!(progress(&w, 0, &c, 180).collected, 0);
    assert_eq!(progress(&w, 1, &c, 180).collected, 0);

    let mut w = world(&c);
    w.members[1].position = [20., 21.];
    w.tick(&c, 30);
    w.protect(1, 60, &mut vec![]);
    w.action(1, &Action::Foreground {}, "fg", &c, 60).unwrap();
    w.action(
        1,
        &Action::CombatReady {
            publication_id: "full".into(),
        },
        "ready",
        &c,
        60,
    )
    .unwrap();
    w.members[0].position = [20., 27.];
    w.members[1].position = [20., 28.];
    let events = w.tick(&c, 150);
    assert_eq!(
        events
            .iter()
            .filter_map(|(seat, event)| match event {
                EventV4::Private {
                    event: Event::CheckpointAvoided { at_ms: 150, .. },
                } => Some(*seat),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [Some(0)]
    );
    assert_eq!(progress(&w, 0, &c, 150).collected, 1);
    assert_eq!(progress(&w, 1, &c, 150).collected, 0);
    w.tick(&c, 180);
    assert_eq!(progress(&w, 0, &c, 180).collected, 1);
}

#[test]
fn first_contact_ignores_a_stale_projectile_and_other_seats_then_consumes_once() {
    let c = catalog(input(), warning_input).unwrap().default_config();
    let mut w = world(&c);
    grant(&mut w, 0, "gain", &c, 0);
    w.tick(&c, 30);
    w.members[0].position = [20., 27.];
    w.tick(&c, 60);
    w.tick(&c, 90);
    w.tick(&c, 120);
    w.action(0, &Action::Attack {}, "before-opening", &c, 150)
        .unwrap();
    let events = w.tick(&c, 150);
    let opened = progress(&w, 0, &c, 150).counter.unwrap();
    assert_eq!(
        (opened.opens_at_ms, opened.expires_at_ms, opened.on_contact),
        (150, 570, Some(true))
    );
    assert!(events.iter().any(|(seat, event)| *seat == Some(0)
        && matches!(
            event,
            EventV4::Private {
                event: Event::CheckpointAvoided { at_ms: 150, .. }
            }
        )));
    assert!(progress(&w, 1, &c, 150).counter.is_none());
    assert_eq!(
        w.action(0, &intent(1, opened.target.clone()), "bypass", &c, 150)
            .unwrap_err(),
        Error::Unsupported
    );
    let cast = Action::CastUnit {
        skill_id: "skill".into(),
        target: opened.target.clone(),
    };
    w.action(1, &cast, "other-seat", &c, 150).unwrap();
    for now in (180..=330).step_by(30) {
        let events = w.tick(&c, now);
        assert!(events.iter().all(|(_, event)| !matches!(
            event,
            EventV4::Private {
                event: Event::CheckpointCounterResolved { .. }
            }
        )));
    }
    assert_eq!(w.npcs[0].hp, 80);
    assert_eq!(progress(&w, 0, &c, 330).counter, Some(opened));
    w.action(0, &cast, "after-opening", &c, 330).unwrap();
    let events = w.tick(&c, 360);
    assert_eq!(w.npcs[0].hp, 20);
    assert!(progress(&w, 0, &c, 360).counter.is_none());
    assert_eq!(
        events
            .iter()
            .filter_map(|(seat, event)| match event {
                EventV4::Private {
                    event:
                        Event::CheckpointCounterResolved {
                            raw_damage,
                            hp_damage,
                            ..
                        },
                } => Some((*seat, *raw_damage, *hp_damage)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [(Some(0), 50, 50)]
    );
    w.action(0, &Action::Attack {}, "after-consuming", &c, 510)
        .unwrap();
    for now in (540..=690).step_by(30) {
        assert!(w.tick(&c, now).iter().all(|(_, event)| !matches!(
            event,
            EventV4::Private {
                event: Event::CheckpointCounterResolved { .. }
            }
        )));
    }
    assert_eq!(w.npcs[0].hp, 10);
}

#[test]
fn warning_requires_a_settlement_step_and_contact_at_expiry_is_ordinary_damage() {
    let config = catalog(input(), |v| {
        warning_input(v);
        v["terminal_ms"] = json!(300);
        v["members"]["checkpoint_schedules"][0]["first_delay_ms"] = json!(179);
    });
    assert!(
        config
            .err()
            .unwrap()
            .to_string()
            .contains("CONFIG_CHECKPOINT_SCHEDULES")
    );
    let c = catalog(input(), warning_input).unwrap().default_config();
    let mut w = world(&c);
    w.tick(&c, 30);
    w.members[0].position = [20., 27.];
    for now in (60..=480).step_by(30) {
        w.tick(&c, now);
    }
    let opened = progress(&w, 0, &c, 480).counter.unwrap();
    assert_eq!(opened.expires_at_ms, 510);
    w.action(
        0,
        &Action::CastUnit {
            skill_id: "skill".into(),
            target: opened.target,
        },
        "expires-at-contact",
        &c,
        480,
    )
    .unwrap();
    let events = w.tick(&c, 510);
    assert_eq!(w.npcs[0].hp, 90);
    assert!(progress(&w, 0, &c, 510).counter.is_none());
    assert!(events.iter().all(|(_, event)| !matches!(
        event,
        EventV4::Private {
            event: Event::CheckpointCounterResolved { .. }
        }
    )));
}
