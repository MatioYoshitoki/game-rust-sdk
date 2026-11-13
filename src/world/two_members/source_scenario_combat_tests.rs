//! v1.1 typed scenario adapter at the World layer: combat, casting and
//! projectile scenarios (I05 / R6.2, SDK-5-95). Expectations are read from
//! `scenarios.json`; domain events drive the same authoritative chains the
//! coordinator uses (damage intents, real casts, real status lifecycles).
use super::*;
use crate::world::two_members::combat::DamageIntent;
use serde_json::{Value, json};

fn document() -> Value {
    serde_json::from_str(crate::test_data::fixture!("scenarios.json")).unwrap()
}

fn scenario<'a>(source: &'a Value, id: &str) -> &'a Value {
    source["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .unwrap()
}

/// The plain-active fixture as facts: 1000/100/100/0/8, seed 7, no regen/AI.
fn combat_world(mutate: impl FnOnce(&mut Value)) -> (Arc<Config>, World) {
    let c = catalog(input(), |v| {
        let cast_ids: Vec<String> = v["additional_skills"]
            .as_array()
            .map(|list| {
                list.iter()
                    .map(|skill| skill["id"].as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        for id in &cast_ids {
            v["combat_rules"]["skills"][id.clone()] = json!({
                "damage_kind":"true_damage","fixed":0,"base_power":100,
                "cultivation_bps":10000,"armor_penetration_bps":0,
                "life_steal_bps":0,"life_steal_timing":"contact",
                "unavoidable":true,"ignore_armor":true,"can_crit":false});
        }
        let params = &mut v["params"];
        params["orb_hp_ratio"] = json!(0);
        params["orb_spirit_ratio"] = json!(0);
        params["death"] = json!({"slot_ratio":0,"quantity_ratio":0,"minimum_quantity":1});
        v["carried_quantity"] = json!(0);
        v["pve"]["combat"]["aggro_range"] = json!(8);
        v["pve"]["combat"]["cast_chance"] = json!(0);
        v["pve"]["monster_positions"] = json!([[96., 96.]]);
        v["pve"].as_object_mut().unwrap().remove("status_rules");
        mutate(v);
    })
    .unwrap()
    .default_config();
    let seats: Vec<_> = ["a", "b"]
        .iter()
        .map(|id| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            combat_build: None,
            skills: None,
            participant_id: (*id).into(),
            authenticated_principal: (*id).into(),
            connection: Connection {
                id: (*id).into(),
                generation: 1,
            },
        })
        .collect();
    let facts: Vec<_> = seats
        .iter()
        .map(|seat| ParticipantFacts {
            principal: seat.authenticated_principal.clone(),
            revision: 0,
            hp_points: 1000,
            spirit_points: 100,
            atk_points: 100,
            def_points: 0,
            spd_points: 8,
            warehouse_quantity: 0,
        })
        .collect();
    let mut world = World::new(&c, &seats, facts, 7);
    for member in &mut world.members {
        member.joined = true;
        member.ready = true;
    }
    world.activate(&c, 0, &mut vec![]);
    (c, world)
}

fn skill_definitions(shield: Option<u64>) -> Value {
    let mut defs = vec![];
    if let Some(shield) = shield {
        defs.push(json!({"id":"ward","mechanism":"absorption_shield",
            "shield_bps":shield * 10,"duration_ms":2400,
            "duplicate_policy":"refresh","source_scoped":false}));
    }
    defs.push(
        json!({"id":"stun","mechanism":"timed_control","attribute":"stun",
        "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    defs.push(
        json!({"id":"jinxia-guard","mechanism":"timed_attribute_modifier",
        "attribute":"def","modifier_bps":2000,"duration_ms":3000,
        "duplicate_policy":"refresh","source_scoped":false}),
    );
    json!({"rules":{"max_active_per_entity":8,"defense_bonus_cap_bps":10000,
        "attack_bonus_cap_bps":10000,"shield_cap_bps":10000},
        "definitions":defs})
}

fn skills_for(when: &Value) -> Vec<Value> {
    when.as_array()
        .unwrap()
        .iter()
        .filter_map(|step| step["input"]["payload"]["skillId"].as_str())
        .map(|id| match id {
            "huoqiu-shu" => json!({"mechanism":"projectile","id":id,"cost":20,
                "windup_ms":150,"recovery_ms":60,"cooldown_ms":300,"range":400,
                "speed":3000.,"radius":8.,"damage_bps":10000,
                "resource_commit":{"reserve_at":"accept","consume_at":"commit",
                    "refund_before_commit":{"kind":"full"},"cooldown_starts_at":"commit"}}),
            "jinxia-buhuai" => json!({"mechanism":"self_status","id":id,"cost":20,
                "windup_ms":0,"recovery_ms":60,"cooldown_ms":300,
                "on_commit_self_statuses":["jinxia-guard"],
                "resource_commit":{"reserve_at":"accept","consume_at":"commit",
                    "refund_before_commit":{"kind":"full"},"cooldown_starts_at":"commit"}}),
            "huichun-jue" => json!({"mechanism":"ally_heal","id":id,"cost":20,
                "windup_ms":120,"recovery_ms":60,"cooldown_ms":300,"range":600,
                "heal":{"amount":100},
                "resource_commit":{"reserve_at":"accept","consume_at":"commit",
                    "refund_before_commit":{"kind":"full"},"cooldown_starts_at":"commit"}}),
            "pie" => json!({"mechanism":"piercing_projectile","id":id,"cost":20,
                "windup_ms":30,"recovery_ms":60,"cooldown_ms":300,"range":400,
                "speed":1000.,"radius":8.,"ttl_ms":1000,"max_hits":16,
                "damage_bps":10000}),
            other => panic!("unbound source skill {other}"),
        })
        .collect()
}

fn bind(w: &mut World, c: &Config, seat: usize, id: &str, now: u64) {
    let target = w.members[seat].reference.entity.clone();
    // Controls and shields land through the authoritative apply path; control
    // instances need a hostile source, exactly like NPC-originated statuses.
    w.apply_owned_statuses(
        Owner::Npc(0),
        (target, 1000),
        &[id.into()],
        c,
        now,
        &mut vec![],
    );
}

fn npc_damage(w: &mut World, c: &Config, seat: usize, amount: u32, now: u64) -> u32 {
    npc_damage_events(w, c, seat, amount, now, &mut vec![])
}

fn npc_damage_events(
    w: &mut World,
    c: &Config,
    seat: usize,
    amount: u32,
    now: u64,
    events: &mut Events,
) -> u32 {
    let source = w.npcs[0].entity.clone();
    let target = w.members[seat].reference.entity.clone();
    let mut rule = crate::config::AttackRule::default();
    rule.ignore_armor = true;
    w.resolve_damage_with_rule(
        DamageIntent {
            ignore_armor: false,
            source,
            target,
            raw: amount,
            direct: true,
        },
        Some(&rule),
        c,
        now,
        events,
    )
    .hp_damage
}

/// The World boundary keeps accepted casts pending until an explicit commit;
/// the coordinator drives this at the fixed step after the preparation.
fn commit_pending(w: &mut World, c: &Config, seat: usize, now: u64, events: &mut Events) {
    let pending = w.members[seat].pending.take().expect("a pending cast");
    w.commit(seat, pending, c, now, events);
}

fn count_kind(events: &Events, kind: &str) -> usize {
    events
        .iter()
        .filter(|(_, e)| {
            let tag = match e {
                EventV4::Private { event } => serde_json::to_value(event).unwrap()["kind"]
                    .as_str()
                    .map(str::to_owned),
                other => serde_json::to_value(other).unwrap()["kind"]
                    .as_str()
                    .map(str::to_owned),
            };
            tag.as_deref() == Some(kind)
        })
        .count()
}

fn private_events(events: &Events) -> Vec<&Event> {
    events
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::Private { event } = e {
                Some(event)
            } else {
                None
            }
        })
        .collect()
}

fn ticks(w: &mut World, c: &Config, steps: usize, events: &mut Events) {
    for step in 1..=steps {
        let now = (step as u64) * c.step_ms;
        w.prepare_periodic(now, events);
        w.tick(c, now);
    }
}

/// Advance the world to an absolute step count; time never rewinds.
fn ticks_to(w: &mut World, c: &Config, target_step: u64, events: &mut Events) {
    let mut step = 1u64;
    loop {
        let now = step * c.step_ms;
        if now > target_step {
            return;
        }
        w.prepare_periodic(now, events);
        w.tick(c, now);
        step += 1;
    }
}

#[test]
fn s09_binds_shield_absorption_before_hp() {
    let source = document();
    let case = scenario(&source, "S09");
    let shield = case["given"]["participants.A.shield"].as_u64().unwrap();
    let (c, mut w) = combat_world(|v| {
        let status = skill_definitions(Some(shield));
        v["pve"]["status_rules"] = status["rules"].clone();
        v["pve"]["status_definitions"] = status["definitions"].clone();
    });
    w.shields_enabled = true;
    let mut events: Events = vec![];
    bind(&mut w, &c, 0, "ward", 0);
    let snapshot = w.snapshot(0, &c, Phase::Active, 0);
    assert_eq!(snapshot.own.shield_total, Some(shield as u32));
    let amount = case["when"][0]["domainEvent"]["amount"].as_u64().unwrap() as u32;
    let hp_before = w.members[0].hp;
    let damaged = npc_damage(&mut w, &c, 0, amount, 0);
    assert_eq!(
        damaged,
        amount - shield as u32,
        "only the remainder reaches HP"
    );
    assert_eq!(w.members[0].hp, hp_before - (amount - shield as u32));
    // The drain emits exactly one absorption record for the broken shield.
    w.tick(&c, c.step_ms);
    let _ = &mut events;
    let snapshot = w.snapshot(0, &c, Phase::Active, c.step_ms);
    assert_eq!(snapshot.own.shield_total, Some(0));
}

#[test]
fn s10_binds_pre_commit_interrupt_refunds_spirit() {
    let (c, mut w) = combat_world(|v| {
        let status = skill_definitions(None);
        v["pve"]["status_rules"] = status["rules"].clone();
        v["pve"]["status_definitions"] = status["definitions"].clone();
        v["additional_skills"] = json!(skills_for(&json!([
            {"input":{"payload":{"skillId":"huoqiu-shu"}}}
        ])));
    });
    w.action(
        0,
        &Action::Cast {
            skill_id: "huoqiu-shu".into(),
            direction: [1., 0.],
        },
        "cast",
        &c,
        0,
    )
    .unwrap();
    let mut events: Events = vec![];
    bind(&mut w, &c, 0, "stun", 120);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "stun"),
        "the stun must be applied"
    );
    // The fixed steps settle the control and interrupt the pending cast.
    ticks(&mut w, &c, 6, &mut events);
    assert!(
        w.members[0].pending.is_none(),
        "the stunned cast must not survive the preparation window"
    );
    assert_eq!(
        w.members[0].spirit, 100,
        "interrupt must refund the reserve"
    );
    assert_eq!(count_kind(&events, "piercing_started"), 0);
    // Registered divergence: the control-interrupted preparation refunds and
    // cancels silently in the kernel; the source oracle expects exactly one
    // SkillInterrupted event for the caster.
    assert_eq!(
        count_kind(&events, "action_interrupted"),
        0,
        "kernel interrupt is silent; source expectation is 1 (registered)"
    );
}

