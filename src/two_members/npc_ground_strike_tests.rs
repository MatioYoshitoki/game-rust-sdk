use super::*;

const SKILL: &str = "tribulation-thunder-strike";
const SLOW: &str = "effect_tribulation_paralysis";

fn configure(v: &mut Value) {
    *v = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/mastery-stages.json"
    ))
    .unwrap();
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/npc-ground-strike.json"
    ))
    .unwrap();
    v["pve"]["combat"]["projectile"] = sample["npc_ability"].clone();
    v["pve"]["combat"]["cast_chance"] = json!(1);
    v["pve"]["combat"]["aggro_range"] = json!(700);
    v["pve"]["memory_range"] = json!(700);
    v["pve"]["monster_positions"] = json!([[280, 400]]);
    v["members"]["area_capacity"] = json!(4);
    v["combat_rules"]["npc_ground_strikes"] = sample["npc_ground_strikes"].clone();
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
}

fn host(seed: u32, change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let catalog = delta_catalog(2, |v| {
        configure(v);
        change(v);
    })
    .unwrap();
    let mut request = delta_request(&catalog.default_config());
    request.seed = seed;
    let mut h = DeltaHarness::from_request(catalog, request);
    for store in &mut h.stores {
        store.hp = 1000;
        store.spirit = 2_000_000;
    }
    h.active();
    h
}

fn until(h: &mut DeltaHarness, predicate: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..180 {
        if predicate(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("expected ground strike state");
}

fn warning(h: &mut DeltaHarness) -> AreaPlanViewV4 {
    let strike = |h: &DeltaHarness| {
        h.snapshot(0)
            .areas
            .as_ref()?
            .iter()
            .find(|a| a.mechanism == AreaPlanKind::NpcGroundStrike)
            .cloned()
    };
    until(h, |h| strike(h).is_some());
    strike(h).unwrap()
}

fn ended(h: &DeltaHarness, id: &str) -> Option<AreaEndReason> {
    h.events.iter().find_map(|(_, e)| match e {
        EventV4::AreaEnded { area_id, reason } if area_id == id => Some(*reason),
        _ => None,
    })
}

fn targets<'a>(h: &'a DeltaHarness, warning: &AreaPlanViewV4) -> Vec<&'a EntityRef> {
    let ticks: Vec<_> = h
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            EventV4::AreaTick {
                area_id,
                tick_index,
                scheduled_at_ms,
                targets,
            } if area_id == &warning.id => {
                assert_eq!(
                    (*tick_index, *scheduled_at_ms),
                    (1, warning.activates_at_ms)
                );
                Some(targets)
            }
            _ => None,
        })
        .collect();
    assert_eq!(ticks.len(), 1);
    ticks[0].iter().collect()
}

fn place(h: &mut DeltaHarness, i: usize, position: [f64; 2]) {
    h.engine.world.as_mut().unwrap().members[i].position = position;
}

fn slow(h: &DeltaHarness, i: usize) -> Option<&StatusView> {
    h.snapshot(i)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == SLOW)
}

#[test]
fn npc_ground_strike_keeps_the_warning_point_and_hits_the_nearest_current_member_once() {
    let mut h = host(1, |_| {});
    let strike = warning(&mut h);
    assert_eq!((strike.center, strike.radius), ([400., 400.], 96.));
    assert_eq!(strike.activates_at_ms - strike.created_at_ms, 750);
    assert_eq!(strike.next_tick_at_ms, Some(strike.activates_at_ms));
    assert_eq!(strike.expires_at_ms, strike.activates_at_ms);
    assert!(h.snapshot(0).projectiles.is_empty());
    place(&mut h, 0, [470., 400.]);
    place(&mut h, 1, [425., 400.]);
    until(&mut h, |h| ended(h, &strike.id).is_some());
    assert_eq!(ended(&h, &strike.id), Some(AreaEndReason::Resolved));
    assert_eq!(
        targets(&h, &strike),
        vec![&h.snapshot(1).own.participant.entity]
    );
    assert_eq!((h.snapshot(0).own.hp, h.snapshot(1).own.hp), (1000, 920));
    assert!(slow(&h, 0).is_none());
    let status = slow(&h, 1).unwrap();
    assert_eq!(
        (&status.source.id, status.source.generation),
        (&strike.source.id, strike.source.generation)
    );
    assert_eq!(
        (
            status.modifier_bps,
            status.expires_at_ms - status.applied_at_ms
        ),
        (-3000, 3000)
    );
    assert!(h.snapshot(1).areas.as_ref().unwrap().is_empty());
    assert!(h.snapshot(1).projectiles.is_empty());
}

