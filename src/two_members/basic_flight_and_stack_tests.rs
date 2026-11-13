use super::*;

const FLIGHT: &str = "cauldron_core_burst";
const TOWER: &str = "tower_double_ring";
const FLOOR: &str = "ascending-pagoda-floor";
const BURN: &str = "effect_weapon_cauldron_burn";

fn install(v: &mut Value) {
    let kit: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/basic-flight-and-stack-finisher.json"
    ))
    .unwrap();
    let burn: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/basic-burning-profiles.json"
    ))
    .unwrap();
    v["combat_rules"]["basic_profiles"] = kit["basic_profiles"].clone();
    v["combat_rules"]["elements"]
        .as_array_mut()
        .unwrap()
        .push(json!("fire"));
    v["combat_rules"]["periodic_damage"][BURN] = burn["periodic_damage"][BURN].clone();
    v["combat_rules"]["profiles"]["npc"]["element_resistance"] = json!(0);
    let statuses = v["pve"]["status_definitions"].as_array_mut().unwrap();
    statuses.push(burn["status_definitions"][0].clone());
    statuses.extend(
        kit["status_definitions"]
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    v["pve"]["combat"]["aggro_range"] = json!(10);
}

fn host(profiles: [&str; 2], change: impl FnOnce(&mut Value)) -> DeltaHarness {
    profile_host(profiles.map(Some), |v| {
        install(v);
        change(v);
    })
}

fn until(h: &mut DeltaHarness, f: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..400 {
        if f(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("expected flight or wave state");
}

fn release(h: &mut DeltaHarness, i: usize, point: Option<[f64; 2]>, npc: Option<usize>) {
    let session_id = h
        .snapshot(i)
        .own
        .basic_charge
        .as_ref()
        .unwrap()
        .session_id
        .clone();
    let target = npc.map(|index| {
        let e = &h.snapshot(i).monsters[index].entity;
        UnitTarget {
            id: e.id.clone(),
            generation: e.generation,
        }
    });
    h.action(
        i,
        Action::ReleaseBasicCharge {
            session_id,
            target,
            direction: None,
            position: point,
        },
    )
    .unwrap();
}

fn launch(h: &mut DeltaHarness, i: usize) -> ProjectileViewV4 {
    until(h, |h| {
        h.snapshot(i).projectiles.iter().any(|p| {
            p.basic_segment
                .as_ref()
                .is_some_and(|s| s.role == Some(BasicProjectileRole::ChargedFlight))
        })
    });
    h.snapshot(i)
        .projectiles
        .iter()
        .find(|p| {
            p.basic_segment
                .as_ref()
                .is_some_and(|s| s.role == Some(BasicProjectileRole::ChargedFlight))
        })
        .unwrap()
        .clone()
}

fn end_flight(h: &mut DeltaHarness, id: &str, reason: ProjectileEndReasonV4) {
    until(h, |h| {
        h.events.iter().any(|(_,e)| matches!(e,EventV4::ProjectileEnded { projectile_id, reason:Some(why) } if projectile_id==id && *why==reason))
    });
}

#[test]
fn basic_flight_frozen_tier_and_target_survive_replacement_with_per_target_limit_and_burst() {
    let mut h = host([FLIGHT, TOWER], |v| {
        v["pve"]["monster_positions"] = json!([[490, 400], [760, 400], [750, 450]]);
        v["combat_rules"]["basic_profiles"][FLIGHT]["charge"]["behavior"]["stages"][1]["max_contacts_per_target"] =
            json!(2);
    });
    h.action(0, Action::StartBasicCharge {}).unwrap();
    h.action(
        0,
        Action::Move {
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(53);
    assert_eq!(h.snapshot(0).own.position, [400., 400.]);
    assert_eq!(h.snapshot(0).own.basic_charge.as_ref().unwrap().tier, 2);
    release(&mut h, 0, None, Some(1));
    let w = h.engine.world.as_mut().unwrap();
    w.replace_wave_test_npc(1);
    w.place_wave_test_npc(1, [900., 700.]);
    let p = launch(&mut h, 0);
    assert_eq!(p.position, [400., 400.]);
    assert_eq!(p.radius, 42.);
    let s = p.basic_segment.as_ref().unwrap();
    assert_eq!((s.tier, s.aim_point), (Some(2), [760., 400.]));
    assert!(s.launched_at_ms > 1600);
    h.action(0, Action::Stop {}).unwrap();
    end_flight(&mut h, &p.id, ProjectileEndReasonV4::RangeExhausted);
    let first = &h.snapshot(0).monsters[0].entity;
    let contacts: Vec<_> = h
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            EventV4::BasicFlightContact {
                projectile_id,
                target,
                hit_at_ms,
                target_hits,
                ..
            } if projectile_id == &p.id && target == first => Some((*hit_at_ms, *target_hits)),
            _ => None,
        })
        .collect();
    assert_eq!(contacts.len(), 2);
    assert_eq!(contacts[1], (contacts[0].0 + 90, 2));
    let endpoint = &h.snapshot(0).monsters[2];
    let bursts: Vec<_> = h
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            EventV4::BasicFlightBurst {
                projectile_id,
                center,
                radius,
                targets,
                ..
            } if projectile_id == &p.id => Some((*center, *radius, targets.as_slice())),
            _ => None,
        })
        .collect();
    assert_eq!(
        bursts,
        [([760., 400.], 115., std::slice::from_ref(&endpoint.entity))]
    );
    let path_hits = h.events.iter().filter(|(_,e)| matches!(e,
        EventV4::BasicFlightContact { projectile_id,target,.. } if projectile_id==&p.id && *target==endpoint.entity)).count();
    let burn = endpoint
        .statuses
        .as_ref()
        .unwrap()
        .iter()
        .find(|s| s.definition_id == BURN)
        .unwrap();
    assert_eq!(burn.stacks, Some(path_hits as u32 + 2));
    assert_eq!(h.snapshot(0).own.spirit, 102);
    h.close();
}

