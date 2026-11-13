// Host-side bounds. Neither queue is allowed to decide gameplay or retry asset writes.
export class OrderedQueue {
  constructor({ capacity = 64, reserved = 8, full = () => Object.assign(new Error('QUEUE_FULL'), { status: 429 }) } = {}) {
    this.capacity = capacity; this.reserved = reserved; this.full = full;
    this.depth = 0; this.ordinary = 0; this.rejected = 0; this.peak = 0; this.tail = Promise.resolve();
  }
  run(work, control = false) {
    if (this.depth >= this.capacity || (!control && this.ordinary >= this.capacity - this.reserved)) {
      this.rejected++; return Promise.reject(this.full());
    }
    this.depth++; if (!control) this.ordinary++; this.peak = Math.max(this.peak, this.depth);
    const result = this.tail.then(work).finally(() => { this.depth--; if (!control) this.ordinary--; });
    this.tail = result.catch(() => {}); return result;
  }
  stats() { return { depth: this.depth, capacity: this.capacity, reserved: this.reserved, peak: this.peak, rejected: this.rejected }; }
}

export class IoScheduler {
  constructor({ concurrency = 8, perOwner = 2, capacity = 32 } = {}) {
    this.concurrency = concurrency; this.perOwner = perOwner; this.capacity = capacity;
    this.entries = new Map(); this.waiting = []; this.owners = new Map(); this.active = 0; this.peak = 0; this.rejected = 0;
  }
  submit(owner, request, execute, complete) {
    const key = JSON.stringify([owner, request.operation_id, request.attempt_id]);
    const existing = this.entries.get(key);
    if (existing) {
      if (JSON.stringify(existing.request.body) !== JSON.stringify(request.body)) throw new Error('FROZEN_PAYLOAD_CONFLICT');
      return true;
    }
    if (this.entries.size >= this.capacity) { this.rejected++; return false; }
    const entry = { key, owner, request: structuredClone(request), execute, complete };
    this.entries.set(key, entry); this.waiting.push(entry); this.peak = Math.max(this.peak, this.entries.size);
    this.dispatch(); return true;
  }
  dispatch() {
    while (this.active < this.concurrency) {
      const index = this.waiting.findIndex(entry => (this.owners.get(entry.owner) ?? 0) < this.perOwner);
      if (index < 0) return;
      const [entry] = this.waiting.splice(index, 1);
      this.active++; this.owners.set(entry.owner, (this.owners.get(entry.owner) ?? 0) + 1);
      Promise.resolve().then(() => entry.execute(structuredClone(entry.request)))
        .catch(() => ({ kind: 'unknown' })) // A transport failure is never proof that a write did not commit.
        .then(outcome => { if (!entry.cancelled) entry.complete(outcome); })
        .catch(() => {}) // Session retains receipt delivery separately; no unhandled host rejection.
        .finally(() => {
          this.entries.delete(entry.key); this.active--;
          const count = this.owners.get(entry.owner) - 1;
          if (count) this.owners.set(entry.owner, count); else this.owners.delete(entry.owner);
          this.dispatch();
        });
    }
  }
  cancel(owner) {
    for (const entry of this.entries.values()) if (entry.owner === owner) entry.cancelled = true;
    this.waiting = this.waiting.filter(entry => {
      if (entry.owner !== owner) return true;
      this.entries.delete(entry.key); return false;
    });
    // Running writes retain their slots until they actually finish. A local timeout cannot revoke a remote commit.
  }
  has(owner) { return [...this.entries.values()].some(entry => entry.owner === owner); }
  stats() { return { active: this.active, queued: this.waiting.length, retained: this.entries.size,
    concurrency: this.concurrency, capacity: this.capacity, peak: this.peak, rejected: this.rejected }; }
}
