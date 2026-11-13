use super::member_item_use::use_item;
use super::member_loot_drop::{acquire, held};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(65);
    v["member_item_use"]["huo-ling"] =
        json!({"heal_hp":40,"cooldown_ms":600,"on_use_self_statuses":["item-fury"]});
    v["pve"]["status_definitions"].as_array_mut().unwrap().extend([
        json!({"id":"item-fury","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":3000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"wound","mechanism":"periodic_damage","damage_bps":1000,"tick_interval_ms":60,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"renewal","mechanism":"periodic_heal","heal_bps":1000,"tick_interval_ms":60,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
        json!({"id":"ward","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    ]);
}

#[test]
fn member_item_self_status_requires_valid_version_and_self_bindings() {
    assert!(delta_catalog(8, configure).is_ok());
    for ids in [json!(["item-fury", "guard"]), json!(["ward", "renewal"])] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"]["huo-ling"]["on_use_self_statuses"] = ids;
            })
            .is_ok()
        );
    }
    for ids in [
        json!([]),
        json!(null),
        json!("guard"),
        json!(["missing"]),
        json!(["wound"]),
        json!(["guard", "guard"]),
        json!(["item-fury", "guard", "resolve"]),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"]["huo-ling"]["on_use_self_statuses"] = ids;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(64);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["member_item_use"]["huo-ling"] = json!({"on_use_self_statuses":["guard"]});
        })
        .is_err()
    );
}

#[test]
fn member_item_self_status_is_private_preserves_deadlines_and_settles_once() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    let before = h.snapshot(0).own.clone();
    let peer = h.snapshot(1).own.clone();
    let request = h.input(0, use_item(before.inventory_revision));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    let after = h.snapshot(0).own.clone();
    assert_eq!(after.hp, before.hp + 40);
    assert_eq!(after.inventory_revision, before.inventory_revision + 1);
    assert_eq!(held(&h, 0), 2);
    assert_eq!(after.effective_atk, before.effective_atk * 13 / 10);
    assert_eq!(h.snapshot(1).own, peer);
    let status = after
        .statuses
        .iter()
        .find(|s| s.definition_id == "item-fury")
        .unwrap()
        .clone();
    let events: Vec<_> = h
        .events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event: Event::StatusApplied { status },
            } if status.definition_id == "item-fury" => Some((recipient.as_deref(), "applied")),
            EventV4::Private {
                event: Event::ItemUsed { .. },
            } => Some((recipient.as_deref(), "used")),
            _ => None,
        })
        .collect();
    assert_eq!(events, [(Some("P00"), "applied"), (Some("P00"), "used")]);
    assert_eq!(
        h.action(0, use_item(after.inventory_revision)),
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
        id: "item-status-reconnect".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.pump();
    assert!(h.snapshot(0).own.statuses.contains(&status));
    assert_eq!(h.snapshot(0).own.item_cooldowns, after.item_cooldowns);
    h.ticks(10);
    assert!(
        h.snapshot(0)
            .own
            .statuses
            .iter()
            .all(|s| s.definition_id != "item-fury")
    );
    assert_eq!(h.snapshot(0).own.effective_atk, before.effective_atk);
    h.close();
    assert_eq!(h.stores[0].balances["huo-ling"], 2);
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}
