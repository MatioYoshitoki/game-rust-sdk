//! The released basic delivery holds admission until its native plans end.
use super::triggers::ActionKey;
use super::*;
use std::collections::BTreeSet;

#[derive(PartialEq, Eq)]
enum ContactReward {
    AwaitingContact,
    Earned,
    Canceled,
}

pub(super) struct BasicDelivery {
    action: ActionKey,
    reward: ContactReward,
    damaged: Vec<EntityRef>,
    finisher_status: Option<String>,
}
impl BasicDelivery {
    pub(super) fn is_action(&self, id: &str) -> bool {
        self.action.id == id
    }
    pub(super) fn cancel_reward(&mut self) {
        self.reward = ContactReward::Canceled;
    }
    pub(super) fn earned(&self) -> bool {
        self.reward == ContactReward::Earned
    }
}

pub(super) enum NativeBasicPlan {
    Wave(super::basic_wave::BasicWavePlan),
    Dash(super::basic_charge::BasicDashPlan),
    Flight(Box<Shot>),
    Impact(super::basic_impact::BasicImpactPlan),
    Projectile(super::basic_projectiles::BasicProjectilePlan),
}

impl NativeBasicPlan {
    pub(super) fn deadline(&self) -> u64 {
        match self {
            Self::Wave(plan) => plan.deadline(),
            Self::Dash(plan) => plan.at,
            Self::Flight(shot) => shot.expires,
            Self::Impact(plan) => plan.view.impact_at_ms,
            Self::Projectile(plan) => plan.deadline,
        }
    }

    pub(super) fn projectile_slots(&self) -> usize {
        match self {
            Self::Projectile(plan) => plan.slots,
            Self::Flight(_) => 1,
            _ => 0,
        }
    }
}

impl World {
    pub(super) fn plan_native_basic(
        &self,
        i: usize,
        p: &Pending,
        c: &Config,
        now: u64,
    ) -> Result<Option<NativeBasicPlan>> {
        let member = &self.members[i];
        let basic = member.basic(c);
        if let Some(ChargedBasic::Dash(distance)) = p.charged_basic {
            return self
                .plan_basic_dash(i, p.direction, distance, c, now)
                .map(NativeBasicPlan::Dash)
                .map(Some);
        }
        if let Some(ChargedBasic::Flight(stage)) = p.charged_basic {
            return self
                .plan_charged_flight(i, p, stage, c, now)
                .map(Box::new)
                .map(NativeBasicPlan::Flight)
                .map(Some);
        }
        if let Some(plan) = &basic.projectile_followup {
            self.plan_basic_projectile(i, p, plan, c, now)
                .map(NativeBasicPlan::Projectile)
                .map(Some)
        } else if let Some(wave) = &basic.wave {
            self.plan_basic_wave(i, p, wave, c, now)
                .map(NativeBasicPlan::Wave)
                .map(Some)
        } else {
            basic
                .impact
                .as_ref()
                .map(|impact| {
                    self.plan_basic_impact(i, p, impact, c, now)
                        .map(NativeBasicPlan::Impact)
                })
                .transpose()
        }
    }

