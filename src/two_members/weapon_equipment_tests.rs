use super::*;

fn equip_catalog(change: impl FnOnce(&mut Value)) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    profile_catalog_with_features(true, true, |v| {
        let catalog: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/weapon-catalog.json"
        ))
        .unwrap();
        v["combat_rules"]["weapon_catalog"] = catalog["weapon_catalog"].clone();
        for recipe in [
            crate::test_data::fixture!("examples/combat-host/basic-burning-profiles.json"),
            crate::test_data::fixture!("examples/combat-host/basic-charge-profiles.json"),
            crate::test_data::fixture!("examples/combat-host/basic-flight-and-stack-finisher.json"),
        ] {
            let recipe: Value = serde_json::from_str(recipe).unwrap();
            v["combat_rules"]["basic_profiles"]
                .as_object_mut()
                .unwrap()
                .extend(recipe["basic_profiles"].as_object().unwrap().clone());
            v["pve"]["status_definitions"]
                .as_array_mut()
                .unwrap()
                .extend(recipe["status_definitions"].as_array().unwrap().clone());
            if let Some(elements) = recipe["elements"].as_array() {
                v["combat_rules"]["elements"]
                    .as_array_mut()
                    .unwrap()
                    .extend(elements.clone());
                v["combat_rules"]["periodic_damage"] = recipe["periodic_damage"].clone();
            }
        }
        v["pve"]["combat"]["aggro_range"] = json!(1);
        change(v);
    })
}

fn instance(id: &str, model: &str, morph: Option<&str>) -> WeaponInstance {
    WeaponInstance {
        instance_id: id.into(),
        model_id: model.into(),
        morph_id: morph.map(str::to_owned),
    }
}

fn armed(weapons: [Option<WeaponInstance>; 2], change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let cat = equip_catalog(change).unwrap();
    let mut r = create(&cat.default_config());
    r.protocol_version = 6;
    for (s, weapon) in r.participants.iter_mut().zip(weapons) {
        s.weapon = weapon;
    }
    let mut h = DeltaHarness::from_request(cat, r);
    for store in &mut h.stores {
        store.hp = 1000;
        store.spirit = 2_000_000;
    }
    h.active();
    h
}

fn change(
    h: &DeltaHarness,
    i: usize,
    op: &str,
    revision: u32,
    weapon: Option<WeaponInstance>,
) -> SetEquippedWeapon {
    SetEquippedWeapon {
        key: h.engine.create.key.clone(),
        participant_id: h.engine.create.participants[i].participant_id.clone(),
        entity_generation: 1,
        operation_id: op.into(),
        expected_revision: revision,
        weapon,
    }
}

fn wait(h: &mut DeltaHarness, done: impl Fn(&DeltaHarness) -> bool) {
    for _ in 0..400 {
        if done(h) {
            return;
        }
        h.ticks(1);
    }
    panic!("expected equipment or combat state");
}

#[test]
fn weapon_equipment_resolves_source_models_and_morphs_from_one_frozen_catalog() {
    let catalog = equip_catalog(|_| {}).unwrap();
    let c = catalog.default_config();
    let weapons = c.weapon_catalog().unwrap();
    assert_eq!((weapons.models.len(), weapons.families.len()), (12, 6));
    assert_eq!(
        weapons
            .families
            .values()
            .map(|f| f.morphs.len())
            .sum::<usize>(),
        18
    );
    for (model, family) in &weapons.models {
        let base = &weapons.families[family].base_profile;
        for wrong in [None, Some("retired-placeholder"), Some("unrecognized")] {
            let (profile, equip) = weapons
                .resolve(&instance("weapon-1", model, wrong))
                .unwrap();
            assert_eq!(profile, base);
            assert_eq!(equip.morph_id, None);
        }
        for (morph, wanted) in &weapons.families[family].morphs {
            let (profile, equip) = weapons
                .resolve(&instance("weapon-1", model, Some(morph)))
                .unwrap();
            assert_eq!(profile, wanted);
            assert_eq!(equip.morph_id.as_deref(), Some(morph.as_str()));
        }
    }
    for config in [
        json!({"models":{"sword":"other"},"families":{"sword":{"base_profile":"sword"}}}),
        json!({"models":{"sword":"sword"},"families":{"sword":{"base_profile":"unknown"}}}),
        json!({"models":{"sword":"sword","fan":"fan"},"families":{"sword":{"base_profile":"sword","morphs":{"same":"sword_dual_strike"}},"fan":{"base_profile":"fan","morphs":{"same":"fan_four_winds"}}}}),
    ] {
        assert!(equip_catalog(|v| v["combat_rules"]["weapon_catalog"] = config).is_err());
    }
    let mut r = create(&c);
    r.protocol_version = 6;
    r.participants[0].weapon = Some(instance("one", "guhong-sword", Some("fan_four_winds")));
    r.participants[1].weapon = Some(instance("one", "fan", None));
    assert!(matches!(
        TwoMembers::create((*c).clone(), catalog.clone(), r.clone()),
        Err(Error::Conflict)
    ));
    r.participants[1].weapon = Some(instance("two", "unknown", None));
    assert!(matches!(
        TwoMembers::create((*c).clone(), catalog.clone(), r.clone()),
        Err(Error::InvalidMessage)
    ));
    r.participants[1].weapon = None;
    r.participants[0].basic_profile = Some("sword".into());
    assert!(matches!(
        TwoMembers::create((*c).clone(), catalog, r),
        Err(Error::InvalidMessage)
    ));
    assert!(serde_json::from_value::<Action>(json!({"kind":"set_equipped_weapon"})).is_err());
}

