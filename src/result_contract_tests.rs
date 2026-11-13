use super::*;
use crate::{asset_contract::tests as support, engine::Engine, world::World};
use support::{
    Package, admitted_with_config, create, facts, receipt, requests, resign, settlement_with_config,
};

fn config() -> Config {
    let mut c = support::config();
    c.schema_version = 4;
    c.result_contract = Some(ResultContract {
        extraction: MatchResult {
            result_id: "secured".into(),
            policy: SettlementPolicy::Preserve,
        },
        death: MatchResult {
            result_id: "defeated".into(),
            policy: SettlementPolicy::Attrition,
        },
    });
    c
}
fn restore(c: Config, cp: IoCheckpoint) -> Result<Engine> {
    let mut e = Engine::new(c.clone());
    let mut r = create(&c);
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    e.create(r)?;
    Ok(e)
}
fn summary(request: &BusinessRequest) -> &ResultSettlement {
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { summary, .. },
    } = &request.body
    else {
        panic!("result settlement required")
    };
    summary
}

#[test]
fn explicit_result_schema_loads_and_reports_supported_protocol() {
    let p = Package::new();
    let v = serde_json::from_str(crate::test_data::fixture!(
        "examples/result-host/arena.json"
    ))
    .unwrap();
    p.write(&v);
    let catalog = p.load().unwrap();
    assert_eq!(catalog.default_config().protocol_version(), 3);
    assert_eq!(catalog.describe()["versions"][0]["protocolVersion"], 3);
}

#[test]
fn invalid_result_ids_policies_triggers_and_schema_combinations_fail_startup() {
    let p = Package::new();
    let original: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/result-host/arena.json"
    ))
    .unwrap();
    for case in 0..12 {
        let mut v = original.clone();
        match case {
            0 => v["result_contract"]["death"]["result_id"] = serde_json::json!(""),
            1 => v["result_contract"]["death"]["result_id"] = serde_json::json!("x".repeat(65)),
            2 => v["result_contract"]["death"]["result_id"] = serde_json::json!("bad\nresult"),
            3 => {
                v["result_contract"]["death"]["result_id"] =
                    v["result_contract"]["extraction"]["result_id"].clone()
            }
            4 => v["result_contract"]["death"]["policy"] = serde_json::json!("win_everything"),
            5 => v["result_contract"]["victory"] = v["result_contract"]["death"].clone(),
            6 => {
                v["result_contract"]
                    .as_object_mut()
                    .unwrap()
                    .remove("death");
            }
            7 => v["schema_version"] = serde_json::json!(3),
            8 => {
                v.as_object_mut().unwrap().remove("asset_contract");
            }
            9 => {
                v.as_object_mut().unwrap().remove("result_contract");
            }
            10 => v["result_contract"]["death"]["amount"] = serde_json::json!(100),
            _ => v["result_contract"] = serde_json::Value::Null,
        }
        p.write(&v);
        assert!(p.load().is_err(), "case {case}");
    }
}

#[test]
fn old_protocol_is_rejected_without_partial_initialization() {
    let c = config();
    let mut e = Engine::new(c.clone());
    for protocol in [1, 2] {
        let mut r = create(&c);
        r.protocol_version = protocol;
        assert_eq!(e.create(r), Err(Error::VersionMismatch));
        assert!(e.drain().is_empty());
    }
    e.create(create(&c)).unwrap();
    assert_eq!(requests(&e.drain())[0].body, Business::LoadAssets {});
}

#[test]
fn default_death_result_has_generic_identity_and_matches_prior_amounts() {
    let (_, r, _) = settlement_with_config(config());
    let s = summary(&r);
    assert_eq!(s.result, config().result_contract.unwrap().death);
    assert_eq!((s.inventory[0].kept, s.inventory[0].dropped), (3, 2));
    let body = serde_json::to_string(&r.body).unwrap();
    assert!(!body.contains("outcome"));
    assert!(!body.contains("cultivation"));
    assert!(!body.contains("entry_years"));
    assert_eq!(
        config().decode_business(&r.body).unwrap(),
        support::config()
            .decode_business(&support::settlement().1.body)
            .unwrap()
    );
}

