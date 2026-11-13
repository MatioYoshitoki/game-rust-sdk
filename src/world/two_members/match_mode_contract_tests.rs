use super::*;

fn mode_input(v: &mut Value, stages: Value) {
    v["pve"]["combat"]["aggro_range"] = json!(2);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["status_definitions"] = json!([
        {"id":"ward","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":300,
         "duplicate_policy":"refresh","source_scoped":true}
    ]);
    v["match_mode"] = json!({
        "stages":stages,
        "results":{
            "win":{"result_id":"passed","policy":"preserve"},
            "loss":{"result_id":"failed","policy":"attrition"},
            "draw":{"result_id":"tied","policy":"preserve"}
        }
    });
}

pub(super) fn contest_input(v: &mut Value) {
    mode_input(
        v,
        json!([{"id":"round","duration_ms":600,"objective":{"kind":"contest"}}]),
    );
    v["skill"]["cooldown_ms"] = json!(30);
    v["skill"]["recovery_ms"] = json!(0);
}

fn course_input(v: &mut Value) {
    mode_input(
        v,
        json!([
            {"id":"wait","duration_ms":120,"objective":{"kind":"wait"}},
            {"id":"gate","duration_ms":180,"objective":{"kind":"checkpoint","schedule_id":"route","members_required":2}},
            {"id":"clear","duration_ms":300,"objective":{"kind":"clear_targets","target_ids":["npc-1"]}}
        ]),
    );
    v["members"]["checkpoint_schedules"] = json!([{
        "id":"route","first_delay_ms":120,"interval_ms":90,"window_ms":90,"radius":12,
        "required":1,"centers":[[20,30]]
    }]);
    v["pve"]["stats"]["hp"] = json!(10);
}

fn cast(target: &EntityRef) -> Action {
    Action::CastUnit {
        skill_id: "skill".into(),
        target: World::unit_wire(target),
    }
}

fn mode(w: &World, c: &Config, seat: usize, now: u64) -> ModeView {
    w.snapshot(seat, c, Phase::Active, now).own.mode.unwrap()
}

fn survival_waves(v: &mut Value) {
    mode_input(
        v,
        json!([
            {"id":"hold","duration_ms":120,"objective":{"kind":"survive","members_required":1}},
            {"id":"course","duration_ms":450,"objective":{"kind":"waves","waves":[
                {"delay_ms":30,"target_ids":["npc-1"]},
                {"delay_ms":90,"target_ids":["npc-2"]}
            ]}}
        ]),
    );
    v["pve"]["stats"]["hp"] = json!(10);
    v["pve"]["monster_positions"] = json!([[52, 20], [52, 40]]);
    v["skill"]["cooldown_ms"] = json!(30);
    v["skill"]["recovery_ms"] = json!(0);
}

