"""SDK-5-13 fixed periodic fields: ordinary H5 and durable recovery."""
import argparse,json,subprocess,tempfile,uuid,re
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_h5 import active,closed,wait
from verify_sdk_5_1 import Host,assert_settlement,digest
import verify_sdk_5_7 as previous
from verify_sdk_5_7 import StableJourney as LegacyStableJourney,ReceivedJourney as LegacyReceivedJourney
ROOT=Path(__file__).resolve().parents[1]
OUTPUT=ROOT/'artifacts/sdk-5-13'
def sources():
    paths=previous.sources()
    for name in ['verify_sdk_5_7','verify_sdk_5_8','verify_sdk_5_9','verify_sdk_5_10','verify_sdk_5_11','verify_sdk_5_12','verify_sdk_5_13']:paths['scripts/'+name+'.py']=digest(ROOT/('scripts/'+name+'.py'))
    return paths
class StableJourney(LegacyStableJourney):
    def send(self,action,*,repeat=False):
        for _ in range(10):
            current=self.current();payload={'connection':current['connection'],'sequence':current['sequence']+1,'requestId':'sdk59-'+str(uuid.uuid4()),'action':action}
            url=f"{self.host.url}/api/sessions/{current['id']}/input";response=self.page.request.post(url,data=payload)
            # Only an explicit rejection establishes that this attempt was never accepted.
            if not response.ok and response.json().get('error')=='STALE_SEQUENCE':
                self.page.wait_for_timeout(50);continue
            assert response.ok,response.text()
            if repeat:
                again=self.page.request.post(url,data=payload)
                if again.ok:assert again.json()==response.json(),again.text()
                else:
                    # An accepted action is never resubmitted across an epoch change.
                    # The exact old request must be fenced, even if it is a duplicate.
                    after=self.current();assert again.json().get('error')=='STALE_CONNECTION' and after['connection']['generation']>payload['connection']['generation'],again.text()
                    self.repeat_handoffs=getattr(self,'repeat_handoffs',[])+[{'requestId':payload['requestId'],'action':action['kind'],'oldConnection':payload['connection'],'newConnection':after['connection'],'duplicateError':'STALE_CONNECTION','acceptedActionResubmitted':False}]
            return payload
        raise AssertionError('Readiness did not settle within the bounded input retry window')
class ReceivedJourney(LegacyReceivedJourney):
    send=StableJourney.send
def package(directory,variant='a',*,legacy=False,touch=False,poor=False,blocked=None):
    if legacy:return previous.package(directory,variant,legacy=True,touch=touch)
    directory.mkdir();sample=ROOT/'examples/contact-host'
    value=json.loads((sample/'arena.json').read_text());account=json.loads((sample/'account.json').read_text())
    value['id']=f'outside-contact-{variant}-{int(touch)}-{int(poor)}-{blocked or "clear"}';value['basic']['windup_ms']=600
    ground=value['skill'];own=value['additional_skills'][0]
    ground.update(windup_ms=900,recovery_ms=300,cooldown_ms=1200,warning_ms=1200,duration_ms=4800,tick_interval_ms=1200);ground['contact_policy']['repeat_interval_ms']=0
    own.update(windup_ms=150,recovery_ms=300,cooldown_ms=5000)
    value['pve']['combat']['heal']['hp_threshold']=0
    for d in value['pve']['status_definitions']:d['duration_ms']=8000
    if variant=='b':
        ground.update(id='outside-budget',cost=14,damage_bps=7500,radius=84,warning_ms=1500,duration_ms=5600,tick_interval_ms=1400,contact_policy={'max_targets_per_tick':1,'max_total_hits':3,'repeat_interval_ms':2800},on_hit_target_statuses=['outside-exposed'])
        own.update(id='outside-own-care',cost=10,cooldown_ms=5200,on_commit_self_statuses=['outside-renewal'])
        for d in value['pve']['status_definitions']:d['id']='outside-'+d['id']
        value['skill'],value['additional_skills'][0]=own,ground
        value['asset_contract']={'entry_cost_asset_id':'pass','reward_asset_id':'merit'};account['assets']={'pass':100,'merit':0,'xuan-tie':100}
    if poor:value['params']['initial_player']['spirit_points']=15;account['stats']['spirit_points']=15
    if blocked=='range':ground['range']=40
    if blocked=='vision':value['pve']['palette'][0]['blocksVision']=True
    for name,data in [('arena',value),('account',account),('catalog',{'schema_version':1,'default_scenario':'outside','active':{'outside':'arena.json'},'archives':[]})]:(directory/(name+'.json')).write_text(json.dumps(data))
    return value
