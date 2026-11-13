use super::*;
use crate::config::encounter_progress::{EncounterModifier, EncounterProgress};

pub(super) struct EncounterState {
    view: EncounterProgressView,
}

impl EncounterState {
    pub(super) fn new(rule: &EncounterProgress) -> Self {
        Self {
            view: EncounterProgressView {
                initial: rule.initial,
                remaining: rule.initial,
                stats_bps: 10000,
                surge_started: false,
                surge_ends_at_ms: None,
            },
        }
    }

    fn modifier<'a>(&self, rule: &'a EncounterProgress) -> Option<&'a EncounterModifier> {
        if self.view.surge_ends_at_ms.is_some() {
            return rule.surge.as_ref().map(|surge| &surge.modifier);
        }
        rule.tiers
            .iter()
            .find(|tier| self.view.remaining <= tier.remaining_at_most)
            .map(|tier| &tier.modifier)
    }
}

impl World {
    pub(super) fn encounter_view(&self) -> Option<EncounterProgressView> {
        self.encounter.as_ref().map(|state| state.view.clone())
    }

    pub(super) fn encounter_loot_pool<'a>(&self, template: &str, c: &'a Config) -> Option<&'a str> {
        self.encounter
            .as_ref()?
            .modifier(c.encounter_progress.as_ref()?)?
            .loot_pools
            .get(template)
            .map(String::as_str)
    }

    pub(super) fn record_encounter_kill(&mut self, j: usize, c: &Config, now: u64) {
        let Some(rule) = &c.encounter_progress else {
            return;
        };
        let Some(weight) = rule.kill_weights.get(&self.npcs[j].entity.id) else {
            return;
        };
        let state = self.encounter.as_mut().unwrap();
        if state.view.remaining == 0 {
            return;
        }
        state.view.remaining = state.view.remaining.saturating_sub(*weight);
        if state.view.remaining == 0
            && let Some(surge) = &rule.surge
        {
            state.view.surge_started = true;
            state.view.surge_ends_at_ms = Some(now + surge.duration_ms);
        }
        self.refresh_encounter_stats(c, now);
    }

    pub(super) fn advance_encounter_progress(&mut self, c: &Config, now: u64) {
        if let Some(state) = &mut self.encounter
            && state.view.surge_ends_at_ms.is_some_and(|ends| now >= ends)
        {
            state.view.surge_ends_at_ms = None;
            self.refresh_encounter_stats(c, now);
        }
    }

    pub(super) fn refresh_encounter_stats(&mut self, c: &Config, now: u64) {
        let Some(rule) = &c.encounter_progress else {
            return;
        };
        let state = self.encounter.as_mut().unwrap();
        let bps = state.modifier(rule).map_or(10000, |m| m.stats_bps);
        if state.view.stats_bps == bps {
            return;
        }
        state.view.stats_bps = bps;
        let pve = c.pve.as_ref().unwrap();
        let profiles: BTreeMap<_, _> = rule
            .affected_templates
            .iter()
            .map(|id| {
                let mut profile = pve.profile(id).unwrap().as_ref().clone();
                for stat in ["hp", "atk", "def"] {
                    let points = u64::from(profile.stats[stat]) * u64::from(bps) / 10000;
                    profile.stats.insert(
                        stat.into(),
                        (if stat == "hp" { points.max(1) } else { points }) as u32,
                    );
                }
                (id, Arc::new(profile))
            })
            .collect();
        for j in 0..self.npcs.len() {
            let npc = &self.npcs[j];
            let Some(profile) = profiles.get(&npc.template_id) else {
                continue;
            };
            let entity = npc.entity.clone();
            let before = self.entity_max_hp(&entity, c, now).unwrap();
            self.npcs[j].profile = profile.clone();
            let after = self.entity_max_hp(&entity, c, now).unwrap();
            let hp = &mut self.npcs[j].hp;
            if *hp > 0 {
                *hp = (u64::from((*hp).min(before)) * u64::from(after) / u64::from(before)).max(1)
                    as u32;
            }
        }
    }
}
