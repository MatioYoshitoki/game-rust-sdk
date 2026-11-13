"""B3-5 public HTTP/SSE acceptance with real Rust clocks and SQLite recovery."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-5';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir()
    aliases=['b1','b3-1','b3-2','b3-3','b3-4','b3-5']
    for alias in aliases:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-5.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[200,400],extraction_point=[900,800],geometry={'min':[0,0],'max':[1024,1024],'walls':[]},extraction_unlock_ms=12000)
        c['params']['runtime']['publish_batched_ms']=900
        p=c['pve'];p.update(map_id='encounter-test-flat',width=32,height=32,cell_size=32,cells_rle=[[0,1024]])
        p['stats'].update(hp=60,spirit=0,atk=0,spd=0,crit=0,dodge=0);p['combat']['cast_chance']=0
        e=p['encounters'][0];e['trigger']={'center':[500,400],'radius':100}
        for i,m in enumerate(e['monsters']):m['position']=[600+i*100,400]
        for i,chest in enumerate(p['chest_spawns']):chest['position']=[900,400+i*250]
        if mode=='blocked':c['spawn']=[600,400]
        if mode=='two':
            second=copy.deepcopy(e);second['id']='second-clearing';second['name']='第二遭遇';second['trigger']['center']=[500,650];second['trigger']['radius']=80
            for m in second['monsters']:m['id']='second-'+m['id'];m['position'][1]=650
            second['required_monster_ids']=[m['id'] for m in second['monsters']];second['reward_chest_ids']=['encounter-cache-2'];e['reward_chest_ids']=['encounter-cache-1'];p['encounters'].append(second)
        if mode=='free':
            t={'mechanism':'melee_projectile_heal','name':'无计分目标','stats':copy.deepcopy(p['stats']),'combat':copy.deepcopy(p['combat']),'drop_table':{'guaranteed':[{'itemId':'amber','quantityMin':2,'quantityMax':2}],'random':[]},'leash':p['leash'],'help_range':p['help_range'],'memory_range':p['memory_range'],'kill_credit':False,'drop_orb':False}
            p['additional_templates']={'free-target':t};p['items']['amber']={'name':'配置晶石','stackLimit':20}
            for m in e['monsters']:m['template_id']='free-target'
        if mode=='respawn':p['treasure_rules']['template']['respawn_delay_ms']=3000;p['treasure_rules']['template']['seal'].update(layers=1,layer_duration_ms=9000)
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{a:a+'.json' for a in aliases},'archives':[]}));return manifest

def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_5.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,text=True,capture_output=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M)
    tests=[t for t in all_tests if '::encounter_tests::' in t or t.startswith('verification::encounters::') or t.startswith('config::catalog::tests::encounter_')]
    assert len(all_tests)>=247 and len(tests)==31,(len(all_tests),len(tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def encounter(v,n=0):return v['snapshot']['pve']['encounters'][n]
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
            page.locator('#scenario').select_option('b3-5');page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.url.endswith('/api/sessions') and r.request.method=='POST'):page.locator('#verify').click()
        def begin(page):page.locator('#scenario').select_option('b3-5');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        serial=0
        def send(page,host,action):
            nonlocal serial
            for _ in range(3):
                serial+=1;v=current(page,host)
                response=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'encounter-input-{serial}','action':action})
                if response.ok or response.json().get('error')!='STALE_SEQUENCE':return response
            raise AssertionError(response.text())
        def trigger(page,host,n=0):
            center=[500,400] if n==0 else [500,650]
            assert send(page,host,{'kind':'navigate','destination':center}).ok
            wait(lambda:encounter(current(page,host),n)['phase']=='active');assert send(page,host,{'kind':'stop'}).ok
        def kill_one(page,host,n=0):
            v=current(page,host);ids=encounter(v,n)['required_monster_ids'];m=next(m for m in v['snapshot']['pve']['monsters'] if m['id'] in ids and m['hp']>0);target=m['id']
            assert send(page,host,{'kind':'navigate','destination':m['position']}).ok
            wait(lambda:(v:=current(page,host)) and sum((a-b)**2 for a,b in zip(v['snapshot']['position'],m['position']))<65**2)
            assert send(page,host,{'kind':'stop'}).ok
            wait(lambda:(v:=current(page,host)) and v['snapshot']['logical_ms']>=v['snapshot']['basic_ready_at_ms'])
            assert send(page,host,{'kind':'attack'}).ok
            return wait(lambda:v if (v:=current(page,host)) and next(m for m in v['snapshot']['pve']['monsters'] if m['id']==target)['hp']==0 else None)
        def clear(page,host,n=0):
            for _ in range(3):kill_one(page,host,n)
            return wait(lambda:v if encounter(v:=current(page,host),n)['phase']=='completed' else None)
        def opening(g=1,chest=1):return {'kind':'open_chest','chest_id':f'encounter-cache-{chest}','expected_generation':g}
        def scenario(name,work,mode='source',point='',mobile=False):
            with tempfile.TemporaryDirectory(prefix='encounter-') as directory:
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
            auto(page);v=done(page,host);assert v['ledger']['cultivation']==45 and v['snapshot']['pve']['kills']==3 and encounter(v)['phase']=='completed'
            assert sum(e['kind']=='encounter_triggered' for e in v['events'])==sum(e['kind']=='encounter_started' for e in v['events'])==sum(e['kind']=='encounter_completed' for e in v['events'])==1
            assert sum(e['kind']=='chest_opened' for e in v['events'])==2
            page.wait_for_function("document.body.dataset.verification==='passed'");page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True)
            before=v['ledger'];request=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
        scenario('area_trigger_targets_reward_pickup_extraction_and_repeated_restart',extraction)
        def death(host,page,context):
            auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and encounter(v)['phase']=='completed' and all(c['opened'] for c in v['snapshot']['pve']['chests']) and not v['snapshot']['resources'])
            page.locator('#close').click();v=done(page,host);assert v['ledger']['cultivation']==11 and frozen(v)['body']['outcome']=='death'
        scenario('completed_encounter_picked_assets_death_conserve_each_item',death)
        def dormant(host,page,context):
            begin(page);v=current(page,host);assert encounter(v)['phase']=='dormant' and not v['snapshot']['pve']['monsters'] and not v['snapshot']['target']
            assert send(page,host,opening()).json()['error']=='ENCOUNTER_LOCKED'
            action=opening();action['encounter_completed']=True;assert send(page,host,action).json()['error']=='INVALID_MESSAGE'
            page.screenshot(path=str(OUTPUT/'dormant.png'),full_page=True);page.locator('#close').click();v=done(page,host);assert v['ledger']['cultivation']==0 and not v['ledger']['items']
        scenario('dormant_has_no_enemies_and_public_reward_cannot_forge_completion',dormant,mode='flat')
        def blocked(host,page,context):
            begin(page);v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='spawning' else None);assert encounter(v)['monster_ids']==[]
            assert send(page,host,opening()).json()['error']=='ENCOUNTER_LOCKED'
            page.reload();active(page);assert encounter(current(page,host))['phase']=='spawning'
            assert send(page,host,{'kind':'navigate','destination':[200,700]}).ok
            v=wait(lambda:v if encounter(v:=current(page,host))['phase']=='active' else None);assert len(encounter(v)['monster_ids'])==3
            assert sum(e['kind']=='encounter_spawn_blocked' for e in v['events'])==sum(e['kind']=='encounter_started' for e in v['events'])==sum(e['kind']=='encounter_triggered' for e in v['events'])==1
            page.locator('#close').click();done(page,host)
        scenario('blocked_group_retains_trigger_through_refresh_and_spawns_atomically_after_leaving',blocked,mode='blocked')
        def partial(host,page,context):
            begin(page);trigger(page,host);v=kill_one(page,host);assert encounter(v)['defeated_required']==1 and encounter(v)['phase']=='active'
            assert send(page,host,opening()).json()['error']=='ENCOUNTER_LOCKED'
            page.locator('#close').click();v=done(page,host);assert v['ledger']['cultivation']==3 and not any(e['kind']=='chest_opened' for e in v['events'])
        scenario('partial_kills_keep_reward_locked_and_early_death_is_valid',partial,mode='flat')
        def independent(host,page,context):
            begin(page);trigger(page,host);v=clear(page,host);assert encounter(v,1)['phase']=='dormant' and not v['snapshot']['pve']['chests'][1]['encounter']['completed']
            assert send(page,host,opening(chest=2)).json()['error']=='ENCOUNTER_LOCKED';assert send(page,host,{'kind':'navigate','destination':[900,400]}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],[900,400]))<50**2);assert send(page,host,{'kind':'stop'}).ok;assert send(page,host,opening()).ok
            page.locator('#close').click();done(page,host)
        scenario('two_encounters_only_release_their_own_reward_containers',independent,mode='two')
        def free(host,page,context):
            auto(page);v=done(page,host);assert encounter(v)['phase']=='completed' and v['snapshot']['pve']['kills']==0 and v['ledger']['cultivation']==22 and v['ledger']['items']['amber']==6
        scenario('noncredit_targets_complete_and_their_loot_settles_with_zero_kills',free,mode='free')
        def mobile(host,page,context):
            begin(page);page.locator('#navigate-encounter').click();wait(lambda:encounter(current(page,host))['phase']=='active');page.wait_for_function("document.querySelector('#encounter-status').textContent.includes('战斗中')")
            assert page.locator('#open-chest').is_disabled() and page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);page.locator('#close').click();done(page,host)
        scenario('mobile_area_navigation_and_authoritative_target_progress',mobile,mode='flat',mobile=True)
        def refresh(host,page,context):
            begin(page);trigger(page,host);v=kill_one(page,host);before=encounter(v);old=v['connection'];page.reload();active(page);v=current(page,host);assert encounter(v)==before
            r=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':old,'sequence':v['sequence']+1,'requestId':'old-encounter-open','action':opening()});assert r.json()['error']=='STALE_CONNECTION'
            assert sum(e['kind']=='encounter_started' for e in v['events'])==1;page.locator('#close').click();done(page,host)
        scenario('active_partial_progress_and_entity_ids_survive_refresh_old_connection_fenced',refresh,mode='flat')
        def early_exit(host,page,context):
            begin(page);assert send(page,host,{'kind':'navigate','destination':[900,800]}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],[900,800]))<60**2);assert send(page,host,{'kind':'stop'}).ok
            wait(lambda:current(page,host)['snapshot']['extraction_open']);assert send(page,host,{'kind':'extract_start'}).ok
            v=done(page,host);assert encounter(v)['phase']=='dormant' and v['ledger']['cultivation']==22 and not v['ledger']['items'] and v['ledger']['warehouse']==100
        scenario('extraction_before_trigger_does_not_grant_locked_rewards',early_exit,mode='flat')
        def respawn(host,page,context):
            begin(page);trigger(page,host);clear(page,host);assert send(page,host,{'kind':'navigate','destination':[900,400]}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],[900,400]))<50**2);assert send(page,host,{'kind':'stop'}).ok
            assert send(page,host,opening()).ok;wait(lambda:current(page,host)['snapshot']['pve']['chests'][0]['seal']['completed_layers']==1);assert send(page,host,opening()).ok
            v=wait(lambda:v if (v:=current(page,host))['snapshot']['pve']['chests'][0]['generation']==2 else None)
            assert encounter(v)['phase']=='completed' and v['snapshot']['pve']['chests'][0]['encounter']['completed'] and sum(e['kind']=='encounter_started' for e in v['events'])==1
            assert send(page,host,opening()).json()['error']=='REVISION_CONFLICT';page.locator('#close').click();done(page,host)
        scenario('container_respawn_keeps_one_shot_encounter_complete_without_new_enemies',respawn,mode='respawn')
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context,point=point):
                auto(page);host.killed();page.goto('about:blank');state=host.state();request=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==45 and frozen(v)['body']==request['body'] and frozen(v)['operation_id']==request['operation_id']
            scenario('encounter_assets_'+point,crash,point=point)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['cultivation']==45 and any(e['kind']=='io_timed_out' for e in v['events'])
        scenario('lost_receipt_queries_original_encounter_assets_once',lost)
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=30000);before=current(page,host);request=frozen(before);old=before['configHash'];host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-5.json';c=json.loads(path.read_text());c['pve']['encounters'][0]['required_monster_ids']=c['pve']['encounters'][0]['required_monster_ids'][:1];c['pve']['encounters'][0]['trigger']['radius']=10;c['cultivation_coefficient']=.5;path.write_text(json.dumps(c))
            host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000);v=current(page,host);assert v['configHash']==old and frozen(v)==request
            pin=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json();assert len(pin['pve']['encounters'][0]['required_monster_ids'])==3 and pin['pve']['encounters'][0]['trigger']['radius']==96
            page.locator('#reconcile').click();v=done(page,host);assert v['ledger']['cultivation']==45;assets=v['ledger'];host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==assets and frozen(v)==request
        scenario('encounter_policy_update_preserves_old_manual_assets_and_profile',version)
        with tempfile.TemporaryDirectory(prefix='encounter-unfrozen-') as directory:
            host=Host(directory,package(directory,'flat'));context=browser.new_context();page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
            try:
                host.start();page.goto(host.url);begin(page);trigger(page,host);host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);l=v['ledger'];assert l['warehouse']==100 and l['compensations']==1 and l['settlements']==0 and not l['items'] and not l['pending'] and not v['settlement'];results['unfrozen_encounter_restart_refunds_entry_without_recreating_world']={'ledger':l,'events':v['events'],'configHash':v['configHash']};print('unfrozen_encounter_restart_refunds_entry_without_recreating_world: passed',flush=True)
            finally:context.close();host.stop();host.error_log.close()
        browser.close()
    assert not errors and sources==hashes(),'page error or source change during verification';assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest()
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-5','mechanismVersion':4,'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests)}),flush=True)
if __name__=='__main__':main()
