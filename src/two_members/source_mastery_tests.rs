use super::*;

const SKILL: &str = "yishen-weizhong";
const COST: u32 = 1_800_000;

fn catalog(change: impl FnOnce(&mut Value)) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    catalog_with(|v| {
        *v = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/mastery-stages.json"
        ))
        .unwrap();
        change(v);
    })
}

fn host(stages: [Option<usize>; 2], change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let cat = catalog(change).unwrap();
    let config = cat.default_config();
    let mut request = create(&config);
    request.protocol_version = 6;
    for (seat, stage) in request.participants.iter_mut().zip(stages) {
        seat.combat_build = stage.map(|mastery| CombatBuild {
            power_points: 0,
            skill_power_points: 0,
            element_resistance_points: 0,
            powers: BTreeMap::from([(
                SKILL.into(),
                PowerGrowth {
                    model_id: "mastery-demo".into(),
                    rank: 0,
                    quality: 0,
                    level: 0,
                    mastery,
                },
            )]),
        });
    }
    let mut h = DeltaHarness::from_request(cat, request);
    for store in &mut h.stores {
        store.hp = 1000;
        store.spirit = 2_000_000;
    }
    h.active();
    // Give life steal and expiry recovery enough missing HP to observe their amounts.
    for member in &mut h.engine.world.as_mut().unwrap().members {
        member.hp = 100;
    }
    h
}

fn cast(skill_id: &str) -> Action {
    Action::CastSelf {
        skill_id: skill_id.into(),
    }
}

fn strike(h: &mut DeltaHarness, i: usize) {
    let target = &h.snapshot(i).monsters[0].entity;
    h.action(
        i,
        Action::CastUnit {
            skill_id: "seed-strike".into(),
            target: UnitTarget {
                id: target.id.clone(),
                generation: target.generation,
            },
        },
    )
    .unwrap();
}

fn seed(h: &DeltaHarness, i: usize) -> &StatusView {
    h.snapshot(i)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id.starts_with("dao-seed-"))
        .unwrap()
}

fn debuff(h: &DeltaHarness, i: usize) -> bool {
    h.snapshot(i)
        .own
        .statuses
        .iter()
        .any(|s| s.definition_id == "test-debuff")
}

fn ready_at(h: &DeltaHarness, i: usize) -> u64 {
    h.snapshot(i)
        .own
        .player_skills
        .iter()
        .find(|s| s.skill_id == SKILL)
        .unwrap()
        .ready_at_ms
}

#[test]
fn source_mastery_stages_select_five_active_profiles_on_the_same_skill() {
    for stages in [[Some(0), Some(1)], [Some(2), Some(3)], [Some(4), None]] {
        let mut h = host(stages, |_| {});
        for (i, stage) in stages.into_iter().enumerate() {
            let mastery = stage.unwrap_or(0);
            let request = h.input(i, cast(SKILL));
            h.engine.input(request.clone()).unwrap();
            h.pump();
            h.engine.input(request).unwrap();
            h.pump();
            assert_eq!(h.snapshot(i).own.spirit, 2_000_000 - COST);
            assert_eq!(ready_at(&h, i), 60_000);
            assert_eq!(
                seed(&h, i).definition_id,
                format!("dao-seed-{}", mastery + 1)
            );
            assert_eq!(seed(&h, i).stacks, Some(1));
            assert_eq!(seed(&h, i).max_stacks, Some(3));
            assert_eq!(
                seed(&h, i).expires_at_ms,
                if mastery >= 3 { 15000 } else { 12000 }
            );
            assert_eq!(
                seed(&h, i).source.id,
                h.snapshot(i).own.participant.entity.id
            );
            assert_eq!(seed(&h, i).target, seed(&h, i).source);
        }
        let commits = h.events.iter().filter(|(_, e)| matches!(e,
            EventV4::Private { event: Event::ActionCommitted { skill_id: Some(id), spirit_cost: COST, .. } }
            if id == SKILL
        )).count();
        assert_eq!(commits, 2);
        for i in 0..2 {
            h.action(i, cast("tag-debuff")).unwrap();
        }
        h.ticks(2);
        for (hit, expected_heal) in [18, 21, 24].into_iter().enumerate() {
            let before = [h.snapshot(0).own.hp, h.snapshot(1).own.hp];
            strike(&mut h, 0);
            strike(&mut h, 1);
            h.ticks(1);
            for (i, stage) in stages.into_iter().enumerate() {
                let mastery = stage.unwrap_or(0);
                assert_eq!(
                    h.snapshot(i).own.hp - before[i],
                    if mastery == 0 { 15 } else { expected_heal }
                );
                assert_eq!(seed(&h, i).stacks, Some((hit as u32 + 2).min(3)));
                assert_eq!(debuff(&h, i), mastery < 2 || hit != 1);
            }
            h.ticks(5);
            if hit == 1 {
                for (i, stage) in stages.into_iter().enumerate() {
                    if stage.is_some_and(|mastery| mastery >= 2) {
                        h.action(i, cast("tag-debuff")).unwrap();
                    }
                }
                h.ticks(2);
            }
        }
    }
}

