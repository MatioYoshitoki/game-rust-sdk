use super::{combat::DirectContact, *};
use crate::config::member_summons::{SummonCast, SummonOwnerBonus};

pub(super) struct Summon {
    pub entity: EntityRef,
    pub owner: EntityRef,
    pub member: usize,
    pub template_id: String,
    pub profile: Arc<MonsterTemplate>,
    pub cast_skill_id: String,
    pub base_hp: u32,
    pub base_atk: u32,
    pub base_def: u32,
    pub death_damage: u32,
    owner_bonus: Option<SummonOwnerBonus>,
    pub position: [f64; 2],
    pub hp: u32,
    pub expires_at_ms: u64,
    pub ready_after_spawn: u64,
    pub copy: summon_copy::CopyState,
    pub next_attack: u64,
    pub attack_clock: Option<attack_speed::AttackCooldown>,
    removed: bool,
}

impl World {
    pub(super) fn summon_owner_bonus(&self, owner: Owner, now: u64) -> SummonOwnerBonus {
        let Owner::Member(member) = owner else {
            return SummonOwnerBonus::default();
        };
        self.summons
            .iter()
            .enumerate()
            .filter_map(|(i, summon)| {
                (summon.member == member && now < summon.expires_at_ms && self.summon_alive(i))
                    .then_some(summon.owner_bonus)
                    .flatten()
            })
            .fold(SummonOwnerBonus::default(), |a, b| SummonOwnerBonus {
                damage_bonus_bps: a.damage_bonus_bps.max(b.damage_bonus_bps),
                damage_reduction_bps: a.damage_reduction_bps.max(b.damage_reduction_bps),
            })
    }

    pub(super) fn summon_alive(&self, index: usize) -> bool {
        let summon = &self.summons[index];
        let member = &self.members[summon.member];
        !summon.removed
            && summon.hp > 0
            && member.reference.entity == summon.owner
            && member.living()
    }

    pub(super) fn summon_eligible(&self, index: usize) -> bool {
        self.summon_alive(index) && self.members[self.summons[index].member].eligible()
    }

    pub(super) fn plan_summons(
        &self,
        member: usize,
        cast: &SummonCast,
        c: &Config,
    ) -> Result<Arc<MonsterTemplate>> {
        let caster = &self.members[member];
        let active = self
            .summons
            .iter()
            .filter(|s| !s.removed && s.owner == caster.reference.entity);
        if active.count() + cast.count > c.member_summons().unwrap().max_active_per_member {
            return Err(Error::Capacity);
        }
        let profile = c.pve.as_ref().unwrap().profile(&cast.template_id).unwrap();
        if !self.grid.clear(caster.position, profile.combat.radius, 0) {
            return Err(Error::InvalidTarget);
        }
        Ok(profile)
    }

    pub(super) fn start_summons(
        &mut self,
        member: usize,
        (skill_id, cast, profile): (&str, &SummonCast, Arc<MonsterTemplate>),
        c: &Config,
        now: u64,
    ) {
        if !self.members[member].eligible() {
            return;
        }
        let owner = self.members[member].reference.entity.clone();
        let position = self.members[member].position;
        let [hp, atk, def] = if let Some(bps) = cast.inherit_attributes_bps {
            let (atk, def) = self.stats(member, c, now);
            [
                scale(self.entity_max_hp(&owner, c, now).unwrap(), bps).max(1),
                scale(atk, bps),
                scale(def, bps),
            ]
        } else {
            [
                profile.stats["hp"],
                profile.stats["atk"],
                profile.stats["def"],
            ]
        };
        let death_damage = cast.death_burst.map_or(0, |burst| {
            scale(
                self.recovery_power(Owner::Member(member), skill_id, c, now),
                burst.damage_bps,
            )
        });
        for _ in 0..cast.count {
            let id = loop {
                let id = self.next_id("summon");
                if !self.npcs.iter().any(|n| n.entity.id == id)
                    && !self.members.iter().any(|m| m.reference.entity.id == id)
                {
                    break id;
                }
            };
            self.summons.push(Summon {
                entity: EntityRef {
                    kind: StatusEntityKind::Monster,
                    id,
                    generation: 1,
                },
                owner: owner.clone(),
                member,
                template_id: cast.template_id.clone(),
                cast_skill_id: skill_id.into(),
                base_hp: hp,
                base_atk: atk,
                base_def: def,
                death_damage,
                owner_bonus: cast.owner_while_alive,
                hp,
                profile: profile.clone(),
                position,
                next_attack: now.saturating_add(1),
                ready_after_spawn: now.saturating_add(1),
                attack_clock: None,
                expires_at_ms: now.saturating_add(cast.duration_ms),
                copy: Default::default(),
                removed: false,
            });
        }
    }

