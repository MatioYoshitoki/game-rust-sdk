//! Validated definitions of catalog-selected basic attacks.
use super::*;
use crate::protocol::{StatusAttribute, valid_id};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) const MAX_BASIC_PROFILES: usize = 32;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicEffects {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub on_release_self_status: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub on_end_damage_self_status: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub on_end_damage_target_status: Option<BasicEndTargetStatus>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub on_end_damage_self_heal_max_hp_bps: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasicEndTargetStatus {
    pub choices: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub target_profile_bps: BTreeMap<String, u32>,
}

impl Config {
    pub(crate) fn basic_profiles(&self) -> Option<&BTreeMap<String, BasicConfig>> {
        self.combat_rules
            .as_ref()
            .map(|rules| &rules.basic_profiles)
    }

    pub(crate) fn basic_configs(&self) -> impl Iterator<Item = &BasicConfig> {
        std::iter::once(&self.basic).chain(
            self.basic_profiles()
                .into_iter()
                .flat_map(|profiles| profiles.values()),
        )
    }
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    if let Some(profiles) = c.basic_profiles()
        && (profiles.len() > MAX_BASIC_PROFILES
            || profiles.iter().any(|(id, basic)| {
                !valid_id(id)
                    || id.len() > 64
                    || !basic.settle_on_end
                    || basic.windup_ms > 3_600_000
                    || !(1..=3_600_000).contains(&basic.interval_ms)
                    || !(0.1..=10000.).contains(&basic.range)
                    || !(0.1..=10000.).contains(&basic.projectile_speed)
            }))
    {
        return Err("CONFIG_BASIC_PROFILES_BOUNDS");
    }
    for basic in c.basic_configs() {
        validate_basic(c, basic)?;
    }
    super::weapon_equipment::validate(c)
}

fn validate_basic(c: &Config, basic: &BasicConfig) -> Result<(), &'static str> {
    if (basic.settle_on_end
        || basic.piercing.is_some()
        || basic.non_projectile()
        || basic.beam.is_some()
        || basic.projectile_followup.is_some()
        || basic.effects.is_some()
        || basic.charge.is_some()
        || basic.stacked_wave.is_some())
        && (c.schema_version != 7 || c.mechanism_version < 105 || c.combat_rules.is_none())
    {
        return Err("CONFIG_BASIC_DELIVERY_CAPABILITY");
    }
    if let Some(plan) = &basic.piercing
        && (!crate::protocol::valid_id(&plan.plan_id)
            || plan.plan_id.len() > 64
            || c.player_skill(&plan.plan_id).is_some()
            || !(1..=128).contains(&plan.max_hits)
            || !(c.step_ms.saturating_mul(2)..=60000).contains(&c.params.player_projectile_ttl_ms))
    {
        return Err("CONFIG_BASIC_PIERCING_BOUNDS");
    }
    if let Some(wave) = &basic.wave
        && (!basic.settle_on_end || basic.piercing.is_some() || invalid_wave(c, basic.range, wave))
    {
        return Err("CONFIG_BASIC_WAVE_BOUNDS");
    }
    if let Some(charge) = &basic.charge
        && !valid_charge(c, basic, charge)
    {
        return Err("CONFIG_BASIC_CHARGE_BOUNDS");
    }
    if let Some(upgrade) = &basic.stacked_wave
        && !valid_stacked_wave(c, basic, upgrade)
    {
        return Err("CONFIG_BASIC_STACKED_WAVE");
    }
    if let Some(impact) = &basic.impact
        && (!basic.settle_on_end
            || basic.piercing.is_some()
            || basic.wave.is_some()
            || !valid_id(&impact.plan_id)
            || impact.plan_id.len() > 64
            || c.player_skill(&impact.plan_id).is_some()
            || if impact.contact_status.is_some() || impact.field().is_some() {
                impact.damage_bps != 0 || basic.effects.is_some()
            } else {
                !(1..=100000).contains(&impact.damage_bps)
            }
            || impact.contact_status.as_ref().is_some_and(|contact| {
                !matches!(impact.timing, BasicImpactTiming::FixedDelay { .. })
                    || status_definition(c, &contact.status_id).is_none_or(|d| {
                        !d.is_harmful() || d.is_heal() || d.periodic_stack.is_none()
                    })
                    || contact.death_burst.as_ref().is_some_and(|burst| {
                        !valid_hitscan_followup(burst.radius, burst.damage_bps_per_stack)
                            || !(1..=10000).contains(&burst.inherited_stack_bps)
                            || !(1..=32).contains(&burst.target_cap)
                            || !c
                                .combat_rules
                                .as_ref()
                                .unwrap()
                                .periodic_damage
                                .contains_key(&contact.status_id)
                    })
            })
            || match &impact.timing {
                BasicImpactTiming::Hitscan {
                    refraction,
                    followup,
                } => {
                    refraction.as_ref().is_some_and(|r| {
                        followup.is_some() || !valid_hitscan_followup(r.radius, r.damage_bps)
                    }) || followup.as_ref().is_some_and(|plan| !match plan {
                        BasicHitscanFollowup::AlternatingRefraction {
                            radius,
                            damage_bps,
                            max_contacts,
                            repeat_decay_bps,
                            minimum_damage_bps,
                        } => {
                            valid_hitscan_followup(*radius, *damage_bps)
                                && (2..=8).contains(max_contacts)
                                && (1..=10000).contains(repeat_decay_bps)
                                && (1..=impact.damage_bps.min(*damage_bps))
                                    .contains(minimum_damage_bps)
                        }
                        BasicHitscanFollowup::TargetBurst {
                            radius,
                            damage_bps,
                            max_targets,
                        } => {
                            valid_hitscan_followup(*radius, *damage_bps)
                                && (2..=8).contains(max_targets)
                        }
                    })
                }
                BasicImpactTiming::FixedDelay { delay_ms } => !(1..=60000).contains(delay_ms),
                BasicImpactTiming::GroundField { delay_ms, field } => {
                    !(1..=60000).contains(delay_ms)
                        || field.plan_id == impact.plan_id
                        || !matches!(field.trigger, BasicFieldTrigger::Ground {})
                        || !valid_basic_field(c, field)
                }
            })
    {
        return Err("CONFIG_BASIC_IMPACT_BOUNDS");
    }
    if let Some(beam) = &basic.beam
        && (!basic.settle_on_end
            || basic.non_projectile()
            || basic.piercing.is_some()
            || !valid_id(&beam.plan_id)
            || beam.plan_id.len() > 64
            || c.player_skill(&beam.plan_id).is_some()
            || c.step_ms == 0
            || !(c.step_ms..=60000).contains(&beam.interval_ms)
            || !beam.interval_ms.is_multiple_of(c.step_ms)
            || !(0.1..=128.).contains(&beam.half_width)
            || !(1..=8).contains(&beam.target_cap)
            || !(1..=100000).contains(&beam.damage_bps))
    {
        return Err("CONFIG_BASIC_BEAM_BOUNDS");
    }
    if let Some(plan) = &basic.projectile_followup
        && (!basic.settle_on_end
            || basic.piercing.is_some()
            || basic.non_projectile()
            || basic.beam.is_some()
            || !valid_id(&plan.plan_id)
            || plan.plan_id.len() > 64
            || c.player_skill(&plan.plan_id).is_some()
            || !(1..=100000).contains(&plan.primary_damage_bps)
            || !(c.step_ms.saturating_mul(2)..=60000).contains(&c.params.player_projectile_ttl_ms)
            || match &plan.pattern {
                BasicProjectilePattern::Companion {
                    delay_ms,
                    damage_bps,
                } => !(1..=10000).contains(delay_ms) || !(1..=100000).contains(damage_bps),
                BasicProjectilePattern::Volley {
                    hits,
                    interval_ms,
                    damage_bps,
                    attack_speed_bps_per_hit,
                } => {
                    !(2..=8).contains(hits)
                        || !(1..=10000).contains(interval_ms)
                        || !(1..=100000).contains(damage_bps)
                        || !(1..=10000 / (*hits).max(1)).contains(attack_speed_bps_per_hit)
                }
                BasicProjectilePattern::SealRecall(rule) => !valid_recall(c, basic, rule),
                BasicProjectilePattern::Ricochet(rule) => !valid_ricochet(c, basic, rule),
            })
    {
        return Err("CONFIG_BASIC_PROJECTILE_FOLLOWUP");
    }
    if let Some(effects) = &basic.effects {
        let rules = c.combat_rules.as_ref().unwrap();
        let definition = |id: &str| {
            c.pve
                .as_ref()?
                .status_definitions
                .iter()
                .find(|d| d.id == id)
        };
        if !basic.settle_on_end
            || (effects.on_release_self_status.is_none()
                && effects.on_end_damage_self_status.is_none()
                && effects.on_end_damage_target_status.is_none()
                && effects.on_end_damage_self_heal_max_hp_bps.is_none())
            || effects
                .on_end_damage_self_heal_max_hp_bps
                .is_some_and(|bps| !(1..=10000).contains(&bps))
            || effects
                .on_release_self_status
                .iter()
                .chain(&effects.on_end_damage_self_status)
                .any(|id| {
                    definition(id)
                        .is_none_or(|d| d.is_harmful() || d.is_hostile_control() || d.is_periodic())
                })
        {
            return Err("CONFIG_BASIC_SELF_STATUS");
        }
        if let Some(effect) = &effects.on_end_damage_target_status
            && (effect.choices.is_empty()
                || effect.choices.len() > 8
                || effect.choices.iter().collect::<BTreeSet<_>>().len() != effect.choices.len()
                || effect.choices.iter().any(|id| {
                    definition(id).is_none_or(|d| {
                        !matches!(d.mechanism, StatusMechanism::TimedAttributeModifier)
                            || !matches!(
                                d.attribute,
                                Some(
                                    StatusAttribute::Atk
                                        | StatusAttribute::Def
                                        | StatusAttribute::AttackSpeed
                                        | StatusAttribute::MovementSlow
                                )
                            )
                            || d.modifier_bps >= 0
                            || !d.is_harmful()
                    })
                })
                || effect.target_profile_bps.iter().any(|(id, bps)| {
                    !rules.profiles.contains_key(id) || !(1..=10000).contains(bps)
                }))
        {
            return Err("CONFIG_BASIC_TARGET_STATUS");
        }
    }
    Ok(())
}

