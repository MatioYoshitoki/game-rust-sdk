use super::PveConfig;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IsometricGrid {
    pub tile_height: f64,
    pub origin: [f64; 2],
}

impl PveConfig {
    pub(crate) fn to_tile(&self, point: [f64; 2]) -> [f64; 2] {
        if let Some(iso) = self.isometric_grid {
            self.to_tile_vector([point[0] - iso.origin[0], point[1] - iso.origin[1]])
        } else {
            point.map(|coordinate| coordinate / self.cell_size)
        }
    }

    fn to_tile_vector(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let height = self.isometric_grid.unwrap().tile_height;
        let (x, y) = (x / self.cell_size, y / height);
        [x + y, y - x]
    }

    pub(crate) fn to_world(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        if let Some(iso) = self.isometric_grid {
            [
                iso.origin[0] + (x - y) * (self.cell_size / 2.),
                iso.origin[1] + (x + y) * (iso.tile_height / 2.),
            ]
        } else {
            [x * self.cell_size, y * self.cell_size]
        }
    }

    pub(crate) fn contains_world(&self, point: [f64; 2]) -> bool {
        self.to_tile(point)
            .into_iter()
            .zip([self.width, self.height])
            .all(|(coordinate, limit)| (0. ..limit as f64).contains(&coordinate))
    }

    pub(crate) fn tile_margin(&self, radius: f64) -> f64 {
        let height = self.isometric_grid.unwrap().tile_height;
        radius * self.cell_size.recip().hypot(height.recip())
    }

    pub(crate) fn world_bounds(&self) -> ([f64; 2], [f64; 2]) {
        let (w, h) = (self.width as f64, self.height as f64);
        if self.isometric_grid.is_some() {
            (
                [self.to_world([0., h])[0], self.to_world([0., 0.])[1]],
                [self.to_world([w, 0.])[0], self.to_world([w, h])[1]],
            )
        } else {
            ([0.; 2], self.to_world([w, h]))
        }
    }

    pub(crate) fn map_diagonal(&self) -> f64 {
        let (lo, hi) = self.world_bounds();
        (hi[0] - lo[0]).hypot(hi[1] - lo[1])
    }

    pub(crate) fn isometric_ray_limit(
        &self,
        origin: [f64; 2],
        direction: [f64; 2],
        radius: f64,
    ) -> Option<f64> {
        self.isometric_grid?;
        let origin = self.to_tile(origin);
        let direction = self.to_tile_vector(direction);
        let margin = self.tile_margin(radius);
        let limits = [self.width as f64, self.height as f64];
        let distance = [0, 1]
            .into_iter()
            .filter_map(|axis| {
                let delta = direction[axis];
                if delta > 0. {
                    Some((limits[axis] - margin - origin[axis]) / delta)
                } else if delta < 0. {
                    Some((margin - origin[axis]) / delta)
                } else {
                    None
                }
            })
            .min_by(f64::total_cmp)
            .unwrap_or(f64::INFINITY);
        Some(distance.max(0.))
    }
}
