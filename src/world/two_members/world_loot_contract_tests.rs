use super::*;

#[path = "relic_contract_tests.rs"]
mod relic_contract_tests;

fn loot_input(v: &mut Value) {
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["pve"]["monster_positions"] = json!([]);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["params"]["runtime"]["resource_sweep_ms"] = json!(30);
    v["world_loot"] = json!({
        "environment_capacity":8,"container_capacity":4,"partial_pickup":true,
        "interaction_range":16,"shared_clearance":5,"pools":{},"nodes":[],"containers":[]
    });
}

fn pool(item: &str, quantity: u32) -> Value {
    json!({"guaranteed":[{"item_id":item,"quantity_min":quantity,"quantity_max":quantity}],"independent":[],"groups":[]})
}

fn container(id: &str, position: [f64; 2], visibility: Value) -> Value {
    json!({"id":id,"position":position,"pool":"box","visibility":visibility,
           "unlock_ms":30,"replenish_ms":60,"ttl_ms":600})
}

fn pickup(w: &mut World, i: usize, id: &str, c: &Config, now: u64) -> Result<Events> {
    w.action(
        i,
        &Action::Pickup {
            resource_id: id.into(),
            expected_revision: w.members[i].revision,
        },
        "pick",
        c,
        now,
    )
}

fn open(
    w: &mut World,
    i: usize,
    id: &str,
    generation: u32,
    c: &Config,
    now: u64,
) -> Result<Events> {
    w.action(
        i,
        &Action::OpenChest {
            chest_id: id.into(),
            expected_generation: generation,
        },
        "open",
        c,
        now,
    )
}

#[test]
fn partial_pickup_keeps_one_id_and_node_replenishes_after_removal_or_expiry() {
    let c = catalog(input(), |v| {
        loot_input(v);
        v["inventory_capacity"] = json!(1);
        v["world_loot"]["pools"]["supply"] = pool("item", 5);
        v["world_loot"]["nodes"] = json!([{"id":"supply","pool":"supply","points":[[20,30]],"replenish_ms":90,"ttl_ms":60}]);
    }).unwrap().default_config();
    let mut w = world(&c);
    w.tick(&c, 30);
    let id = w.resources[0].view.id.clone();
    let public = w.spectator_snapshot(&c, Phase::Active, 30).unwrap();
    assert_eq!(public.resources[0].id, id);
    let encoded = serde_json::to_value(public).unwrap();
    assert!(encoded.get("self").is_none());
    assert!(encoded["resources"][0].get("item_id").is_none());
    let partial = pickup(&mut w, 0, &id, &c, 30).unwrap();
    assert!(partial.iter().any(|(_, e)| matches!(e, EventV4::ResourceUpdated { resource, .. } if resource.id == id && resource.quantity == 2)));
    assert_eq!(w.members[0].inventory["item"], 4);
    let revision = w.members[0].revision;
    assert_eq!(pickup(&mut w, 0, &id, &c, 30).unwrap_err(), Error::Capacity);
    assert_eq!(w.members[0].revision, revision);
    assert_eq!(w.resources[0].view.quantity, 2);
    pickup(&mut w, 1, &id, &c, 30).unwrap();
    assert_eq!(w.members[1].inventory["item"], 3);
    assert!(w.resources.is_empty());
    w.tick(&c, 90);
    assert!(w.resources.is_empty());
    w.tick(&c, 120);
    assert_eq!(w.resources[0].view.quantity, 5);
    let second = w.resources[0].view.id.clone();
    assert_ne!(second, id);
    w.tick(&c, 180);
    assert!(w.resources.is_empty());
    w.tick(&c, 270);
    assert_ne!(w.resources[0].view.id, second);
    assert_eq!(c.world_loot_bound("item", 269), 15);
}

