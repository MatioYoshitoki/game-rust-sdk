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

async function journey(scenario, accountFile, restored, raw, granted, leavingRole) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const skill = record.config.additional_skills.find(skill => skill.mechanism === 'ally_heal');
  const drain = record.config.additional_skills.find(skill => skill.mechanism === 'self_status');
  const definition = skill.heal.overheal_shield.status_id;
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
  const shield = member => own(member).statuses.find(status => status.definition_id === definition);
  const results = member => facts.filter(({recipient, event}) => recipient === id(member) && (
    event.kind === 'action_committed' && event.skill_id === skill.id ||
    event.kind === 'heal_resolved' && event.heal_kind === 'ally_skill' ||
    event.kind === 'shield_granted' && event.definition_id === definition
  )).map(row => row.event);
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    if (restored) await host.input(id(target), {kind: 'cast_self', skill_id: drain.id});
    await host.until(() => own(target).hp === own(target).max_hp - restored);
    const sourceEntity = own(caster).participant.entity;
    const targetEntity = own(target).participant.entity;
    const selection = {id: targetEntity.id, generation: targetEntity.generation};
    const spirit = own(caster).spirit;
    await host.input(id(caster), {kind: 'cast_unit', skill_id: skill.id, target: selection});
    await host.until(() => shield(target) && results(caster).length >= 3 && results(target).length >= 2);
    const instance = shield(target);
    assert.deepEqual(results(caster).map(event => event.kind), ['action_committed', 'heal_resolved', 'shield_granted']);
    const [commit, heal, grant] = results(caster);
    assert.deepEqual(results(target), [heal, grant]);
    assert.deepEqual(commit.unit_target, selection);
    assert.equal(commit.spirit_cost, skill.cost);
    assert.equal(own(caster).spirit, spirit - skill.cost);
    assert.deepEqual([heal.raw_heal, heal.multiplier_bps, heal.adjusted_heal, heal.hp_restored],
      [raw, 10000, raw, restored]);
    assert.deepEqual([grant.instance_id, grant.requested, grant.granted, grant.remaining],
      [instance.instance_id, granted, granted, granted]);
    for (const row of [heal, grant, instance]) {
      assert.deepEqual([row.source, row.target], [sourceEntity, targetEntity]);
    }
    assert.equal(own(target).hp, own(target).max_hp);
    assert.equal(own(target).shield_total, granted);
    assert.ok([caster, ...peers].every(member => !shield(member) && own(member).shield_total === 0));
    const leaver = leavingRole === 'source' ? caster : target;
    const observer = leavingRole === 'source' ? target : caster;
    const reason = leavingRole === 'source' ? 'source_invalid' : 'target_invalid';
    await host.input(id(leaver), {kind: 'leave'});
    await host.until(() => facts.some(({recipient, event}) => recipient === id(observer) &&
      event.kind === 'status_removed' && event.reason === reason && event.instance_ids.includes(instance.instance_id)));
    if (leavingRole === 'source') await host.until(() => !shield(target) && own(target).shield_total === 0);
    for (const member of host.members) {
      if (member !== leaver) await host.input(id(member), {kind: 'leave'});
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    assert.ok(peers.every(peer => results(peer).length === 0));
    assert.ok(facts.filter(({event}) => event.kind === 'status_removed' &&
      event.instance_ids.includes(instance.instance_id)).every(row => [id(caster), id(target)].includes(row.recipient)));
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: id(caster), target: id(target),
      restored, raw, granted, reason, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [
  ['four', 'accounts-a.json', 0, 100, 50, 'source'],
  ['eight', 'accounts-b.json', 160, 300, 70, 'target'],
]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
