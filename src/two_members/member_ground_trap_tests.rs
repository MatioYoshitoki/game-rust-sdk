use super::*;

const SKILL: &str = "test-ground-trap";
const POINT: [f64; 2] = [560., 400.];

fn configure(v: &mut Value) {
    *v = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/mastery-stages.json"
    ))
    .unwrap();
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/member-ground-trap.json"
    ))
    .unwrap();
    v["additional_skills"]
        .as_array_mut()
        .unwrap()
        .push(sample["skill"].clone());
    v["combat_rules"]["member_ground_traps"] = sample["member_ground_traps"].clone();
    v["combat_rules"]["skills"][SKILL] = sample["damage_rule"].clone();
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
    v["pve"]["monster_positions"] = json!([POINT, [680, 400]]);
    v["pve"]["combat"]["return_distance"] = json!(1000);
}

fn host(change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let mut h = DeltaHarness::new(2, |v| {
        configure(v);
        change(v);
    });
    for store in &mut h.stores {
        store.hp = 1000;
    }
    h.active();
    h
}

fn at(h: &mut DeltaHarness, deadline: u64) {
    let steps = deadline
        .saturating_sub(h.engine.now)
        .div_ceil(h.engine.config.step_ms);
    h.ticks(steps as usize);
}

fn trap(h: &DeltaHarness, i: usize) -> Option<&AreaPlanViewV4> {
    h.snapshot(i)
        .areas
        .as_ref()?
        .iter()
        .find(|a| a.mechanism == AreaPlanKind::MemberGroundTrap)
}

fn deploy(h: &mut DeltaHarness, i: usize) -> AreaPlanViewV4 {
    h.action(
        i,
        Action::CastGround {
            skill_id: SKILL.into(),
            position: POINT,
        },
    )
    .unwrap();
    for _ in 0..4 {
        h.ticks(1);
        if let Some(p) = trap(h, i) {
            return p.clone();
        }
    }
    panic!("expected trap warning");
}

fn place_npc(h: &mut DeltaHarness, i: usize, point: [f64; 2]) {
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(i, point);
    h.ticks(1);
}

fn entries<'a>(h: &'a DeltaHarness, id: &str) -> Vec<&'a EntityRef> {
    h.events
        .iter()
        .filter_map(|(_, e)| match e {
            EventV4::AreaEntered {
                area_id, target, ..
            } if area_id == id => Some(target),
            _ => None,
        })
        .collect()
}

#[test]
fn member_ground_trap_arms_before_damage_and_requires_new_entry_after_cooldown() {
    let mut h = host(|v| {
        v["combat_rules"]["member_ground_traps"][SKILL]["contact_policy"]["max_total_hits"] =
            json!(2)
    });
    h.engine.world.as_mut().unwrap().members[1].position = [570., 440.];
    let spirit = h.snapshot(0).own.spirit;
    let hp = h.snapshot(0).monsters[0].hp;
    let p = deploy(&mut h, 0);
    assert_eq!(p.center, POINT);
    assert_eq!(p.activates_at_ms - p.created_at_ms, 600);
    assert_eq!(p.expires_at_ms - p.activates_at_ms, 4500);
    assert_eq!(h.snapshot(0).own.spirit, spirit - 3);
    assert!(h.snapshot(0).projectiles.is_empty());
    at(&mut h, p.activates_at_ms - 30);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp);
    assert!(entries(&h, &p.id).is_empty());
    at(&mut h, p.activates_at_ms);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp - 100);
    assert_eq!(h.snapshot(1).own.hp, 1000);
    assert_eq!(entries(&h, &p.id), vec![&h.snapshot(0).monsters[0].entity]);
    assert_eq!(
        trap(&h, 0)
            .unwrap()
            .contact_policy
            .as_ref()
            .unwrap()
            .remaining_hits,
        1
    );
    let applied = h.snapshot(0).monsters[0]
        .statuses
        .as_ref()
        .unwrap()
        .iter()
        .find(|s| s.definition_id == "trap-slow")
        .unwrap();
    assert_eq!(
        (&applied.source.id, applied.modifier_bps),
        (&p.source.id, -2000)
    );
    place_npc(&mut h, 0, [680., 440.]);
    place_npc(&mut h, 0, POINT);
    at(&mut h, p.activates_at_ms + 900);
    assert_eq!(entries(&h, &p.id).len(), 1);
    place_npc(&mut h, 0, [680., 440.]);
    place_npc(&mut h, 0, POINT);
    assert_eq!(entries(&h, &p.id).len(), 2);
    assert!(trap(&h, 0).is_none());
    assert_eq!(h.snapshot(0).monsters[0].hp, hp - 200);
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::AreaEnded{area_id,reason:AreaEndReason::HitBudgetExhausted} if area_id==&p.id)));
}

