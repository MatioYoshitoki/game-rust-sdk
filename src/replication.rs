//! Pure recipient-view differencing and client decoding. This module owns no World.
use crate::protocol::*;
#[path = "replication_status.rs"]
mod status;
fn state(s: &SnapshotV4) -> SnapshotStateV6 {
    SnapshotStateV6 {
        contested_equipment: s.contested_equipment.clone(),
        extraction_pressure: s.extraction_pressure.clone(),
        encounter_progress: s.encounter_progress.clone(),
        phase: s.phase,
        active_at_ms: s.active_at_ms,
        extraction_point: s.extraction_point,
        extraction_open: s.extraction_open,
    }
}
pub(crate) fn changed<T: Clone + PartialEq, K: Clone + PartialEq>(
    a: &[T],
    b: &[T],
    key: impl Fn(&T) -> K,
) -> Option<ListDelta<T, K>> {
    if a == b {
        return None;
    }
    let ak: Vec<_> = a.iter().map(&key).collect();
    let bk: Vec<_> = b.iter().map(&key).collect();
    Some(ListDelta {
        upsert: b
            .iter()
            .zip(&bk)
            .filter(|(v, k)| {
                ak.iter()
                    .position(|old| old == *k)
                    .is_none_or(|i| a[i] != **v)
            })
            .map(|(v, _)| v.clone())
            .collect(),
        removed: ak.iter().filter(|k| !bk.contains(k)).cloned().collect(),
        order: bk,
    })
}
pub(crate) fn diff(a: &SnapshotV4, b: &SnapshotV4) -> SnapshotDeltaV6 {
    SnapshotDeltaV6 {
        relics: (a.relics != b.relics).then(|| b.relics.clone()).flatten(),
        containers: (a.containers != b.containers)
            .then(|| b.containers.clone())
            .flatten(),
        active_blockers: (a.active_blockers != b.active_blockers)
            .then(|| b.active_blockers.clone())
            .flatten(),
        checkpoint_zones: (a.checkpoint_zones != b.checkpoint_zones)
            .then(|| b.checkpoint_zones.clone())
            .flatten(),
        basic_beams: (a.basic_beams != b.basic_beams)
            .then(|| b.basic_beams.clone())
            .flatten(),
        basic_impacts: (a.basic_impacts != b.basic_impacts)
            .then(|| b.basic_impacts.clone())
            .flatten(),
        basic_waves: (a.basic_waves != b.basic_waves)
            .then(|| b.basic_waves.clone())
            .flatten(),
        areas: (a.areas != b.areas).then(|| b.areas.clone()).flatten(),
        logical_ms: b.logical_ms,
        state: (state(a) != state(b)).then(|| state(b)),
        own: (a.own != b.own).then(|| b.own.clone()),
        members: changed(&a.members, &b.members, |v| v.entity.clone()),
        monsters: changed(&a.monsters, &b.monsters, |v| v.entity.clone()),
        resources: changed(&a.resources, &b.resources, |v| v.id.clone()),
        projectiles: changed(&a.projectiles, &b.projectiles, |v| v.id.clone()),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    WrongRecipient,
    StaleFrame,
    BaselineMissing,
    InvalidFrame,
    Conflict,
}
impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for DecodeError {}
fn unique<K: PartialEq>(v: &[K]) -> bool {
    v.iter().enumerate().all(|(i, k)| !v[..i].contains(k))
}
fn apply_list<T: Clone, K: Clone + PartialEq>(
    old: &[T],
    d: &ListDelta<T, K>,
    key: impl Fn(&T) -> K,
) -> std::result::Result<Vec<T>, DecodeError> {
    let oldkeys: Vec<_> = old.iter().map(&key).collect();
    let upkeys: Vec<_> = d.upsert.iter().map(&key).collect();
    if !unique(&oldkeys)
        || !unique(&upkeys)
        || !unique(&d.removed)
        || !unique(&d.order)
        || d.removed
            .iter()
            .any(|k| !oldkeys.contains(k) || upkeys.contains(k))
    {
        return Err(DecodeError::InvalidFrame);
    }
    let mut values: Vec<_> = old
        .iter()
        .filter(|v| !d.removed.contains(&key(v)))
        .cloned()
        .collect();
    for v in &d.upsert {
        if let Some(i) = values.iter().position(|x| key(x) == key(v)) {
            values[i] = v.clone();
        } else {
            values.push(v.clone());
        }
    }
    if values.len() != d.order.len() {
        return Err(DecodeError::InvalidFrame);
    }
    d.order
        .iter()
        .map(|k| {
            values
                .iter()
                .find(|v| key(v) == *k)
                .cloned()
                .ok_or(DecodeError::InvalidFrame)
        })
        .collect()
}
fn valid_reservation_transition(before: &PrivateMemberView, after: &PrivateMemberView) -> bool {
    if before.participant.entity != after.participant.entity {
        return true;
    }
    let (Some(a), Some(b)) = (&before.pending_action, &after.pending_action) else {
        return true;
    };
    if a.action_id.is_some() && b.action_id.is_none() && a.skill_id == b.skill_id {
        return false;
    }
    a.action_id.is_none()
        || a.action_id != b.action_id
        || a.skill_id == b.skill_id && a.resource_reservation == b.resource_reservation
}
fn valid_channel_transition(before: &PrivateMemberView, after: &PrivateMemberView) -> bool {
    if before.participant.entity != after.participant.entity {
        return true;
    }
    let (Some(a), Some(b)) = (&before.active_channel, &after.active_channel) else {
        return true;
    };
    a.action_id != b.action_id
        || a.target.is_none() && b.target.is_none()
        || a.target == b.target
            && a.skill_id == b.skill_id
            && a.started_at_ms == b.started_at_ms
            && a.ends_at_ms == b.ends_at_ms
            && a.total_pulses == b.total_pulses
            && a.recovery == b.recovery
            && a.direction == b.direction
            && b.pulses >= a.pulses
}
fn valid_loadout_transition(before: &PrivateMemberView, after: &PrivateMemberView) -> bool {
    if before.participant.entity != after.participant.entity {
        return true;
    }
    match (before.skill_loadout_revision, after.skill_loadout_revision) {
        (None, None) => true,
        (Some(_), None) => false,
        (a, Some(b)) => {
            b >= a.unwrap_or(0)
                && (b != a.unwrap_or(0)
                    || before
                        .player_skills
                        .iter()
                        .map(|s| &s.skill_id)
                        .eq(after.player_skills.iter().map(|s| &s.skill_id)))
        }
    }
}
fn valid_equipment_transition(before: &PrivateMemberView, after: &PrivateMemberView) -> bool {
    match (&before.equipment, &after.equipment) {
        (None, None) => before.basic_profile == after.basic_profile,
        (Some(_), None) => false,
        (None, Some(next)) => next.revision > 0,
        (Some(previous), Some(next)) => {
            next.revision > previous.revision
                || previous == next && before.basic_profile == after.basic_profile
        }
    }
}

fn valid_shield_view(
    entries: &[crate::protocol::StatusView],
    total: Option<u32>,
    _max_hp: u32,
    exact: bool,
) -> bool {
    let mut sum = 0u64;
    for s in entries {
        if !status::valid_status(s) {
            return false;
        }
        if let Some(shield) = &s.shield {
            if shield.remaining > shield.granted || s.expires_at_ms < s.applied_at_ms {
                return false;
            }
            sum += u64::from(shield.remaining);
        }
    }
    match total {
        Some(t) => {
            if exact {
                sum == u64::from(t)
            } else {
                sum <= u64::from(t)
            }
        }
        None => entries.iter().all(|s| s.shield.is_none()),
    }
}
fn valid_unit(t: &UnitTarget, s: &SnapshotV4) -> bool {
    valid_id(&t.id)
        && t.generation > 0
        && s.monsters.iter().any(|n| {
            n.summon.is_none() && n.entity.id == t.id && n.entity.generation == t.generation
        })
}
fn member_combat(s: &SnapshotV4) -> bool {
    s.member_conflict
        || s.own
            .mode
            .as_ref()
            .is_some_and(|m| m.objective == ModeObjective::Contest)
}
fn valid_hostile_unit(t: &UnitTarget, source: &EntityRef, s: &SnapshotV4) -> bool {
    valid_unit(t, s)
        && s.monsters.iter().any(|m| {
            m.entity.id == t.id
                && m.entity.generation == t.generation
                && m.training_owner_id
                    .as_ref()
                    .is_none_or(|owner| *owner == source.id)
        })
        || member_combat(s)
            && s.members.iter().any(|m| {
                m.entity != *source && m.entity.id == t.id && m.entity.generation == t.generation
            })
}

fn valid_mode(s: &SnapshotV4) -> bool {
    let Some(mode) = &s.own.mode else { return true };
    !s.member_conflict
        && valid_id(&mode.stage_id)
        && mode.stage_id.len() <= 64
        && (1..=crate::config::match_mode::MAX_STAGES as u8).contains(&mode.stage_index)
        && s.active_at_ms
            .is_some_and(|active| active <= mode.opens_at_ms)
        && mode.opens_at_ms <= s.logical_ms
        && mode.opens_at_ms < mode.expires_at_ms
        && mode.expires_at_ms < (1u64 << 53)
        && (mode.wave.is_some() == (mode.objective == ModeObjective::Waves))
        && (mode.progress.is_some() == (mode.objective == ModeObjective::KillQuota))
        && mode
            .progress
            .is_none_or(|p| p.required > 0 && p.current <= p.required)
        && mode.wave.as_ref().is_none_or(|wave| {
            (1..=crate::config::match_mode::MAX_WAVES as u8).contains(&wave.total)
                && (1..=wave.total).contains(&wave.index)
                && (mode.opens_at_ms..=mode.expires_at_ms).contains(&wave.opens_at_ms)
                && (!wave.opened
                    || (wave.opens_at_ms <= s.logical_ms && wave.opens_at_ms < mode.expires_at_ms))
        })
        && (!mode.submitted || mode.objective == ModeObjective::Checkpoint)
        && mode.respawns_left <= crate::config::match_mode::MAX_RESPAWNS
        && !s.extraction_open
        && mode.respawn_at_ms.is_none_or(|at| {
            s.own.hp == 0 && mode.outcome.is_none() && s.logical_ms < at && at < (1u64 << 53)
        })
        && mode.forfeit_at_ms.is_none_or(|at| {
            !s.own.ready && mode.outcome.is_none() && s.logical_ms < at && at < (1u64 << 53)
        })
        && match mode.outcome {
            None => {
                s.own.phase == MemberPhase::Active
                    && s.logical_ms < mode.expires_at_ms
                    && (s.own.hp > 0 || mode.respawn_at_ms.is_some())
            }
            Some(Outcome::Extraction) => false,
            Some(_) => matches!(s.own.phase, MemberPhase::Settling | MemberPhase::Terminal),
        }
}

fn valid_training(s: &SnapshotV4) -> bool {
    const MAX: u64 = (1 << 53) - 1;
    let Some(t) = &s.own.training else {
        return true;
    };
    !s.member_conflict
        && s.own.mode.is_none()
        && !s.extraction_open
        && (t.respawn_at_ms.is_some() == (s.own.phase == MemberPhase::Active && s.own.hp == 0))
        && t.respawn_at_ms
            .is_none_or(|at| s.logical_ms < at && at <= MAX)
        && t.metrics.as_ref().is_none_or(|m| {
            let b = &m.by_source;
            m.sampled_at_ms <= s.logical_ms
                && [m.damage, m.rolling_dps, m.average_dps]
                    .into_iter()
                    .all(|v| v <= MAX)
                && m.critical_hits <= m.hits
                && m.hits <= m.damage
                && u64::from(m.highest_hit) <= m.damage
                && ((m.damage == 0) == (m.highest_hit == 0))
                && ((m.damage == 0) == (m.hits == 0))
                && (m.damage > 0 || m.rolling_dps == 0 && m.average_dps == 0)
                && [b.basic, b.skill, b.periodic, b.summon, b.other]
                    .into_iter()
                    .map(u128::from)
                    .sum::<u128>()
                    == u128::from(m.damage)
        })
}

fn valid_training_transition(before: &PrivateMemberView, after: &PrivateMemberView) -> bool {
    match (&before.training, &after.training) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            before.participant.entity.generation <= after.participant.entity.generation
                && match (&a.metrics, &b.metrics) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.sampled_at_ms <= b.sampled_at_ms,
                    _ => false,
                }
        }
        _ => false,
    }
}