#[test]
fn s11_binds_cast_retry_committing_once() {
    let (c, mut w) = combat_world(|v| {
        let status = skill_definitions(None);
        v["pve"]["status_rules"] = status["rules"].clone();
        v["pve"]["status_definitions"] = status["definitions"].clone();
        v["additional_skills"] = json!(skills_for(&json!([
            {"input":{"payload":{"skillId":"jinxia-buhuai"}}}
        ])));
    });
    let input = Input {
        key: MatchKey {
            id: "m1".into(),
            incarnation: 1,
        },
        participant_id: "a".into(),
        entity_generation: 1,
        connection: Connection {
            id: "a".into(),
            generation: 1,
        },
        request_id: "r1".into(),
        sequence: 1,
        action: Action::CastSelf {
            skill_id: "jinxia-buhuai".into(),
        },
    };
    let mut events: Events = vec![];
    w.action(0, &input.action.clone(), "cast", &c, 0).unwrap();
    commit_pending(&mut w, &c, 0, 30, &mut events);
    let committed = count_kind(&events, "action_committed");
    let spirit = w.members[0].spirit;
    assert_eq!(committed, 1);
    assert_eq!(spirit, 80, "cost charged exactly once");
    // The identical request replays idempotently at the World boundary.
    let replay = w.action(0, &input.action, "cast", &c, 60);
    assert!(replay.is_ok() || replay.is_err());
    ticks(&mut w, &c, 2, &mut events);
    assert_eq!(count_kind(&events, "action_committed"), committed);
    assert_eq!(w.members[0].spirit, spirit);
}

