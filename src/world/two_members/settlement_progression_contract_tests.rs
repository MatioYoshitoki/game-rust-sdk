use super::settlement_reward_contract_tests::{receipt, requests};
use super::*;
use crate::engine::Engine;
use crate::world::two_members::combat::DamageIntent;
use sha2::{Digest, Sha256};

fn growth_input(v: &mut Value) {
    v["params"]["entry_years"] = json!(2);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["status_definitions"] = json!([{
        "id":"ward","mechanism":"absorption_shield","shield_bps":2000,
        "duration_ms":300,"duplicate_policy":"refresh","source_scoped":false
    }]);
    v["pve"]["items"]["unique"] = json!({"name":"instance","stackLimit":1});
    v["world_loot"] = json!({
        "environment_capacity":8,"container_capacity":4,"partial_pickup":true,
        "interaction_range":16,"shared_clearance":5,"containers":[],
        "pools":{"source":{"guaranteed":[
            {"item_id":"unique","quantity_min":1,"quantity_max":1},
            {"item_id":"unique","quantity_min":1,"quantity_max":1}
        ],"independent":[],"groups":[]}},
        "nodes":[{"id":"source","pool":"source","points":[[20,30]],"replenish_ms":600,"ttl_ms":600}],
        "instances":{"unique":{"quality":8,"relic_eligible":true}}
    });
    v["settlement_rewards"] = json!({
        "reward":{"kill_points":{"npc":10},"rate_bps":7500,"balance_cap":25,
            "multiply_by_entry_cost":true,"by_result":{
                "extract":{"bonus_points":3,"multiplier_bps":10000},
                "death":{"bonus_points":0,"multiplier_bps":5000}
            }},
        "track":{"kill_points":{},"rate_bps":10000,"balance_cap":100,
            "damage":{"rate_bps":5000,"per_hit_cap_points":2,"match_cap_points":3},
            "by_result":{
                "extract":{"bonus_points":1,"multiplier_bps":7500},
                "death":{"bonus_points":1,"multiplier_bps":5000}
            }}
    });
    v["settlement_facts"] = json!({"extract":["example"]});
    v["settlement_progression"] = json!({
        "age_asset_id":"age","storage_caps":{"item":3},"instance_capacity":1,
        "conversions":[
            {"id":"first","when":{"kind":"at_most","asset_id":"entry","quantity":0},
             "changes":{"age":{"kind":"set","quantity":18},
                        "entry":{"kind":"set","quantity":12},
                        "reward":{"kind":"scale","bps":5000},
                        "turn":{"kind":"add_capped","quantity":1,"cap":2}}},
            {"id":"later","when":{"kind":"at_least","asset_id":"reward","quantity":25},
             "changes":{"turn":{"kind":"set","quantity":99}}}
        ],
        "destinations":{"extract":"next"}
    });
}

fn facts(remaining: u32) -> AssetParticipantFacts {
    AssetParticipantFacts {
        principal: "a".into(),
        revision: 0,
        hp_points: 100,
        spirit_points: 100,
        atk_points: 10,
        def_points: 0,
        spd_points: 1,
        assets: [
            ("age", 18),
            ("entry", remaining),
            ("item", 4),
            ("reward", 9),
            ("turn", 1),
        ]
        .into_iter()
        .map(|(asset_id, quantity)| AssetAmount {
            asset_id: asset_id.into(),
            quantity,
        })
        .collect(),
        instance_count: Some(0),
        warehouse_other_slots: None,
        location: Some(AccountLocation {
            region_id: "prior".into(),
            revision: 7,
        }),
    }
}

fn created_ledger(
    change: impl FnOnce(&mut Value),
) -> (Arc<crate::ConfigCatalog>, Arc<Config>, Engine, Create) {
    let catalog = Arc::new(
        catalog(input(), |v| {
            growth_input(v);
            change(v);
        })
        .unwrap(),
    );
    let c = catalog.default_config();
    let mut ledger = Engine::new_member_ledger(c.single_member_validation(), catalog.clone());
    let create = Create {
        protocol_version: 3,
        key: MatchKey {
            id: "growth".into(),
            incarnation: 1,
        },
        request_id: "create".into(),
        config_id: c.id.clone(),
        config_hash: c.hash(),
        seed: 1,
        participant_id: "a".into(),
        authenticated_principal: "a".into(),
        connection: Connection {
            id: "a".into(),
            generation: 1,
        },
        publication_mode: PublicationMode::Interactive,
        recovery: None,
    };
    ledger.create(create.clone()).unwrap();
    (catalog, c, ledger, create)
}

fn ordered_clock_input(v: &mut Value) {
    v["settlement_rewards"]["reward"]["entry_cost_basis"] = json!("charged");
    v["settlement_rewards"]["track"]["multiply_by_entry_cost"] = json!(true);
    v["settlement_progression"]["partial_entry_charge"] = json!(true);
    v["settlement_progression"]["conversion_mode"] = json!("ordered");
    v["settlement_progression"]["conversions"] = json!([
        {"id":"charge", "result_ids":["death"], "when":{"kind":"at_least","asset_id":"active","quantity":1},
         "and":[{"kind":"at_least","asset_id":"phase","quantity":2}], "clock_cost":7,
         "changes":{"active":{"kind":"set","quantity":0},"phase":{"kind":"set","quantity":0},"reward":{"kind":"set","quantity":6}}},
        {"id":"exhausted", "when":{"kind":"at_most","asset_id":"entry","quantity":0},
         "and":[{"kind":"at_most","asset_id":"pending","quantity":0}],
         "changes":{"pending":{"kind":"set","quantity":1}}}
    ]);
    v["settlement_progression"]["threshold_progress"] = json!({
        "stage_asset_id":"stage","total_asset_id":"reward","progress_asset_id":"progress","thresholds":[0,5,10]
    });
}

fn ordered_clock_facts(entry: u32, phase: u32) -> AssetParticipantFacts {
    let mut account = facts(entry);
    for (id, quantity) in [
        ("active", 1),
        ("phase", phase),
        ("pending", 0),
        ("stage", 1),
        ("progress", 4),
    ] {
        account.assets.push(AssetAmount {
            asset_id: id.into(),
            quantity,
        });
    }
    account.assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    account
}

