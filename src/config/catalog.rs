use super::Config;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Instant,
};

#[cfg(all(test, match_sdk_test_data))]
#[path = "catalog_tests.rs"]
mod tests;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    default_scenario: Option<String>,
    active: BTreeMap<String, String>,
    archives: Vec<String>,
}
struct Version {
    config: Arc<Config>,
    raw: Arc<str>,
}
/// Validated startup data. Sharing this Arc shares the parsed configurations and terrain cells.
pub struct ConfigCatalog {
    default_scenario: String,
    active: BTreeMap<String, (String, String)>,
    versions: BTreeMap<(String, String), Version>,
    load_us: u128,
    prepared_maps: usize,
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}
#[cfg(all(test, match_sdk_test_data))]
pub(super) fn rules_hash(mut value: Value) -> Option<String> {
    let object = value.as_object_mut()?;
    for key in ["id", "purpose", "geometry", "spawn", "extraction_point"] {
        object.remove(key);
    }
    object
        .get_mut("target")?
        .as_object_mut()?
        .remove("position");
    if object.get("pve").is_some_and(Value::is_null) {
        object.remove("pve");
    }
    if let Some(pve) = object.get_mut("pve").and_then(Value::as_object_mut) {
        for key in [
            "map_id",
            "width",
            "height",
            "cell_size",
            "isometric_grid",
            "palette",
            "cells_rle",
            "monster_positions",
            "chest_spawns",
        ] {
            pve.remove(key);
        }
    }
    Some(hash(&serde_json::to_vec(&value).unwrap()))
}
impl ConfigCatalog {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let started = Instant::now();
        let path = path.as_ref().canonicalize()?;
        let parent = path
            .parent()
            .ok_or_else(|| invalid("CONFIG_MANIFEST_PATH"))?;
        if fs::metadata(&path)?.len() > 65536 {
            return Err(invalid("CONFIG_MANIFEST_TOO_LARGE"));
        }
        let bytes = fs::read(&path)?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|e| invalid(format!("CONFIG_MANIFEST: {e}")))?;
        if manifest.schema_version != 1
            || manifest.active.is_empty()
            || manifest.active.keys().any(|alias| !valid_id(alias))
            || manifest.archives.len() > 30
            || manifest.active.len() + manifest.archives.len() > 32
        {
            return Err(invalid("CONFIG_MANIFEST_SCHEMA"));
        }
        let default_scenario = manifest
            .default_scenario
            .or_else(|| {
                (manifest.active.len() == 1)
                    .then(|| manifest.active.first_key_value().unwrap().0.clone())
            })
            .ok_or_else(|| invalid("CONFIG_MANIFEST_DEFAULT"))?;
        if !manifest.active.contains_key(&default_scenario) {
            return Err(invalid("CONFIG_MANIFEST_DEFAULT"));
        }
        let mut result = Self {
            default_scenario,
            active: BTreeMap::new(),
            versions: BTreeMap::new(),
            load_us: 0,
            prepared_maps: 0,
        };
        for (alias, file) in manifest
            .active
            .iter()
            .map(|(a, f)| (Some(a), f))
            .chain(manifest.archives.iter().map(|f| (None, f)))
        {
            let relative = Path::new(file);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)))
                || relative.extension().is_none_or(|e| e != "json")
            {
                return Err(invalid("CONFIG_PATH_OUTSIDE_PACKAGE"));
            }
            let file = parent.join(relative).canonicalize()?;
            if !file.starts_with(parent) || fs::metadata(&file)?.len() > 1048576 {
                return Err(invalid("CONFIG_FILE_BOUNDS"));
            }
            let bytes = fs::read(file)?;
            let digest = hash(&bytes);
            if let Some(key) = result.versions.keys().find(|key| key.1 == digest) {
                if let Some(alias) = alias {
                    result.active.insert(alias.clone(), key.clone());
                }
                continue;
            }
            let raw = std::str::from_utf8(&bytes).map_err(|_| invalid("CONFIG_UTF8"))?;
            let document: Value =
                serde_json::from_slice(&bytes).map_err(|e| invalid(format!("CONFIG_JSON: {e}")))?;
            let id = document
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| valid_id(id))
                .ok_or_else(|| invalid("CONFIG_ID"))?
                .to_owned();
            let key = (id, digest.clone());
            if !result.versions.contains_key(&key) {
                let normalized = if document
                    .get("schema_version")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    == 1
                {
                    #[cfg(all(test, match_sdk_test_data))]
                    {
                        super::legacy::upgrade(document.clone()).map_err(invalid)?
                    }
                    #[cfg(not(all(test, match_sdk_test_data)))]
                    {
                        return Err(invalid(
                            "LEGACY_CONFIG_REQUIRES_EXPLICIT_PARAMETERS_USE_SCHEMA_V2",
                        ));
                    }
                } else {
                    document.clone()
                };
                super::resource_steal::fields(&normalized).map_err(invalid)?;
                super::resource_commit::fields(&normalized).map_err(invalid)?;
                super::skill_loadouts::fields(&normalized).map_err(invalid)?;
                super::hp_cost::fields(&normalized).map_err(invalid)?;
                super::cast_movement::fields(&normalized).map_err(invalid)?;
                super::charge::fields(&normalized).map_err(invalid)?;
                super::channel::fields(&normalized).map_err(invalid)?;
                super::recast::fields(&normalized).map_err(invalid)?;
                super::followup::fields(&normalized).map_err(invalid)?;
                super::recovery_cancel::fields(&normalized).map_err(invalid)?;
                super::basic_followup::fields(&normalized).map_err(invalid)?;
                super::basic_pursuit::fields(&normalized).map_err(invalid)?;
                super::member_loot_drop::fields(&normalized).map_err(invalid)?;
                super::member_item_use::fields(&normalized).map_err(invalid)?;
                let validation =
                    super::two_members::validation_document(&normalized).map_err(invalid)?;
                super::validate::shapes(&validation).map_err(invalid)?;
                let skill_limit = super::player_skills::field_limit(&normalized);
                super::player_skills::fields(&validation, skill_limit).map_err(invalid)?;
                super::status::validate_fields(&validation).map_err(invalid)?;
                super::combat::validate_fields(&normalized).map_err(invalid)?;
                let mut config: Config = serde_json::from_value(normalized)
                    .map_err(|e| invalid(format!("CONFIG_SCHEMA: {e}")))?;
                super::resource_limits::validate(&config).map_err(invalid)?;
                super::combat::validate(&config).map_err(invalid)?;
                super::resource_steal::validate(&config).map_err(invalid)?;
                super::triggers::validate(&config).map_err(invalid)?;
                super::shield_records::validate(&config).map_err(invalid)?;
                super::status_windows::validate(&config).map_err(invalid)?;
                super::status_cleanses::validate(&config).map_err(invalid)?;
                super::periodic_recovery::validate(&config).map_err(invalid)?;
                super::status_operations::validate(&config).map_err(invalid)?;
                super::control_policies::validate(&config).map_err(invalid)?;
                super::contact_status::validate(&config).map_err(invalid)?;
                super::contact_reactions::validate(&config).map_err(invalid)?;
                super::post_consumption::validate(&config).map_err(invalid)?;
                super::finishers::validate(&config).map_err(invalid)?;
                super::finisher_followups::validate(&config).map_err(invalid)?;
                super::chain_damage::validate(&config).map_err(invalid)?;
                super::status::validate_member_status_capabilities(&config).map_err(invalid)?;
                super::population::validate_recovery_orbs(&config).map_err(invalid)?;
                super::population::validate_npc_hit(&config).map_err(invalid)?;
                super::population::validate_npc_unavoidable(&config).map_err(invalid)?;
                super::population::validate_npc_ignore_armor(&config).map_err(invalid)?;
                super::player_skills::validate_skill_effects(&config).map_err(invalid)?;
                super::hp_cost::validate(&config).map_err(invalid)?;
                super::charge::validate(&config).map_err(invalid)?;
                super::channel::validate(&config).map_err(invalid)?;
                super::recast::validate(&config).map_err(invalid)?;
                super::followup::validate(&config).map_err(invalid)?;
                super::recovery_cancel::validate(&config).map_err(invalid)?;
                super::basic_followup::validate(&config).map_err(invalid)?;
                super::basic_pursuit::validate(&config).map_err(invalid)?;
                super::member_loot_drop::validate(&config).map_err(invalid)?;
                super::member_item_use::validate(&config).map_err(invalid)?;
                super::cast_movement::validate(&config).map_err(invalid)?;
                super::world_loot::validate(&config).map_err(invalid)?;
                super::world_boons::validate(&config).map_err(invalid)?;
                super::validate::validate(&config.single_member_validation(), skill_limit)
                    .map_err(invalid)?;
                super::two_members::validate(&config).map_err(invalid)?;
                super::match_mode::validate(&config).map_err(invalid)?;
                super::encounter_progress::validate(&config).map_err(invalid)?;
                super::extraction_lifecycle::validate(&config).map_err(invalid)?;
                super::contested_equipment::validate(&config).map_err(invalid)?;
                super::zone_exposure::validate(&config).map_err(invalid)?;
                super::training_targets::validate(&config).map_err(invalid)?;
                super::settlement_rewards::validate(&config).map_err(invalid)?;
                super::settlement_progression::validate(&config).map_err(invalid)?;
                super::settlement_facts::validate(&config).map_err(invalid)?;
                validate_geometry(&config)?;
                super::world_geometry::validate(&config).map_err(invalid)?;
                super::npc_ai::validate(&config).map_err(invalid)?;
                super::participant_ai::validate(&config).map_err(invalid)?;
                let prepared = Arc::new(config.params.clone());
                if let Some(pve) = &mut config.pve {
                    Arc::make_mut(pve).prepared = Some(prepared);
                    let pve = Arc::make_mut(pve);
                    pve.profiles = pve.build_profiles();
                    if !(4..=512).contains(&pve.width)
                        || !(4..=512).contains(&pve.height)
                        || !(4.0..=128.0).contains(&pve.cell_size)
                        || pve.palette.is_empty()
                        || pve.palette.len() > 64
                        || pve.monster_positions.len() > 32
                        || !valid_id(&pve.map_id)
                    {
                        return Err(invalid("CONFIG_MAP_BOUNDS"));
                    }
                    if pve.isometric_grid.is_some_and(|iso| {
                        config.mechanism_version != 105
                            || config.replication.is_none()
                            || !(4.0..=128.0).contains(&iso.tile_height)
                            || !iso.origin.iter().all(|v| v.is_finite())
                    }) {
                        return Err(invalid("CONFIG_ISOMETRIC_GRID"));
                    }
                    if (config.geometry.min, config.geometry.max) != pve.world_bounds()
                        || !config.geometry.walls.is_empty()
                    {
                        return Err(invalid("CONFIG_MAP_GEOMETRY"));
                    }
                    let flat_elevation = pve.palette[0].elevation;
                    let advanced_terrain =
                        pve.world_geometry.is_some() || pve.isometric_grid.is_some();
                    let bounded = |value: f64| (0.0..=10000.0).contains(&value);
                    let tiles = document["pve"]["palette"].as_array().unwrap();
                    for (raw_tile, tile) in tiles.iter().zip(&pve.palette) {
                        let fields = raw_tile
                            .as_object()
                            .ok_or_else(|| invalid("CONFIG_UNSUPPORTED_TERRAIN"))?;
                        if fields.keys().any(|field| {
                            !matches!(
                                field.as_str(),
                                "terrain"
                                    | "walkable"
                                    | "speedMultiplier"
                                    | "dotDamagePerSecond"
                                    | "blocksProjectiles"
                                    | "blocksVision"
                                    | "elevation"
                                    | "maxClimbHeight"
                                    | "maxDropHeight"
                                    | "slope"
                            )
                        }) || !valid_id(&tile.terrain)
                            || !bounded(tile.elevation)
                            || if advanced_terrain {
                                !bounded(tile.max_climb_height)
                                    || !bounded(tile.max_drop_height)
                                    || !bounded(tile.dot_damage_per_second)
                                    || (tile.dot_damage_per_second * 1000.0
                                        - tile.damage_milli_per_second() as f64)
                                        .abs()
                                        > 1e-8
                                    || tile
                                        .slope
                                        .is_some_and(|slope| !bounded(slope.end_elevation))
                            } else {
                                tile.elevation != flat_elevation
                                    || tile.max_climb_height != 0.0
                                    || tile.max_drop_height != 0.0
                                    || tile.dot_damage_per_second != 0.0
                                    || tile.slope.is_some()
                            }
                        {
                            return Err(invalid("CONFIG_UNSUPPORTED_TERRAIN"));
                        }
                    }
                    if pve.palette.iter().any(|tile| {
                        !tile.speed_multiplier.is_finite()
                            || !(0.0..=10.0).contains(&tile.speed_multiplier)
                            || (tile.walkable && tile.speed_multiplier == 0.0)
                    }) {
                        return Err(invalid("CONFIG_TERRAIN_SPEED"));
                    }
                    let mut total = 0usize;
                    for &(index, count) in &pve.cells_rle {
                        if index >= pve.palette.len() || count == 0 {
                            return Err(invalid("CONFIG_RLE_INDEX"));
                        }
                        total = total
                            .checked_add(count)
                            .filter(|n| *n <= pve.width * pve.height)
                            .ok_or_else(|| invalid("CONFIG_RLE_COUNT"))?;
                    }
                    if total != pve.width * pve.height {
                        return Err(invalid("CONFIG_RLE_COUNT"));
                    }
                    pve.cells = pve
                        .cells_rle
                        .iter()
                        .flat_map(|(index, count)| std::iter::repeat_n(*index, *count))
                        .collect::<Vec<_>>()
                        .into();
                    let pve = config.pve.as_ref().unwrap();
                    if !crate::world::pve::validate_map(pve, config.spawn, config.extraction_point)
                    {
                        return Err(invalid("CONFIG_MAP_POINT_OR_PATH"));
                    }
                    result.prepared_maps += 1;
                } else if !config
                    .geometry
                    .walkable(config.spawn, config.params.player_radius)
                    || !config
                        .geometry
                        .walkable(config.extraction_point, config.params.player_radius)
                    || !config
                        .geometry
                        .walkable(config.target.position, config.target.radius)
                {
                    return Err(invalid("CONFIG_MAP_POINT"));
                }
                super::two_members::validate_prepared(&config).map_err(invalid)?;
                super::training_targets::validate_prepared(&config).map_err(invalid)?;
                super::contested_equipment::validate_prepared(&config).map_err(invalid)?;
                super::zone_exposure::validate_prepared(&config).map_err(invalid)?;
                config.loaded_hash = digest;
                result.versions.insert(
                    key.clone(),
                    Version {
                        config: Arc::new(config),
                        raw: Arc::from(raw),
                    },
                );
            }
            if let Some(alias) = alias {
                result.active.insert(alias.clone(), key);
            }
        }
        result.load_us = started.elapsed().as_micros();
        Ok(result)
    }
    pub fn active(&self, alias: &str) -> Option<Arc<Config>> {
        self.active
            .get(alias)
            .and_then(|key| self.versions.get(key))
            .map(|version| version.config.clone())
    }
    /// Startup configuration selected by the manifest.
    pub fn default_config(&self) -> Arc<Config> {
        self.versions[&self.active[&self.default_scenario]]
            .config
            .clone()
    }
    pub(crate) fn find(&self, id: &str, hash: &str) -> Option<Arc<Config>> {
        self.versions
            .get(&(id.into(), hash.into()))
            .map(|version| version.config.clone())
    }
    pub fn describe(&self) -> Value {
        let active: Vec<_> = self
            .active
            .iter()
            .map(|(scenario, (id, hash))| json!({"scenario": scenario, "id": id, "hash": hash}))
            .collect();
        let versions: Vec<_> = self
            .versions
            .iter()
            .map(|((id, hash), version)| {
                json!({"id": id, "hash": hash, "raw": version.raw, "config": version.config,
                "protocolVersion": version.config.protocol_version()})
            })
            .collect();
        json!({
            "defaultScenario": self.default_scenario,
            "active": active,
            "versions": versions,
            "stats": {
                "parsedVersions": self.versions.len(),
                "preparedMaps": self.prepared_maps,
                "loadUs": self.load_us,
            },
        })
    }
}
fn validate_geometry(c: &Config) -> io::Result<()> {
    let point = |p: [f64; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 32768.0);
    let g = &c.geometry;
    if !point(g.min)
        || !point(g.max)
        || (0..2).any(|i| g.min[i] >= g.max[i])
        || g.walls.len() > 128
        || !point(c.spawn)
        || !point(c.extraction_point)
        || !point(c.target.position)
    {
        return Err(invalid("CONFIG_GEOMETRY"));
    }
    for wall in &g.walls {
        if !point(wall.min)
            || !point(wall.max)
            || (0..2).any(|i| {
                wall.min[i] >= wall.max[i] || wall.min[i] < g.min[i] || wall.max[i] > g.max[i]
            })
        {
            return Err(invalid("CONFIG_WALL"));
        }
    }
    if let Some(pve) = &c.pve
        && pve.monster_positions.iter().any(|p| !point(*p))
    {
        return Err(invalid("CONFIG_MONSTER_POINT"));
    }
    if let Some(pve) = &c.pve {
        if (pve.treasure_rules.is_none() && !pve.chest_spawns.is_empty())
            || pve.chest_spawns.len() > 16
        {
            return Err(invalid("CONFIG_CHEST_SCHEMA"));
        }
        let mut ids = std::collections::BTreeSet::new();
        for chest in &pve.chest_spawns {
            if !valid_id(&chest.id)
                || chest.id.len() > 64
                || !ids.insert(&chest.id)
                || !point(chest.position)
            {
                return Err(invalid("CONFIG_CHEST_ID_OR_POINT"));
            }
        }
    }
    Ok(())
}
fn default_path() -> io::Result<PathBuf> {
    let args: Vec<_> = std::env::args_os().collect();
    let path = args
        .windows(2)
        .find(|pair| pair[0] == "--configs")
        .map(|pair| PathBuf::from(&pair[1]))
        .or_else(|| std::env::var_os("MATCH_CONFIG_MANIFEST").map(PathBuf::from))
        .filter(|path| !path.as_os_str().is_empty());
    #[cfg(all(test, match_sdk_test_data))]
    let path = path
        .or_else(|| Some(Path::new(env!("MATCH_SDK_TEST_DATA_DIR")).join("config/catalog.json")));
    path.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "set MATCH_CONFIG_MANIFEST to a host-supplied catalog",
        )
    })
}
pub(crate) fn default_catalog() -> Arc<ConfigCatalog> {
    static DEFAULT: OnceLock<Arc<ConfigCatalog>> = OnceLock::new();
    DEFAULT
        .get_or_init(|| {
            Arc::new(
                default_path()
                    .and_then(ConfigCatalog::load)
                    .expect("load host-supplied configuration catalog"),
            )
        })
        .clone()
}