#[test]
fn frozen_pool_uses_no_replacement_and_prefers_an_actual_hotspot_anchor() {
    let c = catalog(input(), |v| {
        loot_input(v);
        for id in ["key","never","left","right"] {
            v["pve"]["items"][id] = json!({"name":id,"stackLimit":4});
        }
        v["pve"]["world_geometry"] = json!({
            "boundary":[[0,0],[128,0],[128,128],[0,128]],"blockers":[
                {"id":"gate","vertices":[[62,35],[70,35],[70,45],[62,45]],"blocks_movement":true,
                 "blocks_projectiles":false,"blocks_vision":false,"active_window":{"start_ms":0,"end_ms":90}}
            ]
        });
        v["world_loot"]["pools"]["supply"] = json!({"guaranteed":[{"item_id":"item","quantity_min":2,"quantity_max":2}],
            "independent":[
                {"chance_bps":0,"entry":{"item_id":"never","quantity_min":1,"quantity_max":1}},
                {"chance_bps":10000,"entry":{"item_id":"key","quantity_min":1,"quantity_max":1}}
            ],"groups":[{"chance_bps":10000,"count_min":2,"count_max":2,"entries":[
                {"item_id":"left","quantity_min":1,"quantity_max":3},{"item_id":"right","quantity_min":1,"quantity_max":3}
            ]}]});
        v["world_loot"]["pools"]["box"] = pool("item", 1);
        let mut anchor = container("anchor", [78.,40.], json!({"kind":"shared"}));
        anchor["hotspot_group"] = json!("cluster");
        v["world_loot"]["containers"] = json!([anchor]);
        v["world_loot"]["nodes"] = json!([
            {"id":"blocked","pool":"supply","points":[[66,40]],"replenish_ms":90,"ttl_ms":300},
            {"id":"preferred","pool":"box","points":[[20,30],[88,40]],"replenish_ms":90,"ttl_ms":300,
             "hotspot":{"group":"cluster","radius":16}}
        ]);
    }).unwrap().default_config();
    let mut w = world(&c);
    w.tick(&c, 30);
    assert_eq!(w.resources.len(), 1);
    assert_eq!(w.resources[0].view.position, [88., 40.]);
    let rng = w.rng;
    w.tick(&c, 60);
    assert_eq!(w.rng, rng);
    w.tick(&c, 90);
    assert_eq!(w.rng, rng);
    let supply: BTreeMap<_, _> = w
        .resources
        .iter()
        .filter(|r| r.view.position == [66., 40.])
        .map(|r| (r.view.item_id.as_deref().unwrap(), r.view.quantity))
        .collect();
    assert_eq!(supply.len(), 4);
    assert_eq!(supply["item"], 2);
    assert_eq!(supply["key"], 1);
    assert!((1..=3).contains(&supply["left"]) && (1..=3).contains(&supply["right"]));
}

