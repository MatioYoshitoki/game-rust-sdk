"""SDK-3-2: configured result identity, independent policies and versioned frozen responsibility."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

import verify_sdk_3_1 as assets

ROOT = assets.ROOT
OUTPUT = ROOT / 'artifacts/sdk-3-2'
BASE = ROOT / 'examples/result-host'


def sources():
    result = assets.sources()
    path = ROOT / 'docs/batches/SDK-3-2-results.md'
    result[str(path.relative_to(ROOT))] = assets.digest(path)
    return result


def run(name, command, expected=0):
    start = time.monotonic()
    p = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=1800)
    (OUTPUT / f'{name}.log').write_text(p.stdout + p.stderr)
    assert p.returncode == expected, (name, p.returncode, p.stdout[-1000:], p.stderr[-4000:])
    print(f'{name}: passed ({time.monotonic()-start:.1f}s)', flush=True)
    return p.stdout


def validate_result(value, state, pending=False):
    assets.verify_assets(value, state, pending=pending)
    assert state['create']['protocol_version'] == 3
    config = json.loads(state['config_raw'])
    for request in state['requests'].values():
        transaction = request['body'].get('transaction', {})
        if transaction.get('kind') in ['settlement', 'result_settlement']:
            assert transaction['kind'] == 'result_settlement'
            summary = transaction['summary']
            assert 'outcome' not in summary
            assert summary['result'] in list(config['result_contract'].values())
    for event in value['events']:
        assert event['kind'] not in ['asset_settlement_applied', 'settlement_applied']
        if event['kind'] == 'result_settlement_applied':
            assert 'outcome' not in event['summary']
    if value['counts']['settlement']:
        request = next(r for r in state['requests'].values() if r['body'].get('transaction', {}).get('kind') == 'result_settlement')
        policy = request['body']['transaction']['summary']['result']['policy']
        expected = {'entry-credit':99, 'experience':30 if policy=='preserve' else 3,
                    'xuan-tie':102 if policy=='preserve' else 99}
        assert value['balances'] == expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reuse-legacy', action='store_true', help='reuse legacy report only if every behavior input hash still matches')
    parser.add_argument('--reuse-b1', action='store_true', help='reuse matching native/B1 proof while rerunning legacy integration')
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    frozen = sources()
    prior = json.loads((ROOT / 'artifacts/sdk-3-1/verification.json').read_text())
    original_paths = ['config/b1.json', *[f'config/b3-{i}.json' for i in range(1,20)],
                      *[f'examples/asset-host/{p}.json' for p in ['arena','catalog','account']]]
    original_hashes = {p:assets.digest(ROOT / p) for p in original_paths}
    assert all(prior['sourceHashes'][p] == h for p,h in original_hashes.items())
    run('format', ['cargo','fmt','--all','--','--check'])
    run('build', ['cargo','build','--locked','--features','replay','--bin','kernel-host'])
    binary = ROOT / 'target/debug/kernel-host'
    cases = {}
    crashes = []
    with tempfile.TemporaryDirectory(prefix='sdk-3-2-') as tmp:
        directory = Path(tmp)

        def host(name, state, *, mode='death', flags=(), config=BASE/'catalog.json', ordinary=False, crash=False):
            command = [sys.executable,'adapters/python/asset_host.py','--binary',str(binary),
                       '--configs',str(config),'--account',str(ROOT/'examples/asset-host/account.json'),
                       '--state',str(state),'--mode',mode,*flags]
            if not ordinary:
                command.append('--verification')
            output = run(name,command,expected=-9 if crash else 0)
            if crash:
                assert json.loads(output)['crash'] in ['before','after']
                crashes.append(name)
                return None
            value = json.loads(output)
            stored = assets.read_state(state)
            validate_result(value,stored,pending=(mode=='refused-settlement'))
            value['protocolVersion'] = stored['create']['protocol_version']
            value['clock'] = 'ordinary' if ordinary else 'verification'
            cases[name] = value
            return value

        def result(value):
            return next(e['summary']['result'] for e in value['events'] if e['kind']=='result_settlement_applied')

        for mode in ['death','extraction','active-close','unknown-settlement','retry-settlement','admission-refused','close-before-ready']:
            value=host(mode,directory/(mode+'.sqlite'),mode=mode)
            if value['counts']['settlement']:
                assert result(value)['result_id'] == ('secured' if mode=='extraction' else 'defeated')
            if mode in ['close-before-ready','admission-refused']:
                assert value['balances']=={'entry-credit':100,'experience':0,'xuan-tie':100}
        path=directory/'review.sqlite'
        host('refused-settlement',path,mode='refused-settlement')
        reviewed=host('review-original',path,flags=['--reconcile'])
        repeated=host('review-again',path)
        assert reviewed['balances']==repeated['balances'] and reviewed['counts']==repeated['counts']

        for point,kind in [('before','settlement'),('after','settlement'),('after','admission'),
                           ('after','active'),('before','compensation'),('after','compensation')]:
            path=directory/f'crash-{point}-{kind}.sqlite'
            mode='close-before-ready' if kind=='compensation' else 'death'
            host(f'crash-{point}-{kind}',path,mode=mode,flags=[f'--crash-{point}',kind],crash=True)
            before=assets.read_state(path)
            recovered=host(f'recover-{point}-{kind}',path)
            repeated=host(f'repeat-{point}-{kind}',path)
            assert recovered['counts']==repeated['counts'] and recovered['balances']==repeated['balances']
            if kind=='settlement':
                original=next(r for r in before['requests'].values() if r['body'].get('transaction',{}).get('kind')=='result_settlement')
                assert recovered['requests'][original['operation_id']]['body']==original['body']
            else:
                assert recovered['counts']['compensation']==1 and recovered['balances']==before['initial']['assets']

        def package(name, extraction, death, *, rename=False):
            folder=directory/name;folder.mkdir()
            raw=json.loads((BASE/'arena.json').read_text())
            raw['id']=name+'-v1'
            for trigger,policy in [('extraction',extraction),('death',death)]:
                raw['result_contract'][trigger]['policy']=policy
                if rename:
                    raw['result_contract'][trigger]['result_id']='return-home' if trigger=='extraction' else 'session-ended'
            (folder/'arena.json').write_text(json.dumps(raw))
            (folder/'catalog.json').write_text(json.dumps({'schema_version':1,'default_scenario':'custom','active':{'custom':'arena.json'},'archives':[]}))
            return folder

        renamed=package('renamed','preserve','attrition',rename=True)
        for mode in ['death','extraction']:
            value=host('renamed-'+mode,directory/('renamed-'+mode+'.sqlite'),config=renamed/'catalog.json',mode=mode)
            assert result(value)['result_id']==('return-home' if mode=='extraction' else 'session-ended')
        for extraction,death in [('attrition','preserve'),('preserve','preserve'),('attrition','attrition')]:
            folder=package(extraction+'-'+death,extraction,death)
            for mode in ['extraction','death']:
                value=host(f'matrix-{extraction}-{death}-{mode}',directory/f'{extraction}-{death}-{mode}.sqlite',config=folder/'catalog.json',mode=mode)
                assert result(value)['policy']==(extraction if mode=='extraction' else death)

        path=directory/'mapping.sqlite'
        host('mapping-crash',path,config=renamed/'catalog.json',flags=['--crash-before','settlement'],crash=True)
        before=assets.read_state(path)
        raw=json.loads((renamed/'arena.json').read_text())
        raw['result_contract']['death']={'result_id':'new-session-result','policy':'preserve'}
        raw['result_contract']['extraction']={'result_id':'new-exit-result','policy':'attrition'}
        (renamed/'arena.json').write_text(json.dumps(raw))
        recovered=host('frozen-mapping-recovery',path,config=renamed/'catalog.json')
        repeated=host('frozen-mapping-repeat',path,config=renamed/'catalog.json')
        assert result(recovered)=={'result_id':'session-ended','policy':'attrition'}
        assert recovered['configHash']==before['create']['config_hash']
        assert recovered['balances']==repeated['balances'] and recovered['counts']==repeated['counts']
        host('ordinary-clock',directory/'ordinary.sqlite',ordinary=True)

    assert sources()==frozen,'Sources changed during result acceptance'
    (OUTPUT/'result-cases.json').write_text(json.dumps({'passed':True,'sourceHashes':frozen,'cases':cases,'crashes':crashes},indent=2)+'\n')
    legacy_path=ROOT/'artifacts/sdk-3-1/verification.json'
    if args.reuse_legacy:
        legacy=json.loads(legacy_path.read_text())
        for path,h in legacy['sourceHashes'].items():
            if path != str(Path(__file__).relative_to(ROOT)):
                assert assets.digest(ROOT/path)==h,('Legacy input changed; rerun baseline',path)
    else:
        run('legacy',[sys.executable,'scripts/verify_sdk_3_1.py', *(['--reuse-b1'] if args.reuse_b1 else [])])
        legacy=json.loads(legacy_path.read_text())
    assert legacy['passed'] and legacy['rustTestCount']>=643
    assert sources()==frozen,'Sources changed during regression'
    (OUTPUT/'verification.json').write_text(json.dumps({'passed':True,'scope':'SDK-3-2 registered result bindings and settlement policies',
        'sourceHashes':frozen,'rustTestCount':legacy['rustTestCount'],'newResultTests':17,'resultCases':cases,'crashChecks':crashes,
        'legacyReportSha256':assets.digest(legacy_path),'reusedLegacy':args.reuse_legacy,
        'originalConfigHashes':original_hashes,'binarySha256':assets.digest(binary)},indent=2)+'\n')
    print(f"SDK-3-2: {len(cases)} result outcomes, {len(crashes)} crash boundaries and {legacy['rustTestCount']} native tests passed")


if __name__=='__main__':
    main()
