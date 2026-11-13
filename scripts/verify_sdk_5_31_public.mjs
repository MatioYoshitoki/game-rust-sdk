// Use the ordinary JSONL host and published charge facts with external configs.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const [binaryPath, hostPath, configPath, outputPath] = process.argv.slice(2);
const binary = resolve(binaryPath);
const configs = resolve(configPath);
const output = resolve(outputPath);
mkdirSync(output, { recursive: true });
const { TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog } =
  await import(pathToFileURL(resolve(hostPath)).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);

async function verifyScenario(scenario, accountFile, automatic) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({ catalog, scenario, accounts, manifest });
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const commits = [];
  const projectiles = [];
  const host = new TwoMemberHost({
    binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private' && event.event.kind === 'action_committed' && event.event.charge) {
        commits.push({ recipient, ...event.event });
      }
      if (event.kind === 'projectile_started' && event.projectile.source.id === 'P00') {
        projectiles.push(event.projectile);
      }
    },
  });
  const actor = host.member('P00');
  const peer = host.member('P01');
  const skill = record.config.skill;
  const start = { kind: 'start_charge', skill_id: skill.id, direction: [1, 0] };
  const pending = () => actor.snapshot?.self.pending_action?.charge;
  try {
    await host.start();
    await host.joinAndReady({ reverse: automatic });
    await assert.rejects(host.input('P00', { ...start, kind: 'cast' }), { code: 'UNSUPPORTED' });
    await host.input('P00', start);
    await host.until(() => !!pending());
    const charge = pending();
    assert.deepEqual(charge.aim.direction, [1, 0]);
    assert.equal(charge.aim.updated_at_ms, charge.started_at_ms);
    const peerStart = peer.snapshot.self.position[0];
    const update = { kind: 'update_charge_aim', action_id: charge.action_id, direction: [1, 1] };
    await assert.rejects(host.input('P01', update), { code: 'INVALID_TARGET' });
    await host.input('P01', { kind: 'move', direction: [1, 0] });
    const interval = skill.charge.aim_update.min_interval_ms;
    await host.until(() => actor.snapshot.logical_ms >= charge.started_at_ms + interval);
    const updated = await host.input('P00', update);
    await host.until(() => pending()?.aim.updated_at_ms === updated.logical_ms);
    const acceptedAim = pending().aim;
    assert.ok(Math.abs(acceptedAim.direction[0] - Math.SQRT1_2) < 1e-9);
    assert.ok(Math.abs(acceptedAim.direction[1] - Math.SQRT1_2) < 1e-9);
    await host.input('P00', { ...update, direction: [2, 2] });
    await host.pump();
    assert.deepEqual(pending().aim, acceptedAim);
    assert.equal(commits.length, 0);
    assert.equal(actor.hpCostStats.commits, 0);
    assert.equal(peer.snapshot.self.pending_action, null);
    assert.ok(actor.snapshot.members.every(member => !Object.hasOwn(member, 'pending_action')));
    let heldMs = skill.charge.max_ms;
    let finalDirection = acceptedAim.direction;
    if (!automatic) {
      await host.until(() => actor.snapshot.logical_ms >= Math.max(
        charge.min_release_at_ms, acceptedAim.updated_at_ms + interval,
      ));
      finalDirection = [0, 1];
      const released = await host.input('P00', {
        kind: 'release_charge', action_id: charge.action_id, direction: finalDirection,
      });
      heldMs = released.logical_ms - charge.started_at_ms;
      assert.ok(heldMs >= skill.charge.min_ms && heldMs < skill.charge.max_ms);
    }
    await host.until(() => commits.length === 1 && !pending() && projectiles.length === 1);
    const [commit] = commits;
    const { min_ms: minimum, max_ms: maximum, min_damage_bps: floor } = skill.charge;
    const expectedFactor = floor + Math.floor((10000 - floor) * (heldMs - minimum) / (maximum - minimum));
    assert.deepEqual(commit.charge, { held_ms: heldMs, damage_factor_bps: expectedFactor, direction: finalDirection });
    assert.equal(commit.recipient, 'P00');
    assert.equal(commit.hp_cost, 3);
    assert.equal(commit.spirit_cost, skill.cost);
    assert.equal(Boolean(projectiles[0].piercing), automatic);
    if (automatic) {
      assert.deepEqual(projectiles[0].piercing.direction, finalDirection);
    } else {
      const shot = projectiles[0];
      await host.until(() => actor.snapshot.projectiles.some(projectile => projectile.id === shot.id &&
        Math.abs(projectile.position[0] - shot.position[0]) < 1e-9 && projectile.position[1] > shot.position[1]));
    }
    await assert.rejects(host.input('P00', update), { code: 'INVALID_TARGET' });
    assert.ok(peer.snapshot.self.position[0] > peerStart);
    assert.equal(peer.hpCostStats.commits, 0);
    await assert.rejects(host.input('P00', { kind: 'release_charge', action_id: commit.action_id }),
      { code: 'INVALID_TARGET' });
    await host.input('P01', { kind: 'stop' });

    const destination = actor.snapshot.extraction_point;
    await host.input('P00', { kind: 'navigate', destination });
    await host.until(() => actor.snapshot.extraction_open && Math.hypot(
      actor.snapshot.self.position[0] - destination[0],
      actor.snapshot.self.position[1] - destination[1],
    ) < 1);
    await host.input('P00', { kind: 'extract_start' });
    for (const member of host.members.slice(1)) {
      await host.input(member.account.participant_id, { kind: 'leave' });
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(commits.length, 1);
    assert.equal(actor.hpCostStats.commits, 1);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {
      scenario, members: host.members.length, automatic, heldMs, factorBps: expectedFactor,
      acceptedDirection: acceptedAim.direction, finalDirection, sameDirectionDedup: true,
      hpCost: commit.hp_cost, spiritCost: commit.spirit_cost,
      piercing: Boolean(projectiles[0].piercing), peerContinued: true, settled: ledgers.length,
      phase: host.phase, pending: host.pending(), full: host.replicationStats.full, delta: host.replicationStats.delta,
    };
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const options of [['four', 'accounts-a.json', false], ['eight', 'accounts-b.json', true]]) {
  const evidence = await verifyScenario(...options);
  cases.push(evidence);
  console.log(JSON.stringify(evidence));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({ passed: true, binarySha256, cases }, null, 2) + '\n');
