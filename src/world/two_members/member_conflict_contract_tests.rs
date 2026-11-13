use super::*;

fn conflict_input(v: &mut Value) {
    v["members"]["capacity"] = json!(3);
    v["members"]["spawns"] = json!([[20, 20], [20, 40], [20, 60]]);
    v["members"]["member_conflict"] = json!(true);
    v["members"]["spectators"] = json!({"capacity":1,"terminal_retention_ms":400});
    v["extraction_point"] = json!([20, 40]);
    v["carried_quantity"] = json!(0);
    v["params"]["entry_years"] = json!(0);
    v["params"]["death"] = json!({"slot_ratio":1,"quantity_ratio":1,"minimum_quantity":1});
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["pve"]["combat"]["aggro_range"] = json!(2);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["world_loot"] = json!({
        "environment_capacity":8,"container_capacity":4,"partial_pickup":true,
        "interaction_range":16,"shared_clearance":5,"containers":[],
        "pools":{"source":{"guaranteed":[{"item_id":"item","quantity_min":2,"quantity_max":2}],
                         "independent":[],"groups":[]}},
        "nodes":[{"id":"source","pool":"source","points":[[20,40]],"replenish_ms":600,"ttl_ms":600}],
        "relics":{"capacity":2,"interaction_range":24,"excluded_items":[]}
    });
    v["settlement_progression"] = json!({"age_asset_id":"elapsed"});
    v["settlement_rewards"] = json!({"track":{
        "kill_points":{"npc":10},"rate_bps":10000,"balance_cap":100,
        "damage":{"rate_bps":10000,"per_hit_cap_points":10,"match_cap_points":100},
        "by_result":{
            "extract":{"bonus_points":0,"multiplier_bps":10000},
            "death":{"bonus_points":0,"multiplier_bps":10000}
        }
    }});
}

#[test]
fn conflict_is_opt_in_and_requires_an_extraction_roster() {
    let old = catalog(input(), |v| {
        v["mechanism_version"] = json!(104);
        v["params"]["entry_years"] = json!(0);
        v["skill"]["mechanism"] = json!("projectile");
    });
    let old_error = old.err().unwrap().to_string();
    assert!(old_error.contains("CONFIG_PARAMETER_BOUNDS"), "{old_error}");
    let mut default = input();
    default["pve"]
        .as_object_mut()
        .unwrap()
        .remove("status_rules");
    let c = catalog(default, |_| {}).unwrap().default_config();
    let mut w = world(&c);
    let cast = Action::CastUnit {
        skill_id: c.skill.id.clone(),
        target: World::unit_wire(&w.members[1].reference.entity),
    };
    assert_eq!(
        w.action(0, &cast, "no-pvp", &c, 0),
        Err(Error::InvalidTarget)
    );
    for case in ["one", "contest"] {
        let result = catalog(input(), |v| {
            conflict_input(v);
            if case == "one" {
                v["members"]["capacity"] = json!(1);
                v["members"]["spawns"] = json!([[20, 20]]);
            } else {
                super::match_mode_contract_tests::contest_input(v);
                v["pve"]
                    .as_object_mut()
                    .unwrap()
                    .remove("status_definitions");
            }
        });
        let error = result.err().unwrap().to_string();
        assert!(
            error.contains("CONFIG_MEMBER_CONFLICT"),
            "{case} returned {error}"
        );
    }
}

#[test]
fn rust_replica_replays_a_real_member_target_and_keeps_conflict_frozen() {
    use crate::replication::{DecodeError, ReplicaV6, diff};
    let c = catalog(input(), conflict_input).unwrap().default_config();
    let mut w = world_with_seats(&c, &["a", "b", "c"]);
    let before = w.snapshot(0, &c, Phase::Active, 0);
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&w.members[1].reference.entity),
        },
        "member",
        &c,
        0,
    )
    .unwrap();
    let after = w.snapshot(0, &c, Phase::Active, 0);
    let mut frame = PublicationEnvelopeV6 {
        key: MatchKey {
            id: "replica".into(),
            incarnation: 1,
        },
        publication_id: "first".into(),
        recipient_id: "a".into(),
        connection: Connection {
            id: "c".into(),
            generation: 1,
        },
        stream_epoch: 1,
        revision: 1,
        payload: PublicationPayloadV6::Full {
            snapshot: Box::new(before.clone()),
            reason: FullReason::Initial,
        },
    };
    let mut replica = ReplicaV6::new(frame.key.clone(), frame.recipient_id.clone());
    replica.apply(&frame).unwrap();
    frame.publication_id = "second".into();
    frame.revision = 2;
    frame.payload = PublicationPayloadV6::Delta {
        base_publication_id: "first".into(),
        base_revision: 1,
        delta: Box::new(diff(&before, &after)),
    };
    assert_eq!(replica.apply(&frame), Ok(&after));
    replica.forget_baseline();
    frame.publication_id = "replacement".into();
    frame.revision = 3;
    frame.stream_epoch = 2;
    frame.payload = PublicationPayloadV6::Full {
        snapshot: Box::new(SnapshotV4 {
            member_conflict: false,
            ..before
        }),
        reason: FullReason::Resync,
    };
    assert_eq!(replica.apply(&frame), Err(DecodeError::InvalidFrame));
}