import verify_sdk_5_10 as unit_module
import verify_sdk_5_11 as ground_module
import verify_sdk_5_12 as field_module
class PolicyJourney(field_module.FieldJourney):
    send=StableJourney.send
    def finish_field(self,identity):
        wait(lambda:self.ended(identity));ticks=self.field_ticks(identity);skill=self.skills()[self.index('ground_field')];p=skill['contact_policy'];remaining=p['max_total_hits'];history={}
        assert [t['tick_index'] for t in ticks]==list(range(1,len(ticks)+1))
        for t in ticks:
            assert len(t['hit_targets'])==t['hits']<=p['max_targets_per_tick'];assert len({(x['id'],x['generation']) for x in t['hit_targets']})==t['hits']
            remaining-=t['hits'];assert t['remaining_hits']==remaining>=0
            for target in t['hit_targets']:
                key=(target['id'],target['generation'])
                if key in history:assert p['repeat_interval_ms']>0 and t['scheduled_at_ms']-history[key]>=p['repeat_interval_ms']
                history[key]=t['scheduled_at_ms']
        assert all(b['scheduled_at_ms']-a['scheduled_at_ms']==skill['tick_interval_ms'] for a,b in zip(ticks,ticks[1:]))
        if remaining==0:assert self.ended(identity)['reason']=='hit_budget_exhausted'
        else:assert self.ended(identity)['reason']=='expired' and len(ticks)==(skill['duration_ms']-1)//skill['tick_interval_ms']
        return ticks
    def probe(self,*,touch=False,reconnect=False):
        gi=self.index('ground_field');si=self.index('self_status');button='#cast' if gi==0 else '#cast-secondary'
        self.page.wait_for_function('(id)=>document.querySelector(id).disabled',arg=button)
        wait(lambda:v if (v:=self.current())['snapshot']['hp']<=v['snapshot']['max_hp']-10 else None)
        self.use(si,touch=touch);actual=wait(lambda:v if (v:=self.current()) and any(e['kind']=='periodic_heal_tick' and e['hp_restored']>0 for e in v['events']) else None)
        self.choose([700,400],touch=touch);_,_,empty=self.use(gi,touch=touch);empty_ticks=self.finish_field(empty['action_id']);assert all(t['hits']==0 and t['remaining_hits']==3 for t in empty_ticks)
        self.choose([450,390],touch=touch);old,pending,committed=self.use(gi,touch=touch,switch_to=[700,400]);identity=committed['action_id']
        initial=wait(lambda:f if (f:=next((f for f in self.current()['snapshot']['pve']['player_fields'] if f['action_id']==identity),None)) else None)
        skill=self.skills()[gi];assert initial['center']==pending['ground_position']==committed['ground_position'];assert initial['contact_policy']['used_hits']==0 and initial['contact_policy']['remaining_hits']==3 and not initial['contact_policy']['contacts']
        self.page.wait_for_function("document.querySelector('#arena').dataset.playerFields==='1'")
        if not reconnect:self.page.screenshot(path=str(OUTPUT/(self.config['id']+'-policy-warning.png')),full_page=True)
        first=wait(lambda:v if (v:=self.current()) and any(f['action_id']==identity and f['contact_policy']['used_hits']>=1 for f in v['snapshot']['pve']['player_fields']) else None)
        progress=next(f for f in first['snapshot']['pve']['player_fields'] if f['action_id']==identity);assert progress['contact_policy']['contacts'][0]['target']['id']=='status-target-1'
        self.page.wait_for_function("/历史 [1-9]/.test(document.querySelector('#field-contact-summary').textContent)")
        if not reconnect:self.page.screenshot(path=str(OUTPUT/(self.config['id']+'-policy-active.png')),full_page=True)
        if reconnect:
            self.page.reload();active(self.page);current=self.current();assert current['connection']['generation']>old['connection']['generation'];matches=[f for f in current['snapshot']['pve']['player_fields'] if f['action_id']==identity];assert matches,json.dumps({'beforeProgress':progress,'afterReload':current['snapshot'],'fieldEvents':[e for e in current['events'] if e['kind'].startswith('player_field')]});f=matches[0]
            assert all(f[k]==initial[k] for k in ['action_id','skill_id','center','radius','started_at_ms','activates_at_ms','expires_at_ms','total_ticks']);assert f['contact_policy']['used_hits']>=progress['contact_policy']['used_hits'];assert f['contact_policy']['remaining_hits']<=progress['contact_policy']['remaining_hits']
            for h in progress['contact_policy']['contacts']:assert any(x['target']==h['target'] and x['last_hit_at_ms']>=h['last_hit_at_ms'] for x in f['contact_policy']['contacts'])
            assert current['snapshot']['player_skills']==first['snapshot']['player_skills'] and current['snapshot']['spirit']==first['snapshot']['spirit']
            self.page.wait_for_function("document.querySelector('#ground-target-selected').textContent.includes('点击或触摸')")
            self.page.screenshot(path=str(OUTPUT/(self.config['id']+'-policy-reconnected.png')),full_page=True)
            stale=self.page.request.post(f"{self.host.url}/api/sessions/{current['id']}/input",data=old);assert not stale.ok and stale.json()['error']=='STALE_CONNECTION'
        ticks=self.finish_field(identity);assert len(ticks)==3
        expected=['status-target-1','status-target-2']+(['status-target-1'] if skill['contact_policy']['repeat_interval_ms'] else [])
        assert [t['id'] for tick in ticks for t in tick['hit_targets']]==expected
        assert ticks[0]['scheduled_at_ms']==initial['activates_at_ms']+skill['tick_interval_ms'] and ticks[-1]['scheduled_at_ms']<initial['expires_at_ms']
        after=self.current()['snapshot'];assert all(0<m['hp']<180 and m['statuses'] for m in after['pve']['monsters']);assert after['spirit']==200-self.skills()[si]['cost']-2*skill['cost'];assert after['skill_ready_at_ms']==after['player_skills'][0]['ready_at_ms']
        return {'ids':[s['id'] for s in self.skills()],'fieldIndex':gi,'selfIndex':si,'pending':pending,'committed':committed,'field':initial,'progressBeforeReconnect':progress,'ticks':ticks,'emptyTicks':empty_ticks,'ended':self.ended(identity),'afterField':after,'actualHealing':next(e for e in actual['events'] if e['kind']=='periodic_heal_tick' and e['hp_restored']>0),'reconnected':reconnect,'switchedSelectionDuringPreparation':True}