#[test]
fn every_trigger_can_independently_choose_either_registered_policy() {
    for extraction in [SettlementPolicy::Preserve, SettlementPolicy::Attrition] {
        for death in [SettlementPolicy::Preserve, SettlementPolicy::Attrition] {
            let mut c = config();
            let rules = c.result_contract.as_mut().unwrap();
            rules.extraction.policy = extraction;
            rules.death.policy = death;
            for trigger in [Outcome::Extraction, Outcome::Death] {
                let mut f = c.loaded_facts(&facts()).unwrap();
                f.revision = 1;
                let mut w = World::new(&c, f, 7);
                w.desired_outcome = Some(trigger);
                let old = w.settlement(&c);
                let Business::CommitSettlement {
                    outcome,
                    kept,
                    dropped,
                    cultivation,
                    ..
                } = old
                else {
                    panic!()
                };
                assert_eq!(outcome, trigger);
                let policy = c.result_binding(trigger).unwrap().policy;
                assert_eq!(
                    (kept, dropped, cultivation),
                    if policy == SettlementPolicy::Preserve {
                        (5, 0, 22)
                    } else {
                        (3, 2, 0)
                    }
                );
                let wire = c.encode_business(&old);
                assert_eq!(c.decode_business(&wire).unwrap(), old);
            }
        }
    }
}

#[test]
fn renaming_result_changes_identity_without_changing_assets_or_rules() {
    let (_, a, _) = settlement_with_config(config());
    let mut c = config();
    c.result_contract.as_mut().unwrap().death.result_id = "session-ended".into();
    let (_, b, _) = settlement_with_config(c);
    let Business::CommitAssets {
        transaction: AssetTransaction::ResultSettlement { credits: a, .. },
    } = a.body
    else {
        panic!()
    };
    let Business::CommitAssets {
        transaction:
            AssetTransaction::ResultSettlement {
                credits: b,
                summary,
                ..
            },
    } = b.body
    else {
        panic!()
    };
    assert_eq!(a, b);
    assert_eq!(summary.result.result_id, "session-ended");
}

#[test]
fn unknown_query_and_fenced_retry_keep_exact_result_policy_and_operation_id() {
    let (mut e, original, _) = settlement_with_config(config());
    let c = e.config.clone();
    e.business(receipt(&c, &original, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = requests(&e.drain()).remove(0);
    e.business(receipt(&c, &query, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    let retry = requests(&e.drain()).remove(0);
    assert_eq!(retry.body, original.body);
    assert_eq!(retry.operation_id, original.operation_id);
    assert_eq!(retry.attempt_id, 2);
    let proof = ReceiptOutcome::Committed {
        revision: 2,
        commit_id: "same-proof".into(),
    };
    e.business(receipt(&c, &retry, proof.clone())).unwrap();
    e.business(receipt(&c, &retry, proof)).unwrap();
    let outputs = e.drain();
    assert_eq!(
        outputs
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::ResultSettlementApplied { .. }
                }
            ))
            .count(),
        1
    );
    assert_eq!((e.live_worlds(), e.pending_count()), (0, 0));
}

#[test]
fn refusal_requires_original_reconciliation_and_result_is_applied_once() {
    let (mut e, original, _) = settlement_with_config(config());
    let c = e.config.clone();
    e.business(receipt(&c, &original, ReceiptOutcome::Refused {}))
        .unwrap();
    let review = requests(&e.drain()).remove(0);
    assert_eq!(
        e.business(receipt(
            &c,
            &original,
            ReceiptOutcome::Committed {
                revision: 2,
                commit_id: "bypass".into()
            }
        )),
        Err(Error::Conflict)
    );
    e.close(Close {
        key: create(&c).key,
        request_id: "close".into(),
    })
    .unwrap();
    assert!(e.pending_count() > 0);
    e.business(receipt(
        &c,
        &review,
        ReceiptOutcome::Reconciled {
            revision: 2,
            commit_id: "reviewed".into(),
        },
    ))
    .unwrap();
    assert_eq!((e.live_worlds(), e.pending_count()), (0, 0));
}

#[test]
fn closed_world_still_finishes_the_original_frozen_result() {
    let (mut e, r, _) = settlement_with_config(config());
    let c = e.config.clone();
    let close = Close {
        key: create(&c).key,
        request_id: "close".into(),
    };
    let a = e.close(close.clone()).unwrap();
    assert_eq!(e.close(close).unwrap(), a);
    assert_eq!((e.live_worlds(), e.pending_count()), (0, 1));
    assert!(requests(&e.drain()).is_empty());
    e.business(receipt(
        &c,
        &r,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "late".into(),
        },
    ))
    .unwrap();
    assert_eq!(e.pending_count(), 0);
}

#[test]
fn preparing_close_refunds_admission_without_a_result_settlement() {
    let mut c = config();
    c.result_contract.as_mut().unwrap().death.policy = SettlementPolicy::Preserve;
    let mut e = admitted_with_config(c.clone());
    e.close(Close {
        key: create(&c).key,
        request_id: "close".into(),
    })
    .unwrap();
    let r = requests(&e.drain()).remove(0);
    assert!(matches!(
        r.body,
        Business::CommitAssets {
            transaction: AssetTransaction::Compensation { .. }
        }
    ));
}

