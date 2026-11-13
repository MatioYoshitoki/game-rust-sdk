import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binary, hostPath, configs, output] = process.argv.slice(2, 6).map(path => resolve(path));
const ignoreArmorProfile = process.argv[6] === '--ignore-armor';
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(hostPath).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);

async function journey(scenario, accountFile, basicUnavoidable) {
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
  const actor = host.members[0];
  const source = actor.account.participant_id;
  const target = ignoreArmorProfile ? 'armor-target' : 'unavoidable-target';
  const npc = () => actor.snapshot.monsters.find(row => row.entity.id === target);
  const shots = () => facts.filter(({event}) =>
    event.kind === 'projectile_started' && event.projectile.source.id === source);

  async function shot(index) {
    await host.until(() => shots().length > index);
    const row = shots()[index];
    const start = facts.indexOf(row);
    const end = () => facts.findIndex(({event}) =>
      event.kind === 'projectile_ended' && event.projectile_id === row.event.projectile.id);
    await host.until(() => end() > start);
    const hits = facts.slice(start + 1, end()).filter(({event}) =>
      event.kind === 'damage' && event.source.id === source);
    assert.ok(hits.length <= 1 && hits.every(({event}) => event.target.id === target));
    return hits[0]?.event.amount ?? 0;
  }

  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const spirit = actor.snapshot.self.spirit;
    await host.input(source, {kind: 'attack'});
    const basicDamage = await shot(0);
    const direction = npc().position.map((value, index) => value - actor.snapshot.self.position[index]);
    await host.input(source, {kind: 'cast', skill_id: record.config.skill.id, direction});
    const skillDamage = await shot(1);
    const damage = [basicDamage, skillDamage];
    const expected = ignoreArmorProfile
      ? (basicUnavoidable ? [66, 100] : [66, 66])
      : (basicUnavoidable ? [66, 66] : [0, 66]);
    assert.deepEqual(damage, expected);
    await host.until(() => npc().hp === 1000 - damage.reduce((sum, value) => sum + value, 0));
    assert.equal(actor.snapshot.self.spirit, spirit - record.config.skill.cost);
    assert.equal(shots().length, 2);
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source, target,
      ...(ignoreArmorProfile ? {ignoreArmor: basicUnavoidable} : {basicUnavoidable, skillUnavoidable: true}),
      damage, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', true], ['eight', 'accounts-b.json', false]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
