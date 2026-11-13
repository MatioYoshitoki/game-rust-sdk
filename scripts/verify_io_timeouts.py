"""B2-2: silent/delayed I/O through the public H5 and real, unaccelerated Rust I/O deadlines."""
import json
import re
import subprocess
import time
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_h5 import active, closed, enter, info, wait
from verify_reconnect import hashes

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/io-timeouts'


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    rust = subprocess.run(['cargo', 'test', '--locked', '--all-features', 'verification::timeouts'], cwd=ROOT,
                          text=True, capture_output=True, check=True)
    rust_tests = re.findall(r'^test (verification::timeouts::\S+) \.\.\. ok$', rust.stdout, re.M)
    assert len(rust_tests) == 16, rust.stdout
    subprocess.run(['cargo', 'build', '--locked', '--features', 'replay', '--bin', 'kernel-host'], cwd=ROOT, check=True)
    server = subprocess.Popen(['node', 'adapters/h5/server.mjs', '--port', '0'], cwd=ROOT,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    url = json.loads(server.stdout.readline())['url']
    results, errors = {}, []
    page = None
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(headless=True)
            context = browser.new_context(viewport={'width': 1440, 'height': 1100})
            page = context.new_page(); page.on('pageerror', lambda error: errors.append(str(error))); page.goto(url)

            def current():
                return info(page, url)

            def select(fault, clock='live'):
                page.locator('#fault').select_option(fault); page.locator('#clock').select_option(clock)

            def create(fault, clock='live'):
                select(fault, clock); previous = current(); started = time.monotonic()
                page.locator('#start').click()
                value = wait(lambda: data if (data := current()) and (not previous or data['id'] != previous['id']) else None)
                # The API may expose the new session while its Rust process is still starting.
                # Wait for this match's view before testing phase/pending, which may describe the old result.
                page.wait_for_function("prefix => document.querySelector('#match-id').textContent.includes(prefix)", arg=value['id'][:8])
                return value, started

            def has_event(kind, value=None):
                return any(event['kind'] == kind for event in (value or current())['events'])

            def record(name, started=None):
                value = current()
                assert value['phase'] == 'closed' and value['ledger']['pending'] == [], value
                assert not errors, errors
                results[name] = {'ledger': value['ledger'], 'events': value['events'], 'io': value['io'],
                                 'logicalMs': value['logicalMs'], 'wallElapsedMs': round((time.monotonic() - started) * 1000) if started else None}
                print(f'{name}: passed', flush=True)

            _, started = create('load_silent')
            closed(page)
            value = current()
            assert time.monotonic() - started >= 4.8
            assert value['ledger']['warehouse'] == 100 and value['ledger']['admissions'] == 0
            assert has_event('io_timed_out') and not has_event('activated')
            record('silent_load', started)

            before, started = create('load_delayed', 'fast')
            page.wait_for_timeout(100); page.reload(); active(page)
            value = current()
            assert value['id'] == before['id'] and value['connection']['generation'] == 2
            assert value['ledger']['admissions'] == 1 and not has_event('io_timed_out')
            page.locator('#close').click(); closed(page)
            record('delayed_loading_refresh', started)

            _, started = create('admission_silent')
            page.locator('#reconcile').wait_for(state='visible', timeout=12000)
            value = current()
            assert value['phase'] == 'closed' and value['ledger']['warehouse'] == 100 and value['ledger']['admissions'] == 0
            assert not has_event('activated') and page.locator('#world-live').inner_text() == '0'
            page.locator('#reconcile').click(); closed(page)
            assert (current()['ledger']['warehouse'], current()['ledger']['admissions'], current()['ledger']['compensations']) == (100, 1, 1)
            record('silent_admission_reconciled_and_compensated', started)

            _, started = create('admission_lost_receipt')
            closed(page)
            value = current()
            assert (value['ledger']['warehouse'], value['ledger']['admissions'], value['ledger']['compensations'], value['ledger']['entryYears']) == (100, 1, 1, 0)
            assert not has_event('activated') and has_event('io_timed_out')
            record('lost_admission_receipt', started)

            _, started = create('compensation_silent')
            page.locator('#reconcile').wait_for(state='visible', timeout=16000)
            value = current()
            assert value['phase'] == 'closed' and value['ledger']['warehouse'] == 95
            assert sum(event['kind'] == 'io_timed_out' for event in value['events']) == 2
            page.locator('#reconcile').click(); closed(page)
            assert current()['ledger']['warehouse'] == 100 and current()['ledger']['entryYears'] == 0
            record('silent_compensation_after_close', started)

            def fight_and_leave(fault):
                select(fault); enter(page, url)
                page.locator('#attack').click()
                wait(lambda: current()['snapshot']['target']['hp'] == 100)
                page.locator('#cast').click(); wait(lambda: current()['snapshot']['target'] is None)
                for remaining in [1, 0]:
                    page.locator('#pickup').click(); wait(lambda: len(current()['snapshot']['resources']) == remaining)
                assert current()['snapshot']['inventory_quantity'] == 7
                started = time.monotonic(); page.locator('#leave').click()
                value = wait(lambda: data if (data := current())['phase'] == 'draining' else None)
                return started, value['logicalMs']

            started, frozen = fight_and_leave('settlement_silent')
            page.locator('#reconcile').wait_for(state='visible', timeout=12000)
            value = current()
            assert value['phase'] == 'closed' and value['logicalMs'] == frozen
            assert value['ledger']['warehouse'] == 95 and value['ledger']['settlements'] == 0
            assert not has_event('settlement_applied')
            blocked = page.request.post(url + '/api/sessions', data={'clock': 'live', 'previousConnection': value['connection']})
            assert blocked.status == 409 and blocked.json()['error'] == 'UNRESOLVED_IO'
            assert current()['id'] == value['id']
            page.wait_for_function("document.querySelector('#phase').dataset.phase === 'closed' && document.querySelector('#world-live').textContent === '0'")
            page.screenshot(path=str(OUTPUT / 'closed-pending.png'), full_page=True)
            page.locator('#reconcile').click(); closed(page)
            assert (current()['ledger']['warehouse'], current()['ledger']['cultivation'], current()['ledger']['settlements']) == (99, 3, 1)
            record('silent_settlement_preserves_frozen_obligation', started)

            started, frozen = fight_and_leave('settlement_lost_receipt')
            closed(page); value = current()
            assert value['logicalMs'] == frozen and has_event('io_timed_out')
            assert (value['ledger']['warehouse'], value['ledger']['cultivation'], value['ledger']['settlements'], value['ledger']['reconciliations']) == (99, 3, 1, 0)
            record('lost_settlement_receipt_queries_commit', started)

            for manual in [False, True]:
                started, frozen = fight_and_leave('settlement_delayed')
                page.locator('#reconcile').wait_for(state='visible', timeout=12000)
                value = current()
                assert value['phase'] == 'closed' and value['ledger']['settlements'] == 0 and value['logicalMs'] == frozen
                if manual:
                    page.locator('#reconcile').click()
                closed(page)
                wait(lambda: current()['io']['delayed'] == 0)
                value = current()
                assert (value['ledger']['warehouse'], value['ledger']['cultivation'], value['ledger']['settlements']) == (99, 3, 1)
                assert value['ledger']['reconciliations'] == int(manual)
                assert sum(event['kind'] == 'settlement_applied' for event in value['events']) == 1
                assert sum(item['action'] == 'late_execution' for item in value['io']['audit']) == 1
                record('late_settlement_after_manual_proof' if manual else 'late_settlement_proof', started)

            started, frozen = fight_and_leave('query_silent')
            value = wait(lambda: data if (data := current())['phase'] == 'closed' else None)
            assert value['ledger']['manual'] == 0 and value['ledger']['settlements'] == 0
            page.locator('#reconcile').wait_for(state='visible', timeout=12000)
            value = current()
            assert value['logicalMs'] == frozen and sum(event['kind'] == 'io_timed_out' for event in value['events']) == 2
            assert all(request['attempt_id'] == 1 for request in value['io']['requests'])
            page.locator('#reconcile').click(); closed(page)
            assert current()['ledger']['settlements'] == 1
            record('silent_query_escalates_without_retrying_write', started)

            started, frozen = fight_and_leave('refused')
            page.locator('#reconcile').wait_for(state='visible')
            assert current()['phase'] == 'draining'
            value = wait(lambda: data if (data := current())['phase'] == 'closed' else None, timeout=20)
            assert value['logicalMs'] == frozen and value['ledger']['manual'] == 1
            assert value['ledger']['settlements'] == 0 and has_event('io_drain_expired')
            assert time.monotonic() - started >= 14.8
            page.locator('#reconcile').click(); closed(page)
            record('manual_work_cannot_hold_world_forever', started)

            previous = current(); page.goto('about:blank')
            created = page.request.post(url + '/api/sessions', data={
                'clock': 'fast', 'fault': 'none', 'previousConnection': previous['connection']})
            assert created.status == 201, created.text()
            orphan = created.json(); started = time.monotonic()
            value = wait(lambda: data if (data := current())['phase'] == 'closed' else None)
            assert value['id'] == orphan['id'] and not has_event('activated', value)
            assert (value['ledger']['warehouse'], value['ledger']['admissions'], value['ledger']['compensations'], value['ledger']['entryYears']) == (100, 1, 1, 0)
            page.goto(url); closed(page)
            record('missing_client_ready_compensates_entry', started)
            context.close(); browser.close(); page = None
        assert sources == hashes(), 'Sources changed during I/O verification'
    except Exception:
        if page is not None:
            try:
                page.screenshot(path=str(OUTPUT / 'failure.png'), full_page=True)
                (OUTPUT / 'failure.json').write_text(json.dumps(info(page, url), ensure_ascii=False, indent=2) + '\n')
            except Exception:
                pass
        raise
    finally:
        server.terminate()
        try:
            server.wait(timeout=8)
        except subprocess.TimeoutExpired:
            server.kill(); server.wait()
    report = {'scope': 'B2-2 real Rust monotonic I/O timeouts with controlled storage', 'rustTests': rust_tests,
              'results': results, 'pageErrors': errors, 'sourceSha256': sources}
    (OUTPUT / 'verification.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'status': 'passed', 'rustTests': len(rust_tests), 'browserCases': len(results), 'pageErrors': len(errors)}), flush=True)


if __name__ == '__main__':
    main()
