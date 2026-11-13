use super::*;

fn setup(v: &mut Value) {
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["members"]["member_conflict"] = json!(true);
    v["members"]["spectators"] = json!({"capacity":1,"terminal_retention_ms":400});
    v["extraction_point"] = json!([20, 30]);
    v["pve"]["combat"]["aggro_range"] = json!(2);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["monster_positions"] = json!([[100, 100]]);
    v["pve"]["extra_spawns"] = json!([
        {"id":"chaser-a","template_id":"npc","position":[45,40]},
        {"id":"chaser-b","template_id":"npc","position":[45,20]}
    ]);
    v["combat_rules"]["basic_profiles"]["fast"] = v["basic"].clone();
    v["combat_rules"]["basic_profiles"]["fast"]["settle_on_end"] = json!(true);
    v["combat_rules"]["basic_profiles"]["fast"]["interval_ms"] = json!(180);
    v["combat_rules"]["basic_profiles"]["fast"]["projectile_speed"] = json!(900);
    v["combat_rules"]["basic_profiles"]["second"] = v["basic"].clone();
    v["combat_rules"]["basic_profiles"]["second"]["settle_on_end"] = json!(true);
    v["combat_rules"]["basic_profiles"]["second"]["interval_ms"] = json!(210);
    v["world_loot"] = json!({
        "environment_capacity":0,"container_capacity":0,"partial_pickup":false,
        "interaction_range":12,"shared_clearance":0,"pools":{},"nodes":[],"containers":[],
        "instances":{"unique-a":{"quality":8,"relic_eligible":false},
                     "unique-b":{"quality":9,"relic_eligible":false}}
    });
    v["pve"]["items"]["unique-a"] = json!({"name":"A","stackLimit":1});
    v["pve"]["items"]["unique-b"] = json!({"name":"B","stackLimit":1});
    v["contested_equipment"] = json!({
        "interaction_range":12,"pursuit_range":75,"retire_delay_ms":90,
        "nodes":[
            {"id":"node-a","position":[20,30],"basic_profile":"fast",
             "secured_item_id":"unique-a","pursuit_ids":["chaser-a"]},
            {"id":"node-b","position":[20,30],"basic_profile":"second",
             "secured_item_id":"unique-b","pursuit_ids":["chaser-b"]}
        ]
    });
}

fn claim(
    w: &mut World,
    seat: usize,
    id: &str,
    revision: u32,
    c: &Config,
    now: u64,
) -> Result<Events> {
    w.action(
        seat,
        &Action::ClaimWorldEquipment {
            node_id: id.into(),
            expected_revision: revision,
        },
        "claim",
        c,
        now,
    )
}

#[test]
fn contested_equipment_claim_changes_combat_and_preserves_public_private_boundaries() {
    let c = catalog(input(), setup).unwrap().default_config();
    let mut w = world(&c);
    let before = w.snapshot(0, &c, Phase::Active, 0);
    assert_eq!(before.monsters.len(), 1);
    w.members[0].position = [20., 10.];
    assert_eq!(
        claim(&mut w, 0, "node-a", 1, &c, 0),
        Err(Error::InvalidTarget)
    );
    w.members[0].position = [20., 20.];
    claim(&mut w, 0, "node-a", 1, &c, 0).unwrap();
    assert_eq!(
        claim(&mut w, 1, "node-a", 1, &c, 0),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        claim(&mut w, 1, "node-a", 2, &c, 0),
        Err(Error::InvalidState)
    );
    let a = w.members[0].reference.entity.clone();
    let b = w.members[1].reference.entity.clone();
    let row = &w.contested_views().unwrap()[0];
    assert_eq!(row.holder, Some(a.clone()));
    let before_tick = w.snapshot(0, &c, Phase::Active, 0);
    assert_eq!(
        before_tick.own.world_weapon_profile.as_deref(),
        Some("fast")
    );
    assert!(before_tick.own.basic_profile.is_none());
    assert!(before_tick.own.equipment.is_none());
    assert!(before_tick.own.instances.is_empty());
    assert_eq!(w.members[0].free_inventory_slots(&c), 2);
    assert!(
        w.snapshot(1, &c, Phase::Active, 0)
            .own
            .world_weapon_profile
            .is_none()
    );
    let observer = w.spectator_snapshot(&c, Phase::Active, 0).unwrap();
    assert_eq!(
        observer.contested_equipment,
        before_tick.contested_equipment
    );
    assert!(!serde_json::to_string(&observer).unwrap().contains("fast"));

    let j = w
        .npcs
        .iter()
        .position(|n| n.entity.id == "chaser-a")
        .unwrap();
    let hunter = w.npcs[j].entity.clone();
    assert!(w.hostile_contact(&b, &hunter));
    assert!(!w.hostile_contact(&hunter, &b));
    assert!(w.hostile_contact(&hunter, &a));
    let target = World::unit_wire(&hunter);
    w.action(0, &Action::AttackUnit { target }, "basic", &c, 0)
        .unwrap();
    w.tick(&c, 30);
    assert_eq!(w.npcs[j].target, Some(a.clone()));
    assert_eq!(w.members[1].basic(&c).interval_ms, 300);
    let hit_at = [60, 90, 120]
        .into_iter()
        .find(|&now| {
            w.tick(&c, now);
            w.npcs[j].hp < 100
        })
        .unwrap();
    assert_eq!(w.members[0].basic_ready, hit_at + 180);
    w.protect(0, hit_at, &mut vec![]);
    assert_eq!(w.npcs[j].target, None);
    w.tick(&c, hit_at + 30);
    assert_eq!(w.npcs[j].target, None);
    assert_eq!(w.contested_views().unwrap()[0].holder, Some(a));
    assert_eq!(w.npcs[j].entity, hunter);
}