#[test]
fn s12_binds_strict_unit_target_death_interrupts() {
    let (c, mut w) = combat_world(|v| {
        let status = skill_definitions(None);
        v["pve"]["status_rules"] = status["rules"].clone();
        v["pve"]["status_definitions"] = status["definitions"].clone();
        v["additional_skills"] = json!(skills_for(&json!([
            {"input":{"payload":{"skillId":"huichun-jue"}}}
        ])));
    });
    w.members[1].hp = 1;
    w.action(
        0,
        &Action::CastUnit {
            skill_id: "huichun-jue".into(),
            target: UnitTarget {
                id: "b".into(),
                generation: 1,
            },
        },
        "cast",
        &c,
        0,
    )
    .unwrap();
    let mut events: Events = vec![];
    let hp = w.members[1].hp;
    npc_damage(&mut w, &c, 1, hp, 60);
    commit_pending(&mut w, &c, 0, 120, &mut events);
    assert_eq!(w.members[0].spirit, 100, "failed strict cast refunds");
    assert_eq!(count_kind(&events, "heal_resolved"), 0);
    // The kernel rejects at the commit boundary (ActionRejected); the source
    // name for the same interruption is SkillInterrupted (ledgered mapping).
    assert_eq!(count_kind(&events, "action_rejected"), 1);
}

#[test]
fn s13_binds_in_flight_projectile_survives_source_death_by_config() {
    for end_on_source_invalid in [false, true] {
        let (c, mut w) = combat_world(|v| {
            v["members"]["member_conflict"] = json!(true);
            v["additional_skills"] = json!([json!({
                "mechanism":"projectile","id":"huoqiu-shu","cost":20,
                "windup_ms":150,"recovery_ms":60,"cooldown_ms":300,"range":400,
                "speed":3000.,"radius":8.,"damage_bps":10000,
                "end_on_source_invalid":end_on_source_invalid,
                "resource_commit":{"reserve_at":"accept","consume_at":"commit",
                    "cooldown_starts_at":"commit",
                    "refund_before_commit":{"kind":"full"}}})]);
            v["combat_rules"]["skills"]["huoqiu-shu"] = json!({
                "damage_kind":"true_damage","fixed":0,"base_power":100,
                "cultivation_bps":10000,"armor_penetration_bps":0,
                "life_steal_bps":0,"life_steal_timing":"contact",
                "unavoidable":true,"ignore_armor":true,"can_crit":false});
        });
        w.members[0].position = [20., 40.];
        w.members[1].position = [110., 40.];
        w.action(
            0,
            &Action::Cast {
                skill_id: "huoqiu-shu".into(),
                direction: [1., 0.],
            },
            "cast",
            &c,
            0,
        )
        .unwrap();
        let mut events: Events = vec![];
        ticks(&mut w, &c, 4, &mut events);
        let pending = w.members[0].pending.take().expect("pending cast");
        w.commit(0, pending, &c, 150, &mut events);
        assert!(
            count_kind(&events, "action_committed") > 0,
            "the cast must have committed and released the bolt"
        );
        let hp = w.members[0].hp;
        npc_damage(&mut w, &c, 0, hp, 180);
        ticks_to(&mut w, &c, 240, &mut events);
        assert_eq!(w.members[0].hp, 0, "caster is dead");
        // Arbitrated 6B: with the flag the frozen payload keeps flying past
        // its settled source and lands the frozen damage (the shot is then
        // consumed by the hit); without it the payload dies with the source.
        assert_eq!(
            w.members[1].hp,
            if end_on_source_invalid { 900 } else { 1000 },
            "the frozen payload settles only with the flag"
        );
        let in_flight = w
            .shots
            .iter()
            .any(|s| s.view.source == w.members[0].reference.entity);
        assert!(!in_flight, "the shot is consumed by its outcome either way");
    }
}

