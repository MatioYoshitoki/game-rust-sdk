"""One native counter/lifetime through real H5 controls and recipient delta caches."""
import json
from pathlib import Path
import subprocess,time
from urllib.parse import urlsplit
from uuid import uuid4
from playwright.sync_api import Error,sync_playwright

def verify_stack_window(browser,request,address,scenario,seats,output):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201;room=response.json();links=room['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];states=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=JSON.parse(sessionStorage.getItem('counter-events')||'[]');const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>{window.__probeEvents.push(JSON.parse(e.data));sessionStorage.setItem('counter-events',JSON.stringify(window.__probeEvents));});}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';carrier=own+'.statuses.find(s=>s.definition_id==="seed-carrier")'
        def status(page=actor):return page.evaluate(carrier)
        def events(kind,page=actor):return page.evaluate('kind=>window.__probeEvents.map(r=>r.kind==="private"?r.event:r).filter(e=>e.kind===kind)',kind)
        def fired(id):return [e for e in events('combat_trigger_fired')if e['trigger_id']==id]
        def debuff():return actor.evaluate(own+'.statuses.some(s=>s.definition_id==="test-debuff")')
        def cast(page,id):
            page.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms<=s.logical_ms}',arg=id)
            n=len(events('action_committed',page));page.locator('#skill').select_option(id);page.locator('#cast').click();page.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===n&&!window.__lastSnapshot.self.pending_action',arg=n+1)
        cast(actor,'seed-window');cast(pages[1],'seed-window');assert status()['stacks']==0 and actor.evaluate(own+'.hp')==600;original=status();assert '层数 seed-carrier · 0 / 3'in actor.locator('#own').inner_text();states.append(actor.evaluate(own));cast(pages[1],'seed-strike');assert status(pages[1])['stacks']==1 and status()['stacks']==0
        for i in range(1,4):cast(actor,'seed-strike');assert status()['stacks']==i
        assert not debuff()and len(fired('full-seed-cleanse'))==1;cast(actor,'tag-debuff');cast(actor,'seed-strike');assert debuff()and len(fired('full-seed-cleanse'))==1 and status()['stacks']==3 and actor.evaluate(own+'.hp')==678;assert '层数 seed-carrier · 3 / 3'in actor.locator('#own').inner_text();states.append(actor.evaluate(own))
        cast(actor,'consume-seeds');zero=status();assert zero['stacks']==0 and zero['instance_id']==original['instance_id']and zero['expires_at_ms']==original['expires_at_ms']and zero['window']['recorded']==400 and not events('window_record_ended');states.append(actor.evaluate(own))
        for _ in range(3):cast(actor,'seed-strike')
        assert len(fired('full-seed-cleanse'))==2 and not debuff()and actor.evaluate(own+'.hp')==732 and status()['window']['recorded']==700 and status(pages[1])['stacks']==1;states.append(actor.evaluate(own))
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert status()['instance_id']==original['instance_id']and status()['expires_at_ms']==original['expires_at_ms']and status()['window']['recorded']==700 and len(fired('full-seed-cleanse'))==2
        actor.wait_for_function('!'+carrier,timeout=20000);pages[1].wait_for_function('!'+carrier,timeout=20000);assert actor.evaluate(own+'.hp')==802 and pages[1].evaluate(own+'.hp')==625;end=events('window_record_ended');assert len(end)==1 and end[0]['recorded']==700 and end[0]['raw_hp']==end[0]['hp_restored']==70;assert [e['hp_restored']for e in events('heal_resolved')if e['heal_kind']=='life_steal']==[15,18,21,24,15,18,21];states.append(actor.evaluate(own));assert not events('window_record_ended',pages[2]);cast(actor,'seed-window');exit_carrier=status();assert exit_carrier['stacks']==0;actor.locator('#leave').click();pages[1].wait_for_function('!window.__lastSnapshot.members.some(m=>m.entity.id==="P00"&&m.phase==="active")');actor.wait_for_function('window.__probeEvents.filter(r=>r.event?.kind==="window_record_ended").length===2');cleaned=events('window_record_ended')[1];assert cleaned['reason']=='target_invalid'and cleaned['raw_hp']==cleaned['hp_restored']==0
        for pid,count in [(0,7),(1,1),(2,0)]:
            totals=[e for e in events('combat_action_total',pages[pid]) if e['trigger_id']=='gain-seed']
            assert len(totals)==(0 if seats==4 else count)
            assert len({e['action_id']for e in totals})==len(totals)
            assert all(e['source']['id']==e['action_source']['id']==f'P{pid:02}'and e['contacts']==1 and e['total_amount']==100 and e['fired']for e in totals)
        Path(output,scenario+'.counter-browser-facts.json').write_text(json.dumps({'states':states,'original':original,'exitCarrier':exit_carrier,'eventsByMember':[{'participant':f'P{i:02}','events':page.evaluate('window.__probeEvents')}for i,page in enumerate(pages)]},ensure_ascii=False,indent=2)+'\n')
        for page in pages[1:]:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'initial':0,'maximum':3,'fixedLifeSteal':1500,'layerLifeSteal':300,'fixedReduction':2000,'lifetime':12000 if seats==4 else 15000,'heals':[15,18,21,24,15,18,21],'cleanses':2,'consumedToZero':True,'recorded':700,'expiryHeal':70,'finalHp':802,'peerHp':625,'sameInstance':True,'sameDeadline':True,'reconnected':True,'private':True,'exitCleaned':True,'settled':seats}
    except Exception:
        Path(output,scenario+'.counter-browser-error.json').write_text(json.dumps({'errors':errors,'events':[p.evaluate('window.__probeEvents')for p in pages],'snapshots':[p.evaluate('window.__lastSnapshot')for p in pages]},ensure_ascii=False,indent=2)+'\n');raise
    finally:
        for context in contexts:context.close()

def verify_counter_matrix(args):
    database=args.out/f'counter-rooms-{uuid4().hex}.sqlite';address=f'http://127.0.0.1:{args.port}'
    command=['node',str(args.server),'--binary',str(args.binary),'--configs',str(args.configs/'counter-catalog.json'),'--host-config',str(args.configs/'counter-host.json'),'--database',str(database),'--port',str(args.port)]
    with (args.out/'counter-server.log').open('w')as log:
        server=subprocess.Popen(command,cwd=args.cwd,stdout=log,stderr=subprocess.STDOUT,text=True)
        try:
            with sync_playwright()as p:
                request=p.request.new_context(base_url=address);deadline=time.monotonic()+20
                while True:
                    try:r=request.get('/api/config',timeout=1000)
                    except Error:r=None
                    if r and r.ok:break
                    assert server.poll()is None and time.monotonic()<deadline;time.sleep(.1)
                browser=p.chromium.launch();cases=[verify_stack_window(browser,request,address,s,n,args.out)for s,n in [('counter-four',4),('counter-eight',8)]];browser.close();request.dispose();return cases
        finally:server.terminate();server.wait(timeout=10)
