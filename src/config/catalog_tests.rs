use super::*;
use crate::{Runtime, protocol::*};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Package(PathBuf);
impl Package {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "match-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let source = Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config");
        for file in ["b1.json", "b3-1.json", "b3-2.json"] {
            fs::copy(source.join(file), root.join(file)).unwrap();
        }
        let result = Self(root);
        result.manifest(json!([]));
        result
    }
    fn manifest(&self, archives: Value) {
        fs::write(
            self.0.join("catalog.json"),
            serde_json::to_vec(&json!({
                "schema_version":1,"default_scenario":"b1","active":{"b1":"b1.json","b3-1":"b3-1.json"},"archives":archives
            }))
            .unwrap(),
        )
        .unwrap();
    }
    fn edit(&self, file: &str, change: impl FnOnce(&mut Value)) {
        let path = self.0.join(file);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        change(&mut value);
        fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    }
    fn load(&self) -> io::Result<ConfigCatalog> {
        ConfigCatalog::load(self.0.join("catalog.json"))
    }
    fn rejects(&self, code: &str) {
        let error = match self.load() {
            Ok(_) => panic!("expected {code}"),
            Err(error) => error,
        };
        assert!(error.to_string().contains(code), "{error}");
    }
}
impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn request(config: &Config) -> Create {
    Create {
        protocol_version: 1,
        key: MatchKey {
            id: "config-test".into(),
            incarnation: 1,
        },
        request_id: "create-1".into(),
        config_id: config.id().into(),
        config_hash: config.content_hash().into(),
        seed: 7,
        participant_id: "A".into(),
        authenticated_principal: "principal-A".into(),
        connection: Connection {
            id: "conn-A".into(),
            generation: 1,
        },
        publication_mode: PublicationMode::default(),
        recovery: None,
    }
}

#[test]
fn deduplicates_versions_and_shares_prepared_cells() {
    let p = Package::new();
    p.manifest(json!(["b1.json", "b3-1.json"]));
    let c = p.load().unwrap();
    assert_eq!(c.describe()["stats"]["parsedVersions"], 2);
    assert_eq!(c.describe()["stats"]["preparedMaps"], 1);
    let a = c.active("b3-1").unwrap();
    let b = c.active("b3-1").unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    let cloned = a.as_ref().clone();
    assert!(Arc::ptr_eq(
        a.pve.as_ref().unwrap(),
        cloned.pve.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &a.pve.as_ref().unwrap().cells,
        &b.pve.as_ref().unwrap().cells
    ));
    assert_eq!(a.pve.as_ref().unwrap().cells.len(), 65536);
    let started = Instant::now();
    for _ in 0..100_000 {
        std::hint::black_box(c.active("b3-1").unwrap());
    }
    println!(
        "CONFIG_CACHE_MEASURE={}",
        json!({"coldLoadUs":c.load_us,"cachedLookups":100_000,"cachedLookupTotalUs":started.elapsed().as_micros(),"sharedConfig":true,"sharedCells":true})
    );
}

#[test]
fn loaded_runtime_uses_no_source_files_and_rejects_unknown_version() {
    let p = Package::new();
    p.edit("b3-1.json", |v| v["id"] = json!("external-map"));
    let c = Arc::new(p.load().unwrap());
    let config = c.active("b3-1").unwrap();
    fs::remove_dir_all(&p.0).unwrap();
    let runtimes = [Runtime::with_catalog(c.clone()), Runtime::with_catalog(c)];
    for runtime in &runtimes {
        let mut wrong = request(&config);
        wrong.config_hash = "0".repeat(64);
        assert!(matches!(
            runtime.client().create(wrong),
            Err(Error::ConfigMismatch)
        ));
        assert!(runtime.client().create(request(&config)).is_ok());
        assert!(!runtime.client().drain_outputs().unwrap().is_empty());
    }
}

#[test]
fn file_edits_require_reload_and_archived_hash_selects_original_version() {
    let p = Package::new();
    let old = p.load().unwrap();
    let a = old.active("b3-1").unwrap();
    fs::copy(p.0.join("b3-1.json"), p.0.join("old.json")).unwrap();
    p.edit("b3-1.json", |v| {
        v["pve"]["map_id"] = json!("edited-forest");
        v["pve"]["palette"][0]["speedMultiplier"] = json!(0.95);
    });
    p.manifest(json!(["old.json"]));
    let new = p.load().unwrap();
    let b = new.active("b3-1").unwrap();
    assert_ne!(a.content_hash(), b.content_hash());
    assert_eq!(old.active("b3-1").unwrap().content_hash(), a.content_hash());
    assert_eq!(
        new.find(a.id(), a.content_hash())
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .map_id,
        a.pve.as_ref().unwrap().map_id
    );
    assert_eq!(b.pve.as_ref().unwrap().map_id, "edited-forest");
    assert_eq!(new.describe()["stats"]["preparedMaps"], 2);
}

#[test]
fn rejects_bad_rle_without_expanding_unbounded_counts() {
    for (rle, code) in [
        (json!([[0, 1]]), "CONFIG_RLE_COUNT"),
        (json!([[999, 65536]]), "CONFIG_RLE_INDEX"),
        (json!([[0, 0]]), "CONFIG_RLE_INDEX"),
        (json!([[0, u64::MAX]]), "CONFIG_RLE_COUNT"),
    ] {
        let p = Package::new();
        p.edit("b3-1.json", |v| v["pve"]["cells_rle"] = rle);
        p.rejects(code);
    }
}

#[test]
fn rejects_blocked_points_unsupported_terrain_and_invalid_bounds() {
    let p = Package::new();
    p.edit("b3-1.json", |v| {
        v["geometry"]["max"] = json!([256, 256]);
        v["spawn"] = json!([48, 48]);
        v["extraction_point"] = json!([208, 208]);
        let map = &mut v["pve"];
        map["width"] = json!(8);
        map["height"] = json!(8);
        map["monster_positions"] = json!([[80, 48], [48, 80], [80, 80]]);
        let mut blocked = map["palette"][0].clone();
        blocked["walkable"] = json!(false);
        map["palette"] = json!([map["palette"][0], blocked]);
        map["cells_rle"] = json!(
            (0..64)
                .map(|i| vec![usize::from(i % 8 == 3), 1])
                .collect::<Vec<_>>()
        );
    });
    p.rejects("CONFIG_MAP_POINT_OR_PATH");
    let p = Package::new();
    p.edit("b3-1.json", |v| v["spawn"] = json!([-100, 0]));
    p.rejects("CONFIG_MAP_POINT_OR_PATH");
    let p = Package::new();
    p.edit("b3-1.json", |v| {
        v["pve"]["palette"][0]["dotDamagePerSecond"] = json!(1)
    });
    p.rejects("CONFIG_UNSUPPORTED_TERRAIN");
    let p = Package::new();
    p.edit("b3-1.json", |v| {
        v["pve"]["palette"][0]["healPerSecond"] = json!(1)
    });
    p.rejects("CONFIG_UNSUPPORTED_TERRAIN");
    let p = Package::new();
    p.edit("b3-1.json", |v| v["pve"]["width"] = json!(513));
    p.rejects("CONFIG_MAP_BOUNDS");
    let p = Package::new();
    p.edit("b3-1.json", |v| {
        v["pve"]["palette"][0]["speedMultiplier"] = json!(11)
    });
    p.rejects("CONFIG_TERRAIN_SPEED");
    let p = Package::new();
    p.edit("b1.json", |v| v["target"]["position"] = json!([9999, 9999]));
    p.rejects("CONFIG_MAP_POINT");
}

#[test]
fn permits_numeric_changes_and_rejects_unknown_mechanisms() {
    let p = Package::new();
    p.edit("b1.json", |v| {
        v["target"] = json!(
            [
                "id",
                "position",
                "hp",
                "atk",
                "armor",
                "speed",
                "radius",
                "aggro_range",
                "attack_range",
                "interval_ms",
                "drop_quantity"
            ]
            .iter()
            .map(|key| v["target"][*key].clone())
            .collect::<Vec<_>>()
        );
    });
    p.rejects("CONFIG_SCHEMA");
    let p = Package::new();
    p.edit("b3-1.json", |v| v["skill"]["cost"] = json!(1));
    assert_eq!(p.load().unwrap().active("b3-1").unwrap().skill.cost, 1);
    p.edit("b3-1.json", |v| {
        v["skill"]["mechanism"] = json!("unsupported_script")
    });
    p.rejects("CONFIG_SCHEMA");
    let p = Package::new();
    p.edit("b1.json", |v| v["host_tick"] = json!(true));
    p.rejects("CONFIG_SCHEMA");
}

#[test]
fn rejects_manifest_escape_unknown_schema_and_unknown_default() {
    let p = Package::new();
    fs::write(p.0.join("oversized.json"), vec![b' '; 1048577]).unwrap();
    p.manifest(json!(["oversized.json"]));
    p.rejects("CONFIG_FILE_BOUNDS");
    #[cfg(unix)]
    {
        let p = Package::new();
        std::os::unix::fs::symlink(
            Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b1.json"),
            p.0.join("escape.json"),
        )
        .unwrap();
        p.manifest(json!(["escape.json"]));
        p.rejects("CONFIG_FILE_BOUNDS");
    }
    let p = Package::new();
    p.manifest(json!(["../b1.json"]));
    p.rejects("CONFIG_PATH_OUTSIDE_PACKAGE");
    let p = Package::new();
    p.edit("catalog.json", |v| v["schema_version"] = json!(2));
    p.rejects("CONFIG_MANIFEST_SCHEMA");
    let p = Package::new();
    p.edit("catalog.json", |v| v["default_scenario"] = json!("missing"));
    p.rejects("CONFIG_MANIFEST_DEFAULT");
    let p = Package::new();
    fs::remove_file(p.0.join("b3-1.json")).unwrap();
    assert!(p.load().is_err());
}

#[test]
fn consumer_manifest_selects_default_and_uses_aliases_only_as_names() {
    let p = Package::new();
    p.edit("catalog.json", |v| {
        v["active"] = json!({"arena": "b1.json", "b3-19": "b3-1.json", "outpost": "b3-1.json"});
    });
    p.rejects("CONFIG_MANIFEST_DEFAULT");
    p.edit("catalog.json", |v| v["default_scenario"] = json!("outpost"));
    let catalog = Arc::new(p.load().unwrap());
    let map = catalog.active("outpost").unwrap();
    assert!(Arc::ptr_eq(&map, &catalog.default_config()));
    assert!(Arc::ptr_eq(&map, &catalog.active("b3-19").unwrap()));
    assert_eq!(catalog.describe()["defaultScenario"], "outpost");
    let runtime = Runtime::with_catalog(catalog);
    let client = runtime.client();
    assert_eq!(
        client.create(request(&map)).unwrap().phase,
        Phase::Preparing
    );
    assert!(
        client
            .drain_outputs()
            .unwrap()
            .iter()
            .any(|output| { matches!(output.body, OutputBody::BusinessRequest { .. }) })
    );

    p.manifest(json!([]));
    let legacy = p.load().unwrap();
    assert!(Arc::ptr_eq(
        &legacy.default_config(),
        &legacy.active("b1").unwrap()
    ));
}

#[test]
fn chest_profile_loads_alongside_unchanged_legacy_profiles() {
    let p = Package::new();
    p.edit("catalog.json", |v| v["active"]["b3-2"] = json!("b3-2.json"));
    let c = p.load().unwrap();
    assert!(
        c.active("b3-2")
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .treasure_rules
            .is_some()
    );
    assert!(
        c.active("b3-1")
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .chest_spawns
            .is_empty()
    );
    assert_eq!(c.describe()["stats"]["parsedVersions"], 3);
    assert_eq!(c.describe()["stats"]["preparedMaps"], 2);
    p.edit("catalog.json", |v| v["active"]["b3-2"] = json!("b3-1.json"));
    let remapped = p.load().unwrap();
    assert!(Arc::ptr_eq(
        &remapped.active("b3-2").unwrap(),
        &remapped.active("b3-1").unwrap()
    ));
}

