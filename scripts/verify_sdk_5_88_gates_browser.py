"""Action provenance, live provider qualification and attempt quotas through H5."""
import json
from pathlib import Path
import subprocess
import time
from urllib.parse import urlsplit
from uuid import uuid4
from playwright.sync_api import Error,sync_playwright

def verify_source_gates(browser,request,address,scenario,seats,output,trusted_control,owned):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201;room=response.json();links=room['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];states=[];quota=2 if seats==4 else 3
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=JSON.parse(sessionStorage.getItem('source-events')||'[]');const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>{window.__probeEvents.push(JSON.parse(e.data));sessionStorage.setItem('source-events',JSON.stringify(window.__probeEvents));});}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';monster='window.__lastSnapshot.monsters[0]'
        def events(page,kind):return page.evaluate('kind=>window.__probeEvents.map(r=>r.kind==="private"?r.event:r).filter(e=>e.kind===kind)',kind)
        def fired(page,id):return [e for e in events(page,'combat_trigger_fired')if e['trigger_id']==id]
        def marker(page):return page.evaluate('window.__lastSnapshot.self.statuses.find(s=>s.definition_id==="intent-marker")?.stacks??0')
        def mark():return actor.evaluate(monster+'.statuses.find(s=>s.definition_id==="target-mark")')
        def cast(page,id):
            page.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms<=s.logical_ms}',arg=id)
            count=len(events(page,'action_committed'));page.locator('#skill').select_option(id);page.locator('#selectGround').click();position=page.evaluate(own+'.position');destination=page.evaluate(monster+'.position');box=page.locator('#map').bounding_box();page.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4});page.locator('#cast').click();page.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===n&&!window.__lastSnapshot.self.pending_action',arg=count+1)
        cast(pages[1],'qualifying-strike');cast(actor,'qualifying-strike');assert pages[1].evaluate(monster+'.statuses.some(s=>s.definition_id==="gated-root"&&s.source.id==="P01")')
        cast(pages[3],'qualifying-strike');actor.wait_for_function('window.__probeEvents.some(r=>r.event?.kind==="contact_reaction_resolved"&&r.event.source.id==="P00")');assert next(e for e in events(actor,'contact_reaction_resolved')if e['source']['id']=='P00')['raw_damage']==25;assert not events(pages[1],'contact_reaction_resolved')
        cast(pages[2],'qualifying-strike')
        for page in pages[:4]:page.wait_for_function('window.__probeEvents.filter(r=>r.event?.kind==="periodic_tick"&&r.event.definition_id==="provenance-dot").length===2')
        assert [marker(p)for p in pages[:4]]==[1,0,0,1];assert [len(fired(p,'selected-damage-return'))for p in pages[:4]]==[3,0,0,3];states.append(actor.evaluate(own))
        cast(actor,'intent-release');cast(actor,'advanced-release');assert marker(actor)==1;actor.wait_for_function('window.__lastSnapshot.logical_ms>='+own+'.recovery_until_ms');actor.locator('#attack').click();actor.wait_for_function(own+'.statuses.some(s=>s.definition_id==="intent-marker"&&s.stacks===2)')
        cast(actor,'quota-field');actor.wait_for_function('q=>!window.__lastSnapshot.areas.length&&window.__lastSnapshot.monsters[0].statuses.some(s=>s.definition_id==="target-mark"&&s.stacks===q)',arg=quota);assert len(fired(actor,'quota-mark'))==quota and marker(actor)==3;original=mark();states.append(actor.evaluate(monster));actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert mark()['instance_id']==original['instance_id']and mark()['expires_at_ms']==original['expires_at_ms']
        before=len(fired(actor,'quota-mark'));cast(actor,'quota-field');actor.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="combat_trigger_fired"&&r.event.trigger_id==="quota-mark").length===n',arg=before+1)
        body={'operation_id':'remove-provider','expected_revision':actor.evaluate(own+'.skill_loadout_revision'),'loadout':{'owned_skill_ids':owned,'equipped_skill_ids':['qualifying-strike','quota-field']}};receipt=trusted_control(room['room_id'],'P00',body);assert 'error'not in receipt;revision=receipt['receipt']['revision'];actor.wait_for_function('r=>window.__lastSnapshot.self.skill_loadout_revision===r',arg=revision);actor.wait_for_function('!window.__lastSnapshot.areas.length');assert len(fired(actor,'quota-mark'))==before+1 and mark()['stacks']==quota+1;states.append(actor.evaluate(own))
        for page in pages:assert page.evaluate('!JSON.stringify(window.__lastSnapshot).includes("action_origin")&&!JSON.stringify(window.__lastSnapshot).includes("minimum_mastery")&&!JSON.stringify(window.__lastSnapshot).includes("model_id")')
        cast(actor,'qualifying-strike');assert actor.evaluate(monster+'.statuses.some(s=>s.definition_id==="provenance-dot")');actor.locator('#leave').click();peer=pages[1];peer.wait_for_function('!window.__lastSnapshot.members.some(m=>m.entity.id==="P00"&&m.phase==="active")');hp=peer.evaluate(monster+'.hp');until=peer.evaluate('window.__lastSnapshot.logical_ms')+1000;peer.wait_for_function('t=>window.__lastSnapshot.logical_ms>=t',arg=until);assert peer.evaluate(monster+'.hp')==hp and len([e for e in events(actor,'periodic_tick')if e['definition_id']=='provenance-dot'])==2
        Path(output,scenario+'.source-gates-browser-facts.json').write_text(json.dumps({'states':states,'receipt':receipt,'eventsByMember':[{'participant':f'P{i:02}','events':page.evaluate('window.__probeEvents')}for i,page in enumerate(pages)]},ensure_ascii=False,indent=2)+'\n')
        for page in pages[1:]:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'initialMarkers':[1,0,0,1],'directAndPeriodicReturns':[3,0,0,3],'thirdPartySource':'P00','reactionRaw':25,'excludedOwnSkills':2,'basicQualified':True,'quota':quota,'afterRevocation':quota+1,'inFlightCompleted':True,'canonicalGrowthWithAlternateEquipped':True,'reconnected':True,'deadlinePreserved':True,'privateSourceMetadata':True,'sourceExitCleaned':True,'settled':seats}
    except Exception:
        Path(output,scenario+'.source-gates-browser-error.json').write_text(json.dumps({'errors':errors,'events':[p.evaluate('window.__probeEvents')for p in pages],'snapshots':[p.evaluate('window.__lastSnapshot')for p in pages]},ensure_ascii=False,indent=2)+'\n');raise
    finally:
        for context in contexts:context.close()

