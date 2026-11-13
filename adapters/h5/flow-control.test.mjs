import test from 'node:test';
import assert from 'node:assert/strict';
import { OrderedQueue, IoScheduler } from './flow-control.mjs';
const latch = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
const turn = () => new Promise(resolve => setImmediate(resolve));
const request = (id = 'root', attempt = 1) => ({ operation_id: id, attempt_id: attempt, body: { kind: 'commit_admission', quantity: 5 } });

test('ordinary saturation reserves close capacity and rejects before executing the excess request', async () => {
  const queue = new OrderedQueue({ capacity: 4, reserved: 1 }); const held = latch(); const calls = [];
  const accepted = [queue.run(() => held.promise), queue.run(() => calls.push('first')), queue.run(() => calls.push('second'))];
  await assert.rejects(queue.run(() => calls.push('rejected')), /QUEUE_FULL/);
  const closed = queue.run(() => calls.push('close'), true);
  assert.equal(queue.stats().depth, 4); held.resolve(); await Promise.all([...accepted, closed]);
  assert.deepEqual(calls, ['first', 'second', 'close']); assert.equal(queue.stats().depth, 0);
} );

test('a failed queued task releases capacity and cannot poison the following command', async () => {
  const queue = new OrderedQueue(); const first = queue.run(() => { throw new Error('failed'); });
  const second = queue.run(() => 'closed', true); await assert.rejects(first, /failed/);
  assert.equal(await second, 'closed'); assert.equal(queue.stats().depth, 0);
});

test('hung I/O holds its slot while a different owner can finish', async () => {
  const io = new IoScheduler({ concurrency: 2, perOwner: 1 }); const held = latch(); const results = [];
  io.submit('slow', request(), () => held.promise, value => results.push(value));
  io.submit('slow', request('second'), () => 'second', value => results.push(value));
  io.submit('healthy', request(), () => 'healthy', value => results.push(value));
  await turn(); assert.deepEqual(results, ['healthy']); assert.equal(io.stats().active, 1);
  held.resolve('late'); await turn(); await turn(); assert.deepEqual(results, ['healthy', 'late', 'second']);
});

test('an exception after a possible commit becomes unknown and never retries the write', async () => {
  const io = new IoScheduler(); let executions = 0; let outcome;
  io.submit('owner', request(), () => { executions++; throw new Error('connection lost after commit'); }, value => { outcome = value; });
  await turn(); assert.equal(executions, 1); assert.deepEqual(outcome, { kind: 'unknown' });
});

test('duplicate in-flight attempts execute once and cannot change their frozen payload', async () => {
  const io = new IoScheduler(); const held = latch(); let executions = 0; const original = request();
  io.submit('owner', original, frozen => { executions++; frozen.body.quantity = 999; return held.promise; }, () => {});
  io.submit('owner', original, () => { executions++; }, () => {});
  assert.throws(() => io.submit('owner', { ...original, body: { ...original.body, quantity: 7 } }, () => {}, () => {}), /FROZEN_PAYLOAD_CONFLICT/);
  await turn(); assert.equal(executions, 1); assert.equal(original.body.quantity, 5); held.resolve({ kind: 'unknown' }); await turn();
});

test('I/O saturation is bounded and a rejected operation can be admitted after a slot is freed', async () => {
  const io = new IoScheduler({ concurrency: 1, capacity: 2 }); const held = latch(); let count = 0;
  assert.equal(io.submit('owner', request('one'), () => held.promise, () => {}), true);
  assert.equal(io.submit('owner', request('two'), () => ++count, () => {}), true);
  assert.equal(io.submit('owner', request('three'), () => ++count, () => {}), false);
  assert.equal(io.stats().retained, 2); held.resolve({ kind: 'unknown' }); await turn(); await turn();
  assert.equal(io.submit('owner', request('three'), () => ++count, () => {}), true);
  await turn(); assert.equal(count, 2); assert.equal(io.stats().retained, 0);
});

test('cancelling a session removes queued work but cannot release an executing remote write early', async () => {
  const io = new IoScheduler({ concurrency: 1 }); const held = latch(); let callbacks = 0; let queuedExecutions = 0;
  io.submit('owner', request('one'), () => held.promise, () => callbacks++);
  io.submit('owner', request('two'), () => queuedExecutions++, () => callbacks++);
  io.cancel('owner'); await turn(); assert.equal(io.stats().active, 1); assert.equal(io.stats().queued, 0);
  held.resolve({ kind: 'committed', revision: 1, commit_id: 'tx-1' }); await turn();
  assert.equal(callbacks, 0); assert.equal(queuedExecutions, 0); assert.equal(io.stats().retained, 0);
});
