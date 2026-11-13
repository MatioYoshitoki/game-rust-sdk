"""SDK-5-2: real-clock stacking, legacy consumers and durable host recovery."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from playwright.sync_api import sync_playwright
from verify_h5 import active, closed, wait
from verify_sdk_5_1 import Host, Journey, assert_settlement, digest

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT/'artifacts/sdk-5-2'


def sources():
    paths = [ROOT/'Cargo.toml', ROOT/'Cargo.lock', *ROOT.glob('src/**/*.rs'),
             *ROOT.glob('adapters/**/*.mjs'), *ROOT.glob('web/*'), *ROOT.glob('config/**/*.json'),
             *ROOT.glob('examples/*/*.json'), *ROOT.glob('examples/minimal-host/src/*.rs'),
             ROOT/'examples/minimal-host/Cargo.toml', ROOT/'examples/minimal-host/Cargo.lock',
             Path(__file__), ROOT/'scripts/verify_sdk_5_1.py', ROOT/'scripts/verify_h5.py']
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(set(paths)) if p.is_file()}


def package(directory, variant='a', *, legacy=False, touch=False):
    directory.mkdir()
    sample = ROOT/'examples'/('status-host' if legacy else 'stack-host')
    value = json.loads((sample/'arena.json').read_text())
    account = json.loads((sample/'account.json').read_text())
    value['id'] = f'outside-{variant}-{"legacy" if legacy else "stack"}-{int(touch)}'
    if not legacy:
        for d in value['pve']['status_definitions']:
            d['duration_ms'] = 4000
        if variant == 'b':
            value['skill']['id'] = 'outside-bolt'
            for d, identity, modifier in zip(value['pve']['status_definitions'],
                                            ['outside-ward','outside-exposure'], [1000,-5000]):
                d.update(id=identity, modifier_bps=modifier, max_stacks=2,
                         source_scoped=not d['source_scoped'], duration_ms=3200)
            value['skill']['on_commit_self_statuses'] = ['outside-ward']
            value['skill']['on_hit_target_statuses'] = ['outside-exposure']
            value['asset_contract'] = {'entry_cost_asset_id':'pass','reward_asset_id':'merit'}
            account['assets'] = {'pass':100,'merit':0,'xuan-tie':100}
    if touch:
        value['pve']['monster_positions'] = [[450.,400.]]
    (directory/'arena.json').write_text(json.dumps(value))
    (directory/'catalog.json').write_text(json.dumps({'schema_version':1,'default_scenario':'outside',
        'active':{'outside':'arena.json'},'archives':[]}))
    (directory/'account.json').write_text(json.dumps(account))
    return value


class StackJourney(Journey):
    def layer(self, count, *, touch=False):
        self.page.wait_for_function("!document.querySelector('#cast').disabled")
        if touch:
            self.page.locator('#cast').tap()
            old = None
        else:
            old = self.cast(repeat=True)
        value = wait(lambda: v if (v:=self.current())['snapshot'].get('statuses')
                     and v['snapshot']['statuses'][0]['stacks']==count
                     and v['snapshot']['pve']['monsters'][0].get('statuses')
                     and v['snapshot']['pve']['monsters'][0]['statuses'][0]['stacks']==count else None)
        return value, old

    def stack_probe(self, *, reconnect=False, touch=False):
        definition = self.config['pve']['status_definitions'][0]
        cap = definition['max_stacks']
        records = []
        first_id = None
        for count in [*range(1,cap+1), cap]:
            before = self.current()['snapshot'].get('statuses', [])
            value, old = self.layer(count,touch=touch)
            # At cap, wait for the new application rather than accepting the old count.
            if before and count==cap:
                value = wait(lambda: v if (v:=self.current())['snapshot']['statuses'][0]['applied_at_ms']
                             > before[0]['applied_at_ms'] else None)
            status = value['snapshot']['statuses'][0]
            first_id = first_id or status['instance_id']
            assert status['instance_id']==first_id and status['max_stacks']==cap
            assert status['expires_at_ms']-status['applied_at_ms']==definition['duration_ms']
            assert value['snapshot']['effective_def']==100*(10000+definition['modifier_bps']*count)//10000
            self.page.wait_for_function("label => document.querySelector('#status-effects').textContent.includes(label)",arg=f'{count}/{cap}层')
            records.append(status)
            if count==2 and reconnect:
                self.page.reload();active(self.page)
                after = self.current()
                assert after['connection']['generation']>old['connection']['generation']
                assert after['snapshot']['statuses'][0]==status
                stale = self.page.request.post(f"{self.host.url}/api/sessions/{after['id']}/input",data=old)
                assert not stale.ok and stale.json()['error']=='STALE_CONNECTION'
        current = self.current()
        events = [e for e in current['events'] if e['kind']=='status_stacked'
                  and e['status']['definition_id']==definition['id']]
        assert [e['added_stacks'] for e in events]==[1]*(cap-1)+[0], events
        self.page.screenshot(path=str(OUTPUT/f'{self.config["id"]}-layers.png'),full_page=True)
        last = records[-1]
        wait(lambda:self.current()['snapshot']['logical_ms']>=last['expires_at_ms']+30)
        ended = self.current()
        assert not ended['snapshot'].get('statuses') and ended['snapshot']['effective_def']==100
        return {'records':records,'stackEvents':events,'afterExpiry':100,'reconnected':reconnect}


def assert_closed(value):
    events=[e for e in value['events'] if e['kind']=='closed']
    # Each recovered worker emits Closed; persisted history can contain several.
    assert events and all(e['live_worlds']==0 for e in events)
    if not value.get('recovered'):assert len(events)==1
    assert not value['ledger']['pending']
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
        commands = [('default',['cargo','test','--locked','--offline']),
                    ('all-features',['cargo','test','--locked','--offline','--all-features']),
                    ('clippy',['cargo','clippy','--locked','--offline','--all-targets','--all-features','--','-D','warnings']),
                    ('node-storage',['node','--test','adapters/h5/persistence.test.mjs','adapters/h5/flow-control.test.mjs',
                                     'adapters/h5/isometric.test.mjs','adapters/node/asset-store.test.mjs'])]
    commands += [('build',['cargo','build','--locked','--offline','--features','replay','--bin','kernel-host'])]
    for name, command in commands:
        run(name,command);checks[name]=digest(OUTPUT/f'{name}.log')
    binary = digest(ROOT/'target/debug/kernel-host')
    results = {};touch_results = {};crashes = {};legacy_results = {}
    with tempfile.TemporaryDirectory(prefix='sdk-5-2-outside-') as temporary, sync_playwright() as pw:
        root = Path(temporary);browser = pw.chromium.launch()
        for variant in ['a','b']:
            folder=root/variant;config=package(folder,variant)
            for mode in ['extraction','death']:
                name=variant+'-'+mode;directory=root/name;directory.mkdir()
                host=Host(directory,folder/'catalog.json',folder/'account.json');host.start()
                journey=StackJourney(browser,host,config,mobile=(variant=='b'))
                try:
                    journey.start();probe=journey.stack_probe(reconnect=(name=='a-extraction'))
                    journey.kill_and_collect();value=journey.finish(mode)
                    assert not journey.errors,journey.errors
                    host.stop();state,create=host.stored();assert create['protocol_version']==3
                    assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                    results[name]={'closed':assert_closed(value),'clock':'ordinary','status':probe,'ledger':value['ledger'],
                                   'result':value['settlement'],'pageErrors':journey.errors}
                    print(name+': passed',flush=True)
                finally:
                    host.stop();journey.context.close()
        for mode in ['extraction','death']:
            folder=root/('touch-'+mode);config=package(folder,touch=True)
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=StackJourney(browser,host,config,mobile=True)
            try:
                journey.page.locator('#start').tap();active(journey.page)
                assert journey.current()['clock']=='live'
                probe=journey.stack_probe(touch=True)
                casts=4
                while journey.current()['snapshot']['pve']['monsters'][0]['hp']:
                    hp=journey.current()['snapshot']['pve']['monsters'][0]['hp']
                    journey.page.wait_for_function("!document.querySelector('#cast').disabled")
                    journey.page.locator('#cast').tap();casts+=1
                    wait(lambda:journey.current()['snapshot']['pve']['monsters'][0]['hp']<hp)
                while journey.current()['snapshot']['resources']:
                    n=len(journey.current()['snapshot']['resources'])
                    journey.page.locator('#pickup').tap()
                    wait(lambda:len(journey.current()['snapshot']['resources'])<n)
                if mode=='extraction':
                    journey.page.wait_for_function("!document.querySelector('#extract').disabled")
                    journey.page.locator('#extract').tap()
                else:journey.page.locator('#leave').tap()
                closed(journey.page);value=journey.current();host.stop();state,_=host.stored()
                assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                assert not journey.errors,journey.errors
                touch_results[mode]={'closed':assert_closed(value),'status':probe,'castTaps':casts,'ledger':value['ledger'],
                                     'actions':'touch controls only; HTTP read-only observations','pageErrors':journey.errors}
                print('touch-'+mode+': passed',flush=True)
            finally:
                host.stop();journey.context.close()
        for point in ['active','after_intent_persist','after_storage_commit']:
            folder=root/('crash-config-'+point);config=package(folder)
            directory=root/('crash-'+point);directory.mkdir()
            host=Host(directory,folder/'catalog.json',folder/'account.json')
            host.start('' if point=='active' else point)
            journey=StackJourney(browser,host,config)
            try:
                journey.start();journey.layer(1);multi,_=journey.layer(2)
                if point=='active':host.kill()
                else:
                    try:journey.send({'kind':'leave'})
                    except Exception:
                        if host.process.poll() is None:raise
                    assert host.process.wait(timeout=15)==-9
                frozen_state,original_create=host.stored()
                changed=json.loads((folder/'arena.json').read_text())
                for d in changed['pve']['status_definitions']:
                    d.update(modifier_bps=0,max_stacks=1,duration_ms=1800)
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
                crashes[point]={'multiLayerBeforeCrash':multi['snapshot']['statuses'],
                               'recoveries':recoveries,'originalCreate':original_create}
                print('crash-'+point+': passed',flush=True)
            finally:
                host.stop();journey.context.close()
        for mode in ['extraction','death']:
            folder=root/('legacy-'+mode);config=package(folder,legacy=True)
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=Journey(browser,host,config)
            try:
                journey.start();probe=journey.status_probe()
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
        browser.close()
    assert sources()==frozen,'Behavior inputs changed during acceptance'
    assert digest(ROOT/'target/debug/kernel-host')==binary
    report={'passed':True,'scope':'SDK-5-2 local ordinary Node/H5, finite DEF stack; no production signoff',
            'sourceHashes':frozen,'binarySha256':binary,'checks':checks,'cases':results,
            'touchCases':touch_results,'crashes':crashes,'legacy19':legacy_results}
    (OUTPUT/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('SDK-5-2: 4 outside journeys, 2 pure-touch journeys, 3 crash boundaries and legacy19 dual outcomes passed',flush=True)


if __name__=='__main__':main()