#[test]
fn source_mastery_seed_reduces_actual_incoming_damage_by_twenty_percent() {
    let mut h = host([Some(0), Some(4)], |v| {
        let npc = v["pve"]["template_id"].as_str().unwrap().to_owned();
        v["pve"]["stats"]["atk"] = json!(100);
        v["pve"]["combat"]["aggro_range"] = json!(60);
        v["pve"]["combat"]["melee_range"] = json!(60);
        v["pve"]["combat"]["attack_interval_ms"] = json!(300);
        v["pve"]["monster_positions"] = json!([[440, 400], [440, 480]]);
        let mut attack = v["combat_rules"]["basic_attack"].clone();
        attack.as_object_mut().unwrap().remove("life_steal_timing");
        v["combat_rules"]["npc_attacks"][npc] = attack;
    });
    for member in &mut h.engine.world.as_mut().unwrap().members {
        member.hp = 500;
    }
    h.action(0, cast(SKILL)).unwrap();
    h.pump();
    h.ticks(12);
    assert_eq!(h.snapshot(0).own.hp, 420);
    assert_eq!(h.snapshot(1).own.hp, 400);
}

#[test]
fn source_mastery_action_life_steal_rounds_once_before_gaining_a_stack() {
    let contacts = |h: &DeltaHarness| {
        h.events
            .iter()
            .filter(|(_, e)| {
                matches!(e,
                    EventV4::Damage { source, amount: 5, .. } if source.id == "A"
                )
            })
            .count()
    };
    let heals = |h: &DeltaHarness| {
        h.events
            .iter()
            .enumerate()
            .filter_map(|(at, (recipient, event))| match event {
                EventV4::Private {
                    event:
                        Event::HealResolved {
                            heal_kind: HealKind::LifeSteal,
                            raw_heal,
                            hp_restored,
                            ..
                        },
                } => {
                    assert_eq!(recipient.as_deref(), Some("A"));
                    Some((at, *raw_heal, *hp_restored))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for action_end in [true, false] {
        let mut h = host([Some(3), Some(0)], |v| {
            let field = &mut v["combat_rules"]["skills"]["seed-field"];
            if !action_end {
                field.as_object_mut().unwrap().remove("life_steal_timing");
            }
        });
        h.action(0, cast(SKILL)).unwrap();
        h.pump();
        h.action(
            0,
            Action::CastGround {
                skill_id: "seed-field".into(),
                position: [440., 440.],
            },
        )
        .unwrap();
        h.ticks(12);
        assert_eq!(contacts(&h), 2);
        assert_eq!(seed(&h, 0).stacks, Some(1));
        assert_eq!(h.snapshot(0).own.hp, 100);
        assert_eq!(heals(&h).len(), if action_end { 0 } else { 2 });
        h.ticks(45);
        assert_eq!(contacts(&h), 4);
        assert_eq!(seed(&h, 0).stacks, Some(2));
        assert_eq!(seed(&h, 0).expires_at_ms, 15_000);
        assert!(h.snapshot(1).own.statuses.is_empty());
        let heals = heals(&h);
        if action_end {
            let [(heal_at, 3, 3)] = heals.as_slice() else {
                panic!("{heals:?}")
            };
            let stacks_at = h.events.iter().position(|(_, e)| matches!(e,
                EventV4::Private { event: Event::CombatActionTotal { trigger_id, fired: true, .. } }
                if trigger_id == "dao-seed-4-gain"
            )).unwrap();
            assert!(*heal_at < stacks_at);
            assert_eq!(h.snapshot(0).own.hp, 103);
        } else {
            assert_eq!(heals.len(), 4);
            assert!(
                heals
                    .iter()
                    .all(|(_, raw, restored)| (*raw, *restored) == (0, 0))
            );
            assert_eq!(h.snapshot(0).own.hp, 100);
        }
    }
}

#[test]
fn source_mastery_perfected_window_pays_capped_recovery_only_on_expiry() {
    for (damage, expected_heal) in [(100, 20), (1100, 200)] {
        let mut h = host([Some(4), Some(4)], |v| {
            v["combat_rules"]["skills"]["seed-strike"]["base_power"] = json!(damage);
        });
        for i in 0..2 {
            h.action(i, cast(SKILL)).unwrap();
        }
        h.pump();
        for _ in 0..2 {
            strike(&mut h, 0);
            strike(&mut h, 1);
            h.ticks(6);
        }
        assert_eq!(seed(&h, 0).window.as_ref().unwrap().recorded, damage * 2);
        let hp = h.snapshot(0).own.hp;
        h.action(1, Action::Leave {}).unwrap();
        h.pump();
        let steps = ((15000 - h.engine.now) / h.engine.config.step_ms) as usize;
        h.ticks(steps + 1);
        assert_eq!(h.snapshot(0).own.hp, hp + expected_heal);
        assert!(h.snapshot(0).own.statuses.is_empty());
        let recoveries: Vec<_> = h
            .events
            .iter()
            .filter_map(|(recipient, e)| match e {
                EventV4::Private {
                    event: Event::WindowRecordEnded { hp_restored, .. },
                } if *hp_restored > 0 => Some((recipient.as_deref(), *hp_restored)),
                _ => None,
            })
            .collect();
        assert_eq!(recoveries, [(Some("A"), expected_heal)]);
        h.close();
        assert!(h.stores.iter().all(|s| s.settlements == 1));
    }
}

#[test]
fn source_mastery_selected_status_capacity_does_not_pay_or_use_fallback() {
    let mut h = host([Some(4), None], |v| {
        v["pve"]["status_rules"]["max_active_per_entity"] = json!(1);
        v["additional_skills"][1]["on_commit_self_statuses"] = json!(["dao-seed-1"]);
    });
    for i in 0..2 {
        h.action(i, cast("tag-debuff")).unwrap();
    }
    h.ticks(2);
    let before = h.snapshot(0).own.clone();
    assert_eq!(h.action(0, cast(SKILL)), Err(Error::Capacity));
    h.pump();
    assert_eq!(h.snapshot(0).own, before);
    assert_eq!(ready_at(&h, 0), 0);
    h.action(1, cast(SKILL)).unwrap();
    h.pump();
    assert_eq!(h.snapshot(1).own.spirit, 2_000_000 - COST);
    assert_eq!(seed(&h, 1).definition_id, "dao-seed-1");
}

#[test]
fn source_mastery_configuration_rejects_invalid_tables_and_round_trips() {
    let config = catalog(|_| {}).unwrap().default_config();
    let normalized = serde_json::to_value(&*config).unwrap();
    let rows = &normalized["combat_rules"]["self_statuses_by_mastery"][SKILL];
    assert_eq!(rows.as_array().unwrap().len(), 4);
    let restored: Config = serde_json::from_value(normalized.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap()["combat_rules"]["self_statuses_by_mastery"][SKILL],
        *rows
    );
    for invalid in [
        json!(null),
        json!([]),
        json!([{"minimum_mastery":5,"statuses":["dao-seed-1"]}]),
        json!([{"minimum_mastery":1,"statuses":[]}]),
        json!([{"minimum_mastery":1,"statuses":["unknown"]}]),
        json!([{"minimum_mastery":1,"statuses":["dao-seed-1","dao-seed-1"]}]),
        json!([{"minimum_mastery":1,"statuses":["dao-seed-1"],"unknown":true}]),
        json!([rows[1], rows[0]]),
        json!([rows[0], rows[0]]),
    ] {
        assert!(
            catalog(|v| v["combat_rules"]["self_statuses_by_mastery"][SKILL] = invalid).is_err()
        );
    }
    assert!(
        catalog(|v| {
            v["combat_rules"]["self_statuses_by_mastery"] = json!({"seed-strike": rows});
        })
        .is_err()
    );
    let mut old = host([Some(4), None], |v| {
        v["combat_rules"]
            .as_object_mut()
            .unwrap()
            .remove("self_statuses_by_mastery");
    });
    old.action(0, cast(SKILL)).unwrap();
    old.pump();
    assert_eq!(seed(&old, 0).definition_id, "dao-seed-1");
}

fn basic_contacts(h: &DeltaHarness) -> Vec<&EntityRef> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::ProjectileContact { source, target, .. } if source.id == "A" => Some(target),
            _ => None,
        })
        .collect()
}

#[test]
fn basic_delivery_two_targets_settle_spirit_life_steal_and_cooldown_once() {
    let mut h = host([Some(0), None], |_| {});
    h.action(0, cast(SKILL)).unwrap();
    h.pump();
    let spirit = h.snapshot(0).own.spirit;
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(4);
    let shot = &h.snapshot(0).projectiles[0];
    let piercing = shot.piercing.as_ref().unwrap();
    assert_eq!(piercing.skill_id, "basic-piercing-demo");
    assert_eq!(piercing.created_at_ms, 120);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, piercing.expires_at_ms);
    h.ticks(1);
    assert_eq!(basic_contacts(&h).len(), 1);
    assert_eq!(h.snapshot(0).own.spirit, spirit);
    assert_eq!(h.snapshot(0).own.hp, 100);
    h.action(0, Action::Attack {}).unwrap();
    h.pump();
    let queued = h.snapshot(0).own.basic_followup.clone().unwrap();
    h.action(
        0,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(1);
    let contacts = basic_contacts(&h);
    assert_eq!(contacts.len(), 2);
    assert_ne!(contacts[0], contacts[1]);
    assert!(h.snapshot(0).projectiles.is_empty());
    assert!(h.snapshot(0).own.position[1] > 400.);
    assert_eq!(h.snapshot(0).own.spirit, spirit + 2);
    assert_eq!(h.snapshot(0).own.hp, 130);
    assert_eq!(h.snapshot(1).own.spirit, 2_000_000);
    assert_eq!(h.snapshot(1).own.basic_ready_at_ms, 0);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 180 + 700);
    let ready = h.snapshot(0).own.basic_followup.clone().unwrap();
    assert_eq!(ready.action_id, queued.action_id);
    assert_eq!((ready.promotes_at_ms, ready.expires_at_ms), (900, 1050));
    h.action(0, Action::Stop {}).unwrap();
    h.ticks(23);
    assert!(h.snapshot(0).own.pending_action.is_none());
    assert_eq!(h.snapshot(0).own.spirit, spirit + 2);
    h.ticks(1);
    assert!(h.snapshot(0).own.pending_action.is_some());
    assert!(h.snapshot(0).own.basic_followup.is_none());
    h.ticks(4);
    assert_eq!(h.events.iter().filter(|(_, e)| matches!(e,
        EventV4::Private { event: Event::ActionCommitted { action_id, skill: false, .. } } if action_id == &queued.action_id
    )).count(), 1);
    h.close();
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}

