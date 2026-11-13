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
const item = 'overflow-tonic';

async function journey(scenario, accountFile, restored, requested) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private') facts.push({recipient, event: event.event});
    },
  });
  const actor = host.members[0];
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const held = () => own().inventory.find(row => row.item_id === item)?.quantity ?? 0;
  const shield = () => own().statuses.find(row => row.definition_id === 'overflow-shield');
  const dose = () => host.input(id, {kind: 'use_item', item_id: item, expected_revision: own().inventory_revision});
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const {id: target, generation} = actor.snapshot.monsters.find(npc => npc.entity.id === 'orb-target').entity;
    await host.input(id, {kind: 'attack_unit', target: {id: target, generation}});
    const drop = () => actor.snapshot.resources.find(row => row.item_id === item);
    await host.until(drop);
    await host.input(id, {kind: 'pickup', resource_id: drop().id, expected_revision: own().inventory_revision});
    await host.until(() => held() === 2);
    if (restored > 0) {
      await host.input(id, {kind: 'cast_self', skill_id: record.config.additional_skills[0].id});
      await host.until(() => own().hp === own().max_hp - restored);
    }
    const before = own();
    assert.equal(before.hp, before.max_hp - restored);
    assert.equal(before.shield_total, 0);
    await dose();
    await host.until(() => held() === 1 && shield());
    const state = shield();
    assert.equal(own().hp - before.hp, restored);
    assert.equal(own().inventory_revision, before.inventory_revision + 1);
    assert.equal(own().shield_total, 100);
    assert.equal(state.shield.remaining, 100);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 3000);
    assert.deepEqual(facts.filter(row => row.event.kind === 'item_used').map(row =>
      [row.recipient, row.event.hp_restored]), [[id, restored]]);
    assert.deepEqual(facts.filter(row => row.event.kind === 'shield_granted' &&
      row.event.definition_id === 'overflow-shield').map(row =>
      [row.recipient, row.event.requested, row.event.granted]), [[id, requested, 100]]);
    assert.ok(host.members.slice(1).every(member => member.snapshot.self.shield_total === 0));
    await host.until(() => (own().item_cooldowns?.length ?? 0) === 0);
    const beforeRetry = own();
    assert.ok(shield());
    await assert.rejects(dose(), {code: 'INVALID_STATE'});
    assert.deepEqual(own(), beforeRetry);
    await host.until(() => !shield() && own().shield_total === 0 &&
      actor.snapshot.logical_ms >= state.expires_at_ms);
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(ledgers[0].balances[item], 1);
    assert.ok(ledgers.slice(1).every(ledger => !ledger.balances[item]));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, restored, requested, granted: 100,
      remaining: 1, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 0, 100], ['eight', 'accounts-b.json', 360, 120]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
