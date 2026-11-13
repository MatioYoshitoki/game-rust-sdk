"""Ordinary UI support submission takes an ally's native NPC control."""
import time
from urllib.parse import urlsplit

def verify_ally_commit(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links]
    pages=[c.new_page() for c in contexts];errors=[]
    try:
        for p,link in zip(pages,links):
            p.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...args){super(...args);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            p.on('pageerror',lambda e:errors.append(str(e)))
            p.on('console',lambda m:errors.append(m.text) if m.type=='error' else None)
            p.goto(address+link['url'],wait_until='domcontentloaded')
        for p in pages:p.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor,holder=pages[2:4];own='window.__lastSnapshot.self'
        source=actor.evaluate(own+'.participant.entity.id');target=holder.evaluate(own+'.participant.entity.id')
        def navigate(page,destination):
            pos=page.evaluate(own+'.position');box=page.locator('#map').bounding_box()
            dx=((destination[0]-destination[1])-(pos[0]-pos[1]))/2
            dy=((destination[0]+destination[1])-(pos[0]+pos[1]))/4
            page.locator('#map').click(position={'x':box['width']/2+dx,'y':box['height']/2+dy})
        navigate(actor,[890,800]);actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-890,window.__lastSnapshot.self.position[1]-800)<5');actor.locator('#stop').click()
        navigate(holder,[930,800]);holder.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='compound-lock')")
        before=holder.evaluate("window.__lastSnapshot.self.statuses.find(s=>s.definition_id==='compound-lock')")
        assert before['source']['id']=='control-source' and before['flags']==(['silence','root'] if seats==4 else ['stun'])
        actor.locator('#skill').select_option('ally-renew');actor.locator('#allyTarget').select_option(label=target);actor.locator('#cast').click()
        actor.wait_for_function('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',arg=before['instance_id'])
        after=actor.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',before['instance_id'])
        assert after=={**before,'target':actor.evaluate(own+'.participant.entity')}
        event="id=>window.__probeEvents.filter(r=>r.event?.kind==='status_transferred'&&r.event.status.instance_id===id).map(r=>r.event)"
        holder.wait_for_function('id=>window.__probeEvents.some(r=>r.event?.kind==="status_transferred"&&r.event.status.instance_id===id)',arg=before['instance_id'])
        assert actor.evaluate(event,before['instance_id'])==holder.evaluate(event,before['instance_id']) and len(actor.evaluate(event,before['instance_id']))==1
        trace=actor.evaluate('window.__probeEvents');commit=next(i for i,r in enumerate(trace) if r.get('event',{}).get('kind')=='action_committed' and r['event'].get('skill_id')=='ally-renew')
        primary=next(i for i,r in enumerate(trace) if r.get('event',{}).get('kind')=='status_applied' and r['event']['status']['definition_id']=='ally-renewal')
        moved=next(i for i,r in enumerate(trace) if r.get('event',{}).get('kind')=='status_transferred' and r['event']['status']['instance_id']==before['instance_id']);assert commit<primary<moved
        for p in [p for p in pages if p not in [actor,holder]]:
            assert not p.evaluate(event,before['instance_id'])
            assert not p.evaluate('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',before['instance_id'])
        # Exercise the public input with the page's authenticated session. The UI
        # disables the button while controlled; the server must reject it too.
        paid=actor.evaluate(own+'.spirit')
        result=request.post('/api/input?'+urlsplit(actor.url).query,data={'kind':'cast_self','skill_id':'focus-power'})
        assert result.status==409 and result.json()['error']=='INVALID_STATE' and actor.evaluate(own+'.spirit')==paid
        holder.locator('#leave').click()
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready')
        assert actor.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',before['instance_id'])==after
        actor.wait_for_function('id=>!window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',arg=before['instance_id'])
        assert actor.evaluate('id=>window.__probeEvents.filter(r=>r.event?.kind==="status_removed"&&r.event.reason==="expired"&&r.event.instance_ids.includes(id)).length',before['instance_id'])==1
        actor.locator('#skill').select_option('focus-power');actor.locator('#cast').click();actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='power-focus')")
        for p in pages:
            if p!=holder:p.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json() for l in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers) and not errors, {'ledgers':ledgers,'errors':errors}
        return {'scenario':scenario,'members':seats,'source':source,'target':target,'originalSource':'control-source','flags':before['flags'],'preserved':True,'privateRecipients':2,'blockedWithoutCost':True,'oldHolderLeft':True,'reconnected':True,'expired':True,'resumed':True,'settled':seats}
    finally:
        for c in contexts:c.close()

def verify_lifecycle_transfer(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for p,link in zip(pages,links):
            p.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            p.on('pageerror',lambda e:errors.append(str(e)));p.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);p.goto(address+link['url'],wait_until='domcontentloaded')
        for p in pages:p.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        origin,old,taker=pages[0],pages[1],pages[3];own='window.__lastSnapshot.self'
        source=origin.evaluate(own+'.participant.entity.id');previous=old.evaluate(own+'.participant.entity.id');target=taker.evaluate(own+'.participant.entity.id')
        def cast(page,skill,ally=None):
            page.wait_for_function("id=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms}",arg=skill)
            page.locator('#skill').select_option(skill)
            if ally:page.locator('#allyTarget').select_option(label=ally)
            page.locator('#cast').click()
        cast(origin,'ally-renew',previous);old.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='ally-renewal'&&s.periodic.ticks_done===1)")
        before=old.evaluate("window.__lastSnapshot.self.statuses.find(s=>s.definition_id==='ally-renewal')");instance=before['instance_id'];old_shield=old.evaluate("window.__lastSnapshot.self.statuses.find(s=>s.definition_id==='overflow-shield')");assert old_shield
        cast(taker,'training-tempo');taker.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='training-tempo')");tempo=taker.evaluate("window.__lastSnapshot.self.statuses.find(s=>s.definition_id==='training-tempo')");assert tempo['applied_at_ms']>=before['applied_at_ms']
        cast(taker,'ally-renew',previous);taker.wait_for_function('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',arg=instance)
        after=taker.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',instance);assert after=={**before,'target':taker.evaluate(own+'.participant.entity')}
        assert old.evaluate('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',old_shield['instance_id'])
        assert not taker.evaluate('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',old_shield['instance_id'])
        transfer_event="id=>window.__probeEvents.filter(r=>r.event?.kind==='status_transferred'&&r.event.status.instance_id===id).map(r=>r.event)"
        expected=taker.evaluate(transfer_event,instance);assert len(expected)==1
        for p in [origin,old]:
            p.wait_for_function('id=>window.__probeEvents.some(r=>r.event?.kind==="status_transferred"&&r.event.status.instance_id===id)',arg=instance);assert p.evaluate(transfer_event,instance)==expected
        for p in [p for p in pages if p not in [origin,old,taker]]:assert not p.evaluate(transfer_event,instance)
        old.locator('#leave').click();taker.reload(wait_until='domcontentloaded');taker.wait_for_function('window.__lastSnapshot?.self.ready')
        resumed=taker.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',instance)
        assert all(resumed[k]==after[k] for k in ['instance_id','source','target','applied_at_ms','expires_at_ms','window','cleanse'])
        taker.wait_for_function('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)?.periodic.ticks_done===3',arg=instance)
        assert not taker.evaluate('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',tempo['instance_id'])
        cast(taker,'training-tempo');taker.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='training-tempo')");late=taker.evaluate("window.__lastSnapshot.self.statuses.find(s=>s.definition_id==='training-tempo')")
        ended="id=>window.__probeEvents.filter(r=>r.event?.kind==='window_record_ended'&&r.event.instance_id===id).map(r=>r.event)"
        taker.wait_for_function('id=>window.__probeEvents.some(r=>r.event?.kind==="window_record_ended"&&r.event.instance_id===id)',arg=instance);end=taker.evaluate(ended,instance);assert len(end)==1 and end[0]['spirit_restored']==2
        origin.wait_for_function('id=>window.__probeEvents.some(r=>r.event?.kind==="window_record_ended"&&r.event.instance_id===id)',arg=instance);assert origin.evaluate(ended,instance)==end
        cleanse="id=>window.__probeEvents.filter(r=>r.event?.kind==='status_cleanse_resolved'&&r.event.instance_id===id).map(r=>r.event)"
        rows=taker.evaluate(cleanse,instance);assert [r['removed_count'] for r in rows]==[1,0,1] and [r['phase'] for r in rows]==['tick','tick','expired']
        assert [r['scheduled_at_ms']-before['applied_at_ms'] for r in rows[:2]]==[2000,3000]
        assert origin.evaluate(cleanse,instance)[1:]==rows
        ticks=taker.evaluate("id=>window.__probeEvents.filter(r=>r.event?.kind==='periodic_heal_tick'&&r.event.instance_id===id).map(r=>r.event)",instance);assert [r['tick_index'] for r in ticks]==[2,3]
        assert all(r['source']['id']==source and r['target']['id']==target and r['scheduled_at_ms']==before['applied_at_ms']+1000*r['tick_index'] for r in ticks)
        trace=taker.evaluate('window.__probeEvents');clean_index=next(i for i,r in enumerate(trace) if r.get('event',{}).get('kind')=='status_cleanse_resolved' and r['event']['instance_id']==instance and r['event']['phase']=='expired');end_index=next(i for i,r in enumerate(trace) if r.get('event')==end[0]);assert clean_index<end_index
        assert not taker.evaluate('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',late['instance_id'])
        for p in [p for p in pages if p not in [origin,old,taker]]:assert not p.evaluate(ended,instance) and not p.evaluate(cleanse,instance)
        for p in pages:
            if p!=old:p.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json() for l in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'source':source,'previous':previous,'target':target,'ticksBefore':1,'ticksAfter':[2,3],'cleanseRemoved':[1,0,1],'windowSpirit':2,'transferRecipients':3,'subsequentRecipients':2,'oldShieldIsolated':True,'oldHolderLeft':True,'reconnected':True,'expired':True,'settled':seats}
    finally:
        for c in contexts:c.close()

