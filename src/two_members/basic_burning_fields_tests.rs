use super::*;

const SEED: &str = "cauldron_flame_field";
const SEA: &str = "cauldron_triple_fall";
const BURN: &str = "effect_weapon_cauldron_burn";
const SLOW: &str = "cauldron-burning-field-slow";

fn config(v: &mut Value) {
    *v = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/mastery-stages.json"
    ))
    .unwrap();
    let kit: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/basic-burning-profiles.json"
    ))
    .unwrap();
    v["combat_rules"]["basic_profiles"] = kit["basic_profiles"].clone();
    v["combat_rules"]["periodic_damage"] = kit["periodic_damage"].clone();
    v["combat_rules"]["profiles"]["npc"]["element_resistance"] = json!(0);
    v["combat_rules"]["elements"]
        .as_array_mut()
        .unwrap()
        .extend(kit["elements"].as_array().unwrap().iter().cloned());
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .extend(
            kit["status_definitions"]
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
    v["members"]["basic_field_capacity"] = kit["basic_field_capacity"].clone();
    v["pve"]["monster_positions"] = json!([[460, 440], [500, 440], [620, 480]]);
    v["pve"]["combat"]["aggro_range"] = json!(10);
}

fn host(profiles: [&str; 2], adjust: impl FnOnce(&mut Value)) -> DeltaHarness {
    let catalog = catalog_with(|v| {
        config(v);
        adjust(v);
    })
    .unwrap();
    let mut request = create(&catalog.default_config());
    request.protocol_version = 6;
    for (member, profile) in request.participants.iter_mut().zip(profiles) {
        member.basic_profile = Some(profile.into());
    }
    let mut h = DeltaHarness::from_request(catalog, request);
    for store in &mut h.stores {
        store.hp = 1000;
        store.spirit = 2_000_000;
    }
    h.active();
    for member in &mut h.engine.world.as_mut().unwrap().members {
        member.spirit = 100;
    }
    h
}

fn until(h: &mut DeltaHarness, predicate: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..300 {
        if predicate(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("expected publication was not received: {:?}", h.snapshot(1));
}

fn field(h: &DeltaHarness, source: &str) -> Option<AreaPlanViewV4> {
    h.snapshot(1)
        .areas
        .as_deref()
        .unwrap()
        .iter()
        .find(|area| area.source.id == source)
        .cloned()
}

fn status<'a>(h: &'a DeltaHarness, index: usize, source: &str, id: &str) -> Option<&'a StatusView> {
    h.snapshot(usize::from(source == "B")).monsters[index]
        .statuses
        .as_deref()?
        .iter()
        .find(|s| s.source.id == source && s.definition_id == id)
}

