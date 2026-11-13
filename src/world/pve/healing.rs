use super::*;
impl Arena {
    pub(super) fn resolve_heal(
        &self,
        w: &World,
        c: &Config,
        entities: (&StatusEntity, &StatusEntity),
        heal: (HealKind, u32),
        now: u64,
        events: &mut Vec<Event>,
    ) -> u32 {
        let (source, target) = entities;
        if !self.status_source_valid(w, source) || !self.status_source_valid(w, target) {
            return 0;
        }
        let (hp, max_hp) = if target.kind == StatusEntityKind::Participant {
            (w.hp, w.facts.hp_points)
        } else if let Some(i) = self.status_monster_index(target) {
            (self.monsters[i].hp, self.monsters[i].profile.stats["hp"])
        } else {
            return 0;
        };
        let (kind, raw) = heal;
        let multiplier =
            self.statuses
                .healing_received_bps(target, now, self.grid.config.status_rules.as_ref());
        let adjusted = u64::from(raw) * u64::from(multiplier) / 10000;
        let restored = adjusted.min(u64::from(max_hp.saturating_sub(hp))) as u32;
        if c.mechanism_version >= 25 {
            events.push(Event::HealResolved {
                source: source.clone(),
                target: target.clone(),
                heal_kind: kind,
                raw_heal: raw,
                multiplier_bps: multiplier,
                adjusted_heal: adjusted,
                hp_restored: restored,
            });
        }
        restored
    }
}
