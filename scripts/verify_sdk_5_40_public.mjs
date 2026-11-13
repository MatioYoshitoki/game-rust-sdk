// Run an actual loot transfer and settlement through the ordinary JSONL host.
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
const item = 'xue-zhi';
const held = member => member.snapshot.self.inventory.find(row => row.item_id === item)?.quantity ?? 0;

async function journey(scenario, accountFile, competing) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true});
  const [donor, ...peers] = host.members;
  const candidates = peers.slice(0, competing ? 2 : 1);
  const id = member => member.account.participant_id;
  const pickup = (member, resource) => host.input(id(member), {
    kind: 'pickup', resource_id: resource.id, expected_revision: member.snapshot.self.inventory_revision,
  });
  try {
    await host.start();
    await host.joinAndReady({reverse: competing});
    const startRevision = donor.snapshot.self.inventory_revision;
    await assert.rejects(host.input(id(donor), {
      kind: 'drop_item', item_id: record.config.item_id, quantity: 1, expected_revision: startRevision,
    }), {code: 'INVALID_TARGET'});
    await host.input(id(donor), {kind: 'attack'});
    await host.until(() => donor.snapshot.resources.some(resource => resource.item_id === item));
    const loot = donor.snapshot.resources.find(resource => resource.item_id === item);
    await pickup(donor, loot);
    await host.until(() => held(donor) === 3);
    const revision = donor.snapshot.self.inventory_revision;
    await host.input(id(donor), {kind: 'drop_item', item_id: item, quantity: 2, expected_revision: revision});
    await host.until(() => held(donor) === 1 && donor.snapshot.resources.some(r => r.id.startsWith('player-drop-')));
    const drop = donor.snapshot.resources.find(r => r.id.startsWith('player-drop-'));
    assert.deepEqual(drop.position, donor.snapshot.self.position);
    assert.equal(drop.quantity, 2);
    assert.equal(drop.owner, null);
    assert.equal(donor.snapshot.self.inventory_revision, revision + 1);
    await host.until(() => host.members.every(member => member.snapshot.resources.some(r => r.id === drop.id)));
    assert.ok(peers.every(member => held(member) === 0));

    const claims = await Promise.allSettled(candidates.map(member => pickup(member, drop)));
    const winners = candidates.filter((_, index) => claims[index].status === 'fulfilled');
    const failedClaims = claims.filter(claim => claim.status === 'rejected');
    assert.equal(winners.length, 1);
    assert.equal(failedClaims.length, Number(competing));
    assert.ok(failedClaims.every(claim => claim.reason.code === 'INVALID_TARGET'));
    const winner = winners[0];
    await host.until(() => held(winner) === 2 && host.members.every(
      member => member.snapshot.resources.every(resource => resource.id !== drop.id)));
    assert.equal(peers.reduce((total, member) => total + held(member), 0), 2);
    for (const member of host.members) {
      await host.input(id(member), {kind: 'leave'});
    }
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    const balances = Object.fromEntries(ledgers.map(ledger => [ledger.participant_id, ledger.balances[item] ?? 0]));
    assert.equal(balances[id(donor)], 1);
    assert.equal(balances[id(winner)], 2);
    assert.equal(Object.values(balances).reduce((a, b) => a + b, 0), 3);
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {scenario, members: host.members.length, drop: drop.id, retained: 1, received: 2,
      winner: id(winner), failedClaims: failedClaims.length, total: 3, settled: ledgers.length,
      phase: host.phase, pending: host.pending(), full: host.replicationStats.full, delta: host.replicationStats.delta};
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