def verify_source_matrix(args):
    database=args.out/f'source-rooms-{uuid4().hex}.sqlite';address=f'http://127.0.0.1:{args.port}'
    command=['node',str(args.server),'--binary',str(args.binary),'--configs',str(args.configs/'source-catalog.json'),'--host-config',str(args.configs/'source-host.json'),'--database',str(database),'--port',str(args.port),'--trusted-control-stdin']
    with (args.out/'source-server.log').open('w')as log:
        server=subprocess.Popen(command,cwd=args.cwd,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
        def trusted_control(room_id,participant_id,body):
            log_path=args.out/'source-server.log';offset=len(log_path.read_text().splitlines());server.stdin.write(json.dumps({'kind':'set_skill_loadout','room_id':room_id,'participant_id':participant_id,**body})+'\n');server.stdin.flush();deadline=time.monotonic()+10
            while True:
                for line in log_path.read_text().splitlines()[offset:]:
                    try:row=json.loads(line)
                    except ValueError:continue
                    if row.get('kind')=='trusted_control_result'and row.get('operation_id')==body['operation_id']:return row
                assert server.poll()is None and time.monotonic()<deadline;time.sleep(.02)
        try:
            with sync_playwright()as p:
                request=p.request.new_context(base_url=address);deadline=time.monotonic()+20
                while True:
                    try:r=request.get('/api/config',timeout=1000)
                    except Error:r=None
                    if r and r.ok:break
                    assert server.poll()is None and time.monotonic()<deadline;time.sleep(.1)
                browser=p.chromium.launch();cases=[]
                for scenario,seats,label in [('source-four',4,'a'),('source-eight',8,'b')]:
                    accounts=json.loads((args.configs/f'source-accounts-{label}.json').read_text());cases.append(verify_source_gates(browser,request,address,scenario,seats,args.out,trusted_control,accounts['participants'][0]['skills']['owned_skill_ids']))
                browser.close();request.dispose();return cases
        finally:server.terminate();server.wait(timeout=10)