def verify_control_policy(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    room=response.json();links=room['links'];contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for p,link in zip(pages,links):
            p.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            p.on('pageerror',lambda e:errors.append(str(e)));p.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);p.goto(address+link['url'],wait_until='domcontentloaded')
        for p in pages:p.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';npc="window.__lastSnapshot.monsters.find(n=>n.entity.id==='replacement-target')"
        def cast(target):
            actor.wait_for_function("()=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id==='focus-target').ready_at_ms}")
            actor.locator('#skill').select_option('focus-target');actor.locator('#selectGround').click()
            position=actor.evaluate(own+'.position');destination=actor.evaluate('id=>window.__lastSnapshot.monsters.find(n=>n.entity.id===id).position',target);box=actor.locator('#map').bounding_box()
            actor.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4});actor.locator('#cast').click()
        position=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();actor.locator('#map').click(position={'x':box['width']/2+(200-(position[0]-position[1]))/2,'y':box['height']/2+(800-(position[0]+position[1]))/4});actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-500,window.__lastSnapshot.self.position[1]-300)<5');actor.locator('#stop').click()
        cast('capped-target');actor.wait_for_function("window.__lastSnapshot.monsters.find(n=>n.entity.id==='capped-target').statuses.some(s=>s.definition_id==='compound-lock')");capped=actor.evaluate("window.__lastSnapshot.monsters.find(n=>n.entity.id==='capped-target').statuses.find(s=>s.definition_id==='compound-lock')");assert capped['expires_at_ms']-capped['applied_at_ms']==1800
        cast('replacement-target');actor.wait_for_function(npc+".statuses.some(s=>s.definition_id==='control-softened')");first=actor.evaluate(npc+".statuses.find(s=>s.definition_id==='control-softened')");duration=3600 if seats==4 else 4500
        assert first['expires_at_ms']-first['applied_at_ms']==duration and first['modifiers']==[{'attribute':'movement_slow','modifier_bps':-5000},{'attribute':'basic_interval_delay','modifier_bps':1500}]
        assert not actor.evaluate(npc+".statuses.some(s=>s.definition_id==='compound-lock')")
        cast('replacement-target');actor.wait_for_function("arg=>window.__lastSnapshot.monsters.find(n=>n.entity.id==='replacement-target').statuses.some(s=>s.instance_id===arg[0]&&s.applied_at_ms>arg[1])",arg=[first['instance_id'],first['applied_at_ms']]);refreshed=actor.evaluate(npc+".statuses.find(s=>s.definition_id==='control-softened')")
        assert refreshed=={**first,'applied_at_ms':refreshed['applied_at_ms'],'expires_at_ms':refreshed['expires_at_ms']} and refreshed['expires_at_ms']-refreshed['applied_at_ms']==duration
        for peer in pages[1:]:assert not peer.evaluate('id=>window.__probeEvents.some(r=>r.event?.status?.instance_id===id||r.event?.instance_ids?.includes(id))',first['instance_id'])
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate(npc+".statuses.find(s=>s.definition_id==='control-softened')")==refreshed
        pos=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();dx=((588-400)-(pos[0]-pos[1]))/2;dy=((588+400)-(pos[0]+pos[1]))/4;actor.locator('#map').click(position={'x':box['width']/2+dx,'y':box['height']/2+dy})
        actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-588,window.__lastSnapshot.self.position[1]-400)<5');actor.locator('#stop').click()
        actor.wait_for_function('id=>!window.__lastSnapshot.monsters.find(n=>n.entity.id==="replacement-target").statuses.some(s=>s.instance_id===id)',arg=first['instance_id'])
        assert actor.evaluate('id=>window.__probeEvents.some(r=>r.event?.kind==="status_removed"&&r.event.reason==="expired"&&r.event.instance_ids.includes(id))',first['instance_id'])
        for p in pages:p.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json() for l in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'cap':1800,'duration':duration,'movementSlow':-5000,'intervalDelay':1500,'interval':690 if seats==4 else 930,'timingProbe':{'room':room['room_id'],'before':refreshed['expires_at_ms']},'refreshed':True,'reconnected':True,'expired':True,'privateRecipients':1,'settled':seats}
    finally:
        for c in contexts:c.close()


def verify_control_policy_timing(cases,database):
    # The host intentionally holds an exclusive SQLite lock. Read its native
    # journal only after shutdown rather than loosening production durability.
    import json,sqlite3
    with sqlite3.connect('file:'+str(database)+'?mode=ro',uri=True) as connection:
        for case in cases:
            probe=case.pop('timingProbe')
            outputs=[json.loads(r[0]) for r in connection.execute('select body from duo_outputs where room_id=?',[probe['room']])]
            attacks=[r['logical_ms'] for r in outputs if r['logical_ms']<probe['before'] and r['body']['kind']=='event_v4' and r['body']['event']['kind']=='damage' and r['body']['event']['source']['id']=='replacement-target' and r['body']['event']['target']['id']=='P00']
            assert len(attacks)>=2,attacks
            assert attacks[1]-attacks[0]==case['interval'],attacks