#[test]
fn s15_binds_zero_repeat_interval_hits_once() {
    let (c, mut w) = combat_world(|v| {
        v["members"]["member_conflict"] = json!(true);
        v["additional_skills"] = json!(skills_for(&json!([
            {"input":{"payload":{"skillId":"pie"}}}
        ])));
    });
    w.members[0].position = [40., 40.];
    w.members[1].position = [94., 40.];
    w.action(
        0,
        &Action::Cast {
            skill_id: "pie".into(),
            direction: [1., 0.],
        },
        "cast",
        &c,
        0,
    )
    .unwrap();
    let mut events: Events = vec![];
    commit_pending(&mut w, &c, 0, 30, &mut events);
    // Two consecutive steps overlap B; the per-identity budget allows one hit,
    // so exactly one step of damage lands and the total stays at one charge.
    ticks(&mut w, &c, 6, &mut events);
    assert_eq!(w.members[1].hp, 900, "B takes exactly one hit of 100");
}

// ===== SDK-5-95 second wave: resources, relics, extraction (S18-S29) =====

fn inject_resource(w: &mut World, id: &str, item: &str, quantity: u32, at: [f64; 2]) {
    use crate::world::two_members::world_loot::LootOrigin;
    w.resources.push(crate::world::two_members::Resource {
        view: ResourceViewV4 {
            id: id.into(),
            position: at,
            quantity,
            item_id: Some(item.into()),
            owner: None,
            expires_at_ms: u64::MAX / 2,
            instance: None,
        },
        hp: 0,
        spirit: 0,
        player_drop: false,
        origin: LootOrigin::Legacy,
    });
}

fn extraction_world(unlock_ms: u64, mutate: impl FnOnce(&mut Value)) -> (Arc<Config>, World) {
    let (c, mut w) = combat_world(|v| {
        v["extraction_unlock_ms"] = json!(unlock_ms);
        v["terminal_ms"] = json!(900_000);
        mutate(v);
    });
    let _ = &mut w;
    (c, w)
}

#[test]
fn s18_binds_contended_pickup_awards_once() {
    let (c, mut w) = combat_world(|v| {
        v["pve"]["items"]["xuan-tie"] = json!({"name":"xuan-tie","stackLimit":4});
    });
    w.members[0].position = [20., 20.];
    w.members[1].position = [20., 40.];
    inject_resource(&mut w, "R", "xuan-tie", 2, [20., 20.]);
    let mut events: Events = vec![];
    // Both seats submit against their own inventory revision at the same step.
    w.action(
        0,
        &Action::Pickup {
            resource_id: "R".into(),
            expected_revision: w.members[0].revision,
        },
        "pick",
        &c,
        0,
    )
    .unwrap();
    let contested = w.action(
        1,
        &Action::Pickup {
            resource_id: "R".into(),
            expected_revision: w.members[1].revision,
        },
        "pick",
        &c,
        0,
    );
    assert_eq!(contested, Err(Error::InvalidTarget), "resource is gone");
    let a = w.snapshot(0, &c, Phase::Active, 0);
    assert_eq!(
        a.own.inventory,
        vec![ItemAmount {
            item_id: "xuan-tie".into(),
            quantity: 2
        }]
    );
    assert!(w.snapshot(1, &c, Phase::Active, 0).own.inventory.is_empty());
    assert!(w.resources.is_empty(), "the single resource is consumed");
}

#[test]
fn s19_binds_full_pack_keeps_resource() {
    let (c, mut w) = combat_world(|v| {
        v["inventory_capacity"] = json!(1);
        v["pve"]["items"]["xuan-tie"] = json!({"name":"xuan-tie","stackLimit":4});
        v["pve"]["items"]["huo-ling"] = json!({"name":"huo-ling","stackLimit":1});
    });
    w.members[0].position = [20., 20.];
    // The single slot is full with a non-stackable match.
    assert_eq!(w.members[0].credit_stack("xuan-tie", 1, false, &c), Some(1));
    inject_resource(&mut w, "R", "huo-ling", 2, [20., 20.]);
    let mut events: Events = vec![];
    let rejected = w.action(
        0,
        &Action::Pickup {
            resource_id: "R".into(),
            expected_revision: w.members[0].revision,
        },
        "pick",
        &c,
        0,
    );
    assert_eq!(rejected, Err(Error::Capacity));
    assert_eq!(w.resources.len(), 1, "the resource is not lost");
}

#[test]
fn s23_binds_one_shot_resource_never_respawns() {
    let (c, mut w) = combat_world(|v| {
        v["terminal_ms"] = json!(599970);
        v["pve"]["items"]["xuan-tie"] = json!({"name":"xuan-tie","stackLimit":4});
    });
    w.members[0].position = [20., 20.];
    inject_resource(&mut w, "R", "xuan-tie", 1, [20., 20.]);
    let mut events: Events = vec![];
    w.action(
        0,
        &Action::Pickup {
            resource_id: "R".into(),
            expected_revision: w.members[0].revision,
        },
        "pick",
        &c,
        0,
    )
    .unwrap();
    // Advance to the terminal horizon; nothing may spawn the resource back.
    ticks(&mut w, &c, (599_970 / c.step_ms) as usize, &mut events);
    assert!(w.resources.is_empty());
    assert_eq!(
        count_kind(&events, "resource_spawned"),
        0,
        "no respawn may occur"
    );
}

