"""Public H5 consumption of raw growth, native derivation and selected-source effects."""
import time
from urllib.parse import urlsplit

def verify_character_power(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    room=response.json();links=room['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];four=seats==4
    base=99 if four else 162;boosted=108 if four else 189;other=112 if four else 202;basic=150 if four else 200
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=JSON.parse(sessionStorage.getItem('power-events')||'[]');const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>{window.__probeEvents.push(JSON.parse(e.data));sessionStorage.setItem('power-events',JSON.stringify(window.__probeEvents));});}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];peer=pages[1];own='window.__lastSnapshot.self'
        for page in [actor,peer]:page.wait_for_function(own+'.hp==='+own+'.max_hp-40')
        assert actor.evaluate(own+'.effective_atk')==basic and actor.evaluate(own+'.max_hp')==(1000 if four else 1250)
        assert actor.evaluate(own+'.skill_powers["growth-strike"]')==base and peer.evaluate(own+'.skill_powers["growth-strike"]')==other
        def events(page,kind):return page.evaluate('kind=>window.__probeEvents.map(r=>r.kind==="private"?r.event:r).filter(e=>e.kind===kind)',kind)
        def damage(page,pid):return [e['amount']for e in events(page,'damage')if e['source']['id']==pid]
        def wait_damage(page,pid,n):page.wait_for_function('q=>window.__probeEvents.map(r=>r.kind==="private"?r.event:r).filter(e=>e.kind==="damage"&&e.source.id===q.pid).length===q.n',arg={'pid':pid,'n':n})
        def cast(page,skill,ally=False):
            page.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms<=s.logical_ms}',arg=skill)
            count=len(events(page,'action_committed'));page.locator('#skill').select_option(skill)
            if ally:page.locator('#allyTarget').select_option(label='P01')
            page.locator('#cast').click();page.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===n&&!window.__lastSnapshot.self.pending_action',arg=count+1)
        cast(actor,'growth-ward');assert actor.evaluate(own+'.statuses.find(s=>s.definition_id==="power-record").shield.record.cap')==base
        actor.wait_for_function('window.__lastSnapshot.logical_ms>='+own+'.recovery_until_ms');actor.locator('#attack').click();wait_damage(actor,'P00',1);assert damage(actor,'P00')==[basic]
        cast(actor,'growth-strike');wait_damage(actor,'P00',2);assert damage(actor,'P00')==[basic,base]
        cast(peer,'growth-strike');wait_damage(peer,'P01',1);assert damage(peer,'P01')==[other]
        cast(actor,'growth-heal',True);peer.wait_for_function(own+'.hp==='+own+'.max_hp');heal=next(e for e in events(actor,'heal_resolved')if e['heal_kind']=='ally_skill');assert (heal['raw_heal'],heal['hp_restored'])==(base,40)
        cast(actor,'growth-regrowth',True);cast(actor,'power-focus');actor.wait_for_function('p=>window.__lastSnapshot.self.skill_powers["growth-strike"]===p',arg=boosted)
        assert actor.evaluate(own+'.statuses.find(s=>s.definition_id==="power-record").shield.record.cap')==base and peer.evaluate(own+'.skill_powers["growth-strike"]')==other
        cast(actor,'growth-strike');wait_damage(actor,'P00',3);assert damage(actor,'P00')==[basic,base,boosted]
        actor.wait_for_function('window.__probeEvents.filter(r=>r.event?.kind==="periodic_heal_tick"&&r.event.source.id==="P00"&&r.event.target.id==="P01").length===4&&!window.__lastSnapshot.self.statuses.some(s=>s.definition_id==="power-focus")')
        hot=[e['raw_heal']for e in events(actor,'periodic_heal_tick')if e['source']['id']=='P00'and e['target']['id']=='P01'];assert hot==([99]*4 if four else[189,189,162,162])
        assert actor.evaluate(own+'.skill_powers["growth-strike"]')==base and '技能威力' in actor.locator('#own').inner_text()
        ready=actor.evaluate(own+'.player_skills.find(s=>s.skill_id==="growth-strike").ready_at_ms');actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready')
        assert actor.evaluate(own+'.skill_powers["growth-strike"]')==base and actor.evaluate(own+'.player_skills.find(s=>s.skill_id==="growth-strike").ready_at_ms')==ready
        for page in pages:
            assert page.evaluate('window.__lastSnapshot.members.every(m=>!Object.hasOwn(m,"skill_powers")&&!Object.hasOwn(m,"combat_build"))')
            assert not page.evaluate('JSON.stringify(window.__lastSnapshot).includes("model_id")')
            page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'basic':basic,'skill':base,'boosted':boosted,'peerSkill':other,'recordCap':base,'healRaw':base,'healActual':40,'hotRaw':hot,'sampling':'source_snapshot'if four else'source_live','derivedMaxHp':1000 if four else 1250,'rawGrowthPrivate':True,'memberIsolation':True,'reconnected':True,'expired':True,'settled':seats}
    finally:
        for context in contexts:context.close()
