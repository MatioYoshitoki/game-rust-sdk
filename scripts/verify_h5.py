"""Exercise the local H5 against a real Rust process using Chromium/Playwright."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/h5'

PERFORMANCE_PROBE = r"""
window.h5Probe = { snapshots: [], frames: [], draws: [], longTasks: [], inputs: [], moveAt: 0 };
const NativeEventSource = window.EventSource;
window.EventSource = class extends NativeEventSource {
  constructor(...args) {
    super(...args);
    this.addEventListener('output', event => {
      const body = JSON.parse(event.data).body;
      if (body.kind === 'publication') h5Probe.snapshots.push({ at: performance.now(), logical: body.snapshot.logical_ms, position: body.snapshot.position });
    });
  }
};
let previousFrame;
function sampleFrame(now) {
  if (previousFrame) h5Probe.frames.push(now - previousFrame);
  previousFrame = now; requestAnimationFrame(sampleFrame);
}
requestAnimationFrame(sampleFrame);
new PerformanceObserver(list => h5Probe.longTasks.push(...list.getEntries().map(item => item.duration))).observe({ type: 'longtask', buffered: true });
const nativeFetch = window.fetch;
window.fetch = async (...args) => {
  const at = performance.now(); const response = await nativeFetch(...args);
  if (String(args[0]).endsWith('/input')) h5Probe.inputs.push(performance.now() - at);
  return response;
};
const nativeFillText = CanvasRenderingContext2D.prototype.fillText;
CanvasRenderingContext2D.prototype.fillText = function(label, x, y, ...args) {
  if (this.canvas.id === 'arena' && label === '你') h5Probe.draws.push({ at: performance.now(), x, y });
  return nativeFillText.call(this, label, x, y, ...args);
};
"""


def performance_result(page):
    probe = page.evaluate('h5Probe')
    gaps = sorted(b['at'] - a['at'] for a, b in zip(probe['snapshots'], probe['snapshots'][1:]))
    frames = sorted(probe['frames'])
    draws = [item for item in probe['draws'] if probe['moveAt'] + 150 < item['at'] < probe['moveAt'] + 1250]
    steps = [((b['x'] - a['x']) ** 2 + (b['y'] - a['y']) ** 2) ** .5 for a, b in zip(draws, draws[1:])]
    first_move = next(item['at'] for item in probe['snapshots'] if item['at'] > probe['moveAt'] and item['position'][1] < -1)
    result = {
        'snapshotCount': len(probe['snapshots']), 'snapshotGapMedianMs': gaps[len(gaps) // 2],
        'snapshotGapP95Ms': gaps[int(len(gaps) * .95)], 'rafGapMedianMs': frames[len(frames) // 2],
        'rafGapP95Ms': frames[int(len(frames) * .95)], 'longTasks': probe['longTasks'],
        'inputHttpMs': probe['inputs'], 'keydownToAuthoritativeMovementMs': first_move - probe['moveAt'],
        'movingFramesFraction': sum(step > .01 for step in steps) / len(steps), 'maxVisualStepPixels': max(steps),
        'projection': '2:1 isometric', 'visualCoordinates': 'canvas CSS pixels',
    }
    assert len(gaps) >= 50 and result['snapshotGapMedianMs'] < 65 and result['snapshotGapP95Ms'] < 100, result
    assert result['keydownToAuthoritativeMovementMs'] < 150, result
    assert result['movingFramesFraction'] > .65 and result['maxVisualStepPixels'] < 8, result
    (OUTPUT / 'performance-isometric.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


def wait(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.08)
    raise AssertionError('Timed out waiting for public H5 state')


def info(page, url):
    result = page.request.get(url + '/api/current')
    assert result.ok, result.text()
    return result.json()


def active(page):
    page.wait_for_function("document.querySelector('#phase').dataset.phase === 'active' && !document.querySelector('#attack').disabled", timeout=15000)


def closed(page):
    page.wait_for_function("document.querySelector('#phase').dataset.phase === 'closed' && document.querySelector('#pending').textContent === '0'", timeout=15000)


def enter(page, url):
    old = info(page, url)
    page.locator('#start').click()
    current = wait(lambda: value if (value := info(page, url)) and (not old or value['id'] != old['id']) else None)
    page.wait_for_function("prefix => document.querySelector('#match-id').textContent.includes(prefix) && !document.querySelector('#start').disabled && document.querySelector('#phase').dataset.phase === 'active' && !document.querySelector('#attack').disabled", arg=current['id'][:8])
    page.locator('#arena').focus()


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    subprocess.run(['node', '--test', 'adapters/h5/isometric.test.mjs'], cwd=ROOT, check=True)
    subprocess.run(['cargo', 'build', '--locked', '--features', 'replay', '--bin', 'kernel-host'], cwd=ROOT, check=True)
    server = subprocess.Popen(['node', 'adapters/h5/server.mjs', '--port', '0'], cwd=ROOT,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    url = json.loads(server.stdout.readline())['url']
    results = {}
    errors = []
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(headless=True)

            def page_for(profile=False, **options):
                context = browser.new_context(**options)
                page = context.new_page()
                page.on('pageerror', lambda error: errors.append(str(error)))
                if profile:
                    page.add_init_script(PERFORMANCE_PROBE)
                response = page.goto(url)
                assert response.status == 200
                page.wait_for_function("document.querySelector('#verify') && document.querySelector('canvas').width > 0")
                return context, page

            context, page = page_for(profile=True, viewport={'width': 1440, 'height': 1100})
            assert page.request.head(url + '/').status == 200
            page.screenshot(path=str(OUTPUT / 'desktop-initial.png'), full_page=True)
            page.locator('#start').click(); active(page)
            page.locator('#arena').focus()
            page.evaluate('h5Probe.snapshots = []; h5Probe.frames = []; h5Probe.draws = []; h5Probe.inputs = []; h5Probe.longTasks = []; h5Probe.moveAt = performance.now()')
            page.keyboard.down('w'); page.wait_for_timeout(1400); page.keyboard.up('w')
            moved = wait(lambda: current if (current := info(page, url))['snapshot']['position'][1] < -20 else None)
            page.keyboard.down('s'); page.wait_for_timeout(1400); page.keyboard.up('s')
            page.wait_for_timeout(350)
            results['realtime_performance'] = performance_result(page)
            page.wait_for_timeout(300); stopped = info(page, url)['snapshot']['position']
            page.wait_for_timeout(350); assert info(page, url)['snapshot']['position'] == stopped
            page.locator('#pause').click()
            wait(lambda: info(page, url)['snapshot']['protected'])
            page.wait_for_function("document.querySelector('#attack').disabled")
            page.wait_for_timeout(300); assert info(page, url)['snapshot']['position'] == stopped
            page.locator('#pause').click(); active(page)
            page.locator('#extract').click()
            page.wait_for_function("document.querySelector('#event-log').textContent.includes('INVALID_STATE')")
            session = info(page, url)
            other, other_page = page_for(viewport={'width': 1000, 'height': 800})
            assert other_page.request.get(url + f"/api/sessions/{session['id']}").status == 404
            other.close()
            rejected = page.request.post(url + f"/api/sessions/{session['id']}/input", data={
                'requestId': 'invalid-patch', 'sequence': 100,
                'action': {'kind': 'attack', 'world': {'hp': 100000}}})
            assert rejected.status == 400
            assert info(page, url)['snapshot']['hp'] <= 1000
            page.locator('#close').click(); closed(page)
            current = info(page, url)
            assert current['ledger']['warehouse'] == 98 and current['ledger']['settlements'] == 1
            assert current['ledger']['pending'] == []
            page.screenshot(path=str(OUTPUT / 'manual-closed.png'), full_page=True)
            results['manual_live'] = {'keyboardMoved': moved['snapshot']['position'], 'pauseStoppedMovement': True,
                                      'earlyExtractionRejected': True, 'otherOwnerDenied': True,
                                      'worldPatchRejected': True, 'ledger': current['ledger']}
            context.close()
            print('manual_live: passed', flush=True)

            context, page = page_for(viewport={'width': 1440, 'height': 1100})
            assert page.locator('#arena').get_attribute('data-projection') == 'isometric'
            input_failures = []
            page.on('response', lambda response: input_failures.append({'url': response.url, 'status': response.status})
                    if '/api/sessions/' in response.url and response.status >= 400 else None)
            directions = {}
            for keys, axis, sign in [(('w',), 1, -1), (('d',), 0, 1), (('s',), 1, 1), (('a',), 0, -1), (('w', 'd'), None, 1)]:
                # Independent starts keep target-body blocking out of the direction checks.
                enter(page, url)
                before = info(page, url)['snapshot']['position']
                for key in keys:
                    page.keyboard.down(key)
                page.wait_for_timeout(320)
                for key in keys:
                    page.keyboard.up(key)
                page.wait_for_timeout(200)
                after = info(page, url)['snapshot']['position']
                dx, dy = after[0] - before[0], after[1] - before[1]
                projected = [dx - dy, (dx + dy) / 2]
                if axis is not None:
                    assert projected[axis] * sign > 10 and abs(projected[1 - axis]) < 6, (keys, projected)
                else:
                    assert projected[0] > 10 and projected[1] < -10
                    assert abs(projected[0] + projected[1]) < 8, projected
                directions['+'.join(keys)] = projected

            # Start at the frozen spawn, then walk diagonally into the real world-space wall.
            enter(page, url)
            page.locator('#attack').click()
            wait(lambda: info(page, url)['snapshot']['target']['hp'] == 100)
            page.wait_for_timeout(800); page.locator('#attack').click()
            wait(lambda: info(page, url)['snapshot']['target'] is None)
            page.locator('#arena').focus()
            page.keyboard.down('d'); page.keyboard.down('s')
            wait(lambda: value if (value := info(page, url))['snapshot']['position'][0] >= 195 else None)
            page.wait_for_timeout(350)
            held = info(page, url)['snapshot']['position']
            page.wait_for_timeout(200)
            assert info(page, url)['snapshot']['position'] == held, 'Held movement crossed the wall'
            page.keyboard.up('d'); page.keyboard.up('s'); page.wait_for_timeout(200)
            blocked = info(page, url)['snapshot']['position']
            assert 195 <= blocked[0] <= 204.00001 and blocked[1] < 96, blocked

            # Resize, aim at a known ground point through the visible canvas, and inspect the public input.
            page.set_viewport_size({'width': 1120, 'height': 1000}); page.wait_for_timeout(250)
            box = page.locator('#arena').bounding_box()
            current = info(page, url)['snapshot']['position']
            point = [current[0] + 40, current[1] - 140]
            scale = min((box['width'] - 64) / 2048, (box['height'] - 104) / 1024)
            pointer = [box['x'] + box['width'] / 2 + (point[0] - point[1]) * scale,
                       box['y'] + box['height'] / 2 + 8 + (point[0] + point[1]) * scale / 2]
            page.mouse.move(*pointer); page.locator('#arena').focus()
            with page.expect_request(lambda request: request.url.endswith('/input') and request.post_data_json['action']['kind'] == 'cast') as cast:
                page.keyboard.press('q')
            aimed = cast.value.post_data_json['action']['direction']
            assert abs(aimed[0] - 40) < .001 and abs(aimed[1] + 140) < .001, aimed
            page.wait_for_function("document.querySelector('#event-log').textContent.includes('duanjin-shou 已提交，灵力 −20')")
            # Passive regeneration may share the commit step; the event proves the exact cost.
            assert 80 <= info(page, url)['snapshot']['spirit'] < 100
            page.screenshot(path=str(OUTPUT / 'isometric.png'), full_page=True)
            assert not input_failures, input_failures
            page.locator('#close').click(); closed(page)
            results['isometric_controls'] = {'screenDirections': directions, 'wallContact': blocked,
                                               'pointerWorldDirectionAfterResize': aimed, 'projection': '2:1 diamond'}
            context.close(); print('isometric_controls: passed', flush=True)

            for case, fault, close_before_reconcile in [
                ('normal', 'none', False), ('lost_receipt', 'lost_receipt', False),
                ('refused', 'refused', False), ('refused_after_close', 'refused', True),
            ]:
                context, page = page_for(viewport={'width': 1440, 'height': 1100})
                page.locator('#fault').select_option(fault)
                page.locator('#verify').click()
                if fault == 'refused':
                    page.locator('#reconcile').wait_for(state='visible', timeout=30000)
                    pending = info(page, url)
                    assert pending['phase'] == 'draining' and pending['ledger']['warehouse'] == 95
                    assert pending['ledger']['settlements'] == 0 and len(pending['ledger']['pending']) == 2
                    if close_before_reconcile:
                        rejected = page.request.post(url + '/api/sessions', data={'clock': 'live', 'fault': 'none', 'seed': 7,
                                                                                'previousConnection': pending['connection']})
                        assert rejected.status == 409 and rejected.json()['error'] == 'UNRESOLVED_IO'
                        wait(lambda: info(page, url)['phase'] == 'closed')
                        assert info(page, url)['id'] == pending['id']
                    page.locator('#reconcile').click()
                page.wait_for_function("document.body.dataset.verification === 'passed' || document.body.dataset.verification === 'failed'", timeout=40000)
                assert page.locator('body').get_attribute('data-verification') == 'passed', page.locator('#event-log').inner_text()
                current = info(page, url)
                assert current['phase'] == 'closed' and current['ledger']['pending'] == []
                assert (current['ledger']['warehouse'], current['ledger']['cultivation']) == (102, 30)
                assert (current['ledger']['admissions'], current['ledger']['settlements']) == (1, 1)
                assert page.locator('#journey li.done').count() == 5
                assert page.locator('#start').is_enabled()
                assert page.locator('#clock').input_value() == 'live', 'Automatic verification changed the next manual match clock'
                if fault == 'refused':
                    assert current['ledger']['reconciliations'] == 1
                if fault == 'lost_receipt':
                    assert '查询原业务结果' in page.locator('#event-log').inner_text()
                page.screenshot(path=str(OUTPUT / f'{case}.png'), full_page=True)
                results[case] = {'phase': current['phase'], 'ledger': current['ledger'], 'progress': '5 / 5'}
                if case == 'normal':
                    page.locator('#start').click(); active(page)
                    assert info(page, url)['clock'] == 'live'
                    page.locator('#close').click(); closed(page)
                context.close()
                print(f'{case}: passed', flush=True)

            context, page = page_for(viewport={'width': 1440, 'height': 1100})
            page.locator('#verify').click()
            page.wait_for_function("document.querySelector('#phase').dataset.phase === 'active'", timeout=15000)
            page.locator('#close').click(); closed(page)
            assert page.locator('body').get_attribute('data-verification') == 'cancelled'
            assert page.locator('#verify').is_enabled()
            results['cancel_auto'] = {'phase': info(page, url)['phase'], 'automaticStopped': True, 'ledger': info(page, url)['ledger']}
            context.close(); print('cancel_auto: passed', flush=True)

            context, page = page_for(viewport={'width': 390, 'height': 844}, is_mobile=True, has_touch=True, device_scale_factor=1)
            assert page.evaluate('document.documentElement.scrollWidth <= window.innerWidth')
            page.locator('#start').click(); active(page)
            right = page.locator('[data-dir="1,0"]'); right.scroll_into_view_if_needed()
            box = right.bounding_box(); cdp = context.new_cdp_session(page)
            start_position = info(page, url)['snapshot']['position']
            cdp.send('Input.dispatchTouchEvent', {'type': 'touchStart', 'touchPoints': [{'x': box['x'] + box['width'] / 2, 'y': box['y'] + box['height'] / 2}]})
            page.wait_for_timeout(450)
            cdp.send('Input.dispatchTouchEvent', {'type': 'touchEnd', 'touchPoints': []})
            wait(lambda: info(page, url)['snapshot']['position'][0] > start_position[0] + 1)
            page.wait_for_timeout(350); released = info(page, url)['snapshot']['position']
            assert abs((released[0] - start_position[0]) + (released[1] - start_position[1])) < 1e-6, released
            page.wait_for_timeout(350); assert info(page, url)['snapshot']['position'] == released
            page.locator('#arena-wrap').scroll_into_view_if_needed()
            page.screenshot(path=str(OUTPUT / 'mobile.png'), full_page=True)
            page.locator('#close').click(); closed(page)
            results['mobile'] = {'width': 390, 'noHorizontalOverflow': True, 'touchMovement': True, 'ledger': info(page, url)['ledger']}
            context.close(); browser.close()
            assert not errors, errors
            print('mobile: passed', flush=True)
    finally:
        server.terminate()
        try:
            server.wait(timeout=8)
        except subprocess.TimeoutExpired:
            server.kill(); server.wait()
    paths = [*ROOT.glob('config/**/*.json'), ROOT / 'package.json', *ROOT.glob('web/*'), *ROOT.glob('adapters/h5/*.mjs'),
             ROOT / 'adapters/node/controlled-store.mjs', ROOT / 'scripts/verify_h5.py', ROOT / 'config/b1.json', ROOT / 'config/b3-1.json']
    report = {'scope': 'H5 against the real B1 Rust kernel and controlled storage', 'results': results,
              'pageErrors': errors, 'sourceSha256': {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}}
    (OUTPUT / 'verification.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'status': 'passed', 'browserCases': len(results), 'pageErrors': len(errors), 'report': 'artifacts/h5/verification.json'}), flush=True)


if __name__ == '__main__':
    main()
