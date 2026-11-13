"""Exercise frozen and live HP costs through real H5 controls."""
import time
from urllib.parse import urlsplit

def verify_reservations(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    links=response.json()['links'];contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];four=seats==4
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=JSON.parse(sessionStorage.getItem('reserve-events')||'[]');const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>{window.__probeEvents.push(JSON.parse(e.data));sessionStorage.setItem('reserve-events',JSON.stringify(window.__probeEvents));});}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];own='window.__lastSnapshot.self';actor.wait_for_function(own+'.hp<'+own+'.max_hp');assert actor.evaluate(own+'.hp')==960;unpaid=actor.evaluate(own+'.spirit')
        def commits():return actor.evaluate('window.__probeEvents.filter(r=>r.event?.kind==="action_committed").map(r=>r.event)')
        def cast(skill):
            actor.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms<=s.logical_ms}',arg=skill)
            actor.locator('#skill').select_option(skill);actor.locator('#cast').click();actor.wait_for_function(own+'.pending_action!==null')
        def heal():
            pages[1].locator('#skill').select_option('give-life');pages[1].locator('#allyTarget').select_option(label='P00');pages[1].locator('#cast').click();actor.wait_for_function(own+'.hp==='+own+'.max_hp')
        cast('reserved-bolt');pending=actor.evaluate(own+'.pending_action');quote=pending['resource_reservation'];assert quote=={'spirit':2,'hp':96 if four else 192,'minimum_hp_after_cost':1};assert actor.evaluate(own+'.spirit')==unpaid and actor.evaluate(own+'.hp')==960;assert '准备预留' in actor.locator('#own').inner_text()
        denied=request.post('/api/input?'+urlsplit(links[0]['url']).query,data={'kind':'cast','skill_id':'live-bolt','direction':[1,0]});assert not denied.ok and denied.json()['error']=='INVALID_STATE'
        heal();assert actor.evaluate(own+'.pending_action.resource_reservation')==quote and actor.evaluate(own+'.pending_action.action_id')==pending['action_id'];actor.wait_for_function('window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===1&&!window.__lastSnapshot.self.pending_action');assert commits()[0]['hp_cost']==quote['hp'] and actor.evaluate(own+'.hp')==1000-quote['hp']
        cast('live-bolt');assert actor.evaluate(own+'.pending_action.resource_reservation') is None;heal();actor.wait_for_function('window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===2&&!window.__lastSnapshot.self.pending_action');live=100 if four else 200;assert commits()[1]['hp_cost']==live and actor.evaluate(own+'.hp')==1000-live and actor.evaluate(own+'.spirit')==unpaid-4
        cast('reserved-bolt');before=actor.evaluate('({hp:window.__lastSnapshot.self.hp,spirit:window.__lastSnapshot.self.spirit})');actor.reload(wait_until='domcontentloaded');actor.wait_for_function('window.__lastSnapshot?.self.ready');assert actor.evaluate(own+'.pending_action') is None
        mark=actor.evaluate('window.__lastSnapshot.logical_ms');actor.wait_for_function('t=>window.__lastSnapshot.logical_ms>=t+1600',arg=mark);assert len(commits())==2
        cast('reserved-bolt');actor.locator('#cancel').click();actor.wait_for_function(own+'.pending_action===null');mark=actor.evaluate('window.__lastSnapshot.logical_ms');actor.wait_for_function('t=>window.__lastSnapshot.logical_ms>=t+1600',arg=mark);assert len(commits())==2 and actor.evaluate('({hp:window.__lastSnapshot.self.hp,spirit:window.__lastSnapshot.self.spirit})')==before
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            rows=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in rows):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in rows),errors
        return {'scenario':scenario,'members':seats,'initialHp':960,'quotedHp':quote['hp'],'liveHpCost':live,'paidSpirit':4,'noEarlyDebit':True,'reconnectReleased':True,'cancelReleased':True,'privateRecipients':1,'settled':seats}
    finally:
        for context in contexts:context.close()
