use super::*;

const VORTEX: &str = "fan_echo_wave";
const TOWER: &str = "tower_expanding_ring";

fn until(h: &mut DeltaHarness, done: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..500 {
        if done(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("basic field did not reach its expected state");
}

fn ready(h: &mut DeltaHarness, i: usize) {
    until(h, |h| {
        h.snapshot(i).logical_ms >= h.snapshot(i).own.basic_ready_at_ms
    });
}

fn areas(h: &DeltaHarness) -> &[AreaPlanViewV4] {
    h.snapshot(1).areas.as_deref().unwrap()
}

fn pulses<'a>(h: &'a DeltaHarness, id: &str) -> Vec<(u32, u64, &'a [EntityRef])> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::AreaTick {
                area_id,
                tick_index,
                scheduled_at_ms,
                targets,
            } if area_id == id => Some((*tick_index, *scheduled_at_ms, targets.as_slice())),
            _ => None,
        })
        .collect()
}

fn damage(h: &DeltaHarness, source: &str) -> Vec<u32> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::Damage {
                source: actor,
                amount,
                ..
            } if actor.id == source => Some(*amount),
            _ => None,
        })
        .collect()
}

#[test]
fn basic_world_fields_vortices_keep_contact_centers_sample_live_and_never_hold_rewards() {
    let mut h = profile_host([Some(VORTEX), None], |v| {
        v["pve"]["monster_positions"] = json!([[460, 400]]);
        v["pve"]["combat"]["return_distance"] = json!(500);
        v["combat_rules"]["basic_attack"]["life_steal_bps"] = json!(5000);
    });
    h.engine.world.as_mut().unwrap().members[0].hp = 500;
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.ticks(5);
    assert!(areas(&h).is_empty());
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(0, [465., 400.]);
    until(&mut h, |h| areas(h).len() == 1);
    let first = areas(&h)[0].clone();
    assert_eq!(
        (&*first.skill_id, first.center, first.radius),
        ("fan-persistent-vortex", [465., 400.], 74.)
    );
    assert_eq!(first.mechanism, AreaPlanKind::BasicField);
    assert!(first.basic_action_id.is_some());
    assert_eq!(h.snapshot(0).own.spirit, 102);
    assert_eq!(h.snapshot(0).own.hp, 532);
    assert_eq!(
        h.snapshot(0).own.basic_ready_at_ms,
        first.created_at_ms + 800
    );
    assert_eq!(damage(&h, "A"), [65]);

    let w = h.engine.world.as_mut().unwrap();
    w.members[0].facts.atk_points = 200;
    w.place_wave_test_npc(0, [485., 400.]);
    ready(&mut h, 0);
    h.action(0, attack(&h, 0, 0)).unwrap();
    until(&mut h, |h| areas(h).len() == 2);
    let second = areas(&h)[1].clone();
    assert_eq!(second.center, [485., 400.]);
    assert_eq!(areas(&h)[0].center, first.center);
    assert_ne!(second.basic_action_id, first.basic_action_id);
    assert_eq!(h.snapshot(0).own.spirit, 104);
    assert_eq!(h.snapshot(0).own.hp, 597);
    let ready_at = h.snapshot(0).own.basic_ready_at_ms;

    until(&mut h, |h| areas(h).is_empty());
    for field in [&first, &second] {
        let ticks = pulses(&h, &field.id);
        assert_eq!(ticks.len(), 10);
        assert!(
            ticks
                .iter()
                .enumerate()
                .all(|(i, (index, time, targets))| *index == i as u32 + 1
                    && *time == field.created_at_ms + 1000 * u64::from(*index)
                    && targets.len() == 1)
        );
    }
    let amounts = damage(&h, "A");
    assert_eq!(amounts.iter().filter(|amount| **amount == 30).count(), 20);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, ready_at);
    assert_eq!(h.snapshot(0).own.spirit, 104);
    assert_eq!(h.snapshot(0).own.hp, 597);
}

#[test]
fn basic_world_fields_tower_lands_at_frozen_target_after_replacement_and_replaces_own_field() {
    let mut h = profile_host([None, Some(TOWER)], |v| {
        v["pve"]["monster_positions"] = json!([[650, 480], [690, 480]]);
        v["pve"]["combat"]["return_distance"] = json!(500);
    });
    h.action(1, attack(&h, 1, 0)).unwrap();
    h.ticks(6);
    let drop = h.snapshot(1).basic_waves.as_ref().unwrap()[0].clone();
    assert_eq!(drop.origin, [650., 480.]);
    assert_eq!(
        drop.geometry,
        BasicWaveGeometry::Landing { from: [400., 480.] }
    );
    assert_eq!(drop.pending.len(), 2);
    assert_eq!(drop.pending[0].at_ms, 359);
    assert_eq!(drop.pending[1].at_ms, 359);
    assert!(areas(&h).is_empty());
    assert!(damage(&h, "B").is_empty());
    let world = h.engine.world.as_mut().unwrap();
    world.replace_wave_test_npc(0);
    world.place_wave_test_npc(0, [850., 480.]);
    world.members[1].facts.atk_points = 200;
    h.ticks(6);
    let first = areas(&h)[0].clone();
    assert_eq!(
        (&*first.skill_id, first.center, first.radius),
        ("tower-falling-field", [650., 480.], 150.)
    );
    assert_eq!(first.total_ticks, 6);
    assert_eq!(damage(&h, "B"), [62]);
    assert_eq!(h.snapshot(1).own.spirit, 102);
    until(&mut h, |h| pulses(h, &first.id).len() == 1);
    assert_eq!(damage(&h, "B"), [62, 18]);
    ready(&mut h, 1);
    h.action(1, attack(&h, 1, 1)).unwrap();
    until(&mut h, |h| areas(h)[0].id != first.id);
    assert_eq!(areas(&h).len(), 1);
    let second = areas(&h)[0].clone();
    assert_eq!(second.center, [690., 480.]);
    assert!(
        h.events
            .iter()
            .any(|(_, event)| matches!(event, EventV4::AreaEnded {
        area_id, reason: AreaEndReason::Replaced,
    } if area_id == &first.id))
    );
    assert_eq!(damage(&h, "B"), [62, 18, 144, 124]);
    until(&mut h, |h| pulses(h, &second.id).len() == 6);
    assert_eq!(pulses(&h, &first.id).len(), 1);
    assert!(areas(&h).is_empty());
    assert_eq!(h.snapshot(1).own.spirit, 104);
}

