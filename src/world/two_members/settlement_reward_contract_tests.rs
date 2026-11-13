use super::*;
use crate::engine::Engine;
use sha2::{Digest, Sha256};

fn reward_input(v: &mut Value) {
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["settlement_rewards"] = json!({
        "reward": {
            "kill_points":{"npc":10}, "rate_bps":7500, "balance_cap":100,
            "by_result":{
                "extract":{"bonus_points":3,"multiplier_bps":10000},
                "death":{"bonus_points":0,"multiplier_bps":5000}
            }
        },
        "track": {
            "kill_points":{"npc":7}, "rate_bps":12500, "balance_cap":200,
            "by_result":{
                "extract":{"bonus_points":2,"multiplier_bps":8000},
                "death":{"bonus_points":3,"multiplier_bps":5000}
            }
        }
    });
}

fn score(c: &Config) -> ScoreSummary {
    c.score_summary(&BTreeMap::from([("npc".into(), 1)]))
}

#[test]
fn reward_channels_use_independent_result_math_and_caps() {
    let c = catalog(input(), reward_input).unwrap().default_config();
    let tally = score(&c);
    let rows = c
        .settlement_reward_plan(Outcome::Extraction, Some(&tally), |id| {
            if id == "reward" { 97 } else { 0 }
        })
        .unwrap();
    assert_eq!(
        (
            rows[0].base_points,
            rows[0].requested,
            rows[0].awarded,
            rows[0].balance_after
        ),
        (13, 9, 3, 100)
    );
    assert_eq!(
        (
            rows[1].base_points,
            rows[1].requested,
            rows[1].awarded,
            rows[1].balance_after
        ),
        (9, 9, 9, 9)
    );

    let rows = c
        .settlement_reward_plan(Outcome::Death, Some(&tally), |id| {
            if id == "reward" { 101 } else { 0 }
        })
        .unwrap();
    assert_eq!(
        (rows[0].requested, rows[0].awarded, rows[0].balance_after),
        (3, 0, 101)
    );
    assert_eq!(
        (rows[1].requested, rows[1].awarded, rows[1].balance_after),
        (6, 6, 6)
    );
}

#[test]
fn reward_catalog_rejects_ambiguous_or_unbounded_rules() {
    for (case, error) in [
        (0, "CONFIG_SETTLEMENT_REWARDS_CAPABILITY"),
        (1, "CONFIG_SETTLEMENT_REWARDS_RULE"),
        (2, "CONFIG_SETTLEMENT_REWARDS_RULE"),
        (3, "CONFIG_SETTLEMENT_REWARDS_RULE"),
        (4, "CONFIG_SETTLEMENT_REWARDS_BOUNDS"),
    ] {
        let result = catalog(input(), |v| {
            reward_input(v);
            match case {
                0 => v["cultivation_coefficient"] = json!(1),
                1 => v["settlement_rewards"]["track"]["kill_points"]["missing"] = json!(1),
                2 => {
                    v["settlement_rewards"]["reward"]["by_result"]
                        .as_object_mut()
                        .unwrap()
                        .remove("death");
                }
                3 => v["settlement_rewards"]["item"] = v["settlement_rewards"]["track"].clone(),
                _ => {
                    v["settlement_rewards"]["track"]["by_result"]["death"]["multiplier_bps"] =
                        json!(100001)
                }
            }
        });
        assert!(result.err().unwrap().to_string().contains(error));
    }
}

pub(super) fn receipt(ledger: &mut Engine, key: &MatchKey, suffix: &str, outcome: ReceiptOutcome) {
    ledger
        .business(BusinessReceipt {
            key: key.clone(),
            operation_id: format!("{}:{}:{suffix}", key.id, key.incarnation),
            attempt_id: 1,
            outcome,
        })
        .unwrap();
}

