import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [binaryPath, hostPath, configPath, outputPath, profile] = process.argv.slice(2);
assert.ok(profile === undefined || ['--ratio', '--quality', '--status', '--status-quality'].includes(profile));
const binary = resolve(binaryPath);
const configs = resolve(configPath);
const output = resolve(outputPath);
mkdirSync(output, {recursive: true});
const {TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog} =
  await import(pathToFileURL(resolve(hostPath)).href);
const manifest = join(configs, 'catalog.json');
const catalog = readCatalog(binary, manifest);
const qualityIds = ['pill_mana_low', 'pill_mana_high'];

async function journey(scenario, accountFile, hpDeficit, expected, itemId = 'pill_mana', attachedStatus, qualitySteps) {
  const accounts = JSON.parse(readFileSync(join(configs, accountFile), 'utf8'));
  const record = newRecord({catalog, scenario, accounts, manifest});
  const database = new TwoMemberDatabase(join(output, scenario + '.sqlite'));
  database.create(record);
  const facts = [];
  const statusEvents = [];
  const host = new TwoMemberHost({binary, manifest, database, record, autoAck: true,
    onEvent(recipient, event) {
      if (event.kind === 'private' && event.event.kind === 'item_used') facts.push({recipient, ...event.event});
      if (event.kind === 'private' && ['status_applied', 'status_refreshed', 'status_removed'].includes(event.event.kind)) {
        statusEvents.push({recipient, ...event.event});
      }
    },
  });
  const actor = host.members[0];
  const id = actor.account.participant_id;
  const own = () => actor.snapshot.self;
  const items = qualitySteps ? qualitySteps.map(step => step[0]) : profile === '--quality' ? qualityIds : [itemId];
  const steps = qualitySteps ?? [[itemId, expected[0], expected[1]]];
  const remainingQuantity = item => steps.some(step => step[0] === item) ? 1 : 2;
  const held = item => own().inventory.find(row => row.item_id === item)?.quantity ?? 0;
  const use = (item = itemId) => host.input(id, {kind: 'use_item', item_id: item, expected_revision: own().inventory_revision});
  const skill = record.config.additional_skills.find(skill => skill.mechanism === 'self_status');
  try {
    await host.start();
    await host.joinAndReady({reverse: scenario === 'eight'});
    await host.input(id, {kind: 'attack'});
    const drop = item => actor.snapshot.resources.find(resource => resource.item_id === item);
    await host.until(() => items.every(drop));
    for (const item of items) {
      await host.input(id, {kind: 'pickup', resource_id: drop(item).id, expected_revision: own().inventory_revision});
      await host.until(() => held(item) === 2);
    }
    const full = own();
    await assert.rejects(use(steps[0][0]), {code: 'INVALID_STATE'});
    assert.equal(own().inventory_revision, full.inventory_revision);
    assert.equal(own().item_cooldowns?.length ?? 0, 0);
    assert.equal(facts.length, 0);
    if (attachedStatus) {
      assert.ok(!statusEvents.some(row => row.kind === 'status_applied' && row.status.definition_id === attachedStatus[0]));
    }

    await host.input(id, {kind: 'cast_self', skill_id: skill.id});
    await host.until(() => own().spirit === own().max_spirit - skill.cost && own().hp === own().max_hp - hpDeficit);
    const before = own();
    const observed = [];
    for (const [index, [item, hp, spirit, modifierBps, durationMs, effectiveAtk]] of steps.entries()) {
      const prior = own();
      await use(item);
      const cooldownCount = qualitySteps ? index + 1 : items.length;
      await host.until(() => facts.length === index + 1 && held(item) === 1 && own().item_cooldowns?.length === cooldownCount);
      const current = own();
      assert.equal(current.inventory_revision, prior.inventory_revision + 1);
      assert.equal(current.hp, prior.hp + hp);
      assert.equal(current.spirit, prior.spirit + spirit);
      const definition = record.config.member_item_use[item];
      const spiritConfigured = definition.restore_spirit !== undefined || definition.restore_max_spirit_bps !== undefined;
      const fact = facts[index];
      assert.deepEqual([fact.recipient, fact.item_id, fact.hp_restored, fact.spirit_restored],
        [id, item, hp, spiritConfigured ? spirit : undefined]);
      if (qualitySteps) {
        const instance = current.statuses.find(row => row.definition_id === 'pill-fury');
        assert.ok(instance);
        assert.equal(current.statuses.filter(row => row.definition_id === 'pill-fury').length, 1);
        assert.equal(instance.modifier_bps, modifierBps);
        assert.equal(instance.expires_at_ms - instance.applied_at_ms, durationMs);
        assert.equal(current.effective_atk, effectiveAtk);
        observed.push({itemId: item, hp, modifierBps, durationMs, effectiveAtk,
          instanceId: instance.instance_id, deadline: instance.expires_at_ms});
        if (index === 0) await assert.rejects(use(item), {code: 'INVALID_STATE'});
      }
    }
    const after = own();
    for (const item of items) {
      assert.equal(held(item), remainingQuantity(item));
      await assert.rejects(use(item), {code: 'INVALID_STATE'});
    }
    if (items.length > 1 && !qualitySteps) {
      assert.equal(after.item_cooldowns[0].ready_at_ms, after.item_cooldowns[1].ready_at_ms);
    }
    assert.equal(own().inventory_revision, after.inventory_revision);
    assert.deepEqual(own().item_cooldowns, after.item_cooldowns);
    assert.equal(facts.length, steps.length);

    let status;
    if (attachedStatus) {
      const [definitionId, attribute, activeValue] = attachedStatus;
      const instance = after.statuses.find(row => row.definition_id === definitionId);
      assert.ok(instance);
      assert.equal(after[attribute], activeValue);
      const applied = statusEvents.filter(row => row.kind === 'status_applied' && row.status.definition_id === definitionId);
      assert.deepEqual(applied.map(row => [row.recipient, row.status.instance_id]), [[id, instance.instance_id]]);
      assert.ok(host.members.slice(1).every(member =>
        !member.snapshot.self.statuses.some(row => row.definition_id === definitionId)));
      const removed = () => statusEvents.filter(row => row.kind === 'status_removed' && row.instance_ids.includes(instance.instance_id));
      await host.until(() => actor.snapshot.logical_ms >= instance.expires_at_ms &&
        !own().statuses.some(row => row.instance_id === instance.instance_id) && removed().length > 0);
      assert.equal(own()[attribute], before[attribute]);
      assert.deepEqual(removed().map(row => [row.recipient, row.reason]), [[id, 'expired']]);
      status = {definitionId, attribute, activeValue, expired: true};
    }
    if (qualitySteps) {
      const [first, latest] = observed;
      assert.equal(latest.instanceId, first.instanceId);
      const deadlineShortened = latest.deadline < first.deadline;
      assert.equal(deadlineShortened, scenario === 'eight');
      const rows = statusEvents.filter(row => row.status?.definition_id === 'pill-fury');
      assert.deepEqual(rows.map(row => [row.recipient, row.kind, row.status.instance_id]),
        [[id, 'status_applied', first.instanceId], [id, 'status_refreshed', latest.instanceId]]);
      assert.ok(host.members.slice(1).every(member =>
        !member.snapshot.self.statuses.some(row => row.definition_id === 'pill-fury')));
      const expired = () => statusEvents.filter(row => row.kind === 'status_removed' && row.instance_ids.includes(latest.instanceId));
      await host.until(() => actor.snapshot.logical_ms >= latest.deadline &&
        !own().statuses.some(row => row.instance_id === latest.instanceId) && expired().length > 0);
      assert.equal(own().effective_atk, before.effective_atk);
      assert.deepEqual(expired().map(row => [row.recipient, row.reason]), [[id, 'expired']]);
      status = {steps: observed, deadlineShortened, expired: true};
    }

    for (const member of host.members) await host.input(member.account.participant_id, {kind: 'leave'});
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    const ledgers = database.views(record.id);
    assert.ok(ledgers.every(ledger => ledger.admissions === 1 && ledger.settlements === 1));
    const balances = Object.fromEntries(items.map(item => [item,
      ledgers.reduce((sum, ledger) => sum + (ledger.balances[item] ?? 0), 0)]));
    for (const item of items) assert.equal(balances[item], remainingQuantity(item));
    const remaining = items.length === 1 ? balances[itemId] : balances;
    assert.equal(host.errors.length, 0);
    assert.ok(host.replicationStats.full > 0 && host.replicationStats.delta > 0);
    return {scenario, members: host.members.length, restored: qualitySteps ? observed.map(row => row.hp) : expected,
      consumed: steps.length, remaining,
      settled: ledgers.length, fullRejected: true, cooldownRejected: true,
      ...(!qualitySteps && (items.length > 1 || attachedStatus) ? {itemId} : {}), ...(status ? {status} : {})};
  } finally {
    await host.dispose();
    database.close();
  }
}

