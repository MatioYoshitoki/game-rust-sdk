//! Typed domain scenario adapter. Source timestamps and resolved input identities are preserved.
use super::*;
use serde_json::{Value, json};

fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_cast_scenarios.json"
    ))
    .unwrap()
}

fn u(row: &Value, key: &str) -> u64 {
    row[key].as_u64().unwrap()
}

fn id(row: &Value, key: &str) -> String {
    row[key].as_str().unwrap().to_owned()
}

fn scenario<'a>(source: &'a Value, key: &str) -> &'a Value {
    source["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == key)
        .unwrap()
}

fn fixture(source: &Value, case: &Value) -> (Config, World) {
    let (mut c, _) = super::unit_effect_tests::setup(false, false);
    let f = &source["fixture"];
    assert_eq!(case["fixture"], "plain-active");
    assert_eq!(f["ambientRegeneration"], false);
    assert_eq!(f["aiEnabled"], false);
    assert_eq!(f["phase"], "active");
    assert_eq!(f["geometry"]["terrain"], "plain");
    assert_eq!(f["geometry"]["blockingPolygons"], json!([]));
    let origin = [
        f["geometry"]["bounds"][0].as_f64().unwrap(),
        f["geometry"]["bounds"][1].as_f64().unwrap(),
    ];
    c.geometry.min = [0., 0.];
    c.geometry.max = [
        f["geometry"]["bounds"][2].as_f64().unwrap() - origin[0],
        f["geometry"]["bounds"][3].as_f64().unwrap() - origin[1],
    ];
    c.geometry.walls.clear();
    let skill_id = case["when"][0]["input"]["payload"]["skillId"]
        .as_str()
        .unwrap();
    let definition = &source["skills"][skill_id];
    let lifecycle = &definition["castLifecycle"];
    let preparation = &lifecycle["preparation"];
    let mut skill = json!({
        "id":skill_id,"cost":definition["spiritCost"],
        "windup_ms":preparation["durationMs"],"recovery_ms":lifecycle["recovery"]["durationMs"],
        "range":definition["range"],"cooldown_ms":u(definition,"cooldown")*1000,
        "resource_commit":{"reserve_at":"accept","consume_at":"commit","cooldown_starts_at":"commit","refund_before_commit":{"kind":"full"}},
    });
    assert_eq!(lifecycle["resources"]["reserveAt"], "accept");
    assert_eq!(lifecycle["resources"]["consumeAt"], "commit");
    assert_eq!(lifecycle["resources"]["cooldownStartsAt"], "commit");
    if definition["targeting"]["aim"] == "direction" {
        let runtime = &definition["runtimes"][0];
        assert_eq!(runtime["kind"], "projectile");
        assert_eq!(runtime["motion"]["type"], "linear");
        skill["mechanism"] = json!("projectile");
        skill["speed"] = runtime["motion"]["speed"].clone();
        skill["radius"] = runtime["shape"]["radius"].clone();
        skill["damage_bps"] = definition["factors"][0]["params"][3].clone();
    } else {
        assert_eq!(definition["targeting"]["relation"], "ally");
        assert_eq!(lifecycle["activation"]["revalidation"], "strict");
        skill["mechanism"] = json!("ally_heal");
        // The original fixed attack points are fixture facts, not a production formula.
        let base_attack = u(&f["participants"][0], "atk");
        let heal_factor = &definition["factors"][0];
        assert_eq!(heal_factor["factorId"], "factor_heal");
        let base = heal_factor["params"][0].as_u64().unwrap();
        let attack_bps = heal_factor["params"][2].as_u64().unwrap();
        skill["heal"] = json!({"amount":base + base_attack*attack_bps/10000});
    }
    c.skill = serde_json::from_value(skill).unwrap();
    c.additional_skills.clear();
    c.params.player_radius = f["participants"][0]["radius"].as_f64().unwrap();
    c.params.regen.near_ratio = 0.;
    c.params.regen.far_ratio = 0.;
    c.carried_quantity = 0;
    let p = c.pve_mut();
    p.monster_positions.clear();
    p.extra_spawns.clear();
    p.status_definitions.clear();
    p.palette.truncate(1);
    let [width, height] = c.geometry.max.map(|v| (v / 40.).round() as usize);
    let p = c.pve_mut();
    p.width = width;
    p.height = height;
    p.cell_size = 40.;
    p.cells = vec![0; width * height].into();
    p.cells_rle = vec![(0, width * height)];
    let rows = f["participants"].as_array().unwrap();
    let positions = rows
        .iter()
        .map(|r| {
            [
                r["position"][0].as_f64().unwrap() - origin[0],
                r["position"][1].as_f64().unwrap() - origin[1],
            ]
        })
        .collect::<Vec<_>>();
    c.spawn = positions[0];
    c.extraction_point = c.spawn;
    c.extraction_unlock_ms = u(&f["extractionPoints"][0], "opensAtMs");
    c.terminal_ms = u(
        &source["standardTerminalScenario"]["when"][0],
        "advanceToMs",
    );
    let m = c.members.as_mut().unwrap();
    m.capacity = 2;
    m.spawns = positions;
    let r = c.combat_rules.as_mut().unwrap();
    r.triggers.clear();
    r.cast_interceptions.clear();
    let seats = rows
        .iter()
        .map(|r| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            participant_id: id(r, "id"),
            authenticated_principal: id(r, "ownerId"),
            connection: Connection {
                id: id(r, "connectionId"),
                generation: u(r, "connectionGeneration").try_into().unwrap(),
            },
            combat_build: None,
            skills: None,
        })
        .collect::<Vec<_>>();
    let facts = rows
        .iter()
        .map(|r| ParticipantFacts {
            principal: id(r, "ownerId"),
            revision: 0,
            warehouse_quantity: 0,
            hp_points: u(r, "maxHp").try_into().unwrap(),
            spirit_points: u(r, "maxSpirit").try_into().unwrap(),
            atk_points: u(r, "atk").try_into().unwrap(),
            def_points: u(r, "def").try_into().unwrap(),
            spd_points: u(r, "spd").try_into().unwrap(),
        })
        .collect();
    let mut w = World::new(&c, &seats, facts, u(f, "seed").try_into().unwrap());
    for (m, row) in w.members.iter_mut().zip(rows) {
        assert_eq!(
            m.reference.entity.generation,
            u(row, "entityGeneration") as u32
        );
        m.joined = true;
        m.ready = row["ready"].as_bool().unwrap();
        m.hp = u(row, "hp").try_into().unwrap();
        m.spirit = u(row, "spirit").try_into().unwrap();
        m.inventory.clear();
    }
    if case["given"]["participants.B.hp"].is_u64() {
        assert_eq!(case["given"]["pvpEnabled"], false);
        assert_eq!(case["given"]["participants.B.faction"], "player");
        assert_eq!(case["given"]["participants.A.skillIds"], json!([skill_id]));
        w.members[1].hp = u(&case["given"], "participants.B.hp") as u32;
    }
    w.activate(&c, u(f, "activeAtMs"), &mut vec![]);
    assert!(w.npcs.is_empty() && w.statuses.entries().is_empty());
    (c, w)
}

