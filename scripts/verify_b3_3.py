"""B3-3 real public-input layered interactions, assets, and process recovery."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-3';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,flat=False,enemy=False,respawn=False):
    root=Path(directory)/'config';root.mkdir()
    for alias in ['b1','b3-1','b3-2','b3-3']:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    if flat:
        path=root/'b3-3.json';c=json.loads(path.read_text());c.update(spawn=[400,400],extraction_point=[700,400],geometry={'min':[0,0],'max':[1024,1024],'walls':[]})
        p=c['pve'];p.update(map_id='sealed-test-flat',width=16,height=16,cell_size=64,cells_rle=[[0,256]],monster_positions=[])
        p['chest_spawns']=p['chest_spawns'][:1];p['chest_spawns'][0]['position']=[400,400]
        seal=p['treasure_rules']['template']['seal'];seal.update(layers=3,layer_duration_ms=30000,interrupt_retention_ratio=.5)
        if enemy:
            p['monster_positions']=[[440,400]];p['stats'].update(atk=5,spd=0,crit=0);p['combat'].update(cast_chance=0,attack_interval_ms=6000)
        if respawn:
            seal.update(layers=1,layer_duration_ms=3000);p['treasure_rules']['template']['respawn_delay_ms']=3000
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{a:a+'.json' for a in ['b1','b3-1','b3-2','b3-3']},'archives':[]}));return manifest
def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_3.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,capture_output=True,text=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M)
    tests=[t for t in all_tests if '::sealed_tests::' in t or t.startswith('verification::sealed::') or t.startswith('config::catalog::tests::sealed_')]
    assert len(tests)==24 and len(all_tests)>=186,(len(tests),len(all_tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def chest(value):return value['snapshot']['pve']['chests'][0]
        def frozen(value):return next(r for r in value['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):
            page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed'&&document.querySelector('#pending').textContent==='0'",timeout=30000)
            return wait(lambda:v if (v:=current(page,host)) and v['settlement'] and not v['ledger']['pending'] else None)
        def check(v):
            b=frozen(v)['body'];l=v['ledger'];assert b['carried']+b['looted']==b['kept']+b['dropped']
            assert l['warehouse']==100-b['carried']+b['kept'] and l['cultivation']==b['cultivation']
            assert l['items']=={i['item_id']:i['kept'] for i in b.get('items',[])}
            assert all(i['looted']==i['kept']+i['dropped'] for i in b.get('items',[]))
            assert l['admissions']==l['settlements']==1 and not l['pending'] and l['manual']==0
            assert v['phase']=='closed' and sum(e['kind']=='settlement_applied' for e in v['events'])==1
        def auto(page,fault='none'):
            page.locator('#scenario').select_option('b3-3');page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.url.endswith('/api/sessions') and r.request.method=='POST'):page.locator('#verify').click()
        def begin(page):
            page.locator('#scenario').select_option('b3-3');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        def approach(page):
            page.locator('#navigate-chest').click();page.wait_for_function("!document.querySelector('#open-chest').disabled")
        def scenario(name,work,point='',flat=False,enemy=False,respawn=False,mobile=False):
            with tempfile.TemporaryDirectory(prefix='sealed-') as directory:
                manifest=package(directory,flat,enemy,respawn);host=Host(directory,manifest)
                context=browser.new_context(viewport={'width':390,'height':844} if mobile else {'width':1440,'height':1100},has_touch=mobile,is_mobile=mobile)
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
            auto(page);v=done(page,host);check(v);assert v['ledger']['cultivation']==45 and 4<=v['ledger']['items']['han-tan']<=6
            assert sum(e['kind']=='chest_layer_completed' for e in v['events'])==4
            assert sum(e['kind']=='chest_opened' for e in v['events'])==2
            page.wait_for_function("document.body.dataset.verification==='passed'");page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True)
            before=v['ledger'];request=frozen(v)
            for _ in range(2):
                host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
        scenario('layered_two_container_extraction_and_repeated_restart',extraction)
        def death(host,page,context):
            auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and v['snapshot']['pve']['kills']==3 and all(c['opened'] for c in v['snapshot']['pve']['chests']) and not v['snapshot']['resources'])
            page.locator('#close').click();v=done(page,host);assert v['ledger']['cultivation']==11 and frozen(v)['body']['items']
        scenario('picked_sealed_loot_death_conserves_each_item',death)
        def unlock_only(host,page,context):
            begin(page);approach(page);page.locator('#open-chest').click();wait(lambda:chest(current(page,host))['seal']['completed_layers']==2)
            v=current(page,host);assert not v['snapshot']['resources'] and v['ledger']['warehouse']==95 and not chest(v)['opened']
            page.locator('#close').click();v=done(page,host);assert not v['ledger']['items'] and v['ledger']['cultivation']==0
        scenario('unlock_only_never_mints_unclaimed_rewards',unlock_only)
        def retention(host,page,context):
            begin(page);page.locator('#open-chest').click();wait(lambda:(s:=chest(current(page,host))['seal'])['completed_layers']==1 and s['layer_progress_ms']>3000)
            page.locator('#cancel-chest').click();v=wait(lambda:v if not (s:=chest(v:=current(page,host))['seal'])['unlocking'] else None);s=chest(v)['seal'];assert s['completed_layers']==1 and 0<s['layer_progress_ms']<30000 and s['interruption_reason']=='cancelled'
            page.locator('#open-chest').click();wait(lambda:chest(current(page,host))['seal']['unlocking']);before=current(page,host);page.reload();active(page);after=current(page,host)
            assert after['connection']['generation']>before['connection']['generation'];s=chest(after)['seal'];assert s['completed_layers']==1 and not s['unlocking'] and s['interruption_reason']=='background'
            page.wait_for_function("document.querySelector('#seal-reason').textContent.includes('后台')");page.screenshot(path=str(OUTPUT/'interrupted.png'),full_page=True)
            stale=page.request.post(host.url+f"/api/sessions/{after['id']}/input",data={'connection':before['connection'],'sequence':after['sequence']+1,'requestId':'stale-cancel','action':{'kind':'cancel_chest_unlock','chest_id':chest(after)['id'],'expected_generation':1}});assert stale.json()['error']=='STALE_CONNECTION'
            page.locator('#close').click();done(page,host)
        scenario('partial_retention_manual_cancel_refresh_and_old_connection_fence',retention,flat=True)
        def damage(host,page,context):
            begin(page);page.locator('#open-chest').click();v=wait(lambda:v if chest(v:=current(page,host))['seal']['interruption_reason']=='damage' else None)
            assert not chest(v)['seal']['unlocking'] and any(e['kind']=='damage' and e['amount']>0 for e in v['events'])
            page.wait_for_function("document.querySelector('#seal-reason').textContent.includes('受到伤害')");page.locator('#close').click();done(page,host)
        scenario('real_monster_damage_interrupts_authoritative_progress',damage,flat=True,enemy=True)
        def duplicate(host,page,context):
            begin(page)
            with page.expect_request(lambda r:r.url.endswith('/input') and r.method=='POST' and r.post_data_json.get('action',{}).get('kind')=='open_chest') as request:page.locator('#open-chest').click()
            payload=request.value.post_data_json;v=current(page,host)
            for _ in range(2):assert page.request.post(host.url+f"/api/sessions/{v['id']}/input",data=payload).status==200
            assert sum(e['kind']=='chest_unlock_started' for e in current(page,host)['events'])==1
            bad=copy.deepcopy(payload);bad['action']['progress_ms']=999999
            assert page.request.post(host.url+f"/api/sessions/{v['id']}/input",data=bad).json()['error']=='INVALID_MESSAGE'
            page.locator('#arena').focus();page.keyboard.press('Escape');wait(lambda:not chest(current(page,host))['seal']['unlocking']);page.locator('#close').click();done(page,host)
        scenario('duplicate_public_start_and_keyboard_cancel_have_no_extra_effect',duplicate,flat=True)
        def respawn(host,page,context):
            begin(page);page.locator('#open-chest').click();wait(lambda:chest(current(page,host))['seal']['completed_layers']==1);page.wait_for_function("!document.querySelector('#open-chest').disabled");page.locator('#open-chest').click();v=wait(lambda:v if chest(v:=current(page,host))['generation']==2 else None)
            s=chest(v)['seal'];assert s['completed_layers']==s['layer_progress_ms']==0 and not s['unlocking'];page.locator('#open-chest').click();wait(lambda:chest(current(page,host))['seal']['unlocking']);v=current(page,host)
            bad=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':'old-generation','action':{'kind':'cancel_chest_unlock','chest_id':chest(v)['id'],'expected_generation':1}});assert bad.json()['error']=='REVISION_CONFLICT'
            assert chest(current(page,host))['seal']['unlocking'];page.locator('#close').click();done(page,host)
        scenario('real_clock_respawn_resets_seals_and_rejects_stale_cancel',respawn,flat=True,respawn=True)
        def mobile(host,page,context):
            begin(page);approach(page);page.locator('#open-chest').click();wait(lambda:chest(current(page,host))['seal']['completed_layers']==2);page.wait_for_function("!document.querySelector('#open-chest').disabled");assert page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);page.locator('#close').click();done(page,host)
        scenario('mobile_displays_progress_and_unlock_action',mobile,mobile=True)
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context,point=point):
                auto(page);host.killed();page.goto('about:blank');state=host.state();request=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==45 and frozen(v)['body']==request['body'] and frozen(v)['operation_id']==request['operation_id']
            scenario('sealed_assets_'+point,crash,point=point)
        def lost(host,page,context):
            auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['cultivation']==45 and any(e['kind']=='io_timed_out' for e in v['events'])
        scenario('lost_settlement_receipt_queries_original_sealed_assets',lost)
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=30000);before=current(page,host);request=frozen(before);old=before['configHash'];host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-3.json';c=json.loads(path.read_text());c['pve']['treasure_rules']['template']['seal'].update(layers=3,layer_duration_ms=3000,interrupt_retention_ratio=1);c['cultivation_coefficient']=.5;path.write_text(json.dumps(c))
            host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000);v=current(page,host);assert v['configHash']==old and frozen(v)==request
            pinned=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json();assert pinned['pve']['treasure_rules']['template']['seal']['layers']==2
            page.locator('#reconcile').click();v=done(page,host);assert v['ledger']['cultivation']==45;assets=v['ledger'];host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==assets and frozen(v)==request
        scenario('new_seal_configuration_cannot_reinterpret_old_manual_responsibility',version)
        def interrupted_restart(host,page,context):
            begin(page);page.locator('#open-chest').click();wait(lambda:chest(current(page,host))['seal']['layer_progress_ms']>1000);host.kill();page.goto('about:blank');host.start();page.goto(host.url);closed(page);v=current(page,host);l=v['ledger'];assert l['warehouse']==100 and l['compensations']==1 and l['settlements']==0 and not l['items'];assert not v['settlement'];assert not l['pending'];assert v['phase']=='closed'
            # The refund has no settlement request; record this scenario separately.
            results['unfrozen_unlock_restart_refunds_only_entry']={'ledger':l,'events':v['events'],'configHash':v['configHash']};assert not errors
        with tempfile.TemporaryDirectory(prefix='sealed-interrupted-') as directory:
            host=Host(directory,package(directory,flat=True));context=browser.new_context();page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
            try:host.start();page.goto(host.url);interrupted_restart(host,page,context);print('unfrozen_unlock_restart_refunds_only_entry: passed',flush=True)
            finally:context.close();host.stop();host.error_log.close()
        browser.close()
    assert not errors and sources==hashes(),'page error or sources changed during B3-3 verification'
    assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest()
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-3','mechanismVersion':2,'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests)}),flush=True)
if __name__=='__main__':main()
