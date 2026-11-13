use super::*;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TwoMembersConfig {
    pub spawns: Vec<[f64; 2]>,
    pub deployment_timeout_ms: u64,
    pub publication_queue_capacity: usize,
    pub projectile_capacity: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MembersConfig {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub training_targets: Option<Vec<super::training_targets::TrainingSeat>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub training_runtime: Option<super::training_targets::TrainingRuntime>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub member_conflict: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub spectators: Option<SpectatorsConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoint_schedules: Vec<super::checkpoint_zones::CheckpointSchedule>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_area_capacity"
    )]
    pub area_capacity: Option<usize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_area_capacity"
    )]
    pub basic_field_capacity: Option<usize>,
    pub capacity: usize,
    pub spawns: Vec<[f64; 2]>,
    pub deployment_timeout_ms: u64,
    pub publication_queue_capacity: usize,
    pub projectile_capacity: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_tools: Option<TrainingTools>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrainingTools {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restore_clears: Vec<String>,
}
pub(crate) const MAX_SPECTATORS: u8 = 8;
pub(crate) const MAX_SPECTATOR_EVENTS: usize = 32;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpectatorsConfig {
    pub capacity: u8,
    pub terminal_retention_ms: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicationConfig {
    pub ack_timeout_ms: u64,
    pub baseline_ttl_ms: u64,
    pub full_interval_ms: u64,
    pub terminal_grace_ms: u64,
}
#[derive(Clone, Copy)]
pub(crate) struct MemberSettings<'a> {
    pub capacity: usize,
    pub spawns: &'a [[f64; 2]],
    pub deployment_timeout_ms: u64,
    pub publication_queue_capacity: usize,
    pub projectile_capacity: usize,
}

// The original validators still validate every shared field. Their shadow version
// is internal only: raw bytes/hash, normalized catalog and public protocol stay v4.
pub(super) fn validation_document(v: &Value) -> Result<Value, &'static str> {
    let replicated = v["schema_version"] == 7;
    if v.pointer("/params/entry_years").and_then(Value::as_u64) == Some(0)
        && (!replicated || !matches!(v["mechanism_version"].as_u64(), Some(105..=106)))
    {
        return Err("CONFIG_PARAMETER_BOUNDS");
    }
    let multi = v["schema_version"] == 6 || replicated;
    let two = v["schema_version"] == 5;
    if !multi && !two {
        return if v.get("two_members").is_some()
            || v.get("members").is_some()
            || v.get("replication").is_some()
        {
            Err("CONFIG_MEMBERS_VERSION")
        } else {
            Ok(v.clone())
        };
    }
    if two
        && (v["mechanism_version"] != 35
            || !v["two_members"].is_object()
            || v.get("members").is_some())
        || multi
            && (if replicated {
                !matches!(v["mechanism_version"].as_u64(), Some(37..=106))
            } else {
                v["mechanism_version"] != 36
            } || !v["members"].is_object()
                || v.get("two_members").is_some())
    {
        return Err("CONFIG_MEMBERS_VERSION");
    }
    if replicated != v.get("replication").is_some() || replicated && !v["replication"].is_object() {
        return Err("CONFIG_REPLICATION_VERSION");
    }
    let mut shadow = v.clone();
    shadow.as_object_mut().unwrap().remove("two_members");
    shadow.as_object_mut().unwrap().remove("members");
    shadow.as_object_mut().unwrap().remove("replication");
    shadow["schema_version"] = 4.into();
    shadow["mechanism_version"] = 34.into();
    Ok(shadow)
}
impl Config {
    pub(crate) fn member_conflict(&self) -> bool {
        self.members.as_ref().is_some_and(|m| m.member_conflict)
    }

    pub(crate) fn spectators(&self) -> Option<&SpectatorsConfig> {
        self.members.as_ref()?.spectators.as_ref()
    }