fn accept(w: &mut World, c: &Config, step: &Value, source: &Value) -> Events {
    let input = &step["input"];
    let payload = &input["payload"];
    let intent = &payload["intent"];
    assert_eq!(input["participantId"], w.members[0].reference.entity.id);
    assert_eq!(
        input["connectionId"],
        source["fixture"]["participants"][0]["connectionId"]
    );
    assert_eq!(
        input["connectionGeneration"],
        source["fixture"]["participants"][0]["connectionGeneration"]
    );
    assert_eq!(input["sequence"], 1);
    assert_eq!(source["fixture"]["inputSequences"]["A"], 0);
    assert_eq!(
        input["entityGeneration"],
        w.members[0].reference.entity.generation
    );
    assert_eq!(input["kind"], "Cast");
    let skill_id = id(payload, "skillId");
    let action = match intent["kind"].as_str().unwrap() {
        "direction" => Action::Cast {
            skill_id,
            direction: serde_json::from_value(intent["direction"].clone()).unwrap(),
        },
        "unit" => Action::CastUnit {
            skill_id,
            target: UnitTarget {
                id: id(intent, "targetId"),
                generation: u(intent, "entityGeneration") as u32,
            },
        },
        _ => panic!("unsupported fixture intent"),
    };
    w.action(
        0,
        &action,
        input["requestId"].as_str().unwrap(),
        c,
        u(step, "atMs"),
    )
    .unwrap()
}