#[test]
fn npc_ground_strike_uses_seeded_status_chance_and_stable_tie_break() {
    for (seed, chance, applied) in [
        (1, 1500, true),
        (0x12345678, 1500, false),
        (1, 0, false),
        (0x12345678, 10000, true),
    ] {
        let mut h = host(seed, |v| {
            v["combat_rules"]["npc_ground_strikes"][SKILL]["hit_status_chance_bps"] = json!(chance)
        });
        let strike = warning(&mut h);
        place(&mut h, 0, [398., 400.]);
        place(&mut h, 1, [402., 400.]);
        until(&mut h, |h| ended(h, &strike.id).is_some());
        assert_eq!(
            targets(&h, &strike),
            vec![&h.snapshot(0).own.participant.entity]
        );
        assert_eq!(
            slow(&h, 0).is_some(),
            applied,
            "seed {seed} chance {chance}"
        );
        assert!(slow(&h, 1).is_none());
    }
}

#[test]
fn npc_ground_strike_misses_empty_ground_and_does_not_retarget_an_immune_selection() {
    for immune in [false, true] {
        let mut h = host(1, |v| {
            v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({
                "mechanism":"timed_control","attribute":"damage_immunity","id":"strike-immunity",
                "duration_ms":2000,"duplicate_policy":"refresh","source_scoped":true
            }));
            v["additional_skills"].as_array_mut().unwrap().push(json!({
                "id":"strike-guard","mechanism":"self_status","cost":0,"windup_ms":30,"recovery_ms":30,"cooldown_ms":1000,
                "on_commit_self_statuses":["strike-immunity"]
            }));
        });
        let strike = warning(&mut h);
        if immune {
            h.action(
                0,
                Action::CastSelf {
                    skill_id: "strike-guard".into(),
                },
            )
            .unwrap();
        } else {
            place(&mut h, 0, [580., 400.]);
            place(&mut h, 1, [585., 460.]);
        }
        until(&mut h, |h| ended(h, &strike.id).is_some());
        assert_eq!(ended(&h, &strike.id), Some(AreaEndReason::Resolved));
        assert!(targets(&h, &strike).is_empty());
        assert_eq!((h.snapshot(0).own.hp, h.snapshot(1).own.hp), (1000, 1000));
        assert!(slow(&h, 0).is_none() && slow(&h, 1).is_none());
        h.ticks(2);
        assert!(h.snapshot(0).areas.as_ref().unwrap().is_empty());
    }
}