#[test]
fn rejects_duplicate_chest_ids_unknown_fields_and_unsupported_chest_mechanics() {
    for mode in 0..7 {
        let p = Package::new();
        p.edit("catalog.json", |v| v["active"]["b3-2"] = json!("b3-2.json"));
        p.edit("b3-2.json", |v| {
            let p = &mut v["pve"];
            match mode {
                0 => p["chest_spawns"][1]["id"] = p["chest_spawns"][0]["id"].clone(),
                1 => p["chest_spawns"][0]["id"] = json!("x".repeat(65)),
                2 => p["chest_spawns"][0]["position"] = json!([-100, 0]),
                3 => p["chest_spawns"][0]["guardian"] = json!(true),
                4 => p["treasure_rules"]["template"]["sealLayers"] = json!(1),
                5 => p["chest_spawns"] = json!([]),
                _ => p["treasure_rules"]["loot"]["guaranteed"][0]["itemId"] = json!("missing-item"),
            }
        });
        let code = match mode {
            0 | 1 => "CONFIG_CHEST_ID_OR_POINT",
            2 => "CONFIG_MAP_POINT_OR_PATH",
            3 => "CONFIG_SCHEMA",
            4 => "CONFIG_SCHEMA",
            _ => "CONFIG_ITEM_REFERENCE",
        };
        if mode == 5 {
            assert!(
                p.load()
                    .unwrap()
                    .active("b3-2")
                    .unwrap()
                    .pve
                    .as_ref()
                    .unwrap()
                    .chest_spawns
                    .is_empty()
            );
        } else {
            p.rejects(code);
        }
    }
}

#[test]
fn chest_positions_reload_without_recompile_and_archived_version_remains_selectable() {
    let p = Package::new();
    p.edit("catalog.json", |v| v["active"]["b3-2"] = json!("b3-2.json"));
    let old = p.load().unwrap().active("b3-2").unwrap();
    fs::copy(p.0.join("b3-2.json"), p.0.join("old-chests.json")).unwrap();
    p.edit("b3-2.json", |v| {
        v["pve"]["chest_spawns"][0]["position"] = v["spawn"].clone()
    });
    p.edit("catalog.json", |v| {
        v["archives"] = json!(["old-chests.json"])
    });
    let new = p.load().unwrap();
    let active = new.active("b3-2").unwrap();
    assert_ne!(active.content_hash(), old.content_hash());
    assert_eq!(
        active.pve.as_ref().unwrap().chest_spawns[0].position,
        active.spawn
    );
    assert_eq!(
        new.find(old.id(), old.content_hash())
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .chest_spawns[0]
            .position,
        [4176.0, 2640.0]
    );
}