def verify_hit_status(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';target='capped-target' if seats==4 else 'replacement-target';npc="window.__lastSnapshot.monsters.find(n=>n.entity.id==='"+target+"')"
        def point(destination,select=False):
            if select:actor.locator('#selectGround').click()
            position=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();actor.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast():
            actor.wait_for_function("()=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id==='mark-bolt').ready_at_ms}")
            actor.locator('#skill').select_option('mark-bolt');point(actor.evaluate(npc+'.position'),True);actor.locator('#cast').click()
        point([500,300]);actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-500,window.__lastSnapshot.self.position[1]-300)<5');actor.locator('#stop').click()
        marks=[]
        for layers in [1,2]:
            cast();actor.wait_for_function('arg=>window.__lastSnapshot.monsters.find(n=>n.entity.id===arg[0]).statuses.some(s=>s.definition_id==="threshold-mark"&&s.stacks===arg[1])',arg=[target,layers]);marks.append(actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')"))
        assert marks[0]['instance_id']==marks[1]['instance_id'] and marks[1]['modifier_bps']==(-800 if seats==4 else -1000)
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')")==marks[1]
        cast();effect='compound-lock' if seats==4 else 'control-softened';actor.wait_for_function('arg=>window.__lastSnapshot.monsters.find(n=>n.entity.id===arg[0]).statuses.some(s=>s.definition_id===arg[1])',arg=[target,effect]);converted=actor.evaluate('arg=>window.__lastSnapshot.monsters.find(n=>n.entity.id===arg[0]).statuses.find(s=>s.definition_id===arg[1])',[target,effect])
        assert not actor.evaluate(npc+".statuses.some(s=>s.definition_id==='threshold-mark')") and converted['expires_at_ms']-converted['applied_at_ms']==(1200 if seats==4 else 1800)
        consumed=actor.evaluate('id=>window.__probeEvents.filter(r=>r.event?.kind==="status_consumed"&&r.event.instance_id===id).map(r=>r.event)',marks[0]['instance_id']);assert len(consumed)==1 and consumed[0]['consumed_stacks']==3
        actor.wait_for_function('arg=>!window.__lastSnapshot.monsters.find(n=>n.entity.id===arg[0]).statuses.some(s=>s.instance_id===arg[1])',arg=[target,converted['instance_id']])
        for peer in pages[1:]:assert not peer.evaluate('ids=>window.__probeEvents.some(r=>ids.includes(r.event?.status?.instance_id)||r.event?.instance_ids?.some(id=>ids.includes(id)))',[marks[0]['instance_id'],converted['instance_id']])
        cast();actor.wait_for_function(npc+".statuses.some(s=>s.definition_id==='threshold-mark')");partial=actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark').instance_id");actor.locator('#leave').click()
        actor.wait_for_function('arg=>!window.__lastSnapshot.monsters.find(n=>n.entity.id===arg[0]).statuses.some(s=>s.instance_id===arg[1])',arg=[target,partial])
        for page in pages[1:]:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'layers':[1,2,3],'consumed':3,'effect':effect,'duration':1200 if seats==4 else 1800,'source':'P00','privateRecipients':1,'reconnected':True,'expired':True,'sourceLeftWithMark':True,'settled':seats}
    finally:
        for context in contexts:context.close()


def verify_contact_modifier(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';npc="window.__lastSnapshot.monsters.find(n=>n.entity.id==='capped-target')"
        def point(destination,select=False):
            if select:actor.locator('#selectGround').click()
            position=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();actor.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast(skill):
            actor.wait_for_function("id=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms}",arg=skill)
            actor.locator('#skill').select_option(skill);point(actor.evaluate(npc+'.position'),True);start=actor.evaluate('window.__probeEvents.length');actor.locator('#cast').click()
            actor.wait_for_function('start=>window.__probeEvents.slice(start).some(r=>{const e=r.event??r;return e.kind==="damage"&&e.source.id==="P00"&&e.target.id==="capped-target"})',arg=start)
            events=actor.evaluate('start=>window.__probeEvents.slice(start).map(r=>r.event??r)',start);hits=[e for e in events if e['kind']=='damage' and e['source']['id']=='P00' and e['target']['id']=='capped-target'];assert len(hits)==1
            return hits[0]['amount'],events
        point([500,300]);actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-500,window.__lastSnapshot.self.position[1]-300)<5');actor.locator('#stop').click()
        damage=[cast('break-bolt')[0]];consumed=[];marks=[]
        for layers in [1,2,3]:
            for _ in range(layers):cast('mark-bolt')
            actor.wait_for_function('n=>window.__lastSnapshot.monsters.find(npc=>npc.entity.id==="capped-target").statuses.some(s=>s.definition_id==="threshold-mark"&&s.stacks===n)',arg=layers);before=actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')");marks.append(before)
            if layers==2:
                actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')")==before
            amount,events=cast('break-bolt');damage.append(amount);used=[e for e in events if e['kind']=='status_consumed' and e['instance_id']==before['instance_id']];assert len(used)==1;consumed.append(used[0]['consumed_stacks']);remaining=layers-min(layers,3 if seats==4 else 2)
            if remaining:
                actor.wait_for_function('id=>window.__lastSnapshot.monsters.find(n=>n.entity.id==="capped-target").statuses.some(s=>s.instance_id===id&&s.stacks===1)',arg=before['instance_id']);assert actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')")=={**before,'stacks':remaining}
                amount,events=cast('break-bolt');damage.append(amount);used=[e for e in events if e['kind']=='status_consumed' and e['instance_id']==before['instance_id']];assert len(used)==1;consumed.append(used[0]['consumed_stacks'])
            actor.wait_for_function('!'+npc+".statuses.some(s=>s.definition_id==='threshold-mark')")
        damage.append(cast('break-bolt')[0]);assert damage==([50,64,78,94,50] if seats==4 else [50,78,111,111,78,50]),damage;assert consumed==([1,2,3] if seats==4 else [1,2,2,1])
        for peer in pages[1:]:assert not peer.evaluate('ids=>window.__probeEvents.some(r=>r.event?.kind==="status_consumed"&&ids.includes(r.event.instance_id))',[s['instance_id'] for s in marks])
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'damage':damage,'consumed':consumed,'partial':seats==8,'reconnected':True,'privateRecipients':1,'settled':seats}
    finally:
        for context in contexts:context.close()


def verify_chain_damage(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';npc="window.__lastSnapshot.monsters.find(n=>n.entity.id==='capped-target')";ids=['chain-a','chain-b','chain-c'];expected=[80,60,54] if seats==4 else [50,25,18]
        def point(destination,select=False):
            if select:actor.locator('#selectGround').click()
            position=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();actor.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast():
            actor.wait_for_function("()=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id==='mark-bolt').ready_at_ms}")
            actor.locator('#skill').select_option('mark-bolt');point(actor.evaluate(npc+'.position'),True);actor.locator('#cast').click()
        point([500,300]);actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-500,window.__lastSnapshot.self.position[1]-300)<5');actor.locator('#stop').click();marks=[]
        for layers in [1,2]:
            cast();actor.wait_for_function('n=>window.__lastSnapshot.monsters.find(x=>x.entity.id==="capped-target").statuses.some(s=>s.definition_id==="threshold-mark"&&s.stacks===n)',arg=layers);marks.append(actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')"))
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')")==marks[1]
        cast();actor.wait_for_function('window.__lastChainDamage?.hops.length===3');chain=actor.evaluate('window.__lastChainDamage');assert chain['source']['id']=='P00' and chain['reason']=='complete' and chain['initial_raw']==expected[0]
        assert [h['target']['id'] for h in chain['hops']]==ids and [h['from']['id'] for h in chain['hops']]==['capped-target','chain-a','chain-b'] and [h['raw_damage'] for h in chain['hops']]==expected and all(h['applied'] for h in chain['hops'])
        actor.wait_for_function('args=>args[0].every((id,i)=>{const n=window.__lastSnapshot.monsters.find(n=>n.entity.id===id);return n.hp===10000-args[1][i]&&n.statuses.some(s=>s.source.id==="P00")})',arg=[ids,expected])
        statuses=actor.evaluate('ids=>ids.map(id=>window.__lastSnapshot.monsters.find(n=>n.entity.id===id).statuses.find(s=>s.source.id==="P00"))',ids);assert [s['definition_id'] for s in statuses]==['compound-lock','compound-lock','control-softened'] and all(s['expires_at_ms']-s['applied_at_ms']==(1200 if seats==4 else 1800) for s in statuses)
        assert not actor.evaluate(npc+".statuses.some(s=>s.definition_id==='threshold-mark')") and actor.evaluate(npc+'.hp')==(9997 if seats==4 else 9700)
        used=actor.evaluate('id=>window.__probeEvents.filter(r=>r.event?.kind==="status_consumed"&&r.event.instance_id===id).map(r=>r.event)',marks[0]['instance_id']);assert len(used)==1 and used[0]['consumed_stacks']==3
        assert '连锁跳跃 3 次' in actor.locator('#own').inner_text()
        for peer in pages[1:]:assert not peer.evaluate('window.__probeEvents.some(r=>r.event?.kind==="chain_damage_resolved")')
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate('window.__lastChainDamage===undefined');assert actor.evaluate('ids=>ids.map(id=>window.__lastSnapshot.monsters.find(n=>n.entity.id===id).hp)',ids)==[10000-n for n in expected]
        actor.wait_for_function('ids=>ids.every(id=>!window.__lastSnapshot.monsters.find(n=>n.entity.id===id).statuses.some(s=>s.source.id==="P00"))',arg=ids)
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'damage':expected,'targets':ids,'consumed':3,'privateRecipients':1,'reconnected':True,'noReplay':True,'expired':True,'statuses':['compound-lock','compound-lock','control-softened'],'settled':seats}
    finally:
        for context in contexts:context.close()

