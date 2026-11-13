use super::*;

#[path = "weapon_equipment_tests.rs"]
mod weapon_equipment;

fn profile_catalog(change: impl FnOnce(&mut Value)) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    profile_catalog_with_paths(false, change)
}

fn profile_catalog_with_paths(
    include_paths: bool,
    change: impl FnOnce(&mut Value),
) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    profile_catalog_with_features(include_paths, false, change)
}

fn profile_catalog_with_features(
    include_paths: bool,
    include_fields: bool,
    change: impl FnOnce(&mut Value),
) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    catalog_with(|v| {
        *v = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/mastery-stages.json"
        ))
        .unwrap();
        let mut recipe: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/basic-weapon-profiles.json"
        ))
        .unwrap();
        if !include_paths {
            let profiles = recipe["basic_profiles"].as_object_mut().unwrap();
            for id in [
                "sword_piercing_array",
                "shield_double_quake",
                "shield_double_quake_return",
            ] {
                profiles.remove(id);
            }
            recipe["status_definitions"]
                .as_array_mut()
                .unwrap()
                .retain(|status| {
                    !matches!(
                        status["id"].as_str().unwrap(),
                        "seal-recall-slow" | "ricochet-stun" | "ricochet-return-shield"
                    )
                });
        }
        if include_fields {
            v["members"]["basic_field_capacity"] = recipe["basic_field_capacity"].clone();
        } else {
            let profiles = recipe["basic_profiles"].as_object_mut().unwrap();
            profiles.remove("fan_echo_wave");
            profiles.remove("tower_expanding_ring");
        }
        v["combat_rules"]["basic_profiles"] = recipe["basic_profiles"].clone();
        for name in ["elite", "boss"] {
            v["combat_rules"]["profiles"][name] = v["combat_rules"]["profiles"]["npc"].clone();
        }
        v["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .extend(
                recipe["status_definitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
        change(v);
    })
}

fn profile_host(selected: [Option<&str>; 2], change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let include_paths = selected.iter().flatten().any(|id| {
        matches!(
            *id,
            "sword_piercing_array" | "shield_double_quake" | "shield_double_quake_return"
        )
    });
    let include_fields = selected
        .iter()
        .flatten()
        .any(|id| matches!(*id, "fan_echo_wave" | "tower_expanding_ring"));
    let cat = profile_catalog_with_features(include_paths, include_fields, change).unwrap();
    let mut request = create(&cat.default_config());
    request.protocol_version = 6;
    for (seat, profile) in request.participants.iter_mut().zip(selected) {
        seat.basic_profile = profile.map(str::to_owned);
    }
    let mut h = DeltaHarness::from_request(cat, request);
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

fn attack(h: &DeltaHarness, i: usize, target: usize) -> Action {
    let entity = &h.snapshot(i).monsters[target].entity;
    Action::AttackUnit {
        target: UnitTarget {
            id: entity.id.clone(),
            generation: entity.generation,
        },
    }
}

fn own_status<'a>(h: &'a DeltaHarness, i: usize, id: &str) -> Option<&'a StatusView> {
    h.snapshot(i)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == id)
}

