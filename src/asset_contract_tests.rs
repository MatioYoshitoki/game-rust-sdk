use super::*;
use crate::engine::Engine;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(crate) struct Package(pub(crate) PathBuf);
impl Package {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "asset-contract-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("catalog.json"),r#"{"schema_version":1,"default_scenario":"arena","active":{"arena":"arena.json"},"archives":[]}"#).unwrap();
        Self(path)
    }
    pub(crate) fn write(&self, value: &serde_json::Value) {
        fs::write(
            self.0.join("arena.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
    }
    pub(crate) fn load(&self) -> std::io::Result<crate::ConfigCatalog> {
        crate::ConfigCatalog::load(self.0.join("catalog.json"))
    }
}
impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn asset_configuration_loads_with_explicit_schema_and_validated_bindings() {
    let package = Package::new();
    let value: serde_json::Value =
        serde_json::from_str(crate::test_data::fixture!("examples/asset-host/arena.json")).unwrap();
    package.write(&value);
    let catalog = package.load().unwrap();
    assert_eq!(catalog.default_config().protocol_version(), 2);
    assert_eq!(
        catalog.describe()["versions"][0]["config"]["asset_contract"],
        value["asset_contract"]
    );
}

#[test]
fn invalid_asset_bindings_schema_and_unknown_fields_fail_startup() {
    let package = Package::new();
    let source: serde_json::Value =
        serde_json::from_str(crate::test_data::fixture!("examples/asset-host/arena.json")).unwrap();
    for case in 0..8 {
        let mut v = source.clone();
        match case {
            0 => v["schema_version"] = serde_json::json!(2),
            1 => {
                v.as_object_mut().unwrap().remove("asset_contract");
            }
            2 => {
                v["asset_contract"]["reward_asset_id"] =
                    v["asset_contract"]["entry_cost_asset_id"].clone()
            }
            3 => v["asset_contract"]["entry_cost_asset_id"] = v["item_id"].clone(),
            4 => v["asset_contract"]["reward_asset_id"] = serde_json::json!(""),
            5 => v["asset_contract"]["extra"] = serde_json::json!(1),
            6 => v["asset_contract"]["entry_cost_asset_id"] = serde_json::json!("x".repeat(65)),
            _ => v["asset_contract"]["reward_asset_id"] = serde_json::json!("invalid\nasset"),
        }
        package.write(&v);
        assert!(package.load().is_err(), "case {case}");
    }
}

#[test]
fn asset_bindings_cannot_alias_additional_pve_inventory_items() {
    let package = Package::new();
    let mut v: serde_json::Value =
        serde_json::from_str(crate::test_data::fixture!("config/b3-1.json")).unwrap();
    v["schema_version"] = serde_json::json!(3);
    let item = v["pve"]["items"]
        .as_object()
        .unwrap()
        .keys()
        .find(|id| *id != v["item_id"].as_str().unwrap())
        .unwrap()
        .clone();
    v["asset_contract"] =
        serde_json::json!({"entry_cost_asset_id":"ticket","reward_asset_id":item});
    package.write(&v);
    assert!(
        package
            .load()
            .err()
            .unwrap()
            .to_string()
            .contains("CONFIG_ASSET_CONTRACT")
    );
}

pub(crate) fn config() -> Config {
    let mut c = Config::frozen();
    c.schema_version = 3;
    c.id = "asset-test".into();
    c.loaded_hash = "f".repeat(64);
    c.asset_contract = Some(AssetContract {
        entry_cost_asset_id: "entry-credit".into(),
        reward_asset_id: "experience".into(),
    });
    c
}
pub(crate) fn create(c: &Config) -> Create {
    Create {
        protocol_version: c.protocol_version(),
        key: MatchKey {
            id: "asset-test".into(),
            incarnation: 1,
        },
        request_id: "create".into(),
        config_id: c.id().into(),
        config_hash: c.content_hash().into(),
        seed: 7,
        participant_id: "A".into(),
        authenticated_principal: "principal-A".into(),
        connection: Connection {
            id: "connection".into(),
            generation: 1,
        },
        publication_mode: PublicationMode::Batched,
        recovery: None,
    }
}
pub(crate) fn facts() -> ReceiptOutcome {
    ReceiptOutcome::AssetsLoaded {
        facts: AssetParticipantFacts {
            instance_count: None,
            warehouse_other_slots: None,
            location: None,
            principal: "principal-A".into(),
            revision: 0,
            hp_points: 1000,
            spirit_points: 100,
            atk_points: 100,
            def_points: 10,
            spd_points: 8,
            assets: vec![amount("entry-credit", 100), amount("xuan-tie", 100)],
        },
    }
}
pub(crate) fn requests(outputs: &[Output]) -> Vec<BusinessRequest> {
    outputs
        .iter()
        .filter_map(|o| {
            if let OutputBody::BusinessRequest { request } = &o.body {
                Some(request.clone())
            } else {
                None
            }
        })
        .collect()
}
pub(crate) fn receipt(c: &Config, r: &BusinessRequest, outcome: ReceiptOutcome) -> BusinessReceipt {
    BusinessReceipt {
        key: create(c).key,
        operation_id: r.operation_id.clone(),
        attempt_id: r.attempt_id,
        outcome,
    }
}
pub(crate) fn checkpoint(outputs: &[Output]) -> IoCheckpoint {
    outputs
        .iter()
        .rev()
        .find_map(|o| o.recovery.as_deref())
        .unwrap()
        .clone()
}
pub(crate) fn resign(cp: &mut IoCheckpoint) {
    cp.sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&cp.state).unwrap())
    );
}
pub(crate) fn input(e: &mut Engine, sequence: u32, action: Action) {
    let c = create(&e.config);
    e.input(Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: c.connection,
        request_id: format!("input-{sequence}"),
        sequence,
        action,
    })
    .unwrap();
}
pub(crate) fn admitted() -> Engine {
    admitted_with_config(config())
}
pub(crate) fn admitted_with_config(c: Config) -> Engine {
    let mut e = Engine::new(c.clone());
    e.create(create(&c)).unwrap();
    let load = requests(&e.drain()).remove(0);
    e.business(receipt(&c, &load, facts())).unwrap();
    let admit = requests(&e.drain()).remove(0);
    e.business(receipt(
        &c,
        &admit,
        ReceiptOutcome::Committed {
            revision: 1,
            commit_id: "admit-1".into(),
        },
    ))
    .unwrap();
    e
}
pub(crate) fn settlement() -> (Engine, BusinessRequest, IoCheckpoint) {
    settlement_with_config(config())
}
pub(crate) fn settlement_with_config(c: Config) -> (Engine, BusinessRequest, IoCheckpoint) {
    let mut e = admitted_with_config(c);
    input(&mut e, 1, Action::Join {});
    let publication = e
        .drain()
        .into_iter()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap();
    let c = create(&e.config);
    e.transport(TransportReceipt {
        key: c.key,
        output_id: publication.id.clone(),
        recipient_id: c.participant_id,
        connection: c.connection,
        outcome: TransportOutcome::Sent,
    })
    .unwrap();
    input(
        &mut e,
        2,
        Action::CombatReady {
            publication_id: publication.id,
        },
    );
    e.tick();
    e.drain();
    input(&mut e, 3, Action::Leave {});
    e.tick();
    let outputs = e.drain();
    let request = requests(&outputs).remove(0);
    let cp = checkpoint(&outputs);
    (e, request, cp)
}
pub(crate) fn restored(cp: IoCheckpoint) -> Result<Engine> {
    let c = config();
    let mut e = Engine::new(c.clone());
    let mut request = create(&c);
    request.recovery = Some(Recovery {
        generation: 1,
        checkpoint: cp,
    });
    e.create(request)?;
    Ok(e)
}

