"""SDK-5-16: real ordinary H5 capsule fields, compatibility and durable responsibility."""
import argparse,json,tempfile,subprocess,uuid
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_h5 import active,closed,wait
from verify_sdk_5_1 import Host,assert_settlement,digest
import verify_sdk_5_15 as old
import verify_sdk_5_14 as piercing
import verify_sdk_5_13 as policy
import verify_sdk_5_12 as circle
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/sdk-5-16'
def sources():
    p=old.sources();p={k:v for k,v in p.items() if 'target/' not in k and 'node_modules/' not in k}
    p['scripts/verify_sdk_5_16.py']=digest(Path(__file__));return p
def package(directory,variant='a',*,poor=False,long=False):
    directory.mkdir();value=json.loads((ROOT/'examples/line-field-host/arena.json').read_text());account=json.loads((ROOT/'examples/line-field-host/account.json').read_text())
    value['id']='outside-line-'+variant+('-poor' if poor else '');value['basic']['windup_ms']=600
    skill=value['skill'];own=value['additional_skills'][0]
    skill.update(windup_ms=900,warning_ms=900,duration_ms=4800,tick_interval_ms=1200);skill['contact_policy'].update(max_total_hits=6,repeat_interval_ms=0)
    own.update(windup_ms=150,recovery_ms=300,cooldown_ms=5000);value['pve']['combat']['heal']['hp_threshold']=0
    for definition in value['pve']['status_definitions']:definition['duration_ms']=16000
    if variant=='b':
        skill.update(id='outside-fixed-beam',cost=14,length=90,width=52,warning_ms=1200,duration_ms=5400,tick_interval_ms=600,on_hit_target_statuses=['outside-exposed']);skill['contact_policy'].update(max_targets_per_tick=1,max_total_hits=4,repeat_interval_ms=1200)
        own.update(id='outside-own-care',cost=10,on_commit_self_statuses=['outside-renewal'])
        for definition in value['pve']['status_definitions']:definition['id']='outside-'+definition['id']
        value['skill'],value['additional_skills'][0]=own,skill;value['asset_contract']={'entry_cost_asset_id':'pass','reward_asset_id':'merit'};account['assets']={'pass':100,'merit':0,'xuan-tie':100}
    if poor:value['params']['initial_player']['spirit_points']=15;account['stats']['spirit_points']=15
    if long:skill.update(duration_ms=16000,tick_interval_ms=4000)
    for name,data in [('arena',value),('account',account),('catalog',{'schema_version':1,'default_scenario':'outside','active':{'outside':'arena.json'},'archives':[]})]:(directory/(name+'.json')).write_text(json.dumps(data))
    return value
