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

async function journey(scenario, accountFile) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const skill = record.config.additional_skills.find(skill =>
    skill.mechanism === 'self_status' && skill.id !== 'guard-first');
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
  const immunity = () => own().statuses.find(status => status.attribute === 'first_control_immunity');
  const silence = () => own().statuses.find(status => status.attribute === 'silence');
  const blocked = () => facts.find(({recipient, event}) =>
    recipient === id && event.kind === 'status_rejected' && event.reason === 'IMMUNE');
  const protectedCommit = () => facts.find(({recipient, event}) =>
    recipient === id && event.kind === 'action_committed' && event.skill_id === skill.id);
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    await host.input(id, {kind: 'cast_self', skill_id: 'guard-first'});
    await host.until(immunity);
    const guard = immunity();
    assert.deepEqual([guard.source, guard.target], [target, target]);
    assert.equal(guard.expires_at_ms - guard.applied_at_ms, 10000);
    await host.input(id, {kind: 'cast_self', skill_id: skill.id});
    const depleted = () => facts.find(({event}) =>
      event.kind === 'status_removed' && event.reason === 'depleted' &&
      event.instance_ids.includes(guard.instance_id));
    await host.until(() => blocked() && depleted() && !immunity() &&
      own().pending_action?.skill_id === skill.id);
    const rejected = blocked().event;
    assert.equal(rejected.source.id, 'silence-probe');
    assert.deepEqual(rejected.target, target);
    assert.deepEqual(rejected.definition_ids, ['npc-silence']);
    assert.ok(!silence());
    assert.ok(actor.snapshot.logical_ms < guard.expires_at_ms);
    const firstDamage = facts.slice(0, facts.indexOf(blocked())).find(({event}) => event.kind === 'damage' &&
      event.source.id === 'silence-probe' && event.target.id === id);
    assert.ok(firstDamage && firstDamage.event.amount > 0);
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row =>
      row.attribute !== 'first_control_immunity' && row.attribute !== 'silence')));
    await host.until(() => protectedCommit() && !immunity() && silence());
    assert.equal(protectedCommit().event.spirit_cost, skill.cost);
    const state = silence();
    assert.ok(state.applied_at_ms < guard.expires_at_ms);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 2800);
    assert.deepEqual([state.source, state.target], [rejected.source, rejected.target]);
    const index = predicate => facts.findIndex(({event}) => predicate(event));
    const depletedAt = facts.indexOf(depleted());
    const silenceAt = index(event => event.kind === 'status_applied' && event.status.instance_id === state.instance_id);
    assert.ok(facts.indexOf(blocked()) < depletedAt && depletedAt < facts.indexOf(protectedCommit()));
    assert.ok(silenceAt > facts.indexOf(protectedCommit()));
    assert.equal(facts.filter(({event}) => event.reason === 'IMMUNE').length, 1);
    await assert.rejects(host.input(id, {kind: 'cast_self', skill_id: 'guard-first'}), {code: 'INVALID_STATE'});
    assert.ok(facts.filter(({event}) => event.reason === 'IMMUNE' ||
      [guard.instance_id, state.instance_id].includes(event.status?.instance_id) ||
      event.instance_ids?.includes(guard.instance_id)).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: rejected.source.id, target: id,
      durationMs: 10000, firstDamage: firstDamage.event.amount, protectedCommit: true,
      firstGuardConsumed: true, secondDurationMs: 2800, secondBeforeOriginalExpiry: true, settled: ledgers.length};
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
