import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binary, hostPath, configs, output] = process.argv.slice(2).map(path => resolve(path));
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(hostPath).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);

async function journey(scenario, accountFile, bonusBps, expectedDamage) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      facts.push({recipient, event: event.kind === 'private' ? event.event : event});
    },
  });
  const [actor, ...peers] = host.members;
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const chance = () => own().statuses.find(row => row.attribute === 'critical_chance');
  const focus = () => own().statuses.find(row => row.attribute === 'critical_damage_bonus');
  const time = () => actor.snapshot.logical_ms;
  const targetId = 'armor-target';
  const hits = () => facts.filter(({event}) =>
    event.kind === 'damage' && event.source.id === id && event.target.id === targetId);
  async function attack() {
    const total = hits().length;
    await host.until(() => time() >= own().basic_ready_at_ms);
    await host.input(id, {kind: 'attack'});
    await host.until(() => hits().length > total);
    assert.equal(hits().length, total + 1);
    return hits()[total].event.amount;
  }
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    await host.input(id, {kind: 'cast_self', skill_id: 'focus-critical'});
    await host.until(chance);
    const critical = chance();
    assert.equal(critical.modifier_bps, 10000);
    const before = await attack();
    await host.input(id, {kind: 'cast_self', skill_id: 'focus-critical-damage'});
    await host.until(focus);
    const buff = focus();
    assert.deepEqual([buff.source, buff.target], [target, target]);
    assert.equal(buff.modifier_bps, bonusBps);
    assert.equal(record.config.params.critical_multiplier, 1.5);
    assert.equal(buff.expires_at_ms - buff.applied_at_ms, 1000);
    const boosted = await attack();
    assert.ok(focus() && time() < buff.expires_at_ms);
    const expired = () => facts.find(({event}) => event.kind === 'status_removed' &&
      event.reason === 'expired' && event.instance_ids.includes(buff.instance_id));
    await host.until(() => !focus() && expired());
    const after = await attack();
    const damage = [before, boosted, after];
    assert.deepEqual(damage, [100, expectedDamage, 100]);
    assert.deepEqual(chance(), critical);
    assert.equal(hits().at(-1).event.hp, 1000 - damage.reduce((sum, value) => sum + value, 0));
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => !['critical_chance', 'critical_damage_bonus'].includes(row.attribute))));
    const ids = [buff.instance_id, critical.instance_id];
    assert.ok(facts.filter(({event}) => ids.includes(event.status?.instance_id) ||
      event.instance_ids?.some(key => ids.includes(key))).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: id, target: targetId,
      bonusBps, damage, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 7500, 150], ['eight', 'accounts-b.json', 15000, 200]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
