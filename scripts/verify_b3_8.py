"""Template scores, immutable score proofs and assets via public H5/Rust/SQLite."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-8';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir();aliases=['b1','b3-1','b3-2','b3-3','b3-4','b3-5','b3-6','b3-7','b3-8']
    for alias in aliases:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-8.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[200,400],extraction_point=[900,800],geometry={'min':[0,0],'max':[1024,1024],'walls':[]},extraction_unlock_ms=20000)
        c['params']['runtime']['publish_batched_ms']=900;c['params']['regen'].update(near_ratio=0,far_ratio=0)
        p=c['pve'];p.update(map_id='scoring-test-flat',width=32,height=32,cell_size=32,cells_rle=[[0,1024]])
        p['stats'].update(hp=60,spirit=0,atk=0,spd=0,crit=0,dodge=0);p['combat']['cast_chance']=0
        elite=p['additional_templates']['verification-score-elite'];elite['stats']=copy.deepcopy(p['stats']);elite['combat']['cast_chance']=0
        e=p['encounters'][0];e['trigger']={'center':[500,400],'radius':100};e['waves'][1]['delay_ms']=3000
        for i,m in enumerate(m for w in e['waves'] for m in w['monsters']):m['position']=[600+i*100,400]
        for i,chest in enumerate(p['chest_spawns']):chest['position']=[900,400+i*250]
        t=p['additional_templates']['verification-phase-boss'];t['stats'].update(hp=300,spirit=100,atk=0,spd=0,crit=0,dodge=0);t['stats']['def']=0;t['leash']=700
        for phase in t['boss']['phases']:phase['cast_chance']=0
        if mode=='home':t['leash']=150;t['combat']['aggro_range']=150
        if mode in ['zero-boss','noncredit']:t['kill_score']=0
        if mode=='noncredit':
            t['kill_credit']=False;t['drop_table']['guaranteed']=[{'itemId':'amber','quantityMin':2,'quantityMax':2}];p['items']['amber']={'name':'配置晶石','stackLimit':20}
        if mode=='zero':
            p['kill_score']=0
            for profile in p['additional_templates'].values():profile['kill_score']=0
        if mode=='coefficient-zero':c['cultivation_coefficient']=0
        if mode=='inherit':c['kill_score']=7;p.pop('kill_score');elite.pop('kill_score')
        if mode=='round':
            p['kill_score']=1;c['cultivation_coefficient']=.6;c['extraction_score']=0
            for profile in p['additional_templates'].values():profile['kill_score']=1
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{a:a+'.json' for a in aliases},'archives':[]}));return manifest
def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_8.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,text=True,capture_output=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M);tests=[t for t in all_tests if '::scoring_tests::' in t or t.startswith('verification::scoring::') or t.startswith('config::catalog::tests::scoring_')]
    assert len(all_tests)>=353 and len(tests)==36,(len(all_tests),len(tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def encounter(v):return v['snapshot']['pve']['encounters'][0]
        def boss(v):return next(m for m in v['snapshot']['pve']['monsters'] if m.get('boss'))
        def changes(v):return [e for e in v['events'] if e['kind']=='boss_phase_changed']
        def score(v):return v['snapshot']['pve']['scoring']
        def score_events(v):return [e for e in v['events'] if e['kind']=='kill_scored']
        def frozen(v):return next(r for r in v['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):
            page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed'&&document.querySelector('#pending').textContent==='0'",timeout=30000)
            return wait(lambda:v if (v:=current(page,host)) and v['settlement'] and not v['ledger']['pending'] else None)
        def check(v):
            b=frozen(v)['body'];l=v['ledger'];s=b['scoring'];assert s['kill_score']==sum(e['score'] for e in score_events(v)) and s['kills']==len(score_events(v));assert s==v['settlement']['scoring'];assert b['carried']+b['looted']==b['kept']+b['dropped']
            assert l['warehouse']==100-b['carried']+b['kept'] and l['cultivation']==b['cultivation']
            assert l['items']=={i['item_id']:i['kept'] for i in b.get('items',[])} and all(i['looted']==i['kept']+i['dropped'] for i in b.get('items',[]))
            assert l['admissions']==l['settlements']==1 and not l['pending'] and l['manual']==0 and v['phase']=='closed'
            assert sum(e['kind']=='settlement_applied' for e in v['events'])==1
        def auto(page,fault='none'):
            page.locator('#scenario').select_option('b3-8');page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.url.endswith('/api/sessions') and r.request.method=='POST'):page.locator('#verify').click()
        def begin(page):page.locator('#scenario').select_option('b3-8');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        serial=0;last_input=None
        def send(page,host,action):
            nonlocal serial,last_input
            for _ in range(3):
                serial+=1;v=current(page,host);last_input=(host.url+f"/api/sessions/{v['id']}/input",{'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'scoring-input-{serial}','action':action});response=page.request.post(last_input[0],data=last_input[1])
                if response.ok or response.json().get('error')!='STALE_SEQUENCE':return response
            raise AssertionError(response.text())
        def navigate(page,host,position):
            assert send(page,host,{'kind':'navigate','destination':position}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],position))<50**2);assert send(page,host,{'kind':'stop'}).ok
        def attack(page,host,target):
            v=current(page,host);m=next(m for m in v['snapshot']['pve']['monsters'] if m['id']==target);navigate(page,host,m['position'])
            wait(lambda:(v:=current(page,host)) and v['snapshot']['logical_ms']>=v['snapshot']['basic_ready_at_ms'])
            assert send(page,host,{'kind':'attack'}).ok
            return wait(lambda:v if (v:=current(page,host)) and next(m for m in v['snapshot']['pve']['monsters'] if m['id']==target)['hp']<m['hp'] else None)
        def reach_boss(page,host):
            begin(page);navigate(page,host,[500,400]);wait(lambda:encounter(current(page,host))['phase']=='active')
            for target in ['forest-wave-1-wolf-1','forest-wave-1-wolf-2']:
                attack(page,host,target);assert next(m for m in current(page,host)['snapshot']['pve']['monsters'] if m['id']==target)['hp']==0
            wait(lambda:encounter(current(page,host))['phase']=='active' and encounter(current(page,host))['wave']['index']==2)
        def phase2(page,host):
            reach_boss(page,host)
            for _ in range(5):
                v=attack(page,host,'forest-phase-boss-1')
                if boss(v)['boss']['phase_index']==2:return v
            raise AssertionError('No phase transition')
        def opening():return {'kind':'open_chest','chest_id':'scoring-cache-1','expected_generation':1}
        def close(page,host):page.locator('#close').click();return done(page,host)
        def scenario(name,work,mode='source',point='',mobile=False,unfrozen=False):
            with tempfile.TemporaryDirectory(prefix='scoring-') as directory:
                host=Host(directory,package(directory,mode));context=browser.new_context(viewport={'width':390,'height':844} if mobile else {'width':1440,'height':1100},has_touch=mobile,is_mobile=mobile)
                page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                try:
                    host.start(point,'commit_settlement' if point else '');page.goto(host.url);work(host,page,context);v=current(page,host)
                    if not unfrozen:check(v)
                    assert page.locator('#world-live').inner_text()=='0' and not errors,errors
                    results[name]={'ledger':v['ledger'],'snapshot':v['snapshot'],'events':v['events'],'configHash':v['configHash'],'settlementRequest':frozen(v) if not unfrozen else None};print(name+': passed',flush=True)
                except Exception:
                    try:page.screenshot(path=str(OUTPUT/'failure.png'),full_page=True);(OUTPUT/'failure.json').write_text(json.dumps(current(page,host),ensure_ascii=False,indent=2))
                    except Exception:pass
                    host.error_log.seek(0);print(host.error_log.read(),flush=True);raise
                finally:context.close();host.stop();host.error_log.close()
        def extraction(host,page,context):
            auto(page);v=done(page,host);assert v['ledger']['cultivation']==127 and score(v)['kill_score']==140 and score(v)['kills']==3
            assert [e['score'] for e in score_events(v)]==[10,30,100] and sum(e['kind']=='chest_opened' for e in v['events'])==2
            page.wait_for_function("document.body.dataset.verification==='passed'");page.wait_for_function("document.querySelector('#score-status').textContent.includes('击杀积分 140')");page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True);before=v['ledger'];request=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
        scenario('ordinary_elite_boss_scores_boxes_assets_extract_and_two_restarts',extraction)
        def death(host,page,context):
            auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and encounter(v)['phase']=='completed' and all(c['opened'] for c in v['snapshot']['pve']['chests']) and not v['snapshot']['resources']);v=close(page,host);assert v['ledger']['cultivation']==52 and frozen(v)['body']['outcome']=='death'
        scenario('complete_weighted_score_death_conserves_all_assets',death)
        def partial(host,page,context):
            v=phase2(page,host);s=score(v);assert (s['kills'],s['credited_kills'],s['kill_score'],s['extraction_cultivation'],s['death_cultivation'])==(2,2,40,52,15)
            assert len(score_events(v))==2 and send(page,host,opening()).json()['error']=='ENCOUNTER_LOCKED';page.screenshot(path=str(OUTPUT/'partial-score.png'),full_page=True);close(page,host)
        scenario('alive_boss_and_phase_transition_do_not_award_score_or_unlock_boxes',partial,mode='flat')
        def refresh(host,page,context):
            v=phase2(page,host);before=score(v);old=v['connection'];page.reload();active(page);v=current(page,host);assert score(v)==before
            r=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':old,'sequence':v['sequence']+1,'requestId':'old-score-owner','action':{'kind':'attack'}});assert r.json()['error']=='STALE_CONNECTION';close(page,host)
        scenario('refresh_preserves_actual_kills_points_previews_and_old_owner_fence',refresh,mode='flat')
        def protected(host,page,context):
            phase2(page,host);page.reload();active(page);before=score(current(page,host));page.locator('#pause').click();wait(lambda:current(page,host)['snapshot']['protected']);page.wait_for_timeout(300);assert score(current(page,host))==before
            page.locator('#pause').click();active(page);assert score(current(page,host))==before;close(page,host)
        scenario('background_foreground_do_not_change_or_repeat_score',protected,mode='flat')
        def home(host,page,context):
            before=score(phase2(page,host));navigate(page,host,[200,700]);v=wait(lambda:v if boss(v:=current(page,host))['boss']['epoch']==2 else None)
            assert score(v)==before and boss(v)['hp']==300 and len(score_events(v))==2;close(page,host)
        scenario('real_boss_home_reset_preserves_score_without_new_credit',home,mode='home')
        for mode,growth,points,credited in [('zero-boss',52,40,3),('noncredit',52,40,2),('zero',22,0,3),('coefficient-zero',0,140,3),('inherit',108,114,3),('round',1,3,3)]:
            def matrix(host,page,context,mode=mode,growth=growth,points=points,credited=credited):
                auto(page);v=done(page,host);s=frozen(v)['body']['scoring'];assert v['ledger']['cultivation']==growth and (s['kills'],s['credited_kills'],s['kill_score'])==(3,credited,points)
                if mode=='noncredit':assert v['ledger']['items']['amber']==2
                if mode in ['zero','coefficient-zero']:assert frozen(v)['body']['looted']>0 or frozen(v)['body']['items']
                before=v['ledger'];request=frozen(v);host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
            scenario('configured_'+mode+'_and_real_loot_restore',matrix,mode=mode)
        def partial_extract(host,page,context):
            phase2(page,host);navigate(page,host,[700,700]);navigate(page,host,[900,800]);wait(lambda:current(page,host)['snapshot']['extraction_open']);assert send(page,host,{'kind':'extract_start'}).ok;v=done(page,host)
            assert boss(v)['hp']>0 and frozen(v)['body']['scoring']['kill_score']==40 and v['ledger']['cultivation']==52 and not any(e['kind']=='chest_opened' for e in v['events'])
        scenario('early_extract_only_settles_actual_ordinary_and_elite_score',partial_extract,mode='flat')
        def partial_death(host,page,context):phase2(page,host);v=close(page,host);assert v['ledger']['cultivation']==15 and frozen(v)['body']['scoring']['kill_score']==40
        scenario('early_death_does_not_award_alive_boss_points',partial_death,mode='flat')
        def forgery(host,page,context):
            phase2(page,host);assert send(page,host,{'kind':'attack','kill_score':9999}).json()['error']=='INVALID_MESSAGE';assert send(page,host,{'kind':'set_score','score':9999}).json()['error']=='UNSUPPORTED'
            attack(page,host,'forest-phase-boss-1');assert page.request.post(last_input[0],data=last_input[1]).ok
            v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='completed' else None);assert score(v)['kill_score']==140 and len(score_events(v))==3
            assert send(page,host,{'kind':'attack'}).json()['error']=='NO_RESOURCE';close(page,host)
        scenario('forged_score_duplicate_attack_and_dead_target_cannot_add_points',forgery,mode='flat')
        def mobile(host,page,context):
            phase2(page,host);page.wait_for_function("document.querySelector('#score-status').textContent.includes('实际击杀 2')&&document.querySelector('#score-status').textContent.includes('击杀积分 40')&&document.querySelector('#score-status').textContent.includes('若撤离 +52')");assert page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);close(page,host)
        scenario('mobile_shows_authoritative_actual_kills_points_and_growth_previews',mobile,mode='flat',mobile=True)
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context,point=point):
                auto(page);host.killed();page.goto('about:blank');state=host.state();request=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==127 and frozen(v)['body']==request['body'] and frozen(v)['operation_id']==request['operation_id']
            scenario('score_and_assets_'+point,crash,point=point)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['cultivation']==127 and any(e['kind']=='io_timed_out' for e in v['events'])
        scenario('lost_receipt_queries_original_weighted_score_assets_once',lost)
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=30000);before=current(page,host);request=frozen(before);old=before['configHash'];host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-8.json';c=json.loads(path.read_text());c['pve']['kill_score']=1;c['cultivation_coefficient']=.25
            for t in c['pve']['additional_templates'].values():t['kill_score']=1
            path.write_text(json.dumps(c));host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000);v=current(page,host);assert v['configHash']==old and frozen(v)==request
            pin=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json();assert pin['pve']['additional_templates']['verification-phase-boss']['kill_score']==100
            page.locator('#reconcile').click();v=done(page,host);assert v['ledger']['cultivation']==127 and v['settlement']['scoring']['kill_score']==140;assets=v['ledger'];host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==assets and frozen(v)==request
        scenario('score_coefficient_update_cannot_reinterpret_old_frozen_manual_responsibility',version)
        def interrupted(host,page,context):
            history=score_events(phase2(page,host));host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);l=v['ledger'];assert l['warehouse']==100 and l['cultivation']==0 and l['compensations']==1 and l['settlements']==0 and not l['items'] and not l['pending'] and not v['settlement'] and score_events(v)==history
            assert page.locator('#score-status').inner_text()=='';host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);assert v['ledger']==l and score_events(v)==history
        scenario('unfrozen_weighted_score_crash_only_refunds_entry_without_minting_points',interrupted,mode='flat',unfrozen=True)
        browser.close()
    assert len(results)==21 and not errors and sources==hashes(),'page error or source change during verification';assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest()
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-8','mechanismVersion':7,'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests)}),flush=True)
if __name__=='__main__':main()
