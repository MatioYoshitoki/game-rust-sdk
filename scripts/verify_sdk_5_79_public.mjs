import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binary, hostPath, configs, output] = process.argv.slice(2, 6).map(path => resolve(path));
const profile = process.argv[6] === '--accuracy'
  ? {attribute: 'accuracy', skill: 'focus-accuracy', target: 'accuracy-target',
      bpsKey: 'accuracyBps', values: [10000, 7500]}
  : {attribute: 'dodge_penalty', skill: 'focus-dodge', target: 'dodge-target',
      bpsKey: 'penaltyBps', values: [6000, 8000]};
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(hostPath).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);

async function journey(scenario, accountFile, bps) {
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
  const own = () => actor.snapshot.self;
  const focus = () => own().statuses.find(row => row.attribute === profile.attribute);
  const time = () => actor.snapshot.logical_ms;
  const targetId = profile.target;
  const npc = () => actor.snapshot.monsters.find(row => row.entity.id === targetId);
  const shots = () => facts.filter(({event}) =>
    event.kind === 'projectile_started' && event.projectile.source.id === id);
  const hits = () => facts.filter(({event}) =>
    event.kind === 'damage' && event.source.id === id && event.target.id === targetId);
  const shotOrigins = [];
  async function attack() {
    const total = shots().length;
    const previousHits = hits().length;
    await host.until(() => time() >= own().basic_ready_at_ms);
    await host.input(id, {kind: 'attack'});
    await host.until(() => shots().length === total + 1);
    const projectile = shots().at(-1).event.projectile;
    shotOrigins.push(projectile.position);
    await host.until(() => facts.some(({event}) =>
      event.kind === 'projectile_ended' && event.projectile_id === projectile.id));
    const newHits = hits().slice(previousHits);
    assert.ok(newHits.length <= 1);
    return newHits[0]?.event.amount ?? 0;
  }
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const target = own().participant.entity;
    const npcStart = npc().position;
    const before = await attack();
    await host.input(id, {kind: 'cast_self', skill_id: profile.skill});
    await host.until(focus);
    const buff = focus();
    assert.deepEqual([buff.source, buff.target], [target, target]);
    assert.equal(buff.modifier_bps, bps);
    assert.equal(buff.expires_at_ms - buff.applied_at_ms, 1000);
    const boosted = await attack();
    assert.ok(focus() && time() < buff.expires_at_ms);
    const expired = () => facts.find(({event}) => event.kind === 'status_removed' &&
      event.reason === 'expired' && event.instance_ids.includes(buff.instance_id));
    await host.until(() => !focus() && expired());
    const after = await attack();
    const damage = [before, boosted, after];
    assert.deepEqual(damage, [0, 66, 0]);
    assert.equal(shots().length, 3);
    assert.deepEqual(shotOrigins, [shotOrigins[0], shotOrigins[0], shotOrigins[0]]);
    assert.equal(hits().length, 1);
    await host.until(() => npc().hp === 934);
    assert.deepEqual(npc().position, npcStart);
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => row.attribute !== profile.attribute)));
    assert.ok(facts.filter(({event}) => event.status?.instance_id === buff.instance_id ||
      event.instance_ids?.includes(buff.instance_id)).every(row => row.recipient === id));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source: id, target: targetId,
      [profile.bpsKey]: bps, damage, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', profile.values[0]], ['eight', 'accounts-b.json', profile.values[1]]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
