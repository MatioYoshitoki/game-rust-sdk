//! Capsule geometry only; field scheduling and contact accounting stay shared.
use super::*;
fn closest(line: &LineFieldView, position: [f64; 2]) -> (f64, [f64; 2]) {
    let d = [line.end[0] - line.start[0], line.end[1] - line.start[1]];
    let length2 = d[0] * d[0] + d[1] * d[1];
    let t = if length2 == 0. {
        0.
    } else {
        (((position[0] - line.start[0]) * d[0] + (position[1] - line.start[1]) * d[1]) / length2)
            .clamp(0., 1.)
    };
    (t, [line.start[0] + t * d[0], line.start[1] + t * d[1]])
}
pub(in crate::world) fn order(line: &LineFieldView, position: [f64; 2]) -> (f64, f64) {
    let (t, p) = closest(line, position);
    (
        t,
        (position[0] - p[0]).powi(2) + (position[1] - p[1]).powi(2),
    )
}
impl Grid {
    pub(in crate::world) fn line_field_contact(
        &self,
        line: &LineFieldView,
        position: [f64; 2],
        radius: f64,
    ) -> bool {
        let (_, p) = closest(line, position);
        distance(p, position) <= line.width / 2. + radius && self.line(p, position, 0., 1)
    }
}
impl Arena {
    pub(super) fn line_origin(&self, w: &World, s: &SkillConfig) -> Result<()> {
        if self.grid.index(w.position).is_none() || !self.grid.clear(w.position, s.radius, 1) {
            return Err(Error::InvalidTarget);
        }
        Ok(())
    }
    pub(super) fn line_plan(
        &self,
        w: &World,
        s: &SkillConfig,
        direction: [f64; 2],
    ) -> Result<LineFieldView> {
        self.line_origin(w, s)?;
        let requested = [
            w.position[0] + direction[0] * s.range,
            w.position[1] + direction[1] * s.range,
        ];
        let t = self
            .grid
            .piercing_wall(w.position, requested, s.radius)
            .unwrap_or(1.);
        Ok(LineFieldView {
            start: w.position,
            end: [
                w.position[0] + (requested[0] - w.position[0]) * t,
                w.position[1] + (requested[1] - w.position[1]) * t,
            ],
            width: s.radius * 2.,
        })
    }
}