def verify_post_consumption(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';npc="window.__lastSnapshot.monsters.find(n=>n.entity.id==='capped-target')";raw=175 if seats==4 else 130;pursuit=105 if seats==4 else 52;bps=3000 if seats==4 else 5000;fixed=0 if seats==4 else 60
        def point(destination,select=False):
            if select:actor.locator('#selectGround').click()
            position=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();actor.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast(skill):
            actor.wait_for_function("id=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms}",arg=skill)
            actor.locator('#skill').select_option(skill);point(actor.evaluate(npc+'.position'),True);actor.locator('#cast').click()
        point([500,300]);actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-500,window.__lastSnapshot.self.position[1]-300)<5');actor.locator('#stop').click();marks=[]
        for layers in [1,2]:
            cast('mark-bolt');actor.wait_for_function('n=>window.__lastSnapshot.monsters.find(x=>x.entity.id==="capped-target").statuses.some(s=>s.definition_id==="threshold-mark"&&s.stacks===n)',arg=layers);marks.append(actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')"))
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')")==marks[1]
        cast('mark-bolt');actor.wait_for_function(npc+".statuses.some(s=>s.definition_id==='threshold-mark'&&s.stacks===3)");cast('break-bolt');actor.wait_for_function('window.__lastCooldownReturn&&window.__lastChainDamage?.hops.length===1')
        chain=actor.evaluate('window.__lastChainDamage');refund=actor.evaluate('window.__lastCooldownReturn');change=refund['changes'][0]
        assert [(h['target']['id'],h['raw_damage'],h['applied']) for h in chain['hops']]==[('chain-a',pursuit,True)] and chain['reason']=='complete'
        assert change['skill_id']=='break-bolt' and change['reduced_ms']==min(change['previous_ready_at_ms']-refund['at_ms'],(change['previous_ready_at_ms']-refund['at_ms'])*bps//10000+fixed) and change['ready_at_ms']==change['previous_ready_at_ms']-change['reduced_ms']
        actor.wait_for_function('expected=>window.__lastSnapshot.monsters.find(n=>n.entity.id==="capped-target").hp===expected',arg=10000-3-raw);assert actor.evaluate('window.__lastSnapshot.monsters.find(n=>n.entity.id==="near-behind").hp')==10000
        assert actor.evaluate('window.__lastSnapshot.monsters.find(n=>n.entity.id==="chain-a").hp')==10000-pursuit
        used=actor.evaluate('id=>window.__probeEvents.filter(r=>r.event?.kind==="status_consumed"&&r.event.instance_id===id).map(r=>r.event)',marks[0]['instance_id']);assert len(used)==1 and used[0]['consumed_stacks']==3
        assert '冷却返还 break-bolt' in actor.locator('#own').inner_text() and '连锁跳跃 1 次' in actor.locator('#own').inner_text()
        for peer in pages[1:]:assert not peer.evaluate('window.__probeEvents.some(r=>["skill_cooldowns_reduced","chain_damage_resolved"].includes(r.event?.kind))')
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate('window.__lastCooldownReturn===undefined&&window.__lastChainDamage===undefined');assert actor.evaluate(own+".player_skills.find(s=>s.skill_id==='break-bolt').ready_at_ms")==change['ready_at_ms']
        cast('break-bolt');assert actor.evaluate('window.__lastSnapshot.logical_ms')<change['previous_ready_at_ms'];actor.wait_for_function('expected=>window.__lastSnapshot.monsters.find(n=>n.entity.id==="capped-target").hp===expected',arg=10000-3-raw-100)
        assert actor.evaluate('window.__lastCooldownReturn===undefined&&window.__lastChainDamage===undefined');actor.wait_for_function('!window.__lastSnapshot.monsters.find(n=>n.entity.id==="chain-a").statuses.some(s=>s.source.id==="P00")')
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'consumed':3,'raw':raw,'pursuit':pursuit,'target':'chain-a','skippedBehind':True,'remainingBps':bps,'fixedMs':fixed,'privateRecipients':1,'reconnected':True,'noReplay':True,'readyBeforeOriginal':True,'settled':seats}
    finally:
        for context in contexts:context.close()

def verify_contact_reaction(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        provoker,source=pages[:2];own='window.__lastSnapshot.self';npc="window.__lastSnapshot.monsters.find(n=>n.entity.id==='capped-target')";expected=25 if seats==4 else 67
        def point(page,destination,select=False):
            if select:page.locator('#selectGround').click()
            position=page.evaluate(own+'.position');box=page.locator('#map').bounding_box();page.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast(page):
            page.wait_for_function("()=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id==='mark-bolt').ready_at_ms}")
            page.locator('#skill').select_option('mark-bolt');point(page,page.evaluate(npc+'.position'),True);page.locator('#cast').click()
        for page,pos in [(provoker,[500,300]),(source,[480,240])]:
            point(page,pos);page.wait_for_function('p=>Math.hypot(window.__lastSnapshot.self.position[0]-p[0],window.__lastSnapshot.self.position[1]-p[1])<5',arg=pos);page.locator('#stop').click()
        marks=[]
        for layers in [1,2]:
            cast(source);source.wait_for_function('n=>window.__lastSnapshot.monsters.find(x=>x.entity.id==="capped-target").statuses.some(s=>s.definition_id==="threshold-mark"&&s.stacks===n)',arg=layers);marks.append(source.evaluate(npc+".statuses.find(s=>s.definition_id==='threshold-mark')"))
        cast(source);source.wait_for_function(npc+".statuses.some(s=>s.definition_id==='compound-lock')");frozen=source.evaluate(npc+".statuses.find(s=>s.definition_id==='compound-lock')");assert frozen['source']['id']=='P01' and source.evaluate(npc+'.hp')==9997 and source.evaluate('window.__lastContactReaction===undefined')
        source.reload(wait_until='domcontentloaded');source.wait_for_function('window.__lastSnapshot?.self.ready');assert source.evaluate(npc+".statuses.find(s=>s.definition_id==='compound-lock')")==frozen
        cast(provoker);source.wait_for_function('window.__lastContactReaction!==undefined');r=source.evaluate('window.__lastContactReaction');assert r['source']['id']=='P01' and r['provoker']['id']=='P00' and r['target']['id']=='capped-target' and r['raw_damage']==r['hp_damage']==expected and r['absorbed']==0
        source.wait_for_function('hp=>window.__lastSnapshot.monsters.find(n=>n.entity.id==="capped-target").hp===hp',arg=9996-expected);assert not source.evaluate(npc+".statuses.some(s=>s.definition_id==='compound-lock')")
        used=source.evaluate('id=>window.__probeEvents.filter(r=>r.event?.kind==="status_consumed"&&r.event.instance_id===id).map(r=>r.event)',frozen['instance_id']);assert len(used)==1 and used[0]['consumed_stacks']==1
        assert '接触反应 · 来源 P01' in source.locator('#own').inner_text()
        for page in [provoker,*pages[2:]]:assert page.evaluate('window.__lastContactReaction===undefined')
        source.reload(wait_until='domcontentloaded');source.wait_for_function('window.__lastSnapshot?.self.ready');assert source.evaluate('window.__lastContactReaction===undefined') and source.evaluate(npc+'.hp')==9996-expected
        cast(provoker);source.wait_for_function('hp=>window.__lastSnapshot.monsters.find(n=>n.entity.id==="capped-target").hp===hp',arg=9995-expected);assert source.evaluate('window.__lastContactReaction===undefined')
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'source':'P01','provoker':'P00','raw':expected,'hpDamage':expected,'consumed':1,'privateRecipients':1,'preexistingOnly':True,'reconnected':True,'noReplay':True,'once':True,'settled':seats}
    finally:
        for context in contexts:context.close()

def verify_finisher(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';ordinary=400 if seats==4 else 200;replacement=100 if seats==4 else 150;replacement_hp=300 if seats==4 else 50;targets=['capped-target','replacement-target','guard-target']
        def point(destination,select=False):
            if select:actor.locator('#selectGround').click()
            position=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();actor.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast(id):
            actor.wait_for_function("()=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id==='threshold-bolt').ready_at_ms}")
            actor.locator('#skill').select_option('threshold-bolt');point(actor.evaluate('id=>window.__lastSnapshot.monsters.find(n=>n.entity.id===id).position',id),True);actor.locator('#cast').click()
        point([500,300]);actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-500,window.__lastSnapshot.self.position[1]-300)<5');actor.locator('#stop').click()
        for id in targets:
            cast(id);actor.wait_for_function('id=>window.__lastFinisher?.target.id===id',arg=id)
        resolved=actor.evaluate('window.__probeEvents.filter(r=>r.event?.kind==="finisher_resolved").map(r=>r.event)');assert len(resolved)==3
        assert [r['target']['id'] for r in resolved]==targets and [r['mode'] for r in resolved]==['execute','power_damage','execute'] and [r['raw_damage'] for r in resolved]==[ordinary,replacement,ordinary] and [r['hp_damage'] for r in resolved]==[ordinary,replacement,ordinary-1] and [r['lethal'] for r in resolved]==[True,False,False]
        assert all(r['source']['id']=='P00' and r['absorbed']==0 for r in resolved)
        actor.wait_for_function('hp=>!window.__lastSnapshot.monsters.find(n=>n.entity.id==="capped-target")?.hp&&window.__lastSnapshot.monsters.find(n=>n.entity.id==="replacement-target")?.hp===hp&&window.__lastSnapshot.monsters.find(n=>n.entity.id==="guard-target")?.hp===1',arg=replacement_hp)
        consumed=actor.evaluate('window.__probeEvents.filter(r=>r.event?.kind==="status_consumed"&&r.event.definition_id==="threshold-mark").map(r=>r.event)');assert len(consumed)==3 and all(r['consumed_stacks']==1 for r in consumed)
        assert '终结尝试' in actor.locator('#own').inner_text() and '目标存活' in actor.locator('#own').inner_text()
        for page in pages[1:]:assert page.evaluate('window.__lastFinisher===undefined')
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate('window.__lastFinisher===undefined');assert actor.evaluate('window.__lastSnapshot.monsters.find(n=>n.entity.id==="replacement-target").hp')==replacement_hp and actor.evaluate('window.__lastSnapshot.monsters.find(n=>n.entity.id==="guard-target").hp')==1
        cast('guard-target');actor.wait_for_function('!window.__lastSnapshot.monsters.find(n=>n.entity.id==="guard-target")?.hp');assert actor.evaluate('window.__lastFinisher===undefined')
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'targets':targets,'modes':['execute','power_damage','execute'],'raw':[ordinary,replacement,ordinary],'lethal':[True,False,False],'guardHp':1,'replacementHp':replacement_hp,'consumed':3,'privateRecipients':1,'reconnected':True,'noReplay':True,'guardKilledAfterward':True,'settled':seats}
    finally:
        for context in contexts:context.close()

