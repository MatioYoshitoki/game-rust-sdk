use super::*;

#[test]
fn source_terminal_scenario_s28_settles_both_protected_and_unprotected_members_once() {
    let source: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_cast_scenarios.json"
    ))
    .unwrap();
    let case = &source["standardTerminalScenario"];
    let fixture = &source["fixture"];
    assert_eq!(case["id"], "S28");
    assert_eq!(case["fixture"], "plain-active");
    let deadline = case["given"]["nowMs"].as_u64().unwrap();
    assert_eq!(
        case["when"],
        json!([{"atMs":deadline,"advanceToMs":deadline}])
    );
    let catalog = super::super::source_instant_cast::catalog(None, |v| {
        v["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id":"scenario-invincible","mechanism":"timed_control",
                "attribute":"damage_immunity","duration_ms":300,
                "duplicate_policy":"refresh","source_scoped":true
            }));
        v["additional_skills"] = json!([{
            "id":"scenario-immunity","mechanism":"self_status","cost":0,
            "windup_ms":0,"recovery_ms":0,"cooldown_ms":0,
            "commit_on_accept":true,"on_commit_self_statuses":["scenario-invincible"]
        }]);
    })
    .unwrap();
    let config = catalog.default_config();
    let mut request = create(&config);
    request.protocol_version = 6;
    request.key = serde_json::from_value(
        json!({"id":fixture["matchId"],"incarnation":fixture["incarnation"]}),
    )
    .unwrap();
    request.seed = fixture["seed"].as_u64().unwrap().try_into().unwrap();
    let mut host = DeltaHarness::from_request(catalog, request);
    for (store, participant) in host
        .stores
        .iter_mut()
        .zip(fixture["participants"].as_array().unwrap())
    {
        store.hp = participant["maxHp"].as_u64().unwrap().try_into().unwrap();
        store.spirit = participant["maxSpirit"]
            .as_u64()
            .unwrap()
            .try_into()
            .unwrap();
    }
    host.active();
    assert_eq!(
        host.snapshot(0).active_at_ms,
        fixture["activeAtMs"].as_u64()
    );
    assert_eq!(config.terminal_ms, deadline);
    assert!(host.snapshot(0).monsters.is_empty());
    // This fixture has no ambient combat. Start the final steps at the source
    // deadline rather than replaying an empty world for fifteen minutes.
    host.engine.now = deadline - 2 * config.step_ms;
    host.action(
        0,
        Action::CastSelf {
            skill_id: "scenario-immunity".into(),
        },
    )
    .unwrap();
    host.pump();
    host.action(0, Action::Background {}).unwrap();
    host.pump();
    host.ticks(1);
    assert_eq!(host.engine.now, deadline - config.step_ms);
    assert_eq!(
        host.snapshot(0).own.protected,
        case["given"]["participants.A.protected"]
    );
    assert_eq!(
        host.snapshot(0).own.statuses.iter().any(|status| {
            status.has_attribute(StatusAttribute::DamageImmunity) && status.expires_at_ms > deadline
        }),
        case["given"]["participants.A.invincible"]
    );
    assert!(
        host.engine
            .world
            .as_ref()
            .unwrap()
            .members
            .iter()
            .all(|member| {
                member.phase == MemberPhase::Active && member.hp > 0 && member.desired.is_none()
            })
    );
    assert!(
        host.stores
            .iter()
            .all(|store| store.admissions == 1 && store.settlements == 0)
    );

    host.hold_settle.fill(true);
    host.ticks(1);
    assert_eq!(host.engine.now, deadline);
    let oracle = case["then"].as_array().unwrap();
    assert_eq!(host.engine.phase, Phase::Closed);
    assert!(host.engine.world.is_none());
    for (index, assertion) in oracle[..2].iter().enumerate() {
        let snapshot = host.snapshot(index);
        assert_eq!(snapshot.phase, Phase::Closed);
        assert_eq!(snapshot.logical_ms, deadline);
        assert_eq!(
            snapshot.own.phase == MemberPhase::Active,
            assertion["value"]
        );
        assert_eq!(snapshot.own.phase, MemberPhase::Settling);
        assert!(snapshot.own.statuses.is_empty());
        assert!(!host.engine.seats[index].ledger.ledger_settled());
    }
    let deaths: Vec<_> = host
        .events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::MemberChanged {
                entity,
                phase: MemberPhase::Settling,
            } => Some(entity.id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(deaths.len(), oracle[2]["value"]);
    assert_eq!(deaths, ["A", "B"]);
    assert_eq!(host.held.len(), 2);
    let mut held = std::mem::take(&mut host.held);
    held.sort_by_key(|(index, _)| *index);
    assert_eq!(held[0].0, 0);
    assert_eq!(held[1].0, 1);
    assert_ne!(held[0].1.operation_id, held[1].1.operation_id);
    for (index, request) in held {
        let intent = config.decode_business(&request.body).unwrap();
        assert!(matches!(
            intent,
            Business::CommitSettlement {
                outcome: Outcome::Death,
                ..
            }
        ));
        let outcome = host.stores[index].execute(&request);
        assert_eq!(host.stores[index].execute(&request), outcome);
        assert!(matches!(outcome, ReceiptOutcome::Committed { .. }));
        host.receipt(index, &request, outcome).unwrap();
        host.pump();
        assert!(host.engine.seats[index].ledger.ledger_settled());
        if index == 0 {
            assert_eq!(host.stores[1].settlements, 0);
            assert!(!host.engine.seats[1].ledger.ledger_settled());
        }
    }
    assert_eq!(host.engine.phase, Phase::Closed);
    assert_eq!(host.engine.pending(), 0);
    host.ticks(1);
    host.close();
    assert_eq!(host.engine.now, deadline);
    assert_eq!(
        host.events
            .iter()
            .filter(|(_, event)| matches!(event, EventV4::Closed { .. }))
            .count(),
        1
    );
    assert!(
        host.stores
            .iter()
            .all(|store| store.admissions == 1 && store.settlements == 1)
    );
}
