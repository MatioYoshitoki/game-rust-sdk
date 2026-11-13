"""Run B1's declared acceptance matrix and bind evidence to the tested sources."""
import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
import shutil
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from pathlib import Path
from zoneinfo import ZoneInfo

ROOT = Path(__file__).resolve().parents[1]
LOGS = ROOT / 'artifacts' / 'b1'
NODE_CASES = {
    'normal': [], 'death': ['--death'], 'unknown': ['--unknown'],
    'duplicates': ['--death', '--duplicates'],
    'publication_failure': ['--death', '--publication-failure'],
    'retry': ['--death', '--retry-settlement'],
    'refused': ['--death', '--refused-settlement'],
    'retry_exhausted': ['--death', '--retry-exhausted'],
    'query_unknown': ['--death', '--query-unknown'],
    'close_pending': ['--death', '--close-pending'],
    'combined': ['--death', '--duplicates', '--publication-failure', '--refused-settlement'],
    'oversized': ['--death', '--oversized'],
}


def source_hashes():
    paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'config/b1.json', ROOT / 'config/b3-1.json',
             ROOT / 'docs/checklists/B1-acceptance.json',
             *ROOT.glob('config/**/*.json'), *ROOT.glob('examples/asset-host/*.json'), *ROOT.glob('examples/result-host/*.json'),
             *ROOT.glob('src/**/*.rs'), *ROOT.glob('src/**/*.json'), *ROOT.glob('adapters/node/*.mjs'), *ROOT.glob('scripts/*.py')]
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(paths)}


def run(name, command, *, payload=None, cwd=ROOT):
    result = subprocess.run(command, input=payload, cwd=cwd, text=True, capture_output=True,
                            timeout=240 if command[:2] == ['cargo', 'test'] else 120,
                            env={**os.environ, 'CARGO_TERM_COLOR': 'never'})
    (LOGS / f'{name}.log').write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f'{name}: {result.returncode}\n{result.stdout[-10000:]}\n{result.stderr[-10000:]}')
    print(f'{name}: passed', flush=True)
    return result.stdout


def passed_rust_tests(output):
    tests = set(re.findall(r'^test (\S+) \.\.\. ok$', output, re.M))
    assert tests, 'No executed Rust acceptance tests found'
    return tests


def node_case(item):
    name, flags = item
    command = ['node', 'adapters/node/demo.mjs', *flags]
    result = json.loads(run(f'node-{name}', command))
    assert result['admissions'] == result['settlements'] == result['skillCommits'] == 1
    assert result['liveWorlds'] == result['pendingOperations'] == 0
    assert (result['warehouse'], result['cultivation']) == ((99, 3) if '--death' in flags else (102, 30))
    return name, {'command': command, 'result': result}


def wire_checks():
    binary = str(ROOT / 'target/debug/kernel-host')
    malformed = ['{', '{"type":"advance_world","milliseconds":30}',
                 '{"type":"drain_outputs","world":{"hp":1}}',
                 '{"type":"drain_outputs","type":"drain_outputs"}',
                 '{"type":"drain_outputs"}']
    output = run('wire-strict', [binary], payload='\n'.join(malformed) + '\n')
    replies = [json.loads(line) for line in output.splitlines()]
    assert replies[:-1] == [{'error': 'INVALID_MESSAGE'}] * 4, replies
    assert replies[-1] == {'ok': {'outputs': []}}, replies
    payload = json.dumps({'type': 'drain_outputs', 'padding': 'x' * 262145}) + '\n{"type":"drain_outputs"}\n'
    output = run('wire-oversized', [binary], payload=payload)
    assert [json.loads(line) for line in output.splitlines()] == [
        {'error': 'INVALID_MESSAGE'}, {'ok': {'outputs': []}}]
    return {'strict_schema_and_recovery': 'passed', 'oversized_line_recovery': 'passed'}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--date', default=datetime.now(ZoneInfo('Asia/Shanghai')).date().isoformat())
    parser.add_argument('--node-workers', type=int, choices=range(1, 4), default=3)
    args = parser.parse_args()
    LOGS.mkdir(parents=True, exist_ok=True)
    start_hashes = source_hashes()
    matrix = json.loads((ROOT / 'docs/checklists/B1-acceptance.json').read_text())
    assert set(matrix['criteria']) == {*(f'B1-K{i:02}' for i in range(1, 11)), 'B1-H01', 'B1-H02'}
    run('format', ['cargo', 'fmt', '--check'])
    default_tests = passed_rust_tests(run('rust-default', ['cargo', 'test', '--locked']))
    feature_tests = passed_rust_tests(run('rust-all-features', ['cargo', 'test', '--locked', '--all-features']))
    assert default_tests == feature_tests, 'Default/replay test coverage diverged'
    run('clippy', ['cargo', 'clippy', '--locked', '--all-targets', '--all-features', '--', '-D', 'warnings'])
    run('build', ['cargo', 'build', '--locked', '--features', 'replay', '--bin', 'kernel-host', '--bin', 'replay'])
    bundle = json.loads(run('source-bundle', [sys.executable, 'rust-match-kernel-v1.1/validate.py']))
    run('frozen-export', [sys.executable, 'scripts/export_b1.py', '--check'])
    native = {}
    with tempfile.TemporaryDirectory(prefix='b1-standalone-') as directory:
        shutil.copytree(ROOT / 'config', Path(directory) / 'config')
        for name, flags in [('extraction', []), ('death', ['--death'])]:
            result = json.loads(run(f'native-{name}', [str(ROOT / 'target/debug/replay'), *flags], cwd=directory))
            assert result['live_worlds'] == result['pending_operations'] == result['queued_outputs'] == 0
            assert result['config_hash'] == start_hashes['config/b1.json']
            native[name] = {'workingDirectory': 'isolated temporary directory with declared config package', 'result': result}
    wire = wire_checks()
    with ThreadPoolExecutor(max_workers=args.node_workers) as workers:
        node = dict(workers.map(node_case, NODE_CASES.items()))
    criteria = {}
    for name, required in matrix['criteria'].items():
        assert set(required.get('rustTests', [])) <= default_tests, (name, 'unexecuted Rust tests')
        assert set(required.get('nodeCases', [])) <= node.keys(), (name, 'unexecuted Node cases')
        assert set(required.get('wireChecks', [])) <= wire.keys(), (name, 'unexecuted wire checks')
        criteria[name] = {'status': 'passed', **required}
    assert start_hashes == source_hashes(), 'Candidate sources changed during verification'
    report = {
        'schemaVersion': 2, 'date': args.date, 'scope': matrix['scope'],
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'sourceSha256': start_hashes,
        'rust': {'defaultPassed': len(default_tests), 'allFeaturesPassed': len(feature_tests),
                 'uniqueTests': len(default_tests), 'executedTests': sorted(default_tests)},
        'clippy': 'passed with -D warnings', 'native': native, 'node': node,
        'wire': wire, 'sourceBundle': bundle, 'criteria': criteria,
        'notVerified': ['production map or monster equivalence', 'existing client', 'real database durability',
                        'production reconnect across process restart or host migration', 'production I/O deadline integration', 'release capacity or long soak'],
    }
    destination = ROOT / 'docs/evidence/b1-results.json'
    temporary = destination.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    temporary.replace(destination)
    print(json.dumps({'status': 'passed', 'rustTests': len(default_tests), 'nodeJourneys': len(node),
                      'wireChecks': len(wire), 'criteria': len(criteria), 'report': str(destination.relative_to(ROOT))}), flush=True)


if __name__ == '__main__':
    main()