#[test]
fn weapon_equipment_swaps_beam_with_private_cas_and_retains_cooldown() {
    let mut h = armed(
        [
            Some(instance("mirror-1", "mirror", Some("mirror_world_beam"))),
            Some(instance("sword-2", "guhong-sword", Some("fan_four_winds"))),
        ],
        |_| {},
    );
    let peer = h.snapshot(1).own.equipment.clone().unwrap();
    assert!(peer.weapon.as_ref().unwrap().morph_id.is_none());
    assert_eq!(h.snapshot(1).own.basic_profile.as_deref(), Some("sword"));
    h.action(
        0,
        Action::StartAttackBeam {
            direction: [1., 0.],
        },
    )
    .unwrap();
    wait(&mut h, |h| {
        h.snapshot(0)
            .basic_beams
            .as_ref()
            .is_some_and(|b| b.iter().any(|b| b.ticks > 0))
    });
    let cd = h.engine.now + 900;
    let duplicate = change(&h, 0, "duplicate", 0, peer.weapon.clone());
    assert_eq!(
        h.engine.set_equipped_weapon(duplicate),
        Err(Error::Conflict)
    );
    let q = change(
        &h,
        0,
        "equip",
        0,
        Some(instance("fan-1", "fan", Some("fan_four_winds"))),
    );
    let receipt = h.engine.set_equipped_weapon(q.clone()).unwrap();
    assert_eq!(h.engine.set_equipped_weapon(q.clone()), Ok(receipt.clone()));
    let mut conflicting = q.clone();
    conflicting.weapon = None;
    assert_eq!(
        h.engine.set_equipped_weapon(conflicting),
        Err(Error::Conflict)
    );
    let mut stale = q;
    stale.operation_id = "stale".into();
    assert_eq!(
        h.engine.set_equipped_weapon(stale),
        Err(Error::RevisionConflict)
    );
    wait(&mut h, |h| {
        h.snapshot(0)
            .own
            .equipment
            .as_ref()
            .is_some_and(|e| e.revision == 1)
    });
    assert_eq!(
        h.snapshot(0).own.basic_profile.as_deref(),
        Some("fan_four_winds")
    );
    assert!(h.snapshot(0).basic_beams.as_ref().unwrap().is_empty());
    assert_eq!(h.snapshot(1).own.equipment.as_ref(), Some(&peer));
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, cd);
    assert!(h.events.iter().any(|(_, e)| matches!(
        e,
        EventV4::BasicBeamEnded {
            reason: BasicBeamEndReason::EquipmentChanged,
            ..
        }
    )));
    assert_eq!(
        h.events
            .iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::WeaponEquipmentChanged { .. }
                }
            ))
            .count(),
        1
    );
}

#[test]
fn weapon_equipment_waits_for_released_flight_and_interrupts_next_hold() {
    let mut h = armed(
        [
            Some(instance(
                "cauldron-1",
                "qingnang-cauldron",
                Some("cauldron_core_burst"),
            )),
            None,
        ],
        |_| {},
    );
    h.action(0, Action::StartBasicCharge {}).unwrap();
    h.ticks(1);
    let charge = h
        .snapshot(0)
        .own
        .basic_charge
        .as_ref()
        .unwrap()
        .session_id
        .clone();
    h.action(
        0,
        Action::ReleaseBasicCharge {
            session_id: charge,
            target: None,
            direction: None,
            position: Some([800., 400.]),
        },
    )
    .unwrap();
    wait(&mut h, |h| !h.snapshot(0).projectiles.is_empty());
    let q = change(
        &h,
        0,
        "fan",
        0,
        Some(instance("fan-1", "fan", Some("fan_four_winds"))),
    );
    assert_eq!(
        h.engine.set_equipped_weapon(q.clone()),
        Err(Error::InvalidState)
    );
    let p = h.snapshot(0).projectiles[0].id.clone();
    wait(&mut h, |h| {
        h.events.iter().any(
            |(_, e)| matches!(e,EventV4::BasicFlightBurst{projectile_id,..} if projectile_id==&p),
        )
    });
    assert_eq!(
        h.engine.set_equipped_weapon(q).unwrap().equipment.revision,
        1
    );
    wait(&mut h, |h| {
        h.snapshot(0).logical_ms >= h.snapshot(0).own.basic_ready_at_ms
    });
    h.action(0, Action::StartBasicCharge {}).unwrap();
    wait(&mut h, |h| h.snapshot(0).own.basic_charge.is_some());
    let q = change(&h, 0, "unarmed", 1, None);
    h.engine.set_equipped_weapon(q).unwrap();
    wait(&mut h, |h| h.snapshot(0).own.basic_profile.is_none());
    assert!(h.snapshot(0).own.basic_charge.is_none());
    assert_eq!(
        h.snapshot(0).own.equipment,
        Some(WeaponEquipmentView {
            revision: 2,
            weapon: None
        })
    );
    assert_eq!(h.snapshot(1).own.equipment, None);
}

