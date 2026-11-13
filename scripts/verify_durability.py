"""B2-3: kill the real Node host at durable boundaries and recover through public H5 APIs."""
from contextlib import closing
import hashlib
import json
import re
import selectors
import sqlite3
import subprocess
import tempfile
import time
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_h5 import active, closed, enter, info, wait
from verify_reconnect import hashes

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/durability'


class Host:
    def __init__(self, directory, manifest=None):
        self.path = Path(directory) / 'lab.sqlite'
        self.port = 0
        self.process = None
        self.manifest = manifest
        self.error_log = tempfile.TemporaryFile(mode='w+t')

    def start(self, point='', kind='', expect_crash=False):
        command = ['node', 'adapters/h5/server.mjs', '--port', str(self.port), '--state', str(self.path)]
        if self.manifest:
            command += ['--configs', str(self.manifest)]
        if point:
            command += ['--crash-at', point, '--crash-kind', kind]
        self.process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=self.error_log, text=True)
        selector = selectors.DefaultSelector(); selector.register(self.process.stdout, selectors.EVENT_READ)
        try:
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                if selector.select(.1):
                    line = self.process.stdout.readline()
                    if line:
                        record = json.loads(line)
                        if 'url' in record:
                            assert not expect_crash, record
                            self.url = record['url']; self.port = int(self.url.rsplit(':', 1)[1])
                            return
                if self.process.poll() is not None:
                    if expect_crash:
                        assert self.process.returncode == -9
                        return
                    self.error_log.seek(0)
                    raise AssertionError(self.error_log.read())
            raise AssertionError('Host startup timed out')
        finally:
            selector.close()

    def killed(self):
        assert self.process.wait(timeout=18) == -9

    def kill(self):
        if self.process and self.process.poll() is None:
            self.process.kill(); self.killed()

    def stop(self):
        if self.process and self.process.poll() is None:
            self.process.terminate(); self.process.wait(timeout=8)

    def state(self):
        assert self.process.poll() is not None
        with closing(sqlite3.connect(self.path)) as database:
            row = database.execute('SELECT state FROM stores ORDER BY rowid DESC LIMIT 1').fetchone()
            return json.loads(row[0])


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    rust = subprocess.run(['cargo', 'test', '--locked', '--all-features', 'verification::durability'],
                          cwd=ROOT, text=True, capture_output=True, check=True)
    rust_tests = re.findall(r'^test (verification::durability::\S+) \.\.\. ok$', rust.stdout, re.M)
    assert len(rust_tests) == 9, rust.stdout
    storage = subprocess.run(['node', '--test', 'adapters/h5/persistence.test.mjs'], cwd=ROOT,
                             capture_output=True, text=True, check=True)
    assert '# pass 6' in storage.stdout, storage.stdout
    subprocess.run(['cargo', 'build', '--locked', '--features', 'replay', '--bin', 'kernel-host'], cwd=ROOT, check=True)
    results = {}
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)

        def scenario(name, work, point='', kind=''):
            with tempfile.TemporaryDirectory(prefix='rust-durability-') as directory:
                host = Host(directory); context = browser.new_context(viewport={'width': 1440, 'height': 1100})
                page = context.new_page()
                try:
                    host.start(point, kind); page.goto(host.url)
                    work(host, page, context)
                    value = info(page, host.url)
                    results[name] = {'id': value['id'], 'ledger': value['ledger'], 'recovered': value['recovered'],
                                     'events': value['events'], 'requests': value['io']['requests'],
                                     'resultTitle': page.locator('#result-title').inner_text()}
                    assert value['phase'] == 'closed', value
                    assert page.locator('#world-live').inner_text() == '0'
                    print(f'{name}: passed', flush=True)
                except Exception:
                    page.screenshot(path=str(OUTPUT / 'failure.png'), full_page=True)
                    host.error_log.seek(0); print(host.error_log.read(), flush=True)
                    raise
                finally:
                    context.close(); host.stop(); host.error_log.close()

        def reload(host, page):
            host.start(); page.goto(host.url); closed(page)
            return info(page, host.url)

        def balances(value, warehouse, admissions, settlements=0, compensations=0, growth=0, manual=0):
            ledger = value['ledger']
            expected = (warehouse, admissions, settlements, compensations, growth, manual)
            actual = tuple(ledger[key] for key in ['warehouse', 'admissions', 'settlements', 'compensations', 'cultivation', 'manual'])
            assert actual == expected, (actual, expected, value)
            assert ledger['entryYears'] == admissions - compensations, ledger
            if not manual:
                assert ledger['pending'] == [], ledger

        # A stored intent must precede execution. Pre-commit death rolls back both money and proof.
        for point in ['after_intent_persist', 'before_storage_commit', 'after_storage_commit', 'after_kernel_receipt']:
            def admission_crash(host, page, context, point=point):
                page.locator('#start').click(); host.killed(); page.goto('about:blank')
                before = host.state()
                committed = point in ['after_storage_commit', 'after_kernel_receipt']
                assert (before['warehouse'], before['admissions']) == ((95, 1) if committed else (100, 0))
                value = reload(host, page); balances(value, 100, int(committed), compensations=int(committed))
                host.kill(); page.goto('about:blank')
                again = reload(host, page); balances(again, 100, int(committed), compensations=int(committed))
            scenario('admission_' + point, admission_crash, point, 'commit_admission')

        def interrupted(host, page, context):
            enter(page, host.url); before = info(page, host.url); host.kill()
            host.start()
            # The still-open H5 must distinguish a new process from an ordinary stale tab.
            page.wait_for_function("document.querySelector('#result-title').textContent === '对局中断，入场已退还'", timeout=15000)
            value = info(page, host.url); balances(value, 100, 1, compensations=1)
            assert value['id'] == before['id'] and value['connection']['generation'] > before['connection']['generation']
            stale = page.request.post(host.url + f"/api/sessions/{value['id']}/close", data={'connection': before['connection']})
            assert stale.status == 409 and stale.json()['error'] == 'STALE_CONNECTION'
            outsider = browser.new_context()
            try:
                other = outsider.new_page(); other.goto(host.url)
                assert info(other, host.url) is None
                assert other.request.get(host.url + f"/api/sessions/{value['id']}").status == 404
            finally:
                outsider.close()
            page.screenshot(path=str(OUTPUT / 'interrupted-refunded.png'), full_page=True)
        scenario('active_interrupt_refund_and_connection_isolation', interrupted)

        # Each point below is a SIGKILL, not the server's graceful close path.
        for point in ['after_drain_before_persist', 'after_intent_persist', 'before_storage_commit',
                      'after_storage_commit', 'after_kernel_receipt', 'after_result_persist']:
            def settlement_crash(host, page, context, point=point):
                page.locator('#verify').click(); host.killed(); page.goto('about:blank')
                before = host.state()
                original = next((request for _, request in before['requests'] if request['body']['kind'] == 'commit_settlement'), None)
                value = reload(host, page)
                if point == 'after_drain_before_persist':
                    assert original is None
                    balances(value, 100, 1, compensations=1)
                else:
                    assert original is not None
                    balances(value, 102, 1, settlements=1, growth=30)
                    restored = next(request for request in value['io']['requests'] if request['body']['kind'] == 'commit_settlement')
                    assert restored['operation_id'] == original['operation_id'] and restored['body'] == original['body']
                    assert len([event for event in value['events'] if event['kind'] == 'settlement_applied']) == 1
                    assert page.locator('#result-title').inner_text() == '撤离成功'
                for _ in range(2):
                    prior = value['ledger']; host.kill(); page.goto('about:blank'); value = reload(host, page)
                    assert value['ledger'] == prior
                if point == 'after_result_persist':
                    assert page.locator('#progress-count').inner_text() == '5 / 5'
                    page.screenshot(path=str(OUTPUT / 'restored-result.png'), full_page=True)
            scenario('settlement_' + point, settlement_crash, point, 'commit_settlement')

        def compensate_crash(host, page, context):
            enter(page, host.url); host.kill(); page.goto('about:blank')
            host.start('after_storage_commit', 'compensate_admission', expect_crash=True)
            assert (host.state()['warehouse'], host.state()['compensations']) == (100, 1)
            value = reload(host, page); balances(value, 100, 1, compensations=1)
        scenario('compensation_commit_without_receipt', compensate_crash)

        def manual_pending(host, page, context):
            page.locator('#fault').select_option('refused'); page.locator('#verify').click()
            page.locator('#reconcile').wait_for(state='visible', timeout=30000)
            before = info(page, host.url); host.kill(); page.goto('about:blank'); host.start(); page.goto(host.url)
            page.locator('#reconcile').wait_for(state='visible'); value = info(page, host.url)
            balances(value, 95, 1, manual=1)
            assert sorted(value['ledger']['pending']) == sorted(before['ledger']['pending'])
            assert not value['settlement'] and page.locator('#world-live').inner_text() == '0'
            denied = page.request.post(host.url + '/api/sessions', data={'previousConnection': value['connection']})
            assert denied.status == 409 and denied.json()['error'] == 'UNRESOLVED_IO'
            page.screenshot(path=str(OUTPUT / 'restored-pending.png'), full_page=True)
            page.locator('#reconcile').click(); closed(page)
            balances(info(page, host.url), 102, 1, settlements=1, growth=30)
        scenario('manual_responsibility_survives_restart', manual_pending)

        for point in ['before_storage_commit', 'after_storage_commit', 'after_kernel_receipt']:
            def manual_crash(host, page, context, point=point):
                page.locator('#fault').select_option('refused'); page.locator('#verify').click()
                page.locator('#reconcile').wait_for(state='visible', timeout=30000)
                page.locator('#reconcile').click(); host.killed(); page.goto('about:blank'); host.start(); page.goto(host.url)
                if point == 'before_storage_commit':
                    page.locator('#reconcile').wait_for(state='visible'); balances(info(page, host.url), 95, 1, manual=1)
                    page.locator('#reconcile').click()
                closed(page); value = info(page, host.url); balances(value, 102, 1, settlements=1, growth=30)
                assert value['ledger']['reconciliations'] == 1
                host.kill(); page.goto('about:blank'); value = reload(host, page)
                balances(value, 102, 1, settlements=1, growth=30)
                assert value['ledger']['reconciliations'] == 1
            scenario('manual_' + point, manual_crash, point, 'reconcile_operation')

        # Fail startup before exposing HTTP if the checkpoint/config identity or money ledger is damaged.
        for damage in ['checkpoint', 'store', 'config']:
            def corrupt(host, page, context, damage=damage):
                enter(page, host.url); host.kill(); page.goto('about:blank')
                original = host.state()
                with closing(sqlite3.connect(host.path)) as database:
                    if damage == 'config':
                        key, text = database.execute('SELECT id,original_create FROM sessions').fetchone()
                        create = json.loads(text); create['config_hash'] = '0' * 64
                        database.execute('UPDATE sessions SET original_create=? WHERE id=?', (json.dumps(create), key))
                    else:
                        table, field = ('sessions', 'checkpoint_digest') if damage == 'checkpoint' else ('stores', 'digest')
                        database.execute(f'UPDATE {table} SET {field}=?', ('damaged',))
                    database.commit()
                failed = subprocess.run(['node', 'adapters/h5/server.mjs', '--port', '0', '--state', str(host.path)],
                                        cwd=ROOT, text=True, capture_output=True, timeout=12)
                assert failed.returncode != 0 and '"url"' not in failed.stdout
                assert host.state() == original
                # Restore only the QA damage, then verify funds are recovered from the original operation.
                with closing(sqlite3.connect(host.path)) as database:
                    if damage == 'config':
                        database.execute('UPDATE sessions SET original_create=? WHERE id=?', (text, key))
                    else:
                        column = 'checkpoint' if damage == 'checkpoint' else 'state'
                        value = database.execute(f'SELECT {column} FROM {table}').fetchone()[0]
                        database.execute(f'UPDATE {table} SET {field}=?', (hashlib.sha256(value.encode()).hexdigest(),))
                    database.commit()
                balances(reload(host, page), 100, 1, compensations=1)
            scenario('fail_closed_' + damage, corrupt)
        browser.close()
    assert sources == hashes(), 'Sources changed during durability verification'
    report = {'batch': 'B2-3', 'scope': 'local durable I/O only; no World restoration', 'rustTests': rust_tests,
              'storageTests': 6, 'killAndRestartChecks': len(results), 'checks': results, 'sourceHashes': sources}
    (OUTPUT / 'verification.json').write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps({'passed': len(results), 'evidence': str(OUTPUT / 'verification.json')}), flush=True)


if __name__ == '__main__':
    main()