#[test]
fn basic_delivery_miss_starts_cooldown_but_a_canceled_windup_starts_neither() {
    let mut h = host([None, None], |v| {
        v["params"]["player_projectile_ttl_ms"] = json!(60);
        v["pve"]["monster_positions"] = json!([[480, 400], [510, 400]]);
    });
    h.engine.world.as_mut().unwrap().members[0].spirit = 100;
    h.action(0, Action::Attack {}).unwrap();
    h.action(0, Action::CancelAction {}).unwrap();
    h.ticks(5);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 0);
    assert!(h.snapshot(0).projectiles.is_empty());
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(6);
    assert!(basic_contacts(&h).is_empty());
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert!(h.snapshot(0).projectiles.is_empty());
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 330 + 700);
}

#[test]
fn basic_delivery_protection_discards_reward_but_cooldown_survives_reconnect() {
    let mut h = host([None, None], |_| {});
    h.engine.world.as_mut().unwrap().members[0].spirit = 100;
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(5);
    assert_eq!(basic_contacts(&h).len(), 1);
    let previous = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous,
            authenticated_principal: "principal-A".into(),
        },
    );
    reconnect.connection = Connection {
        id: "basic-replacement".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    h.pump();
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    let publication_id = h.replicas[0].last_full_id().unwrap().to_owned();
    h.action(0, Action::CombatReady { publication_id }).unwrap();
    h.pump();
    h.ticks(1);
    assert_eq!(basic_contacts(&h).len(), 2);
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 880);
    assert!(h.snapshot(0).own.ready);
}