#[test]
fn ordered_clock_uses_the_actual_charge_and_freezes_each_conversion() {
    for (outcome, entry, phase, expected) in [
        (Outcome::Extraction, 1, 2, vec!["exhausted"]),
        (Outcome::Death, 5, 2, vec!["charge", "exhausted"]),
        (Outcome::Death, 5, 1, vec![]),
        (Outcome::Extraction, 5, 2, vec![]),
    ] {
        let (catalog, c, mut ledger, mut create) = created_ledger(ordered_clock_input);
        let load = ReceiptOutcome::AssetsLoaded {
            facts: ordered_clock_facts(entry, phase),
        };
        receipt(&mut ledger, &create.key, "load", load);
        let admission = requests(&mut ledger)
            .into_iter()
            .find(|request| {
                matches!(
                    &request.body,
                    Business::CommitAssets {
                        transaction: AssetTransaction::Admission { .. }
                    }
                )
            })
            .unwrap();
        let Business::CommitAssets {
            transaction:
                AssetTransaction::Admission {
                    debits,
                    clock: Some(clock),
                    ..
                },
        } = &admission.body
        else {
            panic!()
        };
        let fee = entry.min(2);
        assert_eq!(debits[0].quantity, fee);
        assert_eq!(
            (clock.remaining.after, clock.elapsed.after),
            (entry - fee, 18 + fee)
        );
        assert!(c.decode_business(&admission.body).is_ok());
        receipt(
            &mut ledger,
            &create.key,
            "admit",
            ReceiptOutcome::Committed {
                revision: 1,
                commit_id: "admitted".into(),
            },
        );
        ledger.ledger_activate();
        let mut w = world(&c);
        w.members[0].facts.revision = 1;
        w.terminal_intent(0, outcome, &mut vec![]);
        ledger.ledger_settle(w.settlement(0, &c)).unwrap();
        let checkpoint = ledger.ledger_checkpoint();
        let mut invalid = checkpoint.clone();
        let op = invalid
            .state
            .operations
            .iter_mut()
            .find(|op| op.request.operation_id.ends_with(":settle"))
            .unwrap();
        let Business::CommitAssets {
            transaction: AssetTransaction::ResultSettlement { summary, .. },
        } = &mut op.request.body
        else {
            panic!()
        };
        let plan = summary.progression.as_mut().unwrap();
        assert_eq!(plan.entry_charged, Some(fee));
        assert!(plan.conversion.is_none());
        let steps = plan.conversion_steps.as_mut().unwrap();
        assert_eq!(
            steps
                .iter()
                .map(|step| step.rule_id.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        if entry == 1 {
            assert_eq!(summary.rewards[0].requested, 2);
            assert_eq!(summary.rewards[1].requested, 1);
        }
        if expected.len() == 2 {
            let change = |id: &str| {
                let row = steps[0].changes.iter().find(|r| r.asset_id == id).unwrap();
                (row.before, row.after)
            };
            assert_eq!(change("entry"), (3, 0));
            assert_eq!(change("age"), (20, 23));
            assert_eq!(change("active"), (1, 0));
            assert_eq!(change("phase"), (2, 0));
            let stage = plan.threshold_progress.as_ref().unwrap();
            assert_eq!((stage.total, stage.progress.after), (6, 1));
            assert_eq!(
                (steps[1].changes[0].before, steps[1].changes[0].after),
                (0, 1)
            );
            steps.swap(0, 1);
            invalid.sha256 = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&invalid.state).unwrap())
            );
            let mut restored =
                Engine::new_member_ledger(c.single_member_validation(), catalog.clone());
            create.recovery = Some(Recovery {
                generation: 1,
                checkpoint: invalid,
            });
            assert!(matches!(
                restored.create(create.clone()),
                Err(Error::InvalidMessage)
            ));
        }
        let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
        create.recovery = Some(Recovery {
            generation: 1,
            checkpoint,
        });
        restored.create(create).unwrap();
    }
}

#[test]
fn partial_clock_refunds_the_actual_amount_before_activation() {
    let (catalog, c, mut ledger, mut create) = created_ledger(ordered_clock_input);
    assert_eq!(
        c.loaded_facts(&ReceiptOutcome::AssetsLoaded {
            facts: ordered_clock_facts(0, 2)
        }),
        Err(Error::NoResource)
    );
    receipt(
        &mut ledger,
        &create.key,
        "load",
        ReceiptOutcome::AssetsLoaded {
            facts: ordered_clock_facts(1, 2),
        },
    );
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admitted".into(),
        },
    );
    ledger.ledger_close().unwrap();
    let refund = requests(&mut ledger)
        .into_iter()
        .find(|r| {
            matches!(
                &r.body,
                Business::CommitAssets {
                    transaction: AssetTransaction::Compensation { .. }
                }
            )
        })
        .unwrap();
    let Business::CommitAssets {
        transaction:
            AssetTransaction::Compensation {
                credits,
                clock: Some(clock),
                ..
            },
    } = &refund.body
    else {
        panic!()
    };
    assert_eq!(credits[0].quantity, 1);
    assert_eq!(
        (
            clock.remaining.before,
            clock.remaining.after,
            clock.elapsed.before,
            clock.elapsed.after
        ),
        (0, 1, 19, 18)
    );
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    create.recovery = Some(Recovery {
        generation: 1,
        checkpoint: ledger.ledger_checkpoint(),
    });
    restored.create(create).unwrap();
}

fn adjustment_input(v: &mut Value, original: u32) {
    v["settlement_progression"]
        .as_object_mut()
        .unwrap()
        .remove("conversions");
    v["settlement_rewards"]["reward"] = json!({
        "kill_points":{}, "rate_bps":10000, "balance_cap":200,
        "by_result":{
            "extract":{"bonus_points":original,"multiplier_bps":10000},
            "death":{"bonus_points":original,"multiplier_bps":10000}
        },
        "adjustments":[
            {"uses_asset_id":"scale-uses","effect":{"kind":"scale","bps":12500}},
            {"uses_asset_id":"add-uses","effect":{"kind":"add_original","bps":2500}},
            {"uses_asset_id":"defer-uses","effect":{"kind":"defer","escrow_asset_id":"escrow",
                "cap":100,"divert_bps":2500,"release_bps":15000,"forfeit_results":["death"]}}
        ]
    });
}

