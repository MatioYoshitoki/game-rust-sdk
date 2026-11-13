//! Fixed and immediate deliveries without a moving collision body.
use super::areas::basic_fields::PendingBasicField;
use super::triggers::ActionKey;
use super::*;
use crate::config::{BasicHitscanFollowup, BasicImpactConfig, BasicImpactTiming, BasicRefraction};

struct ImpactLeg {
    target: EntityRef,
    origin: [f64; 2],
    raw: u32,
    role: BasicImpactSegmentRole,
}

struct ImpactAnchor<'a> {
    source: &'a EntityRef,
    primary: &'a EntityRef,
    center: [f64; 2],
}

pub(super) struct BasicImpactPlan {
    pub(super) view: BasicImpactView,
    pub(super) action: ActionKey,
    delivery: ImpactDelivery,
}

enum ImpactDelivery {
    Unit {
        primary: ImpactLeg,
        followups: Vec<ImpactLeg>,
        segmented: bool,
    },
    Ground(PendingBasicField),
}

impl BasicImpactPlan {
    pub(super) fn pending_field(&self) -> Option<(&EntityRef, &PendingBasicField)> {
        match &self.delivery {
            ImpactDelivery::Ground(field) => Some((&self.view.source, field)),
            ImpactDelivery::Unit { .. } => None,
        }
    }

    fn end(
        &self,
        reason: BasicImpactEndReason,
        segments: Option<Vec<BasicImpactSegment>>,
        events: &mut Events,
    ) {
        events.push((
            None,
            EventV4::BasicImpactEnded {
                impact_id: self.view.id.clone(),
                reason,
                segments,
            },
        ));
    }
}

fn alternating_legs(
    anchor: &ImpactAnchor<'_>,
    candidates: &[(&Npc, f64)],
    max_contacts: usize,
    (mut primary_bps, secondary_bps, decay_bps, minimum_bps): (u32, u32, u32, u32),
    budget: impl Fn(u32) -> u32,
) -> Vec<ImpactLeg> {
    let decay = |bps: u32| (bps * decay_bps / 10000).max(minimum_bps);
    let mut next_bps = BTreeMap::new();
    let mut legs = Vec::with_capacity(max_contacts - 1);
    for (candidate, _) in candidates.iter().cycle().take(max_contacts / 2) {
        let bps = next_bps
            .entry(candidate.entity.id.as_str())
            .or_insert(secondary_bps);
        legs.push(ImpactLeg {
            target: candidate.entity.clone(),
            origin: anchor.center,
            raw: budget(*bps),
            role: BasicImpactSegmentRole::Secondary,
        });
        *bps = decay(*bps);
        if legs.len() + 1 < max_contacts {
            primary_bps = decay(primary_bps);
            legs.push(ImpactLeg {
                target: anchor.primary.clone(),
                origin: candidate.position,
                raw: budget(primary_bps),
                role: BasicImpactSegmentRole::Return,
            });
        }
    }
    legs
}

impl World {
    fn impact_neighbors(&self, anchor: &ImpactAnchor<'_>, radius: f64) -> Vec<(&Npc, f64)> {
        let mut candidates = self
            .npcs
            .iter()
            .filter_map(|npc| {
                let range = distance(anchor.center, npc.position);
                (npc.entity != *anchor.primary
                    && range <= radius
                    && self.hostile_contact(anchor.source, &npc.entity)
                    && self
                        .grid
                        .piercing_wall(anchor.center, npc.position, 0.)
                        .is_none())
                .then_some((npc, range))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.entity.id.cmp(&b.0.entity.id)));
        candidates
    }

