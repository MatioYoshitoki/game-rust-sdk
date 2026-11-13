"""Ordinary-clock, real UI acceptance of explicit current-resource transfer."""
import time
from verify_sdk_5_88_browser_actions import arm_resource_action, observed_resource_action, observe_resource_actions
from urllib.parse import urlsplit

def verify_resource_steal(browser,request,address,scenario,seats):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201
    room=response.json();links=room['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];four=seats==4
    try:
        for page,link in zip(pages,links):
            observe_resource_actions(page, 'steal-events')
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        actor=pages[0];actor.bring_to_front();own='window.__lastSnapshot.self';actor.wait_for_function(own+'.hp===960')
        def events(kind):return actor.evaluate('kind=>window.__probeEvents.filter(r=>r.event?.kind===kind).map(r=>r.event)',kind)
        def transfers(id):return [e for e in events('resource_stolen')if e.get('action_id')==id]
        def wait_count(id,n):actor.wait_for_function('q=>window.__probeEvents.filter(r=>r.event?.kind==="resource_stolen"&&r.event.action_id===q.id).length===q.n',arg={'id':id,'n':n})
        def begin():
            actor.wait_for_function('()=>{const s=window.__lastSnapshot;return !s.self.pending_action&&!s.self.active_channel&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills[0].ready_at_ms<=s.logical_ms}')
            prior=len(events('action_committed'))
            actor.locator('#skill').select_option('resource-channel')
            actor.locator('#cast').click()
            actor.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===n',arg=prior+1)
            return events('action_committed')[-1]['action_id']
        def no_late(id):
            mark=pages[1].evaluate('window.__lastSnapshot.logical_ms');pages[1].wait_for_function('t=>window.__lastSnapshot.logical_ms>=t+1350',arg=mark);assert len(transfers(id))==1, {'action':id,'reaction':actor.evaluate('window.__readResourceReaction()'),'transfers':transfers(id),'browserErrors':errors}
        first=begin();wait_count(first,3);actor.wait_for_function('!'+own+'.active_channel');complete=transfers(first)
        assert [e['requested_hp']for e in complete]==([33,33,34]if four else[50,50,50])
        assert [e['requested_spirit']for e in complete]==([4,4,4]if four else[4,4,5])
        assert sum(e['hp_removed']for e in complete)==(100 if four else 91)
        assert sum(e['spirit_removed']for e in complete)==(12 if four else 13)
        assert sum(e['hp_restored']for e in complete)==40 and sum(e['spirit_restored']for e in complete)==6
        assert actor.evaluate(own+'.hp')==1000 and actor.evaluate(own+'.spirit')==200
        assert actor.evaluate('window.__lastSnapshot.monsters[0].hp')==(892 if four else 1405)
        assert all(e['adjusted_hp_return']==e['hp_restored']+e['hp_excess']and e['spirit_removed']==e['spirit_restored']+e['spirit_excess']for e in complete)
        status=actor.evaluate(own+'.statuses.find(s=>s.definition_id==="devoured-power")');assert status['attribute']=='life_steal'and status['stacks']==1 and status['modifier_bps']==(1000 if four else 1500)
        assert '资源夺取' in actor.locator('#lastSelfHeal').inner_text() and '吸血状态' in actor.locator('#own').inner_text()
        arm_resource_action(actor,'cancel')
        cancelled=begin()
        observed_resource_action(actor,'cancel',cancelled)
        actor.wait_for_function('!'+own+'.active_channel')
        no_late(cancelled)
        arm_resource_action(actor,'reload')
        old_document=actor.evaluate('performance.timeOrigin')
        reconnected=begin()
        actor.wait_for_function('origin=>performance.timeOrigin!==origin&&window.__lastSnapshot?.self.ready',arg=old_document)
        reaction=observed_resource_action(actor,'reload',reconnected)
        assert not actor.evaluate(own+'.active_channel')
        assert actor.evaluate(own+'.player_skills[0].ready_at_ms')==reaction['readyAt']
        no_late(reconnected)
        arm_resource_action(actor,'leave')
        left=begin()
        observed_resource_action(actor,'leave',left)
        actor.wait_for_function(own+'.phase!=="active"')
        no_late(left)
        assert not [e for e in events('status_stacked')if e['status']['definition_id']=='devoured-power']
        for peer in pages[1:]:assert not peer.evaluate('window.__probeEvents.some(r=>r.event?.kind==="resource_stolen")')
        for page in pages[1:]:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'hpPolicy':'direct_transfer'if four else'damage','requestedHp':[33,33,34]if four else[50,50,50],'requestedSpirit':[4,4,4]if four else[4,4,5],'hpRemoved':100 if four else 91,'spiritRemoved':12 if four else 13,'hpRestored':40,'spiritRestored':6,'nthStatus':'life_steal','lifestealBps':1000 if four else 1500,'prefixCounts':[1,1,1],'cancelled':True,'reconnected':True,'sourceLeft':True,'noReplay':True,'privateRecipients':1,'settled':seats}
    finally:
        for context in contexts:context.close()