fn valid_mode_transition(previous: &SnapshotV4, next: &SnapshotV4) -> bool {
    let Some(a) = &previous.own.mode else {
        return next.own.mode.is_none() || previous.active_at_ms.is_none();
    };
    let Some(b) = &next.own.mode else {
        return false;
    };
    let prior_entity = &previous.own.participant.entity;
    let entity = &next.own.participant.entity;
    let spent = a.respawns_left.checked_sub(b.respawns_left);
    entity
        .generation
        .checked_sub(prior_entity.generation)
        .zip(spent)
        .is_some_and(|(generations, spent)| {
            generations <= u32::from(spent) + u32::from(a.respawn_at_ms.is_some())
        })
        && b.stage_index >= a.stage_index
        && (a.outcome.is_none() || a == b && prior_entity == entity)
        && if a.stage_index == b.stage_index {
            a.stage_id == b.stage_id
                && a.objective == b.objective
                && a.opens_at_ms == b.opens_at_ms
                && a.expires_at_ms == b.expires_at_ms
                && (!a.submitted || b.submitted)
                && match (a.progress, b.progress) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.required == b.required && a.current <= b.current,
                    _ => false,
                }
                && match (&a.wave, &b.wave) {
                    (None, None) => true,
                    (Some(a), Some(b)) => {
                        a.total == b.total
                            && b.index >= a.index
                            && if a.index == b.index {
                                a.opens_at_ms == b.opens_at_ms && (!a.opened || b.opened)
                            } else {
                                b.opens_at_ms >= a.opens_at_ms
                            }
                    }
                    _ => false,
                }
        } else {
            b.opens_at_ms >= a.opens_at_ms
        }
}
fn valid_beam(beam: &BasicBeamView, s: &SnapshotV4) -> bool {
    valid_id(&beam.action_id)
        && valid_id(&beam.plan_id)
        && beam.plan_id.len() <= 64
        && beam.source.kind == StatusEntityKind::Participant
        && s.members.iter().any(|m| {
            m.entity == beam.source && m.phase == MemberPhase::Active && m.hp > 0 && !m.protected
        })
        && !s
            .basic_waves
            .iter()
            .flatten()
            .any(|w| w.source == beam.source)
        && !s
            .basic_impacts
            .iter()
            .flatten()
            .any(|p| p.source == beam.source)
        && beam.direction.iter().all(|v| v.is_finite())
        && (beam.direction[0].hypot(beam.direction[1]) - 1.).abs() <= 1e-9
        && (0.1..=10000.).contains(&beam.range)
        && (0.1..=128.).contains(&beam.half_width)
        && (1..=60000).contains(&beam.interval_ms)
        && beam.started_at_ms <= s.logical_ms
        && beam.next_tick_at_ms > s.logical_ms
        && beam.next_tick_at_ms < (1u64 << 53)
        && beam.next_tick_at_ms - beam.started_at_ms
            == (u64::from(beam.ticks) + 1) * beam.interval_ms
        && (!beam.released || beam.ticks == 0)
        && (beam.source != s.own.participant.entity
            || (s.own.ready
                && !s.own.protected
                && s.own.pending_action.is_none()
                && s.own.active_channel.is_none()
                && s.own.basic_pursuit.is_none()
                && s.own.extraction_finishes_ms.is_none()))
}

fn valid_impact(plan: &BasicImpactView, s: &SnapshotV4) -> bool {
    valid_id(&plan.id)
        && valid_id(&plan.plan_id)
        && plan.plan_id.len() <= 64
        && plan.source.kind == StatusEntityKind::Participant
        && s.members
            .iter()
            .any(|m| m.entity == plan.source && m.phase == MemberPhase::Active && m.hp > 0)
        && !s
            .basic_waves
            .iter()
            .flatten()
            .any(|wave| wave.source == plan.source)
        && plan.origin.iter().all(|v| v.is_finite())
        && plan.released_at_ms <= s.logical_ms
        && plan.impact_at_ms > s.logical_ms
        && plan.impact_at_ms < (1u64 << 53)
        && plan.impact_at_ms - plan.released_at_ms <= 60000
        && match (plan.kind, &plan.target, plan.position) {
            (BasicImpactKind::FixedDelay, Some(target), None) => {
                valid_id(&target.id)
                    && target.generation > 0
                    && s.monsters.iter().any(|m| {
                        m.entity.id == target.id && m.entity.generation >= target.generation
                    })
            }
            (BasicImpactKind::GroundField, None, Some(position)) => {
                position.iter().all(|v| v.is_finite())
            }
            _ => false,
        }
}

fn valid_wave(wave: &BasicWaveView, s: &SnapshotV4) -> bool {
    if !valid_id(&wave.id)
        || !valid_id(&wave.plan_id)
        || wave.plan_id.len() > 64
        || wave.source.kind != StatusEntityKind::Participant
        || !s
            .members
            .iter()
            .any(|m| m.entity == wave.source && m.phase == MemberPhase::Active && m.hp > 0)
        || !wave.origin.iter().all(|v| v.is_finite())
        || !(0.1..=10000.).contains(&wave.range)
        || !(0.1..=10000.).contains(&wave.speed)
        || wave.released_at_ms > s.logical_ms
        || !(1..=16).contains(&wave.pending.len())
    {
        return false;
    }
    if let BasicWaveGeometry::Directional {
        direction,
        cone_degrees,
    } = wave.geometry
        && (!direction.iter().all(|v| v.is_finite())
            || (direction[0].hypot(direction[1]) - 1.).abs() > 1e-9
            || !(0.1..=180.).contains(&cone_degrees))
    {
        return false;
    }
    let landing = if let BasicWaveGeometry::Landing { from } = wave.geometry {
        let length = (from[0] - wave.origin[0]).hypot(from[1] - wave.origin[1]);
        if !from.iter().all(|v| v.is_finite()) || length > 10000. || wave.range > 512. {
            return false;
        }
        Some(length)
    } else {
        None
    };
    let travel = landing.unwrap_or(wave.range) * 1000. / wave.speed;
    let last = (travel.ceil() as u64).max(1);
    travel <= 60000.
        && unique(
            &wave
                .pending
                .iter()
                .map(|p| &p.target.id)
                .collect::<Vec<_>>(),
        )
        && wave
            .pending
            .windows(2)
            .all(|pair| pair[0].at_ms <= pair[1].at_ms)
        && wave.pending.iter().all(|p| {
            let elapsed = p.at_ms.saturating_sub(wave.released_at_ms);
            p.at_ms > s.logical_ms
                && p.at_ms < (1u64 << 53)
                && elapsed <= last
                && (landing.is_none() || elapsed == last)
                && valid_id(&p.target.id)
                && p.target.generation > 0
                && s.monsters.iter().any(|m| {
                    m.entity.id == p.target.id && m.entity.generation >= p.target.generation
                })
        })
}

fn valid_piercing(r: &ProjectileViewV4, s: &SnapshotV4) -> bool {
    let Some(p) = &r.piercing else {
        return true;
    };
    r.unit_target.is_none()
        && r.source.kind == StatusEntityKind::Participant
        && s.members.iter().any(|m| m.entity == r.source)
        && valid_id(&p.skill_id)
        && r.radius <= 128.
        && p.direction.iter().all(|v| v.is_finite())
        && (p.direction[0].hypot(p.direction[1]) - 1.).abs() <= 1e-9
        && p.speed.is_finite()
        && (0.1..=10000.).contains(&p.speed)
        && p.remaining_range.is_finite()
        && p.remaining_range > 0.
        && p.remaining_range <= 10000.
        && p.created_at_ms <= s.logical_ms
        && p.expires_at_ms > s.logical_ms
        && p.expires_at_ms > p.created_at_ms
        && p.expires_at_ms - p.created_at_ms <= 60000
        && p.expires_at_ms < (1u64 << 53)
        && (1..=128).contains(&p.max_hits)
        && p.used_hits < p.max_hits
        && p.remaining_hits == p.max_hits - p.used_hits
        && p.repeat_interval_ms <= 60000
        && p.contacts.len() <= p.used_hits as usize
        && (p.used_hits == 0) == p.contacts.is_empty()
        && (p.repeat_interval_ms > 0 || p.contacts.len() == p.used_hits as usize)
        && unique(
            &p.contacts
                .iter()
                .map(|c| (&c.target.id, c.target.generation))
                .collect::<Vec<_>>(),
        )
        && p.contacts.iter().all(|c| {
            valid_id(&c.target.id)
                && c.target.generation > 0
                && s.monsters.iter().any(|m| {
                    m.entity.id == c.target.id && m.entity.generation >= c.target.generation
                })
                && c.hit_at_ms > p.created_at_ms
                && c.hit_at_ms <= s.logical_ms
                && c.hit_at_ms < p.expires_at_ms
                && c.next_eligible_at_ms
                    == (p.repeat_interval_ms > 0)
                        .then(|| c.hit_at_ms.saturating_add(p.repeat_interval_ms))
                && c.next_eligible_at_ms.is_none_or(|v| v < (1u64 << 53))
        })
}