fn invalid_wave(c: &Config, range: f64, wave: &BasicWaveConfig) -> bool {
    !valid_id(&wave.plan_id)
        || wave.plan_id.len() > 64
        || c.player_skill(&wave.plan_id).is_some()
        || !(0.1..=10000.).contains(&wave.speed)
        || range * 1000. / wave.speed > 60000.
        || !(1..=16).contains(&wave.target_cap)
        || [wave.primary_damage_bps, wave.additional_damage_bps]
            .iter()
            .any(|n| !(1..=100000).contains(n))
        || matches!(wave.geometry, BasicWaveGeometryConfig::Directional { cone_degrees }
                if !(0.1..=180.).contains(&cone_degrees))
        || wave
            .behavior
            .as_ref()
            .is_some_and(|behavior| !valid_wave_behavior(c, wave, behavior))
        || wave
            .field
            .as_ref()
            .is_some_and(|field| !valid_wave_field(c, wave, field))
}

fn valid_charge(c: &Config, basic: &BasicConfig, charge: &BasicChargeConfig) -> bool {
    if c.step_ms == 0
        || !(c.step_ms..=30000).contains(&charge.max_ms)
        || basic.beam.is_some()
        || basic.projectile_followup.is_some()
        || basic.stacked_wave.is_some()
    {
        return false;
    }
    if let BasicChargeBehavior::Flight(flight) = &charge.behavior {
        return valid_charge_flight(c, basic, charge, flight);
    }
    let Some(base_wave) = &basic.wave else {
        return false;
    };
    if base_wave.behavior.is_some() || base_wave.field.is_some() {
        return false;
    }
    match &charge.behavior {
        BasicChargeBehavior::StagedWave { upgrades } => {
            let mut plans = BTreeSet::from([&base_wave.plan_id]);
            (1..=2).contains(&upgrades.len())
                && upgrades
                    .windows(2)
                    .all(|pair| pair[0].after_ms < pair[1].after_ms)
                && upgrades.iter().all(|stage| {
                    (1..charge.max_ms).contains(&stage.after_ms)
                        && (0.1..=10000.).contains(&stage.range)
                        && plans.insert(&stage.wave.plan_id)
                        && stage.wave.field.is_none()
                        && !matches!(
                            stage.wave.behavior,
                            Some(BasicWaveBehavior::Alternate { .. })
                        )
                        && !invalid_wave(c, stage.range, &stage.wave)
                        && valid_basic_contacts(c, stage.wave.target_cap)
                })
        }
        BasicChargeBehavior::Guard {
            reduction_bps,
            reflect_bps,
            reflect_damage_kind,
        } => {
            c.pve.as_ref().is_some_and(|pve| pve.status_rules.is_some())
                && (1..=10000).contains(reduction_bps)
                && (1..=10000).contains(reflect_bps)
                && *reflect_damage_kind != DamageKind::Elemental
        }
        BasicChargeBehavior::Dash(dash) => {
            (1..=60000).contains(&dash.ready_interval_ms)
                && dash.tap_max_ms < charge.max_ms
                && (0.1..=1000.).contains(&dash.min_distance)
                && (dash.min_distance..=1000.).contains(&dash.max_distance)
                && (0.1..=128.).contains(&dash.half_width)
                && (1..=16).contains(&dash.target_cap)
                && valid_basic_contacts(c, dash.target_cap)
                && [dash.primary_damage_bps, dash.additional_damage_bps]
                    .iter()
                    .all(|bps| (1..=100000).contains(bps))
                && (0.1..=500.).contains(&dash.knockback_distance)
                && valid_profiles(c, &dash.stun_resistant_profiles)
                && status_definition(c, &dash.stun_status).is_some_and(|d| {
                    d.attribute == Some(StatusAttribute::Stun)
                        && d.is_hostile_control()
                        && d.is_harmful()
                })
        }
        BasicChargeBehavior::Flight(_) => unreachable!(),
    }
}

