"""SDK-5-1: ordinary Node/H5 consumption, outside configurations and durable asset recovery."""
import argparse
from contextlib import closing
import hashlib
import json
from pathlib import Path
import selectors
import sqlite3
import subprocess
import tempfile
import time

from playwright.sync_api import sync_playwright
from verify_h5 import active, closed, info, wait

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/sdk-5-1'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def sources():
    paths = [ROOT/'Cargo.toml', ROOT/'Cargo.lock', *ROOT.glob('src/**/*.rs'),
             *ROOT.glob('adapters/**/*.mjs'), *ROOT.glob('web/*'), *ROOT.glob('config/**/*.json'),
             *ROOT.glob('examples/**/*.json'), Path(__file__)]
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(set(paths)) if p.is_file()}


class Host:
    def __init__(self, directory, manifest, account):
        self.path = directory/'lab.sqlite'
        self.manifest, self.account, self.port = manifest, account, 0
        self.err = tempfile.TemporaryFile(mode='w+t')
        self.process = None

    def start(self, point='', kind='commit_settlement'):
        command = ['node', 'adapters/h5/server.mjs', '--port', str(self.port), '--state', str(self.path),
                   '--configs', str(self.manifest), '--account', str(self.account)]
        if point:
            command += ['--crash-at', point, '--crash-kind', kind]
        self.process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=self.err, text=True)
        selector = selectors.DefaultSelector()
        selector.register(self.process.stdout, selectors.EVENT_READ)
        try:
            assert selector.select(20), 'Node startup deadline'
            line = self.process.stdout.readline()
            if not line:
                self.err.seek(0)
                raise AssertionError(self.err.read())
            self.url = json.loads(line)['url']
            self.port = int(self.url.rsplit(':', 1)[1])
        finally:
            selector.close()

    def stop(self):
        if self.process and self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=10)

    def kill(self):
        self.process.kill()
        assert self.process.wait(timeout=10) == -9

    def stored(self):
        assert self.process.poll() is not None, 'never read the exclusively owned live SQLite file'
        with closing(sqlite3.connect(self.path)) as db:
            state = json.loads(db.execute('SELECT state FROM stores ORDER BY rowid DESC LIMIT 1').fetchone()[0])
            create = json.loads(db.execute('SELECT original_create FROM sessions ORDER BY rowid DESC LIMIT 1').fetchone()[0])
        return state, create


def package(directory, variant, *, long=False):
    directory.mkdir()
    value = json.loads((ROOT/'examples/status-host/arena.json').read_text())
    account = json.loads((ROOT/'examples/status-host/account.json').read_text())
    value['id'] = f'outside-status-{variant}-{int(long)}'
    if long:
        for d in value['pve']['status_definitions']:
            d['duration_ms'] = 4000
    if variant == 'b':
        value['skill']['id'] = 'outside-projectile'
        definitions = value['pve']['status_definitions']
        for d, identity, modifier in zip(definitions, ['outside-ward', 'outside-exposure'], [5000, -10000]):
            d.update(id=identity, modifier_bps=modifier, duplicate_policy='ignore', source_scoped=False)
        value['skill']['on_commit_self_statuses'] = ['outside-ward']
        value['skill']['on_hit_target_statuses'] = ['outside-exposure']
        value['asset_contract'] = {'entry_cost_asset_id':'pass', 'reward_asset_id':'merit'}
        account['assets'] = {'pass':100, 'merit':0, 'xuan-tie':100}
    (directory/'arena.json').write_text(json.dumps(value))
    (directory/'catalog.json').write_text(json.dumps({'schema_version':1,'default_scenario':'outside',
        'active':{'outside':'arena.json'},'archives':[]}))
    (directory/'account.json').write_text(json.dumps(account))
    return value


