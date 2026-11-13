"""Local protocol-2 reference host: frozen asset transactions + exclusive SQLite journal.

This synchronous local executor has no remote/in-flight writers after its process exits.
Only that constraint permits a missing local commit to be fenced as NotCommitted.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import sqlite3
import subprocess
import time
import uuid


def packed(value):
    return json.dumps(value, separators=(',', ':'), sort_keys=True)


def purpose(transaction):
    kind = transaction.get('kind', '')
    return 'settlement' if kind == 'result_settlement' else kind


class Ledger:
    def __init__(self, path):
        self.db = sqlite3.connect(path, timeout=.5)
        self.db.executescript('PRAGMA locking_mode=EXCLUSIVE; PRAGMA synchronous=FULL; '
                              'CREATE TABLE IF NOT EXISTS host (id INTEGER PRIMARY KEY, state TEXT NOT NULL);')
        assert self.db.execute('PRAGMA quick_check').fetchone()[0] == 'ok'
        row = self.db.execute('SELECT state FROM host WHERE id=1').fetchone()
        self.state = json.loads(row[0]) if row else None

    def save(self):
        with self.db:
            self.db.execute('INSERT OR REPLACE INTO host VALUES (1,?)', (packed(self.state),))

    def accept(self, outputs):
        for output in outputs:
            if output.get('recovery'):
                self.state['checkpoint'] = output['recovery']
            body = output['body']
            if body['kind'] == 'business_request':
                request = body['request']
                old = self.state['requests'].get(request['operation_id'])
                assert not old or old['body'] == request['body'], 'FROZEN_PAYLOAD_CONFLICT'
                self.state['requests'][request['operation_id']] = request
            elif body['kind'] == 'event':
                self.state['events'].setdefault(output['id'], body['event'])
        # Freeze the entire batch before account I/O or local-player delivery.
        self.save()

    def execute(self, request, *, refuse=False, repair=False):
        op = request['operation_id']
        frozen = self.state['requests'].get(op)
        assert frozen and frozen['body'] == request['body'], 'UNFROZEN_OR_CONFLICTING_REQUEST'
        previous = self.state['journal'].get(op)
        if previous and not (repair and previous['kind'] == 'refused'):
            return previous
        body = request['body']
        if body['kind'] == 'load_assets':
            initial = self.state['initial']
            result = {'kind': 'assets_loaded', 'facts': {
                'principal': initial['principal'], 'revision': self.state['revision'],
                **initial['stats'], 'assets': [{'asset_id': k, 'quantity': v}
                                             for k, v in sorted(self.state['balances'].items())]}}
        elif body['kind'] == 'query_operation':
            # The exclusive local process serializes ALL account writes. No delayed remote writer exists.
            result = self.state['journal'].get(body['operation_id'], {'kind': 'not_committed'})
        elif body['kind'] == 'reconcile_operation':
            if not repair:
                return {'kind': 'unknown'}
            original = self.state['requests'][body['operation_id']]
            proof = self.execute(original, repair=True)
            if proof['kind'] != 'committed':
                return {'kind': 'unknown'}
            result = {**proof, 'kind': 'reconciled'}
        elif body['kind'] == 'commit_assets':
            transaction = body['transaction']
            kind = purpose(transaction)
            assert kind in ['admission', 'settlement', 'compensation']
            revision = transaction.get('expected_revision')
            if kind == 'compensation':
                original = self.state['requests'][transaction['original_operation_id']]
                proof = self.state['journal'][transaction['original_operation_id']]
                assert proof['kind'] == 'committed'
                assert original['body']['transaction']['kind'] == 'admission'
                assert transaction['credits'] == original['body']['transaction']['debits']
                revision = proof['revision']
            rows = transaction.get('debits', transaction.get('credits', []))
            assert len(rows) <= 65 and all(0 < row['quantity'] <= 2**32-1 for row in rows)
            assert [row['asset_id'] for row in rows] == sorted({row['asset_id'] for row in rows})
            balances = dict(self.state['balances'])
            sign = -1 if kind == 'admission' else 1
            for row in rows:
                balances[row['asset_id']] = balances.get(row['asset_id'], 0) + sign * row['quantity']
            if refuse or revision != self.state['revision'] or any(not 0 <= q <= 2**32-1 for q in balances.values()):
                result = {'kind': 'refused'}
            else:
                self.state['balances'] = balances
                self.state['revision'] += 1
                self.state['counts'][kind] += 1
                result = {'kind': 'committed', 'revision': self.state['revision'], 'commit_id': f'local-tx-{op}'}
        else:
            raise ValueError(f'unsupported business kind: {body["kind"]}')
        self.state['journal'][op] = result
        # Balances, revision and commit proof are committed in one SQLite transaction.
        self.save()
        return result


class Host:
    def __init__(self, args, ledger):
        self.args, self.ledger = args, ledger
        self.state = ledger.state
        snapshot = Path(args.state).resolve().parent / (Path(args.state).name + '.config')
        snapshot.mkdir(exist_ok=True)
        raw = self.state['config_raw']
        assert hashlib.sha256(raw.encode()).hexdigest() == self.state['create']['config_hash']
        (snapshot / 'version.json').write_text(raw)
        (snapshot / 'catalog.json').write_text(packed({'schema_version': 1, 'default_scenario': 'frozen',
                                                      'active': {'frozen': 'version.json'}, 'archives': []}))
        command = [args.binary, '--configs', str(snapshot / 'catalog.json')]
        if args.verification:
            command.append('--verification')
        self.worker = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=None, text=True, bufsize=1)
        self.snapshot = None
        self.sequence = 0
        self.phase = 'preparing'
        self.ready_sent = False

    def rpc(self, kind, request=None):
        command = {'type': kind}
        if request is not None:
            command['request'] = request
        self.worker.stdin.write(packed(command) + '\n')
        self.worker.stdin.flush()
        if not select.select([self.worker.stdout], [], [], 6)[0]:
            raise TimeoutError('RPC response unknown; persist responsibility and investigate')
        line = self.worker.stdout.readline()
        if not line:
            raise RuntimeError(f'kernel exited: {self.worker.poll()}')
        result = json.loads(line)
        if 'error' in result:
            raise RuntimeError(result['error'])
        if 'status' in result['ok']:
            self.phase = result['ok']['status']['phase']
        return result['ok']

    def action(self, action):
        self.sequence += 1
        request = {'key': self.state['create']['key'], 'participant_id': self.state['create']['participant_id'],
                   'entity_generation': 1, 'connection': self.state['create']['connection'],
                   'request_id': f'input-{self.sequence}', 'sequence': self.sequence, 'action': action}
        self.rpc('submit_input', request)
        self.rpc('submit_input', request)

    def close(self):
        request = {'key': self.state['create']['key'], 'request_id': 'close'}
        self.rpc('close', request)
        self.rpc('close', request)

    def crash(self, point, kind):
        if kind and getattr(self.args, 'crash_' + point) == kind:
            print(packed({'crash': point, 'kind': kind}), flush=True)
            os.kill(os.getpid(), signal.SIGKILL)

    def pump(self):
        outputs = self.rpc('drain_outputs')['outputs']
        self.ledger.accept(outputs)
        for output in outputs:
            body = output['body']
            if body['kind'] == 'business_request':
                request = body['request']
                transaction = request['body'].get('transaction', {})
                kind = purpose(transaction)
                self.crash('before', kind)
                if self.args.mode == 'retry-settlement' and kind == 'settlement' and request['attempt_id'] == 1:
                    # This synchronous executor has not issued this attempt and fences it locally.
                    outcome = {'kind': 'not_committed'}
                else:
                    refuse = ((self.args.mode == 'refused-settlement' and kind == 'settlement')
                              or (self.args.mode == 'admission-refused' and kind == 'admission'))
                    outcome = self.ledger.execute(request, refuse=refuse, repair=self.args.reconcile)
                    committed_state = packed(self.state)
                    assert self.ledger.execute(request, repair=self.args.reconcile) == outcome
                    assert packed(self.state) == committed_state, 'duplicate account execution changed state'
                self.crash('after', kind)
                if self.args.mode == 'unknown-settlement' and kind == 'settlement':
                    outcome = {'kind': 'unknown'}
                receipt = {'key': output['key'], 'operation_id': request['operation_id'],
                           'attempt_id': request['attempt_id'], 'outcome': outcome}
                self.rpc('submit_business_receipt', receipt)
                # A fenced negative result may advance the attempt immediately; its older
                # attempt is then stale. Stable terminal proofs remain repeatable.
                if outcome['kind'] != 'not_committed':
                    self.rpc('submit_business_receipt', receipt)
                if request['body']['kind'] == 'reconcile_operation' and outcome['kind'] == 'unknown':
                    self.close()
                    self.state['needs_reconciliation'] = True
                    self.ledger.save()
            elif body['kind'] == 'publication':
                # Delivery to this process's local scripted player precedes Sent.
                self.snapshot = body['snapshot']
                try:
                    self.rpc('submit_transport_receipt', {'key': output['key'], 'output_id': output['id'],
                        'recipient_id': body['recipient_id'], 'connection': body['connection'], 'outcome': 'sent'})
                except RuntimeError as error:
                    if str(error) != 'CLOSED':
                        raise
                if (self.args.mode != 'close-before-ready' and not self.ready_sent
                        and self.snapshot['phase'] == 'deploying' and self.snapshot['protected']):
                    self.action({'kind': 'combat_ready', 'publication_id': output['id']})
                    self.ready_sent = True
            elif body['kind'] == 'event' and body['event']['kind'] == 'fault':
                raise RuntimeError(body['event'])

    def until(self, condition):
        deadline = time.monotonic() + 25
        while not condition():
            if time.monotonic() > deadline:
                raise TimeoutError('host deadline; unresolved responsibilities remain in SQLite')
            self.pump()
            time.sleep(.003)

    def pending(self):
        checkpoint = self.state.get('checkpoint')
        return [op['request']['operation_id'] for op in checkpoint['state']['operations']
                if op['terminal'] is None] if checkpoint else []

    def run(self, recovering):
        request = dict(self.state['create'])
        if recovering:
            assert self.state.get('checkpoint'), 'No persisted checkpoint; investigate account journal before retry'
            self.state['generation'] = max(self.state['generation'], self.state['checkpoint']['state']['recovery_generation']) + 1
            self.state['needs_reconciliation'] = False
            self.ledger.save()
            request['recovery'] = {'generation': self.state['generation'], 'checkpoint': self.state['checkpoint']}
        self.rpc('create', request)
        self.rpc('create', request)
        if not recovering:
            if self.args.mode != 'admission-refused':
                self.until(lambda: self.phase == 'deploying')
                self.action({'kind': 'join'})
                if self.args.mode == 'close-before-ready':
                    self.until(lambda: self.snapshot is not None)
                    self.close()
                else:
                    self.until(lambda: self.snapshot and self.snapshot['phase'] == 'active')
                    self.action({'kind': 'move', 'direction': [1, 0]})
                    self.until(lambda: self.snapshot['position'][0] > 0)
                    self.action({'kind': 'stop'})
                    self.action({'kind': 'attack'})
                    self.until(lambda: any(e['kind'] == 'damage' and e['source'] == 'participant' for e in self.state['events'].values()))
                    skill = json.loads(self.state['config_raw'])['skill']
                    self.action({'kind': 'cast', 'skill_id': skill['id'], 'direction': [1, 0]})
                    self.until(lambda: any(e['kind'] == 'target_killed' for e in self.state['events'].values()))
                    while self.snapshot['resources']:
                        resource = self.snapshot['resources'][0]['id']
                        self.action({'kind': 'pickup', 'resource_id': resource, 'expected_revision': self.snapshot['inventory_revision']})
                        self.until(lambda: all(row['id'] != resource for row in self.snapshot['resources']))
                    self.crash('after', 'active')
                    if self.args.mode == 'extraction':
                        self.until(lambda: self.snapshot['extraction_open'])
                        self.action({'kind': 'extract_start'})
                    elif self.args.mode == 'active-close':
                        self.close()
                    else:
                        self.action({'kind': 'leave'})
        # close/create may have emitted new responsibilities before the stored checkpoint changes.
        self.pump()
        self.until(lambda: self.phase == 'closed' and (not self.pending() or self.state['needs_reconciliation']))
        self.pump()
        return {'phase': self.phase, 'balances': self.state['balances'], 'revision': self.state['revision'],
                'counts': self.state['counts'], 'pending': self.pending(), 'generation': self.state['generation'],
                'events': list(self.state['events'].values()), 'requests': self.state['requests'],
                'configHash': self.state['create']['config_hash'], 'liveWorlds': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--configs', required=True)
    parser.add_argument('--account', required=True)
    parser.add_argument('--state', required=True)
    parser.add_argument('--verification', action='store_true')
    parser.add_argument('--reconcile', action='store_true', help='authorize local correction of the original frozen transaction')
    parser.add_argument('--mode', default='death', choices=['death', 'extraction', 'active-close', 'unknown-settlement', 'retry-settlement',
                        'refused-settlement', 'close-before-ready', 'admission-refused'])
    parser.add_argument('--crash-before', default='')
    parser.add_argument('--crash-after', default='')
    args = parser.parse_args()
    ledger = Ledger(args.state)
    host = None
    try:
        recovering = ledger.state is not None
        if not recovering:
            catalog = json.loads(subprocess.check_output([args.binary, '--configs', args.configs, '--catalog'], text=True))
            selected = next(p for p in catalog['active'] if p['scenario'] == catalog['defaultScenario'])
            version = next(p for p in catalog['versions'] if p['id'] == selected['id'] and p['hash'] == selected['hash'])
            assert version['protocolVersion'] in [2,3], 'asset host requires asset or result configuration'
            account = json.loads(Path(args.account).read_text())
            create = {'protocol_version': version['protocolVersion'], 'key': {'id': uuid.uuid4().hex, 'incarnation': 1},
                      'request_id': 'create', 'config_id': selected['id'], 'config_hash': selected['hash'],
                      'seed': account['seed'], 'participant_id': account['participant_id'],
                      'authenticated_principal': account['principal'], 'connection': {'id': 'local-player', 'generation': 1},
                      'publication_mode': 'batched'}
            ledger.state = {'create': create, 'config_raw': version['raw'], 'initial': account,
                            'balances': account['assets'].copy(), 'revision': account['revision'],
                            'counts': {'admission': 0, 'settlement': 0, 'compensation': 0},
                            'requests': {}, 'journal': {}, 'events': {}, 'generation': 0, 'needs_reconciliation': False}
            ledger.save()
        host = Host(args, ledger)
        print(packed(host.run(recovering)), flush=True)
    finally:
        if host:
            host.worker.stdin.close()
            host.worker.wait(timeout=8)
        ledger.db.close()


if __name__ == '__main__':
    main()