def assert_closed(value):
    result=previous.assert_closed(value)
    if value.get('recovered'):assert all('unit_target' not in m for m in (value.get('snapshot') or {}).get('pve',{}).get('monsters',[]))
    if value.get('recovered'):assert 'player_fields' not in (value.get('snapshot') or {}).get('pve',{})
    if value.get('recovered'):assert 'player_area_warnings' not in (value.get('snapshot') or {}).get('pve',{})
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
    binary=digest(ROOT/'target/debug/kernel-host');results={};touch_results={};crashes={};legacy_results={};resource={};target_rejections={}
    with tempfile.TemporaryDirectory(prefix='sdk-5-13-outside-') as temporary,sync_playwright() as pw:
        root=Path(temporary);browser=pw.chromium.launch()
        for variant in ['a','b']:
            folder=root/variant;config=package(folder,variant)
            for mode in ['extraction','death']:
                name=variant+'-'+mode;directory=root/name;directory.mkdir();host=Host(directory,folder/'catalog.json',folder/'account.json');host.start();journey=PolicyJourney(browser,host,config,mobile=variant=='b')
                try:
                    journey.start();probe=journey.probe(reconnect=name=='a-extraction');fight=journey.fight();value=journey.finish(mode);assert not journey.errors,journey.errors
                    host.stop();state,create=host.stored();assert create['protocol_version']==3;assert_settlement(state,value,json.loads((folder/'account.json').read_text()))
                    results[name]={'closed':assert_closed(value),'clock':'ordinary','skills':probe,'fight':fight,'ledger':value['ledger'],'result':value['settlement'],'pageErrors':journey.errors,'repeatEpochChanges':getattr(journey,'repeat_handoffs',[])};print(name+': passed',flush=True)
                finally:host.stop();journey.context.close()
        for mode in ['extraction','death']:
            folder=root/('touch-'+mode);config=package(folder,'b',touch=True);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=PolicyJourney(browser,host,config,mobile=True)
            try:
                journey.page.locator('#start').tap();active(journey.page);probe=journey.probe(touch=True);fight=journey.fight(touch=True)
                if mode=='extraction':journey.page.wait_for_function("!document.querySelector('#extract').disabled");journey.page.locator('#extract').tap()
                else:journey.page.locator('#leave').tap()
                closed(journey.page);value=journey.current();host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors
                touch_results[mode]={'closed':assert_closed(value),'skills':probe,'fight':fight,'actions':'touch controls only; HTTP read-only observations','ledger':value['ledger'],'pageErrors':journey.errors};print('touch-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        folder=root/'poor';config=package(folder,poor=True);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=PolicyJourney(browser,host,config)
        try:
            journey.start();journey.choose([450,400]);journey.use(journey.index('ground_field'));wait(lambda:journey.current()['snapshot']['logical_ms']>=journey.current()['snapshot']['recovery_until_ms']);resource=journey.rejected(journey.index('self_status'),'NO_RESOURCE')
            journey.page.wait_for_function("document.querySelector('#cast-secondary').disabled && document.querySelector('#cast-secondary-detail').textContent.includes('灵力不足')")
            value=journey.finish('death');host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));resource.update(closed=assert_closed(value),ledger=value['ledger']);assert not journey.errors,journey.errors;print('resource-rejection: passed',flush=True)
        finally:host.stop();journey.context.close()
        for boundary in ['range','vision']:
            folder=root/('boundary-'+boundary);config=package(folder,blocked=boundary);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=PolicyJourney(browser,host,config)
            try:
                journey.start();journey.choose([450,400]);probe=journey.rejected(journey.index('ground_field'),'OUT_OF_RANGE');value=journey.finish('death');host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors
                target_rejections[boundary]={'probe':probe,'closed':assert_closed(value),'ledger':value['ledger'],'pageErrors':journey.errors};print('boundary-'+boundary+': passed',flush=True)
            finally:host.stop();journey.context.close()
        for point in ['active','after_intent_persist','after_storage_commit']:
            folder=root/('crash-config-'+point);config=package(folder);directory=root/('crash-'+point);directory.mkdir();host=Host(directory,folder/'catalog.json',folder/'account.json');host.start('' if point=='active' else point);journey=PolicyJourney(browser,host,config)
            try:
                journey.start();probe=journey.probe()
                if point=='active':
                    journey.choose([450,390]);_,_,committed=journey.use(journey.index('ground_field'));wait(lambda:journey.field_ticks(committed['action_id']));journey.page.wait_for_function("!document.querySelector('#cast').disabled");journey.send(journey.intent(journey.index('ground_field')));pending=wait(lambda:s if (s:=journey.current()['snapshot']).get('pending_action') and any(f['contact_policy']['used_hits']>=1 for f in s['pve']['player_fields']) else None);host.kill()
                else:
                    pending=journey.current()['snapshot']
                    try:journey.send({'kind':'leave'})
                    except Exception:
                        if host.process.poll() is None:raise
                    assert host.process.wait(timeout=15)==-9
                frozen_state,original_create=host.stored();changed=json.loads((folder/'arena.json').read_text());changed['skill'].update(cost=1,cooldown_ms=1,damage_bps=1,duration_ms=1200,tick_interval_ms=300,contact_policy={'max_targets_per_tick':2,'max_total_hits':1,'repeat_interval_ms':0});changed['additional_skills'][0].update(cost=2,cooldown_ms=2,on_commit_self_statuses=['renewal']);(folder/'arena.json').write_text(json.dumps(changed));recoveries=[]
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
        class LegacyStackJourney(legacy_module.StackJourney,StableJourney):send=StableJourney.send
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
        class LegacyAttributeJourney(attribute_module.StackJourney,StableJourney):send=StableJourney.send
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
        class LegacyShieldJourney(shield_module.ShieldJourney):send=StableJourney.send
        for mode in ['extraction','death']:
            folder=root/('legacy22-'+mode);folder.mkdir()
            for filename in ['arena.json','account.json','catalog.json']:
                shutil.copy2(ROOT/'examples/shield-host'/filename,folder/filename)
            config=json.loads((folder/'arena.json').read_text())
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=LegacyShieldJourney(browser,host,config)
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
        class LegacyDotJourney(dot_module.DotJourney):send=StableJourney.send
        for mode in ['extraction','death']:
            folder=root/('legacy23-'+mode);config=dot_module.package(folder)
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=LegacyDotJourney(browser,host,config)
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
        class LegacyHotJourney(hot_module.HotJourney):send=StableJourney.send
        for mode in ['extraction','death']:
            folder=root/('legacy24-'+mode);config=hot_module.package(folder)
            host=Host(folder,folder/'catalog.json',folder/'account.json');host.start()
            journey=LegacyHotJourney(browser,host,config)
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
        import verify_sdk_5_8 as multi_module
        multi_module.OUTPUT=OUTPUT
        class LegacyMultiJourney(multi_module.MultiJourney):send=StableJourney.send
        for mode in ['extraction','death']:
            folder=root/('legacy26-'+mode);config=multi_module.package(folder);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=LegacyMultiJourney(browser,host,config)
            try:
                journey.start();probe=journey.probe();fight=journey.fight();value=journey.finish(mode);host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors
                legacy_results['26-'+mode]={'closed':assert_closed(value),'skills':probe,'fight':fight,'ledger':value['ledger'],'pageErrors':journey.errors};print('legacy26-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        import verify_sdk_5_9 as self_module
        self_module.OUTPUT=OUTPUT
        class LegacySelfJourney(self_module.SelfJourney):send=StableJourney.send
        for mode in ['extraction','death']:
            folder=root/('legacy27-'+mode);config=self_module.package(folder);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=LegacySelfJourney(browser,host,config)
            try:
                journey.start();probe=journey.probe();fight=journey.fight();value=journey.finish(mode);host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors
                assert all('unit_target' not in m for m in value['snapshot']['pve']['monsters'])
                legacy_results['27-'+mode]={'closed':assert_closed(value),'skills':probe,'fight':fight,'ledger':value['ledger'],'pageErrors':journey.errors};print('legacy27-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        unit_module.OUTPUT=OUTPUT
        class LegacyUnitJourney(unit_module.UnitJourney):send=StableJourney.send
        for mode in ['extraction','death']:
            folder=root/('legacy28-'+mode);config=unit_module.package(folder);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=LegacyUnitJourney(browser,host,config)
            try:
                journey.start();probe=journey.probe();fight=journey.fight();value=journey.finish(mode);host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors;assert 'player_area_warnings' not in value['snapshot']['pve']
                legacy_results['28-'+mode]={'closed':assert_closed(value),'skills':probe,'fight':fight,'ledger':value['ledger'],'pageErrors':journey.errors};print('legacy28-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        ground_module.OUTPUT=OUTPUT
        class LegacyGroundJourney(ground_module.GroundJourney):send=StableJourney.send
        for mode in ['extraction','death']:
            folder=root/('legacy29-'+mode);config=ground_module.package(folder);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=LegacyGroundJourney(browser,host,config)
            try:
                journey.start();probe=journey.probe();fight=journey.fight();value=journey.finish(mode);host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors;assert 'player_fields' not in value['snapshot']['pve']
                legacy_results['29-'+mode]={'closed':assert_closed(value),'skills':probe,'fight':fight,'ledger':value['ledger'],'pageErrors':journey.errors};print('legacy29-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        field_module.OUTPUT=OUTPUT
        class LegacyFieldJourney(field_module.FieldJourney):send=StableJourney.send
        for mode in ['extraction','death']:
            folder=root/('legacy30-'+mode);config=field_module.package(folder);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();journey=LegacyFieldJourney(browser,host,config)
            try:
                journey.start();probe=journey.probe();fight=journey.fight();value=journey.finish(mode);host.stop();state,_=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not journey.errors,journey.errors
                assert all('hit_targets' not in e and 'remaining_hits' not in e for e in value['events'] if e['kind']=='player_field_tick')
                legacy_results['30-'+mode]={'closed':assert_closed(value),'skills':probe,'fight':fight,'ledger':value['ledger'],'pageErrors':journey.errors};print('legacy30-'+mode+': passed',flush=True)
            finally:host.stop();journey.context.close()
        browser.close()
    assert sources()==frozen,'Behavior inputs changed during acceptance';assert digest(ROOT/'target/debug/kernel-host')==binary
    report={'passed':True,'scope':'SDK-5-13 bounded identity contact policy on fixed circle fields, local ordinary Node/H5; no production signoff','sourceHashes':frozen,'binarySha256':binary,'checks':checks,'cases':results,'touchCases':touch_results,'crashes':crashes,'legacy':legacy_results,'resourceRejection':resource,'targetRejections':target_rejections}
    (OUTPUT/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('SDK-5-13: contact budgets and repeat identities, pure touch, insufficient resources, crash recovery and legacy19-30 passed',flush=True)
if __name__=='__main__':main()