fn valid_basic_projectile_segments(s: &SnapshotV4) -> bool {
    let mut sources = std::collections::BTreeMap::new();
    s.projectiles.iter().all(|p| {
        let Some(segment) = &p.basic_segment else {
            return true;
        };
        if !valid_id(&segment.action_id)
            || !valid_id(&segment.plan_id)
            || segment.plan_id.len() > 64
            || segment.index > 8
            || segment.launched_at_ms > s.logical_ms
            || s.logical_ms - segment.launched_at_ms >= 60000
            || !segment.aim_point.iter().all(|p| p.is_finite())
            || (segment.tier.is_some() && segment.role != Some(BasicProjectileRole::ChargedFlight))
            || match segment.role {
                Some(BasicProjectileRole::ChargedFlight) => {
                    segment.index != 0
                        || p.unit_target.is_some()
                        || !(0.1..=128.).contains(&p.radius)
                        || segment.tier.is_none_or(|tier| !(1..=3).contains(&tier))
                        || s.projectiles.iter().any(|other| {
                            other.source == p.source
                                && other.id != p.id
                                && other.basic_segment.is_some()
                        })
                }
                Some(BasicProjectileRole::SealOutbound) => segment.index != 0,
                Some(BasicProjectileRole::SealRecall | BasicProjectileRole::ReturnToOwner) => {
                    segment.index == 0 || p.unit_target.is_some()
                }
                Some(BasicProjectileRole::Ricochet) => segment.index == 8,
                None => false,
            }
            || p.piercing.is_some()
            || p.source.kind != StatusEntityKind::Participant
            || !s.members.iter().any(|m| {
                m.entity == p.source
                    && m.phase == MemberPhase::Active
                    && (!matches!(
                        segment.role,
                        Some(BasicProjectileRole::SealOutbound | BasicProjectileRole::SealRecall)
                    ) || m.basic_seals.is_some())
            })
        {
            return false;
        }
        let (action, plan, mask) = sources
            .entry((&p.source.id, p.source.generation))
            .or_insert((&segment.action_id, &segment.plan_id, 0u16));
        let bit = 1 << segment.index;
        let valid = *action == &segment.action_id && *plan == &segment.plan_id && *mask & bit == 0;
        *mask |= bit;
        valid
    })
}
fn valid_area(a: &AreaPlanViewV4, s: &SnapshotV4) -> bool {
    if let Some(anchor_id) = &a.attached_anchor_id
        && (a.mechanism != AreaPlanKind::Field
            || a.expires_at_ms <= s.logical_ms
            || !s.areas.as_ref().is_some_and(|areas| {
                areas.iter().any(|anchor| {
                    &anchor.id == anchor_id
                        && anchor.mechanism == AreaPlanKind::MemberCastAnchor
                        && anchor.source == a.source
                        && anchor.center == a.center
                        && anchor.created_at_ms <= a.created_at_ms
                })
            }))
    {
        return false;
    }
    if a.mechanism == AreaPlanKind::MemberCastAnchor {
        return valid_member_cast_anchor(a, s);
    }
    if a.anchor_recast.is_some() {
        return false;
    }
    if a.mechanism == AreaPlanKind::MovingEnter {
        return valid_moving_enter_area(a, s);
    }
    if a.mechanism == AreaPlanKind::NpcGroundField {
        return valid_npc_ground_field(a, s);
    }
    if a.mechanism == AreaPlanKind::MemberGroundTrap {
        return valid_member_ground_trap(a, s);
    }
    let basic = a.mechanism == AreaPlanKind::BasicField;
    let strike = a.mechanism == AreaPlanKind::NpcGroundStrike;
    let max_radius = if basic || strike { 512. } else { 128. };
    let valid_source = if strike {
        a.source.kind == StatusEntityKind::Monster
            && a.activates_at_ms > s.logical_ms
            && s.monsters
                .iter()
                .any(|npc| npc.entity == a.source && npc.hp > 0)
    } else {
        a.source.kind == StatusEntityKind::Participant
            && s.members.iter().any(|m| m.entity == a.source)
    };
    if a.movement.is_some()
        || !valid_id(&a.id)
        || !valid_id(&a.skill_id)
        || !valid_source
        || !a.center.iter().all(|n| n.is_finite())
        || !a.radius.is_finite()
        || !(0.1..=max_radius).contains(&a.radius)
        || a.created_at_ms > s.logical_ms
        || a.activates_at_ms < a.created_at_ms
        || a.activates_at_ms - a.created_at_ms > 60000
        || a.expires_at_ms < a.activates_at_ms
        || a.expires_at_ms >= (1u64 << 53)
        || a.completed_ticks > a.total_ticks
        || !(1..=128).contains(&a.total_ticks)
    {
        return false;
    }
    if basic {
        let duration = a.expires_at_ms - a.activates_at_ms;
        return a.basic_action_id.as_ref().is_some_and(|id| valid_id(id))
            && a.skill_id.len() <= 64
            && a.line.is_none()
            && a.contact_policy.is_none()
            && a.created_at_ms == a.activates_at_ms
            && a.expires_at_ms > s.logical_ms
            && (1..=60000).contains(&a.interval_ms)
            && duration <= 60000
            && duration.is_multiple_of(a.interval_ms)
            && duration / a.interval_ms == u64::from(a.total_ticks)
            && a.completed_ticks < a.total_ticks
            && a.next_tick_at_ms.is_some_and(|at| {
                at > s.logical_ms
                    && at == a.activates_at_ms + (u64::from(a.completed_ticks) + 1) * a.interval_ms
            })
            && s.members
                .iter()
                .any(|m| m.entity == a.source && m.phase == MemberPhase::Active && m.hp > 0);
    }
    if a.basic_action_id.is_some() {
        return false;
    }
    match (&a.line, a.mechanism) {
        (Some(line), AreaPlanKind::LineField)
            if line.start == a.center
                && line.width == a.radius * 2.
                && line
                    .start
                    .iter()
                    .chain(line.end.iter())
                    .all(|n| n.is_finite())
                && (line.end[0] - line.start[0]).hypot(line.end[1] - line.start[1])
                    <= 10000. + 1e-9 => {}
        (None, AreaPlanKind::Circle | AreaPlanKind::Field | AreaPlanKind::NpcGroundStrike) => {}
        _ => return false,
    }
    if a.mechanism == AreaPlanKind::Circle || strike {
        return a.interval_ms == 0
            && a.expires_at_ms == a.activates_at_ms
            && a.total_ticks == 1
            && a.completed_ticks == 0
            && a.next_tick_at_ms == Some(a.activates_at_ms)
            && a.contact_policy.is_none();
    }
    let duration = a.expires_at_ms - a.activates_at_ms;
    if !(1..=60000).contains(&a.interval_ms)
        || duration > 60000
        || duration <= a.interval_ms
        || u64::from(a.total_ticks) != (duration - 1) / a.interval_ms
    {
        return false;
    }
    let next = (a.completed_ticks < a.total_ticks)
        .then(|| a.activates_at_ms + (u64::from(a.completed_ticks) + 1) * a.interval_ms);
    if a.next_tick_at_ms != next {
        return false;
    }
    valid_member_area_history(a, s)
}

fn valid_member_cast_anchor(a: &AreaPlanViewV4, s: &SnapshotV4) -> bool {
    valid_id(&a.id)
        && valid_id(&a.skill_id)
        && a.source.kind == StatusEntityKind::Participant
        && s.members
            .iter()
            .any(|m| m.entity == a.source && m.phase == MemberPhase::Active && m.hp > 0)
        && a.center.iter().all(|v| v.is_finite())
        && a.radius == 0.
        && a.created_at_ms <= s.logical_ms
        && a.created_at_ms == a.activates_at_ms
        && a.expires_at_ms > s.logical_ms
        && a.expires_at_ms < (1u64 << 53)
        && a.expires_at_ms - a.created_at_ms <= 60000
        && a.interval_ms == 0
        && a.next_tick_at_ms.is_none()
        && a.completed_ticks == 0
        && a.total_ticks == 0
        && a.contact_policy.is_none()
        && a.movement.is_none()
        && a.line.is_none()
        && a.basic_action_id.is_none()
        && a.anchor_recast.as_ref().is_none_or(|recast| {
            valid_id(&recast.action_id)
                && recast.expires_at_ms > a.created_at_ms
                && recast.expires_at_ms <= a.expires_at_ms
                && recast.expires_at_ms - a.created_at_ms <= crate::config::MAX_RECAST_WINDOW_MS
                && (1..=crate::config::MAX_ANCHOR_RECAST_COUNT).contains(&recast.max_count)
                && recast.used_count <= recast.max_count
                && recast.behaviors.as_ref().is_none_or(|behaviors| {
                    AnchorRecastBehavior::valid_set(behaviors)
                        && (recast.used_count == 0
                            || behaviors.iter().any(|b| *b != AnchorRecastBehavior::End))
                })
        })
}

fn valid_anchor_transition(previous: &SnapshotV4, next: &SnapshotV4) -> bool {
    let before = previous.areas.as_deref().unwrap_or_default();
    let after = next.areas.as_deref().unwrap_or_default();
    let is_anchor = |a: &&AreaPlanViewV4| a.mechanism == AreaPlanKind::MemberCastAnchor;
    before.iter().filter(is_anchor).all(|old| {
        after
            .iter()
            .find(|a| a.id == old.id)
            .is_none_or(|a| valid_anchor_update(old, a, previous.logical_ms))
    }) && after.iter().filter(is_anchor).all(|a| {
        before
            .iter()
            .find(|old| old.id == a.id)
            .is_none_or(|old| old.mechanism == AreaPlanKind::MemberCastAnchor)
    })
}

fn valid_anchor_update(old: &AreaPlanViewV4, next: &AreaPlanViewV4, previous_ms: u64) -> bool {
    if old == next {
        return true;
    }
    let (Some(a), Some(b)) = (&old.anchor_recast, &next.anchor_recast) else {
        return false;
    };
    old.mechanism == next.mechanism
        && old.source == next.source
        && old.skill_id == next.skill_id
        && old.created_at_ms == next.created_at_ms
        && old.expires_at_ms == next.expires_at_ms
        && a.action_id == b.action_id
        && a.expires_at_ms == b.expires_at_ms
        && a.max_count == b.max_count
        && a.behaviors == b.behaviors
        && previous_ms < a.expires_at_ms
        && b.used_count > a.used_count
        // Coalesced updates can represent multiple moves back to the original point.
        && (b.used_count > a.used_count + 1 || next.center != old.center)
}

fn valid_anchor_field_transition(previous: &SnapshotV4, next: &SnapshotV4) -> bool {
    let before = previous.areas.as_deref().unwrap_or_default();
    let after = next.areas.as_deref().unwrap_or_default();
    let is_attached = |a: &&AreaPlanViewV4| a.attached_anchor_id.is_some();
    before.iter().filter(is_attached).all(|old| {
        after.iter().find(|a| a.id == old.id).is_none_or(|a| {
            a.attached_anchor_id == old.attached_anchor_id
                && a.source == old.source
                && a.skill_id == old.skill_id
                && a.radius == old.radius
                && a.created_at_ms == old.created_at_ms
                && a.activates_at_ms == old.activates_at_ms
                && a.expires_at_ms == old.expires_at_ms
                && a.interval_ms == old.interval_ms
                && a.completed_ticks >= old.completed_ticks
        })
    }) && after.iter().filter(is_attached).all(|a| {
        before
            .iter()
            .find(|old| old.id == a.id)
            .is_none_or(|old| old.attached_anchor_id.is_some())
    })
}