#[test]
fn basic_profile_same_world_runs_shield_and_tower_with_private_selection_and_terminal_effects() {
    let mut h = profile_host([Some("shield"), Some("tower")], |v| {
        v["pve"]["monster_positions"] = json!([[520, 400], [450, 400], [440, 480], [570, 440]]);
        v["pve"]["combat"]["aggro_range"] = json!(60);
        v["pve"]["combat"]["melee_range"] = json!(60);
        v["pve"]["combat"]["attack_interval_ms"] = json!(240);
        v["pve"]["stats"]["atk"] = json!(100);
        let npc = v["pve"]["template_id"].as_str().unwrap().to_owned();
        let mut rule = v["combat_rules"]["basic_attack"].clone();
        rule.as_object_mut().unwrap().remove("life_steal_timing");
        v["combat_rules"]["npc_attacks"][npc] = rule;
    });
    assert_eq!(h.snapshot(0).own.basic_profile.as_deref(), Some("shield"));
    assert_eq!(h.snapshot(1).own.basic_profile.as_deref(), Some("tower"));
    assert_eq!(h.action(0, attack(&h, 0, 3)), Err(Error::InvalidTarget));
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.action(1, attack(&h, 1, 3)).unwrap();
    h.pump();
    assert_eq!(
        h.snapshot(0)
            .own
            .pending_action
            .as_ref()
            .unwrap()
            .commits_at_ms,
        160
    );
    assert_eq!(
        h.snapshot(1)
            .own
            .pending_action
            .as_ref()
            .unwrap()
            .commits_at_ms,
        180
    );

    h.ticks(6);
    let state = h.snapshot(0);
    let plans = state.basic_waves.as_ref().unwrap();
    assert_eq!(
        plans.iter().map(|w| w.plan_id.as_str()).collect::<Vec<_>>(),
        ["shield-wave", "tower-wave"]
    );
    assert_eq!((plans[0].pending.len(), plans[1].pending.len()), (2, 4));
    assert_eq!(h.snapshot(1).basic_waves.as_ref(), Some(plans));
    let reduction = own_status(&h, 1, "tower-reduction").unwrap();
    assert_eq!(
        (
            reduction.applied_at_ms,
            reduction.expires_at_ms,
            reduction.modifier_bps
        ),
        (180, 1080, 1500)
    );
    assert!(own_status(&h, 0, "shield-absorption").is_none());

    h.ticks(2);
    assert_eq!(h.snapshot(1).own.hp, 915);
    assert!(own_status(&h, 0, "shield-absorption").is_none());
    assert!(
        h.snapshot(1)
            .monsters
            .iter()
            .all(|npc| npc.statuses.as_ref().is_none_or(Vec::is_empty))
    );
    assert_eq!(h.snapshot(0).own.spirit, 100);
    assert_eq!(h.snapshot(1).own.spirit, 100);

    h.ticks(2);
    let absorption = own_status(&h, 0, "shield-absorption").unwrap();
    assert_eq!(
        (absorption.applied_at_ms, absorption.expires_at_ms),
        (300, 1500)
    );
    assert_eq!(absorption.shield.as_ref().unwrap().granted, 50);
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1300);
    assert_eq!(h.snapshot(0).own.spirit, 102);
    assert!(own_status(&h, 1, "shield-absorption").is_none());

    h.ticks(2);
    assert_eq!(h.snapshot(1).own.basic_ready_at_ms, 1460);
    assert_eq!(h.snapshot(1).own.spirit, 102);
    assert!(h.snapshot(1).basic_waves.as_ref().unwrap().is_empty());
    for npc in &h.snapshot(1).monsters {
        let statuses = npc.statuses.as_ref().unwrap();
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.source.kind, StatusEntityKind::Participant);
        assert_eq!(status.source.id, h.snapshot(1).own.participant.entity.id);
        assert_eq!(
            status.source.generation,
            h.snapshot(1).own.participant.entity.generation
        );
        assert_eq!(status.target.kind, StatusEntityKind::Monster);
        assert_eq!(status.target.id, npc.entity.id);
        assert_eq!(status.target.generation, npc.entity.generation);
        assert_eq!(
            (
                status.applied_at_ms,
                status.expires_at_ms,
                status.stacks,
                status.max_stacks
            ),
            (360, 5360, Some(1), Some(2))
        );
        assert_eq!(
            status.modifier_bps,
            if status.definition_id == "tower-attack" {
                -800
            } else {
                -1000
            }
        );
    }
    assert!(
        h.snapshot(0)
            .monsters
            .iter()
            .all(|npc| npc.statuses.as_ref().is_none_or(Vec::is_empty))
    );
    h.close();
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}

