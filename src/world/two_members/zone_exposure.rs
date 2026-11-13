use super::*;
use crate::config::zone_exposure::{ExposureInteraction, ExposureNode, ZoneExposure};

const STACK_UNIT: u64 = 1_000_000_000;

#[derive(Default)]
pub(super) struct Exposure {
    stacks: u8,
    progress: u64,
    remainder: u64,
    band: usize,
    boost_until: u64,
}

impl Exposure {
    fn advance(&mut self, rule: &ZoneExposure, band: usize, elapsed_ms: u64) {
        if self.stacks == rule.max_stacks {
            return;
        }
        if self.band != band {
            self.band = band;
            self.remainder = 0;
        }
        let interval = rule.bands[band].interval_ms;
        // Keep the fractional remainder within a band so a full interval is exact.
        let amount = u128::from(elapsed_ms) * u128::from(STACK_UNIT) + u128::from(self.remainder);
        let progress = u128::from(self.progress) + amount / u128::from(interval);
        let gained =
            (progress / u128::from(STACK_UNIT)).min(u128::from(rule.max_stacks - self.stacks));
        self.stacks += gained as u8;
        if self.stacks == rule.max_stacks {
            self.progress = 0;
            self.remainder = 0;
        } else {
            self.progress = (progress % u128::from(STACK_UNIT)) as u64;
            self.remainder = (amount % u128::from(interval)) as u64;
        }
    }
}

#[derive(Clone, Copy)]
struct NodeState {
    revision: u32,
    ready_at_ms: u64,
}

pub(super) struct ExposureWorld {
    sampled_at_ms: u64,
    nodes: Vec<NodeState>,
}

impl ExposureWorld {
    pub(super) fn new(rule: &ZoneExposure) -> Self {
        Self {
            sampled_at_ms: 0,
            nodes: vec![
                NodeState {
                    revision: 1,
                    ready_at_ms: 0
                };
                rule.interaction
                    .as_ref()
                    .map_or(0, |config| config.nodes.len())
            ],
        }
    }

    pub(super) fn activate(&mut self, now: u64) {
        self.sampled_at_ms = now;
    }
}

impl Member {
    pub(super) fn exposure_points(&self, points: u32, c: &Config, now: u64) -> u32 {
        let Some((state, rule)) = self.exposure.as_ref().zip(c.zone_exposure.as_ref()) else {
            return points;
        };
        let reduced = 10_000 - u64::from(state.stacks) * u64::from(rule.reduction_per_stack_bps);
        let boost = rule
            .interaction
            .as_ref()
            .filter(|_| now < state.boost_until)
            .map_or(10_000, |interaction| {
                10_000 + u64::from(interaction.boost_bps)
            });
        (u64::from(points) * reduced * boost / 100_000_000).min(u64::from(u32::MAX)) as u32
    }
}

impl World {
    pub(super) fn advance_zone_exposure(&mut self, c: &Config, now: u64) {
        let Some((state, rule)) = self.exposure.as_mut().zip(c.zone_exposure.as_ref()) else {
            return;
        };
        if now <= state.sampled_at_ms {
            return;
        }
        let elapsed = now - state.sampled_at_ms;
        state.sampled_at_ms = now;
        for member in self.members.iter_mut().filter(|member| member.living()) {
            let band = rule.band_at(member.position);
            member
                .exposure
                .as_mut()
                .unwrap()
                .advance(rule, band, elapsed);
        }
    }

    fn exposure_node_in_reach(
        &self,
        seat: usize,
        node: &ExposureNode,
        rule: &ExposureInteraction,
    ) -> bool {
        let position = self.members[seat].position;
        distance(position, node.position) <= rule.range
            && self.grid.line(position, node.position, 0., 0)
    }

    pub(super) fn use_exposure_node(
        &mut self,
        seat: usize,
        (id, revision, choice): (&str, u32, ExposureNodeChoice),
        c: &Config,
        now: u64,
    ) -> Result<()> {
        let rule = c
            .zone_exposure
            .as_ref()
            .and_then(|rule| rule.interaction.as_ref())
            .ok_or(Error::Unsupported)?;
        let index = rule
            .nodes
            .iter()
            .position(|node| node.id == id)
            .ok_or(Error::InvalidTarget)?;
        let current = self.exposure.as_ref().unwrap().nodes[index];
        if current.revision != revision {
            return Err(Error::RevisionConflict);
        }
        if now < current.ready_at_ms {
            return Err(Error::InvalidState);
        }
        if !self.exposure_node_in_reach(seat, &rule.nodes[index], rule) {
            return Err(Error::InvalidTarget);
        }
        let next_revision = current.revision.checked_add(1).ok_or(Error::Capacity)?;
        let state = self.members[seat]
            .exposure
            .as_mut()
            .ok_or(Error::InvalidState)?;
        match choice {
            ExposureNodeChoice::Reduce => {
                if state.stacks == 0 {
                    return Err(Error::InvalidState);
                }
                state.stacks = state.stacks.saturating_sub(rule.remove_stacks);
            }
            ExposureNodeChoice::Boost => state.boost_until = now + rule.boost_duration_ms,
        }
        self.exposure.as_mut().unwrap().nodes[index] = NodeState {
            revision: next_revision,
            ready_at_ms: now + rule.cooldown_ms,
        };
        Ok(())
    }

    pub(super) fn zone_exposure_view(
        &self,
        seat: usize,
        c: &Config,
        now: u64,
    ) -> Option<ZoneExposureView> {
        let member = &self.members[seat];
        if !member.living() {
            return None;
        }
        let personal = member.exposure.as_ref()?;
        let world = self.exposure.as_ref()?;
        let rule = c.zone_exposure.as_ref()?;
        let nodes = rule
            .interaction
            .as_ref()
            .map_or_else(Vec::new, |interaction| {
                interaction
                    .nodes
                    .iter()
                    .zip(&world.nodes)
                    .filter(|(node, _)| self.exposure_node_in_reach(seat, node, interaction))
                    .map(|(node, state)| ExposureNodeView {
                        id: node.id.clone(),
                        position: node.position,
                        revision: state.revision,
                        ready_at_ms: state.ready_at_ms,
                    })
                    .collect()
            });
        Some(ZoneExposureView {
            band_id: rule.bands[rule.band_at(member.position)].id.clone(),
            stacks: personal.stacks,
            max_stacks: rule.max_stacks,
            boost_ends_at_ms: (now < personal.boost_until).then_some(personal.boost_until),
            nodes,
        })
    }
}