fn sealed_package() -> Package {
    let p = Package::new();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-3.json"),
        p.0.join("b3-3.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| v["active"]["b3-3"] = json!("b3-3.json"));
    p
}
#[test]
fn sealed_profile_loads_with_original_unsealed_versions_unchanged() {
    let p = sealed_package();
    let catalog = p.load().unwrap();
    assert!(
        catalog
            .active("b3-3")
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .treasure_rules
            .as_ref()
            .unwrap()
            .template
            .seal
            .is_some()
    );
    assert!(
        catalog
            .active("b3-1")
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .treasure_rules
            .is_none()
    );
    assert_eq!(
        catalog.active("b1").unwrap().content_hash(),
        hash(&fs::read(p.0.join("b1.json")).unwrap())
    );
}
#[test]
fn sealed_configuration_rejects_missing_capability_and_unsupported_fields() {
    let p = sealed_package();
    p.edit("b3-3.json", |v| {
        v["pve"]["treasure_rules"]["template"]["seal"] = json!([2, 1200, 0])
    });
    p.rejects("CONFIG_SCHEMA");
    for (pointer, value, code) in [
        (
            "/mechanism_version",
            json!(1),
            "CONFIG_SEAL_CAPABILITY_OR_BOUNDS",
        ),
        (
            "/pve/treasure_rules/template/seal",
            Value::Null,
            "CONFIG_SEAL_CAPABILITY_OR_BOUNDS",
        ),
        (
            "/pve/treasure_rules/template/seal",
            json!({"layers":2,"layer_duration_ms":1200,"interrupt_retention_ratio":0,"guardian":true}),
            "CONFIG_SCHEMA",
        ),
        (
            "/pve/treasure_rules/template/mechanism",
            json!("guarded_seal"),
            "CONFIG_SCHEMA",
        ),
    ] {
        let p = sealed_package();
        p.edit("b3-3.json", |v| {
            *v.pointer_mut(pointer)
                .unwrap_or_else(|| panic!("missing pointer {pointer}")) = value
        });
        p.rejects(code);
    }
}
#[test]
fn sealed_configuration_bounds_and_time_cross_constraints_are_enforced() {
    for (field, value) in [
        ("layers", json!(0)),
        ("layers", json!(33)),
        ("layer_duration_ms", json!(0)),
        ("layer_duration_ms", json!(29)),
        ("layer_duration_ms", json!(60001)),
        ("interrupt_retention_ratio", json!(-0.1)),
        ("interrupt_retention_ratio", json!(1.1)),
    ] {
        let p = sealed_package();
        p.edit("b3-3.json", |v| {
            v["pve"]["treasure_rules"]["template"]["seal"][field] = value
        });
        p.rejects("CONFIG_SEAL_CAPABILITY_OR_BOUNDS");
    }
    let p = sealed_package();
    p.edit("b3-3.json", |v| {
        v["pve"]["treasure_rules"]["template"]["seal"]["layers"] = json!(32);
        v["pve"]["treasure_rules"]["template"]["seal"]["layer_duration_ms"] = json!(60000);
    });
    p.rejects("CONFIG_SEAL_CAPABILITY_OR_BOUNDS");
}
#[test]
fn sealed_legal_variants_load_and_immediate_mode_cannot_silently_ignore_seal() {
    let p = sealed_package();
    p.edit("b3-3.json", |v| {
        let s = &mut v["pve"]["treasure_rules"]["template"]["seal"];
        s["layers"] = json!(3);
        s["layer_duration_ms"] = json!(900);
        s["interrupt_retention_ratio"] = json!(0.5);
    });
    p.load().unwrap();
    p.edit("b3-3.json", |v| {
        v["pve"]["treasure_rules"]["template"]["mechanism"] = json!("immediate_ground_loot")
    });
    p.rejects("CONFIG_SEAL_CAPABILITY_OR_BOUNDS");
}

fn guarded_package() -> Package {
    let p = sealed_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-4.json"),
        p.0.join("b3-4.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| v["active"]["b3-4"] = json!("b3-4.json"));
    p
}
#[test]
fn guarded_profile_loads_mixed_templates_and_keeps_original_versions() {
    let p = guarded_package();
    let c = p.load().unwrap();
    let g = c.active("b3-4").unwrap();
    assert_eq!(g.mechanism_version, 3);
    assert_eq!(g.pve.as_ref().unwrap().profiles.len(), 2);
    for name in ["b1", "b3-1", "b3-3"] {
        assert_eq!(
            c.active(name).unwrap().content_hash(),
            hash(&fs::read(p.0.join(format!("{name}.json"))).unwrap())
        );
    }
}
#[test]
fn guarded_invalid_references_reserved_ids_and_old_capabilities_fail() {
    for mode in 0..5 {
        let p = guarded_package();
        p.edit("b3-4.json",|v|match mode{0=>v["mechanism_version"]=json!(2),1=>v["pve"]["treasure_rules"]["template"]["guardian"]["template_id"]=json!("missing"),2=>v["pve"]["extra_spawns"]=json!([{"id":"extra","template_id":"missing","position":[4000,1600]}]),3=>v["pve"]["extra_spawns"]=json!([{"id":"guardian-forged","template_id":"verification-guardian","position":[4000,1600]}]),_=>v["pve"]["template_id"]=json!("guardian-forged")});
        assert!(p.load().is_err());
    }
}
#[test]
fn guarded_probability_count_offset_and_budget_bounds_fail_closed() {
    for (field, value) in [
        ("chance", json!(-0.1)),
        ("chance", json!(1.1)),
        ("min_count", json!(0)),
        ("max_count", json!(5)),
        ("spawn_offsets", json!([[0, 0], [0, 0]])),
        ("spawn_offsets", json!([])),
    ] {
        let p = guarded_package();
        p.edit("b3-4.json", |v| {
            v["pve"]["treasure_rules"]["template"]["guardian"][field] = value
        });
        assert!(p.load().is_err());
    }
    let p = guarded_package();
    p.edit("b3-4.json", |v| {
        let chest = v["pve"]["chest_spawns"][0].clone();
        v["pve"]["chest_spawns"] = json!(
            (0..16)
                .map(|i| json!({"id":format!("box-{i}"),"position":chest["position"]}))
                .collect::<Vec<_>>()
        );
        v["pve"]["treasure_rules"]["template"]["guardian"]["max_count"] = json!(2);
        v["pve"]["treasure_rules"]["loot"] = json!({"guaranteed":[],"random":[]});
    });
    p.rejects("CONFIG_POPULATION_BUDGET");
}
#[test]
fn guarded_template_stats_loot_mechanism_and_struct_shapes_are_strict() {
    for mode in 0..6 {
        let p = guarded_package();
        p.edit("b3-4.json", |v| {
            let t = &mut v["pve"]["additional_templates"]["verification-guardian"];
            match mode {
                0 => t["stats"]["hp"] = json!(0),
                1 => t["combat"]["cast_chance"] = json!(1.1),
                2 => t["mechanism"] = json!("guardian_special_script"),
                3 => {
                    t["drop_table"]["guaranteed"] =
                        json!([{"itemId":"unknown","quantityMin":1,"quantityMax":1}])
                }
                4 => *t = json!([]),
                _ => {
                    v["pve"]["treasure_rules"]["template"]["guardian"] =
                        json!(["verification-guardian", 1, 1, 1, []])
                }
            }
        });
        assert!(p.load().is_err());
    }
}
#[test]
fn guarded_points_validate_each_template_radius_and_candidate_paths() {
    let p = guarded_package();
    p.edit(
        "b3-4.json",
        |v| {
            v["pve"]["extra_spawns"] =
                json!([{"id":"outside","template_id":"verification-guardian","position":[1,1]}])
        },
    );
    p.rejects("CONFIG_MAP_POINT_OR_PATH");
    let p = guarded_package();
    p.edit("b3-4.json", |v| {
        v["pve"]["treasure_rules"]["template"]["guardian"]["spawn_offsets"] =
            json!([[1000, 1000], [-1000, -1000]])
    });
    assert!(p.load().is_err());
}
#[test]
fn guarded_lifetime_kill_budget_is_bounded_before_restore_or_world_creation() {
    let p = guarded_package();
    p.edit("b3-4.json", |v| {
        v["terminal_ms"] = json!(3600000);
        let r = &mut v["pve"]["treasure_rules"]["template"];
        r["respawn_delay_ms"] = json!(1000);
        r["seal"]["layers"] = json!(1);
        r["seal"]["layer_duration_ms"] = json!(30);
        r["guardian"]["max_count"] = json!(2);
        r["guardian"]["chance"] = json!(1);
    });
    p.rejects("CONFIG_POPULATION_BUDGET");
}
fn encounter_package() -> Package {
    let p = guarded_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-5.json"),
        p.0.join("b3-5.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| {
        v["active"]["b3-5"] = json!("b3-5.json");
        v["active"]["b3-2"] = json!("b3-2.json");
    });
    p
}
#[test]
fn encounter_profile_caches_templates_and_preserves_five_existing_raw_versions() {
    let p = encounter_package();
    let catalog = p.load().unwrap();
    let c = catalog.active("b3-5").unwrap();
    assert_eq!(c.mechanism_version, 4);
    assert_eq!(c.max_kills(c.terminal_ms), 3);
    for alias in ["b1", "b3-1", "b3-2", "b3-3", "b3-4"] {
        if let Some(c) = catalog.active(alias) {
            assert_eq!(
                c.hash(),
                hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
            );
        }
    }
}
#[test]
fn encounter_rejects_invalid_capability_and_struct_shapes() {
    for mode in 0..8 {
        let p = encounter_package();
        p.edit("b3-5.json", |v| match mode {
            0 => v["mechanism_version"] = json!(3),
            1 => v["pve"]["encounters"] = json!({}),
            2 => v["pve"]["encounters"][0] = json!([]),
            3 => v["pve"]["encounters"][0]["trigger"] = json!([[0, 0], 2]),
            4 => v["pve"]["encounters"][0]["monsters"][0] = json!(["id", "yao-wolf", [1, 2]]),
            5 => v["pve"]["encounters"][0]["phase"] = json!("completed"),
            6 => v["pve"]["encounters"][0]["trigger"]["radius"] = json!("100"),
            _ => v["pve"]["encounters"][0]["required_monster_ids"] = json!([1]),
        });
        assert!(p.load().is_err(), "mode {mode}");
    }
}
#[test]
fn encounter_rejects_duplicate_reserved_and_foreign_references() {
    for mode in 0..9 {
        let p = encounter_package();
        p.edit("b3-5.json", |v| {
            let e = &mut v["pve"]["encounters"][0];
            match mode {
                0 => e["required_monster_ids"] = json!([]),
                1 => e["required_monster_ids"] = json!(["missing"]),
                2 => e["reward_chest_ids"] = json!(["missing"]),
                3 => e["reward_chest_ids"] = json!([]),
                4 => e["monsters"][1]["id"] = e["monsters"][0]["id"].clone(),
                5 => e["monsters"][0]["id"] = json!("guardian-forged"),
                6 => e["monsters"][0]["template_id"] = json!("missing"),
                7 => {
                    e["required_monster_ids"] =
                        json!(["forest-encounter-wolf-1", "forest-encounter-wolf-1"])
                }
                _ => e["reward_chest_ids"] = json!(["encounter-cache-1", "encounter-cache-1"]),
            }
        });
        assert!(p.load().is_err(), "mode {mode}");
    }
}
#[test]
fn encounter_trigger_and_actor_footprints_require_valid_reachable_map_points() {
    for mode in 0..5 {
        let p = encounter_package();
        p.edit("b3-5.json", |v| {
            let e = &mut v["pve"]["encounters"][0];
            match mode {
                0 => e["trigger"]["radius"] = json!(0),
                1 => e["trigger"]["radius"] = json!(1001),
                2 => e["trigger"]["center"] = json!([-100, 0]),
                3 => e["monsters"][0]["position"] = json!([-100, 0]),
                _ => e["monsters"][1]["position"] = e["monsters"][0]["position"].clone(),
            }
        });
        assert!(p.load().is_err(), "mode {mode}");
    }
}
#[test]
fn encounter_groups_cannot_share_rewards_or_global_monster_ids() {
    for mode in 0..3 {
        let p = encounter_package();
        p.edit("b3-5.json", |v| {
            let mut e = v["pve"]["encounters"][0].clone();
            if mode != 0 {
                e["id"] = json!("second");
            }
            if mode == 2 {
                for (i, m) in e["monsters"].as_array_mut().unwrap().iter_mut().enumerate() {
                    m["id"] = json!(format!("second-{i}"));
                }
                e["required_monster_ids"] = json!(["second-0"]);
            }
            v["pve"]["encounters"].as_array_mut().unwrap().push(e);
        });
        assert!(p.load().is_err());
    }
}
#[test]
fn encounter_storage_and_loot_budgets_include_reserved_inactive_groups() {
    let p = encounter_package();
    p.edit("b3-5.json", |v| {
        let e = v["pve"]["encounters"][0].clone();
        for i in 1..9 {
            let mut next = e.clone();
            next["id"] = json!(format!("extra-{i}"));
            for (n, m) in next["monsters"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .enumerate()
            {
                m["id"] = json!(format!("extra-{i}-{n}"));
            }
            next["required_monster_ids"] = json!([format!("extra-{i}-0")]);
            v["pve"]["encounters"].as_array_mut().unwrap().push(next);
        }
    });
    p.rejects("CONFIG_ENCOUNTER_BUDGET");
    let p = encounter_package();
    p.edit("b3-5.json", |v| {
        let m = v["pve"]["encounters"][0]["monsters"][0].clone();
        v["pve"]["monster_positions"] = json!([v["spawn"].clone()]);
        for i in 3..32 {
            let mut next = m.clone();
            next["id"] = json!(format!("extra-{i}"));
            next["position"] = json!([1000 + i * 64, 1000]);
            v["pve"]["encounters"][0]["monsters"]
                .as_array_mut()
                .unwrap()
                .push(next);
        }
    });
    p.rejects("CONFIG_POPULATION_BUDGET");
    let p = encounter_package();
    p.edit("b3-5.json", |v| {
        v["pve"]["drop_table"]["guaranteed"] = json!(
            (0..44)
                .map(|_| json!({"itemId":"xuan-tie","quantityMin":1,"quantityMax":1}))
                .collect::<Vec<_>>()
        )
    });
    p.rejects("CONFIG_RESOURCE_BUDGET");
}
fn wave_package() -> Package {
    let p = encounter_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-6.json"),
        p.0.join("b3-6.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| v["active"]["b3-6"] = json!("b3-6.json"));
    p
}
#[test]
fn wave_profile_loads_without_reinterpreting_six_existing_versions() {
    let p = wave_package();
    let catalog = p.load().unwrap();
    let c = catalog.active("b3-6").unwrap();
    assert_eq!(c.mechanism_version, 5);
    assert_eq!(c.pve.as_ref().unwrap().encounters[0].waves.len(), 2);
    assert_eq!(c.max_kills(c.terminal_ms), 3);
    for alias in ["b1", "b3-1", "b3-2", "b3-3", "b3-4", "b3-5"] {
        let old = catalog.active(alias).unwrap();
        assert_eq!(
            old.hash(),
            hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
        );
    }
}
#[test]
fn wave_capability_mixed_forms_and_structures_are_strict() {
    for mode in 0..10 {
        let p = wave_package();
        p.edit("b3-6.json", |v| match mode {
            0 => v["mechanism_version"] = json!(4),
            1 => {
                v["pve"]["encounters"][0]["monsters"] =
                    v["pve"]["encounters"][0]["waves"][0]["monsters"].clone()
            }
            2 => v["pve"]["encounters"][0]["required_monster_ids"] = json!(["foreign"]),
            3 => v["pve"]["encounters"][0]["waves"] = json!({}),
            4 => v["pve"]["encounters"][0]["waves"][0] = json!(["id", 0, [], []]),
            5 => {
                v["pve"]["encounters"][0]["waves"][0]["monsters"][0] =
                    json!(["id", "yao-wolf", [1, 2]])
            }
            6 => v["pve"]["encounters"][0]["waves"][0]["delay_ms"] = json!(-1),
            7 => v["pve"]["encounters"][0]["waves"][0]["delay_ms"] = json!(1.5),
            8 => v["pve"]["encounters"][0]["waves"][0]["repeat"] = json!(true),
            _ => {
                v["pve"]["encounters"][0]["waves"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("required_monster_ids");
            }
        });
        assert!(p.load().is_err(), "mode {mode}");
    }
}
#[test]
fn wave_ids_targets_and_templates_cannot_alias_other_waves() {
    for mode in 0..8 {
        let p = wave_package();
        p.edit("b3-6.json", |v| {
            let e = &mut v["pve"]["encounters"][0];
            match mode {
                0 => e["waves"][1]["id"] = e["waves"][0]["id"].clone(),
                1 => e["waves"][1]["id"] = json!(""),
                2 => {
                    e["waves"][1]["monsters"][0]["id"] = e["waves"][0]["monsters"][0]["id"].clone()
                }
                3 => {
                    e["waves"][1]["required_monster_ids"] =
                        e["waves"][0]["required_monster_ids"].clone()
                }
                4 => e["waves"][1]["monsters"][0]["template_id"] = json!("missing"),
                5 => e["waves"][1]["monsters"][0]["id"] = json!("guardian-forged"),
                6 => e["waves"][1]["required_monster_ids"] = json!([]),
                _ => e["waves"][1]["monsters"] = json!([]),
            }
        });
        assert!(p.load().is_err(), "mode {mode}");
    }
}
#[test]
fn wave_count_delay_and_total_interval_budgets_are_bounded() {
    let p = wave_package();
    p.edit("b3-6.json", |v| {
        v["pve"]["encounters"][0]["waves"][1]["delay_ms"] = json!(60001)
    });
    p.rejects("CONFIG_WAVE_BOUNDS");
    for (count, delay, code) in [
        (9, 0, "CONFIG_WAVE_BUDGET"),
        (4, 60000, "CONFIG_WAVE_BOUNDS"),
    ] {
        let p = wave_package();
        p.edit("b3-6.json", |v| {
            v["terminal_ms"] = json!(200000);
            let base = v["pve"]["encounters"][0]["waves"][1].clone();
            let mut waves = vec![];
            for i in 0..count {
                let mut w = base.clone();
                w["id"] = json!(format!("wave-{i}"));
                w["delay_ms"] = json!(delay);
                w["monsters"][0]["id"] = json!(format!("target-{i}"));
                w["required_monster_ids"] = json!([format!("target-{i}")]);
                waves.push(w);
            }
            v["pve"]["encounters"][0]["waves"] = json!(waves);
        });
        p.rejects(code);
    }
}
#[test]
fn wave_placement_allows_cross_wave_coordinates_but_checks_future_footprints() {
    let p = wave_package();
    p.edit("b3-6.json", |v| {
        v["pve"]["encounters"][0]["waves"][1]["monsters"][0]["position"] =
            v["pve"]["encounters"][0]["waves"][0]["monsters"][0]["position"].clone()
    });
    p.load().unwrap();
    let p = wave_package();
    p.edit("b3-6.json",|v|{v["spawn"]=json!([200,400]);v["extraction_point"]=json!([900,800]);v["geometry"]["max"]=json!([1024,1024]);let p=&mut v["pve"];p["width"]=json!(32);p["height"]=json!(32);p["cell_size"]=json!(32);p["cells_rle"]=json!([[0,1024]]);p["encounters"][0]["trigger"]["center"]=json!([500,400]);for (i,m) in p["encounters"][0]["waves"][0]["monsters"].as_array_mut().unwrap().iter_mut().enumerate(){m["position"]=json!([600+i*100,400]);}for (i,c) in p["chest_spawns"].as_array_mut().unwrap().iter_mut().enumerate(){c["position"]=json!([900,400+i*250]);}let mut combat=p["combat"].clone();combat["radius"]=json!(32);p["additional_templates"]=json!({"large":{"mechanism":"melee_projectile_heal","name":"large","stats":p["stats"],"combat":combat,"drop_table":p["drop_table"],"leash":p["leash"],"help_range":p["help_range"],"memory_range":p["memory_range"],"kill_credit":true,"drop_orb":true}});p["encounters"][0]["waves"][1]["monsters"][0]["template_id"]=json!("large");p["encounters"][0]["waves"][1]["monsters"][0]["position"]=json!([20,20]);});
    p.rejects("CONFIG_MAP_POINT_OR_PATH");
}
#[test]
fn wave_resource_and_slot_budgets_include_not_yet_generated_enemies() {
    let p = wave_package();
    p.edit("b3-6.json", |v| {
        v["pve"]["monster_positions"] =
            json!((0..32).map(|_| v["spawn"].clone()).collect::<Vec<_>>())
    });
    p.rejects("CONFIG_POPULATION_BUDGET");
    let p = wave_package();
    p.edit("b3-6.json", |v| {
        v["pve"]["drop_table"]["guaranteed"] = json!(
            (0..44)
                .map(|_| json!({"itemId":"xuan-tie","quantityMin":1,"quantityMax":1}))
                .collect::<Vec<_>>()
        )
    });
    p.rejects("CONFIG_RESOURCE_BUDGET");
}
fn boss_package() -> Package {
    let p = wave_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-7.json"),
        p.0.join("b3-7.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| v["active"]["b3-7"] = json!("b3-7.json"));
    p
}
#[test]
fn boss_profile_loads_and_preserves_seven_existing_raw_versions() {
    let p = boss_package();
    let c = p.load().unwrap();
    let b = c.active("b3-7").unwrap();
    assert_eq!(b.mechanism_version, 6);
    assert_eq!(b.max_kills(b.terminal_ms), 3);
    assert_eq!(
        b.pve.as_ref().unwrap().additional_templates["verification-phase-boss"]
            .boss
            .as_ref()
            .unwrap()
            .phases
            .len(),
        2
    );
    for alias in ["b1", "b3-1", "b3-2", "b3-3", "b3-4", "b3-5", "b3-6"] {
        assert_eq!(
            c.active(alias).unwrap().hash(),
            hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
        );
    }
}
#[test]
fn boss_capability_and_nested_structures_reject_unknown_or_positional_data() {
    for mode in 0..8 {
        let p = boss_package();
        p.edit("b3-7.json",|v|match mode{0=>v["mechanism_version"]=json!(5),1=>v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]=json!([]),2=>v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"]=json!({}),3=>v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"][0]=json!(["id","name",10000,1000,0]),4=>v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"][0]["hp"]=json!(10000),5=>v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"][0]["cast_chance"]=json!("0.5"),6=>v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"][0]["attack_interval_ms"]=json!(-1),_=>{v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"][0].as_object_mut().unwrap().remove("name");}});
        assert!(p.load().is_err(), "mode {mode}");
    }
}
#[test]
fn boss_phase_ids_names_and_identity_lengths_are_bounded() {
    for mode in 0..5 {
        let p = boss_package();
        p.edit("b3-7.json", |v| {
            let b =
                &mut v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"];
            match mode {
                0 => b[1]["id"] = b[0]["id"].clone(),
                1 => b[1]["id"] = json!(""),
                2 => b[1]["name"] = json!(""),
                3 => b[1]["id"] = json!("bad\nidentity"),
                _ => b[1]["name"] = json!("x".repeat(65)),
            }
        });
        p.rejects("CONFIG_BOSS_PHASE_ID");
    }
}
#[test]
fn boss_thresholds_require_full_first_and_strictly_decreasing_positive_bps() {
    for (field, value) in [(0, 9999), (0, 10001), (1, 0), (1, 10000), (1, 10001)] {
        let p = boss_package();
        p.edit("b3-7.json",|v|v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"][field]["hp_threshold_bps"]=json!(value));
        p.rejects("CONFIG_BOSS_THRESHOLDS");
    }
}
#[test]
fn boss_effective_timing_and_probability_have_strict_ranges() {
    for (field, value) in [
        ("attack_interval_ms", json!(0)),
        ("attack_interval_ms", json!(3600001)),
        ("attack_interval_ms", json!(1.5)),
        ("cast_chance", json!(-0.1)),
        ("cast_chance", json!(1.01)),
    ] {
        let p = boss_package();
        p.edit("b3-7.json", |v| {
            v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"][0]
                [field] = value
        });
        assert!(p.load().is_err());
    }
    let p = boss_package();
    p.edit("b3-7.json", |v| {
        let phases =
            &mut v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"];
        phases[0]["cast_chance"] = json!(0);
        phases[1]["cast_chance"] = json!(1);
        phases[1]["attack_interval_ms"] = json!(1);
    });
    p.load().unwrap();
}
#[test]
fn boss_one_three_eight_phases_work_and_empty_or_nine_fail() {
    for count in [0, 1, 3, 8, 9] {
        let p = boss_package();
        p.edit("b3-7.json", |v| {
            let base =
                v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"][0]
                    .clone();
            let mut phases = vec![];
            for i in 0..count {
                let mut next = base.clone();
                next["id"] = json!(format!("phase-{i}"));
                next["hp_threshold_bps"] = json!(10000 - i * 1000);
                phases.push(next);
            }
            v["pve"]["additional_templates"]["verification-phase-boss"]["boss"]["phases"] =
                json!(phases);
        });
        if count == 0 || count == 9 {
            p.rejects("CONFIG_BOSS_PHASE_BUDGET");
        } else {
            p.load().unwrap();
        }
    }
}

fn scoring_package() -> Package {
    let p = boss_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-8.json"),
        p.0.join("b3-8.json"),
    )
    .unwrap();
    let path = p.0.join("catalog.json");
    let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    v["active"]["b3-8"] = json!("b3-8.json");
    fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
    p
}
#[test]
fn scoring_capability_loads_and_all_eight_old_raw_identities_stay_pinned() {
    let p = scoring_package();
    let c = p.load().unwrap();
    for alias in ["b1", "b3-1", "b3-2", "b3-3", "b3-4", "b3-5", "b3-6", "b3-7"] {
        assert_eq!(
            c.active(alias).unwrap().hash(),
            hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
        );
    }
    let new = c.active("b3-8").unwrap();
    assert_eq!(new.max_kill_score(0), 140);
    assert!(new.scoring_enabled());
}
#[test]
fn scoring_requires_new_pve_capability() {
    let p = scoring_package();
    p.edit("b3-8.json", |v| v["mechanism_version"] = json!(6));
    p.rejects("CONFIG_SCORING_CAPABILITY");
    let p = scoring_package();
    p.edit("b1.json", |v| v["mechanism_version"] = json!(7));
    p.rejects("CONFIG_SCORING_CAPABILITY");
}
#[test]
fn scoring_fields_are_strict_integer_bounds_and_zero_is_supported() {
    for value in [
        json!(-1),
        json!(1.5),
        json!(null),
        json!([]),
        json!("10"),
        json!(true),
        json!(1000001),
    ] {
        for base in [true, false] {
            let p = scoring_package();
            p.edit("b3-8.json", |v| {
                if base {
                    v["pve"]["kill_score"] = value.clone()
                } else {
                    v["pve"]["additional_templates"]["verification-phase-boss"]["kill_score"] =
                        value.clone()
                }
            });
            assert!(p.load().is_err(), "{value}, {base}");
        }
    }
    for n in [0, 1000000] {
        let p = scoring_package();
        p.edit("b3-8.json", |v| {
            v["cultivation_coefficient"] = json!(0);
            v["pve"]["kill_score"] = json!(n);
            v["pve"]["additional_templates"]["verification-phase-boss"]["kill_score"] = json!(n);
        });
        p.load().unwrap();
    }
}
#[test]
fn scoring_omission_inherits_root_while_noncredit_positive_override_is_rejected() {
    let p = scoring_package();
    p.edit("b3-8.json", |v| {
        v["kill_score"] = json!(7);
        v["pve"].as_object_mut().unwrap().remove("kill_score");
        v["pve"]["additional_templates"]["verification-score-elite"]
            .as_object_mut()
            .unwrap()
            .remove("kill_score");
    });
    assert_eq!(
        p.load().unwrap().active("b3-8").unwrap().max_kill_score(0),
        114
    );
    let p = scoring_package();
    p.edit("b3-8.json", |v| {
        v["pve"]["additional_templates"]["verification-phase-boss"]["kill_credit"] = json!(false)
    });
    p.rejects("CONFIG_KILL_SCORE_BOUNDS");
    p.edit("b3-8.json", |v| {
        v["pve"]["additional_templates"]["verification-phase-boss"]["kill_score"] = json!(0)
    });
    assert_eq!(
        p.load().unwrap().active("b3-8").unwrap().max_kill_score(0),
        40
    );
}
#[test]
fn scoring_growth_bound_includes_unspawned_boss_not_only_first_wave() {
    let p = scoring_package();
    p.edit("b3-8.json", |v| {
        v["cultivation_coefficient"] = json!(1000);
        v["pve"]["additional_templates"]["verification-phase-boss"]["kill_score"] = json!(1000000);
        v["death_multiplier"] = json!(5);
    });
    p.rejects("CONFIG_GROWTH_OVERFLOW");
}
#[test]
fn scoring_growth_bound_counts_every_possible_guardian_generation() {
    let p = scoring_package();
    p.edit("b3-8.json",|v|{let g=json!({"template_id":"verification-score-elite","chance":1,"min_count":1,"max_count":1,"spawn_offsets":[[64,0],[-64,0],[0,64],[0,-64]]});v["pve"]["additional_templates"]["verification-score-elite"]["drop_table"]=json!({"guaranteed":[],"random":[]});v["pve"]["additional_templates"]["verification-score-elite"]["drop_orb"]=json!(false);v["cultivation_coefficient"]=json!(1000);v["pve"]["additional_templates"]["verification-score-elite"]["kill_score"]=json!(1000000);v["pve"]["treasure_rules"]["template"]["respawn_delay_ms"]=json!(1000);v["pve"]["treasure_rules"]["template"]["guardian"]=g;});
    p.rejects("CONFIG_GROWTH_OVERFLOW");
}
fn area_package() -> Package {
    let p = scoring_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-9.json"),
        p.0.join("b3-9.json"),
    )
    .unwrap();
    let path = p.0.join("catalog.json");
    let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    v["active"]["b3-9"] = json!("b3-9.json");
    fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
    p
}
#[test]
fn area_loads_and_preserves_all_nine_prior_raw_versions() {
    let p = area_package();
    let c = p.load().unwrap();
    for alias in [
        "b1", "b3-1", "b3-2", "b3-3", "b3-4", "b3-5", "b3-6", "b3-7", "b3-8",
    ] {
        assert_eq!(
            c.active(alias).unwrap().hash(),
            hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
        );
    }
    let c = c.active("b3-9").unwrap();
    assert_eq!(c.max_kill_score(0), 140);
    assert_eq!(
        c.pve.as_ref().unwrap().additional_templates["verification-phase-boss"]
            .area
            .as_ref()
            .unwrap()
            .radius,
        96.0
    );
}
#[test]
fn area_capability_shape_unknown_mechanism_and_fields_are_strict() {
    let p = area_package();
    p.edit("b3-9.json", |v| v["mechanism_version"] = json!(7));
    p.rejects("CONFIG_AREA_CAPABILITY");
    for mode in 0..5 {
        let p = area_package();
        p.edit("b3-9.json", |v| {
            let a = &mut v["pve"]["additional_templates"]["verification-phase-boss"]["area"];
            match mode {
                0 => *a = json!([]),
                1 => a["mechanism"] = json!("fire_pool"),
                2 => a["unknown"] = json!(1),
                3 => {
                    a.as_object_mut().unwrap().remove("radius");
                }
                _ => a["radius"] = json!("96"),
            }
        });
        assert!(p.load().is_err(), "mode {mode}");
    }
}
#[test]
fn area_ids_names_lengths_and_existing_skill_conflicts_are_rejected() {
    for mode in 0..5 {
        let p = area_package();
        p.edit("b3-9.json", |v| {
            let t = &mut v["pve"]["additional_templates"]["verification-phase-boss"];
            let projectile = t["combat"]["projectile"]["id"].clone();
            let a = &mut t["area"];
            match mode {
                0 => a["id"] = json!(""),
                1 => a["name"] = json!("bad\nname"),
                2 => a["id"] = json!("x".repeat(65)),
                3 => a["name"] = json!("中".repeat(22)),
                _ => a["id"] = projectile,
            }
        });
        p.rejects("CONFIG_AREA_ID");
    }
}
#[test]
fn area_numeric_bounds_and_integer_fields_are_checked() {
    for (key, value) in [
        ("range", json!(0)),
        ("radius", json!(513)),
        ("windup_ms", json!(29)),
        ("windup_ms", json!(60001)),
        ("cooldown_ms", json!(0)),
        ("recovery_ms", json!(3600001)),
        ("cost", json!(1000001)),
        ("damage_bps", json!(100001)),
        ("cast_chance", json!(-0.1)),
        ("cast_chance", json!(1.1)),
        ("windup_ms", json!(1.5)),
        ("radius", json!(null)),
    ] {
        let p = area_package();
        p.edit("b3-9.json", |v| {
            v["pve"]["additional_templates"]["verification-phase-boss"]["area"][key] = value.clone()
        });
        assert!(p.load().is_err(), "{key}:{value}");
    }
}
#[test]
fn area_phase_condition_is_bounded_by_actual_template_phases() {
    for (phase, boss) in [(0, true), (3, true), (2, false)] {
        let p = area_package();
        p.edit("b3-9.json", |v| {
            let t = &mut v["pve"]["additional_templates"]["verification-phase-boss"];
            t["area"]["min_phase_index"] = json!(phase);
            if !boss {
                t.as_object_mut().unwrap().remove("boss");
            }
        });
        p.rejects("CONFIG_AREA_PHASE");
    }
}
#[test]
fn area_legal_boundary_parameters_and_generic_nonboss_template_load() {
    for high in [false, true] {
        let p = area_package();
        p.edit("b3-9.json", |v| {
            let t = &mut v["pve"]["additional_templates"]["verification-phase-boss"];
            t.as_object_mut().unwrap().remove("boss");
            let a = &mut t["area"];
            a["range"] = json!(if high { 10000.0 } else { 0.1 });
            a["radius"] = json!(if high { 512.0 } else { 0.1 });
            a["windup_ms"] = json!(if high { 60000 } else { 30 });
            a["cooldown_ms"] = json!(if high { 3600000 } else { 1 });
            a["recovery_ms"] = json!(if high { 3600000 } else { 0 });
            a["cost"] = json!(if high { 1000000 } else { 0 });
            a["damage_bps"] = json!(if high { 100000 } else { 0 });
            a["cast_chance"] = json!(if high { 1.0 } else { 0.0 });
        });
        p.load().unwrap();
    }
    let value = json!({"mechanism":"locked_circle","id":"x","name":"x","range":1,"radius":1,"windup_ms":30,"cooldown_ms":1,"recovery_ms":0,"cost":0,"damage_bps":0,"cast_chance":0,"min_phase_index":1,"bogus":0});
    assert!(serde_json::from_value::<super::super::AreaSkill>(value).is_err());
}

