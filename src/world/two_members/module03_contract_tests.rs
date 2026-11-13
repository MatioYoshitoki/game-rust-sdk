use super::*;

fn boss_input(v: &mut Value) {
    let p = &mut v["pve"];
    p.as_object_mut().unwrap().remove("status_rules");
    p["monster_positions"] = json!([]);
    let mut combat = p["combat"].clone();
    combat["projectile"]["windup_ms"] = json!(60);
    combat["projectile"]["cost"] = json!(10);
    let mut alternate = combat["projectile"].clone();
    alternate["id"] = json!("phase-shot");
    alternate["cost"] = json!(7);
    p["additional_templates"] = json!({"phase-template": {
        "name":"synthetic phases", "mechanism":"melee_projectile_heal", "stats":p["stats"],
        "combat":combat, "drop_table":p["drop_table"], "leash":64, "help_range":1,
        "memory_range":64, "kill_credit":true, "drop_orb":false,
        "boss":{"phases":[
            {"id":"first","name":"First","hp_threshold_bps":10000,"attack_interval_ms":1000,"cast_chance":1},
            {"id":"second","name":"Second","hp_threshold_bps":8000,"attack_interval_ms":600,"cast_chance":1},
            {"id":"third","name":"Third","hp_threshold_bps":5000,"attack_interval_ms":350,"cast_chance":1}
        ]}
    }});
    p["extra_spawns"] =
        json!([{ "id":"phase-unit","template_id":"phase-template","position":[52,20] }]);
    p["npc_ai"] = json!({"switch_threat_bps":12000, "profiles":{"phase-template":{
        "slots":[{"kind":"projectile","chance":1},{"kind":"projectile","chance":1,"ability":alternate}],
        "phase_slots":{"first":[0],"second":[],"third":[1]}
    }}});
}

#[test]
fn boss_crosses_every_stage_keeps_prepared_slot_and_only_resets_at_home() {
    let c = catalog(input(), boss_input).unwrap().default_config();
    let mut w = world(&c);
    let entity = w.npcs[0].entity.clone();
    w.tick(&c, 30);
    assert!(w.npcs[0].casting.is_some());
    let mut events = vec![];
    w.resolve_damage(
        combat::DamageIntent {
            source: w.members[0].reference.entity.clone(),
            target: entity.clone(),
            raw: 60,
            direct: false,
            ignore_armor: true,
        },
        &c,
        30,
        &mut events,
    );
    let changes: Vec<_> = events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::BossPhaseChanged {
                from_index, phase, ..
            } => Some((*from_index, phase.phase_index)),
            _ => None,
        })
        .collect();
    assert_eq!(changes, [(1, 2), (2, 3)]);
    assert_eq!(w.npcs[0].hp, 40);
    w.restore_hp(&entity, 60, &c, 60);
    w.tick(&c, 60);
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 60).monsters[0]
            .boss_phase
            .as_ref()
            .unwrap()
            .phase_index,
        3
    );
    assert!(w.tick(&c, 90).iter().any(|(_, event)| matches!(event,
        EventV4::NpcSkillCommitted { skill_id, .. } if skill_id == "npc-cast"
    )));
    w.tick(&c, 120);
    w.tick(&c, 150);
    w.tick(&c, 180);
    assert!(w.tick(&c, 210).iter().any(|(_, event)| matches!(event,
        EventV4::NpcSkillCommitted { skill_id, .. } if skill_id == "phase-shot"
    )));
    assert_eq!(w.npcs[0].spirit, 83);
    w.npcs[0].position = [67., 20.];
    w.protect(0, 210, &mut vec![]);
    w.protect(1, 210, &mut vec![]);
    let mut events = w.tick(&c, 240);
    assert!(w.npcs[0].returning);
    assert_eq!(w.npcs[0].boss_view().unwrap().phase_index, 3);
    for step in 9..=22 {
        events.extend(w.tick(&c, step * 30));
    }
    let reset = events.iter().filter(|(_, e)| matches!(e,
        EventV4::BossReset { entity, phase } if entity.generation == 2 && phase.phase_index == 1 && phase.epoch == 2
    )).count();
    assert_eq!(reset, 1);
    assert_eq!(w.npcs[0].hp, 100);
}

