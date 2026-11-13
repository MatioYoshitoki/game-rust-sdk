import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { DurableLab, PersistentStore } from './persistence.mjs';

const admission = { operation_id: 'match:admit', attempt_id: 1, body: {
  kind: 'commit_admission', expected_revision: 0, item_id: 'xuan-iron', quantity: 5, entry_years: 1,
} };
function fixture(work) {
  const directory = mkdtempSync(join(tmpdir(), 'rust-ledger-test-'));
  const path = join(directory, 'state.sqlite'); let database = new DurableLab(path);
  database.create({ id: 'match', owner: 'owner', connection: { id: 'web', generation: 1 } }, {}, {warehouse_quantity:100,revision:0});
  const reopen = () => { database.close(); database = new DurableLab(path); return database; };
  try { work(database, reopen, path); } finally { database.close(); rmSync(directory, { recursive: true, force: true }); }
}

test('reopening preserves the original transaction and deduplicates a repeated admission', () => fixture((db, reopen) => {
  const before = new PersistentStore(db, 'match'); const proof = before.execute(admission);
  const after = new PersistentStore(reopen(), 'match');
  assert.deepEqual(after.execute({ ...admission, attempt_id: 2 }), proof);
  assert.equal(after.warehouse, 95); assert.equal(after.years, 1); assert.equal(after.admissions, 1);
  assert.throws(() => after.execute({ ...admission, body: { ...admission.body, quantity: 6 } }));
  assert.equal(after.warehouse, 95);
}));

test('an interrupted SQL transaction cannot leave a debit without its deduplication proof', () => fixture((db, reopen) => {
  const store = new PersistentStore(db, 'match');
  const original = db.writeStore.bind(db);
  db.writeStore = (...args) => { original(...args); throw new Error('write interrupted'); };
  assert.throws(() => store.execute(admission), /write interrupted/);
  db.writeStore = original;
  const after = new PersistentStore(reopen(), 'match');
  assert.equal(after.warehouse, 100); assert.equal(after.journal.size, 0);
  assert.equal(after.execute(admission).kind, 'committed'); assert.equal(after.admissions, 1);
}));

test('manual correction and its proof commit together and survive replay', () => fixture((db, reopen) => {
  const store = new PersistentStore(db, 'match'); store.refuse(admission);
  const request = { operation_id: 'match:reconcile-admit', attempt_id: 1,
    body: { kind: 'reconcile_operation', operation_id: admission.operation_id } };
  const proof = store.execute(request);
  const after = new PersistentStore(reopen(), 'match');
  assert.deepEqual(after.execute(request), proof);
  assert.equal(after.warehouse, 95); assert.equal(after.admissions, 1); assert.equal(after.reconciliations, 1);
}));

test('a second process cannot write to a live ledger', () => fixture((db, reopen, path) => {
  const result = spawnSync(process.execPath, ['--input-type=module', '-e',
    `import { DurableLab } from './adapters/h5/persistence.mjs'; new DurableLab(process.argv[1]);`, path],
    { encoding: 'utf8', timeout: 5000 });
  assert.notEqual(result.status, 0); assert.match(result.stderr, /locked/);
  new PersistentStore(db, 'match').execute(admission);
  assert.equal(new PersistentStore(reopen(), 'match').warehouse, 95);
}));

test('a damaged store fails closed without resetting the balance', () => fixture((db) => {
  new PersistentStore(db, 'match').execute(admission);
  db.db.prepare('UPDATE stores SET digest=?').run('damaged');
  assert.throws(() => db.recoverable(), /STORE_CORRUPT/);
  assert.equal(JSON.parse(db.db.prepare('SELECT state FROM stores').get().state).warehouse, 95);
}));

test('a failed output batch rolls back events and frozen requests together, then permits an exact retry', () => fixture((db) => {
  const outputs = [
    { id: 'out1', logical_ms: 0, body: { kind: 'business_request', request: admission } },
    { id: 'out2', logical_ms: 0, body: { kind: 'event', event: { kind: 'joined' } } },
  ];
  const original = db.writeStore.bind(db);
  db.writeStore = () => { throw new Error('disk write failed'); };
  assert.throws(() => db.persistOutputs('match', outputs, {}), /disk write failed/);
  assert.equal(db.db.prepare('SELECT count(*) AS n FROM outputs').get().n, 0);
  assert.equal(db.readStore('match').requests.length, 0);
  assert.equal(db.readStore('match').warehouse, 100);
  db.writeStore = original; db.persistOutputs('match', outputs, {});
  assert.equal(db.db.prepare('SELECT count(*) AS n FROM outputs').get().n, 1);
  assert.deepEqual(db.readStore('match').requests[0][1], admission);
  assert.equal(db.readStore('match').warehouse, 100);
}));