#[test]
fn basic_profile_tower_scales_target_status_and_caps_two_layers_with_refresh() {
    for (profile, expected) in [("elite", -750), ("boss", -500)] {
        let mut h = profile_host([Some("tower"), None], |v| {
            v["pve"]["monster_positions"] = json!([[450, 400]]);
            v["combat_rules"]["default_npc_profile"] = json!(profile);
            v["combat_rules"]["basic_profiles"]["tower"]["effects"]["on_end_damage_target_status"]
                ["choices"] = json!(["tower-movement"]);
        });
        assert!(h.snapshot(1).own.basic_profile.is_none());
        let mut prior = None;
        for stacks in [1, 2, 2] {
            h.action(0, Action::Attack {}).unwrap();
            h.ticks(8);
            let snapshot = h.snapshot(0);
            let statuses = snapshot.monsters[0].statuses.as_ref().unwrap();
            assert_eq!(statuses.len(), 1);
            let s = &statuses[0];
            assert_eq!(
                (s.modifier_bps, s.stacks, s.max_stacks),
                (expected, Some(stacks), Some(2))
            );
            assert_eq!(s.expires_at_ms - s.applied_at_ms, 5000);
            if let Some((id, deadline)) = prior.replace((s.instance_id.clone(), s.expires_at_ms)) {
                assert_eq!(id, s.instance_id);
                assert!(deadline < s.expires_at_ms);
            }
            h.ticks(37);
        }
    }
}

#[test]
fn basic_profile_reconnect_replays_the_same_selection_and_cancels_terminal_grant() {
    let mut h = profile_host([Some("shield"), None], |v| {
        v["pve"]["monster_positions"] = json!([[520, 400], [440, 400]]);
    });
    h.action(0, attack(&h, 0, 0)).unwrap();
    h.ticks(8);
    assert_eq!(h.snapshot(0).monsters[1].hp, 9938);
    assert!(own_status(&h, 0, "shield-absorption").is_none());
    let previous_connection = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection,
            authenticated_principal: "principal-A".into(),
        },
    );
    reconnect.connection = Connection {
        id: "profile-reconnect".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.sequences[0] = 1;
    h.pump();
    assert!(matches!(
        h.latest(0).payload,
        PublicationPayloadV6::Full { .. }
    ));
    assert_eq!(h.snapshot(0).own.basic_profile.as_deref(), Some("shield"));
    assert_eq!(
        h.snapshot(0).basic_waves.as_ref().unwrap()[0].pending.len(),
        1
    );
    h.ticks(2);
    assert_eq!(h.snapshot(0).monsters[0].hp, 9928);
    assert!(own_status(&h, 0, "shield-absorption").is_none());
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 1300);
    assert_eq!(h.snapshot(0).own.spirit, 100);
}

#[test]
fn basic_profile_sword_locks_the_projectile_to_the_trusted_primary() {
    let mut h = profile_host([Some("sword"), None], |v| {
        v["pve"]["monster_positions"] = json!([[450, 400], [540, 400]]);
    });
    h.action(0, attack(&h, 0, 1)).unwrap();
    h.ticks(4);
    assert_eq!(
        h.snapshot(0).projectiles[0]
            .unit_target
            .as_ref()
            .unwrap()
            .id,
        h.snapshot(0).monsters[1].entity.id
    );
    h.ticks(1);
    assert!(h.snapshot(0).projectiles.is_empty());
    assert_eq!(
        h.snapshot(0)
            .monsters
            .iter()
            .map(|npc| npc.hp)
            .collect::<Vec<_>>(),
        [10000, 10000]
    );
    assert_eq!(h.snapshot(0).own.basic_ready_at_ms, 850);
    assert_eq!(h.snapshot(0).own.spirit, 100);
}

