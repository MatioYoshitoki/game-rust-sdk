"""Sequential wave acceptance via public inputs, Rust clocks and durable SQLite."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-6';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir();aliases=['b1','b3-1','b3-2','b3-3','b3-4','b3-5','b3-6']
    for alias in aliases:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-6.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[200,400],extraction_point=[900,800],geometry={'min':[0,0],'max':[1024,1024],'walls':[]},extraction_unlock_ms=20000)
        c['params']['runtime']['publish_batched_ms']=900;p=c['pve'];p.update(map_id='wave-test-flat',width=32,height=32,cell_size=32,cells_rle=[[0,1024]])
        p['stats'].update(hp=60,spirit=0,atk=0,spd=0,crit=0,dodge=0);p['combat']['cast_chance']=0
        e=p['encounters'][0];e['trigger']={'center':[500,400],'radius':100};e['waves'][1]['delay_ms']=30000
        for i,m in enumerate(m for w in e['waves'] for m in w['monsters']):m['position']=[600+i*100,400]
        for i,chest in enumerate(p['chest_spawns']):chest['position']=[900,400+i*250]
        if mode=='paused':e['waves'][1]['delay_ms']=12000
        if mode=='old-blocker':e['waves'][0]['required_monster_ids']=e['waves'][0]['required_monster_ids'][:1];e['waves'][1]['monsters'][0]['position']=[700,400]
        if mode=='free':
            t={'mechanism':'melee_projectile_heal','name':'无计分目标','stats':copy.deepcopy(p['stats']),'combat':copy.deepcopy(p['combat']),'drop_table':{'guaranteed':[{'itemId':'amber','quantityMin':2,'quantityMax':2}],'random':[]},'leash':p['leash'],'help_range':p['help_range'],'memory_range':p['memory_range'],'kill_credit':False,'drop_orb':False};p['additional_templates']={'free-target':t};p['items']['amber']={'name':'配置晶石','stackLimit':20}
            for w in e['waves']:
                for m in w['monsters']:m['template_id']='free-target'
        if mode=='one':e['waves']=e['waves'][:1]
        if mode=='three':
            monsters=[m for w in e['waves'] for m in w['monsters']];e['waves']=[{'id':f'part-{i+1}','delay_ms':i*1500,'monsters':[m],'required_monster_ids':[m['id']]} for i,m in enumerate(monsters)]
        if mode=='zero':e['waves'][1]['delay_ms']=0;e['waves'][0]['delay_ms']=3000
        if mode=='respawn':p['treasure_rules']['template'].update(respawn_delay_ms=3000);p['treasure_rules']['template']['seal'].update(layers=1,layer_duration_ms=9000)
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{a:a+'.json' for a in aliases},'archives':[]}));return manifest

def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_6.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,text=True,capture_output=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M);tests=[t for t in all_tests if '::wave_tests::' in t or t.startswith('verification::waves::') or t.startswith('config::catalog::tests::wave_')]
    assert len(all_tests)>=282 and len(tests)==35,(len(all_tests),len(tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def encounter(v):return v['snapshot']['pve']['encounters'][0]
        def wave(v):return encounter(v)['wave']
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
            page.locator('#scenario').select_option('b3-6');page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.url.endswith('/api/sessions') and r.request.method=='POST'):page.locator('#verify').click()
        def begin(page):page.locator('#scenario').select_option('b3-6');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        serial=0
        def send(page,host,action):
            nonlocal serial
            for _ in range(3):
                serial+=1;v=current(page,host);response=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'wave-input-{serial}','action':action})
                if response.ok or response.json().get('error')!='STALE_SEQUENCE':return response
            raise AssertionError(response.text())
        def trigger(page,host):
            assert send(page,host,{'kind':'navigate','destination':[500,400]}).ok
            wait(lambda:(v:=current(page,host)) and encounter(v)['phase']=='active' and wave(v)['index']==1);assert send(page,host,{'kind':'stop'}).ok
        def kill_id(page,host,target):
            v=current(page,host);m=next(m for m in v['snapshot']['pve']['monsters'] if m['id']==target)
            assert send(page,host,{'kind':'navigate','destination':m['position']}).ok
            wait(lambda:(v:=current(page,host)) and sum((a-b)**2 for a,b in zip(v['snapshot']['position'],m['position']))<65**2);assert send(page,host,{'kind':'stop'}).ok
            wait(lambda:(v:=current(page,host)) and v['snapshot']['logical_ms']>=v['snapshot']['basic_ready_at_ms']);assert send(page,host,{'kind':'attack'}).ok
            return wait(lambda:v if (v:=current(page,host)) and next(m for m in v['snapshot']['pve']['monsters'] if m['id']==target)['hp']==0 else None)
        def clear_wave(page,host):
            ids=list(wave(current(page,host))['required_monster_ids'])
            for target in ids:kill_id(page,host,target)
            return current(page,host)
        def first_done(page,host):
            trigger(page,host);clear_wave(page,host)
            return wait(lambda:v if (v:=current(page,host)) and wave(v)['completed_waves']==1 else None)
        def opening(g=1):return {'kind':'open_chest','chest_id':'wave-cache-1','expected_generation':g}
        def finish_manual(page,host):
            first_done(page,host);wait(lambda:(v:=current(page,host)) and encounter(v)['phase']=='active' and wave(v)['index']==2);clear_wave(page,host)
            return wait(lambda:v if encounter(v:=current(page,host))['phase']=='completed' else None)
        def scenario(name,work,mode='source',point='',mobile=False):
            with tempfile.TemporaryDirectory(prefix='waves-') as directory:
                host=Host(directory,package(directory,mode));context=browser.new_context(viewport={'width':390,'height':844} if mobile else {'width':1440,'height':1100},has_touch=mobile,is_mobile=mobile)
                page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                try:
                    host.start(point,'commit_settlement' if point else '');page.goto(host.url);work(host,page,context);v=current(page,host);check(v)
                    assert page.locator('#world-live').inner_text()=='0' and not errors,errors
                    results[name]={'ledger':v['ledger'],'snapshot':v['snapshot'],'events':v['events'],'configHash':v['configHash'],'settlementRequest':frozen(v)};print(name+': passed',flush=True)
                except Exception:
                    try:page.screenshot(path=str(OUTPUT/'failure.png'),full_page=True);(OUTPUT/'failure.json').write_text(json.dumps(current(page,host),ensure_ascii=False,indent=2))
                    except Exception:pass
                    host.error_log.seek(0);print(host.error_log.read(),flush=True);raise
                finally:context.close();host.stop();host.error_log.close()
        def extraction(host,page,context):
            auto(page);v=done(page,host);assert v['ledger']['cultivation']==45 and v['snapshot']['pve']['kills']==3 and wave(v)['completed_waves']==2
            assert [e['wave_index'] for e in v['events'] if e['kind']=='encounter_wave_started']==[1,2]
            assert [e['wave_index'] for e in v['events'] if e['kind']=='encounter_wave_completed']==[1,2]
            assert sum(e['kind']=='encounter_completed' for e in v['events'])==sum(e['kind']=='encounter_started' for e in v['events'])==1
            assert sum(e['kind']=='chest_opened' for e in v['events'])==2
            page.wait_for_function("document.body.dataset.verification==='passed'");page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True);before=v['ledger'];request=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
        scenario('two_waves_targets_reward_pickup_extraction_and_repeated_restart',extraction)
        def death(host,page,context):
            auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and encounter(v)['phase']=='completed' and all(c['opened'] for c in v['snapshot']['pve']['chests']) and not v['snapshot']['resources']);page.locator('#close').click();v=done(page,host);assert v['ledger']['cultivation']==11 and frozen(v)['body']['outcome']=='death'
        scenario('all_wave_picked_assets_death_conserve_each_item',death)
        def waiting(host,page,context):
            begin(page);v=first_done(page,host);assert encounter(v)['phase']=='waiting' and wave(v)['index']==2 and not wave(v)['monster_ids'];deadline=wave(v)['ready_at_ms'];old=v['connection'];ids=encounter(v)['monster_ids']
            assert send(page,host,opening()).json()['error']=='ENCOUNTER_LOCKED'
            forged=opening();forged['wave_index']=2;assert send(page,host,forged).json()['error']=='INVALID_MESSAGE'
            page.reload();active(page);v=current(page,host);assert wave(v)['ready_at_ms']==deadline and encounter(v)['monster_ids']==ids and wave(v)['completed_waves']==1
            r=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':old,'sequence':v['sequence']+1,'requestId':'old-wave-open','action':opening()});assert r.json()['error']=='STALE_CONNECTION'
            page.screenshot(path=str(OUTPUT/'waiting.png'),full_page=True)
            wait(lambda:(v:=current(page,host)) and encounter(v)['phase']=='active' and wave(v)['index']==2);v=current(page,host);assert sum(e['kind']=='encounter_wave_started' and e['wave_index']==2 for e in v['events'])==1
            page.locator('#close').click();done(page,host)
        scenario('waiting_deadline_and_old_entities_survive_refresh_reward_locked_and_old_owner_fenced',waiting,mode='flat')
        def blocked(host,page,context):
            begin(page);first_done(page,host);assert send(page,host,{'kind':'navigate','destination':[800,400]}).ok
            v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='spawning' else None);assert not wave(v)['monster_ids'] and wave(v)['index']==2 and len(encounter(v)['monster_ids'])==2
            assert send(page,host,opening()).json()['error']=='ENCOUNTER_LOCKED';page.reload();active(page);assert encounter(current(page,host))['phase']=='spawning'
            assert send(page,host,{'kind':'navigate','destination':[200,700]}).ok
            v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='active' else None);assert len(wave(v)['monster_ids'])==1
            assert sum(e['kind']=='encounter_wave_spawn_blocked' for e in v['events'])==1;page.locator('#close').click();done(page,host)
        scenario('later_wave_player_blocked_group_retries_once_after_refresh_and_clearance',blocked,mode='flat')
        def old_blocker(host,page,context):
            begin(page);first_done(page,host);v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='spawning' else None);assert not wave(v)['monster_ids']
            kill_id(page,host,'forest-wave-1-wolf-2');v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='active' else None)
            assert wave(v)['index']==2 and wave(v)['defeated_required']==0
            clear_wave(page,host);v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='completed' else None)
            assert v['snapshot']['pve']['kills']==3;page.locator('#close').click();done(page,host)
        scenario('nonrequired_previous_enemy_blocks_new_wave_and_its_death_is_not_new_progress',old_blocker,mode='old-blocker')
        def protected(host,page,context):
            begin(page);v=first_done(page,host);deadline=wave(v)['ready_at_ms'];page.reload();active(page);page.locator('#pause').click();wait(lambda:current(page,host)['snapshot']['protected']);page.wait_for_timeout(600)
            v=current(page,host);assert v['snapshot']['logical_ms']>=deadline and encounter(v)['phase']=='waiting' and wave(v)['ready_at_ms']==deadline and not wave(v)['monster_ids']
            page.locator('#pause').click();active(page);v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='active' else None);assert wave(v)['index']==2;page.locator('#close').click();done(page,host)
        scenario('background_crosses_deadline_without_spawning_then_resumes_original_next_wave',protected,mode='paused')
        def active_refresh(host,page,context):
            begin(page);trigger(page,host);kill_id(page,host,'forest-wave-1-wolf-1');before=wave(current(page,host));page.reload();active(page);assert wave(current(page,host))==before
            page.locator('#close').click();v=done(page,host);assert v['ledger']['cultivation']==3 and not any(e['kind']=='encounter_wave_started' and e['wave_index']==2 for e in v['events'])
        scenario('active_partial_first_wave_refresh_and_death_never_start_future_wave',active_refresh,mode='flat')
        for mode,count,growth,kills in [('one',1,37,2),('three',3,45,3),('zero',2,45,3),('free',2,22,0)]:
            def matrix(host,page,context,mode=mode,count=count,growth=growth,kills=kills):
                auto(page);v=done(page,host);assert wave(v)['completed_waves']==count and wave(v)['total']==count and v['ledger']['cultivation']==growth and v['snapshot']['pve']['kills']==kills
                assert [e['wave_index'] for e in v['events'] if e['kind']=='encounter_wave_started']==list(range(1,count+1))
                if mode=='free':assert v['ledger']['items']['amber']==6
            scenario('configured_'+mode+'_wave_count_delay_or_noncredit_targets',matrix,mode=mode)
        def mobile(host,page,context):
            begin(page);first_done(page,host);page.reload();active(page);page.wait_for_function("document.querySelector('#encounter-status').textContent.includes('第2/2波')");assert page.locator('#open-chest').is_disabled() and page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);page.locator('#close').click();done(page,host)
        scenario('mobile_shows_current_wave_targets_and_authoritative_wait',mobile,mode='flat',mobile=True)
        def partial_extract(host,page,context):
            begin(page);first_done(page,host);assert send(page,host,{'kind':'navigate','destination':[900,800]}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],[900,800]))<60**2);assert send(page,host,{'kind':'stop'}).ok
            wait(lambda:current(page,host)['snapshot']['extraction_open']);assert send(page,host,{'kind':'extract_start'}).ok;v=done(page,host)
            assert encounter(v)['phase']!='completed' and v['snapshot']['pve']['kills']==2 and v['ledger']['cultivation']==37 and not any(e['kind']=='chest_opened' for e in v['events'])
        scenario('partial_wave_extraction_only_settles_real_kills_and_picked_assets',partial_extract,mode='flat')
        def respawn(host,page,context):
            begin(page);finish_manual(page,host);assert send(page,host,{'kind':'navigate','destination':[900,400]}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],[900,400]))<50**2);assert send(page,host,{'kind':'stop'}).ok
            assert send(page,host,opening()).ok;wait(lambda:current(page,host)['snapshot']['pve']['chests'][0]['seal']['completed_layers']==1);assert send(page,host,opening()).ok
            v=wait(lambda:v if (v:=current(page,host))['snapshot']['pve']['chests'][0]['generation']==2 else None);assert wave(v)['completed_waves']==2 and sum(e['kind']=='encounter_wave_started' for e in v['events'])==2
            assert send(page,host,opening()).json()['error']=='REVISION_CONFLICT';page.locator('#close').click();done(page,host)
        scenario('container_respawn_preserves_all_waves_complete_and_old_generation_fenced',respawn,mode='respawn')
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context,point=point):
                auto(page);host.killed();page.goto('about:blank');state=host.state();request=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==45 and frozen(v)['body']==request['body'] and frozen(v)['operation_id']==request['operation_id']
            scenario('multiwave_assets_'+point,crash,point=point)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['cultivation']==45 and any(e['kind']=='io_timed_out' for e in v['events'])
        scenario('lost_receipt_queries_frozen_multiwave_assets_once',lost)
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=30000);before=current(page,host);request=frozen(before);old=before['configHash'];host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-6.json';c=json.loads(path.read_text());c['pve']['encounters'][0]['waves']=c['pve']['encounters'][0]['waves'][:1];c['pve']['encounters'][0]['waves'][0]['delay_ms']=6000;c['cultivation_coefficient']=.5;path.write_text(json.dumps(c))
            host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000);v=current(page,host);assert v['configHash']==old and frozen(v)==request
            pin=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json();assert len(pin['pve']['encounters'][0]['waves'])==2
            page.locator('#reconcile').click();v=done(page,host);assert v['ledger']['cultivation']==45;assets=v['ledger'];host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==assets and frozen(v)==request
        scenario('wave_count_delay_growth_update_cannot_reinterpret_old_manual_responsibility',version)
        for phase in ['waiting','second-active']:
            with tempfile.TemporaryDirectory(prefix='wave-unfrozen-') as directory:
                host=Host(directory,package(directory,'flat'));context=browser.new_context();page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                try:
                    host.start();page.goto(host.url);begin(page);first_done(page,host)
                    if phase=='second-active':wait(lambda:(v:=current(page,host)) and encounter(v)['phase']=='active' and wave(v)['index']==2)
                    host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);l=v['ledger'];assert l['warehouse']==100 and l['compensations']==1 and l['settlements']==0 and not l['items'] and not l['pending'] and not v['settlement'];name='unfrozen_'+phase+'_restart_refunds_entry_without_replaying_waves';results[name]={'ledger':l,'events':v['events'],'configHash':v['configHash']};print(name+': passed',flush=True)
                finally:context.close();host.stop();host.error_log.close()
        browser.close()
    assert not errors and sources==hashes(),'page error or source change during verification';assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest()
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-6','mechanismVersion':5,'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests)}),flush=True)
if __name__=='__main__':main()
