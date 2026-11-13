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

async function journey(scenario, accountFile, hpCost, hp, spirit) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const events = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) { events.push({recipient, event}); },
  });
  const [actor, other] = host.members;
  const id = actor.account.participant_id;
  const otherId = other.account.participant_id;
  const own = () => actor.snapshot.self;
  const orb = () => actor.snapshot.resources.find(resource => !resource.item_id && resource.owner?.id === id);
  const pickup = (member, resource, revision) =>
    host.input(member, {kind: 'pickup', resource_id: resource, expected_revision: revision});
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    await host.input(id, {kind: 'cast_self', skill_id: record.config.additional_skills[0].id});
    await host.until(() => own().hp === 1000 - hpCost && own().spirit === 50 &&
      actor.snapshot.logical_ms >= own().recovery_until_ms);
    await host.input(id, {kind: 'attack'});
    await host.until(() => orb() && other.snapshot.resources.some(row => row.id === orb().id));
    const resource = orb();
    const before = own();
    assert.deepEqual([before.hp, before.spirit], [1000 - hpCost, 50 + record.config.params.basic_spirit_on_hit]);
    assert.equal(resource.quantity, 0);
    assert.equal(resource.owner.kind, 'participant');
    const defeated = events.find(({event}) => event.kind === 'monster_defeated' && event.entity.id === 'orb-target');
    assert.deepEqual(resource.owner, defeated.event.credited_to);
    const beforeOther = other.snapshot.self;
    await assert.rejects(pickup(otherId, resource.id, beforeOther.inventory_revision), {code: 'UNAUTHORIZED'});
    assert.equal(orb().id, resource.id);
    await pickup(id, resource.id, before.inventory_revision);
    await host.until(() => !orb() && own().inventory_revision === before.inventory_revision + 1 &&
      other.snapshot.resources.every(row => row.id !== resource.id));
    const after = own();
    assert.deepEqual([after.hp - before.hp, after.spirit - before.spirit], [hp, spirit]);
    assert.deepEqual(after.inventory, before.inventory);
    assert.deepEqual(
      [other.snapshot.self.hp, other.snapshot.self.spirit, other.snapshot.self.inventory_revision],
      [beforeOther.hp, beforeOther.spirit, beforeOther.inventory_revision],
    );
    assert.deepEqual(events.filter(({event}) => event.kind === 'private' && event.event.kind === 'picked_up')
      .map(({recipient, event}) => [recipient, event.event.resource_id, event.event.quantity]), [[id, resource.id, 0]]);
    await assert.rejects(pickup(id, resource.id, after.inventory_revision), {code: 'INVALID_TARGET'});
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.ok(ledgers.every((ledger, i) => Object.keys(ledger.balances).every(key => key in accounts.participants[i].assets)));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, restored: [hp, spirit], ownerOnly: true,
      inventoryUnchanged: true, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 330, 30, 30], ['eight', 'accounts-b.json', 360, 150, 120]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