fn objective_package() -> Package {
    let p = area_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-10.json"),
        p.0.join("b3-10.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| {
        v["active"]["b3-10"] = json!("b3-10.json")
    });
    p
}
#[test]
fn objective_loads_and_preserves_ten_prior_raw_versions() {
    let p = objective_package();
    let c = p.load().unwrap();
    for alias in [
        "b1", "b3-1", "b3-2", "b3-3", "b3-4", "b3-5", "b3-6", "b3-7", "b3-8", "b3-9",
    ] {
        assert_eq!(
            c.active(alias).unwrap().hash(),
            hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
        );
    }
    let c = c.active("b3-10").unwrap();
    assert_eq!(
        c.pve
            .as_ref()
            .unwrap()
            .map_objective
            .as_ref()
            .unwrap()
            .cost
            .quantity,
        2
    );
    assert_eq!(c.max_kill_score(c.terminal_ms), 200);
}
#[test]
fn objective_rejects_old_capability_and_unknown_fields() {
    let p = objective_package();
    p.edit("b3-10.json", |v| v["mechanism_version"] = json!(8));
    p.rejects("CONFIG_OBJECTIVE_CAPABILITY");
    let p = objective_package();
    p.edit("b3-10.json", |v| {
        v["pve"]["map_objective"]["typo"] = json!(1)
    });
    p.rejects("CONFIG_SCHEMA");
}
#[test]
fn objective_dependency_references_and_cycles() {
    for bad in [
        json!(["missing"]),
        json!(["wetland"]),
        json!(["ruins", "ruins"]),
    ] {
        let p = objective_package();
        p.edit("b3-10.json", |v| {
            v["pve"]["encounters"][0]["requires"] = bad
        });
        p.rejects("CONFIG_OBJECTIVE_DEPENDENCY");
    }
    let p = objective_package();
    p.edit("b3-10.json", |v| {
        v["pve"]["encounters"][0]["requires"] = json!(["sanctum"])
    });
    p.rejects("CONFIG_OBJECTIVE_CYCLE");
}
#[test]
fn objective_forward_references_and_multiple_dependencies_load() {
    let p = objective_package();
    p.edit("b3-10.json", |v| {
        v["pve"]["encounters"][2]["requires"] = json!(["wetland", "ruins"]);
        v["pve"]["encounters"].as_array_mut().unwrap().reverse();
    });
    p.load().unwrap();
}
#[test]
fn objective_requires_dependency_closed_unique_target_set() {
    for ids in [
        json!([]),
        json!(["wetland", "wetland"]),
        json!(["missing"]),
        json!(["sanctum"]),
    ] {
        let p = objective_package();
        p.edit("b3-10.json", |v| {
            v["pve"]["map_objective"]["required_encounter_ids"] = ids
        });
        p.rejects("CONFIG_OBJECTIVE_REFERENCE_OR_BOUNDS");
    }
}
#[test]
fn objective_cost_time_id_and_chest_bounds() {
    for (key, value) in [
        ("id", json!("")),
        ("name", json!("\n")),
        ("reward_chest_id", json!("missing")),
        ("attune_duration_ms", json!(0)),
        ("attune_duration_ms", json!(60001)),
        ("cost", json!({"item_id":"missing","quantity":1})),
        ("cost", json!({"item_id":"huo-ling","quantity":1000001})),
    ] {
        let p = objective_package();
        p.edit("b3-10.json", |v| v["pve"]["map_objective"][key] = value);
        p.rejects("CONFIG_OBJECTIVE_REFERENCE_OR_BOUNDS");
    }
}
#[test]
fn objective_parameter_variants_and_one_shot_loot_bound() {
    let p = objective_package();
    p.edit("b3-10.json", |v| {
        v["pve"]["map_objective"]["attune_duration_ms"] = json!(300);
        v["pve"]["map_objective"]["cost"] = json!({"item_id":"xuan-tie","quantity":0});
        v["pve"]["treasure_rules"]["template"]["respawn_delay_ms"] = json!(1000);
    });
    let c = p.load().unwrap().active("b3-10").unwrap();
    assert_eq!(
        c.pve_chest_loot_bound("misty-spirit-wood", c.terminal_ms),
        8
    );
}
#[test]
fn objective_strict_payloads_and_missing_fields() {
    for action in [
        json!({"kind":"attune_objective","objective_id":"misty-sanctum"}),
        json!({"kind":"cancel_attunement","objective_id":"misty-sanctum"}),
        json!({"kind":"attune_objective","objective_id":"misty-sanctum","expected_revision":1,"set_completed":true}),
    ] {
        assert!(serde_json::from_value::<Action>(action).is_err());
    }
}
fn support_package() -> Package {
    let p = objective_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-11.json"),
        p.0.join("b3-11.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| {
        v["active"]["b3-11"] = json!("b3-11.json")
    });
    p
}
#[test]
fn support_loads_preserves_eleven_prior_hashes_and_source_skill() {
    let p = support_package();
    let c = p.load().unwrap();
    for alias in [
        "b1", "b3-1", "b3-2", "b3-3", "b3-4", "b3-5", "b3-6", "b3-7", "b3-8", "b3-9", "b3-10",
    ] {
        assert_eq!(
            c.active(alias).unwrap().hash(),
            hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
        );
    }
    let c = c.active("b3-11").unwrap();
    let s = c.pve.as_ref().unwrap().additional_templates["verification-forest-support"]
        .support
        .as_ref()
        .unwrap();
    assert_eq!((s.max_casts, s.heal_bps, s.windup_ms), (4, 600, 1800));
    assert_eq!(c.max_kill_score(c.terminal_ms), 200);
}
#[test]
fn support_old_capability_unknown_fields_and_wrong_types_rejected() {
    let p = support_package();
    p.edit("b3-11.json", |v| v["mechanism_version"] = json!(9));
    p.rejects("CONFIG_SUPPORT_CAPABILITY");
    for key in ["extra", "include_self", "ticks"] {
        let p = support_package();
        p.edit("b3-11.json", |v| {
            v["pve"]["additional_templates"]["verification-forest-support"]["support"][key] =
                json!("invalid")
        });
        p.rejects("CONFIG_SCHEMA");
    }
}
#[test]
fn support_ids_names_and_collision_rejected() {
    for (key, value) in [
        ("id", json!("")),
        ("name", json!("\n")),
        ("id", json!("yujian-shu")),
        ("id", json!("monster-regrowth")),
    ] {
        let p = support_package();
        p.edit("b3-11.json", |v| {
            v["pve"]["additional_templates"]["verification-forest-support"]["support"][key] = value
        });
        p.rejects("CONFIG_SUPPORT_ID");
    }
}
#[test]
fn support_numeric_bounds_and_lifecycle_limits() {
    for (key, value) in [
        ("range", json!(0)),
        ("range", json!(10001)),
        ("hp_threshold", json!(1.01)),
        ("cast_chance", json!(-0.1)),
        ("max_casts", json!(65)),
        ("ticks", json!(0)),
        ("ticks", json!(65)),
        ("windup_ms", json!(0)),
        ("tick_interval_ms", json!(0)),
        ("heal_bps", json!(100001)),
        ("cost", json!(1000001)),
        ("cooldown_ms", json!(0)),
        ("recovery_ms", json!(3600001)),
        ("interrupt_window_ms", json!(1801)),
        ("interrupt_cooldown_ms", json!(3600001)),
    ] {
        let p = support_package();
        p.edit("b3-11.json", |v| {
            v["pve"]["additional_templates"]["verification-forest-support"]["support"][key] = value
        });
        p.rejects("CONFIG_SUPPORT_BOUNDS");
    }
}
#[test]
fn support_missing_fields_and_unknown_mechanism_fail_loading() {
    for key in ["range", "cost", "include_self", "interrupt_window_ms"] {
        let p = support_package();
        p.edit("b3-11.json", |v| {
            v["pve"]["additional_templates"]["verification-forest-support"]["support"]
                .as_object_mut()
                .unwrap()
                .remove(key);
        });
        p.rejects("CONFIG_SCHEMA");
    }
    let p = support_package();
    p.edit("b3-11.json", |v| {
        v["pve"]["additional_templates"]["verification-forest-support"]["support"]["mechanism"] =
            json!("instant_win")
    });
    p.rejects("CONFIG_SCHEMA");
}
#[test]
fn support_zero_free_channel_parameters_are_configuration() {
    let p = support_package();
    p.edit("b3-11.json", |v| {
        let s = &mut v["pve"]["additional_templates"]["verification-forest-support"]["support"];
        s["id"] = json!("configured-support");
        s["name"] = json!("配置治疗");
        s["ticks"] = json!(4);
        s["tick_interval_ms"] = json!(300);
        s["interrupt_window_ms"] = json!(2700);
        s["heal_bps"] = json!(0);
        s["cost"] = json!(0);
        s["cast_chance"] = json!(0);
        s["max_casts"] = json!(0);
        s["interrupt_cooldown_ms"] = json!(0);
    });
    p.load().unwrap();
}
#[test]
fn support_total_duration_and_tick_integer_are_validated() {
    let p = support_package();
    p.edit("b3-11.json",|v|{v["pve"]["additional_templates"]["verification-forest-support"]["support"]["ticks"]=json!(64);v["pve"]["additional_templates"]["verification-forest-support"]["support"]["tick_interval_ms"]=json!(60000);});
    p.rejects("CONFIG_SUPPORT_BOUNDS");
    let p = support_package();
    p.edit("b3-11.json", |v| {
        v["pve"]["additional_templates"]["verification-forest-support"]["support"]["ticks"] =
            json!(1.5)
    });
    p.rejects("CONFIG_SCHEMA");
}
#[test]
fn support_cannot_add_direct_interrupt_or_mutable_world_inputs() {
    for value in [
        json!({"kind":"interrupt_support","monster_id":"support-a"}),
        json!({"kind":"set_support_budget","casts":99}),
        json!({"kind":"finish_heal","target_id":"boss","amount":99}),
    ] {
        assert!(serde_json::from_value::<Action>(value).is_err());
    }
}
fn heavy_package() -> Package {
    let p = support_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-12.json"),
        p.0.join("b3-12.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| {
        v["active"]["b3-12"] = json!("b3-12.json")
    });
    p
}
#[test]
fn heavy_preserves_twelve_hashes_and_source_numbers() {
    let p = heavy_package();
    let c = p.load().unwrap();
    for alias in [
        "b1", "b3-1", "b3-2", "b3-3", "b3-4", "b3-5", "b3-6", "b3-7", "b3-8", "b3-9", "b3-10",
        "b3-11",
    ] {
        assert_eq!(
            c.active(alias).unwrap().hash(),
            hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
        );
    }
    let c = c.active("b3-12").unwrap();
    let h = c.pve.as_ref().unwrap().additional_templates["verification-phase-boss"]
        .heavy
        .as_ref()
        .unwrap();
    assert_eq!(
        (
            h.range,
            h.half_width,
            h.windup_ms,
            h.travel_ms,
            h.counter_window_ms,
            h.counter_damage_bps
        ),
        (350.0, 34.0, 1600, 650, 3000, 12000)
    );
    assert_eq!(c.max_kill_score(c.terminal_ms), 200);
}
#[test]
fn heavy_old_capability_rejected_and_absence_allowed() {
    let p = heavy_package();
    p.edit("b3-12.json", |v| v["mechanism_version"] = json!(10));
    p.rejects("CONFIG_HEAVY_CAPABILITY");
    let p = heavy_package();
    p.edit("b3-12.json", |v| {
        v["pve"]["additional_templates"]["verification-phase-boss"]
            .as_object_mut()
            .unwrap()
            .remove("heavy");
    });
    assert!(p.load().is_ok());
}
#[test]
fn heavy_strict_types_fields_and_mechanism() {
    for (key, value) in [
        ("extra", json!(1)),
        ("mechanism", json!("instant")),
        ("range", json!("350")),
        ("travel_ms", json!(1.5)),
        ("cost", json!(-1)),
        ("cast_chance", json!(null)),
        ("speed", json!(560)),
    ] {
        let p = heavy_package();
        p.edit("b3-12.json", |v| {
            v["pve"]["additional_templates"]["verification-phase-boss"]["heavy"][key] = value
        });
        p.rejects("CONFIG_SCHEMA");
    }
    let value = serde_json::to_value(
        default_catalog()
            .active("b3-12")
            .unwrap()
            .pve
            .as_ref()
            .unwrap()
            .additional_templates["verification-phase-boss"]
            .heavy
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    for key in value.as_object().unwrap().keys() {
        let p = heavy_package();
        p.edit("b3-12.json", |v| {
            v["pve"]["additional_templates"]["verification-phase-boss"]["heavy"]
                .as_object_mut()
                .unwrap()
                .remove(key);
        });
        p.rejects("CONFIG_SCHEMA");
    }
}
#[test]
fn heavy_id_name_and_cross_ability_collisions() {
    for (key, value) in [
        ("id", json!("")),
        ("name", json!("\n")),
        ("id", json!("yujian-shu")),
        ("id", json!("monster-regrowth")),
    ] {
        let p = heavy_package();
        p.edit("b3-12.json", |v| {
            v["pve"]["additional_templates"]["verification-phase-boss"]["heavy"][key] = value
        });
        p.rejects("CONFIG_HEAVY_ID");
    }
    let p = heavy_package();
    p.edit("b3-12.json", |v| {
        v["pve"]["additional_templates"]["verification-phase-boss"]["heavy"]["id"] =
            v["pve"]["additional_templates"]["verification-phase-boss"]["area"]["id"].clone()
    });
    p.rejects("CONFIG_HEAVY_ID");
    let p = heavy_package();
    p.edit("b3-12.json", |v| {
        let s = v["pve"]["additional_templates"]["verification-forest-support"]["support"].clone();
        v["pve"]["additional_templates"]["verification-phase-boss"]["heavy"]["id"] =
            s["id"].clone();
        v["pve"]["additional_templates"]["verification-phase-boss"]["support"] = s;
    });
    p.rejects("CONFIG_HEAVY_ID");
}
#[test]
fn heavy_numeric_bounds_and_total_time() {
    for (key, value) in [
        ("range", json!(0)),
        ("range", json!(10001)),
        ("half_width", json!(0)),
        ("half_width", json!(513)),
        ("windup_ms", json!(0)),
        ("windup_ms", json!(60001)),
        ("travel_ms", json!(0)),
        ("travel_ms", json!(60001)),
        ("cost", json!(1000001)),
        ("damage_bps", json!(100001)),
        ("counter_damage_bps", json!(100001)),
        ("cooldown_ms", json!(0)),
        ("recovery_ms", json!(3600001)),
        ("cast_chance", json!(-0.1)),
        ("cast_chance", json!(1.01)),
        ("counter_window_ms", json!(0)),
        ("counter_window_ms", json!(60001)),
    ] {
        let p = heavy_package();
        p.edit("b3-12.json", |v| {
            v["pve"]["additional_templates"]["verification-phase-boss"]["heavy"][key] = value
        });
        p.rejects("CONFIG_HEAVY_BOUNDS");
    }
    let p = heavy_package();
    p.edit("b3-12.json", |v| v["terminal_ms"] = json!(2100));
    assert!(p.load().is_err());
}
#[test]
fn heavy_phase_and_finite_numbers_are_validated() {
    for phase in [0, 3] {
        let p = heavy_package();
        p.edit("b3-12.json",|v|v["pve"]["additional_templates"]["verification-phase-boss"]["heavy"]["min_phase_index"]=json!(phase));
        p.rejects("CONFIG_HEAVY_PHASE");
    }
    let mut c = default_catalog().active("b3-12").unwrap().as_ref().clone();
    let t = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    t.heavy.as_mut().unwrap().range = f64::INFINITY;
    let t = t.clone();
    assert_eq!(
        super::super::heavy::validate(&c, &t),
        Err("CONFIG_HEAVY_BOUNDS")
    );
}
#[test]
fn heavy_zero_free_and_multiple_parameter_variants() {
    let p = heavy_package();
    p.edit("b3-12.json", |v| {
        let h = &mut v["pve"]["additional_templates"]["verification-phase-boss"]["heavy"];
        for key in [
            "cost",
            "damage_bps",
            "counter_damage_bps",
            "cast_chance",
            "recovery_ms",
        ] {
            h[key] = json!(0);
        }
        h["id"] = json!("configured-heavy");
        h["name"] = json!("配置重击");
        h["half_width"] = json!(0.1);
        h["range"] = json!(10000);
        h["travel_ms"] = json!(60000);
        h["counter_window_ms"] = json!(30);
    });
    p.load().unwrap();
}
#[test]
fn heavy_no_client_grant_consume_or_damage_shortcuts() {
    for v in [
        json!({"kind":"grant_counter"}),
        json!({"kind":"counter","monster_id":"boss"}),
        json!({"kind":"set_heavy_hit","damage":99}),
        json!({"kind":"attack","counter_damage":99}),
    ] {
        assert!(serde_json::from_value::<Action>(v).is_err());
    }
}
fn reset_package() -> Package {
    let p = heavy_package();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-13.json"),
        p.0.join("b3-13.json"),
    )
    .unwrap();
    p.edit("catalog.json", |v| {
        v["active"]["b3-13"] = json!("b3-13.json")
    });
    p
}

