use super::*;
use crate::protocol::{WeaponInstance, valid_id};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WeaponCatalog {
    pub models: BTreeMap<String, String>,
    pub families: BTreeMap<String, WeaponFamily>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WeaponFamily {
    pub base_profile: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub morphs: BTreeMap<String, String>,
}

pub(crate) fn weapon_id(id: &str) -> bool {
    id.len() <= 64 && valid_id(id)
}

impl WeaponCatalog {
    pub(crate) fn resolve(&self, instance: &WeaponInstance) -> Option<(&str, WeaponInstance)> {
        if !weapon_id(&instance.instance_id)
            || instance
                .morph_id
                .as_deref()
                .is_some_and(|id| !weapon_id(id))
        {
            return None;
        }
        let family = self.families.get(self.models.get(&instance.model_id)?)?;
        let mut equipped = instance.clone();
        let profile = equipped
            .morph_id
            .as_ref()
            .and_then(|id| family.morphs.get(id));
        if profile.is_none() {
            equipped.morph_id = None;
        }
        Some((profile.unwrap_or(&family.base_profile), equipped))
    }
}

impl Config {
    pub(crate) fn weapon_catalog(&self) -> Option<&WeaponCatalog> {
        self.combat_rules.as_ref()?.weapon_catalog.as_ref()
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(catalog) = c.weapon_catalog() else {
        return Ok(());
    };
    let profiles = c.basic_profiles().unwrap();
    let mut morphs = BTreeSet::new();
    if c.schema_version != 7
        || c.mechanism_version < 105
        || !(1..=64).contains(&catalog.models.len())
        || !(1..=16).contains(&catalog.families.len())
        || catalog
            .models
            .iter()
            .any(|(model, family)| !weapon_id(model) || !catalog.families.contains_key(family))
        || catalog.families.iter().any(|(id, family)| {
            !weapon_id(id)
                || !catalog
                    .models
                    .values()
                    .any(|model_family| model_family == id)
                || !profiles.contains_key(&family.base_profile)
                || family.morphs.iter().any(|(morph, profile)| {
                    !weapon_id(morph)
                        || !morphs.insert(morph)
                        || !profiles.contains_key(profile)
                        || *profile == family.base_profile
                })
        })
        || morphs.len() > super::basic_profiles::MAX_BASIC_PROFILES
    {
        return Err("CONFIG_WEAPON_CATALOG");
    }
    Ok(())
}
