use super::*;

const SKILL: &str = "suiyue-ruge";
const BASE_STATUS: &str = "effect_suiyue_immunity";
const MAX_HP: u32 = 1000;
const MAX_SPIRIT: u32 = 6_000_000;
const COST: u32 = 5_000_000;
const HP_TICK: u32 = 50;
const SPIRIT_TICK: u32 = 300_000;

fn host(stages: [usize; 2]) -> DeltaHarness {
    let catalog = catalog_with(|v| {
        *v = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/mastery-stages.json"
        ))
        .unwrap();
        let tag = v["additional_skills"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|s| s["id"] == "tag-debuff")
            .unwrap();
        tag["recovery_ms"] = json!(0);
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/source-suiyue-ruge.json"
        ))
        .unwrap();
        v["additional_skills"]
            .as_array_mut()
            .unwrap()
            .push(sample["skill"].clone());
        v["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .extend(
                sample["status_definitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
        for (table, entries) in sample["combat_rules"].as_object().unwrap() {
            let target = &mut v["combat_rules"][table];
            if target.is_null() {
                *target = json!({});
            }
            target
                .as_object_mut()
                .unwrap()
                .extend(entries.as_object().unwrap().clone());
        }
        v["resource_limits"]["max_spirit"] = json!(MAX_SPIRIT);
        v["resource_limits"]["max_spirit_cost"] = json!(COST);
    })
    .unwrap();
    let config = catalog.default_config();
    let mut request = create(&config);
    request.protocol_version = 6;
    for (seat, mastery) in request.participants.iter_mut().zip(stages) {
        seat.combat_build = Some(CombatBuild {
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
    let mut h = DeltaHarness::from_request(catalog, request);
    for s in &mut h.stores {
        s.hp = MAX_HP;
        s.spirit = MAX_SPIRIT;
    }
    h.active();
    h
}

fn cast(skill_id: &str) -> Action {
    Action::CastSelf {
        skill_id: skill_id.into(),
    }
}

fn status<'a>(h: &'a DeltaHarness, i: usize, definition_id: &str) -> Option<&'a StatusView> {
    h.snapshot(i)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == definition_id)
}

fn selected_id(stage: usize) -> String {
    if stage == 0 {
        BASE_STATUS.into()
    } else {
        format!("{BASE_STATUS}_stage_{}", stage + 1)
    }
}

