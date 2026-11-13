use super::member_item_use::use_item;
use super::member_loot_drop::{acquire, drop, held, pickup, published_drop};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(60);
    v["member_item_use"]["huo-ling"] = json!({"restore_spirit":40,"cooldown_ms":600});
}

#[test]
fn member_item_spirit_requires_version_and_single_positive_effect() {
    assert!(delta_catalog(8, configure).is_ok());
    for definition in [
        json!({"restore_spirit":0}),
        json!({"restore_spirit":1000001}),
        json!({"restore_spirit":null}),
        json!({"restore_spirit":1.5}),
        json!({"heal_hp":40,"restore_spirit":40}),
        json!({"cooldown_ms":600}),
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
            v["mechanism_version"] = json!(59);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["member_item_use"]["huo-ling"] = json!({"restore_spirit":1000000});
            v["member_item_use"]["pill_healing"] = json!({"heal_hp":40});
        })
        .is_ok()
    );
}

#[test]
fn member_item_spirit_transfer_and_retry_settle_remaining_loot() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    h.action(0, drop(h.snapshot(0).own.inventory_revision, 1))
        .unwrap();
    h.pump();
    let resource = published_drop(&h);
    h.engine.world.as_mut().unwrap().members[1].position = resource.position;
    h.action(1, pickup(&h, 1, &resource)).unwrap();
    h.pump();
    let revision = h.snapshot(1).own.inventory_revision;
    assert_eq!(h.action(1, use_item(revision)), Err(Error::InvalidState));
    let peer_spirit = h.snapshot(0).own.spirit;
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
    let own = &h.snapshot(1).own;
    let hp_before = own.hp;
    let deficit = own.max_spirit - own.spirit;
    assert!((1..40).contains(&deficit));
    let request = h.input(1, use_item(revision));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    assert_eq!(h.snapshot(1).own.spirit, h.snapshot(1).own.max_spirit);
    assert_eq!(h.snapshot(1).own.hp, hp_before);
    assert_eq!(h.snapshot(1).own.inventory_revision, revision + 1);
    assert_eq!(h.snapshot(0).own.spirit, peer_spirit);
    assert_eq!((held(&h, 0), held(&h, 1)), (2, 0));
    let facts: Vec<_> = h
        .events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event:
                    Event::ItemUsed {
                        action_id,
                        hp_restored,
                        spirit_restored,
                        ..
                    },
            } => Some((
                recipient.as_deref(),
                action_id.as_str(),
                *hp_restored,
                *spirit_restored,
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        facts,
        [(Some("P01"), request.request_id.as_str(), 0, Some(deficit))]
    );
    let cooldowns = h.snapshot(1).own.item_cooldowns.clone();
    assert_eq!(cooldowns.len(), 1);
    assert!(h.snapshot(0).own.item_cooldowns.is_empty());
    let mut reconnect = h.input(
        1,
        Action::Reconnect {
            previous_connection: request.connection.clone(),
            authenticated_principal: "principal-P01".into(),
        },
    );
    reconnect.connection = Connection {
        id: "item-spirit-reconnect".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(request), Err(Error::StaleConnection));
    h.pump();
    assert_eq!(h.snapshot(1).own.item_cooldowns, cooldowns);
    h.close();
    assert_eq!(h.stores[0].balances["huo-ling"], 2);
    assert!(!h.stores[1].balances.contains_key("huo-ling"));
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}
