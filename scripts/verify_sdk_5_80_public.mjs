import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binary, hostPath, configs, output] = process.argv.slice(2, 6).map(path => resolve(path));
const npcHitProfile = process.argv[6] === '--npc-hit';
const immunityProfile = process.argv[6] === '--damage-immunity';
const defense = immunityProfile
  ? {source: 'immunity-probe', attribute: 'damage_immunity', skill: 'protect-damage'}
  : {source: 'evasion-probe', attribute: 'dodge_chance', skill: 'focus-dodge'};
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(hostPath).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);

async function journey(scenario, accountFile, dodgeBps, npcHit) {
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
  const source = defense.source;
  const own = () => actor.snapshot.self;
  const focus = () => own().statuses.find(row => row.attribute === defense.attribute);
  const shots = () => facts.filter(({event}) =>
    event.kind === 'projectile_started' && event.projectile.source.id === source);
  const fromProbe = ({event}) => event.kind === 'damage' && event.source.id === source;

  async function shot(index) {
    await host.until(() => shots().length > index);
    const row = shots()[index];
    const start = facts.indexOf(row);
    const end = () => facts.findIndex(({event}) =>
      event.kind === 'projectile_ended' && event.projectile_id === row.event.projectile.id);
    await host.until(() => end() > start);
    const hits = facts.slice(start + 1, end()).filter(fromProbe);
    assert.ok(hits.length <= 1);
    assert.ok(hits.every(({event}) => event.target.id === id));
    return hits[0]?.event.amount ?? 0;
  }

  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    const before = await shot(0);
    await host.input(id, {kind: 'cast_self', skill_id: defense.skill});
    await host.until(focus);
    const buff = focus();
    assert.deepEqual([buff.source, buff.target], [target, target]);
    if (!immunityProfile) assert.equal(buff.modifier_bps, dodgeBps);
    assert.equal(buff.expires_at_ms - buff.applied_at_ms, 1300);
    const boosted = await shot(1);
    assert.ok(focus() && actor.snapshot.logical_ms < buff.expires_at_ms);
    const expired = () => facts.find(({event}) => event.kind === 'status_removed' &&
      event.reason === 'expired' && event.instance_ids.includes(buff.instance_id));
    await host.until(() => !focus() && expired());
    const after = await shot(2);
    const damage = [before, boosted, after];
    const expectedDamage = npcHit === 100 ? [66, 66, 66] : [66, 0, 66];
    assert.deepEqual(damage, expectedDamage);
    assert.equal(facts.filter(fromProbe).length, expectedDamage.filter(amount => amount > 0).length);
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => row.attribute !== defense.attribute)));
    assert.ok(facts.filter(({event}) => event.status?.instance_id === buff.instance_id ||
      event.instance_ids?.includes(buff.instance_id)).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source, target: id, ...(immunityProfile ? {immunityDurationMs: 1300} : {dodgeBps}), ...(npcHit === undefined ? {} : {npcHit}), damage, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
const profiles = npcHitProfile
  ? [['four', 'accounts-a.json', 5000, 100], ['eight', 'accounts-b.json', 5000, 50]]
  : [['four', 'accounts-a.json', 2500], ['eight', 'accounts-b.json', 5000]];
for (const args of profiles) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