#[test]
fn basic_flight_wall_blocks_swept_body_and_prevents_burst() {
    let mut h = host([FLIGHT, TOWER], |v| {
        v["members"]["spawns"][0] = json!([400, 420]);
        v["spawn"] = json!([400, 420]);
        v["pve"]["monster_positions"] = json!([[480, 420], [700, 420]]);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let tile = 9 * 32 + 14;
        v["pve"]["cells_rle"] = json!([[0, tile], [1, 1], [0, 1024 - tile - 1]]);
    });
    h.action(0, Action::StartBasicCharge {}).unwrap();
    h.pump();
    release(&mut h, 0, Some([760., 420.]), None);
    let p = launch(&mut h, 0);
    assert_eq!(p.basic_segment.as_ref().unwrap().tier, Some(1));
    end_flight(&mut h, &p.id, ProjectileEndReasonV4::Wall);
    assert!(!h.events.iter().any(
        |(_, e)| matches!(e,EventV4::BasicFlightBurst { projectile_id,.. } if projectile_id==&p.id)
    ));
    assert_eq!(h.snapshot(0).monsters[1].hp, 10000);
    assert!(
        h.snapshot(0).monsters[1]
            .statuses
            .as_ref()
            .unwrap()
            .is_empty()
    );
    h.close();
}

#[test]
fn basic_stack_finisher_fully_absorbed_contact_does_not_add_a_floor() {
    let mut h = host([TOWER, FLIGHT], |v| {
        v["pve"]["monster_positions"] = json!([[460, 400]]);
    });
    let now = h.snapshot(0).logical_ms;
    h.engine
        .world
        .as_mut()
        .unwrap()
        .grant_npc_test_shield(0, &h.engine.config, now);
    h.action(0, attack(&h, 0, 0)).unwrap();
    until(&mut h, |h| {
        h.events
            .iter()
            .any(|(_, event)| matches!(event, EventV4::BasicWaveEnded { .. }))
    });
    assert_eq!(h.snapshot(0).monsters[0].hp, 10000);
    assert_eq!(h.snapshot(0).own.spirit, 102);
    assert!(floor(&h, 0).is_none());
    h.close();
}

#[test]
fn basic_flight_ordinary_tap_reserves_slot_and_protection_preserves_only_released_flight() {
    let mut h = host([FLIGHT, FLIGHT], |v| {
        v["members"]["projectile_capacity"] = json!(1);
        v["pve"]["monster_positions"] = json!([[650, 440]]);
    });
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.action(1, attack(&h, 1, 0)).unwrap();
    let p = launch(&mut h, 0);
    assert_eq!(p.basic_segment.as_ref().unwrap().tier, Some(1));
    assert!(
        h.events
            .iter()
            .any(|(owner, e)| owner.as_deref() == Some("B")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionRejected {
                            reason: Error::Capacity,
                            ..
                        }
                    }
                ))
    );
    h.action(0, Action::Background {}).unwrap();
    h.pump();
    assert!(
        h.snapshot(1)
            .projectiles
            .iter()
            .any(|other| other.id == p.id)
    );
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    let publication_id = h.replicas[0].last_full_id().unwrap().to_owned();
    h.action(0, Action::CombatReady { publication_id }).unwrap();
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(
        !h.snapshot(1)
            .projectiles
            .iter()
            .any(|other| other.id == p.id)
    );
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::ProjectileEnded { projectile_id,reason:Some(ProjectileEndReasonV4::SourceInvalid) } if projectile_id==&p.id)));
    h.close();
}

