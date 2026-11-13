//! The active channel owns periodic costs and issues the ordinary projectile plan.
use super::*;

impl World {
    pub(super) fn finish_channel(
        &mut self,
        i: usize,
        reason: ChannelEndReason,
        now: u64,
        events: &mut Events,
    ) {
        let member = &mut self.members[i];
        if let Some(channel) = member.channel.take() {
            member.recovery_until = now + channel.skill.recovery_ms;
            member.recovery_movement = channel.skill.cast_movement.unwrap_or_default().recovery;
            member.recovery_cancel_policy = channel.skill.recovery_cancel;
            member.recovery_cancel_at = channel.skill.recovery_cancel.map(|p| now + p.after_ms);
            member.recovery_skill = channel
                .skill
                .recovery_cancel
                .map(|_| channel.skill.id.clone());
            private(
                events,
                i,
                Event::ChannelEnded {
                    action_id: channel.action_id,
                    reason,
                },
            );
        }
    }

    pub(super) fn advance_channel(&mut self, i: usize, c: &Config, now: u64, events: &mut Events) {
        let Some(channel) = self.members[i].channel.as_ref() else {
            return;
        };
        let origin = crate::origin::ActionOrigin::Skill(channel.skill.id.clone());
        let action = channel.action_id.clone();
        let source = self.members[i].reference.entity.clone();
        let prior_origin = self.triggers.origin.replace(origin);
        let prior_source = self.triggers.action_source.replace(source);
        let prior_action = self.triggers.action.replace(action);
        self.advance_channel_inner(i, c, now, events);
        self.triggers.origin = prior_origin;
        self.triggers.action_source = prior_source;
        self.triggers.action = prior_action;
    }
    fn advance_channel_inner(&mut self, i: usize, c: &Config, now: u64, events: &mut Events) {
        let Some(channel) = self.members[i].channel.as_ref() else {
            return;
        };
        let definition = channel
            .skill
            .channel
            .clone()
            .expect("frozen channel definition");
        if (definition.pulse_count.is_none() && now >= channel.ends_at_ms)
            || (definition.pulse_count.is_some()
                && now.saturating_sub(channel.ends_at_ms) >= c.step_ms)
        {
            self.finish_channel(i, ChannelEndReason::Completed, now, events);
            return;
        }
        if !self.members[i].eligible() {
            self.finish_channel(i, ChannelEndReason::SourceInvalid, now, events);
            return;
        }
        if c.mechanism_version >= 105
            && channel.target.is_some()
            && [StatusAttribute::Stun, StatusAttribute::Silence]
                .iter()
                .any(|a| self.member_has_status(i, *a, now))
        {
            self.finish_channel(i, ChannelEndReason::Interrupted, now, events);
            return;
        }
        if let Some(target) = &channel.target
            && let Err(error) = if channel.skill.targets_ally() {
                self.ally_skill_target(i, &channel.skill, target, c)
                    .map(|_| ())
            } else {
                self.unit_aim(i, &channel.skill, target).map(|_| ())
            }
        {
            self.finish_channel(
                i,
                if error == Error::OutOfRange {
                    ChannelEndReason::OutOfRange
                } else {
                    ChannelEndReason::InvalidTarget
                },
                now,
                events,
            );
            return;
        }
        if now < channel.next_pulse_at_ms {
            return;
        }
        let failure = if channel.target.is_none()
            && self.projectile_origin(i, channel.skill.radius).is_err()
        {
            Some(ChannelEndReason::InvalidOrigin)
        } else if self.members[i].spirit < definition.spirit_per_pulse {
            Some(ChannelEndReason::NoResource)
        } else if definition.recovery.is_none()
            && channel.target.is_none()
            && self.projectile_slots_used()
                >= c.member_settings()
                    .expect("member settings")
                    .projectile_capacity
        {
            Some(ChannelEndReason::Capacity)
        } else {
            None
        };
        if let Some(reason) = failure {
            self.finish_channel(i, reason, now, events);
            return;
        }
        let mut channel = self.members[i].channel.take().expect("due channel");
        self.members[i].spirit -= definition.spirit_per_pulse;
        if channel.recovery_budget.is_some() {
            self.channel_recovery(i, &channel, c, now, events);
        } else if let Some(target) = &channel.target {
            let raw = definition.share(channel.raw, channel.pulses);
            self.unit_effect(
                (i, target, &channel.skill, &channel.action_id),
                raw,
                c,
                now,
                events,
            );
            if let (Some(steal), Some(budget)) =
                (channel.skill.resource_steal.as_ref(), channel.steal_budget)
            {
                self.steal_resources(
                    (Owner::Member(i), target, Some(&channel.action_id)),
                    steal,
                    [
                        definition.share(budget[0], channel.pulses),
                        definition.share(budget[1], channel.pulses),
                    ],
                    c,
                    now,
                    events,
                );
            }
            private(
                events,
                i,
                Event::UnitChannelPulse {
                    action_id: channel.action_id.clone(),
                    target: target.clone(),
                    pulse: channel.pulses + 1,
                    spirit_cost: definition.spirit_per_pulse,
                    raw_damage: raw,
                    scheduled_at_ms: channel.next_pulse_at_ms,
                    executed_at_ms: now,
                },
            );
        } else {
            let projectile_id = self
                .spawn_member_shot(
                    i,
                    MemberShot {
                        action_id: Some(&channel.action_id),
                        start_position: None,
                        skill: Some(&channel.skill),
                        direction: channel.direction,
                        target: None,
                        raw: channel.raw,
                    },
                    c,
                    now,
                    events,
                )
                .to_owned();
            private(
                events,
                i,
                Event::ChannelPulse {
                    action_id: channel.action_id.clone(),
                    pulse: channel.pulses + 1,
                    spirit_cost: definition.spirit_per_pulse,
                    projectile_id,
                },
            );
        }
        // A legal tick is counted even when immunity, shields or full resources yield zero.
        // Revalidation and resource failures above never reach this boundary.
        channel.pulses += 1;
        for gate in &definition.on_success {
            if gate.after_pulse == channel.pulses {
                let source = self.members[i].reference.entity.clone();
                self.apply_owned_statuses(
                    Owner::Member(i),
                    (
                        source,
                        self.members[i].base_points(c, StatusAttribute::MaxHp),
                    ),
                    &gate.self_statuses,
                    c,
                    now,
                    events,
                );
                if let Some(target) = &channel.target {
                    let maximum = self.entity_max_hp(target, c, now).unwrap_or(0);
                    self.apply_owned_statuses(
                        Owner::Member(i),
                        (target.clone(), maximum),
                        &gate.target_statuses,
                        c,
                        now,
                        events,
                    );
                }
            }
        }
        channel.next_pulse_at_ms = if definition.pulse_count.is_some() {
            channel.started_at_ms + u64::from(channel.pulses + 1) * definition.interval_ms
        } else {
            now + definition.interval_ms
        };
        let complete = definition.pulse_count == Some(channel.pulses);
        let interrupted = channel.target.is_some()
            && (!self.members[i].eligible()
                || [StatusAttribute::Stun, StatusAttribute::Silence]
                    .iter()
                    .any(|a| self.member_has_status(i, *a, now)));
        let target_lost = !complete
            && channel.target.as_ref().is_some_and(|t| {
                if channel.skill.targets_ally() {
                    self.ally_skill_target(i, &channel.skill, t, c).is_err()
                } else {
                    self.unit_aim(i, &channel.skill, t).is_err()
                }
            });
        self.members[i].channel = Some(channel);
        if interrupted {
            self.finish_channel(
                i,
                if self.members[i].eligible() {
                    ChannelEndReason::Interrupted
                } else {
                    ChannelEndReason::SourceInvalid
                },
                now,
                events,
            );
        } else if target_lost {
            self.finish_channel(i, ChannelEndReason::InvalidTarget, now, events);
        } else if complete {
            self.finish_channel(i, ChannelEndReason::Completed, now, events);
        }
    }