#[test]
fn reward_adjustments_freeze_order_limits_and_trusted_baselines() {
    for (outcome, original, uses, escrow, starting, expected) in [
        (Outcome::Extraction, 101, 2, 9, 9, (114, 0, 46, 1, 114)),
        (Outcome::Death, 101, 2, 9, 9, (114, 0, 0, 0, 114)),
        (Outcome::Death, 101, 1, 46, 123, (238, 124, 0, 0, 77)),
        (Outcome::Extraction, 101, 1, 98, 9, (299, 150, 0, 0, 191)),
        (Outcome::Extraction, 0, 2, 9, 9, (0, 0, 9, 2, 0)),
    ] {
        let (catalog, c, mut ledger, mut create) =
            created_ledger(|v| adjustment_input(v, original));
        let mut account = facts(2);
        account
            .assets
            .iter_mut()
            .find(|row| row.asset_id == "reward")
            .unwrap()
            .quantity = starting;
        for (id, quantity) in [
            ("scale-uses", uses),
            ("add-uses", uses),
            ("defer-uses", uses),
            ("escrow", escrow),
        ] {
            account.assets.push(AssetAmount {
                asset_id: id.into(),
                quantity,
            });
        }
        account.assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        receipt(
            &mut ledger,
            &create.key,
            "load",
            ReceiptOutcome::AssetsLoaded { facts: account },
        );
        receipt(
            &mut ledger,
            &create.key,
            "admit",
            ReceiptOutcome::Committed {
                revision: 1,
                commit_id: "admitted".into(),
            },
        );
        ledger.ledger_activate();
        let mut w = world(&c);
        w.members[0].facts.revision = 1;
        w.terminal_intent(0, outcome, &mut vec![]);
        ledger.ledger_settle(w.settlement(0, &c)).unwrap();
        let checkpoint = ledger.ledger_checkpoint();
        let mut tampered = checkpoint.clone();
        let frozen = tampered
            .state
            .operations
            .iter_mut()
            .find(|op| op.request.operation_id.ends_with(":settle"))
            .unwrap();
        let operation_id = frozen.request.operation_id.clone();
        let Business::CommitAssets {
            transaction: AssetTransaction::ResultSettlement { summary, .. },
        } = &mut frozen.request.body
        else {
            panic!()
        };
        let reward = summary
            .rewards
            .iter_mut()
            .find(|r| r.asset_id == "reward")
            .unwrap();
        let (requested, payout, after_escrow, after_uses, awarded) = expected;
        assert_eq!((reward.requested, reward.awarded), (requested, awarded));
        if original == 0 {
            assert!(reward.adjustment.is_none());
        } else {
            let adjustment = reward.adjustment.as_mut().unwrap();
            assert_eq!(
                (adjustment.original_requested, adjustment.deferred_payout),
                (original, payout)
            );
            assert_eq!(
                adjustment
                    .changes
                    .iter()
                    .find(|r| r.asset_id == "defer-uses")
                    .unwrap()
                    .after,
                after_uses
            );
            let saved = adjustment
                .changes
                .iter_mut()
                .find(|r| r.asset_id == "escrow")
                .unwrap();
            assert_eq!((saved.before, saved.after), (escrow, after_escrow));
            if outcome == Outcome::Extraction && uses == 2 {
                saved.before += 1;
                saved.after += 1;
                assert!(c.decode_business(&frozen.request.body).is_ok());
                tampered.sha256 = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&tampered.state).unwrap())
                );
                let mut restored =
                    Engine::new_member_ledger(c.single_member_validation(), catalog.clone());
                create.recovery = Some(Recovery {
                    generation: 1,
                    checkpoint: tampered,
                });
                assert!(matches!(
                    restored.create(create.clone()),
                    Err(Error::InvalidMessage)
                ));
            }
        }
        let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
        create.recovery = Some(Recovery {
            generation: 1,
            checkpoint,
        });
        restored.create(create).unwrap();
        assert!(requests(&mut restored).iter().any(
            |r| matches!(&r.body,Business::QueryOperation{operation_id:id} if id==&operation_id)
        ));
    }
}

fn stack_flow_input(v: &mut Value) {
    v["inventory_capacity"] = json!(2);
    v["params"]["death"] = json!({"slot_ratio":1,"quantity_ratio":1,"minimum_quantity":1});
    v["params"]["runtime"]["resource_sweep_ms"] = json!(30);
    v["pve"]["items"]["supply"] = json!({"name":"supply","stackLimit":1});
    v["member_item_use"] = json!({"item":{"heal_hp":10},"supply":{"heal_hp":10}});
    v["member_loot_drop"] = json!({"max_active":2});
    v["world_loot"]["pools"] = json!({
        "main":{"guaranteed":[{"item_id":"item","quantity_min":8,"quantity_max":8}],"independent":[],"groups":[]},
        "supply":{"guaranteed":[{"item_id":"supply","quantity_min":1,"quantity_max":1}],"independent":[],"groups":[]}
    });
    v["world_loot"].as_object_mut().unwrap().remove("instances");
    v["pve"]["items"].as_object_mut().unwrap().remove("unique");
    v["world_loot"]["nodes"] = json!([
        {"id":"main","pool":"main","points":[[20,30]],"replenish_ms":600,"ttl_ms":600},
        {"id":"supply","pool":"supply","points":[[28,30]],"replenish_ms":600,"ttl_ms":600}
    ]);
    v["world_loot"]["relics"] = json!({"capacity":2,"interaction_range":24});
    v["settlement_progression"]["stack_flows"] = json!(true);
    v["settlement_progression"]["storage_caps"]["item"] = json!(6);
}

