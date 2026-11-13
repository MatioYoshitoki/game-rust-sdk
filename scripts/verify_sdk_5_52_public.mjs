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
const item = 'pill_renewal';

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
  const held = () => own().inventory.find(row => row.item_id === item)?.quantity ?? 0;
  const eventOf = (kind, predicate) => trail.find(({event}) => event.kind === kind && predicate(event));
  const ticks = () => trail.filter(({event}) => event.kind === 'periodic_heal_tick' &&
    event.definition_id === 'pill-hot');
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    await host.input(id, {kind: 'attack'});
    const drop = () => actor.snapshot.resources.find(resource => resource.item_id === item);
    await host.until(drop);
    await host.input(id, {kind: 'pickup', resource_id: drop().id, expected_revision: own().inventory_revision});
    await host.until(() => held() === 2 && eventOf('periodic_tick', event =>
      event.definition_id === 'probe-dot' && event.target.id === id && event.hp_damage === 100));
    const revision = own().inventory_revision;
    const care = () => eventOf('status_applied', event => event.status.definition_id === 'pill-care');
    const used = () => eventOf('item_used', event => event.item_id === item);
    await host.input(id, {kind: 'use_item', item_id: item, expected_revision: revision});
    await host.until(() => held() === 1 && care() && used());
    const state = care().event.status;
    assert.equal(care().recipient, id);
    assert.equal(state.attribute, 'healing_received');
    assert.equal(state.modifier_bps, bps);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 3300);
    assert.equal(used().recipient, id);
    assert.equal(used().event.hp_restored, 10);
    assert.equal(own().inventory_revision, revision + 1);
    assert.ok(host.members.slice(1).every(member => !member.snapshot.self.statuses.some(row =>
      ['pill-care', 'pill-hot'].includes(row.definition_id))));
    await assert.rejects(host.input(id, {kind: 'use_item', item_id: item, expected_revision: revision + 1}),
      {code: 'INVALID_STATE'});
    assert.equal(held(), 1);
    await host.until(() => ticks().some(({event}) => event.executed_at_ms >= state.expires_at_ms));
    const expired = eventOf('status_removed', event => event.instance_ids.includes(state.instance_id));
    assert.equal(expired.recipient, id);
    assert.equal(expired.event.reason, 'expired');
    const phases = [[], []];
    for (const {recipient, event} of ticks()) {
      assert.equal(recipient, id);
      assert.equal(event.source.id, id);
      assert.deepEqual(event.source, event.target);
      assert.equal(event.raw_heal, 51);
      assert.equal(event.outcome, 'applied');
      const phase = Number(event.executed_at_ms >= state.expires_at_ms);
      phases[phase].push(event.hp_restored);
    }
    assert.ok(phases[0].length >= 2 && phases[0].every(amount => amount === adjusted), phases);
    assert.ok(phases[1].length > 0 && phases[1].every(amount => amount === 51), phases);
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(ledgers[0].balances[item], 1);
    assert.ok(ledgers.slice(1).every(ledger => !ledger.balances[item]));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, bps, phases, rawHeal: 51, immediateHeal: 10,
      consumed: 1, remaining: 1, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 5000, 76], ['eight', 'accounts-b.json', -5000, 25]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
