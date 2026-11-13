use super::member_loot_drop::{acquire, pickup, published_drop};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(61);
    v["member_item_use"]["huo-ling"] =
        json!({"heal_hp":40,"cooldown_ms":600,"cooldown_group":"recovery"});
    v["member_item_use"]["pill_ningqi"] =
        json!({"restore_spirit":40,"cooldown_ms":600,"cooldown_group":"recovery"});
    v["pve"]["drop_table"]["guaranteed"]
        .as_array_mut()
        .unwrap()
        .push(json!({"itemId":"pill_ningqi","quantityMin":2,"quantityMax":2}));
}

#[test]
fn member_item_cooldown_group_validates_version_id_and_duration() {
    assert!(delta_catalog(8, configure).is_ok());
    for definition in [
        json!({"heal_hp":40,"cooldown_group":"recovery"}),
        json!({"heal_hp":40,"cooldown_ms":630,"cooldown_group":"recovery"}),
        json!({"heal_hp":40,"cooldown_ms":600,"cooldown_group":""}),
        json!({"heal_hp":40,"cooldown_ms":600,"cooldown_group":"a\nb"}),
        json!({"heal_hp":40,"cooldown_ms":600,"cooldown_group":"火".repeat(22)}),
        json!({"heal_hp":40,"cooldown_ms":600,"cooldown_group":null}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"]["huo-ling"] = definition;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(60);
        })
        .is_err()
    );
}

#[test]
fn member_item_cooldown_group_transfer_and_reconnect_keep_owner_deadline() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    let spirit = h
        .snapshot(0)
        .resources
        .iter()
        .find(|r| r.item_id.as_deref() == Some("pill_ningqi"))
        .unwrap()
        .clone();
    h.action(0, pickup(&h, 0, &spirit)).unwrap();
    h.pump();
    let use_item = |item_id: &str, expected_revision| Action::UseItem {
        item_id: item_id.into(),
        expected_revision,
    };
    let initial = h.input(
        0,
        use_item("huo-ling", h.snapshot(0).own.inventory_revision),
    );
    let result = h.engine.input(initial.clone()).unwrap();
    assert_eq!(h.engine.input(initial.clone()).unwrap(), result);
    h.pump();
    let cooldowns = h.snapshot(0).own.item_cooldowns.clone();
    assert_eq!(cooldowns.len(), 2);
    let revision = h.snapshot(0).own.inventory_revision;
    h.action(
        0,
        Action::DropItem {
            item_id: "pill_ningqi".into(),
            quantity: 1,
            expected_revision: revision,
        },
    )
    .unwrap();
    h.pump();
    let resource = published_drop(&h);
    h.engine.world.as_mut().unwrap().members[1].position = resource.position;
    h.action(1, pickup(&h, 1, &resource)).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.item_cooldowns, cooldowns);
    assert!(h.snapshot(1).own.item_cooldowns.is_empty());
    let skill_id = h.engine.config.skill.id.clone();
    h.action(
        1,
        Action::Cast {
            skill_id,
            direction: [0., -1.],
        },
    )
    .unwrap();
    h.ticks(5);
    assert!(h.engine.now < cooldowns[0].ready_at_ms);
    h.action(
        1,
        use_item("pill_ningqi", h.snapshot(1).own.inventory_revision),
    )
    .unwrap();
    h.pump();
    assert_eq!(h.snapshot(1).own.item_cooldowns.len(), 2);
    assert_eq!(h.snapshot(0).own.item_cooldowns, cooldowns);

    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: initial.connection.clone(),
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "item-group-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.item_cooldowns, cooldowns);
    h.action(1, Action::Leave {}).unwrap();
    h.pump();
    assert!(h.snapshot(1).own.item_cooldowns.is_empty());
    h.close();
    assert_eq!(h.stores[0].balances["huo-ling"], 2);
    assert_eq!(h.stores[0].balances["pill_ningqi"], 1);
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}
