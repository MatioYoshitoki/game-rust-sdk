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

async function journey(scenario, accountFile, first, leavingRole) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const skill = record.config.additional_skills.find(skill => skill.hot_status);
  const drain = record.config.additional_skills.find(skill => skill.mechanism === 'self_status');
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private') facts.push({recipient, event: event.event});
    },
  });
  const [caster, target, other] = host.members;
  const from = leavingRole === 'source' ? [caster, other] : [caster];
  const id = member => member.account.participant_id;
  const own = member => member.snapshot.self;
  const targetId = id(target);
  const hot = member => own(member).statuses.filter(status => status.definition_id === skill.hot_status);
  const notices = (kind, instanceId) => facts.filter(({event}) => event.kind === kind &&
    (event.instance_id === instanceId || event.status?.instance_id === instanceId ||
      event.instance_ids?.includes(instanceId)));
  const ticks = (recipient, instanceId) => notices('periodic_heal_tick', instanceId)
    .filter(row => row.recipient === recipient).map(row => row.event);
  const removed = (recipient, instanceId, reason) => notices('status_removed', instanceId)
    .some(row => row.recipient === recipient && row.event.reason === reason);
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const targetEntity = own(target).participant.entity;
    const selection = {id: targetEntity.id, generation: targetEntity.generation};
    await host.input(targetId, {kind: 'cast_self', skill_id: drain.id});
    await host.until(() => own(target).hp === own(target).max_hp - 360);
    for (const source of from) {
      await host.input(id(source), {kind: 'cast_unit', skill_id: skill.id, target: selection});
    }
    await host.until(() => hot(target).length === from.length);
    const instances = from.map(source => hot(target).find(row => row.source.id === id(source)));
    assert.equal(new Set(instances.map(row => row.instance_id)).size, from.length);
    await host.until(() => instances.every(row => notices('status_applied', row.instance_id).length >= 2));
    for (const [source, instance] of from.map((member, i) => [member, instances[i]])) {
      assert.deepEqual(instance.source, own(source).participant.entity);
      assert.deepEqual(instance.target, targetEntity);
      assert.equal(instance.periodic.raw_heal, first);
      assert.deepEqual(notices('status_applied', instance.instance_id).map(row => row.recipient).sort(),
        [id(source), targetId].sort());
    }
    assert.ok(host.members.filter(member => member !== target).every(member => hot(member).length === 0));
    const firstHp = own(target).max_hp - 360 + first * from.length;
    await host.until(() => own(target).hp === firstHp &&
      instances.every((row, i) => ticks(targetId, row.instance_id).length >= 1 &&
        ticks(id(from[i]), row.instance_id).length >= 1));
    const firstAmounts = instances.map(row => ticks(targetId, row.instance_id)[0].hp_restored);
    assert.deepEqual(firstAmounts, from.map(() => first));
    for (const [source, instance] of from.map((member, i) => [member, instances[i]])) {
      assert.deepEqual(ticks(id(source), instance.instance_id), ticks(targetId, instance.instance_id));
      assert.equal(ticks(targetId, instance.instance_id)[0].outcome, 'applied');
    }
    const leaver = leavingRole === 'source' ? caster : target;
    const reason = leavingRole === 'source' ? 'source_invalid' : 'target_invalid';
    const observer = leavingRole === 'source' ? target : caster;
    const ended = instances[0];
    await host.input(id(leaver), {kind: 'leave'});
    await host.until(() => removed(id(observer), ended.instance_id, reason));
    let continued = null;
    if (leavingRole === 'source') {
      const survivor = instances[1];
      await host.until(() => hot(target).length === 1 && hot(target)[0].instance_id === survivor.instance_id &&
        ticks(targetId, survivor.instance_id).length >= 2 && own(target).hp === firstHp + first);
      continued = ticks(targetId, survivor.instance_id)[1].hp_restored;
      assert.equal(continued, first);
    }
    const nextDue = ended.applied_at_ms + ended.periodic.interval_ms * 2;
    await host.until(() => observer.snapshot.logical_ms >= nextDue);
    for (const recipient of [id(caster), targetId]) assert.equal(ticks(recipient, ended.instance_id).length, 1);
    assert.equal(notices('periodic_heal_tick', ended.instance_id).length, 2);
    for (const member of host.members) {
      if (member !== leaver) await host.input(id(member), {kind: 'leave'});
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    for (const [source, instance] of from.map((member, i) => [member, instances[i]])) {
      for (const kind of ['status_applied', 'periodic_heal_tick', 'status_removed']) {
        assert.ok(notices(kind, instance.instance_id).every(row => [id(source), targetId].includes(row.recipient)));
      }
    }
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, sources: from.map(id), target: targetId,
      firstAmounts, reason, continued, endedTicks: 1, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [
  ['four', 'accounts-a.json', 110, 'source'],
  ['eight', 'accounts-b.json', 100, 'target'],
]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
