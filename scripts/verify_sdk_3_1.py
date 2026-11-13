"""SDK-3-1: versioned assets, real local crash recovery and unchanged protocol-1 consumers."""
import hashlib
import argparse
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

from verify_sdk_2 import verify_result

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/sdk-3-1'
BINARY = ROOT / 'target/debug/kernel-host'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'README.md',
             *ROOT.glob('src/**/*.rs'), *ROOT.glob('config/**/*.json'),
             *ROOT.glob('examples/asset-host/*.json'), *ROOT.glob('examples/result-host/*.json'), *ROOT.glob('examples/minimal-host/src/*.rs'),
             ROOT / 'examples/minimal-host/Cargo.toml', ROOT / 'examples/minimal-host/Cargo.lock',
             ROOT / 'examples/minimal-host/account.json', *ROOT.glob('adapters/python/*.py'),
             *ROOT.glob('adapters/node/*.mjs'), *ROOT.glob('adapters/h5/*.mjs'),
             *ROOT.glob('scripts/*.py'), *ROOT.glob('docs/interfaces/*.md'),
             ROOT / 'docs/guides/sdk-integration.md', ROOT / 'docs/batches/SDK-3-1-assets.md']
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(set(paths))}


def run(name, command, *, cwd=ROOT, env=None, expected=0):
    start = time.monotonic()
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True, timeout=1200,
                            env={**os.environ, 'CARGO_TERM_COLOR': 'never', **(env or {})})
    (OUTPUT / f'{name}.log').write_text(result.stdout + result.stderr)
    assert result.returncode == expected, (name, result.returncode, result.stdout[-1000:], result.stderr[-3500:])
    print(f'{name}: passed ({time.monotonic()-start:.1f}s)', flush=True)
    return result.stdout


def read_state(path):
    with sqlite3.connect(path) as db:
        assert db.execute('PRAGMA quick_check').fetchone()[0] == 'ok'
        return json.loads(db.execute('SELECT state FROM host WHERE id=1').fetchone()[0])


def verify_assets(result, state, *, pending=False):
    assert result['phase'] == 'closed' and result['liveWorlds'] == 0
    assert bool(result['pending']) == pending
    expected = state['initial']['assets'].copy()
    counts = {'admission': 0, 'settlement': 0, 'compensation': 0}
    committed = []
    for op, request in state['requests'].items():
        body = request['body']
        assert 'cultivation' not in json.dumps(body) and 'entry_years' not in json.dumps(body)
        assert body['kind'] in ['load_assets', 'commit_assets', 'query_operation', 'reconcile_operation']
        if body['kind'] != 'commit_assets':
            continue
        transaction = body['transaction']
        rows = transaction.get('debits', transaction.get('credits', []))
        assert [r['asset_id'] for r in rows] == sorted({r['asset_id'] for r in rows})
        assert all(r['quantity'] > 0 for r in rows)
        proof = state['journal'].get(op)
        if proof and proof['kind'] == 'committed':
            committed.append((proof['revision'], transaction))
    committed.sort(key=lambda row: row[0])
    assert [r for r, _ in committed] == list(range(state['initial']['revision'] + 1, state['revision'] + 1))
    for _, transaction in committed:
        kind = transaction['kind']
        if kind == 'result_settlement':
            kind = 'settlement'
        counts[kind] += 1
        sign = -1 if kind == 'admission' else 1
        for row in transaction.get('debits', transaction.get('credits', [])):
            expected[row['asset_id']] = expected.get(row['asset_id'], 0) + sign * row['quantity']
    assert result['counts'] == counts and result['balances'] == expected
    events = result['events']
    assert not any(e['kind'] == 'settlement_applied' for e in events)
    event_kind = 'result_settlement_applied' if state['create']['protocol_version'] == 3 else 'asset_settlement_applied'
    assert sum(e['kind'] == event_kind for e in events) == counts['settlement']


