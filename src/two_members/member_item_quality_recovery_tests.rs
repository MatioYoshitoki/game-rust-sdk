use super::member_loot_drop::{acquire, pickup};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(64);
    v["member_item_use"] = json!({
        "huo-ling":{"heal_hp":20,"restore_spirit":5,"quality_multiplier_bps":7000,"cooldown_ms":600,"cooldown_group":"recovery"},
        "pill_healing":{"heal_hp":20,"restore_spirit":5,"quality_multiplier_bps":15000,"cooldown_ms":600,"cooldown_group":"recovery"},
    });
    v["pve"]["drop_table"]["guaranteed"]
        .as_array_mut()
        .unwrap()
        .push(json!({"itemId":"pill_healing","quantityMin":2,"quantityMax":2}));
}

#[test]
fn member_item_quality_recovery_requires_valid_version_and_integer_multiplier() {
    assert!(delta_catalog(8, configure).is_ok());
    for multiplier in [1, 10_000, 40_000] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"]["huo-ling"]["quality_multiplier_bps"] = json!(multiplier);
            })
            .is_ok()
        );
    }
    for multiplier in [
        json!(0),
        json!(40_001),
        json!(-1),
        json!(1.5),
        json!(null),
        json!("15000"),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"]["huo-ling"]["quality_multiplier_bps"] = multiplier;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["member_item_use"]["huo-ling"] = json!({"quality_multiplier_bps":15000});
        })
        .is_err()
    );
    for mechanism in [58, 63] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["mechanism_version"] = json!(mechanism);
                v["member_item_use"] =
                    json!({"huo-ling":{"heal_hp":20,"quality_multiplier_bps":15000}});
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["member_item_use"]["huo-ling"]
                .as_object_mut()
                .unwrap()
                .remove("quality_multiplier_bps");
        })
        .is_ok()
    );
}

#[test]
fn member_item_quality_recovery_consumes_only_selected_variant_and_settles_by_id() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    let high = h
        .snapshot(0)
        .resources
        .iter()
        .find(|r| r.item_id.as_deref() == Some("pill_healing"))
        .unwrap()
        .clone();
    h.action(0, pickup(&h, 0, &high)).unwrap();
    h.pump();
    let before = h.snapshot(0).own.clone();
    let peer = h.snapshot(1).own.clone();
    let input = |item: &str, expected_revision| Action::UseItem {
        item_id: item.into(),
        expected_revision,
    };
    let request = h.input(0, input("pill_healing", before.inventory_revision));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    let after = h.snapshot(0).own.clone();
    assert_eq!(after.inventory_revision, before.inventory_revision + 1);
    assert_eq!(after.hp, before.hp + 30);
    let restored_spirit = (before.max_spirit - before.spirit).min(7);
    assert_eq!(after.spirit, before.spirit + restored_spirit);
    let quantity = |id: &str| {
        after
            .inventory
            .iter()
            .find(|entry| entry.item_id == id)
            .unwrap()
            .quantity
    };
    assert_eq!((quantity("huo-ling"), quantity("pill_healing")), (3, 1));
    assert_eq!(h.snapshot(1).own, peer);
    let facts: Vec<_> = h
        .events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event:
                    Event::ItemUsed {
                        item_id,
                        hp_restored,
                        spirit_restored,
                        ..
                    },
            } => Some((
                recipient.as_deref(),
                item_id.as_str(),
                *hp_restored,
                *spirit_restored,
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        facts,
        [(Some("P00"), "pill_healing", 30, Some(restored_spirit))]
    );
    assert_eq!(after.item_cooldowns.len(), 2);
    assert_eq!(
        after.item_cooldowns[0].ready_at_ms,
        after.item_cooldowns[1].ready_at_ms
    );
    assert_eq!(
        h.action(0, input("huo-ling", after.inventory_revision)),
        Err(Error::InvalidState)
    );

    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: request.connection,
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "quality-recovery-reconnect".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.item_cooldowns, after.item_cooldowns);
    h.close();
    assert_eq!(h.stores[0].balances["huo-ling"], 3);
    assert_eq!(h.stores[0].balances["pill_healing"], 1);
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}
