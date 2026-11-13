import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binary, hostPath, configs, output] = process.argv.slice(2, 6).map(path => resolve(path));
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(hostPath).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);

async function journey(scenario, accountFile, unavoidable) {
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
  const source = 'unavoidable-probe';
  const own = () => actor.snapshot.self;
  const status = attribute => own().statuses.find(row => row.attribute === attribute);
  const shots = () => facts.filter(({event}) =>
    event.kind === 'projectile_started' && event.projectile.source.id === source);

  async function shot(index) {
    await host.until(() => shots().length > index);
    const row = shots()[index];
    const start = facts.indexOf(row);
    const end = () => facts.findIndex(({event}) =>
      event.kind === 'projectile_ended' && event.projectile_id === row.event.projectile.id);
    await host.until(() => end() > start);
    const hits = facts.slice(start + 1, end()).filter(({event}) =>
      event.kind === 'damage' && event.source.id === source);
    assert.ok(hits.length <= 1 && hits.every(({event}) => event.target.id === id));
    return hits[0]?.event.amount ?? 0;
  }

  async function cast(skill, attribute) {
    await host.input(id, {kind: 'cast_self', skill_id: skill});
    await host.until(() => status(attribute));
    const buff = status(attribute);
    assert.deepEqual([buff.source, buff.target], [own().participant.entity, own().participant.entity]);
    assert.equal(buff.expires_at_ms - buff.applied_at_ms, 1300);
    return buff;
  }

  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const before = await shot(0);
    const dodge = await cast('focus-dodge', 'dodge_chance');
    assert.equal(dodge.modifier_bps, 5000);
    const dodging = await shot(1);
    assert.ok(status('dodge_chance') && actor.snapshot.logical_ms < dodge.expires_at_ms);
    const guard = await cast('protect-damage', 'damage_immunity');
    const immune = await shot(2);
    assert.ok(status('damage_immunity') && actor.snapshot.logical_ms < guard.expires_at_ms);
    await host.until(() => !status('damage_immunity') && facts.some(({event}) =>
      event.kind === 'status_removed' && event.reason === 'expired' &&
      event.instance_ids.includes(guard.instance_id)));
    const after = await shot(3);
    const damage = [before, dodging, immune, after];
    assert.deepEqual(damage, unavoidable ? [66, 66, 0, 66] : [66, 0, 0, 66]);
    const protectedAttributes = new Set(['dodge_chance', 'damage_immunity']);
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => !protectedAttributes.has(row.attribute))));
    const buffIds = new Set([dodge.instance_id, guard.instance_id]);
    assert.ok(facts.filter(({event}) => buffIds.has(event.status?.instance_id) ||
      event.instance_ids?.some(value => buffIds.has(value))).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source, target: id, unavoidable, damage, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', true], ['eight', 'accounts-b.json', false]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
