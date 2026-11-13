use super::*;

const SKILL: &str = "tribulation-wind-blade";
const SLOW: &str = "effect_tribulation_windbound";

fn configure(v: &mut Value) {
    *v = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/mastery-stages.json"
    ))
    .unwrap();
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/moving-enter-area.json"
    ))
    .unwrap();
    v["pve"]["combat"]["projectile"] = sample["npc_ability"].clone();
    v["pve"]["combat"]["cast_chance"] = json!(1);
    v["pve"]["combat"]["aggro_range"] = json!(700);
    v["pve"]["memory_range"] = json!(700);
    v["pve"]["monster_positions"] = json!([[280, 400]]);
    v["members"]["area_capacity"] = json!(4);
    v["combat_rules"]["npc_enter_areas"] = sample["npc_enter_areas"].clone();
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

fn host(change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let catalog = delta_catalog(2, |v| {
        configure(v);
        change(v);
    })
    .unwrap();
    let mut h =
        DeltaHarness::from_request(catalog.clone(), delta_request(&catalog.default_config()));
    for store in &mut h.stores {
        store.hp = 1000;
        store.spirit = 2_000_000;
    }
    h.active();
    h
}

fn area(h: &DeltaHarness) -> Option<&AreaPlanViewV4> {
    h.snapshot(0)
        .areas
        .as_ref()?
        .iter()
        .find(|a| a.mechanism == AreaPlanKind::MovingEnter)
}

fn until(h: &mut DeltaHarness, predicate: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..200 {
        if predicate(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("expected moving area state");
}

fn entered(h: &DeltaHarness, id: &str) -> usize {
    h.events
        .iter()
        .filter(|(_, e)| matches!(e, EventV4::AreaEntered{area_id,..} if area_id == id))
        .count()
}

fn committed(h: &mut DeltaHarness) -> String {
    until(h, |h| {
        area(h).is_some_and(|a| a.movement.as_ref().unwrap().committed)
    });
    area(h).unwrap().id.clone()
}

#[test]
fn moving_enter_area_source_values_publish_warning_and_apply_one_hit_group() {
    let mut h = host(|_| {});
    until(&mut h, |h| area(h).is_some());
    let first = area(&h).unwrap();
    assert_eq!(first.activates_at_ms - first.created_at_ms, 750);
    assert_eq!((first.center, first.radius), ([400., 400.], 60.));
    let motion = first.movement.as_ref().unwrap();
    assert_eq!((motion.speed, motion.committed), (400., false));
    assert!((motion.distance - 880.).abs() < 1e-5);
    assert_eq!(h.snapshot(0).own.hp, 1000);
    let id = committed(&mut h);
    assert_eq!(h.snapshot(0).own.position, [600., 400.]);
    assert_eq!(h.snapshot(0).own.hp, 950);
    assert_eq!(h.snapshot(1).own.hp, 1000);
    assert!(h.snapshot(0).projectiles.is_empty());
    let slow = h
        .snapshot(0)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == SLOW)
        .unwrap();
    assert_eq!(
        (slow.modifier_bps, slow.expires_at_ms - slow.applied_at_ms),
        (-2000, 3000)
    );
    assert!(h.events.iter().any(
        |(_, e)| matches!(e,EventV4::ForcedDisplacement{area_id,from,to,slide_speed_multiplier,..}
        if area_id==&id && *from==[400.,400.] && *to==[600.,400.] && *slide_speed_multiplier==3.)
    ));
    until(&mut h, |h| area(h).is_none());
    assert_eq!(entered(&h, &id), 1);
}

#[test]
fn moving_enter_area_clips_push_to_the_member_footprint() {
    let mut h = host(|v| {
        let mut wall = v["pve"]["palette"][0].clone();
        wall["walkable"] = json!(false);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let first = 9 * 32 + 13;
        v["pve"]["cells_rle"] =
            json!([[0, first], [1, 1], [0, 31], [1, 1], [0, 1024 - first - 33]]);
    });
    let id = committed(&mut h);
    let p = h.snapshot(0).own.position;
    assert!(p[0] > 503.9 && p[0] < 504. && p[1] == 400.);
    assert_eq!(h.snapshot(0).own.hp, 950);
    assert_eq!(entered(&h, &id), 1);
}

#[test]
fn moving_enter_area_requires_reentry_after_interval() {
    let mut h = host(|v| {
        let area = &mut v["combat_rules"]["npc_enter_areas"][SKILL];
        area["speed"] = json!(5);
        area["knockback"]["distance"] = json!(1);
        area["contact_policy"]["max_total_hits"] = json!(2);
        area["contact_policy"]["repeat_interval_ms"] = json!(150);
    });
    let id = committed(&mut h);
    h.ticks(1);
    assert_eq!(entered(&h, &id), 1);
    let place =
        |h: &mut DeltaHarness, pos| h.engine.world.as_mut().unwrap().members[0].position = pos;
    place(&mut h, [400., 520.]);
    h.ticks(1);
    place(&mut h, [400., 400.]);
    h.ticks(1);
    assert_eq!(entered(&h, &id), 1);
    h.ticks(4);
    assert_eq!(entered(&h, &id), 1);
    place(&mut h, [400., 520.]);
    h.ticks(1);
    place(&mut h, [400., 400.]);
    h.ticks(1);
    assert_eq!(entered(&h, &id), 2);
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::AreaEnded{area_id,reason:AreaEndReason::HitBudgetExhausted} if area_id==&id)));
}

