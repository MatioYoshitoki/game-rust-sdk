import http from 'node:http';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFile, access } from 'node:fs/promises';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { createHash, randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { resolve, join } from 'node:path';
import { DurableLab, PersistentStore } from './persistence.mjs';
import { OrderedQueue, IoScheduler } from './flow-control.mjs';
import { readSourceCatalog, snapshotCatalog } from './configuration.mjs';
import { businessKind, settlementEvent } from '../node/asset-store.mjs';

const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const PUBLIC = join(ROOT, 'web');
const BINARY = join(ROOT, 'target/debug/kernel-host');
const faultModes = ['none', 'lost_receipt', 'refused', 'load_silent', 'load_delayed', 'admission_silent',
  'admission_lost_receipt', 'settlement_silent', 'settlement_lost_receipt', 'settlement_delayed', 'query_silent', 'compensation_silent'];
const actionFields = {
  attune_objective: ['objective_id','expected_revision'], cancel_attunement: [],
  open_chest: ['chest_id', 'expected_generation'],
  cancel_chest_unlock: ['chest_id', 'expected_generation'],
  navigate: ['destination'], join: [], combat_ready: ['publication_id'], move: ['direction'], stop: [], attack: [],
  cast: ['skill_id', 'direction'], cast_self: ['skill_id'], cast_unit: ['skill_id','target'], cast_ground: ['skill_id','position'], cancel_action: [], pickup: ['resource_id', 'expected_revision'],
  extract_start: [], extract_cancel: [], background: [], foreground: [], leave: [],
};

class ApiError extends Error {
  constructor(code, status = 400) { super(code); this.status = status; }
}

class Kernel {
  constructor(fast, onExit, manifest) {
    this.pending = [];
    this.child = spawn(BINARY, ['--configs', manifest, ...(fast ? ['--verification'] : [])], { stdio: ['pipe', 'pipe', 'pipe'] });
    this.lines = createInterface({ input: this.child.stdout });
    this.lines.on('line', line => {
      const waiter = this.pending.shift();
      if (!waiter) return;
      clearTimeout(waiter.timer);
      try {
        const result = JSON.parse(line);
        if (result.error) waiter.reject(new ApiError(result.error, 409)); else waiter.resolve(result.ok);
      } catch (error) { waiter.reject(error); }
    });
    this.child.on('error', error => this.fail(error));
    this.child.on('exit', code => {
      this.exited = true; this.fail(new ApiError('KERNEL_EXITED', 503)); onExit(code);
    });
    this.child.stdin.on('error', error => this.fail(error));
    this.child.stderr.on('data', bytes => { this.lastError = bytes.toString().slice(-2000); });
  }
  fail(error) { for (const waiter of this.pending.splice(0)) { clearTimeout(waiter.timer); waiter.reject(error); } }
  call(command) {
    if (this.exited || this.failed) return Promise.reject(new ApiError('KERNEL_EXITED', 503));
    if (this.pending.length >= 64) return Promise.reject(new ApiError('QUEUE_FULL', 429));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.failed = true; this.fail(new ApiError('KERNEL_TIMEOUT', 503)); this.child.kill(); }, 5000);
      this.pending.push({ resolve, reject, timer });
      this.child.stdin.write(`${JSON.stringify(command)}\n`);
    });
  }
  async dispose() {
    if (this.exited) return;
    const exited = new Promise(resolve => this.child.once('exit', resolve));
    this.child.stdin.end();
    const timer = setTimeout(() => this.child.kill(), 1000);
    await exited; clearTimeout(timer); this.lines.close();
  }
}