#[test]
fn basic_burning_fields_seed_and_sea_are_independent_and_ground_survives_target_replacement() {
    let mut h = host([SEED, SEA], |v| {
        v["combat_rules"]["basic_attack"]["fixed"] = json!(99)
    });
    assert_eq!(
        h.action(
            0,
            Action::AttackGround {
                position: [460., 440.]
            }
        ),
        Err(Error::Unsupported)
    );
    assert_eq!(
        h.action(
            1,
            Action::AttackGround {
                position: [-1., 440.]
            }
        ),
        Err(Error::InvalidTarget)
    );
    assert_eq!(
        h.action(
            1,
            Action::AttackGround {
                position: [900., 480.]
            }
        ),
        Err(Error::OutOfRange)
    );
    assert_eq!(
        h.action(
            1,
            Action::AttackGround {
                position: [f64::NAN, 440.]
            }
        ),
        Err(Error::InvalidMessage)
    );

    h.action(0, attack(&h, 0, 1)).unwrap();
    h.action(1, attack(&h, 1, 0)).unwrap();
    {
        let world = h.engine.world.as_mut().unwrap();
        world.replace_wave_test_npc(0);
        world.place_wave_test_npc(0, [850., 700.]);
    }
    until(&mut h, |h| {
        h.snapshot(1)
            .basic_impacts
            .as_deref()
            .is_some_and(|i| i.len() == 2)
    });
    let ground = h
        .snapshot(1)
        .basic_impacts
        .as_ref()
        .unwrap()
        .iter()
        .find(|i| i.source.id == "B")
        .unwrap();
    assert_eq!(
        (ground.kind, ground.position, ground.target.as_ref()),
        (BasicImpactKind::GroundField, Some([460., 440.]), None)
    );
    assert_eq!(ground.impact_at_ms - ground.released_at_ms, 350);
    until(&mut h, |h| {
        field(h, "B").is_some() && status(h, 1, "A", BURN).is_some()
    });
    let first = field(&h, "B").unwrap();
    assert_eq!(
        (
            first.center,
            first.radius,
            first.interval_ms,
            first.total_ticks
        ),
        ([460., 440.], 135., 500, 14)
    );
    assert!(!h.events.iter().any(
        |(_, e)| matches!(e, EventV4::Damage { source, .. } if source.id == "A" || source.id == "B")
    ));
    assert_eq!(h.snapshot(1).own.spirit, 100);
    until(&mut h, |h| status(h, 1, "B", BURN).is_some());
    assert_eq!(status(&h, 1, "A", BURN).unwrap().stacks, Some(1));
    assert_eq!(status(&h, 1, "B", BURN).unwrap().stacks, Some(1));
    let slow = status(&h, 1, "B", SLOW).unwrap();
    assert_eq!(
        (slow.modifier_bps, slow.expires_at_ms - slow.applied_at_ms),
        (-2500, 750)
    );
    assert!(
        h.events.iter().any(
            |(_, e)| matches!(e, EventV4::Damage { source, amount: 9, .. } if source.id == "B")
        )
    );
    let cooldown = h.snapshot(1).own.basic_ready_at_ms;
    until(&mut h, |h| {
        status(h, 1, "B", BURN).unwrap().stacks == Some(2)
    });
    assert_eq!(h.snapshot(1).own.basic_ready_at_ms, cooldown);
    assert_eq!(h.snapshot(1).own.spirit, 100);

    h.action(
        1,
        Action::AttackGround {
            position: [620., 480.],
        },
    )
    .unwrap();
    until(&mut h, |h| {
        field(h, "B").is_some_and(|area| area.id != first.id)
    });
    assert_eq!(field(&h, "B").unwrap().center, [620., 480.]);
    assert!(h.events.iter().any(|(_, e)| matches!(e, EventV4::AreaEnded { area_id, reason: AreaEndReason::Replaced } if area_id == &first.id)));
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(status(&h, 1, "A", BURN).is_none());
    assert!(status(&h, 1, "B", BURN).is_some());
    assert!(field(&h, "B").is_some());
    h.close();
}

