use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorldGeometry {
    pub boundary: Vec<[f64; 2]>,
    pub blockers: Vec<WorldBlocker>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorldBlocker {
    pub id: String,
    pub vertices: Vec<[f64; 2]>,
    pub blocks_movement: bool,
    pub blocks_projectiles: bool,
    pub blocks_vision: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::protocol::present_value"
    )]
    pub active_window: Option<BlockerWindow>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlockerWindow {
    pub start_ms: u64,
    pub end_ms: u64,
}

pub(super) fn validate(c: &Config) -> Result<(), &'static str> {
    let Some(g) = c.pve.as_ref().and_then(|p| p.world_geometry.as_ref()) else {
        return Ok(());
    };
    if c.mechanism_version < 105 || c.protocol_version() != 6 || c.members.is_none() {
        return Err("CONFIG_WORLD_GEOMETRY_CAPABILITY");
    }
    let mut ids = BTreeSet::new();
    let within_map = |p: [f64; 2]| {
        (0..2).all(|axis| {
            p[axis].is_finite()
                && p[axis] >= c.geometry.min[axis]
                && p[axis] <= c.geometry.max[axis]
        })
    };
    let convex = |points: &[[f64; 2]]| {
        (3..=16).contains(&points.len())
            && points.iter().copied().all(within_map)
            && (0..points.len()).all(|i| {
                let next = (i + 1) % points.len();
                let a = points[i];
                let b = points[next];
                points.iter().enumerate().all(|(j, p)| {
                    j == i
                        || j == next
                        || (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) > 1e-8
                })
            })
    };
    if !convex(&g.boundary)
        || g.blockers.len() > 64
        || g.blockers.iter().any(|b| {
            !crate::protocol::valid_id(&b.id)
                || !ids.insert(&b.id)
                || !convex(&b.vertices)
                || !(b.blocks_movement || b.blocks_projectiles || b.blocks_vision)
                || b.active_window
                    .is_some_and(|w| w.start_ms >= w.end_ms || w.end_ms > c.terminal_ms)
        })
    {
        return Err("CONFIG_WORLD_GEOMETRY");
    }
    Ok(())
}
