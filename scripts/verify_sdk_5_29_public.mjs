// Consume the normal JSONL API using the delivered Node host.
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

function horizontalSpeed(frames) {
  assert.ok(frames.length >= 2, 'movement phase requires two published samples');
  const first = frames[0];
  const last = frames.at(-1);
  assert.ok(last.logical_ms > first.logical_ms);
  return (last.self.position[0] - first.self.position[0]) /
    (last.logical_ms - first.logical_ms);
}

async function verifyScenario(scenario, accountFile, preparationFactor, recoveryFactor) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({ catalog, scenario, accounts, manifest });
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const frames = [];
  const host = new TwoMemberHost({
    binary, manifest, database, record, autoAck: true,
    onPublication(recipient, _output, runtime) {
      if (recipient === 'P00') frames.push(runtime.member(recipient).snapshot);
    },
  });
  const actor = host.member('P00');
  const peer = host.member('P01');
  try {
    await host.start();
    await host.joinAndReady({ reverse: scenario === 'eight' });
    const peerStart = peer.snapshot.self.position[0];
    const skill = record.config.skill.id;
    await host.input('P00', { kind: 'cast', skill_id: skill, direction: [1, 0] });
    const movement = { kind: 'move', direction: [1, 0] };
    const move = await host.input('P00', movement);
    await host.input('P01', movement);
    await host.until(() => actor.hpCostStats.commits === 1 &&
      actor.snapshot.self.recovery_until_ms > 0 &&
      actor.snapshot.logical_ms >= actor.snapshot.self.recovery_until_ms + 420);
    const end = actor.snapshot.self.recovery_until_ms;
    const afterMove = frames.filter(frame => frame.logical_ms > move.logical_ms);
    const preparation = afterMove.filter(frame => frame.self.pending_action?.skill_id === skill);
    const recovery = afterMove.filter(frame => !frame.self.pending_action &&
      frame.self.recovery_until_ms === end && frame.logical_ms < end);
    const free = afterMove.filter(frame => frame.logical_ms >= end);
    const freeSpeed = horizontalSpeed(free);
    assert.ok(freeSpeed > 0);
    const preparationRatio = horizontalSpeed(preparation) / freeSpeed;
    const recoveryRatio = horizontalSpeed(recovery) / freeSpeed;
    assert.ok(Math.abs(preparationRatio - preparationFactor) < 1e-6);
    assert.ok(Math.abs(recoveryRatio - recoveryFactor) < 1e-6);
    assert.ok(peer.snapshot.self.position[0] > peerStart);
    assert.equal(peer.hpCostStats.commits, 0);
    assert.equal(actor.hpCostStats.total, 3);
    assert.ok(actor.flightStats.ownStarted > 0);
    await host.input('P00', { kind: 'stop' });
    await host.input('P01', { kind: 'stop' });

    const destination = actor.snapshot.extraction_point;
    await host.input('P00', { kind: 'navigate', destination });
    await host.until(() => Math.hypot(
      actor.snapshot.self.position[0] - destination[0],
      actor.snapshot.self.position[1] - destination[1],
    ) < 1);
    await host.input('P00', { kind: 'extract_start' });
    for (const member of host.members.slice(1)) {
      await host.input(member.account.participant_id, { kind: 'leave' });
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const accountsAfter = database.views(record.id);
    assert.ok(accountsAfter.every(account => account.admissions === 1 && account.settlements === 1));
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    assert.equal(host.errors.length, 0);
    return {
      scenario, members: host.members.length, preparationRatio, recoveryRatio,
      phaseSamples: { preparation: preparation.length, recovery: recovery.length, free: free.length },
      hpCost: actor.hpCostStats.total, peerContinued: true, settled: accountsAfter.length,
      phase: host.phase, pending: host.pending(), full: host.replicationStats.full, delta: host.replicationStats.delta,
    };
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const options of [['four', 'accounts-a.json', 0, 0.5], ['eight', 'accounts-b.json', 0.25, 0]]) {
  const evidence = await verifyScenario(...options);
  cases.push(evidence);
  console.log(JSON.stringify(evidence));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({ passed: true, binarySha256, cases }, null, 2) + '\n');
