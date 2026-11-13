use super::*;

fn source() -> Value {
    serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_cast_scenarios.json"
    ))
    .unwrap()
}

pub(super) fn catalog(
    enabled: Option<Value>,
    change: impl FnOnce(&mut Value),
) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    let source = source();
    let fixture = &source["fixture"];
    let row = &source["skills"]["jinxia-buhuai"];
    let lifecycle = &row["castLifecycle"];
    assert_eq!(lifecycle["preparation"]["kind"], "none");
    assert_eq!(lifecycle["resources"]["reserveAt"], "none");
    assert_eq!(lifecycle["resources"]["consumeAt"], "commit");
    assert_eq!(row["factors"][0]["factorId"], "factor_cultivation_guard");
    let shield_bps = &row["factors"][0]["params"][0];
    catalog_with(|v| {
        *v = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/interception-a.json"
        ))
        .unwrap();
        v["id"] = json!("source-instant");
        let bounds = &fixture["geometry"]["bounds"];
        let x = bounds[0].as_f64().unwrap();
        let y = bounds[1].as_f64().unwrap();
        let w = bounds[2].as_f64().unwrap() - x;
        let h = bounds[3].as_f64().unwrap() - y;
        v["geometry"] = json!({"min":[0,0],"max":[w,h],"walls":[]});
        let positions = fixture["participants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                [
                    r["position"][0].as_f64().unwrap() - x,
                    r["position"][1].as_f64().unwrap() - y,
                ]
            })
            .collect::<Vec<_>>();
        v["spawn"] = json!(positions[0]);
        v["extraction_point"] = v["spawn"].clone();
        v["extraction_unlock_ms"] = fixture["extractionPoints"][0]["opensAtMs"].clone();
        v["terminal_ms"] = source["standardTerminalScenario"]["when"][0]["advanceToMs"].clone();
        v["members"]["spawns"] = json!(positions);
        v["members"]["capacity"] = json!(2);
        v["pve"]["width"] = json!(50);
        v["pve"]["height"] = json!(50);
        v["pve"]["cells_rle"] = json!([[0, 2500]]);
        v["pve"]["monster_positions"] = json!([]);
        v["pve"]["extra_spawns"] = json!([]);
        v["pve"]["additional_templates"] = json!({});
        v["pve"]["status_definitions"] = json!([{"id":"source-guard","mechanism":"absorption_shield",
            "shield_bps":shield_bps,"duration_ms":row["factors"][0]["params"][3],
            "duplicate_policy":"refresh","source_scoped":true}]);
        v["params"]["player_radius"] = fixture["participants"][0]["radius"].clone();
        v["carried_quantity"] = json!(0);
        v["params"]["regen"]["near_ratio"] = json!(0);
        v["params"]["regen"]["far_ratio"] = json!(0);
        v["skill"] = json!({"id":row["id"],"mechanism":"self_status","cost":row["spiritCost"],"windup_ms":0,
            "recovery_ms":lifecycle["recovery"]["durationMs"],"cooldown_ms":row["cooldown"].as_u64().unwrap()*1000,
            "on_commit_self_statuses":["source-guard"]});
        if let Some(enabled) = enabled {
            v["skill"]["commit_on_accept"] = enabled;
        }
        v["additional_skills"] = json!([]);
        let rules = v["combat_rules"].as_object_mut().unwrap();
        rules.remove("cast_interceptions");
        for k in ["npc_profiles", "status_policies", "skills"] {
            rules.insert(k.into(), json!({}));
        }
        rules.insert("triggers".into(), json!([]));
        change(v)
    })
}

