// Use the ordinary JSONL host and published redirect facts with external configs.
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

async function verifyScenario(scenario, accountFile, expire) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({ catalog, scenario, accounts, manifest });
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const commits = [];
  const redirects = [];
  const projectiles = [];
  const host = new TwoMemberHost({
    binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private') {
        const fact = { recipient, ...event.event };
        if (fact.kind === 'action_committed') commits.push(fact);
        if (fact.kind === 'projectile_recast') redirects.push(fact);
      }
      if (event.kind === 'projectile_started' && event.projectile.source.id === 'P00') {
        projectiles.push(event.projectile);
      }
    },
  });
  const actor = host.member('P00');
  const peer = host.member('P01');
  const skill = record.config.skill;
  const opportunity = () => actor.snapshot?.self.projectile_recast;
  try {
    await host.start();
    await host.joinAndReady({ reverse: expire });
    const origin = actor.snapshot.self.position;
    const peerStart = peer.snapshot.self.position[0];
    await host.input('P00', { kind: 'cast', skill_id: skill.id, direction: [1, 0] });
    await host.until(() => !!opportunity() && commits.length === 1 && projectiles.length === 1);
    const window = opportunity();
    const [commit] = commits;
    const action = { kind: 'recast_projectile', action_id: window.action_id, recast_index: window.next_index, direction: [0, 1] };
    assert.equal(window.action_id, commit.action_id);
    assert.equal(window.projectile_id, projectiles[0].id);
    assert.equal(commit.recipient, 'P00');
    assert.equal(commit.hp_cost, 3);
    assert.equal(commit.spirit_cost, skill.cost);
    const cooldown = () => actor.snapshot.self.player_skills.find(slot => slot.skill_id === skill.id).ready_at_ms;
    const initialCooldown = window.expires_at_ms - skill.recast.window_ms + skill.cooldown_ms;
    assert.equal(cooldown(), initialCooldown);
    const shot = () => actor.snapshot.projectiles.find(projectile => projectile.id === window.projectile_id);
    assert.ok(!peer.snapshot.self.projectile_recast);
    assert.ok(actor.snapshot.members.every(member => !Object.hasOwn(member, 'projectile_recast')));
    await assert.rejects(host.input('P01', action), { code: 'INVALID_TARGET' });
    await host.input('P01', { kind: 'move', direction: [1, 0] });
    await host.until(() => shot()?.position[0] > origin[0] && peer.snapshot.self.position[0] > peerStart);
    const priorPosition = shot().position;
    if (expire) {
      await host.until(() => actor.snapshot.logical_ms >= window.expires_at_ms && !opportunity());
      assert.ok(shot() && shot().position[0] > priorPosition[0]);
      assert.equal(shot().position[1], priorPosition[1]);
    } else {
      await host.input('P00', action);
      await host.until(() => redirects.length === 1 && !opportunity());
      assert.deepEqual(redirects[0], {
        recipient: 'P00', kind: 'projectile_recast', action_id: window.action_id,
        projectile_id: window.projectile_id, recast_index: 1, direction: [0, 1],
        spirit_cost: skill.recast.spirit_cost,
      });
      const position = shot().position;
      await host.until(() => shot()?.position[1] > position[1]);
      assert.equal(shot().position[0], position[0]);
    }
    await assert.rejects(host.input('P00', action), { code: 'INVALID_TARGET' });
    assert.equal(cooldown(), initialCooldown);
    await host.input('P01', { kind: 'stop' });
    await host.until(() => actor.snapshot.logical_ms >= actor.snapshot.self.recovery_until_ms);

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
    assert.equal(projectiles.length, 1);
    assert.equal(redirects.length, expire ? 0 : 1);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {
      scenario, members: host.members.length, outcome: expire ? 'expired' : 'redirected',
      projectileId: window.projectile_id, projectiles: projectiles.length,
      hpCost: commit.hp_cost, spiritCost: commit.spirit_cost, redirectSpiritCost: redirects[0]?.spirit_cost ?? 0,
      peerContinued: true, settled: ledgers.length,
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
