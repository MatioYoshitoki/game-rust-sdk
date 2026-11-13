use super::{Grid, segment_circle, segment_rect};
use crate::config::world_geometry::{BlockerWindow, WorldGeometry};

type Point = [f64; 2];

struct Edge {
    start: Point,
    tangent: Point,
    length: f64,
}

impl Edge {
    fn local(&self, p: Point) -> Point {
        let [dx, dy] = [p[0] - self.start[0], p[1] - self.start[1]];
        [
            dx * self.tangent[0] + dy * self.tangent[1],
            -dx * self.tangent[1] + dy * self.tangent[0],
        ]
    }
}

pub(super) struct Polygon {
    edges: Vec<Edge>,
    min: Point,
    max: Point,
}

impl Polygon {
    pub(super) fn new(vertices: &[Point]) -> Self {
        let edges = vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .map(|(&start, &end)| {
                let [dx, dy] = [end[0] - start[0], end[1] - start[1]];
                let length = dx.hypot(dy);
                Edge {
                    start,
                    tangent: [dx / length, dy / length],
                    length,
                }
            })
            .collect();
        let min = [0, 1].map(|axis| {
            vertices
                .iter()
                .map(|p| p[axis])
                .fold(f64::INFINITY, f64::min)
        });
        let max = [0, 1].map(|axis| {
            vertices
                .iter()
                .map(|p| p[axis])
                .fold(f64::NEG_INFINITY, f64::max)
        });
        Self { edges, min, max }
    }

    fn boundary_exit(&self, start: Point, end: Point, radius: f64) -> Option<f64> {
        self.edges
            .iter()
            .filter_map(|edge| {
                let from = edge.local(start)[1] - radius;
                let to = edge.local(end)[1] - radius;
                if from < 0. {
                    Some(0.)
                } else {
                    (to < 0.).then(|| from / (from - to))
                }
            })
            .min_by(f64::total_cmp)
    }

    pub(super) fn nearest_offset(&self, point: Point) -> Point {
        if self.edges.iter().all(|edge| edge.local(point)[1] >= 0.) {
            return [0.; 2];
        }
        let (edge, offset) = self
            .edges
            .iter()
            .map(|edge| {
                let [along, normal] = edge.local(point);
                (edge, [along - along.clamp(0., edge.length), normal])
            })
            .min_by(|(_, a), (_, b)| {
                (a[0] * a[0] + a[1] * a[1]).total_cmp(&(b[0] * b[0] + b[1] * b[1]))
            })
            .unwrap();
        let [x, y] = edge.tangent;
        [x * offset[0] - y * offset[1], y * offset[0] + x * offset[1]]
    }

    pub(super) fn blocker_contact(&self, start: Point, end: Point, radius: f64) -> Option<f64> {
        segment_rect(start, end, self.min, self.max, radius)?;
        if self.edges.iter().all(|edge| edge.local(start)[1] >= 0.) {
            return Some(0.);
        }
        // The disk-expanded polygon is the union of its interior and the edge capsules.
        self.edges
            .iter()
            .flat_map(|edge| {
                [
                    segment_rect(
                        edge.local(start),
                        edge.local(end),
                        [0., -radius],
                        [edge.length, radius],
                        0.,
                    ),
                    segment_circle(start, end, edge.start, radius),
                ]
                .into_iter()
                .flatten()
            })
            .min_by(f64::total_cmp)
    }
}

struct Blocker {
    id: String,
    shape: Polygon,
    channels: [bool; 3],
    window: Option<BlockerWindow>,
    active: bool,
}

pub(super) struct Geometry {
    boundary: Polygon,
    blockers: Vec<Blocker>,
}

impl Geometry {
    pub(super) fn new(config: &WorldGeometry) -> Self {
        let mut blockers = config
            .blockers
            .iter()
            .map(|b| Blocker {
                id: b.id.clone(),
                shape: Polygon::new(&b.vertices),
                channels: [b.blocks_movement, b.blocks_projectiles, b.blocks_vision],
                window: b.active_window,
                active: b.active_window.is_none_or(|window| window.start_ms == 0),
            })
            .collect::<Vec<_>>();
        blockers.sort_by(|a, b| a.id.cmp(&b.id));
        Self {
            boundary: Polygon::new(&config.boundary),
            blockers,
        }
    }

    fn contact(&self, start: Point, end: Point, radius: f64, kind: u8) -> Option<f64> {
        self.boundary
            .boundary_exit(start, end, radius)
            .into_iter()
            .chain(
                self.blockers
                    .iter()
                    .filter(|b| b.active && b.channels[kind as usize])
                    .filter_map(|b| b.shape.blocker_contact(start, end, radius)),
            )
            .min_by(f64::total_cmp)
    }
}

impl Grid {
    pub(super) fn geometry_contact(
        &self,
        start: Point,
        end: Point,
        radius: f64,
        kind: u8,
    ) -> Option<f64> {
        self.geometry.as_ref()?.contact(start, end, radius, kind)
    }

    pub(in crate::world) fn advance_blockers(
        &mut self,
        elapsed_ms: u64,
        occupants: &[(Point, f64)],
    ) -> bool {
        let Some(geometry) = &mut self.geometry else {
            return false;
        };
        let mut changed = false;
        for blocker in &mut geometry.blockers {
            let Some(window) = blocker.window else {
                continue;
            };
            let requested = (window.start_ms..window.end_ms).contains(&elapsed_ms);
            if requested
                && !blocker.active
                && blocker.channels[0]
                && occupants
                    .iter()
                    .any(|&(p, radius)| blocker.shape.blocker_contact(p, p, radius).is_some())
            {
                continue;
            }
            changed |= blocker.active != requested;
            blocker.active = requested;
        }
        changed
    }

    pub(in crate::world) fn active_blockers(&self) -> Option<Vec<String>> {
        self.geometry.as_ref().map(|geometry| {
            geometry
                .blockers
                .iter()
                .filter(|b| b.active)
                .map(|b| b.id.clone())
                .collect()
        })
    }
}