fn commits(outputs: Vec<Output>, action_id: &str) -> Vec<(u64, u32)> {
    outputs
        .into_iter()
        .filter_map(|o| match o.body {
            OutputBody::EventV4 {
                recipient_id: Some(owner),
                event:
                    EventV4::Private {
                        event:
                            Event::ActionCommitted {
                                action_id: id,
                                skill_id: Some(skill),
                                spirit_cost,
                                ..
                            },
                    },
            } if owner == "A" && id == action_id && skill == "jinxia-buhuai" => {
                Some((o.logical_ms, spirit_cost))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn source_cast_scenario_s11_instant_protocol_retry_commits_and_pays_once_at_original_time() {
    for enabled in [None, Some(json!(true))] {
        let source = source();
        let fixture = &source["fixture"];
        let scenario = source["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "S11")
            .unwrap();
        let input_row = &scenario["when"][0]["input"];
        assert_eq!(input_row["payload"]["intent"], json!({"kind":"none"}));
        let cat = catalog(enabled.clone(), |_| {}).unwrap();
        let c = cat.default_config().as_ref().clone();
        let mut create = create(&c);
        create.protocol_version = 6;
        create.key = serde_json::from_value(
            json!({"id":fixture["matchId"],"incarnation":fixture["incarnation"]}),
        )
        .unwrap();
        create.seed = fixture["seed"].as_u64().unwrap() as u32;
        let mut facts = Vec::new();
        for (seat, row) in create
            .participants
            .iter_mut()
            .zip(fixture["participants"].as_array().unwrap())
        {
            assert_eq!(seat.participant_id, row["id"]);
            assert_eq!(seat.connection.id, row["connectionId"]);
            assert_eq!(seat.connection.generation, row["connectionGeneration"]);
            seat.authenticated_principal = row["ownerId"].as_str().unwrap().into();
            facts.push(serde_json::from_value(json!({
                "principal":row["ownerId"],"revision":0,"hp_points":row["maxHp"],"spirit_points":row["maxSpirit"],
                "atk_points":row["atk"],"def_points":row["def"],"spd_points":row["spd"],"warehouse_quantity":0,
            })).unwrap());
        }
        let mut world = World::new(&c, &create.participants, facts, create.seed);
        for m in &mut world.members {
            m.joined = true;
            m.ready = true;
        }
        world.activate(&c, fixture["activeAtMs"].as_u64().unwrap(), &mut vec![]);
        let mut engine = TwoMembers::create(c.clone(), cat, create.clone()).unwrap();
        engine.phase = Phase::Active;
        engine.now = fixture["nowMs"].as_u64().unwrap();
        engine.world = Some(world);
        assert_eq!(engine.seats[0].sequence, fixture["inputSequences"]["A"]);
        let input:Input=serde_json::from_value(json!({
            "key":create.key,"participant_id":input_row["participantId"],"entity_generation":input_row["entityGeneration"],
            "connection":{"id":input_row["connectionId"],"generation":input_row["connectionGeneration"]},
            "request_id":input_row["requestId"],"sequence":input_row["sequence"],
            "action":{"kind":"cast_self","skill_id":input_row["payload"]["skillId"]}
        })).unwrap();
        assert_eq!(engine.now, scenario["when"][0]["atMs"]);
        assert_eq!(scenario["when"][1]["repeatInput"], 0);
        assert_eq!(scenario["when"][1]["atMs"], engine.now);
        let first = engine.input(input.clone()).unwrap();
        let results = commits(engine.drain(), &input.request_id);
        let retry = engine.input(input.clone()).unwrap();
        assert_eq!(first, retry);
        assert!(commits(engine.drain(), &input.request_id).is_empty());
        let world = engine.world.as_ref().unwrap();
        let view = world.snapshot(0, &c, Phase::Active, engine.now).own;
        if enabled.is_some() {
            assert_eq!(results.len(), scenario["then"][0]["value"]);
            assert_eq!(
                results.iter().filter(|(_, fee)| *fee == 30).count(),
                scenario["then"][1]["value"]
            );
            assert_eq!(results, [(0, 30)]);
            assert_eq!(view.spirit, 70);
            assert!(view.pending_action.is_none());
            assert_eq!(view.shield_total, Some(180));
            assert_eq!(view.player_skills[0].ready_at_ms, 60_000);
        } else {
            assert!(results.is_empty());
            assert_eq!(view.spirit, 100);
            assert!(view.pending_action.is_some());
            engine.tick();
            assert_eq!(
                commits(engine.drain(), &input.request_id),
                [(c.step_ms, 30)]
            );
        }
    }
}

#[test]
fn source_cast_scenario_immediate_commit_rejects_unbounded_or_incompatible_forms() {
    for patch in 0..4 {
        assert!(catalog(Some(json!(true)),|v|match patch{
            0=>v["skill"]["windup_ms"]=json!(30),
            1=>v["skill"]["commit_on_accept"]=Value::Null,
            2=>v["skill"]["channel"]=json!({"duration_ms":60,"interval_ms":60,"pulse_count":1,"spirit_per_pulse":0,
               "recovery":{"remainder_policy":"final","spirit":{"fixed":1,"source_attack_bps":0,"target_max_spirit_bps":0}}}),
            _=>v["mechanism_version"]=json!(104),
        }).is_err());
    }
}