    pub(crate) fn member_settings(&self) -> Option<MemberSettings<'_>> {
        if let Some(m) = &self.members {
            Some(MemberSettings {
                capacity: m.capacity,
                spawns: &m.spawns,
                deployment_timeout_ms: m.deployment_timeout_ms,
                publication_queue_capacity: m.publication_queue_capacity,
                projectile_capacity: m.projectile_capacity,
            })
        } else {
            self.two_members.as_ref().map(|m| MemberSettings {
                capacity: 2,
                spawns: &m.spawns,
                deployment_timeout_ms: m.deployment_timeout_ms,
                publication_queue_capacity: m.publication_queue_capacity,
                projectile_capacity: m.projectile_capacity,
            })
        }
    }
    pub(crate) fn single_member_validation(&self) -> Self {
        let mut c = self.clone();
        if matches!(c.schema_version, 5..=7) {
            c.schema_version = 4;
            c.mechanism_version = 34;
            c.two_members = None;
            c.members = None;
            c.replication = None;
        }
        c
    }
}
pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(d) = c.member_settings() else {
        return if matches!(c.schema_version, 5..=7) {
            Err("CONFIG_MEMBERS_VERSION")
        } else {
            Ok(())
        };
    };
    if !matches!(
        (
            c.schema_version,
            c.mechanism_version,
            c.two_members.is_some(),
            c.members.is_some()
        ),
        (5, 35, true, false) | (6, 36, false, true) | (7, 37..=106, false, true)
    ) || c.asset_contract.is_none()
        || c.result_contract.is_none()
    {
        return Err("CONFIG_MEMBERS_VERSION");
    }
    let area_capacity = c.members.as_ref().and_then(|m| m.area_capacity);
    let basic_field_capacity = c.members.as_ref().and_then(|m| m.basic_field_capacity);
    let max_field_targets = c
        .basic_configs()
        .filter_map(|basic| basic.field().map(|field| field.target_cap))
        .max();
    if basic_field_capacity.is_some_and(|n| c.mechanism_version < 105 || !(1..=12).contains(&n))
        || max_field_targets.is_some() && basic_field_capacity.is_none()
    {
        return Err("CONFIG_BASIC_FIELD_CAPACITY");
    }
    if (c.mechanism_version >= 41) != area_capacity.is_some()
        || area_capacity.is_some_and(|n| !(1..=8).contains(&n))
    {
        return Err("CONFIG_MEMBER_AREA_CAPACITY");
    }
    if c.mechanism_version >= 41
        && c.player_skills()
            .any(|s| s.is_field() && s.contact_policy.is_none())
    {
        return Err("CONFIG_MEMBER_AREA_POLICY");
    }
    if (c.schema_version == 7) != c.replication.is_some() {
        return Err("CONFIG_REPLICATION_VERSION");
    }
    if c.member_conflict()
        && (c.mechanism_version != 105
            || c.schema_version != 7
            || c.combat_rules.is_none()
            || c.match_mode.is_some()
            || d.capacity < 2)
    {
        return Err("CONFIG_MEMBER_CONFLICT");
    }
    if let Some(r) = &c.replication
        && (!(100..=60000).contains(&r.ack_timeout_ms)
            || !(r.ack_timeout_ms..=60000).contains(&r.baseline_ttl_ms)
            || !(100..=r.baseline_ttl_ms).contains(&r.full_interval_ms)
            || !(2 * r.ack_timeout_ms..=60000).contains(&r.terminal_grace_ms))
    {
        return Err("CONFIG_REPLICATION_BOUNDS");
    }
    if let Some(s) = c.spectators()
        && (c.mechanism_version != 105
            || !(1..=MAX_SPECTATORS).contains(&s.capacity)
            || c.replication.as_ref().is_none_or(|r| {
                !(2 * r.ack_timeout_ms..=3_600_000).contains(&s.terminal_retention_ms)
            }))
    {
        return Err("CONFIG_SPECTATORS");
    }
    if !(1..=8).contains(&d.capacity)
        || d.spawns.len() != d.capacity
        || !d.spawns.iter().flatten().all(|x| x.is_finite())
        || d.spawns.iter().enumerate().any(|(i, &a)| {
            d.spawns[..i]
                .iter()
                .any(|&b| distance(a, b) <= 2. * c.params.player_radius)
        })
        || d.spawns.first() != Some(&c.spawn)
        || !(100..=60000).contains(&d.deployment_timeout_ms)
        || !(1..=8).contains(&d.publication_queue_capacity)
        || !(1..=64).contains(&d.projectile_capacity)
        || c.output_capacity < 128.max(64 * d.capacity)
    {
        return Err("CONFIG_MEMBERS_BOUNDS");
    }
    let summon_reserve = c.member_summons().map_or(0, |s| {
        12 * s.contact_reserve(c, d.capacity, d.projectile_capacity)
    });
    let base_output_budget =
        64 * d.capacity + summon_reserve + c.spectators().map_or(0, |s| usize::from(s.capacity));
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_MEMBER_SUMMON_OUTPUT_BUDGET");
    }
    let boon_shield_reserve = c.world_boons.as_ref().map_or(0, |boons| {
        2 * d.capacity * usize::from(boons.definitions.values().any(|boon| boon.shield.is_some()))
    });
    let base_output_budget = base_output_budget + boon_shield_reserve;
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_BOON_SHIELD_OUTPUT_BUDGET");
    }
    let timed_boons = c.world_boons.as_ref().is_some_and(|boons| {
        boons
            .opportunities
            .iter()
            .any(|source| source.interaction_ms.is_some())
    });
    let base_output_budget =
        base_output_budget + usize::from(timed_boons) * d.capacity * (d.capacity + 1);
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_BOON_INTERACTION_OUTPUT_BUDGET");
    }
    super::checkpoint_zones::validate(c, d.capacity)?;
    let automatic_counters = c
        .checkpoint_schedules()
        .iter()
        .filter(|s| {
            s.counter
                .as_ref()
                .is_some_and(|r| r.activation != super::checkpoint_zones::CounterActivation::Manual)
        })
        .count();
    let base_output_budget = base_output_budget + 12 * d.capacity * automatic_counters;
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_CHECKPOINT_COUNTER_OUTPUT_BUDGET");
    }
    let base_output_budget =
        base_output_budget + usize::from(c.match_mode.is_some()) * (4 * d.capacity + 1);
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_MATCH_MODE_OUTPUT_BUDGET");
    }
    let Some(p) = &c.pve else {
        return Err("CONFIG_TWO_MEMBERS_UNSUPPORTED");
    };
    if p.monster_positions.len() + p.extra_spawns.len() > 32
        || p.items.len() > 16
        || p.status_definitions.len() > super::status::definition_limit(c)
        || p.timed_position.is_some()
        || p.map_objective.is_some()
        || !p.encounters.is_empty()
        || !p.chest_spawns.is_empty()
        || p.treasure_rules.is_some()
        || p.status_definitions.iter().any(|d| {
            d.is_shield() && c.mechanism_version < 39
                || d.is_periodic() && c.mechanism_version < 38
                || c.mechanism_version < 69
                    && d.effective_attribute() == crate::protocol::StatusAttribute::HealingReceived
        })
        || c.player_skills().any(|s| {
            !(matches!(
                s.mechanism,
                PlayerSkillMechanism::Projectile | PlayerSkillMechanism::SelfStatus
            ) || c.mechanism_version >= 40 && s.is_unit()
                || c.mechanism_version >= 41 && s.is_ground()
                || c.mechanism_version >= 42 && s.is_line()
                || c.mechanism_version >= 43 && s.is_piercing()
                || c.mechanism_version >= 78 && s.is_ally_heal()
                || c.mechanism_version >= 80 && s.is_ally_shield())
        })
    {
        return Err("CONFIG_TWO_MEMBERS_UNSUPPORTED");
    }
    let terrain_reserve = if p.world_geometry.is_some()
        && p.palette
            .iter()
            .any(|tile| tile.damage_milli_per_second() > 0)
    {
        let summons = c.member_summons().map_or(0, |s| s.max_active_per_member);
        12 * (d.capacity * (1 + summons) + p.monster_positions.len() + p.extra_spawns.len())
    } else {
        0
    };
    let base_output_budget = base_output_budget + terrain_reserve;
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_TERRAIN_OUTPUT_BUDGET");
    }
    let extraction_reserve = c.extraction_lifecycle.as_ref().map_or(0, |rule| {
        usize::from(rule.broadcast_channels) * d.capacity
            + usize::from(rule.pressure.is_some())
                * (12 * d.capacity + p.monster_positions.len() + p.extra_spawns.len())
    });
    let base_output_budget = base_output_budget + extraction_reserve;
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_EXTRACTION_OUTPUT_BUDGET");
    }
    let npc_ai_reserve = p.npc_ai.as_ref().map_or(0, |ai| {
        4 * p
            .monster_positions
            .iter()
            .map(|_| p.template_id.as_str())
            .chain(
                p.extra_spawns
                    .iter()
                    .map(|spawn| spawn.template_id.as_str()),
            )
            .filter(|id| {
                ai.profiles
                    .get(*id)
                    .is_some_and(|profile| !profile.slots.is_empty())
            })
            .count()
    });
    let profiles = p.build_profiles();
    let boss_reserve: usize = p
        .monster_positions
        .iter()
        .map(|_| p.template_id.as_str())
        .chain(
            p.extra_spawns
                .iter()
                .map(|spawn| spawn.template_id.as_str()),
        )
        .filter_map(|id| profiles.get(id)?.boss.as_ref())
        .map(|boss| boss.phases.len())
        .sum();
    let base_output_budget = base_output_budget + npc_ai_reserve + boss_reserve;
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_NPC_AI_OUTPUT_BUDGET");
    }
    let base_output_budget = base_output_budget + 4 * c.participant_ai.len();
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_PARTICIPANT_AI_OUTPUT_BUDGET");
    }
    let world_loot_reserve = c.world_loot.as_ref().map_or(0, |world| {
        world.environment_capacity
            + world.container_capacity
            + world.one_shot_capacity
            + 4 * d.capacity
    });
    let base_output_budget = base_output_budget + world_loot_reserve;
    if base_output_budget > c.output_capacity {
        return Err("CONFIG_WORLD_LOOT_OUTPUT_BUDGET");
    }
    if c.player_skills()
        .flat_map(|s| &s.on_hit_target_statuses)
        .any(|id| {
            p.status_definitions.iter().any(|d| {
                &d.id == id
                    && d.effective_attribute() == crate::protocol::StatusAttribute::HealingReceived
            })
        })
    {
        return Err("CONFIG_MEMBER_HEALING_RECEIVED_BINDING");
    }
    for (id, t) in profiles {
        let ai = p.npc_ai.as_ref().and_then(|ai| ai.profiles.get(&id));
        let has_heal_slot = ai.is_some_and(|profile| {
            profile
                .slots
                .iter()
                .any(|slot| matches!(slot, super::npc_ai::NpcAiSlot::Heal { .. }))
        });
        if t.area.is_some()
            || t.heavy.is_some()
            || t.support.is_some()
            || t.boss.is_some() && ai.is_none()
            || t.combat.lifesteal_ratio != 0.
            || t.combat.mark_max != 0
            || t.combat.heal.hp_threshold != 0. && !has_heal_slot
            || if c.mechanism_version < 38 {
                !t.combat.projectile.on_hit_target_statuses.is_empty()
                    || !t.combat.projectile.on_commit_self_statuses.is_empty()
            } else {
                t.combat.projectile.on_hit_target_statuses.iter().any(|id| {
                    !p.status_definitions.iter().any(|d| {
                        &d.id == id
                            && (c
                                .combat_rules
                                .as_ref()
                                .is_some_and(|r| r.cast_interceptions.contains_key(id))
                                || c.combat_rules.is_some()
                                    && !d.flags.is_empty()
                                    && d.is_hostile_control()
                                || d.is_periodic() && !d.is_heal()
                                || c.mechanism_version >= 39 && d.is_shield()
                                || c.mechanism_version >= 74
                                    && d.attribute
                                        == Some(
                                            crate::protocol::StatusAttribute::SpiritRecoveryLock,
                                        )
                                || c.mechanism_version >= 83
                                    && d.attribute
                                        == Some(crate::protocol::StatusAttribute::Silence)
                                || c.mechanism_version >= 88
                                    && d.attribute == Some(crate::protocol::StatusAttribute::Root)
                                || c.mechanism_version >= 89
                                    && d.attribute == Some(crate::protocol::StatusAttribute::Stun)
                                || c.mechanism_version >= 90
                                    && d.attribute
                                        == Some(crate::protocol::StatusAttribute::MovementSlow))
                    })
                }) || t
                    .combat
                    .projectile
                    .on_commit_self_statuses
                    .iter()
                    .any(|id| {
                        !p.status_definitions.iter().any(|d| {
                            &d.id == id
                                && (c.combat_rules.is_some()
                                    && !d.flags.is_empty()
                                    && !d.is_hostile_control()
                                    || c.mechanism_version >= 105
                                        && c.combat_rules.is_some()
                                        && matches!(
                                            d.mechanism,
                                            StatusMechanism::TimedAttributeModifier
                                                | StatusMechanism::TimedDefenseModifier
                                                | StatusMechanism::TimedCompoundModifier
                                        )
                                        && !d.is_hostile_control()
                                    || d.is_heal() && d.overheal_shield.is_none()
                                    || c.mechanism_version >= 39 && d.is_shield())
                        })
                    })
            }
            || t.drop_table.guaranteed.len() + t.drop_table.random.len() > 8
        {
            return Err("CONFIG_TWO_MEMBERS_UNSUPPORTED");
        }
    }
    if c.mechanism_version >= 38
        && p.status_rules.as_ref().is_some_and(|r| {
            (d.capacity + p.monster_positions.len() + p.extra_spawns.len())
                * r.max_active_per_entity
                > 128
        })
    {
        return Err("CONFIG_MEMBERS_STATUS_BUDGET");
    }
    if let Some(capacity) = area_capacity {
        let enter_areas = c
            .combat_rules
            .iter()
            .flat_map(|r| r.npc_enter_areas.values());
        let traps = c
            .combat_rules
            .iter()
            .flat_map(|r| r.member_ground_traps.values());
        if c.player_skills()
            .filter_map(|s| s.contact_policy.as_ref())
            .chain(enter_areas.clone().map(|area| &area.contact_policy))
            .chain(traps.map(|trap| &trap.contact_policy))
            .map(|p| u64::from(p.max_total_hits))
            .max()
            .unwrap_or(0)
            * capacity as u64
            > 128
        {
            return Err("CONFIG_MEMBER_AREA_HISTORY_BUDGET");
        }
        let npcs = p.monster_positions.len() + p.extra_spawns.len();
        let per_plan = c
            .player_skills()
            .filter(|s| (s.is_ground() || s.is_line()) && c.member_cast_anchor(&s.id).is_none())
            .map(|s| {
                c.member_ground_trap(&s.id)
                    .map(|trap| &trap.contact_policy)
                    .or(s.contact_policy.as_ref())
                    .map_or(npcs, |p| npcs.min(p.max_targets_per_tick as usize))
            })
            .chain(enter_areas.map(|area| {
                d.capacity
                    .min(area.contact_policy.max_targets_per_tick as usize)
            }))
            .chain(
                c.combat_rules
                    .iter()
                    .flat_map(|rules| rules.npc_ground_fields.values())
                    .map(|field| d.capacity + d.capacity.min(field.max_targets_per_tick as usize)),
            )
            .chain(
                c.combat_rules
                    .iter()
                    .filter(|r| !r.npc_ground_strikes.is_empty())
                    .map(|_| 1),
            )
            .max()
            .unwrap_or(0);
        let field_contacts =
            basic_field_capacity.unwrap_or(0) * max_field_targets.unwrap_or(0).min(npcs);
        if base_output_budget + 12 * (capacity * per_plan + field_contacts) > c.output_capacity {
            return Err("CONFIG_MEMBER_AREA_OUTPUT_BUDGET");
        }
    }
    if c.mechanism_version >= 43 {
        let hits = c
            .player_skills()
            .filter(|s| s.is_piercing())
            .map(|s| s.max_hits)
            .chain(
                c.basic_configs()
                    .filter_map(|b| b.piercing.as_ref().map(|p| p.max_hits)),
            )
            .max()
            .unwrap_or(0);
        if u64::from(hits) * d.projectile_capacity as u64 > 128 {
            return Err("CONFIG_MEMBER_PIERCING_HISTORY_BUDGET");
        }
        let targets = (p.monster_positions.len() + p.extra_spawns.len()).min(hits as usize);
        if base_output_budget + 12 * d.projectile_capacity * targets > c.output_capacity {
            return Err("CONFIG_MEMBER_PIERCING_OUTPUT_BUDGET");
        }
    }
    if c.basic_configs()
        .filter_map(|basic| basic.wave.as_ref())
        .any(|wave| base_output_budget + 12 * wave.target_cap * d.capacity > c.output_capacity)
    {
        return Err("CONFIG_BASIC_WAVE_OUTPUT_BUDGET");
    }
    if c.basic_configs()
        .filter_map(|basic| basic.impact.as_ref())
        .any(|plan| base_output_budget + 12 * plan.max_contacts() * d.capacity > c.output_capacity)
    {
        return Err("CONFIG_BASIC_IMPACT_OUTPUT_BUDGET");
    }
    let npcs = p.monster_positions.len() + p.extra_spawns.len();
    if c.basic_configs()
        .filter_map(|basic| {
            basic
                .impact
                .as_ref()?
                .contact_status
                .as_ref()?
                .death_burst
                .as_ref()
        })
        .any(|burst| {
            base_output_budget + 12 * npcs * burst.target_cap.min(npcs.saturating_sub(1))
                > c.output_capacity
        })
    {
        return Err("CONFIG_BASIC_DEATH_BURST_OUTPUT_BUDGET");
    }
    if c.basic_configs()
        .filter_map(|basic| basic.beam.as_ref())
        .any(|beam| base_output_budget + 12 * beam.target_cap * d.capacity > c.output_capacity)
    {
        return Err("CONFIG_BASIC_BEAM_OUTPUT_BUDGET");
    }
    if c.basic_configs()
        .filter_map(|basic| basic.projectile_followup.as_ref())
        .any(|plan| {
            let segments = match &plan.pattern {
                super::BasicProjectilePattern::Companion { .. } => 1,
                super::BasicProjectilePattern::Volley { hits, .. } => *hits as usize,
                super::BasicProjectilePattern::Ricochet(_) => 1,
                super::BasicProjectilePattern::SealRecall(rule) => rule.required_seals as usize,
            };
            let max_contacts_per_step =
                if matches!(plan.pattern, super::BasicProjectilePattern::SealRecall(_)) {
                    let npcs = p.monster_positions.len() + p.extra_spawns.len();
                    d.projectile_capacity
                        .max((segments * d.capacity).min(d.projectile_capacity) * npcs)
                } else {
                    d.projectile_capacity
                };
            segments > d.projectile_capacity
                || base_output_budget + 12 * max_contacts_per_step > c.output_capacity
        })
    {
        return Err("CONFIG_BASIC_PROJECTILE_FOLLOWUP_BUDGET");
    }
    let resource_bound: u64 = p
        .population_bound(c.terminal_ms)
        .iter()
        .map(|(t, n)| {
            u64::from(*n)
                * (t.drop_table.guaranteed.len()
                    + t.drop_table.random.len()
                    + usize::from(t.drop_orb)) as u64
        })
        .sum();
    let player_drops = c
        .member_loot_drop
        .map_or(0, |definition| definition.max_active as u64);
    let new_resources = c.world_loot.as_ref().map_or(0, |world| {
        (world.environment_capacity + world.container_capacity + world.one_shot_capacity) as u64
    });
    if resource_bound + player_drops + new_resources > 128 {
        return Err("CONFIG_TWO_MEMBERS_RESOURCE_BUDGET");
    }
    Ok(())
}
pub(super) fn validate_prepared(c: &Config) -> Result<(), &'static str> {
    if let Some(d) = c.member_settings() {
        let p = c.pve.as_ref().ok_or("CONFIG_TWO_MEMBERS_UNSUPPORTED")?;
        if d.spawns
            .iter()
            .any(|&spawn| !crate::world::pve::validate_map(p, spawn, c.extraction_point))
        {
            return Err("CONFIG_TWO_MEMBERS_SPAWN");
        }
        if !c.checkpoint_schedules().is_empty()
            && !crate::world::pve::validate_walkable_centers(
                p,
                c.checkpoint_schedules()
                    .iter()
                    .flat_map(|s| s.centers.iter().copied()),
                c.params.player_radius,
            )
        {
            return Err("CONFIG_CHECKPOINT_CENTER");
        }
        if !c.participant_ai.is_empty()
            && !crate::world::pve::validate_walkable_centers(
                p,
                c.participant_ai
                    .values()
                    .flat_map(|policy| policy.search_points.iter().copied()),
                c.params.player_radius,
            )
        {
            return Err("CONFIG_PARTICIPANT_AI_SEARCH_POINT");
        }
        if let Some(world) = &c.world_loot
            && !crate::world::pve::validate_walkable_centers(
                p,
                world.containers.iter().map(|container| container.position),
                c.params.player_radius,
            )
        {
            return Err("CONFIG_WORLD_LOOT_CONTAINER_POINT");
        }
    }
    Ok(())
}

fn present_area_capacity<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<usize>, D::Error> {
    usize::deserialize(d).map(Some)
}
