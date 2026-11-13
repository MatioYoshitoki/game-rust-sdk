"""B2-1 browser acceptance through authenticated public HTTP/SSE and a real Rust host."""
import hashlib
import json
import re
import subprocess
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_h5 import active, closed, enter, info, wait

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/reconnect'


def hashes():
    paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'package.json', ROOT / 'config/b1.json', ROOT / 'config/b3-1.json',
             *ROOT.glob('config/**/*.json'), *ROOT.glob('src/**/*.rs'), *ROOT.glob('src/**/*.json'), *ROOT.glob('web/*'), *ROOT.glob('adapters/h5/*.mjs'),
             ROOT / 'adapters/node/controlled-store.mjs', *ROOT.glob('scripts/*.py')]
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(paths)}


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    rust = subprocess.run(['cargo', 'test', '--locked', '--all-features', 'verification::reconnect'], cwd=ROOT,
                          text=True, capture_output=True, check=True)
    rust_tests = re.findall(r'^test (verification::reconnect::\S+) \.\.\. ok$', rust.stdout, re.M)
    assert len(rust_tests) == 8, rust.stdout
    subprocess.run(['cargo', 'build', '--locked', '--features', 'replay', '--bin', 'kernel-host'], cwd=ROOT, check=True)
    server = subprocess.Popen(['node', 'adapters/h5/server.mjs', '--port', '0'], cwd=ROOT,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    url = json.loads(server.stdout.readline())['url']
    results, errors = {}, []
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(headless=True)

            def page_for():
                context = browser.new_context(viewport={'width': 1440, 'height': 1100})
                page = context.new_page()
                page.on('pageerror', lambda error: errors.append(str(error)))
                page.goto(url)
                return context, page

            context, page = page_for(); enter(page, url)
            wait(lambda: info(page, url)['snapshot']['hp'] < 1000)
            page.locator('#attack').click(); wait(lambda: info(page, url)['snapshot']['target']['hp'] == 100)
            page.locator('#cast').click(); wait(lambda: info(page, url)['snapshot']['target'] is None)
            before = info(page, url)
            page.reload(); active(page)
            restored = info(page, url)
            assert restored['id'] == before['id'] and restored['key'] == before['key']
            assert restored['connection']['generation'] == before['connection']['generation'] + 1
            for field in ['position', 'active_at_ms', 'inventory_quantity', 'inventory_revision', 'skill_ready_at_ms', 'basic_ready_at_ms', 'resources', 'target']:
                assert restored['snapshot'][field] == before['snapshot'][field], field
            assert restored['ledger']['admissions'] == 1 and restored['ledger']['warehouse'] == 95
            assert page.locator('#cast').is_disabled() and '冷却' in page.locator('#cast-detail').inner_text()
            for remaining in [1, 0]:
                page.locator('#pickup').click()
                wait(lambda: len(info(page, url)['snapshot']['resources']) == remaining)
            looted = info(page, url)
            assert looted['snapshot']['inventory_quantity'] == 7
            page.reload(); active(page)
            again = info(page, url)
            assert again['id'] == before['id'] and again['snapshot']['inventory_quantity'] == 7
            assert again['snapshot']['inventory_revision'] == looted['snapshot']['inventory_revision']
            assert again['snapshot']['skill_ready_at_ms'] == before['snapshot']['skill_ready_at_ms']
            early = page.request.post(url + f"/api/sessions/{again['id']}/input", data={
                'connection': again['connection'], 'sequence': again['sequence'] + 1, 'requestId': 'early-skill',
                'action': {'kind': 'cast', 'skill_id': 'duanjin-shou', 'direction': [1, 0]}})
            assert early.status == 409 and early.json()['error'] == 'INVALID_STATE', early.text()
            page.reload(); active(page)
            final = info(page, url)
            assert final['id'] == before['id'] and final['ledger']['admissions'] == 1
            assert final['sequence'] > again['sequence'] + 1
            assert final['snapshot']['skill_ready_at_ms'] == before['snapshot']['skill_ready_at_ms']
            page.screenshot(path=str(OUTPUT / 'reloaded.png'), full_page=True)
            page.locator('#close').click(); closed(page)
            assert info(page, url)['ledger']['warehouse'] == 99
            results['reload_state'] = {'id': before['id'], 'before': before['snapshot'], 'restored': restored['snapshot'],
                                       'finalConnection': final['connection'], 'ledger': info(page, url)['ledger']}
            context.close(); print('reload_state: passed', flush=True)

            context, page = page_for(); enter(page, url)
            page.locator('#arena').focus(); page.keyboard.down('w'); page.wait_for_timeout(150)
            before = info(page, url)
            context.set_offline(True)
            offline = wait(lambda: value if (value := info(page, url))['snapshot']['protected'] else None)
            stopped = offline['snapshot']['position']
            assert sum((a - b) ** 2 for a, b in zip(stopped, before['snapshot']['position'])) ** .5 < 40, 'Movement continued after transport failure'
            page.wait_for_timeout(1200)
            assert info(page, url)['snapshot']['position'] == stopped
            context.set_offline(False); active(page); page.keyboard.up('w')
            restored = info(page, url)
            assert restored['id'] == before['id'] and restored['connection']['generation'] == before['connection']['generation'] + 1
            assert restored['snapshot']['position'] == stopped and restored['ledger']['admissions'] == 1
            page.locator('#arena').focus(); page.keyboard.down('d'); page.wait_for_timeout(180); page.keyboard.up('d')
            wait(lambda: info(page, url)['snapshot']['position'] != stopped)
            page.locator('#close').click(); closed(page)
            results['network_recovery'] = {'stoppedPosition': stopped, 'connection': restored['connection'], 'sameMatch': True}
            context.close(); print('network_recovery: passed', flush=True)

            context, first = page_for(); enter(first, url)
            original = info(first, url); resume_requests = []
            second = context.new_page(); second.on('pageerror', lambda error: errors.append(str(error)))
            second.on('request', lambda request: resume_requests.append(request.post_data_json) if request.url.endswith('/resume') else None)
            second.goto(url); active(second)
            current = info(second, url)
            first.wait_for_function("document.body.dataset.connection === 'replaced'")
            assert first.locator('#attack').is_disabled() and first.locator('#start').is_disabled()
            old_input = first.request.post(url + f"/api/sessions/{current['id']}/input", data={
                'connection': original['connection'], 'requestId': 'old-tab', 'sequence': current['sequence'] + 100,
                'action': {'kind': 'move', 'direction': [1, 0]}})
            old_close = first.request.post(url + f"/api/sessions/{current['id']}/close", data={'connection': original['connection']})
            old_ack = first.request.post(url + f"/api/sessions/{current['id']}/ack", data={'connection': original['connection'], 'outputId': 'old-full'})
            old_create = first.request.post(url + '/api/sessions', data={'clock': 'live', 'previousConnection': original['connection']})
            assert all(reply.status == 409 and reply.json()['error'] == 'STALE_CONNECTION' for reply in [old_input, old_close, old_ack, old_create])
            retry = second.request.post(url + f"/api/sessions/{current['id']}/resume", data=resume_requests[0])
            assert retry.ok and retry.json()['connection'] == current['connection']
            assert info(second, url)['ledger']['admissions'] == 1 and info(second, url)['phase'] == 'active'
            second.locator('#close').click(); closed(second)
            results['tab_takeover'] = {'oldRejected': ['input', 'close', 'ack', 'new_match'], 'resumeRetryIdempotent': True,
                                        'oldConnection': original['connection'], 'newConnection': current['connection']}
            context.close(); print('tab_takeover: passed', flush=True)

            context, page = page_for(); page.locator('#clock').select_option('fast'); enter(page, url)
            before = info(page, url); context.set_offline(True)
            expired = wait(lambda: value if (value := info(page, url))['phase'] == 'closed' and value['ledger']['pending'] == [] else None)
            assert expired['id'] == before['id'] and expired['ledger']['admissions'] == expired['ledger']['settlements'] == 1
            assert expired['ledger']['pending'] == [] and expired['ledger']['warehouse'] == 98
            assert any(event['kind'] == 'connection_expired' for event in expired['events'])
            context.set_offline(False); closed(page)
            page.wait_for_function("document.body.dataset.connection === 'connected'")
            assert info(page, url)['phase'] == 'closed' and page.locator('#start').is_enabled()
            results['grace_expiry'] = {'clock': 'accelerated verification', 'ledger': expired['ledger'], 'worldLive': 0}
            context.close(); print('grace_expiry: passed', flush=True)

            context, page = page_for()
            page.route('**/ack', lambda route: route.abort())
            page.locator('#start').click()
            deploying = wait(lambda: value if (value := info(page, url)) and value['phase'] == 'deploying' and value['snapshot'] else None)
            page.unroute('**/ack'); page.reload(); active(page)
            restored = info(page, url)
            assert restored['id'] == deploying['id'] and restored['ledger']['admissions'] == 1
            assert sum(event['kind'] == 'activated' for event in restored['events']) == 1
            page.locator('#close').click(); closed(page)
            results['deployment_refresh'] = {'sameMatch': True, 'admissions': 1, 'activations': 1}
            context.close(); print('deployment_refresh: passed', flush=True)

            context, page = page_for(); page.locator('#fault').select_option('refused'); page.locator('#verify').click()
            page.locator('#reconcile').wait_for(state='visible', timeout=30000)
            before = info(page, url); page.reload()
            page.locator('#reconcile').wait_for(state='visible')
            assert info(page, url)['id'] == before['id'] and info(page, url)['phase'] == 'draining'
            page.locator('#reconcile').click(); closed(page)
            settled = info(page, url)
            assert (settled['ledger']['warehouse'], settled['ledger']['cultivation'], settled['ledger']['settlements']) == (102, 30, 1)
            page.reload(); closed(page)
            assert page.locator('#journey li.done').count() == 5
            assert page.locator('#result-title').inner_text() == '撤离成功'
            results['reload_pending_settlement'] = {'sameMatch': True, 'ledger': settled['ledger'], 'restoredClosedResult': True}
            context.close(); print('reload_pending_settlement: passed', flush=True)
            browser.close()
        assert not errors, errors
        assert sources == hashes(), 'Sources changed during reconnect verification'
    finally:
        server.terminate()
        try:
            server.wait(timeout=8)
        except subprocess.TimeoutExpired:
            server.kill(); server.wait()
    report = {'scope': 'B2-1 same-process reconnect, real Rust and controlled storage', 'rustTests': rust_tests,
              'results': results, 'pageErrors': errors, 'sourceSha256': sources}
    (OUTPUT / 'verification.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'status': 'passed', 'rustTests': len(rust_tests), 'browserCases': len(results), 'pageErrors': len(errors)}), flush=True)


if __name__ == '__main__':
    main()