#[test]
fn moving_enter_area_sweeps_relative_motion_and_uses_shields_after_source_replacement() {
    let mut h = host(|v| {
        let statuses = v["pve"]["status_definitions"].as_array_mut().unwrap();
        statuses.extend([
            json!({"id":"area-test-reduction","mechanism":"timed_attribute_modifier","attribute":"damage_reduction","modifier_bps":2000,
                "duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true}),
            json!({"id":"area-test-shield","mechanism":"absorption_shield","shield_bps":300,"duration_ms":3000,
                "duplicate_policy":"refresh","source_scoped":true}),
        ]);
        v["additional_skills"].as_array_mut().unwrap().push(json!({
            "id":"area-test-guard","mechanism":"self_status","cost":0,"windup_ms":30,"recovery_ms":30,"cooldown_ms":1000,
            "on_commit_self_statuses":["area-test-reduction","area-test-shield"]
        }));
    });
    h.action(
        1,
        Action::CastSelf {
            skill_id: "area-test-guard".into(),
        },
    )
    .unwrap();
    let id = committed(&mut h);
    let now = h.engine.now + h.engine.config.step_ms;
    let c = h.engine.config.clone();
    let w = h.engine.world.as_mut().unwrap();
    w.replace_wave_test_npc(0);
    // Both of the second member's endpoints are outside the circle.
    let previous = [w.members[0].position, [400., 520.]];
    w.members[1].position = [400., 280.];
    let mut events = vec![];
    w.advance_npc_areas(Some(&previous), &c, now, &mut events);
    let target = &w.members[1].reference.entity;
    assert!(events.iter().any(
        |(_, e)| matches!(e,EventV4::AreaEntered{area_id,target:t,..} if area_id==&id && t==target)
    ));
    assert_eq!(w.members[1].hp, 990);
}

#[test]
fn moving_enter_area_keeps_hits_and_statuses_attributed_to_the_original_generation() {
    let mut h = host(|_| {});
    let id = committed(&mut h);
    let source = area(&h).unwrap().source.clone();
    let w = h.engine.world.as_mut().unwrap();
    w.replace_wave_test_npc(0);
    w.members[1].position = [470., 400.];
    h.ticks(8);
    assert_eq!(area(&h).unwrap().source, source);
    assert!(area(&h).unwrap().center[0] > 470.);
    assert_eq!(h.snapshot(1).own.hp, 950);
    assert_eq!(h.snapshot(1).own.position, [670., 400.]);
    assert_eq!(entered(&h, &id), 2);
    for i in 0..2 {
        let slow = h
            .snapshot(i)
            .own
            .statuses
            .iter()
            .find(|s| s.definition_id == SLOW)
            .unwrap();
        assert_eq!(slow.source.id, source.id);
        assert_eq!(slow.source.generation, source.generation);
        assert_eq!(slow.retained_source, Some(true));
    }
    let target = &h.snapshot(1).own.participant.entity;
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::Damage{source:s,target:t,amount:50,..} if s==&source && t==target)));
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::ForcedDisplacement{source:s,target:t,area_id,..} if s==&source && t==target && area_id==&id)));
    h.ticks(105);
    assert!((0..2).all(|i| {
        h.snapshot(i)
            .own
            .statuses
            .iter()
            .all(|s| s.definition_id != SLOW)
    }));
}