fn valid_member_ground_trap(a: &AreaPlanViewV4, s: &SnapshotV4) -> bool {
    let duration = a.expires_at_ms.saturating_sub(a.activates_at_ms);
    valid_id(&a.id)
        && valid_id(&a.skill_id)
        && a.source.kind == StatusEntityKind::Participant
        && s.members
            .iter()
            .any(|m| m.entity == a.source && m.phase == MemberPhase::Active && m.hp > 0)
        && a.movement.is_none()
        && a.line.is_none()
        && a.basic_action_id.is_none()
        && a.center.iter().all(|v| v.is_finite())
        && (0.1..=128.).contains(&a.radius)
        && a.created_at_ms <= s.logical_ms
        && a.activates_at_ms >= a.created_at_ms
        && a.activates_at_ms - a.created_at_ms <= 60000
        && (1..=60000).contains(&duration)
        && s.logical_ms < a.expires_at_ms
        && a.expires_at_ms < (1u64 << 53)
        && a.interval_ms == 0
        && a.next_tick_at_ms.is_none()
        && a.completed_ticks == 0
        && a.total_ticks == 0
        && a.contact_policy
            .as_ref()
            .is_some_and(|p| p.repeat_interval_ms > 0 || p.contacts.len() == p.used_hits as usize)
        && valid_member_area_history(a, s)
}

fn valid_member_area_history(a: &AreaPlanViewV4, s: &SnapshotV4) -> bool {
    let Some(p) = &a.contact_policy else {
        return false;
    };
    (1..=32).contains(&p.max_targets_per_tick)
        && (1..=128).contains(&p.max_total_hits)
        && p.repeat_interval_ms <= 60000
        && p.used_hits < p.max_total_hits
        && p.remaining_hits == p.max_total_hits - p.used_hits
        && p.contacts.len() <= p.used_hits as usize
        && (p.used_hits == 0) == p.contacts.is_empty()
        && unique(
            &p.contacts
                .iter()
                .map(|c| (&c.target.id, c.target.generation))
                .collect::<Vec<_>>(),
        )
        && p.contacts.iter().all(|c| {
            valid_id(&c.target.id)
                && c.target.generation > 0
                && s.monsters.iter().any(|m| {
                    m.entity.id == c.target.id && m.entity.generation >= c.target.generation
                })
                && c.last_hit_at_ms >= a.activates_at_ms + a.interval_ms
                && c.last_hit_at_ms < a.expires_at_ms
                && c.last_hit_at_ms <= s.logical_ms
                && c.next_eligible_at_ms.is_none_or(|n| n < (1u64 << 53))
                && c.next_eligible_at_ms
                    == (p.repeat_interval_ms > 0).then(|| c.last_hit_at_ms + p.repeat_interval_ms)
        })
}

fn valid_npc_ground_field(a: &AreaPlanViewV4, s: &SnapshotV4) -> bool {
    let Some(p) = &a.contact_policy else {
        return false;
    };
    let duration = a.expires_at_ms.saturating_sub(a.activates_at_ms);
    let pending = s.logical_ms < a.activates_at_ms;
    if !valid_id(&a.id)
        || !valid_id(&a.skill_id)
        || a.source.kind != StatusEntityKind::Monster
        || !s.monsters.iter().any(|n| {
            n.entity.id == a.source.id
                && if pending {
                    n.entity.generation == a.source.generation && n.hp > 0
                } else {
                    n.entity.generation >= a.source.generation
                }
        })
        || a.line.is_some()
        || a.basic_action_id.is_some()
        || a.movement.is_some()
        || !(0.1..=512.).contains(&a.radius)
        || !a.center.iter().all(|x| x.is_finite())
        || a.created_at_ms > s.logical_ms
        || !(1..=60000).contains(&a.activates_at_ms.saturating_sub(a.created_at_ms))
        || a.expires_at_ms <= s.logical_ms
        || a.expires_at_ms >= (1u64 << 53)
        || !(1..=60000).contains(&a.interval_ms)
        || !(a.interval_ms..=60000).contains(&duration)
        || !duration.is_multiple_of(a.interval_ms)
        || !(1..=128).contains(&a.total_ticks)
        || duration / a.interval_ms != u64::from(a.total_ticks)
        || a.completed_ticks >= a.total_ticks
        || u64::from(a.completed_ticks)
            != s.logical_ms.saturating_sub(a.activates_at_ms) / a.interval_ms
        || a.next_tick_at_ms
            != Some(a.activates_at_ms + (u64::from(a.completed_ticks) + 1) * a.interval_ms)
        || !(1..=16).contains(&p.max_targets_per_tick)
        || !(1..=256).contains(&p.max_total_hits)
        || p.repeat_interval_ms != a.interval_ms
        || p.used_hits >= p.max_total_hits
        || p.used_hits > a.completed_ticks * p.max_targets_per_tick
        || p.remaining_hits != p.max_total_hits - p.used_hits
        || p.contacts.len() > p.used_hits as usize
        || (p.used_hits == 0) != p.contacts.is_empty()
    {
        return false;
    }
    unique(
        &p.contacts
            .iter()
            .map(|c| (&c.target.id, c.target.generation))
            .collect::<Vec<_>>(),
    ) && p.contacts.iter().all(|c| {
        s.members
            .iter()
            .any(|m| m.entity.id == c.target.id && m.entity.generation == c.target.generation)
            && c.last_hit_at_ms >= a.activates_at_ms + a.interval_ms
            && c.last_hit_at_ms <= s.logical_ms
            && (c.last_hit_at_ms - a.activates_at_ms).is_multiple_of(a.interval_ms)
            && c.next_eligible_at_ms == Some(c.last_hit_at_ms + a.interval_ms)
    })
}

fn valid_moving_enter_area(a: &AreaPlanViewV4, s: &SnapshotV4) -> bool {
    let (Some(m), Some(p)) = (&a.movement, &a.contact_policy) else {
        return false;
    };
    if !valid_id(&a.id)
        || !valid_id(&a.skill_id)
        || a.source.kind != StatusEntityKind::Monster
        || !s
            .monsters
            .iter()
            .any(|n| n.entity.id == a.source.id && n.entity.generation >= a.source.generation)
        || a.line.is_some()
        || a.basic_action_id.is_some()
        || !(0.1..=128.).contains(&a.radius)
        || a.created_at_ms > s.logical_ms
        || !(1..=60000).contains(&a.activates_at_ms.saturating_sub(a.created_at_ms))
        || !(1..=60000).contains(&a.expires_at_ms.saturating_sub(a.activates_at_ms))
        || a.expires_at_ms <= s.logical_ms
        || a.expires_at_ms >= (1u64 << 53)
        || m.committed != (s.logical_ms >= a.activates_at_ms)
        || a.interval_ms != 0
        || a.next_tick_at_ms.is_some()
        || a.completed_ticks != 0
        || a.total_ticks != 0
        || !m
            .origin
            .iter()
            .chain(m.direction.iter())
            .chain(a.center.iter())
            .all(|n| n.is_finite())
        || (m.direction[0].hypot(m.direction[1]) - 1.).abs() > 1e-9
        || !(0.1..=10000.).contains(&m.speed)
        || !(0.0..=10000.).contains(&m.distance)
    {
        return false;
    }
    let progress = m.speed * s.logical_ms.saturating_sub(a.activates_at_ms) as f64 / 1000.;
    (!m.committed || progress < m.distance)
        && (0..2).all(|i| (a.center[i] - (m.origin[i] + m.direction[i] * progress)).abs() <= 1e-7)
        && (1..=8).contains(&p.max_targets_per_tick)
        && (1..=128).contains(&p.max_total_hits)
        && p.repeat_interval_ms <= 60000
        && p.used_hits < p.max_total_hits
        && p.remaining_hits == p.max_total_hits - p.used_hits
        && p.contacts.len() <= p.used_hits as usize
        && (p.used_hits == 0) == p.contacts.is_empty()
        && (m.committed || p.used_hits == 0)
        && unique(
            &p.contacts
                .iter()
                .map(|c| (&c.target.id, c.target.generation))
                .collect::<Vec<_>>(),
        )
        && p.contacts.iter().all(|c| {
            s.members.iter().any(|member| {
                member.entity.id == c.target.id && member.entity.generation == c.target.generation
            }) && c.last_hit_at_ms >= a.activates_at_ms
                && c.last_hit_at_ms <= s.logical_ms
                && c.next_eligible_at_ms
                    == (p.repeat_interval_ms > 0).then(|| c.last_hit_at_ms + p.repeat_interval_ms)
        })
}
fn valid_checkpoints(s: &SnapshotV4) -> bool {
    use crate::config::checkpoint_zones::{MAX_CHECKPOINT_SCHEDULES, MAX_CHECKPOINT_WINDOWS};
    let (zones, progress) = match (&s.checkpoint_zones, &s.own.checkpoint_progress) {
        (None, None) => return true,
        (Some(zones), Some(progress)) => (zones, progress),
        _ => return false,
    };
    if !(1..=MAX_CHECKPOINT_SCHEDULES).contains(&progress.len())
        || zones.len() > progress.len()
        || (!zones.is_empty() && (s.own.phase != MemberPhase::Active || s.phase != Phase::Active))
        || !unique(&progress.iter().map(|p| &p.schedule_id).collect::<Vec<_>>())
        || !unique(&zones.iter().map(|z| &z.schedule_id).collect::<Vec<_>>())
    {
        return false;
    }
    progress.iter().all(|p| {
        valid_id(&p.schedule_id)
            && (1..=MAX_CHECKPOINT_WINDOWS as u32).contains(&p.total_windows)
            && (1..=p.total_windows).contains(&p.required)
            && (p.collected..=p.total_windows).contains(&p.last_window_index)
            && (p.collected == 0) == (p.last_window_index == 0)
            && p.counter.as_ref().is_none_or(|counter| {
                s.phase == Phase::Active
                    && s.own.phase == MemberPhase::Active
                    && counter.window_index > 0
                    && counter.window_index == p.last_window_index
                    && counter.on_contact != Some(false)
                    && s.active_at_ms.is_some_and(|at| at <= counter.opens_at_ms)
                    && counter.opens_at_ms <= s.logical_ms
                    && s.logical_ms < counter.expires_at_ms
                    && counter.expires_at_ms < (1u64 << 53)
                    && counter.expires_at_ms - counter.opens_at_ms <= 60000
                    && valid_unit(&counter.target, s)
                    && s.monsters.iter().any(|n| {
                        n.entity.id == counter.target.id
                            && n.entity.generation == counter.target.generation
                            && n.hp > 0
                    })
            })
    }) && zones.iter().all(|z| {
        s.active_at_ms.is_some_and(|at| at <= z.opens_at_ms)
            && z.opens_at_ms <= s.logical_ms
            && s.logical_ms < z.expires_at_ms
            && z.expires_at_ms < (1u64 << 53)
            && z.expires_at_ms - z.opens_at_ms <= 60000
            && z.avoid != Some(false)
            && z.center.iter().all(|v| v.is_finite())
            && (0.1..=512.).contains(&z.radius)
            && progress.iter().any(|p| {
                p.schedule_id == z.schedule_id
                    && (1..=p.total_windows).contains(&z.window_index)
                    && p.last_window_index <= z.window_index
            })
    })
}

