// World coordinates stay orthogonal. Only the view uses a 2:1 diamond projection.
export function project([x, y]) { return [x - y, (x + y) / 2]; }
export function unproject([x, y]) { return [x / 2 + y, y - x / 2]; }

// An authoritative isometric grid already lays diamond tiles out in world
// coordinates; the browser renders those world positions with a plain scale.
export const display = { project, unproject };
export const plain = { project: point => point, unproject: point => point };

export function projectionFor(pve) { return pve?.isometric_grid ? plain : display; }

// Mirrors the kernel's to_world: a grid corner (u, v) maps to a world point.
export function toWorld(pve, [u, v]) {
  if (pve?.isometric_grid) {
    const { origin, tile_height } = pve.isometric_grid;
    return [origin[0] + (u - v) * pve.cell_size / 2, origin[1] + (u + v) * tile_height / 2];
  }
  return [u * pve.cell_size, v * pve.cell_size];
}

// The four world vertices of tile (u, v): diamonds for isometric grids, axis-aligned squares otherwise.
export function tilePolygon(pve, u, v) {
  const next = [u + 1, v + 1];
  if (pve?.isometric_grid) {
    return [toWorld(pve, [u, v]), toWorld(pve, [u + 1, v]), toWorld(pve, next), toWorld(pve, [u, v + 1])];
  }
  const [x1, y1] = toWorld(pve, next);
  const [x0, y0] = toWorld(pve, [u, v]);
  return [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
}

export function corners({ min: [x0, y0], max: [x1, y1] }) {
  return [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
}

export function createViewport(width, height, bounds, projection = display) {
  const points = corners(bounds).map(projection.project);
  const min = [Math.min(...points.map(p => p[0])), Math.min(...points.map(p => p[1]))];
  const max = [Math.max(...points.map(p => p[0])), Math.max(...points.map(p => p[1]))];
  const scale = Math.max(.001, Math.min((width - 64) / (max[0] - min[0]), (height - 104) / (max[1] - min[1])));
  const origin = [width / 2 - (min[0] + max[0]) * scale / 2,
    height / 2 + 8 - (min[1] + max[1]) * scale / 2];
  return {
    scale, origin,
    screen(point, elevation = 0) {
      const [x, y] = projection.project(point);
      return [origin[0] + x * scale, origin[1] + y * scale - elevation];
    },
    world(point) { return projection.unproject([(point[0] - origin[0]) / scale, (point[1] - origin[1]) / scale]); },
  };
}
