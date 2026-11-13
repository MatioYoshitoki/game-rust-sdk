import test from 'node:test';
import assert from 'node:assert/strict';
import { project, unproject, corners, createViewport, display, plain, projectionFor, toWorld, tilePolygon } from '../../web/isometric.js';

const near = (actual, expected) => actual.forEach((value, i) => assert.ok(Math.abs(value - expected[i]) < 1e-8, `${actual} != ${expected}`));

test('world points survive projection and pointer inversion, including negative and fractional coordinates', () => {
  for (const point of [[0, 0], [-512, 512], [250.25, -79.75], [1e6, -2e6]]) near(unproject(project(point)), point);
});

test('screen direction inputs keep their intended axis and diagonal after world conversion', () => {
  for (const direction of [[0, -1], [1, 0], [0, 1], [-1, 0], [1, -1], [-1, 1]]) {
    const world = unproject(direction);
    const length = Math.hypot(...world);
    const rendered = project(world.map(value => value / length));
    assert.ok(rendered[0] * direction[0] + rendered[1] * direction[1] > 0);
    assert.ok(Math.abs(rendered[0] * direction[1] - rendered[1] * direction[0]) < 1e-8);
  }
});

test('desktop and mobile viewports fit shifted rectangular worlds and invert screen aim after resize', () => {
  for (const [width, height] of [[1010, 590], [356, 380], [610, 580]]) {
    for (const bounds of [{ min: [-512, -512], max: [512, 512] }, { min: [100, -200], max: [1200, 300] }]) {
      const view = createViewport(width, height, bounds);
      for (const point of [...corners(bounds), [(bounds.min[0] + bounds.max[0]) / 2, bounds.min[1]]]) {
        const screen = view.screen(point);
        assert.ok(screen[0] >= 32 - 1e-8 && screen[0] <= width - 32 + 1e-8);
        assert.ok(screen[1] >= 60 - 1e-8 && screen[1] <= height - 44 + 1e-8);
        near(view.world(screen), point);
      }
    }
  }
});

test('world axes form a 2:1 diamond and billboard elevation leaves the ground anchor intact', () => {
  const view = createViewport(1000, 600, { min: [-512, -512], max: [512, 512] });
  const center = view.screen([0, 0]);
  for (const point of [[64, 0], [0, 64]]) {
    const foot = view.screen(point); const raised = view.screen(point, 24);
    assert.ok(Math.abs(Math.abs(foot[0] - center[0]) - 2 * (foot[1] - center[1])) < 1e-8);
    near(raised, [foot[0], foot[1] - 24]);
  }
});

// Reference numbers mirror the kernel contract test: cell_size 64, tile_height 32, origin [-100, 20].
const isoPve = { cell_size: 64, isometric_grid: { tile_height: 32, origin: [-100, 20] }, width: 6, height: 6 };
const flatPve = { cell_size: 32, width: 4, height: 4 };

test('an authoritative isometric grid renders through the plain projection and orthogonal grids keep the 2:1 display', () => {
  assert.equal(projectionFor(isoPve), plain);
  assert.equal(projectionFor(flatPve), display);
  assert.equal(projectionFor(null), display);
  for (const point of [[0, 0], [-292, 20], [92, 212], [12.5, -7.25]]) near(plain.unproject(plain.project(point)), point);
  near(plain.project([-68, 84]), [-68, 84]);
});

test('grid corners map to world points exactly as the kernel does', () => {
  near(toWorld(isoPve, [0, 0]), [-100, 20]);
  near(toWorld(isoPve, [1.5, 1.5]), [-100, 68]);
  near(toWorld(isoPve, [6, 0]), [92, 116]);
  near(toWorld(isoPve, [0, 6]), [-292, 116]);
  near(toWorld(isoPve, [6, 6]), [-100, 212]);
  near(toWorld(flatPve, [1, 2]), [32, 64]);
});

test('tile polygons are diamonds for isometric grids and squares for orthogonal ones', () => {
  assert.deepEqual(tilePolygon(isoPve, 1, 1), [[-100, 52], [-68, 68], [-100, 84], [-132, 68]]);
  assert.deepEqual(tilePolygon(flatPve, 1, 2), [[32, 64], [64, 64], [64, 96], [32, 96]]);
  // The diamond diagonals match the configured full tile width and height.
  const diamond = tilePolygon(isoPve, 1, 1);
  assert.equal(Math.abs(diamond[1][0] - diamond[3][0]), 64);
  assert.equal(Math.abs(diamond[2][1] - diamond[0][1]), 32);
});

test('plain viewports fit the projected isometric bounds, invert clicks, and keep world distances uniform', () => {
  for (const [width, height] of [[1010, 590], [356, 380]]) {
    const bounds = { min: [-292, 20], max: [92, 212] };
    const view = createViewport(width, height, bounds, plain);
    for (const point of [...corners(bounds), [-100, 68]]) {
      const screen = view.screen(point);
      assert.ok(screen[0] >= 32 - 1e-8 && screen[0] <= width - 32 + 1e-8);
      assert.ok(screen[1] >= 60 - 1e-8 && screen[1] <= height - 44 + 1e-8);
      near(view.world(screen), point);
    }
    const a = view.screen([-100, 52]), b = view.screen([-68, 84]);
    const expected = Math.hypot(32, 32) * view.scale;
    assert.ok(Math.abs(Math.hypot(b[0] - a[0], b[1] - a[1]) - expected) < 1e-8);
  }
});
