"""Qualified multi-member activation, atomic status pairs and frozen resource budgets."""
import json
from pathlib import Path
import subprocess,time
from verify_sdk_5_88_browser_actions import arm_resource_action, observed_resource_action, observe_resource_actions
from urllib.parse import urlsplit
from uuid import uuid4
from playwright.sync_api import Error,sync_playwright

def verify_activation(browser,request,address,scenario,seats,output):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201;room=response.json();links=room['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];states=[];samples=[];mode='multiply_base'if seats==4 else'add_current_bps'
    try:
        for page,link in zip(pages,links):
            observe_resource_actions(page, 'activation-events')
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        own='window.__lastSnapshot.self'
        def status(page,id):return page.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.definition_id===id)',id)
        def marks(page):return (status(page,'marks')or{}).get('stacks',0)
        def buff(page):return status(page,'source-power')or status(page,'mastered-power')
        def target(page,index):return page.evaluate('i=>window.__lastSnapshot.monsters[i]',index%2)
        def events(page,kind):return page.evaluate('kind=>window.__probeEvents.map(r=>r.kind==="private"?r.event:r).filter(e=>e.kind===kind)',kind)
        def select_target(page,npc):
            position=page.evaluate(own+'.position');destination=npc['position'];page.locator('#selectGround').click();box=page.locator('#map').bounding_box();page.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast(page,id):
            page.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&!s.self.active_channel&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms<=s.logical_ms}',arg=id)
            n=len(events(page,'action_committed'));page.locator('#skill').select_option(id);page.locator('#cast').click();page.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===n&&!window.__lastSnapshot.self.pending_action',arg=n+1);return events(page,'action_committed')[-1]
        for i,stage in enumerate([1,2,4,0]if seats==4 else[1,3,4,0]):
            page=pages[i];pid=f'P{i:02}'
            for _ in range(3):cast(page,'gather-marks')
            assert marks(page)==3;entity=target(page,i)['entity'];before=target(page,i)['hp'];prior_spirit=10000-sum(e['spirit_removed']for p in pages for e in events(p,'resource_stolen')if e['target']['id']==entity['id']);start=len(events(page,'resource_stolen'));commit=cast(page,'drain');assert commit['hp_cost']==400 and commit['spirit_cost']==2;channel=page.evaluate(own+'.active_channel');assert channel;paired=stage>=2;assert marks(page)==(0 if paired else 3);assert page.evaluate(own+'.effective_atk')==(105 if paired else 100)and page.evaluate(own+'.effective_def')==(105 if paired else 100);source_buff=buff(page);drained=next((s for s in target(page,i)['statuses']if s['definition_id']=='target-drained'and s['source']['id']==pid),None);assert bool(source_buff)==paired and bool(drained)==paired
            if paired:assert source_buff['definition_id']==('mastered-power'if stage==4 else'source-power')and source_buff['expires_at_ms']==channel['started_at_ms']+5000 and drained['expires_at_ms']==source_buff['expires_at_ms']and[m['modifier_bps']for m in drained['modifiers']]==[-500,-500]
            states.append({'pid':pid,'own':page.evaluate(own),'target':target(page,i)});page.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="resource_stolen").length===n&&!window.__lastSnapshot.self.active_channel',arg=start+3);rows=events(page,'resource_stolen')[start:];ratio=1000 if stage==0 else(1300 if seats==4 else 4000);requested_hp=before*ratio//10000;requested_spirit=prior_spirit*ratio//10000;assert sum(e['requested_hp']for e in rows)==sum(e['hp_removed']for e in rows)==requested_hp;assert sum(e['requested_spirit']for e in rows)==sum(e['spirit_removed']for e in rows)==requested_spirit;assert marks(page)==(1 if paired else 3);assert len([e for e in events(page,'status_consumed')if e['definition_id']=='marks'])==(1 if paired else 0);assert[e['raw_damage']for e in events(page,'unit_channel_pulse')[-3:]]==[33,33,34];assert all(e['action_id']==commit['action_id']for e in rows);samples.append({'pid':pid,'stage':stage,'requestedHp':requested_hp,'requestedSpirit':requested_spirit,'marksAfter':marks(page),'sourceBuff':source_buff['definition_id']if source_buff else None,'effectiveAtk':105 if paired else 100})
            if pid=='P02':
                original=buff(page);count=len(events(page,'resource_stolen'));page.reload(wait_until='domcontentloaded');page.wait_for_function('window.__lastSnapshot?.self.ready');assert buff(page)['instance_id']==original['instance_id']and buff(page)['expires_at_ms']==original['expires_at_ms']and len(events(page,'resource_stolen'))==count
        for page in pages[1:3]:page.wait_for_function('!window.__lastSnapshot.self.statuses.some(s=>["source-power","mastered-power"].includes(s.definition_id))');assert page.evaluate(own+'.effective_atk')==100
        states.append({'pid':'P02','own':pages[2].evaluate(own),'target':target(pages[2],2)});assert not any(s['definition_id']=='target-drained'and s['source']['id']=='P02'for s in target(pages[2],2)['statuses'])
        clock_before=status(pages[2],'marks');assert clock_before['stacks']==1;dummy=pages[2].evaluate('window.__lastSnapshot.monsters.find(n=>n.entity.id==="clock-target")');assert dummy['hp']==1;clock_events=len([e for e in events(pages[2],'status_refreshed')if e['status']['definition_id']=='marks']);select_target(pages[2],dummy);cast(pages[2],'drain');clock_channel=pages[2].evaluate(own+'.active_channel');pages[2].wait_for_function('window.__lastSnapshot.monsters.find(n=>n.entity.id==="clock-target").hp===0&&!window.__lastSnapshot.self.active_channel');clock_after=status(pages[2],'marks');assert clock_after['instance_id']==clock_before['instance_id']and clock_after['applied_at_ms']==clock_before['applied_at_ms']and clock_after['stacks']==1 and clock_after['expires_at_ms']>clock_before['expires_at_ms']and 10400<=clock_after['expires_at_ms']-clock_channel['started_at_ms']<=10430;assert len([e for e in events(pages[2],'status_refreshed')if e['status']['definition_id']=='marks'])==clock_events+1;pages[2].reload(wait_until='domcontentloaded');pages[2].wait_for_function('window.__lastSnapshot?.self.ready');assert status(pages[2],'marks')['expires_at_ms']==clock_after['expires_at_ms']and len([e for e in events(pages[2],'status_refreshed')if e['status']['definition_id']=='marks'])==clock_events+1;states.append({'pid':'P02','own':pages[2].evaluate(own),'target':target(pages[2],2)});select_target(pages[2],target(pages[2],2))
        while marks(pages[2])<3:cast(pages[2],'gather-marks')
        n=len(events(pages[2],'resource_stolen'))
        pages[2].bring_to_front()
        arm_resource_action(pages[2],'leave')
        exit_action=cast(pages[2],'drain')['action_id']
        observed_resource_action(pages[2],'leave',exit_action)
        pages[2].wait_for_function(own+'.phase!=="active"&&'+own+'.statuses.length===0')
        pages[0].wait_for_function('!window.__lastSnapshot.members.some(m=>m.entity.id==="P02"&&m.phase==="active")')
        hp=target(pages[0],0)['hp']
        until=pages[0].evaluate('window.__lastSnapshot.logical_ms')+1400
        pages[0].wait_for_function('t=>window.__lastSnapshot.logical_ms>=t',arg=until)
        assert target(pages[0],0)['hp']==hp
        assert [e['action_id']for e in events(pages[2],'resource_stolen')[n:]]==[exit_action]
        for page in pages:assert page.evaluate('!JSON.stringify(window.__lastSnapshot).includes("maximum_mastery")&&!JSON.stringify(window.__lastSnapshot).includes("qualification")&&!JSON.stringify(window.__lastSnapshot).includes("model_id")')
        Path(output,scenario+'.activation-browser-facts.json').write_text(json.dumps({'mode':mode,'states':states,'samples':samples,'clockBefore':clock_before,'clockAfter':clock_after,'eventsByMember':[{'participant':f'P{i:02}','events':page.evaluate('window.__probeEvents')}for i,page in enumerate(pages)]},ensure_ascii=False,indent=2)+'\n')
        for i,page in enumerate(pages):
            if i!=2:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'mode':mode,'samples':samples,'pulses':[33,33,34],'consumed':[0,3,3,0],'marksAfter':[3,1,1,3],'frozenBeforeConsumption':True,'atomicPair':True,'sourceDurations':5000,'killClockRenewed':True,'killRefreshMs':10000,'reconnected':True,'expired':True,'exitPrefix':1,'private':True,'settled':seats}
    except Exception:
        Path(output,scenario+'.activation-browser-error.json').write_text(json.dumps({'errors':errors,'events':[p.evaluate('window.__probeEvents')for p in pages],'snapshots':[p.evaluate('window.__lastSnapshot')for p in pages]},ensure_ascii=False,indent=2)+'\n');raise
    finally:
        for context in contexts:context.close()

def verify_activation_matrix(args):
    database=args.out/f'activation-rooms-{uuid4().hex}.sqlite';address=f'http://127.0.0.1:{args.port}';command=['node',str(args.server),'--binary',str(args.binary),'--configs',str(args.configs/'activation-catalog.json'),'--host-config',str(args.configs/'activation-host.json'),'--database',str(database),'--port',str(args.port)]
    with (args.out/'activation-server.log').open('w')as log:
        server=subprocess.Popen(command,cwd=args.cwd,stdout=log,stderr=subprocess.STDOUT,text=True)
        try:
            with sync_playwright()as p:
                request=p.request.new_context(base_url=address);deadline=time.monotonic()+20
                while True:
                    try:r=request.get('/api/config',timeout=1000)
                    except Error:r=None
                    if r and r.ok:break
                    assert server.poll()is None and time.monotonic()<deadline;time.sleep(.1)
                browser=p.chromium.launch();cases=[verify_activation(browser,request,address,s,n,args.out)for s,n in [('activation-four',4),('activation-eight',8)]];browser.close();request.dispose();return cases
        finally:server.terminate();server.wait(timeout=10)