class Session {
  constructor(owner, { clock, fault, seed, scenario }, database, io, executeIo, configuration, record = null) {
    const metadata = record?.metadata;
    this.scenario = metadata?.scenario ?? scenario ?? configuration.defaultScenario;
    this.configuration = configuration;
    const selected=record ? configuration.find(record.originalCreate.config_id,record.originalCreate.config_hash) : configuration.catalog.get(this.scenario);
    if (!selected) throw new ApiError('CONFIG_VERSION_MISSING',503);
    const { config: sessionConfig, hash: sessionHash } = selected;
    this.configVersion=selected;
    this.id = metadata?.id ?? randomUUID(); this.owner = owner; this.clock = clock; this.fault = fault; this.seed = seed;
    this.database = database; this.record = record; this.io = io; this.executeIo = executeIo;
    this.deferredIo = new Map(); this.ioDone = new Map();
    this.key = { id: `h5-${this.id}`, incarnation: 1 };
    this.connection = metadata?.connection ?? { id: `web-${this.id}`, generation: 1 };
    this.originalCreate = record?.originalCreate ?? {
      protocol_version: sessionConfig.schema_version === 4 ? 3 : sessionConfig.schema_version === 3 ? 2 : 1, key: this.key, request_id: 'create', config_id: sessionConfig.id, config_hash: sessionHash,
      seed: this.seed, participant_id: 'A', authenticated_principal: configuration.account.principal, connection: { ...this.connection },
      publication_mode: this.clock === 'live' ? 'interactive' : 'batched',
    };
    if (!record) database.create({ id: this.id, owner, clock, fault, seed, scenario: this.scenario, key: this.key, connection: this.connection,
      phase: 'preparing', logicalMs: 0, sequence: 0, events: [] }, this.originalCreate, sessionConfig.schema_version === 2 ? configuration.account : undefined);
    this.store = new PersistentStore(database, this.id, configuration.account.stats, {config:sessionConfig,account:configuration.account});
    this.pending = new Set(record?.recovery?.checkpoint.state.operations.filter(op => !op.terminal).map(op => op.request.operation_id) ?? []);
    this.manual = new Map();
    this.publications = new Map(); this.history = []; this.eventId = 0;
    this.events = [...(metadata?.events ?? [])];
    for (const milestone of Object.values(metadata?.milestones ?? {})) {
      if (!this.events.some(event => JSON.stringify(event) === JSON.stringify(milestone))) this.events.push(milestone);
    }
    this.settlement = metadata?.settlement ?? null; this.recovered = metadata?.recovered ?? null;
    this.snapshot = metadata?.snapshot ?? null;
    this.resumes = new Map(); this.connected = false; this.disconnected = false;
    this.delayed = new Set(); this.ioAudit = []; this.startedAt = performance.now();
    this.phase = record ? 'closed' : 'preparing'; this.logicalMs = metadata?.logicalMs ?? 0;
    this.sequence = metadata?.sequence ?? 0; this.queue = new OrderedQueue({ full: () => new ApiError('QUEUE_FULL', 429) });
    this.kernel = new Kernel(clock === 'fast', () => {
      if (!this.disposed) { this.phase = 'faulted'; this.send('problem', { code: 'KERNEL_EXITED' }); }
    }, selected.manifest);
  }
  enqueue(work, control = true) {
    return this.queue.run(() => { if (this.disposed) throw new ApiError('CLOSED', 409); return work(); }, control);
  }
  status(status) {
    if (status.logical_ms < this.logicalMs) return;
    if (['closed', 'faulted'].includes(this.phase) && !['closed', 'faulted'].includes(status.phase)) return;
    const changed = this.phase !== status.phase;
    if (changed && ['closed', 'faulted'].includes(status.phase)) { this.terminalStatus = status; return; }
    this.phase = status.phase; this.logicalMs = status.logical_ms;
    if (changed) this.send('status', { phase: this.phase, logicalMs: this.logicalMs });
  }
  ledger() {
    return { items: this.store.items, warehouse: this.store.warehouse, cultivation: this.store.cultivation, revision: this.store.revision,
      entryYears: this.store.years, admissions: this.store.admissions, settlements: this.store.settlements,
      compensations: this.store.compensations, reconciliations: this.store.reconciliations,
      pending: [...this.pending], manual: this.manual.size };
  }
  publishLedger() {
    const ledger = this.ledger();
    const fingerprint = JSON.stringify(ledger);
    if (fingerprint !== this.lastLedger) {
      this.lastLedger = fingerprint; this.send('ledger', ledger);
    }
  }
  info() {
    return { id: this.id, scenario: this.scenario, configId: this.originalCreate.config_id, configHash: this.originalCreate.config_hash, key: this.key, clock: this.clock, fault: this.fault, seed: this.seed,
      phase: this.phase, logicalMs: this.logicalMs, sequence: this.sequence,
      settlement: this.settlement, recovered: this.recovered,
      host: { queue: this.queue.stats(), io: this.io.stats(), deferred: this.deferredIo.size, receipts: this.ioDone.size },
      connection: { ...this.connection }, connected: this.connected, events: [...this.events],
      io: { delayed: this.delayed.size, audit: [...this.ioAudit], requests: [...this.store.requests.values()] },
      ledger: this.ledger(), snapshot: this.snapshot ?? null };
  }
  send(type, data) {
    const frame = { id: ++this.eventId, type, data };
    this.history.push(frame); if (this.history.length > 128) this.history.shift();
    if (this.stream) this.write(frame);
  }
  write(frame) {
    if (!this.stream || this.stream.destroyed) return;
    if (this.stream.writableLength > 512 * 1024) { this.stream.destroy(); return; }
    this.stream.write(`id: ${frame.id}\nevent: ${frame.type}\ndata: ${JSON.stringify(frame.data)}\n\n`);
  }
  checkConnection(connection) {
    if (!sameConnection(connection, this.connection)) throw new ApiError('STALE_CONNECTION', 409);
  }
  async attach(response, connection) {
    return this.enqueue(async () => {
    this.checkConnection(connection);
    if (this.disconnected && ['preparing', 'deploying', 'active'].includes(this.phase)) throw new ApiError('RESUME_REQUIRED', 409);
    if (this.stream) throw new ApiError('ALREADY_ATTACHED', 409);
    this.connected = true;
    this.stream = response; response.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-store', 'Connection': 'keep-alive' });
    for (const frame of this.history) this.write(frame);
    this.send('session', this.info());
    response.on('close', () => {
      if (this.stream !== response) return;
      this.stream = null;
      this.connected = false;
      if (!this.disposed) this.lose(connection).catch(error => this.send('problem', { code: error.message }));
    });
    });
  }
  hostInput(action, connection = this.connection, requestId = randomUUID()) {
    return this.kernel.call({ type: 'submit_input', request: {
      key: this.key, participant_id: 'A', entity_generation: 1, connection,
      request_id: requestId, sequence: ++this.sequence, action,
    } });
  }
  lose(connection) {
    return this.enqueue(async () => {
      if (!sameConnection(connection, this.connection) || this.stream || !['preparing', 'active', 'deploying'].includes(this.phase)) return;
      const result = await this.hostInput({ kind: 'connection_lost' });
      this.disconnected = true; this.publications.clear(); this.status(result.status); await this.pump();
    });
  }
  resume({ connection, requestId }) {
    return this.enqueue(async () => {
      const prior = this.resumes.get(requestId);
      if (prior) {
        if (!sameConnection(prior.previous, connection)) throw new ApiError('CONFLICT', 409);
        this.checkConnection(prior.next); return this.info();
      }
      this.checkConnection(connection);
      if (['preparing', 'active', 'deploying'].includes(this.phase)) {
        const next = { id: `web-${randomUUID()}`, generation: connection.generation + 1 };
        if (next.generation > 4294967295) throw new ApiError('CONNECTION_EXHAUSTED', 409);
        const result = await this.hostInput({ kind: 'reconnect', previous_connection: connection,
          authenticated_principal: this.originalCreate.authenticated_principal }, next, requestId);
        this.connection = next; this.status(result.status);
      }
      const old = this.stream; this.stream = null; this.connected = false; this.disconnected = false;
      if (old) { old.write('event: replaced\ndata: {}\n\n'); old.end(); }
      this.publications.clear(); this.history = [];
      this.resumes.set(requestId, { previous: { ...connection }, next: { ...this.connection } });
      if (this.resumes.size > 16) this.resumes.delete(this.resumes.keys().next().value);
      await this.pump(); return this.info();
    });
  }
  async initialize() {
    await this.enqueue(async () => {
      let result = await this.kernel.call({ type: 'create', request: {
        ...this.originalCreate, ...(this.record?.recovery ? { recovery: this.record.recovery } : {}),
      } });
      if (this.record && !this.record.recovery) {
        result = await this.kernel.call({ type: 'close', request: { key: this.key, request_id: 'empty-recovery-close' } });
      }
      this.status(result.status); await this.pump();
    });
    this.timer = setInterval(() => {
      if (this.pumpQueued || this.disposed) return;
      this.pumpQueued = true;
      this.enqueue(() => this.pump()).catch(error => this.send('problem', { code: error.message }))
        .finally(() => { this.pumpQueued = false; });
    }, 30);
  }
  scheduleIo(request, execute = original => this.store.execute(original)) {
    const token = JSON.stringify([request.operation_id, request.attempt_id]);
    if (this.ioDone.has(token)) return;
    this.deferredIo.set(token, { request, execute });
    this.dispatchIo();
  }
  dispatchIo() {
    for (const [token, { request, execute }] of this.deferredIo) {
      const accepted = this.io.submit(this.id, request, frozen => this.executeIo({ sessionId: this.id,
        key: structuredClone(this.key), principal: this.originalCreate.authenticated_principal, request: frozen }, execute), outcome => {
        if (!this.disposed) this.ioDone.set(token, { request, outcome });
      });
      if (accepted) this.deferredIo.delete(token);
    }
  }
  async flushIo(milliseconds = 500) {
    const deadline = performance.now() + milliseconds;
    while (!this.disposed && (this.io.has(this.id) || this.ioDone.size || this.deferredIo.size) && performance.now() < deadline) {
      await this.enqueue(() => this.pump());
      await new Promise(resolve => setTimeout(resolve, 5));
    }
  }
  async pump() {
    this.dispatchIo();
    for (let round = 0; round < 24; round++) {
      for (const [token, receipt] of this.ioDone) {
        try { await this.deliver(receipt.request, receipt.outcome); }
        catch (error) {
          if (error.message === 'QUEUE_FULL') continue;
          if (!['CONFLICT', 'INVALID_MESSAGE', 'REVISION_CONFLICT'].includes(error.message)) throw error;
          this.auditIo(receipt.request, 'rejected_late_or_invalid_proof');
        }
        this.ioDone.delete(token);
      }
      // A failed durable write must not discard a batch already drained from Rust.
      // Retry that exact frozen batch before draining or executing anything else.
      const outputs = this.unsavedOutputs ?? (await this.kernel.call({ type: 'drain_outputs' })).outputs;
      if (!outputs.length) {
        if (this.terminalStatus) {
          this.phase = this.terminalStatus.phase; this.logicalMs = Math.max(this.logicalMs, this.terminalStatus.logical_ms);
          this.terminalStatus = null; this.publishLedger(); this.send('status', {phase:this.phase,logicalMs:this.logicalMs});
        }
        return;
      }
      this.unsavedOutputs = outputs;
      this.database.persistOutputs(this.id, outputs, this.info());
      this.unsavedOutputs = null;
      for (const raw of outputs) {
        const { recovery, ...output } = raw;
        const body = output.body;
        if (body.kind === 'business_request') {
          this.pending.add(body.request.operation_id);
          this.send('output', output);
          await this.business(body.request);
        } else if (body.kind === 'publication') {
          if (!sameConnection(body.connection, this.connection)) continue;
          this.snapshot = body.snapshot; this.status({ phase: body.snapshot.phase, logical_ms: output.logical_ms });
          if (!this.stream) this.publications.clear();
          this.publications.set(output.id, output);
          if (this.publications.size > 128) {
            this.send('problem', { code: 'CLIENT_TOO_SLOW' });
            this.stream?.destroy();
            // Refuse unbounded transport buffering even for a client that never acknowledges.
            const oldest = this.publications.keys().next().value; this.publications.delete(oldest);
          }
          this.send('output', output);
        } else {
          if (settlementEvent(body.event.kind)) this.settlement = body.event;
          if (body.event.kind === 'recovery_restored') this.recovered = { generation: body.event.generation, compensatingEntry: body.event.compensating_entry };
          this.events.push(body.event); if (this.events.length > 512) this.events.shift();
          this.logicalMs = Math.max(this.logicalMs, output.logical_ms);
          if (body.event.kind === 'operation_resolved') {
            this.pending.delete(body.event.operation_id); this.manual.delete(body.event.operation_id);
          }
          if (body.event.kind === 'closed') {
            for (const operation of body.event.pending_operations) this.pending.add(operation);
            this.phase = 'closed'; this.terminalStatus = null;
          }
          if (body.event.kind === 'fault') {this.phase = 'faulted'; this.terminalStatus = null;}
          this.send('output', output);
        }
        this.publishLedger();
      }
    }
    throw new ApiError('OUTPUT_BUSY', 503);
  }
  async business(request) {
    this.store.remember(request);
    if (request.body.kind === 'reconcile_operation') {
      const known = this.store.journal.get(request.operation_id);
      if (this.record && known?.kind === 'reconciled') { await this.deliver(request, known); return; }
      this.manual.set(request.operation_id, request);
      this.publishLedger();
      return;
    }
    const kind = businessKind(request);
    if (!this.record && ((kind === 'load_participant' && this.fault === 'load_silent')
      || (kind === 'commit_admission' && this.fault === 'admission_silent')
      || (kind === 'commit_settlement' && ['settlement_silent', 'query_silent'].includes(this.fault))
      || (kind === 'query_operation' && this.fault === 'query_silent')
      || (kind === 'compensate_admission' && this.fault === 'compensation_silent'))) {
      this.auditIo(request, 'silent'); return;
    }
    if (!this.record && ((kind === 'commit_admission' && ['admission_lost_receipt', 'compensation_silent'].includes(this.fault))
      || (kind === 'commit_settlement' && this.fault === 'settlement_lost_receipt'))) {
      this.store.execute(request); this.auditIo(request, 'committed_without_receipt'); return;
    }
    if (!this.record && ((kind === 'load_participant' && this.fault === 'load_delayed')
      || (kind === 'commit_settlement' && this.fault === 'settlement_delayed'))) {
      this.auditIo(request, 'scheduled');
      const timer = setTimeout(() => {
        this.delayed.delete(timer);
        this.enqueue(async () => {
          const outcome = this.store.execute(request); this.auditIo(request, 'late_execution');
          await this.deliver(request, outcome); await this.pump();
        }).catch(error => { if (!this.disposed) this.send('problem', { code: error.message }); });
      }, kind === 'load_participant' ? 1000 : 6500);
      this.delayed.add(timer); return;
    }
    this.scheduleIo(request, original => {
      let outcome;
      if (!this.record && businessKind(original) === 'commit_settlement' && this.fault === 'refused' && !this.injected) {
        this.injected = true; outcome = this.store.refuse(original);
        this.send('notice', { message: '存储拒绝结算。等待内核提出核对请求。' });
      } else {
        outcome = this.store.execute(original);
        if (!this.record && businessKind(original) === 'commit_settlement' && this.fault === 'lost_receipt' && !this.injected) {
          this.injected = true; outcome = { kind: 'unknown' };
          this.send('notice', { message: '模拟提交成功、回执丢失。由内核查询原业务结果。' });
        }
      }
      return outcome;
    });
  }

