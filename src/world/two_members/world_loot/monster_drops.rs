use super::*;
use crate::world::pve::Grid;
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct MonsterDrops {
    contributions: BTreeMap<usize, BTreeMap<usize, u64>>,
    pending: VecDeque<Draft>,
}

struct Draft {
    items: Vec<ItemAmount>,
    source: [f64; 2],
    candidate: [f64; 2],
    correction_radius: f64,
    priority_ms: u64,
    winner: Option<usize>,
    retry_at: u64,
}

impl World {
    pub(in crate::world::two_members) fn record_monster_contribution(
        &mut self,
        npc: usize,
        member: usize,
        damage: u32,
        c: &Config,
    ) {
        if damage == 0
            || !c.world_loot.as_ref().is_some_and(|config| {
                config
                    .monster_pools
                    .get(&self.npcs[npc].template_id)
                    .is_some_and(|source| source.priority_ms != 0)
            })
        {
            return;
        }
        let contribution = self
            .world_loot
            .monster_drops
            .contributions
            .entry(npc)
            .or_default()
            .entry(member)
            .or_default();
        *contribution += u64::from(damage);
    }

    pub(in crate::world::two_members) fn clear_monster_contribution(&mut self, npc: usize) {
        self.world_loot.monster_drops.contributions.remove(&npc);
    }

    pub(in crate::world::two_members) fn release_monster_priority(&mut self, entity: &EntityRef) {
        for resource in &mut self.resources {
            if matches!(resource.origin, LootOrigin::Monster { .. })
                && resource.view.owner.as_ref() == Some(entity)
            {
                resource.view.owner = None;
            }
        }
    }

    pub(in crate::world::two_members) fn queue_monster_loot(
        &mut self,
        npc: usize,
        c: &Config,
        now: u64,
    ) -> bool {
        let Some(config) = &c.world_loot else {
            return false;
        };
        let Some(definition) = config.monster_pools.get(&self.npcs[npc].template_id) else {
            return false;
        };
        let winner = self
            .world_loot
            .monster_drops
            .contributions
            .remove(&npc)
            .into_iter()
            .flatten()
            .filter(|(member, _)| self.members[*member].living())
            .min_by(|(a, da), (b, db)| {
                db.cmp(da).then_with(|| {
                    self.members[*a]
                        .reference
                        .entity
                        .id
                        .cmp(&self.members[*b].reference.entity.id)
                })
            })
            .map(|(member, _)| member);
        let pool = self
            .encounter_loot_pool(&self.npcs[npc].template_id, c)
            .unwrap_or(&definition.pool);
        let items = self.draw_loot(&config.pools[pool]);
        if !items.is_empty() {
            let source = self.npcs[npc].position;
            let angle = f64::from(self.loot_index(360)) * std::f64::consts::TAU / 360.0;
            let radius = c.params.drop_min_radius
                + self.loot_index(c.params.drop_max_radius - c.params.drop_min_radius + 1);
            let candidate = scatter(source, f64::from(radius), angle);
            self.world_loot.monster_drops.pending.push_back(Draft {
                items,
                source,
                candidate,
                correction_radius: definition.correction_radius,
                priority_ms: definition.priority_ms,
                winner,
                retry_at: now,
            });
        }
        true
    }

    pub(super) fn advance_monster_drops(
        &mut self,
        c: &Config,
        config: &WorldLoot,
        now: u64,
        events: &mut Events,
    ) {
        if self.world_loot.monster_drops.pending.is_empty() {
            return;
        }
        let mut count = self
            .resources
            .iter()
            .filter(|r| matches!(r.origin, LootOrigin::Monster { .. }))
            .count();
        for _ in 0..self.world_loot.monster_drops.pending.len() {
            let mut draft = self.world_loot.monster_drops.pending.pop_front().unwrap();
            let size = draft.items.len();
            let point = (now >= draft.retry_at && count + size <= config.one_shot_capacity)
                .then(|| draft.position(&self.grid, c.params.player_radius))
                .flatten();
            let Some(point) = point else {
                if now >= draft.retry_at {
                    draft.retry_at = now + c.params.runtime.resource_sweep_ms;
                }
                self.world_loot.monster_drops.pending.push_back(draft);
                continue;
            };
            let winner = draft.winner.filter(|&i| self.members[i].living());
            self.publish_loot(
                draft.items,
                point,
                now + c.params.resource_ttl_ms,
                (
                    LootOrigin::Monster {
                        priority_until_ms: now + draft.priority_ms,
                    },
                    winner,
                ),
                c,
                events,
            );
            count += size;
        }
    }
}

impl Draft {
    fn position(&self, grid: &Grid, footprint: f64) -> Option<[f64; 2]> {
        if !grid.clear(self.source, footprint, 0) {
            return None;
        }
        let legal = |point: &[f64; 2]| {
            grid.clear(*point, footprint, 0)
                && !grid.route(self.source, *point, footprint).is_empty()
        };
        if legal(&self.candidate) {
            return Some(self.candidate);
        }
        if self.correction_radius == 0.0 {
            return None;
        }
        if distance(self.source, self.candidate) <= self.correction_radius && legal(&self.source) {
            return Some(self.source);
        }
        [0.5, 1.0]
            .into_iter()
            .flat_map(|fraction| {
                (0..8).map(move |direction| {
                    scatter(
                        self.candidate,
                        fraction * self.correction_radius,
                        f64::from(direction) * std::f64::consts::TAU / 8.0,
                    )
                })
            })
            .find(legal)
    }
}

fn scatter(origin: [f64; 2], radius: f64, angle: f64) -> [f64; 2] {
    [
        origin[0] + radius * angle.cos(),
        origin[1] + radius * angle.sin(),
    ]
}
