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

async function journey(scenario, accountFile, hpCost, bps, active, expired) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const events = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) { events.push({recipient, event}); },
  });
  const actor = host.members[0];
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const strength = () => own().statuses.find(row => row.definition_id === 'regen-strength');

  async function observeTick(expected) {
    const before = own();
    await host.until(() => own().hp > before.hp);
    const after = own();
    const change = [after.hp - before.hp, after.spirit - before.spirit];
    assert.deepEqual(change, expected);
    assert.equal(after.inventory_revision, before.inventory_revision);
    return change;
  }

  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const skill = record.config.additional_skills[0];
    await host.input(id, {kind: 'cast_self', skill_id: skill.id});
    await host.until(strength);
    const state = strength();
    assert.equal(state.attribute, 'recovery_strength');
    assert.equal(state.modifier_bps, bps);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 3300);
    assert.ok(own().hp >= 1000 - hpCost && own().hp <= 1000 - hpCost + active[0]);
    assert.ok(own().spirit >= 50 && own().spirit <= 50 + active[1]);
    const activeTick = await observeTick(active);
    assert.ok(strength());
    const privateRecipients = events.filter(({event}) => event.kind === 'private' &&
      event.event.kind === 'status_applied' && event.event.status.instance_id === state.instance_id)
      .map(({recipient}) => recipient);
    assert.deepEqual(privateRecipients, [id]);
    assert.ok(host.members.slice(1).every(member =>
      !member.snapshot.self.statuses.some(row => row.definition_id === 'regen-strength')));
    await host.until(() => !strength() && actor.snapshot.logical_ms >= state.expires_at_ms);
    const expiredTick = await observeTick(expired);
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, bps, active: activeTick, expired: expiredTick,
      settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [
  ['four', 'accounts-a.json', 330, 10000, [10, 2], [5, 1]],
  ['eight', 'accounts-b.json', 360, -5000, [7, 1], [15, 3]],
]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
