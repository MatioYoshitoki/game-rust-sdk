"""Ordinary four/eight real-page periodic lifecycle consumer, using public inputs only."""
import argparse,hashlib,json,subprocess,time
from pathlib import Path
from urllib.parse import urlparse
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,default=ROOT/'target/debug/kernel-host');p.add_argument('--out',type=Path,default=ROOT/'artifacts/sdk-5-25/browser');p.add_argument('--port',type=int,default=4211);a=p.parse_args();out=a.out.resolve();out.mkdir(parents=True,exist_ok=True);url=f'http://127.0.0.1:{a.port}';log=(out/'server.log').open('w');server=subprocess.Popen(['node','adapters/h5/member-line-server.mjs','--port',str(a.port),'--binary',str(a.binary.resolve()),'--database',str(out/'rooms-rechecked.sqlite')],cwd=ROOT,stdout=log,stderr=subprocess.STDOUT);results=[]
 try:
  with sync_playwright() as pw:
   api=pw.request.new_context(base_url=url);browser=pw.chromium.launch();deadline=time.monotonic()+15
   while True:
    try:r=api.get('/api/config');assert r.ok;break
    except Exception:
     assert server.poll() is None and time.monotonic()<deadline;time.sleep(.1)
   assert r.json()['sdk']['crateVersion']=='0.27.0' and r.json()['sdk']['memberLineAreas']['minimumMechanismVersion']==42
   for scenario in ['four','eight']:
    r=api.post('/api/rooms',data={'scenario':scenario});assert r.status==201,r.text();room=r.json();links=room['links'];n=len(links);contexts=[browser.new_context(viewport={'width':1100,'height':1000})for _ in links];pages=[c.new_page()for c in contexts];errors=[]
    for i in (reversed(range(n))if n==8 else range(n)):
     page=pages[i];page.on('pageerror',lambda e:errors.append(str(e)));page.on('console',lambda m:errors.append(m.text)if m.type=='error'else None)
     page.add_init_script("""window.__groundInputs=[];const originalFetch=window.fetch;window.fetch=(url,options)=>{if(options?.body){try{const a=JSON.parse(options.body);if(a.kind==='cast')window.__groundInputs.push(a);}catch{}}return originalFetch(url,options);};window.__dropped=0;window.__wire=[];const add=EventSource.prototype.addEventListener;EventSource.prototype.addEventListener=function(type,fn,...args){if(type==='gameplay'){return add.call(this,type,e=>{const wrapped=JSON.parse(e.data),event=wrapped.kind==='private'?wrapped.event:wrapped;if(event.kind==='area_ended'){window.__areaEnds??={};window.__areaEnds[event.area_id]=event.reason;}if(event.kind==='status_removed'){window.__removals??={};for(const id of event.instance_ids)window.__removals[id]=event.reason;}return fn(e);},...args);}if(type==='snapshot'){return add.call(this,type,e=>{const f=JSON.parse(e.data);window.__wire.push(f);if(window.__wire.length>500)window.__wire.shift();window.__deliverOld=frame=>fn(new MessageEvent('snapshot',{data:JSON.stringify(frame)}));if(window.__dropNextDelta&&f.payload.kind==='delta'){window.__dropNextDelta=false;window.__dropped++;return;}const result=fn(e),own=window.__lastSnapshot?.self;if(own){window.__observedHot??=structuredClone(own.statuses.find(s=>s.periodic?.kind==='max_hp_heal')??null);window.__observedShield??=structuredClone(own.statuses.find(s=>s.shield)??null);}return result;},...args);}return add.call(this,type,fn,...args);};""")
     page.goto(url+links[i]['url'],wait_until='domcontentloaded')
    query=lambda i:urlparse(links[i]['url']).query
    def view(i):return api.get('/api/view?'+query(i)).json()
    def snap(i):return pages[i].evaluate('window.__lastSnapshot')
    def stats(i):return pages[i].evaluate('window.__periodicStats')or{}
    def until(check,seconds=15):
     deadline=time.monotonic()+seconds
     while not check():
      assert time.monotonic()<deadline,{'scenario':scenario,'views':[view(i)for i in range(n)],'errors':errors}
      pages[1].wait_for_timeout(30)
    def command(i,action):
     r=api.post('/api/input?'+query(i),data=action);return {'status':r.status,'body':r.json()}
    def required(i,action):
     r=command(i,action);assert r['status']==200,r;return r['body']
    until(lambda:all((s:=snap(i))and s['phase']=='active'and s['self']['ready']for i in range(n)))
    initial=[snap(i)for i in range(n)];assert len({s['active_at_ms']for s in initial})==1
    skills=view(0)['configuration']['skills'];shot=next(s['id']for s in skills if s['mechanism']=='line_field');heal=next(s['id']for s in skills if s['mechanism']=='self_status')
    for i,s in enumerate(initial):
     assert s['self']['participant']['owner_id']==links[i]['participant_id']
     assert all(not set(m)&{'statuses','spirit','inventory','player_skills','shield_total'}for m in s['members'])
    until(lambda:sum(snap(i)['self']['hp']<snap(i)['self']['max_hp']for i in range(n))>=2)
    for i in range(n):required(i,{'kind':'cast_self','skill_id':heal})
    until(lambda:all(stats(i).get('selfHotTicks',0)>0 and snap(i)['self']['pending_action']is None and snap(i)['logical_ms']>=snap(i)['self']['recovery_until_ms']for i in range(n)))
    def fire(i):
     s=snap(i);alive=[m for m in s['monsters']if m['hp']]
     if not alive:return False
     target=min(alive,key=lambda m:sum((a-b)**2 for a,b in zip(m['position'],s['self']['position'])))
     ready=next(s['ready_at_ms']for s in s['self']['player_skills']if s['skill_id']==shot)
     if s['self']['pending_action']or s['logical_ms']<max(ready,s['self']['recovery_until_ms']):return False
     r=command(i,{'kind':'cast','skill_id':shot,'direction':[v-s['self']['position'][j]for j,v in enumerate(target['position'])]});assert r['status']==200 or r['body'].get('error')in['INVALID_STATE','INVALID_TARGET','NO_RESOURCE','OUT_OF_RANGE'],r;return r['status']==200
    target=snap(0)['monsters'][0]['position'];own=snap(0)['self']['position'];box=pages[0].locator('#map').bounding_box();pages[0].locator('#selectGround').click();pages[0].locator('#map').click(position={'x':box['width']/2+(target[0]-target[1]-own[0]+own[1])/2,'y':box['height']/2+(target[0]+target[1]-own[0]-own[1])/4});pages[0].locator('#cast').click()
    until(lambda:pages[0].evaluate('window.__groundInputs.length')>0)
    chosen=pages[0].evaluate('window.__groundInputs[0].direction');assert sum((a-(b-o))**2 for a,b,o in zip(chosen,target,own))<16
    until(lambda:any(a['source']['id']=='P00'and a.get('line')for a in snap(0)['areas']))
    pages[0].screenshot(path=str(out/f'{scenario}-line-first-plan.png'),full_page=True)
    for i in range(1,n):assert fire(i)
    until(lambda:any(a.get('line')and snap(1)['logical_ms']>=a['activates_at_ms']for a in snap(1)['areas']))
    pages[1].screenshot(path=str(out/f'{scenario}-line-active-plan.png'),full_page=True)
    until(lambda:any(s['source']['id']==links[0]['participant_id']for m in snap(0)['monsters']for s in m.get('statuses',[]))and any(stats(i).get('outgoingDotTicks',0)>0 for i in range(n))and any(stats(i).get('incomingDotTicks',0)>0 for i in range(n))and any(stats(i).get('selfHotTicks',0)>0 for i in range(n))and any(stats(i).get('npcHotTicks',0)>0 for i in range(n)))
    pages[0].locator('#leave').click();until(lambda:view(0)['ledger']['settlements']==1 and snap(0)['self']['phase']=='terminal');retired=stats(0)['outgoingDotTicks']
    old_area=next((a for a in snap(1)['areas']if a['source']['id']=='P01'),None)
    if old_area is None:
     until(lambda:snap(1)['logical_ms']>=next(s['ready_at_ms']for s in snap(1)['self']['player_skills']if s['skill_id']==shot))
     required(1,{'kind':'cast','skill_id':shot,'direction':[1,0]});until(lambda:any(a['source']['id']=='P01'for a in snap(1)['areas']));old_area=next(a for a in snap(1)['areas']if a['source']['id']=='P01')
    old_shield=next((s for s in snap(1)['self']['statuses']if s.get('shield')),None)or pages[1].evaluate('window.__observedShield');assert old_shield;shield_grants=pages[1].evaluate('window.__shieldStats.ownGrants')
    old_hot=pages[1].evaluate('window.__observedHot');assert old_hot;old_frame=pages[1].evaluate('window.__lastFrame');old_generation=pages[1].evaluate('window.__lastConnection.generation')
    required(2,{'kind':'move','direction':[0,1]});before=snap(2)['self']['position'];pages[1].locator('#reconnect').click();until(lambda:pages[1].evaluate('window.__lastConnection.generation')>old_generation and snap(1)['self']['ready']and snap(2)['self']['position']!=before);required(2,{'kind':'stop'})
    after_reconnect=snap(1);preserved=next((s for s in after_reconnect['self']['statuses']if s['instance_id']==old_hot['instance_id']),None)
    if preserved:assert preserved['applied_at_ms']==old_hot['applied_at_ms']and preserved['expires_at_ms']==old_hot['expires_at_ms']
    else:assert after_reconnect['logical_ms']>=old_hot['expires_at_ms']
    kept_area=next((a for a in after_reconnect['areas']if a['id']==old_area['id']),None);ended_area=pages[1].evaluate('(id)=>window.__areaEnds?.[id]??null',old_area['id'])
    if kept_area:
     assert all(kept_area[k]==old_area[k]for k in ['id','source','center','line','created_at_ms','activates_at_ms','expires_at_ms'])and kept_area['completed_ticks']>=old_area['completed_ticks']
     if kept_area['contact_policy']:assert kept_area['contact_policy']['used_hits']>=old_area['contact_policy']['used_hits']
    else:assert ended_area in['resolved','expired','hit_budget_exhausted']
    area_reconnect={'id':old_area['id'],'preserved':kept_area is not None,'endedReason':ended_area,'deadline':old_area['expires_at_ms']}
    after_shield=next((s for s in after_reconnect['self']['statuses']if s['instance_id']==old_shield['instance_id']),None)
    if after_shield:assert after_shield['expires_at_ms']==old_shield['expires_at_ms']and after_shield['applied_at_ms']==old_shield['applied_at_ms']and after_shield['shield']['remaining']<=old_shield['shield']['remaining']
    else:assert after_reconnect['logical_ms']>=old_shield['expires_at_ms']or pages[1].evaluate('(id)=>window.__removals?.[id]',old_shield['instance_id'])in['depleted','source_invalid','expired']
    assert pages[1].evaluate('window.__shieldStats.ownGrants')==shield_grants
    shield_reconnect={'instanceId':old_shield['instance_id'],'expiresAt':old_shield['expires_at_ms'],'preservedOrExpired':True,'removedReason':pages[1].evaluate('(id)=>window.__removals?.[id]??null',old_shield['instance_id'])}
    reconnect_cadence={'instanceId':old_hot['instance_id'],'expiresAt':old_hot['expires_at_ms'],'afterLogical':after_reconnect['logical_ms'],'expiredNormally':preserved is None}
    assert pages[1].evaluate('(f)=>{const id=window.__lastFrame.publication_id,stale=window.__replicationStats.stale;window.__deliverOld(f);return window.__lastFrame.publication_id===id&&window.__replicationStats.stale===stale+1;}',old_frame)
    for lose in ['drop','cache']:
     old=pages[1].evaluate('window.__replicationStats.resyncs')
     if lose=='drop':pages[1].evaluate('window.__dropNextDelta=true')
     else:pages[1].locator('#resetCache').click()
     until(lambda:pages[1].evaluate('window.__replicationStats.resyncs')>old)
     until(lambda:pages[1].evaluate("window.__lastFrame.payload.kind==='delta'"))
    for i in range(1,n):required(i,{'kind':'navigate','destination':initial[i]['self']['position']})
    until(lambda:all(sum((a-b)**2 for a,b in zip(snap(i)['self']['position'],initial[i]['self']['position']))<1 for i in range(1,n)))
    for i in range(1,n):required(i,{'kind':'stop'})
    deadline=time.monotonic()+22;casts=0
    while any(m['hp']for m in snap(1)['monsters']):
     assert time.monotonic()<deadline,'fight deadline'
     for i in range(1,n):casts+=fire(i)
     pages[1].wait_for_timeout(60)
    until(lambda:all(not any(m['hp']for m in snap(i)['monsters'])for i in range(1,n)))
    assert stats(0)['outgoingDotTicks']==retired
    for i in range(n):
     for m in snap(i)['monsters']:
      for s in m.get('statuses',[]):assert s['target']==m['entity']and(s['source']['id']==links[i]['participant_id']or s['source']==s['target'])
    resource=next(r for r in snap(1)['resources']if r['item_id'])
    for i in range(1,n):required(i,{'kind':'navigate','destination':resource['position']})
    until(lambda:all(sum((a-b)**2 for a,b in zip(snap(i)['self']['position'],resource['position']))<1 for i in range(1,n)))
    for i in range(1,n):required(i,{'kind':'stop'})
    winner=n-1 if n==8 else 1;losers=[i for i in range(1,n)if i!=winner];required(winner,{'kind':'pickup','resource_id':resource['id'],'expected_revision':snap(winner)['self']['inventory_revision']})
    for i in losers:assert command(i,{'kind':'pickup','resource_id':resource['id'],'expected_revision':snap(i)['self']['inventory_revision']})['body']['error']=='INVALID_TARGET'
    for i,p in enumerate(pages):p.screenshot(path=str(out/f'{scenario}-{i}-active.png'),full_page=True)
    for i in range(1,n):required(i,{'kind':'navigate','destination':snap(i)['extraction_point']})
    until(lambda:all(sum((a-b)**2 for a,b in zip(snap(i)['self']['position'],snap(i)['extraction_point']))<1 for i in range(1,n)))
    for i in range(1,n):required(i,{'kind':'stop'})
    required(winner,{'kind':'extract_start'});until(lambda:view(winner)['ledger']['settlements']==1)
    for i in losers:required(i,{'kind':'leave'})
    until(lambda:all((v:=view(i))['phase']=='closed'and v['ledger']['settlements']==1 and not v['host_live']for i in range(n)))
    final=[view(i)for i in range(n)];metrics=[stats(i)for i in range(n)];replication=[p.evaluate('window.__replicationStats')for p in pages]
    assert any(m['hpRestored']>0 for m in metrics)and all(not v['errors']for v in final)and not errors,errors
    assert all(r['delta']>0 for r in replication);assert pages[1].evaluate('window.__dropped')==1 and replication[1]['resyncs']>=2
    assert all(p.evaluate("window.__wire.filter(f=>f.payload.kind==='delta').every(f=>!('snapshot'in f)&&!('snapshot'in f.payload))")for p in pages)
    for i,p in enumerate(pages):p.screenshot(path=str(out/f'{scenario}-{i}-terminal.png'),full_page=True)
    shields=[p.evaluate('window.__shieldStats')for p in pages];assert all(m['ownGrants']>0 for m in shields)and any(m['ownAbsorptions']>0 for m in shields)and any(m['npcAbsorptions']>0 for m in shields)and any(m['dotAbsorbed']>0 for m in shields)
    assert all(p.evaluate('!!window.__areaRendered?.line')for p in pages)
    area_metrics=[p.evaluate('window.__areaStats')for p in pages];assert all(m['ownStarted']>0 and m['ownTicks']>0 and m['ownContacts']>0 for m in area_metrics)
    results.append({'scenario':scenario,'pages':n,'clock':'ordinary','casts':casts,'earlyExit':'P00','retiredSourceStopped':True,'reconnectPreserved':True,'reconnectCadence':reconnect_cadence,'cacheLossRecovered':True,'lostFrameRecovered':True,'oldFrameRejected':True,'privateProjection':True,'winner':links[winner]['participant_id'],'areaStats':area_metrics,'areaUiUsed':True,'chosenPoint':chosen,'areaReconnect':area_reconnect,'periodic':metrics,'shields':shields,'shieldReconnect':shield_reconnect,'replication':replication,'hostReleased':True,'errors':errors,'final':[v['ledger']for v in final]})
    for c in contexts:c.close()
   browser.close();api.dispose()
  report={'passed':True,'crate':'0.27.0','binarySha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'cases':results};(out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'passed':True,'cases':len(results)}))
 finally:
  server.terminate()
  try:server.wait(timeout=10)
  except subprocess.TimeoutExpired:server.kill();server.wait()
  log.close()
if __name__=='__main__':main()
