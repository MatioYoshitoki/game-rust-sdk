"""SDK-5-6: real-clock finite periodic healing, legacy consumers and durable host recovery."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from playwright.sync_api import sync_playwright
from verify_h5 import active, closed, wait
from verify_sdk_5_1 import Host, Journey, assert_settlement, digest

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT/'artifacts/sdk-5-6'


def sources():
    paths = [ROOT/'Cargo.toml', ROOT/'Cargo.lock', *ROOT.glob('src/**/*.rs'),
             *ROOT.glob('adapters/**/*.mjs'), *ROOT.glob('web/*'), *ROOT.glob('config/**/*.json'),
             *ROOT.glob('examples/*/*.json'), *ROOT.glob('examples/minimal-host/src/*.rs'),
             ROOT/'examples/minimal-host/Cargo.toml', ROOT/'examples/minimal-host/Cargo.lock',
             Path(__file__), ROOT/'scripts/verify_sdk_5_5.py', ROOT/'scripts/verify_sdk_5_4.py', ROOT/'scripts/verify_sdk_5_3.py', ROOT/'scripts/verify_sdk_5_2.py', ROOT/'scripts/verify_sdk_5_1.py', ROOT/'scripts/verify_h5.py']
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(set(paths)) if p.is_file()}


def package(directory, variant='a', *, legacy=False, touch=False):
    directory.mkdir()
    sample=ROOT/'examples'/('status-host' if legacy else 'hot-host')
    value=json.loads((sample/'arena.json').read_text())
    account=json.loads((sample/'account.json').read_text())
    value['id']=f'outside-{variant}-{"legacy" if legacy else "hot"}-{int(touch)}'
    if not legacy:
        # Leave a generous wall-clock observation window for browser reloads.
        # Exact 2200/600 timing remains exercised by native tests and the fixture.
        value['pve']['status_definitions'][0]['duration_ms']=6000
        if variant=='b':
            value['skill']['id']='outside-renewal-bolt'
            hot=value['pve']['status_definitions'][0]
            hot.update(id='outside-renewal',heal_bps=500,tick_interval_ms=700,duration_ms=2500,
                       duplicate_policy='ignore',source_scoped=False)
            value['skill']['on_commit_self_statuses']=['outside-renewal']
            value['asset_contract']={'entry_cost_asset_id':'pass','reward_asset_id':'merit'}
            account['assets']={'pass':100,'merit':0,'xuan-tie':100}
    (directory/'arena.json').write_text(json.dumps(value))
    (directory/'catalog.json').write_text(json.dumps({'schema_version':1,'default_scenario':'outside',
        'active':{'outside':'arena.json'},'archives':[]}))
    (directory/'account.json').write_text(json.dumps(account))
    return value


class StableJourney(Journey):
    def send(self, action, *, repeat=False):
        for _ in range(3):
            current=self.current()
            payload={'connection':current['connection'],'sequence':current['sequence']+1,
                     'requestId':f"hot-{current['sequence']+1}",'action':action}
            url=f"{self.host.url}/api/sessions/{current['id']}/input"
            response=self.page.request.post(url,data=payload)
            # An explicit stale-sequence rejection has not accepted this action.
            # Browser readiness may have advanced the sequence after our read.
            if not response.ok and response.json().get('error')=='STALE_SEQUENCE':continue
            assert response.ok,response.text()
            if repeat:
                again=self.page.request.post(url,data=payload)
                assert again.ok and again.json()==response.json(),again.text()
            return payload
        raise AssertionError('Browser readiness kept racing the scripted input')


class HotJourney(StableJourney):
    def hot(self):
        return next((s for s in self.current()['snapshot'].get('statuses',[]) if s.get('periodic',{}).get('kind')=='max_hp_heal'),None)

    def apply(self, *, touch=False):
        self.page.wait_for_function("!document.querySelector('#cast').disabled")
        before=sum(e['kind']=='action_committed' for e in self.current()['events'])
        previous=self.hot();old=None
        if touch:self.page.locator('#cast').tap()
        else:old=self.cast(repeat=True)
        wait(lambda:sum(e['kind']=='action_committed' for e in self.current()['events'])>before)
        definition=self.config['pve']['status_definitions'][0]
        if previous is None or definition['duplicate_policy']=='refresh':
            value=wait(lambda:s if (s:=self.hot()) and (previous is None or s['applied_at_ms']>previous['applied_at_ms']) else None)
        else:
            wait(lambda:any(e['kind']=='status_ignored' for e in self.current()['events']))
            value=self.hot()
        return value,old

    def partial(self, *, touch=False):
        injured=wait(lambda:v if (v:=self.current())['snapshot']['hp']<=v['snapshot']['max_hp']-50 else None)
        initial,old=self.apply(touch=touch)
        wait(lambda:(s:=self.hot()) and s['periodic']['ticks_done']>=1 and s['periodic']['next_tick_at_ms'] is not None)
        partial=self.hot();current=self.current()
        heal=next(e for e in current['events'] if e['kind']=='periodic_heal_tick' and e['instance_id']==initial['instance_id'] and e['hp_restored']>0)
        assert current['snapshot']['hp']>injured['snapshot']['hp']
        return initial,partial,old,{'hpBefore':injured['snapshot']['hp'],'hpAfter':current['snapshot']['hp'],'tick':heal}

    def hot_probe(self, *, reconnect=False, touch=False):
        initial,partial,old,healed=self.partial(touch=touch)
        p=partial['periodic']
        assert p['raw_heal'] in [50,100] and 'raw_damage' not in p
        if reconnect:
            self.page.reload();active(self.page);after=self.current();latest=self.hot()
            assert latest is not None, {'logical':after['snapshot']['logical_ms'],'expires':partial['expires_at_ms'],'recentEvents':after['events'][-20:]}
            assert after['snapshot']['logical_ms']<partial['expires_at_ms']
            assert after['connection']['generation']>old['connection']['generation']
            for key in ['instance_id','source','target','applied_at_ms','expires_at_ms']:assert latest[key]==partial[key]
            assert latest['periodic']['raw_heal']==p['raw_heal']
            assert latest['periodic']['ticks_done']>=p['ticks_done']
            assert latest['periodic']['next_tick_at_ms']==latest['applied_at_ms']+(latest['periodic']['ticks_done']+1)*p['interval_ms']
            stale=self.page.request.post(f"{self.host.url}/api/sessions/{after['id']}/input",data=old)
            assert not stale.ok and stale.json()['error']=='STALE_CONNECTION'
        latest,_=self.apply(touch=touch)
        assert latest['instance_id']==initial['instance_id']
        if self.config['pve']['status_definitions'][0]['duplicate_policy']=='refresh':
            assert latest['applied_at_ms']>initial['applied_at_ms'] and latest['periodic']['ticks_done']==0
            assert latest['periodic']['next_tick_at_ms']==latest['applied_at_ms']+p['interval_ms']
        else:
            assert latest['applied_at_ms']==initial['applied_at_ms'] and latest['expires_at_ms']==initial['expires_at_ms']
            assert latest['periodic']['next_tick_at_ms']==latest['applied_at_ms']+(latest['periodic']['ticks_done']+1)*p['interval_ms']
        self.page.wait_for_function("document.querySelector('#status-effects').textContent.includes('HOT') && document.body.textContent.includes('周期治疗')")
        self.page.screenshot(path=str(OUTPUT/f'{self.config["id"]}-hot.png'),full_page=True)
        wait(lambda:self.current()['snapshot']['logical_ms']>=latest['expires_at_ms']+30)
        assert self.hot() is None
        assert self.current()['snapshot']['pve']['monsters'][0]['hp']>0
        return {'initial':initial,'partial':partial,'afterReapply':latest,'healed':healed,'reconnected':reconnect,'afterExpiry':0}

    def fight_and_collect(self, *, touch=False):
        casts=2
        while self.current()['snapshot']['pve']['monsters'][0]['hp']:
            hp=self.current()['snapshot']['pve']['monsters'][0]['hp']
            self.page.wait_for_function("!document.querySelector('#cast').disabled")
            if touch:self.page.locator('#cast').tap()
            else:self.cast(repeat=True)
            casts+=1
            wait(lambda:self.current()['snapshot']['pve']['monsters'][0]['hp']<hp)
        events=self.current()['events'];assert sum(e['kind']=='monster_killed' for e in events)==1
        assert any(e['kind']=='periodic_heal_tick' and e['hp_restored']>0 for e in events)
        while self.current()['snapshot']['resources']:
            n=len(self.current()['snapshot']['resources'])
            if touch:self.page.locator('#pickup').tap()
            else:
                snap=self.current()['snapshot'];resource=snap['resources'][0]
                self.send({'kind':'pickup','resource_id':resource['id'],'expected_revision':snap['inventory_revision']},repeat=True)
            wait(lambda:len(self.current()['snapshot']['resources'])<n)
        return {'casts':casts,'kills':1,'pickedUp':True,'actualHealing':True}


def assert_closed(value):
    events=[e for e in value['events'] if e['kind']=='closed']
    # Each recovered worker emits Closed; persisted history can contain several.
    assert events and all(e['live_worlds']==0 for e in events)
    if not value.get('recovered'):assert len(events)==1
    assert not value['ledger']['pending']
    if value.get('recovered'):
        assert 'shield_total' not in (value.get('snapshot') or {})
        assert 'effective_atk' not in (value.get('snapshot') or {})
        assert all('shield_total' not in m and 'effective_atk' not in m for m in (value.get('snapshot') or {}).get('pve',{}).get('monsters',[]))
    assert not (value.get('snapshot') or {}).get('statuses')
    assert all(not m.get('statuses') for m in (value.get('snapshot') or {}).get('pve',{}).get('monsters',[]))
    return {'live_worlds':0}


def run(name,command,*,timeout=900):
    completed = subprocess.run(command,cwd=ROOT,capture_output=True,text=True,timeout=timeout)
    (OUTPUT/f'{name}.log').write_text(completed.stdout+completed.stderr)
    assert completed.returncode==0,(name,completed.stdout[-1500:],completed.stderr[-3000:])
    print(name+': passed',flush=True)
    return completed.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',action='store_true')
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True,exist_ok=True)
    frozen = sources()
    checks = {}
    commands = []
    if args.native:
        commands = [('clippy',['cargo','clippy','--locked','--offline','--all-targets','--all-features','--','-D','warnings']),
                    ('default',['cargo','test','--locked','--offline']),
                    ('all-features',['cargo','test','--locked','--offline','--all-features']),
                    ('node-storage',['node','--test','adapters/h5/persistence.test.mjs','adapters/h5/flow-control.test.mjs',
                                     'adapters/h5/isometric.test.mjs','adapters/node/asset-store.test.mjs'])]
    commands += [('build',['cargo','build','--locked','--offline','--features','replay','--bin','kernel-host'])]
    for name, command in commands:
        run(name,command);checks[name]=digest(OUTPUT/f'{name}.log')
    native_sources={p:h for p,h in frozen.items() if not p.startswith('scripts/')}
    if args.native:
        (OUTPUT/'native-verification.json').write_text(json.dumps({'passed':True,'sourceHashes':native_sources,
            'checks':{n:checks[n] for n in ['default','all-features','clippy','node-storage']}},indent=2)+'\n')
    else:
        proof=json.loads((OUTPUT/'native-verification.json').read_text())
        assert proof['passed'] and proof['sourceHashes']==native_sources
        for name,h in proof['checks'].items():
            assert digest(OUTPUT/(name+'.log'))==h
            checks[name]=h
    binary = digest(ROOT/'target/debug/kernel-host')
    results = {};touch_results = {};crashes = {};legacy_results = {}
    with tempfile.TemporaryDirectory(prefix='sdk-5-6-outside-') as temporary, sync_playwright() as pw:
        root = Path(temporary);browser = pw.chromium.launch()
        for variant in ['a','b']:
            folder=root/variant;config=package(folder,variant)
            for mode in ['extraction','death']:
                name=variant+'-'+mode;directory=root/name;directory.mkdir()
                host=Host(directory,folder/'catalog.json',folder/'account.json');host.start()
                journey=HotJourney(browser,host,config,mobile=(variant=='b'))
                try:
                    journey.start();probe=journey.hot_probe(reconnect=(name=='a-extraction'))
                    fight=journey.fight_and_collect();value=journey.finish(mode)
                    assert not journey.errors,journey.errors
                    host.stop();state,create=host.stored();assert create['protocol_version']==3
                    assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                    results[name]={'closed':assert_closed(value),'clock':'ordinary','status':probe,'fight':fight,'ledger':value['ledger'],
                                   'result':value['settlement'],'pageErrors':journey.errors}
                    print(name+': passed',flush=True)
                finally:
                    host.stop();journey.context.close()
        for mode in ['extraction','death']:
            folder=root/('touch-'+mode);config=package(folder,touch=True)
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=HotJourney(browser,host,config,mobile=True)
            try:
                journey.page.locator('#start').tap();active(journey.page)
                assert journey.current()['clock']=='live'
                probe=journey.hot_probe(touch=True)
                fight=journey.fight_and_collect(touch=True)
                if mode=='extraction':
                    journey.page.wait_for_function("!document.querySelector('#extract').disabled")
                    journey.page.locator('#extract').tap()
                else:journey.page.locator('#leave').tap()
                closed(journey.page);value=journey.current();host.stop();state,_=host.stored()
                assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                assert not journey.errors,journey.errors
                touch_results[mode]={'closed':assert_closed(value),'status':probe,'fight':fight,'castTaps':fight['casts'],'ledger':value['ledger'],
                                     'actions':'touch controls only; HTTP read-only observations','pageErrors':journey.errors}
                print('touch-'+mode+': passed',flush=True)
            finally:
                host.stop();journey.context.close()
        for point in ['active','after_intent_persist','after_storage_commit']:
            folder=root/('crash-config-'+point);config=package(folder)
            directory=root/('crash-'+point);directory.mkdir()
            host=Host(directory,folder/'catalog.json',folder/'account.json')
            host.start('' if point=='active' else point)
            journey=HotJourney(browser,host,config)
            try:
                journey.start();_,partial,_,healed=journey.partial()
                if point=='active':host.kill()
                else:
                    try:journey.send({'kind':'leave'})
                    except Exception:
                        if host.process.poll() is None:raise
                    assert host.process.wait(timeout=15)==-9
                frozen_state,original_create=host.stored()
                changed=json.loads((folder/'arena.json').read_text())
                for d in changed['pve']['status_definitions']:
                    if d['mechanism']=='periodic_heal':d.update(heal_bps=100,tick_interval_ms=900,duration_ms=2800)
                (folder/'arena.json').write_text(json.dumps(changed))
                recoveries=[]
                for _ in range(2):
                    host.start();journey.page.goto(host.url)
                    wait(lambda:journey.current() and journey.current()['phase']=='closed' and not journey.current()['ledger']['pending'])
                    value=journey.current();host.stop();state,create=host.stored();assert create==original_create
                    assert not (value.get('snapshot') or {}).get('statuses')
                    assert all(not m.get('statuses') for m in (value.get('snapshot') or {}).get('pve',{}).get('monsters',[]))
                    for identity,request in frozen_state['requests']:
                        assert dict(state['requests'])[identity]['body']==request['body']
                    assert not any('statuses' in str(op) for op in state['requests'])
                    recoveries.append({'closed':assert_closed(value),'ledger':value['ledger'],'balances':state['balances']})
                assert recoveries[0]==recoveries[1]
                assert recoveries[0]['ledger']['compensations' if point=='active' else 'settlements']==1
                if point=='active':assert state['balances']==json.loads((folder/'account.json').read_text())['assets']
                assert not journey.errors,journey.errors
                crashes[point]={'partialHotBeforeCrash':partial,'healedBeforeCrash':healed,
                               'recoveries':recoveries,'originalCreate':original_create}
                print('crash-'+point+': passed',flush=True)
            finally:
                host.stop();journey.context.close()
        for mode in ['extraction','death']:
            folder=root/('legacy-'+mode);config=package(folder,legacy=True)
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=StableJourney(browser,host,config)
            try:
                journey.start();probe=journey.status_probe()
                assert 'attribute' not in probe['initial'] and 'effective_atk' not in journey.current()['snapshot']
                assert 'stacks' not in probe['initial'] and 'max_stacks' not in probe['initial']
                journey.kill_and_collect();value=journey.finish(mode)
                assert not any(e['kind']=='status_stacked' for e in value['events'])
                assert not journey.errors,journey.errors
                host.stop();state,_=host.stored()
                assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                legacy_results[mode]={'closed':assert_closed(value),'status':probe,'ledger':value['ledger'],'pageErrors':journey.errors}
                print('legacy19-'+mode+': passed',flush=True)
            finally:
                host.stop();journey.context.close()
        # The old mechanism-20 stack fixture must retain its JSON and ordinary behavior.
        import verify_sdk_5_2 as legacy_module
        legacy_module.OUTPUT=OUTPUT
        class LegacyStackJourney(legacy_module.StackJourney,StableJourney):pass
        for mode in ['extraction','death']:
            folder=root/('legacy20-'+mode);folder.mkdir()
            import shutil
            for filename in ['arena.json','account.json','catalog.json']:
                shutil.copy2(ROOT/'examples/stack-host'/filename,folder/filename)
            config=json.loads((folder/'arena.json').read_text())
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=LegacyStackJourney(browser,host,config)
            try:
                journey.start();probe=journey.stack_probe()
                assert 'attribute' not in probe['records'][0] and 'effective_atk' not in journey.current()['snapshot']
                journey.kill_and_collect();value=journey.finish(mode);host.stop();state,_=host.stored()
                assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                assert not journey.errors,journey.errors
                legacy_results['20-'+mode]={'closed':assert_closed(value),'status':probe,'ledger':value['ledger'],'pageErrors':journey.errors}
                print('legacy20-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        import verify_sdk_5_3 as attribute_module
        attribute_module.OUTPUT=OUTPUT
        class LegacyAttributeJourney(attribute_module.StackJourney,StableJourney):pass
        for mode in ['extraction','death']:
            folder=root/('legacy21-'+mode);folder.mkdir()
            for filename in ['arena.json','account.json','catalog.json']:
                shutil.copy2(ROOT/'examples/attribute-host'/filename,folder/filename)
            config=json.loads((folder/'arena.json').read_text())
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=LegacyAttributeJourney(browser,host,config)
            try:
                journey.start();probe=journey.stack_probe()
                assert 'shield' not in probe['records'][0] and 'shield_total' not in journey.current()['snapshot']
                journey.kill_and_collect();value=journey.finish(mode);host.stop();state,_=host.stored()
                assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                assert not journey.errors,journey.errors
                legacy_results['21-'+mode]={'closed':assert_closed(value),'status':probe,'ledger':value['ledger'],'pageErrors':journey.errors}
                print('legacy21-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        import verify_sdk_5_4 as shield_module
        shield_module.OUTPUT=OUTPUT
        for mode in ['extraction','death']:
            folder=root/('legacy22-'+mode);folder.mkdir()
            for filename in ['arena.json','account.json','catalog.json']:
                shutil.copy2(ROOT/'examples/shield-host'/filename,folder/filename)
            config=json.loads((folder/'arena.json').read_text())
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=shield_module.ShieldJourney(browser,host,config)
            try:
                journey.start();probe=journey.shield_probe()
                assert 'periodic' not in probe['initial']
                journey.kill_and_collect();value=journey.finish(mode);host.stop();state,_=host.stored()
                assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                assert not journey.errors,journey.errors
                legacy_results['22-'+mode]={'closed':assert_closed(value),'status':probe,'ledger':value['ledger'],'pageErrors':journey.errors}
                print('legacy22-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        import verify_sdk_5_5 as dot_module
        dot_module.OUTPUT=OUTPUT
        for mode in ['extraction','death']:
            folder=root/('legacy23-'+mode);config=dot_module.package(folder)
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=dot_module.DotJourney(browser,host,config)
            try:
                journey.start();probe=journey.dot_probe()
                assert 'raw_heal' not in probe['initial']['periodic']
                journey.kill_with_dot_and_collect();value=journey.finish(mode);host.stop();state,_=host.stored()
                assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                assert not journey.errors,journey.errors
                legacy_results['23-'+mode]={'closed':assert_closed(value),'status':probe,'ledger':value['ledger'],'pageErrors':journey.errors}
                print('legacy23-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        browser.close()
    assert sources()==frozen,'Behavior inputs changed during acceptance'
    assert digest(ROOT/'target/debug/kernel-host')==binary
    report={'passed':True,'scope':'SDK-5-6 local ordinary Node/H5, finite self max-HP periodic healing; no production signoff',
            'sourceHashes':frozen,'binarySha256':binary,'checks':checks,'cases':results,
            'touchCases':touch_results,'crashes':crashes,'legacy':legacy_results}
    (OUTPUT/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('SDK-5-6: 4 outside journeys, 2 pure-touch journeys, 3 crash boundaries and legacy19/20/21/22/23 dual outcomes passed',flush=True)


if __name__=='__main__':main()