#[test]
fn member_ground_trap_can_arm_and_contact_in_the_commit_step() {
    let mut h = host(|v| {
        v["additional_skills"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()["warning_ms"] = json!(0);
        v["combat_rules"]["member_ground_traps"][SKILL]["duration_ms"] = v["step_ms"].clone();
    });
    let p = deploy(&mut h, 0);
    assert_eq!(p.created_at_ms, p.activates_at_ms);
    assert_eq!(entries(&h, &p.id).len(), 1);
    assert!(h.events.iter().any(|(_, e)| matches!(e, EventV4::AreaEntered{area_id,at_ms,..} if area_id==&p.id && *at_ms==p.created_at_ms)));
    at(&mut h, p.expires_at_ms);
    assert!(trap(&h, 0).is_none());
}

#[test]
fn member_ground_trap_orders_ties_and_keeps_skipped_occupants_out_of_later_steps() {
    let mut h = host(|v| {
        v["pve"]["monster_positions"] = json!([[530, 400], [590, 400]]);
        v["combat_rules"]["member_ground_traps"][SKILL]["contact_policy"]["max_targets_per_tick"] =
            json!(1);
    });
    let p = deploy(&mut h, 0);
    at(&mut h, p.activates_at_ms);
    h.ticks(5);
    assert_eq!(entries(&h, &p.id), vec![&h.snapshot(0).monsters[0].entity]);
    place_npc(&mut h, 1, [700., 440.]);
    place_npc(&mut h, 1, [590., 400.]);
    assert_eq!(
        entries(&h, &p.id),
        vec![
            &h.snapshot(0).monsters[0].entity,
            &h.snapshot(0).monsters[1].entity
        ]
    );
}

#[test]
fn member_ground_trap_filters_walls_and_preserves_budget_at_expiry() {
    let mut h = host(|v| {
        v["pve"]["monster_positions"] = json!([[570, 410], [635, 410]]);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let first = 10 * 32 + 15;
        v["pve"]["cells_rle"] = json!([[0, first], [1, 1], [0, 1024 - first - 1]]);
        v["additional_skills"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()["radius"] = json!(80);
    });
    let p = deploy(&mut h, 0);
    at(&mut h, p.expires_at_ms - 30);
    assert_eq!(entries(&h, &p.id), vec![&h.snapshot(0).monsters[0].entity]);
    assert_eq!(
        trap(&h, 0)
            .unwrap()
            .contact_policy
            .as_ref()
            .unwrap()
            .used_hits,
        1
    );
    h.engine
        .world
        .as_mut()
        .unwrap()
        .place_wave_test_npc(1, [560., 440.]);
    at(&mut h, p.expires_at_ms);
    assert_eq!(entries(&h, &p.id).len(), 1);
    assert!(trap(&h, 0).is_none());
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::AreaEnded{area_id,reason:AreaEndReason::Expired} if area_id==&p.id)));
}