#[test]
fn stack_flows_track_only_committed_movements_and_freeze_for_recovery() {
    let (catalog, c, mut ledger, mut create) = created_ledger(stack_flow_input);
    admission(&mut ledger, &create.key);
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admitted".into(),
        },
    );
    ledger.ledger_activate();
    let mut w = world(&c);
    w.members[0].facts.revision = 1;
    w.tick(&c, 30);
    let id = |item: &str| {
        w.resources
            .iter()
            .find(|r| r.view.item_id.as_deref() == Some(item))
            .unwrap()
            .view
            .id
            .clone()
    };
    let main = id("item");
    let supply = id("supply");
    let pickup = |w: &mut World, i: usize, id: &str| {
        w.action(
            i,
            &Action::Pickup {
                resource_id: id.into(),
                expected_revision: w.members[i].revision,
            },
            "pick",
            &c,
            30,
        )
    };
    let use_item = |w: &mut World, i: usize, item: &str| {
        w.action(
            i,
            &Action::UseItem {
                item_id: item.into(),
                expected_revision: w.members[i].revision,
            },
            "use",
            &c,
            30,
        )
    };
    let release = |w: &mut World| {
        let rev = w.members[0].revision;
        w.action(
            0,
            &Action::DropItem {
                item_id: "item".into(),
                quantity: 4,
                expected_revision: rev,
            },
            "drop",
            &c,
            30,
        )
        .unwrap();
        w.resources
            .iter()
            .find(|r| r.player_drop)
            .unwrap()
            .view
            .id
            .clone()
    };
    pickup(&mut w, 0, &main).unwrap();
    assert_eq!(w.members[0].inventory["item"], 8);
    assert_eq!(pickup(&mut w, 0, &main).unwrap_err(), Error::Capacity);
    assert_eq!(
        use_item(&mut w, 0, "item").unwrap_err(),
        Error::InvalidState
    );
    let first = release(&mut w);
    pickup(&mut w, 0, &first).unwrap();
    let second = release(&mut w);
    pickup(&mut w, 1, &second).unwrap();
    pickup(&mut w, 0, &supply).unwrap();
    w.members[0].hp = 80;
    use_item(&mut w, 0, "supply").unwrap();
    pickup(&mut w, 0, &main).unwrap();
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    let victim = w.settlement(0, &c);
    let Business::CommitSettlement {
        progression,
        carried,
        looted,
        kept,
        dropped,
        items,
        ..
    } = victim
    else {
        panic!()
    };
    assert_eq!((carried, looted, kept, dropped), (1, 0, 1, 0));
    assert!(items.is_empty());
    let flows = progression.unwrap().stack_flows.unwrap();
    assert_eq!(
        flows.as_ref(),
        [
            StackFlow {
                item_id: "item".into(),
                picked_up: 12,
                consumed: 0,
                released: 8,
                relic_lost: 4
            },
            StackFlow {
                item_id: "supply".into(),
                picked_up: 1,
                consumed: 1,
                released: 0,
                relic_lost: 0
            },
        ]
    );

    w.members[1].hp = 80;
    use_item(&mut w, 1, "item").unwrap();
    let relic = w.relic_views(&c).unwrap().pop().unwrap();
    w.action(
        1,
        &Action::OpenRelic {
            relic_id: relic.id.clone(),
        },
        "open",
        &c,
        30,
    )
    .unwrap();
    w.action(
        1,
        &Action::ClaimRelic {
            relic_id: relic.id,
            expected_revision: relic.revision,
            expected_inventory_revision: w.members[1].revision,
            entry_id: None,
        },
        "claim",
        &c,
        30,
    )
    .unwrap();
    w.members[1].facts.revision = 1;
    w.terminal_intent(1, Outcome::Extraction, &mut vec![]);
    ledger.ledger_clock(30);
    ledger.ledger_settle(w.settlement(1, &c)).unwrap();
    let checkpoint = ledger.ledger_checkpoint();
    let original = checkpoint
        .state
        .operations
        .iter()
        .find(|op| op.request.operation_id.ends_with(":settle"))
        .unwrap()
        .request
        .clone();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &original.body
    else {
        panic!()
    };
    let plan = summary.progression.as_ref().unwrap();
    assert_eq!(summary.inventory[0].kept, 8);
    assert_eq!((plan.storage[0].stored, plan.storage[0].overflow), (3, 5));
    assert_eq!(
        plan.stack_flows.as_deref().unwrap(),
        [StackFlow {
            item_id: "item".into(),
            picked_up: 8,
            consumed: 1,
            released: 0,
            relic_lost: 0
        }]
    );

    let mut corrupted = checkpoint.clone();
    let op = corrupted
        .state
        .operations
        .iter_mut()
        .find(|op| op.request.operation_id == original.operation_id)
        .unwrap();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &mut op.request.body
    else {
        panic!()
    };
    summary
        .progression
        .as_mut()
        .unwrap()
        .stack_flows
        .as_mut()
        .unwrap()[0]
        .picked_up += 1;
    corrupted.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&corrupted.state).unwrap())
    );
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog.clone());
    create.recovery = Some(Recovery {
        generation: 1,
        checkpoint: corrupted,
    });
    assert!(matches!(
        restored.create(create.clone()),
        Err(Error::InvalidMessage)
    ));
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    create.recovery = Some(Recovery {
        generation: 1,
        checkpoint,
    });
    restored.create(create).unwrap();
    let requests = requests(&mut restored);
    assert!(requests.iter().any(|r|matches!(&r.body,Business::QueryOperation{operation_id} if operation_id==&original.operation_id)));
}

fn admission(ledger: &mut Engine, key: &MatchKey) -> EntryClock {
    receipt(
        ledger,
        key,
        "load",
        ReceiptOutcome::AssetsLoaded { facts: facts(2) },
    );
    let clock = requests(ledger)
        .into_iter()
        .find_map(|r| match r.body {
            Business::CommitAssets {
                transaction: AssetTransaction::Admission { clock, .. },
            } => clock,
            _ => None,
        })
        .unwrap();
    assert_eq!((clock.remaining.before, clock.remaining.after), (2, 0));
    assert_eq!((clock.elapsed.before, clock.elapsed.after), (18, 20));
    clock
}

#[test]
fn entry_clock_validates_before_debit_and_compensation_reverses_frozen_clock() {
    let (_, c, mut ledger, create) = created_ledger(|_| {});
    requests(&mut ledger);
    let mut account = facts(1);
    assert!(matches!(
        ledger.business(BusinessReceipt {
            key: create.key.clone(),
            operation_id: "growth:1:load".into(),
            attempt_id: 1,
            outcome: ReceiptOutcome::AssetsLoaded { facts: account },
        }),
        Err(Error::NoResource)
    ));
    assert!(requests(&mut ledger).is_empty());
    account = facts(2);
    account.revision = u32::MAX - 1;
    assert_eq!(
        c.progression_entry_clock(&account),
        Err(Error::InvalidMessage)
    );
    account.revision = 0;
    account.assets[0].quantity = u32::MAX;
    assert_eq!(c.progression_entry_clock(&account), Err(Error::NoResource));
    let clock = admission(&mut ledger, &create.key);
    ledger.ledger_close().unwrap();
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admit-proof".into(),
        },
    );
    let refund = requests(&mut ledger)
        .into_iter()
        .find(|r| {
            matches!(
                &r.body,
                Business::CommitAssets {
                    transaction: AssetTransaction::Compensation { .. }
                }
            )
        })
        .unwrap();
    assert!(matches!(refund.body,
        Business::CommitAssets { transaction: AssetTransaction::Compensation { clock: Some(reverse), .. } }
            if reverse == clock.reversed()));
}

#[test]
fn zero_cost_uses_revisioned_empty_admission_and_compensation() {
    let (_, _, mut ledger, create) = created_ledger(|v| {
        v["params"]["entry_years"] = json!(0);
        v["carried_quantity"] = json!(0);
    });
    let mut account = facts(0);
    account.assets.clear();
    receipt(
        &mut ledger,
        &create.key,
        "load",
        ReceiptOutcome::AssetsLoaded { facts: account },
    );
    let original = requests(&mut ledger)
        .into_iter()
        .find(|r| {
            matches!(
                &r.body,
                Business::CommitAssets {
                    transaction: AssetTransaction::Admission { .. }
                }
            )
        })
        .unwrap();
    assert!(matches!(&original.body,
        Business::CommitAssets { transaction: AssetTransaction::Admission {
            debits, clock: None, expected_revision:0,
        }} if debits.is_empty()));
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Unknown {},
    );
    assert!(requests(&mut ledger).iter().any(|r| matches!(&r.body,
        Business::QueryOperation {operation_id} if *operation_id==original.operation_id)));
    receipt(
        &mut ledger,
        &create.key,
        "query-admit-1",
        ReceiptOutcome::NotCommitted {},
    );
    let retry = requests(&mut ledger)
        .into_iter()
        .find(|r| r.operation_id == original.operation_id)
        .unwrap();
    assert_eq!((retry.attempt_id, retry.body), (2, original.body));
    ledger.ledger_close().unwrap();
    ledger
        .business(BusinessReceipt {
            key: create.key,
            operation_id: retry.operation_id,
            attempt_id: 2,
            outcome: ReceiptOutcome::Committed {
                revision: 1,
                commit_id: "empty-admit".into(),
            },
        })
        .unwrap();
    let compensation = requests(&mut ledger)
        .into_iter()
        .find(|r| {
            matches!(
                &r.body,
                Business::CommitAssets {
                    transaction: AssetTransaction::Compensation { .. }
                }
            )
        })
        .unwrap();
    assert!(matches!(compensation.body,
        Business::CommitAssets {transaction:AssetTransaction::Compensation {
            original_operation_id, credits, clock:None,
        }} if original_operation_id==original.operation_id && credits.is_empty()));
}