#[test]
fn schema_and_protocol_are_selected_together_without_fallback() {
    let c = config();
    let mut e = Engine::new(c.clone());
    let mut r = create(&c);
    r.protocol_version = 1;
    assert_eq!(e.create(r), Err(Error::VersionMismatch));
    assert!(e.drain().is_empty());
    e.create(create(&c)).unwrap();
    assert_eq!(requests(&e.drain())[0].body, Business::LoadAssets {});
    let legacy = Config::frozen();
    let mut e = Engine::new(legacy.clone());
    let mut r = create(&legacy);
    r.protocol_version = 2;
    assert_eq!(e.create(r), Err(Error::VersionMismatch));
}

#[test]
fn asset_facts_validate_cost_primary_identity_and_canonical_rows_before_debit() {
    let c = config();
    let mut e = Engine::new(c.clone());
    e.create(create(&c)).unwrap();
    let load = requests(&e.drain()).remove(0);
    for (change, error) in [
        (0, Error::NoResource),
        (1, Error::InvalidMessage),
        (2, Error::InvalidMessage),
        (3, Error::Unauthorized),
        (4, Error::NoResource),
    ] {
        let ReceiptOutcome::AssetsLoaded { mut facts } = facts() else {
            unreachable!()
        };
        match change {
            0 => facts.assets[0].quantity = 0,
            1 => facts.assets.reverse(),
            2 => facts.assets.push(facts.assets[1].clone()),
            3 => facts.principal = "other".into(),
            _ => facts.assets[1].quantity = 0,
        }
        assert_eq!(
            e.business(receipt(&c, &load, ReceiptOutcome::AssetsLoaded { facts })),
            Err(error)
        );
        assert!(requests(&e.drain()).is_empty());
    }
    assert_eq!(
        e.business(receipt(
            &c,
            &load,
            ReceiptOutcome::Loaded {
                facts: ParticipantFacts {
                    principal: "principal-A".into(),
                    revision: 0,
                    hp_points: 1000,
                    spirit_points: 100,
                    atk_points: 100,
                    def_points: 10,
                    spd_points: 8,
                    warehouse_quantity: 100
                }
            }
        )),
        Err(Error::InvalidMessage)
    );
    e.business(receipt(&c, &load, facts())).unwrap();
    let request = requests(&e.drain()).remove(0);
    assert!(matches!(
        request.body,
        Business::CommitAssets {
            transaction: AssetTransaction::Admission { .. }
        }
    ));
}