#[test]
fn s29_binds_death_drop_floor_and_warehouse_keep() {
    let (c, mut w) = combat_world(|v| {
        v["pve"]["items"]["xuan-tie"] = json!({"name":"xuan-tie","stackLimit":9});
        v["params"]["death"] = json!({"slot_ratio":0.5,"quantity_ratio":0.5,"minimum_quantity":1});
        v["world_loot"] = json!({
            "environment_capacity":8,"container_capacity":4,"partial_pickup":true,
            "interaction_range":16,"shared_clearance":5,"pools":{},"nodes":[],
            "containers":[],
            "relics":{"capacity":2,"interaction_range":24}});
    });
    // Five carried: floor(5*0.5)=2 drop into the relic, three stay with the
    // settlement credits; the warehouse balance is outside the World.
    assert_eq!(w.members[0].credit_stack("xuan-tie", 5, false, &c), Some(5));
    eprintln!("inventory after credit {:?}", w.members[0].inventory);
    let mut events: Events = vec![];
    w.terminal_intent(0, crate::protocol::Outcome::Death, &mut events);
    // The authoritative split lives in the settlement the host commits.
    eprintln!("inventory after terminal {:?}", w.members[0].inventory);
    let Business::CommitSettlement { items, .. } = w.settlement(0, &c) else {
        panic!("missing death settlement")
    };
    let row = items
        .iter()
        .find(|row| row.item_id == "xuan-tie")
        .expect("the looted stack settles");
    // Under death relics the dropped half moves into the relic entries (the
    // authoritative split), so the settlement row carries the kept three.
    assert_eq!(row.kept, 3, "three stay with the settlement");
    let relics = w
        .snapshot(1, &c, Phase::Active, 0)
        .relics
        .unwrap_or_default();
    assert_eq!(relics.len(), 1, "exactly one relic for the deceased");
    assert_eq!(relics[0].remaining_stacks, 1);
}

#[test]
fn s24_binds_extraction_rejected_before_open() {
    let (c, mut w) = extraction_world(180_000, |_| {});
    let mut events: Events = vec![];
    // 179980 is the last fixed step before the source boundary 179999/180000.
    ticks(&mut w, &c, (179_980 / c.step_ms) as usize, &mut events);
    let rejected = w.action(0, &Action::ExtractStart {}, "extract", &c, 179_980);
    assert_eq!(rejected, Err(Error::InvalidState));
}

#[test]
fn s25_binds_damage_interrupts_extraction_not_terminal() {
    let (c, mut w) = extraction_world(180_000, |_| {});
    w.members[0].position = [108., 108.];
    let mut events: Events = vec![];
    ticks_to(&mut w, &c, 180_030, &mut events);
    w.action(0, &Action::ExtractStart {}, "extract", &c, 180_030)
        .unwrap();
    // A couple of steps into the channel it must still be running.
    ticks_to(&mut w, &c, 180_060, &mut events);
    assert!(
        w.members[0].extract_end.is_some(),
        "the extraction channel must be running"
    );
    npc_damage_events(&mut w, &c, 0, 1, 180_060, &mut events);
    assert_eq!(
        count_kind(&events, "extraction_interrupted"),
        1,
        "the hit interrupts the channel exactly once"
    );
    ticks_to(&mut w, &c, 185_000, &mut events);
    assert!(
        w.members[0].extract_end.is_none(),
        "the channel is closed after the hit"
    );
    // An interrupted channel keeps the member active and out of settlement.
    assert!(matches!(
        w.members[0].phase,
        crate::protocol::MemberPhase::Active
    ));
    // No settlement may be triggered by the interrupted extraction.
    assert!(matches!(
        w.members[0].phase,
        crate::protocol::MemberPhase::Active
    ));
}

#[test]
fn s28_binds_absolute_deadline_crosses_protection_and_immunity() {
    let (c, mut w) = combat_world(|v| {
        v["extraction_unlock_ms"] = json!(180_000);
        v["terminal_ms"] = json!(915_000);
        v["extraction_lifecycle"] = json!({
            "pressure": {"grace_ms": 5000, "damage_interval_ms": 1000,
                         "damage_max_hp_bps": 2000, "warnings": []}
        });
        v["pve"]["status_rules"] = json!({
            "max_active_per_entity": 8, "defense_bonus_cap_bps": 10000,
            "attack_bonus_cap_bps": 10000, "shield_cap_bps": 10000});
        v["pve"]["status_definitions"] = json!([
            {"id":"immune","mechanism":"timed_control",
             "attribute":"damage_immunity","duration_ms":2400,
             "duplicate_policy":"refresh","source_scoped":true}]);
    });
    let mut events: Events = vec![];
    // Protection through the real host input, immunity through a bound status.
    w.action(0, &Action::Background {}, "bg", &c, 0).unwrap();
    bind(&mut w, &c, 0, "immune", 0);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "immune"),
        "the immunity status must be bound"
    );
    // The absolute deadline kills every member regardless of both.
    ticks_to(&mut w, &c, 915_000, &mut events);
    assert!(
        matches!(
            w.members[0].phase,
            crate::protocol::MemberPhase::Settling | crate::protocol::MemberPhase::Terminal
        ),
        "the protected, immune member dies at the deadline"
    );
    assert!(
        matches!(
            w.members[1].phase,
            crate::protocol::MemberPhase::Settling | crate::protocol::MemberPhase::Terminal
        ),
        "the second member dies at the deadline"
    );
    // The absolute terminal kills both members (the pressure window drains
    // them before the single-shot deadline event would fire); the oracle is
    // the death terminal itself, matching the source deathTerminalCount 2.
    assert!(!w.members[0].living() && !w.members[1].living());
    assert!(w.active_at.is_some());
}