fn shared_warehouse(v: &mut Value) {
    v["inventory_capacity"] = json!(8);
    for id in ["alpha", "zeta"] {
        v["pve"]["items"][id] = json!({"name":id,"stackLimit":4});
        v["world_loot"]["pools"]["source"]["guaranteed"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "item_id":id,"quantity_min":2,"quantity_max":2
            }));
    }
    v["settlement_progression"]["storage_caps"] = json!({"alpha":4});
    v["settlement_progression"]["instance_capacity"] = json!(2);
    v["settlement_progression"]["warehouse_slots"] = json!({
        "capacity":5,"stack_sizes":{"alpha":4,"item":3,"zeta":2}
    });
}

#[test]
fn shared_warehouse_freezes_admission_space_stack_priority_and_instance_overflow() {
    let (catalog, c, mut ledger, create) = created_ledger(shared_warehouse);
    let mut account = facts(2);
    account.assets.insert(
        1,
        AssetAmount {
            asset_id: "alpha".into(),
            quantity: 3,
        },
    );
    account.instance_count = Some(1);
    assert_eq!(
        c.progression_entry_clock(&account),
        Err(Error::InvalidMessage)
    );
    account.warehouse_other_slots = Some(u32::MAX);
    assert_eq!(
        c.progression_entry_clock(&account),
        Err(Error::InvalidMessage)
    );
    account.warehouse_other_slots = Some(1);
    receipt(
        &mut ledger,
        &create.key,
        "load",
        ReceiptOutcome::AssetsLoaded {
            facts: account.clone(),
        },
    );
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admitted".into(),
        },
    );
    ledger.ledger_activate();

    let mut w = world(&c);
    w.bind_instance_scope("a".repeat(64));
    w.members[0].facts.revision = 1;
    w.tick(&c, 30);
    for drop in w.snapshot(0, &c, Phase::Active, 30).resources {
        w.action(
            0,
            &Action::Pickup {
                resource_id: drop.id,
                expected_revision: w.members[0].revision,
            },
            "pick",
            &c,
            30,
        )
        .unwrap();
    }
    w.terminal_intent(0, Outcome::Extraction, &mut vec![]);
    let draft = w.settlement(0, &c);
    for (other, existing_instances, before, after, item, zeta, stored_instances) in [
        (1, 1, 4, 5, 1, 0, 0),
        (0, 1, 3, 5, 1, 2, 0),
        (3, 1, 6, 6, 0, 0, 0),
        (0, 0, 2, 5, 1, 2, 1),
    ] {
        account.warehouse_other_slots = Some(other);
        account.instance_count = Some(existing_instances);
        let plan = c.plan_progression(&draft, &account).unwrap();
        assert_eq!(plan.warehouse_slots, Some(WarehouseSlots { before, after }));
        assert_eq!(
            plan.storage
                .iter()
                .map(|row| (row.asset_id.as_str(), row.stored, row.overflow))
                .collect::<Vec<_>>(),
            [
                ("alpha", 1, 1),
                ("item", item, 1 - item),
                ("zeta", zeta, 2 - zeta)
            ]
        );
        let instances = plan.instance_storage.unwrap();
        assert_eq!(instances.stored_ids.len(), stored_instances);
        assert_eq!(instances.overflow_ids.len(), 2 - stored_instances);
    }

    ledger.ledger_settle(draft).unwrap();
    let checkpoint = ledger.ledger_checkpoint();
    let mut corrupt = checkpoint.clone();
    let op = corrupt
        .state
        .operations
        .iter_mut()
        .find(|op| op.request.operation_id.ends_with(":settle"))
        .unwrap();
    let frozen = op.request.clone();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement {
            summary, credits, ..
        },
    } = &mut op.request.body
    else {
        panic!()
    };
    let plan = summary.progression.as_mut().unwrap();
    assert_eq!(
        plan.warehouse_slots,
        Some(WarehouseSlots {
            before: 4,
            after: 5
        })
    );
    assert_eq!(
        credits
            .iter()
            .filter(|r| matches!(r.asset_id.as_str(), "alpha" | "item" | "zeta"))
            .map(|r| (r.asset_id.as_str(), r.quantity))
            .collect::<Vec<_>>(),
        [("alpha", 1), ("item", 1)]
    );
    plan.warehouse_slots.as_mut().unwrap().before = 3;
    corrupt.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&corrupt.state).unwrap())
    );
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    assert!(matches!(
        restored.create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint: corrupt
            }),
            ..create.clone()
        }),
        Err(Error::InvalidMessage)
    ));
    restored
        .create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint,
            }),
            ..create.clone()
        })
        .unwrap();
    receipt(
        &mut restored,
        &create.key,
        "query-settle-1",
        ReceiptOutcome::NotCommitted {},
    );
    let retry = requests(&mut restored)
        .into_iter()
        .find(|r| r.operation_id == frozen.operation_id)
        .unwrap();
    assert_eq!((retry.attempt_id, retry.body), (2, frozen.body));
}

#[test]
fn shared_warehouse_layout_covers_every_stack_and_excludes_instances() {
    for invalid in ["missing", "instance", "zero"] {
        let result = catalog(input(), |v| {
            growth_input(v);
            shared_warehouse(v);
            let sizes = v["settlement_progression"]["warehouse_slots"]["stack_sizes"]
                .as_object_mut()
                .unwrap();
            match invalid {
                "missing" => {
                    sizes.remove("alpha");
                }
                "instance" => {
                    sizes.insert("unique".into(), json!(1));
                }
                _ => {
                    sizes.insert("alpha".into(), json!(0));
                }
            }
        });
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("CONFIG_PROGRESSION_WAREHOUSE")
        );
    }
}