fn support_variant_package() -> Package {
    let package = Package::new();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-14.json"),
        package.0.join("b3-14.json"),
    )
    .unwrap();
    package.edit("catalog.json", |v| {
        v["active"]["b3-14"] = json!("b3-14.json")
    });
    package
}

fn timed_position_package() -> Package {
    let package = Package::new();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-15.json"),
        package.0.join("b3-15.json"),
    )
    .unwrap();
    package.edit("catalog.json", |v| {
        v["active"]["b3-15"] = json!("b3-15.json")
    });
    package
}

#[test]
fn sword_trial_validates_version_side_references_and_combined_bounds() {
    let package = Package::new();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-19.json"),
        package.0.join("b3-19.json"),
    )
    .unwrap();
    package.edit("catalog.json", |v| {
        v["active"]["b3-19"] = json!("b3-19.json")
    });
    let catalog = package.load().unwrap();
    let config = catalog.active("b3-19").unwrap();
    let heavy = config.pve.as_ref().unwrap().additional_templates["sword-core"]
        .heavy
        .as_ref()
        .unwrap();
    assert_eq!(
        (heavy.counter_window_ms, heavy.counter_damage_bps),
        (2400, 20000)
    );
    assert_eq!(heavy.counter_window_bonus_ms.as_ref().unwrap().value, 1400);
    assert_eq!(
        heavy.counter_damage_scale_bps.as_ref().unwrap().value,
        13500
    );
    package.edit("b3-19.json", |v| v["mechanism_version"] = json!(17));
    package.rejects("CONFIG_COUNTER_SIDE_CAPABILITY");
    package.edit("b3-19.json", |v| v["mechanism_version"] = json!(18));
    let document: Value =
        serde_json::from_slice(&fs::read(package.0.join("b3-19.json")).unwrap()).unwrap();
    let original = &document["pve"]["additional_templates"]["sword-core"]["heavy"];
    for (field, value, error) in [
        (
            "counter_window_bonus_ms",
            json!({"encounter_id":"missing","value":1400}),
            "CONFIG_COUNTER_SIDE_REFERENCE",
        ),
        (
            "counter_damage_scale_bps",
            json!({"encounter_id":"missing","value":13500}),
            "CONFIG_COUNTER_SIDE_REFERENCE",
        ),
        (
            "counter_window_bonus_ms",
            json!({"encounter_id":"ruins","value":u64::MAX}),
            "CONFIG_COUNTER_SIDE_BOUNDS",
        ),
        (
            "counter_window_bonus_ms",
            json!({"encounter_id":"ruins","value":57601}),
            "CONFIG_COUNTER_SIDE_BOUNDS",
        ),
        (
            "counter_damage_scale_bps",
            json!({"encounter_id":"wetland","value":9999}),
            "CONFIG_COUNTER_SIDE_BOUNDS",
        ),
        (
            "counter_damage_scale_bps",
            json!({"encounter_id":"wetland","value":50001}),
            "CONFIG_COUNTER_SIDE_BOUNDS",
        ),
    ] {
        package.edit("b3-19.json", |v| {
            let mut heavy = original.clone();
            heavy[field] = value;
            v["pve"]["additional_templates"]["sword-core"]["heavy"] = heavy;
        });
        package.rejects(error);
    }
}

