import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binary, hostPath, configs, output] = process.argv.slice(2, 6).map(path => resolve(path));
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(hostPath).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);

async function journey(scenario, accountFile, powerBps, boostedDamage) {
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
  const source = actor.account.participant_id;
  const target = 'power-target';
  const own = () => actor.snapshot.self;
  const npc = () => actor.snapshot.monsters.find(row => row.entity.id === target);
  const focus = () => own().statuses.find(row => row.attribute === 'skill_power_bonus');
  const primary = record.config.skill;
  const selfSkill = record.config.additional_skills.find(row => row.id === 'focus-power');
  const shots = () => facts.filter(({event}) =>
    event.kind === 'projectile_started' && event.projectile.source.id === source);

  async function shot(index, isSkill) {
    const ready = () => isSkill
      ? own().player_skills.find(row => row.skill_id === primary.id).ready_at_ms
      : own().basic_ready_at_ms;
    await host.until(() => actor.snapshot.logical_ms >= ready() && actor.snapshot.logical_ms >= own().recovery_until_ms && own().pending_action === null);
    const direction = npc().position.map((value, i) => value - own().position[i]);
    await host.input(source, isSkill ? {kind: 'cast', skill_id: primary.id, direction} : {kind: 'attack'});
    await host.until(() => shots().length > index);
    const row = shots()[index];
    const start = facts.indexOf(row);
    const end = () => facts.findIndex(({event}) =>
      event.kind === 'projectile_ended' && event.projectile_id === row.event.projectile.id);
    await host.until(() => end() > start);
    const hits = facts.slice(start + 1, end()).filter(({event}) =>
      event.kind === 'damage' && event.source.id === source && event.target.id === target);
    assert.equal(hits.length, 1);
    const amount = hits[0].event.amount;
    await host.until(() => npc().hp === hits[0].event.hp);
    return amount;
  }

  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    const before = await shot(0, false);
    const initialSpirit = own().spirit;
    await host.input(source, {kind: 'cast_self', skill_id: selfSkill.id});
    await host.until(focus);
    const buff = focus();
    assert.deepEqual([buff.source, buff.target], [own().participant.entity, own().participant.entity]);
    assert.equal(buff.modifier_bps, powerBps);
    assert.equal(buff.expires_at_ms - buff.applied_at_ms, 1300);
    assert.equal(own().spirit, initialSpirit - selfSkill.cost);
    const boosted = await shot(1, true);
    assert.equal(own().spirit, initialSpirit - selfSkill.cost - primary.cost);
    const spiritBeforeBasic = own().spirit;
    const basic = await shot(2, false);
    assert.ok(focus() && actor.snapshot.logical_ms < buff.expires_at_ms);
    assert.equal(own().spirit, Math.min(own().max_spirit,
      spiritBeforeBasic + record.config.params.basic_spirit_on_hit));
    await host.until(() => !focus() && facts.some(({event}) => event.kind === 'status_removed' &&
      event.reason === 'expired' && event.instance_ids.includes(buff.instance_id)));
    const after = await shot(3, true);
    const damage = [before, boosted, basic, after];
    assert.deepEqual(damage, [66, boostedDamage, 66, 66]);
    assert.equal(own().spirit, initialSpirit - selfSkill.cost - 2 * primary.cost + record.config.params.basic_spirit_on_hit);
    assert.ok(peers.every(member => member.snapshot.self.statuses.every(row => row.attribute !== 'skill_power_bonus')));
    assert.ok(facts.filter(({event}) => event.status?.instance_id === buff.instance_id ||
      event.instance_ids?.includes(buff.instance_id)).every(row => row.recipient === source));
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.equal(ledgers.length, host.members.length);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, source, target, powerBps, damage, settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 5000, 100], ['eight', 'accounts-b.json', 10000, 133]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
