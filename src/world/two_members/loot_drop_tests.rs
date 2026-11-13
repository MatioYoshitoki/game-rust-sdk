use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::hp_cost_tests::setup(2);
    c.mechanism_version = 57;
    c.member_loot_drop = serde_json::from_value(json!({"max_active":1})).unwrap();
    c.params.resource_ttl_ms = 60;
    let item = "huo-ling";
    assert!(c.pve.as_ref().unwrap().items.contains_key(item));
    w.members[0].inventory.insert(item.into(), 3);
    w.members[0].looted.insert(item.into(), 3);
    w.members[1].position = w.members[0].position;
    (c, w)
}

fn drop(w: &mut World, c: &Config, item: &str, quantity: u32, revision: u32) -> Result<Events> {
    w.action(
        0,
        &Action::DropItem {
            item_id: item.into(),
            quantity,
            expected_revision: revision,
        },
        "drop",
        c,
        0,
    )
}

#[test]
fn member_loot_drop_capacity_expiry_and_revision_are_atomic() {
    let (c, mut w) = setup();
    let revision = w.members[0].revision;
    let inventory = w.members[0].inventory.clone();
    assert_eq!(
        drop(&mut w, &c, &c.item_id, 1, revision),
        Err(Error::InvalidTarget)
    );
    assert_eq!(
        drop(&mut w, &c, "huo-ling", 4, revision),
        Err(Error::InvalidTarget)
    );
    assert_eq!(
        drop(&mut w, &c, "huo-ling", 1, revision + 1),
        Err(Error::RevisionConflict)
    );
    Arc::make_mut(&mut w.grid.config).palette[0].walkable = false;
    assert_eq!(
        drop(&mut w, &c, "huo-ling", 1, revision),
        Err(Error::InvalidTarget)
    );
    assert_eq!(w.members[0].inventory, inventory);
    assert_eq!(w.members[0].revision, revision);
    assert!(w.resources.is_empty());
    Arc::make_mut(&mut w.grid.config).palette[0].walkable = true;

    let events = drop(&mut w, &c, "huo-ling", 1, revision).unwrap();
    let [(None, EventV4::ResourceSpawned { resource })] = events.as_slice() else {
        panic!("expected one public drop")
    };
    assert_eq!(resource.quantity, 1);
    assert!(resource.owner.is_none());
    assert_eq!(resource.position, w.members[0].position);
    assert_eq!(w.members[0].inventory["huo-ling"], 2);
    assert_eq!(w.members[0].looted["huo-ling"], 2);
    assert_eq!(
        drop(&mut w, &c, "huo-ling", 1, revision + 1),
        Err(Error::Capacity)
    );
    let pickup = Action::Pickup {
        resource_id: resource.id.clone(),
        expected_revision: w.members[1].revision,
    };
    assert_eq!(
        w.action(1, &pickup, "expired", &c, 60),
        Err(Error::InvalidTarget)
    );
    w.tick(&c, 60);
    assert!(w.resources.is_empty());
    assert_eq!(w.members[0].revision, revision + 1);
    assert_eq!(w.members[0].inventory["huo-ling"], 2);
    assert!(!w.members[1].inventory.contains_key("huo-ling"));
}

#[test]
fn member_loot_drop_pickup_checks_collision_range_and_full_inventory() {
    let (mut c, mut w) = setup();
    let donor = w.members[0].revision;
    drop(&mut w, &c, "huo-ling", 3, donor).unwrap();
    assert!(!w.members[0].inventory.contains_key("huo-ling"));
    assert!(!w.members[0].looted.contains_key("huo-ling"));
    let resource = w.resources[0].view.clone();
    let revision = w.members[1].revision;
    let pickup = Action::Pickup {
        resource_id: resource.id,
        expected_revision: revision,
    };
    w.members[1].position[0] += c.params.pickup_range + 1.;
    assert_eq!(w.action(1, &pickup, "range", &c, 0), Err(Error::OutOfRange));
    w.members[1].position = w.members[0].position;
    Arc::make_mut(&mut w.grid.config).palette[0].walkable = false;
    assert_eq!(
        w.action(1, &pickup, "wall", &c, 0),
        Err(Error::InvalidTarget)
    );
    Arc::make_mut(&mut w.grid.config).palette[0].walkable = true;
    let capacity = c.inventory_capacity;
    c.inventory_capacity = 1;
    assert_eq!(
        w.action(1, &pickup, "capacity", &c, 0),
        Err(Error::Capacity)
    );
    assert_eq!(w.members[1].revision, revision);
    assert_eq!(w.resources.len(), 1);
    c.inventory_capacity = capacity;
    w.action(1, &pickup, "ok", &c, 0).unwrap();
    assert_eq!(w.members[1].inventory["huo-ling"], 3);
    assert_eq!(w.members[1].looted["huo-ling"], 3);
    assert!(w.resources.is_empty());
}