fn floor(h: &DeltaHarness, i: usize) -> Option<&StatusView> {
    h.snapshot(i)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == FLOOR)
}

fn ready(h: &mut DeltaHarness, i: usize) {
    until(h, |h| {
        h.snapshot(i).own.basic_ready_at_ms <= h.snapshot(i).logical_ms
            && h.snapshot(i).own.pending_action.is_none()
            && h.snapshot(i)
                .basic_waves
                .as_ref()
                .unwrap()
                .iter()
                .all(|w| w.source.id != if i == 0 { "A" } else { "B" })
    });
}

fn build_floors(h: &mut DeltaHarness, i: usize, count: u32) {
    for expected in 1..=count {
        ready(h, i);
        h.action(i, attack(h, i, 0)).unwrap();
        until(h, |h| {
            floor(h, i).is_some_and(|s| s.stacks == Some(expected))
        });
    }
}

#[test]
fn basic_stack_finisher_consumes_five_only_on_terminal_and_is_source_independent() {
    let mut h = host([TOWER, TOWER], |v| {
        v["pve"]["monster_positions"] = json!([[470, 440], [620, 400]])
    });
    build_floors(&mut h, 0, 5);
    build_floors(&mut h, 1, 1);
    ready(&mut h, 0);
    let old = floor(&h, 0).unwrap().clone();
    assert_eq!(old.modifiers[0].modifier_bps, 200);
    assert_eq!(old.expires_at_ms - old.applied_at_ms, 6000);
    assert_eq!(floor(&h, 1).unwrap().stacks, Some(1));
    let now = h.snapshot(0).logical_ms;
    let incoming = h
        .engine
        .world
        .as_mut()
        .unwrap()
        .charge_test_hit(0, 0, &h.engine.config, now);
    assert!(
        incoming
            .iter()
            .any(|(_, event)| matches!(event, EventV4::Damage { amount: 180, .. }))
    );
    h.action(0, attack(&h, 0, 1)).unwrap();
    h.action(0, Action::CancelAction {}).unwrap();
    h.pump();
    assert_eq!(floor(&h, 0), Some(&old));
    h.action(0, attack(&h, 0, 1)).unwrap();
    until(&mut h, |h| {
        h.snapshot(0)
            .basic_waves
            .as_ref()
            .unwrap()
            .iter()
            .any(|p| p.plan_id == "ascending-pagoda-finisher")
    });
    let wave = h
        .snapshot(0)
        .basic_waves
        .as_ref()
        .unwrap()
        .iter()
        .find(|p| p.plan_id == "ascending-pagoda-finisher")
        .unwrap();
    assert_eq!((wave.range, wave.pending.len()), (230., 2));
    assert_eq!(floor(&h, 0), Some(&old));
    let w = h.engine.world.as_mut().unwrap();
    w.replace_wave_test_npc(0);
    w.replace_wave_test_npc(1);
    until(&mut h, |h| floor(h, 0).is_none());
    assert!(
        h.events
            .iter()
            .any(|(recipient, e)| recipient.as_deref() == Some("A")
                && matches!(e,EventV4::Private {
        event: Event::StatusRemoved { instance_ids, reason:StatusRemovalReason::Consumed }
    } if instance_ids.contains(&old.instance_id)))
    );
    h.close();
}

#[test]
fn basic_stack_finisher_rechecks_expiry_during_windup() {
    let mut h = host([TOWER, FLIGHT], |v| {
        v["pve"]["monster_positions"] = json!([[460, 400], [620, 400]])
    });
    build_floors(&mut h, 0, 5);
    let expiry = floor(&h, 0).unwrap().expires_at_ms;
    until(&mut h, |h| h.snapshot(0).logical_ms >= expiry - 60);
    h.action(0, attack(&h, 0, 1)).unwrap();
    h.ticks(6);
    assert!(floor(&h, 0).is_none());
    assert!(
        h.events
            .iter()
            .any(|(owner, e)| owner.as_deref() == Some("A")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionRejected {
                            reason: Error::InvalidTarget,
                            ..
                        }
                    }
                ))
    );
    assert!(!h.events.iter().any(|(_,e)| matches!(e,EventV4::BasicWaveStarted { wave } if wave.plan_id=="ascending-pagoda-finisher")));
    h.close();
}

#[test]
fn basic_flight_and_stack_loader_rejects_unbounded_config() {
    assert!(profile_catalog(install).is_ok());
    assert!(profile_catalog(|v|{
        install(v);
        v["combat_rules"]["basic_profiles"][FLIGHT]["charge"]["behavior"]["stages"][0]["after_ms"]=json!(1);
    }).is_err());
    assert!(
        profile_catalog(|v| {
            install(v);
            v["combat_rules"]["basic_profiles"][TOWER]["stacked_wave"]["stack_status"] =
                json!(BURN);
        })
        .is_err()
    );
}
