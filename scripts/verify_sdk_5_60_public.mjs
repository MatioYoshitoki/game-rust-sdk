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

async function journey(scenario, accountFile, hpCost, raw, restored, granted) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const skill = record.config.additional_skills.find(skill => skill.self_heal);
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind !== 'private') return;
      const row = event.event;
      if ((row.kind === 'action_committed' && row.skill_id === skill.id) ||
          (row.kind === 'heal_resolved' && row.heal_kind === 'self_skill') ||
          (row.kind === 'shield_granted' && row.definition_id === 'overflow-shield')) {
        facts.push({recipient, row});
      }
    },
  });
  const actor = host.members[0];
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const shield = () => own().statuses.find(row => row.definition_id === 'overflow-shield');
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    assert.equal(own().hp, own().max_hp);
    assert.equal(own().shield_total, 0);
    await host.input(id, {kind: 'cast_self', skill_id: skill.id});
    await host.until(() => facts.length >= 3 && own().shield_total === granted && shield());
    assert.deepEqual(facts.map(({recipient, row}) => [recipient, row.kind]),
      [[id, 'action_committed'], [id, 'heal_resolved'], [id, 'shield_granted']]);
    const [commit, heal, grant] = facts.map(({row}) => row);
    assert.equal(commit.hp_cost ?? 0, hpCost);
    assert.equal(commit.spirit_cost, skill.cost);
    assert.deepEqual([heal.raw_heal, heal.multiplier_bps, heal.adjusted_heal, heal.hp_restored],
      [raw, 10000, raw, restored]);
    assert.deepEqual([grant.requested, grant.granted, grant.remaining], [granted, granted, granted]);
    for (const event of [heal, grant]) {
      assert.deepEqual(event.source, own().participant.entity);
      assert.deepEqual(event.target, own().participant.entity);
    }
    assert.equal(own().hp, own().max_hp);
    assert.ok(host.members.slice(1).every(member => member.snapshot.self.shield_total === 0));
    const deadline = shield().expires_at_ms;
    await host.until(() => !shield() && own().shield_total === 0 && actor.snapshot.logical_ms >= deadline);
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(facts.length, 3);
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, hpCost, raw, restored, granted, expired: true,
      settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [
  ['four', 'accounts-a.json', 0, 100, 0, 50],
  ['eight', 'accounts-b.json', 160, 300, 160, 70],
]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
