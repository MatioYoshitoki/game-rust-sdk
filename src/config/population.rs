use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryOrb {
    pub hp_bps: u16,
    pub spirit_bps: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MonsterTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heavy: Option<HeavySkill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support: Option<SupportSkill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<AreaSkill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kill_score: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss: Option<BossConfig>,
    pub mechanism: MonsterMechanism,
    pub name: String,
    pub stats: BTreeMap<String, u32>,
    pub combat: MonsterCombat,
    pub drop_table: LootTable,
    pub leash: f64,
    pub help_range: f64,
    pub memory_range: f64,
    pub kill_credit: bool,
    pub drop_orb: bool,
    #[serde(
        default,
        deserialize_with = "crate::protocol::present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub recovery_orb: Option<RecoveryOrb>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MonsterMechanism {
    #[serde(rename = "melee_projectile_heal")]
    MeleeProjectileHeal,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MonsterSpawn {
    pub id: String,
    pub template_id: String,
    pub position: [f64; 2],
}
impl PveConfig {
    pub fn build_profiles(&self) -> BTreeMap<String, Arc<MonsterTemplate>> {
        let mut profiles: BTreeMap<_, _> = self
            .additional_templates
            .iter()
            .map(|(id, p)| (id.clone(), Arc::new(p.clone())))
            .collect();
        profiles.insert(
            self.template_id.clone(),
            Arc::new(MonsterTemplate {
                support: None,
                heavy: None,
                boss: None,
                area: None,
                kill_score: self.kill_score,
                mechanism: MonsterMechanism::MeleeProjectileHeal,
                name: self.template_id.clone(),
                stats: self.stats.clone(),
                combat: self.combat.clone(),
                drop_table: self.drop_table.clone(),
                leash: self.leash,
                help_range: self.help_range,
                memory_range: self.memory_range,
                kill_credit: true,
                drop_orb: true,
                recovery_orb: None,
            }),
        );
        profiles
    }
    pub fn guardian(&self) -> Option<&GuardianConfig> {
        self.treasure_rules.as_ref()?.template.guardian.as_ref()
    }
    /// Maximum instances over the bounded match lifetime; not a claim of actual kills.
    pub fn population_bound(&self, now: u64) -> Vec<(Arc<MonsterTemplate>, u32)> {
        let profiles = if self.profiles.is_empty() {
            self.build_profiles()
        } else {
            self.profiles.clone()
        };
        self.population_counts(now)
            .into_iter()
            .map(|(id, count)| (profiles[&id].clone(), count))
            .collect()
    }
    pub(crate) fn population_counts(&self, now: u64) -> BTreeMap<String, u32> {
        let mut counts = BTreeMap::from([(
            self.template_id.clone(),
            self.monster_positions.len() as u32,
        )]);
        for spawn in self
            .extra_spawns
            .iter()
            .chain(self.encounters.iter().flat_map(|e| e.all_monsters()))
        {
            *counts.entry(spawn.template_id.clone()).or_default() += 1;
        }
        if let Some(g) = self.guardian().filter(|g| g.chance > 0.0) {
            let r = self.treasure_rules.as_ref().unwrap();
            let unlock = r
                .template
                .seal
                .as_ref()
                .map_or(0, |s| u64::from(s.layers) * s.layer_duration_ms);
            let generations = now / (r.template.respawn_delay_ms + unlock) + 1;
            *counts.entry(g.template_id.clone()).or_default() +=
                self.chest_spawns.len() as u32 * g.max_count * generations as u32;
        }
        counts
    }
}
impl Config {
    pub(crate) fn max_kills(&self, now: u64) -> u32 {
        self.pve.as_ref().map_or(1, |p| {
            p.population_bound(now.min(self.terminal_ms))
                .iter()
                .filter(|(t, _)| t.kill_credit)
                .map(|(_, n)| *n)
                .sum()
        })
    }
}
pub(super) fn validate_npc_ignore_armor(c: &Config) -> Result<(), &'static str> {
    let configured = c.pve.iter().any(|p| {
        p.combat.projectile.ignore_armor.is_some()
            || p.additional_templates
                .values()
                .any(|t| t.combat.projectile.ignore_armor.is_some())
    });
    if configured && (c.schema_version != 7 || c.mechanism_version < 103) {
        return Err("CONFIG_DIRECT_IGNORE_ARMOR_CAPABILITY");
    }
    Ok(())
}

pub(super) fn validate_npc_unavoidable(c: &Config) -> Result<(), &'static str> {
    let configured = c.pve.iter().any(|p| {
        p.combat.projectile.unavoidable.is_some()
            || p.additional_templates
                .values()
                .any(|t| t.combat.projectile.unavoidable.is_some())
    });
    if configured && (c.schema_version != 7 || c.mechanism_version < 101) {
        return Err("CONFIG_NPC_UNAVOIDABLE_CAPABILITY");
    }
    Ok(())
}