#[test]
fn configured_mapping_emits_only_canonical_positive_assets_and_preserves_zero_reward() {
    let c = config();
    let old = Business::CommitSettlement {
        outcome: Outcome::Death,
        expected_revision: 1,
        item_id: c.item_id.clone(),
        carried: 5,
        looted: 0,
        kept: 3,
        dropped: 2,
        cultivation: 0,
        items: vec![],
        instances: vec![],
        rewards: vec![],
        progression: None,
        scoring: None,
        map_progress: None,
        reset_progress: None,
    };
    let wire = c.encode_business(&old);
    assert_eq!(c.decode_business(&wire).unwrap(), old);
    let Business::CommitAssets {
        transaction: AssetTransaction::Settlement { credits, .. },
    } = wire
    else {
        panic!()
    };
    assert_eq!(credits, vec![amount("xuan-tie", 3)]);
    let mut other = c.clone();
    other.asset_contract.as_mut().unwrap().entry_cost_asset_id = "ticket".into();
    let admit = Business::CommitAdmission {
        expected_revision: 0,
        item_id: "xuan-tie".into(),
        quantity: 5,
        entry_years: 1,
        clock: None,
    };
    assert_ne!(c.encode_business(&admit), other.encode_business(&admit));
}

#[test]
fn unknown_query_and_fenced_retry_reuse_exact_generic_body_and_identity() {
    let (mut e, original, _) = settlement();
    let c = e.config.clone();
    e.business(receipt(&c, &original, ReceiptOutcome::Unknown {}))
        .unwrap();
    let query = requests(&e.drain()).remove(0);
    assert_eq!(
        query.body,
        Business::QueryOperation {
            operation_id: original.operation_id.clone()
        }
    );
    e.business(receipt(&c, &query, ReceiptOutcome::NotCommitted {}))
        .unwrap();
    let retry = requests(&e.drain()).remove(0);
    assert_eq!(retry.body, original.body);
    assert_eq!(retry.operation_id, original.operation_id);
    assert_eq!(retry.attempt_id, 2);
    let proof = ReceiptOutcome::Committed {
        revision: 2,
        commit_id: "settle-2".into(),
    };
    e.business(receipt(&c, &retry, proof.clone())).unwrap();
    e.business(receipt(&c, &retry, proof)).unwrap();
    let trace = e.drain();
    assert_eq!(
        trace
            .iter()
            .filter(|o| matches!(
                o.body,
                OutputBody::Event {
                    event: Event::AssetSettlementApplied { .. }
                }
            ))
            .count(),
        1
    );
    assert_eq!((e.live_worlds(), e.pending_count()), (0, 0));
}

#[test]
fn refused_settlement_requires_original_reconciliation_proof() {
    let (mut e, original, _) = settlement();
    let c = e.config.clone();
    e.business(receipt(&c, &original, ReceiptOutcome::Refused {}))
        .unwrap();
    let reconcile = requests(&e.drain()).remove(0);
    assert_eq!(
        reconcile.body,
        Business::ReconcileOperation {
            operation_id: original.operation_id.clone()
        }
    );
    assert_eq!(
        e.business(receipt(
            &c,
            &original,
            ReceiptOutcome::Committed {
                revision: 2,
                commit_id: "wrong-path".into()
            }
        )),
        Err(Error::Conflict)
    );
    e.business(receipt(
        &c,
        &reconcile,
        ReceiptOutcome::Reconciled {
            revision: 2,
            commit_id: "reviewed-2".into(),
        },
    ))
    .unwrap();
    assert_eq!((e.live_worlds(), e.pending_count()), (0, 0));
}

