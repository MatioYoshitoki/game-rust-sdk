//! Finite native derived contacts. No delayed jobs or recursive triggers.
use super::*;
use crate::config::ChainDamageRule;
impl World {
    fn chain_start(&self, actors: (Owner, Owner)) -> Result<(EntityRef, EntityRef, [f64; 2])> {
        let source = self.owner_entity(actors.0);
        let origin = self.owner_entity(actors.1);
        let alive = self.current_hp(actors.1) > 0;
        if !self.valid_status_entity(&status_entity(&source))
            || self.relation(&source, &origin) != Some(combat::Relation::Hostile)
            || alive && !self.valid_status_entity(&status_entity(&origin))
        {
            return Err(Error::InvalidTarget);
        }
        Ok((source, origin, self.actor_position(actors.1)))
    }
    fn chain_raw(
        &self,
        statuses: &Statuses,
        owner: Owner,
        amount: u64,
        rule: &ChainDamageRule,
        c: &Config,
        now: u64,
    ) -> u32 {
        let source = status_entity(&self.owner_entity(owner));
        let base = self.base_attack(owner, c);
        let attack = rule.source_skill_id.as_ref().map_or_else(
            || {
                statuses.attribute(
                    &source,
                    base,
                    now,
                    c.pve.as_ref().unwrap().status_rules.as_ref(),
                    StatusAttribute::Atk,
                )
            },
            |id| self.selected_power_with(statuses, owner, id, c, now),
        );
        (u128::from(rule.formula.fixed)
            + (u128::from(attack) * u128::from(rule.formula.source_attack_bps)
                + u128::from(amount) * u128::from(rule.formula.fact_amount_bps))
                / 10000)
            .min(u128::from(u32::MAX)) as u32
    }
    fn next_chain_target(
        &self,
        source: &EntityRef,
        point: [f64; 2],
        visited: &[EntityRef],
        rule: &ChainDamageRule,
        previous: [f64; 2],
    ) -> Option<(Owner, EntityRef, [f64; 2])> {
        let mut candidates = self
            .members
            .iter()
            .enumerate()
            .map(|(i, m)| (Owner::Member(i), &m.reference.entity, m.position))
            .chain(
                self.npcs
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (Owner::Npc(i), &n.entity, n.position)),
            )
            .filter(|(_, e, p)| {
                !visited.contains(e)
                    && self.hostile_contact(source, e)
                    && distance(point, *p) <= rule.radius
                    && rule.direction.allows(previous, point, *p)
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            distance(point, a.2)
                .total_cmp(&distance(point, b.2))
                .then(
                    u8::from(a.1.kind == StatusEntityKind::Monster)
                        .cmp(&u8::from(b.1.kind == StatusEntityKind::Monster)),
                )
                .then(a.1.id.cmp(&b.1.id))
                .then(a.1.generation.cmp(&b.1.generation))
        });
        candidates
            .into_iter()
            .find(|(_, _, p)| self.grid.line(point, *p, 0., 2))
            .map(|(o, e, p)| (o, e.clone(), p))
    }
    pub(super) fn preview_chain_damage(
        &self,
        preview: &Statuses,
        actors: (Owner, Owner),
        amount: u64,
        rule: &ChainDamageRule,
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let (source, origin, point) = self.chain_start(actors)?;
        if self.triggers.chain_used >= c.combat_rules.as_ref().unwrap().max_chain_hits_per_step {
            return Err(Error::Capacity);
        }
        if self.chain_raw(preview, actors.0, amount, rule, c, now) == 0 {
            return Err(Error::InvalidState);
        }
        if self
            .next_chain_target(
                &source,
                point,
                &[source.clone(), origin],
                rule,
                self.actor_position(actors.0),
            )
            .is_none()
        {
            return Err(Error::InvalidTarget);
        }
        Ok(())
    }
    pub(super) fn execute_chain_damage(
        &mut self,
        input: (&str, (Owner, Owner), u64),
        rule: &ChainDamageRule,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let (trigger_id, actors, amount) = input;
        let Ok((source, origin, mut point)) = self.chain_start(actors) else {
            return;
        };
        let initial_raw = self.chain_raw(&self.statuses, actors.0, amount, rule, c, now);
        let mut raw = initial_raw;
        let mut previous_point = self.actor_position(actors.0);
        let mut from = origin.clone();
        let mut visited = vec![source.clone(), origin.clone()];
        let mut hops = vec![];
        let mut reason = ChainEndReason::Complete;
        let attack = crate::config::AttackRule {
            damage_kind: rule.damage_kind,
            element: rule.element.clone(),
            ignore_armor: rule.ignore_armor,
            armor_penetration_cap_bps: Some(rule.armor_penetration_cap_bps),
            unavoidable: true,
            can_crit: false,
            ..Default::default()
        };
        let previous = self.triggers.executing;
        self.triggers.executing = true;
        for hop in 1..=rule.max_hops {
            if !self.valid_status_entity(&status_entity(&source)) {
                reason = ChainEndReason::SourceInvalid;
                break;
            }
            let current = if hop == rule.max_hops {
                (u128::from(raw) * u128::from(rule.final_multiplier_bps) / 10000)
                    .min(u128::from(u32::MAX)) as u32
            } else {
                raw
            };
            if current == 0 {
                reason = ChainEndReason::ZeroBudget;
                break;
            }
            if self.triggers.chain_used >= c.combat_rules.as_ref().unwrap().max_chain_hits_per_step
            {
                reason = ChainEndReason::Capacity;
                break;
            }
            let Some((target, entity, next_point)) =
                self.next_chain_target(&source, point, &visited, rule, previous_point)
            else {
                reason = ChainEndReason::NoTarget;
                break;
            };
            visited.push(entity.clone());
            self.triggers.chain_used += 1;
            let applied = self.direct_contact(
                actors.0,
                target,
                combat::DirectContact {
                    origin: None,
                    action_id: None,
                    rule: Some(&attack),
                    ignore_armor: rule.ignore_armor,
                    unavoidable: true,
                    dead_source: false,
                    source: &source,
                    raw: current,
                    metal: true,
                    hit_statuses: &rule.on_hit_target_statuses,
                },
                c,
                now,
                events,
            );
            hops.push(ChainHopView {
                from: status_entity(&from),
                target: status_entity(&entity),
                raw_damage: current,
                applied,
            });
            previous_point = point;
            point = next_point;
            from = entity;
            raw = (u64::from(raw) * u64::from(rule.retention_bps) / 10000) as u32;
        }
        self.triggers.executing = previous;
        for i in self.status_observers(&status_entity(&source), &status_entity(&source)) {
            private(
                events,
                i,
                Event::ChainDamageResolved {
                    trigger_id: trigger_id.into(),
                    source: status_entity(&source),
                    origin: status_entity(&origin),
                    initial_raw,
                    hops: hops.clone(),
                    reason,
                },
            );
        }
    }
}
