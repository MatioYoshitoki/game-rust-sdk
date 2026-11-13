"""SDK-5-8 configured dual player skills: ordinary H5 and durable recovery."""
import argparse,json,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_h5 import active,closed,wait
from verify_sdk_5_1 import Host,assert_settlement,digest
import verify_sdk_5_7 as previous
from verify_sdk_5_7 import StableJourney,ReceivedJourney
ROOT=Path(__file__).resolve().parents[1]
OUTPUT=ROOT/'artifacts/sdk-5-8'
def sources():
    paths=previous.sources()
    for name in ['verify_sdk_5_7','verify_sdk_5_8']:paths['scripts/'+name+'.py']=digest(ROOT/('scripts/'+name+'.py'))
    return paths
def package(directory,variant='a',*,legacy=False,touch=False,poor=False):
    if legacy:return previous.package(directory,variant,legacy=True,touch=touch)
    directory.mkdir();sample=ROOT/'examples/dual-skill-host'
    value=json.loads((sample/'arena.json').read_text());account=json.loads((sample/'account.json').read_text())
    value['id']=f'outside-dual-{variant}-{int(touch)}-{int(poor)}'
    primary=value['skill'];secondary=value['additional_skills'][0]
    value['basic']['windup_ms']=600
    primary.update(windup_ms=600,recovery_ms=900,cooldown_ms=3600)
    secondary.update(windup_ms=450,recovery_ms=900,cooldown_ms=4800)
    if variant=='b':
        primary.update(id='outside-shield-shot',cost=14,cooldown_ms=4200,damage_bps=11000,speed=800)
        secondary.update(id='outside-heal-shot',cost=10,cooldown_ms=5200,damage_bps=12500,speed=900)
        for d in value['pve']['status_definitions']:d['id']='outside-'+d['id']
        primary['on_commit_self_statuses']=['outside-ward'];secondary['on_commit_self_statuses']=['outside-renewal']
        value['asset_contract']={'entry_cost_asset_id':'pass','reward_asset_id':'merit'};account['assets']={'pass':100,'merit':0,'xuan-tie':100}
    if poor:value['params']['initial_player']['spirit_points']=15;account['stats']['spirit_points']=15
    for name,data in [('arena',value),('account',account),('catalog',{'schema_version':1,'default_scenario':'outside','active':{'outside':'arena.json'},'archives':[]})]:(directory/(name+'.json')).write_text(json.dumps(data))
    return value
