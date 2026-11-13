// Verify consumable use, private facts and settlement through the ordinary host.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binaryPath, hostPath, configPath, outputPath] = process.argv.slice(2);
const binary = resolve(binaryPath);
const configs = resolve(configPath);
const output = resolve(outputPath);
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(resolve(hostPath)).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);
const item = 'pill_healing';
const own = member => member.snapshot.self;
const held = member => own(member).inventory.find(row => row.item_id === item)?.quantity ?? 0;

async function journey(scenario, accountFile, transferred) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const host = new TwoMemberHost({
    binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private' && event.event.kind === 'item_used') facts.push({recipient, ...event.event});
    },
  });
  const [donor, peer] = host.members;
  const actor = transferred ? peer : donor;
  const id = member => member.account.participant_id;
  const cooldown = member => own(member).item_cooldowns?.find(row => row.item_id === item)?.ready_at_ms;
  const pickup = (member, resource) => host.input(id(member), {
    kind: 'pickup', resource_id: resource.id, expected_revision: own(member).inventory_revision,
  });
  const use = member => host.input(id(member), {
    kind: 'use_item', item_id: item, expected_revision: own(member).inventory_revision,
  });
  const skill = record.config.additional_skills.find(skill => skill.mechanism === 'self_status');
  async function spendHp(member) {
    await host.input(id(member), {kind: 'cast_self', skill_id: skill.id});
    await host.until(() => own(member).hp === own(member).max_hp - 60);
  }
  try {
    await host.start();
    await host.joinAndReady({reverse: transferred});
    await host.input(id(donor), {kind: 'attack'});
    await host.until(() => donor.snapshot.resources.some(resource => resource.item_id === item));
    await pickup(donor, donor.snapshot.resources.find(resource => resource.item_id === item));
    await host.until(() => held(donor) === 3);
    await assert.rejects(use(donor), {code: 'INVALID_STATE'});
    assert.equal(cooldown(donor), undefined);
    await spendHp(donor);
    await use(donor);
    await host.until(() => held(donor) === 2 && cooldown(donor) && facts.length === 1);
    const deadline = cooldown(donor);
    const before = own(donor);
    assert.ok(donor.snapshot.logical_ms < deadline);
    await assert.rejects(use(donor), {code: 'INVALID_STATE'});
    assert.equal(own(donor).inventory_revision, before.inventory_revision);
    assert.equal(own(donor).hp, before.hp);
    assert.equal(cooldown(donor), deadline);
    assert.equal(facts.length, 1);

    if (transferred) {
      assert.equal(cooldown(peer), undefined);
      await host.input(id(donor), {
        kind: 'drop_item', item_id: item, quantity: 1, expected_revision: own(donor).inventory_revision,
      });
      await host.until(() => peer.snapshot.resources.some(resource => resource.id.startsWith('player-drop-')));
      const drop = peer.snapshot.resources.find(resource => resource.id.startsWith('player-drop-'));
      await pickup(peer, drop);
      await host.until(() => held(donor) === 1 && held(peer) === 1);
      await spendHp(peer);
    } else {
      await host.until(() => donor.snapshot.logical_ms >= deadline && cooldown(donor) === undefined);
    }
    const count = held(actor);
    const revision = own(actor).inventory_revision;
    const status = await use(actor);
    assert.equal(status.logical_ms < deadline, transferred);
    await host.until(() => held(actor) === count - 1 && facts.length === 2 && cooldown(actor) > deadline);
    assert.equal(own(actor).inventory_revision, revision + 1);
    assert.deepEqual(facts.map(row => [row.recipient, row.hp_restored]), [
      [id(donor), 40], [id(actor), transferred ? 40 : 20],
    ]);
    assert.ok(facts.every(fact => fact.item_id === item && fact.action_id.startsWith('host-')));

    for (const member of host.members) {
      await host.input(id(member), {kind: 'leave'});
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    const balance = ledgers.reduce((sum, ledger) => sum + (ledger.balances[item] ?? 0), 0);
    assert.equal(ledgers.find(ledger => ledger.participant_id === id(donor)).balances[item], 1);
    assert.equal(balance, 1);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {scenario, members: host.members.length, usedBy: facts.map(fact => fact.recipient), transferred,
      earlyRejected: true, restored: facts.map(fact => fact.hp_restored), consumed: 2, remaining: balance,
      settled: ledgers.length, phase: host.phase, pending: host.pending(), full: host.replicationStats.full,
      delta: host.replicationStats.delta};
  } finally {
    await host.dispose();
    database.close();
  }
}


const cases = [];
for (const options of [['four', 'accounts-a.json', false], ['eight', 'accounts-b.json', true]]) {
  const evidence = await journey(...options);
  cases.push(evidence);
  console.log(JSON.stringify(evidence));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
