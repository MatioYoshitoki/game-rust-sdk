// B1 verification storage only. All gameplay quantities arrive in frozen Rust requests.
import assert from 'node:assert/strict';

export class ControlledStore {
  constructor(initialStats, account) {
    this.initialStats = structuredClone(initialStats);
    this.principal = account?.principal;
    this.warehouse = account?.warehouse_quantity ?? 0;
    this.revision = account?.revision ?? 0;
  }
  cultivation = 0;
  items = {};
  years = 0;
  admissions = 0;
  settlements = 0;
  compensations = 0;
  reconciliations = 0;
  requests = new Map();
  journal = new Map();
  fenced = new Set();
  audit = [];

  remember(request) {
    const previous = this.requests.get(request.operation_id);
    if (previous) assert.deepEqual(previous.body, request.body);
    this.requests.set(request.operation_id, structuredClone(request));
  }

  refuse(request) {
    this.remember(request);
    assert.ok(!['committed','facts_appended'].includes(this.journal.get(request.operation_id)?.kind));
    const result = { kind: 'refused' };
    this.journal.set(request.operation_id, result);
    this.audit.push({ operationId: request.operation_id, outcome: 'refused', attempt: request.attempt_id });
    return result;
  }

  notCommitted(request) {
    this.remember(request);
    this.fenced.add(`${request.operation_id}/${request.attempt_id}`);
    this.audit.push({ operationId: request.operation_id, outcome: 'fenced_not_committed', attempt: request.attempt_id });
    return { kind: 'not_committed' };
  }

  execute(request, repair = false) {
    this.remember(request);
    const previous = this.journal.get(request.operation_id);
    if (previous && (previous.kind !== 'refused' || !repair)) return previous;
    if (!repair && this.fenced.has(`${request.operation_id}/${request.attempt_id}`)) return { kind: 'not_committed' };
    const body = request.body;
    let result;
    switch (body.kind) {
      case 'load_participant':
        assert.ok(this.principal && this.initialStats, 'EXPLICIT_PARTICIPANT_ACCOUNT_REQUIRED');
        result = { kind: 'loaded', facts: { principal: this.principal, revision: this.revision,
          ...this.initialStats,
          warehouse_quantity: this.warehouse } }; break;
      case 'commit_admission':
        if (body.expected_revision !== this.revision || this.warehouse < body.quantity) return this.refuse(request);
        this.warehouse -= body.quantity; this.years += body.entry_years; this.revision++; this.admissions++;
        result = { kind: 'committed', revision: this.revision, commit_id: `tx-${this.revision}` }; break;
      case 'commit_settlement': {
        const consumed = id => (body.map_progress?.consumed ?? []).find(row => row.item_id === id)?.quantity ?? 0;
        if (body.expected_revision !== this.revision) return this.refuse(request);
        assert.equal(body.carried + body.looted, body.kept + body.dropped + consumed(body.item_id));
        for (const item of body.items ?? []) {
          assert.equal(item.looted, item.kept + item.dropped + consumed(item.item_id));
          assert.notEqual(item.item_id, body.item_id);
          this.items[item.item_id] = (this.items[item.item_id] ?? 0) + item.kept;
        }
        this.warehouse += body.kept; this.cultivation += body.cultivation; this.revision++; this.settlements++;
        result = { kind: 'committed', revision: this.revision, commit_id: `tx-${this.revision}` }; break;
      }
      case 'compensate_admission': {
        assert.equal(this.journal.get(body.original_operation_id)?.kind, 'committed');
        const original = this.requests.get(body.original_operation_id).body;
        assert.equal(body.quantity, original.quantity); assert.equal(body.entry_years, original.entry_years);
        assert.ok(this.years >= body.entry_years);
        this.warehouse += body.quantity; this.years -= body.entry_years; this.revision++; this.compensations++;
        result = { kind: 'committed', revision: this.revision, commit_id: `tx-${this.revision}` }; break;
      }
      case 'query_operation': {
        const committed = this.journal.get(body.operation_id);
        const original = this.requests.get(body.operation_id);
        result = committed ?? (original && this.fenced.has(`${original.operation_id}/${original.attempt_id}`)
          ? { kind: 'not_committed' } : { kind: 'unknown' });
        break;
      }
      case 'reconcile_operation': {
        // Controlled stand-in for an authorized storage-side investigation/correction.
        // It uses the original immutable intent, never a gameplay phase or new amount.
        const original = this.requests.get(body.operation_id);
        assert.ok(original && ['commit_admission', 'commit_settlement', 'compensate_admission'].includes(original.body.kind));
        const committed = this.execute(original, true);
        if (committed.kind !== 'committed') return { kind: 'unknown' };
        this.reconciliations++;
        result = { kind: 'reconciled', revision: committed.revision, commit_id: committed.commit_id };
        break;
      }
      default: throw new Error(`Unsupported I/O request ${body.kind}`);
    }
    if (result.kind !== 'unknown') this.journal.set(request.operation_id, result);
    return result;
  }
}
