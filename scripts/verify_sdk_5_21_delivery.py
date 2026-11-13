"""Consume the exact release archive outside the checkout; empty Cargo home and offline source checks."""
import concurrent.futures
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
from playwright.sync_api import sync_playwright

from package_sdk import ROOT, digest, extract_crate
from verify_sdk_2 import verify_result
from verify_sdk_5_16_delivery import balances

OUT=ROOT/'artifacts/sdk-5-21/delivery'
def names(text):return sorted(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M))
def run(name,command,cwd,env=None):
    p=subprocess.run(command,cwd=cwd,env={**os.environ,'CARGO_TERM_COLOR':'never','RUST_TEST_THREADS':'16',**(env or {})},capture_output=True,text=True,timeout=1200)
    (OUT/(name+'.log')).write_text(p.stdout+p.stderr)
    assert p.returncode==0,(name,p.returncode,p.stderr[-3000:],p.stdout[-2000:])
    print(name+': passed',flush=True);return p.stdout

def main():
    package=json.loads((OUT/'package.json').read_text());assert digest(package['archive'])==package['sha256']
    expected=json.loads((OUT.parent/'expected-tests.json').read_text());assert len(expected)==1275
    with tempfile.TemporaryDirectory(prefix='sdk-5-21-independent-') as temp:
        work=Path(temp);bundle=work/'bundle';extract_crate(package['archive'],bundle,package['rootName'])
        run('independent-integrity',[sys.executable,'verify_bundle.py'],bundle)
        source_hashes={str(p.relative_to(ROOT)):digest(p) for p in ROOT.glob('src/**/*.rs')}
        for p,h in source_hashes.items():assert digest(bundle/'sdk'/p)==h,p
        home=work/'empty-cargo-home';home.mkdir();env={'CARGO_HOME':str(home),'CARGO_NET_OFFLINE':'true','CARGO_TARGET_DIR':str(work/'target')}
        source={}
        for label,flags in [('default',[]),('all-features',['--all-features'])]:
            text=run('source-'+label,['cargo','test','--offline','--locked','--manifest-path','sdk/Cargo.toml',*flags],bundle,env)
            assert names(text)==expected;source[label]=digest(OUT/('source-'+label+'.log'))
        run('source-clippy',['cargo','clippy','--offline','--locked','--manifest-path','sdk/Cargo.toml','--all-targets','--all-features','--','-D','warnings'],bundle,env)
        source['clippy']=digest(OUT/'source-clippy.log')
        run('consumer-build',['cargo','build','--offline','--locked','--manifest-path','consumer/Cargo.toml'],bundle,env)
        own=work/'outside';own.mkdir()
        for kind in ['legacy','assets','results','two-members','members','fields','projectile-repeat','line-fields','delta','multiplayer-periodic']:
            shutil.copytree(bundle/'samples'/kind,own/kind)
        for kind in ['assets','results']:
            path=own/kind/'arena.json';v=json.loads(path.read_text());v.update(extraction_unlock_ms=3000,extraction_channel_ms=500);path.write_text(json.dumps(v))
        # Tune only copied data for a bounded ordinary-clock old-mechanism journey.
        node_profiles=[]
        for family in ['fields','projectile-repeat','line-fields']:
            for variant in (['a','b'] if family!='fields' else ['a']):
                folder=own/(family+'-'+variant);shutil.copytree(own/family,folder)
                path=folder/'arena.json';v=json.loads(path.read_text());v['id']='external-'+family+'-'+variant;v['pve']['combat']['heal']['hp_threshold']=0
                if family=='line-fields':
                    v['skill'].update(warning_ms=0,windup_ms=120,cooldown_ms=1800,tick_interval_ms=300,duration_ms=1500,length=260,width=32,contact_policy={'max_targets_per_tick':3,'max_total_hits':3,'repeat_interval_ms':0})
                    if variant=='b':v['skill'].update(width=52,contact_policy={'max_targets_per_tick':1,'max_total_hits':4,'repeat_interval_ms':300});v['skill'],v['additional_skills'][0]=v['additional_skills'][0],v['skill']
                elif family=='projectile-repeat':
                    v['skill'].update(repeat_interval_ms=0,speed=500,radius=6,ttl_ms=4000)
                    if variant=='b':v['skill'].update(repeat_interval_ms=600,speed=20,radius=42,ttl_ms=22000);v['skill'],v['additional_skills'][0]=v['additional_skills'][0],v['skill']
                path.write_text(json.dumps(v));node_profiles.append(folder)
        # Source regression data is unavailable throughout ordinary public consumption.
        hidden=[]
        for path in [bundle/'sdk/config',bundle/'sdk/examples',bundle/'samples']:
            destination=work/('hidden-'+path.name);path.rename(destination);hidden.append((path,destination))
        binary=str(bundle/'bin/kernel-host');invalid={'MATCH_CONFIG_MANIFEST':str(work/'missing-default.json')}
        info=json.loads(run('independent-info',[binary,'--sdk-info'],own,invalid))
        assert info['crateVersion']=='0.23.0' and info['protocolVersions']==[1,2,3,4,5,6] and info['mechanismVersions']['max']==38 and not info['clocks']['replay']
        assert digest(binary)==package['binarySha256']
        jobs=[]
        for mode in ['normal','unknown-settlement','refused-admission','close-before-ready','failed-publication','config-mismatch']:
            def legacy(mode=mode):
                value=json.loads(run('protocol1-'+mode,[str(work/'target/debug/minimal-match-host'),str(own/'legacy/catalog.json'),str(own/'legacy/account.json'),mode],own,invalid))
                verify_result(value,mode,json.loads((own/'legacy/arena.json').read_text()),json.loads((own/'legacy/account.json').read_text()));return 'protocol1-'+mode,value
            jobs.append(legacy)
        for kind in ['assets','results']:
            for mode in ['death','extraction','unknown-settlement','retry-settlement','admission-refused','close-before-ready','refused-settlement']:
                def old(kind=kind,mode=mode):
                    label=kind+'-'+mode;command=[sys.executable,str(bundle/'hosts/asset_host.py'),'--binary',binary,'--configs',str(own/kind/'catalog.json'),'--account',str(own/kind/'account.json'),'--state',str(own/(label+'.sqlite')),'--mode',mode]
                    value=json.loads(run(label,command,own,invalid))
                    if mode=='refused-settlement':assert value['pending'] and value['counts']['settlement']==0;value=json.loads(run(label+'-reconcile',command+['--reconcile'],own,invalid))
                    assert value['phase']=='closed' and value['liveWorlds']==0 and not value['pending']
                    expected={'admission':0,'settlement':0,'compensation':0} if mode=='admission-refused' else {'admission':1,'settlement':0,'compensation':1} if mode=='close-before-ready' else {'admission':1,'settlement':1,'compensation':0}
                    assert value['counts']==expected;balances(value,json.loads((own/kind/'account.json').read_text()))
                    again=json.loads(run(label+'-restart',command,own,invalid))
                    for key in ['balances','counts','requests','configHash']:assert again[key]==value[key]
                    assert not again['pending'];return label,value
                jobs.append(old)
        for folder in node_profiles:
            for mode in ['death','extraction']:
                def old_node(folder=folder,mode=mode):
                    command=['node',str(bundle/'hosts/node/asset-host.mjs'),'--binary',binary,'--configs',str(folder/'catalog.json'),'--account',str(folder/'account.json')]
                    if mode=='extraction':command.append('--extraction')
                    value=json.loads(run('old-node-'+folder.name+'-'+mode,command,own,invalid));assert value['admissions']==value['settlements']==1 and value['statusApplications']>0 and value['clock']=='ordinary'
                    if folder.name.startswith('line-fields'):assert value['fieldStarts'] and any(e.get('line') for e in value['fieldStarts'])
                    if folder.name.startswith('projectile-repeat'):assert value['piercingHits'] and value['piercingEnds']
                    return 'old-node-'+folder.name+'-'+mode,value
                jobs.append(old_node)
        for scenario,account,reverse in [('pair-a','accounts-a.json',False),('pair-b','accounts-b.json',True)]:
            def duo(scenario=scenario,account=account,reverse=reverse):
                command=['node',str(bundle/'hosts/node/two-member-host.mjs'),'--binary',binary,'--configs',str(own/'two-members/catalog.json'),'--accounts',str(own/'two-members'/account),'--scenario',scenario,'--database',str(own/(scenario+'.sqlite'))]
                if reverse:command.append('--reverse')
                value=json.loads(run('two-'+scenario,command,own,invalid));accounts=json.loads((own/'two-members'/account).read_text());assert value['admissionOrder'][0]==accounts['participants'][1 if reverse else 0]['participant_id'];assert value['phase']=='closed' and value['pending']==0 and value['worlds']==1 and all(m['admissions']==m['settlements']==1 for m in value['members']);return 'two-'+scenario,value
            jobs.append(duo)
        for scenario,account,reverse in [('four','accounts-a.json',False),('eight','accounts-b.json',True)]:
            def members(scenario=scenario,account=account,reverse=reverse):
                command=['node',str(bundle/'hosts/node/member-host.mjs'),'--binary',binary,'--configs',str(own/'members/catalog.json'),'--accounts',str(own/'members'/account),'--scenario',scenario,'--database',str(own/(scenario+'.sqlite'))]
                if reverse:command.append('--reverse')
                value=json.loads(run('members-'+scenario,command,own,invalid));accounts=json.loads((own/'members'/account).read_text());order=[a['participant_id'] for a in accounts['participants']]
                assert value['admissionOrder']==(list(reversed(order)) if reverse else order)
                assert value['phase']=='closed' and value['pending']==0 and value['worlds']==1 and value['capacity']==len(order) and len(value['refused'])==len(order)-1 and all(m['admissions']==m['settlements']==1 for m in value['accounts'])
                return 'members-'+scenario,value
            jobs.append(members)
        for scenario,account,reverse in [('four','accounts-a.json',False),('eight','accounts-b.json',True)]:
            def delta(scenario=scenario,account=account,reverse=reverse):
                command=['node',str(bundle/'hosts/node/delta-host.mjs'),'--binary',binary,'--configs',str(own/'delta/catalog.json'),'--accounts',str(own/'delta'/account),'--scenario',scenario,'--database',str(own/('delta-'+scenario+'.sqlite'))]
                if reverse:command.append('--reverse')
                value=json.loads(run('delta-'+scenario,command,own,invalid));r=value['replication']
                assert value['phase']=='closed' and value['pending']==0 and r['full']>0 and r['delta']>0 and r['bytes']<r['fullEquivalentBytes'] and r['expired']==0
                assert all(a['admissions']==a['settlements']==1 for a in value['accounts'])
                return 'delta-'+scenario,value
            jobs.append(delta)
        for scenario,account,reverse in [('four','accounts-a.json',False),('eight','accounts-b.json',True)]:
            def periodic(scenario=scenario,account=account,reverse=reverse):
                command=['node',str(bundle/'hosts/node/multiplayer-periodic-host.mjs'),'--binary',binary,'--configs',str(own/'multiplayer-periodic/catalog.json'),'--accounts',str(own/'multiplayer-periodic'/account),'--scenario',scenario,'--database',str(own/('periodic-'+scenario+'.sqlite'))]
                if reverse:command.append('--reverse')
                value=json.loads(run('periodic-'+scenario,command,own,invalid));r=value['replication'];m=value['periodic']
                assert value['phase']=='closed' and value['pending']==0 and value['retiredSourceStopped'] and value['reconnectPreserved'] and value['cacheResynced']
                assert r['delta']>0 and r['expired']==0 and all(a['admissions']==a['settlements']==1 for a in value['accounts'])
                assert all(any(s[k]>0 for s in m) for k in ['incomingDotTicks','outgoingDotTicks','selfHotTicks','npcHotTicks','hpRestored'])
                return 'periodic-'+scenario,value
            jobs.append(periodic)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:results=dict(pool.map(lambda job:job(),jobs))
        # Use delivered H5 assets and explicit outside data while SDK fixtures are hidden.
        with (OUT/'independent-h5.log').open('w') as log:
            server=subprocess.Popen(['node',str(bundle/'hosts/h5/multiplayer-periodic-server.mjs'),'--binary',binary,'--configs',str(own/'multiplayer-periodic/catalog.json'),'--host-config',str(own/'multiplayer-periodic/host.json'),'--database',str(own/'h5.sqlite'),'--port','4200'],cwd=own,stdout=log,stderr=subprocess.STDOUT)
            try:
                with sync_playwright() as pw:
                    browser=pw.chromium.launch();page=browser.new_page();errors=[];methods=[]
                    page.on('pageerror',lambda e:errors.append(str(e)))
                    page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None)
                    page.on('request',lambda r:methods.append(r.method))
                    deadline=time.monotonic()+15
                    while True:
                        try:page.goto('http://127.0.0.1:4200/',wait_until='networkidle');break
                        except Exception:
                            assert server.poll() is None and time.monotonic()<deadline;time.sleep(.1)
                    info_h5=page.request.get('http://127.0.0.1:4200/api/config').json();assert info_h5['sdk']['crateVersion']=='0.23.0' and len(info_h5['scenarios'])==2
                    assert page.locator('#create').is_visible();assert page.request.get('http://127.0.0.1:4200/replication.mjs').text()==(bundle/'web/replication.mjs').read_text()
                    assert not errors and set(methods)=={'GET'};browser.close()
                    h5={'passed':True,'outsideCheckout':True,'sourceFixturesHidden':True,'createdMatch':False,'browserErrors':errors,'methods':methods}
            finally:
                server.terminate();server.wait(timeout=10)
        for path,backup in hidden:backup.rename(path)
        run('independent-final-integrity',[sys.executable,'verify_bundle.py'],bundle)
        source_report={'passed':True,'archiveSha256':package['sha256'],'emptyCargoHome':True,'offline':True,'testCount':len(expected),'sameWorkspaceDefaultAndAllFeatureSet':True,'checks':source}
        (OUT/'source-verification.json').write_text(json.dumps(source_report,indent=2)+'\n')
        report={'passed':True,'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],'binarySha256':package['binarySha256'],'support':info,'rustSourceHashes':source_hashes,'removedSdkRegressionsBeforeConsumption':True,'outsideCheckout':True,'emptyCargoHome':True,'offline':True,'cases':results,'h5':h5}
        (OUT/'independent-verification.json').write_text(json.dumps(report,indent=2)+'\n')
    candidate=OUT.parent/'candidate';candidate.mkdir(exist_ok=True);extract_crate(package['archive'],candidate,package['rootName'])
    print(json.dumps({'passed':True,'tests':len(expected),'independentCases':len(results),'binary':str(candidate/'bin/kernel-host')}),flush=True)

if __name__=='__main__':main()