#[test]
fn basic_delivery_rechecks_the_selected_target_before_release() {
    let mut h = host([None, None], |v| {
        v["combat_rules"]["skills"]["seed-strike"]["base_power"] = json!(10000);
    });
    h.engine.world.as_mut().unwrap().members[0].spirit = 100;
    let input = h.input(0, Action::Attack {});
    let action_id = input.request_id.clone();
    h.engine.input(input).unwrap();
    strike(&mut h, 1);
    h.ticks(5);
    assert!(h.snapshot(0).projectiles.is_empty());
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 0);
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert!(h.events.iter().any(|(_, e)| matches!(e,
        EventV4::Private { event: Event::ActionRejected { action_id: id, reason: Error::InvalidTarget } } if id == &action_id
    )));
}

#[test]
fn basic_delivery_applies_current_spirit_lock_and_capacity_at_the_end() {
    let mut h = host([None, None], |v| {
        v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({
            "id":"basic-probe-lock","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock",
            "modifier_bps":-10000,"duration_ms":60,"duplicate_policy":"refresh","source_scoped":true
        }));
        let probe = v["additional_skills"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|s| s["id"] == "tag-debuff")
            .unwrap();
        probe["on_commit_self_statuses"] = json!(["basic-probe-lock"]);
        probe["commit_on_accept"] = json!(true);
        probe["windup_ms"] = json!(0);
    });
    h.engine.world.as_mut().unwrap().members[0].spirit = 100;
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(5);
    assert_eq!(basic_contacts(&h).len(), 1);
    h.action(0, cast("tag-debuff")).unwrap();
    h.ticks(3);
    assert_eq!(basic_contacts(&h).len(), 2);
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert!(h.snapshot(0).own.statuses.is_empty());
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 880);

    let mut h = host([None, None], |_| {});
    h.engine.world.as_mut().unwrap().members[0].spirit = 1_999_999;
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(6);
    assert_eq!(basic_contacts(&h).len(), 2);
    assert_eq!(h.snapshot(0).own.spirit, 2_000_000);
}