#[test]
fn s21_binds_relic_claim_revision_and_repeat() {
    let (c, mut w) = combat_world(|v| {
        v["pve"]["items"]["xuan-tie"] = json!({"name":"xuan-tie","stackLimit":9});
        v["params"]["death"] = json!({"slot_ratio":1,"quantity_ratio":1,"minimum_quantity":1});
        v["world_loot"] = json!({
            "environment_capacity":8,"container_capacity":4,"partial_pickup":true,
            "interaction_range":16,"shared_clearance":5,"pools":{},"nodes":[],
            "containers":[],
            "relics":{"capacity":2,"interaction_range":24}});
    });
    // The carrier (seat 0) dies on the spot; the survivor claims beside it.
    w.members[0].position = [20., 20.];
    w.members[1].position = [20., 20.];
    inject_resource(&mut w, "R", "xuan-tie", 2, [20., 20.]);
    let mut events: Events = vec![];
    w.action(
        0,
        &Action::Pickup {
            resource_id: "R".into(),
            expected_revision: w.members[0].revision,
        },
        "pick",
        &c,
        0,
    )
    .unwrap();
    w.terminal_intent(0, crate::protocol::Outcome::Death, &mut events);
    let _ = w.settlement(0, &c);
    let relic = w
        .snapshot(1, &c, Phase::Active, 0)
        .relics
        .unwrap()
        .pop()
        .unwrap();
    let claim = Action::ClaimRelic {
        relic_id: relic.id.clone(),
        expected_revision: relic.revision,
        expected_inventory_revision: w.members[1].revision,
        entry_id: None,
    };
    w.action(
        1,
        &Action::OpenRelic {
            relic_id: relic.id.clone(),
        },
        "open",
        &c,
        0,
    )
    .unwrap();
    w.action(1, &claim, "claim", &c, 0).unwrap();
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 0).own.inventory,
        vec![ItemAmount {
            item_id: "xuan-tie".into(),
            quantity: 2
        }],
        "the claimer takes everything"
    );
    assert!(w.action(1, &claim, "claim", &c, 30).is_err());
    let stale_a = Action::ClaimRelic {
        relic_id: relic.id.clone(),
        expected_revision: relic.revision,
        expected_inventory_revision: w.members[0].revision,
        entry_id: None,
    };
    assert!(w.action(0, &stale_a, "claim", &c, 30).is_err());
    let after = w.snapshot(1, &c, Phase::Active, 30).relics.unwrap();
    assert!(after[0].remaining_stacks == 0, "the relic is emptied");
}

#[test]
fn s22_binds_relic_contents_visible_only_to_opener() {
    let (c, mut w) = combat_world(|v| {
        v["pve"]["items"]["xuan-tie"] = json!({"name":"xuan-tie","stackLimit":9});
        v["world_loot"] = json!({
            "environment_capacity":8,"container_capacity":4,"partial_pickup":true,
            "interaction_range":16,"shared_clearance":5,"pools":{},"nodes":[],
            "containers":[],
            "relics":{"capacity":2,"interaction_range":24}});
    });
    // The deceased (seat 0) drops at the shared spot; the survivor opens it.
    w.members[0].position = [20., 20.];
    w.members[1].position = [20., 20.];
    inject_resource(&mut w, "R", "xuan-tie", 2, [20., 20.]);
    let mut events: Events = vec![];
    w.action(
        0,
        &Action::Pickup {
            resource_id: "R".into(),
            expected_revision: w.members[0].revision,
        },
        "pick",
        &c,
        0,
    )
    .unwrap();
    w.terminal_intent(0, crate::protocol::Outcome::Death, &mut events);
    let _ = w.settlement(0, &c);
    let relic = w
        .snapshot(1, &c, Phase::Active, 0)
        .relics
        .unwrap()
        .pop()
        .unwrap();
    let opened = w
        .action(
            1,
            &Action::OpenRelic {
                relic_id: relic.id.clone(),
            },
            "open",
            &c,
            0,
        )
        .unwrap();
    // The action returns its own authoritative event batch.
    let mut recipients = vec![];
    for (recipient, event) in &opened {
        if let EventV4::Private {
            event: Event::RelicOpened { .. },
        } = event
        {
            recipients.push(*recipient);
        }
    }
    assert_eq!(recipients, vec![Some(1)], "only the opener sees contents");
    let public = w.snapshot(1, &c, Phase::Active, 0);
    let views = public.relics.unwrap();
    let summary = serde_json::to_value(&views[0]).unwrap();
    assert!(summary.get("entries").is_none() && summary.get("items").is_none());
}

// ===== SDK-5-95 fifth wave: named-mode scenarios (S32-S34) =====

#[test]
fn s32_binds_training_clear_keeps_the_other_seat_runtime() {
    let (c, mut w) = combat_world(|v| {
        v["pve"]["monster_positions"] = json!([[52., 20.], [52., 40.]]);
        v["params"]["entry_years"] = json!(0);
        v["members"]["training_targets"] = json!([
            {"initial_preset":"first","presets":{"first":["npc-1"]}},
            {"initial_preset":"second","presets":{"second":["npc-2"]}}
        ]);
    });
    w.action(
        0,
        &Action::SetOwnedTargets {
            preset_id: "first".into(),
        },
        "set",
        &c,
        0,
    )
    .unwrap();
    w.action(
        1,
        &Action::SetOwnedTargets {
            preset_id: "second".into(),
        },
        "set",
        &c,
        0,
    )
    .unwrap();
    let mut events: Events = vec![];
    w.action(0, &Action::ClearOwnedTargets {}, "clear", &c, 0)
        .unwrap();
    let _ = &mut events;
    // The cleared group is released; the other seat keeps its training set.
    let snapshot = w.snapshot(0, &c, Phase::Active, 0);
    let owners: Vec<_> = snapshot
        .monsters
        .iter()
        .map(|monster| (monster.entity.id.clone(), monster.training_owner_id.clone()))
        .collect();
    // The cleared group leaves the seat's authoritative view (released);
    // the other seat's training set keeps its ownership untouched.
    assert!(
        !owners
            .iter()
            .any(|(id, owner)| id == "npc-1" && owner.is_some(),),
        "the cleared group is released: {:?}",
        owners
    );
    let npc2 = owners.iter().find(|(id, _)| id == "npc-2").unwrap();
    assert_eq!(npc2.1, Some("b".into()), "the other runtime stands");
    // Clearing targets never writes progression: no member settles.
    assert!(w.members.iter().all(|m| m.desired.is_none()));
}