class MultiJourney(StableJourney):
    def commits(self):return [e for e in self.current()['events'] if e['kind']=='action_committed' and e.get('skill_id')]
    def use(self,index,*,touch=False):
        button='#cast' if index==0 else '#cast-secondary';skill=([self.config['skill']]+self.config['additional_skills'])[index]
        self.page.wait_for_function('(id)=>!document.querySelector(id).disabled',arg=button)
        before=len(self.commits())
        if touch:self.page.locator(button).tap();payload=None
        else:payload=self.send({'kind':'cast','skill_id':skill['id'],'direction':[1,0]},repeat=True)
        wait(lambda:len(self.commits())>before)
        committed=self.commits()[-1];assert committed['skill_id']==skill['id'] and committed['spirit_cost']==skill['cost']
        return payload
    def rejected(self,index,reason):
        skill=([self.config['skill']]+self.config['additional_skills'])[index]
        for _ in range(3):
            old=self.current();payload={'connection':old['connection'],'sequence':old['sequence']+1,'requestId':f"reject-{old['sequence']+1}",'action':{'kind':'cast','skill_id':skill['id'],'direction':[1,0]}}
            response=self.page.request.post(f"{self.host.url}/api/sessions/{old['id']}/input",data=payload)
            if response.json().get('error')=='STALE_SEQUENCE':continue
            assert not response.ok and response.json()['error']==reason,response.text()
            break
        else:raise AssertionError('Readiness raced rejected intent')
        after=self.current();assert after['snapshot']['spirit']==old['snapshot']['spirit'];assert after['snapshot']['player_skills']==old['snapshot']['player_skills']
        return {'reason':reason,'unchangedSpirit':after['snapshot']['spirit'],'unchangedCooldowns':after['snapshot']['player_skills']}
    def probe(self,*,touch=False,reconnect=False):
        first=self.use(0,touch=touch);one=self.current()['snapshot'];ids=[self.config['skill']['id'],self.config['additional_skills'][0]['id']]
        assert one['player_skills'][0]['ready_at_ms']==one['skill_ready_at_ms']>one['logical_ms'];assert one['player_skills'][1]['ready_at_ms']==0
        rejected=None if touch else self.rejected(1,'INVALID_STATE')
        self.use(1,touch=touch);two=self.current()['snapshot'];assert two['player_skills'][0]['ready_at_ms']==one['player_skills'][0]['ready_at_ms'];assert all(v['ready_at_ms']>two['logical_ms'] for v in two['player_skills'])
        assert two['spirit']==200-self.config['skill']['cost']-self.config['additional_skills'][0]['cost']
        assert {s['definition_id'] for s in two['statuses']}=={self.config['skill']['on_commit_self_statuses'][0],self.config['additional_skills'][0]['on_commit_self_statuses'][0]}
        if reconnect:
            self.page.reload();active(self.page);after=self.current();assert after['connection']['generation']>first['connection']['generation'];assert after['snapshot']['player_skills']==two['player_skills'];assert after['snapshot']['recovery_until_ms']==two['recovery_until_ms'];assert after['snapshot']['spirit']==two['spirit']
            stale=self.page.request.post(f"{self.host.url}/api/sessions/{after['id']}/input",data=first);assert not stale.ok and stale.json()['error']=='STALE_CONNECTION'
        if not touch:wait(lambda:self.current()['snapshot']['logical_ms']>=two['recovery_until_ms']);cd=self.rejected(0,'INVALID_STATE')
        else:cd=None
        self.page.screenshot(path=str(OUTPUT/(self.config['id']+'-skills.png')),full_page=True)
        return {'ids':ids,'afterPrimary':one,'afterSecondary':two,'recoveryRejection':rejected,'cooldownRejection':cd,'reconnected':reconnect}
    def fight(self,*,touch=False):
        index=0
        while self.current()['snapshot']['pve']['monsters'][0]['hp']:
            before=self.current()['snapshot']['pve']['monsters'][0]['hp'];self.use(index,touch=touch);index=1-index
            wait(lambda:self.current()['snapshot']['pve']['monsters'][0]['hp']<before)
        while self.current()['snapshot']['resources']:
            n=len(self.current()['snapshot']['resources'])
            if touch:self.page.locator('#pickup').tap()
            else:
                snap=self.current()['snapshot'];self.send({'kind':'pickup','resource_id':snap['resources'][0]['id'],'expected_revision':snap['inventory_revision']},repeat=True)
            wait(lambda:len(self.current()['snapshot']['resources'])<n)
        assert sum(e['kind']=='monster_killed' for e in self.current()['events'])==1
        return {'kills':1,'pickedUp':True,'committedSkills':sorted(set(e['skill_id'] for e in self.commits()))}
def assert_closed(value):
    result=previous.assert_closed(value)
    if value.get('recovered'):assert all(k not in (value.get('snapshot') or {}) for k in ['player_skills','pending_action'])
    return result
