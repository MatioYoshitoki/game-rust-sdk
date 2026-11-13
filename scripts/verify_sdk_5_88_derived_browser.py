"""Native status-end and delayed plans through an ordinary H5 consumer."""
import hashlib
import json
from pathlib import Path
import subprocess
import time
from urllib.parse import urlsplit
from uuid import uuid4
from playwright.sync_api import Error, sync_playwright

def verify_derived_effects(browser,request,address,scenario,seats,output):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    room=response.json();links=room['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];four=seats==4;states=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=JSON.parse(sessionStorage.getItem('derived-events')||'[]');const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>{window.__probeEvents.push(JSON.parse(e.data));sessionStorage.setItem('derived-events',JSON.stringify(window.__probeEvents));});}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];peer=pages[1];own='window.__lastSnapshot.self'
        for page in [actor,peer]:page.wait_for_function(own+'.hp==='+own+'.max_hp-40')
        def events(page,kind):return page.evaluate('kind=>window.__probeEvents.map(r=>r.kind==="private"?r.event:r).filter(e=>e.kind===kind)',kind)
        def ticks(id):return [e for e in events(actor,'periodic_tick')if e['definition_id']==id]
        def wait_ticks(id,n):actor.wait_for_function('q=>window.__probeEvents.filter(r=>r.event?.kind==="periodic_tick"&&r.event.definition_id===q.id).length===q.n',arg={'id':id,'n':n})
        def cast(page,skill):
            page.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms<=s.logical_ms}',arg=skill)
            count=len(events(page,'action_committed'));page.locator('#skill').select_option(skill);page.locator('#cast').click();page.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===n&&!window.__lastSnapshot.self.pending_action',arg=count+1)
        def reload_actor():actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready')
        cast(actor,'derived-seed');cast(peer,'derived-seed')
        for page in [actor,peer]:page.wait_for_function('window.__probeEvents.filter(r=>r.event?.kind==="periodic_heal_tick").length===2')
        end=events(actor,'status_end_resolved');assert len(end)==1 and end[0]['active_definition_ids']==['end-hot','end-shield']and end[0]['reason']=='expired'and 'failure'not in end[0]
        assert actor.evaluate(own+'.hp==='+own+'.max_hp')and peer.evaluate(own+'.hp==='+own+'.max_hp');assert actor.evaluate('window.__lastStatusEnd.definition_id')=='end-carrier';assert '状态结束' in actor.locator('#lastSelfHeal').inner_text();states.append(actor.evaluate(own))
        cast(actor,'derived-guard');original=actor.evaluate(own+'.statuses.find(s=>s.definition_id==="record-carrier")');actor.wait_for_function(own+'.statuses.some(s=>s.definition_id==="record-carrier"&&s.shield.record.recorded===40)');reload_actor();preserved=actor.evaluate(own+'.statuses.find(s=>s.definition_id==="record-carrier")');assert preserved['instance_id']==original['instance_id']and preserved['expires_at_ms']==original['expires_at_ms']and preserved['shield']['record']['recorded']==40;states.append(actor.evaluate(own))
        wait_ticks('end-dot',2);ended=events(actor,'shield_record_ended');assert len(ended)==1;counter=20 if four else 30;assert (ended[0]['recorded'],ended[0]['raw_counter'],ended[0]['targets_hit'],ended[0]['reason'])==(40,counter,1,'expired');assert [e['raw_damage']for e in ticks('end-dot')]==([10,10]if four else[15,15]);assert not events(actor,'combat_trigger_fired');states.append(actor.evaluate(own))
        cast(actor,'delayed-unit');delayed=actor.evaluate('window.__lastSnapshot.monsters[0].statuses.find(s=>s.definition_id==="delayed")');cast(actor,'derived-focus');reload_actor();after=actor.evaluate('window.__lastSnapshot.monsters[0].statuses.find(s=>s.definition_id==="delayed")');assert after['instance_id']==delayed['instance_id']and after['periodic']['next_tick_at_ms']==delayed['periodic']['next_tick_at_ms']and after['expires_at_ms']==delayed['expires_at_ms']
        wait_ticks('delayed',1);assert ticks('delayed')[0]['raw_damage']==(58 if four else 100)and ticks('delayed')[0]['scheduled_at_ms']==delayed['periodic']['next_tick_at_ms'];assert len(events(actor,'combat_trigger_fired'))==2;states.append(actor.evaluate('window.__lastSnapshot.monsters[0]'))
        private=['status_end_resolved','status_ends_suppressed','periodic_tick','periodic_heal_tick','shield_record_ended']
        for i,page in enumerate(pages):
            pid=f'P{i:02}'
            for kind in private:
                assert all(pid in [e.get('source',{}).get('id'),e.get('target',{}).get('id')]for e in events(page,kind))
        cast(actor,'delayed-unit');assert actor.evaluate('window.__lastSnapshot.monsters[0].statuses.some(s=>s.definition_id==="delayed")');actor.locator('#leave').click();peer.wait_for_function('!window.__lastSnapshot.monsters[0].statuses.some(s=>s.definition_id==="delayed")');hp=peer.evaluate('window.__lastSnapshot.monsters[0].hp');until=peer.evaluate('window.__lastSnapshot.logical_ms')+1500;peer.wait_for_function('t=>window.__lastSnapshot.logical_ms>=t',arg=until);assert peer.evaluate('window.__lastSnapshot.monsters[0].hp')==hp and len(ticks('delayed'))==1
        Path(output,scenario+'.derived-browser-facts.json').write_text(json.dumps({'states':states,'eventsByMember':[{'participant':f'P{i:02}','events':page.evaluate('window.__probeEvents')}for i,page in enumerate(pages)]},ensure_ascii=False,indent=2)+'\n')
        for page in pages[1:]:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'seedEnds':2,'recorded':40,'counter':counter,'dot':[10,10]if four else[15,15],'delayed':58 if four else 100,'sampling':'source_snapshot'if four else'source_live','derivedTriggerCount':0,'ordinaryTriggerCount':2,'reconnected':True,'deadlinePreserved':True,'sourceExitCancelled':True,'privateRecipients':True,'settled':seats}
    except Exception:
        Path(output,scenario+'.derived-browser-error.json').write_text(json.dumps({'errors':errors,'events':[p.evaluate('window.__probeEvents')for p in pages],'snapshots':[p.evaluate('window.__lastSnapshot')for p in pages]},ensure_ascii=False,indent=2)+'\n');raise
    finally:
        for context in contexts:context.close()

def verify_derived_matrix(args):
    database=args.out/f'derived-rooms-{uuid4().hex}.sqlite';address=f'http://127.0.0.1:{args.port}'
    command=['node',str(args.server),'--binary',str(args.binary),'--configs',str(args.configs/'derived-catalog.json'),'--host-config',str(args.configs/'derived-host.json'),'--database',str(database),'--port',str(args.port)]
    with (args.out/'derived-server.log').open('w')as log:
        server=subprocess.Popen(command,cwd=args.cwd,stdout=log,stderr=subprocess.STDOUT,text=True)
        try:
            with sync_playwright()as p:
                request=p.request.new_context(base_url=address);deadline=time.monotonic()+20
                while True:
                    try:r=request.get('/api/config',timeout=1000)
                    except Error:r=None
                    if r and r.ok:break
                    assert server.poll()is None and time.monotonic()<deadline;time.sleep(.1)
                browser=p.chromium.launch();cases=[verify_derived_effects(browser,request,address,scenario,seats,args.out)for scenario,seats in [('derived-four',4),('derived-eight',8)]];browser.close();request.dispose();return cases
        finally:server.terminate();server.wait(timeout=10)
