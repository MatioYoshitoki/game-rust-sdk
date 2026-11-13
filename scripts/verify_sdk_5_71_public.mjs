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
  const skill = record.config.additional_skills.find(row => row.mechanism === 'self_status');
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, row) {
      facts.push({recipient, event: row.kind === 'private' ? row.event : row});
    },
  });
  const [actor, ...peers] = host.members;
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const root = () => own().statuses.find(row => row.attribute === 'root');
  const move = () => host.input(id, {kind: 'move', direction: [1, 0]});
  const committed = predicate => facts.some(({recipient, event}) =>
    recipient === id && event.kind === 'action_committed' && predicate(event));
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    await host.until(root);
    const state = root();
    assert.equal(state.source.id, 'root-probe');
    assert.deepEqual(state.target, target);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 2800);
    const position = own().position;
    const applied = facts.findIndex(({event}) =>
      event.kind === 'status_applied' && event.status.instance_id === state.instance_id);
    assert.ok(facts.slice(0, applied).some(({event}) => event.kind === 'damage' &&
      event.source.id === 'root-probe' && event.target.id === id && event.amount > 0));
    await assert.rejects(move(), {code: 'INVALID_STATE'});
    await assert.rejects(host.input(id, {kind: 'navigate', destination: [440, 400]}),
      {code: 'INVALID_STATE'});
    const far = actor.snapshot.monsters.find(({position: p}) => {
      const distance = Math.hypot(p[0] - position[0], p[1] - position[1]);
      return distance > record.config.basic.range && distance <= record.config.basic_pursuit.max_range;
    });
    const {id: npcId, generation} = far.entity;
    await assert.rejects(host.input(id, {kind: 'attack_unit', target: {id: npcId, generation}}),
      {code: 'INVALID_STATE'});
    await host.input(id, {kind: 'cast_self', skill_id: skill.id});
    await host.until(() => committed(event => event.skill_id === skill.id) &&
      actor.snapshot.logical_ms >= own().recovery_until_ms);
    await host.input(id, {kind: 'attack'});
    await host.until(() => committed(event => event.skill === false));
    assert.ok(root() && actor.snapshot.logical_ms < state.expires_at_ms);
    assert.deepEqual(own().position, position);
    const expired = () => facts.find(({event}) => event.kind === 'status_removed' &&
      event.reason === 'expired' && event.instance_ids.includes(state.instance_id));
    await host.until(() => !root() && expired() && actor.snapshot.logical_ms >= state.expires_at_ms);
    assert.deepEqual(own().position, position);
    await move();
    await host.until(() => own().position[0] > position[0]);
    await host.input(id, {kind: 'stop'});
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => row.attribute !== 'root')));
    assert.ok(facts.filter(({event}) => event.status?.instance_id === state.instance_id ||
      event.instance_ids?.includes(state.instance_id)).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: state.source.id, target: id,
      durationMs: 2800, stationary: true, cast: true, basic: true, movedAfterExpiry: true,
      settled: ledgers.length};
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
