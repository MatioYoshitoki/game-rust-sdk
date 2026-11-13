"""Real four/eight H5 unit effects, recovery targets and interrupted plans."""
import time
from urllib.parse import urlsplit

def verify_unit_effects(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    room=response.json();links=room['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];four=seats==4
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=JSON.parse(sessionStorage.getItem('unit-events')||'[]');const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>{window.__probeEvents.push(JSON.parse(e.data));sessionStorage.setItem('unit-events',JSON.stringify(window.__probeEvents));});}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self'
        pages[1].wait_for_function(own+'.hp<'+own+'.max_hp')
        def events(kind):return actor.evaluate('kind=>window.__probeEvents.filter(r=>r.event?.kind===kind).map(r=>r.event)',kind)
        def wait_event(kind,id,count):actor.wait_for_function('q=>window.__probeEvents.filter(r=>r.event?.kind===q.kind&&r.event.action_id===q.id).length===q.count',arg={'kind':kind,'id':id,'count':count})
        def cast(skill,ally=False):
            actor.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&!s.self.active_channel&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms<=s.logical_ms}',arg=skill)
            actor.locator('#skill').select_option(skill)
            if ally:actor.locator('#allyTarget').select_option(label='P01')
            actor.locator('#cast').click()
        def begin(skill,ally=False):
            cast(skill,ally);actor.wait_for_function(own+'.active_channel!==undefined');return actor.evaluate(own+'.active_channel.action_id')
        paid=actor.evaluate(own+'.spirit');cast('unit-strike');actor.wait_for_function('window.__probeEvents.some(r=>r.event?.kind==="action_committed"&&r.event.skill_id==="unit-strike")')
        assert not actor.evaluate('window.__lastSnapshot.projectiles.some(p=>p.source.id==="P00")')
        enemy=begin('enemy-channel');wait_event('unit_channel_pulse',enemy,3);actor.wait_for_function('!'+own+'.active_channel')
        damage=[e['raw_damage']for e in events('unit_channel_pulse')if e['action_id']==enemy];assert damage==([2,2,4]if four else[3,3,5])
        ally=begin('ally-channel',True);wait_event('channel_recovery_pulse',ally,3);actor.wait_for_function('!'+own+'.active_channel')
        healing=[e for e in events('channel_recovery_pulse')if e['action_id']==ally];assert [e['raw_hp']for e in healing]==damage and sum(e['hp_restored']for e in healing)==(8 if four else 11)
        for peer in pages[2:]:assert not peer.evaluate('window.__probeEvents.some(r=>r.event?.kind==="heal_resolved"&&r.event.heal_kind==="ally_skill")')
        assert pages[1].evaluate('window.__probeEvents.filter(r=>r.event?.kind==="heal_resolved"&&r.event.heal_kind==="ally_skill").length')==3
        stopped=begin('enemy-channel');wait_event('unit_channel_pulse',stopped,1);ready=actor.evaluate(own+'.player_skills.find(s=>s.skill_id==="enemy-channel").ready_at_ms')
        actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert not actor.evaluate(own+'.active_channel');assert actor.evaluate(own+'.player_skills.find(s=>s.skill_id==="enemy-channel").ready_at_ms')==ready
        lost=begin('ally-channel',True);wait_event('channel_recovery_pulse',lost,1);pages[1].locator('#leave').click();actor.wait_for_function('!'+own+'.active_channel');mark=actor.evaluate('window.__lastSnapshot.logical_ms');actor.wait_for_function('t=>window.__lastSnapshot.logical_ms>=t+1300',arg=mark)
        assert len([e for e in events('unit_channel_pulse')if e['action_id']==stopped])==1 and len([e for e in events('channel_recovery_pulse')if e['action_id']==lost])==1
        grants=[e for e in [*events('status_applied'),*events('status_stacked')]if e['status']['definition_id']=='channel-focus'];assert len(grants)==2 and actor.evaluate(own+'.spirit')==paid-22
        for peer in pages[1:]:assert not peer.evaluate('window.__probeEvents.some(r=>["unit_channel_pulse","channel_recovery_pulse"].includes(r.event?.kind))')
        for page in [pages[0],*pages[2:]]:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'noProjectile':True,'damageShares':damage,'healShares':damage,'restoredHp':8 if four else 11,'fullOnceGates':2,'healRecipients':2,'planRecipients':1,'reconnectStopped':True,'targetExitStopped':True,'paidSpirit':22,'settled':seats}
    finally:
        for context in contexts:context.close()
