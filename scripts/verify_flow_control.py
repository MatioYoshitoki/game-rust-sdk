"""B2-4: real Rust/HTTP concurrency, slow async provider isolation and invalid/late proof handling."""
import json
import re
import subprocess
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_h5 import active, closed, enter, info, wait
from verify_reconnect import hashes

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/flow-control'


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True); sources = hashes()
    rust = subprocess.run(['cargo', 'test', '--locked', '--all-features', 'verification::concurrency'], cwd=ROOT,
                          capture_output=True, text=True, check=True)
    rust_tests = re.findall(r'^test (verification::concurrency::\S+) \.\.\. ok$', rust.stdout, re.M)
    assert len(rust_tests) == 5
    unit = subprocess.run(['node', '--test', 'adapters/h5/flow-control.test.mjs'], cwd=ROOT,
                          capture_output=True, text=True, check=True)
    assert '# pass 7' in unit.stdout
    subprocess.run(['cargo', 'build', '--locked', '--features', 'replay', '--bin', 'kernel-host'], cwd=ROOT, check=True)
    results, errors = {}, []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)

        def scenario(name, mode, work):
            server = subprocess.Popen(['node', 'adapters/h5/verify-flow-host.mjs', mode], cwd=ROOT,
                                      stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            url = json.loads(server.stdout.readline())['url']
            context = browser.new_context(viewport={'width': 1440, 'height': 1100})
            page = context.new_page(); page.on('pageerror', lambda error: errors.append(str(error))); page.goto(url)
            try:
                work(page, url, server)
                value = info(page, url)
                assert value['phase'] == 'closed' and value['ledger']['pending'] == [], value
                assert value['host']['queue']['peak'] <= 64 and value['host']['io']['peak'] <= 32
                assert page.locator('#world-live').inner_text() == '0'
                assert not errors, errors
                results[name] = {'ledger': value['ledger'], 'host': value['host'], 'io': value['io'], 'events': value['events']}
                print(name + ': passed', flush=True)
            except Exception:
                page.screenshot(path=str(OUTPUT / 'failure.png'), full_page=True)
                raise
            finally:
                context.close(); server.terminate()
                try: server.wait(timeout=10)
                except subprocess.TimeoutExpired: server.kill(); server.wait()
                stderr = server.stderr.read()
                assert server.returncode == 0, stderr

        def fight(page, url):
            enter(page, url); page.locator('#attack').click()
            wait(lambda: info(page, url)['snapshot']['target']['hp'] == 100)
            page.locator('#cast').click(); wait(lambda: info(page, url)['snapshot']['target'] is None)
            for remaining in [1, 0]:
                page.locator('#pickup').click(); wait(lambda: len(info(page, url)['snapshot']['resources']) == remaining)

        def duplicate_pickup(page, url, server):
            enter(page, url); page.locator('#attack').click()
            wait(lambda: info(page, url)['snapshot']['target']['hp'] == 100)
            page.locator('#cast').click(); wait(lambda: info(page, url)['snapshot']['target'] is None)
            current = info(page, url)
            command = {'connection': current['connection'], 'sequence': current['sequence'] + 1, 'requestId': 'concurrent-pickup',
                       'action': {'kind': 'pickup', 'resource_id': 'drop-1', 'expected_revision': current['snapshot']['inventory_revision']}}
            replies = page.evaluate("""async ({id,command}) => Promise.all(Array.from({length:96}, async () => {
              const response = await fetch(`/api/sessions/${id}/input`, {method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(command)});
              return {status:response.status,body:await response.json()};
            }))""", {'id': current['id'], 'command': command})
            assert any(reply['status'] == 200 for reply in replies)
            assert all(reply['status'] == 200 or (reply['status'] == 429 and reply['body']['error'] == 'QUEUE_FULL') for reply in replies), replies
            current = info(page, url)
            assert current['snapshot']['inventory_quantity'] == 7
            assert len([event for event in current['events'] if event['kind'] == 'picked_up' and event['resource_id'] == 'drop-1']) == 1
            response = page.request.post(url + f"/api/sessions/{current['id']}/close", data={'connection': current['connection']})
            assert response.ok; closed(page)
            assert info(page, url)['ledger']['warehouse'] == 99
        scenario('concurrent_pickup_is_idempotent_and_bounded', 'normal', duplicate_pickup)

        def create_race(page, url, server):
            replies = page.evaluate("""async () => Promise.all(Array.from({length:16}, async () => {
              const response=await fetch('/api/sessions',{method:'POST',headers:{'Content-Type':'application/json'},body:'{"clock":"live"}'});
              return {status:response.status,body:await response.json()};
            }))""")
            assert sum(reply['status'] == 201 for reply in replies) == 1, replies
            assert all(reply['status'] in [201, 409] or (reply['status'] == 400 and reply['body']['error'] == 'INVALID_MESSAGE') for reply in replies), replies
            page.reload(); active(page); current = info(page, url)
            assert current['ledger']['admissions'] == 1
            page.locator('#close').click(); closed(page)
        scenario('same_owner_concurrent_create_has_one_winner', 'normal', create_race)

        def held_settlement(page, url, server):
            fight(page, url); page.locator('#leave').click()
            wait(lambda: info(page, url)['phase'] == 'draining')
            other = browser.new_context(); healthy = other.new_page(); healthy.goto(url)
            try:
                enter(healthy, url); before = info(healthy, url)['logicalMs']; healthy.wait_for_timeout(100)
                assert info(healthy, url)['logicalMs'] > before
                healthy.locator('#close').click(); closed(healthy)
                assert info(healthy, url)['ledger']['settlements'] == 1
            finally: other.close()
            page.locator('#reconcile').wait_for(state='visible', timeout=12000)
            value = info(page, url)
            assert value['phase'] == 'closed' and value['ledger']['warehouse'] == 95
            assert value['host']['io']['active'] == 1  # timeout has not released the outstanding write slot
            page.locator('#reconcile').click(); closed(page)
            server.stdin.write('release\n'); server.stdin.flush()
            wait(lambda: (value := info(page, url))['host']['io']['active'] == 0 and value['host']['receipts'] == 0)
            value = info(page, url)
            assert (value['ledger']['warehouse'], value['ledger']['settlements'], value['ledger']['reconciliations']) == (99, 1, 1)
            assert len([event for event in value['events'] if event['kind'] == 'settlement_applied']) == 1
        scenario('hung_write_does_not_block_other_match_or_close_and_late_commit_deduplicates', 'hold_settlement', held_settlement)

        def held_load(page, url, server):
            page.locator('#start').click(); value = wait(lambda: info(page, url))
            response = page.request.post(url + f"/api/sessions/{value['id']}/close", data={'connection': value['connection']})
            assert response.ok and response.json()['phase'] == 'closed'
            server.stdin.write('release\n'); server.stdin.flush(); closed(page)
            assert info(page, url)['ledger']['admissions'] == 0
        scenario('hung_load_can_close_before_provider_finishes', 'hold_load', held_load)

        def uncertain_commit(page, url, server):
            fight(page, url); page.locator('#leave').click(); closed(page)
            value = info(page, url)
            assert value['ledger']['warehouse'] == 99 and value['ledger']['settlements'] == 1
            assert any(request['body']['kind'] == 'query_operation' for request in value['io']['requests'])
        scenario('transport_error_after_commit_queries_without_rewriting', 'throw_after_commit', uncertain_commit)

        def invalid_proof(page, url, server):
            fight(page, url); page.locator('#leave').click()
            page.locator('#reconcile').wait_for(state='visible', timeout=12000)
            value = info(page, url)
            assert value['phase'] == 'closed' and value['ledger']['warehouse'] == 95 and not value['settlement']
            assert any(item['action'] == 'rejected_late_or_invalid_proof' for item in value['io']['audit'])
            page.locator('#reconcile').click(); closed(page)
            assert info(page, url)['ledger']['settlements'] == 1
        scenario('invalid_remote_commit_proof_cannot_complete_assets', 'invalid_proof', invalid_proof)

        def admission_unknown(page, url, server):
            page.locator('#start').click(); page.locator('#reconcile').wait_for(state='visible', timeout=12000)
            value = info(page, url); assert value['ledger']['warehouse'] == 100 and value['ledger']['manual'] == 1
            page.locator('#reconcile').click(); closed(page)
            value = info(page, url)
            assert (value['ledger']['warehouse'], value['ledger']['admissions'], value['ledger']['compensations']) == (100, 1, 1)
        scenario('transport_error_before_commit_is_still_unknown_until_proven', 'throw_before_admission', admission_unknown)

        def slow_client(page, url, server):
            enter(page, url); before = info(page, url)
            page.route('**/ack', lambda route: route.fulfill(status=200, json={'ignored': True}))
            value = wait(lambda: current if (current := info(page, url))['connection']['generation'] > before['connection']['generation'] else None, timeout=12)
            assert value['ledger']['admissions'] == 1
            page.request.post(url + f"/api/sessions/{value['id']}/close", data={'connection': value['connection']})
            closed(page); assert info(page, url)['ledger']['settlements'] == 1
        scenario('client_without_ack_is_detached_without_duplicate_admission', 'normal', slow_client)
        browser.close()
    assert sources == hashes(), 'Sources changed during flow-control verification'
    report = {'batch': 'B2-4', 'rustTests': rust_tests, 'queueAndIoTests': 7, 'results': results,
              'pageErrors': errors, 'sourceSha256': sources}
    (OUTPUT / 'verification.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'passed': len(results), 'rustTests': len(rust_tests), 'queueAndIoTests': 7}), flush=True)


if __name__ == '__main__':
    main()