#[test]
fn recovery_queries_existing_result_without_world_or_recalculation() {
    let (_, r, cp) = settlement_with_config(config());
    let mut e = restore(config(), cp).unwrap();
    assert_eq!(e.live_worlds(), 0);
    let query = requests(&e.drain()).remove(0);
    assert_eq!(
        query.body,
        Business::QueryOperation {
            operation_id: r.operation_id
        }
    );
    let c = e.config.clone();
    e.business(receipt(
        &c,
        &query,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "persisted".into(),
        },
    ))
    .unwrap();
    assert_eq!(e.pending_count(), 0);
}

#[test]
fn resealed_unknown_result_wrong_policy_and_invalid_amounts_do_not_restore() {
    let (_, r, cp) = settlement_with_config(config());
    for case in 0..7 {
        let mut bad = cp.clone();
        let op = bad
            .state
            .operations
            .iter_mut()
            .find(|o| o.request.operation_id == r.operation_id)
            .unwrap();
        let Business::CommitAssets {
            transaction:
                AssetTransaction::ResultSettlement {
                    summary, credits, ..
                },
        } = &mut op.request.body
        else {
            panic!()
        };
        match case {
            0 => summary.result.result_id = "unknown".into(),
            1 => summary.result.policy = SettlementPolicy::Preserve,
            2 => summary.result = config().result_contract.unwrap().extraction,
            3 => summary.inventory[0].kept += 1,
            4 => credits[0].quantity += 1,
            5 => credits.push(credits[0].clone()),
            _ => summary.inventory[0].dropped = 0,
        }
        resign(&mut bad);
        assert!(
            matches!(restore(config(), bad), Err(Error::InvalidMessage)),
            "case {case}"
        );
    }
    assert!(restore(config(), cp).is_ok());
}

#[test]
fn protocol_two_and_three_settlement_bodies_cannot_be_mixed() {
    let (_, r, mut cp) = settlement_with_config(config());
    let s = summary(&r);
    let op = cp
        .state
        .operations
        .iter_mut()
        .find(|o| o.request.operation_id == r.operation_id)
        .unwrap();
    let Business::CommitAssets {
        transaction:
            AssetTransaction::ResultSettlement {
                expected_revision,
                credits,
                ..
            },
    } = &r.body
    else {
        panic!()
    };
    op.request.body = Business::CommitAssets {
        transaction: AssetTransaction::Settlement {
            expected_revision: *expected_revision,
            credits: credits.clone(),
            summary: s.to_asset(Outcome::Death),
        },
    };
    resign(&mut cp);
    assert!(matches!(restore(config(), cp), Err(Error::InvalidMessage)));
    assert_eq!(
        support::config().decode_business(&r.body),
        Err(Error::InvalidMessage)
    );
}

#[test]
fn invalid_result_checkpoint_does_not_prevent_valid_recovery_in_same_engine() {
    let (_, _, cp) = settlement_with_config(config());
    let c = config();
    let mut e = Engine::new(c.clone());
    let mut bad = cp.clone();
    bad.sha256 = "0".repeat(64);
    let mut r = create(&c);
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: bad,
    });
    assert_eq!(e.create(r), Err(Error::InvalidMessage));
    assert!(e.drain().is_empty());
    let mut r = create(&c);
    r.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    assert_eq!(e.create(r).unwrap().phase, Phase::Closed);
}

#[test]
fn wrong_revision_does_not_apply_or_rewrite_frozen_result() {
    let (mut e, r, _) = settlement_with_config(config());
    let c = e.config.clone();
    assert_eq!(
        e.business(receipt(
            &c,
            &r,
            ReceiptOutcome::Committed {
                revision: 3,
                commit_id: "bad".into()
            }
        )),
        Err(Error::RevisionConflict)
    );
    assert!(e.drain().is_empty());
    assert_eq!(e.pending_count(), 1);
    e.business(receipt(
        &c,
        &r,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "good".into(),
        },
    ))
    .unwrap();
    assert_eq!(e.pending_count(), 0);
}

#[test]
fn client_cannot_set_result_policy_or_amount() {
    for v in [
        serde_json::json!({"kind":"leave","result_id":"secured"}),
        serde_json::json!({"kind":"set_result","policy":"preserve"}),
        serde_json::json!({"kind":"extract_start","amount":100}),
    ] {
        assert!(serde_json::from_value::<Action>(v).is_err());
    }
    assert!(serde_json::from_value::<ResultSettlement>(serde_json::json!({"result":{"result_id":"x","policy":"preserve"},"inventory":[],"outcome":"death"})).is_err());
}
