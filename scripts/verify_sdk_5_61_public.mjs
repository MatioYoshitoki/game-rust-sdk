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

async function journey(scenario, accountFile, missing, restored) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const skill = record.config.additional_skills.find(skill => skill.mechanism === 'ally_heal');
  const drain = record.config.additional_skills.find(skill => skill.mechanism === 'self_status');
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind !== 'private') return;
      const row = event.event;
      if ((row.kind === 'action_committed' && row.skill_id === skill.id) ||
          (row.kind === 'heal_resolved' && row.heal_kind === 'ally_skill')) {
        facts.push({recipient, row});
      }
    },
  });
  const [actor, target, ...peers] = host.members;
  const id = actor.account.participant_id;
  const targetId = target.account.participant_id;
  const own = member => member.snapshot.self;
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const sourceEntity = own(actor).participant.entity;
    const targetEntity = own(target).participant.entity;
    const selection = {id: targetEntity.id, generation: targetEntity.generation};
    const beforeSpirit = own(actor).spirit;
    const beforeHp = own(actor).hp;
    await host.input(targetId, {kind: 'cast_self', skill_id: drain.id});
    await host.until(() => own(target).hp === own(target).max_hp - missing);
    const injuredHp = own(target).hp;
    await host.input(id, {kind: 'cast_unit', skill_id: skill.id, target: selection});
    await host.until(() => facts.length >= 3 && own(target).hp === injuredHp + restored &&
      own(actor).spirit === beforeSpirit - skill.cost);
    assert.deepEqual(facts.map(({recipient, row}) => [recipient, row.kind]),
      [[id, 'action_committed'], [id, 'heal_resolved'], [targetId, 'heal_resolved']]);
    const [commit, fromCaster, fromTarget] = facts.map(({row}) => row);
    assert.deepEqual(commit.unit_target, selection);
    assert.equal(commit.spirit_cost, skill.cost);
    assert.deepEqual(fromCaster, fromTarget);
    assert.deepEqual([fromCaster.source, fromCaster.target], [sourceEntity, targetEntity]);
    assert.deepEqual([fromCaster.raw_heal, fromCaster.multiplier_bps, fromCaster.adjusted_heal,
      fromCaster.hp_restored], [restored, 10000, restored, restored]);
    assert.equal(own(actor).hp, beforeHp);
    assert.equal(own(target).shield_total, 0);
    assert.ok(peers.every(member => own(member).hp === own(member).max_hp));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(facts.length, 3);
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, missing, restored, source: id, target: targetId,
      settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [
  ['four', 'accounts-a.json', 160, 120],
  ['eight', 'accounts-b.json', 360, 300],
]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