def verify_finisher_followup(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';restored=20 if seats==4 else 30;spirit=80+restored;maximum=250 if seats==4 else 300;chain_raw=320 if seats==4 else 120
        leaf_ids=['finish-state','finish-spirit','finish-refund','finish-chain']
        def leaves():return actor.evaluate('ids=>window.__probeEvents.filter(r=>r.event?.kind==="combat_trigger_fired"&&ids.includes(r.event.trigger_id)).map(r=>r.event)',leaf_ids)
        def point(destination,select=False):
            if select:actor.locator('#selectGround').click()
            position=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();actor.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast(id):
            actor.wait_for_function("()=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id==='threshold-bolt').ready_at_ms}")
            actor.locator('#skill').select_option('threshold-bolt');point(actor.evaluate('id=>window.__lastSnapshot.monsters.find(n=>n.entity.id===id).position',id),True);actor.locator('#cast').click()
        point([500,300]);actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-500,window.__lastSnapshot.self.position[1]-300)<5');actor.locator('#stop').click()
        for i,id in enumerate(['guard-target','replacement-target']):
            cast(id);actor.wait_for_function('id=>window.__lastFinisher?.target.id===id',arg=id);assert not leaves();assert actor.evaluate(own+'.spirit')==200-(i+1)*40
        assert actor.evaluate('window.__lastSnapshot.monsters.find(n=>n.entity.id==="guard-target").hp')==1
        assert actor.evaluate('window.__lastSnapshot.monsters.find(n=>n.entity.id==="replacement-target").hp')==(300 if seats==4 else 50)
        cast('capped-target');actor.wait_for_function('expected=>window.__lastChainDamage?.initial_raw===expected',arg=chain_raw)
        actor.wait_for_function('spirit=>window.__lastSnapshot.self.spirit===spirit',arg=spirit)
        assert [e['trigger_id'] for e in leaves()]==leaf_ids and all(e['source']['id']=='P00' for e in leaves())
        status=actor.evaluate(own+".statuses.find(s=>s.definition_id==='afterglow')");assert status['source']['id']==status['target']['id']=='P00';assert status['expires_at_ms']-status['applied_at_ms']==(3000 if seats==4 else 4500)
        assert actor.evaluate(own+'.max_spirit')==maximum
        refund=actor.evaluate('window.__lastCooldownReturn');chain=actor.evaluate('window.__lastChainDamage');change=refund['changes'][0]
        assert change['reduced_ms']==(change['previous_ready_at_ms']-refund['at_ms'])*(3000 if seats==4 else 5000)//10000+(0 if seats==4 else 60)
        assert change['ready_at_ms']==change['previous_ready_at_ms']-change['reduced_ms']
        assert chain['source']['id']=='P00' and chain['origin']['id']=='capped-target' and chain['reason']=='complete' and len(chain['hops'])==1 and chain['hops'][0]['target']['id']=='follow-target' and chain['hops'][0]['raw_damage']==chain_raw and chain['hops'][0]['applied']
        assert actor.evaluate('window.__lastSnapshot.monsters.find(n=>n.entity.id==="follow-target").hp')==1000-chain_raw
        resolved=actor.evaluate('window.__probeEvents.filter(r=>r.event?.kind==="finisher_resolved").map(r=>r.event)');assert [r['lethal'] for r in resolved]==[False,False,True]
        assert '冷却返还 threshold-bolt' in actor.locator('#own').inner_text() and '连锁跳跃 1 次' in actor.locator('#own').inner_text()
        for peer in pages[1:]:assert not peer.evaluate('window.__probeEvents.some(r=>["skill_cooldowns_reduced","chain_damage_resolved","finisher_resolved","combat_trigger_fired"].includes(r.event?.kind)||r.event?.status?.definition_id==="afterglow")')
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate('window.__lastCooldownReturn===undefined&&window.__lastChainDamage===undefined&&window.__lastFinisher===undefined')
        assert actor.evaluate(own+".statuses.find(s=>s.definition_id==='afterglow')")==status and actor.evaluate(own+'.spirit')==spirit and actor.evaluate(own+'.max_spirit')==maximum
        assert actor.evaluate(own+".player_skills.find(s=>s.skill_id==='threshold-bolt').ready_at_ms")==change['ready_at_ms'] and not leaves()
        cast('guard-target');assert actor.evaluate('window.__lastSnapshot.logical_ms')<change['previous_ready_at_ms'];actor.wait_for_function('!window.__lastSnapshot.monsters.find(n=>n.entity.id==="guard-target")?.hp')
        assert not leaves() and actor.evaluate(own+'.spirit')==spirit-40 and actor.evaluate('window.__lastCooldownReturn===undefined&&window.__lastChainDamage===undefined&&window.__lastFinisher===undefined')
        actor.wait_for_function('!window.__lastSnapshot.self.statuses.some(s=>s.definition_id==="afterglow")');assert actor.evaluate(own+'.max_spirit')==200
        assert actor.evaluate('id=>window.__probeEvents.filter(r=>r.event?.kind==="status_removed"&&r.event.reason==="expired"&&r.event.instance_ids.includes(id)).length',status['instance_id'])==1
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'provider':'threshold-finish','mode':'execute','postEffects':4,'spiritRestored':restored,'spiritAfter':spirit,'maxSpirit':maximum,'chainRaw':chain_raw,'chainTarget':'follow-target','privateRecipients':1,'reconnected':True,'noReplay':True,'readyBeforeOriginal':True,'expired':True,'settled':seats}
    finally:
        for context in contexts:context.close()

