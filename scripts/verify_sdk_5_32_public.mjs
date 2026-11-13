// Use the ordinary JSONL host and published channel facts with external configs.
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
  const pulses = [];
  const ends = [];
  const projectiles = [];
  const host = new TwoMemberHost({
    binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private') {
        const fact = { recipient, ...event.event };
        if (fact.kind === 'action_committed') commits.push(fact);
        if (fact.kind === 'channel_pulse') pulses.push(fact);
        if (fact.kind === 'channel_ended') ends.push(fact);
      }
      if (event.kind === 'projectile_started' && event.projectile.source.id === 'P00') {
        projectiles.push(event.projectile);
      }
    },
  });
  const actor = host.member('P00');
  const peer = host.member('P01');
  const skill = record.config.skill;
  const active = () => actor.snapshot?.self.active_channel;
  try {
    await host.start();
    await host.joinAndReady({ reverse: cancelled });
    await host.input('P00', { kind: 'cast', skill_id: skill.id, direction: [1, 0] });
    await host.until(() => !!active() && commits.length === 1);
    const channel = active();
    const [commit] = commits;
    const origin = actor.snapshot.self.position;
    const peerStart = peer.snapshot.self.position[0];
    const definition = skill.channel;
    assert.deepEqual(channel.direction, [1, 0]);
    assert.equal(channel.ends_at_ms, channel.started_at_ms + definition.duration_ms);
    assert.equal(channel.action_id, commit.action_id);
    assert.equal(commit.recipient, 'P00');
    assert.equal(commit.hp_cost, 3);
    assert.equal(commit.spirit_cost, skill.cost);
    assert.equal(actor.snapshot.self.player_skills.find(slot => slot.skill_id === skill.id).ready_at_ms,
      channel.started_at_ms + skill.cooldown_ms);
    await host.input('P00', { kind: 'move', direction: [1, 0] });
    await host.input('P01', { kind: 'move', direction: [1, 0] });
    await host.until(() => pulses.length >= 1 && active()?.pulses >= 1);
    assert.deepEqual(actor.snapshot.self.position, origin);
    assert.ok(peer.snapshot.self.position[0] > peerStart);
    assert.ok(!peer.snapshot.self.active_channel);
    assert.ok(actor.snapshot.members.every(member => !Object.hasOwn(member, 'active_channel')));
    const endAt = cancelled
      ? (await host.input('P00', { kind: 'cancel_action' })).logical_ms
      : channel.ends_at_ms;
    const endReason = cancelled ? 'cancelled' : 'completed';
    const pulseCount = cancelled ? 1 : definition.duration_ms / definition.interval_ms - 1;
    await host.until(() => ends.length === 1 && !active());
    assert.deepEqual(ends[0], { recipient: 'P00', kind: 'channel_ended', action_id: channel.action_id, reason: endReason });
    const recoveryUntil = endAt + skill.recovery_ms;
    assert.equal(actor.snapshot.self.recovery_until_ms, recoveryUntil);
    assert.equal(pulses.length, pulseCount);
    assert.equal(projectiles.length, pulseCount);
    for (const [index, pulse] of pulses.entries()) {
      assert.equal(pulse.recipient, 'P00');
      assert.equal(pulse.action_id, channel.action_id);
      assert.equal(pulse.pulse, index + 1);
      assert.equal(pulse.spirit_cost, definition.spirit_per_pulse);
      assert.equal(pulse.projectile_id, projectiles[index].id);
      assert.ok(!projectiles[index].piercing);
    }
    await host.input('P01', { kind: 'stop' });
    await host.until(() => actor.snapshot.logical_ms >= recoveryUntil);

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
    assert.equal(pulses.length, pulseCount);
    assert.equal(ends.length, 1);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {
      scenario, members: host.members.length, pulses: pulseCount, endReason,
      hpCost: commit.hp_cost, spiritCost: commit.spirit_cost,
      pulseSpiritCost: pulses.reduce((sum, pulse) => sum + pulse.spirit_cost, 0),
      locked: true, peerContinued: true, settled: ledgers.length,
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