#[test]
fn basic_burning_fields_share_reservations_and_only_replace_the_owners_field() {
    let mut h = host([SEA, SEA], |v| {
        v["members"]["basic_field_capacity"] = json!(1)
    });
    h.action(
        0,
        Action::AttackGround {
            position: [440., 400.],
        },
    )
    .unwrap();
    h.action(
        1,
        Action::AttackGround {
            position: [440., 480.],
        },
    )
    .unwrap();
    until(&mut h, |h| field(h, "A").is_some());
    let first = field(&h, "A").unwrap();
    assert!(field(&h, "B").is_none());
    assert!(
        h.events
            .iter()
            .any(|(recipient, e)| recipient.as_deref() == Some("B")
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
    until(&mut h, |h| {
        h.snapshot(0).logical_ms >= h.snapshot(0).own.basic_ready_at_ms
    });
    h.action(
        0,
        Action::AttackGround {
            position: [445., 400.],
        },
    )
    .unwrap();
    until(&mut h, |h| {
        field(h, "A").is_some_and(|area| area.id != first.id)
    });
    assert_eq!(field(&h, "A").unwrap().center, [445., 400.]);
    h.close();
}

#[test]
fn basic_burning_fields_invalid_configuration_and_projectile_wall_are_rejected() {
    let load = |adjust: fn(&mut Value)| {
        catalog_with(|v| {
            config(v);
            adjust(v);
        })
    };
    assert!(load(|_| {}).is_ok());
    for (index, adjust) in [
        (|v: &mut Value| {
            v["combat_rules"]["basic_profiles"][SEED]["impact"]["damage_bps"] = json!(1)
        }) as fn(&mut Value),
        |v| {
            v["combat_rules"]["basic_profiles"][SEA]["impact"]["timing"]["field"]["on_tick_statuses"] =
                json!(["missing"])
        },
        |v| {
            v["pve"]["status_definitions"][6]["periodic_stack"]["healing_reduction_cap_bps"] =
                json!(0)
        },
        |v| {
            v["combat_rules"]["basic_profiles"][SEED]["impact"]["contact_status"]["death_burst"]["inherited_stack_bps"] =
                json!(10001)
        },
    ].into_iter().enumerate() {
        assert!(load(adjust).is_err(), "invalid case {index}");
    }
    let mut h = host([SEED, SEA], |v| {
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        let mut nonwalkable = v["pve"]["palette"][0].clone();
        nonwalkable["walkable"] = json!(false);
        v["pve"]["palette"]
            .as_array_mut()
            .unwrap()
            .extend([wall, nonwalkable]);
        let index = 12 * 32 + 11;
        v["pve"]["cells_rle"] =
            json!([[0, index], [1, 1], [0, 31], [2, 1], [0, 1024 - index - 33]]);
    });
    assert_eq!(
        h.action(
            1,
            Action::AttackGround {
                position: [490., 490.]
            }
        ),
        Err(Error::OutOfRange)
    );
    assert!(serde_json::from_value::<Action>(json!({"kind":"attack_ground","position":[490,480],"target":{"id":"npc","generation":1}})).is_err());
    let position = [455., 550.];
    h.action(1, Action::AttackGround { position }).unwrap();
    until(&mut h, |h| field(h, "B").is_some());
    assert_eq!(field(&h, "B").unwrap().center, position);
    h.close();
}

#[test]
fn basic_burning_fields_lethal_tick_scales_burst_and_rounds_inherited_stacks_without_recursing() {
    let mut h = host([SEED, SEA], |v| {
        v["pve"]["monster_positions"] = json!([[460, 440], [500, 440], [540, 440], [625, 440]]);
    });
    for stacks in 1..=3 {
        until(&mut h, |h| {
            h.snapshot(0).logical_ms >= h.snapshot(0).own.basic_ready_at_ms
        });
        h.action(0, attack(&h, 0, 0)).unwrap();
        until(&mut h, |h| {
            status(h, 0, "A", BURN).is_some_and(|s| s.stacks == Some(stacks))
        });
    }
    let old = h.snapshot(0).monsters[1].hp;
    let distal = h.snapshot(0).monsters[3].hp;
    let world = h.engine.world.as_mut().unwrap();
    world.set_basic_test_npc_hp(0, 1);
    world.set_basic_test_npc_hp(2, 1);
    world.members[0].facts.atk_points = 200;

    until(&mut h, |h| h.snapshot(0).monsters[0].hp == 0);
    assert_eq!(h.snapshot(0).monsters[1].hp, old - 27);
    assert_eq!(h.snapshot(0).monsters[2].hp, 0);
    assert_eq!(h.snapshot(0).monsters[3].hp, distal);
    assert!(status(&h, 3, "A", BURN).is_none());
    let inherited = status(&h, 1, "A", BURN).unwrap();
    assert_eq!(inherited.stacks, Some(2));
    assert_eq!(inherited.periodic.as_ref().unwrap().raw_damage, 14);
    assert!(h.events.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::PeriodicTick {
                stacks: Some(3),
                raw_damage: 21,
                hp_damage: 1,
                ..
            }
        }
    )));
    h.close();
}

#[test]
fn basic_burning_fields_heal_reduction_caps_across_sources_and_fire_sea_kills_never_burst() {
    let mut h = host([SEA, SEA], |_| {});
    for i in 0..2 {
        h.action(
            i,
            Action::AttackGround {
                position: [460., 440.],
            },
        )
        .unwrap();
    }
    until(&mut h, |h| {
        status(h, 0, "A", BURN).is_some_and(|s| s.stacks == Some(1))
            && status(h, 0, "B", BURN).is_some_and(|s| s.stacks == Some(1))
    });
    let adjusted = |h: &DeltaHarness| {
        h.engine.world.as_ref().unwrap().basic_test_npc_heal(
            0,
            &h.engine.config,
            h.snapshot(0).logical_ms,
        )
    };
    assert_eq!(adjusted(&h), 9400);
    until(&mut h, |h| {
        status(h, 0, "A", BURN).is_some_and(|s| s.stacks == Some(6))
            && status(h, 0, "B", BURN).is_some_and(|s| s.stacks == Some(6))
    });
    assert_eq!(adjusted(&h), 7000);
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert_eq!(adjusted(&h), 8200);

    // Move the burning NPC and its unmarked neighbor outside the field.
    // The burn still deals its own damage, but this profile grants no burst.
    let world = h.engine.world.as_mut().unwrap();
    world.place_wave_test_npc(0, [700., 520.]);
    world.set_basic_test_npc_hp(0, 1);
    let target = h.snapshot(1).monsters[2].entity.clone();
    until(&mut h, |h| h.snapshot(1).monsters[0].hp == 0);
    assert!(status(&h, 2, "B", BURN).is_none());
    assert!(
        !h.events
            .iter()
            .any(|(_, e)| matches!(e, EventV4::Damage { target: unit, .. } if *unit == target))
    );
    h.close();
}