def verify_action_enhancement(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';primary=450 if seats==4 else 460;spirit=156 if seats==4 else 160;threshold=5500 if seats==4 else 6000
        def events(kind):return actor.evaluate('kind=>window.__probeEvents.filter(r=>(r.event?.kind??r.kind)===kind).map(r=>r.event??r)',kind)
        def point(destination,select=False):
            if select:actor.locator('#selectGround').click()
            position=actor.evaluate(own+'.position');box=actor.locator('#map').bounding_box();actor.locator('#map').click(position={'x':box['width']/2+((destination[0]-destination[1])-(position[0]-position[1]))/2,'y':box['height']/2+((destination[0]+destination[1])-(position[0]+position[1]))/4})
        def cast(skill,target=None):
            actor.wait_for_function('id=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms}',arg=skill)
            actor.locator('#skill').select_option(skill);point(actor.evaluate('id=>window.__lastSnapshot.monsters.find(n=>n.entity.id===id).position',target) if target else [800,300],True);actor.locator('#cast').click()
        point([500,300]);actor.wait_for_function('Math.hypot(window.__lastSnapshot.self.position[0]-500,window.__lastSnapshot.self.position[1]-300)<5');actor.locator('#stop').click()
        cast('threshold-bolt','capped-target');actor.wait_for_function('window.__lastSnapshot.self.statuses.some(s=>s.definition_id==="afterglow")');initial=actor.evaluate(own+'.statuses.find(s=>s.definition_id==="afterglow")');first=events('finisher_resolved');assert len(first)==1 and first[0]['target']['id']=='capped-target';assert actor.evaluate(own+'.spirit')==180
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate(own+'.statuses.find(s=>s.definition_id==="afterglow")')==initial and actor.evaluate('window.__lastFinisher===undefined')
        cast('surge-line');actor.wait_for_function('window.__lastFinisher?.target.id==="line-a"');actor.wait_for_function('spirit=>window.__lastSnapshot.self.spirit===spirit&&!window.__lastSnapshot.self.statuses.some(s=>s.definition_id==="afterglow")',arg=spirit)
        before=actor.evaluate('window.__lastSnapshot.projectiles.find(p=>p.source.id==="P00"&&p.piercing)');assert before and before['piercing']['used_hits']==1
        line_first=events('finisher_resolved');assert len(line_first)==1;used=[e for e in events('status_consumed') if e['definition_id']=='afterglow'];assert len(used)==1 and used[0]['instance_id']==initial['instance_id'] and used[0]['consumed_stacks']==1
        refunds=events('skill_cooldowns_reduced');assert len(refunds)==1;change=refunds[0]['changes'][0];assert change['reduced_ms']==(change['previous_ready_at_ms']-refunds[0]['at_ms'])*(3000 if seats==4 else 5000)//10000
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');after=actor.evaluate('id=>window.__lastSnapshot.projectiles.find(p=>p.id===id)',before['id']);assert after
        assert {k:v for k,v in before['piercing'].items() if k!='remaining_range'}=={k:v for k,v in after['piercing'].items() if k!='remaining_range'} and 0<after['piercing']['remaining_range']<=before['piercing']['remaining_range']
        assert actor.evaluate('window.__lastFinisher===undefined&&window.__lastCooldownReturn===undefined') and actor.evaluate(own+'.spirit')==spirit
        actor.wait_for_function('window.__lastFinisher?.target.id==="line-b"');assert actor.evaluate('!window.__lastSnapshot.self.statuses.some(s=>s.definition_id==="afterglow")') and actor.evaluate(own+'.spirit')==spirit
        assert not [e for e in events('combat_trigger_fired') if e['trigger_id'] in ['enhance-next','enhance-spirit','enhance-refund']]
        line_second=events('finisher_resolved');assert len(line_second)==1 and line_second[0]['raw_damage']==1000-primary and line_first[0]['raw_damage']==1000-primary
        assert actor.evaluate(own+'.player_skills.find(s=>s.skill_id==="surge-line").ready_at_ms')==change['ready_at_ms']
        cast('threshold-bolt','renew-target');actor.wait_for_function('window.__lastSnapshot.self.statuses.some(s=>s.definition_id==="afterglow")');renewed=actor.evaluate(own+'.statuses.find(s=>s.definition_id==="afterglow")');assert renewed['instance_id']!=initial['instance_id'] and renewed['expires_at_ms']-renewed['applied_at_ms']==(5000 if seats==4 else 6500)
        assert actor.evaluate(own+'.spirit')==spirit-20;assert len([e for e in events('combat_trigger_fired') if e['trigger_id']=='enhance-next'])==1
        assert not [e for e in events('combat_trigger_fired') if e['trigger_id'] in ['enhance-spirit','enhance-refund']]
        for peer in pages[1:]:assert not peer.evaluate('window.__probeEvents.some(r=>["finisher_resolved","skill_cooldowns_reduced"].includes(r.event?.kind)||r.event?.status?.definition_id==="afterglow"||r.event?.definition_id==="afterglow")')
        actor.wait_for_function('!window.__lastSnapshot.self.statuses.some(s=>s.definition_id==="afterglow")');assert actor.evaluate(own+'.max_spirit')==200
        assert len([e for e in events('status_removed') if e['reason']=='expired' and renewed['instance_id'] in e['instance_ids']])==1
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'targets':['line-a','line-b'],'thresholdBps':threshold,'primaryRaw':primary,'consumed':1,'enhancedKills':2,'grantCount':2,'spiritAfterLine':spirit,'spiritRewards':1,'cooldownRewards':1,'privateRecipients':1,'reconnected':True,'inFlightReconnect':True,'noSelfRefresh':True,'newActionGranted':True,'expired':True,'settled':seats}
    finally:
        for context in contexts:context.close()