#[test]
fn unsettled_recovery_queries_original_transaction_without_building_world() {
    let (_, original, cp) = settlement();
    let mut e = restored(cp).unwrap();
    assert_eq!(e.live_worlds(), 0);
    let query = requests(&e.drain()).remove(0);
    assert_eq!(
        query.body,
        Business::QueryOperation {
            operation_id: original.operation_id
        }
    );
    let c = e.config.clone();
    e.business(receipt(
        &c,
        &query,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "old-commit".into(),
        },
    ))
    .unwrap();
    assert_eq!(e.pending_count(), 0);
}

#[test]
fn changed_settlement_assets_summary_and_order_are_rejected_even_with_resealed_checkpoint() {
    let (_, _, cp) = settlement();
    for change in 0..7 {
        let mut altered = cp.clone();
        let op = altered
            .state
            .operations
            .iter_mut()
            .find(|op| {
                matches!(
                    op.request.body,
                    Business::CommitAssets {
                        transaction: AssetTransaction::Settlement { .. }
                    }
                )
            })
            .unwrap();
        let Business::CommitAssets {
            transaction:
                AssetTransaction::Settlement {
                    credits, summary, ..
                },
        } = &mut op.request.body
        else {
            panic!()
        };
        match change {
            0 => credits[0].quantity += 1,
            1 => credits[0].asset_id = "other-asset".into(),
            2 => credits.push(credits[0].clone()),
            3 => summary.inventory[0].kept += 1,
            4 => summary.inventory[0].carried += 1,
            5 => credits.push(amount("experience", 1)),
            _ => summary.inventory[0].item_id = "other-item".into(),
        }
        resign(&mut altered);
        assert!(
            matches!(restored(altered), Err(Error::InvalidMessage)),
            "change {change}"
        );
    }
    assert!(restored(cp).is_ok());
}

#[test]
fn old_body_cannot_be_reinterpreted_as_generic_transaction_on_recovery() {
    let (_, _, mut cp) = settlement();
    let op = cp
        .state
        .operations
        .iter_mut()
        .find(|op| {
            matches!(
                op.request.body,
                Business::CommitAssets {
                    transaction: AssetTransaction::Admission { .. }
                }
            )
        })
        .unwrap();
    op.request.body = config().decode_business(&op.request.body).unwrap();
    resign(&mut cp);
    assert!(matches!(restored(cp), Err(Error::InvalidMessage)));
}

#[test]
fn compensation_has_exact_original_debits_and_wrong_reference_cannot_restore() {
    let mut e = admitted();
    let c = e.config.clone();
    e.close(Close {
        key: create(&c).key,
        request_id: "close".into(),
    })
    .unwrap();
    let outputs = e.drain();
    let compensation = requests(&outputs).remove(0);
    let Business::CommitAssets {
        transaction: AssetTransaction::Compensation { credits, .. },
    } = &compensation.body
    else {
        panic!()
    };
    assert_eq!(
        *credits,
        vec![amount("entry-credit", 1), amount("xuan-tie", 5)]
    );
    let mut cp = checkpoint(&outputs);
    let op = cp
        .state
        .operations
        .iter_mut()
        .find(|op| op.request.operation_id == compensation.operation_id)
        .unwrap();
    let Business::CommitAssets {
        transaction:
            AssetTransaction::Compensation {
                original_operation_id,
                ..
            },
    } = &mut op.request.body
    else {
        panic!()
    };
    *original_operation_id = "other:admission".into();
    resign(&mut cp);
    assert!(matches!(restored(cp), Err(Error::InvalidMessage)));
}

#[test]
fn invalid_recovery_does_not_initialize_and_valid_payload_still_works() {
    let (_, _, cp) = settlement();
    let c = config();
    let mut e = Engine::new(c.clone());
    let mut r = create(&c);
    let mut bad = cp.clone();
    bad.state.operations[0].terminal = Some(ReceiptOutcome::Committed {
        revision: 9,
        commit_id: "wrong".into(),
    });
    resign(&mut bad);
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
fn strict_asset_transactions_reject_unknown_fields_and_unknown_variants() {
    for body in [
        serde_json::json!({"kind":"load_assets","cultivation":1}),
        serde_json::json!({"kind":"commit_assets","transaction":{
        "kind":"admission","expected_revision":0,"debits":[],"entry_years":1}}),
        serde_json::json!({"kind":"commit_assets","transaction":{"kind":"arbitrary_script"}}),
    ] {
        assert!(serde_json::from_value::<Business>(body).is_err());
    }
}
