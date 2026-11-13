use super::member_item_use::use_item;
use super::member_loot_drop::{acquire, held};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(62);
    v["member_item_use"]["huo-ling"] = json!({"heal_hp":40,"restore_spirit":40,"cooldown_ms":600});
}

#[test]
fn member_item_dual_recovery_requires_valid_effects_and_mechanism_62() {
    assert!(delta_catalog(8, configure).is_ok());
    for mechanism in [60, 61] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["mechanism_version"] = json!(mechanism);
            })
            .is_err()
        );
    }
    for definition in [
        json!({"heal_hp":0,"restore_spirit":40}),
        json!({"heal_hp":40,"restore_spirit":1_000_001}),
        json!({"heal_hp":40,"restore_spirit":null}),
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
            v["member_item_use"]["huo-ling"] = json!({"heal_hp":40});
            v["member_item_use"]["pill_healing"] = json!({"restore_spirit":40});
        })
        .is_ok()
    );
}

#[test]
fn member_item_dual_recovery_replays_once_and_settles_remaining_loot() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    let before = h.snapshot(0).own.clone();
    let peer_before = h.snapshot(1).own.clone();
    let expected_hp = (before.max_hp - before.hp).min(40);
    let expected_spirit = (before.max_spirit - before.spirit).min(40);
    assert!(expected_hp > 0 && expected_spirit > 0);
    let request = h.input(0, use_item(before.inventory_revision));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    let after = h.snapshot(0).own.clone();
    assert_eq!(after.inventory_revision, before.inventory_revision + 1);
    assert_eq!(after.hp, before.hp + expected_hp);
    assert_eq!(after.spirit, before.spirit + expected_spirit);
    assert_eq!(held(&h, 0), 2);
    assert_eq!(h.snapshot(1).own, peer_before);
    let facts: Vec<_> = h
        .events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event:
                    Event::ItemUsed {
                        hp_restored,
                        spirit_restored,
                        ..
                    },
            } => Some((recipient.as_deref(), *hp_restored, *spirit_restored)),
            _ => None,
        })
        .collect();
    assert_eq!(facts, [(Some("P00"), expected_hp, Some(expected_spirit))]);
    assert_eq!(after.item_cooldowns.len(), 1);

    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: request.connection,
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "dual-recovery-reconnect".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.pump();
    assert_eq!(h.snapshot(0).own.item_cooldowns, after.item_cooldowns);
    h.close();
    assert_eq!(h.stores[0].balances["huo-ling"], 2);
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}