#[test]
fn contested_equipment_transfers_once_and_only_extraction_converts() {
    let c = catalog(input(), setup).unwrap().default_config();
    let mut w = world(&c);
    w.bind_instance_scope("a".repeat(64));
    claim(&mut w, 0, "node-a", 1, &c, 0).unwrap();
    claim(&mut w, 0, "node-b", 1, &c, 0).unwrap();
    assert_eq!(w.members[0].basic(&c).interval_ms, 210);
    let j = w
        .npcs
        .iter()
        .position(|n| n.entity.id == "chaser-a")
        .unwrap();
    let k = w
        .npcs
        .iter()
        .position(|n| n.entity.id == "chaser-b")
        .unwrap();
    let first_life = w.npcs[j].entity.clone();
    w.tick(&c, 30);
    w.members[0].position = [20., 30.];
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    let death = w.settlement(0, &c);
    assert!(matches!(death, Business::CommitSettlement { instances, .. } if instances.is_empty()));
    assert!(w.members[0].world_weapon_profile.is_none());
    assert_eq!(
        w.contested_views().unwrap()[0].state,
        WorldEquipmentState::Dropped
    );
    assert_eq!(w.contested_views().unwrap()[0].position, [20., 30.]);
    assert!(w.npcs[k].pursuit_visible());
    assert!(!w.npc_can_be_attacked(k));
    assert_eq!(w.npcs[k].target, None);
    w.tick(&c, 60);
    claim(&mut w, 1, "node-a", 3, &c, 60).unwrap();
    assert_eq!(w.npcs[j].entity.generation, first_life.generation + 1);
    assert!(!w.pursuit_allows_detached(&first_life, Owner::Member(1)));
    w.tick(&c, 90);
    assert_eq!(
        w.npcs[j].target,
        Some(w.members[1].reference.entity.clone())
    );
    assert!(w.npcs[k].pursuit_visible());
    w.action(1, &Action::ExtractStart {}, "extract", &c, 90)
        .unwrap();
    w.tick(&c, 120);
    assert!(!w.npcs[k].pursuit_visible());
    assert!(w.npcs[j].pursuit_visible());
    w.tick(&c, 150);
    w.tick(&c, 180);
    assert_eq!(w.members[1].desired, Some(Outcome::Extraction));
    assert_eq!(
        w.contested_views().unwrap()[0].state,
        WorldEquipmentState::Secured
    );
    let Business::CommitSettlement { instances, .. } = w.settlement(1, &c) else {
        panic!()
    };
    assert_eq!(instances.len(), 1);
    assert_eq!(
        instances[0].instance_id,
        format!("{}-{:016x}", "a".repeat(64), 1)
    );
    assert_eq!(instances[0].item_id, "unique-a");
    assert_eq!(instances[0].quality, 8);
    assert_eq!(c.pve_loot_bound("unique-a", 0, 180), 1);
    w.tick(&c, 3000);
    assert_eq!(
        w.contested_views().unwrap()[1].state,
        WorldEquipmentState::Lost
    );
    assert!(
        w.npcs
            .iter()
            .filter(|n| n.pursuit.is_some())
            .all(|n| !n.pursuit_visible())
    );
}

#[test]
fn contested_equipment_rejects_invalid_configuration_and_full_capacity() {
    for change in [
        |v: &mut Value| v["contested_equipment"]["nodes"][1]["pursuit_ids"] = json!(["chaser-a"]),
        |v: &mut Value| v["contested_equipment"]["nodes"][0]["basic_profile"] = json!("unknown"),
        |v: &mut Value| v["contested_equipment"]["nodes"][0]["position"] = json!([128, 20]),
        |v: &mut Value| v["contested_equipment"]["nodes"][0]["secured_item_id"] = json!("item"),
    ] {
        let error = catalog(input(), |v| {
            setup(v);
            change(v);
        })
        .err()
        .unwrap()
        .to_string();
        assert!(error.contains("CONFIG_CONTESTED_EQUIPMENT"), "{error}");
    }
    let c = catalog(input(), |v| {
        setup(v);
        v["inventory_capacity"] = json!(1);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    assert_eq!(claim(&mut w, 0, "node-a", 1, &c, 0), Err(Error::Capacity));
    assert_eq!(w.contested_views().unwrap()[0].revision, 1);
    assert!(
        w.npcs
            .iter()
            .filter(|n| n.pursuit.is_some())
            .all(|n| !n.pursuit_visible())
    );
}