fn admitted_reward_ledger(
    change: impl FnOnce(&mut Value),
) -> (Arc<crate::ConfigCatalog>, Arc<Config>, Engine, Create) {
    let catalog = Arc::new(
        catalog(input(), |v| {
            reward_input(v);
            change(v);
        })
        .unwrap(),
    );
    let c = catalog.default_config();
    let mut ledger = Engine::new_member_ledger(c.single_member_validation(), catalog.clone());
    let key = MatchKey {
        id: "synthetic".into(),
        incarnation: 1,
    };
    let create = Create {
        protocol_version: 3,
        key: key.clone(),
        request_id: "create".into(),
        config_id: c.id.clone(),
        config_hash: c.hash(),
        seed: 1,
        participant_id: "a".into(),
        authenticated_principal: "a".into(),
        connection: Connection {
            id: "conn".into(),
            generation: 1,
        },
        publication_mode: PublicationMode::Interactive,
        recovery: None,
    };
    ledger.create(create.clone()).unwrap();
    receipt(
        &mut ledger,
        &key,
        "load",
        ReceiptOutcome::AssetsLoaded {
            facts: AssetParticipantFacts {
                instance_count: None,
                warehouse_other_slots: None,
                location: None,
                principal: "a".into(),
                revision: 0,
                hp_points: 100,
                spirit_points: 100,
                atk_points: 10,
                def_points: 0,
                spd_points: 1,
                assets: [("entry", 4), ("item", 4), ("reward", 97), ("track", 11)]
                    .into_iter()
                    .map(|(asset_id, quantity)| AssetAmount {
                        asset_id: asset_id.into(),
                        quantity,
                    })
                    .collect(),
            },
        },
    );
    receipt(
        &mut ledger,
        &key,
        "admit",
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admit-proof".into(),
        },
    );
    ledger.ledger_activate();
    (catalog, c, ledger, create)
}

fn draft_settlement(c: &Config) -> Business {
    let mut w = world(c);
    w.members[0].tally.insert("npc".into(), 1);
    w.members[0].facts.revision = 1;
    w.terminal_intent(0, Outcome::Extraction, &mut vec![]);
    w.settlement(0, c)
}

#[test]
fn mode_result_and_reward_resume_from_the_frozen_member_ledger() {
    let (catalog, c, mut ledger, create) = admitted_reward_ledger(|v| {
        super::match_mode_contract_tests::contest_input(v);
        v["pve"]
            .as_object_mut()
            .unwrap()
            .remove("status_definitions");
        for asset in ["reward", "track"] {
            for (result, points) in [("passed", 9), ("failed", 2), ("tied", 4)] {
                v["settlement_rewards"][asset]["by_result"][result] =
                    json!({"bonus_points": points, "multiplier_bps": 10000});
            }
        }
    });
    let mut w = world(&c);
    w.members[0].facts.revision = 1;
    w.tick(&c, 600);
    assert_eq!(w.members[0].desired, Some(Outcome::Draw));
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
    assert!(matches!(&frozen.body, Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. }
    } if summary.result.result_id=="tied" && summary.rewards[0].awarded==3));
    let key = create.key.clone();
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    restored
        .create(Create {
            recovery: Some(Recovery {
                generation: 1,
                checkpoint,
            }),
            ..create
        })
        .unwrap();
    assert!(restored.world.is_none());
    receipt(
        &mut restored,
        &key,
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
fn settlement_recovery_checks_the_original_reward_baseline_and_reuses_frozen_request() {
    let (catalog, c, mut ledger, create) = admitted_reward_ledger(|_| {});
    let key = create.key.clone();
    ledger.ledger_settle(draft_settlement(&c)).unwrap();
    let checkpoint = ledger.ledger_checkpoint();
    let op = checkpoint
        .state
        .operations
        .iter()
        .find(|op| op.request.operation_id.ends_with(":settle"))
        .unwrap();
    assert!(matches!(&op.request.body, Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement {summary,credits,..}
    } if summary.rewards[1].balance_after == 20
        && credits.iter().map(|row| (row.asset_id.as_str(), row.quantity)).collect::<Vec<_>>()
            == vec![("item",1),("reward",3),("track",9)]));
    let frozen_request = op.request.clone();

    let mut forged = checkpoint.clone();
    let op = forged
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
    summary.rewards[1].balance_before += 1;
    summary.rewards[1].balance_after += 1;
    assert!(c.decode_business(&op.request.body).is_ok());
    forged.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&forged.state).unwrap())
    );

    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    let restore = |checkpoint| Create {
        recovery: Some(Recovery {
            generation: 1,
            checkpoint,
        }),
        ..create.clone()
    };
    assert_eq!(restored.create(restore(forged)), Err(Error::InvalidMessage));
    restored.create(restore(checkpoint)).unwrap();
    let pending = restored
        .drain()
        .into_iter()
        .filter_map(|o| match o.body {
            OutputBody::BusinessRequest { request } => Some(request.body),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(&pending[..], [Business::QueryOperation {operation_id}] if operation_id.ends_with(":settle"))
    );
    receipt(
        &mut restored,
        &key,
        "query-settle-1",
        ReceiptOutcome::NotCommitted {},
    );
    let retry = restored
        .drain()
        .into_iter()
        .find_map(|o| match o.body {
            OutputBody::BusinessRequest { request } => Some(request),
            _ => None,
        })
        .unwrap();
    assert_eq!(retry.operation_id, frozen_request.operation_id);
    assert_eq!(retry.attempt_id, 2);
    assert_eq!(retry.body, frozen_request.body);
}

