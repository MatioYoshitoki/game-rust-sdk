// Use the ordinary JSONL host and published locked basic targets with external configs.
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

async function verifyScenario(scenario, accountFile, replaceTarget) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({ catalog, scenario, accounts, manifest });
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const commits = [];
  const projectiles = [];
  const host = new TwoMemberHost({
    binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private') {
        const fact = { recipient, ...event.event };
        if (fact.kind === 'action_committed' && !fact.skill) commits.push(fact);
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
  const attack = target => host.input('P00', {kind: 'attack_unit', target});
  try {
    await host.start();
    await host.joinAndReady({ reverse: replaceTarget });
    const peerStart = peer.snapshot.self.position[0];
    await host.input('P01', {kind: 'move', direction: [1, 0]});
    await host.until(() => peer.snapshot.self.position[0] > peerStart);
    await host.input('P01', {kind: 'stop'});
    const candidates = actor.snapshot.monsters.filter(npc => npc.hp > 0 && !npc.returning)
      .map(npc => ({npc, distance: Math.hypot(...npc.position.map((p, i) => p - own().position[i]))}))
      .filter(row => row.distance <= basic.range)
      .sort((a, b) => a.distance - b.distance || a.npc.entity.id.localeCompare(b.npc.entity.id));
    assert.ok(candidates.length >= 2 && candidates[0].distance < candidates[1].distance);
    const [near, chosen] = candidates.slice(0, 2).map(({npc: {entity: {id, generation}}}) => ({id, generation}));
    await assert.rejects(attack({...chosen, generation: chosen.generation + 1}), {code: 'INVALID_TARGET'});
    await attack(chosen);
    await host.until(() => !!own().pending_action?.unit_target);
    assert.deepEqual(own().pending_action.unit_target, chosen);
    assert.equal(own().pending_action.skill_id, null);
    assert.ok(!peer.snapshot.self.pending_action);
    await host.until(() => commits.length === 1 && projectiles.length === 1);
    assert.deepEqual(commits[0].unit_target, chosen);
    assert.ok(!projectiles[0].unit_target);
    let latest = null;
    if (replaceTarget) {
      await attack(chosen);
      await host.until(() => own().basic_followup?.target.id === chosen.id);
      const first = own().basic_followup;
      await attack(near);
      await host.until(() => own().basic_followup?.target.id === near.id);
      latest = own().basic_followup;
      assert.notEqual(latest.action_id, first.action_id);
      assert.equal(latest.promotes_at_ms, first.promotes_at_ms);
      assert.deepEqual(latest.target, near);
      assert.equal(commits.length, 1);
      await host.until(() => commits.length === 2 && projectiles.length === 2);
      assert.equal(commits[1].action_id, latest.action_id);
      assert.deepEqual(commits[1].unit_target, near);
      assert.ok(!projectiles[1].unit_target);
    }
    assert.ok(commits.every(fact => fact.recipient === 'P00' && fact.spirit_cost === 0 && !Object.hasOwn(fact, 'hp_cost')));

    for (const member of host.members) {
      await host.input(member.account.participant_id, { kind: 'leave' });
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(commits.length, replaceTarget ? 2 : 1);
    assert.equal(actor.hpCostStats.commits, 0);
    assert.equal(projectiles.length, commits.length);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {
      scenario, members: host.members.length, outcome: replaceTarget ? 'retargeted_queue' : 'non_nearest',
      projectiles: projectiles.length, commits: commits.length, firstTarget: chosen, nextTarget: latest?.target ?? null,
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
