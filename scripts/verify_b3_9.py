"""Locked-circle telegraphs via public movement, real Rust clocks and durable assets."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-9';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir();aliases=['b1','b3-1','b3-2','b3-3','b3-4','b3-5','b3-6','b3-7','b3-8','b3-9']
    for alias in aliases:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-9.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[200,400],extraction_point=[900,800],geometry={'min':[0,0],'max':[1024,1024],'walls':[]},extraction_unlock_ms=20000)
        c['params']['runtime']['publish_batched_ms']=900;c['params']['regen'].update(near_ratio=0,far_ratio=0);c['params']['minimum_damage']=0
        p=c['pve'];p.update(map_id='area-test-flat',width=32,height=32,cell_size=32,cells_rle=[[0,1024]])
        p['stats'].update(hp=60,spirit=0,atk=0,spd=0,crit=0,dodge=0);p['combat']['cast_chance']=0
        elite=p['additional_templates']['verification-score-elite'];elite['stats']=copy.deepcopy(p['stats']);elite['combat']['cast_chance']=0
        e=p['encounters'][0];e['trigger']={'center':[500,400],'radius':100};e['waves'][1]['delay_ms']=3000
        for i,m in enumerate(m for w in e['waves'] for m in w['monsters']):m['position']=[600+i*100,400]
        for i,chest in enumerate(p['chest_spawns']):chest['position']=[900,400+i*250]
        t=p['additional_templates']['verification-phase-boss'];t['stats'].update(hp=300,spirit=100,atk=100,spd=0,crit=0,dodge=0);t['stats']['def']=0;t['leash']=700;t['combat']['melee_range']=1
        for phase in t['boss']['phases']:phase['cast_chance']=0
        a=t['area'];a.update(windup_ms=60000,cooldown_ms=180000,recovery_ms=900,damage_bps=10000)
        if mode=='return':t['leash']=200;t['combat']['aggro_range']=200
        if mode=='zero-chance':a['cast_chance']=0
        if mode=='short-mp':t['stats']['spirit']=11
        if mode=='phase2':a['min_phase_index']=2
        if mode=='zero-damage':a['damage_bps']=0
        if mode=='variant':a.update(id='configured-shock',name='配置冲击',radius=48,windup_ms=30000,cost=20,damage_bps=5000)
        if mode=='lethal':t['stats']['atk']=1000000;a['damage_bps']=100000
        if mode=='two':p['extra_spawns']=[{'id':'extra-area-2','template_id':'verification-phase-boss','position':[800,700]}]
        if mode in ['vision','effect']:
            tile=copy.deepcopy(p['palette'][0]);tile.update(walkable=True,blocksVision=mode=='vision',blocksProjectiles=mode=='effect');index=len(p['palette']);p['palette'].append(tile);cells=[0]*1024
            for y in range(11,14):cells[y*32+22]=index
            p['cells_rle']=[[value,1] for value in cells]
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{a:a+'.json' for a in aliases},'archives':[]}));return manifest
def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_9.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,text=True,capture_output=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M);tests=[t for t in all_tests if '::area_tests::' in t or t.startswith('verification::area::') or t.startswith('config::catalog::tests::area_')]
    assert len(all_tests)>=395 and len(tests)==42,(len(all_tests),len(tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def encounter(v):return v['snapshot']['pve']['encounters'][0]
        def boss(v):return next(m for m in v['snapshot']['pve']['monsters'] if m['id']=='forest-phase-boss-1')
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
            page.locator('#scenario').select_option('b3-9');page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.url.endswith('/api/sessions') and r.request.method=='POST'):page.locator('#verify').click()
        def begin(page):page.locator('#scenario').select_option('b3-9');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
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
        def opening():return {'kind':'open_chest','chest_id':'area-cache-1','expected_generation':1}
        def close(page,host):page.locator('#close').click();return done(page,host)
        def scenario(name,work,mode='source',point='',mobile=False,unfrozen=False):
            with tempfile.TemporaryDirectory(prefix='area-') as directory:
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
        def warnings(v):return [m['area_warning'] for m in v['snapshot']['pve']['monsters'] if m.get('area_warning')]
        def events(v,kind):return [e for e in v['events'] if e['kind']==kind]
        def warning(page,host):return wait(lambda:a if (v:=current(page,host)) and (a:=boss(v).get('area_warning')) else None)
        def resolved(page,host,a):return wait(lambda:e if (v:=current(page,host)) and (e:=next((e for e in events(v,'area_resolved') if e['monster_id']=='forest-phase-boss-1' and e['cast_id']==a['cast_id']),None)) else None)
        def canceled(v,a,reason):return any(e['reason']==reason and any(k['monster_id']=='forest-phase-boss-1' and k['cast_id']==a['cast_id'] for k in e['casts']) for e in events(v,'area_cancelled'))
        def no_old_impact(page,host,a):
            wait(lambda:current(page,host)['snapshot']['logical_ms']>a['resolves_at_ms']+900)
            assert not any(e['cast_id']==a['cast_id'] and e['monster_id']=='forest-phase-boss-1' for e in events(current(page,host),'area_resolved'))
        def setup(page,host):reach_boss(page,host);return warning(page,host)
        def extraction(host,page,context):
            auto(page);v=done(page,host);assert v['ledger']['cultivation']==127 and score(v)['kill_score']==140
            assert events(v,'area_started') and events(v,'area_resolved') and all(not m.get('area_warning') for m in v['snapshot']['pve']['monsters'])
            page.wait_for_function("document.body.dataset.verification==='passed'");page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True);before=v['ledger'];request=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
        scenario('real_forest_area_skill_boss_boxes_loot_extract_and_two_restarts',extraction)
        def death(host,page,context):
            auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and encounter(v)['phase']=='completed' and all(c['opened'] for c in v['snapshot']['pve']['chests']) and not v['snapshot']['resources']);v=close(page,host);assert v['ledger']['cultivation']==52
        scenario('complete_area_boss_loot_death_conserves_all_assets',death)
        def stay(host,page,context):
            a=setup(page,host);hp=current(page,host)['snapshot']['hp'];page.wait_for_function("document.querySelector('#area-status').textContent.includes('撼地击')&&document.querySelector('#arena').dataset.areaWarnings==='1'");page.screenshot(path=str(OUTPUT/'warning.png'),full_page=True)
            e=resolved(page,host,a);assert e['hit'] and e['damage']>0;v=current(page,host);assert v['snapshot']['hp']==hp-e['damage'] and boss(v)['spirit']==88 and len(events(v,'area_resolved'))==1;close(page,host)
        scenario('stationary_player_is_hit_once_after_authoritative_windup',stay,mode='flat')
        def dodge(host,page,context):
            a=setup(page,host);hp=current(page,host)['snapshot']['hp'];navigate(page,host,[500,700]);e=resolved(page,host,a);assert not e['hit'] and e['damage']==0 and current(page,host)['snapshot']['hp']==hp;close(page,host)
        scenario('public_navigation_leaves_locked_circle_and_avoids_damage',dodge,mode='flat')
        def relock(host,page,context):
            a=setup(page,host);navigate(page,host,[500,700]);assert boss(current(page,host))['area_warning']['center']==a['center'];navigate(page,host,a['center']);assert resolved(page,host,a)['hit'];close(page,host)
        scenario('circle_does_not_follow_movement_and_reentry_is_judged_at_deadline',relock,mode='flat')
        def vision(host,page,context):
            reach_boss(page,host);page.wait_for_timeout(250);v=current(page,host);assert not boss(v).get('area_warning') and not events(v,'area_started') and boss(v)['spirit']==100;close(page,host)
        scenario('vision_only_terrain_blocks_start_without_spirit_cost',vision,mode='vision')
        def effect(host,page,context):
            a=setup(page,host);navigate(page,host,[a['center'][0]+100,a['center'][1]]);position=current(page,host)['snapshot']['position'];assert sum((x-y)**2 for x,y in zip(position,a['center']))<(a['radius']+16)**2
            assert not resolved(page,host,a)['hit'];close(page,host)
        scenario('projectile_only_terrain_blocks_area_effect_inside_radius',effect,mode='effect')
        for mode in ['zero-chance','short-mp']:
            def disabled(host,page,context,mode=mode):
                reach_boss(page,host);page.wait_for_timeout(250);v=current(page,host);assert not events(v,'area_started') and boss(v)['spirit']==(11 if mode=='short-mp' else 100);close(page,host)
            scenario('configured_'+mode+'_does_not_start_warning_or_spend',disabled,mode=mode)
        def phase(host,page,context):
            reach_boss(page,host);assert not boss(current(page,host)).get('area_warning');attack(page,host,'forest-phase-boss-1');attack(page,host,'forest-phase-boss-1');a=warning(page,host);assert boss(current(page,host))['boss']['phase_index']==2 and a['skill_id']=='verification-earth-shock';close(page,host)
        scenario('configured_phase_gate_enables_skill_only_after_actual_threshold',phase,mode='phase2')
        def zero(host,page,context):
            a=setup(page,host);hp=current(page,host)['snapshot']['hp'];e=resolved(page,host,a);assert e['hit'] and e['damage']==0 and current(page,host)['snapshot']['hp']==hp;close(page,host)
        scenario('configured_zero_damage_has_one_resolution_without_hp_loss',zero,mode='zero-damage')
        def variant(host,page,context):
            a=setup(page,host);assert a['name']=='配置冲击' and a['radius']==48 and a['resolves_at_ms']-a['started_at_ms']==30000 and boss(current(page,host))['spirit']==80;assert resolved(page,host,a)['hit'];close(page,host)
        scenario('configured_name_radius_time_cost_and_damage_load_without_recompile',variant,mode='variant')
        def pause_direct(host,page,context):
            begin(page);page.locator('#navigate-encounter').click();wait(lambda:encounter(current(page,host))['phase']=='active')
            for target in ['forest-wave-1-wolf-1','forest-wave-1-wolf-2']:
                wait(lambda:current(page,host)['snapshot']['logical_ms']>=current(page,host)['snapshot']['basic_ready_at_ms']);page.locator('#attack').click();wait(lambda:next(m for m in current(page,host)['snapshot']['pve']['monsters'] if m['id']==target)['hp']==0)
            wait(lambda:encounter(current(page,host))['phase']=='active' and encounter(current(page,host))['wave']['index']==2)
            # Move into the configured aggro range using the page's own navigation control.
            page.locator('#navigate-chest').click();a=warning(page,host);page.locator('#pause').click();wait(lambda:current(page,host)['snapshot']['protected']);v=current(page,host);assert canceled(v,a,'background');mp=boss(v)['spirit'];no_old_impact(page,host,a);page.locator('#pause').click();active(page);assert boss(current(page,host))['spirit']==mp;close(page,host)
        scenario('protection_cancels_warning_and_keeps_paid_spirit_without_late_hit',pause_direct,mode='flat')
        def refresh(host,page,context):
            a=setup(page,host);v=current(page,host);old=v['connection'];mp=boss(v)['spirit'];page.reload();active(page);v=current(page,host);assert canceled(v,a,'background') and not boss(v).get('area_warning') and boss(v)['spirit']==mp
            r=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':old,'sequence':v['sequence']+1,'requestId':'old-area-owner','action':{'kind':'attack'}});assert r.json()['error']=='STALE_CONNECTION';no_old_impact(page,host,a);close(page,host)
        scenario('refresh_cancels_old_circle_preserves_cost_and_fences_old_connection',refresh,mode='flat')
        def owner_death(host,page,context):
            a=setup(page,host)
            for _ in range(3):attack(page,host,'forest-phase-boss-1')
            v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='completed' else None);assert canceled(v,a,'owner_died') and score(v)['kill_score']==140;no_old_impact(page,host,a);close(page,host)
        scenario('owner_death_cancels_pending_impact_and_scores_boss_once',owner_death,mode='flat')
        def returning(host,page,context):
            a=setup(page,host);navigate(page,host,[200,700]);v=wait(lambda:v if canceled(v:=current(page,host),a,'returning') else None);assert boss(v)['spirit']==88 and boss(v)['boss']['epoch']==2 and score(v)['kill_score']==40;no_old_impact(page,host,a);close(page,host)
        scenario('returning_and_home_reset_cancel_circle_without_refunding_mp',returning,mode='return')
        def closed_warning(host,page,context):
            a=setup(page,host);v=close(page,host);assert canceled(v,a,'closed') and page.locator('#area-status').inner_text()=='' and v['ledger']['cultivation']==15
        scenario('close_clears_warning_and_freezes_only_existing_kills',closed_warning,mode='flat')
        def lethal(host,page,context):
            a=setup(page,host);v=done(page,host);e=next(e for e in events(v,'area_resolved') if e['cast_id']==a['cast_id']);assert e['hit'] and e['damage']==1000 and v['ledger']['cultivation']==15 and frozen(v)['body']['scoring']['kill_score']==40
        scenario('lethal_impact_ends_world_and_preserves_partial_score_assets',lethal,mode='lethal')
        def multiple(host,page,context):
            reach_boss(page,host);wait(lambda:len(warnings(current(page,host)))==2);page.reload();active(page);v=current(page,host);assert not warnings(v) and any(len(e['casts'])==2 and e['reason']=='background' for e in events(v,'area_cancelled'));close(page,host)
        scenario('multiple_independent_circles_cancel_in_one_bounded_event',multiple,mode='two')
        def mobile(host,page,context):
            a=setup(page,host);page.wait_for_function("document.querySelector('#arena').dataset.areaWarnings==='1'&&document.querySelector('#area-status').textContent.includes('整体离开预警圈')");assert page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);navigate(page,host,[500,700]);assert not resolved(page,host,a)['hit'];close(page,host)
        scenario('mobile_isometric_warning_countdown_and_real_public_dodge',mobile,mode='flat',mobile=True)
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context,point=point):
                auto(page);host.killed();page.goto('about:blank');state=host.state();request=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==127 and frozen(v)['body']==request['body'] and frozen(v)['operation_id']==request['operation_id']
            scenario('area_boss_assets_'+point,crash,point=point)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['cultivation']==127 and any(e['kind']=='io_timed_out' for e in v['events'])
        scenario('lost_receipt_queries_original_area_boss_assets_once',lost)
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=30000);before=current(page,host);request=frozen(before);old=before['configHash'];history=events(before,'area_started');host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-9.json';c=json.loads(path.read_text());c['pve']['additional_templates']['verification-phase-boss']['area'].update(radius=32,windup_ms=600,damage_bps=0,cooldown_ms=9000);c['cultivation_coefficient']=.5;path.write_text(json.dumps(c));host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000);v=current(page,host);assert v['configHash']==old and frozen(v)==request and events(v,'area_started')==history
            pin=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json();assert pin['pve']['additional_templates']['verification-phase-boss']['area']['radius']==96
            page.locator('#reconcile').click();v=done(page,host);assert v['ledger']['cultivation']==127;assets=v['ledger'];host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==assets and frozen(v)==request and events(v,'area_started')==history
        scenario('area_parameter_growth_update_keeps_original_manual_responsibility_and_no_skill_replay',version)
        def interrupted(host,page,context):
            setup(page,host);history=events(current(page,host),'area_started');host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);l=v['ledger'];assert l['warehouse']==100 and l['cultivation']==0 and l['compensations']==1 and l['settlements']==0 and not l['items'] and not l['pending'] and not v['settlement'] and events(v,'area_started')==history and not events(v,'area_resolved');assert page.locator('#area-status').inner_text()==''
            host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);assert v['ledger']==l and events(v,'area_started')==history and not events(v,'area_resolved')
        scenario('unfrozen_pending_circle_restart_only_refunds_entry_without_world_or_impact_replay',interrupted,mode='flat',unfrozen=True)
        browser.close()
    assert len(results)==25 and not errors and sources==hashes(),'page error or source change during verification';assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest()
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-9','mechanismVersion':8,'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests)}),flush=True)
if __name__=='__main__':main()