  auditIo(request, action) {
    this.ioAudit.push({ operationId: request.operation_id, kind: request.body.kind, attempt: request.attempt_id,
      action, elapsedMs: performance.now() - this.startedAt });
    if (this.ioAudit.length > 64) this.ioAudit.shift();
  }
  async deliver(request, outcome) {
    const result = await this.kernel.call({ type: 'submit_business_receipt', request: {
      key: this.key, operation_id: request.operation_id, attempt_id: request.attempt_id, outcome,
    } });
    this.database.crash('after_kernel_receipt', request);
    this.status(result.status);
  }
  input({ requestId, sequence, action, connection }) {
    return this.enqueue(async () => {
      this.checkConnection(connection);
      this.sequence = Math.max(this.sequence, sequence);
      const result = await this.kernel.call({ type: 'submit_input', request: {
        key: this.key, participant_id: 'A', entity_generation: 1, connection: this.connection,
        request_id: requestId, sequence, action,
      } });
      this.status(result.status);
      await this.pump(); return result;
    }, false);
  }
  ack(outputId, connection) {
    return this.enqueue(async () => {
      this.checkConnection(connection);
      const output = this.publications.get(outputId);
      if (!output) return { ignored: true };
      try {
        await this.kernel.call({ type: 'submit_transport_receipt', request: {
          key: this.key, output_id: output.id, recipient_id: output.body.recipient_id,
          connection: output.body.connection, outcome: 'sent',
        } });
      } catch (error) {
        if (!['CLOSED', 'STALE_PUBLICATION', 'INVALID_MESSAGE'].includes(error.message)) throw error;
        return { ignored: true };
      } finally { this.publications.delete(outputId); }
      return { acknowledged: true };
    }, false);
  }
  reconcile(connection) {
    return this.enqueue(async () => {
      this.checkConnection(connection);
      for (const request of [...this.manual.values()]) {
        this.scheduleIo(request);
      }
      await this.pump(); return this.info();
    });
  }
  close(connection) {
    return this.enqueue(async () => {
      if (connection) this.checkConnection(connection);
      const result = await this.kernel.call({ type: 'close', request: { key: this.key, request_id: 'h5-close' } });
      this.status(result.status); await this.pump(); return this.info();
    });
  }
  async dispose() {
    clearInterval(this.timer);
    for (const timer of this.delayed) clearTimeout(timer); this.delayed.clear();
    await this.close().catch(() => {});
    await this.flushIo().catch(() => {});
    this.io.cancel(this.id); this.disposed = true; this.stream?.end(); this.stream = null;
    await this.kernel.dispose();
  }
  async abort() {
    clearInterval(this.timer); for (const timer of this.delayed) clearTimeout(timer);
    this.io.cancel(this.id); this.disposed = true; this.stream?.end(); await this.kernel.dispose();
  }
}