fn valid_basic_contacts(c: &Config, cap: usize) -> bool {
    let members = c.members.as_ref().map_or(0, |m| m.capacity);
    (64 + 12 * cap) * members <= c.output_capacity
}

fn valid_charge_flight(
    c: &Config,
    basic: &BasicConfig,
    charge: &BasicChargeConfig,
    flight: &BasicChargeFlight,
) -> bool {
    let stack_cap = status_definition(c, &flight.status_id)
        .filter(|d| d.is_periodic() && !d.is_heal() && d.periodic_stack.is_some())
        .and_then(|d| d.max_stacks)
        .unwrap_or(0);
    basic.settle_on_end
        && basic.wave.is_none()
        && basic.impact.is_none()
        && basic.piercing.is_none()
        && (0.1..=10000.).contains(&basic.range)
        && (0.1..=10000.).contains(&basic.projectile_speed)
        && basic.range * 1000. / basic.projectile_speed + 2. * (c.step_ms as f64) < 60000.
        && valid_id(&flight.plan_id)
        && flight.plan_id.len() <= 64
        && c.player_skill(&flight.plan_id).is_none()
        && flight.damage_rule.contact_status.is_none()
        && (1..=16).contains(&flight.target_cap)
        && valid_basic_contacts(c, 2 * flight.target_cap)
        && (c.step_ms..=60000).contains(&flight.contact_interval_ms)
        && (1..=stack_cap).contains(&flight.contact_stacks)
        && (1..=3).contains(&flight.stages.len())
        && flight.stages[0].after_ms == 0
        && flight
            .stages
            .windows(2)
            .all(|w| w[0].after_ms < w[1].after_ms)
        && flight.stages.iter().all(|s| {
            s.after_ms < charge.max_ms
                && (0.1..=128.).contains(&s.radius)
                && (0.1..=512.).contains(&s.burst_radius)
                && (1..=16).contains(&s.max_contacts_per_target)
                && (1..=stack_cap).contains(&s.burst_stacks)
                && [s.contact_damage_bps, s.burst_damage_bps]
                    .iter()
                    .all(|n| (1..=100000).contains(n))
        })
}

