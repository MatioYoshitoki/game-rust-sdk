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

async function journey(scenario, accountFile, expected) {
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
  const hot = () => own().statuses.find(row => row.definition_id === 'overflow-hot');
  const shield = () => own().statuses.find(row => row.definition_id === 'overflow-shield');
  const events = () => facts.filter(({event}) =>
    (event.kind === 'periodic_heal_tick' && event.definition_id === 'overflow-hot') ||
    (event.kind === 'shield_granted' && event.definition_id === 'overflow-shield'));
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    await host.input(id, {kind: 'cast_self', skill_id: record.config.additional_skills[0].id});
    await host.until(hot);
    await host.until(() => events().filter(({event}) => event.kind === 'periodic_heal_tick').length === 3 &&
      own().shield_total === expected.reduce((sum, tick) => sum + tick[1], 0));
    const ticks = [];
    for (const {recipient, event} of events()) {
      assert.equal(recipient, id);
      assert.deepEqual(event.source, own().participant.entity);
      assert.deepEqual(event.target, own().participant.entity);
      if (event.kind === 'periodic_heal_tick') {
        assert.equal(event.raw_heal, 100);
        assert.equal(event.outcome, 'applied');
        ticks.push([event.hp_restored, 0]);
      } else {
        assert.equal(ticks.at(-1)[1], 0);
        ticks.at(-1)[1] = event.granted;
      }
    }
    assert.deepEqual(ticks, expected);
    assert.equal(own().hp, own().max_hp);
    const deadline = shield().expires_at_ms;
    assert.ok(host.members.slice(1).every(member => member.snapshot.self.shield_total === 0));
    await host.until(() => !hot() && !shield() && own().shield_total === 0 &&
      actor.snapshot.logical_ms >= deadline);
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, ticks, expired: true, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [
  ['four', 'accounts-a.json', [[0, 50], [0, 50], [0, 0]]],
  ['eight', 'accounts-b.json', [[100, 0], [60, 20], [0, 50]]],
]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
