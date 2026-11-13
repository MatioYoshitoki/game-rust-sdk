import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binary, hostPath, configs, output] = process.argv.slice(2, 6).map(path => resolve(path));
const npcSource = process.argv[6] === '--npc-source';
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(hostPath).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);

async function journey(scenario, accountFile, hpCost, hp, spirit) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const events = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) { events.push({recipient, event}); },
  });
  const actor = host.members[0];
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const lock = () => own().statuses.find(row => row.definition_id === 'spirit-seal');
  const orb = () => actor.snapshot.resources.find(row => !row.item_id && row.owner?.id === id);

  async function defeatAndPickup(targetId, expectedSpirit, basicSpirit) {
    const {id: target, generation} = actor.snapshot.monsters.find(npc => npc.entity.id === targetId).entity;
    const beforeAttack = own();
    await host.input(id, {kind: 'attack_unit', target: {id: target, generation}});
    await host.until(orb);
    const resource = orb();
    assert.equal(resource.owner.kind, 'participant');
    assert.equal(resource.quantity, 0);
    const before = own();
    assert.equal(before.hp, beforeAttack.hp);
    assert.equal(before.spirit - beforeAttack.spirit, basicSpirit);
    await host.input(id, {kind: 'pickup', resource_id: resource.id, expected_revision: before.inventory_revision});
    await host.until(() => !orb() && own().inventory_revision === before.inventory_revision + 1);
    const after = own();
    const change = [after.hp - before.hp, after.spirit - before.spirit];
    assert.deepEqual(change, [hp, expectedSpirit]);
    assert.deepEqual(after.inventory, before.inventory);
    await assert.rejects(host.input(id, {kind: 'pickup', resource_id: resource.id,
      expected_revision: after.inventory_revision}), {code: 'INVALID_TARGET'});
    return {change, basicSpirit: before.spirit - beforeAttack.spirit};
  }

  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    await host.input(id, {kind: 'cast_self', skill_id: record.config.additional_skills[0].id});
    await host.until(() => lock() && actor.snapshot.logical_ms >= own().recovery_until_ms);
    assert.equal(own().spirit, 50);
    if (npcSource) assert.ok(own().hp <= 1000 - hpCost);
    else assert.equal(own().hp, 1000 - hpCost);
    const state = lock();
    assert.equal(state.attribute, 'spirit_recovery_lock');
    assert.equal(state.modifier_bps, -10000);
    assert.equal(state.expires_at_ms - state.applied_at_ms, 4200);
    if (npcSource) {
      const source = actor.snapshot.monsters.find(row => row.entity.id === 'seal-caster').entity;
      assert.deepEqual(state.source, source);
      assert.deepEqual(state.target, own().participant.entity);
      const launch = events.findIndex(({event}) => event.kind === 'projectile_started' &&
        event.projectile.source.id === source.id);
      const impact = events.findIndex(({event}) => event.kind === 'damage' &&
        event.source.id === source.id && event.target.id === id);
      const applied = events.findIndex(({recipient, event}) => recipient === id &&
        event.kind === 'private' && event.event.kind === 'status_applied' &&
        event.event.status.instance_id === state.instance_id);
      assert.ok(launch >= 0 && launch < impact && impact < applied);
    }
    assert.ok(host.members.slice(1).every(member =>
      !member.snapshot.self.statuses.some(row => row.definition_id === 'spirit-seal')));
    const locked = await defeatAndPickup('orb-target', 0, 0);
    assert.ok(lock());
    await host.until(() => !lock() && actor.snapshot.logical_ms >= state.expires_at_ms &&
      actor.snapshot.logical_ms >= own().basic_ready_at_ms);
    const expired = await defeatAndPickup('orb-target-after', spirit, record.config.params.basic_spirit_on_hit);
    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    assert.equal(host.errors.length, 0);
    return {scenario, members: host.members.length, locked: locked.change, expired: expired.change,
      basicSpirit: [locked.basicSpirit, expired.basicSpirit],
      ...(npcSource ? {source: state.source.id, contactVerified: true} : {}),
      settled: ledgers.length};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
for (const args of [['four', 'accounts-a.json', 330, 30, 30], ['eight', 'accounts-b.json', 360, 150, 120]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
