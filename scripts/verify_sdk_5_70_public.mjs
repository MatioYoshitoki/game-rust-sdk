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
const itemId = 'cleanse-tonic';

async function journey(scenario, accountFile) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const skill = record.config.additional_skills.find(skill => skill.mechanism === 'self_status');
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const privateFacts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, row) {
      if (row.kind === 'private') privateFacts.push({recipient, event: row.event});
    },
  });
  const [actor, ...peers] = host.members;
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const silence = () => own().statuses.find(row => row.attribute === 'silence');
  const held = () => own().inventory.find(row => row.item_id === itemId)?.quantity ?? 0;
  const cooldown = () => own().item_cooldowns?.find(row => row.item_id === itemId);
  const tonic = (revision = own().inventory_revision) =>
    host.input(id, {kind: 'use_item', item_id: itemId, expected_revision: revision});
  const cast = () => host.input(id, {kind: 'cast_self', skill_id: skill.id});
  const itemFacts = () => privateFacts.filter(({event}) => event.kind === 'item_used' && event.item_id === itemId);
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    await host.input(id, {kind: 'attack'});
    const drop = () => actor.snapshot.resources.find(row => row.item_id === itemId);
    await host.until(drop);
    assert.equal(drop().quantity, 2);
    await host.input(id, {kind: 'pickup', resource_id: drop().id, expected_revision: own().inventory_revision});
    await host.until(() => held() === 2);
    const revision = own().inventory_revision;
    await assert.rejects(tonic(), {code: 'INVALID_STATE'});
    assert.equal(own().inventory_revision, revision);
    assert.equal(held(), 2);
    assert.equal(cooldown(), undefined);
    await host.until(silence);
    const first = silence();
    assert.equal(first.source.id, 'silence-probe');
    assert.deepEqual(first.target, target);
    await assert.rejects(cast(), {code: 'INVALID_STATE'});
    await assert.rejects(tonic(revision - 1), {code: 'REVISION_CONFLICT'});
    const {hp, spirit} = own();
    assert.equal(held(), 2);
    assert.equal(own().inventory_revision, revision);
    await tonic();
    const removed = () => privateFacts.find(({event}) => event.kind === 'status_removed' &&
      event.reason === 'cleansed' && event.instance_ids.includes(first.instance_id));
    await host.until(() => !silence() && removed() && itemFacts().length === 1 && held() === 1 && cooldown());
    assert.equal(own().inventory_revision, revision + 1);
    assert.deepEqual([own().hp, own().spirit], [hp, spirit]);
    assert.equal(itemFacts()[0].event.hp_restored, 0);
    assert.equal(itemFacts()[0].event.spirit_restored, undefined);
    assert.ok(privateFacts.indexOf(removed()) < privateFacts.indexOf(itemFacts()[0]));
    assert.deepEqual(removed().event.instance_ids, [first.instance_id]);
    assert.ok(actor.snapshot.logical_ms < first.expires_at_ms);
    await cast();
    await host.until(() => privateFacts.some(({event}) =>
      event.kind === 'action_committed' && event.skill_id === skill.id));
    assert.ok(actor.snapshot.logical_ms < first.expires_at_ms);
    await host.until(silence);
    const second = silence();
    assert.notEqual(second.instance_id, first.instance_id);
    assert.deepEqual([second.source, second.target], [first.source, target]);
    assert.equal(second.expires_at_ms - second.applied_at_ms, 2800);
    await assert.rejects(tonic(), {code: 'INVALID_STATE'});
    assert.equal(held(), 1);
    assert.deepEqual(silence(), second);
    assert.equal(itemFacts().length, 1);
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => row.attribute !== 'silence')));
    const ids = [first.instance_id, second.instance_id];
    assert.ok(privateFacts.filter(({event}) =>
      ids.includes(event.status?.instance_id) || event.instance_ids?.some(key => ids.includes(key)) ||
      event.kind === 'item_used').every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    const remaining = ledgers.reduce((sum, ledger) => sum + (ledger.balances[itemId] ?? 0), 0);
    assert.equal(remaining, 1);
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: first.source.id, target: id,
      cleansed: true, resumed: true, secondDurationMs: 2800, consumed: 1, remaining, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json'], ['eight', 'accounts-b.json']]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