#[test]
fn container_reservation_cost_privacy_and_generation_share_the_world_transaction() {
    let c = catalog(input(), |v| {
        loot_input(v);
        v["pve"]["monster_positions"] = json!([[80,80]]);
        v["pve"]["items"]["key"] = json!({"name":"key","stackLimit":4});
        let world = &mut v["world_loot"];
        world["container_capacity"] = json!(1);
        world["pools"]["box"] = pool("item", 2);
        world["pools"]["key"] = pool("key", 1);
        world["nodes"] = json!([{"id":"key-source","pool":"key","points":[[25,20]],"replenish_ms":600,"ttl_ms":600}]);
        let mut shared = container("shared", [20.,30.], json!({"kind":"shared"}));
        shared["guardians"] = json!(["npc-1"]);
        let mut personal = container("personal", [27.,25.], json!({"kind":"personal","participant_id":"a"}));
        personal["cost"] = json!({"item_id":"key","quantity":1});
        world["containers"] = json!([shared, personal, container("hidden", [20.,10.], json!({"kind":"hidden","reveal_radius":18}))]);
    }).unwrap().default_config();
    let mut w = world(&c);
    w.tick(&c, 30);
    assert_eq!(w.container_views(0, &c).unwrap().len(), 3);
    assert_eq!(w.container_views(1, &c).unwrap().len(), 1);
    assert_eq!(
        open(&mut w, 1, "personal", 1, &c, 30).unwrap_err(),
        Error::InvalidTarget
    );
    assert_eq!(
        open(&mut w, 0, "shared", 1, &c, 30).unwrap_err(),
        Error::InvalidTarget
    );
    w.npcs[0].hp = 0;
    let key = w.resources[0].view.id.clone();
    pickup(&mut w, 0, &key, &c, 30).unwrap();
    open(&mut w, 0, "shared", 1, &c, 30).unwrap();
    assert_eq!(
        open(&mut w, 1, "shared", 1, &c, 30).unwrap_err(),
        Error::Conflict
    );
    w.protect(0, 30, &mut vec![]);
    open(&mut w, 1, "shared", 1, &c, 30).unwrap();
    w.tick(&c, 60);
    w.action(0, &Action::Foreground {}, "foreground", &c, 60)
        .unwrap();
    w.action(
        0,
        &Action::CombatReady {
            publication_id: "ready".into(),
        },
        "ready",
        &c,
        60,
    )
    .unwrap();
    open(&mut w, 0, "personal", 1, &c, 60).unwrap();
    let released = w.tick(&c, 90);
    assert_eq!(w.members[0].inventory["key"], 1);
    assert!(released.iter().any(|(i, e)| *i == Some(0) && matches!(e, EventV4::ContainerReleased { container_id, .. } if container_id == "personal")));
    let shared = w.resources[0].view.id.clone();
    pickup(&mut w, 1, &shared, &c, 90).unwrap();
    open(&mut w, 0, "personal", 1, &c, 90).unwrap();
    let committed = w.tick(&c, 120);
    assert_eq!(w.members[0].inventory.get("key"), None);
    assert_eq!(w.container_views(1, &c).unwrap()[0].generation, 2);
    assert_eq!(
        open(&mut w, 1, "shared", 1, &c, 120).unwrap_err(),
        Error::RevisionConflict
    );
    let secret = w.resources[0].view.id.clone();
    assert!(committed.iter().any(|(i, e)| *i == Some(0)
        && matches!(e, EventV4::ResourceSpawned { resource } if resource.id == secret)));
    assert!(w.snapshot(1, &c, Phase::Active, 120).resources.is_empty());
    assert!(
        w.spectator_snapshot(&c, Phase::Active, 120)
            .unwrap()
            .resources
            .is_empty()
    );
    assert_eq!(
        pickup(&mut w, 1, &secret, &c, 120).unwrap_err(),
        Error::InvalidTarget
    );
    pickup(&mut w, 0, &secret, &c, 120).unwrap();
    assert_eq!(w.members[0].inventory["item"], 3);
}

#[test]
fn participant_ai_opens_its_container_through_the_shared_action() {
    let c = catalog(input(), |v| {
        loot_input(v);
        v["world_loot"]["pools"]["box"] = pool("item", 2);
        v["world_loot"]["containers"] = json!([
            container("bot-source", [20.,40.], json!({"kind":"personal","participant_id":"b"}))
        ]);
        v["participant_ai"] = json!({"b":{"decision_interval_ms":30,"loot_radius":32,"threat_radius":0,
            "withdraw_hp_bps":0,"extract_after_looted":2,"extract_before_timeout_ms":0}});
    }).unwrap().default_config();
    let mut w = world(&c);
    let started = w.tick(&c, 30);
    assert!(
        started.iter().any(|(recipient, e)| *recipient == Some(1)
            && matches!(e, EventV4::ContainerReserved { .. }))
    );
    let opened = w.tick(&c, 60);
    assert!(opened.iter().any(
        |(recipient, e)| *recipient == Some(1) && matches!(e, EventV4::ContainerOpened { .. })
    ));
    w.tick(&c, 90);
    assert_eq!(w.members[1].inventory["item"], 3);
    assert_eq!(w.members[0].inventory["item"], 1);
}

fn monster_loot_input(v: &mut Value) {
    loot_input(v);
    v["pve"]["monster_positions"] = json!([[40, 30]]);
    v["world_loot"]["one_shot_capacity"] = json!(1);
    v["world_loot"]["pools"]["drop"] = pool("item", 5);
    v["world_loot"]["monster_pools"] = json!({
        "npc":{"pool":"drop","priority_ms":90,"correction_radius":0}
    });
}