#[test]
fn moving_enter_area_hits_after_the_caster_is_killed_during_real_combat() {
    let mut h = host(|v| {
        v["pve"]["stats"]["hp"] = json!(50);
        v["members"]["spawns"][1] = json!([700, 400]);
        v["combat_rules"]["status_policies"][SLOW] =
            json!({"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]});
        v["combat_rules"]["triggers"].as_array_mut().unwrap().push(json!({
            "id":"area-test-cleanse","profiles":["member"],"when":"skill_committed",
            "include_periodic":false,"every":1,"cooldown_ms":0,"priority":0,
            "effect":{"kind":"dispel_statuses","subject":"owner","disposition":"harmful","max_count":1}
        }));
    });
    let id = committed(&mut h);
    let source = area(&h).unwrap().source.clone();
    h.action(
        0,
        Action::CastUnit {
            skill_id: "seed-strike".into(),
            target: UnitTarget {
                id: source.id.clone(),
                generation: source.generation,
            },
        },
    )
    .unwrap();
    until(&mut h, |h| h.snapshot(0).monsters[0].hp == 0);
    assert_eq!(h.snapshot(1).own.hp, 1000);
    until(&mut h, |h| h.snapshot(1).own.hp == 950);
    assert_eq!(h.snapshot(1).own.position, [900., 400.]);
    let slow = h
        .snapshot(1)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == SLOW)
        .unwrap();
    assert_eq!(slow.source.generation, source.generation);
    assert_eq!(slow.retained_source, Some(true));
    assert_eq!(entered(&h, &id), 2);
    let slow_id = slow.instance_id.clone();
    h.action(
        1,
        Action::CastSelf {
            skill_id: "tag-debuff".into(),
        },
    )
    .unwrap();
    until(&mut h, |h| {
        h.snapshot(1)
            .own
            .statuses
            .iter()
            .all(|s| s.definition_id != SLOW)
    });
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::Private{event:Event::StatusRemoved{instance_ids,reason:StatusRemovalReason::Cleansed}} if instance_ids.contains(&slow_id))));
}

#[test]
fn moving_enter_area_stops_at_invalid_source_when_configured() {
    let mut h = host(|v| {
        let area = v["combat_rules"]["npc_enter_areas"][SKILL]
            .as_object_mut()
            .unwrap();
        area.insert("end_on_source_invalid".into(), json!(true));
        area.remove("retained_hit_statuses");
        v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!([SLOW]);
    });
    let id = committed(&mut h);
    let world = h.engine.world.as_mut().unwrap();
    world.replace_wave_test_npc(0);
    world.members[1].position = [470., 400.];
    h.ticks(1);
    assert_eq!(h.snapshot(1).own.hp, 1000);
    assert!(
        h.snapshot(0)
            .own
            .statuses
            .iter()
            .all(|s| s.definition_id != SLOW)
    );
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::AreaEnded{area_id,reason:AreaEndReason::SourceInvalid} if area_id==&id)));
}

#[test]
fn moving_enter_area_uses_released_accuracy_when_the_successor_has_a_different_profile() {
    let mut h = host(|v| {
        v["pve"]["combat"]["projectile"]["unavoidable"] = json!(false);
        v["combat_rules"]["profiles"]["member"]["dodge_bps"] = json!(10000);
        v["combat_rules"]["profiles"]["npc"]["hit_bps"] = json!(0);
        v["combat_rules"]["dodge_cap_bps"] = json!(10000);
    });
    let id = committed(&mut h);
    assert_eq!(entered(&h, &id), 0);
    let mut c = h.engine.config.clone();
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .get_mut("npc")
        .unwrap()
        .hit_bps = 100_000_000;
    let now = h.engine.now + c.step_ms;
    let w = h.engine.world.as_mut().unwrap();
    w.replace_wave_test_npc(0);
    let previous = [w.members[0].position, w.members[1].position];
    w.members[1].position = [470., 400.];
    let mut events = vec![];
    w.advance_npc_areas(Some(&previous), &c, now, &mut events);
    assert_eq!(w.members[1].hp, 1000);
    assert!(
        events
            .iter()
            .all(|(_, e)| !matches!(e,EventV4::AreaEntered{area_id,..} if area_id==&id))
    );
}

#[test]
fn moving_enter_area_cancels_unreleased_warning_and_rejects_unbounded_config() {
    let mut h = host(|_| {});
    until(&mut h, |h| area(h).is_some());
    let id = area(&h).unwrap().id.clone();
    h.engine.world.as_mut().unwrap().replace_wave_test_npc(0);
    h.ticks(1);
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::AreaEnded{area_id,reason:AreaEndReason::SourceInvalid} if area_id==&id)));
    assert_eq!(h.snapshot(0).own.hp, 1000);
    for case in 0..7 {
        assert!(delta_catalog(2,|v| {
            configure(v);
            match case {
                0 => v["pve"]["combat"]["projectile"]["id"] = json!("unknown"),
                1 => v["combat_rules"]["npc_enter_areas"][SKILL]["knockback"]["distance"] = json!(1001),
                2 => v["combat_rules"]["npc_enter_areas"][SKILL]["contact_policy"]["max_total_hits"] = json!(128),
                3 => v["combat_rules"]["npc_enter_areas"][SKILL]["retained_hit_statuses"] = json!([SLOW,SLOW]),
                4 => v["combat_rules"]["npc_enter_areas"][SKILL]["retained_hit_statuses"] = json!(["unknown"]),
                5 => v["combat_rules"]["npc_enter_areas"][SKILL]["end_on_source_invalid"] = json!(true),
                _ => v["pve"]["combat"]["projectile"]["on_hit_target_statuses"] = json!([SLOW]),
            }
        }).is_err());
    }
}
