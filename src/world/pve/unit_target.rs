//! Strict hostile unit identity checks shared by acceptance, commit and in-flight ownership.
use super::*;
impl Arena {
    fn unit_eligible(&self, i: usize) -> bool {
        let m = &self.monsters[i];
        m.enabled
            && m.hp > 0
            && !m.returning
            && m.encounter_index
                .is_none_or(|j| self.encounters[j].reset_at.is_none())
    }
    pub(super) fn unit_reference(&self, i: usize) -> Option<UnitTarget> {
        self.unit_eligible(i).then(|| UnitTarget {
            id: self.monsters[i].id.clone(),
            generation: self.monsters[i].status_generation,
        })
    }
    pub(super) fn unit_index(&self, t: &UnitTarget) -> Result<usize> {
        self.monsters
            .iter()
            .enumerate()
            .position(|(i, m)| {
                m.id == t.id && m.status_generation == t.generation && self.unit_eligible(i)
            })
            .ok_or(Error::InvalidTarget)
    }
    pub(super) fn unit_aim(
        &self,
        w: &World,
        s: &crate::config::SkillConfig,
        t: &UnitTarget,
    ) -> Result<[f64; 2]> {
        let i = self.unit_index(t)?;
        let p = self.monsters[i].position;
        if distance(w.position, p) > s.range || !self.grid.line(w.position, p, 0., 2) {
            return Err(Error::OutOfRange);
        }
        let d = normalize([p[0] - w.position[0], p[1] - w.position[1]]);
        if d == [0., 0.] {
            return Err(Error::OutOfRange);
        }
        Ok(d)
    }
}