fn valid_checkpoint_transition(previous: &SnapshotV4, next: &SnapshotV4) -> bool {
    match (
        &previous.own.checkpoint_progress,
        &next.own.checkpoint_progress,
    ) {
        (None, None) => true,
        (Some(old), Some(new)) => {
            old.len() == new.len()
                && old.iter().zip(new).all(|(a, b)| {
                    a.schedule_id == b.schedule_id
                        && a.required == b.required
                        && a.total_windows == b.total_windows
                        && (b.last_window_index > a.last_window_index
                            || b.counter.is_none()
                            || a.counter == b.counter)
                        && b.collected.checked_sub(a.collected).is_some_and(|added| {
                            if added == 0 {
                                a.last_window_index == b.last_window_index
                            } else {
                                a.last_window_index + added <= b.last_window_index
                            }
                        })
                })
                && previous.checkpoint_zones.as_ref().unwrap().iter().all(|a| {
                    next.checkpoint_zones
                        .as_ref()
                        .unwrap()
                        .iter()
                        .find(|b| b.schedule_id == a.schedule_id)
                        .is_none_or(|b| b.window_index > a.window_index || b == a)
                })
        }
        _ => false,
    }
}

fn valid_contested_equipment(s: &SnapshotV4) -> bool {
    let holder = &s.own.participant.entity;
    let Some(rows) = &s.contested_equipment else {
        return s.own.world_weapon_profile.is_none();
    };
    let profile_valid = s
        .own
        .world_weapon_profile
        .as_ref()
        .is_none_or(|id| valid_id(id) && id.len() <= 64);
    profile_valid
        && s.own.mode.is_none()
        && s.own.training.is_none()
        && s.own.world_weapon_profile.is_some()
            == rows.iter().any(|row| row.holder.as_ref() == Some(holder))
        && (1..=crate::config::contested_equipment::MAX_CLAIMS).contains(&rows.len())
        && unique(&rows.iter().map(|row| &row.id).collect::<Vec<_>>())
        && rows.iter().all(|row| {
            valid_id(&row.id)
                && row.id.len() <= 64
                && row.position.iter().all(|v| v.is_finite())
                && row.revision > 0
                && (row.state == WorldEquipmentState::Available) == (row.revision == 1)
                && (row.state == WorldEquipmentState::Held) == row.holder.is_some()
                && row.holder.as_ref().is_none_or(|entity| {
                    s.members
                        .iter()
                        .any(|m| m.entity == *entity && m.phase == MemberPhase::Active && m.hp > 0)
                })
        })
}

fn valid_contested_transition(a: &SnapshotV4, b: &SnapshotV4) -> bool {
    match (&a.contested_equipment, &b.contested_equipment) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.len() == b.len()
                && a.iter().zip(b).all(|(x, y)| {
                    x == y
                        || x.id == y.id
                            && x.revision < y.revision
                            && !matches!(
                                x.state,
                                WorldEquipmentState::Secured | WorldEquipmentState::Lost
                            )
                })
        }
        _ => false,
    }
}

fn valid_zone_exposure(s: &SnapshotV4) -> bool {
    use crate::config::zone_exposure::{MAX_NODES, MAX_STACKS};
    let Some(view) = &s.own.zone_exposure else {
        return true;
    };
    let short_id = |id: &str| valid_id(id) && id.len() <= 64;
    s.own.phase == MemberPhase::Active
        && s.own.hp > 0
        && s.own.mode.is_none()
        && s.own.training.is_none()
        && short_id(&view.band_id)
        && (1..=MAX_STACKS).contains(&view.max_stacks)
        && view.stacks <= view.max_stacks
        && view.boost_ends_at_ms.is_none_or(|end| {
            end > s.logical_ms && end < (1 << 53) && end - s.logical_ms <= 3_600_000
        })
        && view.nodes.len() <= MAX_NODES
        && unique(&view.nodes.iter().map(|node| &node.id).collect::<Vec<_>>())
        && view.nodes.iter().all(|node| {
            short_id(&node.id)
                && node.position.iter().all(|v| v.is_finite())
                && node.revision > 0
                && (node.revision == 1) == (node.ready_at_ms == 0)
                && node.ready_at_ms < (1 << 53)
                && node.ready_at_ms.saturating_sub(s.logical_ms) <= 3_600_000
        })
}

