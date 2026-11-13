use super::*;

fn encounter_input(v: &mut Value) {
    v["params"]["entry_years"] = json!(0);
    v["carried_quantity"] = json!(0);
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["pve"]["monster_positions"] = json!([[52, 20], [52, 40], [70, 20], [35, 40]]);
    v["pve"]["combat"]["aggro_range"] = json!(2);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["match_mode"] = json!({
        "stages":[
            {"id":"guard","duration_ms":180,"objective":{"kind":"kill_quota","target_weights":{"npc-1":1,"npc-2":2},"required":3,"advance_on_timeout":true}},
            {"id":"final","duration_ms":450,"absolute_deadline":true,"objective":{"kind":"waves","waves":[{"delay_ms":0,"target_ids":["npc-4"]}]}}
        ],
        "results":{
            "win":{"result_id":"passed","policy":"preserve"},
            "loss":{"result_id":"failed","policy":"attrition"},
            "draw":{"result_id":"tied","policy":"preserve"}
        }
    });
    v["world_loot"] = json!({
        "environment_capacity":0,"container_capacity":0,"one_shot_capacity":8,
        "partial_pickup":true,"interaction_range":16,"shared_clearance":0,
        "pools":{},"nodes":[],"containers":[],
        "monster_pools":{"npc":{"pool":"base","priority_ms":0,"correction_radius":0}}
    });
    for (id, quantity) in [("base", 1), ("tier", 2), ("surge", 3), ("rest", 4)] {
        v["world_loot"]["pools"][id] = json!({
            "guaranteed":[{"item_id":"item","quantity_min":quantity,"quantity_max":quantity}],
            "independent":[],"groups":[]
        });
    }
    v["encounter_progress"] = json!({
        "initial":10,"kill_weights":{"npc-1":4,"npc-2":6,"npc-3":6,"npc-4":6},
        "affected_templates":["npc"],
        "tiers":[
            {"remaining_at_most":0,"modifier":{"stats_bps":15000,"loot_pools":{"npc":"rest"}}},
            {"remaining_at_most":6,"modifier":{"stats_bps":12500,"loot_pools":{"npc":"tier"}}}
        ],
        "surge":{"duration_ms":120,"modifier":{"stats_bps":17500,"loot_pools":{"npc":"surge"}}}
    });
}

fn hit(w: &mut World, i: usize, j: usize, raw: u32, c: &Config, now: u64) -> u32 {
    w.resolve_damage(
        combat::DamageIntent {
            source: w.members[i].reference.entity.clone(),
            target: w.npcs[j].entity.clone(),
            raw,
            direct: true,
            ignore_armor: true,
        },
        c,
        now,
        &mut vec![],
    )
    .hp_damage
}

fn loot(w: &World) -> Vec<u32> {
    w.resources
        .iter()
        .filter_map(|r| r.view.item_id.as_ref().map(|_| r.view.quantity))
        .collect()
}

#[test]
fn weighted_kills_reprofile_live_and_future_targets_and_trigger_one_timed_surge() {
    let c = catalog(input(), encounter_input).unwrap().default_config();
    let mut w = world(&c);
    assert_eq!(w.npcs[3].hp, 0);
    assert!(!w.mode_npc_visible("npc-4"));
    assert_eq!(w.encounter_view().unwrap().remaining, 10);
    assert_eq!(hit(&mut w, 1, 1, 50, &c, 30), 50);
    assert_eq!(hit(&mut w, 0, 0, 1000, &c, 30), 100);
    w.tick(&c, 30);
    assert_eq!(loot(&w), [1]);
    assert_eq!(w.encounter_view().unwrap().remaining, 6);
    assert_eq!((w.npcs[1].profile.stats["hp"], w.npcs[1].hp), (125, 62));
    assert_eq!(w.npcs[1].profile.stats["atk"], 12);
    assert_eq!(w.mode_view(0, &c).unwrap().progress.unwrap().current, 1);
    assert_eq!(hit(&mut w, 1, 1, 1000, &c, 60), 62);
    w.tick(&c, 60);
    assert_eq!(loot(&w), [1, 2]);
    assert_eq!(w.encounter_view().unwrap().surge_ends_at_ms, Some(180));
    let stage = w.mode_view(0, &c).unwrap();
    assert_eq!(
        (stage.stage_index, stage.opens_at_ms, stage.expires_at_ms),
        (2, 60, 630)
    );
    assert!(w.mode_npc_visible("npc-4"));
    assert_eq!((w.npcs[3].hp, w.npcs[3].profile.stats["hp"]), (175, 175));
    assert_eq!(hit(&mut w, 0, 2, 1000, &c, 90), 175);
    w.tick(&c, 90);
    assert_eq!(loot(&w), [1, 2, 3]);
    assert_eq!(w.encounter_view().unwrap().surge_ends_at_ms, Some(180));
    w.tick(&c, 180);
    assert_eq!(
        w.encounter_view().unwrap(),
        EncounterProgressView {
            initial: 10,
            remaining: 0,
            stats_bps: 15000,
            surge_started: true,
            surge_ends_at_ms: None
        }
    );
    assert_eq!(w.npcs[3].hp, 150);
    assert_eq!(
        c.pve.as_ref().unwrap().profile("npc").unwrap().stats["hp"],
        100
    );
    assert_eq!(hit(&mut w, 1, 3, 1000, &c, 210), 150);
    w.tick(&c, 210);
    assert_eq!(loot(&w), [1, 2, 3, 4]);
    assert_eq!(c.pve_loot_bound("item", 1, 210), 4);
    assert!(w.members.iter().all(|m| m.desired == Some(Outcome::Win)));
    assert!(w.encounter_view().unwrap().surge_ends_at_ms.is_none());
}