#[test]
fn effective_hit_growth_and_storage_conversion_region_freeze_together() {
    let (catalog, c, mut ledger, create) = created_ledger(|_| {});
    admission(&mut ledger, &create.key);
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admit-proof".into(),
        },
    );
    ledger.ledger_activate();
    let mut w = world(&c);
    w.bind_instance_scope("a".repeat(64));
    w.members[0].facts.revision = 1;
    let target = w.owner_entity(Owner::Npc(0));
    w.apply_owned_statuses(
        Owner::Npc(0),
        (target.clone(), 100),
        &["ward".into()],
        &c,
        0,
        &mut vec![],
    );
    let a = w.owner_entity(Owner::Member(0));
    let b = w.owner_entity(Owner::Member(1));
    let mut strike = |source: &EntityRef, raw, direct| {
        w.resolve_damage(
            DamageIntent {
                source: source.clone(),
                target: target.clone(),
                raw,
                direct,
                ignore_armor: true,
            },
            &c,
            0,
            &mut vec![],
        )
    };
    assert_eq!(strike(&a, 20, true).absorbed, 20);
    assert_eq!(strike(&a, 3, true).hp_damage, 3);
    assert_eq!(strike(&b, 1, true).hp_damage, 1);
    assert_eq!(strike(&a, 93, false).hp_damage, 93);
    assert_eq!(strike(&a, 100, true).hp_damage, 3);
    assert_eq!(w.members[0].damage_growth["track"], 30_000);
    assert_eq!(w.members[1].damage_growth["track"], 5_000);
    let death_rows = c
        .reward_plan_with_damage(
            Outcome::Death,
            Some(&c.score_summary(&BTreeMap::new())),
            c.progression_draft(&w.members[1].damage_growth).as_ref(),
            |_| 0,
        )
        .unwrap();
    assert_eq!(death_rows[1].requested, 0);

    w.tick(&c, 30);
    let resources = w.snapshot(0, &c, Phase::Active, 30).resources;
    let instances = resources.into_iter().filter(|drop| drop.instance.is_some());
    for drop in instances {
        w.action(
            0,
            &Action::Pickup {
                resource_id: drop.id,
                expected_revision: w.members[0].revision,
            },
            "pick",
            &c,
            30,
        )
        .unwrap();
    }
    w.terminal_intent(0, Outcome::Extraction, &mut vec![]);
    ledger.ledger_settle(w.settlement(0, &c)).unwrap();
    let checkpoint = ledger.ledger_checkpoint();
    let frozen = checkpoint
        .state
        .operations
        .iter()
        .find(|op| op.request.operation_id.ends_with(":settle"))
        .unwrap()
        .request
        .clone();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement {
            summary, credits, ..
        },
    } = &frozen.body
    else {
        panic!()
    };
    let growth = summary.progression.as_ref().unwrap();
    assert_eq!(
        (summary.rewards[0].requested, summary.rewards[0].awarded),
        (19, 16)
    );
    assert_eq!(summary.rewards[1].awarded, 3);
    assert_eq!(
        (growth.storage[0].stored, growth.storage[0].overflow),
        (0, 1)
    );
    assert!(!credits.iter().any(|r| r.asset_id == "item"));
    let instances = growth.instance_storage.as_ref().unwrap();
    assert_eq!((instances.before, instances.after), (0, 1));
    assert_eq!(
        instances.stored_ids,
        [summary.instances[0].instance_id.clone()]
    );
    assert_eq!(
        instances.overflow_ids,
        [summary.instances[1].instance_id.clone()]
    );
    let conversion = growth.conversion.as_ref().unwrap();
    assert_eq!(conversion.rule_id, "first");
    assert_eq!(
        conversion
            .changes
            .iter()
            .map(|r| (r.before, r.after))
            .collect::<Vec<_>>(),
        [(20, 18), (0, 12), (25, 12), (1, 2)]
    );
    let region = growth.region.as_ref().unwrap();
    assert_eq!(
        (
            &*region.before.region_id,
            region.before.revision,
            &*region.after.region_id,
            region.after.revision
        ),
        ("prior", 7, "next", 8)
    );

    let mut corrupt = checkpoint.clone();
    let op = corrupt
        .state
        .operations
        .iter_mut()
        .find(|op| op.request.operation_id == frozen.operation_id)
        .unwrap();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &mut op.request.body
    else {
        panic!()
    };
    summary
        .progression
        .as_mut()
        .unwrap()
        .region
        .as_mut()
        .unwrap()
        .after
        .region_id = "wrong".into();
    corrupt.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&corrupt.state).unwrap())
    );
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    assert!(matches!(
        restored.create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint: corrupt
            }),
            ..create.clone()
        }),
        Err(Error::InvalidMessage)
    ));
    restored
        .create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint,
            }),
            ..create.clone()
        })
        .unwrap();
    receipt(
        &mut restored,
        &create.key,
        "query-settle-1",
        ReceiptOutcome::NotCommitted {},
    );
    let retry = requests(&mut restored)
        .into_iter()
        .find(|r| r.operation_id == frozen.operation_id)
        .unwrap();
    assert_eq!(retry.attempt_id, 2);
    assert_eq!(retry.body, frozen.body);
}

#[test]
fn per_contact_growth_rounding_preserves_default_and_rejects_fractional_recovery() {
    let (catalog, c, mut ledger, create) = created_ledger(|v| {
        let mut rounded = v["settlement_rewards"]["track"].clone();
        rounded["damage"]["rounding"] = json!("per_contact");
        v["settlement_rewards"]["rounded"] = rounded;
    });
    admission(&mut ledger, &create.key);
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admitted".into(),
        },
    );
    ledger.ledger_activate();
    let mut w = world(&c);
    w.members[0].facts.revision = 1;
    let target = w.owner_entity(Owner::Npc(0));
    for (seat, amount) in [(0, 1), (0, 1), (1, 3), (1, 20), (1, 20)] {
        let source = w.owner_entity(Owner::Member(seat));
        let result = w.resolve_damage(
            DamageIntent {
                source,
                target: target.clone(),
                raw: amount,
                direct: true,
                ignore_armor: true,
            },
            &c,
            0,
            &mut vec![],
        );
        assert_eq!(result.hp_damage, amount);
        if seat == 1 && amount == 3 {
            assert_eq!(w.members[1].damage_growth["rounded"], 10_000);
            assert_eq!(w.members[1].damage_growth["track"], 15_000);
        }
    }
    assert_eq!(w.members[0].damage_growth["track"], 10_000);
    assert_eq!(w.members[0].damage_growth["rounded"], 0);
    assert_eq!(w.members[1].damage_growth["rounded"], 30_000);
    assert_eq!(w.members[1].damage_growth["track"], 30_000);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    ledger.ledger_settle(w.settlement(0, &c)).unwrap();
    let checkpoint = ledger.ledger_checkpoint();
    let frozen = checkpoint
        .state
        .operations
        .iter()
        .find(|op| op.request.operation_id.ends_with(":settle"))
        .unwrap()
        .request
        .clone();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &frozen.body
    else {
        panic!()
    };
    let requested = |id| {
        summary
            .rewards
            .iter()
            .find(|r| r.asset_id == id)
            .unwrap()
            .requested
    };
    assert_eq!(requested("rounded"), 0);
    assert_eq!(requested("track"), 1);

    let mut forged = checkpoint.clone();
    let op = forged
        .state
        .operations
        .iter_mut()
        .find(|op| op.request.operation_id == frozen.operation_id)
        .unwrap();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &mut op.request.body
    else {
        panic!()
    };
    summary
        .progression
        .as_mut()
        .unwrap()
        .damage
        .iter_mut()
        .find(|row| row.asset_id == "rounded")
        .unwrap()
        .points_bps = 1;
    // This change cannot alter the rounded award, but it is impossible for this policy.
    forged.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&forged.state).unwrap())
    );
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    assert!(matches!(
        restored.create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint: forged
            }),
            ..create.clone()
        }),
        Err(Error::InvalidMessage)
    ));
    restored
        .create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint,
            }),
            ..create
        })
        .unwrap();
    assert!(
        restored
            .ledger_checkpoint()
            .state
            .operations
            .iter()
            .any(|op| op.request.operation_id == frozen.operation_id
                && op.request.body == frozen.body)
    );
}