    pub(super) fn retire_summon(&mut self, index: usize, events: &mut Events) {
        let summon = &mut self.summons[index];
        if summon.removed {
            return;
        }
        summon.removed = true;
        summon.copy.pending = None;
        let entity = summon.entity.clone();
        self.cancel_summon_shots(&entity, events);
        self.clear_owned_statuses(&entity, StatusRemovalReason::TargetInvalid, events);
        self.forget_npc_enemy(&entity, events);
        for npc in &mut self.npcs {
            if npc.target.as_ref() == Some(&entity) {
                npc.target = None;
            }
        }
    }

    pub(super) fn retire_member_summons(&mut self, owner: &EntityRef, events: &mut Events) {
        for i in 0..self.summons.len() {
            if self.summons[i].owner == *owner {
                self.retire_summon(i, events);
            }
        }
    }

    pub(super) fn expire_summons(&mut self, now: u64, events: &mut Events) {
        for i in 0..self.summons.len() {
            if !self.summon_alive(i) || now >= self.summons[i].expires_at_ms {
                self.retire_summon(i, events);
            }
        }
        // Keep indices stable until all of the preceding step's damage has settled.
        self.summons.retain(|summon| !summon.removed);
    }

    pub(super) fn advance_summons(&mut self, c: &Config, now: u64, events: &mut Events) {
        for i in 0..self.summons.len() {
            let actor = Owner::Summon(i);
            if !self.summon_eligible(i)
                || now < self.summons[i].ready_after_spawn
                || self.entity_has_status(actor, StatusAttribute::Stun, now)
            {
                self.summons[i].copy.pending = None;
                continue;
            }
            if self.advance_summon_copy(i, c, now, events) {
                continue;
            }
            let summon = &self.summons[i];
            if now < summon.next_attack {
                continue;
            }
            let target = self
                .npcs
                .iter()
                .enumerate()
                .filter(|(j, n)| {
                    n.hp > 0 && !n.returning && self.same_training_scope(actor, Owner::Npc(*j))
                })
                .map(|(j, n)| (j, n, distance(summon.position, n.position)))
                .filter(|(_, n, d)| {
                    *d <= summon.profile.combat.melee_range
                        && self.grid.line(summon.position, n.position, 0., 1)
                })
                .min_by(|a, b| a.2.total_cmp(&b.2).then(a.1.entity.id.cmp(&b.1.entity.id)))
                .map(|(j, _, _)| Owner::Npc(j));
            let Some(target) = target else {
                continue;
            };
            let source = summon.entity.clone();
            let interval = summon.profile.combat.attack_interval_ms;
            let raw = self.entity_attack(&status_entity(&source), c, now).unwrap();
            self.start_attack_clock(actor, interval, c, now);
            self.direct_contact(
                actor,
                target,
                DirectContact {
                    origin: None,
                    action_id: None,
                    rule: None,
                    ignore_armor: false,
                    unavoidable: false,
                    dead_source: false,
                    source: &source,
                    raw,
                    metal: false,
                    hit_statuses: &[],
                },
                c,
                now,
                events,
            );
        }
    }

    pub(super) fn summon_views<'a>(
        &'a self,
        observer: usize,
        c: &'a Config,
        now: u64,
    ) -> impl Iterator<Item = MonsterViewV4> + 'a {
        self.summons
            .iter()
            .enumerate()
            .filter_map(move |(i, summon)| {
                if !self.summon_alive(i) || now >= summon.expires_at_ms {
                    return None;
                }
                let max_hp = self.entity_max_hp(&summon.entity, c, now).unwrap();
                Some(MonsterViewV4 {
                    training_owner_id: None,
                    boss_phase: None,
                    summon: Some(SummonOwnershipView {
                        owner: summon.owner.clone(),
                        expires_at_ms: summon.expires_at_ms,
                    }),
                    entity: summon.entity.clone(),
                    template_id: summon.template_id.clone(),
                    position: summon.position,
                    hp: summon.hp.min(max_hp),
                    max_hp,
                    radius: summon.profile.combat.radius,
                    returning: false,
                    target: None,
                    shield_total: Some(
                        self.statuses
                            .shield_total(&status_entity(&summon.entity), now),
                    ),
                    statuses: Some(self.visible_npc_statuses(observer, &summon.entity, now)),
                })
            })
    }
}

pub(super) fn scale(value: u32, bps: u32) -> u32 {
    (u64::from(value) * u64::from(bps) / 10000).min(u64::from(u32::MAX)) as u32
}