#[test]
fn basic_delivery_cold_load_rejects_invalid_plans_and_retains_release_mode() {
    for (key, value) in [
        ("settle_on_end", json!(null)),
        ("piercing", json!(null)),
        ("piercing", json!({"plan_id":"","max_hits":2})),
        ("piercing", json!({"plan_id":"seed-strike","max_hits":2})),
        ("piercing", json!({"plan_id":"probe","max_hits":0})),
        ("piercing", json!({"plan_id":"probe","max_hits":129})),
        ("piercing", json!({"plan_id":"probe","max_hits":9})),
        (
            "piercing",
            json!({"plan_id":"probe","max_hits":2,"repeat":true}),
        ),
    ] {
        assert!(catalog(|v| v["basic"][key] = value).is_err(), "{key}");
    }
    for ttl in [30, 60_001] {
        assert!(catalog(|v| v["params"]["player_projectile_ttl_ms"] = json!(ttl)).is_err());
    }
    let mut h = host([None, None], |v| {
        v["basic"].as_object_mut().unwrap().remove("settle_on_end");
    });
    h.engine.world.as_mut().unwrap().members[0].spirit = 100;
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(4);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 120 + 700);
    h.ticks(1);
    assert_eq!(basic_contacts(&h).len(), 1);
    assert_eq!(h.snapshot(0).own.spirit, 102);
    h.ticks(1);
    assert_eq!(basic_contacts(&h).len(), 2);
    assert_eq!(h.snapshot(0).own.spirit, 104);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 820);
}