#[test]
fn basic_world_fields_owner_reduction_precedes_shield_and_stops_outside() {
    let mut h = profile_host([Some(TOWER), None], |v| {
        v["pve"]["monster_positions"] = json!([[500, 400]]);
        v["pve"]["stats"]["atk"] = json!(100);
        v["pve"]["combat"]["aggro_range"] = json!(280);
        v["pve"]["combat"]["melee_range"] = json!(280);
        v["pve"]["combat"]["attack_interval_ms"] = json!(300);
        v["combat_rules"]["basic_profiles"][TOWER]["effects"]["on_release_self_status"] =
            json!("shield-absorption");
        v["combat_rules"]["basic_profiles"][TOWER]["effects"]["on_end_damage_target_status"]["choices"] =
            json!(["tower-movement"]);
        let id = v["pve"]["template_id"].as_str().unwrap().to_owned();
        let mut rule = v["combat_rules"]["basic_attack"].clone();
        rule.as_object_mut().unwrap().remove("life_steal_timing");
        v["combat_rules"]["npc_attacks"][id] = rule;
    });
    h.action(0, attack(&h, 0, 0)).unwrap();
    let npc = h.snapshot(0).monsters[0].entity.id.clone();
    until(&mut h, |h| damage(h, &npc).len() == 1);
    assert_eq!(damage(&h, &npc), [38]);
    assert_eq!(h.snapshot(0).own.hp, 962);
    assert_eq!(h.snapshot(0).own.shield_total, Some(0));
    h.engine.world.as_mut().unwrap().members[0].position = [300., 400.];
    until(&mut h, |h| damage(h, &npc).len() == 2);
    assert_eq!(h.snapshot(1).own.hp, 900);
    h.engine.world.as_mut().unwrap().members[1].position = [800., 600.];
    until(&mut h, |h| damage(h, &npc).len() == 3);
    assert_eq!(damage(&h, &npc), [38, 100, 100]);
    assert_eq!(h.snapshot(0).own.hp, 862);
    assert_eq!(areas(&h).len(), 1);
}

#[test]
fn basic_world_fields_reserve_capacity_during_flight_and_release_on_source_exit() {
    let mut h = profile_host([Some(VORTEX), Some(VORTEX)], |v| {
        v["pve"]["monster_positions"] = json!([[460, 400], [460, 480]]);
        v["members"]["basic_field_capacity"] = json!(1);
    });
    for i in 0..2 {
        h.action(i, attack(&h, i, i)).unwrap();
    }
    h.ticks(5);
    assert!(areas(&h).is_empty());
    assert!(
        h.events
            .iter()
            .any(|(recipient, event)| recipient.as_deref() == Some("B")
                && matches!(
                    event,
                    EventV4::Private {
                        event: Event::ActionRejected {
                            reason: Error::Capacity,
                            ..
                        }
                    }
                ))
    );
    until(&mut h, |h| areas(h).len() == 1);
    let initial = areas(&h)[0].id.clone();
    ready(&mut h, 0);
    h.action(0, attack(&h, 0, 0)).unwrap();
    until(&mut h, |h| areas(h)[0].id != initial);
    let replaced = areas(&h)[0].id.clone();
    assert_eq!(areas(&h).len(), 1);
    assert_eq!(areas(&h)[0].source.id, "A");
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(areas(&h).is_empty());
    assert!(
        h.events
            .iter()
            .any(|(_, event)| matches!(event, EventV4::AreaEnded {
        area_id, reason: AreaEndReason::SourceInvalid,
    } if area_id == &replaced))
    );
    h.action(1, attack(&h, 1, 1)).unwrap();
    until(&mut h, |h| areas(h).len() == 1);
    assert_eq!(areas(&h)[0].source.id, "B");
    h.close();
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}

#[test]
fn basic_world_fields_invalid_capacity_and_clocks_fail_at_load() {
    let load = |change: fn(&mut Value)| profile_catalog_with_features(false, true, change);
    assert!(load(|_| {}).is_ok());
    assert!(
        load(|v| {
            v["members"]
                .as_object_mut()
                .unwrap()
                .remove("basic_field_capacity");
        })
        .is_err()
    );
    assert!(load(|v| v["members"]["basic_field_capacity"] = json!(13)).is_err());
    assert!(
        load(|v| {
            v["combat_rules"]["basic_profiles"][VORTEX]["wave"]["field"]["interval_ms"] = json!(0);
        })
        .is_err()
    );
    assert!(
        load(|v| {
            v["combat_rules"]["basic_profiles"][TOWER]["wave"]["field"]["max_per_source"] =
                json!(2);
        })
        .is_err()
    );
    assert!(
        load(|v| {
            v["combat_rules"]["basic_profiles"][TOWER]["wave"]["geometry"] =
                json!({"kind":"directional","cone_degrees":60});
        })
        .is_err()
    );
}