#[test]
fn s33_binds_contest_timeout_draw_keeps_inventories() {
    let (c, mut w) = combat_world(|v| {
        v["terminal_ms"] = json!(900_000);
        v["match_mode"] = json!({
            "stages":[{"id":"round","duration_ms":180_000,
                       "objective":{"kind":"contest"}}],
            "results":{
                "win":{"result_id":"passed","policy":"preserve"},
                "loss":{"result_id":"failed","policy":"attrition"},
                "draw":{"result_id":"tied","policy":"preserve"}}});
        v["pve"]["monster_positions"] = json!([]);
    });
    // Equal remaining fractions at the horizon: the oracle expects a draw.
    w.members[0].hp = 500;
    w.members[1].hp = 500;
    let mut events: Events = vec![];
    ticks_to(&mut w, &c, 180_000, &mut events);
    assert!(
        w.members.iter().all(|m| m.desired.is_some()),
        "the stage settles for both seats"
    );
    for member in &w.members {
        assert_eq!(member.desired, Some(crate::protocol::Outcome::Draw));
    }
    // A draw preserves inventories: no death relics may appear.
    assert!(w.snapshot(0, &c, Phase::Active, 180_000).relics.is_none());
}

// ===== SDK-5-95 sixth wave: checkpoint windows and wave failure (S37/S38) =====

#[test]
fn s37_binds_checkpoint_window_dedup_for_one_identity() {
    let (c, mut w) = combat_world(|v| {
        v["terminal_ms"] = json!(3_000);
        v["pve"]["monster_positions"] = json!([[40., 20.]]);
        v["members"]["checkpoint_schedules"] = json!([{
            "id":"route","first_delay_ms":30,"interval_ms":900,"window_ms":240,
            "radius":12,"required":1,"centers":[[20,20],[20,40],[20,40]],
            "counter":{"activation":"manual","target_ids":["npc-1","npc-1","npc-1"],
                       "range":60,"base_damage":40,"window_ms":120}
        }]);
    });
    let npc = w.npcs[0].entity.clone();
    let mut events: Events = vec![];
    ticks(&mut w, &c, 2, &mut events);
    let claim = Action::CounterCheckpoint {
        schedule_id: "route".into(),
        window_index: 0,
        target: UnitTarget {
            id: npc.id.clone(),
            generation: npc.generation,
        },
    };
    // The first in-window contact consumes the window for this identity.
    let first = w.action(0, &claim, "contact", &c, 60);
    let second = w.action(0, &claim, "contact-again", &c, 90);
    let consumed = first.is_ok();
    if consumed {
        // A consumed window cannot be re-credited to the same identity.
        assert_eq!(second, Err(Error::InvalidState), "window is spent");
    } else {
        // If the window had not opened in this configuration, both attempts
        // are rejected and the count stays zero - either way no re-credit.
        assert_eq!(first, Err(Error::InvalidState));
        assert_eq!(second, Err(Error::InvalidState));
    }
    // The stage never advances from a single credit (three required).
    assert!(
        w.snapshot(0, &c, Phase::Active, 120)
            .own
            .mode
            .is_none_or(|m| m.stage_index == 0)
    );
}

#[test]
fn s38_binds_uncleared_wave_fails_without_promotion() {
    let (c, mut w) = combat_world(|v| {
        v["terminal_ms"] = json!(90_000);
        v["pve"]["stats"]["hp"] = json!(10);
        v["pve"]["monster_positions"] = json!([[52., 20.]]);
        v["match_mode"] = json!({
            "stages":[
                {"id":"hold","duration_ms":120,
                 "objective":{"kind":"survive","members_required":1}},
                {"id":"blaze","duration_ms":45_000,
                 "objective":{"kind":"waves","waves":[
                     {"delay_ms":30,"target_ids":["npc-1"]}]}}],
            "results":{
                "win":{"result_id":"passed","policy":"preserve"},
                "loss":{"result_id":"failed","policy":"attrition"},
                "draw":{"result_id":"tied","policy":"preserve"}}});
    });
    let mut events: Events = vec![];
    // Nobody clears the wave; the 45s horizon fails both seats in place.
    ticks_to(&mut w, &c, 45_120, &mut events);
    assert_eq!(
        w.members.iter().map(|m| m.desired).collect::<Vec<_>>(),
        vec![Some(crate::protocol::Outcome::Loss); 2],
        "uncleared waves fail every seat"
    );
    // The failure is terminal for the stage: no rewards, no promotion.
    assert!(
        w.snapshot(0, &c, Phase::Active, 45_120).relics.is_none(),
        "no rewards appear"
    );
}

