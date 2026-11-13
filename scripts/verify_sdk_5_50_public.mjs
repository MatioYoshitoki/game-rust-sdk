import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binaryPath, hostPath, configPath, outputPath, profile] = process.argv.slice(2);
assert.ok(profile === undefined || profile === '--periodic');
const periodic = profile === '--periodic';
const binary = resolve(binaryPath);
const configs = resolve(configPath);
const output = resolve(outputPath);
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(resolve(hostPath)).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);
const item = 'pill_guard';
const effect = 'pill-guard';

async function journey(scenario, accountFile, bps, mitigated, shield) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const trail = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) { trail.push({recipient, event}); },
  });
  const actor = host.members[0];
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const hits = rows => rows.flatMap(({recipient, event}) => {
    const fact = periodic ? event.event : event;
    if (!fact || fact.kind !== (periodic ? 'periodic_tick' : 'damage') ||
        fact.source.id !== 'damage-probe' || fact.target.id !== id) return [];
    if (!periodic) return [fact.amount];
    assert.equal(recipient, id);
    assert.equal(fact.definition_id, 'probe-dot');
    assert.equal(fact.raw_damage, 101);
    assert.equal(fact.outcome, 'applied');
    return [[fact.absorbed, fact.hp_damage]];
  });
  const applied = ({event}) => event.kind === 'private' && event.event.kind === 'status_applied' && event.event.status.definition_id === effect;
  const used = ({event}) => event.kind === 'private' && event.event.kind === 'item_used';
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    await host.input(id, {kind: 'attack'});
    const drop = () => actor.snapshot.resources.find(resource => resource.item_id === item);
    await host.until(drop);
    await host.input(id, {kind: 'pickup', resource_id: drop().id, expected_revision: own().inventory_revision});
    const held = () => own().inventory.find(row => row.item_id === item)?.quantity ?? 0;
    await host.until(() => held() === 2 && hits(trail).length > 0 && own().hp < own().max_hp);
    const revision = own().inventory_revision;
    await host.input(id, {kind: 'use_item', item_id: item, expected_revision: revision});
    await host.until(() => held() === 1 && trail.some(applied) && trail.some(used));
    const start = trail.findIndex(applied);
    const effectRow = trail[start];
    const state = effectRow.event.event.status;
    const itemRow = trail.find(used);
    assert.equal(effectRow.recipient, id);
    assert.equal(itemRow.recipient, id);
    assert.equal(state.attribute, 'damage_reduction');
    assert.equal(state.modifier_bps, bps);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 3300);
    assert.equal(own().inventory_revision, revision + 1);
    assert.ok(itemRow.event.event.hp_restored > 0);
    if (periodic) assert.equal(own().shield_total, shield);
    const cooldowns = own().item_cooldowns;
    assert.equal(cooldowns.length, 1);
    await assert.rejects(host.input(id, {kind: 'use_item', item_id: item, expected_revision: revision + 1}),
      {code: 'INVALID_STATE'});
    assert.equal(held(), 1);
    assert.deepEqual(own().item_cooldowns, cooldowns);
    assert.ok(host.members.slice(1).every(member => !member.snapshot.self.statuses.some(row => row.definition_id === effect)));
    const end = () => trail.findIndex(({event}) => event.kind === 'private' && event.event.kind === 'status_removed' &&
      event.event.instance_ids.includes(state.instance_id));
    await host.until(() => end() > start && hits(trail.slice(end() + 1)).length > 0);
    const phases = [hits(trail.slice(0, start)), hits(trail.slice(start + 1, end())), hits(trail.slice(end() + 1))];
    if (periodic) {
      const [before, during, after] = phases;
      assert.ok(before.length > 0 && during.length >= 2 && after.length > 0, phases);
      assert.deepEqual(during[0], [shield, mitigated - shield]);
      assert.ok(during.slice(1).every(([absorbed, hp]) => absorbed === 0 && hp === mitigated), phases);
      assert.ok([...before, ...after].every(([absorbed, hp]) => absorbed === 0 && hp === 100), phases);
      const absorption = trail.filter(({event}) => event.kind === 'private' && event.event.kind === 'shield_absorbed');
      assert.deepEqual(absorption.map(({recipient, event}) => [recipient, event.event.amount]), [[id, shield]]);
    } else {
      const expected = [100, mitigated, 100];
      for (const [i, rows] of phases.entries()) assert.ok(rows.length > 0 && rows.every(value => value === expected[i]), phases);
    }
    assert.equal(trail[end()].recipient, id);
    assert.equal(trail[end()].event.event.reason, 'expired');
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    const remaining = ledgers.reduce((sum, ledger) => sum + (ledger.balances[item] ?? 0), 0);
    assert.equal(remaining, 1);
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, bps, phases, restoredHp: itemRow.event.event.hp_restored,
      expired: true, cooldownRejected: true, consumed: 1, remaining, settled: ledgers.length,
      ...(periodic ? {shield} : {})};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 2500, 75, 75], ['eight', 'accounts-b.json', 3200, 68, 40]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
