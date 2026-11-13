use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MAX_SLOTS: usize = 8;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcAiConfig {
    pub switch_threat_bps: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub groups: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forced_target_statuses: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, NpcAiProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcAiProfile {
    pub slots: Vec<NpcAiSlot>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub phase_slots: BTreeMap<String, Vec<usize>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub retreat_below: Option<f64>,
}

impl NpcAiProfile {
    pub(crate) fn slots_for(
        &self,
        phase: Option<&str>,
    ) -> impl Iterator<Item = (usize, &NpcAiSlot)> {
        let configured = phase.and_then(|id| self.phase_slots.get(id));
        let len = configured.map_or(self.slots.len(), Vec::len);
        (0..len).map(move |i| {
            let index = configured.map_or(i, |slots| slots[i]);
            (index, &self.slots[index])
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum NpcAiSlot {
    Projectile {
        chance: f64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::protocol::present_value"
        )]
        ability: Option<Box<ProjectileAbility>>,
    },
    Heal {
        chance: f64,
        range: f64,
        include_self: bool,
    },
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(pve) = &c.pve else { return Ok(()) };
    let Some(ai) = &pve.npc_ai else { return Ok(()) };
    if c.mechanism_version < 105 || c.protocol_version() != 6 {
        return Err("CONFIG_NPC_AI_CAPABILITY");
    }
    if !(10000..=100000).contains(&ai.switch_threat_bps)
        || ai.groups.len() > 32
        || ai.profiles.len() > 17
        || ai.forced_target_statuses.len() > 8
    {
        return Err("CONFIG_NPC_AI_BOUNDS");
    }
    let id = crate::protocol::valid_id;
    let spawns: BTreeSet<_> = (0..pve.monster_positions.len())
        .map(|i| format!("{}-{}", pve.template_id, i + 1))
        .chain(pve.extra_spawns.iter().map(|s| s.id.clone()))
        .collect();
    if ai
        .groups
        .iter()
        .any(|(npc, group)| !spawns.contains(npc) || !id(group))
    {
        return Err("CONFIG_NPC_AI_GROUP");
    }
    let mut marks = BTreeSet::new();
    if ai.forced_target_statuses.iter().any(|key| {
        !marks.insert(key)
            || !pve.status_definitions.iter().any(|status| {
                status.id == *key
                    && status.source_scoped
                    && status.is_harmful()
                    && !status.is_periodic()
                    && !status.is_shield()
            })
    }) {
        return Err("CONFIG_NPC_AI_FORCED_TARGET");
    }
    let profiles = pve.build_profiles();
    if profiles
        .values()
        .any(|p| p.combat.return_distance > p.leash)
    {
        return Err("CONFIG_NPC_AI_RETURN_DISTANCE");
    }
    let mut ability_ids: BTreeSet<_> = profiles
        .values()
        .flat_map(|t| [t.combat.projectile.id.as_str(), t.combat.heal.id.as_str()])
        .collect();
    for (template, profile) in &ai.profiles {
        let template = profiles.get(template).ok_or("CONFIG_NPC_AI_PROFILE")?;
        let combat = &template.combat;
        if profile.slots.len() > MAX_SLOTS
            || profile
                .retreat_below
                .is_some_and(|d| !(0.1..combat.melee_range).contains(&d))
        {
            return Err("CONFIG_NPC_AI_PROFILE");
        }
        if profile.phase_slots.iter().any(|(phase, slots)| {
            template
                .boss
                .as_ref()
                .is_none_or(|boss| !boss.phases.iter().any(|p| p.id == *phase))
                || slots.len() > profile.slots.len()
                || slots.iter().any(|i| *i >= profile.slots.len())
                || slots.iter().collect::<BTreeSet<_>>().len() != slots.len()
        }) {
            return Err("CONFIG_NPC_AI_BOSS_SLOTS");
        }
        let mut primary = false;
        let mut healer = false;
        for slot in &profile.slots {
            let (chance, legal) = match slot {
                NpcAiSlot::Projectile {
                    chance,
                    ability: None,
                } => (*chance, !std::mem::replace(&mut primary, true)),
                NpcAiSlot::Projectile {
                    chance,
                    ability: Some(a),
                } => {
                    let legal = id(&a.id)
                        && ability_ids.insert(&a.id)
                        && c.player_skill(&a.id).is_none()
                        && a.cost <= c.resource_bounds().max_spirit_cost
                        && [a.windup_ms, a.cooldown_ms, a.recovery_ms, a.ttl_ms]
                            .iter()
                            .all(|t| *t <= 3_600_000)
                        && a.ttl_ms > 0
                        && a.damage_bps <= 100000
                        && (0.1..=10000.0).contains(&a.range)
                        && (0.1..=10000.0).contains(&a.speed)
                        && (0.0..=128.0).contains(&a.radius)
                        && a.on_commit_self_statuses.is_empty()
                        && a.on_hit_target_statuses.is_empty();
                    (*chance, legal)
                }
                NpcAiSlot::Heal { chance, range, .. } => (
                    *chance,
                    !std::mem::replace(&mut healer, true)
                        && (0.1..=10000.0).contains(range)
                        && combat.heal.heal_bps > 0
                        && combat.heal.hp_threshold > 0.0
                        && combat.heal.interval_ms >= c.step_ms,
                ),
            };
            if !legal || !(0.0..=1.0).contains(&chance) {
                return Err("CONFIG_NPC_AI_SLOT");
            }
        }
    }
    Ok(())
}