fn valid_snapshot(s: &SnapshotV4, pid: &str) -> bool {
    if s.active_blockers
        .as_ref()
        .is_some_and(|ids| ids.len() > 64 || !ids.iter().all(|id| valid_id(id)) || !unique(ids))
    {
        return false;
    }
    if !valid_checkpoints(s) || !valid_boons(&s.own) || !valid_mode(s) || !valid_training(s) {
        return false;
    }
    if !valid_extraction_view(s) || !valid_contested_equipment(s) || !valid_zone_exposure(s) {
        return false;
    }
    if s.encounter_progress.as_ref().is_some_and(|p| {
        s.own.training.is_some()
            || p.initial == 0
            || p.remaining > p.initial
            || !(1..=100_000).contains(&p.stats_bps)
            || p.surge_started && p.remaining != 0
            || p.surge_ends_at_ms.is_some_and(|at| {
                !p.surge_started
                    || at == 0
                    || at >= (1u64 << 53)
                    || s.own.phase == MemberPhase::Active && at <= s.logical_ms
            })
    }) {
        return false;
    }
    let entity = &s.own.participant.entity;
    let cooldowns = &s.own.item_cooldowns;
    if cooldowns.len() > crate::config::MAX_MEMBER_ITEM_USES
        || !unique(
            &cooldowns
                .iter()
                .map(|entry| &entry.item_id)
                .collect::<Vec<_>>(),
        )
        || cooldowns.iter().any(|entry| {
            !valid_id(&entry.item_id)
                || entry.item_id.len() > 64
                || entry.ready_at_ms >= (1u64 << 53)
                || entry.ready_at_ms <= s.logical_ms
                || entry.ready_at_ms - s.logical_ms > crate::config::MAX_MEMBER_ITEM_COOLDOWN_MS
        })
    {
        return false;
    }
    if s.own
        .basic_profile
        .as_ref()
        .is_some_and(|id| !valid_id(id) || id.len() > 64)
    {
        return false;
    }
    if let Some(equipment) = &s.own.equipment
        && (equipment.weapon.is_some() != s.own.basic_profile.is_some()
            || equipment.revision == 0 && equipment.weapon.is_none()
            || equipment.weapon.as_ref().is_some_and(|weapon| {
                use crate::config::weapon_equipment::weapon_id;
                !weapon_id(&weapon.instance_id)
                    || !weapon_id(&weapon.model_id)
                    || weapon.morph_id.as_deref().is_some_and(|id| !weapon_id(id))
            }))
    {
        return false;
    }
    if s.own.basic_wave_phase.is_some()
        && (!matches!(s.own.phase, MemberPhase::Deploying | MemberPhase::Active)
            || s.basic_waves.is_none())
    {
        return false;
    }
    if s.own.basic_charge_ready_at_ms.is_some_and(|at| {
        s.own.phase != MemberPhase::Active
            || at >= (1u64 << 53)
            || at.saturating_sub(s.logical_ms) > 60000
    }) || s.own.basic_charge.as_ref().is_some_and(|charge| {
        !valid_id(&charge.session_id)
            || s.own.phase != MemberPhase::Active
            || s.own.protected
            || !s.own.ready
            || s.own.pending_action.is_some()
            || s.own.basic_pursuit.is_some()
            || s.own.active_channel.is_some()
            || s.own.extraction_finishes_ms.is_some()
            || charge.started_at_ms > s.logical_ms
            || charge.max_charge_at_ms >= (1u64 << 53)
            || !(1..=30000).contains(&charge.max_charge_at_ms.saturating_sub(charge.started_at_ms))
            || match charge.kind {
                BasicChargeKind::StagedWave | BasicChargeKind::Flight => {
                    !(1..=3).contains(&charge.tier) || s.own.basic_charge_ready_at_ms.is_some()
                }
                BasicChargeKind::Guard => {
                    charge.tier != 1
                        || s.logical_ms >= charge.max_charge_at_ms
                        || s.own.basic_charge_ready_at_ms.is_some()
                }
                BasicChargeKind::Dash => {
                    charge.tier != 1
                        || s.own
                            .basic_charge_ready_at_ms
                            .is_none_or(|at| at > charge.started_at_ms)
                }
            }
    }) {
        return false;
    }
    if s.own.basic_projectile_progress.as_ref().is_some_and(|p| {
        !matches!(s.own.phase, MemberPhase::Deploying | MemberPhase::Active)
            || !(2..=8).contains(&p.maximum)
            || p.stored > p.maximum
            || if p.stored == 0 {
                p.attack_speed_bonus_bps != 0
            } else {
                !p.attack_speed_bonus_bps.is_multiple_of(p.stored)
                    || !(1..=10000 / p.maximum).contains(&(p.attack_speed_bonus_bps / p.stored))
            }
    }) {
        return false;
    }
    if s.own.skill_powers.as_ref().is_some_and(|m| {
        m.len() != s.own.player_skills.len()
            || m.keys()
                .any(|id| !s.own.player_skills.iter().any(|slot| slot.skill_id == *id))
    }) {
        return false;
    }
    if s.own.player_skills.len() > crate::config::MAX_MEMBER_SKILLS
        || !unique(
            &s.own
                .player_skills
                .iter()
                .map(|s| &s.skill_id)
                .collect::<Vec<_>>(),
        )
        || s.own
            .player_skills
            .iter()
            .any(|s| !valid_id(&s.skill_id) || s.ready_at_ms >= (1u64 << 53))
        || s.own.pending_action.as_ref().is_some_and(|p| {
            p.action_id.as_ref().is_some_and(|id| !valid_id(id))
                || p.resource_reservation.is_some_and(|r| {
                    p.action_id.is_none() || p.skill_id.is_none() || r.minimum_hp_after_cost == 0
                })
                || p.charge.as_ref().is_some_and(|charge| {
                    p.action_id
                        .as_ref()
                        .is_some_and(|id| *id != charge.action_id)
                })
                || p.commits_at_ms >= (1u64 << 53)
                || p.unit_target.is_some() && p.ground_position.is_some()
                || match &p.ally_target {
                    Some(target) => {
                        let self_target = target.id == entity.id;
                        let self_marker = if self_target { Some(true) } else { None };
                        p.ally_target_is_self != self_marker
                            || member_combat(s) && !self_target
                            || p.skill_id.is_none()
                            || p.unit_target.is_some()
                            || p.ground_position.is_some()
                            || p.charge.is_some()
                            || !s.members.iter().any(|member| {
                                member.entity.id == target.id
                                    && member.entity.generation == target.generation
                            })
                    }
                    None => p.ally_target_is_self.is_some(),
                }
                || p.skill_id
                    .as_ref()
                    .is_some_and(|id| !s.own.player_skills.iter().any(|s| s.skill_id == *id))
                || p.charge.as_ref().is_some_and(|charge| {
                    let windup = charge.release_windup_ms.unwrap_or(0);
                    let latest_release = charge.auto_commit_at_ms.saturating_sub(windup);
                    p.skill_id.is_none()
                        || p.ground_position.is_some()
                        || p.unit_target.is_some()
                        || !valid_id(&charge.action_id)
                        || charge.started_at_ms > s.logical_ms
                        || charge.started_at_ms >= charge.min_release_at_ms
                        || charge.min_release_at_ms >= latest_release
                        || charge
                            .release_windup_ms
                            .is_some_and(|w| w == 0 || w > crate::config::MAX_CHARGE_MS)
                        || charge.auto_commit_at_ms >= (1u64 << 53)
                        || charge
                            .auto_commit_at_ms
                            .saturating_sub(charge.started_at_ms)
                            > crate::config::MAX_CHARGE_MS
                        || !(charge.min_release_at_ms.saturating_add(windup)
                            ..=charge.auto_commit_at_ms)
                            .contains(&p.commits_at_ms)
                        || (p.commits_at_ms < charge.auto_commit_at_ms
                            && p.commits_at_ms > s.logical_ms
                            && charge.released_at_ms.is_none())
                        || charge.released_at_ms.is_some_and(|released| {
                            released < charge.started_at_ms
                                || released > s.logical_ms
                                || released > latest_release
                                || p.commits_at_ms
                                    != released.max(charge.min_release_at_ms) + windup
                        })
                        || charge.aim.as_ref().is_some_and(|aim| {
                            let [x, y] = aim.direction;
                            !x.is_finite()
                                || !y.is_finite()
                                || (x.hypot(y) - 1.).abs() > 1e-9
                                || aim.updated_at_ms < charge.started_at_ms
                                || aim.updated_at_ms
                                    > charge
                                        .released_at_ms
                                        .unwrap_or(latest_release)
                                        .min(s.logical_ms)
                                || aim.updated_at_ms >= charge.auto_commit_at_ms
                        })
                })
        })
    {
        return false;
    }

    if s.own.active_channel.as_ref().is_some_and(|channel| {
        let [x, y] = channel.direction;
        !valid_id(&channel.action_id)
            || !s
                .own
                .player_skills
                .iter()
                .any(|skill| skill.skill_id == channel.skill_id)
            || s.phase != Phase::Active
            || s.own.phase != MemberPhase::Active
            || s.own.protected
            || !s.own.ready
            || s.own.hp == 0
            || s.own.pending_action.is_some()
            || s.own.extraction_finishes_ms.is_some()
            || s.own.recovery_until_ms > channel.started_at_ms
            || !x.is_finite()
            || !y.is_finite()
            || channel.target.as_ref().is_some_and(|target| {
                !valid_id(&target.id)
                    || target.generation == 0
                    || channel.total_pulses.is_none()
                    || if channel.recovery == Some(true) {
                        target.kind != StatusEntityKind::Participant
                            || *target == s.own.participant.entity
                            || !s.members.iter().any(|m| m.entity == *target)
                    } else {
                        target.kind != StatusEntityKind::Monster
                            || !s.monsters.iter().any(|m| m.entity == *target)
                    }
            })
            || match channel.recovery {
                Some(true) => [x, y] != [0., 0.] || channel.total_pulses.is_none(),
                Some(false) => true,
                None => {
                    (x.hypot(y) - 1.).abs() > 1e-9
                        && !(channel.target.is_some() && [x, y] == [0., 0.])
                }
            }
            || channel.started_at_ms > s.logical_ms
            || channel.ends_at_ms <= s.logical_ms
            || channel.ends_at_ms >= (1u64 << 53)
            || channel.ends_at_ms.saturating_sub(channel.started_at_ms)
                > crate::config::MAX_CHANNEL_MS
            || channel.pulses > crate::config::MAX_CHANNEL_PULSES
            || channel
                .next_pulse_at_ms
                .map_or(channel.pulses == 0, |next| {
                    next <= s.logical_ms
                        || next > channel.ends_at_ms
                        || next == channel.ends_at_ms && channel.total_pulses.is_none()
                })
            || channel.total_pulses.is_some_and(|count| {
                !(1..=crate::config::MAX_CHANNEL_PULSES).contains(&count)
                    || channel.pulses >= count
                    || !(channel.ends_at_ms - channel.started_at_ms)
                        .is_multiple_of(u64::from(count))
                    || channel.next_pulse_at_ms
                        != Some(
                            channel.started_at_ms
                                + u64::from(channel.pulses + 1)
                                    * ((channel.ends_at_ms - channel.started_at_ms)
                                        / u64::from(count)),
                        )
            })
    }) {
        return false;
    }

    if s.own.projectile_recast.as_ref().is_some_and(|recast| {
        !valid_id(&recast.action_id)
            || s.phase != Phase::Active
            || s.own.phase != MemberPhase::Active
            || s.own.hp == 0
            || recast.next_index != 1
            || recast.expires_at_ms <= s.logical_ms
            || recast.expires_at_ms >= (1u64 << 53)
            || recast.expires_at_ms - s.logical_ms > crate::config::MAX_RECAST_WINDOW_MS
            || !s
                .own
                .player_skills
                .iter()
                .any(|skill| skill.skill_id == recast.skill_id)
            || !s.projectiles.iter().any(|shot| {
                shot.id == recast.projectile_id
                    && shot.source == s.own.participant.entity
                    && shot.unit_target.is_none()
                    && shot.piercing.is_none()
            })
    }) {
        return false;
    }

    if s.own.followup.as_ref().is_some_and(|followup| {
        s.phase != Phase::Active
            || s.own.phase != MemberPhase::Active
            || s.own.protected
            || !s.own.ready
            || s.own.hp == 0
            || s.own.pending_action.is_some()
            || s.own.active_channel.is_some()
            || s.own.extraction_finishes_ms.is_some()
            || s.own.recovery_until_ms <= s.logical_ms
            || followup.promotes_at_ms < s.own.recovery_until_ms
            || followup.promotes_at_ms - s.own.recovery_until_ms >= 100
            || followup.promotes_at_ms >= (1u64 << 53)
            || !s
                .own
                .player_skills
                .iter()
                .any(|slot| slot.skill_id == followup.source_skill_id)
            || followup.queued.as_ref().is_some_and(|cast| {
                let [x, y] = cast.direction;
                !valid_id(&cast.action_id)
                    || cast.skill_id == followup.source_skill_id
                    || !s.own.player_skills.iter().any(|slot| {
                        slot.skill_id == cast.skill_id
                            && slot.ready_at_ms <= followup.promotes_at_ms
                    })
                    || cast.expires_at_ms < followup.promotes_at_ms
                    || cast.expires_at_ms - s.logical_ms > crate::config::MAX_FOLLOWUP_WINDOW_MS
                    || cast.expires_at_ms >= (1u64 << 53)
                    || !x.is_finite()
                    || !y.is_finite()
                    || (x.hypot(y) - 1.).abs() > 1e-9
            })
    }) {
        return false;
    }

    if s.own.recovery_cancel.as_ref().is_some_and(|cancel| {
        s.phase != Phase::Active
            || s.own.phase != MemberPhase::Active
            || s.own.protected
            || !s.own.ready
            || s.own.hp == 0
            || s.own.pending_action.is_some()
            || s.own.active_channel.is_some()
            || s.own.extraction_finishes_ms.is_some()
            || s.own.recovery_until_ms <= s.logical_ms
            || s.own.recovery_until_ms >= (1u64 << 53)
            || cancel.cancel_at_ms >= s.own.recovery_until_ms
            || !s
                .own
                .player_skills
                .iter()
                .any(|slot| slot.skill_id == cancel.skill_id)
            || s.own
                .followup
                .as_ref()
                .is_some_and(|followup| followup.source_skill_id != cancel.skill_id)
    }) {
        return false;
    }

    if s.own.basic_followup.as_ref().is_some_and(|queued| {
        s.phase != Phase::Active
            || s.own.phase != MemberPhase::Active
            || s.own.protected
            || !s.own.ready
            || s.own.hp == 0
            || s.own.pending_action.is_some()
            || s.own.active_channel.is_some()
            || s.own.followup.is_some()
            || s.own.recovery_cancel.is_some()
            || s.own.extraction_finishes_ms.is_some()
            || s.own.recovery_until_ms > s.logical_ms
            || !valid_id(&queued.action_id)
            || !valid_id(&queued.target.id)
            || queued.target.generation == 0
            || s.own.basic_ready_at_ms == 0
            || queued.promotes_at_ms < s.own.basic_ready_at_ms
            || queued.promotes_at_ms - s.own.basic_ready_at_ms >= 100
            || queued.expires_at_ms <= s.logical_ms
            || queued.expires_at_ms < queued.promotes_at_ms
            || queued.expires_at_ms - queued.promotes_at_ms > crate::config::MAX_FOLLOWUP_WINDOW_MS
            || queued.expires_at_ms >= (1u64 << 53)
    }) {
        return false;
    }

    if s.own.basic_pursuit.as_ref().is_some_and(|plan| {
        s.phase != Phase::Active
            || s.own.phase != MemberPhase::Active
            || s.own.protected
            || !s.own.ready
            || s.own.hp == 0
            || s.own.pending_action.is_some()
            || s.own.active_channel.is_some()
            || s.own.basic_followup.is_some()
            || s.own.extraction_finishes_ms.is_some()
            || s.own.recovery_until_ms > s.logical_ms
            || s.own.basic_ready_at_ms > s.logical_ms
            || !valid_id(&plan.action_id)
            || !valid_id(&plan.target.id)
            || plan.target.generation == 0
            || plan.started_at_ms > s.logical_ms
            || plan.expires_at_ms <= s.logical_ms
            || plan.expires_at_ms - plan.started_at_ms > crate::config::MAX_BASIC_PURSUIT_MS
            || plan.expires_at_ms >= (1u64 << 53)
    }) {
        return false;
    }

    if s.areas.as_ref().is_some_and(|v| {
        v.iter()
            .filter(|a| a.mechanism == AreaPlanKind::BasicField)
            .count()
            > 12
            || v.iter()
                .filter(|a| a.mechanism != AreaPlanKind::BasicField)
                .count()
                > 8
            || v.iter()
                // Ground fields can repeatedly hit the fixed member roster; each
                // can publish at most one history row per seat.
                .filter(|a| a.mechanism != AreaPlanKind::NpcGroundField)
                .filter_map(|a| a.contact_policy.as_ref())
                .map(|p| u64::from(p.max_total_hits))
                .sum::<u64>()
                > 128
            || !unique(&v.iter().map(|a| &a.id).collect::<Vec<_>>())
            || !unique(
                &v.iter()
                    .filter(|a| a.mechanism == AreaPlanKind::MemberCastAnchor)
                    .map(|a| (&a.source, &a.skill_id))
                    .collect::<Vec<_>>(),
            )
            || v.iter().any(|a| !valid_area(a, s))
    }) {
        return false;
    }
    if s.basic_beams.as_ref().is_some_and(|beams| {
        beams.len() > 8
            || !unique(&beams.iter().map(|b| &b.action_id).collect::<Vec<_>>())
            || !unique(&beams.iter().map(|b| &b.source).collect::<Vec<_>>())
            || beams.iter().any(|beam| !valid_beam(beam, s))
    }) {
        return false;
    }
    if s.basic_impacts.as_ref().is_some_and(|plans| {
        plans.len() > 8
            || !unique(&plans.iter().map(|p| &p.id).collect::<Vec<_>>())
            || !unique(&plans.iter().map(|p| &p.source).collect::<Vec<_>>())
            || plans.iter().any(|p| !valid_impact(p, s))
    }) {
        return false;
    }
    if s.basic_waves.as_ref().is_some_and(|waves| {
        waves.len() > 8
            || !unique(&waves.iter().map(|w| &w.id).collect::<Vec<_>>())
            || !unique(&waves.iter().map(|w| &w.source).collect::<Vec<_>>())
            || waves.iter().any(|w| !valid_wave(w, s))
    }) {
        return false;
    }
    if s.own
        .pending_action
        .as_ref()
        .and_then(|a| a.unit_target.as_ref())
        .is_some_and(|t| !valid_hostile_unit(t, entity, s))
    {
        return false;
    }
    valid_shield_view(&s.own.statuses, s.own.shield_total, s.own.max_hp, true)
        && {
            s.own.statuses.len() <= 32
                && unique(
                    &s.own
                        .statuses
                        .iter()
                        .map(|v| &v.instance_id)
                        .collect::<Vec<_>>(),
                )
                && s.own.statuses.iter().all(|v| {
                    v.target.kind == entity.kind
                        && v.target.id == entity.id
                        && v.target.generation == entity.generation
                        && v.target.encounter_round.is_none()
                        && v.source.encounter_round.is_none()
                        && valid_id(&v.instance_id)
                        && valid_id(&v.definition_id)
                        && (v.source == v.target
                            || s.members.iter().any(|m| {
                                v.source.kind == m.entity.kind
                                    && v.source.id == m.entity.id
                                    && v.source.generation == m.entity.generation
                                    && (member_combat(s)
                                        || v.attribute.is_none_or(|a| {
                                            !a.is_control_flag()
                                                && a != StatusAttribute::SpiritRecoveryLock
                                        }) && v.periodic.as_ref().is_none_or(|p| {
                                            p.kind == PeriodicEffectKind::MaxHpHeal
                                        }) && !v.flags.iter().any(|f| f.is_control_flag()))
                            })
                            || s.monsters.iter().any(|m| {
                                m.entity.kind == v.source.kind
                                    && m.entity.id == v.source.id
                                    && (m.entity.generation == v.source.generation
                                        || v.retained_source == Some(true)
                                            && m.entity.generation > v.source.generation)
                            }))
                })
        }
        && entity.kind == StatusEntityKind::Participant
        && entity.id == pid
        && s.own.participant.owner_id == pid
        && entity.generation > 0
        && s.members.iter().all(|m| {
            let seals_valid = m.basic_seals.as_ref().is_none_or(|seals| {
                matches!(m.phase, MemberPhase::Deploying | MemberPhase::Active)
                    && seals.len() <= 8
                    && seals.iter().flatten().all(|v| v.is_finite())
            });
            seals_valid
                && m.entity.kind == StatusEntityKind::Participant
                && m.entity.generation > 0
                && valid_id(&m.entity.id)
                && m.position.iter().all(|n| n.is_finite())
        })
        && s.containers.as_ref().is_none_or(|rows| {
            rows.len() <= 16
                && unique(&rows.iter().map(|c| &c.id).collect::<Vec<_>>())
                && rows.iter().all(|c| {
                    valid_id(&c.id) && c.generation > 0 && c.position.iter().all(|v| v.is_finite())
                })
        })
        && s.relics.as_ref().is_none_or(|rows| {
            rows.len() <= 8
                && unique(&rows.iter().map(|r| &r.id).collect::<Vec<_>>())
                && unique(&rows.iter().map(|r| &r.deceased).collect::<Vec<_>>())
                && rows.iter().all(|r| {
                    valid_id(&r.id)
                        && r.revision > 0
                        && r.remaining_stacks <= 128
                        && (r.remaining_stacks == 0) == r.highest_quality.is_none()
                        && r.position.iter().all(|v| v.is_finite())
                        && s.members.iter().any(|m| {
                            m.entity == r.deceased
                                && matches!(m.phase, MemberPhase::Settling | MemberPhase::Terminal)
                        })
                })
        })
        && s.own.instances.len() <= 128
        && unique(
            &s.own
                .instances
                .iter()
                .map(|r| &r.instance_id)
                .collect::<Vec<_>>(),
        )
        && s.own.instances.iter().all(valid_item_instance)
        && s.monsters.iter().all(|m| {
            let training_valid = m.training_owner_id.as_ref().is_none_or(|owner| {
                m.summon.is_none() && s.members.iter().any(|member| member.entity.id == *owner)
            });
            let boss_valid = m.boss_phase.as_ref().is_none_or(|phase| {
                m.summon.is_none()
                    && phase.epoch == m.entity.generation
                    && (1..=8).contains(&phase.phase_count)
                    && (1..=phase.phase_count).contains(&phase.phase_index)
                    && valid_id(&phase.phase_id)
                    && valid_id(&phase.phase_name)
                    && (1..=3_600_000).contains(&phase.attack_interval_ms)
                    && (0.0..=1.0).contains(&phase.cast_chance)
            });
            let summon_valid = m.summon.as_ref().is_none_or(|summon| {
                summon.owner.kind == StatusEntityKind::Participant
                    && s.members.iter().any(|member| {
                        member.entity == summon.owner
                            && member.phase == MemberPhase::Active
                            && member.hp > 0
                    })
                    && m.hp > 0
                    && m.hp <= m.max_hp
                    && !m.returning
                    && m.target.is_none()
                    && (1..=60000).contains(&summon.expires_at_ms.saturating_sub(s.logical_ms))
                    && summon.expires_at_ms < (1u64 << 53)
            });
            valid_shield_view(
                m.statuses.as_deref().unwrap_or(&[]),
                m.shield_total,
                m.max_hp,
                false,
            ) && summon_valid
                && training_valid
                && boss_valid
                && m.target.as_ref().is_none_or(|target| {
                    target.kind == StatusEntityKind::Participant
                        || s.monsters
                            .iter()
                            .any(|actor| actor.summon.is_some() && actor.entity == *target)
                })
                && (m.shield_total.is_none() || m.statuses.is_some())
                && m.entity.kind == StatusEntityKind::Monster
                && m.entity.generation > 0
                && valid_id(&m.entity.id)
                && m.position.iter().all(|n| n.is_finite())
                && m.radius.is_finite()
                && m.radius >= 0.
                && m.statuses.as_ref().is_none_or(|entries| {
                    entries.len() <= 32
                        && unique(&entries.iter().map(|s| &s.instance_id).collect::<Vec<_>>())
                        && entries.iter().all(|v| {
                            let target = StatusEntity {
                                kind: m.entity.kind,
                                id: m.entity.id.clone(),
                                generation: m.entity.generation,
                                encounter_round: None,
                            };
                            let owner = StatusEntity {
                                kind: entity.kind,
                                id: entity.id.clone(),
                                generation: entity.generation,
                                encounter_round: None,
                            };
                            v.target == target
                                && (v.source == owner
                                    || v.source == target
                                    || m.summon.as_ref().is_some_and(|summon| {
                                        summon.owner == *entity
                                            && s.monsters.iter().any(|actor| {
                                                actor.summon.is_none()
                                                    && v.source.encounter_round.is_none()
                                                    && actor.entity.kind == v.source.kind
                                                    && actor.entity.id == v.source.id
                                                    && actor.entity.generation
                                                        == v.source.generation
                                            })
                                    }))
                                && valid_id(&v.instance_id)
                                && valid_id(&v.definition_id)
                        })
                })
        })
        && s.resources.iter().all(|r| {
            valid_id(&r.id)
                && r.position.iter().all(|n| n.is_finite())
                && r.instance.as_ref().is_none_or(|item| {
                    r.quantity == 1
                        && r.item_id.as_ref() == Some(&item.item_id)
                        && valid_item_instance(item)
                })
        })
        && unique(
            &s.resources
                .iter()
                .filter_map(|r| r.instance.as_ref())
                .chain(&s.own.instances)
                .map(|r| &r.instance_id)
                .collect::<Vec<_>>(),
        )
        && s.projectiles.len() <= 64
        && valid_basic_projectile_segments(s)
        && s.projectiles
            .iter()
            .filter_map(|r| r.piercing.as_ref())
            .map(|p| u64::from(p.max_hits))
            .sum::<u64>()
            <= 128
        && s.projectiles.iter().all(|r| {
            valid_piercing(r, s)
                && r.unit_target.as_ref().is_none_or(|t| {
                    valid_hostile_unit(t, &r.source, s)
                        && r.source.kind == StatusEntityKind::Participant
                        && s.members.iter().any(|m| m.entity == r.source)
                })
                && valid_id(&r.id)
                && r.position.iter().all(|n| n.is_finite())
                && r.radius.is_finite()
                && r.radius >= 0.
        })
        && s.logical_ms < (1u64 << 53)
        && s.members.iter().filter(|m| m.entity == *entity).count() == 1
        && unique(
            &s.members
                .iter()
                .map(|v| (&v.entity.kind, &v.entity.id))
                .collect::<Vec<_>>(),
        )
        && unique(
            &s.monsters
                .iter()
                .map(|v| (&v.entity.kind, &v.entity.id))
                .collect::<Vec<_>>(),
        )
        && unique(&s.resources.iter().map(|v| &v.id).collect::<Vec<_>>())
        && unique(&s.projectiles.iter().map(|v| &v.id).collect::<Vec<_>>())
        && s.own
            .position
            .iter()
            .chain(s.extraction_point.iter())
            .all(|v| v.is_finite())
}