#[test]
fn survival_freezes_eliminated_seat_and_each_wave_opens_after_its_own_delay() {
    let c = catalog(input(), |v| {
        survival_waves(v);
        v["world_loot"] = json!({
            "environment_capacity":8,"container_capacity":4,"partial_pickup":true,
            "interaction_range":16,"shared_clearance":5,"nodes":[],
            "pools":{"box":{"guaranteed":[{"item_id":"item","quantity_min":1,"quantity_max":1}],
                            "independent":[],"groups":[]}},
            "containers":[{"id":"box","position":[20,26],"pool":"box",
                "visibility":{"kind":"shared"},"guardians":["npc-1"],
                "unlock_ms":30,"replenish_ms":60,"ttl_ms":600}]
        });
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let first = w.npcs[0].entity.clone();
    let second = w.npcs[1].entity.clone();
    assert!(w.snapshot(0, &c, Phase::Active, 0).monsters.is_empty());
    let open = Action::OpenChest {
        chest_id: "box".into(),
        expected_generation: 1,
    };
    assert_eq!(
        w.action(0, &open, "guard-waiting", &c, 0),
        Err(Error::InvalidTarget)
    );
    w.protect(1, 30, &mut vec![]);
    for now in [30, 60, 90, 120] {
        w.tick(&c, now);
    }
    let lost = mode(&w, &c, 1, 120);
    assert_eq!(
        (lost.stage_id.as_str(), lost.outcome),
        ("hold", Some(Outcome::Loss))
    );
    assert_eq!(
        mode(&w, &c, 0, 120).wave,
        Some(ModeWaveView {
            index: 1,
            total: 2,
            opens_at_ms: 150,
            opened: false
        })
    );
    assert_eq!(
        w.action(0, &cast(&first), "before-wave", &c, 120),
        Err(Error::InvalidTarget)
    );
    w.tick(&c, 150);
    assert!(
        w.spectator_snapshot(&c, Phase::Active, 150)
            .unwrap()
            .stage
            .unwrap()
            .wave
            .unwrap()
            .opened
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 150).monsters.len(), 1);
    assert_eq!(
        w.action(0, &cast(&second), "before-second", &c, 150),
        Err(Error::InvalidTarget)
    );
    w.action(0, &cast(&first), "clear-first", &c, 150).unwrap();
    w.tick(&c, 180);
    w.action(0, &open, "guard-cleared", &c, 180).unwrap();
    assert_eq!(
        mode(&w, &c, 0, 180).wave,
        Some(ModeWaveView {
            index: 2,
            total: 2,
            opens_at_ms: 270,
            opened: false
        })
    );
    for now in [210, 240, 270] {
        w.tick(&c, now);
    }
    assert_eq!(w.snapshot(0, &c, Phase::Active, 270).monsters.len(), 2);
    assert_eq!(w.npcs[1].hp, 10);
    w.action(0, &cast(&second), "clear-second", &c, 270)
        .unwrap();
    w.tick(&c, 300);
    assert_eq!(mode(&w, &c, 0, 300).outcome, Some(Outcome::Win));
    assert_eq!(mode(&w, &c, 1, 300), lost);
    assert_eq!(w.members[0].tally["npc"], 2);
    assert!(w.tick(&c, 330).is_empty());
}

#[test]
fn survival_quorum_and_wave_deadline_fail_without_advancing_or_committing_late_damage() {
    let c = catalog(input(), survival_waves).unwrap().default_config();
    let mut failed = world(&c);
    for i in 0..2 {
        failed.protect(i, 0, &mut vec![]);
    }
    failed.tick(&c, 120);
    assert!((0..2).all(|i| mode(&failed, &c, i, 120).outcome == Some(Outcome::Loss)));
    assert!(
        failed
            .snapshot(0, &c, Phase::Active, 120)
            .monsters
            .is_empty()
    );
    let mut w = world(&c);
    for now in (30..=150).step_by(30) {
        w.tick(&c, now);
    }
    let first = w.npcs[0].entity.clone();
    w.action(0, &cast(&first), "clear-first", &c, 150).unwrap();
    for now in (180..=540).step_by(30) {
        w.tick(&c, now);
    }
    let second = w.npcs[1].entity.clone();
    w.action(0, &cast(&second), "late", &c, 540).unwrap();
    let events = w.tick(&c, 570);
    assert_eq!(w.npcs[1].hp, 10);
    assert!((0..2).all(|i| mode(&w, &c, i, 570).outcome == Some(Outcome::Loss)));
    assert_eq!(
        events
            .iter()
            .filter(|(_, e)| matches!(e, EventV4::ModeConcluded { .. }))
            .count(),
        2
    );
    assert!(w.tick(&c, 600).is_empty());
}

#[test]
fn wave_targets_and_timing_are_validated_at_startup() {
    for (pointer, value) in [
        ("/match_mode/stages/0/objective/members_required", json!(3)),
        ("/match_mode/stages/1/objective/waves/1/delay_ms", json!(91)),
        (
            "/match_mode/stages/1/objective/waves/1/target_ids",
            json!(["npc-1"]),
        ),
        (
            "/match_mode/stages/1/objective/waves/1/delay_ms",
            json!(420),
        ),
    ] {
        let invalid = catalog(input(), |v| {
            survival_waves(v);
            *v.pointer_mut(pointer).unwrap() = value;
        });
        assert!(
            invalid
                .err()
                .unwrap()
                .to_string()
                .contains("CONFIG_MATCH_MODE_OBJECTIVE")
        );
    }
}

#[test]
fn ordered_course_requires_each_checkpoint_receipt_then_a_real_npc_defeat() {
    let c = catalog(input(), course_input).unwrap().default_config();
    let mut w = world(&c);
    let npc = w.npcs[0].entity.clone();
    let claim = Action::CompleteModeCheckpoint {
        stage_id: "gate".into(),
    };
    assert_eq!(mode(&w, &c, 0, 0).objective, ModeObjective::Wait);
    assert!(!w.snapshot(0, &c, Phase::Active, 0).extraction_open);
    assert_eq!(
        w.action(0, &cast(&npc), "waiting", &c, 0).unwrap_err(),
        Error::InvalidState
    );
    for now in [30, 60, 90, 120] {
        w.tick(&c, now);
    }
    assert_eq!(mode(&w, &c, 0, 120).stage_index, 2);
    assert_eq!(
        w.action(0, &claim, "no-receipt", &c, 120).unwrap_err(),
        Error::InvalidState
    );
    assert_eq!(
        w.action(0, &cast(&npc), "future-target", &c, 120)
            .unwrap_err(),
        Error::InvalidTarget
    );
    assert_eq!(
        w.action(0, &Action::Attack {}, "auto-target", &c, 120)
            .unwrap_err(),
        Error::InvalidTarget
    );
    w.tick(&c, 150);
    w.action(0, &claim, "claim-a", &c, 150).unwrap();
    assert!(mode(&w, &c, 0, 150).submitted);
    assert!(!mode(&w, &c, 1, 150).submitted);
    assert_eq!(
        w.action(0, &claim, "duplicate", &c, 150).unwrap_err(),
        Error::InvalidState
    );
    let events = w.action(1, &claim, "claim-b", &c, 150).unwrap();
    assert!(events.iter().any(|(seat, event)| seat.is_none()
        && matches!(event, EventV4::ModeStageOpened { stage_index: 3, .. })));
    assert_eq!(
        w.action(1, &claim, "old-stage", &c, 150).unwrap_err(),
        Error::InvalidState
    );
    w.action(0, &cast(&npc), "clear", &c, 150).unwrap();
    let events = w.tick(&c, 180);
    assert_eq!(w.npcs[0].hp, 0);
    assert_eq!(w.members[0].tally["npc"], 1);
    assert_eq!(
        events
            .iter()
            .filter_map(|(seat, event)| match event {
                EventV4::ModeConcluded {
                    outcome: Outcome::Win,
                    result,
                } if result.result_id == "passed" => *seat,
                _ => None,
            })
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(w.members.iter().all(|m| m.desired == Some(Outcome::Win)));
    assert!(w.tick(&c, 210).is_empty());
}

#[test]
fn checkpoint_deadline_does_not_advance_an_incomplete_course() {
    let c = catalog(input(), course_input).unwrap().default_config();
    let mut w = world(&c);
    for now in (30..300).step_by(30) {
        w.tick(&c, now);
    }
    w.action(
        0,
        &Action::CompleteModeCheckpoint {
            stage_id: "gate".into(),
        },
        "last-valid",
        &c,
        270,
    )
    .unwrap();
    let events = w.tick(&c, 300);
    assert!(w.members.iter().all(|m| m.desired == Some(Outcome::Loss)));
    assert_eq!(mode(&w, &c, 1, 300).stage_index, 2);
    assert!(
        events
            .iter()
            .all(|(_, e)| !matches!(e, EventV4::ModeStageOpened { .. }))
    );
    assert_eq!(w.npcs[0].hp, 10);
}

#[test]
fn lethal_damage_respawns_once_under_a_new_combat_generation_then_ends_the_duel() {
    let c = catalog(input(), |v| {
        contest_input(v);
        v["match_mode"]["respawn"] =
            json!({"max_per_member":1,"delay_ms":60,"hp_bps":5000,"spirit_bps":2500});
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let a = w.members[0].reference.entity.clone();
    let b = w.members[1].reference.entity.clone();
    w.members[1].facts.atk_points = 100;
    w.apply_owned_statuses(
        Owner::Member(1),
        (b.clone(), 100),
        &["ward".into()],
        &c,
        0,
        &mut vec![],
    );
    w.action(
        0,
        &Action::AttackUnit {
            target: World::unit_wire(&b),
        },
        "old-attack",
        &c,
        0,
    )
    .unwrap();
    w.action(1, &cast(&a), "knockout", &c, 0).unwrap();
    let events = w.tick(&c, 30);
    assert_eq!(w.members[0].hp, 0);
    assert_eq!(w.members[0].desired, None);
    assert_eq!(mode(&w, &c, 0, 30).respawn_at_ms, Some(90));
    assert!(w.shots.is_empty());
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "ward" && s.target == status_entity(&b))
    );
    assert!(
        events
            .iter()
            .all(|(_, e)| !matches!(e, EventV4::ModeConcluded { .. }))
    );
    assert_eq!(
        w.action(0, &cast(&b), "down", &c, 30).unwrap_err(),
        Error::NotReady
    );
    w.tick(&c, 60);
    let events = w.tick(&c, 90);
    let revived = w.members[0].reference.entity.clone();
    assert_eq!(
        (revived.generation, w.members[0].hp, w.members[0].spirit),
        (2, 50, 25)
    );
    assert!(
        events
            .iter()
            .any(|(_, e)| matches!(e, EventV4::ModeRespawned { entity } if entity == &revived))
    );
    assert_eq!(
        w.action(1, &cast(&a), "stale", &c, 90).unwrap_err(),
        Error::InvalidTarget
    );
    w.action(1, &cast(&revived), "eliminate", &c, 90).unwrap();
    w.tick(&c, 120);
    assert_eq!(w.members[0].desired, Some(Outcome::Loss));
    assert_eq!(w.members[1].desired, Some(Outcome::Win));
}

#[test]
fn contest_deadline_compares_exact_hp_ratios_and_draw_keeps_inventory() {
    let c = catalog(input(), |v| {
        contest_input(v);
        v["params"]["death"] = json!({"slot_ratio":1,"quantity_ratio":1,"minimum_quantity":1});
    })
    .unwrap()
    .default_config();
    for (b_hp, expected) in [
        (60, [Outcome::Draw, Outcome::Draw]),
        (59, [Outcome::Win, Outcome::Loss]),
    ] {
        let mut w = world(&c);
        w.members[0].hp = 30;
        w.members[1].facts.hp_points = 200;
        w.members[1].hp = b_hp;
        let a = w.members[0].reference.entity.clone();
        w.action(1, &cast(&a), "deadline-contact", &c, 570).unwrap();
        w.tick(&c, 600);
        assert_eq!(w.members[0].hp, 30);
        for (seat, expected) in expected.into_iter().enumerate() {
            assert_eq!(w.members[seat].desired, Some(expected));
            if expected == Outcome::Draw {
                assert!(matches!(
                    w.settlement(seat, &c),
                    Business::CommitSettlement {
                        outcome: Outcome::Draw,
                        kept: 1,
                        dropped: 0,
                        ..
                    }
                ));
            }
        }
    }
}

#[test]
fn mode_configuration_rejects_unbound_targets_ambiguous_results_and_time_budgets() {
    for (pointer, value, error) in [
        (
            "/match_mode/stages/1/objective/schedule_id",
            json!("unknown"),
            "CONFIG_MATCH_MODE_OBJECTIVE",
        ),
        (
            "/match_mode/stages/2/objective/target_ids",
            json!(["npc-1", "npc-1"]),
            "CONFIG_MATCH_MODE_OBJECTIVE",
        ),
        (
            "/match_mode/stages/2/duration_ms",
            json!(3000),
            "CONFIG_MATCH_MODE_BOUNDS",
        ),
        (
            "/match_mode/stages/2/duration_ms",
            json!(301),
            "CONFIG_MATCH_MODE_STAGES",
        ),
        (
            "/match_mode/results/draw/result_id",
            json!("extract"),
            "CONFIG_MATCH_MODE_RESULTS",
        ),
        (
            "/match_mode/results/draw/policy",
            json!("attrition"),
            "CONFIG_MATCH_MODE_RESULTS",
        ),
    ] {
        let result = catalog(input(), |v| {
            course_input(v);
            *v.pointer_mut(pointer).unwrap() = value;
        });
        assert!(
            result.err().unwrap().to_string().contains(error),
            "{pointer}"
        );
    }
}

#[test]
fn simultaneous_lethal_terrain_in_a_contest_produces_two_draws() {
    let c = catalog(input(), |v| {
        contest_input(v);
        v["pve"]["palette"][0]["dotDamagePerSecond"] = json!(100);
        v["pve"]["world_geometry"] =
            json!({"boundary":[[0,0],[128,0],[128,128],[0,128]],"blockers":[]});
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    for m in &mut w.members {
        m.hp = 1;
    }
    let events = w.tick(&c, 30);
    assert_eq!(
        events
            .iter()
            .filter(|(_, event)| matches!(event, EventV4::TerrainDamage {
        target, hp:0, ..
    } if target.kind == StatusEntityKind::Participant))
            .count(),
        2
    );
    assert!(w.members.iter().all(|m| m.desired == Some(Outcome::Draw)));
}