fn hit_monster(
    w: &mut World,
    c: &Config,
    (npc, member): (usize, usize),
    raw: u32,
    now: u64,
) -> u32 {
    w.resolve_damage(
        combat::DamageIntent {
            source: w.members[member].reference.entity.clone(),
            target: w.npcs[npc].entity.clone(),
            raw,
            direct: false,
            ignore_armor: true,
        },
        c,
        now,
        &mut vec![],
    )
    .hp_damage
}

#[test]
fn monster_pool_priority_uses_actual_damage_and_expires_on_the_boundary() {
    let c = catalog(input(), |v| {
        monster_loot_input(v);
        v["inventory_capacity"] = json!(1);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    assert_eq!(hit_monster(&mut w, &c, (0, 0), 60, 0), 60);
    assert_eq!(hit_monster(&mut w, &c, (0, 1), 100, 0), 40);
    let events = w.tick(&c, 30);
    let a = w.members[0].reference.entity.clone();
    let resource = w
        .snapshot(1, &c, Phase::Active, 30)
        .resources
        .into_iter()
        .find(|r| r.item_id.is_some())
        .unwrap();
    let id = &resource.id;
    assert_eq!(resource.owner.as_ref(), Some(&a));
    assert!(events.iter().any(|(seat, event)| seat.is_none()
        && matches!(event, EventV4::ResourceSpawned {resource} if resource.id == *id)));
    assert_eq!(w.members[1].tally["npc"], 1);
    assert_eq!(c.pve_loot_bound("item", 1, 30), 5);
    assert_eq!(c.pve_loot_bound("item", 0, 30), 0);
    assert_eq!(
        pickup(&mut w, 1, id, &c, 30).unwrap_err(),
        Error::Unauthorized
    );
    pickup(&mut w, 0, id, &c, 30).unwrap();
    w.protect(0, 60, &mut vec![]);
    assert_eq!(
        pickup(&mut w, 1, id, &c, 119).unwrap_err(),
        Error::Unauthorized
    );
    assert!(
        w.snapshot(1, &c, Phase::Active, 120)
            .resources
            .iter()
            .any(|r| r.id == *id && r.owner.is_none() && r.quantity == 2)
    );
    pickup(&mut w, 1, id, &c, 120).unwrap();
    assert_eq!(
        (w.members[0].looted["item"], w.members[1].looted["item"]),
        (3, 2)
    );
    assert_eq!(
        pickup(&mut w, 1, id, &c, 120).unwrap_err(),
        Error::InvalidTarget
    );
}

#[test]
fn monster_pool_freezes_blocked_draw_and_waits_for_its_reserved_capacity() {
    let c = catalog(input(), |v| {
        monster_loot_input(v);
        v["pve"]["monster_positions"] = json!([[40, 30], [35, 45]]);
        v["pve"]["world_geometry"] = json!({
            "boundary":[[0,0],[128,0],[128,128],[0,128]], "blockers":[
                {"id":"gate","vertices":[[36,26],[44,26],[44,34],[36,34]],
                 "blocks_movement":true,"blocks_projectiles":false,"blocks_vision":false,
                 "active_window":{"start_ms":30,"end_ms":90}}
            ]
        });
        v["world_loot"]["pools"]["drop"]["guaranteed"][0]["quantity_min"] = json!(2);
        v["world_loot"]["monster_pools"]["npc"]["priority_ms"] = json!(0);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    hit_monster(&mut w, &c, (0, 0), 100, 0);
    hit_monster(&mut w, &c, (1, 1), 100, 0);
    let rng = w.rng;
    for now in [30, 60, 90] {
        w.tick(&c, now);
        let resource = w
            .resources
            .iter()
            .find(|r| r.view.item_id.is_some())
            .unwrap();
        assert_eq!(resource.view.position, [35., 45.]);
        assert_eq!(
            w.resources
                .iter()
                .filter(|r| r.view.item_id.is_some())
                .count(),
            1
        );
    }
    assert_eq!(w.rng, rng);
    let first_id = w
        .resources
        .iter()
        .find(|r| r.view.item_id.is_some())
        .unwrap()
        .view
        .id
        .clone();
    pickup(&mut w, 1, &first_id, &c, 90).unwrap();
    w.tick(&c, 120);
    let resource = w
        .resources
        .iter()
        .find(|r| r.view.item_id.is_some())
        .unwrap();
    assert_eq!(resource.view.position, [40., 30.]);
    assert_ne!(resource.view.id, first_id);
    assert_eq!(resource.view.expires_at_ms, 1120);
    assert_eq!(w.rng, rng);
    w.tick(&c, 1140);
    assert!(w.resources.is_empty());
}

#[test]
fn monster_pool_ties_are_stable_and_terminal_releases_priority() {
    let c = catalog(input(), monster_loot_input)
        .unwrap()
        .default_config();
    let mut w = world(&c);
    hit_monster(&mut w, &c, (0, 0), 50, 0);
    hit_monster(&mut w, &c, (0, 1), 50, 0);
    w.tick(&c, 30);
    let r = w
        .resources
        .iter()
        .find(|r| r.view.item_id.is_some())
        .unwrap();
    let id = r.view.id.clone();
    assert_eq!(r.view.owner, Some(w.members[0].reference.entity.clone()));
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    pickup(&mut w, 1, &id, &c, 30).unwrap();
    assert_eq!(w.members[1].looted["item"], 5);
}

#[test]
fn monster_pool_clears_contribution_at_home_and_credits_summon_to_owner() {
    let c = catalog(input(), |v| {
        monster_loot_input(v);
        summon_input(v, 360);
        v["pve"]["combat"]["aggro_range"] = json!(3);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    hit_monster(&mut w, &c, (0, 0), 80, 0);
    w.npcs[0].returning = true;
    w.tick(&c, 30);
    assert_eq!(w.npcs[0].hp, 100);
    cast_summon(&mut w, &c, 1, 30, "summon");
    w.tick(&c, 60);
    let source = w.summons[0].entity.clone();
    let target = w.npcs[0].entity.clone();
    let result = w.resolve_damage(
        combat::DamageIntent {
            source,
            target,
            raw: 60,
            direct: false,
            ignore_armor: true,
        },
        &c,
        60,
        &mut vec![],
    );
    assert_eq!(result.hp_damage, 60);
    hit_monster(&mut w, &c, (0, 0), 100, 60);
    w.tick(&c, 90);
    let r = w
        .resources
        .iter()
        .find(|r| r.view.item_id.is_some())
        .unwrap();
    assert_eq!(r.view.owner, Some(w.members[1].reference.entity.clone()));
}

#[test]
fn monster_pool_rejects_unknown_bindings_and_double_mint_configuration() {
    for (field, value) in [
        ("pool", json!("unknown")),
        ("priority_ms", json!(1001)),
        ("correction_radius", json!(76)),
    ] {
        assert!(
            catalog(input(), |v| {
                monster_loot_input(v);
                v["world_loot"]["monster_pools"]["npc"][field] = value;
            })
            .err()
            .unwrap()
            .to_string()
            .contains("CONFIG_WORLD_LOOT_MONSTER")
        );
    }
    assert!(
        catalog(input(), |v| {
            monster_loot_input(v);
            v["pve"]["drop_table"]["guaranteed"] =
                json!([{"itemId":"item","quantityMin":1,"quantityMax":1}]);
        })
        .err()
        .unwrap()
        .to_string()
        .contains("CONFIG_WORLD_LOOT_MONSTER")
    );
    assert!(
        catalog(input(), |v| {
            monster_loot_input(v);
            v["world_loot"]["one_shot_capacity"] = json!(0);
        })
        .err()
        .unwrap()
        .to_string()
        .contains("CONFIG_WORLD_LOOT_MONSTER")
    );
}