#[test]
fn member_ground_trap_uses_full_target_identity_and_dodge_does_not_spend_budget() {
    for dodge in [false, true] {
        let mut h = host(|v| {
            v["combat_rules"]["member_ground_traps"][SKILL]["contact_policy"]["repeat_interval_ms"] =
                json!(0);
            v["additional_skills"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()["unavoidable"] = json!(!dodge);
            if dodge {
                v["combat_rules"]["profiles"]["npc"]["dodge_bps"] = json!(10000);
                v["combat_rules"]["profiles"]["member"]["hit_bps"] = json!(0);
                v["combat_rules"]["dodge_cap_bps"] = json!(10000);
            }
        });
        let p = deploy(&mut h, 0);
        at(&mut h, p.activates_at_ms);
        let original = h.snapshot(0).monsters[0].entity.clone();
        place_npc(&mut h, 0, [700., 400.]);
        place_npc(&mut h, 0, POINT);
        assert_eq!(entries(&h, &p.id).len(), usize::from(!dodge));
        h.engine.world.as_mut().unwrap().replace_wave_test_npc(0);
        h.ticks(1);
        let budget = trap(&h, 0).unwrap().contact_policy.as_ref().unwrap();
        if dodge {
            assert_eq!(budget.used_hits, 0);
            assert!(entries(&h, &p.id).is_empty());
        } else {
            assert_eq!(budget.used_hits, 2);
            let next = &h.snapshot(0).monsters[0].entity;
            assert_eq!(entries(&h, &p.id), vec![&original, next]);
            assert_eq!(next.generation, original.generation + 1);
        }
    }
}

#[test]
fn member_ground_trap_capacity_rejects_before_cost_and_releases_on_owner_exit() {
    let mut h = host(|v| v["members"]["area_capacity"] = json!(1));
    let p = deploy(&mut h, 0);
    let spirit = h.snapshot(1).own.spirit;
    h.action(
        1,
        Action::CastGround {
            skill_id: SKILL.into(),
            position: POINT,
        },
    )
    .unwrap();
    h.ticks(2);
    assert_eq!(h.snapshot(1).own.spirit, spirit);
    assert!(h.events.iter().any(|(pid, e)| pid.as_deref() == Some("P01")
        && matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected {
                    reason: Error::Capacity,
                    ..
                }
            }
        )));
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::AreaEnded{area_id,reason:AreaEndReason::SourceInvalid} if area_id==&p.id)));
    let next = deploy(&mut h, 1);
    assert_ne!(next.id, p.id);
    assert_eq!(h.snapshot(1).own.spirit, spirit - 3);
    assert!(trap(&h, 0).is_none());
}

#[test]
fn member_ground_trap_retains_committed_contacts_on_reconnect_and_clears_at_close() {
    let mut h = host(|_| {});
    let p = deploy(&mut h, 0);
    at(&mut h, p.activates_at_ms);
    let contact = trap(&h, 0).unwrap().clone();
    let previous = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous.clone(),
            authenticated_principal: h.stores[0].principal.clone(),
        },
    );
    reconnect.connection = Connection {
        id: "trap-reconnect".into(),
        generation: previous.generation + 1,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.pump();
    assert_eq!(trap(&h, 0), Some(&contact));
    assert!(h.snapshot(0).own.protected);
    h.ticks(2);
    assert_eq!(entries(&h, &p.id).len(), 1);
    h.close();
    assert!(trap(&h, 1).is_none());
}

#[test]
fn member_ground_trap_rejects_invalid_or_incompatible_bindings() {
    for (key, value) in [
        ("duration_ms", json!(0)),
        ("duration_ms", json!(60001)),
        ("contact_policy", json!(null)),
    ] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                v["combat_rules"]["member_ground_traps"][SKILL][key] = value;
            })
            .is_err()
        );
    }
    for target in ["unknown", "seed-field", "seed-strike"] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                let traps = v["combat_rules"]["member_ground_traps"]
                    .as_object_mut()
                    .unwrap();
                let row = traps.remove(SKILL).unwrap();
                traps.insert(target.into(), row);
            })
            .is_err()
        );
    }
}
