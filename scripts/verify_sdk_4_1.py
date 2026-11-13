"""Verify the shipped archive with isolated Cargo homes and caller-owned configuration/state."""
import hashlib
import argparse
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time

import package_sdk

ROOT = package_sdk.ROOT
OUTPUT = ROOT/'artifacts/sdk-4-1'


def sources():
    paths={ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'SDK.md',ROOT/'README.md',
           *ROOT.glob('src/**/*.rs'),*ROOT.glob('src/**/*.json'),*ROOT.glob('config/**/*.json'),
           *ROOT.glob('delivery/*'),*ROOT.glob('examples/minimal-host/src/*.rs'),
           ROOT/'examples/minimal-host/Cargo.toml',ROOT/'examples/minimal-host/Cargo.lock',
           ROOT/'examples/minimal-host/account.json',*ROOT.glob('examples/asset-host/*.json'),
           *ROOT.glob('examples/result-host/*.json'),ROOT/'adapters/python/asset_host.py',
           Path(__file__),ROOT/'scripts/package_sdk.py',ROOT/'docs/guides/sdk-delivery.md',
           ROOT/'docs/batches/SDK-4-1-delivery.md'}
    return {str(p.relative_to(ROOT)):package_sdk.digest(p) for p in sorted(paths) if p.is_file()}


def run(name, command, *, cwd, env=None, expected=0, payload=None):
    process_env={**os.environ,'CARGO_TERM_COLOR':'never'}
    process_env.pop('MATCH_CONFIG_MANIFEST',None)
    process_env.update(env or {})
    start=time.monotonic()
    p=subprocess.run(command,cwd=cwd,env=process_env,input=payload,text=True,capture_output=True,timeout=600)
    (OUTPUT/f'{name}.log').write_text(p.stdout+p.stderr)
    assert p.returncode==expected,(name,p.returncode,p.stdout[-1500:],p.stderr[-4000:])
    print(f'{name}: passed ({time.monotonic()-start:.1f}s)',flush=True)
    return p.stdout


def extract(archive,directory):
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            parts=PurePosixPath(member.name).parts
            assert parts and not PurePosixPath(member.name).is_absolute() and '..' not in parts
            path=directory.joinpath(*parts)
            if member.isdir():path.mkdir(parents=True,exist_ok=True)
            else:
                assert member.isfile(),member.name
                path.parent.mkdir(parents=True,exist_ok=True)
                with tar.extractfile(member) as source:path.write_bytes(source.read())
                path.chmod(member.mode&0o777)


def caller_config(base,destination,protocol):
    destination.mkdir()
    raw=json.loads((base/'arena.json').read_text())
    raw['id']=f'independent-consumer-protocol-{protocol}'
    if protocol>1:
        raw['asset_contract']={'entry_cost_asset_id':'entry-pass','reward_asset_id':'rank-points'}
        raw['extraction_unlock_ms']=3000
        raw['extraction_channel_ms']=500
    if protocol==3:
        raw['result_contract']['extraction']['result_id']='returned'
        raw['result_contract']['death']['result_id']='finished'
    (destination/'arena.json').write_text(json.dumps(raw))
    (destination/'catalog.json').write_text(json.dumps({'schema_version':1,'default_scenario':'caller-owned',
                                                      'active':{'caller-owned':'arena.json'},'archives':[]}))
    account=json.loads((base/'account.json').read_text())
    if protocol==1:
        account['facts'].update(hp_points=1200,warehouse_quantity=137)
    else:account['assets']={'entry-pass':100,'rank-points':0,'xuan-tie':100}
    (destination/'account.json').write_text(json.dumps(account))
    return raw,account