fn advance(w: &mut World, c: &Config, clock: &mut u64, through: u64, events: &mut Events) {
    while *clock + c.step_ms <= through {
        *clock += c.step_ms;
        events.extend(w.tick(c, *clock));
        assert!(
            w.shots.is_empty(),
            "an originally interrupted cast materialized a shot"
        );
        assert!(w.areas.is_empty());
    }
}

#[test]
fn source_inventory_scenario_s20_failed_placement_preserves_the_original_slot() {
    let source = source();
    let routes: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_execution_routes.json"
    ))
    .unwrap();
    let case = &routes["r3Scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "S20")
        .unwrap()["original"];
    let cast_case = scenario(&source, "S10");
    assert_eq!(case["fixture"], cast_case["fixture"]);
    let (mut c, mut w) = fixture(&source, cast_case);
    c.member_loot_drop = Some(serde_json::from_value(json!({"max_active":1})).unwrap());
    let participant = &source["fixture"]["participants"][0];
    c.inventory_capacity = u(&participant["inventory"], "capacity").try_into().unwrap();
    let step = &case["when"][0];
    let input = &step["input"];
    let payload = &input["payload"];
    let slots = case["given"]["participants.A.inventory.slots"]
        .as_array()
        .unwrap();
    let slot = u(payload, "slot") as usize;
    assert_eq!((slot, slots.len()), (0, 1));
    let item_id = id(&slots[slot], "itemId");
    let amount: u32 = u(&slots[slot], "quantity").try_into().unwrap();
    let revision: u32 = u(&participant["inventory"], "revision").try_into().unwrap();
    assert_eq!(payload["expectedRevision"], revision);
    assert_eq!(input["participantId"], w.members[0].reference.entity.id);
    assert_eq!(
        input["entityGeneration"],
        w.members[0].reference.entity.generation
    );
    assert_eq!(input["kind"], "Drop");
    assert_eq!(step["atMs"], source["fixture"]["nowMs"]);
    // The source fixture specifies the slot and count. Project it as eligible
    // in-match loot so rejection reaches placement with the supplied revision.
    w.members[0].inventory.insert(item_id.clone(), amount);
    w.members[0].looted.insert(item_id.clone(), amount);
    assert_eq!(w.members[0].revision, revision);
    let action = Action::DropItem {
        item_id: item_id.clone(),
        quantity: u(payload, "quantity").try_into().unwrap(),
        expected_revision: revision,
    };
    let request_id = input["requestId"].as_str().unwrap();
    let now = u(step, "atMs");
    let seq = w.sequence;
    let before = w.snapshot(0, &c, Phase::Active, now);

    assert_eq!(case["given"]["placementResult"], "no_legal_point");
    assert_eq!(w.grid.config.palette.len(), 1);
    assert!(w.grid.config.palette[0].walkable);
    Arc::make_mut(&mut w.grid.config).palette[0].walkable = false;
    assert!(
        !w.grid
            .clear(w.members[0].position, c.params.player_radius, 0)
    );
    let result = w.action(0, &action, request_id, &c, now);
    assert_eq!(result.as_ref().err(), Some(&Error::InvalidTarget));
    let events = result.unwrap_or_default();
    let oracle = case["then"].as_array().unwrap();
    assert_eq!(w.members[0].inventory[&item_id], oracle[0]["value"]);
    assert_eq!(
        events
            .iter()
            .filter(|(_, event)| matches!(event, EventV4::ResourceSpawned { .. }))
            .count(),
        oracle[1]["value"]
    );
    assert_eq!(w.members[0].looted[&item_id], amount);
    assert_eq!(w.sequence, seq);
    assert_eq!(w.snapshot(0, &c, Phase::Active, now), before);

    Arc::make_mut(&mut w.grid.config).palette[0].walkable = true;
    let events = w.action(0, &action, request_id, &c, now).unwrap();
    let [(None, EventV4::ResourceSpawned { resource })] = events.as_slice() else {
        panic!("expected one ground resource");
    };
    assert_eq!(resource.item_id.as_deref(), Some(item_id.as_str()));
    assert_eq!(resource.quantity, amount);
    assert_eq!(resource.position, w.members[0].position);
    assert_eq!(w.members[0].revision, revision + 1);
    assert!(!w.members[0].inventory.contains_key(&item_id));
    assert!(!w.members[0].looted.contains_key(&item_id));
    assert_eq!(w.resources.len(), 1);
}