fn wave_profile(profile: &str) -> Value {
    let profiles: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/basic-wave-profiles.json"
    ))
    .unwrap();
    profiles[profile]["basic"].clone()
}

fn wave_attack(h: &mut DeltaHarness, target_index: usize) {
    let target = &h.snapshot(0).monsters[target_index].entity;
    h.action(
        0,
        Action::AttackUnit {
            target: UnitTarget {
                id: target.id.clone(),
                generation: target.generation,
            },
        },
    )
    .unwrap();
}

fn wave_hits(h: &DeltaHarness) -> Vec<(&str, u64)> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::BasicWaveContact {
                target,
                scheduled_at_ms,
                ..
            } => Some((target.id.as_str(), *scheduled_at_ms)),
            _ => None,
        })
        .collect()
}

#[test]
fn basic_wave_directional_freezes_candidates_and_settles_actual_damage_once() {
    let mut h = host([Some(0), None], |v| {
        v["basic"] = wave_profile("fan");
        v["pve"]["monster_positions"] = json!([
            [480, 400],
            [450, 425],
            [450, 375],
            [530, 400],
            [550, 410],
            [430, 433]
        ]);
    });
    h.action(0, cast(SKILL)).unwrap();
    h.pump();
    wave_attack(&mut h, 0);
    h.ticks(5);
    let started = h.snapshot(0).basic_waves.as_ref().unwrap()[0].clone();
    let target_id = |i: usize| h.snapshot(0).monsters[i].entity.id.clone();
    let ids = [target_id(0), target_id(1), target_id(2), target_id(3)];
    assert_eq!(started.released_at_ms, 150);
    assert_eq!(started.origin, [400., 400.]);
    assert_eq!(
        started.geometry,
        BasicWaveGeometry::Directional {
            direction: [1., 0.],
            cone_degrees: 84.
        }
    );
    assert_eq!(
        started
            .pending
            .iter()
            .map(|p| (p.target.id.as_str(), p.at_ms))
            .collect::<Vec<_>>(),
        [
            (ids[1].as_str(), 250),
            (ids[2].as_str(), 250),
            (ids[0].as_str(), 310),
            (ids[3].as_str(), 410)
        ]
    );
    assert_eq!(
        h.snapshot(1).basic_waves.as_deref(),
        Some(std::slice::from_ref(&started))
    );
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 410);
    assert!(h.snapshot(0).projectiles.is_empty());

    let world = h.engine.world.as_mut().unwrap();
    world.place_wave_test_npc(1, [426., 440.]);
    world.place_wave_test_npc(5, [430., 410.]);
    h.action(
        0,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(4);
    assert_eq!(
        wave_hits(&h),
        [(ids[1].as_str(), 250), (ids[2].as_str(), 250)]
    );
    assert_eq!(h.snapshot(0).own.hp, 100);
    assert_eq!(h.snapshot(0).own.spirit, 2_000_000 - COST);
    assert_eq!(
        h.snapshot(0).basic_waves.as_ref().unwrap()[0].origin,
        started.origin
    );
    h.ticks(5);
    assert_eq!(wave_hits(&h).len(), 4);
    assert_eq!(
        h.snapshot(0)
            .monsters
            .iter()
            .map(|n| n.hp)
            .collect::<Vec<_>>(),
        [9920, 9928, 9928, 9928, 10000, 10000]
    );
    assert_eq!(h.snapshot(0).own.hp, 144);
    assert_eq!(h.snapshot(0).own.spirit, 2_000_000 - COST + 2);
    assert_eq!(h.snapshot(1).own.spirit, 2_000_000);
    assert_eq!(h.snapshot(1).own.basic_ready_at_ms, 0);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 420 + 800);
    assert_eq!(h.snapshot(0).basic_waves.as_deref(), Some([].as_slice()));
    assert!(h.frames.iter().any(|frame| matches!(&frame.payload,
        PublicationPayloadV6::Delta { delta, .. } if delta.basic_waves.as_ref().is_some_and(Vec::is_empty)
    )));
    h.close();
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}

