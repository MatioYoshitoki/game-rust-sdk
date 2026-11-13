use super::*;

const SKILL: &str = "tribulation-fire-pool";
const BURN: &str = "effect_burn";

fn configure(v: &mut Value) {
    *v = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/mastery-stages.json"
    ))
    .unwrap();
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/npc-ground-field.json"
    ))
    .unwrap();
    v["pve"]["combat"]["projectile"] = sample["npc_ability"].clone();
    v["pve"]["combat"]["cast_chance"] = json!(1);
    v["pve"]["combat"]["aggro_range"] = json!(700);
    v["pve"]["memory_range"] = json!(700);
    v["pve"]["monster_positions"] = json!([[280, 400]]);
    v["members"]["area_capacity"] = json!(4);
    v["combat_rules"]["npc_ground_fields"] = sample["npc_ground_fields"].clone();
    v["combat_rules"]["elements"]
        .as_array_mut()
        .unwrap()
        .push(json!("fire"));
    for key in ["periodic_damage", "periodic_sampling"] {
        v["combat_rules"][key][BURN] = sample[key][BURN].clone();
    }
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
    let mut h = DeltaHarness::new(2, |v| {
        configure(v);
        change(v);
    });
    for store in &mut h.stores {
        store.hp = 1000;
        store.spirit = 2_000_000;
    }
    h.active();
    h
}

fn until(h: &mut DeltaHarness, predicate: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..400 {
        if predicate(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("expected ground field state");
}

fn field(h: &DeltaHarness) -> Option<&AreaPlanViewV4> {
    h.snapshot(0)
        .areas
        .as_ref()?
        .iter()
        .find(|a| a.mechanism == AreaPlanKind::NpcGroundField)
}

fn place(h: &mut DeltaHarness, i: usize, position: [f64; 2]) {
    h.engine.world.as_mut().unwrap().members[i].position = position;
}

fn burn(h: &DeltaHarness, i: usize) -> Option<&StatusView> {
    h.snapshot(i)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == BURN)
}

#[test]
fn npc_ground_field_keeps_its_point_and_runs_eight_pulses_with_separate_entry_status() {
    let mut h = host(|_| {});
    until(&mut h, |h| field(h).is_some());
    let first = field(&h).unwrap().clone();
    assert_eq!((first.center, first.radius), ([400., 400.], 140.));
    assert_eq!(first.activates_at_ms - first.created_at_ms, 750);
    assert_eq!(first.expires_at_ms - first.activates_at_ms, 8000);
    place(&mut h, 0, [610., 400.]);
    place(&mut h, 1, [450., 400.]);
    until(&mut h, |h| burn(h, 1).is_some());
    let status = burn(&h, 1).unwrap().clone();
    assert_eq!(
        (
            &status.source.id,
            status.source.generation,
            status.source.kind
        ),
        (&first.source.id, first.source.generation, first.source.kind)
    );
    assert_eq!(status.expires_at_ms - status.applied_at_ms, 3000);
    assert_eq!(h.snapshot(1).own.hp, 1000);
    assert!(burn(&h, 0).is_none());
    assert!(h.snapshot(0).projectiles.is_empty());
    assert_eq!(field(&h).unwrap().center, first.center);
    until(&mut h, |h| field(h).unwrap().completed_ticks == 1);
    assert_eq!(
        field(&h)
            .unwrap()
            .contact_policy
            .as_ref()
            .unwrap()
            .used_hits,
        1
    );
    assert_eq!(h.snapshot(0).own.hp, 1000);
    assert!(h.events.iter().any(|(_, e)| matches!(e, EventV4::Damage{source,target,amount:10,..} if source==&first.source && target==&h.snapshot(1).own.participant.entity)));
    assert!(h.events.iter().any(|(_, e)| matches!(e, EventV4::Private{event: Event::PeriodicTick{instance_id,raw_damage:18,..}} if instance_id==&status.instance_id)));
    place(&mut h, 1, [610., 420.]);
    until(&mut h, |h| field(h).unwrap().completed_ticks == 2);
    let before_reentry = field(&h)
        .unwrap()
        .contact_policy
        .as_ref()
        .unwrap()
        .used_hits;
    assert_eq!(before_reentry, 1);
    place(&mut h, 1, [450., 400.]);
    until(&mut h, |h| {
        h.events.iter().any(|(_, e)| matches!(e, EventV4::AreaEnded{area_id,reason:AreaEndReason::Expired} if area_id==&first.id))
    });
    let pulses = h
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            EventV4::AreaTick {
                area_id,
                scheduled_at_ms,
                targets,
                ..
            } if area_id == &first.id => Some((*scheduled_at_ms, targets.len())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(pulses.len(), 8);
    assert_eq!(pulses.last(), Some(&(first.expires_at_ms, 1)));
    assert_eq!(pulses[1].1, 0);
    assert_eq!(
        h.events
            .iter()
            .filter(|(_, e)| matches!(e, EventV4::AreaEntered{area_id,..} if area_id==&first.id))
            .count(),
        1
    );
}

#[test]
fn npc_ground_field_retains_released_damage_but_does_not_move_burn_to_the_successor() {
    let mut h = host(|_| {});
    until(&mut h, |h| burn(h, 0).is_some());
    let first = field(&h).unwrap().clone();
    h.engine.world.as_mut().unwrap().replace_wave_test_npc(0);
    place(&mut h, 1, [430., 400.]);
    until(&mut h, |h| field(h).unwrap().completed_ticks == 1);
    assert_eq!(field(&h).unwrap().source, first.source);
    assert_eq!(
        field(&h)
            .unwrap()
            .contact_policy
            .as_ref()
            .unwrap()
            .used_hits,
        2
    );
    assert!(burn(&h, 0).is_none() && burn(&h, 1).is_none());
    assert_eq!(h.snapshot(1).own.hp, 990);
    assert!(h.events.iter().any(|(_, e)| matches!(e, EventV4::Damage{source,target,amount:10,..} if source==&first.source && target==&h.snapshot(1).own.participant.entity)));
}

#[test]
fn npc_ground_field_cancels_warning_and_configured_released_lifetime() {
    for committed in [false, true] {
        let mut h = host(|v| {
            v["combat_rules"]["npc_ground_fields"][SKILL]["end_on_source_invalid"] =
                json!(committed);
        });
        until(&mut h, |h| {
            field(h).is_some() && (!committed || burn(h, 0).is_some())
        });
        let id = field(&h).unwrap().id.clone();
        h.engine.world.as_mut().unwrap().replace_wave_test_npc(0);
        h.ticks(1);
        assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::AreaEnded{area_id,reason:AreaEndReason::SourceInvalid} if area_id==&id)));
        assert_eq!(h.snapshot(0).own.hp, 1000);
    }
}