def stored(path):
    import sqlite3
    db=sqlite3.connect(path)
    try:
        assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
        return json.loads(db.execute('SELECT state FROM host WHERE id=1').fetchone()[0])
    finally:db.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reuse-native',action='store_true',help='reuse native proof only for identical crate bytes and compiler')
    args=parser.parse_args()
    OUTPUT.mkdir(parents=True,exist_ok=True)
    frozen=sources()
    built=package_sdk.build(OUTPUT)
    assert sources()==frozen,'Sources changed during packaging'
    results={}
    with tempfile.TemporaryDirectory(prefix='sdk-delivery-consumer-') as directory:
        work=Path(directory)
        extract(built['archive'],work)
        bundle=work/built['rootName']
        run('file-check',[sys.executable,'verify_bundle.py'],cwd=bundle)
        manifest=json.loads((bundle/'distribution.json').read_text())
        sdk=bundle/'sdk'
        entries=[p.relative_to(sdk).as_posix() for p in sdk.rglob('*') if p.is_file()]
        assert all(not p.startswith(('target/','artifacts/','.local/','rust-match-kernel-v1.1/'))
                   and '.DS_Store' not in p for p in entries)
        assert (sdk/'Cargo.toml.orig').read_bytes()==(ROOT/'Cargo.toml').read_bytes()
        for file in (ROOT/'src').rglob('*'):
            if file.is_file() and file.suffix in ['.rs','.json']:
                assert (sdk/file.relative_to(ROOT)).read_bytes()==file.read_bytes(),file
        assert str(ROOT) not in (bundle/'consumer/Cargo.toml').read_text()
        assert str(ROOT) not in (bundle/'.cargo/config.toml').read_text()
        assert len(manifest['dependencies'])>0

        native_env={'CARGO_HOME':str(work/'native-cargo-home'),'CARGO_NET_OFFLINE':'true',
                    'CARGO_TARGET_DIR':str(work/'native-target')}
        Path(native_env['CARGO_HOME']).mkdir()
        proof_path=OUTPUT/'native-proof.json'
        if args.reuse_native:
            proof=json.loads(proof_path.read_text())
            assert proof['sourceCrateSha256']==built['sourceCrateSha256'] and proof['rustc']==manifest['rustc']
            for name,sha in proof['logHashes'].items():assert package_sdk.digest(OUTPUT/name)==sha,name
            tests=proof['tests']
        else:
            tests={}
            for scope,flags in [('default',[]),('all-features',['--all-features'])]:
                out=run('package-tests-'+scope,['cargo','test','--offline','--locked',*flags],cwd=sdk,env=native_env)
                tests[scope]=sorted(re.findall(r'^test (\S+) \.\.\. ok$',out,re.M))
            run('package-clippy',['cargo','clippy','--offline','--locked','--all-targets','--all-features','--','-D','warnings'],cwd=sdk,env=native_env)
            proof={'sourceCrateSha256':built['sourceCrateSha256'],'rustc':manifest['rustc'],'tests':tests,
                   'logHashes':{name:package_sdk.digest(OUTPUT/name) for name in
                       ['package-tests-default.log','package-tests-all-features.log','package-clippy.log']}}
            proof_path.write_text(json.dumps(proof,indent=2)+'\n')
        assert len(tests['default'])==643 and tests['default']==tests['all-features']

        # A second copy removes all SDK regression catalogs and sample data before compiling.
        runtime=work/'runtime-copy'
        shutil.copytree(bundle,runtime)
        shutil.rmtree(runtime/'sdk/config')
        shutil.rmtree(runtime/'sdk/examples')
        shutil.rmtree(runtime/'samples')
        owned=work/'caller-config'
        owned.mkdir()
        configurations={}
        for protocol,name in [(1,'legacy'),(2,'assets'),(3,'results')]:
            raw,account=caller_config(bundle/'samples'/name,owned/name,protocol)
            configurations[protocol]=(owned/name,raw,account)
        run_dir=work/'empty-working-directory'
        run_dir.mkdir()
        runtime_env={'CARGO_HOME':str(work/'consumer-cargo-home'),'CARGO_NET_OFFLINE':'true',
                     'CARGO_TARGET_DIR':str(work/'consumer-target'),'MATCH_CONFIG_MANIFEST':str(work/'not-a-catalog.json')}
        Path(runtime_env['CARGO_HOME']).mkdir()
        run('consumer-build',['cargo','build','--offline','--locked','--manifest-path','consumer/Cargo.toml'],cwd=runtime,env=runtime_env)
        metadata=json.loads(run('consumer-metadata',['cargo','metadata','--offline','--locked','--format-version','1',
                             '--manifest-path','consumer/Cargo.toml'],cwd=runtime,env=runtime_env))
        assert all(Path(p['manifest_path']).resolve().is_relative_to(runtime.resolve()) for p in metadata['packages'])
        run('consumer-clippy',['cargo','clippy','--offline','--locked','--manifest-path','consumer/Cargo.toml','--','-D','warnings'],cwd=runtime,env=runtime_env)
        client=work/'consumer-target/debug/minimal-match-host'
        from verify_sdk_2 import verify_result
        cfg,raw,account=configurations[1]
        for mode in ['normal','unknown-settlement','refused-admission','close-before-ready','failed-publication','config-mismatch']:
            value=json.loads(run('rust-'+mode,[str(client),str(cfg/'catalog.json'),str(cfg/'account.json'),mode],cwd=run_dir,env=runtime_env))
            verify_result(value,mode,raw,account)
            assert value['configHash']==package_sdk.digest(cfg/'arena.json')
            results['rust-'+mode]=value

        binary=runtime/'bin/kernel-host'
        info=json.loads(run('info-without-config',[str(binary),'--sdk-info'],cwd=run_dir,env=runtime_env))
        assert info==json.loads((bundle/'support.json').read_text())==manifest['support']
        assert info['crateVersion']=='0.3.1' and not info['clocks']['replay']
        assert info['configSchemaProtocols']==[{'schemaVersion':1,'protocolVersion':1},{'schemaVersion':2,'protocolVersion':1},
                                                {'schemaVersion':3,'protocolVersion':2},{'schemaVersion':4,'protocolVersion':3}]
        cfg3=configurations[3][0]
        run('ordinary-host-rejects-replay',[str(binary),'--configs',str(cfg3/'catalog.json'),'--verification'],cwd=run_dir,env=runtime_env,expected=2)
        missing=subprocess.run([str(binary),'--configs',str(work/'missing.json')],cwd=run_dir,env={**os.environ,**runtime_env},capture_output=True,text=True)
        assert missing.returncode!=0 and not missing.stdout
        wire=run('wire-strict',[str(binary),'--configs',str(cfg3/'catalog.json')],cwd=run_dir,env=runtime_env,
                 payload='{"type":"drain_outputs","extra":1}\n{"type":"drain_outputs"}\n')
        assert [json.loads(line) for line in wire.splitlines()]==[{'error':'INVALID_MESSAGE'},{'ok':{'outputs':[]}}]
        wire=run('wire-oversized',[str(binary),'--configs',str(cfg3/'catalog.json')],cwd=run_dir,env=runtime_env,
                 payload='x'*262145+'\n{"type":"drain_outputs"}\n')
        assert [json.loads(line) for line in wire.splitlines()]==[{'error':'INVALID_MESSAGE'},{'ok':{'outputs':[]}}]
        create={'protocol_version':2,'key':{'id':'package-protocol-check','incarnation':1},'request_id':'create',
                'config_id':configurations[3][1]['id'],'config_hash':package_sdk.digest(cfg3/'arena.json'),
                'seed':7,'participant_id':'A','authenticated_principal':'principal-A','connection':{'id':'conn','generation':1}}
        wire=run('wire-version',[str(binary),'--configs',str(cfg3/'catalog.json')],cwd=run_dir,env=runtime_env,
                 payload=json.dumps({'type':'create','request':create})+'\n{"type":"drain_outputs"}\n')
        assert [json.loads(line) for line in wire.splitlines()]==[{'error':'VERSION_MISMATCH'},{'ok':{'outputs':[]}}]

        # The deployed host is the copy from the archive, with no repository imports or fast clock.
        def host(name,state,*,protocol=3,mode='death',flags=(),crash=False):
            cfg=configurations[protocol][0]
            command=[sys.executable,str(runtime/'hosts/asset_host.py'),'--binary',str(binary),
                     '--configs',str(cfg/'catalog.json'),'--account',str(cfg/'account.json'),
                     '--state',str(state),'--mode',mode,*flags]
            out=run(name,command,cwd=run_dir,env={**runtime_env,'PYTHONPATH':''},expected=-9 if crash else 0)
            if crash:return None
            value=json.loads(out)
            state_value=stored(state)
            from verify_sdk_3_1 import verify_assets
            verify_assets(value,state_value,pending=(mode=='refused-settlement'))
            assert state_value['create']['protocol_version']==protocol
            results[name]=value
            return value
        for protocol in [2,3]:
            for mode in ['death','extraction']:
                host(f'jsonl-{protocol}-{mode}',work/f'{protocol}-{mode}.sqlite',protocol=protocol,mode=mode)
        for mode in ['unknown-settlement','retry-settlement','close-before-ready','active-close']:
            host(mode,work/(mode+'.sqlite'),mode=mode)
        state=work/'review.sqlite'
        host('refused',state,mode='refused-settlement')
        final=host('reviewed',state,flags=['--reconcile'])
        repeat=host('review-repeat',state)
        assert final['counts']==repeat['counts'] and final['balances']==repeat['balances']
        crashes=[]
        for point,kind in [('before','settlement'),('after','settlement'),('after','active')]:
            state=work/f'{point}-{kind}.sqlite'
            host(f'crash-{point}-{kind}',state,flags=[f'--crash-{point}',kind],crash=True)
            before=stored(state)
            if point=='before':
                current=json.loads((cfg3/'arena.json').read_text())
                current['result_contract']['death']={'result_id':'new-result','policy':'preserve'}
                (cfg3/'arena.json').write_text(json.dumps(current))
            value=host(f'recover-{point}-{kind}',state)
            again=host(f'repeat-{point}-{kind}',state)
            assert value['counts']==again['counts'] and value['balances']==again['balances']
            assert value['configHash']==before['create']['config_hash']
            if kind=='settlement':
                original=next(r for r in before['requests'].values() if r['body'].get('transaction',{}).get('kind')=='result_settlement')
                assert value['requests'][original['operation_id']]['body']==original['body']
            else:assert value['balances']==before['initial']['assets'] and value['counts']['compensation']==1
            crashes.append(f'{point}-{kind}')
        # Integrity checker must reject both a modified and a missing delivery file.
        support=bundle/'support.json'
        original=support.read_bytes()
        support.write_bytes(original+b' ')
        run('tamper-rejected',[sys.executable,'verify_bundle.py'],cwd=bundle,expected=1)
        support.write_bytes(original)
        support.unlink()
        run('missing-file-rejected',[sys.executable,'verify_bundle.py'],cwd=bundle,expected=1)
        support.write_bytes(original)
        run('pristine-check-restored',[sys.executable,'verify_bundle.py'],cwd=bundle)
        assert sources()==frozen,'Sources changed during delivery acceptance'
        report={'passed':True,'scope':'local delivery, isolated offline consumers, ordinary clock and package-external recovery',
                'sourceHashes':frozen,'package':built,'manifest':manifest,'support':info,'rustTestCount':len(tests['default']),
                'sameTestsUnderAllFeatures':tests['default']==tests['all-features'],'rustTests':tests['default'],
                'consumerResults':results,'crashBoundaries':crashes,'freshCargoHomes':True,
                'removedSdkRegressionData':True,'offlineBuild':True,'wireChecks':['strict','oversized','version'],
                'reusedNative':args.reuse_native,'nativeProofSha256':package_sdk.digest(proof_path),
                'integrityChecks':['pristine','tamper rejection','missing rejection']}
        (OUTPUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'SDK-4-1 passed: {len(results)} consumer outcomes, 643 native tests, isolated offline delivery')


if __name__=='__main__':main()
