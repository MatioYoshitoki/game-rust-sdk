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

async function journey(scenario, accountFile, modifierBps, hasteBps, combinedSpeed, slowSpeed) {
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
  const slow = () => own().statuses.find(row => row.attribute === 'movement_slow');
  const haste = () => own().statuses.find(row => row.attribute === 'movement_haste');
  const time = () => actor.snapshot.logical_ms;
  async function speed(expected) {
    const at = time();
    const origin = own().position;
    await host.until(() => time() >= at + 180);
    const actual = Math.hypot(...own().position.map((value, index) => value - origin[index])) /
      ((time() - at) / 1000);
    assert.ok(Math.abs(actual - expected) < 1e-6, `${actual} versus ${expected}`);
    return Math.round(actual);
  }
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    await host.input(id, {kind: 'cast_self', skill_id: skill.id});
    const committed = () => facts.find(({recipient, event}) => recipient === id &&
      event.kind === 'action_committed' && event.skill_id === skill.id);
    await host.until(() => haste() && committed());
    const buff = haste();
    assert.deepEqual([buff.source, buff.target], [target, target]);
    assert.equal(buff.modifier_bps, hasteBps);
    assert.equal(buff.expires_at_ms - buff.applied_at_ms, 3000);
    assert.equal(committed().event.spirit_cost, skill.cost);
    await host.until(slow);
    const state = slow();
    assert.equal(state.source.id, 'slow-probe');
    assert.deepEqual(state.target, target);
    assert.equal(state.modifier_bps, modifierBps);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 2800);
    assert.ok(state.applied_at_ms < buff.expires_at_ms && buff.expires_at_ms < state.expires_at_ms);
    const position = own().position;
    await host.input(id, {kind: 'move', direction: [1, 0]});
    await host.until(() => own().position[0] > position[0]);
    const measuredCombined = await speed(combinedSpeed);
    assert.ok(haste() && slow());
    const expired = value => facts.find(({event}) => event.kind === 'status_removed' &&
      event.reason === 'expired' && event.instance_ids.includes(value.instance_id));
    await host.until(() => !haste() && expired(buff) && time() >= buff.expires_at_ms);
    assert.deepEqual(slow(), state);
    const measuredSlow = await speed(slowSpeed);
    assert.ok(slow());
    await host.until(() => !slow() && expired(state) && time() >= state.expires_at_ms);
    const restoredSpeed = await speed(180);
    await host.input(id, {kind: 'stop'});
    assert.ok(!facts.some(({recipient, event}) => recipient === id && event.kind === 'action_interrupted'));
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => !['movement_haste', 'movement_slow'].includes(row.attribute))));
    const ids = [state.instance_id, buff.instance_id];
    assert.ok(facts.filter(({event}) => ids.includes(event.status?.instance_id) ||
      event.instance_ids?.some(key => ids.includes(key))).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: state.source.id, target: id, modifierBps,
      hasteBps, combinedSpeed: measuredCombined, slowedSpeed: measuredSlow, restoredSpeed,
      continuedMovement: true, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', -2500, 3000, 189, 135], ['eight', 'accounts-b.json', -4000, 5000, 198, 108]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
