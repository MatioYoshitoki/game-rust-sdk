// Use the ordinary JSONL host and published pursuit facts with external configs.
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

async function verifyScenario(scenario, accountFile, cancelled) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({ catalog, scenario, accounts, manifest });
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const commits = [];
  const projectiles = [];
  const interruptions = [];
  const host = new TwoMemberHost({
    binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private') {
        const fact = { recipient, ...event.event };
        if (fact.kind === 'action_committed' && !fact.skill) commits.push(fact);
        if (fact.kind === 'action_interrupted') interruptions.push(fact);
      }
      if (event.kind === 'projectile_started' && event.projectile.source.id === 'P00') {
        projectiles.push(event.projectile);
      }
    },
  });
  const actor = host.member('P00');
  const peer = host.member('P01');
  const basic = record.config.basic;
  const own = () => actor.snapshot.self;
  try {
    await host.start();
    await host.joinAndReady({ reverse: cancelled });
    const peerStart = peer.snapshot.self.position[0];
    await host.input('P01', {kind: 'move', direction: [1, 0]});
    await host.until(() => peer.snapshot.self.position[0] > peerStart);
    await host.input('P01', {kind: 'stop'});
    const origin = own().position;
    const npc = actor.snapshot.monsters[0];
    const target = {id: npc.entity.id, generation: npc.entity.generation};
    const distance = (a, b) => Math.hypot(...a.map((x, i) => x - b[i]));
    assert.ok(distance(origin, npc.position) > basic.range);
    const autoRange = record.config.basic_pursuit.auto_max_range;
    const gap = distance(origin, npc.position);
    assert.ok(gap <= record.config.basic_pursuit.max_range);
    if (cancelled) {
      assert.ok(gap > autoRange);
      await assert.rejects(host.input('P00', {kind: 'attack'}), {code: 'INVALID_TARGET'});
      assert.equal(own().basic_pursuit, undefined);
      assert.deepEqual(own().position, origin);
      await host.input('P00', {kind: 'attack_unit', target});
    } else {
      assert.ok(gap <= autoRange);
      await host.input('P00', {kind: 'attack'});
    }
    await host.until(() => !!own().basic_pursuit);
    const pursuit = own().basic_pursuit;
    assert.deepEqual(pursuit.target, target);
    assert.equal(pursuit.expires_at_ms - pursuit.started_at_ms, record.config.basic_pursuit.duration_ms);
    assert.ok(!peer.snapshot.self.basic_pursuit);
    assert.ok(actor.snapshot.members.every(member => !Object.hasOwn(member, 'basic_pursuit')));
    await host.until(() => distance(origin, own().position) > 0 && !!own().basic_pursuit);
    assert.equal(own().basic_ready_at_ms, 0);
    assert.equal(commits.length, 0);
    let travelled;
    if (cancelled) {
      await host.input('P00', {kind: 'cancel_action'});
      await host.until(() => !own().basic_pursuit && interruptions.length === 1);
      const cancelledAt = own().position;
      travelled = distance(origin, cancelledAt);
      await host.until(() => actor.snapshot.logical_ms >= pursuit.expires_at_ms + basic.windup_ms);
      assert.deepEqual(own().position, cancelledAt);
      assert.equal(own().basic_ready_at_ms, 0);
      assert.equal(commits.length, 0);
    } else {
      await host.until(() => !!own().pending_action?.unit_target && !own().basic_pursuit);
      const reached = own().position;
      travelled = distance(origin, reached);
      assert.deepEqual(own().pending_action.unit_target, target);
      const commitsAt = own().pending_action.commits_at_ms;
      assert.ok(distance(reached, npc.position) <= basic.range);
      assert.equal(commits.length, 0);
      await host.until(() => commits.length === 1 && projectiles.length === 1);
      assert.equal(commits[0].action_id, pursuit.action_id);
      assert.deepEqual(commits[0].unit_target, target);
      assert.deepEqual(projectiles[0].position, reached);
      assert.equal(own().basic_ready_at_ms, commitsAt + basic.interval_ms);
      assert.ok(own().basic_ready_at_ms > actor.snapshot.logical_ms);
    }
    assert.ok(travelled > 0);

    for (const member of host.members) {
      await host.input(member.account.participant_id, { kind: 'leave' });
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(commits.length, cancelled ? 0 : 1);
    assert.equal(actor.hpCostStats.commits, 0);
    assert.equal(projectiles.length, commits.length);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {
      scenario, members: host.members.length, outcome: cancelled ? 'cancelled' : 'committed',
      mode: cancelled ? 'explicit' : 'auto', autoRejected: cancelled,
      projectiles: projectiles.length, commits: commits.length, target, travelled,
      hpCost: actor.hpCostStats.total, peerContinued: true, settled: ledgers.length,
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