#[test]
fn source_time_current_five_stages_use_native_deadlines_and_exact_resource_amounts() {
    for stages in [[0, 1], [2, 3], [4, 0]] {
        let mut h = host(stages);
        for member in &mut h.engine.world.as_mut().unwrap().members {
            member.hp = 100;
        }
        for (i, stage) in stages.into_iter().enumerate() {
            let r = h.input(i, cast(SKILL));
            let accepted = h.engine.input(r.clone()).unwrap();
            assert_eq!(h.engine.input(r).unwrap(), accepted);
            h.pump();
            let own = &h.snapshot(i).own;
            let s = status(&h, i, &selected_id(stage)).unwrap();
            let periodic = s.periodic.as_ref().unwrap();
            let duration = if stage == 0 { 10000 } else { 12000 };
            assert_eq!(s.expires_at_ms - s.applied_at_ms, duration);
            assert_eq!(periodic.total_ticks, if stage == 0 { 9 } else { 11 });
            assert_eq!(periodic.next_tick_at_ms, Some(s.applied_at_ms + 1000));
            assert_eq!(s.source, s.target);
            assert_eq!(s.source.id, own.participant.entity.id);
            assert_eq!(own.spirit, MAX_SPIRIT - COST);
            assert_eq!(own.recovery_until_ms, s.applied_at_ms + 80);
            assert!(
                own.player_skills
                    .iter()
                    .any(|row| row.skill_id == SKILL && row.ready_at_ms == s.applied_at_ms + 60000)
            );
        }
        h.ticks(410);
        for (i, stage) in stages.into_iter().enumerate() {
            let own = &h.snapshot(i).own;
            let id = selected_id(stage);
            let total = if stage == 0 { 9 } else { 11 };
            let owner = &own.participant.entity.id;
            let ticks: Vec<_> = h
                .events
                .iter()
                .filter_map(|(recipient, event)| match event {
                    EventV4::Private {
                        event:
                            Event::PeriodicSpiritTick {
                                definition_id,
                                source,
                                target,
                                tick_index,
                                scheduled_at_ms,
                                raw_spirit,
                                base_restored,
                                converted_spirit,
                                ..
                            },
                    } if definition_id == &id && source.id == *owner => {
                        assert_eq!(source, target);
                        assert_eq!(recipient.as_ref(), Some(owner));
                        assert_eq!(
                            (*raw_spirit, *base_restored, *converted_spirit),
                            (SPIRIT_TICK, SPIRIT_TICK, 0)
                        );
                        Some((*tick_index, *scheduled_at_ms))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                ticks,
                (1..=total)
                    .map(|j| (j, u64::from(j) * 1000))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                own.hp,
                100 + total * HP_TICK + if stage == 4 { 100 } else { 0 }
            );
            assert_eq!(
                own.spirit,
                MAX_SPIRIT - COST + total * SPIRIT_TICK + if stage == 4 { 600_000 } else { 0 }
            );
            assert!(status(&h, i, &id).is_none());
            assert_eq!(h.events.iter().filter(|(recipient,event)| {
                matches!(event, EventV4::Private {event: Event::WindowRecordEnded {
                    definition_id, raw_hp:100, hp_restored:100, raw_spirit:600_000, spirit_restored:600_000,
                    reason: StatusRemovalReason::Expired, ..
                }} if definition_id==&id && recipient.as_ref()==Some(owner))
            }).count(), usize::from(stage == 4));
        }
    }
}

#[test]
fn source_time_current_stage_three_cleanses_same_step_and_stage_four_converts_only_when_full() {
    let mut h = host([2, 3]);
    for i in 0..2 {
        h.action(i, cast("tag-debuff")).unwrap();
    }
    h.ticks(2);
    h.action(1, cast(SKILL)).unwrap();
    h.pump();
    // A applies its older debuff on the same World step as the renewal.
    h.ticks(4);
    h.action(0, cast("tag-debuff")).unwrap();
    h.ticks(1);
    h.action(0, cast(SKILL)).unwrap();
    h.pump();
    h.ticks(36);
    assert!(status(&h, 0, "test-debuff").is_none());
    assert!(status(&h, 1, "test-debuff").is_some());
    assert_eq!(h.snapshot(0).own.spirit, MAX_SPIRIT - COST + SPIRIT_TICK);
    assert_eq!(
        h.snapshot(1).own.spirit,
        MAX_SPIRIT - COST + SPIRIT_TICK + 25
    );
    assert!(h.events.iter().any(|(_, e)| matches!(e, EventV4::Private {
        event: Event::StatusCleanseResolved {definition_id, tick_index:Some(1), removed_count:1, ..}
    } if definition_id == &selected_id(2))));
    let _ = h.action(0, cast("tag-debuff"));
    h.ticks(2);
    assert!(status(&h, 0, "test-debuff").is_none());
    h.ticks(400);
    assert!(status(&h, 1, "test-debuff").is_none());
}

#[test]
fn source_time_current_exit_clears_window_without_expiry_reward() {
    let mut h = host([4, 4]);
    for i in 0..2 {
        h.action(i, cast(SKILL)).unwrap();
    }
    h.ticks(34);
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(status(&h, 0, &selected_id(4)).is_none());
    assert!(status(&h, 1, &selected_id(4)).is_some());
    h.close();
    assert_eq!(
        h.stores.iter().map(|s| s.settlements).collect::<Vec<_>>(),
        [1, 1]
    );
    assert!(!h.events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::WindowRecordEnded {
                reason: StatusRemovalReason::Expired,
                ..
            }
        }
    )));
}
