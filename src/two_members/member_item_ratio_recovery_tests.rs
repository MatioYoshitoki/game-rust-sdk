use super::member_item_use::use_item;
use super::member_loot_drop::{acquire, held};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(63);
    v["member_item_use"]["huo-ling"] =
        json!({"heal_max_hp_bps":2000,"restore_max_spirit_bps":2000,"cooldown_ms":600});
}

#[test]
fn member_item_ratio_recovery_requires_valid_version_and_exclusive_amounts() {
    assert!(delta_catalog(8, configure).is_ok());
    for version in [58, 61, 62] {
        for definition in [
            json!({"heal_max_hp_bps":2000}),
            json!({"restore_max_spirit_bps":2000}),
        ] {
            assert!(
                delta_catalog(8, |v| {
                    configure(v);
                    v["mechanism_version"] = json!(version);
                    v["member_item_use"]["huo-ling"] = definition;
                })
                .is_err()
            );
        }
    }
    for definition in [
        json!({"heal_max_hp_bps":1}),
        json!({"restore_max_spirit_bps":10_000}),
        json!({"heal_max_hp_bps":2000,"restore_spirit":40}),
        json!({"heal_hp":40,"restore_max_spirit_bps":2000}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"]["huo-ling"] = definition;
            })
            .is_ok()
        );
    }
    for definition in [
        json!({"heal_max_hp_bps":0}),
        json!({"restore_max_spirit_bps":10_001}),
        json!({"heal_max_hp_bps":1.5}),
        json!({"heal_hp":40,"restore_max_spirit_bps":null}),
        json!({"heal_hp":40,"heal_max_hp_bps":2000}),
        json!({"restore_spirit":40,"restore_max_spirit_bps":2000}),
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
}

#[test]
fn member_item_ratio_recovery_is_private_idempotent_and_settles_remaining_loot() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    let before = h.snapshot(0).own.clone();
    let peer = h.snapshot(1).own.clone();
    let expected_hp = (before.max_hp - before.hp).min(before.max_hp / 5);
    let expected_spirit = (before.max_spirit - before.spirit).min(before.max_spirit / 5);
    assert!(expected_hp > 0 && expected_spirit > 0);
    let request = h.input(0, use_item(before.inventory_revision));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    let after = h.snapshot(0).own.clone();
    assert_eq!(after.inventory_revision, before.inventory_revision + 1);
    assert_eq!(
        (after.hp, after.spirit),
        (before.hp + expected_hp, before.spirit + expected_spirit)
    );
    assert_eq!(held(&h, 0), 2);
    assert_eq!(h.snapshot(1).own, peer);
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
        id: "ratio-recovery-reconnect".into(),
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