fn valid_stacked_wave(c: &Config, basic: &BasicConfig, upgrade: &BasicStackedWave) -> bool {
    let Some(base) = &basic.wave else {
        return false;
    };
    let radial = |wave: &BasicWaveConfig| {
        wave.behavior.is_none()
            && wave.field.is_none()
            && matches!(wave.geometry, BasicWaveGeometryConfig::Radial {})
    };
    basic.charge.is_none()
        && radial(base)
        && radial(&upgrade.finisher)
        && (0.1..=10000.).contains(&upgrade.range)
        && base.plan_id != upgrade.finisher.plan_id
        && !invalid_wave(c, upgrade.range, &upgrade.finisher)
        && valid_basic_contacts(c, upgrade.finisher.target_cap)
        && status_definition(c, &upgrade.stack_status).is_some_and(|d| {
            d.source_scoped
                && !d.is_harmful()
                && matches!(d.mechanism, StatusMechanism::TimedCompoundModifier)
                && matches!(d.duplicate_policy, DuplicatePolicy::Stack)
                && d.max_stacks.is_some_and(|cap| (1..=16).contains(&cap))
        })
}

fn valid_profiles(c: &Config, ids: &[String]) -> bool {
    let profiles = &c.combat_rules.as_ref().unwrap().profiles;
    ids.len() <= profiles.len()
        && ids.iter().all(|id| profiles.contains_key(id))
        && ids.iter().collect::<BTreeSet<_>>().len() == ids.len()
}

