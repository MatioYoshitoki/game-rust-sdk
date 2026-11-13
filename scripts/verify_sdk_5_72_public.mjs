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
  const stun = () => own().statuses.find(row => row.attribute === 'stun');
  const cast = () => host.input(id, {kind: 'cast_self', skill_id: skill.id});
  const move = () => host.input(id, {kind: 'move', direction: [1, 0]});
  const skillCommits = () => facts.filter(({recipient, event}) => recipient === id &&
    event.kind === 'action_committed' && event.skill_id === skill.id);
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    const {hp, spirit, position} = own();
    await cast();
    await host.until(() => own().pending_action?.skill_id === skill.id);
    await host.until(() => stun() && !own().pending_action);
    const state = stun();
    assert.equal(state.source.id, 'stun-probe');
    assert.deepEqual(state.target, target);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 2800);
    const appliedAt = facts.findIndex(({event}) =>
      event.kind === 'status_applied' && event.status.instance_id === state.instance_id);
    const interruption = facts.findIndex(({recipient, event}) =>
      recipient === id && event.kind === 'action_interrupted');
    assert.ok(appliedAt >= 0 && appliedAt < interruption);
    assert.ok(facts.slice(0, appliedAt).some(({event}) => event.kind === 'damage' &&
      event.source.id === 'stun-probe' && event.target.id === id && event.amount > 0));
    assert.ok(own().hp < hp);
    assert.equal(own().spirit, spirit);
    assert.equal(own().player_skills.find(row => row.skill_id === skill.id).ready_at_ms, 0);
    assert.equal(skillCommits().length, 0);
    await assert.rejects(cast(), {code: 'INVALID_STATE'});
    await assert.rejects(move(), {code: 'INVALID_STATE'});
    for (const action of [
      {kind: 'navigate', destination: [440, 400]}, {kind: 'attack'}, {kind: 'extract_start'},
    ]) {
      await assert.rejects(host.input(id, action), {code: 'INVALID_STATE'});
    }
    assert.deepEqual(own().position, position);
    const expired = () => facts.find(({event}) => event.kind === 'status_removed' &&
      event.reason === 'expired' && event.instance_ids.includes(state.instance_id));
    await host.until(() => !stun() && expired() && actor.snapshot.logical_ms >= state.expires_at_ms);
    assert.equal(skillCommits().length, 0);
    assert.deepEqual(own().position, position);
    await move();
    await host.until(() => own().position[0] > position[0]);
    await host.input(id, {kind: 'stop'});
    await cast();
    await host.until(() => skillCommits().length === 1 && own().spirit === spirit - skill.cost);
    assert.equal(skillCommits()[0].event.spirit_cost, skill.cost);
    assert.equal(own().spirit, spirit - skill.cost);
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => row.attribute !== 'stun')));
    assert.ok(facts.filter(({event}) => event.status?.instance_id === state.instance_id ||
      event.instance_ids?.includes(state.instance_id)).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: state.source.id, target: id,
      durationMs: 2800, stationary: true, interrupted: true, noCost: true,
      castAfterExpiry: true, movedAfterExpiry: true, settled: ledgers.length};
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