fn binding_input(v: &mut Value) {
    let mut basic = v["basic"].clone();
    basic["settle_on_end"] = json!(true);
    v["combat_rules"]["basic_profiles"]["profile"] = basic;
    v["combat_rules"]["weapon_catalog"] = json!({
        "models":{"model":"family"},
        "families":{"family":{"base_profile":"profile"}}
    });
    v["settlement_rewards"]["track"]["equipped_weapon"] = json!(true);
    v["settlement_rewards"]["track"]["by_result"]["extract"] =
        json!({"bonus_points":8,"multiplier_bps":10000});
    v["settlement_progression"]["threshold_progress"] = json!({
        "stage_asset_id":"stage","total_asset_id":"reward","progress_asset_id":"progress",
        "thresholds":[0,5,6,9]
    });
}

#[test]
fn progression_binds_equipment_and_derives_stage_after_conversion() {
    let (catalog, c, mut ledger, create) = created_ledger(binding_input);
    let old = equipped_weapon_reward_asset_id("track", "old-instance").unwrap();
    let new = equipped_weapon_reward_asset_id("track", "new-instance").unwrap();
    let mut account = facts(2);
    assert_eq!(
        c.progression_entry_clock(&account),
        Err(Error::InvalidMessage)
    );
    account.assets.extend(
        [
            (old.as_str(), 88),
            (new.as_str(), 12),
            ("stage", 1),
            ("progress", 2),
        ]
        .into_iter()
        .map(|(id, quantity)| AssetAmount {
            asset_id: id.into(),
            quantity,
        }),
    );
    account.assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    receipt(
        &mut ledger,
        &create.key,
        "load",
        ReceiptOutcome::AssetsLoaded {
            facts: account.clone(),
        },
    );
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admitted".into(),
        },
    );
    ledger.ledger_activate();

    let mut w = world(&c);
    w.members[0].facts.revision = 1;
    for (revision, id) in [(1, "old-instance"), (2, "new-instance")] {
        w.equip_weapon(
            0,
            &WeaponEquipmentReceipt {
                operation_id: id.into(),
                equipment: WeaponEquipmentView {
                    revision,
                    weapon: Some(WeaponInstance {
                        instance_id: id.into(),
                        model_id: "model".into(),
                        morph_id: None,
                    }),
                },
                basic_profile: Some("profile".into()),
            },
            &c,
            0,
        )
        .unwrap();
    }
    w.members[1].world_weapon_profile = Some("profile".into());
    w.terminal_intent(1, Outcome::Extraction, &mut vec![]);
    let Business::CommitSettlement {
        progression,
        scoring,
        ..
    } = w.settlement(1, &c)
    else {
        panic!()
    };
    let draft = progression.unwrap();
    assert!(draft.reward_weapon_instance_id.is_none());
    assert_eq!(
        c.reward_plan_with_damage(Outcome::Extraction, scoring.as_ref(), Some(&draft), |_| 0)
            .unwrap()
            .iter()
            .map(|r| r.asset_id.as_str())
            .collect::<Vec<_>>(),
        ["reward"]
    );

    w.terminal_intent(0, Outcome::Extraction, &mut vec![]);
    ledger.ledger_settle(w.settlement(0, &c)).unwrap();
    let checkpoint = ledger.ledger_checkpoint();
    let frozen = checkpoint
        .state
        .operations
        .iter()
        .find(|op| op.request.operation_id.ends_with(":settle"))
        .unwrap()
        .request
        .clone();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &frozen.body
    else {
        panic!()
    };
    let plan = summary.progression.as_ref().unwrap();
    assert_eq!(
        plan.reward_weapon_instance_id.as_deref(),
        Some("new-instance")
    );
    let row = summary
        .rewards
        .iter()
        .find(|row| row.asset_id == new)
        .unwrap();
    assert_eq!(
        (row.balance_before, row.awarded, row.balance_after),
        (12, 8, 20)
    );
    assert!(summary.rewards.iter().all(|row| row.asset_id != old));
    assert_eq!(
        plan.threshold_progress,
        Some(ThresholdProgress {
            stage_index: 1,
            total: 6,
            current_threshold: 5,
            next_threshold: Some(6),
            balance_met: true,
            progress: BalanceChange {
                asset_id: "progress".into(),
                before: 2,
                after: 1
            },
        })
    );
    let intent = c.decode_business(&frozen.body).unwrap();
    for (index, next, met, progress) in [(0, Some(5), true, 6), (3, None, false, 0)] {
        account
            .assets
            .iter_mut()
            .find(|a| a.asset_id == "stage")
            .unwrap()
            .quantity = index;
        let view = c
            .plan_progression(&intent, &account)
            .unwrap()
            .threshold_progress
            .unwrap();
        assert_eq!(
            (
                view.stage_index,
                view.next_threshold,
                view.balance_met,
                view.progress.after
            ),
            (index, next, met, progress)
        );
    }

    let mut corrupt = checkpoint.clone();
    let op = corrupt
        .state
        .operations
        .iter_mut()
        .find(|op| op.request.operation_id == frozen.operation_id)
        .unwrap();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &mut op.request.body
    else {
        panic!()
    };
    summary
        .progression
        .as_mut()
        .unwrap()
        .threshold_progress
        .as_mut()
        .unwrap()
        .progress
        .after += 1;
    corrupt.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&corrupt.state).unwrap())
    );
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    assert!(matches!(
        restored.create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint: corrupt
            }),
            ..create.clone()
        }),
        Err(Error::InvalidMessage)
    ));
    restored
        .create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint,
            }),
            ..create
        })
        .unwrap();
    assert!(
        restored
            .ledger_checkpoint()
            .state
            .operations
            .iter()
            .any(|op| op.request.operation_id == frozen.operation_id
                && op.request.body == frozen.body)
    );
}