fn rotating_sites_package() -> Package {
    let package = Package::new();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-16.json"),
        package.0.join("b3-16.json"),
    )
    .unwrap();
    package.edit("catalog.json", |v| {
        v["active"]["b3-16"] = json!("b3-16.json")
    });
    package
}

#[test]
fn channel_guard_requires_supported_effect_and_valid_site_filters() {
    let package = Package::new();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-17.json"),
        package.0.join("b3-17.json"),
    )
    .unwrap();
    package.edit("catalog.json", |v| {
        v["active"]["b3-17"] = json!("b3-17.json")
    });
    let catalog = package.load().unwrap();
    let config = catalog.active("b3-17").unwrap();
    let check = config
        .pve
        .as_ref()
        .unwrap()
        .timed_position
        .as_ref()
        .unwrap();
    assert_eq!(check.success_spirit_restore_bps, Some(1200));
    assert_eq!(check.skip_sites_after["crystal"], "ruins");
    assert_eq!(check.skip_sites_after["river"], "wetland");
    assert_eq!((check.interval_ms, check.warning_ms), (6500, 3000));
    package.edit("b3-17.json", |v| v["mechanism_version"] = json!(15));
    package.rejects("CONFIG_POSITION_EFFECT_CAPABILITY");
    package.edit("b3-17.json", |v| v["mechanism_version"] = json!(16));

    let document: Value =
        serde_json::from_slice(&fs::read(package.0.join("b3-17.json")).unwrap()).unwrap();
    let original = &document["pve"]["timed_position"];
    for (field, value, error) in [
        (
            "success_damage_bps",
            json!(30000),
            "CONFIG_POSITION_SUCCESS_EFFECT",
        ),
        (
            "success_spirit_restore_bps",
            json!(10001),
            "CONFIG_POSITION_SUCCESS_EFFECT",
        ),
        (
            "success_spirit_restore_bps",
            Value::Null,
            "CONFIG_POSITION_SUCCESS_EFFECT",
        ),
        (
            "skip_sites_after",
            json!({"missing":"ruins"}),
            "CONFIG_POSITION_SITE_FILTER",
        ),
        (
            "skip_sites_after",
            json!({"crystal":"missing"}),
            "CONFIG_POSITION_SITE_FILTER",
        ),
        (
            "skip_sites_after",
            json!({"crystal":"sanctum"}),
            "CONFIG_POSITION_SITE_FILTER",
        ),
        (
            "skip_sites_after",
            json!({"furnace":"ruins","crystal":"ruins","river":"wetland"}),
            "CONFIG_POSITION_SITE_FILTER",
        ),
    ] {
        package.edit("b3-17.json", |v| {
            let mut check = original.clone();
            check[field] = value;
            v["pve"]["timed_position"] = check;
        });
        package.rejects(error);
    }
}

