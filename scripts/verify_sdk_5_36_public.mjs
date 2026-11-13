// Use the ordinary JSONL host and published basic attack facts with external configs.
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
  const interruptions = [];
  const projectiles = [];
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
  const attack = () => host.input('P00', {kind: 'attack'});
  try {
    await host.start();
    await host.joinAndReady({ reverse: cancelled });
    const peerStart = peer.snapshot.self.position[0];
    await host.input('P01', {kind: 'move', direction: [1, 0]});
    await host.until(() => peer.snapshot.self.position[0] > peerStart);
    await host.input('P01', {kind: 'stop'});
    await attack();
    await host.until(() => commits.length === 1 && projectiles.length === 1);
    const ready = own().basic_ready_at_ms;
    assert.ok(ready > actor.snapshot.logical_ms);
    await attack();
    await host.until(() => !!own().basic_followup);
    const first = own().basic_followup;
    assert.equal(first.promotes_at_ms, ready);
    assert.equal(first.expires_at_ms, ready + record.config.basic_followup.grace_ms);
    assert.equal(own().pending_action, null);
    assert.ok(!peer.snapshot.self.basic_followup);
    assert.ok(actor.snapshot.members.every(member => !Object.hasOwn(member, 'basic_followup')));
    let last = first;
    if (cancelled) {
      await host.input('P00', {kind: 'cancel_action'});
      await host.until(() => !own().basic_followup && interruptions.length === 1);
      await host.until(() => actor.snapshot.logical_ms >= first.expires_at_ms + basic.windup_ms);
      assert.equal(own().basic_ready_at_ms, ready);
    } else {
      await attack();
      await host.until(() => own().basic_followup?.action_id !== first.action_id && !!own().basic_followup);
      last = own().basic_followup;
      assert.equal(last.promotes_at_ms, ready);
      assert.equal(own().basic_ready_at_ms, ready);
      assert.equal(commits.length, 1);
      await host.until(() => !own().basic_followup && own().pending_action !== null);
      assert.equal(own().pending_action.skill_id, null);
      assert.equal(own().pending_action.commits_at_ms, ready + basic.windup_ms);
      await host.until(() => commits.length === 2 && projectiles.length === 2);
      assert.equal(commits[1].action_id, last.action_id);
      assert.equal(own().basic_ready_at_ms, ready + basic.windup_ms + basic.interval_ms);
    }
    assert.ok(commits.every(fact => fact.recipient === 'P00' && fact.spirit_cost === 0 && !Object.hasOwn(fact, 'hp_cost')));

    for (const member of host.members) {
      await host.input(member.account.participant_id, { kind: 'leave' });
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(commits.length, cancelled ? 1 : 2);
    assert.equal(actor.hpCostStats.commits, 0);
    assert.equal(projectiles.length, commits.length);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {
      scenario, members: host.members.length, outcome: cancelled ? 'cancelled' : 'committed',
      projectiles: projectiles.length, commits: commits.length, latestReplaced: last.action_id !== first.action_id,
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