fn status_definition<'a>(c: &'a Config, id: &str) -> Option<&'a StatusDefinition> {
    c.pve
        .as_ref()?
        .status_definitions
        .iter()
        .find(|d| d.id == id)
}

fn valid_basic_field(c: &Config, field: &BasicWaveField) -> bool {
    valid_id(&field.plan_id)
        && field.plan_id.len() <= 64
        && c.player_skill(&field.plan_id).is_none()
        && (0.1..=512.).contains(&field.radius)
        && (1..=60000).contains(&field.duration_ms)
        && c.step_ms > 0
        && (c.step_ms..=field.duration_ms).contains(&field.interval_ms)
        && field.duration_ms.is_multiple_of(field.interval_ms)
        && field.duration_ms / field.interval_ms <= 128
        && (1..=100000).contains(&field.damage_bps)
        && (1..=16).contains(&field.target_cap)
        && (1..=12).contains(&field.max_per_source)
        && field.owner_reduction_bps <= 10000
        && (field.owner_reduction_bps == 0 || field.max_per_source == 1)
        && field.damage_rule.as_ref().is_none_or(|rule| {
            rule.contact_status.is_none()
                && rule.life_steal_bps == 0
                && rule.life_steal_timing != super::combat::LifeStealTiming::ActionEnd
        })
        && field.on_tick_statuses.len() <= 4
        && field.on_tick_statuses.iter().collect::<BTreeSet<_>>().len()
            == field.on_tick_statuses.len()
        && field
            .on_tick_statuses
            .iter()
            .all(|id| status_definition(c, id).is_some_and(StatusDefinition::is_harmful))
}

fn valid_wave_field(c: &Config, wave: &BasicWaveConfig, field: &BasicWaveField) -> bool {
    wave.behavior.is_none()
        && field.plan_id != wave.plan_id
        && valid_basic_field(c, field)
        && match field.trigger {
            BasicFieldTrigger::Ground {} => false,
            BasicFieldTrigger::PrimaryContact {} => true,
            BasicFieldTrigger::Landing { impact_radius } => {
                matches!(wave.geometry, BasicWaveGeometryConfig::Radial {})
                    && (0.1..=512.).contains(&impact_radius)
            }
        }
}
fn valid_wave_behavior(c: &Config, wave: &BasicWaveConfig, behavior: &BasicWaveBehavior) -> bool {
    let directional = matches!(wave.geometry, BasicWaveGeometryConfig::Directional { .. });
    let valid_push = |distance: f64, profiles: &[String], status: &str| {
        (0.1..=500.).contains(&distance)
            && valid_profiles(c, profiles)
            && status_definition(c, status).is_some_and(|d| {
                matches!(d.mechanism, StatusMechanism::TimedAttributeModifier)
                    && d.attribute == Some(StatusAttribute::MovementSlow)
                    && d.modifier_bps < 0
                    && matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                    && d.source_scoped
            })
    };
    match behavior {
        BasicWaveBehavior::RadialPush {
            distance,
            resistant_profiles,
            reduced_status,
        } => !directional && valid_push(*distance, resistant_profiles, reduced_status),
        BasicWaveBehavior::Push {
            distance,
            rehit_immunity_ms,
            resistant_profiles,
            reduced_status,
        } => {
            directional
                && (1..=60000).contains(rehit_immunity_ms)
                && valid_push(*distance, resistant_profiles, reduced_status)
        }
        BasicWaveBehavior::Alternate {
            pull_distance,
            resistant_profiles,
            mark_duration_ms,
            marked_damage_bonus_bps,
            alternate,
        } => {
            directional
                && (0.1..=500.).contains(pull_distance)
                && valid_profiles(c, resistant_profiles)
                && (1..=60000).contains(mark_duration_ms)
                && (1..=10000).contains(marked_damage_bonus_bps)
                && valid_id(&alternate.plan_id)
                && alternate.plan_id.len() <= 64
                && alternate.plan_id != wave.plan_id
                && c.player_skill(&alternate.plan_id).is_none()
                && (0.1..=10000.).contains(&alternate.range)
                && (0.1..=10000.).contains(&alternate.speed)
                && alternate.range * 1000. / alternate.speed <= 60000.
                && [
                    alternate.primary_damage_bps,
                    alternate.additional_damage_bps,
                ]
                .iter()
                .all(|n| (1..=100000).contains(n))
        }
    }
}

