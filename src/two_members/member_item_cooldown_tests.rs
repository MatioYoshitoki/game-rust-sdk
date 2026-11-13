use super::member_item_use::use_item;
use super::member_loot_drop::{acquire, drop, held, pickup, published_drop};
use super::*;

fn configure(v: &mut Value) {
    super::member_item_use::configure(v);
    v["mechanism_version"] = json!(59);
    v["member_item_use"]["huo-ling"]["cooldown_ms"] = json!(600);
}

#[test]
fn member_item_cooldown_requires_version_and_fixed_step() {
    assert!(delta_catalog(8, configure).is_ok());
    for duration in [json!(null), json!(0), json!(31), json!("30"), json!(60030)] {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                v["member_item_use"]["huo-ling"]["cooldown_ms"] = duration;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["member_item_use"]["huo-ling"]["cooldown_ms"] = json!(60000);
        })
        .is_ok()
    );
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(58);
        })
        .is_err()
    );
    assert!(
        delta_catalog(8, |v| {
            super::member_item_use::configure(v);
            v["mechanism_version"] = json!(59);
        })
        .is_ok()
    );
}

#[test]
fn member_item_cooldown_preserves_member_deadline_through_transfer_and_reconnect() {
    let mut h = DeltaHarness::new(8, configure);
    acquire(&mut h);
    let use_at = h.engine.now;
    let request = h.input(0, use_item(h.snapshot(0).own.inventory_revision));
    let result = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request.clone()).unwrap(), result);
    h.pump();
    let deadline = h.snapshot(0).own.item_cooldowns[0].clone();
    assert_eq!(deadline.ready_at_ms, use_at + 600);
    assert!(h.snapshot(1).own.item_cooldowns.is_empty());
    assert_eq!(held(&h, 0), 2);
    let revision = h.snapshot(0).own.inventory_revision;
    h.action(0, drop(revision, 1)).unwrap();
    h.pump();
    let dropped = published_drop(&h);
    h.action(0, pickup(&h, 0, &dropped)).unwrap();
    h.pump();
    assert_eq!(
        h.snapshot(0).own.item_cooldowns.as_slice(),
        std::slice::from_ref(&deadline)
    );
    assert_eq!(
        h.action(0, use_item(h.snapshot(0).own.inventory_revision)),
        Err(Error::InvalidState)
    );

    h.action(0, drop(h.snapshot(0).own.inventory_revision, 1))
        .unwrap();
    h.pump();
    let dropped = published_drop(&h);
    h.engine.world.as_mut().unwrap().members[1].position = dropped.position;
    h.action(1, pickup(&h, 1, &dropped)).unwrap();
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
    assert!(h.engine.now < deadline.ready_at_ms);
    h.action(1, use_item(h.snapshot(1).own.inventory_revision))
        .unwrap();
    h.pump();
    assert_eq!(held(&h, 1), 0);
    assert!(h.snapshot(1).own.item_cooldowns[0].ready_at_ms > deadline.ready_at_ms);

    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: request.connection.clone(),
            authenticated_principal: "principal-P00".into(),
        },
    );
    reconnect.connection = Connection {
        id: "item-cooldown-new".into(),
        generation: 2,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    assert_eq!(h.engine.input(request), Err(Error::StaleConnection));
    h.pump();
    assert_eq!(h.snapshot(0).own.item_cooldowns, [deadline]);
    h.hold_settle[1] = true;
    h.action(1, Action::Leave {}).unwrap();
    h.pump();
    assert!(h.snapshot(1).own.item_cooldowns.is_empty());
    h.ticks(25);
    assert_eq!(h.snapshot(0).phase, Phase::Active);
    for (i, settlement) in std::mem::take(&mut h.held) {
        let outcome = h.stores[i].execute(&settlement);
        h.receipt(i, &settlement, outcome).unwrap();
    }
    h.pump();
    assert!(h.snapshot(1).own.item_cooldowns.is_empty());
    h.close();
    assert_eq!(h.stores[0].balances["huo-ling"], 1);
    assert!(h.stores.iter().all(|store| store.settlements == 1));
}

#[test]
fn member_item_cooldown_replica_rejects_invalid_deadlines_and_duplicates() {
    let mut h = DeltaHarness::new(4, configure);
    acquire(&mut h);
    h.action(0, use_item(h.snapshot(0).own.inventory_revision))
        .unwrap();
    h.pump();
    let snap = h.snapshot(0).clone();
    let mut full = h.latest(0);
    full.payload = PublicationPayloadV6::Full {
        snapshot: Box::new(snap.clone()),
        reason: FullReason::Initial,
    };
    let mut replica = ReplicaV6::new(full.key.clone(), full.recipient_id.clone());
    replica.apply(&full).unwrap();
    for kind in 0..3 {
        let mut invalid = full.clone();
        invalid.revision += 1;
        invalid.publication_id = "invalid-cooldown".into();
        let PublicationPayloadV6::Full { snapshot, .. } = &mut invalid.payload else {
            unreachable!()
        };
        match kind {
            0 => snapshot.own.item_cooldowns[0].ready_at_ms = snapshot.logical_ms,
            1 => snapshot.own.item_cooldowns[0].ready_at_ms = snapshot.logical_ms + 60001,
            _ => snapshot
                .own
                .item_cooldowns
                .push(snapshot.own.item_cooldowns[0].clone()),
        }
        assert_eq!(replica.apply(&invalid), Err(DecodeError::InvalidFrame));
        assert_eq!(replica.snapshot(), Some(&snap));
    }
}