fn participant_input(v: &mut Value) {
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["pve"]["monster_positions"] = json!([]);
    v["params"]["movement"]["base"] = json!(200);
    v["inventory_capacity"] = json!(1);
    v["participant_ai"] = json!({"b":{
        "decision_interval_ms":30,"loot_radius":100,"threat_radius":0,
        "withdraw_hp_bps":2500,"extract_after_looted":2,"extract_before_timeout_ms":0,
        "search_points":[[36,40]]
    }});
}

fn material(id: &str, point: [f64; 2], quantity: u32, owner: Option<EntityRef>) -> Resource {
    Resource {
        view: ResourceViewV4 {
            id: id.into(),
            position: point,
            quantity,
            item_id: Some("item".into()),
            owner,
            expires_at_ms: 2000,
            instance: None,
        },
        hp: 0,
        spirit: 0,
        player_drop: false,
        origin: Default::default(),
    }
}

#[test]
fn participant_ai_uses_inventory_ownership_and_shared_extraction() {
    let c = catalog(input(), participant_input)
        .unwrap()
        .default_config();
    let mut w = world(&c);
    let human = w.members[0].reference.entity.clone();
    w.resources
        .push(material("owned", [20., 39.], 1, Some(human)));
    w.resources.push(material("full", [20., 39.], 4, None));
    w.resources.push(material("shared", [20., 34.], 2, None));
    w.protect(1, 0, &mut vec![]);
    w.tick(&c, 30);
    assert_eq!(w.resources.len(), 3);
    w.action(1, &Action::Foreground {}, "foreground", &c, 30)
        .unwrap();
    w.action(
        1,
        &Action::CombatReady {
            publication_id: "ready".into(),
        },
        "ready",
        &c,
        30,
    )
    .unwrap();
    let loot = w.tick(&c, 60);
    assert!(loot.iter().any(|(_, e)| matches!(e,
        EventV4::ResourceTaken { resource_id, participant } if resource_id == "shared" && participant.id == "b"
    )));
    assert_eq!(
        w.resources
            .iter()
            .map(|r| r.view.id.as_str())
            .collect::<Vec<_>>(),
        ["owned", "full"]
    );
    let take = Action::Pickup {
        resource_id: "shared".into(),
        expected_revision: w.members[0].revision,
    };
    assert_eq!(
        w.action(0, &take, "take", &c, 60).unwrap_err(),
        Error::InvalidTarget
    );
    assert_eq!(w.members[0].inventory["item"], 1);
    assert_eq!(w.members[1].inventory["item"], 3);
    for step in 3..=40 {
        w.tick(&c, step * 30);
    }
    assert_eq!(w.members[1].desired, Some(Outcome::Extraction));
    assert!(w.members[0].eligible());
    assert!(matches!(
        w.settlement(1, &c),
        Business::CommitSettlement {
            looted: 2,
            kept: 3,
            ..
        }
    ));
}

#[test]
fn participant_ai_searches_and_latches_low_hp_withdrawal() {
    let c = catalog(input(), participant_input)
        .unwrap()
        .default_config();
    let mut w = world(&c);
    w.tick(&c, 30);
    assert!(w.members[1].position[0] > 20.);
    assert_eq!(w.members[1].path.last(), Some(&[36., 40.]));
    w.members[1].hp = 25;
    w.resources.push(material("near", [28., 40.], 2, None));
    w.tick(&c, 60);
    assert_eq!(w.members[1].path.last(), Some(&c.extraction_point));
    w.members[1].hp = 100;
    for step in 3..=40 {
        w.tick(&c, step * 30);
    }
    assert_eq!(w.members[1].desired, Some(Outcome::Extraction));
    assert_eq!(w.resources[0].view.id, "near");
}

#[test]
fn extended_ai_rejects_invalid_stage_slots_and_search_points() {
    let invalid_boss = catalog(input(), |v| {
        boss_input(v);
        v["pve"]["npc_ai"]["profiles"]["phase-template"]["phase_slots"]["third"] = json!([0, 0]);
    })
    .err()
    .unwrap();
    assert_eq!(invalid_boss.to_string(), "CONFIG_NPC_AI_BOSS_SLOTS");
    let invalid_point = catalog(input(), |v| {
        participant_input(v);
        v["participant_ai"]["b"]["search_points"] = json!([[129, 10]]);
    })
    .err()
    .unwrap();
    assert_eq!(
        invalid_point.to_string(),
        "CONFIG_PARTICIPANT_AI_SEARCH_POINT"
    );
}
