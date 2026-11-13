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

async function journey(scenario, accountFile, reductionBps, firstDurationMs) {
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
  const [actor, ...peers] = host.members;
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const readyAt = skill => own().player_skills.find(row => row.skill_id === skill).ready_at_ms;
  const focus = () => own().statuses.find(row => row.attribute === 'cooldown_reduction');
  const marker = () => own().statuses.find(row => row.definition_id === 'demo-guard');
  const cast = skill_id => host.input(id, {kind: 'cast_self', skill_id});
  const commits = () => facts.filter(({recipient, event}) => recipient === id &&
    event.kind === 'action_committed' && event.skill_id === 'cycle-probe');
  const probe = 'cycle-probe';
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    await cast('focus-cooldown');
    await host.until(focus);
    const buff = focus();
    assert.deepEqual([buff.source, buff.target], [target, target]);
    assert.equal(buff.modifier_bps, reductionBps);
    assert.equal(buff.expires_at_ms - buff.applied_at_ms, 900);
    assert.equal(readyAt('focus-cooldown') - buff.applied_at_ms, 6000);
    await cast(probe);
    await host.until(() => marker() && commits().length === 1);
    const first = marker();
    const deadline = readyAt(probe);
    assert.equal(deadline - first.applied_at_ms, firstDurationMs);
    assert.ok(first.applied_at_ms < buff.expires_at_ms && buff.expires_at_ms < deadline);
    await assert.rejects(cast(probe), {code: 'INVALID_STATE'});
    const expired = () => facts.find(({event}) => event.kind === 'status_removed' &&
      event.reason === 'expired' && event.instance_ids.includes(buff.instance_id));
    await host.until(() => !focus() && expired());
    assert.equal(readyAt(probe), deadline);
    assert.ok(actor.snapshot.logical_ms < deadline);
    assert.equal(commits().length, 1);
    await host.until(() => actor.snapshot.logical_ms >= deadline);
    await cast(probe);
    await host.until(() => marker()?.applied_at_ms > first.applied_at_ms && commits().length === 2);
    const second = marker();
    assert.equal(second.instance_id, first.instance_id);
    assert.equal(readyAt(probe) - second.applied_at_ms, 2000);
    assert.ok(second.applied_at_ms >= deadline && !focus());
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row =>
      row.attribute !== 'cooldown_reduction' && row.definition_id !== 'demo-guard')));
    const ids = [buff.instance_id, first.instance_id];
    assert.ok(facts.filter(({event}) => ids.includes(event.status?.instance_id) ||
      event.instance_ids?.some(key => ids.includes(key))).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: id, target: id, reductionBps,
      firstDurationMs, secondDurationMs: 2000, frozen: true, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 2500, 1500], ['eight', 'accounts-b.json', 4000, 1200]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
