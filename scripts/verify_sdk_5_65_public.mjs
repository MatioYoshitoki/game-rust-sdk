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

async function journey(scenario, accountFile, missing, leavingRole, expected) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const skill = record.config.additional_skills.find(skill => skill.hot_status);
  const drain = record.config.additional_skills.find(skill => skill.mechanism === 'self_status');
  const definition = record.config.pve.status_definitions.find(status => status.id === skill.hot_status);
  const shieldId = definition.overheal_shield.status_id;
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private') facts.push({recipient, event: event.event});
    },
  });
  const [caster, target, ...peers] = host.members;
  const id = member => member.account.participant_id;
  const own = member => member.snapshot.self;
  const instances = member => own(member).statuses.filter(status =>
    [skill.hot_status, shieldId].includes(status.definition_id));
  const outcomes = member => facts.filter(({recipient, event}) => recipient === id(member) && (
    event.kind === 'periodic_heal_tick' && event.definition_id === skill.hot_status ||
    event.kind === 'shield_granted' && event.definition_id === shieldId
  )).map(row => row.event);
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    if (missing) await host.input(id(target), {kind: 'cast_self', skill_id: drain.id});
    await host.until(() => own(target).hp === own(target).max_hp - missing);
    const sourceEntity = own(caster).participant.entity;
    const targetEntity = own(target).participant.entity;
    await host.input(id(caster), {kind: 'cast_unit', skill_id: skill.id,
      target: {id: targetEntity.id, generation: targetEntity.generation}});
    const total = expected.reduce((sum, row) => sum + row[1], 0);
    const count = expected.length + expected.filter(row => row[1]).length;
    await host.until(() => own(target).hp === own(target).max_hp && own(target).shield_total === total &&
      outcomes(target).length >= count && outcomes(caster).length >= count);
    const ticks = [];
    const [hot, shield] = [skill.hot_status, shieldId].map(id =>
      instances(target).find(instance => instance.definition_id === id));
    assert.ok(hot && shield && hot.instance_id !== shield.instance_id);
    for (const row of [hot, shield, ...outcomes(target)]) {
      assert.deepEqual([row.source, row.target], [sourceEntity, targetEntity]);
    }
    assert.deepEqual(outcomes(caster), outcomes(target));
    for (const event of outcomes(target)) {
      if (event.kind === 'periodic_heal_tick') {
        assert.equal(event.instance_id, hot.instance_id);
        assert.equal(event.tick_index, ticks.length + 1);
        assert.equal(event.outcome, 'applied');
        ticks.push([event.hp_restored, 0]);
      } else {
        assert.equal(event.instance_id, shield.instance_id);
        assert.equal(ticks.at(-1)[1], 0);
        ticks.at(-1)[1] = event.granted;
      }
    }
    assert.deepEqual(ticks, expected);
    assert.ok([caster, ...peers].every(member => instances(member).length === 0 && own(member).shield_total === 0));
    const leaver = leavingRole === 'source' ? caster : target;
    const observer = leavingRole === 'source' ? target : caster;
    const reason = leavingRole === 'source' ? 'source_invalid' : 'target_invalid';
    const cleared = [hot.instance_id, shield.instance_id].sort();
    await host.input(id(leaver), {kind: 'leave'});
    await host.until(() => facts.some(({recipient, event}) => recipient === id(observer) &&
      event.kind === 'status_removed' && event.reason === reason &&
      cleared.every(id => event.instance_ids.includes(id))));
    if (leavingRole === 'source') await host.until(() => !instances(target).length && own(target).shield_total === 0);
    for (const member of host.members) {
      if (member !== leaver) await host.input(id(member), {kind: 'leave'});
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    assert.ok(peers.every(peer => outcomes(peer).length === 0));
    assert.ok(facts.filter(({event}) => event.kind === 'status_removed' &&
      event.instance_ids.some(id => cleared.includes(id))).every(row => [id(caster), id(target)].includes(row.recipient)));
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: id(caster), target: id(target),
      ticks, reason, cleared: cleared.length, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [
  ['four', 'accounts-a.json', 0, 'source', [[0, 55], [0, 55], [0, 0]]],
  ['eight', 'accounts-b.json', 160, 'target', [[100, 0], [60, 20], [0, 50]]],
]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