#[test]
fn basic_profile_cold_config_and_create_reject_invalid_or_forged_choices() {
    for change in [
        |v: &mut Value| v["combat_rules"]["basic_profiles"]["shield"]["range"] = json!(0),
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["shield"]["settle_on_end"] = json!(false)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["shield"]["effects"]["on_end_damage_self_status"] =
                json!("missing")
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["tower"]["effects"]["on_end_damage_target_status"]
                ["choices"] = json!(["tower-movement", "tower-movement"])
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"]["tower"]["effects"]["on_end_damage_target_status"]
                ["target_profile_bps"]["elite"] = json!(10001)
        },
    ] {
        assert!(profile_catalog(change).is_err());
    }
    let cat = profile_catalog(|_| {}).unwrap();
    let config = cat.default_config();
    let mut request = create(&config);
    request.protocol_version = 6;
    request.participants[0].basic_profile = Some("unknown".into());
    assert!(matches!(
        TwoMembers::create(config.as_ref().clone(), cat, request),
        Err(Error::InvalidMessage)
    ));
    assert!(
        serde_json::from_value::<Action>(json!({"kind":"attack","basic_profile":"shield"}))
            .is_err()
    );

    let cat = delta_catalog(2, |_| {}).unwrap();
    let config = cat.default_config();
    let mut request = delta_request(&config);
    request.participants[0].basic_profile = Some("shield".into());
    assert!(matches!(
        TwoMembers::create(config.as_ref().clone(), cat, request),
        Err(Error::Unsupported)
    ));
}

#[test]
fn basic_profile_is_part_of_recovery_identity() {
    let mut h = profile_host([Some("shield"), None], |_| {});
    h.close();
    let mut request = h.engine.create.clone();
    request.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: h.checkpoint.clone().unwrap(),
    });
    let config = h.engine.config.clone();
    let cat = profile_catalog(|_| {}).unwrap();
    assert!(TwoMembers::create(config.clone(), cat.clone(), request.clone()).is_ok());
    request.participants[0].basic_profile = Some("tower".into());
    assert!(TwoMembers::create(config, cat, request).is_err());
}

#[test]
fn basic_profile_stacked_slow_is_shared_by_target_and_cleared_with_its_current_source() {
    let mut h = profile_host([Some("tower"), Some("tower")], |v| {
        v["pve"]["monster_positions"] = json!([[450, 440]]);
        v["combat_rules"]["basic_profiles"]["tower"]["effects"]["on_end_damage_target_status"]["choices"] =
            json!(["tower-movement"]);
    });
    for i in 0..2 {
        h.action(i, Action::Attack {}).unwrap();
    }
    h.ticks(9);
    let npc = &h.snapshot(1).monsters[0];
    assert_eq!(npc.hp, 9870);
    let effects = npc.statuses.as_ref().unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].stacks, Some(2));
    assert_eq!(effects[0].source.id, "B");
    assert!(
        h.snapshot(0).monsters[0]
            .statuses
            .as_ref()
            .is_none_or(Vec::is_empty)
    );
    h.action(1, Action::Leave {}).unwrap();
    h.pump();
    assert!(
        h.snapshot(0).monsters[0]
            .statuses
            .as_ref()
            .is_none_or(Vec::is_empty)
    );
    assert!(
        h.events
            .iter()
            .any(|(recipient, e)| recipient.as_deref() == Some("B")
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::StatusRemoved {
                            reason: StatusRemovalReason::SourceInvalid,
                            ..
                        }
                    }
                ))
    );
}

#[path = "basic_impact_tests.rs"]
mod basic_impact;

#[path = "basic_beam_tests.rs"]
mod basic_beam;

#[path = "basic_wave_controls_tests.rs"]
mod basic_wave_controls;

#[path = "basic_projectile_followups_tests.rs"]
mod basic_projectile_followups;

#[path = "basic_world_fields_tests.rs"]
mod basic_world_fields;

#[path = "basic_burning_fields_tests.rs"]
mod basic_burning_fields;

#[path = "basic_charge_tests.rs"]
mod basic_charge;

#[path = "basic_flight_and_stack_tests.rs"]
mod basic_flight_and_stack;
