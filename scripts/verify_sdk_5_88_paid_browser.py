"""Real H5 input verifies full payment, three rebates and recovery after exhaustion."""
import json
from pathlib import Path
import subprocess,time
from urllib.parse import urlsplit
from uuid import uuid4
from playwright.sync_api import Error,sync_playwright

def verify_paid(browser,request,address,scenario,seats,output):
    response=request.post('/api/rooms',data={'scenario':scenario});assert response.status==201;links=response.json()['links'];assert len(links)==seats
    contexts=[browser.new_context(viewport={'width':1100,'height':950})for _ in links];pages=[c.new_page()for c in contexts];errors=[];states=[];samples=[];cost=10 if seats==4 else 20;refund=3 if seats==4 else 10;initial=100 if seats==4 else 200
    try:
        for page,link in zip(pages,links):
            page.add_init_script("""(()=>{window.__probeEvents=JSON.parse(sessionStorage.getItem('paid-events')||'[]');const Native=window.EventSource;window.EventSource=class extends Native{constructor(...a){super(...a);this.addEventListener('gameplay',e=>{window.__probeEvents.push(JSON.parse(e.data));sessionStorage.setItem('paid-events',JSON.stringify(window.__probeEvents));});}};})();""")
            page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None);page.goto(address+link['url'],wait_until='domcontentloaded')
        for page in pages:page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        own='window.__lastSnapshot.self'
        def state(page,pid):return {'pid':pid,'own':page.evaluate(own)}
        def status(page,id):return page.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.definition_id===id)',id)
        def events(page,kind):return page.evaluate('kind=>window.__probeEvents.map(r=>r.kind==="private"?r.event:r).filter(e=>e.kind===kind)',kind)
        def cast(page,id):
            page.wait_for_function('id=>{const s=window.__lastSnapshot;return !s.self.pending_action&&!s.self.active_channel&&s.logical_ms>=s.self.recovery_until_ms&&s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms<=s.logical_ms}',arg=id)
            n=len(events(page,'action_committed'));page.locator('#skill').select_option(id);page.locator('#cast').click();page.wait_for_function('n=>window.__probeEvents.filter(r=>r.event?.kind==="action_committed").length===n&&!window.__lastSnapshot.self.pending_action&&!window.__lastSnapshot.self.active_channel',arg=n+1);return events(page,'action_committed')[-1]
        for i,page in enumerate(pages[:4]):
            pid=f'P{i:02}';cast(page,'activate-charges');original=status(page,'charges');assert original['stacks']==3 and original['expires_at_ms']-original['applied_at_ms']==(8000 if seats==4 else 9000);assert page.evaluate(own+'.spirit')==initial;states.append(state(page,pid))
            for j in range(3):
                commit=cast(page,'paid-shot');returns=events(page,'paid_spirit_restored');assert len(returns)==j+1 and commit['spirit_cost']==cost and returns[-1]['action_id']==commit['action_id'];assert[returns[-1][k]for k in ['paid_spirit','requested','restored','excess']]==[cost,refund,refund,0];assert page.evaluate(own+'.spirit')==initial-(j+1)*(cost-refund)and(status(page,'charges')or{}).get('stacks',0)==2-j;states.append(state(page,pid))
                if j==1 and i==0:
                    page.reload(wait_until='domcontentloaded');page.wait_for_function('window.__lastSnapshot?.self.ready');assert status(page,'charges')['instance_id']==original['instance_id']and status(page,'charges')['expires_at_ms']==original['expires_at_ms']and len(events(page,'paid_spirit_restored'))==2
            lock=status(page,'exhausted');assert lock and lock['expires_at_ms']-lock['applied_at_ms']==(3000 if seats==4 else 3600);before=page.evaluate(own+'.spirit');cast(page,'paid-shot');assert page.evaluate(own+'.spirit')==before-cost and len(events(page,'paid_spirit_restored'))==3;cast(page,'recover');assert page.evaluate(own+'.spirit')==before-cost;states.append(state(page,pid))
            if i==0:
                page.reload(wait_until='domcontentloaded');page.wait_for_function('window.__lastSnapshot?.self.ready');assert status(page,'exhausted')['instance_id']==lock['instance_id']and status(page,'exhausted')['expires_at_ms']==lock['expires_at_ms']and len(events(page,'paid_spirit_restored'))==3
            samples.append({'pid':pid,'paid':4*cost,'refunded':3*refund,'spiritAfter':before-cost,'chargesConsumed':3,'lockMs':3000 if seats==4 else 3600})
        for page in pages[:4]:page.wait_for_function('!window.__lastSnapshot.self.statuses.some(s=>s.definition_id==="exhausted")')
        for i,page in enumerate(pages[:4]):
            before=page.evaluate(own+'.spirit');cast(page,'recover');assert page.evaluate(own+'.spirit')==before+7 and len(events(page,'paid_spirit_restored'))==3;states.append(state(page,f'P{i:02}'))
        for i,page in enumerate(pages):assert all(e['source']['id']==f'P{i:02}'for e in events(page,'paid_spirit_restored'))and page.evaluate('!JSON.stringify(window.__lastSnapshot).includes("zero_policy")')
        Path(output,scenario+'.paid-browser-facts.json').write_text(json.dumps({'states':states,'samples':samples,'eventsByMember':[{'participant':f'P{i:02}','events':page.evaluate('window.__probeEvents')}for i,page in enumerate(pages)]},ensure_ascii=False,indent=2)+'\n')
        for page in pages:page.locator('#leave').click()
        deadline=time.monotonic()+15
        while True:
            ledgers=[request.get('/api/ledger?'+urlsplit(l['url']).query).json()for l in links]
            if all(r['phase']=='closed'and not r['host_live']for r in ledgers):break
            assert time.monotonic()<deadline;time.sleep(.04)
        assert not errors and all(r['ledger']['admissions']==r['ledger']['settlements']==1 and not r['errors']for r in ledgers),errors
        return {'scenario':scenario,'members':seats,'cost':cost,'refund':refund,'samples':samples,'refundsPerActor':3,'fullCostFirst':True,'exhausted':True,'lockRecoveryBlocked':True,'afterExpiryRecovery':7,'reconnected':True,'noReplay':True,'private':True,'settled':seats}
    except Exception:
        Path(output,scenario+'.paid-browser-error.json').write_text(json.dumps({'errors':errors,'events':[p.evaluate('window.__probeEvents')for p in pages],'snapshots':[p.evaluate('window.__lastSnapshot')for p in pages]},ensure_ascii=False,indent=2)+'\n');raise
    finally:
        for c in contexts:c.close()

def verify_paid_matrix(args):
    database=args.out/f'paid-rooms-{uuid4().hex}.sqlite';address=f'http://127.0.0.1:{args.port}';command=['node',str(args.server),'--binary',str(args.binary),'--configs',str(args.configs/'paid-catalog.json'),'--host-config',str(args.configs/'paid-host.json'),'--database',str(database),'--port',str(args.port)]
    with(args.out/'paid-server.log').open('w')as log:
        server=subprocess.Popen(command,cwd=args.cwd,stdout=log,stderr=subprocess.STDOUT,text=True)
        try:
            with sync_playwright()as p:
                request=p.request.new_context(base_url=address);deadline=time.monotonic()+20
                while True:
                    try:r=request.get('/api/config',timeout=1000)
                    except Error:r=None
                    if r and r.ok:break
                    assert server.poll()is None and time.monotonic()<deadline;time.sleep(.1)
                browser=p.chromium.launch();cases=[verify_paid(browser,request,address,s,n,args.out)for s,n in [('paid-four',4),('paid-eight',8)]];browser.close();request.dispose();return cases
        finally:server.terminate();server.wait(timeout=10)
