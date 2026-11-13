"""Boss phase acceptance through public H5 inputs, real Rust clocks and SQLite."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-7';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir();aliases=['b1','b3-1','b3-2','b3-3','b3-4','b3-5','b3-6','b3-7']
    for alias in aliases:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-7.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[200,400],extraction_point=[900,800],geometry={'min':[0,0],'max':[1024,1024],'walls':[]},extraction_unlock_ms=20000)
        c['params']['runtime']['publish_batched_ms']=900;c['params']['regen'].update(near_ratio=0,far_ratio=0)
        p=c['pve'];p.update(map_id='boss-test-flat',width=32,height=32,cell_size=32,cells_rle=[[0,1024]])
        p['stats'].update(hp=60,spirit=0,atk=0,spd=0,crit=0,dodge=0);p['combat']['cast_chance']=0
        e=p['encounters'][0];e['trigger']={'center':[500,400],'radius':100};e['waves'][1]['delay_ms']=3000
        for i,m in enumerate(m for w in e['waves'] for m in w['monsters']):m['position']=[600+i*100,400]
        for i,chest in enumerate(p['chest_spawns']):chest['position']=[900,400+i*250]
        t=p['additional_templates']['verification-phase-boss'];t['stats'].update(hp=300,spirit=100,atk=0,spd=0,crit=0,dodge=0);t['stats']['def']=0;t['leash']=700
        for phase in t['boss']['phases']:phase['cast_chance']=0
        if mode=='one':t['boss']['phases']=t['boss']['phases'][:1]
        if mode in ['three','cross']:
            t['boss']['phases']=[{'id':f'phase-{i+1}','name':f'阶段{i+1}','hp_threshold_bps':threshold,'attack_interval_ms':interval,'cast_chance':0} for i,(threshold,interval) in enumerate([(10000,2000),(8000,1300),(4000,600)])]
        if mode=='home':t['leash']=150;t['combat']['aggro_range']=150
        if mode=='free':
            t['kill_credit']=False;t['drop_table']['guaranteed']=[{'itemId':'amber','quantityMin':2,'quantityMax':2}];p['items']['amber']={'name':'配置晶石','stackLimit':20}
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{a:a+'.json' for a in aliases},'archives':[]}));return manifest

def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_7.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,text=True,capture_output=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M);tests=[t for t in all_tests if '::boss_tests::' in t or t.startswith('verification::bosses::') or t.startswith('config::catalog::tests::boss_')]
    assert len(all_tests)>=317 and len(tests)==35,(len(all_tests),len(tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def encounter(v):return v['snapshot']['pve']['encounters'][0]
        def boss(v):return next(m for m in v['snapshot']['pve']['monsters'] if m.get('boss'))
        def changes(v):return [e for e in v['events'] if e['kind']=='boss_phase_changed']
        def frozen(v):return next(r for r in v['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):
            page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed'&&document.querySelector('#pending').textContent==='0'",timeout=30000)
            return wait(lambda:v if (v:=current(page,host)) and v['settlement'] and not v['ledger']['pending'] else None)
        def check(v):
            b=frozen(v)['body'];l=v['ledger'];assert b['carried']+b['looted']==b['kept']+b['dropped']
            assert l['warehouse']==100-b['carried']+b['kept'] and l['cultivation']==b['cultivation']
            assert l['items']=={i['item_id']:i['kept'] for i in b.get('items',[])} and all(i['looted']==i['kept']+i['dropped'] for i in b.get('items',[]))
            assert l['admissions']==l['settlements']==1 and not l['pending'] and l['manual']==0 and v['phase']=='closed'
            assert sum(e['kind']=='settlement_applied' for e in v['events'])==1
        def auto(page,fault='none'):
            page.locator('#scenario').select_option('b3-7');page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.url.endswith('/api/sessions') and r.request.method=='POST'):page.locator('#verify').click()
        def begin(page):page.locator('#scenario').select_option('b3-7');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        serial=0
        def send(page,host,action):
            nonlocal serial
            for _ in range(3):
                serial+=1;v=current(page,host);response=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'boss-input-{serial}','action':action})
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
        def opening():return {'kind':'open_chest','chest_id':'boss-cache-1','expected_generation':1}
        def close(page,host):page.locator('#close').click();return done(page,host)
        def scenario(name,work,mode='source',point='',mobile=False,unfrozen=False):
            with tempfile.TemporaryDirectory(prefix='boss-') as directory:
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
            auto(page);v=done(page,host);assert v['ledger']['cultivation']==45 and v['snapshot']['pve']['kills']==3 and encounter(v)['phase']=='completed'
            assert len(changes(v))==1 and changes(v)[0]['phase_index']==2 and sum(e['kind']=='chest_opened' for e in v['events'])==2
            assert sum(e['kind']=='monster_killed' and e['monster_id']=='forest-phase-boss-1' for e in v['events'])==1
            page.wait_for_function("document.body.dataset.verification==='passed'");page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True);before=v['ledger'];request=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
        scenario('ordinary_wave_boss_transition_kill_boxes_pickup_extract_and_two_restarts',extraction)
        def death(host,page,context):
            auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and encounter(v)['phase']=='completed' and all(c['opened'] for c in v['snapshot']['pve']['chests']) and not v['snapshot']['resources']);v=close(page,host);assert v['ledger']['cultivation']==11 and frozen(v)['body']['outcome']=='death'
        scenario('complete_boss_loot_death_conserves_each_asset',death)
        def phase1(host,page,context):
            reach_boss(page,host);v=current(page,host);assert boss(v)['boss']['phase_index']==1 and send(page,host,opening()).json()['error']=='ENCOUNTER_LOCKED';assert not changes(v);close(page,host)
        scenario('first_phase_is_configured_and_reward_remains_locked',phase1,mode='flat')
        def gate(host,page,context):
            v=phase2(page,host);assert boss(v)['hp']>0 and v['snapshot']['pve']['kills']==2 and encounter(v)['phase']=='active'
            assert send(page,host,opening()).json()['error']=='ENCOUNTER_LOCKED'
            assert send(page,host,{'kind':'attack','boss_phase':1,'hp':0}).json()['error']=='INVALID_MESSAGE'
            assert send(page,host,{'kind':'set_boss_phase','index':1}).json()['error']=='UNSUPPORTED'
            assert len(changes(current(page,host)))==1;page.screenshot(path=str(OUTPUT/'phase-two.png'),full_page=True);close(page,host)
        scenario('nonlethal_phase_change_neither_completes_wave_nor_accepts_forged_state',gate,mode='flat')
        def refresh(host,page,context):
            v=phase2(page,host);before=boss(v);old=v['connection'];page.reload();active(page);v=current(page,host);assert boss(v)==before
            r=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':old,'sequence':v['sequence']+1,'requestId':'old-boss-owner','action':{'kind':'attack'}});assert r.json()['error']=='STALE_CONNECTION'
            attack(page,host,before['id']);v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='completed' else None);assert boss(v)['hp']==0 and len(changes(v))==1;close(page,host)
        scenario('phase_hp_mp_epoch_survive_refresh_and_old_connection_is_fenced',refresh,mode='flat')
        def protected(host,page,context):
            phase2(page,host);page.reload();active(page);before=boss(current(page,host));page.locator('#pause').click();wait(lambda:current(page,host)['snapshot']['protected']);page.wait_for_timeout(300);assert boss(current(page,host))==before
            page.locator('#pause').click();active(page);assert boss(current(page,host))==before;close(page,host)
        scenario('background_and_foreground_preserve_phase_without_new_transition',protected,mode='flat')
        def home(host,page,context):
            phase2(page,host);navigate(page,host,[200,700]);v=wait(lambda:v if boss(v:=current(page,host))['boss']['epoch']==2 else None)
            assert boss(v)['hp']==300 and boss(v)['boss']['phase_index']==1 and boss(v)['boss']['attack_interval_ms']==2000
            page.wait_for_timeout(200);assert sum(e['kind']=='boss_reset' for e in current(page,host)['events'])==1;close(page,host)
        scenario('real_home_completion_restores_hp_and_first_phase_once',home,mode='home')
        for mode,growth,kills,phase_count in [('one',45,3,1),('three',45,3,3),('free',37,2,2)]:
            def matrix(host,page,context,mode=mode,growth=growth,kills=kills,phase_count=phase_count):
                auto(page);v=done(page,host);assert v['ledger']['cultivation']==growth and v['snapshot']['pve']['kills']==kills and boss(v)['boss']['phase_count']==phase_count
                if mode=='free':assert v['ledger']['items']['amber']==2
                if mode=='one':assert not changes(v)
            scenario('configured_'+mode+'_phases_or_noncredit_boss_with_item_drop',matrix,mode=mode)
        def crossing(host,page,context):
            reach_boss(page,host);attack(page,host,'forest-phase-boss-1');v=attack(page,host,'forest-phase-boss-1');assert [e['phase_index'] for e in changes(v)]==[2,3];assert boss(v)['hp']==100;close(page,host)
        scenario('three_phase_configuration_advances_only_on_actual_threshold_damage',crossing,mode='cross')
        def lethal(host,page,context):
            reach_boss(page,host)
            for _ in range(3):v=attack(page,host,'forest-phase-boss-1')
            v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='completed' else None);assert boss(v)['hp']==0 and len(changes(v))==1;assert send(page,host,{'kind':'attack'}).json()['error']=='NO_RESOURCE';close(page,host)
        scenario('final_hit_completes_once_and_dead_target_cannot_repeat_reward',lethal,mode='flat')
        def mobile(host,page,context):
            phase2(page,host);page.wait_for_function("document.querySelector('#boss-status').textContent.includes('阶段 2/2')");assert page.locator('#open-chest').is_disabled() and page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);close(page,host)
        scenario('mobile_shows_authoritative_stage_hp_and_effective_interval',mobile,mode='flat',mobile=True)
        def partial_extract(host,page,context):
            phase2(page,host);navigate(page,host,[700,700]);navigate(page,host,[900,800]);wait(lambda:current(page,host)['snapshot']['extraction_open']);assert send(page,host,{'kind':'extract_start'}).ok;v=done(page,host)
            assert boss(v)['hp']>0 and v['snapshot']['pve']['kills']==2 and v['ledger']['cultivation']==37 and not any(e['kind']=='chest_opened' for e in v['events'])
        scenario('partial_phase_extraction_only_counts_two_real_kills',partial_extract,mode='flat')
        def partial_death(host,page,context):
            phase2(page,host);v=close(page,host);assert v['ledger']['cultivation']==7 and boss(v)['hp']>0 and v['snapshot']['pve']['kills']==2
        scenario('partial_phase_death_has_no_boss_or_box_credit',partial_death,mode='flat')
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context,point=point):
                auto(page);host.killed();page.goto('about:blank');state=host.state();request=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==45 and frozen(v)['body']==request['body'] and frozen(v)['operation_id']==request['operation_id']
            scenario('boss_assets_'+point,crash,point=point)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['cultivation']==45 and any(e['kind']=='io_timed_out' for e in v['events'])
        scenario('lost_receipt_queries_original_boss_assets_once',lost)
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=30000);before=current(page,host);request=frozen(before);old=before['configHash'];host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-7.json';c=json.loads(path.read_text());phases=c['pve']['additional_templates']['verification-phase-boss']['boss']['phases'];phases[1].update(hp_threshold_bps=7500,attack_interval_ms=400,cast_chance=.8);c['cultivation_coefficient']=.5;path.write_text(json.dumps(c))
            host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000);v=current(page,host);assert v['configHash']==old and frozen(v)==request
            pin=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json();assert pin['pve']['additional_templates']['verification-phase-boss']['boss']['phases'][1]['hp_threshold_bps']==5000
            page.locator('#reconcile').click();v=done(page,host);assert v['ledger']['cultivation']==45;assets=v['ledger'];host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==assets and frozen(v)==request
        scenario('phase_threshold_interval_and_growth_update_preserve_frozen_responsibility',version)
        def interrupted(host,page,context):
            history=changes(phase2(page,host));host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);l=v['ledger'];assert l['warehouse']==100 and l['compensations']==1 and l['settlements']==0 and not l['items'] and not l['pending'] and not v['settlement']
            assert changes(v)==history and not any(e['kind']=='boss_reset' for e in v['events'])
            host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);assert v['ledger']==l and changes(v)==history
        scenario('unfrozen_phase_two_crash_refunds_entry_without_world_or_phase_replay',interrupted,mode='flat',unfrozen=True)
        browser.close()
    assert len(results)==20 and not errors and sources==hashes(),'page error or source change during verification';assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest()
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-7','mechanismVersion':6,'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests)}),flush=True)
if __name__=='__main__':main()
