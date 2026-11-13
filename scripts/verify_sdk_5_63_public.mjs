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

async function journey(scenario, accountFile, granted, leavingRole) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const skill = record.config.additional_skills.find(skill => skill.mechanism === 'ally_shield');
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      facts.push({recipient, event: event.kind === 'private' ? event.event : event});
    },
  });
  const [caster, target, ...peers] = host.members;
  const id = member => member.account.participant_id;
  const own = member => member.snapshot.self;
  const shield = member => own(member).statuses.find(status => status.definition_id === skill.shield_status);
  const grants = () => facts.filter(({event}) =>
    event.kind === 'shield_granted' && event.definition_id === skill.shield_status);
  const hits = from => facts.slice(from).map(({event}) => event).filter(event =>
    event.kind === 'damage' && event.source.id === 'shield-probe' && event.target.id === id(target));
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const sourceEntity = own(caster).participant.entity;
    const targetEntity = own(target).participant.entity;
    const selection = {id: targetEntity.id, generation: targetEntity.generation};
    await host.input(id(caster), {kind: 'cast_unit', skill_id: skill.id, target: selection});
    await host.until(() => shield(target) && grants().length >= 2);
    const instance = shield(target);
    assert.deepEqual([instance.source, instance.target], [sourceEntity, targetEntity]);
    assert.deepEqual(grants().map(row => row.recipient).sort(), [id(caster), id(target)].sort());
    for (const {event} of grants()) {
      assert.deepEqual([event.source, event.target], [sourceEntity, targetEntity]);
      assert.deepEqual([event.instance_id, event.requested, event.granted, event.remaining],
        [instance.instance_id, granted, granted, granted]);
    }
    assert.ok([caster, ...peers].every(member => !shield(member) && own(member).shield_total === 0));
    const absorbs = () => facts.filter(({event}) => event.kind === 'shield_absorbed' &&
      event.consumptions.some(slice => slice.instance_id === instance.instance_id));
    await host.until(() => absorbs().length && hits(facts.indexOf(absorbs()[0])).length);
    const impact = absorbs()[0];
    const absorbed = impact.event.amount;
    const hpDamage = hits(facts.indexOf(impact))[0].amount;
    const remaining = granted - absorbed;
    assert.equal(impact.recipient, id(target));
    assert.equal(impact.event.source, 'shield-probe');
    assert.ok(absorbed > 0 && remaining > 0);
    assert.deepEqual(impact.event.consumptions, [{instance_id: instance.instance_id, absorbed, remaining}]);
    assert.equal(hpDamage, 0);
    await host.until(() => own(target).shield_total === remaining);
    const leaver = leavingRole === 'source' ? caster : target;
    const observer = leavingRole === 'source' ? target : caster;
    const reason = leavingRole === 'source' ? 'source_invalid' : 'target_invalid';
    const removal = () => facts.find(row => row.recipient === id(observer) &&
      row.event.kind === 'status_removed' && row.event.reason === reason &&
      row.event.instance_ids.includes(instance.instance_id));
    await host.input(id(leaver), {kind: 'leave'});
    await host.until(removal);
    let afterExitDamage = null;
    if (leavingRole === 'source') {
      await host.until(() => !shield(target) && own(target).shield_total === 0 &&
        hits(facts.indexOf(removal())).some(hit => hit.amount > 0));
      afterExitDamage = hits(facts.indexOf(removal())).find(hit => hit.amount > 0).amount;
      assert.equal(afterExitDamage, absorbed);
    }
    for (const member of host.members) {
      if (member !== leaver) await host.input(id(member), {kind: 'leave'});
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    assert.deepEqual(absorbs().map(row => row.recipient), [id(target)]);
    assert.ok(facts.filter(({event}) => event.kind === 'status_removed' &&
      event.instance_ids.includes(instance.instance_id)).every(row => [id(caster), id(target)].includes(row.recipient)));
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: id(caster), target: id(target), granted,
      absorbed, hpDamage, shieldRemaining: remaining, reason, afterExitDamage, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [
  ['four', 'accounts-a.json', 220, 'source'],
  ['eight', 'accounts-b.json', 200, 'target'],
]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