function object(value, fields) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(key => !fields.includes(key))) throw new ApiError('INVALID_MESSAGE');
}
function sameConnection(a, b) { return a?.id === b?.id && a?.generation === b?.generation; }
function connectionBody(value) {
  object(value, ['id', 'generation']);
  if (typeof value.id !== 'string' || !value.id.length || value.id.length > 128
    || !Number.isInteger(value.generation) || value.generation < 1 || value.generation > 4294967295) throw new ApiError('INVALID_MESSAGE');
  return value;
}
async function jsonBody(request) {
  let bytes = 0; const chunks = [];
  for await (const chunk of request) {
    bytes += chunk.length; if (bytes > 16384) throw new ApiError('MESSAGE_TOO_LARGE', 413);
    chunks.push(chunk);
  }
  const text = Buffer.concat(chunks).toString('utf8');
  try { return text ? JSON.parse(text) : {}; } catch { throw new ApiError('INVALID_MESSAGE'); }
}
function json(response, status, value) {
  response.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
  response.end(JSON.stringify(value));
}

export async function createLabServer({ statePath = join(ROOT, '.local/h5-lab.sqlite'), crashAt, crashKind, ioExecute = ({ request }, execute) => execute(request), configManifest = process.env.MATCH_CONFIG_MANIFEST, account = null } = {}) {
  if (!configManifest || configManifest.startsWith('--')) throw new Error('Supply --configs <catalog.json> or MATCH_CONFIG_MANIFEST');
  await access(BINARY);
  const sourceCatalog = readSourceCatalog(BINARY,configManifest);
  const sessions = new Map(); const io = new IoScheduler();
  const database = new DurableLab(statePath, { crashAt, crashKind });
  let configuration;
  try {
    configuration = snapshotCatalog(database,BINARY,sourceCatalog);
    configuration.account = account;
    if (sourceCatalog.versions.some(v => v.config.schema_version > 4)) throw new Error('USE_TWO_MEMBER_SERVER_FOR_PROTOCOL_4_OR_LATER');
    if (!account?.principal || !account.stats || !Number.isInteger(account.revision)) throw new Error('EXPLICIT_ACCOUNT_REQUIRED');
    if (sourceCatalog.versions.some(v => v.config.schema_version === 2) && !Number.isInteger(account.warehouse_quantity)) throw new Error('EXPLICIT_WAREHOUSE_QUANTITY_REQUIRED');
    if (sourceCatalog.versions.some(v => v.config.schema_version >= 3) && !account.assets) throw new Error('EXPLICIT_ASSET_ACCOUNT_REQUIRED');
    for (const record of database.recoverable()) {
      const session = new Session(record.metadata.owner, record.metadata, database, io, ioExecute, configuration, record); sessions.set(session.id, session);
      await session.initialize(); await session.flushIo();
    }
  } catch (error) { await Promise.allSettled([...sessions.values()].map(session => session.abort())); database.close(); configuration?.dispose(); throw error; }
  const creating = new Set();
  const server = http.createServer(async (request, response) => {
    try {
      const url = new URL(request.url, 'http://127.0.0.1');
      let owner = /(?:^|;\s*)b1_owner=([a-f0-9-]{36})(?:;|$)/.exec(request.headers.cookie ?? '')?.[1];
      if (!owner) {
        if (!['GET', 'HEAD'].includes(request.method) || !['/', '/index.html'].includes(url.pathname)) throw new ApiError('OPEN_LAB_FIRST', 401);
        owner = randomUUID(); response.setHeader('Set-Cookie', `b1_owner=${owner}; HttpOnly; SameSite=Strict; Path=/`);
      }
      if (request.method === 'POST' && request.headers.origin) {
        const expected = `http://${request.headers.host}`;
        if (request.headers.origin !== expected) throw new ApiError('ORIGIN_MISMATCH', 403);
      }
      const assets = { '/': ['index.html', 'text/html'], '/index.html': ['index.html', 'text/html'],
        '/app.js': ['app.js', 'text/javascript'], '/isometric.js': ['isometric.js', 'text/javascript'],
        '/styles.css': ['styles.css', 'text/css'] };
      if (['GET', 'HEAD'].includes(request.method) && assets[url.pathname]) {
        const [file, mime] = assets[url.pathname];
        response.writeHead(200, { 'Content-Type': `${mime}; charset=utf-8`, 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' });
        response.end(request.method === 'HEAD' ? undefined : await readFile(join(PUBLIC, file))); return;
      }
      if (request.method === 'GET' && url.pathname === '/api/catalog') {
        json(response,200,{defaultScenario:configuration.defaultScenario,profiles:[...configuration.catalog].map(([scenario,version])=>({scenario,configId:version.id,configHash:version.hash,label:scenario}))});return;
      }
      if (request.method === 'GET' && url.pathname === '/api/config') {
        const sessionId = url.searchParams.get('sessionId');
        const pinned = sessionId ? sessions.get(sessionId) : null;
        if (sessionId && (!pinned || pinned.owner !== owner)) throw new ApiError('NOT_FOUND',404);
        const scenario = pinned?.scenario ?? url.searchParams.get('scenario') ?? configuration.defaultScenario;
        const selected = pinned?.configVersion ?? configuration.catalog.get(scenario);
        if (!selected) throw new ApiError('UNSUPPORTED');
        const config = selected.config;
        json(response, 200, { scenario, pve: config.pve ?? null, configId: config.id, configHash: selected.hash, geometry: config.geometry, spawn: config.spawn,
          extractionPoint: config.extraction_point, extractionUnlockMs: config.extraction_unlock_ms,
          extractionChannelMs: config.extraction_channel_ms, skill: config.skill, basic: config.basic,
          playerSkills: config.mechanism_version >= 26 ? [config.skill,...(config.additional_skills??[])] : undefined,
          target: { id: config.target.id, maxHp: config.target.hp, radius: config.target.radius },
          params:config.params, inventoryCapacity:config.inventory_capacity, stackLimit:config.stack_limit,
          itemId:config.item_id, carriedQuantity:config.carried_quantity, assetContract:config.asset_contract??null,
          player: { maxHp: config.params.initial_player.hp_points, maxSpirit: config.params.initial_player.spirit_points, radius: config.params.player_radius } }); return;
      }
      if (request.method === 'GET' && url.pathname === '/api/current') {
        const session = [...sessions.values()].find(session => session.owner === owner);
        json(response, 200, session?.info() ?? null); return;
      }
      if (request.method === 'POST' && url.pathname === '/api/sessions') {
        const body = await jsonBody(request); object(body, ['clock', 'fault', 'seed', 'previousConnection', 'scenario']);
        if (body.previousConnection != null) connectionBody(body.previousConnection);
        const options = { clock: body.clock ?? 'live', fault: body.fault ?? 'none', seed: body.seed ?? 7, scenario: body.scenario ?? configuration.defaultScenario };
        if (!configuration.catalog.has(options.scenario) || !['live', 'fast'].includes(options.clock) || !faultModes.includes(options.fault)
          || !Number.isInteger(options.seed) || options.seed < 0 || options.seed > 4294967295) throw new ApiError('INVALID_MESSAGE');
        if (creating.has(owner)) throw new ApiError('SESSION_BUSY', 409);
        creating.add(owner);
        try {
        const old = [...sessions.values()].find(session => session.owner === owner);
        if (old) {
          await old.close(connectionBody(body.previousConnection));
          await old.flushIo();
          if (old.pending.size) throw new ApiError('UNRESOLVED_IO', 409);
          await old.dispose(); sessions.delete(old.id);
        }
        if (sessions.size >= 8) throw new ApiError('SESSION_CAPACITY', 429);
        const session = new Session(owner, options, database, io, ioExecute, configuration); sessions.set(session.id, session);
        try { await session.initialize(); } catch (error) { await session.dispose(); sessions.delete(session.id); throw error; }
        json(response, 201, session.info()); return;
        } finally { creating.delete(owner); }
      }
      const match = /^\/api\/sessions\/([a-f0-9-]{36})(?:\/(events|input|ack|close|reconcile|resume))?$/.exec(url.pathname);
      if (!match) throw new ApiError('NOT_FOUND', 404);
      const session = sessions.get(match[1]);
      if (!session || session.owner !== owner) throw new ApiError('NOT_FOUND', 404);
      if (request.method === 'GET' && match[2] === 'events') {
        await session.attach(response, connectionBody({ id: url.searchParams.get('connectionId'), generation: Number(url.searchParams.get('generation')) })); return;
      }
      if (request.method === 'GET' && !match[2]) { json(response, 200, session.info()); return; }
      if (request.method !== 'POST') throw new ApiError('NOT_FOUND', 404);
      const body = await jsonBody(request);
      let result;
      switch (match[2]) {
        case 'input':
          object(body, ['requestId', 'sequence', 'action', 'connection']); connectionBody(body.connection);
          if (typeof body.requestId !== 'string' || body.requestId.length < 1 || body.requestId.length > 128
            || !Number.isInteger(body.sequence) || body.sequence < 1 || body.sequence > 4294967295) throw new ApiError('INVALID_MESSAGE');
          if (!body.action || typeof body.action.kind !== 'string' || !Object.hasOwn(actionFields, body.action.kind)) throw new ApiError('UNSUPPORTED');
          object(body.action, ['kind', ...actionFields[body.action.kind]]);
          result = await session.input(body); break;
        case 'ack':
          object(body, ['outputId', 'connection']); connectionBody(body.connection);
          if (typeof body.outputId !== 'string' || body.outputId.length > 128) throw new ApiError('INVALID_MESSAGE');
          result = await session.ack(body.outputId, body.connection); break;
        case 'resume':
          object(body, ['requestId', 'connection']); connectionBody(body.connection);
          if (typeof body.requestId !== 'string' || !body.requestId.length || body.requestId.length > 128) throw new ApiError('INVALID_MESSAGE');
          result = await session.resume(body); break;
        case 'close': object(body, ['connection']); result = await session.close(connectionBody(body.connection)); break;
        case 'reconcile': object(body, ['connection']); result = await session.reconcile(connectionBody(body.connection)); break;
        default: throw new ApiError('NOT_FOUND', 404);
      }
      json(response, 200, result);
    } catch (error) {
      if (!response.headersSent) json(response, error.status ?? 500, { error: error instanceof ApiError ? error.message : 'SERVER_ERROR' });
      else response.end();
    }
  });
  server.maxConnections = 128; server.requestTimeout = 5000; server.headersTimeout = 5000;
  return { server, async shutdown() {
    await Promise.allSettled([...sessions.values()].map(session => session.dispose()));
    const closed = new Promise(resolve => server.close(resolve)); server.closeAllConnections(); await closed;
    database.close(); configuration.dispose();
  } };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const index = process.argv.indexOf('--port');
  const port = index < 0 ? 4173 : Number(process.argv[index + 1]);
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error('Invalid --port');
  const value = flag => { const index = process.argv.indexOf(flag); return index < 0 ? undefined : process.argv[index + 1]; };
  const configManifest = value('--configs') ?? process.env.MATCH_CONFIG_MANIFEST;
  if (!configManifest || configManifest.startsWith('--')) throw new Error('Supply --configs <catalog.json> or MATCH_CONFIG_MANIFEST');
  const temporary = port === 0 && !value('--state') ? mkdtempSync(join(tmpdir(), 'rust-h5-lab-')) : null;
  const app = await createLabServer({ statePath: value('--state') ?? (temporary ? join(temporary, 'lab.sqlite') : undefined),
    configManifest, account: value('--account') ? JSON.parse(await readFile(value('--account'),'utf8')) : null, crashAt: value('--crash-at'), crashKind: value('--crash-kind') });
  app.server.listen(port, '127.0.0.1', () => console.log(JSON.stringify({ url: `http://127.0.0.1:${app.server.address().port}`, app: 'Match SDK Lab' })));
  let shuttingDown = false;
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, async () => {
    if (shuttingDown) return; shuttingDown = true;
    await app.shutdown(); if (temporary) rmSync(temporary, { recursive: true, force: true }); process.exit(0);
  });
}
