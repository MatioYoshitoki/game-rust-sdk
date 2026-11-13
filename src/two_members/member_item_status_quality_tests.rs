use super::member_loot_drop::{acquire, pickup};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(66);
    v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({
        "id":"quality-fury","mechanism":"timed_attribute_modifier","attribute":"atk",
        "modifier_bps":3000,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true,
    }));
    v["member_item_use"] = json!({
        "huo-ling":{"heal_hp":10,"cooldown_ms":600,"on_use_self_statuses":["quality-fury"],
            "on_use_self_status_overrides":{"quality-fury":{"modifier_bps":1500,"duration_ms":300}}},
        "pill_healing":{"heal_hp":20,"cooldown_ms":600,"on_use_self_statuses":["quality-fury","guard"],
            "on_use_self_status_overrides":{"quality-fury":{"modifier_bps":3900,"duration_ms":900}}},
    });
    v["pve"]["drop_table"]["guaranteed"]
        .as_array_mut()
        .unwrap()
        .push(json!({"itemId":"pill_healing","quantityMin":2,"quantityMax":2}));
}

#[test]
fn member_item_status_quality_validates_overrides_against_bound_refreshable_attributes() {
    assert!(delta_catalog(8, configure).is_ok());
    for value in [
        json!({}),
        json!(null),
        json!({"missing":{"modifier_bps":1500,"duration_ms":300}}),
        json!({"quality-fury":{"duration_ms":300}}),
        json!({"quality-fury":{"modifier_bps":1500,"duration_ms":29}}),
        json!({"quality-fury":{"modifier_bps":1500,"duration_ms":3_600_001}}),
        json!({"quality-fury":{"modifier_bps":100_001,"duration_ms":300}}),
        json!({"quality-fury":{"modifier_bps":-10_001,"duration_ms":300}}),
        json!({"quality-fury":{"modifier_bps":1500,"duration_ms":300,"attribute":"def"}}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"]["huo-ling"]["on_use_self_status_overrides"] = value;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(65);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["member_item_use"]["huo-ling"]
                .as_object_mut()
                .unwrap()
                .remove("on_use_self_statuses");
        })
        .is_err()
    );
    for (id, definition) in [
        ("resolve", None),
        (
            "renewal",
            Some(
                json!({"id":"renewal","mechanism":"periodic_heal","heal_bps":1000,
            "tick_interval_ms":60,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
            ),
        ),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                if let Some(definition) = definition {
                    v["pve"]["status_definitions"]
                        .as_array_mut()
                        .unwrap()
                        .push(definition);
                }
                v["member_item_use"]["huo-ling"] = json!({"heal_hp":10,"on_use_self_statuses":[id],
                "on_use_self_status_overrides":{id:{"modifier_bps":1500,"duration_ms":300}}});
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["member_item_use"]["pill_healing"]["on_use_self_status_overrides"]["guard"] =
                json!({"modifier_bps":-10_000,"duration_ms":30});
        })
        .is_ok()
    );
}

#[test]
fn member_item_status_quality_reconnect_preserves_latest_quality_and_net_settlement() {
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
    let peer = h.snapshot(1).own.clone();
    let use_item = |item: &str, revision| Action::UseItem {
        item_id: item.into(),
        expected_revision: revision,
    };
    let revision = h.snapshot(0).own.inventory_revision;
    h.action(0, use_item("huo-ling", revision)).unwrap();
    h.pump();
    let low = h
        .snapshot(0)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == "quality-fury")
        .unwrap()
        .clone();
    let request = h.input(0, use_item("pill_healing", revision + 1));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    let after = h.snapshot(0).own.clone();
    let status = after
        .statuses
        .iter()
        .find(|s| s.definition_id == "quality-fury")
        .unwrap();
    let guard = after
        .statuses
        .iter()
        .find(|s| s.definition_id == "guard")
        .unwrap();
    assert_eq!(guard.modifier_bps, 5000);
    assert_eq!(guard.expires_at_ms - guard.applied_at_ms, 3000);
    assert_eq!(status.instance_id, low.instance_id);
    assert_eq!(status.modifier_bps, 3900);
    assert_eq!(status.expires_at_ms - status.applied_at_ms, 900);
    assert_eq!(after.inventory_revision, revision + 2);
    assert_eq!(h.snapshot(1).own, peer);
    let refreshes: Vec<_> = h
        .events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event: Event::StatusRefreshed { status },
            } if status.definition_id == "quality-fury" => Some((recipient.as_deref(), status)),
            _ => None,
        })
        .collect();
    assert_eq!(refreshes, [(Some("P00"), status)]);

    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: request.connection,
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "item-status-quality-reconnect".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.pump();
    assert!(h.snapshot(0).own.statuses.contains(status));
    assert_eq!(h.snapshot(0).own.item_cooldowns, after.item_cooldowns);
    h.close();
    assert_eq!(h.stores[0].balances["huo-ling"], 2);
    assert_eq!(h.stores[0].balances["pill_healing"], 1);
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}
