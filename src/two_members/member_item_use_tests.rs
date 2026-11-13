use super::member_loot_drop::{acquire, drop, held, pickup, published_drop};
use super::*;

pub(super) fn configure(v: &mut Value) {
    super::member_loot_drop::configure(v);
    v["mechanism_version"] = json!(58);
    v["member_item_use"] = json!({"huo-ling":{"heal_hp":40}});
    v["skill"]["hp_cost"] = json!({"mode":"fixed","value":60,"min_hp_after_cost":1});
    v["pve"]["combat"]["aggro_range"] = json!(0.1);
    v["params"]["regen"]["near_ratio"] = json!(0);
    v["params"]["regen"]["far_ratio"] = json!(0);
}

pub(super) fn use_item(revision: u32) -> Action {
    Action::UseItem {
        item_id: "huo-ling".into(),
        expected_revision: revision,
    }
}

#[test]
fn member_item_use_config_and_input_are_versioned_and_strict() {
    assert!(delta_catalog(8, configure).is_ok());
    for definition in [
        json!(null),
        json!({}),
        json!({"absent":{"heal_hp":1}}),
        json!({"huo-ling":{"heal_hp":0}}),
        json!({"huo-ling":{"heal_hp":1.5}}),
        json!({"huo-ling":{"heal_hp":1000001}}),
        json!({"huo-ling":{"heal_hp":40,"quantity":1}}),
    ] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"] = definition;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(57);
        })
        .is_err()
    );
    for value in [
        json!({"kind":"use_item","item_id":"huo-ling","expected_revision":1,"quantity":2}),
        json!({"kind":"use_item","item_id":"huo-ling","expected_revision":1,"target":"P01"}),
        json!({"kind":"use_item","item_id":"huo-ling","expected_revision":1,"heal_hp":99}),
    ] {
        assert!(serde_json::from_value::<Action>(value).is_err());
    }
    for version in [57, 58] {
        let mut h = DeltaHarness::new(2, |v| {
            configure(v);
            v["mechanism_version"] = json!(version);
            v.as_object_mut().unwrap().remove("member_item_use");
        });
        h.active();
        assert_eq!(h.action(0, use_item(1)), Err(Error::Unsupported));
        h.close();
    }
}

#[test]
fn member_item_use_public_transfer_duplicate_and_reconnect() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    h.action(0, drop(h.snapshot(0).own.inventory_revision, 1))
        .unwrap();
    h.pump();
    let resource = published_drop(&h);
    h.engine.world.as_mut().unwrap().members[1].position = resource.position;
    h.action(1, pickup(&h, 1, &resource)).unwrap();
    h.pump();
    let rev = h.snapshot(1).own.inventory_revision;
    assert_eq!(h.action(1, use_item(rev)), Err(Error::InvalidState));
    let peer_hp = h.snapshot(0).own.hp;
    let max = h.snapshot(1).own.max_hp;
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
    assert_eq!(h.snapshot(1).own.hp, max - 60);
    let request = h.input(1, use_item(rev));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    assert_eq!(h.snapshot(1).own.hp, max - 20);
    assert_eq!(h.snapshot(1).own.inventory_revision, rev + 1);
    assert_eq!((held(&h, 0), held(&h, 1)), (2, 0));
    assert_eq!(h.snapshot(0).own.hp, peer_hp);
    let uses = h
        .events
        .iter()
        .filter_map(|(pid, event)| match event {
            EventV4::Private {
                event:
                    Event::ItemUsed {
                        action_id,
                        item_id,
                        hp_restored,
                        spirit_restored: None,
                    },
            } => Some((
                pid.as_deref(),
                action_id.as_str(),
                item_id.as_str(),
                *hp_restored,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        uses,
        vec![(Some("P01"), request.request_id.as_str(), "huo-ling", 40)]
    );
    assert!(matches!(
        h.latest(1).payload,
        PublicationPayloadV6::Delta { .. }
    ));

    let mut reconnect = h.input(
        1,
        Action::Reconnect {
            previous_connection: request.connection.clone(),
            authenticated_principal: "principal-P01".into(),
        },
    );
    reconnect.connection = Connection {
        id: "item-use-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(request), Err(Error::StaleConnection));
    h.pump();
    assert_eq!(h.snapshot(1).own.hp, max - 20);
    assert_eq!(held(&h, 1), 0);
    h.close();
    assert!(h.stores.iter().all(|store| store.settlements == 1));
    assert_eq!(h.stores[0].balances["huo-ling"], 2);
    assert!(!h.stores[1].balances.contains_key("huo-ling"));
}

#[test]
fn member_item_use_net_settlement_survives_financial_recovery() {
    let mut h = DeltaHarness::new(4, configure);
    acquire(&mut h);
    let rev = h.snapshot(0).own.inventory_revision;
    h.action(0, use_item(rev)).unwrap();
    h.pump();
    assert_eq!(held(&h, 0), 2);
    assert_eq!(h.action(0, drop(rev + 1, 3)), Err(Error::InvalidTarget));
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
    assert_eq!(h.stores[0].balances["huo-ling"], 2);
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}