fn valid_recall(c: &Config, basic: &BasicConfig, rule: &BasicProjectileRecall) -> bool {
    let Some(pve) = &c.pve else {
        return false;
    };
    (2..=8).contains(&rule.required_seals)
        && (0.1..=500.).contains(&rule.overtravel)
        && (0.1..=128.).contains(&rule.corridor_half_width)
        && [rule.additional_damage_bps, rule.recall_damage_bps]
            .into_iter()
            .all(|bps| (1..=100000).contains(&bps))
        && (1..=10000).contains(&rule.settle_delay_ms)
        && (1..=10000).contains(&rule.interval_ms)
        && valid_path_time(c, pve, basic.projectile_speed)
        && pve.status_definitions.iter().any(|d| {
            d.id == rule.slow_status
                && d.attribute == Some(StatusAttribute::MovementSlow)
                && d.modifier_bps < 0
                && matches!(d.mechanism, StatusMechanism::TimedAttributeModifier)
                && matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                && d.source_scoped
        })
}

fn valid_ricochet(c: &Config, basic: &BasicConfig, rule: &BasicProjectileRicochet) -> bool {
    let Some(pve) = &c.pve else {
        return false;
    };
    let rules = c.combat_rules.as_ref().unwrap();
    (2..=8).contains(&rule.max_targets)
        && (0.1..=10000.).contains(&rule.search_radius)
        && (1..=100000).contains(&rule.damage_bps)
        && (1..=60000).contains(&rule.mark_duration_ms)
        && (2..=8).contains(&rule.hits_to_stun)
        && valid_path_time(c, pve, basic.projectile_speed)
        && rule.resistant_profiles.len() <= rules.profiles.len()
        && rule
            .resistant_profiles
            .iter()
            .all(|id| rules.profiles.contains_key(id))
        && rule
            .resistant_profiles
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            == rule.resistant_profiles.len()
        && pve.status_definitions.iter().any(|d| {
            d.id == rule.stun_status
                && (d.attribute == Some(StatusAttribute::Stun)
                    || d.flags.contains(&StatusAttribute::Stun))
        })
        && rule.return_to_owner.as_ref().is_none_or(|returning| {
            (1000..=40000).contains(&returning.speed_bps)
                && valid_path_time(
                    c,
                    pve,
                    basic.projectile_speed * f64::from(returning.speed_bps) / 10000.,
                )
                && (1..=10000).contains(&returning.max_hp_bps)
                && (1..=returning.max_hp_bps).contains(&returning.base_max_hp_bps)
                && returning.per_extra_max_hp_bps <= returning.max_hp_bps
                && pve.status_definitions.iter().any(|d| {
                    d.id == returning.shield_status
                        && d.is_shield()
                        && matches!(d.duplicate_policy, DuplicatePolicy::Refresh)
                        && d.source_scoped
                })
        })
}

fn valid_path_time(c: &Config, pve: &PveConfig, speed: f64) -> bool {
    let diagonal = if pve.isometric_grid.is_some() {
        pve.map_diagonal()
    } else {
        (pve.width as f64).hypot(pve.height as f64) * pve.cell_size
    };
    diagonal * 1000. / speed + 2. * c.step_ms as f64 <= 60000.
}

fn valid_hitscan_followup(radius: f64, damage_bps: u32) -> bool {
    (0.1..=10000.).contains(&radius) && (1..=100000).contains(&damage_bps)
}