#[test]
fn basic_wave_radial_arrives_by_distance_including_behind_and_overlap() {
    let mut h = host([None, None], |v| {
        v["basic"] = wave_profile("tower");
        v["pve"]["monster_positions"] =
            json!([[540, 400], [360, 400], [430, 440], [400, 400], [580, 430]]);
    });
    wave_attack(&mut h, 3);
    h.ticks(6);
    let snapshot = h.snapshot(0);
    let wave = &snapshot.basic_waves.as_ref().unwrap()[0];
    assert_eq!(wave.geometry, BasicWaveGeometry::Radial {});
    assert_eq!(
        wave.pending.iter().map(|p| p.at_ms).collect::<Vec<_>>(),
        [181, 220, 230, 320]
    );
    assert_eq!(wave.pending[0].target.id, snapshot.monsters[3].entity.id);
    assert_eq!(wave.pending[1].target.id, snapshot.monsters[1].entity.id);
    h.ticks(5);
    assert_eq!(wave_hits(&h).len(), 4);
    assert_eq!(
        h.snapshot(0)
            .monsters
            .iter()
            .map(|n| n.hp)
            .collect::<Vec<_>>(),
        [9935, 9935, 9935, 9935, 10000]
    );
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 330 + 1100);
}

#[test]
fn basic_wave_reconnect_preserves_pending_and_skips_a_new_generation() {
    let mut h = host([None, None], |v| {
        v["basic"] = wave_profile("shield");
        v["pve"]["monster_positions"] = json!([[520, 400], [470, 435], [440, 440]]);
    });
    h.engine.world.as_mut().unwrap().members[0].spirit = 100;
    wave_attack(&mut h, 0);
    h.ticks(6);
    let started = h.snapshot(0).basic_waves.as_ref().unwrap()[0].clone();
    assert_eq!(started.pending.len(), 2);
    assert_eq!(
        started.pending[0].target.id,
        h.snapshot(0).monsters[2].entity.id
    );
    assert_eq!(
        started.pending[1].target.id,
        h.snapshot(0).monsters[0].entity.id
    );
    let previous = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous,
            authenticated_principal: "principal-A".into(),
        },
    );
    reconnect.connection = Connection {
        id: "wave-replacement".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    h.pump();
    assert!(matches!(
        &h.latest(0).payload,
        PublicationPayloadV6::Full { .. }
    ));
    assert_eq!(
        h.snapshot(0).basic_waves.as_deref(),
        Some(std::slice::from_ref(&started))
    );

    h.engine.world.as_mut().unwrap().replace_wave_test_npc(0);
    h.ticks(4);
    assert_eq!(
        wave_hits(&h),
        [(started.pending[0].target.id.as_str(), 214)]
    );
    assert_eq!(h.snapshot(1).monsters[0].hp, 10000);
    assert_eq!(h.snapshot(1).monsters[2].hp, 9938);
    assert_eq!(h.snapshot(1).basic_waves.as_deref(), Some([].as_slice()));
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 300 + 1000);
}

