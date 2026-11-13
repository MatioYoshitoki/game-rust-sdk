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
      facts.push({recipient, event: event.kind === 'private' ? event.event : event});
    },
  });
  const [actor, ...peers] = host.members;
  const id = actor.account.participant_id;
  const status = attribute => actor.snapshot.self.statuses.find(row => row.attribute === attribute);
  const reduction = () => status('control_duration_reduction');
  const silence = () => status('silence');
  const duration = row => row.expires_at_ms - row.applied_at_ms;
  const applied = state => facts.findIndex(({event}) =>
    event.kind === 'status_applied' && event.status.instance_id === state.instance_id);
  const expired = state => facts.findIndex(({event}) => event.kind === 'status_removed' &&
    event.reason === 'expired' && event.instance_ids.includes(state.instance_id));
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = actor.snapshot.self.participant.entity;
    await host.input(id, {kind: 'cast_self', skill_id: 'shorten-self'});
    await host.until(reduction);
    const guard = reduction();
    assert.deepEqual([guard.source, guard.target], [target, target]);
    assert.equal(guard.modifier_bps, reductionBps);
    assert.equal(duration(guard), 2200);
    await host.until(silence);
    const first = silence();
    assert.equal(first.source.id, 'silence-probe');
    assert.deepEqual(first.target, target);
    assert.equal(duration(first), firstDurationMs);
    assert.ok(first.applied_at_ms < guard.expires_at_ms && guard.expires_at_ms < first.expires_at_ms);
    await host.until(() => !reduction() && expired(guard) >= 0);
    assert.deepEqual(silence(), first);
    await host.until(() => !silence() && expired(first) >= 0);
    assert.ok(actor.snapshot.logical_ms >= first.expires_at_ms);
    assert.ok(actor.snapshot.logical_ms < first.applied_at_ms + 2800);
    await host.until(silence);
    const second = silence();
    assert.notEqual(second.instance_id, first.instance_id);
    assert.deepEqual([second.source, second.target], [first.source, target]);
    assert.equal(duration(second), 2800);
    assert.ok(applied(guard) >= 0 && applied(guard) < applied(first) && applied(first) < expired(guard));
    assert.ok(expired(guard) < expired(first) && expired(first) < applied(second));
    const ids = [guard.instance_id, first.instance_id, second.instance_id];
    assert.ok(facts.filter(({event}) =>
      ids.includes(event.status?.instance_id) || event.instance_ids?.some(key => ids.includes(key)))
      .every(row => row.recipient === id));
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row =>
      !['control_duration_reduction', 'silence'].includes(row.attribute))));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: first.source.id, target: id,
      reductionBps, firstDurationMs, secondDurationMs: 2800, frozen: true, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 2500, 2100], ['eight', 'accounts-b.json', 4000, 1680]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