#[test]
fn thaw_window_validates_capability_condition_references_and_amounts() {
    let package = Package::new();
    fs::copy(
        Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/b3-18.json"),
        package.0.join("b3-18.json"),
    )
    .unwrap();
    package.edit("catalog.json", |v| {
        v["active"]["b3-18"] = json!("b3-18.json")
    });
    let catalog = package.load().unwrap();
    let config = catalog.active("b3-18").unwrap();
    let check = config
        .pve
        .as_ref()
        .unwrap()
        .timed_position
        .as_ref()
        .unwrap();
    assert_eq!((check.interval_ms, check.warning_ms), (6000, 2200));
    assert_eq!(check.direct_hit.as_ref().unwrap().miss_heal_bps, 800);
    assert_eq!(check.success_damage_bps, Some(16000));
    package.edit("b3-18.json", |v| v["mechanism_version"] = json!(16));
    package.rejects("CONFIG_TIMED_HIT_CAPABILITY");
    package.edit("b3-18.json", |v| v["mechanism_version"] = json!(17));
    let document: Value =
        serde_json::from_slice(&fs::read(package.0.join("b3-18.json")).unwrap()).unwrap();
    let original = &document["pve"]["timed_position"];
    for (field, value, error) in [
        (
            "source_id",
            json!("ruins-1"),
            "CONFIG_TIMED_POSITION_REFERENCE",
        ),
        (
            "failure_scale_bps",
            json!({"encounter_id":"sanctum","value":3500}),
            "CONFIG_TIMED_POSITION_REFERENCE",
        ),
        ("warning_ms", json!(4500), "CONFIG_TIMED_POSITION_BOUNDS"),
        (
            "success_damage_bps",
            json!(100001),
            "CONFIG_TIMED_POSITION_BOUNDS",
        ),
        (
            "success_damage_bps",
            Value::Null,
            "CONFIG_TIMED_HIT_CONDITION",
        ),
        (
            "direct_hit",
            json!({"miss_heal_bps":10001}),
            "CONFIG_TIMED_HIT_CONDITION",
        ),
        (
            "position",
            json!({"kind":"near_living_member","radius":80}),
            "CONFIG_TIMED_HIT_CONDITION",
        ),
        (
            "failure_damage_bps",
            json!(8000),
            "CONFIG_TIMED_HIT_CONDITION",
        ),
        (
            "direct_hit",
            json!({"miss_heal_bps":800,"unknown":true}),
            "CONFIG_SCHEMA",
        ),
    ] {
        package.edit("b3-18.json", |v| {
            let mut check = original.clone();
            check[field] = value;
            v["pve"]["timed_position"] = check;
        });
        package.rejects(error);
    }
}

#[test]
fn rotating_sites_validates_capability_identity_size_and_encounter_bounds() {
    let package = rotating_sites_package();
    let catalog = package.load().unwrap();
    let config = catalog.active("b3-16").unwrap();
    let check = config
        .pve
        .as_ref()
        .unwrap()
        .timed_position
        .as_ref()
        .unwrap();
    let ids: Vec<_> = check.sites().iter().map(|site| site.id.as_str()).collect();
    assert_eq!(ids, ["west-scar", "east-shard", "core-rim"]);
    assert_eq!(check.position.as_ref().unwrap().radius(), 82.0);
    assert_eq!(check.interval_ms, 6000);
    assert_eq!(check.warning_ms, 3200);

    package.edit("b3-16.json", |v| v["mechanism_version"] = json!(14));
    package.rejects("CONFIG_POSITION_SITES_CAPABILITY");
    package.edit("b3-16.json", |v| v["mechanism_version"] = json!(15));
    let document: Value =
        serde_json::from_slice(&fs::read(package.0.join("b3-16.json")).unwrap()).unwrap();
    let original = &document["pve"]["timed_position"]["position"];
    for (path, value, error) in [
        ("/sites", json!([]), "CONFIG_POSITION_SITES"),
        ("/sites/1/id", json!("west-scar"), "CONFIG_POSITION_SITES"),
        ("/sites/1/name", json!(""), "CONFIG_POSITION_SITES"),
        (
            "/sites/1/position",
            original["sites"][0]["position"].clone(),
            "CONFIG_POSITION_SITES",
        ),
        ("/sites/1/position", json!([0, 0]), "CONFIG_POSITION_SITES"),
        ("/radius", json!(0), "CONFIG_TIMED_POSITION_BOUNDS"),
    ] {
        package.edit("b3-16.json", |v| {
            let mut condition = original.clone();
            *condition.pointer_mut(path).unwrap() = value;
            v["pve"]["timed_position"]["position"] = condition;
        });
        package.rejects(error);
    }
    package.edit("b3-16.json", |v| {
        v["pve"]["timed_position"]["position"] = original.clone();
        v["pve"]["timed_position"]["position"]["sites"][0]["unknown"] = json!(true);
    });
    package.rejects("CONFIG_SCHEMA");
}

#[test]
fn rotating_sites_rejects_blocked_site_center() {
    let package = rotating_sites_package();
    package.edit("b3-16.json", |v| {
        let p = &mut v["pve"];
        let site = &p["timed_position"]["position"]["sites"][0]["position"];
        let x = (site[0].as_f64().unwrap() / 32.0) as usize;
        let y = (site[1].as_f64().unwrap() / 32.0) as usize;
        let width = p["width"].as_u64().unwrap() as usize;
        let runs: Vec<(usize, usize)> = serde_json::from_value(p["cells_rle"].clone()).unwrap();
        let mut tiles: Vec<_> = runs
            .into_iter()
            .flat_map(|(tile, count)| std::iter::repeat_n((tile, 1), count))
            .collect();
        tiles[y * width + x] = (0, 1);
        p["cells_rle"] = json!(tiles);
    });
    package.rejects("CONFIG_MAP_POINT_OR_PATH");
}

#[test]
fn timed_position_loads_source_values_and_allows_data_only_remapping() {
    let package = timed_position_package();
    let catalog = package.load().unwrap();
    let config = catalog.active("b3-15").unwrap();
    let pve = config.pve.as_ref().unwrap();
    assert_eq!(pve.map_id, "maps/thunder-valley-isometric.json");
    let check = pve.timed_position.as_ref().unwrap();
    assert_eq!(check.interval_ms, 5500);
    assert_eq!(check.warning_ms, 3000);
    assert_eq!(check.success_damage_bps, Some(30000));
    assert_eq!(check.failure_damage_bps, Some(14000));
    assert_eq!(check.warning_bonus_ms.as_ref().unwrap().value, 1500);
    assert_eq!(check.failure_scale_bps.as_ref().unwrap().value, 5500);
    package.edit("b3-15.json", |v| {
        let check = &mut v["pve"]["timed_position"];
        check["name"] = json!("Station");
        check["warning_bonus_ms"] = json!({"encounter_id":"wetland","value":700});
        check["failure_scale_bps"] = json!({"encounter_id":"ruins","value":8000});
        check["position"]["radius"] = json!(60);
    });
    assert!(package.load().is_ok());
}

#[test]
fn timed_position_rejects_invalid_capabilities_references_and_bounds() {
    let package = timed_position_package();
    package.edit("b3-15.json", |v| v["mechanism_version"] = json!(13));
    package.rejects("CONFIG_TIMED_POSITION_CAPABILITY");
    package.edit("b3-15.json", |v| v["mechanism_version"] = json!(14));
    let path = package.0.join("b3-15.json");
    let document: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let original = &document["pve"]["timed_position"];
    for (field, value, error) in [
        (
            "encounter_id",
            json!("missing"),
            "CONFIG_TIMED_POSITION_REFERENCE",
        ),
        (
            "source_id",
            json!("ruins-1"),
            "CONFIG_TIMED_POSITION_REFERENCE",
        ),
        ("warning_ms", json!(5500), "CONFIG_TIMED_POSITION_BOUNDS"),
        (
            "warning_bonus_ms",
            json!({"encounter_id":"ruins","value":2500}),
            "CONFIG_TIMED_POSITION_BOUNDS",
        ),
        (
            "failure_scale_bps",
            json!({"encounter_id":"sanctum","value":5500}),
            "CONFIG_TIMED_POSITION_REFERENCE",
        ),
        (
            "failure_scale_bps",
            json!({"encounter_id":"wetland","value":10001}),
            "CONFIG_TIMED_POSITION_BOUNDS",
        ),
        (
            "position",
            json!({"kind":"near_living_member","radius":0}),
            "CONFIG_TIMED_POSITION_BOUNDS",
        ),
        (
            "position",
            json!({"kind":"invalid","radius":90}),
            "CONFIG_SCHEMA",
        ),
        ("unknown", json!(true), "CONFIG_SCHEMA"),
    ] {
        package.edit("b3-15.json", |v| {
            let mut check = original.clone();
            check[field] = value;
            v["pve"]["timed_position"] = check;
        });
        package.rejects(error);
    }
    package.edit("b3-15.json", |v| v["pve"]["timed_position"] = Value::Null);
    package.rejects("CONFIG_SCHEMA");
}

const SUPPORT_VARIANT: &str =
    "/pve/additional_templates/verification-forest-support/support/on_encounter_completed";

