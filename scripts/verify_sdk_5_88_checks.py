"""Verify the declared SDK-5-88 candidate and bind results to its source identity."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'artifacts/sdk-5-88'


def source_identity():
    files = [p for p in (ROOT / 'src').rglob('*') if p.is_file() and p.suffix in ('.rs', '.json')]
    files += list((ROOT / 'examples/combat-host').glob('*.json')) + [ROOT / 'SDK.md', ROOT / 'web/replication.mjs', ROOT / 'web/skill-hp-cost.js']
    data = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)}
    return hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest()


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    identity = source_identity()
    checks = [
        ('focused', ['cargo', 'test', '--offline', '--lib', 'numeric_tests']),
        ('source-cast-scenarios', ['cargo', 'test', '--offline', '--lib', 'source_cast_scenario']),
        ('terminal-counters', ['cargo', 'test', '--offline', '--lib', 'terminal_counter']),
        ('cast-interception', ['cargo', 'test', '--offline', '--lib', 'cast_interception']),
        ('source-execution', ['cargo', 'test', '--offline', '--lib', 'source_execution']),
        ('paid-resources', ['cargo', 'test', '--offline', '--lib', 'paid_resource']),
        ('conditional-activation', ['cargo', 'test', '--offline', '--lib', 'conditional_activation']),
        ('stack-windows', ['cargo', 'test', '--offline', '--lib', 'stack_counter']),
        ('source-gates', ['cargo', 'test', '--offline', '--lib', 'source_gate_']),
        ('derived-effects', ['cargo', 'test', '--offline', '--lib', 'derived_effects_tests']),
        ('character-build', ['cargo', 'test', '--offline', '--lib', 'character_build_']),
        ('resource-steal', ['cargo', 'test', '--offline', '--lib', 'resource_steal_']),
        ('resource-reservations', ['cargo', 'test', '--offline', '--lib', 'resource_reservation']),
        ('unit-effects', ['cargo', 'test', '--offline', '--lib', 'unit_effect']),
        ('skill-loadouts', ['cargo', 'test', '--offline', '--lib', 'skill_loadout']),
        ('fmt', ['cargo', 'fmt', '--all', '--check']),
        ('clippy', ['cargo', 'clippy', '--offline', '--all-targets', '--all-features', '--', '-D', 'warnings']),
        ('rust-default', ['cargo', 'test', '--offline']),
        ('rust-all-features', ['cargo', 'test', '--offline', '--all-features']),
        ('node', ['node', '--test', *[str(p.relative_to(ROOT)) for folder in ['adapters/node', 'adapters/h5'] for p in sorted((ROOT / folder).glob('*.test.mjs'))]]),
        ('build', ['cargo', 'build', '--offline', '--bin', 'kernel-host']),
    ]
    results = {}
    for name, command in checks:
        path = OUT / f'extended-{name}.log'
        with path.open('w') as output:
            result = subprocess.run(command, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT)
        print(f'{name}: exit {result.returncode}', flush=True)
        if result.returncode:
            raise RuntimeError(path.read_text()[-4000:])
        text = path.read_text()
        count = re.search(r'test result: ok\. (\d+) passed', text) if name != 'node' else re.search(r'# pass (\d+)', text)
        results[name] = {'passed': True, 'count': int(count[1]) if count else None}
    assert source_identity() == identity, 'runtime source changed during verification'
    assert results['rust-default']['count'] == results['rust-all-features']['count']
    report = {'passed': True, 'runtimeSourceSha256': identity, 'checks': results}
    (OUT / 'extended-checks.json').write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
