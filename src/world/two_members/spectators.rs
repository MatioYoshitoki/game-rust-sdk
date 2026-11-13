use super::*;

impl World {
    /// Construct the public data directly, without choosing a participant as a viewer.
    pub(crate) fn spectator_snapshot(
        &self,
        c: &Config,
        phase: Phase,
        now: u64,
    ) -> Option<SpectatorSnapshotV6> {
        let active_at_ms = self.active_at?;
        let stage = self.spectator_stage(c);
        let contest = stage
            .as_ref()
            .is_some_and(|stage| stage.objective == ModeObjective::Contest);
        let members = self
            .members
            .iter()
            .map(|m| {
                let max_hp = self.entity_max_hp(&m.reference.entity, c, now).unwrap();
                SpectatorMemberV6 {
                    extraction_channel: m.extraction_channel.clone(),
                    entity: m.reference.entity.clone(),
                    position: m.position,
                    hp: m.hp.min(max_hp),
                    max_hp,
                    phase: m.phase,
                    outcome: m.desired.filter(|outcome| {
                        contest && matches!(outcome, Outcome::Win | Outcome::Loss | Outcome::Draw)
                    }),
                }
            })
            .collect();
        let mut monsters: Vec<_> = self
            .npcs
            .iter()
            .filter(|npc| {
                npc.visible_training_target()
                    && npc.pursuit_visible()
                    && self.mode_npc_visible(&npc.entity.id)
            })
            .map(|n| {
                let max_hp = self.entity_max_hp(&n.entity, c, now).unwrap();
                SpectatorMonsterV6 {
                    entity: n.entity.clone(),
                    template_id: n.template_id.clone(),
                    position: n.position,
                    hp: n.hp.min(max_hp),
                    max_hp,
                    radius: n.profile.combat.radius,
                    summon_owner: None,
                }
            })
            .collect();
        monsters.extend(
            self.summons
                .iter()
                .enumerate()
                .filter(|(i, s)| self.summon_alive(*i) && now < s.expires_at_ms)
                .map(|(_, s)| {
                    let max_hp = self.entity_max_hp(&s.entity, c, now).unwrap();
                    SpectatorMonsterV6 {
                        entity: s.entity.clone(),
                        template_id: s.template_id.clone(),
                        position: s.position,
                        hp: s.hp.min(max_hp),
                        max_hp,
                        radius: s.profile.combat.radius,
                        summon_owner: Some(s.owner.clone()),
                    }
                }),
        );
        monsters.sort_by(|a, b| a.entity.id.cmp(&b.entity.id));
        let projectile = |p: &ProjectileViewV4| SpectatorProjectileV6 {
            id: p.id.clone(),
            source: p.source.clone(),
            position: p.position,
            radius: p.radius,
        };
        let projectiles = self
            .shots
            .iter()
            .map(|s| projectile(&s.view))
            .chain(self.basic_projectile_views().map(|p| projectile(&p)))
            .collect();
        let resources = self
            .resources
            .iter()
            .filter(|r| {
                r.origin != world_loot::LootOrigin::PrivateContainer
                    && r.owner_at(now).is_none()
                    && now < r.view.expires_at_ms
            })
            .map(|r| SpectatorObjectV6 {
                id: r.view.id.clone(),
                position: r.view.position,
                expires_at_ms: r.view.expires_at_ms,
            })
            .collect();
        let areas = self
            .areas
            .iter()
            .filter(|a| !a.is_trap() && !a.is_anchor() && !a.follows_anchor())
            .map(|a| {
                let view = a.view();
                SpectatorAreaV6 {
                    id: view.id,
                    center: view.center,
                    radius: view.radius,
                }
            })
            .collect();
        Some(SpectatorSnapshotV6 {
            contested_equipment: self.contested_views(),
            extraction_pressure: self.extraction_pressure_view(c, now),
            encounter_progress: self.encounter_view(),
            phase,
            logical_ms: now,
            active_at_ms,
            members,
            monsters,
            projectiles,
            resources,
            areas,
            events: vec![],
            stage,
        })
    }
}