const cases = [];
const journeys = {
  default: [[60, [40, 8]], [0, [0, 6]]],
  '--ratio': [[300, [200, 40]], [0, [0, 6]]],
  '--quality': [[330, [300, 60], 'pill_mana_high'], [0, [0, 28], 'pill_mana_low']],
  '--status': [
    [300, [200, 0], 'pill_berserk', ['item-fury', 'effective_atk', 130]],
    [0, [0, 6], 'ward_tonic', ['item-guard', 'effective_def', 120]],
  ],
  '--status-quality': [
    [330, null, undefined, undefined,
      [['pill_berserk_low', 140, 0, 1500, 4500, 115], ['pill_berserk_high', 190, 0, 3900, 7500, 139]]],
    [360, null, undefined, undefined,
      [['pill_berserk_high', 300, 0, 3900, 7500, 139], ['pill_berserk_low', 60, 0, 1500, 4500, 115]]],
  ],
};
const [four, eight] = journeys[profile ?? 'default'];
for (const args of [['four', 'accounts-a.json', ...four], ['eight', 'accounts-b.json', ...eight]]) {
  const result = await journey(...args);
  cases.push(result);
  console.log(JSON.stringify(result));
}
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(output, 'report.json'), JSON.stringify({passed: true, binarySha256, cases}, null, 2) + '\n');