def verify_cast_lifecycle(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for p,link in zip(pages,links):
            p.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            p.on('pageerror',lambda e:errors.append(str(e)));p.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);p.goto(address+link['url'],wait_until='domcontentloaded')
        for p in pages:p.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self'
        large=pages[1];assert large.evaluate(own+'.spirit')==8000000;large.locator('#skill').select_option('large-cost');large.locator('#cast').click();large.wait_for_function(own+'.spirit===0');assert large.evaluate(own+'.hp')==1
        assert large.evaluate('window.__probeEvents.filter(r=>r.event?.kind==="action_committed"&&r.event.spirit_cost===8000000&&r.event.hp_cost===7999999).length')==1
        def events(kind):return actor.evaluate('kind=>window.__probeEvents.filter(r=>r.event?.kind===kind).map(r=>r.event)',kind)
        def cast(skill,takeover=False):
            if takeover:actor.wait_for_function('!window.__lastSnapshot.self.pending_action&&!window.__lastSnapshot.self.active_channel')
            else:actor.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&!s.self.active_channel&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms}',arg=skill)
            actor.locator('#skill').select_option(skill);actor.locator('#cast').click()
        before=actor.evaluate('({hp:window.__lastSnapshot.self.hp,spirit:window.__lastSnapshot.self.spirit})')
        cast('breakable-windup');actor.wait_for_function(own+'.pending_action!==null');actor.keyboard.down('d');actor.wait_for_function(own+'.pending_action===null');actor.keyboard.up('d')
        assert actor.evaluate('({hp:window.__lastSnapshot.self.hp,spirit:window.__lastSnapshot.self.spirit})')==before and len(events('action_committed'))==0
        cast('queued-charge');actor.wait_for_function(own+'.pending_action?.charge');charge=actor.evaluate(own+'.pending_action.charge');actor.locator('#releaseCharge').click();actor.wait_for_function(own+'.pending_action?.charge.released_at_ms!==undefined')
        assert actor.evaluate(own+'.pending_action.commits_at_ms')==charge['min_release_at_ms']+charge.get('release_windup_ms',0) and actor.evaluate(own+'.spirit')==before['spirit']
        assert actor.locator('#releaseCharge').is_disabled()
        actor.wait_for_function('id=>window.__probeEvents.some(r=>r.event?.kind==="action_committed"&&r.event.action_id===id)',arg=charge['action_id'])
        assert [e['charge']['damage_factor_bps'] for e in events('action_committed') if e['action_id']==charge['action_id']]==[4000]
        assert actor.evaluate('window.__lastSnapshot.logical_ms<window.__lastSnapshot.self.recovery_until_ms');actor.keyboard.down('d');actor.wait_for_function('window.__lastSnapshot.self.recovery_until_ms<=window.__lastSnapshot.logical_ms');actor.keyboard.up('d')
        cast('mobile-channel');actor.wait_for_function(own+'.active_channel');origin=actor.evaluate(own+'.position');actor.keyboard.down('s')
        actor.wait_for_function('y=>window.__lastSnapshot.self.active_channel?.pulses>=1&&window.__lastSnapshot.self.position[1]>y',arg=origin[1]);actor.keyboard.up('s')
        channel=actor.evaluate(own+'.active_channel');actor.locator('#cancel').click();actor.wait_for_function('window.__lastSnapshot.self.active_channel===undefined')
        pulses=[e for e in events('channel_pulse') if e['action_id']==channel['action_id']];assert pulses
        flight=actor.evaluate('id=>window.__lastSnapshot.projectiles.find(p=>p.id===id)',pulses[-1]['projectile_id']);assert flight
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert not actor.evaluate(own+'.active_channel')
        previous_recovery=actor.evaluate(own+'.recovery_until_ms');assert actor.evaluate('window.__lastSnapshot.logical_ms')<previous_recovery
        cast('recovery-channel',True);actor.wait_for_function(own+'.active_channel?.recovery===true');recovery_id=actor.evaluate(own+'.active_channel.action_id');assert actor.evaluate(own+'.active_channel.started_at_ms')<previous_recovery
        actor.wait_for_function('id=>window.__probeEvents.filter(r=>r.event?.kind==="channel_recovery_pulse"&&r.event.action_id===id).length===3&&!window.__lastSnapshot.self.active_channel',arg=recovery_id)
        recovery=[e for e in events('channel_recovery_pulse') if e['action_id']==recovery_id];assert len(recovery)==3
        assert sum(e['raw_hp'] for e in recovery)==(17 if seats==4 else 19) and sum(e['raw_spirit'] for e in recovery)==(23 if seats==4 else 29)
        for peer in pages[1:]:assert not peer.evaluate('window.__probeEvents.some(r=>r.event?.kind==="channel_recovery_pulse")')
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert not events('channel_recovery_pulse') and not actor.evaluate(own+'.active_channel')
        for p in pages:p.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers) and not errors,{'ledgers':ledgers,'errors':errors}
        return {'scenario':scenario,'members':seats,'largeCost':8000000,'movementRecovery':True,'atomicSelfTakeover':True,'interruptedPreparationFree':True,'queuedReleaseOnce':True,'mobileChannel':True,'committedFlightPreserved':True,'reconnected':True,'recoveryPulses':3,'recoveryRawHp':sum(e['raw_hp'] for e in recovery),'recoveryRawSpirit':sum(e['raw_spirit'] for e in recovery),'private':True,'settled':seats}
    finally:
        for c in contexts:c.close()

def verify_periodic_composition(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    init="""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();"""
    try:
        for page,link in zip(pages,links):
            page.add_init_script(init);page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor,taker,freezer=pages[:3];own='window.__lastSnapshot.self';burn=own+".statuses.find(s=>s.definition_id==='cauldron-burn')"
        def cast(page,skill,ally=None):
            page.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms}',arg=skill)
            page.locator('#skill').select_option(skill)
            if ally:page.locator('#allyTarget').select_option(label=ally)
            page.locator('#cast').click()
        def events(page,kind):return page.evaluate('kind=>window.__probeEvents.filter(r=>r.event?.kind===kind).map(r=>r.event)',kind)
        actor.wait_for_function(burn+'?.stacks>=3');npc=actor.evaluate('window.__lastSnapshot.monsters[0].entity.id');cast(freezer,'freeze-source')
        freezer.wait_for_function("window.__lastSnapshot.monsters[0].statuses?.some(s=>s.attribute==='stun')")
        actor.wait_for_function("!window.__lastSnapshot.projectiles.some(s=>s.source.kind==='monster')")
        before=actor.evaluate(burn);layers=before['stacks'];instance=before['instance_id'];four=seats==4
        assert 3<=layers<=10 and before['periodic']['per_stack'] is True and before['periodic']['interval_ms']==500
        assert before['periodic']['power_sampling']==('target_live' if four else 'target_snapshot')
        cast(actor,'bloom');actor.wait_for_function(own+'.effective_atk===120');hot=actor.evaluate(own+".statuses.find(s=>s.definition_id==='compound-bloom')")
        assert hot['flags']==['control_immunity'] and len(hot['modifiers'])==2
        expected=(120 if four else 100)*700*layers//10000
        actor.wait_for_function('arg=>window.__probeEvents.some(r=>r.event?.kind==="periodic_tick"&&r.event.instance_id===arg[0]&&r.event.raw_damage===arg[1]&&r.event.executed_at_ms>=arg[2])',arg=[instance,expected,hot['applied_at_ms']])
        first=next(e for e in reversed(events(actor,'periodic_tick')) if e['instance_id']==instance and e['executed_at_ms']>=hot['applied_at_ms']);assert first['raw_damage']==expected and first['stacks']==layers
        cast(taker,'take-burn','P00');taker.wait_for_function('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',arg=instance)
        moved=taker.evaluate(burn);assert all(moved[k]==before[k] for k in ['instance_id','source','applied_at_ms','expires_at_ms','stacks','max_stacks','modifiers']);assert moved['target']['id']=='P01' and moved['periodic']['first_tick_at_ms']==before['periodic']['first_tick_at_ms']
        for page in [actor,taker]:assert len([e for e in events(page,'status_transferred') if e['status']['instance_id']==instance])==1
        for page in pages[2:]:assert not [e for e in events(page,'status_transferred') if e['status']['instance_id']==instance]
        actor.locator('#leave').click();taker.reload(wait_until='domcontentloaded');taker.wait_for_function('window.__lastSnapshot?.self.ready');resumed=taker.evaluate(burn)
        assert all(resumed[k]==moved[k] for k in ['instance_id','source','target','applied_at_ms','expires_at_ms','stacks','max_stacks']) and resumed['periodic']['ticks_done']>=moved['periodic']['ticks_done']
        cast(taker,'bloom');taker.wait_for_function(own+'.effective_atk===240');late_hot=taker.evaluate(own+".statuses.find(s=>s.definition_id==='compound-bloom')");late_expected=(240 if four else 100)*700*layers//10000
        taker.wait_for_function('arg=>window.__probeEvents.some(r=>r.event?.kind==="periodic_tick"&&r.event.instance_id===arg[0]&&r.event.raw_damage===arg[1]&&r.event.executed_at_ms>=arg[2])',arg=[instance,late_expected,late_hot['applied_at_ms']])
        late=next(e for e in reversed(events(taker,'periodic_tick')) if e['instance_id']==instance and e['executed_at_ms']>=late_hot['applied_at_ms']);assert late['stacks']==layers
        taker.wait_for_function('window.__probeEvents.some(r=>r.event?.kind==="status_cleanse_resolved"&&r.event.phase==="expired"&&r.event.removed_count===1)')
        assert not taker.evaluate(burn);ticks=[e for e in events(taker,'periodic_tick') if e['instance_id']==instance];count=len(ticks);ended=taker.evaluate('window.__lastSnapshot.logical_ms')
        taker.wait_for_function('t=>window.__lastSnapshot.logical_ms>=t+600',arg=ended)
        assert len([e for e in events(taker,'periodic_tick') if e['instance_id']==instance])==count
        assert len({e['tick_index'] for e in ticks})==len(ticks) and all(e['source']['id']==npc and e['target']['id']=='P01' and e['scheduled_at_ms']==before['periodic']['first_tick_at_ms']+(e['tick_index']-1)*500 for e in ticks)
        for page in pages[2:]:assert not [e for e in events(page,'periodic_tick') if e['instance_id']==instance]
        for page in pages[1:]:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json() for l in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'layers':layers,'sampling':'target_live' if four else 'target_snapshot','firstRaw':first['raw_damage'],'transferredRaw':late['raw_damage'],'source':npc,'target':'P01','transferRecipients':2,'subsequentRecipients':1,'oldHolderLeft':True,'reconnected':True,'cleanseRemoved':1,'noLateTick':True,'settled':seats}
    finally:
        for c in contexts:c.close()