    pub(super) fn resolve_native_basic_contact(
        &mut self,
        i: usize,
        target: Owner,
        (source, action, raw): (&EntityRef, &str, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> bool {
        self.resolve_native_basic_damage(i, target, (source, action, raw), c, now, events)
            .is_some()
    }

    pub(super) fn resolve_native_basic_damage(
        &mut self,
        i: usize,
        target: Owner,
        (source, action, raw): (&EntityRef, &str, u32),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) -> Option<u32> {
        let basic = self.members[i].basic(c);
        self.direct_contact_damage(
            Owner::Member(i),
            target,
            combat::DirectContact {
                origin: Some(crate::origin::ActionOrigin::Basic),
                action_id: Some(action),
                rule: Some(&c.combat_rules.as_ref().unwrap().basic_attack),
                ignore_armor: basic.ignore_armor == Some(true),
                unavoidable: basic.unavoidable == Some(true),
                dead_source: false,
                source,
                raw,
                metal: false,
                hit_statuses: &[],
            },
            c,
            now,
            events,
        )
    }

    pub(super) fn start_basic_delivery(
        &mut self,
        i: usize,
        id: &str,
        deadline: u64,
        finisher_status: Option<String>,
    ) {
        let member = &mut self.members[i];
        debug_assert!(member.basic_delivery.is_none());
        member.basic_delivery = Some(BasicDelivery {
            action: ActionKey::new(&member.reference.entity, id),
            reward: ContactReward::AwaitingContact,
            damaged: vec![],
            finisher_status,
        });
        member.attack_clock = None;
        // Until contact or expiry gives an exact end, publish the delivery deadline.
        member.basic_ready = deadline;
    }

    pub(super) fn basic_contact(
        &mut self,
        i: usize,
        action: Option<&str>,
        damaged: Option<&EntityRef>,
        c: &Config,
        now: u64,
    ) {
        let member = &mut self.members[i];
        let basic = member.basic(c);
        if !basic.settle_on_end {
            self.restore_basic_spirit(i, c, now);
            return;
        }
        if let Some(delivery) = &mut member.basic_delivery
            && Some(delivery.action.id.as_str()) == action
            && delivery.reward != ContactReward::Canceled
        {
            delivery.reward = ContactReward::Earned;
            if let Some(target) = damaged
                && (basic.effects.is_some() || basic.stacked_wave.is_some())
                && !delivery.damaged.contains(target)
            {
                delivery.damaged.push(target.clone());
            }
        }
    }

    pub(super) fn apply_basic_self_status(
        &mut self,
        i: usize,
        status: Option<&String>,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if let Some(id) = status {
            let defs = Self::definitions(c, std::slice::from_ref(id));
            let source = self.owner_entity(Owner::Member(i));
            self.apply_owned_status_definitions(Owner::Member(i), source, &defs, c, now, events);
        }
    }

    fn apply_basic_target_status(
        &mut self,
        i: usize,
        target: EntityRef,
        effect: &crate::config::BasicEndTargetStatus,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let source = &self.members[i].reference.entity;
        if !self.hostile_contact(source, &target) {
            return;
        }
        let Some(Owner::Npc(j)) = self.resolve_entity(&target) else {
            return;
        };
        let rules = c.combat_rules.as_ref().unwrap();
        let profile = rules
            .npc_profiles
            .get(&self.npcs[j].template_id)
            .unwrap_or(&rules.default_npc_profile);
        let scale = effect
            .target_profile_bps
            .get(profile)
            .copied()
            .unwrap_or(10000);
        let count = effect.choices.len() as u32;
        let index = if count == 1 {
            0
        } else {
            let limit = u32::MAX - u32::MAX % count;
            loop {
                let value = self.random();
                if value <= limit {
                    break ((value - 1) % count) as usize;
                }
            }
        };
        let mut defs = Self::definitions(c, std::slice::from_ref(&effect.choices[index]));
        let definition = &mut defs[0];
        definition.modifier_bps =
            (i64::from(definition.modifier_bps) * i64::from(scale) / 10000).min(-1) as i32;
        self.apply_owned_status_definitions(Owner::Member(i), target, &defs, c, now, events);
    }

    fn restore_basic_spirit(&mut self, i: usize, c: &Config, now: u64) {
        let max_spirit = self.member_max_spirit(i, c, now);
        let member = &mut self.members[i];
        let gap = member.spirit_recovery_gap(&self.statuses, now, max_spirit);
        member.spirit += c.params.basic_spirit_on_hit.min(gap);
    }

    pub(super) fn remove_basic_stack(
        &mut self,
        instance: &str,
        reason: StatusRemovalReason,
        events: &mut Events,
    ) {
        let Some(status) = self
            .statuses
            .entries()
            .iter()
            .find(|status| status.instance_id == instance)
            .cloned()
        else {
            return;
        };
        let mut raw = vec![];
        self.statuses
            .remove(reason, |status| status.instance_id == instance, &mut raw);
        self.route_status_events(raw, &[status], events);
    }

    pub(super) fn finish_basic_deliveries(
        &mut self,
        c: &Config,
        now: u64,
        active: &BTreeSet<ActionKey>,
        events: &mut Events,
    ) {
        for i in 0..self.members.len() {
            if self.members[i]
                .basic_delivery
                .as_ref()
                .is_some_and(|d| !active.contains(&d.action))
            {
                self.finish_basic_delivery(i, c, now, events);
            }
        }
    }

    pub(super) fn finish_basic_delivery(
        &mut self,
        i: usize,
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        let member = &mut self.members[i];
        let Some(delivery) = member.basic_delivery.take() else {
            return;
        };
        let basic = member.basic(c);
        let eligible = member.eligible() && delivery.reward == ContactReward::Earned;
        if let Some(instance) = &delivery.finisher_status {
            self.remove_basic_stack(instance, StatusRemovalReason::Consumed, events);
        } else if eligible
            && !delivery.damaged.is_empty()
            && let Some(upgrade) = &basic.stacked_wave
        {
            self.apply_basic_self_status(i, Some(&upgrade.stack_status), c, now, events);
        }
        if eligible {
            self.restore_basic_spirit(i, c, now);
            if let Some(effects) = &basic.effects
                && !delivery.damaged.is_empty()
            {
                self.apply_basic_self_status(
                    i,
                    effects.on_end_damage_self_status.as_ref(),
                    c,
                    now,
                    events,
                );
                if let Some(bps) = effects.on_end_damage_self_heal_max_hp_bps {
                    let source = self.owner_entity(Owner::Member(i));
                    let max_hp = self.entity_max_hp(&source, c, now).unwrap();
                    self.ratio_self_heal(
                        &source,
                        u64::from(max_hp),
                        (bps, HealKind::BasicAttack),
                        c,
                        now,
                        events,
                    );
                }
                if let Some(effect) = &effects.on_end_damage_target_status {
                    for target in delivery.damaged {
                        self.apply_basic_target_status(i, target, effect, c, now, events);
                    }
                }
            }
        }
        self.start_attack_clock(Owner::Member(i), basic.interval_ms, c, now);
    }
}

#[cfg(all(test, match_sdk_test_data))]
mod tests {
    use super::super::member_unavoidable_tests::{commit, setup};
    use super::super::periodic_tests::bind;
    use super::*;
    use serde_json::Value;

    #[test]
    fn basic_delivery_full_shield_counts_as_a_successful_contact_for_spirit() {
        let (mut c, mut w) = setup();
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/mastery-stages.json"
        ))
        .unwrap();
        c.mechanism_version = 105;
        c.combat_rules = Some(serde_json::from_value(sample["combat_rules"].clone()).unwrap());
        c.basic.settle_on_end = true;
        let recipe: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/basic-weapon-profiles.json"
        ))
        .unwrap();
        let mut effects = recipe["basic_profiles"]["shield"]["effects"].clone();
        effects["on_end_damage_self_heal_max_hp_bps"] = serde_json::json!(250);
        c.basic.effects = serde_json::from_value(effects).unwrap();
        Arc::make_mut(c.pve.as_mut().unwrap())
            .status_definitions
            .push(serde_json::from_value(recipe["status_definitions"][0].clone()).unwrap());
        c.params.basic_spirit_on_hit = 2;
        c.params.basic_projectile_radius = 0.;
        c.params.regen.near_ratio = 0.;
        c.params.regen.far_ratio = 0.;
        let rules = c.combat_rules.as_mut().unwrap();
        rules.triggers.clear();
        rules.basic_attack.base_power = Some(1);
        w.members[0].spirit = 10;
        let npc = w.npcs[0].entity.clone();
        bind(&mut w, &c, Owner::Npc(0), npc, "ward", 0);
        let hp = w.npcs[0].hp;
        commit(&mut w, &c, "basic");
        let mut events = vec![];
        w.advance_shots(&c, 60, 1., &mut events);
        assert_eq!(w.npcs[0].hp, hp);
        assert_eq!(w.members[0].spirit, 10);
        assert!(
            events
                .iter()
                .any(|(_, e)| matches!(e, EventV4::Damage { amount: 0, .. }))
        );
        w.finish_trigger_actions(&c, 60, &mut events);
        w.finish_trigger_actions(&c, 90, &mut events);
        assert_eq!(w.members[0].spirit, 12);
        assert!(events.iter().all(|(_, event)| !matches!(
            event,
            EventV4::Private {
                event: Event::HealResolved {
                    heal_kind: HealKind::BasicAttack,
                    ..
                }
            }
        )));
        assert!(
            !w.statuses
                .entries()
                .iter()
                .any(|s| s.definition_id == "shield-absorption")
        );
        assert_eq!(w.members[0].basic_ready, 60 + c.basic.interval_ms);
    }
}
