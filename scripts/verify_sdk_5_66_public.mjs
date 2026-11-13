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
  const skill = record.config.additional_skills.find(skill => skill.mechanism === 'self_status');
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      facts.push({recipient, event: event.kind === 'private' ? event.event : event});
    },
  });
  const [actor, peer, ...otherPeers] = host.members;
  const id = member => member.account.participant_id;
  const own = member => member.snapshot.self;
  const silence = member => own(member).statuses.find(status => status.attribute === 'silence');
  const action = {kind: 'cast_self', skill_id: skill.id};
  const commits = filter => facts.filter(({recipient, event}) =>
    recipient === id(actor) && event.kind === 'action_committed' && filter(event));
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const spirit = own(actor).spirit;
    await host.input(id(actor), action);
    await host.until(() => own(actor).pending_action?.skill_id === skill.id);
    await host.until(() => silence(actor) && own(actor).pending_action === null &&
      facts.some(({recipient, event}) => recipient === id(actor) && event.kind === 'action_interrupted'));
    const state = silence(actor);
    assert.equal(state.source.id, 'silence-probe');
    assert.deepEqual(state.target, own(actor).participant.entity);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 2800);
    const impact = facts.findIndex(({event}) => event.kind === 'damage' &&
      event.source.id === state.source.id && event.target.id === id(actor));
    const applied = facts.findIndex(({recipient, event}) => recipient === id(actor) &&
      event.kind === 'status_applied' && event.status.instance_id === state.instance_id);
    const interrupted = facts.findIndex(({recipient, event}) =>
      recipient === id(actor) && event.kind === 'action_interrupted');
    assert.ok(impact >= 0 && impact < applied && applied < interrupted);
    assert.equal(own(actor).spirit, spirit);
    assert.equal(own(actor).player_skills.find(row => row.skill_id === skill.id).ready_at_ms, 0);
    assert.equal(commits(event => event.skill).length, 0);
    await assert.rejects(host.input(id(actor), action), {code: 'INVALID_STATE'});
    assert.equal(own(actor).spirit, spirit);
    assert.ok([peer, ...otherPeers].every(member => !silence(member)));
    await host.input(id(peer), action);
    await host.until(() => own(peer).pending_action?.skill_id === skill.id);
    await host.input(id(peer), {kind: 'cancel_action'});
    const before = own(actor).position;
    await host.input(id(actor), {kind: 'move', direction: [1, 0]});
    await host.until(() => own(actor).position[0] > before[0]);
    await host.input(id(actor), {kind: 'stop'});
    await host.input(id(actor), {kind: 'attack'});
    await host.until(() => commits(event => !event.skill).length === 1);
    assert.ok(silence(actor) && actor.snapshot.logical_ms < state.expires_at_ms);
    await host.until(() => !silence(actor) && actor.snapshot.logical_ms >= state.expires_at_ms &&
      facts.some(({recipient, event}) => recipient === id(actor) && event.kind === 'status_removed' &&
        event.reason === 'expired' && event.instance_ids.includes(state.instance_id)));
    await host.input(id(actor), action);
    await host.until(() => commits(event => event.skill_id === skill.id).length === 1);
    assert.equal(commits(event => event.skill_id === skill.id)[0].event.spirit_cost, skill.cost);
    assert.ok(facts.filter(({event}) =>
      event.status?.instance_id === state.instance_id || event.instance_ids?.includes(state.instance_id))
      .every(row => row.recipient === id(actor)));
    for (const member of host.members) await host.input(id(member), {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: state.source.id, target: id(actor),
      durationMs: 2800, interrupted: true, basic: true, resumed: true, settled: ledgers.length};
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
