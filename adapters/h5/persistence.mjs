import { DatabaseSync } from 'node:sqlite';
import { mkdirSync, chmodSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { createHash, randomUUID } from 'node:crypto';
import assert from 'node:assert/strict';
import { ControlledStore } from '../node/controlled-store.mjs';
import { AssetStore, businessKind, settlementEvent } from '../node/asset-store.mjs';
export { capture as captureStore, load as restoreStore };

const fields = ['items', 'warehouse', 'cultivation', 'revision', 'years', 'admissions', 'settlements', 'compensations', 'reconciliations', 'balances'];
const durableWrites = new Set(['commit_admission', 'commit_settlement', 'compensate_admission', 'commit_assets', 'append_facts']);
const hash = text => createHash('sha256').update(text).digest('hex');
const capture = store => Object.fromEntries([
  ...fields.map(key => [key, store[key]]), ['requests', [...store.requests]], ['journal', [...store.journal]],
  ...(store.instances?.length ? [['instances', store.instances]] : []),
  ...(store.appendedFacts?.length ? [['appendedFacts', store.appendedFacts]] : []),
  ...(store.location ? [['location', store.location]] : []),
  ...(store.savedPosition ? [['savedPosition', store.savedPosition]] : []),
  ...(store.warehouseOtherSlots != null ? [['warehouseOtherSlots', store.warehouseOtherSlots]] : []),
  ['fenced', [...store.fenced]], ['audit', store.audit],
]);
function load(store, state) {
  for (const key of fields) store[key] = key === 'items' ? (state[key] ?? {}) : key === 'balances' ? (state[key] ?? null) : state[key];
  if (store instanceof AssetStore) {
    store.instances = state.instances ?? [];
    store.appendedFacts = state.appendedFacts ?? [];
    store.location = state.location ?? null;
    store.savedPosition = state.savedPosition ?? null;
    store.warehouseOtherSlots = state.warehouseOtherSlots ?? null;
  }
  store.requests = new Map(state.requests); store.journal = new Map(state.journal);
  store.fenced = new Set(state.fenced); store.audit = state.audit;
}
function remember(state, request) {
  const previous = state.requests.find(([id]) => id === request.operation_id);
  if (previous) {
    assert.deepEqual(previous[1].body, request.body, 'FROZEN_PAYLOAD_CONFLICT');
    if (previous[1].attempt_id <= request.attempt_id) previous[1] = structuredClone(request);
  } else state.requests.push([request.operation_id, structuredClone(request)]);
}

export class DurableLab {
  constructor(path, { crashAt = '', crashKind = '' } = {}) {
    this.path = resolve(path); this.lastView = new Map(); this.crashAt = crashAt; this.crashKind = crashKind;
    mkdirSync(dirname(this.path), { recursive: true, mode: 0o700 });
    this.db = new DatabaseSync(this.path);
    try {
      this.db.exec(`PRAGMA busy_timeout=500; PRAGMA locking_mode=EXCLUSIVE; PRAGMA journal_mode=WAL;
        PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM schema_version);
        CREATE TABLE IF NOT EXISTS config_versions (
          config_id TEXT NOT NULL, hash TEXT NOT NULL, raw TEXT NOT NULL, PRIMARY KEY(config_id,hash)
        );
        CREATE TABLE IF NOT EXISTS sessions (
          id TEXT PRIMARY KEY, owner TEXT NOT NULL, current INTEGER NOT NULL,
          metadata TEXT NOT NULL, original_create TEXT NOT NULL,
          checkpoint TEXT, checkpoint_digest TEXT, recovery_generation INTEGER NOT NULL DEFAULT 0
        );
        CREATE UNIQUE INDEX IF NOT EXISTS current_owner ON sessions(owner) WHERE current=1;
        CREATE TABLE IF NOT EXISTS stores (session_id TEXT PRIMARY KEY REFERENCES sessions(id), state TEXT NOT NULL, digest TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS outputs (
          session_id TEXT NOT NULL REFERENCES sessions(id), output_id TEXT NOT NULL,
          body TEXT NOT NULL, PRIMARY KEY(session_id,output_id)
        );`);
      assert.equal(this.db.prepare('SELECT version FROM schema_version').get().version, 1, 'UNSUPPORTED_STORAGE_VERSION');
      assert.equal(this.db.prepare('PRAGMA quick_check').get().quick_check, 'ok', 'STATE_CORRUPT');
      chmodSync(this.path, 0o600);
    } catch (error) { this.db.close(); throw new Error(`STATE_UNAVAILABLE: ${error.message}`); }
  }
  transaction(work) {
    this.db.exec('BEGIN IMMEDIATE');
    try { const result = work(); this.db.exec('COMMIT'); return result; }
    catch (error) { this.db.exec('ROLLBACK'); throw error; }
  }
  crash(point, request) {
    if (this.crashAt === point && request && businessKind(request) === this.crashKind) {
      process.stdout.write(`${JSON.stringify({ crashPoint: point, kind: this.crashKind })}\n`);
      process.kill(process.pid, 'SIGKILL');
    }
  }
  create(metadata, originalCreate, account) {
    this.transaction(() => {
      this.db.prepare('UPDATE sessions SET current=0 WHERE owner=?').run(metadata.owner);
      this.db.prepare('INSERT INTO sessions(id,owner,current,metadata,original_create) VALUES (?,?,1,?,?)')
        .run(metadata.id, metadata.owner, JSON.stringify(metadata), JSON.stringify(originalCreate));
      const state = JSON.stringify(capture(new ControlledStore(account?.stats, account)));
      this.db.prepare('INSERT INTO stores VALUES (?,?,?)').run(metadata.id, state, hash(state));
    });
  }
  rememberConfigs(versions) {
    this.transaction(() => {
      for (const version of versions) {
        if (hash(version.raw) !== version.hash || JSON.parse(version.raw).id !== version.id) throw new Error('CONFIG_VERSION_CORRUPT');
        const existing = this.db.prepare('SELECT raw FROM config_versions WHERE config_id=? AND hash=?').get(version.id,version.hash);
        if (existing && existing.raw !== version.raw) throw new Error('CONFIG_VERSION_CONFLICT');
        this.db.prepare('INSERT OR IGNORE INTO config_versions VALUES (?,?,?)').run(version.id,version.hash,version.raw);
      }
    });
  }
  configs() {
    return this.db.prepare('SELECT config_id AS id,hash,raw FROM config_versions').all().map(version => {
      if (hash(version.raw) !== version.hash || JSON.parse(version.raw).id !== version.id) throw new Error('CONFIG_VERSION_CORRUPT');
      return version;
    });
  }
  requiredConfigs() { return this.db.prepare('SELECT original_create FROM sessions WHERE current=1').all().map(row => JSON.parse(row.original_create)); }
  readStore(id) {
    const row = this.db.prepare('SELECT state,digest FROM stores WHERE session_id=?').get(id);
    if (!row || hash(row.state) !== row.digest) throw new Error('STORE_CORRUPT');
    return JSON.parse(row.state);
  }
  writeStore(id, state) {
    const text = JSON.stringify(state);
    this.db.prepare('UPDATE stores SET state=?,digest=? WHERE session_id=?').run(text, hash(text), id);
  }
  saveMetadata(id, changes) {
    this.transaction(() => {
      const row = this.db.prepare('SELECT metadata FROM sessions WHERE id=?').get(id);
      const current = JSON.parse(row.metadata);
      this.db.prepare('UPDATE sessions SET metadata=? WHERE id=?').run(JSON.stringify({ ...current, ...changes }), id);
    });
  }
  persistOutputs(id, outputs, view) {
    if (!outputs.length) return;
    const critical = outputs.some(output => output.recovery || output.body.kind !== 'publication');
    if (!critical && performance.now() - (this.lastView.get(id) ?? 0) < 250) return;
    const business = outputs.find(output => output.body.kind === 'business_request')?.body.request;
    if (business) this.crash('after_drain_before_persist', business);
    this.transaction(() => {
      const row = this.db.prepare('SELECT * FROM sessions WHERE id=?').get(id);
      const old = JSON.parse(row.metadata);
      const metadata = { ...old, ...view, owner: old.owner, events: old.events ?? [], milestones: old.milestones ?? {} };
      delete metadata.ledger; delete metadata.io;
      const state = this.readStore(id);
      let checkpoint = row.checkpoint ? JSON.parse(row.checkpoint) : null;
      for (const output of outputs) {
        const body = output.body;
        if (output.recovery) {
          const candidate = output.recovery;
          if (!checkpoint || candidate.state.recovery_generation > checkpoint.state.recovery_generation
            || (candidate.state.recovery_generation === checkpoint.state.recovery_generation && candidate.state.output_sequence >= checkpoint.state.output_sequence)) checkpoint = candidate;
        }
        metadata.logicalMs = Math.max(metadata.logicalMs ?? 0, output.logical_ms);
        if (body.kind === 'business_request') remember(state, body.request);
        if (body.kind === 'publication' && body.connection.id === metadata.connection.id && body.connection.generation === metadata.connection.generation) {
          metadata.snapshot = body.snapshot;
          if (!['closed', 'faulted'].includes(metadata.phase)) metadata.phase = body.snapshot.phase;
        }
        if (body.kind === 'event') {
          const inserted = this.db.prepare('INSERT OR IGNORE INTO outputs VALUES (?,?,?)').run(id, output.id, JSON.stringify(body));
          if (inserted.changes) {
            metadata.events.push(body.event); if (metadata.events.length > 512) metadata.events.shift();
            const event = body.event;
            if (['activated', 'target_killed', 'picked_up', 'settlement_applied', 'closed'].includes(event.kind)) {
              metadata.milestones[`${event.kind}/${event.resource_id ?? ''}`] = event;
            }
            if (settlementEvent(event.kind)) { metadata.settlement = event; metadata.milestones.settlement = event; }
            if (event.kind === 'closed') metadata.phase = 'closed';
            if (event.kind === 'fault') metadata.phase = 'faulted';
            if (event.kind === 'recovery_restored') metadata.recovered = { generation: event.generation, compensatingEntry: event.compensating_entry };
          }
        }
      }
      this.writeStore(id, state);
      const text = checkpoint ? JSON.stringify(checkpoint) : null;
      this.db.prepare('UPDATE sessions SET metadata=?,checkpoint=?,checkpoint_digest=? WHERE id=?')
        .run(JSON.stringify(metadata), text, text ? hash(text) : null, id);
    });
    this.lastView.set(id, performance.now());
    if (business) this.crash('after_intent_persist', business);
    const resolved = outputs.find(output => output.body.kind === 'event' && settlementEvent(output.body.event.kind));
    if (resolved) this.crash('after_result_persist', { body: { kind: 'commit_settlement' } });
  }
  recoverable() {
    return this.transaction(() => this.db.prepare('SELECT * FROM sessions WHERE current=1 ORDER BY rowid').all().map(row => {
      if (row.checkpoint && hash(row.checkpoint) !== row.checkpoint_digest) throw new Error('CHECKPOINT_CORRUPT');
      const checkpoint = row.checkpoint ? JSON.parse(row.checkpoint) : null;
      const originalCreate = JSON.parse(row.original_create);
      const state = this.readStore(row.id);
      if (!checkpoint && state.requests.some(([, request]) => durableWrites.has(request.body.kind))) throw new Error('MISSING_IO_CHECKPOINT');
      for (const op of checkpoint?.state.operations ?? []) {
        const registered = state.requests.find(([id]) => id === op.request.operation_id);
        if (!registered) throw new Error('MISSING_FROZEN_REQUEST');
        assert.deepEqual(registered[1].body, op.request.body, 'FROZEN_PAYLOAD_CONFLICT');
        const proof = state.journal.find(([id]) => id === op.request.operation_id)?.[1];
        if (durableWrites.has(op.request.body.kind) && !op.terminal && !['committed', 'facts_appended', 'refused'].includes(proof?.kind)) {
          state.fenced = [...new Set([...state.fenced, `${op.request.operation_id}/${op.request.attempt_id}`])];
        }
      }
      this.writeStore(row.id, state);
      const metadata = JSON.parse(row.metadata);
      const generation = Math.max(row.recovery_generation, checkpoint?.state.recovery_generation ?? 0) + 1;
      if (generation > 4294967295 || metadata.connection.generation >= 4294967295) throw new Error('RECOVERY_GENERATION_EXHAUSTED');
      metadata.connection = { id: `web-${randomUUID()}`, generation: metadata.connection.generation + 1 };
      metadata.phase = 'closed'; metadata.connected = false; metadata.recovered = { generation, compensatingEntry: false };
      // A recovered responsibility ledger has no combat world or active status instances.
      if (metadata.snapshot) {
        delete metadata.snapshot.statuses; delete metadata.snapshot.effective_def; delete metadata.snapshot.effective_atk; delete metadata.snapshot.shield_total; delete metadata.snapshot.healing_received_bps; delete metadata.snapshot.player_skills; delete metadata.snapshot.pending_action;
        if(metadata.snapshot.pve){delete metadata.snapshot.pve.player_area_warnings;delete metadata.snapshot.pve.player_fields;delete metadata.snapshot.pve.piercing_projectiles;}
        for (const monster of metadata.snapshot.pve?.monsters ?? []) { delete monster.unit_target; delete monster.statuses; delete monster.effective_def; delete monster.effective_atk; delete monster.shield_total; delete monster.healing_received_bps; }
      }
      this.db.prepare('UPDATE sessions SET metadata=?,recovery_generation=? WHERE id=?').run(JSON.stringify(metadata), generation, row.id);
      return { metadata, originalCreate, recovery: checkpoint ? { checkpoint, generation } : null };
    }));
  }
  metadata(id) { return JSON.parse(this.db.prepare('SELECT metadata FROM sessions WHERE id=?').get(id).metadata); }
  close() { this.db.close(); }
}

export class PersistentStore extends AssetStore {
  constructor(database, id, initialStats, options) { super(initialStats, options); this.database = database; this.id = id; this.depth = 0; load(this, database.readStore(id)); }
  transact(work, request) {
    if (this.depth) return work();
    let result;
    try {
      result = this.database.transaction(() => {
        load(this, this.database.readStore(this.id)); this.depth++;
        try {
          const result = work(); this.database.writeStore(this.id, capture(this));
          if (request) this.database.crash('before_storage_commit', request);
          return result;
        } finally { this.depth--; }
      });
    } catch (error) { load(this, this.database.readStore(this.id)); throw error; }
    if (request) this.database.crash('after_storage_commit', request);
    return result;
  }
  remember(request) { return this.transact(() => super.remember(request)); }
  refuse(request) { return this.transact(() => super.refuse(request), request); }
  notCommitted(request) { return this.transact(() => super.notCommitted(request), request); }
  execute(request, repair = false) { return this.transact(() => super.execute(request, repair), request); }
}