class LineJourney(old.RepeatJourney):
    def choose(self,p,*,touch=False):
        self.page.bring_to_front()
        self.page.wait_for_function("document.visibilityState==='visible' && document.hasFocus()")
        wait(lambda:v if (v:=self.current())['phase']=='active' and not v['snapshot']['protected'] else None)
        return super().choose(p,touch=touch)
    def field(self,identity):return next((f for f in self.current()['snapshot']['pve']['player_fields'] if f['action_id']==identity),None)
    def ended(self,identity):return next((e for e in self.current()['events'] if e['kind']=='player_field_ended' and identity in e['action_ids']),None)
    def field_ticks(self,identity):return [e for e in self.current()['events'] if e['kind']=='player_field_tick' and e['action_id']==identity]
    def probe(self,*,touch=False,reconnect=False):
        gi=self.index('line_field');si=self.index('self_status');button='#cast' if gi==0 else '#cast-secondary'
        self.page.wait_for_function('(id)=>document.querySelector(id).disabled',arg=button)
        wait(lambda:v if (v:=self.current())['snapshot']['hp']<=v['snapshot']['max_hp']-10 else None)
        self.use(si,touch=touch);actual=wait(lambda:next((e for e in self.current()['events'] if e['kind']=='periodic_heal_tick' and e['hp_restored']>0),None))
        self.choose([700,400],touch=touch);before=self.current()['snapshot'];payload,pending,committed=self.use(gi,touch=touch,switch_to=[400,700]);identity=committed['action_id'];skill=self.skills()[gi]
        initial=wait(lambda:next((e['field'] for e in self.current()['events'] if e['kind']=='player_field_started' and e['field']['action_id']==identity),None))
        line=initial['line'];assert line['start']==before['position']==initial['center'];assert abs(line['end'][0]-line['start'][0]-skill['length'])<.1 and abs(line['end'][1]-line['start'][1])<.1;assert line['width']==2*initial['radius']==skill['width'];assert not initial['contact_policy']['used_hits'] and not initial['contact_policy']['contacts']
        self.page.wait_for_function("document.querySelector('#arena').dataset.lineFields==='1'")
        progress=wait(lambda:f if (f:=self.field(identity)) and f['contact_policy']['used_hits']>=(2 if reconnect else 1) else None)
        self.page.wait_for_function("document.querySelector('#field-contact-summary').textContent.includes('线形') && /历史 [1-9]/.test(document.querySelector('#field-contact-summary').textContent)")
        if reconnect:
            assert progress['completed_ticks']>=3 and len(progress['contact_policy']['contacts'])==1
            self.page.reload();active(self.page);after=self.current();assert after['connection']['generation']>payload['connection']['generation'];f=self.field(identity);assert f and f['line']==line and f['contact_policy']['used_hits']>=progress['contact_policy']['used_hits']
            for h in progress['contact_policy']['contacts']:assert any(n['target']==h['target'] and n['last_hit_at_ms']>=h['last_hit_at_ms'] and n['next_eligible_at_ms']==n['last_hit_at_ms']+skill['contact_policy']['repeat_interval_ms'] for n in f['contact_policy']['contacts'])
            stale=self.page.request.post(f"{self.host.url}/api/sessions/{after['id']}/input",data=payload);assert not stale.ok and stale.json()['error']=='STALE_CONNECTION'
            self.page.wait_for_function("document.querySelector('#piercing-aim').textContent.includes('点击或触摸')")
        self.page.screenshot(path=str(OUTPUT/(self.config['id']+('-touch' if touch else '')+'-line.png')),full_page=True)
        wait(lambda:self.ended(identity),timeout=25);ticks=self.field_ticks(identity);p=skill['contact_policy'];remaining=p['max_total_hits'];last={};targets=[]
        for i,t in enumerate(ticks):
            assert t['tick_index']==i+1 and t['scheduled_at_ms']==initial['activates_at_ms']+(i+1)*skill['tick_interval_ms']<initial['expires_at_ms'];assert len(t['hit_targets'])==t['hits']<=p['max_targets_per_tick'];remaining-=t['hits'];assert t['remaining_hits']==remaining
            for target in t['hit_targets']:
                k=(target['id'],target['generation'])
                if k in last:assert p['repeat_interval_ms']>0 and t['scheduled_at_ms']-last[k]>=p['repeat_interval_ms']
                last[k]=t['scheduled_at_ms'];targets.append(target['id'])
        expected=['status-target-1']*4 if p['repeat_interval_ms'] else ['status-target-1','status-target-2','status-target-3'];assert targets==expected
        assert self.ended(identity)['reason']==('hit_budget_exhausted' if p['repeat_interval_ms'] else 'expired')
        after=self.current()['snapshot'];assert after['spirit']==200-skill['cost']-self.skills()[si]['cost']
        if p['repeat_interval_ms']:assert after['pve']['monsters'][0]['hp']==0 and all(m['hp']==180 for m in after['pve']['monsters'][1:]) and sum(e['kind']=='monster_killed' for e in self.current()['events'])==1
        else:assert all(0<m['hp']<180 for m in after['pve']['monsters'])
        return {'field':initial,'progress':progress,'ticks':ticks,'ended':self.ended(identity),'afterField':after,'actualHealing':actual,'pending':pending,'committed':committed,'reconnected':reconnect,'fieldIndex':gi,'switchedSelectionDuringPreparation':True}
def close_result(j,host,folder,mode,*,touch=False):
    if touch:
        if mode=='extraction':j.page.wait_for_function("!document.querySelector('#extract').disabled");j.page.locator('#extract').tap()
        else:j.page.locator('#leave').tap()
        closed(j.page);value=j.current()
    else:value=j.finish(mode)
    host.stop();state,create=host.stored();assert_settlement(state,value,json.loads((folder/'account.json').read_text()));assert not j.errors,j.errors
    return {'closed':old.assert_closed(value),'ledger':value['ledger'],'result':value['settlement'],'pageErrors':j.errors,'originalCreate':create}
