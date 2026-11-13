//! Ordinary-attack work clocks. Status mutations first account for elapsed work.
use super::*;
#[derive(Clone)]
pub(super) struct AttackCooldown {
    last_at: u64,
    remaining: u128,
    credit: u128,
    denominator: u128,
    speed_bonus_bps: u32,
}
impl AttackCooldown {
    pub(super) fn new(now: u64, interval: u64) -> Self {
        Self {
            last_at: now,
            remaining: u128::from(interval) * 10000,
            credit: 0,
            denominator: 10000,
            speed_bonus_bps: 0,
        }
    }
    fn expiries(statuses: &Statuses, target: &StatusEntity, after: u64) -> Vec<u64> {
        let affects = |a| {
            matches!(
                a,
                StatusAttribute::AttackSpeed | StatusAttribute::BasicIntervalDelay
            )
        };
        let mut points = statuses
            .entries()
            .iter()
            .filter(|s| {
                s.target == *target
                    && s.expires_at_ms > after
                    && (s.attribute.is_some_and(affects)
                        || s.modifiers.iter().any(|m| affects(m.attribute)))
            })
            .map(|s| s.expires_at_ms)
            .collect::<Vec<_>>();
        points.sort_unstable();
        points.dedup();
        points
    }
    fn rate(
        &self,
        statuses: &Statuses,
        target: &StatusEntity,
        rules: Option<&crate::config::StatusRules>,
        at: u64,
    ) -> (u128, u128) {
        (
            u128::from(statuses.attribute_with_bonus(
                target,
                10000,
                at,
                rules,
                StatusAttribute::AttackSpeed,
                i64::from(self.speed_bonus_bps),
            )) * 10000,
            u128::from(statuses.attribute(
                target,
                10000,
                at,
                rules,
                StatusAttribute::BasicIntervalDelay,
            )),
        )
    }
    fn use_denominator(&mut self, denominator: u128) {
        if self.denominator != denominator {
            self.credit = self.credit * denominator / self.denominator;
            self.denominator = denominator;
        }
    }
    fn wait_time(&mut self, (numerator, denominator): (u128, u128)) -> Option<u64> {
        self.use_denominator(denominator);
        if self.remaining == 0 {
            return Some(0);
        }
        (numerator > 0).then(|| {
            ((self.remaining * denominator - self.credit).div_ceil(numerator))
                .min(u128::from(u64::MAX)) as u64
        })
    }
    fn consume(&mut self, elapsed: u64, (numerator, denominator): (u128, u128)) {
        self.use_denominator(denominator);
        let total = u128::from(elapsed) * numerator + self.credit;
        self.remaining = self.remaining.saturating_sub(total / denominator);
        self.credit = if self.remaining == 0 {
            0
        } else {
            total % denominator
        };
    }
    pub(super) fn deadline(
        &self,
        statuses: &Statuses,
        target: &StatusEntity,
        rules: Option<&crate::config::StatusRules>,
    ) -> u64 {
        let mut projected = self.clone();
        let mut at = self.last_at;
        for end in Self::expiries(statuses, target, at) {
            let rate = projected.rate(statuses, target, rules, at);
            if let Some(wait) = projected.wait_time(rate)
                && wait <= end - at
            {
                return at.saturating_add(wait);
            }
            projected.consume(end - at, rate);
            at = end;
        }
        let rate = projected.rate(statuses, target, rules, at);
        at.saturating_add(projected.wait_time(rate).unwrap_or(u64::MAX))
    }
    fn advance(
        &mut self,
        statuses: &Statuses,
        target: &StatusEntity,
        rules: Option<&crate::config::StatusRules>,
        now: u64,
    ) {
        if now <= self.last_at {
            return;
        }
        let mut at = self.last_at;
        for end in Self::expiries(statuses, target, at)
            .into_iter()
            .filter(|t| *t < now)
            .chain(std::iter::once(now))
        {
            self.consume(end - at, self.rate(statuses, target, rules, at));
            at = end;
        }
        self.last_at = now;
    }
}
impl World {
    pub(super) fn advance_attack_clocks(&mut self, c: &Config, now: u64) {
        if c.combat_rules.is_none() {
            return;
        }
        let rules = c.pve.as_ref().unwrap().status_rules.as_ref();
        for m in &mut self.members {
            if let Some(clock) = &mut m.attack_clock {
                let target = status_entity(&m.reference.entity);
                let ready = clock.deadline(&self.statuses, &target, rules);
                clock.advance(&self.statuses, &target, rules, now);
                if clock.remaining == 0 {
                    m.attack_clock = None;
                }
                Self::update_basic_deadline(m, ready, c, now);
            }
        }
        for n in &mut self.npcs {
            if let Some(clock) = &mut n.attack_clock {
                let target = status_entity(&n.entity);
                n.next_attack = clock.deadline(&self.statuses, &target, rules);
                clock.advance(&self.statuses, &target, rules, now);
                if clock.remaining == 0 {
                    n.attack_clock = None;
                }
            }
        }
        for summon in &mut self.summons {
            if let Some(clock) = &mut summon.attack_clock {
                let target = status_entity(&summon.entity);
                summon.next_attack = clock.deadline(&self.statuses, &target, rules);
                clock.advance(&self.statuses, &target, rules, now);
                if clock.remaining == 0 {
                    summon.attack_clock = None;
                }
            }
        }
    }
    fn update_basic_deadline(member: &mut Member, ready: u64, c: &Config, now: u64) {
        if ready == member.basic_ready {
            return;
        }
        member.basic_ready = ready;
        if let Some(q) = &mut member.queued_basic {
            q.promotes_at_ms =
                now.saturating_add(ready.saturating_sub(now).div_ceil(c.step_ms) * c.step_ms);
            q.expires_at_ms =
                q.promotes_at_ms + c.basic_followup.expect("configured basic queue").grace_ms;
        }
    }
    pub(super) fn start_attack_clock(&mut self, owner: Owner, interval: u64, c: &Config, now: u64) {
        let clock = c.combat_rules.as_ref().map(|_| {
            let mut clock = AttackCooldown::new(now, interval);
            if let Owner::Member(i) = owner {
                clock.speed_bonus_bps = self.members[i]
                    .projectile_progress(c)
                    .map_or(0, |progress| progress.attack_speed_bonus_bps);
            }
            clock
        });
        let ready = clock.as_ref().map_or(now + interval, |clock| {
            clock.deadline(
                &self.statuses,
                &status_entity(&self.owner_entity(owner)),
                c.pve.as_ref().unwrap().status_rules.as_ref(),
            )
        });
        match owner {
            Owner::Member(i) => {
                self.members[i].attack_clock = clock;
                Self::update_basic_deadline(&mut self.members[i], ready, c, now);
            }
            Owner::Npc(j) => {
                self.npcs[j].attack_clock = clock;
                self.npcs[j].next_attack = ready;
            }
            Owner::Summon(j) => {
                self.summons[j].attack_clock = clock;
                self.summons[j].next_attack = ready;
            }
        }
    }
    pub(super) fn member_basic_deadline(&self, i: usize, c: &Config) -> u64 {
        let m = &self.members[i];
        m.attack_clock
            .as_ref()
            .filter(|_| c.combat_rules.is_some())
            .map_or(m.basic_ready, |clock| {
                clock.deadline(
                    &self.statuses,
                    &status_entity(&m.reference.entity),
                    c.pve.as_ref().unwrap().status_rules.as_ref(),
                )
            })
    }
}