class Journey:
    def __init__(self, browser, host, config, *, mobile=False):
        self.context = browser.new_context(viewport={'width':390,'height':844} if mobile else {'width':1440,'height':1100},
                                           is_mobile=mobile, has_touch=mobile)
        self.page = self.context.new_page()
        self.host, self.config = host, config
        self.errors = []
        self.page.on('pageerror', lambda e:self.errors.append(str(e)))
        self.page.goto(host.url)

    def current(self):
        return info(self.page, self.host.url)

    def start(self):
        self.page.locator('#start').click()
        active(self.page)
        assert self.current()['clock'] == 'live'

    def send(self, action, *, repeat=False):
        current = self.current()
        payload = {'connection':current['connection'], 'sequence':current['sequence']+1,
                   'requestId':f"status-{current['sequence']+1}", 'action':action}
        url = f"{self.host.url}/api/sessions/{current['id']}/input"
        response = self.page.request.post(url, data=payload)
        assert response.ok, response.text()
        if repeat:
            again = self.page.request.post(url, data=payload)
            assert again.ok and again.json() == response.json(), again.text()
        return payload

    def cast(self, repeat=False):
        return self.send({'kind':'cast','skill_id':self.config['skill']['id'],'direction':[1,0]},repeat=repeat)

    def status_probe(self, *, reconnect=False):
        old = self.cast(repeat=True)
        current = wait(lambda: v if (v:=self.current())['snapshot'].get('statuses') and v['snapshot']['pve']['monsters'][0].get('statuses') else None)
        initial = current['snapshot']['statuses'][0]
        self.page.wait_for_function("!document.querySelector('#status-effects').hidden && document.querySelector('#status-effects').textContent.includes('来源')")
        assert current['snapshot']['effective_def'] == (200 if self.config['pve']['status_definitions'][0]['modifier_bps']==10000 else 150)
        if reconnect:
            self.page.reload()
            active(self.page)
            current = self.current()
            assert current['connection']['generation'] > old['connection']['generation']
            assert current['snapshot']['statuses'][0] == initial
            stale = self.page.request.post(f"{self.host.url}/api/sessions/{current['id']}/input",data=old)
            assert not stale.ok and stale.json()['error']=='STALE_CONNECTION'
        wait(lambda: self.current()['snapshot']['logical_ms'] >= initial['expires_at_ms']+30)
        ended = self.current()
        assert not ended['snapshot'].get('statuses') and ended['snapshot']['effective_def']==100
        assert len([e for e in ended['events'] if e['kind']=='status_applied' and e['status']['definition_id']==initial['definition_id']])==1
        return {'initial':initial, 'afterExpiry':ended['snapshot']['effective_def'], 'reconnected':reconnect}

    def kill_and_collect(self):
        deadline = time.monotonic()+20
        next_cast = 0
        while self.current()['snapshot']['pve']['monsters'][0]['hp']:
            assert time.monotonic()<deadline, 'fight deadline'
            snapshot = self.current()['snapshot']
            if snapshot['logical_ms']>=max(next_cast,snapshot['recovery_until_ms']):
                if snapshot['logical_ms']>=snapshot['skill_ready_at_ms']:
                    self.cast()
                    next_cast=snapshot['logical_ms']+self.config['skill']['windup_ms']+self.config['skill']['cooldown_ms']+300
            self.page.wait_for_timeout(140)
        for resource in list(self.current()['snapshot']['resources']):
            self.send({'kind':'navigate','destination':resource['position']})
            wait(lambda: sum((a-b)**2 for a,b in zip(self.current()['snapshot']['position'],resource['position']))<100)
            self.send({'kind':'stop'})
            snapshot = self.current()['snapshot']
            self.send({'kind':'pickup','resource_id':resource['id'],'expected_revision':snapshot['inventory_revision']})

    def finish(self, mode):
        if mode=='extraction':
            self.send({'kind':'navigate','destination':self.config['extraction_point']})
            wait(lambda: sum((a-b)**2 for a,b in zip(self.current()['snapshot']['position'],self.config['extraction_point']))<100)
            self.send({'kind':'stop'})
            wait(lambda:self.current()['snapshot']['extraction_open'])
            self.send({'kind':'extract_start'})
        else:
            self.send({'kind':'leave'})
        closed(self.page)
        return self.current()