#[test]
fn source_cast_scenario_s10_stun_before_reserved_projectile_keeps_cost_and_plan_empty() {
    let source = source();
    let case = scenario(&source, "S10");
    let (mut c, mut w) = fixture(&source, case);
    let timeline = case["when"].as_array().unwrap();
    let event = &timeline[1]["hostScenarioEvent"];
    assert_eq!(event["kind"], "Stun");
    assert_eq!(event["targetId"], "A");
    let at = u(&timeline[1], "atMs");
    let stun=serde_json::from_value(json!({"id":"scenario-stun","mechanism":"timed_control","attribute":"stun","duration_ms":u(event,"durationMs"),"duplicate_policy":"refresh","source_scoped":true})).unwrap();
    c.pve_mut().status_definitions.push(stun);
    let mut observed = accept(&mut w, &c, &timeline[0], &source);
    let pending = w.members[0].pending.as_ref().unwrap();
    assert_eq!(pending.id, timeline[0]["input"]["requestId"]);
    assert!(pending.reservation.is_some());
    assert!(pending.commits > at);
    let mut now = u(&source["fixture"], "nowMs");
    advance(&mut w, &c, &mut now, at - 1, &mut observed);
    // The source's typed host stun has no actor. This test projects its resolved
    // status and then calls the same native control consequence as any applied stun.
    let target = status_entity(&w.members[0].reference.entity);
    let rules = c.pve.as_ref().unwrap().status_rules.as_ref().unwrap();
    assert!(w.statuses.apply(
        &c.pve.as_ref().unwrap().status_definitions,
        (&target, 100),
        (&target, 1000),
        at,
        rules,
        &mut vec![]
    ));
    w.apply_control_consequences(&target, (false, false, true), &c, at, &mut observed);
    advance(
        &mut w,
        &c,
        &mut now,
        u(&timeline[2], "advanceToMs"),
        &mut observed,
    );
    let oracle = case["then"].as_array().unwrap();
    assert_eq!(w.members[0].spirit, oracle[0]["value"]);
    assert_eq!(w.shots.len() + w.areas.len(), oracle[1]["value"]);
    assert_eq!(
        observed
            .iter()
            .filter(|(owner, e)| *owner == Some(0)
                && matches!(
                    e,
                    EventV4::Private {
                        event: Event::ActionInterrupted
                    }
                ))
            .count(),
        oracle[2]["value"]
    );
    assert!(w.members[0].pending.is_none() && w.members[0].cooldowns.is_empty());
}

#[test]
fn source_cast_scenario_s12_death_invalidates_the_accepted_target_without_reattempt_or_heal() {
    let source = source();
    let case = scenario(&source, "S12");
    let (c, mut w) = fixture(&source, case);
    let timeline = case["when"].as_array().unwrap();
    let mut observed = accept(&mut w, &c, &timeline[0], &source);
    let pending = w.members[0].pending.as_ref().unwrap();
    let action_id = pending.id.clone();
    assert_eq!(pending.target.as_ref().unwrap().id, "B");
    assert!(pending.reservation.is_some());
    let mut now = u(&source["fixture"], "nowMs");
    let at = u(&timeline[1], "atMs");
    assert!(pending.commits > at);
    assert_eq!(w.members[1].hp, 1);
    advance(&mut w, &c, &mut now, at - 1, &mut observed);
    assert_eq!(
        timeline[1]["domainEvent"],
        json!({"kind":"Death","targetId":"B"})
    );
    w.members[1].hp = 0;
    w.terminal_intent(1, Outcome::Death, &mut observed);
    advance(
        &mut w,
        &c,
        &mut now,
        u(&timeline[2], "advanceToMs"),
        &mut observed,
    );
    let oracle = case["then"].as_array().unwrap();
    assert_eq!(w.members[0].spirit, oracle[0]["value"]);
    assert_eq!(
        observed
            .iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::HealResolved { .. }
                }
            ))
            .count(),
        oracle[1]["value"]
    );
    // Accepted preparation is known to exist above, so only its specific
    // final strict-target invalidation maps to the source SkillInterrupted fact.
    assert_eq!(observed.iter().filter(|(owner,e)|*owner==Some(0)&&matches!(e,EventV4::Private{event:Event::ActionRejected{reason:Error::InvalidTarget,action_id:id}} if id==&action_id)).count(),oracle[2]["value"]);
    assert!(w.members[0].pending.is_none() && w.members[0].cooldowns.is_empty());
    assert_eq!(w.members[1].hp, 0);
}