def verify_skill_loadouts(browser,request,address,scenario,seats,control):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    room=response.json();links=room['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950}) for _ in links];pages=[c.new_page() for c in contexts];errors=[]
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=[];const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>window.__probeEvents.push(JSON.parse(e.data)));}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self'
        def ids(page):return page.locator('#skill option').evaluate_all('(options)=>options.map(option=>option.value)')
        def events(page,kind):return page.evaluate('kind=>window.__probeEvents.filter(r=>r.event?.kind===kind).map(r=>r.event)',kind)
        assert ids(actor)==['old-bolt'] and ids(pages[1])==['learned-bolt']
        for page in pages[2:]:assert ids(page)==[] and page.locator('#cast').is_disabled() and page.locator('#selfCast').is_disabled()
        before_peer=pages[1].evaluate(own+'.spirit')
        denied=request.post('/api/input?'+urlsplit(links[1]['url']).query,data={'kind':'cast','skill_id':'old-bolt','direction':[1,0]});assert not denied.ok and denied.json()['error']=='INVALID_TARGET'
        denied=request.post('/api/input?'+urlsplit(links[1]['url']).query,data={'kind':'set_skill_loadout','loadout':{'owned_skill_ids':['old-bolt'],'equipped_skill_ids':['old-bolt']}});assert not denied.ok
        assert pages[1].evaluate(own+'.spirit')==before_peer and pages[1].evaluate(own+'.skill_loadout_revision')==0
        receipts=[]
        def change(op,rev,owned,equipped):return {'operation_id':op,'expected_revision':rev,'loadout':{'owned_skill_ids':owned,'equipped_skill_ids':equipped}}
        def grant(body):
            result=control(room['room_id'],'P00',body);assert 'receipt' in result,result
            receipt=result['receipt'];receipts.append(receipt);actor.wait_for_function('rev=>window.__lastSnapshot.self.skill_loadout_revision===rev',arg=receipt['revision']);return receipt
        def cast(skill):
            actor.wait_for_function('!window.__lastSnapshot.self.pending_action&&window.__lastSnapshot.logical_ms>=window.__lastSnapshot.self.recovery_until_ms')
            actor.locator('#skill').select_option(skill);actor.locator('#cast').click()
        unpaid=actor.evaluate(own+'.spirit');cast('old-bolt');actor.wait_for_function(own+'.pending_action!==null')
        swap=change('swap-prep',0,['old-bolt','learned-bolt'],['learned-bolt']);first=grant(swap)
        assert actor.evaluate(own+'.pending_action') is None and actor.evaluate(own+'.spirit')==unpaid and ids(actor)==['learned-bolt']
        after=actor.evaluate('window.__lastSnapshot.logical_ms');actor.wait_for_function('t=>window.__lastSnapshot.logical_ms>=t+1300',arg=after);assert not events(actor,'action_committed')
        count=len(events(actor,'skill_loadout_changed'));assert control(room['room_id'],'P00',swap)['receipt']==first and len(events(actor,'skill_loadout_changed'))==count
        assert control(room['room_id'],'P00',change('swap-prep',0,['old-bolt'],['old-bolt']))['error']=='CONFLICT'
        assert control(room['room_id'],'P00',change('stale',0,['old-bolt'],['old-bolt']))['error']=='REVISION_CONFLICT'
        grant(change('return-old',1,['old-bolt','learned-bolt'],['old-bolt']));cast('old-bolt');actor.wait_for_function('window.__probeEvents.some(r=>r.event?.kind==="action_committed"&&r.event.skill_id==="old-bolt")');actor.wait_for_function(own+'.player_skills[0].ready_at_ms>window.__lastSnapshot.logical_ms')
        old_ready=actor.evaluate(own+'.player_skills[0].ready_at_ms');old_flight=actor.evaluate("window.__lastSnapshot.projectiles.find(p=>p.source.id==='P00')");assert old_flight and actor.evaluate(own+'.spirit')==unpaid-2
        grant(change('forget',2,[],[]));assert ids(actor)==[] and actor.locator('#cast').is_disabled() and actor.locator('#selfCast').is_disabled();assert actor.evaluate('id=>window.__lastSnapshot.projectiles.some(p=>p.id===id)',old_flight['id'])
        grant(change('learn',3,['learned-bolt'],['learned-bolt']));cast('learned-bolt');actor.wait_for_function('window.__probeEvents.some(r=>r.event?.kind==="action_committed"&&r.event.skill_id==="learned-bolt")');actor.wait_for_function('n=>window.__lastSnapshot.self.spirit===n',arg=unpaid-4);paid=actor.evaluate(own+'.spirit')
        grant(change('restore',4,['old-bolt','learned-bolt'],['learned-bolt','old-bolt']));assert ids(actor)==['learned-bolt','old-bolt'];assert actor.evaluate(own+".player_skills.find(s=>s.skill_id==='old-bolt').ready_at_ms")==old_ready
        grant(change('reorder',5,['old-bolt','learned-bolt'],['old-bolt','learned-bolt']));assert ids(actor)==['old-bolt','learned-bolt'];actor.locator('#skill').select_option('old-bolt');assert actor.locator('#cast').is_disabled()
        own_events=events(actor,'skill_loadout_changed');assert len(own_events)==6
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready')
        assert actor.evaluate(own+'.skill_loadout_revision')==6 and ids(actor)==['old-bolt','learned-bolt'] and actor.evaluate(own+'.player_skills[0].ready_at_ms')==old_ready and actor.evaluate(own+'.spirit')==paid
        assert actor.evaluate('id=>window.__lastSnapshot.projectiles.some(p=>p.id===id)',old_flight['id']) and not events(actor,'skill_loadout_changed')
        for page in pages[1:]:assert not events(page,'skill_loadout_changed')
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json() for l in links]
            if all(r['phase']=='closed' and not r['host_live'] for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors'] for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'revision':6,'privateRecipients':1,'preparedCancelledFree':True,'emptySlots':True,'slotOrder':['old-bolt','learned-bolt'],'oldCooldownPreserved':True,'committedFlightPreserved':True,'paidSpirit':4,'exactRetryOnce':True,'casRejected':True,'reconnected':True,'settled':seats}
    finally:
        for c in contexts:c.close()