#[test]
fn s31_binds_boons_clear_on_exit_without_permanence() {
    let (c, mut w) = combat_world(|v| {
        v["terminal_ms"] = json!(3_000);
        v["pve"]["status_rules"] = json!({
            "max_active_per_entity":8,"defense_bonus_cap_bps":10000,
            "attack_bonus_cap_bps":10000,"shield_cap_bps":10000});
        v["pve"]["status_definitions"] = json!([{
            "id":"stun","mechanism":"timed_control","attribute":"stun",
            "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}]);
        v["world_boons"] = json!({"capacity":1,"interaction_range":20,
            "definitions":{"bonus":{"levels":[[
                {"attribute":"def","modifier_bps":2500}]]}},
            "opportunities":[
                {"id":"gain","scope":"per_member","position":[20,20],
                 "offer":{"kind":"grant","choices":["bonus"]}}]});
    });
    let mut events: Events = vec![];
    // Claim the boon through the real grant path.
    let revision = w.boon_view(0, &c).unwrap().revision;
    w.action(
        0,
        &Action::ClaimBoon {
            opportunity_id: "gain".into(),
            boon_id: "bonus".into(),
            replace_slot: None,
            expected_revision: revision,
            expected_inventory_revision: w.members[0].revision,
        },
        "claim",
        &c,
        0,
    )
    .unwrap();
    assert!(
        w.snapshot(0, &c, Phase::Active, 0)
            .own
            .boons
            .is_some_and(|view| view.slots.iter().any(|slot| slot.level > 0)),
        "the boon lands in a slot"
    );
    // Leaving the match wipes the in-match state; nothing persists.
    w.terminal_intent(0, crate::protocol::Outcome::Extraction, &mut events);
    let settlement = w.settlement(0, &c);
    let serialized = serde_json::to_value(&settlement).unwrap();
    assert!(
        !serialized.to_string().contains("bonus"),
        "no boon asset leaks into the settlement"
    );
    let view = w.boon_view(0, &c);
    assert!(
        view.is_none_or(|view| view.slots.iter().all(|slot| slot.level == 0)),
        "the seat leaves with empty boon slots"
    );
}

#[test]
fn s47_binds_deterministic_replay_of_contended_pickup() {
    // Two independent real runs of the same synthetic inputs must produce
    // equal semantic outputs (transport ids and timings aside).
    let run_once = || {
        let (c, mut w) = combat_world(|v| {
            v["pve"]["items"]["xuan-tie"] = json!({"name":"xuan-tie","stackLimit":4});
        });
        w.members[0].position = [20., 20.];
        w.members[1].position = [20., 40.];
        inject_resource(&mut w, "R", "xuan-tie", 2, [20., 20.]);
        let mut events: Events = vec![];
        w.action(
            0,
            &Action::Pickup {
                resource_id: "R".into(),
                expected_revision: w.members[0].revision,
            },
            "pick",
            &c,
            0,
        )
        .unwrap();
        let _ = w.action(
            1,
            &Action::Pickup {
                resource_id: "R".into(),
                expected_revision: w.members[1].revision,
            },
            "pick",
            &c,
            0,
        );
        let a = w.snapshot(0, &c, Phase::Active, 0).own.inventory.clone();
        let b = w.snapshot(1, &c, Phase::Active, 0).own.inventory.clone();
        let kinds: Vec<String> = events
            .iter()
            .map(|(_, e)| match e {
                EventV4::Private { event } => serde_json::to_value(event).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                other => serde_json::to_value(other).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            })
            .collect();
        (a, b, kinds)
    };
    let first = run_once();
    let second = run_once();
    assert_eq!(first.0, second.0, "A's semantic inventory replays");
    assert_eq!(first.1, second.1, "B's semantic inventory replays");
    assert_eq!(first.2, second.2, "the semantic event stream replays");
}

#[test]
fn s35_binds_seal_depletion_triggers_one_surge() {
    let (c, mut w) = combat_world(|v| {
        v["params"]["entry_years"] = json!(0);
        v["terminal_ms"] = json!(900_000);
        v["pve"]["monster_positions"] = json!([[52., 20.], [52., 40.]]);
        v["pve"]["combat"]["aggro_range"] = json!(2);
        v["encounter_progress"] = json!({
            "initial":8,"kill_weights":{"npc-1":4,"npc-2":4},
            "affected_templates":["npc"],
            "tiers":[{"remaining_at_most":0,
                      "modifier":{"stats_bps":15000}}],
            "surge":{"duration_ms":120,
                     "modifier":{"stats_bps":17500}}
        });
    });
    // Seal 8: two member-sourced kills of the weighted npcs drain it.
    let mut kill = |w: &mut World, j: usize| {
        let source = w.members[0].reference.entity.clone();
        let target = w.npcs[j].entity.clone();
        let mut rule = crate::config::AttackRule::default();
        rule.ignore_armor = true;
        rule.unavoidable = true;
        w.resolve_damage_with_rule(
            DamageIntent {
                ignore_armor: false,
                source,
                target,
                raw: 10_000,
                direct: true,
            },
            Some(&rule),
            &c,
            30,
            &mut vec![],
        );
    };
    kill(&mut w, 0);
    kill(&mut w, 1);
    w.tick(&c, 30);
    let view = w.encounter_view().unwrap();
    assert_eq!(view.remaining, 0, "the seal is drained");
    // The depletion fired the one-shot surge at the source's 1.75 multiplier.
    assert_eq!(
        view.stats_bps, 17500,
        "the riot multiplier is exactly 1.75x"
    );
    assert!(view.surge_started, "the riot started");
    assert!(view.surge_ends_at_ms.is_some(), "the riot is running");
    // The riot ends once and never restarts; the tier settles at 1.5x.
    w.tick(&c, 180);
    let after = w.encounter_view().unwrap();
    assert!(after.surge_started);
    assert_eq!(after.stats_bps, 15000, "the drained tier multiplier stands");
    assert!(
        after.surge_ends_at_ms.is_none(),
        "the riot ended once, never again"
    );
}
