use super::*;

impl World {
    pub(super) fn start_extraction_channel(
        &mut self,
        i: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let m = &mut self.members[i];
        let ends = now + c.extraction_channel_ms;
        m.extract_end = Some(ends);
        if c.extraction_lifecycle
            .as_ref()
            .is_some_and(|rule| rule.broadcast_channels)
        {
            let channel = ExtractionChannelView {
                position: m.position,
                started_at_ms: now,
                ends_at_ms: ends,
            };
            m.extraction_channel = Some(channel.clone());
            events.push((
                None,
                EventV4::ExtractionChannelChanged(ExtractionChannelChange {
                    entity: m.reference.entity.clone(),
                    channel,
                    state: ExtractionChannelState::Started,
                }),
            ));
        }
        private(events, i, Event::ExtractionStarted);
    }

    pub(super) fn end_extraction_channel(
        &mut self,
        i: usize,
        state: ExtractionChannelState,
        events: &mut Events,
    ) {
        let member = &mut self.members[i];
        if member.extract_end.take().is_none() {
            return;
        }
        if let Some(channel) = member.extraction_channel.take() {
            events.push((
                None,
                EventV4::ExtractionChannelChanged(ExtractionChannelChange {
                    entity: member.reference.entity.clone(),
                    channel,
                    state,
                }),
            ));
        }
        if state == ExtractionChannelState::Interrupted {
            private(events, i, Event::ExtractionInterrupted);
        }
    }

    pub(super) fn extraction_pressure_view(
        &self,
        c: &Config,
        now: u64,
    ) -> Option<ExtractionPressureView> {
        let rule = c.extraction_lifecycle.as_ref()?.pressure.as_ref()?;
        let ends_at_ms = self.active_at? + c.terminal_ms;
        let grace_at_ms = ends_at_ms - rule.grace_ms;
        let phase = if now >= ends_at_ms {
            ExtractionPressurePhase::Expired
        } else if now >= grace_at_ms {
            ExtractionPressurePhase::Grace
        } else {
            ExtractionPressurePhase::Exploring
        };
        let warning = if phase == ExtractionPressurePhase::Exploring {
            rule.warnings
                .iter()
                .enumerate()
                .take_while(|(_, warning)| ends_at_ms - now <= warning.remaining_ms)
                .last()
                .map(|(i, warning)| ExtractionWarningView {
                    id: warning.id.clone(),
                    index: (i + 1) as u8,
                })
        } else {
            None
        };
        Some(ExtractionPressureView {
            grace_at_ms,
            ends_at_ms,
            phase,
            warning,
        })
    }

    pub(super) fn advance_extraction_pressure(
        &mut self,
        c: &Config,
        active: u64,
        now: u64,
        events: &mut Events,
    ) {
        let Some(rule) = c
            .extraction_lifecycle
            .as_ref()
            .and_then(|rule| rule.pressure.as_ref())
        else {
            return;
        };
        let grace = active + c.terminal_ms - rule.grace_ms;
        let pulse = now.saturating_sub(grace) / rule.damage_interval_ms;
        if pulse <= self.extraction_pressure_pulse {
            return;
        }
        self.extraction_pressure_pulse = pulse;
        for i in 0..self.members.len() {
            if self.members[i].living() {
                let entity = self.members[i].reference.entity.clone();
                let max_hp = self.entity_max_hp(&entity, c, now).unwrap();
                let raw =
                    (u64::from(max_hp) * u64::from(rule.damage_max_hp_bps) / 10000).max(1) as u32;
                self.resolve_deadline_damage(entity, raw, c, now, events);
            }
        }
    }

    pub(super) fn force_extraction_deadline(&mut self, events: &mut Events) {
        for i in 0..self.members.len() {
            let m = &mut self.members[i];
            if m.phase != MemberPhase::Active || m.desired.is_some() {
                continue;
            }
            let amount = std::mem::take(&mut m.hp);
            events.push((
                None,
                EventV4::DeadlineDamage {
                    target: m.reference.entity.clone(),
                    amount,
                    absorbed: 0,
                    hp: 0,
                },
            ));
            self.terminal_intent(i, Outcome::Death, events);
        }
        for j in 0..self.npcs.len() {
            let npc = &mut self.npcs[j];
            let amount = std::mem::take(&mut npc.hp);
            let target = npc.entity.clone();
            if amount == 0 {
                self.clear_areas(&target, events);
                continue;
            }
            npc.casting = None;
            npc.target = None;
            self.clear_owned_statuses(&target, StatusRemovalReason::TargetInvalid, events);
            self.clear_areas(&target, events);
            events.push((
                None,
                EventV4::DeadlineDamage {
                    target,
                    amount,
                    absorbed: 0,
                    hp: 0,
                },
            ));
        }
        self.shots.clear();
    }
}
