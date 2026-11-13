use super::Grid;
use crate::config::{Terrain, TerrainAxis};

impl Grid {
    pub(in crate::world) fn terrain_at(&self, point: [f64; 2]) -> Option<&Terrain> {
        let index = self.index(point)?;
        Some(&self.config.palette[self.cells[index]])
    }

    pub(super) fn terrain_step_contact(&self, start: [f64; 2], end: [f64; 2]) -> Option<f64> {
        if !self.terrain_height_enabled || start == end {
            return None;
        }
        self.index(start)?;
        let (start, end, size) = if self.isometric_tiles.is_some() {
            (self.config.to_tile(start), self.config.to_tile(end), 1.)
        } else {
            (start, end, self.config.cell_size)
        };
        let cell_of = |point: [f64; 2]| point.map(|coordinate| (coordinate / size).floor() as i32);
        let mut cell = cell_of(start);
        let last = cell_of(end);
        let delta = [end[0] - start[0], end[1] - start[1]];
        let direction = delta.map(|value| if value > 0.0 { 1 } else { -1 });
        while cell != last {
            let mut times = [f64::INFINITY; 2];
            for axis in 0..2 {
                if cell[axis] != last[axis] {
                    let edge = cell[axis] + i32::from(direction[axis] > 0);
                    times[axis] = (f64::from(edge) * size - start[axis]) / delta[axis];
                }
            }
            let t = times[0].min(times[1]).clamp(0.0, 1.0);
            let point = [start[0] + delta[0] * t, start[1] + delta[1] * t];
            let mut next = cell;
            if (times[0] - times[1]).abs() <= 1e-12 {
                // A corner can be crossed only when neither side hides a blocked seam.
                next = [cell[0] + direction[0], cell[1] + direction[1]];
                for adjacent in [[next[0], cell[1]], [cell[0], next[1]]] {
                    if !self.terrain_step_allowed(cell, adjacent, point)?
                        || !self.terrain_step_allowed(adjacent, next, point)?
                    {
                        return Some(t);
                    }
                }
            } else {
                let axis = usize::from(times[1] < times[0]);
                next[axis] += direction[axis];
                if !self.terrain_step_allowed(cell, next, point)? {
                    return Some(t);
                }
            }
            cell = next;
        }
        None
    }

    // Leaving the grid is handled by the enclosing boundary/footprint check.
    fn terrain_step_allowed(&self, from: [i32; 2], to: [i32; 2], point: [f64; 2]) -> Option<bool> {
        let tile = |[x, y]: [i32; 2]| {
            let (x, y) = (x as usize, y as usize);
            (x < self.config.width && y < self.config.height)
                .then(|| &self.config.palette[self.cells[y * self.config.width + x]])
        };
        let height = |tile: &Terrain, cell: [i32; 2]| {
            tile.slope.map_or(tile.elevation, |slope| {
                let axis = match slope.axis {
                    TerrainAxis::X => 0,
                    TerrainAxis::Y => 1,
                };
                let size = if self.isometric_tiles.is_some() {
                    1.
                } else {
                    self.config.cell_size
                };
                let progress = (point[axis] / size - f64::from(cell[axis])).clamp(0.0, 1.0);
                tile.elevation + progress * (slope.end_elevation - tile.elevation)
            })
        };
        let departure = tile(from)?;
        let change = height(tile(to)?, to) - height(departure, from);
        Some(
            change <= departure.max_climb_height + 1e-8
                && -change <= departure.max_drop_height + 1e-8,
        )
    }
}