#[test]
fn guard_timeout_opens_the_same_final_stage_without_granting_a_result() {
    let c = catalog(input(), encounter_input).unwrap().default_config();
    let mut w = world(&c);
    hit(&mut w, 0, 0, 1000, &c, 30);
    w.tick(&c, 30);
    assert_eq!(w.mode_view(0, &c).unwrap().progress.unwrap().current, 1);
    hit(&mut w, 0, 1, 115, &c, 150);
    let target = World::unit_wire(&w.npcs[1].entity);
    w.action(
        0,
        &Action::CastUnit {
            skill_id: "skill".into(),
            target,
        },
        "deadline",
        &c,
        150,
    )
    .unwrap();
    let events = w.tick(&c, 180);
    assert!(
        events
            .iter()
            .any(|(_, e)| matches!(e, EventV4::ModeStageOpened { stage_index: 2, .. }))
    );
    let stage = w.mode_view(0, &c).unwrap();
    assert_eq!(
        (stage.stage_index, stage.opens_at_ms, stage.expires_at_ms),
        (2, 180, 630)
    );
    assert_eq!(stage.outcome, None);
    assert_eq!(w.npcs[3].hp, 125);
    assert_eq!(w.npcs[1].hp, 10);
    assert_eq!(w.encounter_view().unwrap().remaining, 6);
    w.tick(&c, 630);
    assert!(w.members.iter().all(|m| m.desired == Some(Outcome::Loss)));
}

#[test]
fn participant_defeat_does_not_advance_encounter_progress() {
    let c = catalog(input(), |v| {
        encounter_input(v);
        v.as_object_mut().unwrap().remove("match_mode");
        v["members"]["member_conflict"] = json!(true);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let result = w.resolve_damage(
        combat::DamageIntent {
            source: w.members[0].reference.entity.clone(),
            target: w.members[1].reference.entity.clone(),
            raw: 1000,
            direct: true,
            ignore_armor: true,
        },
        &c,
        30,
        &mut vec![],
    );
    assert!(result.lethal);
    assert_eq!(w.encounter_view().unwrap().remaining, 10);
    assert!(!w.encounter_view().unwrap().surge_started);
    assert_eq!(hit(&mut w, 0, 0, 1000, &c, 30), 100);
    assert_eq!(w.encounter_view().unwrap().remaining, 6);
}

#[test]
fn encounter_validation_rejects_unknown_loot_and_unreachable_quota() {
    for (key, value, expected) in [
        (
            "/encounter_progress/surge/modifier/loot_pools/npc",
            json!("missing"),
            "CONFIG_ENCOUNTER_MODIFIER",
        ),
        (
            "/match_mode/stages/0/objective/required",
            json!(4),
            "CONFIG_MATCH_MODE_OBJECTIVE",
        ),
    ] {
        let r = catalog(input(), |v| {
            encounter_input(v);
            *v.pointer_mut(key).unwrap() = value;
        });
        assert!(r.err().unwrap().to_string().contains(expected));
    }
}