#[test]
fn support_variant_loads_source_values_and_accepts_configured_references() {
    let package = support_variant_package();
    let catalog = package.load().unwrap();
    let c = catalog.active("b3-14").unwrap();
    let pve = c.pve.as_ref().unwrap();
    let skill = pve.additional_templates["verification-forest-support"]
        .support
        .as_ref()
        .unwrap();
    let variant = skill.on_encounter_completed.as_ref().unwrap();
    assert_eq!((skill.id.as_str(), skill.heal_bps), ("map-pve-mend", 1200));
    assert_eq!(
        (variant.id.as_str(), variant.heal_bps),
        ("map-pve-mend-weakened", 600)
    );
    assert_eq!(variant.encounter_id, "wetland");
    assert_eq!(
        pve.map_objective.as_ref().unwrap().required_encounter_ids,
        ["ruins", "sanctum"]
    );
    package.edit("b3-14.json", |v| {
        *v.pointer_mut(SUPPORT_VARIANT).unwrap() = json!({
            "encounter_id": "ruins", "id": "new-heal", "name": "New heal", "heal_bps": 0
        });
    });
    assert!(package.load().is_ok());
}

#[test]
fn support_variant_rejects_invalid_capability_references_and_shapes() {
    let package = support_variant_package();
    package.edit("b3-14.json", |v| v["mechanism_version"] = json!(12));
    package.rejects("CONFIG_SUPPORT_VARIANT_CAPABILITY");
    package.edit("b3-14.json", |v| v["mechanism_version"] = json!(13));
    let original = json!({
        "encounter_id": "wetland", "id": "map-pve-mend-weakened",
        "name": "Weakened heal", "heal_bps": 600
    });
    for (field, value, error) in [
        (
            "encounter_id",
            json!("missing"),
            "CONFIG_SUPPORT_VARIANT_ENCOUNTER",
        ),
        ("id", json!("map-pve-mend"), "CONFIG_SUPPORT_VARIANT_ID"),
        ("name", json!(""), "CONFIG_SUPPORT_VARIANT_ID"),
        ("heal_bps", json!(100001), "CONFIG_SUPPORT_VARIANT_BOUNDS"),
        ("heal_bps", json!(-1), "CONFIG_SCHEMA"),
        ("heal_bps", json!(2.5), "CONFIG_SCHEMA"),
        ("unknown", json!(true), "CONFIG_SCHEMA"),
    ] {
        package.edit("b3-14.json", |v| {
            let mut variant = original.clone();
            variant[field] = value;
            *v.pointer_mut(SUPPORT_VARIANT).unwrap() = variant;
        });
        package.rejects(error);
    }
    for field in ["encounter_id", "id", "name", "heal_bps"] {
        package.edit("b3-14.json", |v| {
            let mut variant = original.clone();
            variant.as_object_mut().unwrap().remove(field);
            *v.pointer_mut(SUPPORT_VARIANT).unwrap() = variant;
        });
        package.rejects("CONFIG_SCHEMA");
    }
    package.edit("b3-14.json", |v| {
        *v.pointer_mut(SUPPORT_VARIANT).unwrap() = Value::Null
    });
    package.rejects("CONFIG_SCHEMA");
}

#[test]
fn reset_preserves_thirteen_raw_hashes_and_source_delay() {
    let p = reset_package();
    let c = p.load().unwrap();
    for alias in [
        "b1", "b3-1", "b3-2", "b3-3", "b3-4", "b3-5", "b3-6", "b3-7", "b3-8", "b3-9", "b3-10",
        "b3-11", "b3-12",
    ] {
        assert_eq!(
            c.active(alias).unwrap().hash(),
            hash(&fs::read(p.0.join(format!("{alias}.json"))).unwrap())
        );
    }
    let c = c.active("b3-13").unwrap();
    let p = c.pve.as_ref().unwrap();
    for e in &p.encounters {
        let r = e.reset.as_ref().unwrap();
        assert_eq!(r.delay_ms, 5000);
        assert_eq!(r.pursuit_radius, e.trigger.radius + 180.0);
    }
    assert_eq!(c.params.resource_ttl_ms, 60000);
    assert_eq!(c.max_kill_score(c.terminal_ms), 200);
}
#[test]
fn reset_old_capability_rejected_and_absence_allowed() {
    let p = reset_package();
    p.edit("b3-13.json", |v| v["mechanism_version"] = json!(11));
    p.rejects("CONFIG_RESET_CAPABILITY");
    let p = reset_package();
    p.edit("b3-13.json", |v| {
        for e in v["pve"]["encounters"].as_array_mut().unwrap() {
            e.as_object_mut().unwrap().remove("reset");
        }
    });
    assert!(p.load().is_ok());
}
#[test]
fn reset_fields_types_and_policy_are_strict() {
    for (key, value) in [
        ("extra", json!(1)),
        ("mechanism", json!("reset_world")),
        ("reward_policy", json!("repeat_rewards")),
        ("pursuit_radius", json!("360")),
        ("delay_ms", json!(1.5)),
        ("delay_ms", json!(-1)),
    ] {
        let p = reset_package();
        p.edit("b3-13.json", |v| {
            v["pve"]["encounters"][0]["reset"][key] = value
        });
        p.rejects("CONFIG_SCHEMA");
    }
    for key in ["mechanism", "reward_policy", "pursuit_radius", "delay_ms"] {
        let p = reset_package();
        p.edit("b3-13.json", |v| {
            v["pve"]["encounters"][0]["reset"]
                .as_object_mut()
                .unwrap()
                .remove(key);
        });
        p.rejects("CONFIG_SCHEMA");
    }
}
#[test]
fn reset_radius_delay_and_finite_bounds() {
    for (key, value) in [
        ("pursuit_radius", json!(1)),
        ("pursuit_radius", json!(10001)),
        ("delay_ms", json!(0)),
        ("delay_ms", json!(60001)),
    ] {
        let p = reset_package();
        p.edit("b3-13.json", |v| {
            v["pve"]["encounters"][0]["reset"][key] = value
        });
        p.rejects("CONFIG_RESET_BOUNDS");
    }
    let mut c = default_catalog().active("b3-13").unwrap().as_ref().clone();
    c.pve_mut().encounters[0]
        .reset
        .as_mut()
        .unwrap()
        .pursuit_radius = f64::INFINITY;
    assert_eq!(
        super::super::reset::validate(&c, c.pve.as_ref().unwrap()),
        Err("CONFIG_RESET_BOUNDS")
    );
}
#[test]
fn reset_layout_must_fit_pursuit_range() {
    let p = reset_package();
    p.edit("b3-13.json", |v| {
        v["pve"]["encounters"][0]["trigger"]["radius"] = json!(1);
        v["pve"]["encounters"][0]["reset"]["pursuit_radius"] = json!(1);
    });
    p.rejects("CONFIG_RESET_PLACEMENT");
}
#[test]
fn reset_real_death_and_repeat_orb_budgets_are_bounded() {
    let p = reset_package();
    p.edit("b3-13.json", |v| {
        v["pve"]["encounters"][0]["reset"]["delay_ms"] = json!(30)
    });
    p.rejects("CONFIG_RESET_DEATH_BUDGET");
    let p = reset_package();
    p.edit("b3-13.json", |v| {
        v["params"]["resource_ttl_ms"] = json!(300000)
    });
    p.rejects("CONFIG_RESET_RESOURCE_BUDGET");
}
#[test]
fn reset_parameter_variants_and_nonreset_groups_load() {
    let p = reset_package();
    p.edit("b3-13.json", |v| {
        v["pve"]["encounters"][0]
            .as_object_mut()
            .unwrap()
            .remove("reset");
        v["pve"]["encounters"][1]["reset"]["pursuit_radius"] = json!(1000);
        v["pve"]["encounters"][1]["reset"]["delay_ms"] = json!(60000);
    });
    p.load().unwrap();
}
#[test]
fn reset_no_client_round_state_or_direct_reset_shortcuts() {
    for v in [
        json!({"kind":"reset_encounter","encounter_id":"team"}),
        json!({"kind":"set_round","round":99}),
        json!({"kind":"attack","rounds":[99]}),
        json!({"kind":"navigate","destination":[1,1],"grant_rewards":true}),
    ] {
        assert!(serde_json::from_value::<Action>(v).is_err());
    }
    assert!(
        serde_json::from_value::<crate::protocol::ResetProgressSummary>(
            json!({"actual_kills":0,"encounters":[],"set_world":true})
        )
        .is_err()
    );
}

fn fixture_profile_matches(alias: &str, config: &Config) -> bool {
    match (alias, &config.pve) {
        ("b1", None) => true,
        ("b3-1", Some(pve)) => pve.treasure_rules.is_none(),
        ("b3-2", Some(pve)) => pve.treasure_rules.as_ref().is_some_and(|r| {
            matches!(
                r.template.mechanism,
                crate::config::ContainerMechanism::ImmediateGroundLoot
            )
        }),
        ("b3-3", Some(pve)) => pve.treasure_rules.as_ref().is_some_and(|r| {
            matches!(
                r.template.mechanism,
                crate::config::ContainerMechanism::LayeredSealGroundLoot
            )
        }),
        ("b3-4", Some(pve)) => pve.guardian().is_some(),
        ("b3-5", Some(pve)) => !pve.encounters.is_empty(),
        ("b3-6", Some(pve)) => pve.encounters.iter().any(|e| !e.waves.is_empty()),
        ("b3-19", Some(pve)) => pve.additional_templates.values().any(|template| {
            template.heavy.as_ref().is_some_and(|heavy| {
                heavy.counter_window_bonus_ms.is_some() && heavy.counter_damage_scale_bps.is_some()
            })
        }),
        ("b3-18", Some(pve)) => pve
            .timed_position
            .as_ref()
            .is_some_and(|check| check.direct_hit.is_some()),
        ("b3-17", Some(pve)) => pve.timed_position.as_ref().is_some_and(|check| {
            check.success_spirit_restore_bps.is_some() && !check.skip_sites_after.is_empty()
        }),
        ("b3-16", Some(pve)) => pve.timed_position.as_ref().is_some_and(|check| {
            matches!(
                check.position,
                Some(crate::config::PositionCondition::RotatingSites { .. })
            )
        }),
        ("b3-15", Some(pve)) => pve.timed_position.is_some(),
        ("b3-14", Some(pve)) => pve.additional_templates.values().any(|t| {
            t.support
                .as_ref()
                .is_some_and(|s| s.on_encounter_completed.is_some())
        }),
        ("b3-13", Some(pve)) => pve.encounters.iter().any(|e| e.reset.is_some()),
        ("b3-12", Some(pve)) => pve.additional_templates.values().any(|t| t.heavy.is_some()),
        ("b3-11", Some(pve)) => pve
            .additional_templates
            .values()
            .any(|t| t.support.is_some()),
        ("b3-10", Some(pve)) => pve.map_objective.is_some(),
        ("b3-9", Some(pve)) => pve.additional_templates.values().any(|t| t.area.is_some()),
        ("b3-8", Some(_)) => config.scoring_enabled(),
        ("b3-7", Some(pve)) => pve.additional_templates.values().any(|t| t.boss.is_some()),
        (
            "b1" | "b3-1" | "b3-2" | "b3-3" | "b3-4" | "b3-5" | "b3-6" | "b3-7" | "b3-8" | "b3-9"
            | "b3-10" | "b3-11" | "b3-12" | "b3-13" | "b3-14" | "b3-15" | "b3-16" | "b3-17"
            | "b3-18" | "b3-19",
            _,
        ) => false,
        _ => true,
    }
}
#[test]
fn bundled_fixtures_have_their_declared_regression_capabilities() {
    let path = Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/catalog.json");
    let catalog = ConfigCatalog::load(path).unwrap();
    for alias in catalog.active.keys() {
        assert!(
            fixture_profile_matches(alias, &catalog.active(alias).unwrap()),
            "{alias}"
        );
    }
}
