// Use the ordinary JSONL host and published followup facts with external configs.
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
        if (fact.kind === 'action_committed') commits.push(fact);
        if (fact.kind === 'action_interrupted') interruptions.push(fact);
      }
      if (event.kind === 'projectile_started' && event.projectile.source.id === 'P00') {
        projectiles.push(event.projectile);
      }
    },
  });
  const actor = host.member('P00');
  const peer = host.member('P01');
  const skill = record.config.skill;
  const own = () => actor.snapshot.self;
  const followup = () => own().followup;
  const window = record.config.skill_followup.window_ms;
  const cast = (skillId, direction) => host.input('P00', {kind: 'cast', skill_id: skillId, direction});
  try {
    await host.start();
    await host.joinAndReady({ reverse: cancelled });
    const peerStart = peer.snapshot.self.position[0];
    await cast(skill.id, [1, 0]);
    await host.until(() => !!followup() && commits.length === 1 && projectiles.length === 1);
    const release = followup().promotes_at_ms;
    assert.equal(followup().source_skill_id, skill.id);
    assert.equal(followup().queued, null);
    assert.equal(commits[0].hp_cost, 3);
    assert.equal(commits[0].spirit_cost, skill.cost);
    await host.input('P01', {kind: 'move', direction: [1, 0]});
    await host.until(() => release - actor.snapshot.logical_ms <= window);
    await cast('followup-first', [1, 0]);
    await host.until(() => followup()?.queued?.skill_id === 'followup-first');
    assert.equal(commits.length, 1);
    assert.equal(own().player_skills.find(slot => slot.skill_id === 'followup-first').ready_at_ms, 0);
    assert.equal(own().pending_action, null);
    assert.ok(!peer.snapshot.self.followup);
    assert.ok(actor.snapshot.members.every(member => !Object.hasOwn(member, 'followup')));
    let latest = null;
    if (cancelled) {
      await host.input('P00', {kind: 'cancel_action'});
      await host.until(() => followup()?.queued === null && interruptions.length === 1);
      assert.equal(own().recovery_until_ms, release);
      await host.until(() => actor.snapshot.logical_ms >= release + 300);
      assert.equal(projectiles.length, 1);
    } else {
      await cast('followup-latest', [0, 1]);
      await host.until(() => followup()?.queued?.skill_id === 'followup-latest');
      latest = followup().queued;
      assert.deepEqual(latest.direction, [0, 1]);
      assert.equal(commits.length, 1);
      await host.until(() => own().pending_action?.skill_id === 'followup-latest');
      assert.equal(own().pending_action.commits_at_ms, release + 300);
      assert.equal(commits.length, 1);
      await host.until(() => commits.length === 2 && projectiles.length === 2);
      assert.equal(commits[1].action_id, latest.action_id);
      assert.equal(commits[1].skill_id, latest.skill_id);
      assert.equal(commits[1].hp_cost, 2);
      assert.equal(commits[1].spirit_cost, 7);
      const shot = projectiles[1];
      await host.until(() => actor.snapshot.projectiles.some(p => p.id === shot.id && p.position[1] > shot.position[1]));
      assert.equal(actor.snapshot.projectiles.find(p => p.id === shot.id).position[0], shot.position[0]);
    }
    assert.ok(peer.snapshot.self.position[0] > peerStart);
    await host.input('P01', {kind: 'stop'});
    await host.until(() => actor.snapshot.logical_ms >= own().recovery_until_ms);

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
    assert.equal(commits.length, cancelled ? 1 : 2);
    assert.equal(actor.hpCostStats.commits, commits.length);
    assert.equal(projectiles.length, commits.length);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {
      scenario, members: host.members.length, outcome: cancelled ? 'cancelled' : 'replaced',
      projectiles: projectiles.length, hpCost: actor.hpCostStats.total,
      followupSpiritCost: commits[1]?.spirit_cost ?? 0, followupSkill: latest?.skill_id ?? null,
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
