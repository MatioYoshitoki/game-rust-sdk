use super::*;

fn relic_input(v: &mut Value) {
    loot_input(v);
    v["inventory_capacity"] = json!(4);
    v["params"]["death"] = json!({"slot_ratio":1,"quantity_ratio":1,"minimum_quantity":1});
    v["world_loot"]["relics"] =
        json!({"capacity":1,"interaction_range":24,"excluded_items":["excluded"]});
    v["world_loot"]["instances"] = json!({"unique-item":{"quality":8,"relic_eligible":true}});
    v["pve"]["items"]["unique-item"] = json!({"name":"instance","stackLimit":1});
    v["pve"]["items"]["excluded"] = json!({"name":"excluded","stackLimit":4});
    v["pve"]["items"]["filler"] = json!({"name":"filler","stackLimit":1});
    v["world_loot"]["pools"]["source"] = pool("item", 3);
    let rows = v["world_loot"]["pools"]["source"]["guaranteed"]
        .as_array_mut()
        .unwrap();
    for item in ["unique-item", "unique-item", "excluded"] {
        rows.push(json!({"item_id":item,"quantity_min":1,"quantity_max":1}));
    }
    v["world_loot"]["nodes"] = json!([
        {"id":"source","pool":"source","points":[[20,30]],"replenish_ms":600,"ttl_ms":600}
    ]);
}

fn relic_claim(id: &str, revision: u32, inventory_revision: u32, entry_id: Option<u32>) -> Action {
    Action::ClaimRelic {
        relic_id: id.into(),
        expected_revision: revision,
        expected_inventory_revision: inventory_revision,
        entry_id,
    }
}

#[test]
fn instance_identity_and_relic_partial_transfer_survive_settlement() {
    let c = catalog(input(), relic_input).unwrap().default_config();
    let mut w = world(&c);
    w.bind_instance_scope("a".repeat(64));
    w.tick(&c, 30);
    let drops = w.snapshot(0, &c, Phase::Active, 30).resources;
    let expected = drops
        .iter()
        .filter_map(|r| r.instance.clone())
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 2);
    assert_ne!(expected[0].instance_id, expected[1].instance_id);
    for drop in drops {
        pickup(&mut w, 0, &drop.id, &c, 30).unwrap();
    }
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own.instances, expected);
    assert!(
        w.snapshot(1, &c, Phase::Active, 30)
            .own
            .instances
            .is_empty()
    );

    assert_eq!(w.members[1].credit_stack("filler", 2, false, &c), Some(2));
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    let Business::CommitSettlement {
        looted,
        kept,
        dropped,
        items,
        instances,
        ..
    } = w.settlement(0, &c)
    else {
        panic!("missing death settlement")
    };
    assert_eq!((looted, kept, dropped), (0, 1, 0));
    assert_eq!((items[0].item_id.as_str(), items[0].kept), ("excluded", 1));
    assert!(instances.is_empty());

    let relic = w
        .snapshot(1, &c, Phase::Active, 30)
        .relics
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        (
            relic.remaining_stacks,
            relic.highest_quality,
            relic.revision
        ),
        (3, Some(8), 1)
    );
    let open = Action::OpenRelic {
        relic_id: relic.id.clone(),
    };
    let first = relic_claim(&relic.id, 1, w.members[1].revision, None);
    assert_eq!(
        w.action(1, &first, "claim", &c, 30).unwrap_err(),
        Error::Unauthorized
    );
    w.members[1].position = [100., 100.];
    assert_eq!(
        w.action(1, &open, "open", &c, 30).unwrap_err(),
        Error::OutOfRange
    );
    w.members[1].position = [20., 40.];
    assert!(matches!(&w.action(1, &open, "open", &c, 30).unwrap()[..],
        [(Some(1), EventV4::Private { event: Event::RelicOpened { contents, .. } })] if contents.len() == 3));
    assert!(matches!(&w.action(1, &first, "claim", &c, 30).unwrap()[..],
        [(Some(1), EventV4::Private { event: Event::RelicClaimed { items, revision: 2, .. } })] if items.len() == 2));
    assert_eq!(w.members[1].looted["item"], 3);
    assert_eq!(w.members[1].instances.len(), 1);
    assert_eq!(
        w.action(1, &first, "stale", &c, 30).unwrap_err(),
        Error::RevisionConflict
    );
    let remaining = w
        .snapshot(1, &c, Phase::Active, 30)
        .relics
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!((remaining.remaining_stacks, remaining.revision), (1, 2));
    let full = relic_claim(&relic.id, 2, w.members[1].revision, None);
    assert_eq!(
        w.action(1, &full, "full", &c, 30).unwrap_err(),
        Error::Capacity
    );
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 30).relics.unwrap(),
        vec![remaining]
    );
    let m = &mut w.members[1];
    let removal = m.plan_acquired_removal("filler", 1, m.revision).unwrap();
    m.commit_acquired_removal("filler", removal, super::loot_drop::RemovalKind::Consumed);
    let last = relic_claim(&relic.id, 2, m.revision, None);
    w.action(1, &last, "remainder", &c, 30).unwrap();
    w.tick(&c, 900);
    let empty = w
        .snapshot(1, &c, Phase::Active, 900)
        .relics
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        (
            empty.remaining_stacks,
            empty.highest_quality,
            empty.revision
        ),
        (0, None, 3)
    );

    w.terminal_intent(1, Outcome::Extraction, &mut vec![]);
    let settlement = w.settlement(1, &c);
    assert!(
        matches!(&settlement, Business::CommitSettlement {instances, looted: 3, kept: 4, ..} if instances == &expected)
    );
    let encoded = c.encode_business(&settlement);
    assert_eq!(c.decode_business(&encoded).unwrap(), settlement);
}