    fn hitscan_legs(
        &self,
        anchor: &ImpactAnchor<'_>,
        (refraction, followup): (Option<&BasicRefraction>, Option<&BasicHitscanFollowup>),
        primary_bps: u32,
        budget: impl Fn(u32) -> u32,
    ) -> Vec<ImpactLeg> {
        match followup {
            Some(BasicHitscanFollowup::AlternatingRefraction {
                radius,
                damage_bps,
                max_contacts,
                repeat_decay_bps,
                minimum_damage_bps,
            }) => alternating_legs(
                anchor,
                &self.impact_neighbors(anchor, *radius),
                *max_contacts,
                (
                    primary_bps,
                    *damage_bps,
                    *repeat_decay_bps,
                    *minimum_damage_bps,
                ),
                budget,
            ),
            Some(BasicHitscanFollowup::TargetBurst {
                radius,
                damage_bps,
                max_targets,
            }) => self
                .impact_neighbors(anchor, *radius)
                .into_iter()
                .take(max_targets - 1)
                .map(|(npc, _)| ImpactLeg {
                    target: npc.entity.clone(),
                    origin: anchor.center,
                    raw: budget(*damage_bps),
                    role: BasicImpactSegmentRole::Blast,
                })
                .collect(),
            None => refraction
                .into_iter()
                .filter_map(|rule| {
                    let neighbors = self.impact_neighbors(anchor, rule.radius);
                    neighbors.first().map(|(npc, _)| ImpactLeg {
                        target: npc.entity.clone(),
                        origin: anchor.center,
                        raw: budget(rule.damage_bps),
                        role: BasicImpactSegmentRole::Secondary,
                    })
                })
                .collect(),
        }
    }

    pub(super) fn plan_basic_impact(
        &self,
        i: usize,
        pending: &Pending,
        impact: &BasicImpactConfig,
        c: &Config,
        now: u64,
    ) -> Result<BasicImpactPlan> {
        let member = &self.members[i];
        let source = &member.reference.entity;
        if let BasicImpactTiming::GroundField { delay_ms, field } = &impact.timing {
            let position = pending.ground.ok_or(Error::InvalidTarget)?;
            let field = self.reserve_basic_field(i, field, c, now)?;
            return Ok(BasicImpactPlan {
                view: BasicImpactView {
                    id: String::new(),
                    plan_id: impact.plan_id.clone(),
                    source: source.clone(),
                    kind: BasicImpactKind::GroundField,
                    origin: member.position,
                    target: None,
                    position: Some(position),
                    released_at_ms: now,
                    impact_at_ms: now + delay_ms,
                },
                action: ActionKey::new(source, &pending.id),
                delivery: ImpactDelivery::Ground(field),
            });
        }
        let primary = pending.target.as_ref().ok_or(Error::InvalidTarget)?;
        let center = self.basic_target_position(i, primary)?;
        let rule = &c.combat_rules.as_ref().unwrap().basic_attack;
        let attack = self.stats(i, c, now).0;
        let budget = |bps| rule.budget(attack, bps, 10000, 0);
        let (kind, impact_at_ms, followups, segmented) = match &impact.timing {
            BasicImpactTiming::Hitscan {
                refraction,
                followup,
            } => (
                BasicImpactKind::Hitscan,
                now,
                self.hitscan_legs(
                    &ImpactAnchor {
                        source,
                        primary,
                        center,
                    },
                    (refraction.as_ref(), followup.as_ref()),
                    impact.damage_bps,
                    budget,
                ),
                followup.is_some(),
            ),
            BasicImpactTiming::FixedDelay { delay_ms } => {
                (BasicImpactKind::FixedDelay, now + delay_ms, vec![], false)
            }
            BasicImpactTiming::GroundField { .. } => unreachable!(),
        };
        Ok(BasicImpactPlan {
            view: BasicImpactView {
                id: String::new(),
                plan_id: impact.plan_id.clone(),
                source: source.clone(),
                kind,
                origin: member.position,
                target: Some(Self::unit_wire(primary)),
                position: None,
                released_at_ms: now,
                impact_at_ms,
            },
            action: ActionKey::new(source, &pending.id),
            delivery: ImpactDelivery::Unit {
                primary: ImpactLeg {
                    target: primary.clone(),
                    origin: member.position,
                    raw: budget(impact.damage_bps),
                    role: BasicImpactSegmentRole::Primary,
                },
                followups,
                segmented,
            },
        })
    }

