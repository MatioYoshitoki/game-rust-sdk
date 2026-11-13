use super::*;
use crate::config::{BasicAlternateWave, BasicWaveBehavior};

impl Member {
    pub(super) fn wave_phase(&self, c: &Config) -> Option<BasicWavePhase> {
        matches!(
            self.basic(c).wave.as_ref()?.behavior,
            Some(BasicWaveBehavior::Alternate { .. })
        )
        .then_some(self.basic_wave_phase)
    }

    pub(super) fn alternate_wave<'a>(&self, c: &'a Config) -> Option<&'a BasicAlternateWave> {
        if self.basic_wave_phase == BasicWavePhase::Alternate
            && let Some(BasicWaveBehavior::Alternate { alternate, .. }) =
                self.basic(c).wave.as_ref()?.behavior.as_ref()
        {
            return Some(alternate);
        }
        None
    }

    pub(super) fn basic_range(&self, c: &Config) -> f64 {
        self.alternate_wave(c)
            .map_or_else(|| self.basic(c).range, |wave| wave.range)
    }
}

impl World {
    fn wave_target_window(&self, i: usize, j: usize, now: u64) -> bool {
        self.members[i]
            .basic_wave_target_until
            .get(&j)
            .is_some_and(|&(generation, end)| {
                generation == self.npcs[j].entity.generation && now < end
            })
    }

    fn set_wave_target_window(&mut self, i: usize, j: usize, end: u64) {
        self.members[i]
            .basic_wave_target_until
            .insert(j, (self.npcs[j].entity.generation, end));
    }

    pub(super) fn wave_contact_budget(
        &self,
        (i, j): (usize, usize),
        phase: Option<BasicWavePhase>,
        behavior: Option<&BasicWaveBehavior>,
        raw: u32,
        now: u64,
    ) -> u32 {
        if phase == Some(BasicWavePhase::Alternate)
            && let Some(BasicWaveBehavior::Alternate {
                marked_damage_bonus_bps,
                ..
            }) = behavior
            && self.wave_target_window(i, j, now)
        {
            return (u64::from(raw) * u64::from(10000 + marked_damage_bonus_bps) / 10000)
                .min(u64::from(u32::MAX)) as u32;
        }
        raw
    }

    pub(super) fn apply_wave_control(
        &mut self,
        (i, j): (usize, usize),
        (phase, geometry, origin, behavior): (
            Option<BasicWavePhase>,
            &BasicWaveGeometry,
            [f64; 2],
            &BasicWaveBehavior,
        ),
        c: &Config,
        now: u64,
        events: &mut Events,
    ) {
        if !self.members[i].living() {
            return;
        }
        if phase == Some(BasicWavePhase::Alternate) {
            self.members[i].basic_wave_target_until.remove(&j);
            return;
        }
        let npc = &self.npcs[j];
        if npc.hp == 0 || npc.returning {
            return;
        }
        let rules = c.combat_rules.as_ref().unwrap();
        let profile = rules
            .npc_profiles
            .get(&npc.template_id)
            .unwrap_or(&rules.default_npc_profile);
        match behavior {
            BasicWaveBehavior::RadialPush {
                distance,
                resistant_profiles,
                reduced_status,
            } => {
                if resistant_profiles.contains(profile) {
                    let defs = Self::definitions(c, std::slice::from_ref(reduced_status));
                    self.apply_owned_status_definitions(
                        Owner::Member(i),
                        npc.entity.clone(),
                        &defs,
                        c,
                        now,
                        events,
                    );
                } else {
                    let direction =
                        normalize([npc.position[0] - origin[0], npc.position[1] - origin[1]]);
                    self.displace_wave_target(j, direction, *distance);
                }
            }
            BasicWaveBehavior::Push {
                distance,
                rehit_immunity_ms,
                resistant_profiles,
                reduced_status,
            } => {
                if resistant_profiles.contains(profile) {
                    let defs = Self::definitions(c, std::slice::from_ref(reduced_status));
                    self.apply_owned_status_definitions(
                        Owner::Member(i),
                        npc.entity.clone(),
                        &defs,
                        c,
                        now,
                        events,
                    );
                } else if let BasicWaveGeometry::Directional { direction, .. } = geometry
                    && !self.wave_target_window(i, j, now)
                    && self.displace_wave_target(j, *direction, *distance)
                {
                    self.set_wave_target_window(i, j, now + rehit_immunity_ms);
                }
            }
            BasicWaveBehavior::Alternate {
                pull_distance,
                resistant_profiles,
                mark_duration_ms,
                ..
            } => {
                if !resistant_profiles.contains(profile) {
                    let source = self.members[i].position;
                    let delta = [source[0] - npc.position[0], source[1] - npc.position[1]];
                    let free = delta[0].hypot(delta[1])
                        - c.params.player_radius
                        - npc.profile.combat.radius;
                    self.displace_wave_target(j, normalize(delta), pull_distance.min(free.max(0.)));
                }
                self.set_wave_target_window(i, j, now + mark_duration_ms);
            }
        }
    }

    pub(super) fn displace_wave_target(
        &mut self,
        j: usize,
        direction: [f64; 2],
        distance: f64,
    ) -> bool {
        if distance <= 0. {
            return false;
        }
        let npc = &mut self.npcs[j];
        let from = npc.position;
        let end = [
            from[0] + direction[0] * distance,
            from[1] + direction[1] * distance,
        ];
        let to = self
            .grid
            .walk_until_blocked(from, end, npc.profile.combat.radius);
        if to == from {
            return false;
        }
        npc.position = to;
        npc.path.clear();
        npc.repath = 0;
        true
    }
}
