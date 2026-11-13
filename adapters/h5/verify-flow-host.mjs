// Verification-only process: controls a fake remote transport via stdin, never a public HTTP route.
import { createLabServer } from './server.mjs';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
const directory = mkdtempSync(join(tmpdir(), 'rust-flow-host-'));
const mode = process.argv[2] ?? 'normal';
let owner; let injected = false; const held = [];
const app = await createLabServer({ statePath: join(directory, 'lab.sqlite'), ioExecute: (context, execute) => {
  owner ??= context.sessionId;
  const request = context.request;
  const selected = context.sessionId === owner;
  const kind = request.body.kind;
  if (selected && !injected && ((mode === 'hold_settlement' && kind === 'commit_settlement')
    || (mode === 'hold_load' && kind === 'load_participant'))) {
    injected = true;
    return new Promise(resolve => held.push(() => resolve(execute(request))));
  }
  if (selected && !injected && mode === 'throw_before_admission' && kind === 'commit_admission') {
    injected = true; throw new Error('transport failed with no authoritative non-commit proof');
  }
  if (selected && !injected && mode === 'throw_after_commit' && kind === 'commit_settlement') {
    injected = true; execute(request); throw new Error('committed, response lost');
  }
  if (selected && !injected && mode === 'invalid_proof' && kind === 'commit_settlement') {
    injected = true; return { kind: 'committed', revision: 999999, commit_id: 'invalid-remote-proof' };
  }
  return execute(request);
} });
app.server.listen(0, '127.0.0.1', () => console.log(JSON.stringify({ url: `http://127.0.0.1:${app.server.address().port}` })));
const input = createInterface({ input: process.stdin });
input.on('line', line => { if (line === 'release') for (const finish of held.splice(0)) finish(); });
let stopping = false;
process.on('SIGTERM', async () => {
  if (stopping) return; stopping = true;
  await app.shutdown(); input.close(); rmSync(directory, { recursive: true, force: true }); process.exit(0);
});