#[test]
fn npc_ground_field_reuses_shared_geometry_and_successful_hit_budget() {
    let mut h = host(|v| {
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        wall["blocksVision"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let first = 10 * 32 + 11;
        v["pve"]["cells_rle"] = json!([[0, first], [1, 1], [0, 1024 - first - 1]]);
        let rule = &mut v["combat_rules"]["npc_ground_fields"][SKILL];
        rule["max_targets_per_tick"] = json!(1);
        rule["max_total_hits"] = json!(1);
    });
    until(&mut h, |h| field(h).is_some());
    let id = field(&h).unwrap().id.clone();
    place(&mut h, 1, [495., 410.]);
    until(&mut h, |h| {
        h.events.iter().any(|(_, e)| matches!(e, EventV4::AreaEnded{area_id,reason:AreaEndReason::HitBudgetExhausted} if area_id==&id))
    });
    assert!(burn(&h, 1).is_none());
    assert_eq!(h.snapshot(1).own.hp, 1000);
    assert!(h.events.iter().any(|(_, e)| matches!(e, EventV4::AreaTick{area_id,targets,..} if area_id==&id && targets==&vec![h.snapshot(0).own.participant.entity.clone()])));
}

#[test]
fn npc_ground_field_survives_one_exit_and_is_reclaimed_on_world_close() {
    let mut h = host(|_| {});
    until(&mut h, |h| burn(h, 0).is_some() && burn(h, 1).is_some());
    let id = field(&h).unwrap().id.clone();
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(
        h.snapshot(1)
            .areas
            .as_ref()
            .unwrap()
            .iter()
            .any(|a| a.id == id)
    );
    assert!(burn(&h, 1).is_some());
    h.close();
    assert!(h.events.iter().any(|(_, e)| matches!(e, EventV4::AreaEnded{area_id,reason:AreaEndReason::Cancelled} if area_id==&id)));
    assert!(h.snapshot(1).areas.as_ref().is_none_or(Vec::is_empty));
    assert!(burn(&h, 1).is_none());
}

#[test]
fn npc_ground_field_config_rejects_invalid_schedule_bounds_and_unknown_bindings() {
    for (key, value) in [
        ("radius", json!(513)),
        ("duration_ms", json!(8100)),
        ("interval_ms", json!(0)),
        ("max_targets_per_tick", json!(17)),
        ("max_total_hits", json!(257)),
        ("target_max_hp_bps", json!(0)),
    ] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                v["combat_rules"]["npc_ground_fields"][SKILL][key] = value;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(2, |v| {
            configure(v);
            let rules = v["combat_rules"]["npc_ground_fields"]
                .as_object_mut()
                .unwrap();
            let unknown = rules.remove(SKILL).unwrap();
            rules.insert("unknown-npc-skill".into(), unknown);
        })
        .is_err()
    );
}
