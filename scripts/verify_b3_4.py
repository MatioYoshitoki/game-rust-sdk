"""B3-4: mixed monsters, per-generation guarded eligibility, and durable assets."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-4';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir()
    for alias in ['b1','b3-1','b3-2','b3-3','b3-4']:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-4.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[400,400],extraction_point=[900,700],geometry={'min':[0,0],'max':[1024,1024],'walls':[]},extraction_unlock_ms=12000)
        c['params']['runtime']['publish_batched_ms']=900
        p=c['pve'];p.update(map_id='guarded-test-flat',width=32,height=32,cell_size=32,cells_rle=[[0,1024]],monster_positions=[])
        for i,chest in enumerate(p['chest_spawns']):chest['position']=[600+i*200,400]
        g=p['treasure_rules']['template']['guardian'];g.update(chance=1,min_count=1,max_count=1,spawn_offsets=[[64,0],[-64,0],[0,64],[0,-64]])
        t=p['additional_templates']['verification-guardian'];t['stats'].update(hp=60,spirit=0,atk=0,spd=0,crit=0,dodge=0);t['combat']['cast_chance']=0
        if mode=='blocked':
            c['spawn']=[664,400];p['chest_spawns']=p['chest_spawns'][:1];g.update(min_count=2,max_count=2,spawn_offsets=[[64,0],[-64,0]])
        if mode=='respawn':
            p['chest_spawns']=p['chest_spawns'][:1];p['treasure_rules']['template']['seal'].update(layers=1,layer_duration_ms=9000);p['treasure_rules']['template']['respawn_delay_ms']=3000
        if mode=='zero':g['chance']=0
        if mode=='two':g.update(min_count=2,max_count=2)
        if mode=='extra':
            g['chance']=0;p['items']['amber']={'name':'配置晶石','stackLimit':3};extra=copy.deepcopy(t);extra['name']='静态配置怪';extra['stats']['hp']=90;extra['combat']['radius']=20;extra['kill_credit']=False;extra['drop_table']['guaranteed']=[{'itemId':'amber','quantityMin':7,'quantityMax':7}];p['additional_templates']['extra-profile']=extra;p['extra_spawns']=[{'id':'extra-one','template_id':'extra-profile','position':[460,400]}]
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{a:a+'.json' for a in ['b1','b3-1','b3-2','b3-3','b3-4']},'archives':[]}));return manifest
def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_4.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,capture_output=True,text=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M)
    tests=[t for t in all_tests if '::guardian_tests::' in t or t.startswith('verification::guardians::') or t.startswith('config::catalog::tests::guarded_')]
    assert len(tests)==30 and len(all_tests)>=216,(len(tests),len(all_tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def chest(v,n=0):return v['snapshot']['pve']['chests'][n]
        def frozen(v):return next(r for r in v['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):
            page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed'&&document.querySelector('#pending').textContent==='0'",timeout=30000)
            return wait(lambda:v if (v:=current(page,host)) and v['settlement'] and not v['ledger']['pending'] else None)
        def check(v):
            b=frozen(v)['body'];l=v['ledger'];assert b['carried']+b['looted']==b['kept']+b['dropped']
            assert l['warehouse']==100-b['carried']+b['kept'] and l['cultivation']==b['cultivation']
            assert l['items']=={i['item_id']:i['kept'] for i in b.get('items',[])}
            assert all(i['looted']==i['kept']+i['dropped'] for i in b.get('items',[]))
            assert l['admissions']==l['settlements']==1 and not l['pending'] and l['manual']==0 and v['phase']=='closed'
            assert sum(e['kind']=='settlement_applied' for e in v['events'])==1
        def auto(page,fault='none'):
            page.locator('#scenario').select_option('b3-4');page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.url.endswith('/api/sessions') and r.request.method=='POST'):page.locator('#verify').click()
        def begin(page):page.locator('#scenario').select_option('b3-4');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        serial=0
        def send(page,host,action):
            nonlocal serial
            for _ in range(3):
                serial+=1;v=current(page,host)
                response=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'guard-public-{serial}','action':action})
                if response.ok or response.json().get('error')!='STALE_SEQUENCE':return response
            raise AssertionError(response.text())
        def enter_box(page,host):
            assert send(page,host,{'kind':'navigate','destination':[600,400]}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],[600,400]))<30**2)
            assert send(page,host,{'kind':'stop'}).ok
        def opening(g=1):return {'kind':'open_chest','chest_id':'guarded-cache-1','expected_generation':g}
        def scenario(name,work,mode='source',point='',mobile=False):
            with tempfile.TemporaryDirectory(prefix='guarded-') as directory:
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
            auto(page);v=done(page,host);assert v['ledger']['cultivation']==60 and v['snapshot']['pve']['kills']==5
            guards=[m for m in v['snapshot']['pve']['monsters'] if m.get('guardian')];assert len(guards)==2 and all(m['hp']==0 and m['max_hp']==225 for m in guards)
            assert sum(e['kind']=='guardians_ready' for e in v['events'])==2 and sum(e['kind']=='chest_opened' for e in v['events'])==2
            page.wait_for_function("document.body.dataset.verification==='passed'");page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True)
            before=v['ledger'];request=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
        scenario('mixed_wolves_two_guardians_full_extraction_and_restart',extraction)
        def death(host,page,context):
            auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and v['snapshot']['pve']['kills']==5 and all(c['opened'] for c in v['snapshot']['pve']['chests']) and not v['snapshot']['resources'])
            page.locator('#close').click();v=done(page,host);assert v['ledger']['cultivation']==18 and frozen(v)['body']['outcome']=='death'
        scenario('guard_kills_and_picked_items_death_conserve_assets',death)
        def gate(host,page,context):
            begin(page);enter_box(page,host);v=current(page,host);assert chest(v)['guardians']['alive']==1 and page.locator('#open-chest').is_disabled()
            r=send(page,host,opening());assert r.json()['error']=='GUARDIAN_ALIVE'
            v=current(page,host);assert not v['snapshot']['resources'] and chest(v)['seal']['completed_layers']==0
            page.screenshot(path=str(OUTPUT/'guarded.png'),full_page=True);page.locator('#close').click();done(page,host)
        scenario('alive_guard_blocks_public_interaction_and_ui',gate,mode='flat')
        def binding(host,page,context):
            begin(page);enter_box(page,host);assert send(page,host,{'kind':'attack'}).ok;v=wait(lambda:v if chest(v:=current(page,host))['guardians']['alive']==0 else None)
            assert chest(v,1)['guardians']['alive']==1
            assert send(page,host,opening()).ok
            r=send(page,host,{'kind':'open_chest','chest_id':'guarded-cache-2','expected_generation':1});assert r.json()['error']=='GUARDIAN_ALIVE'
            assert current(page,host)['snapshot']['pve']['kills']==1;page.locator('#close').click();done(page,host)
        scenario('actual_guard_death_only_releases_its_bound_container',binding,mode='flat')
        def blocked(host,page,context):
            begin(page);v=wait(lambda:v if (v:=current(page,host)) and any(e['kind']=='guardian_spawn_blocked' for e in v['events']) else None)
            assert chest(v)['guardians']=={'pending':True,'planned_count':2,'alive':0,'monster_ids':[]}
            assert send(page,host,opening()).json()['error']=='GUARDIAN_PENDING'
            page.reload();active(page);v=current(page,host);assert chest(v)['guardians']['planned_count']==2 and chest(v)['guardians']['pending']
            assert send(page,host,{'kind':'navigate','destination':[900,700]}).ok
            v=wait(lambda:v if not chest(v:=current(page,host))['guardians']['pending'] else None)
            assert chest(v)['guardians']['alive']==2 and sum(e['kind']=='guardian_spawn_blocked' for e in v['events'])==1
            assert sum(e['kind']=='guardians_ready' for e in v['events'])==1;page.locator('#close').click();done(page,host)
        scenario('blocked_plan_survives_refresh_and_recovers_without_partial_spawn',blocked,mode='blocked')
        def respawn(host,page,context):
            begin(page);enter_box(page,host);old=chest(current(page,host))['guardians']['monster_ids'][0]
            assert send(page,host,{'kind':'attack'}).ok;wait(lambda:chest(current(page,host))['guardians']['alive']==0)
            assert send(page,host,opening()).ok;wait(lambda:chest(current(page,host))['seal']['completed_layers']==1)
            assert send(page,host,opening()).ok;v=wait(lambda:v if chest(v:=current(page,host))['generation']==2 and not chest(v)['guardians']['pending'] else None)
            assert chest(v)['guardians']['monster_ids'][0]!=old and chest(v)['guardians']['alive']==1 and chest(v)['seal']['completed_layers']==0
            assert send(page,host,opening(1)).json()['error']=='REVISION_CONFLICT'
            assert send(page,host,{'kind':'cancel_chest_unlock','chest_id':'guarded-cache-1','expected_generation':1}).json()['error']=='REVISION_CONFLICT'
            assert send(page,host,opening(2)).json()['error']=='GUARDIAN_ALIVE';page.locator('#close').click();done(page,host)
        scenario('respawn_rebinds_new_guard_and_old_generation_cannot_unlock',respawn,mode='respawn')
        for mode,growth,kills in [('zero',22,0),('two',52,4),('extra',22,0)]:
            def matrix(host,page,context,mode=mode,growth=growth,kills=kills):
                auto(page);v=done(page,host);assert v['ledger']['cultivation']==growth and v['snapshot']['pve']['kills']==kills
                if mode=='zero':assert not v['snapshot']['pve']['monsters']
                if mode=='two':assert len([m for m in v['snapshot']['pve']['monsters'] if m.get('guardian')])==4
                if mode=='extra':
                    m=next(m for m in v['snapshot']['pve']['monsters'] if m['id']=='extra-one');assert m['max_hp']==90 and m['radius']==20 and not m.get('guardian');assert v['ledger']['items']['amber']==7
            scenario('data_only_'+mode+'_probability_quantity_or_mixed_reward',matrix,mode=mode)
        def refresh(host,page,context):
            begin(page);v=current(page,host);before=v['snapshot']['pve']['chests'];old=v['connection'];page.reload();active(page);v=current(page,host);assert v['snapshot']['pve']['chests']==before
            r=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':old,'sequence':v['sequence']+1,'requestId':'stale-guard-open','action':opening()});assert r.json()['error']=='STALE_CONNECTION'
            assert sum(e['kind']=='guardians_ready' for e in v['events'])==2;page.locator('#close').click();done(page,host)
        scenario('refresh_preserves_guard_ids_and_previous_connection_is_fenced',refresh,mode='flat')
        def mobile(host,page,context):
            begin(page);enter_box(page,host);page.wait_for_function("document.querySelector('#guardian-status').textContent.includes('剩余守护')");assert page.locator('#open-chest').is_disabled() and page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);page.locator('#close').click();done(page,host)
        scenario('mobile_displays_bound_guards_and_eligibility_gate',mobile,mode='flat',mobile=True)
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context,point=point):
                auto(page);host.killed();page.goto('about:blank');state=host.state();req=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==60 and frozen(v)['body']==req['body'] and frozen(v)['operation_id']==req['operation_id']
            scenario('guarded_assets_'+point,crash,point=point)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['cultivation']==60 and any(e['kind']=='io_timed_out' for e in v['events'])
        scenario('lost_receipt_queries_frozen_guarded_assets_once',lost)
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=30000);before=current(page,host);req=frozen(before);old=before['configHash'];host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-4.json';c=json.loads(path.read_text());c['pve']['treasure_rules']['template']['guardian']['chance']=0;c['pve']['additional_templates']['verification-guardian']['kill_credit']=False;c['cultivation_coefficient']=.5;path.write_text(json.dumps(c))
            host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000);v=current(page,host);assert v['configHash']==old and frozen(v)==req
            pin=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json();assert pin['pve']['treasure_rules']['template']['guardian']['chance']==.25
            page.locator('#reconcile').click();v=done(page,host);assert v['ledger']['cultivation']==60;assets=v['ledger'];host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==assets and frozen(v)==req
        scenario('guardian_policy_update_keeps_old_manual_assets_and_profile',version)
        with tempfile.TemporaryDirectory(prefix='guarded-unfrozen-') as directory:
            host=Host(directory,package(directory,'flat'));context=browser.new_context();page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
            try:
                host.start();page.goto(host.url);begin(page);wait(lambda:any(e['kind']=='guardians_ready' for e in current(page,host)['events']));host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);l=v['ledger'];assert l['warehouse']==100 and l['compensations']==1 and l['settlements']==0 and not l['items'] and not l['pending'];assert not v['settlement'];results['unfrozen_guarded_restart_refunds_entry_without_recreating_world']={'ledger':l,'events':v['events'],'configHash':v['configHash']};print('unfrozen_guarded_restart_refunds_entry_without_recreating_world: passed',flush=True)
            finally:context.close();host.stop();host.error_log.close()
        browser.close()
    assert not errors and sources==hashes(),'page error or sources changed during B3-4 verification'
    assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest()
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-4','mechanismVersion':3,'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests)}),flush=True)
if __name__=='__main__':main()