def capacity(browser,root):
    folder=root/'capacity';cfg=package(folder,long=True);cfg['skill']['on_commit_self_statuses']=['ward'];(folder/'arena.json').write_text(json.dumps(cfg));host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();j=LineJourney(browser,host,cfg)
    try:
        j.start();j.choose([400,700]);j.use(0);j.use(0);j.page.wait_for_function("!document.querySelector('#cast').disabled");before=j.current()['snapshot'];assert len(before['pve']['player_fields'])==2
        with j.page.expect_response(lambda r:'/input' in r.url and r.request.method=='POST' and (r.request.post_data_json or {}).get('action',{}).get('skill_id')==cfg['skill']['id']) as response:
            j.page.locator('#cast').click()
        result=response.value;assert result.ok,result.text();payload=result.request.post_data_json;old.repeat_response(j,result,payload)
        rejected=wait(lambda:next((e for e in j.current()['events'] if e['kind']=='action_rejected' and e['action_id']==payload['requestId']),None));assert rejected['reason']=='CAPACITY';after=j.current()['snapshot']
        assert before['spirit']==after['spirit']==176 and before['player_skills']==after['player_skills'] and before['recovery_until_ms']==after['recovery_until_ms'];j.use(1);assert j.current()['snapshot']['spirit']==168
        return {'rejected':rejected,'before':before,'after':after,'selfStillUsable':True,**close_result(j,host,folder,'death')}
    finally:host.stop();j.context.close()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--focus',choices=['a','b','touch']);args=parser.parse_args();OUTPUT.mkdir(parents=True,exist_ok=True)
    for m in [old,piercing,policy,circle]:m.OUTPUT=OUTPUT
    frozen=sources();binary=digest(ROOT/'target/debug/kernel-host');progress=OUTPUT/'h5-progress.json';report={'sourceHashes':frozen,'binarySha256':binary,'cases':{},'touchCases':{},'crashes':{},'legacy':{}}
    if not args.focus and progress.exists():
        report=json.loads(progress.read_text());assert report['binarySha256']==binary
        changed={k for k in set(report['sourceHashes'])|set(frozen) if report['sourceHashes'].get(k)!=frozen.get(k)}
        if changed:
            assert changed=={'scripts/verify_sdk_5_16.py'},changed
            original=report['sourceHashes']['scripts/verify_sdk_5_16.py'];revision=OUTPUT/'harness-revisions'/(original+'.py');assert digest(revision)==original
            report.setdefault('harnessProvenance',[]).append({'revision':str(revision.relative_to(ROOT)),'sha256':original,'binarySha256':binary,'retainedCases':list(report['cases']),'retainedTouchCases':list(report['touchCases']),'unchangedRuntimeAndData':True,'reason':'Verifier sequence ownership and foreground readiness corrected; retained runtime/data and completed journeys unchanged','retainedCapacity':'capacity' in report})
            report['sourceHashes']=frozen
    def save():
        if not args.focus:progress.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    with tempfile.TemporaryDirectory(prefix='sdk-5-16-h5-') as d,sync_playwright() as pw:
        root=Path(d);browser=pw.chromium.launch()
        for variant in ['a','b']:
            for mode in ['extraction','death']:
                name=variant+'-'+mode
                if name in report['cases'] or args.focus and (variant!=args.focus or mode!='extraction'):continue
                folder=root/name;cfg=package(folder,variant);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();j=LineJourney(browser,host,cfg,mobile=variant=='b')
                try:
                    j.start();probe=j.probe(reconnect=name=='b-extraction');fight=j.fight();report['cases'][name]={'skills':probe,'fight':fight,**close_result(j,host,folder,mode)};print(name+': passed',flush=True)
                finally:save();host.stop();j.context.close()
        for mode in ['extraction','death']:
            if mode in report['touchCases'] or args.focus and (args.focus!='touch' or mode!='extraction'):continue
            folder=root/('touch-'+mode);cfg=package(folder,'b');host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();j=LineJourney(browser,host,cfg,mobile=True)
            try:
                j.page.locator('#start').tap();active(j.page);probe=j.probe(touch=True);fight=j.fight(touch=True);report['touchCases'][mode]={'skills':probe,'fight':fight,'actions':'touch UI only; HTTP observations only',**close_result(j,host,folder,mode,touch=True)};print('touch-'+mode+': passed',flush=True)
            finally:save();host.stop();j.context.close()
        if args.focus:
            (OUTPUT/('focus-'+args.focus+'.json')).write_text(json.dumps({'passed':True,**report},ensure_ascii=False,indent=2)+'\n');browser.close();return
        if 'capacity' not in report:report['capacity']=capacity(browser,root);save();print('capacity: passed',flush=True)
        if 'rejections' not in report:
            folder=root/'rejections';cfg=package(folder,poor=True);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();j=LineJourney(browser,host,cfg)
            try:
                j.start();skill=cfg['skill'];bad={}
                for name,action in [('zero-direction',{'kind':'cast','skill_id':skill['id'],'direction':[0,0]}),('wrong-intent',{'kind':'cast_ground','skill_id':skill['id'],'position':[450,400]})]:bad[name]=j.reject_action(action,'INVALID_MESSAGE')
                invalid_closed=close_result(j,host,folder,'death')
                j.context.close()
                folder=root/'resource';cfg=package(folder,poor=True);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();j=LineJourney(browser,host,cfg)
                j.start();j.choose([700,400]);j.use(0);wait(lambda:j.current()['snapshot']['logical_ms']>=j.current()['snapshot']['recovery_until_ms']);resource=j.rejected(1,'NO_RESOURCE')
                report['rejections']={'invalid':bad,'invalidClosed':invalid_closed,'resource':resource,**close_result(j,host,folder,'death')};save();print('rejections: passed',flush=True)
            finally:host.stop();j.context.close()
        for point in ['active','after_intent_persist','after_storage_commit']:
            if point in report['crashes']:continue
            folder=root/('crash-'+point);cfg=package(folder,'b');host=Host(folder,folder/'catalog.json',folder/'account.json');host.start('' if point=='active' else point);j=LineJourney(browser,host,cfg)
            try:
                j.start()
                if point=='active':
                    j.choose([700,400]);_,_,committed=j.use(j.index('line_field'));wait(lambda:f if (f:=j.field(committed['action_id'])) and f['contact_policy']['used_hits']>=2 else None);j.send(j.intent(j.index('line_field')));pending=wait(lambda:s if (s:=j.current()['snapshot']).get('pending_action') and any(f['contact_policy']['used_hits']>=2 for f in s['pve']['player_fields']) else None);host.kill();probe={'active':pending}
                else:
                    probe=j.probe();pending=j.current()['snapshot']
                    try:j.send({'kind':'leave'})
                    except Exception:
                        if host.process.poll() is None:raise
                    assert host.process.wait(timeout=15)==-9
                frozen_state,create=host.stored();changed=json.loads((folder/'arena.json').read_text());next(s for s in [changed['skill'],*changed['additional_skills']] if s['mechanism']=='line_field').update(length=1,width=2,damage_bps=1,cost=1);(folder/'arena.json').write_text(json.dumps(changed));recoveries=[]
                for _ in range(2):
                    host.start();j.page.goto(host.url);wait(lambda:j.current() and j.current()['phase']=='closed' and not j.current()['ledger']['pending']);value=j.current();host.stop();state,original=host.stored();assert original==create
                    for identity,request in frozen_state['requests']:assert dict(state['requests'])[identity]['body']==request['body']
                    recoveries.append({'closed':old.assert_closed(value),'ledger':value['ledger'],'balances':state['balances'],'requests':state['requests']})
                assert recoveries[0]==recoveries[1] and recoveries[0]['ledger']['compensations' if point=='active' else 'settlements']==1;assert not j.errors
                report['crashes'][point]={'before':pending,'probe':probe,'recoveries':recoveries,'originalCreate':create};save();print('crash-'+point+': passed',flush=True)
            finally:host.stop();j.context.close()
        for kind,module,cls,variant in [('circle',policy,policy.PolicyJourney,'a'),('repeat',old,old.RepeatJourney,'b')]:
            for mode in ['extraction','death']:
                name=kind+'-'+mode
                if name in report['legacy']:continue
                folder=root/name;cfg=module.package(folder,variant);host=Host(folder,folder/'catalog.json',folder/'account.json');host.start();j=cls(browser,host,cfg)
                j.send=old.StableJourney.send.__get__(j);j.use=piercing.PiercingJourney.use.__get__(j) if kind=='repeat' else old.ground_use.__get__(j)
                try:
                    j.start();probe=j.probe();fight=j.fight();value=close_result(j,host,folder,mode)
                    if kind=='circle':assert 'line' not in probe['field']
                    report['legacy'][name]={'skills':probe,'fight':fight,**value};save();print('legacy-'+name+': passed',flush=True)
                finally:host.stop();j.context.close()
        browser.close()
    assert frozen==sources() and binary==digest(ROOT/'target/debug/kernel-host');report['passed']=True;(OUTPUT/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');progress.replace(OUTPUT/'h5-completed-progress.json');print('SDK-5-16 H5, recovery and circle/repeat compatibility passed',flush=True)
if __name__=='__main__':main()