def run(name,command):
    p=subprocess.run(command,cwd=ROOT,capture_output=True,text=True,timeout=900);(OUTPUT/(name+'.log')).write_text(p.stdout+p.stderr);assert p.returncode==0,(name,p.stdout[-1500:],p.stderr[-3000:]);print(name+': passed',flush=True)
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--native',action='store_true');args=parser.parse_args();OUTPUT.mkdir(parents=True,exist_ok=True);previous.OUTPUT=OUTPUT
    frozen=sources();checks={};commands=[]
    if args.native:commands=[('clippy',['cargo','clippy','--locked','--offline','--all-targets','--all-features','--','-D','warnings']),('default',['cargo','test','--locked','--offline']),('all-features',['cargo','test','--locked','--offline','--all-features']),('node-storage',['node','--test','adapters/h5/persistence.test.mjs','adapters/h5/flow-control.test.mjs','adapters/h5/isometric.test.mjs','adapters/node/asset-store.test.mjs'])]
    commands+=[('build',['cargo','build','--locked','--offline','--features','replay','--bin','kernel-host'])]
    for name,command in commands:run(name,command);checks[name]=digest(OUTPUT/(name+'.log'))
    native_sources={p:h for p,h in frozen.items() if not p.startswith('scripts/')}
    if args.native:(OUTPUT/'native-verification.json').write_text(json.dumps({'passed':True,'sourceHashes':native_sources,'checks':{n:checks[n] for n in ['default','all-features','clippy','node-storage']}},indent=2)+'\n')
    else:
        proof=json.loads((OUTPUT/'native-verification.json').read_text());assert proof['passed'] and proof['sourceHashes']==native_sources
        for name,h in proof['checks'].items():assert digest(OUTPUT/(name+'.log'))==h;checks[name]=h
    binary=digest(ROOT/'target/debug/kernel-host');results={};touch_results={};crashes={};legacy_results={};resource={}
    with tempfile.TemporaryDirectory(prefix='sdk-5-8-outside-') as temporary,sync_playwright() as pw:
        root=Path(temporary);browser=pw.chromium.launch()
        for variant in ['a','b']:
            folder=root/variant;config=package(folder,variant)
            for mode in ['extraction','death']:
                name=variant+'-'+mode;directory=root/name;directory.mkdir();host=Host(directory,folder/'catalog.json',folder/'account.json');host.start();journey=MultiJourney(browser,host,config,mobile=variant=='b')
                try:
                    journey.start();probe=journey.probe(reconnect=name=='a-extraction');fight=journey.fight();value=journey.finish(mode);assert not journey.errors,journey.errors
                    host.stop();state,create=host.stored();assert create['protocol_version']==3;assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                    results[name]={'closed':assert_closed(value),'clock':'ordinary','skills':probe,'fight':fight,'ledger':value['ledger'],'result':value['settlement'],'pageErrors':journey.errors};print(name+': passed',flush=True)
                finally:host.stop();journey.context.close()
        for mode in ['extraction','death']:
            folder=root/('touch-'+mode);config=package(folder,touch=True);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=MultiJourney(browser,host,config,mobile=True)
            try:
                journey.page.locator('#start').tap();active(journey.page);probe=journey.probe(touch=True);fight=journey.fight(touch=True)
                if mode=='extraction':journey.page.wait_for_function("!document.querySelector('#extract').disabled");journey.page.locator('#extract').tap()
                else:journey.page.locator('#leave').tap()
                closed(journey.page);value=journey.current();host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors
                touch_results[mode]={'closed':assert_closed(value),'skills':probe,'fight':fight,'actions':'touch controls only; HTTP read-only observations','ledger':value['ledger'],'pageErrors':journey.errors};print('touch-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        folder=root/'poor';config=package(folder,poor=True);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=MultiJourney(browser,host,config)
        try:
            journey.start();journey.use(0);wait(lambda:journey.current()['snapshot']['logical_ms']>=journey.current()['snapshot']['recovery_until_ms']);resource=journey.rejected(1,'NO_RESOURCE')
            journey.page.wait_for_function("document.querySelector('#cast-secondary').disabled && document.querySelector('#cast-secondary-detail').textContent.includes('灵力不足')")
            value=journey.finish('death');host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));resource.update(closed=assert_closed(value),ledger=value['ledger']);assert not journey.errors,journey.errors;print('resource-rejection: passed',flush=True)
        finally:host.stop();journey.context.close()
        for point in ['active','after_intent_persist','after_storage_commit']:
            folder=root/('crash-config-'+point);config=package(folder);directory=root/('crash-'+point);directory.mkdir();host=Host(directory,folder/'catalog.json',folder/'account.json');host.start('' if point=='active' else point);journey=MultiJourney(browser,host,config)
            try:
                journey.start();probe=journey.probe()
                if point=='active':
                    journey.page.wait_for_function("!document.querySelector('#attack').disabled");journey.send({'kind':'attack'});pending=wait(lambda:s if (s:=journey.current()['snapshot']).get('pending_action') else None);assert all(v['ready_at_ms']>pending['logical_ms'] for v in pending['player_skills']);host.kill()
                else:
                    pending=journey.current()['snapshot']
                    try:journey.send({'kind':'leave'})
                    except Exception:
                        if host.process.poll() is None:raise
                    assert host.process.wait(timeout=15)==-9
                frozen_state,original_create=host.stored();changed=json.loads((folder/'arena.json').read_text());changed['skill'].update(cost=1,cooldown_ms=1,damage_bps=1);changed['additional_skills'][0].update(cost=2,cooldown_ms=2,damage_bps=2);(folder/'arena.json').write_text(json.dumps(changed));recoveries=[]
                for _ in range(2):
                    host.start();journey.page.goto(host.url);wait(lambda:journey.current() and journey.current()['phase']=='closed' and not journey.current()['ledger']['pending']);value=journey.current();host.stop();state,create=host.stored();assert create==original_create
                    for identity,request in frozen_state['requests']:assert dict(state['requests'])[identity]['body']==request['body']
                    recoveries.append({'closed':assert_closed(value),'ledger':value['ledger'],'balances':state['balances'],'requests':state['requests']})
                assert recoveries[0]==recoveries[1];assert recoveries[0]['ledger']['compensations' if point=='active' else 'settlements']==1
                if point=='active':assert state['balances']==json.loads((folder/'account.json').read_text())['assets']
                assert not journey.errors,journey.errors;crashes[point]={'skillsBeforeCrash':probe,'snapshotBeforeCrash':pending,'recoveries':recoveries,'originalCreate':original_create};print('crash-'+point+': passed',flush=True)
            finally:host.stop();journey.context.close()
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
        import verify_sdk_5_6 as hot_module
        hot_module.OUTPUT=OUTPUT
        for mode in ['extraction','death']:
            folder=root/('legacy24-'+mode);config=hot_module.package(folder)
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=hot_module.HotJourney(browser,host,config)
            try:
                journey.start();probe=journey.hot_probe()
                assert 'healing_received_bps' not in journey.current()['snapshot']
                journey.fight_and_collect();value=journey.finish(mode);host.stop();state,_=host.stored()
                assert not any(e['kind']=='heal_resolved' for e in value['events'])
                assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors
                legacy_results['24-'+mode]={'closed':assert_closed(value),'status':probe,'ledger':value['ledger'],'pageErrors':journey.errors}
                print('legacy24-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        for mode in ['extraction','death']:
            folder=root/('legacy25-'+mode);config=previous.package(folder);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=ReceivedJourney(browser,host,config)
            try:
                journey.start();probe=journey.received_probe();journey.fight_and_collect();value=journey.finish(mode);assert 'player_skills' not in value['snapshot'];assert not any('skill_id' in e for e in value['events'] if e['kind']=='action_committed');host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors
                legacy_results['25-'+mode]={'closed':assert_closed(value),'status':probe,'ledger':value['ledger'],'pageErrors':journey.errors};print('legacy25-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        browser.close()
    assert sources()==frozen,'Behavior inputs changed during acceptance';assert digest(ROOT/'target/debug/kernel-host')==binary
    report={'passed':True,'scope':'SDK-5-8 finite dual projectile skills, local ordinary Node/H5; no production signoff','sourceHashes':frozen,'binarySha256':binary,'checks':checks,'cases':results,'touchCases':touch_results,'crashes':crashes,'legacy':legacy_results,'resourceRejection':resource}
    (OUTPUT/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('SDK-5-8: dual skills, pure touch, insufficient resources, crash recovery and legacy19-25 passed',flush=True)
if __name__=='__main__':main()
