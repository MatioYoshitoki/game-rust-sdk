use super::*;
pub(super) fn configure(v: &mut Value) {
    super::slots::configure(v);
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/skill-hp-cost-host/arena-b.json"
    ))
    .unwrap();
    v["mechanism_version"] = json!(45);
    v["skill"] = sample["skill"].clone();
    v["additional_skills"] = sample["additional_skills"].clone();
    v["pve"]["stats"]["atk"] = json!(1);
    v["pve"]["stats"]["hp"] = json!(10000);
}
fn cast(h: &mut DeltaHarness, i: usize) {
    let skill_id = h.engine.config.additional_skills[6].id.clone();
    h.action(i, Action::CastSelf { skill_id }).unwrap();
}
fn costs(h: &DeltaHarness, i: usize) -> Vec<u32> {
    h.events
        .iter()
        .filter_map(|(pid, e)| match e {
            EventV4::Private {
                event:
                    Event::ActionCommitted {
                        hp_cost: Some(cost),
                        ..
                    },
            } if pid.as_deref() == Some(format!("P{i:02}").as_str()) => Some(*cost),
            _ => None,
        })
        .collect()
}
#[test]
fn hp_cost_config_strict_schema_last_slot_and_legacy_capability() {
    assert!(delta_catalog(8, configure).is_ok());
    for mode in 0..17 {
        assert!(
            delta_catalog(8, |v| {
                configure(v);
                let s = &mut v["additional_skills"][6];
                match mode {
                    0 => v["mechanism_version"] = json!(44),
                    1 => v["schema_version"] = json!(6),
                    2 => s["hp_cost"] = Value::Null,
                    3 => s["hp_cost"] = json!({}),
                    4 => s["hp_cost"]["mode"] = json!("current_max_hp_ratio"),
                    5 => s["hp_cost"]["value"] = json!(-1),
                    6 => s["hp_cost"]["value"] = json!(1.00001),
                    7 => s["hp_cost"]["value"] = json!("0.05"),
                    8 => s["hp_cost"]["min_hp_after_cost"] = json!(0),
                    9 => s["hp_cost"]["min_hp_after_cost"] = json!(1.5),
                    10 => s["hp_cost"]["min_hp_after_cost"] = json!(1000001),
                    11 => {
                        s["hp_cost"].as_object_mut().unwrap().remove("value");
                    }
                    12 => s["hp_cost"]["damage"] = json!(true),
                    13 => {
                        s["hp_cost"]["mode"] = json!("fixed");
                        s["hp_cost"]["value"] = json!(1000001);
                    }
                    14 => s["hp_cost"] = json!([]),
                    15 => s["hp_cost"]["value"] = Value::Null,
                    _ => {
                        s["hp_cost"]
                            .as_object_mut()
                            .unwrap()
                            .remove("min_hp_after_cost");
                    }
                }
            })
            .is_err(),
            "mode {mode}"
        );
    }
    for version in [37, 38, 39, 40, 41, 42, 43, 44] {
        assert!(
            delta_catalog(1, |v| {
                v["mechanism_version"] = json!(version);
                v["skill"]["hp_cost"] = Value::Null;
            })
            .is_err()
        );
    }
    assert!(
        delta_catalog(8, |v| {
            configure(v);
            v["mechanism_version"] = json!(44);
            v["skill"].as_object_mut().unwrap().remove("hp_cost");
            for s in v["additional_skills"].as_array_mut().unwrap() {
                s.as_object_mut().unwrap().remove("hp_cost");
            }
        })
        .is_ok()
    );
}
#[test]
fn hp_cost_public_one_two_four_eight_members_and_private_costs() {
    for n in [1, 2, 4, 8] {
        let mut h = DeltaHarness::new(n, configure);
        h.active();
        let hp = (0..n).map(|i| h.snapshot(i).own.hp).collect::<Vec<_>>();
        for i in 0..n {
            cast(&mut h, i);
        }
        h.ticks(5);
        for (i, &before) in hp.iter().enumerate() {
            let c = costs(&h, i);
            assert_eq!(c.len(), 1);
            assert!(c[0] > 0);
            assert_eq!(h.snapshot(i).own.hp, before - c[0]);
            assert!(h.snapshot(i).own.player_skills[7].ready_at_ms > 0);
            assert!(
                h.snapshot(i).own.player_skills[..7]
                    .iter()
                    .all(|s| s.ready_at_ms == 0)
            );
        }
        h.close();
        assert_eq!(h.engine.pending(), 0);
    }
}
#[test]
fn hp_cost_duplicate_public_input_charges_once_and_peer_is_independent() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let before = h.snapshot(1).own.clone();
    let id = h.engine.config.additional_skills[6].id.clone();
    let input = h.input(0, Action::CastSelf { skill_id: id });
    h.engine.input(input.clone()).unwrap();
    h.engine.input(input).unwrap();
    h.ticks(5);
    assert_eq!(costs(&h, 0).len(), 1);
    assert!(costs(&h, 1).is_empty());
    assert_eq!(h.snapshot(1).own.hp, before.hp);
    assert_eq!(h.snapshot(1).own.player_skills, before.player_skills);
    h.close();
}
#[test]
fn hp_cost_reconnect_preserves_committed_hp_and_interrupts_unpaid_preparation() {
    for committed in [false, true] {
        let mut h = DeltaHarness::new(2, configure);
        h.active();
        let hp = h.snapshot(0).own.hp;
        cast(&mut h, 0);
        if committed {
            h.ticks(5);
        }
        let expected = h.snapshot(0).own.hp;
        let cd = h.snapshot(0).own.player_skills.clone();
        let old = h.engine.seats[0].connection.clone();
        h.engine
            .input(Input {
                key: h.engine.create.key.clone(),
                participant_id: "P00".into(),
                entity_generation: 1,
                connection: Connection {
                    id: "hp-reconnect".into(),
                    generation: 2,
                },
                request_id: "hp-reconnect".into(),
                sequence: 1,
                action: Action::Reconnect {
                    previous_connection: old.clone(),
                    authenticated_principal: "principal-P00".into(),
                },
            })
            .unwrap();
        h.sequences[0] = 1;
        h.pump();
        assert_eq!(h.snapshot(0).own.hp, expected);
        assert_eq!(h.snapshot(0).own.player_skills, cd);
        h.ticks(5);
        assert_eq!(costs(&h, 0).len(), usize::from(committed));
        assert_eq!(h.snapshot(0).own.hp, if committed { expected } else { hp });
        let mut input = h.input(0, Action::Stop {});
        input.connection = old;
        assert_eq!(h.engine.input(input), Err(Error::StaleConnection));
        h.close();
    }
}
#[test]
fn hp_cost_settling_cleans_owner_without_refund_or_blocking_peer() {
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    cast(&mut h, 0);
    h.ticks(5);
    let hp = h.snapshot(0).own.hp;
    h.hold_settle[0] = true;
    h.action(0, Action::Leave {}).unwrap();
    h.ticks(5);
    assert_eq!(h.snapshot(0).own.hp, hp);
    cast(&mut h, 1);
    h.ticks(5);
    assert_eq!(costs(&h, 1).len(), 1);
    assert!(h.snapshot(1).own.hp > 0);
    h.hold_settle[0] = false;
    for (i, r) in std::mem::take(&mut h.held) {
        let proof = h.stores[i].execute(&r);
        h.receipt(i, &r, proof).unwrap();
    }
    h.pump();
    h.close();
    assert_eq!(h.engine.pending(), 0);
}
#[test]
fn hp_cost_all_slots_hp_full_delta_precise_interop() {
    let mut h = DeltaHarness::new(8, configure);
    h.active();
    for slot in 0..8 {
        for i in 0..8 {
            super::slots::cast_slot(&mut h, i, slot);
        }
        h.ticks(30);
        if slot == 4 {
            let epoch = h.engine.seats[3].epoch;
            h.replicas[3].forget_baseline();
            h.action(
                3,
                Action::RequestFull {
                    stream_epoch: epoch,
                },
            )
            .unwrap();
            h.pump();
        }
    }
    h.ticks(60);
    h.close();
    for i in 0..8 {
        assert!(costs(&h, i).len() >= 8);
        assert!(costs(&h, i).iter().sum::<u32>() > 0);
    }
    assert!(
        h.frames
            .iter()
            .any(|e| matches!(e.payload, PublicationPayloadV6::Delta { .. }))
    );
    if let Ok(path) = std::env::var("SDK528_INTEROP_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&h.vectors).unwrap()).unwrap();
    }
}
#[test]
fn hp_cost_input_cannot_override_amount_minimum_or_member() {
    for field in [
        "hp_cost",
        "min_hp_after_cost",
        "current_hp",
        "cost",
        "target_member",
    ] {
        let input = json!({"kind":"cast_self","skill_id":"focus-self",field:999});
        assert!(serde_json::from_value::<Action>(input).is_err());
    }
    let mut h = DeltaHarness::new(2, configure);
    h.active();
    let before = h.snapshot(0).own.clone();
    assert_eq!(
        h.action(
            0,
            Action::CastSelf {
                skill_id: "missing".into()
            }
        ),
        Err(Error::InvalidTarget)
    );
    h.ticks(5);
    assert_eq!(h.snapshot(0).own.hp, before.hp);
    assert_eq!(h.snapshot(0).own.spirit, before.spirit);
    assert!(costs(&h, 0).is_empty());
    h.close();
}
