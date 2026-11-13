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
const tonic = 'care_tonic';
const pill = 'pill_renewal';

async function journey(scenario, accountFile, bps, adjusted) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const trail = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) { trail.push({recipient, event: event.event ?? event}); },
  });
  const actor = host.members[0];
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const held = item => own().inventory.find(row => row.item_id === item)?.quantity ?? 0;
  const used = item => trail.filter(({event}) => event.kind === 'item_used' && event.item_id === item);
  const care = () => own().statuses.find(row => row.definition_id === 'pill-care');

  async function use(item, expected, quantity) {
    const revision = own().inventory_revision;
    const events = used(item).length;
    await host.input(id, {kind: 'use_item', item_id: item, expected_revision: revision});
    await host.until(() => held(item) === quantity && used(item).length === events + 1);
    const fact = used(item).at(-1);
    assert.equal(fact.recipient, id);
    assert.equal(fact.event.hp_restored, expected);
    assert.equal(fact.event.spirit_restored, undefined);
    assert.equal(own().inventory_revision, revision + 1);
    return fact.event.hp_restored;
  }

  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    await host.input(id, {kind: 'attack'});
    const drop = item => actor.snapshot.resources.find(resource => resource.item_id === item);
    await host.until(() => drop(tonic) && drop(pill));
    for (const [item, quantity] of [[tonic, 1], [pill, 3]]) {
      await host.input(id, {kind: 'pickup', resource_id: drop(item).id, expected_revision: own().inventory_revision});
      await host.until(() => held(item) === quantity);
    }
    await host.until(() => trail.some(({recipient, event}) => recipient === id && event.kind === 'periodic_tick' &&
      event.definition_id === 'probe-dot' && event.target.id === id && event.hp_damage === 100));
    const tonicHeal = await use(tonic, 10, 0);
    const state = care();
    assert.equal(state.attribute, 'healing_received');
    assert.equal(state.modifier_bps, bps);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 3300);
    const activeHeal = await use(pill, adjusted, 2);
    assert.ok(care());
    assert.ok(host.members.slice(1).every(member => !member.snapshot.self.statuses.some(row =>
      row.definition_id === 'pill-care')));
    await assert.rejects(host.input(id, {kind: 'use_item', item_id: pill, expected_revision: own().inventory_revision}),
      {code: 'INVALID_STATE'});
    assert.equal(held(pill), 2);
    await host.until(() => actor.snapshot.logical_ms >= state.expires_at_ms && !care());
    const expiredHeal = await use(pill, 53, 1);
    const removal = trail.find(({event}) => event.kind === 'status_removed' &&
      event.instance_ids.includes(state.instance_id));
    assert.equal(removal.recipient, id);
    assert.equal(removal.event.reason, 'expired');
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1 &&
      !ledger.balances[tonic]));
    assert.equal(ledgers[0].balances[pill], 1);
    assert.ok(ledgers.slice(1).every(ledger => !ledger.balances[pill]));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, bps, tonicHeal, activeHeal, expiredHeal,
      pillsUsed: 2, remaining: 1, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 5000, 79], ['eight', 'accounts-b.json', -5000, 26]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