    pub(super) fn start_basic_impact(
        &mut self,
        i: usize,
        mut plan: BasicImpactPlan,
        release_status: Option<&String>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        plan.view.id = self.next_id("basic-impact");
        events.push((
            None,
            EventV4::BasicImpactStarted {
                impact: plan.view.clone(),
            },
        ));
        self.apply_basic_self_status(i, release_status, c, now, events);
        if plan.view.kind == BasicImpactKind::Hitscan {
            self.resolve_basic_impact(plan, c, now, events);
        } else {
            self.basic_impacts.push(plan);
        }
    }

    pub(super) fn clear_basic_impacts(&mut self, source: &EntityRef, events: &mut Events) {
        self.basic_impacts.retain(|plan| {
            if plan.view.source == *source {
                plan.end(BasicImpactEndReason::SourceInvalid, None, events);
                false
            } else {
                true
            }
        });
    }

    pub(super) fn advance_basic_impacts(&mut self, c: &Config, now: u64, events: &mut Events) {
        for plan in std::mem::take(&mut self.basic_impacts) {
            if plan.view.impact_at_ms <= now {
                self.resolve_basic_impact(plan, c, now, events);
            } else if self
                .resolve_entity(&plan.view.source)
                .is_some_and(|owner| matches!(owner, Owner::Member(i) if self.members[i].living()))
            {
                self.basic_impacts.push(plan);
            } else {
                plan.end(BasicImpactEndReason::SourceInvalid, None, events);
            }
        }
    }

    fn resolve_basic_impact(
        &mut self,
        plan: BasicImpactPlan,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let source = &plan.view.source;
        let mut segments = matches!(
            plan.delivery,
            ImpactDelivery::Unit {
                segmented: true,
                ..
            }
        )
        .then(Vec::new);
        let Some(Owner::Member(i)) = self.resolve_entity(source) else {
            plan.end(BasicImpactEndReason::SourceInvalid, segments, events);
            return;
        };
        if !self.members[i].living() {
            plan.end(BasicImpactEndReason::SourceInvalid, segments, events);
            return;
        }
        if matches!(plan.delivery, ImpactDelivery::Ground(_)) {
            plan.end(BasicImpactEndReason::Resolved, None, events);
        }
        let (primary, followups) = match plan.delivery {
            ImpactDelivery::Unit {
                ref primary,
                ref followups,
                ..
            } => (primary, followups),
            ImpactDelivery::Ground(field) => {
                self.start_basic_field(
                    i,
                    field,
                    &plan.action.id,
                    plan.view.position.unwrap(),
                    now,
                    events,
                );
                return;
            }
        };
        let Ok(position) = self.basic_target_position(i, &primary.target) else {
            plan.end(BasicImpactEndReason::TargetInvalid, segments, events);
            return;
        };
        if plan.view.kind == BasicImpactKind::FixedDelay
            && self
                .grid
                .piercing_wall(plan.view.origin, position, 0.)
                .is_some()
        {
            plan.end(BasicImpactEndReason::TargetInvalid, segments, events);
            return;
        }

        if self.basic_impact_contact((i, &plan, &mut segments), primary, c, now, events) {
            let mut secondary_entered = false;
            for leg in followups {
                if !self.members[i].living() {
                    break;
                }
                let eligible = self.hostile_contact(source, &leg.target);
                match leg.role {
                    BasicImpactSegmentRole::Return if !eligible => break,
                    BasicImpactSegmentRole::Return if !secondary_entered => continue,
                    BasicImpactSegmentRole::Secondary => secondary_entered = eligible,
                    _ => (),
                }
                if eligible {
                    self.basic_impact_contact((i, &plan, &mut segments), leg, c, now, events);
                }
            }
        }
        let reason = if self.members[i].living() {
            BasicImpactEndReason::Resolved
        } else {
            BasicImpactEndReason::SourceInvalid
        };
        plan.end(reason, segments, events);
    }