pub(super) fn validate_npc_hit(c: &Config) -> Result<(), &'static str> {
    let has_hit = c.pve.iter().any(|p| {
        p.stats.contains_key("hit")
            || p.additional_templates
                .values()
                .any(|t| t.stats.contains_key("hit"))
    });
    if has_hit && (c.schema_version != 7 || c.mechanism_version < 99) {
        return Err("CONFIG_NPC_HIT_CAPABILITY");
    }
    Ok(())
}

pub(super) fn validate_recovery_orbs(c: &Config) -> Result<(), &'static str> {
    for template in c.pve.iter().flat_map(|p| p.additional_templates.values()) {
        if let Some(orb) = template.recovery_orb {
            if c.schema_version != 7 || c.mechanism_version < 71 {
                return Err("CONFIG_RECOVERY_ORB_CAPABILITY");
            }
            if !template.drop_orb
                || orb.hp_bps > 10_000
                || orb.spirit_bps > 10_000
                || orb.hp_bps == 0 && orb.spirit_bps == 0
            {
                return Err("CONFIG_RECOVERY_ORB_BOUNDS");
            }
        }
    }
    Ok(())
}

pub(super) fn validate_population(
    c: &Config,
    p: &PveConfig,
    skill_limit: usize,
) -> Result<(), &'static str> {
    let guardian = p.guardian();
    if p.additional_templates.is_empty()
        && p.extra_spawns.is_empty()
        && p.encounters.is_empty()
        && guardian.is_none()
    {
        return Ok(());
    }
    if c.mechanism_version < 3 {
        return Err("CONFIG_SCHEMA_POPULATION_CAPABILITY");
    }
    let id = |s: &str| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control);
    if p.additional_templates.len() > 16 || p.extra_spawns.len() > 32 {
        return Err("CONFIG_POPULATION_BUDGET");
    }
    let mut ids: std::collections::BTreeSet<_> = (0..p.monster_positions.len())
        .map(|i| format!("{}-{}", p.template_id, i + 1))
        .collect();
    for spawn in p
        .extra_spawns
        .iter()
        .chain(p.encounters.iter().flat_map(|e| e.all_monsters()))
    {
        if !id(&spawn.id)
            || spawn.id.starts_with("guardian-")
            || !ids.insert(spawn.id.clone())
            || spawn.template_id != p.template_id
                && !p.additional_templates.contains_key(&spawn.template_id)
        {
            return Err("CONFIG_MONSTER_REFERENCE");
        }
    }
    for (key, t) in &p.additional_templates {
        super::boss::validate(c, t)?;
        super::area::validate(c, t)?;
        super::support::validate(c, p, t)?;
        super::heavy::validate(c, t)?;
        if !id(key) || key == &p.template_id || t.name.is_empty() {
            return Err("CONFIG_MONSTER_REFERENCE");
        }
        let mut candidate = c.clone();
        let v = Arc::make_mut(candidate.pve.as_mut().unwrap());
        v.additional_templates.clear();
        v.extra_spawns.clear();
        v.encounters.clear();
        v.map_objective = None;
        v.timed_position = None;
        v.profiles.clear();
        v.monster_positions.clear();
        if let Some(r) = &mut v.treasure_rules {
            r.template.guardian = None;
        }
        v.template_id = key.clone();
        v.stats = t.stats.clone();
        v.combat = t.combat.clone();
        v.drop_table = t.drop_table.clone();
        v.leash = t.leash;
        v.help_range = t.help_range;
        v.memory_range = t.memory_range;
        super::validate::validate(&candidate, skill_limit)?;
    }
    if let Some(g) = guardian {
        if p.template_id.starts_with("guardian-") {
            return Err("CONFIG_MONSTER_REFERENCE");
        }
        if !p.additional_templates.contains_key(&g.template_id)
            || !g.chance.is_finite()
            || !(0.0..=1.0).contains(&g.chance)
            || g.min_count == 0
            || g.min_count > g.max_count
            || g.max_count > 4
            || g.spawn_offsets.len() < g.max_count as usize
            || g.spawn_offsets.len() > 16
            || g.spawn_offsets
                .iter()
                .any(|o| o.iter().any(|n| !n.is_finite() || n.abs() > 1000.0))
        {
            return Err("CONFIG_GUARDIAN_BOUNDS_OR_REFERENCE");
        }
        let radius = p.additional_templates[&g.template_id].combat.radius;
        if g.spawn_offsets.iter().enumerate().any(|(i, a)| {
            g.spawn_offsets[..i]
                .iter()
                .any(|b| distance(*a, *b) < radius * 2.0)
        }) {
            return Err("CONFIG_GUARDIAN_OFFSETS");
        }
    }
    super::encounters::validate(c, p)?;
    let reserved = p.monster_positions.len()
        + p.extra_spawns.len()
        + p.encounters
            .iter()
            .map(|e| e.all_monsters().count())
            .sum::<usize>()
        + guardian.map_or(0, |g| p.chest_spawns.len() * g.max_count as usize);
    if reserved > 32 || c.max_kills(c.terminal_ms) > 4096 {
        return Err("CONFIG_POPULATION_BUDGET");
    }
    Ok(())
}