#[test]
fn weapon_equipment_drops_only_the_old_weapon_floors_and_replays_closed_receipts() {
    let old = instance("tower-1", "tower", Some("tower_double_ring"));
    let mut h = armed(
        [
            Some(old.clone()),
            Some(instance("tower-2", "tower", Some("tower_double_ring"))),
        ],
        |_| {},
    );
    for i in 0..2 {
        h.action(i, attack(&h, i, 0)).unwrap();
    }
    wait(&mut h, |h| {
        (0..2).all(|i| own_status(h, i, "ascending-pagoda-floor").is_some())
    });
    let q = change(
        &h,
        0,
        "new-instance",
        0,
        Some(instance("tower-3", "tower", old.morph_id.as_deref())),
    );
    let receipt = h.engine.set_equipped_weapon(q.clone()).unwrap();
    wait(&mut h, |h| {
        h.snapshot(0)
            .own
            .equipment
            .as_ref()
            .is_some_and(|e| e.revision == 1)
    });
    assert!(own_status(&h, 0, "ascending-pagoda-floor").is_none());
    assert_eq!(
        own_status(&h, 1, "ascending-pagoda-floor").unwrap().stacks,
        Some(1)
    );
    let mut r = h.engine.create.clone();
    r.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: h.engine.checkpoint(),
    });
    let cat = equip_catalog(|_| {}).unwrap();
    let c = cat.default_config();
    let mut closed = TwoMembers::create((*c).clone(), cat.clone(), r.clone()).unwrap();
    assert!(closed.world.is_none());
    assert_eq!(closed.set_equipped_weapon(q), Ok(receipt));
    assert_eq!(
        closed.set_equipped_weapon(change(&h, 0, "after-close", 1, None)),
        Err(Error::Closed)
    );
    let cp = &mut r.recovery.as_mut().unwrap().checkpoint;
    cp.state.weapon_operations[0].receipt.basic_profile = Some("forged".into());
    cp.sha256 = state_digest(&cp.state);
    assert!(TwoMembers::create((*c).clone(), cat, r).is_err());
}

#[test]
fn weapon_equipment_does_not_change_a_detached_fields_armor_policy() {
    let mut h = armed(
        [Some(instance("fan-1", "fan", Some("fan_echo_wave"))), None],
        |v| {
            v["combat_rules"]["basic_attack"]["ignore_armor"] = json!(false);
            v["combat_rules"]["basic_profiles"]["fan_echo_wave"]["ignore_armor"] = json!(true);
            v["pve"]["stats"]["def"] = json!(1000);
            v["pve"]["monster_positions"] = json!([[450, 400]]);
        },
    );
    h.action(0, attack(&h, 0, 0)).unwrap();
    wait(&mut h, |h| {
        h.snapshot(0).areas.as_ref().is_some_and(|a| !a.is_empty())
    });
    let area = h.snapshot(0).areas.as_ref().unwrap()[0].id.clone();
    let hp = h.snapshot(0).monsters[0].hp;
    let has_tick = |h: &DeltaHarness, n| {
        h.events.iter().any(|(_,e)| {
        matches!(e,EventV4::AreaTick{area_id,tick_index,..} if area_id==&area && *tick_index==n)
    })
    };
    wait(&mut h, |h| has_tick(h, 1));
    let after_first = h.snapshot(0).monsters[0].hp;
    let q = change(
        &h,
        0,
        "shield",
        0,
        Some(instance("shield-1", "shield", None)),
    );
    h.engine.set_equipped_weapon(q).unwrap();
    wait(&mut h, |h| has_tick(h, 2));
    let after_second = h.snapshot(0).monsters[0].hp;
    assert!(hp > after_first);
    assert_eq!(hp - after_first, after_first - after_second);
    assert_eq!(h.snapshot(0).own.basic_profile.as_deref(), Some("shield"));
}