    fn channel_recovery(
        &mut self,
        i: usize,
        channel: &ActiveChannel,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let definition = channel.skill.channel.clone().unwrap();
        let [hp, spirit] = channel.recovery_budget.unwrap();
        let raw_hp = definition.share(hp, channel.pulses);
        let raw_spirit = definition.share(spirit, channel.pulses);
        let j = channel.target.as_ref().map_or(i, |target| {
            match self
                .resolve_entity(target)
                .expect("validated recovery target")
            {
                Owner::Member(j) => j,
                Owner::Npc(_) | Owner::Summon(_) => unreachable!(),
            }
        });
        let entity = self.members[j].reference.entity.clone();
        let target = status_entity(&entity);
        let multiplier_bps = self.statuses.healing_received_bps(
            &target,
            now,
            c.pve.as_ref().and_then(|p| p.status_rules.as_ref()),
        );
        let adjusted_heal = u64::from(raw_hp) * u64::from(multiplier_bps) / 10_000;
        let hp_restored = self.restore_hp(
            &entity,
            adjusted_heal.min(u64::from(u32::MAX)) as u32,
            c,
            now,
        );
        let maximum = self.member_max_spirit(j, c, now);
        self.members[j].spirit = self.members[j].spirit.min(maximum);
        let spirit_restored =
            raw_spirit.min(self.members[j].spirit_recovery_gap(&self.statuses, now, maximum));
        self.members[j].spirit += spirit_restored;
        if definition.recovery.unwrap().hp.is_some() {
            private(
                events,
                i,
                Event::HealResolved {
                    source: status_entity(&self.members[i].reference.entity),
                    target,
                    heal_kind: if i == j {
                        HealKind::SelfChannel
                    } else {
                        HealKind::AllySkill
                    },
                    raw_heal: raw_hp,
                    multiplier_bps,
                    adjusted_heal,
                    hp_restored,
                },
            );
        }
        if i != j && definition.recovery.unwrap().hp.is_some() {
            private(
                events,
                j,
                Event::HealResolved {
                    source: status_entity(&self.members[i].reference.entity),
                    target: status_entity(&entity),
                    heal_kind: HealKind::AllySkill,
                    raw_heal: raw_hp,
                    multiplier_bps,
                    adjusted_heal,
                    hp_restored,
                },
            );
        }
        private(
            events,
            i,
            Event::ChannelRecoveryPulse {
                action_id: channel.action_id.clone(),
                pulse: channel.pulses + 1,
                spirit_cost: definition.spirit_per_pulse,
                raw_hp,
                raw_spirit,
                hp_restored,
                spirit_restored,
                scheduled_at_ms: channel.next_pulse_at_ms,
                executed_at_ms: now,
            },
        );
    }
}