    pub(super) fn periodic_death_burst(
        &mut self,
        status: &StatusView,
        attack: u32,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let source = Self::status_reference(&status.source);
        let dead = Self::status_reference(&status.target);
        let (Some(Owner::Member(i)), Some(Owner::Npc(j)), Some(stacks)) = (
            self.resolve_entity(&source),
            self.resolve_entity(&dead),
            status.stacks,
        ) else {
            return;
        };
        let Some(burst) = self.members[i]
            .basic(c)
            .impact
            .as_ref()
            .and_then(|rule| rule.contact_status.as_ref())
            .filter(|contact| contact.status_id == status.definition_id)
            .and_then(|contact| contact.death_burst.as_ref())
        else {
            return;
        };
        let center = self.npcs[j].position;
        let attack_rule = &c.combat_rules.as_ref().unwrap().periodic_damage[&status.definition_id];
        let raw = attack_rule.budget_layers(attack, burst.damage_bps_per_stack, 10000, 0, stacks);
        let targets: Vec<_> = self
            .impact_neighbors(
                &ImpactAnchor {
                    source: &source,
                    primary: &dead,
                    center,
                },
                burst.radius,
            )
            .into_iter()
            .take(burst.target_cap)
            .map(|(npc, _)| npc.entity.clone())
            .collect();
        let mut defs = Self::definitions(c, std::slice::from_ref(&status.definition_id));
        defs[0].periodic_added_stacks = (stacks * burst.inherited_stack_bps).div_ceil(10000);
        for target in targets {
            if !self.members[i].living() {
                break;
            }
            let Some(owner) = self.resolve_entity(&target) else {
                continue;
            };
            if !self.hostile_contact(&source, &target)
                || self.entity_has_status(owner, StatusAttribute::DamageImmunity, now)
            {
                continue;
            }
            self.resolve_basic_field_damage(
                combat::DamageIntent {
                    source: source.clone(),
                    target: target.clone(),
                    raw,
                    direct: false,
                    ignore_armor: false,
                },
                attack_rule,
                c,
                now,
                events,
            );
            if self.hostile_contact(&source, &target) {
                self.apply_owned_status_definitions(
                    Owner::Member(i),
                    target,
                    &defs,
                    c,
                    now,
                    events,
                );
            }
        }
    }

    fn basic_impact_contact(
        &mut self,
        (i, plan, segments): (
            usize,
            &BasicImpactPlan,
            &mut Option<Vec<BasicImpactSegment>>,
        ),
        leg: &ImpactLeg,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        let owner = self.resolve_entity(&leg.target).expect("validated target");
        let segment = segments.as_mut().map(|trace| {
            let segment = BasicImpactSegment {
                index: trace.len() as u8,
                role: leg.role,
                origin: leg.origin,
                target: Self::unit_wire(&leg.target),
            };
            trace.push(segment.clone());
            segment
        });
        events.push((
            None,
            EventV4::BasicImpactContact {
                impact_id: plan.view.id.clone(),
                source: plan.view.source.clone(),
                target: leg.target.clone(),
                scheduled_at_ms: plan.view.impact_at_ms,
                segment,
            },
        ));
        if let Some(status) = self.members[i]
            .basic(c)
            .impact
            .as_ref()
            .and_then(|rule| rule.contact_status.as_ref())
        {
            let basic = self.members[i].basic(c);
            let rule = &c.combat_rules.as_ref().unwrap().basic_attack;
            if !self.direct_contact_eligible(
                Owner::Member(i),
                owner,
                &plan.view.source,
                (basic.unavoidable == Some(true) || rule.unavoidable, false),
                c,
                now,
            ) {
                return false;
            }
            let defs = Self::definitions(c, std::slice::from_ref(&status.status_id));
            self.apply_owned_status_definitions(
                Owner::Member(i),
                leg.target.clone(),
                &defs,
                c,
                now,
                events,
            );
            self.basic_contact(i, Some(&plan.action.id), None, c, now);
            return true;
        }
        self.resolve_native_basic_contact(
            i,
            owner,
            (&plan.view.source, &plan.action.id, leg.raw),
            c,
            now,
            events,
        )
    }
}

#[cfg(all(test, match_sdk_test_data))]
impl World {
    pub(crate) fn set_basic_test_npc_hp(&mut self, index: usize, hp: u32) {
        self.npcs[index].hp = hp;
    }

    pub(crate) fn basic_test_npc_heal(&self, index: usize, c: &Config, now: u64) -> u64 {
        self.adjusted_heal(&status_entity(&self.npcs[index].entity), 10000, c, now)
    }
}