#[test]
fn member_attacks_interrupt_extraction_and_yield_only_the_original_relic() {
    let c = catalog(input(), conflict_input).unwrap().default_config();
    let mut w = world_with_seats(&c, &["a", "b", "c"]);
    assert!(w.snapshot(0, &c, Phase::Active, 0).member_conflict);
    assert!(w.snapshot(0, &c, Phase::Active, 0).own.inventory.is_empty());
    w.members[1].hp = 15;
    w.tick(&c, 30);
    let resource = w
        .snapshot(1, &c, Phase::Active, 30)
        .resources
        .pop()
        .unwrap();
    w.action(
        1,
        &Action::Pickup {
            resource_id: resource.id,
            expected_revision: w.members[1].revision,
        },
        "pickup",
        &c,
        30,
    )
    .unwrap();
    w.action(1, &Action::ExtractStart {}, "interrupted", &c, 30)
        .unwrap();
    let target = World::unit_wire(&w.members[1].reference.entity);
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: target.clone(),
        },
        "cast",
        &c,
        30,
    )
    .unwrap();
    let events = w.tick(&c, 60);
    assert_eq!(w.members[1].hp, 5);
    assert!(events.iter().any(|(recipient, e)| matches!(
        (recipient, e),
        (
            Some(1),
            EventV4::Private {
                event: Event::ExtractionInterrupted
            }
        )
    )));
    assert!(w.members[1].extract_end.is_none());

    for now in (90..=150).step_by(30) {
        w.tick(&c, now);
    }
    w.action(0, &Action::AttackUnit { target }, "projectile", &c, 150)
        .unwrap();
    for now in (180..=300).step_by(30) {
        w.tick(&c, now);
    }
    assert_eq!(w.members[1].desired, Some(Outcome::Death));
    assert_eq!(w.members[2].hp, 100);
    assert!(w.members[0].tally.is_empty());
    assert!(w.members[0].damage_growth.is_empty());
    let loss = w.settlement(1, &c);
    assert!(matches!(
        loss,
        Business::CommitSettlement {
            outcome: Outcome::Death,
            kept: 0,
            ..
        }
    ));
    let relic = w
        .snapshot(0, &c, Phase::Active, 300)
        .relics
        .unwrap()
        .pop()
        .unwrap();
    let opened = w
        .action(
            0,
            &Action::OpenRelic {
                relic_id: relic.id.clone(),
            },
            "open",
            &c,
            300,
        )
        .unwrap();
    assert!(
        matches!(&opened[..],[(Some(0),EventV4::Private{event:Event::RelicOpened{contents,..}})]
        if contents.len()==1)
    );
    w.action(
        0,
        &Action::ClaimRelic {
            relic_id: relic.id,
            expected_revision: relic.revision,
            expected_inventory_revision: w.members[0].revision,
            entry_id: None,
        },
        "claim",
        &c,
        300,
    )
    .unwrap();
    w.action(0, &Action::ExtractStart {}, "escape", &c, 300)
        .unwrap();
    for now in (330..=390).step_by(30) {
        w.tick(&c, now);
    }
    assert!(matches!(w.settlement(0,&c),Business::CommitSettlement{
        outcome:Outcome::Extraction,kept:2,scoring:Some(score),progression:Some(plan),..
    } if score.credited_kills==0 && plan.damage[0].points_bps==0));
    assert!(w.members[2].eligible());
}
