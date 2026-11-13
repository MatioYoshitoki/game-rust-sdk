"""Verify an external public-API consumer with ordinary Runtime scheduling."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
CONSUMER = ROOT / 'examples/minimal-host'
OUTPUT = ROOT / 'artifacts/sdk-2'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = {
        ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'src/lib.rs',
        ROOT / 'README.md', Path(__file__),
        ROOT / 'docs/batches/SDK-2-host-contract.md',
        ROOT / 'docs/guides/sdk-integration.md',
        ROOT / 'docs/interfaces/independent-match-kernel.INTERFACE.md',
        CONSUMER / 'Cargo.toml', CONSUMER / 'Cargo.lock', CONSUMER / 'account.json',
        *ROOT.glob('src/**/*.rs'), *ROOT.glob('src/**/*.json'),
        *ROOT.glob('config/**/*.json'), *CONSUMER.glob('src/*.rs'),
    }
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}


def run(name, command, *, cwd=ROOT, env=None):
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=240,
                            env={**os.environ, 'CARGO_TERM_COLOR': 'never', **(env or {})})
    (OUTPUT / f'{name}.log').write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f'{name} failed ({result.returncode}):\n'
                           f'{result.stdout[-5000:]}\n{result.stderr[-5000:]}')
    print(f'{name}: passed', flush=True)
    return result.stdout


def verify_result(value, mode, config, account):
    assert value['mode'] == mode and value['clock'] == 'ordinary', value
    assert value['configId'] == config['id']
    assert value['phase'] == 'closed' and value['closedEvents'] == 1
    assert value['pendingOperations'] == value['liveWorlds'] == 0
    assert value['initial'] == account
    operations = value['operations']

    def rows(kind):
        return [(key, body, result) for key, (body, result) in operations.items()
                if body['kind'] == kind]

    loads, admissions = rows('load_participant'), rows('commit_admission')
    settlements, compensations = rows('commit_settlement'), rows('compensate_admission')
    queries = rows('query_operation')
    assert len(loads) == len(admissions) == value['counts']['loads'] == 1
    initial, final = account['facts'], value['account']['facts']
    assert loads[0][2]['facts'] == initial
    assert admissions[0][1]['quantity'] == config['carried_quantity']
    assert admissions[0][1]['entry_years'] == config['params']['entry_years']
    expected_warehouse = initial['warehouse_quantity']
    expected_years, expected_cultivation = account['entry_years'], account['cultivation']
    expected_revision = initial['revision']

    if mode == 'refused-admission':
        assert admissions[0][2]['kind'] == 'refused'
        assert not settlements and not compensations and not value['activeSeen']
        assert value['counts']['admissions'] == 0
    else:
        assert admissions[0][2]['kind'] == 'committed'
        assert value['counts']['admissions'] == 1
        expected_warehouse -= admissions[0][1]['quantity']
        expected_years += admissions[0][1]['entry_years']
        expected_revision += 1
        assert admissions[0][2]['revision'] == expected_revision
        if mode == 'close-before-ready':
            assert len(compensations) == 1 and not settlements and not value['activeSeen']
            assert value['closedWithPending'], 'Closed must not hide pending compensation'
            body, receipt = compensations[0][1:]
            assert body['original_operation_id'] == admissions[0][0]
            assert body['quantity'] == admissions[0][1]['quantity']
            assert body['entry_years'] == admissions[0][1]['entry_years']
            expected_warehouse += body['quantity']
            expected_years -= body['entry_years']
        else:
            assert len(settlements) == 1 and not compensations and value['activeSeen']
            body, receipt = settlements[0][1:]
            assert body['outcome'] == 'death' and body['expected_revision'] == expected_revision
            expected_warehouse += body['kept']
            expected_cultivation += body['cultivation']
        expected_revision += 1
        assert receipt['kind'] == 'committed' and receipt['revision'] == expected_revision
    assert value['counts']['settlements'] == value['settlementEvents'] == len(settlements)
    assert value['counts']['compensations'] == len(compensations)
    assert final['warehouse_quantity'] == expected_warehouse
    assert final['revision'] == expected_revision
    for stat in ['principal', 'hp_points', 'spirit_points', 'atk_points', 'def_points', 'spd_points']:
        assert final[stat] == initial[stat]
    assert value['account']['entry_years'] == expected_years
    assert value['account']['cultivation'] == expected_cultivation
    assert value['account']['items'] == account['items']
    assert value['counts']['queries'] == len(queries) == int(mode == 'unknown-settlement')
    assert value['unknownInjected'] == (mode == 'unknown-settlement')
    if queries:
        assert queries[0][1]['operation_id'] == settlements[0][0]
        assert queries[0][2] == settlements[0][2], 'Query must return the original commit proof'
    assert value['failedPublications'] == int(mode == 'failed-publication')


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    source_hashes = sources()
    run('format-sdk', ['cargo', 'fmt', '--all', '--', '--check'])
    run('format-consumer', ['cargo', 'fmt', '--manifest-path',
                           str(CONSUMER / 'Cargo.toml'), '--', '--check'])
    tests = {}
    for name, flags in [('default', []), ('all-features', ['--all-features'])]:
        out = run(f'rust-{name}', ['cargo', 'test', '--locked', *flags])
        tests[name] = sorted(re.findall(r'^test (\S+) \.\.\. ok$', out, re.M))
    assert len(tests['default']) >= 612 and tests['default'] == tests['all-features']
    run('clippy-sdk', ['cargo', 'clippy', '--locked', '--all-targets', '--all-features',
                       '--', '-D', 'warnings'])
    cases = {}
    with tempfile.TemporaryDirectory(prefix='sdk-2-consumer-') as directory:
        workspace = Path(directory)
        consumer = workspace / 'host'
        shutil.copytree(CONSUMER / 'src', consumer / 'src')
        shutil.copyfile(CONSUMER / 'Cargo.lock', consumer / 'Cargo.lock')
        shutil.copyfile(CONSUMER / 'account.json', consumer / 'account.json')
        manifest = (CONSUMER / 'Cargo.toml').read_text().replace('path = "../.."',
                                                               f'path = {json.dumps(str(ROOT))}')
        (consumer / 'Cargo.toml').write_text(manifest)
        target = OUTPUT / 'consumer-target'
        env = {'CARGO_TARGET_DIR': str(target)}
        for name, command in [('build', ['cargo', 'build', '--locked']),
                              ('clippy', ['cargo', 'clippy', '--locked', '--all-targets',
                                          '--', '-D', 'warnings'])]:
            run(f'external-{name}', command, cwd=consumer, env=env)
        tree = run('external-features', ['cargo', 'tree', '--locked', '-e', 'features'],
                   cwd=consumer, env=env)
        assert 'match-kernel feature "replay"' not in tree
        binary = target / 'debug/minimal-match-host'
        account_path = consumer / 'account.json'
        initial = json.loads(account_path.read_text())
        config = json.loads((ROOT / 'config/b1.json').read_text())

        def case(name, mode, catalog=ROOT / 'config/catalog.json', raw=ROOT / 'config/b1.json',
                 facts=account_path):
            value = json.loads(run(name, [str(binary), str(catalog), str(facts), mode], cwd=consumer))
            verify_result(value, mode, json.loads(raw.read_text()), json.loads(facts.read_text()))
            assert value['configHash'] == digest(raw)
            cases[name] = value

        for mode in ['normal', 'unknown-settlement', 'refused-admission', 'close-before-ready',
                     'failed-publication', 'config-mismatch']:
            case(mode, mode)
        package = workspace / 'config'
        package.mkdir()
        config['id'] = 'consumer-training-v1'
        (package / 'training.json').write_text(json.dumps(config))
        (package / 'catalog.json').write_text(json.dumps({
            'schema_version': 1, 'default_scenario': 'training-yard',
            'active': {'training-yard': 'training.json'}, 'archives': []}))
        case('custom-default', 'normal', package / 'catalog.json', package / 'training.json')
        initial['facts'].update(hp_points=1200, spirit_points=150, atk_points=130,
                                def_points=12, spd_points=10, warehouse_quantity=137, revision=10)
        initial.update(cultivation=11, entry_years=4, seed=19)
        variant = consumer / 'variant-account.json'
        variant.write_text(json.dumps(initial))
        case('custom-account', 'normal', facts=variant)
        binary_hash = digest(binary)
    assert sources() == source_hashes, 'Sources changed during verification'
    (OUTPUT / 'verification.json').write_text(json.dumps({
        'passed': True, 'at': datetime.now(timezone.utc).isoformat(),
        'scope': 'SDK-2 public API consumer; in-memory host; ordinary scheduling',
        'rustTestCount': len(tests['default']), 'sameTestsUnderAllFeatures': True,
        'rustTests': tests['default'], 'consumerCases': cases,
        'sourceHashes': source_hashes, 'consumerBinarySha256': binary_hash,
        'limits': ['single participant', 'in-memory sample storage and local receiver',
                   'no cross-game semantic validation or package publication'],
    }, indent=2) + '\n')
    print(f"SDK-2: 8 consumer cases and {len(tests['default'])} native tests passed")


if __name__ == '__main__':
    main()