def consumer_cases(directory):
    from verify_sdk_2 import CONSUMER
    consumer = directory / 'consumer'
    shutil.copytree(CONSUMER / 'src', consumer / 'src')
    for file in ['Cargo.lock', 'account.json']:
        shutil.copyfile(CONSUMER / file, consumer / file)
    (consumer / 'Cargo.toml').write_text((CONSUMER / 'Cargo.toml').read_text().replace(
        'path = "../.."', f'path = {json.dumps(str(ROOT))}'))
    env = {'CARGO_TARGET_DIR': str(OUTPUT / 'consumer-target')}
    run('consumer-format', ['cargo', 'fmt', '--', '--check'], cwd=consumer, env=env)
    run('consumer-build', ['cargo', 'build', '--locked'], cwd=consumer, env=env)
    run('consumer-clippy', ['cargo', 'clippy', '--locked', '--all-targets', '--', '-D', 'warnings'], cwd=consumer, env=env)
    binary = OUTPUT / 'consumer-target/debug/minimal-match-host'
    values = {}

    def case(name, mode, catalog=ROOT / 'config/b1-catalog.json', raw=ROOT / 'config/b1.json', account=consumer / 'account.json'):
        result = json.loads(run('consumer-' + name, [str(binary), str(catalog), str(account), mode], cwd=consumer))
        verify_result(result, mode, json.loads(raw.read_text()), json.loads(account.read_text()))
        assert result['configHash'] == digest(raw)
        values[name] = result
    for mode in ['normal', 'unknown-settlement', 'refused-admission', 'close-before-ready', 'failed-publication', 'config-mismatch']:
        case(mode, mode)
    package = directory / 'consumer-config'
    package.mkdir()
    raw = json.loads((ROOT / 'config/b1.json').read_text())
    raw['id'] = 'consumer-training-v1'
    (package / 'arena.json').write_text(json.dumps(raw))
    (package / 'catalog.json').write_text(json.dumps({'schema_version': 1, 'default_scenario': 'training-yard',
                                                    'active': {'training-yard': 'arena.json'}, 'archives': []}))
    case('custom-default', 'normal', package / 'catalog.json', package / 'arena.json')
    facts = json.loads((consumer / 'account.json').read_text())
    facts['facts'].update(hp_points=1200, spirit_points=150, atk_points=130, def_points=12,
                          spd_points=10, warehouse_quantity=137, revision=10)
    facts.update(cultivation=11, entry_years=4, seed=19)
    (consumer / 'variant.json').write_text(json.dumps(facts))
    case('custom-account', 'normal', account=consumer / 'variant.json')
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reuse-b1', action='store_true', help='reuse baseline only when every B1 input hash still matches')
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    frozen = sources()
    # Run native + original wire/replay/Node baseline once, then use focused process suites.
    if not args.reuse_b1:
        run('b1', [sys.executable, 'scripts/verify_b1.py', '--node-workers', '1'])
    baseline = json.loads((ROOT / 'docs/evidence/b1-results.json').read_text())
    # This orchestration file is not an input to native/wire/replay/Node B1 behavior.
    # All Rust, configuration, B1 executors and acceptance mappings must match exactly.
    unrelated_harnesses = {str(Path(__file__).relative_to(ROOT)),
                           'scripts/verify_sdk_3_2.py', 'scripts/verify_durability.py'}
    changed_harnesses = []
    for path, expected in baseline['sourceSha256'].items():
        if digest(ROOT / path) != expected:
            assert args.reuse_b1 and path in unrelated_harnesses, ('B1 input changed; rerun baseline', path)
            changed_harnesses.append(path)
    native = baseline['rust']['executedTests']
    assert len([t for t in native if t.startswith('asset_contract::tests::')]) == 14
    cases = {}
    with tempfile.TemporaryDirectory(prefix='sdk-3-1-') as temp:
        workspace = Path(temp)
        consumers = consumer_cases(workspace)

        def host(name, path, *, mode='death', flags=(), config=ROOT / 'examples/asset-host/catalog.json',
                 account=ROOT / 'examples/asset-host/account.json', crash=False):
            command = [sys.executable, 'adapters/python/asset_host.py', '--binary', str(BINARY),
                       '--configs', str(config), '--account', str(account), '--state', str(path),
                       '--verification', '--mode', mode, *flags]
            output = run(name, command, expected=-9 if crash else 0)
            if crash:
                assert json.loads(output)['crash'] in ['before', 'after']
                return None
            value = json.loads(output)
            verify_assets(value, read_state(path), pending=(mode == 'refused-settlement'))
            cases[name] = value
            return value

        for mode in ['death', 'extraction', 'active-close', 'unknown-settlement', 'retry-settlement', 'admission-refused', 'close-before-ready']:
            state = workspace / (mode + '.sqlite')
            result = host(mode, state, mode=mode)
            expected = {'entry-credit': 99, 'experience': 30 if mode == 'extraction' else 3, 'xuan-tie': 102 if mode == 'extraction' else 99}
            if mode in ['admission-refused', 'close-before-ready']:
                expected = {'entry-credit': 100, 'experience': 0, 'xuan-tie': 100}
            assert result['balances'] == expected
            if mode == 'retry-settlement':
                assert next(r for r in result['requests'].values() if r['body'].get('transaction', {}).get('kind') == 'settlement')['attempt_id'] == 2
            if mode == 'unknown-settlement':
                assert any(r['body']['kind'] == 'query_operation' for r in result['requests'].values())
        path = workspace / 'review.sqlite'
        refused = host('refused-settlement', path, mode='refused-settlement')
        assert refused['counts']['settlement'] == 0 and refused['balances']['experience'] == 0
        reviewed = host('review-original', path, flags=['--reconcile'])
        again = host('review-again', path)
        assert again['balances'] == reviewed['balances'] and again['counts'] == reviewed['counts']

        for point, kind in [('before', 'settlement'), ('after', 'settlement'), ('after', 'admission'),
                            ('after', 'active'), ('before', 'compensation'), ('after', 'compensation')]:
            path = workspace / f'crash-{point}-{kind}.sqlite'
            mode = 'close-before-ready' if kind == 'compensation' else 'death'
            host(f'crash-{point}-{kind}', path, mode=mode, flags=[f'--crash-{point}', kind], crash=True)
            old = read_state(path)
            result = host(f'recover-{point}-{kind}', path)
            repeat = host(f'repeat-{point}-{kind}', path)
            assert result['counts'] == repeat['counts'] and result['balances'] == repeat['balances']
            if kind != 'settlement':
                assert result['balances'] == old['initial']['assets'] and result['counts']['compensation'] == 1
            else:
                assert result['counts']['settlement'] == 1
                before = next(r for r in old['requests'].values() if r['body'].get('transaction', {}).get('kind') == 'settlement')
                assert result['requests'][before['operation_id']]['body'] == before['body']

        package = workspace / 'mapped'
        package.mkdir()
        raw = json.loads((ROOT / 'examples/asset-host/arena.json').read_text())
        raw.update(id='configured-assets-v2', asset_contract={'entry_cost_asset_id': 'ticket', 'reward_asset_id': 'rank-points'})
        raw['params']['entry_years'] = 2
        (package / 'arena.json').write_text(json.dumps(raw))
        (package / 'catalog.json').write_text(json.dumps({'schema_version': 1, 'default_scenario': 'custom', 'active': {'custom': 'arena.json'}, 'archives': []}))
        account = json.loads((ROOT / 'examples/asset-host/account.json').read_text())
        account['assets'] = {'ticket': 80, 'rank-points': 11, 'xuan-tie': 137}
        (package / 'account.json').write_text(json.dumps(account))
        path = workspace / 'mapped.sqlite'
        host('mapping-crash', path, flags=['--crash-before', 'settlement'], config=package / 'catalog.json', account=package / 'account.json', crash=True)
        frozen_hash = read_state(path)['create']['config_hash']
        # Startup changes must never change the old account obligation or selected assets.
        raw['params']['entry_years'] = 7
        raw['asset_contract']['reward_asset_id'] = 'new-reward'
        (package / 'arena.json').write_text(json.dumps(raw))
        result = host('mapping-frozen-recovery', path, config=package / 'catalog.json', account=package / 'account.json')
        assert result['configHash'] == frozen_hash and result['balances'] == {'ticket': 78, 'rank-points': 14, 'xuan-tie': 136}

        # Exercise storage atomicity independently of Rust's earlier resource gate.
        sys.path.insert(0, str(ROOT / 'adapters/python'))
        from asset_host import Ledger
        ledger = Ledger(workspace / 'atomic.sqlite')
        request = {'operation_id': 'atomic', 'attempt_id': 1, 'body': {'kind': 'commit_assets', 'transaction': {
            'kind': 'admission', 'expected_revision': 0, 'debits': [{'asset_id': 'entry-credit', 'quantity': 1}, {'asset_id': 'xuan-tie', 'quantity': 5}]}}}
        ledger.state = {'requests': {'atomic': request}, 'journal': {}, 'balances': {'entry-credit': 0, 'xuan-tie': 100},
                        'revision': 0, 'counts': {'admission': 0, 'settlement': 0, 'compensation': 0}}
        ledger.save()
        assert ledger.execute(request) == {'kind': 'refused'}
        ledger.db.close()
        assert read_state(workspace / 'atomic.sqlite')['balances'] == {'entry-credit': 0, 'xuan-tie': 100}
    for name, command in [('configuration', [sys.executable, 'scripts/verify_config_loading.py']),
                          ('durability', [sys.executable, 'scripts/verify_durability.py']),
                          ('h5', [sys.executable, 'scripts/verify_h5.py'])]:
        run(name, command)
    assert frozen == sources(), 'Sources changed during acceptance'
    (OUTPUT / 'verification.json').write_text(json.dumps({
        'passed': True, 'scope': 'SDK-3-1 finite versioned assets and local process durability',
        'sourceHashes': frozen, 'rustTestCount': len(native), 'newAssetTests': 14,
        'sameTestsUnderAllFeatures': baseline['rust']['defaultPassed'] == baseline['rust']['allFeaturesPassed'],
        'assetCases': cases, 'legacyConsumerCases': consumers, 'atomicInsufficientBalance': 'passed',
        'regressions': ['B1', 'SDK-2 consumer', 'configuration loading', 'B2-3 durability', 'H5 baseline'],
        'b1ReportSha256': digest(ROOT / 'docs/evidence/b1-results.json'), 'reusedB1': args.reuse_b1,
        'changedNonB1Harnesses': changed_harnesses,
        'binarySha256': digest(BINARY),
    }, indent=2) + '\n')
    print(f'SDK-3-1: {len(native)} native tests, {len(cases)} asset outcomes, 8 legacy consumers passed')


if __name__ == '__main__':
    main()