fn valid_item_instance(item: &ItemInstance) -> bool {
    valid_id(&item.instance_id) && valid_id(&item.item_id)
}

fn valid_boons(own: &PrivateMemberView) -> bool {
    use crate::config::world_boons::{MAX_LEVELS, MAX_OPPORTUNITIES, MAX_SHARED_USES, MAX_SLOTS};
    let Some(view) = &own.boons else { return true };
    let short_id = |id: &str| valid_id(id) && id.len() <= 64;
    (1..=MAX_SLOTS).contains(&view.capacity)
        && view.revision > 0
        && view.pending_claim.as_ref().is_none_or(|claim| {
            own.phase == MemberPhase::Active
                && own.ready
                && !own.protected
                && short_id(&claim.opportunity_id)
                && short_id(&claim.boon_id)
                && (1..(1u64 << 53)).contains(&claim.completes_at_ms)
        })
        && (own.phase == MemberPhase::Active
            || (view.slots.is_empty() && view.opportunities.is_empty()))
        && view.slots.len() <= usize::from(view.capacity)
        && view
            .slots
            .windows(2)
            .all(|pair| pair[0].slot < pair[1].slot)
        && unique(
            &view
                .slots
                .iter()
                .map(|slot| &slot.boon_id)
                .collect::<Vec<_>>(),
        )
        && view.slots.iter().all(|slot| {
            (1..=view.capacity).contains(&slot.slot)
                && short_id(&slot.boon_id)
                && (1..=slot.max_level).contains(&slot.level)
                && usize::from(slot.max_level) <= MAX_LEVELS
        })
        && view.opportunities.len() <= MAX_OPPORTUNITIES
        && unique(
            &view
                .opportunities
                .iter()
                .map(|source| &source.id)
                .collect::<Vec<_>>(),
        )
        && view.opportunities.iter().all(|source| {
            short_id(&source.id)
                && source.position.iter().all(|n| n.is_finite())
                && source.shared_remaining.is_none_or(|remaining| {
                    remaining <= MAX_SHARED_USES && (remaining > 0 || !source.available)
                })
                && match &source.offer {
                    BoonOffer::Grant { choices } => {
                        (1..=8).contains(&choices.len())
                            && choices.iter().all(|id| short_id(id))
                            && unique(choices)
                    }
                    BoonOffer::Upgrade {} => true,
                }
        })
}
/// A display replica bound to a match and recipient; apply is atomic on all failures.
pub struct ReplicaV6 {
    key: MatchKey,
    recipient: String,
    last: Option<PublicationEnvelopeV6>,
    snapshot: Option<SnapshotV4>,
    last_full: Option<String>,
    member_conflict: Option<bool>,
}
impl ReplicaV6 {
    pub fn new(key: MatchKey, recipient: String) -> Self {
        Self {
            key,
            recipient,
            last: None,
            snapshot: None,
            last_full: None,
            member_conflict: None,
        }
    }
    pub fn snapshot(&self) -> Option<&SnapshotV4> {
        self.snapshot.as_ref()
    }
    pub fn last_full_id(&self) -> Option<&str> {
        self.last_full.as_deref()
    }
    pub fn forget_baseline(&mut self) {
        self.snapshot = None;
        self.last_full = None;
    }
    pub fn apply(
        &mut self,
        e: &PublicationEnvelopeV6,
    ) -> std::result::Result<&SnapshotV4, DecodeError> {
        if e.key != self.key || e.recipient_id != self.recipient {
            return Err(DecodeError::WrongRecipient);
        }
        if !valid_id(&e.publication_id)
            || !valid_id(&e.connection.id)
            || e.connection.generation == 0
            || e.stream_epoch == 0
            || e.revision == 0
            || e.revision >= (1u64 << 53)
        {
            return Err(DecodeError::InvalidFrame);
        }
        let full = matches!(e.payload, PublicationPayloadV6::Full { .. });
        if let Some(old) = &self.last {
            if e.connection.generation < old.connection.generation
                || e.connection.generation == old.connection.generation
                    && (e.connection != old.connection || e.stream_epoch < old.stream_epoch)
                || e.revision < old.revision
            {
                return Err(DecodeError::StaleFrame);
            }
            if e.revision == old.revision {
                return if e == old {
                    self.snapshot.as_ref().ok_or(DecodeError::BaselineMissing)
                } else {
                    Err(DecodeError::Conflict)
                };
            }
            if e.publication_id == old.publication_id {
                return Err(DecodeError::Conflict);
            }
            if (e.connection != old.connection || e.stream_epoch != old.stream_epoch) && !full {
                return Err(DecodeError::BaselineMissing);
            }
        } else if !full {
            return Err(DecodeError::BaselineMissing);
        }
        let next = match &e.payload {
            PublicationPayloadV6::Full { snapshot, .. } => (**snapshot).clone(),
            PublicationPayloadV6::Delta {
                base_publication_id,
                base_revision,
                delta,
            } => {
                let old = self.last.as_ref().ok_or(DecodeError::BaselineMissing)?;
                if *base_publication_id != old.publication_id || *base_revision != old.revision {
                    return Err(DecodeError::BaselineMissing);
                }
                let mut s = self.snapshot.clone().ok_or(DecodeError::BaselineMissing)?;
                s.logical_ms = delta.logical_ms;
                if let Some(v) = &delta.containers {
                    s.containers = Some(v.clone());
                }
                if let Some(v) = &delta.relics {
                    s.relics = Some(v.clone());
                }
                if let Some(v) = &delta.basic_beams {
                    s.basic_beams = Some(v.clone());
                }
                if let Some(v) = &delta.basic_impacts {
                    s.basic_impacts = Some(v.clone());
                }
                if let Some(v) = &delta.basic_waves {
                    s.basic_waves = Some(v.clone());
                }
                if let Some(v) = &delta.areas {
                    s.areas = Some(v.clone());
                }
                if let Some(v) = &delta.checkpoint_zones {
                    s.checkpoint_zones = Some(v.clone());
                }
                if let Some(v) = &delta.active_blockers {
                    s.active_blockers = Some(v.clone());
                }
                if let Some(v) = &delta.state {
                    s.extraction_pressure = v.extraction_pressure.clone();
                    s.contested_equipment = v.contested_equipment.clone();
                    s.encounter_progress = v.encounter_progress.clone();
                    s.phase = v.phase;
                    s.active_at_ms = v.active_at_ms;
                    s.extraction_point = v.extraction_point;
                    s.extraction_open = v.extraction_open;
                }
                if let Some(v) = &delta.own {
                    s.own = v.clone();
                }
                if let Some(v) = &delta.members {
                    s.members = apply_list(&s.members, v, |x| x.entity.clone())?;
                }
                if let Some(v) = &delta.monsters {
                    s.monsters = apply_list(&s.monsters, v, |x| x.entity.clone())?;
                }
                if let Some(v) = &delta.resources {
                    s.resources = apply_list(&s.resources, v, |x| x.id.clone())?;
                }
                if let Some(v) = &delta.projectiles {
                    s.projectiles = apply_list(&s.projectiles, v, |x| x.id.clone())?;
                }
                s
            }
        };
        if !valid_snapshot(&next, &self.recipient)
            || self
                .member_conflict
                .is_some_and(|enabled| enabled != next.member_conflict)
            || self.snapshot.as_ref().is_some_and(|previous| {
                !valid_equipment_transition(&previous.own, &next.own)
                    || !valid_reservation_transition(&previous.own, &next.own)
                    || !valid_loadout_transition(&previous.own, &next.own)
                    || !valid_channel_transition(&previous.own, &next.own)
                    || !valid_checkpoint_transition(previous, &next)
                    || !valid_mode_transition(previous, &next)
                    || !valid_extraction_transition(previous, &next)
                    || !valid_contested_transition(previous, &next)
                    || !valid_encounter_transition(
                        &previous.encounter_progress,
                        &next.encounter_progress,
                        next.logical_ms,
                    )
                    || !valid_training_transition(&previous.own, &next.own)
                    || !valid_anchor_transition(previous, &next)
                    || !valid_anchor_field_transition(previous, &next)
                    || !valid_boss_transition(previous, &next)
            })
        {
            return Err(DecodeError::InvalidFrame);
        }
        if self
            .snapshot
            .as_ref()
            .is_some_and(|s| next.logical_ms < s.logical_ms)
        {
            return Err(DecodeError::StaleFrame);
        }
        if full {
            self.last_full = Some(e.publication_id.clone());
        }
        self.last = Some(e.clone());
        self.member_conflict = Some(next.member_conflict);
        self.snapshot = Some(next);
        Ok(self.snapshot.as_ref().unwrap())
    }
}