pub(super) fn requests(ledger: &mut Engine) -> Vec<BusinessRequest> {
    ledger
        .drain()
        .into_iter()
        .filter_map(|o| match o.body {
            OutputBody::BusinessRequest { request } => Some(request),
            _ => None,
        })
        .collect()
}

#[test]
fn facts_retry_and_recover_independently_of_the_committed_settlement() {
    let (catalog, c, mut ledger, create) = admitted_reward_ledger(|v| {
        v["settlement_facts"] = json!({"extract":["second-fact","first-fact"]});
    });
    let key = &create.key;
    requests(&mut ledger);
    ledger.ledger_settle(draft_settlement(&c)).unwrap();
    assert!(matches!(
        requests(&mut ledger).as_slice(),
        [BusinessRequest {
            body: Business::CommitAssets { .. },
            ..
        }]
    ));
    receipt(&mut ledger, key, "settle", ReceiptOutcome::Refused {});
    assert!(!ledger.ledger_settled());
    assert!(matches!(
        requests(&mut ledger).as_slice(),
        [BusinessRequest {
            body: Business::ReconcileOperation { .. },
            ..
        }]
    ));
    receipt(
        &mut ledger,
        key,
        "reconcile-settle",
        ReceiptOutcome::Reconciled {
            revision: 2,
            commit_id: "settle-proof".into(),
        },
    );
    assert!(ledger.ledger_settled());
    let first = requests(&mut ledger).pop().unwrap();
    assert_eq!(
        first.body,
        Business::AppendFacts {
            settlement_operation_id: "synthetic:1:settle".into(),
            result_id: "extract".into(),
            fact_ids: vec!["first-fact".into(), "second-fact".into()]
        }
    );
    receipt(&mut ledger, key, "facts", ReceiptOutcome::NotCommitted {});
    let retry = requests(&mut ledger).pop().unwrap();
    assert_eq!(retry.attempt_id, 2);
    assert_eq!(retry.operation_id, first.operation_id);
    assert_eq!(retry.body, first.body);

    let checkpoint = ledger.ledger_checkpoint();
    let restore = |checkpoint| Create {
        recovery: Some(Recovery {
            generation: 1,
            checkpoint,
        }),
        ..create.clone()
    };
    let mut restored = Engine::new_member_ledger(c.single_member_validation(), catalog);
    for case in 0..3 {
        let mut forged = checkpoint.clone();
        let operations = &mut forged.state.operations;
        let at = operations
            .iter()
            .position(|op| op.request.operation_id == first.operation_id)
            .unwrap();
        match case {
            0 => {
                let Business::AppendFacts { fact_ids, .. } = &mut operations[at].request.body
                else {
                    panic!()
                };
                fact_ids[0] = "foreign".into();
            }
            1 => {
                operations
                    .iter_mut()
                    .find(|op| op.request.operation_id.ends_with(":settle"))
                    .unwrap()
                    .terminal = None;
            }
            _ => {
                operations.remove(at);
            }
        }
        forged.sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&forged.state).unwrap())
        );
        assert_eq!(restored.create(restore(forged)), Err(Error::InvalidMessage));
    }
    restored.create(restore(checkpoint)).unwrap();
    assert!(
        matches!(requests(&mut restored).as_slice(), [BusinessRequest {
        body:Business::QueryOperation {operation_id}, ..}] if operation_id == &first.operation_id)
    );
    receipt(
        &mut restored,
        key,
        "query-facts-2",
        ReceiptOutcome::FactsAppended {
            commit_id: "fact-proof".into(),
        },
    );
    assert!(restored.ledger_settled());
    assert_eq!(restored.ledger_pending(), 0);
    assert!(requests(&mut restored).is_empty());
}

#[test]
fn fact_routes_reject_duplicate_ids_and_unknown_results() {
    for routes in [
        json!({"extract":["same","same"]}),
        json!({"unknown":["fact"]}),
    ] {
        let result = catalog(input(), |v| {
            reward_input(v);
            v["settlement_facts"] = routes;
        });
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("CONFIG_SETTLEMENT_FACTS_RULE")
        );
    }
}