#[test]
fn progression_rejects_reserved_reward_keys_and_stage_writes() {
    for (change, code) in [
        (0, "CONFIG_SETTLEMENT_REWARDS_WEAPON"),
        (1, "CONFIG_PROGRESSION_THRESHOLD"),
        (2, "CONFIG_PROGRESSION_THRESHOLD"),
    ] {
        let error = catalog(input(), |v| {
            growth_input(v);
            binding_input(v);
            match change {
                0 => v["asset_contract"]["entry_cost_asset_id"] = json!("@scope/ordinary"),
                1 => {
                    v["settlement_progression"]["threshold_progress"]["thresholds"] =
                        json!([0, 5, 5])
                }
                _ => {
                    v["settlement_progression"]["conversions"][0]["changes"]["stage"] =
                        json!({"kind":"set","quantity":2})
                }
            }
        })
        .err()
        .unwrap();
        assert!(error.to_string().contains(code), "{error}");
    }
}

fn earning_input(v: &mut Value) {
    binding_input(v);
    for id in ["alternate", "temporary"] {
        v["combat_rules"]["basic_profiles"][id] =
            v["combat_rules"]["basic_profiles"]["profile"].clone();
    }
    v["pve"]["stats"]["hp"] = json!(20);
    v["pve"]["monster_positions"] = json!([[52, 20], [68, 20], [84, 20], [100, 20]]);
    let channel = &mut v["settlement_rewards"]["track"];
    channel["equipped_weapon"] = json!(false);
    channel["kill_points"] = json!({"npc":3});
    channel["by_result"]["extract"] = json!({"bonus_points":3,"multiplier_bps":10000});
    channel["damage"] = json!({
        "rate_bps":1000,"per_hit_cap_points":2,"match_cap_points":6,"rounding":"per_contact"
    });
    channel["earning_scale"] = json!({
        "default_asset_id":"rate-base","by_basic_profile":{
            "alternate":"rate-alt","temporary":"rate-temp"
        }
    });
}

#[test]
fn progression_earning_scale_follows_each_actual_contact_and_kill() {
    let (catalog, c, mut ledger, create) = created_ledger(earning_input);
    let mut account = facts(2);
    account.assets.push(AssetAmount {
        asset_id: "stage".into(),
        quantity: 0,
    });
    assert!(c.loaded_earning_rates(&account).is_none());
    for (id, quantity) in [("rate-base", 5000), ("rate-alt", 15000), ("rate-temp", 0)] {
        account.assets.push(AssetAmount {
            asset_id: id.into(),
            quantity,
        });
    }
    account.assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    let rates = c.loaded_earning_rates(&account).unwrap();
    receipt(
        &mut ledger,
        &create.key,
        "load",
        ReceiptOutcome::AssetsLoaded { facts: account },
    );
    receipt(
        &mut ledger,
        &create.key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admitted".into(),
        },
    );
    ledger.ledger_activate();
    let mut w = world(&c);
    w.members[0].facts.revision = 1;
    w.bind_reward_rates(0, rates.clone());
    w.bind_reward_rates(
        1,
        rates
            .into_iter()
            .map(|mut rate| {
                rate.bps = 0;
                rate
            })
            .collect(),
    );
    let strike = |w: &mut World, seat, npc, raw| {
        w.resolve_damage(
            DamageIntent {
                source: w.owner_entity(Owner::Member(seat)),
                target: w.owner_entity(Owner::Npc(npc)),
                raw,
                direct: true,
                ignore_armor: true,
            },
            &c,
            0,
            &mut vec![],
        )
        .hp_damage
    };
    assert_eq!(strike(&mut w, 0, 0, 9), 9);
    assert_eq!(strike(&mut w, 0, 0, 100), 11);
    w.members[0].basic_profile = Some("alternate".into());
    assert_eq!(strike(&mut w, 0, 1, 100), 20);
    w.members[0].world_weapon_profile = Some("temporary".into());
    assert_eq!(strike(&mut w, 0, 2, 100), 20);
    assert_eq!(strike(&mut w, 1, 3, 100), 20);
    assert_eq!(w.members[0].damage_growth["track"], 20_000);
    assert_eq!(w.members[1].damage_growth["track"], 0);

    for i in 0..2 {
        w.terminal_intent(i, Outcome::Extraction, &mut vec![]);
    }
    let Business::CommitSettlement {
        progression,
        scoring,
        ..
    } = w.settlement(1, &c)
    else {
        panic!()
    };
    let draft = progression.unwrap();
    let rows = c
        .reward_plan_with_damage(Outcome::Extraction, scoring.as_ref(), Some(&draft), |_| 0)
        .unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.asset_id == "track")
            .unwrap()
            .requested,
        3
    );
    ledger.ledger_settle(w.settlement(0, &c)).unwrap();
    let checkpoint = ledger.ledger_checkpoint();
    let frozen = checkpoint
        .state
        .operations
        .iter()
        .find(|op| op.request.operation_id.ends_with(":settle"))
        .unwrap()
        .request
        .clone();
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &frozen.body
    else {
        panic!()
    };
    let plan = summary.progression.as_ref().unwrap();
    let earnings = plan.reward_earnings.as_ref().unwrap();
    assert_eq!(
        earnings
            .kills
            .iter()
            .map(|row| (row.basic_profile.as_deref(), row.kills))
            .collect::<Vec<_>>(),
        [(None, 1), (Some("alternate"), 1), (Some("temporary"), 1)]
    );
    assert_eq!(
        summary
            .rewards
            .iter()
            .find(|row| row.asset_id == "track")
            .unwrap()
            .requested,
        10
    );

    let corrupt = |change: fn(&mut RewardEarnings)| {
        let mut invalid = checkpoint.clone();
        let op = invalid
            .state
            .operations
            .iter_mut()
            .find(|op| op.request.operation_id == frozen.operation_id)
            .unwrap();
        let Business::CommitAssets {
            transaction: AssetTransaction::ResultSettlement { summary, .. },
        } = &mut op.request.body
        else {
            panic!()
        };
        change(
            summary
                .progression
                .as_mut()
                .unwrap()
                .reward_earnings
                .as_mut()
                .unwrap(),
        );
        invalid.sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&invalid.state).unwrap())
        );
        invalid
    };
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    for invalid in [
        corrupt(|earning| earning.kills[0].kills += 1),
        // This rate change leaves every frozen award unchanged but differs from admission.
        corrupt(|earning| {
            earning
                .rates
                .iter_mut()
                .find(|rate| rate.asset_id == "rate-temp")
                .unwrap()
                .bps = 1
        }),
    ] {
        assert!(matches!(
            restored.create(Create {
                recovery: Some(Recovery {
                    generation: 1,
                    checkpoint: invalid
                }),
                ..create.clone()
            }),
            Err(Error::InvalidMessage)
        ));
    }
    restored
        .create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint,
            }),
            ..create
        })
        .unwrap();
    assert!(
        restored
            .ledger_checkpoint()
            .state
            .operations
            .iter()
            .any(|op| op.request.operation_id == frozen.operation_id
                && op.request.body == frozen.body)
    );
}