def assert_settlement(state, current, original):
    requests = dict(state['requests'])
    transactions = [r['body']['transaction'] for r in requests.values() if r['body']['kind']=='commit_assets']
    balances = dict(original['assets'])
    for tx in transactions:
        for row in tx.get('debits',tx.get('credits',[])):
            balances[row['asset_id']] = balances.get(row['asset_id'],0)+(-1 if tx['kind']=='admission' else 1)*row['quantity']
    assert balances==state['balances'], (balances,state['balances'])
    ledger=current['ledger']
    assert ledger['admissions']==ledger['settlements']==1 and not ledger['pending'] and ledger['manual']==0
    assert len([tx for tx in transactions if tx['kind']=='result_settlement'])==1
    assert all(not any('status' in key for key in tx['summary']) for tx in transactions if tx['kind']=='result_settlement')
    assert current['phase']=='closed'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',action='store_true',help='also run complete Rust tests, Clippy and Node storage tests')
    args=parser.parse_args()
    OUTPUT.mkdir(parents=True,exist_ok=True)
    native={}
    commands=[('build',['cargo','build','--locked','--features','replay','--bin','kernel-host'])]
    if args.native:
        commands=[('default',['cargo','test','--locked']),('all-features',['cargo','test','--locked','--all-features']),
                  ('clippy',['cargo','clippy','--locked','--all-targets','--all-features','--','-D','warnings']),
                  ('node-storage',['node','--test','adapters/h5/persistence.test.mjs','adapters/h5/flow-control.test.mjs','adapters/h5/isometric.test.mjs','adapters/node/asset-store.test.mjs'])]+commands
    frozen=sources()
    for name,command in commands:
        completed=subprocess.run(command,cwd=ROOT,capture_output=True,text=True,timeout=900)
        (OUTPUT/f'{name}.log').write_text(completed.stdout+completed.stderr)
        assert completed.returncode==0,(name,completed.stderr[-3000:])
        native[name]=digest(OUTPUT/f'{name}.log')
        print(name+': passed',flush=True)
    binary=digest(ROOT/'target/debug/kernel-host')
    results={};crashes={}
    with tempfile.TemporaryDirectory(prefix='sdk-5-1-outside-') as temporary, sync_playwright() as pw:
        root=Path(temporary);browser=pw.chromium.launch()
        for variant in ['a','b']:
            folder=root/variant;config=package(folder,variant,long=(variant=='a'))
            for mode in ['extraction','death']:
                name=variant+'-'+mode;directory=root/name;directory.mkdir();host=Host(directory,folder/'catalog.json',folder/'account.json');host.start()
                journey=Journey(browser,host,config,mobile=(variant=='b'))
                try:
                    journey.start();probe=journey.status_probe(reconnect=(name=='a-extraction'))
                    journey.kill_and_collect();value=journey.finish(mode)
                    assert not journey.errors,journey.errors
                    journey.page.screenshot(path=str(OUTPUT/(name+'.png')),full_page=True)
                    host.stop();state,create=host.stored();assert create['protocol_version']==3
                    assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                    results[name]={'clock':'ordinary','status':probe,'ledger':value['ledger'],'result':value['settlement'],'pageErrors':journey.errors}
                    print(name+': passed',flush=True)
                finally:
                    host.stop();journey.context.close()
        # Verify the same Node + exclusive SQLite host at three real crash boundaries.
        for point in ['active','after_intent_persist','after_storage_commit']:
            folder=root/('crash-config-'+point);config=package(folder,'a',long=True)
            directory=root/('crash-'+point);directory.mkdir();host=Host(directory,folder/'catalog.json',folder/'account.json')
            host.start('' if point=='active' else point)
            journey=Journey(browser,host,config)
            try:
                journey.start();journey.cast();wait(lambda:journey.current()['snapshot'].get('statuses'))
                if point=='active':host.kill()
                else:
                    try:journey.send({'kind':'leave'})
                    except Exception:
                        if host.process.poll() is None:raise
                    assert host.process.wait(timeout=15)==-9
                frozen_state,original_create=host.stored()
                # Changing outside tuning must not rewrite any frozen account obligation.
                changed=json.loads((folder/'arena.json').read_text());changed['pve']['status_definitions'][0]['modifier_bps']=0
                (folder/'arena.json').write_text(json.dumps(changed))
                recoveries=[]
                for _ in range(2):
                    host.start();journey.page.goto(host.url);wait(lambda:journey.current() and journey.current()['phase']=='closed' and not journey.current()['ledger']['pending'])
                    value=journey.current();host.stop();state,create=host.stored();assert create==original_create
                    assert not (value.get('snapshot') or {}).get('statuses')
                    assert all(not m.get('statuses') for m in (value.get('snapshot') or {}).get('pve',{}).get('monsters',[]))
                    for identity,request in frozen_state['requests']:
                        assert dict(state['requests'])[identity]['body']==request['body']
                    assert not any('statuses' in str(op) for op in state['requests'])
                    recoveries.append({'ledger':value['ledger'],'balances':state['balances']})
                assert recoveries[0]==recoveries[1]
                assert recoveries[0]['ledger']['compensations' if point=='active' else 'settlements']==1
                if point=='active':assert state['balances']==json.loads((folder/'account.json').read_text())['assets']
                assert not journey.errors,journey.errors
                crashes[point]={'recoveries':recoveries,'originalCreate':original_create}
                print('crash-'+point+': passed',flush=True)
            finally:
                host.stop();journey.context.close()
        browser.close()
    assert sources()==frozen,'Behavior inputs changed during acceptance'
    assert digest(ROOT/'target/debug/kernel-host')==binary
    report={'passed':True,'scope':'SDK-5-1 ordinary Node/H5 and local SQLite; no production signoff',
            'sourceHashes':frozen,'binarySha256':binary,'checks':native,'cases':results,'crashes':crashes}
    (OUTPUT/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('SDK-5-1: 4 outside ordinary Node journeys and 3 crash boundaries passed',flush=True)


if __name__=='__main__':
    main()
