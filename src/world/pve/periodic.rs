use super::*;
impl Arena {
    fn periodic_returning(&self, entity: &StatusEntity) -> bool {
        self.status_monster_index(entity).is_some_and(|i| {
            let m = &self.monsters[i];
            m.returning
                || m.encounter_index
                    .is_some_and(|j| self.encounters[j].reset_at.is_some())
        })
    }

    pub(super) fn advance_periodic(
        &mut self,
        w: &mut World,
        c: &Config,
        now: u64,
        events: &mut Vec<Event>,
    ) {
        if w.hp == 0 || w.desired_outcome.is_some() {
            return;
        }
        while let Some((status, due)) = self.statuses.take_periodic_due(now) {
            if !self.status_source_valid(w, &status.source)
                || !self.status_source_valid(w, &status.target)
            {
                continue;
            }
            let plan = status.periodic.as_ref().unwrap();
            let outcome = if w.protected || !w.foreground || !w.ready {
                PeriodicTickOutcome::Protected
            } else if self.periodic_returning(&status.source)
                || self.periodic_returning(&status.target)
            {
                PeriodicTickOutcome::Returning
            } else {
                PeriodicTickOutcome::Applied
            };
            if plan.kind == PeriodicEffectKind::MaxHpHeal {
                let requested = plan.raw_heal.unwrap();
                let hp_restored = if outcome != PeriodicTickOutcome::Applied {
                    0
                } else {
                    let amount = self.resolve_heal(
                        w,
                        c,
                        (&status.source, &status.target),
                        (HealKind::Hot, requested),
                        now,
                        events,
                    );
                    if status.target.kind == StatusEntityKind::Participant {
                        w.hp += amount;
                    } else if let Some(i) = self.status_monster_index(&status.target) {
                        self.monsters[i].hp += amount;
                    }
                    amount
                };
                events.push(Event::PeriodicHealTick {
                    stacks: None,
                    instance_id: status.instance_id,
                    definition_id: status.definition_id,
                    source: status.source,
                    target: status.target,
                    tick_index: plan.ticks_done,
                    scheduled_at_ms: due,
                    executed_at_ms: now,
                    raw_heal: requested,
                    hp_restored,
                    outcome,
                });
                continue;
            }
            let start = events.len();
            let mut hp_damage = 0;
            if outcome == PeriodicTickOutcome::Applied {
                if status.target.kind == StatusEntityKind::Participant {
                    let index = self.status_monster_index(&status.source);
                    let before = w.hp;
                    self.apply_player_damage(
                        w,
                        c,
                        (&status.source.id, index),
                        (plan.raw_damage, false, true),
                        now,
                        events,
                    );
                    hp_damage = before - w.hp;
                } else if let Some(i) = self.status_monster_index(&status.target) {
                    hp_damage = self.apply_monster_damage(
                        w,
                        c,
                        i,
                        (plan.raw_damage, false, None, "participant"),
                        events,
                    );
                }
            }
            let absorbed = events[start..]
                .iter()
                .filter_map(|e| match e {
                    Event::ShieldAbsorbed { amount, .. } => Some(*amount),
                    _ => None,
                })
                .sum();
            events.push(Event::PeriodicTick {
                stacks: None,
                instance_id: status.instance_id,
                definition_id: status.definition_id,
                source: status.source,
                target: status.target,
                tick_index: plan.ticks_done,
                scheduled_at_ms: due,
                executed_at_ms: now,
                raw_damage: plan.raw_damage,
                absorbed,
                hp_damage,
                outcome,
            });
            if w.hp == 0 || w.desired_outcome.is_some() {
                break;
            }
        }
    }
}