#[test]
fn npc_ground_strike_applies_reduction_before_shield_and_rolls_on_an_absorbed_contact() {
    let mut h = host(1, |v| {
        v["pve"]["status_definitions"].as_array_mut().unwrap().extend([
            json!({"id":"strike-reduction","mechanism":"timed_attribute_modifier","attribute":"damage_reduction","modifier_bps":2000,
                "duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
            json!({"id":"strike-shield","mechanism":"absorption_shield","shield_bps":700,"duration_ms":3000,
                "duplicate_policy":"refresh","source_scoped":true})
        ]);
        v["additional_skills"].as_array_mut().unwrap().push(json!({
            "id":"strike-guard","mechanism":"self_status","cost":0,"windup_ms":30,"recovery_ms":30,"cooldown_ms":1000,
            "on_commit_self_statuses":["strike-reduction","strike-shield"]
        }));
    });
    let strike = warning(&mut h);
    h.action(
        0,
        Action::CastSelf {
            skill_id: "strike-guard".into(),
        },
    )
    .unwrap();
    until(&mut h, |h| ended(h, &strike.id).is_some());
    assert_eq!(
        targets(&h, &strike),
        vec![&h.snapshot(0).own.participant.entity]
    );
    assert_eq!(h.snapshot(0).own.hp, 1000);
    assert_eq!(h.snapshot(0).own.shield_total, Some(6));
    assert!(slow(&h, 0).is_some());
}

#[test]
fn npc_ground_strike_cancels_on_source_death_or_replacement() {
    for replace in [false, true] {
        let mut h = host(1, |v| v["pve"]["stats"]["hp"] = json!(60));
        let strike = warning(&mut h);
        if replace {
            h.engine.world.as_mut().unwrap().replace_wave_test_npc(0);
        } else {
            h.action(
                0,
                Action::CastUnit {
                    skill_id: "seed-strike".into(),
                    target: UnitTarget {
                        id: strike.source.id.clone(),
                        generation: strike.source.generation,
                    },
                },
            )
            .unwrap();
        }
        until(&mut h, |h| ended(h, &strike.id).is_some());
        assert_eq!(ended(&h, &strike.id), Some(AreaEndReason::SourceInvalid));
        assert!(
            !h.events
                .iter()
                .any(|(_, e)| matches!(e,EventV4::AreaTick{area_id,..} if area_id==&strike.id))
        );
        assert_eq!(h.snapshot(0).own.hp, 1000);
        assert!(slow(&h, 0).is_none());
    }
}

#[test]
fn npc_ground_strike_revalidates_the_locked_point_from_the_current_source_position() {
    let mut h = host(1, |v| {
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        wall["blocksVision"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let first = 11 * 32 + 8;
        v["pve"]["cells_rle"] = json!([[0, first], [1, 1], [0, 1024 - first - 1]]);
    });
    let strike = warning(&mut h);
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(0, [280., 520.]);
    until(&mut h, |h| ended(h, &strike.id).is_some());
    assert_eq!(ended(&h, &strike.id), Some(AreaEndReason::Cancelled));
    assert!(
        !h.events
            .iter()
            .any(|(_, e)| matches!(e, EventV4::AreaTick{area_id,..} if area_id==&strike.id))
    );
    assert_eq!((h.snapshot(0).own.hp, h.snapshot(1).own.hp), (1000, 1000));
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(0, [280., 400.]);
    let next = warning(&mut h);
    assert_ne!(next.id, strike.id);
    assert!(next.created_at_ms < strike.activates_at_ms + 3000);
}

#[test]
fn npc_ground_strike_is_absent_from_frozen_departing_member_publications() {
    let mut h = host(1, |_| {});
    let strike = warning(&mut h);
    h.hold_settle[1] = true;
    h.action(1, Action::Leave {}).unwrap();
    h.pump();
    assert_eq!(h.snapshot(1).own.phase, MemberPhase::Settling);
    assert!(h.snapshot(1).areas.as_ref().unwrap().is_empty());
    assert!(
        h.snapshot(0)
            .areas
            .as_ref()
            .unwrap()
            .iter()
            .any(|a| a.id == strike.id)
    );
    until(&mut h, |h| ended(h, &strike.id).is_some());
    assert_eq!(
        targets(&h, &strike),
        vec![&h.snapshot(0).own.participant.entity]
    );
    assert!(h.snapshot(1).logical_ms >= strike.activates_at_ms);
    assert!(h.snapshot(1).areas.as_ref().unwrap().is_empty());
}

#[test]
fn npc_ground_strike_rejects_invalid_and_conflicting_configs() {
    for key in ["radius", "target_max_hp_bps", "hit_status_chance_bps"] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                v["combat_rules"]["npc_ground_strikes"][SKILL][key] = json!(10001);
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(2, |v| {
            configure(v);
            v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!([]);
        })
        .is_err()
    );
    assert!(
        delta_catalog(2, |v| {
            configure(v);
            let rules = v["combat_rules"]["npc_ground_strikes"]
                .as_object_mut()
                .unwrap();
            let row = rules.remove(SKILL).unwrap();
            rules.insert("unknown".into(), row);
        })
        .is_err()
    );
    assert!(
        delta_catalog(2, |v| {
            configure(v);
            v["combat_rules"]["npc_ground_fields"][SKILL] = json!({
                "radius":96,"duration_ms":1000,"interval_ms":1000,"max_targets_per_tick":1,
                "max_total_hits":1,"end_on_source_invalid":true,"target_max_hp_bps":800
            });
        })
        .is_err()
    );
}
