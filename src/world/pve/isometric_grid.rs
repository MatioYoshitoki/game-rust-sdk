use super::{Grid, grid_geometry::Polygon};
use crate::config::PveConfig;
use std::ops::Range;

type Point = [f64; 2];

pub(super) struct IsometricTiles {
    diamond: Polygon,
}

impl IsometricTiles {
    pub(super) fn new(config: &PveConfig) -> Option<Self> {
        let half_width = config.cell_size / 2.;
        let height = config.isometric_grid?.tile_height;
        Some(Self {
            diamond: Polygon::new(&[
                [0., 0.],
                [half_width, height / 2.],
                [0., height],
                [-half_width, height / 2.],
            ]),
        })
    }
}

fn cells(lo: f64, hi: f64, limit: usize) -> Range<usize> {
    ((lo.floor() as usize).min(limit))..(((hi.floor() + 1.) as usize).min(limit))
}

fn interval(start: f64, end: f64, lo: f64, hi: f64) -> Option<[f64; 2]> {
    if start == end {
        return (lo..=hi).contains(&start).then_some([0., 1.]);
    }
    let a = (lo - start) / (end - start);
    let b = (hi - start) / (end - start);
    let first = a.min(b).max(0.);
    let last = a.max(b).min(1.);
    (first <= last).then_some([first, last])
}

impl Grid {
    fn isometric_fits(&self, point: Point, radius: f64) -> bool {
        let margin = self.config.tile_margin(radius);
        self.config
            .to_tile(point)
            .into_iter()
            .zip([self.config.width, self.config.height])
            .all(|(value, limit)| value >= margin && value <= limit as f64 - margin)
    }

    fn any_isometric_blocker(
        &self,
        start: Point,
        end: Point,
        radius: f64,
        kind: u8,
        mut intersects: impl FnMut(&Polygon, Point) -> bool,
    ) -> bool {
        let shape = &self.isometric_tiles.as_ref().unwrap().diamond;
        let a = self.config.to_tile(start);
        let b = self.config.to_tile(end);
        let margin = self.config.tile_margin(radius.max(1e-7)) + 1e-12;
        let delta_y = b[1] - a[1];
        // Clip each column to the swept tile-coordinate interval before visiting its rows.
        for x in cells(
            a[0].min(b[0]) - margin,
            a[0].max(b[0]) + margin,
            self.config.width,
        ) {
            let left = x as f64 - margin;
            let Some([from, to]) = interval(a[0], b[0], left, x as f64 + 1. + margin) else {
                continue;
            };
            let (from_y, to_y) = (a[1] + delta_y * from, a[1] + delta_y * to);
            for y in cells(
                from_y.min(to_y) - margin,
                from_y.max(to_y) + margin,
                self.config.height,
            ) {
                let tile = &self.config.palette[self.cells[y * self.config.width + x]];
                let blocked = match kind {
                    1 => tile.blocks_projectiles,
                    2 => tile.blocks_vision,
                    _ => !tile.walkable,
                };
                if blocked && intersects(shape, self.config.to_world([x as f64, y as f64])) {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn clear_isometric_tiles(&self, point: Point, radius: f64, kind: u8) -> bool {
        self.isometric_fits(point, radius)
            && !self.any_isometric_blocker(point, point, radius, kind, |tile, origin| {
                let offset = tile.nearest_offset([point[0] - origin[0], point[1] - origin[1]]);
                offset[0].hypot(offset[1]) < radius.max(1e-7)
            })
    }

    pub(super) fn isometric_tile_contact(
        &self,
        start: Point,
        end: Point,
        radius: f64,
        kind: u8,
    ) -> Option<f64> {
        let delta = [end[0] - start[0], end[1] - start[1]];
        let mut first = if self.isometric_fits(end, radius) {
            None
        } else {
            self.config
                .isometric_ray_limit(start, delta, radius)
                .map(|t| t.clamp(0., 1.))
        };
        self.any_isometric_blocker(start, end, radius, kind, |tile, origin| {
            let a = [start[0] - origin[0], start[1] - origin[1]];
            let b = [end[0] - origin[0], end[1] - origin[1]];
            if kind == 0 {
                let offset = tile.nearest_offset(a);
                // A disk resting on a tile may move along or away from the tangent.
                if offset[0].hypot(offset[1]) <= radius
                    && offset[0] * delta[0] + offset[1] * delta[1] >= 0.
                {
                    return false;
                }
            }
            if let Some(contact) = tile.blocker_contact(a, b, radius) {
                first = Some(first.map_or(contact, |old| old.min(contact)));
            }
            first == Some(0.)
        });
        first
    }
}