#[test]
fn invalid_position_and_full_relic_budget_keep_inventory() {
    let c = catalog(input(), relic_input).unwrap().default_config();
    for blocked in [true, false] {
        let mut w = world(&c);
        let victim = if blocked { 0 } else { 1 };
        if blocked {
            w.members[0].position = [0., 0.];
        } else {
            w.terminal_intent(0, Outcome::Death, &mut vec![]);
            w.settlement(0, &c);
            assert_eq!(w.relic_views(&c).unwrap()[0].remaining_stacks, 0);
        }
        let revision = w.members[victim].revision;
        w.members[victim]
            .credit_stack("item", 2, false, &c)
            .unwrap();
        w.terminal_intent(victim, Outcome::Death, &mut vec![]);
        assert!(matches!(
            w.settlement(victim, &c),
            Business::CommitSettlement {
                looted: 2,
                kept: 3,
                dropped: 0,
                ..
            }
        ));
        assert_eq!(w.members[victim].revision, revision);
        assert_eq!(w.relic_views(&c).unwrap().len(), usize::from(!blocked));
    }
}

#[test]
fn automatic_seat_claims_through_the_same_relic_actions() {
    let c = catalog(input(), |v| {
        relic_input(v);
        v["world_loot"]["nodes"] = json!([]);
        v["participant_ai"] = json!({"b":{"decision_interval_ms":30,"loot_radius":32,"threat_radius":0,
            "withdraw_hp_bps":0,"extract_after_looted":3,"extract_before_timeout_ms":0}});
    }).unwrap().default_config();
    let mut w = world(&c);
    w.members[0].credit_stack("item", 3, false, &c).unwrap();
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    w.settlement(0, &c);
    let events = w.tick(&c, 30);
    assert!(events.iter().any(|(i, e)| *i == Some(1)
        && matches!(
            e,
            EventV4::Private {
                event: Event::RelicOpened { .. }
            }
        )));
    let events = w.tick(&c, 60);
    assert!(events.iter().any(|(i, e)| *i == Some(1)
        && matches!(
            e,
            EventV4::Private {
                event: Event::RelicClaimed { .. }
            }
        )));
    assert_eq!(w.members[1].inventory["item"], 4);
}

#[test]
fn invalid_instance_and_relic_definitions_fail_catalog_load() {
    for (edit, error) in [
        (0, "CONFIG_WORLD_LOOT_INSTANCE"),
        (1, "CONFIG_WORLD_LOOT_POOL"),
        (2, "CONFIG_WORLD_LOOT_RELIC"),
    ] {
        let result = catalog(input(), |v| {
            relic_input(v);
            match edit {
                0 => v["pve"]["items"]["unique-item"]["stackLimit"] = json!(2),
                1 => v["world_loot"]["pools"]["source"]["guaranteed"][1]["quantity_max"] = json!(2),
                _ => v["world_loot"]["relics"]["capacity"] = json!(9),
            }
        });
        assert!(result.err().unwrap().to_string().contains(error));
    }
}
