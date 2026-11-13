"""Ordinary-clock two-page consumer: only browser input and public HTTP/SDK boundaries."""
import argparse
import hashlib
import json
import subprocess
import time
from pathlib import Path
from urllib.parse import urlparse

from playwright.sync_api import sync_playwright

ROOT=Path(__file__).resolve().parents[1]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/kernel-host');parser.add_argument('--out',type=Path,default=ROOT/'artifacts/sdk-5-18/browser');parser.add_argument('--port',type=int,default=4192)
    args=parser.parse_args();out=args.out.resolve();out.mkdir(parents=True,exist_ok=True);url=f'http://127.0.0.1:{args.port}'
    log=(out/'server.log').open('w')
    process=subprocess.Popen(['node','adapters/h5/two-member-server.mjs','--port',str(args.port),'--binary',str(args.binary.resolve()),'--database',str(out/'rooms.sqlite')],cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
    results=[]
    try:
        with sync_playwright() as pw:
            browser=pw.chromium.launch();api=pw.request.new_context(base_url=url)
            deadline=time.monotonic()+15
            while True:
                try:
                    response=api.get('/api/config');assert response.ok;break
                except Exception:
                    if process.poll() is not None or time.monotonic()>deadline:raise
                    time.sleep(.1)
            info=response.json()['sdk'];assert info['crateVersion']=='0.20.0' and not info['clocks']['replay']
            for scenario in ['pair-a','pair-b']:
                creation=api.post('/api/rooms',data={'scenario':scenario});assert creation.status==201,creation.text();room=creation.json();links=room['links']
                contexts=[browser.new_context(viewport={'width':1100,'height':900}) for _ in links]
                pages=[c.new_page() for c in contexts];errors=[];events=[[],[]]
                for i in ([1,0] if scenario=='pair-b' else [0,1]):
                    page,link=pages[i],links[i]
                    page.on('pageerror',lambda e:errors.append(str(e)))
                    page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None)
                    page.add_init_script('window.__seenEvents=[];')
                    page.goto(url+link['url'],wait_until='domcontentloaded')
                def snap(page):return page.evaluate('window.__lastSnapshot')
                def until(predicate,seconds=15):
                    end=time.monotonic()+seconds
                    while not predicate():
                        assert time.monotonic()<end,{'scenario':scenario,'views':[api.get('/api/view?'+urlparse(link['url']).query).json() for link in links]}
                        pages[0].wait_for_timeout(35)
                def command(i,action):
                    r=api.post('/api/input?'+urlparse(links[i]['url']).query,data=action)
                    return {'status':r.status,'body':r.json()}
                def required(i,action):
                    r=command(i,action);assert r['status']==200,r;return r['body']
                until(lambda:all((s:=snap(p)) and s['phase']=='active' and s['self']['ready'] for p in pages))
                initial=[snap(p) for p in pages];assert initial[0]['active_at_ms']==initial[1]['active_at_ms']
                for i,s in enumerate(initial):
                    assert s['self']['participant']['owner_id']==links[i]['participant_id']
                    for member in s['members']:
                        assert not set(member)&{'spirit','inventory','inventory_revision','player_skills','statuses','principal','operation_id'}
                assert api.get('/api/view?'+urlparse(links[0]['url']).query.replace('seat='+links[0]['participant_id'],'seat='+links[1]['participant_id'])).status==401
                bad=api.post('/api/input?'+urlparse(links[0]['url']).query,data={'kind':'reconnect','authenticated_principal':'fake'});assert bad.status==403
                until(lambda:all(snap(p)['self']['hp']<snap(p)['self']['max_hp'] for p in pages))
                before=snap(pages[0])['self']['position'];peer=snap(pages[1])['self']['position'];pages[0].keyboard.down('d');pages[0].wait_for_timeout(160);pages[0].keyboard.up('d')
                until(lambda:snap(pages[0])['self']['position']!=before);assert snap(pages[1])['self']['position']==peer
                pages[0].locator('#selfCast').click();until(lambda:len(snap(pages[0])['self']['statuses'])>0);assert not snap(pages[1])['self']['statuses']
                # Reconnect A while B keeps moving, then ready from A's new actually sent full.
                required(1,{'kind':'move','direction':[0,1]});peer_before=snap(pages[1])['self']['position'];old_generation=pages[0].evaluate('window.__lastConnection.generation');pages[0].locator('#reconnect').click()
                until(lambda:pages[0].evaluate('window.__lastConnection.generation')>old_generation and snap(pages[0])['self']['ready'] and snap(pages[1])['self']['position']!=peer_before);required(1,{'kind':'stop'})
                # Return to configured safe firing positions through real navigation inputs.
                for i in range(2):required(i,{'kind':'navigate','destination':initial[i]['self']['position']})
                until(lambda:all(sum((a-b)**2 for a,b in zip(snap(p)['self']['position'],initial[i]['self']['position']))<1 for i,p in enumerate(pages)))
                for i in range(2):required(i,{'kind':'stop'})
                deadline_fight=time.monotonic()+18;casts=0
                while any(m['hp'] for m in snap(pages[0])['monsters']):
                    assert time.monotonic()<deadline_fight,'fight deadline'
                    for i,p in enumerate(pages):
                        s=snap(p);view=api.get('/api/view?'+urlparse(links[i]['url']).query).json();skill=next(v for v in view['configuration']['skills'] if v['mechanism']=='projectile')
                        alive=[m for m in s['monsters'] if m['hp']]
                        if not alive:continue
                        target=min(alive,key=lambda n:sum((a-b)**2 for a,b in zip(n['position'],s['self']['position'])))
                        ready=next(v['ready_at_ms'] for v in s['self']['player_skills'] if v['skill_id']==skill['id'])
                        if not s['self']['pending_action'] and s['logical_ms']>=max(ready,s['self']['recovery_until_ms']):
                            r=command(i,{'kind':'cast','skill_id':skill['id'],'direction':[a-b for a,b in zip(target['position'],s['self']['position'])]})
                            assert r['status']==200 or r['body'].get('error') in ['INVALID_STATE','INVALID_TARGET'],r
                            casts+=r['status']==200
                    pages[0].wait_for_timeout(70)
                until(lambda:all(not any(n['hp'] for n in snap(p)['monsters']) for p in pages))
                resource=next(r for r in snap(pages[0])['resources'] if r['item_id'])
                for i in range(2):required(i,{'kind':'navigate','destination':resource['position']})
                until(lambda:all(sum((a-b)**2 for a,b in zip(snap(p)['self']['position'],resource['position']))<1 for p in pages))
                for i in range(2):required(i,{'kind':'stop'})
                winner=1 if scenario=='pair-b' else 0;loser=1-winner
                a=required(winner,{'kind':'pickup','resource_id':resource['id'],'expected_revision':snap(pages[winner])['self']['inventory_revision']})
                refused=command(loser,{'kind':'pickup','resource_id':resource['id'],'expected_revision':snap(pages[loser])['self']['inventory_revision']});assert refused['body']['error']=='INVALID_TARGET'
                until(lambda:all(not any(r['id']==resource['id'] for r in snap(p)['resources']) for p in pages))
                for i,p in enumerate(pages):p.screenshot(path=str(out/f'{scenario}-{i}-active.png'),full_page=True)
                for i in range(2):required(i,{'kind':'navigate','destination':snap(pages[i])['extraction_point']})
                until(lambda:all(sum((a-b)**2 for a,b in zip(snap(p)['self']['position'],snap(p)['extraction_point']))<1 for p in pages))
                for i in range(2):required(i,{'kind':'stop'})
                required(winner,{'kind':'extract_start'})
                until(lambda:api.get('/api/view?'+urlparse(links[winner]['url']).query).json()['ledger']['settlements']==1)
                view=api.get('/api/view?'+urlparse(links[loser]['url']).query).json();assert view['phase']=='active' and view['snapshot']['self']['phase']=='active'
                required(loser,{'kind':'leave'})
                until(lambda:all((v:=api.get('/api/view?'+urlparse(link['url']).query).json())['phase']=='closed' and v['ledger']['settlements']==1 and not v['host_live'] for link in links))
                final=[api.get('/api/view?'+urlparse(link['url']).query).json() for link in links]
                assert not errors,errors;assert all(not v['errors'] for v in final)
                for i,p in enumerate(pages):p.screenshot(path=str(out/f'{scenario}-{i}-terminal.png'),full_page=True)
                results.append({'scenario':scenario,'clock':'ordinary','pages':2,'sameActiveAt':initial[0]['active_at_ms'],'casts':casts,'winner':links[winner]['participant_id'],'raceLoser':'INVALID_TARGET','privateFieldsAbsent':True,'reconnectPeerContinued':True,'hostReleased':True,'errors':errors,'final':[v['ledger'] for v in final]})
                for c in contexts:c.close()
            browser.close();api.dispose()
        report={'passed':True,'crate':'0.20.0','binarySha256':hashlib.sha256(args.binary.read_bytes()).hexdigest(),'cases':results}
        (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'passed':True,'cases':len(results),'report':str(out/'report.json')},ensure_ascii=False))
    finally:
        process.terminate()
        try:process.wait(timeout=10)
        except subprocess.TimeoutExpired:process.kill();process.wait()
        log.close()

if __name__=='__main__':main()
