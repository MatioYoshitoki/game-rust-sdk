use super::member_loot_drop::{acquire, pickup};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(67);
    v["pve"]["status_definitions"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id":"item-guard","mechanism":"timed_attribute_modifier","attribute":"damage_reduction",
            "modifier_bps":2500,"duration_ms":600,"duplicate_policy":"refresh","source_scoped":true,
        }));
    v["member_item_use"] = json!({
        "huo-ling":{"heal_hp":10,"cooldown_ms":900,"on_use_self_statuses":["item-guard"]},
        "pill_healing":{"heal_hp":20,"cooldown_ms":900,"on_use_self_statuses":["item-guard"],
            "on_use_self_status_overrides":{"item-guard":{"modifier_bps":3200,"duration_ms":300}}},
    });
    v["pve"]["drop_table"]["guaranteed"]
        .as_array_mut()
        .unwrap()
        .push(json!({"itemId":"pill_healing","quantityMin":2,"quantityMax":2}));
}

#[test]
fn member_item_damage_reduction_validates_version_binding_and_limits() {
    assert!(delta_catalog(8, configure).is_ok());
    for change in [
        |v: &mut Value| v["mechanism_version"] = json!(66),
        |v: &mut Value| v["pve"]["status_definitions"][2]["modifier_bps"] = json!(0),
        |v: &mut Value| v["pve"]["status_definitions"][2]["modifier_bps"] = json!(5001),
        |v: &mut Value| v["pve"]["status_definitions"][2]["duplicate_policy"] = json!("ignore"),
        |v: &mut Value| v["skill"]["on_commit_self_statuses"] = json!(["item-guard"]),
        |v: &mut Value| v["skill"]["on_hit_target_statuses"] = json!(["item-guard"]),
        |v: &mut Value| {
            v["member_item_use"]["pill_healing"]["on_use_self_status_overrides"]["item-guard"]["modifier_bps"] =
                json!(5001)
        },
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                change(v);
            })
            .is_err()
        );
    }
    assert!(delta_catalog(8, |v| {
        configure(v);
        v["member_item_use"]["pill_healing"]["on_use_self_status_overrides"]["item-guard"]["modifier_bps"] = json!(5000);
    }).is_ok());
}

#[test]
fn member_item_damage_reduction_refresh_is_private_and_survives_reconnect() {
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
    let old = h
        .snapshot(0)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == "item-guard")
        .unwrap()
        .clone();
    assert_eq!(old.modifier_bps, 2500);
    let request = h.input(0, use_item("pill_healing", revision + 1));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    let after = h.snapshot(0).own.clone();
    let status = after
        .statuses
        .iter()
        .find(|s| s.definition_id == "item-guard")
        .unwrap();
    assert_eq!(status.instance_id, old.instance_id);
    assert_eq!(status.modifier_bps, 3200);
    assert_eq!(status.expires_at_ms - status.applied_at_ms, 300);
    assert_eq!(after.inventory_revision, revision + 2);
    assert_eq!(h.snapshot(1).own, peer);
    assert_eq!(
        h.action(0, use_item("pill_healing", revision + 2)),
        Err(Error::InvalidState)
    );
    let refreshes: Vec<_> = h
        .events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event: Event::StatusRefreshed { status },
            } if status.definition_id == "item-guard" => Some((recipient.as_deref(), status)),
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
        id: "item-mitigation-reconnect".into(),
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
