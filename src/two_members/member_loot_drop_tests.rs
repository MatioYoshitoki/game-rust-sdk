use super::*;

pub(super) fn configure(v: &mut Value) {
    v["mechanism_version"] = json!(57);
    v["members"]["area_capacity"] = json!(2);
    v["member_loot_drop"] = json!({"max_active":8});
    v["pve"]["stats"]["hp"] = json!(35);
    v["pve"]["stats"]["def"] = json!(0);
    v["pve"]["drop_table"]["guaranteed"] =
        json!([{"itemId":"huo-ling","quantityMin":3,"quantityMax":3}]);
    v["params"]["death"]["slot_ratio"] = json!(0);
    v["params"]["drop_min_radius"] = json!(0);
    v["params"]["drop_max_radius"] = json!(0);
}

pub(super) fn drop(revision: u32, quantity: u32) -> Action {
    Action::DropItem {
        item_id: "huo-ling".into(),
        quantity,
        expected_revision: revision,
    }
}

pub(super) fn pickup(h: &DeltaHarness, i: usize, resource: &ResourceViewV4) -> Action {
    Action::Pickup {
        resource_id: resource.id.clone(),
        expected_revision: h.snapshot(i).own.inventory_revision,
    }
}

pub(super) fn held(h: &DeltaHarness, i: usize) -> u32 {
    h.snapshot(i)
        .own
        .inventory
        .iter()
        .find(|r| r.item_id == "huo-ling")
        .map_or(0, |r| r.quantity)
}

pub(super) fn acquire(h: &mut DeltaHarness) {
    h.active();
    let skill_id = h.engine.config.skill.id.clone();
    h.action(
        0,
        Action::Cast {
            skill_id,
            direction: [1., 0.],
        },
    )
    .unwrap();
    h.ticks(25);
    let resource = h
        .snapshot(0)
        .resources
        .iter()
        .find(|r| r.item_id.as_deref() == Some("huo-ling"))
        .unwrap()
        .clone();
    h.engine.world.as_mut().unwrap().members[0].position = resource.position;
    h.action(0, pickup(h, 0, &resource)).unwrap();
    h.pump();
    assert_eq!(held(h, 0), 3);
}

pub(super) fn published_drop(h: &DeltaHarness) -> ResourceViewV4 {
    h.snapshot(0)
        .resources
        .iter()
        .find(|r| r.id.starts_with("player-drop-"))
        .unwrap()
        .clone()
}

#[test]
fn member_loot_drop_configuration_and_action_are_strict() {
    assert!(delta_catalog(8, configure).is_ok());
    for definition in [
        json!(null),
        json!({}),
        json!({"max_active":0}),
        json!({"max_active":51}),
        json!({"max_active":1,"ttl":5}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_loot_drop"] = definition;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(56);
        })
        .is_err()
    );
    for value in [json!(-1), json!(1.5), json!("1"), json!(4294967296u64)] {
        assert!(
            serde_json::from_value::<Action>(json!({
                "kind":"drop_item","item_id":"huo-ling","quantity":value,"expected_revision":1
            }))
            .is_err()
        );
    }
    assert!(serde_json::from_value::<Action>(json!({
        "kind":"drop_item","item_id":"huo-ling","quantity":1,"expected_revision":1,"position":[0,0]
    })).is_err());
    for version in [56, 57] {
        let mut h = DeltaHarness::new(2, |v| {
            configure(v);
            v["mechanism_version"] = json!(version);
            v.as_object_mut().unwrap().remove("member_loot_drop");
        });
        h.active();
        assert_eq!(h.action(0, drop(1, 1)), Err(Error::Unsupported));
    }
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    assert_eq!(h.action(0, drop(1, 0)), Err(Error::InvalidMessage));
}

#[test]
fn member_loot_drop_public_duplicate_reconnect_and_competition() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    let revision = h.snapshot(0).own.inventory_revision;
    let intent = h.input(0, drop(revision, 2));
    let accepted = h.engine.input(intent.clone()).unwrap();
    assert_eq!(h.engine.input(intent.clone()).unwrap(), accepted);
    h.pump();
    let resource = published_drop(&h);
    assert_eq!(resource.quantity, 2);
    assert_eq!(h.snapshot(0).own.inventory_revision, revision + 1);
    for i in 0..8 {
        assert_eq!(
            h.snapshot(i)
                .resources
                .iter()
                .filter(|r| r.id == resource.id)
                .count(),
            1
        );
        assert_eq!(held(&h, i), u32::from(i == 0));
    }
    assert!(
        h.frames
            .iter()
            .any(|f| matches!(f.payload, PublicationPayloadV6::Delta { .. }))
    );

    let previous = intent.connection.clone();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous,
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "loot-drop-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(intent), Err(Error::StaleConnection));
    h.pump();
    assert_eq!(published_drop(&h), resource);
    assert_eq!(held(&h, 0), 1);

    for i in 1..=2 {
        h.engine.world.as_mut().unwrap().members[i].position = resource.position;
    }
    h.action(1, pickup(&h, 1, &resource)).unwrap();
    assert_eq!(
        h.action(2, pickup(&h, 2, &resource)),
        Err(Error::InvalidTarget)
    );
    h.pump();
    assert_eq!(held(&h, 1), 2);
    assert_eq!(held(&h, 2), 0);
    assert!(h.snapshot(2).resources.iter().all(|r| r.id != resource.id));
    h.close();
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}

#[test]
fn member_loot_drop_financial_recovery_preserves_net_transfer() {
    let mut h = DeltaHarness::new(4, configure);
    acquire(&mut h);
    h.action(0, drop(h.snapshot(0).own.inventory_revision, 3))
        .unwrap();
    h.pump();
    let resource = published_drop(&h);
    h.engine.world.as_mut().unwrap().members[1].position = resource.position;
    h.action(1, pickup(&h, 1, &resource)).unwrap();
    h.pump();
    h.hold_settle.fill(true);
    h.close();
    let checkpoint = h.checkpoint.clone().unwrap();
    let mut request = h.engine.create.clone();
    request.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint,
    });
    h.engine = TwoMembers::create(
        h.engine.config.clone(),
        delta_catalog(4, configure).unwrap(),
        request,
    )
    .unwrap();
    assert!(h.engine.world.is_none());
    h.hold_settle.fill(false);
    h.pump();
    assert_eq!(h.engine.pending(), 0);
    assert_eq!(
        h.stores[0].balances.get("huo-ling").copied().unwrap_or(0),
        0
    );
    assert_eq!(h.stores[1].balances["huo-ling"], 3);
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}