fn valid_encounter_transition(
    a: &Option<EncounterProgressView>,
    b: &Option<EncounterProgressView>,
    now: u64,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.initial == b.initial
                && b.remaining <= a.remaining
                && (!a.surge_started
                    || b.surge_started
                        && b.surge_ends_at_ms
                            .is_none_or(|at| a.surge_ends_at_ms == Some(at)))
                && (b.surge_ends_at_ms.is_some() || a.surge_ends_at_ms.is_none_or(|at| now >= at))
                && (a.remaining != b.remaining
                    || a.surge_ends_at_ms != b.surge_ends_at_ms
                    || a.stats_bps == b.stats_bps)
        }
        _ => false,
    }
}

fn valid_extraction_view(s: &SnapshotV4) -> bool {
    if let Some(p) = &s.extraction_pressure {
        let phase = if s.logical_ms >= p.ends_at_ms {
            ExtractionPressurePhase::Expired
        } else if s.logical_ms >= p.grace_at_ms {
            ExtractionPressurePhase::Grace
        } else {
            ExtractionPressurePhase::Exploring
        };
        let frozen = matches!(s.own.phase, MemberPhase::Settling | MemberPhase::Terminal);
        let phase_valid = if frozen {
            p.phase <= phase
        } else {
            p.phase == phase
        };
        if s.own.mode.is_some()
            || s.own.training.is_some()
            || s.active_at_ms.is_none_or(|at| at >= p.grace_at_ms)
            || p.grace_at_ms >= p.ends_at_ms
            || p.ends_at_ms >= (1u64 << 53)
            || !phase_valid
            || p.warning.as_ref().is_some_and(|w| {
                p.phase != ExtractionPressurePhase::Exploring
                    || !(1..=8).contains(&w.index)
                    || !valid_id(&w.id)
                    || w.id.len() > 64
            })
        {
            return false;
        }
    }
    s.members.iter().all(|m| {
        m.extraction_channel.as_ref().is_none_or(|ch| {
            s.extraction_open
                && m.phase == MemberPhase::Active
                && !m.protected
                && m.hp > 0
                && s.active_at_ms.is_some_and(|at| at <= ch.started_at_ms)
                && ch.started_at_ms <= s.logical_ms
                && s.logical_ms < ch.ends_at_ms
                && ch.ends_at_ms < (1u64 << 53)
                && ch.position.iter().all(|coord| coord.is_finite())
                && (m.entity != s.own.participant.entity
                    || s.own.extraction_finishes_ms == Some(ch.ends_at_ms))
        })
    })
}

fn valid_extraction_transition(a: &SnapshotV4, b: &SnapshotV4) -> bool {
    match (&a.extraction_pressure, &b.extraction_pressure) {
        (None, None) => true,
        (None, Some(_)) => a.active_at_ms.is_none(),
        (Some(a), Some(b)) => {
            a.grace_at_ms == b.grace_at_ms
                && a.ends_at_ms == b.ends_at_ms
                && a.phase <= b.phase
                && (b.phase != ExtractionPressurePhase::Exploring
                    || a.warning.as_ref().is_none_or(|a| {
                        b.warning
                            .as_ref()
                            .is_some_and(|b| a.index < b.index || a == b)
                    }))
        }
        _ => false,
    }
}

fn valid_boss_transition(previous: &SnapshotV4, next: &SnapshotV4) -> bool {
    previous.monsters.iter().all(|before| {
        next.monsters
            .iter()
            .find(|after| after.entity == before.entity)
            .is_none_or(|after| match (&before.boss_phase, &after.boss_phase) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    a.phase_count == b.phase_count && (b.phase_index > a.phase_index || a == b)
                }
                _ => false,
            })
    })
}