#[test]
fn basic_wave_source_departure_clears_scheduled_contacts() {
    let mut h = host([None, None], |v| {
        v["basic"] = wave_profile("fan");
    });
    wave_attack(&mut h, 0);
    h.ticks(5);
    let wave_id = h.snapshot(1).basic_waves.as_ref().unwrap()[0].id.clone();
    h.action(0, Action::Leave {}).unwrap();
    h.ticks(7);
    assert_eq!(h.snapshot(1).basic_waves.as_deref(), Some([].as_slice()));
    assert!(wave_hits(&h).is_empty());
    assert!(h.events.iter().any(|(_, event)| matches!(event, EventV4::BasicWaveEnded { wave_id: id, reason: BasicWaveEndReason::SourceInvalid } if *id == wave_id)));
    assert_eq!(h.stores[0].settlements, 1);
}

#[test]
fn basic_wave_cold_load_rejects_unbounded_or_conflicting_configuration() {
    for profile in ["fan", "shield", "tower"] {
        assert!(catalog(|v| v["basic"] = wave_profile(profile)).is_ok());
    }
    for (field, invalid) in [
        ("target_cap", json!(17)),
        ("speed", json!(0.1)),
        ("primary_damage_bps", json!(0)),
        ("additional_damage_bps", json!(100001)),
        ("plan_id", json!("seed-strike")),
        ("geometry", json!({"kind":"directional","cone_degrees":181})),
        ("geometry", json!({"kind":"radial","cone_degrees":84})),
    ] {
        assert!(
            catalog(|v| {
                v["basic"] = wave_profile("fan");
                v["basic"]["wave"][field] = invalid;
            })
            .is_err(),
            "{field}"
        );
    }
    for (field, invalid) in [
        ("settle_on_end", json!(false)),
        ("wave", Value::Null),
        ("piercing", json!({"plan_id":"conflict","max_hits":2})),
    ] {
        assert!(
            catalog(|v| {
                v["basic"] = wave_profile("fan");
                v["basic"][field] = invalid;
            })
            .is_err(),
            "{field}"
        );
    }
    assert!(
        catalog(|v| {
            v["basic"] = wave_profile("fan");
            v["mechanism_version"] = json!(104);
        })
        .is_err()
    );
}

#[test]
fn basic_wave_projectile_only_wall_filters_candidates_and_explicit_aim() {
    let mut h = host([None, None], |v| {
        v["basic"] = wave_profile("fan");
        v["pve"]["monster_positions"] = json!([[540, 400], [540, 465]]);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let index = 11 * 32 + 12;
        v["pve"]["cells_rle"] = json!([[0, index], [1, 1], [0, 1024 - index - 1]]);
    });
    let blocked = h.snapshot(0).monsters[1].entity.clone();
    assert_eq!(
        h.action(
            0,
            Action::AttackUnit {
                target: UnitTarget {
                    id: blocked.id.clone(),
                    generation: blocked.generation
                },
            }
        ),
        Err(Error::InvalidTarget)
    );
    wave_attack(&mut h, 0);
    h.ticks(5);
    let wave = &h.snapshot(0).basic_waves.as_ref().unwrap()[0];
    assert_eq!(wave.pending.len(), 1);
    assert_ne!(wave.pending[0].target.id, blocked.id);
}

#[test]
fn basic_wave_replica_rejects_invalid_schedule_without_mutation() {
    let mut h = host([None, None], |v| v["basic"] = wave_profile("fan"));
    wave_attack(&mut h, 0);
    h.ticks(5);
    let before = h.snapshot(0).clone();
    let mut full = h.latest(0);
    full.payload = PublicationPayloadV6::Full {
        snapshot: Box::new(before.clone()),
        reason: FullReason::Initial,
    };
    let mut replica = ReplicaV6::new(full.key.clone(), full.recipient_id.clone());
    replica.apply(&full).unwrap();
    let mut invalid = full;
    invalid.revision += 1;
    invalid.publication_id.push_str("-invalid");
    let PublicationPayloadV6::Full { snapshot, .. } = &mut invalid.payload else {
        unreachable!();
    };
    snapshot.basic_waves.as_mut().unwrap()[0].pending[0].at_ms = before.logical_ms;
    assert!(matches!(
        replica.apply(&invalid),
        Err(DecodeError::InvalidFrame)
    ));
    assert_eq!(replica.snapshot(), Some(&before));
}
