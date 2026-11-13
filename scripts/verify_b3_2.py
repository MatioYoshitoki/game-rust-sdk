"""B3-2: ordinary pouch generation, ground loot, asset settlement and real restart."""
from contextlib import closing
import hashlib
import json
import re
import shutil
import sqlite3
import subprocess
import tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active, closed, info, wait
from verify_reconnect import hashes

ROOT=Path(__file__).resolve().parents[1]
OUTPUT=ROOT/'artifacts/b3-2'

def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_2.py','--check'],cwd=ROOT,check=True)
    rust=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,capture_output=True,text=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',rust.stdout,re.M)
    extra_config=['chest_profile_loads_alongside_unchanged_legacy_profiles',
        'rejects_duplicate_chest_ids_unknown_fields_and_unsupported_chest_mechanics',
        'chest_positions_reload_without_recompile_and_archived_version_remains_selectable']
    native=[name for name in all_tests if '::chests::' in name or name.rsplit('::',1)[-1] in extra_config]
    # Keep the original batch's acceptance set distinct from the nested B3-3 seal suite.
    native=[name for name in native if '::sealed_tests::' not in name]
    assert len(native)==23 and len(all_tests)>=146,(len(native),len(all_tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True)
    binary_hash=hashlib.sha256((ROOT/'target/debug/kernel-host').read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def automatic(page,fault='none'):
            page.locator('#scenario').select_option('b3-2');page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.request.method=='POST' and r.url.endswith('/api/sessions')):
                page.locator('#verify').click()
        def begin(page):
            page.locator('#scenario').select_option('b3-2');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        def current(page,host):return info(page,host.url)
        def frozen(value):return next(r for r in value['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):
            page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed' && document.querySelector('#pending').textContent==='0'",timeout=30000)
            return wait(lambda:v if (v:=current(page,host))['settlement'] and not v['ledger']['pending'] else None)
        def assets(value,growth=None):
            request=frozen(value);body=request['body'];ledger=value['ledger']
            assert ledger['warehouse']==95+body['kept'] and ledger['admissions']==ledger['settlements']==1,value
            assert ledger['items']=={item['item_id']:item['kept'] for item in body.get('items',[])},value
            assert body['carried']+body['looted']==body['kept']+body['dropped']
            for item in body.get('items',[]):assert item['looted']==item['kept']+item['dropped']
            assert not ledger['pending'] and ledger['manual']==0
            if growth is not None:assert ledger['cultivation']==growth,value
            assert sum(e['kind']=='settlement_applied' for e in value['events'])==1
        def openings(value):return [e for e in value['events'] if e['kind']=='chest_opened']
        def chest(value):return value['snapshot']['pve']['chests'][0]
        def approach(page,host):
            page.locator('#navigate-chest').click()
            page.wait_for_function("!document.querySelector('#open-chest').disabled",timeout=15000)
        def open_first(page,host,key=False):
            approach(page,host)
            with page.expect_request(lambda r:r.url.endswith('/input') and r.method=='POST' and r.post_data_json.get('action',{}).get('kind')=='open_chest') as request:
                if key:page.locator('#arena').focus();page.keyboard.press('f')
                else:page.locator('#open-chest').click()
            wait(lambda:chest(current(page,host))['opened'])
            return request.value.post_data_json
        def reload(host,page):host.start();page.goto(host.url);closed(page);return current(page,host)
        def scenario(name,work,point='',mobile=False,custom=False):
            with tempfile.TemporaryDirectory(prefix='rust-pouches-') as directory:
                manifest=None
                if custom:
                    package=Path(directory)/'config';package.mkdir()
                    for alias in ['b1','b3-1','b3-2']:shutil.copyfile(ROOT/'config'/(alias+'.json'),package/(alias+'.json'))
                    manifest=package/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{a:a+'.json' for a in ['b1','b3-1','b3-2']},'archives':[]}))
                host=Host(directory,manifest)
                context=browser.new_context(viewport={'width':390,'height':844} if mobile else {'width':1440,'height':1100},has_touch=mobile,is_mobile=mobile)
                page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                try:
                    host.start(point,'commit_settlement' if point else '');page.goto(host.url);work(host,page,context)
                    value=current(page,host)
                    assert value['phase']=='closed' and not value['ledger']['pending'],value
                    assert page.locator('#world-live').inner_text()=='0' and not errors,errors
                    results[name]={'ledger':value['ledger'],'events':value['events'],'requests':value['io']['requests'],
                                   'configId':value['configId'],'configHash':value['configHash'],'snapshot':value['snapshot']}
                    print(name+': passed',flush=True)
                except Exception:
                    try:page.screenshot(path=str(OUTPUT/'failure.png'),full_page=True);(OUTPUT/'failure.json').write_text(json.dumps(current(page,host),ensure_ascii=False,indent=2))
                    except Exception:pass
                    host.error_log.seek(0);print(host.error_log.read(),flush=True);raise
                finally:context.close();host.stop();host.error_log.close()
        def extraction(host,page,context):
            automatic(page);value=done(page,host);assets(value,45)
            assert len(openings(value))==2 and all(c['opened'] and c['generation']==1 for c in value['snapshot']['pve']['chests'])
            assert not value['snapshot']['resources'] and value['snapshot']['pve']['kills']==3
            assert next(i for i in frozen(value)['body']['items'] if i['item_id']=='huo-ling')['looted']>=2
            page.wait_for_function("document.body.dataset.verification==='passed'",timeout=15000)
            page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True)
            original=value['ledger'];request=frozen(value)
            for _ in range(2):
                host.kill();page.goto('about:blank');value=reload(host,page)
                assert value['ledger']==original and frozen(value)==request;assets(value,45)
            page.screenshot(path=str(OUTPUT/'restored.png'),full_page=True)
        scenario('three_wolves_two_pouches_extraction_and_repeated_restart',extraction)
        def death(host,page,context):
            automatic(page)
            wait(lambda:(v:=current(page,host))['snapshot'] and all(c['opened'] for c in v['snapshot']['pve']['chests']) and not v['snapshot']['resources'] and v['snapshot']['pve']['kills']==3)
            page.locator('#close').click();value=done(page,host);assets(value,11)
            assert frozen(value)['body']['outcome']=='death' and len(openings(value))==2
        scenario('picked_chest_items_follow_death_conservation',death)
        def duplicate(host,page,context):
            begin(page);payload=open_first(page,host,key=True);before=current(page,host)
            for _ in range(2):
                response=page.request.post(host.url+f"/api/sessions/{before['id']}/input",data=payload);assert response.status==200,response.text()
            after=current(page,host);assert len(openings(after))==1 and after['snapshot']['resources']==before['snapshot']['resources']
            assert after['ledger']['warehouse']==95 and after['ledger']['settlements']==0
            for action in [dict(payload['action'],quantity=999),dict(payload['action'],expected_generation=0)]:
                bad=dict(payload,requestId='invalid',sequence=after['sequence']+1,action=action)
                response=page.request.post(host.url+f"/api/sessions/{before['id']}/input",data=bad)
                assert response.json()['error']=='INVALID_MESSAGE',response.text()
            another=dict(payload,requestId='another',sequence=after['sequence']+1)
            response=page.request.post(host.url+f"/api/sessions/{before['id']}/input",data=another)
            assert response.json()['error']=='NO_RESOURCE',response.text()
            assert len(openings(current(page,host)))==1
            page.locator('#close').click();assets(done(page,host),0)
        scenario('keyboard_open_and_duplicate_public_inputs_pay_once',duplicate)
        def refresh(host,page,context):
            begin(page);open_first(page,host);before=current(page,host);page.reload();active(page);after=current(page,host)
            assert before['id']==after['id'] and after['connection']['generation']>before['connection']['generation']
            assert chest(after)==chest(before) and after['snapshot']['resources']==before['snapshot']['resources']
            old={'connection':before['connection'],'requestId':'old-owner','sequence':after['sequence']+1,
                 'action':{'kind':'open_chest','chest_id':chest(before)['id'],'expected_generation':1}}
            response=page.request.post(host.url+f"/api/sessions/{before['id']}/input",data=old)
            assert response.status==409 and response.json()['error']=='STALE_CONNECTION'
            assert len(openings(current(page,host)))==1
            page.screenshot(path=str(OUTPUT/'opened.png'),full_page=True)
            page.locator('#close').click();assets(done(page,host),0)
        scenario('refresh_preserves_chest_generation_ground_loot_and_connection_fence',refresh)
        def respawn(host,page,context):
            begin(page);open_first(page,host);first=current(page,host);old_ids=set(openings(first)[0]['resource_ids'])
            value=wait(lambda:v if (v:=current(page,host))['snapshot']['pve']['chests'][0]['generation']==2 else None,timeout=20)
            assert not chest(value)['opened'] and len(openings(value))==1
            assert value['snapshot']['logical_ms']>=chest(first)['respawn_at_ms']
            old={'connection':value['connection'],'requestId':'old-chest-cycle','sequence':value['sequence']+1,
                'action':{'kind':'open_chest','chest_id':chest(value)['id'],'expected_generation':1}}
            response=page.request.post(host.url+f"/api/sessions/{value['id']}/input",data=old)
            assert response.json()['error']=='REVISION_CONFLICT'
            page.reload();active(page);page.locator('#open-chest').click()
            value=wait(lambda:v if len(openings(v:=current(page,host)))==2 else None)
            assert chest(value)['generation']==2 and chest(value)['opened']
            assert old_ids.isdisjoint(openings(value)[-1]['resource_ids'])
            page.locator('#close').click();assets(done(page,host),0)
        scenario('real_clock_respawn_and_stale_generation_rejection',respawn)
        def mobile(host,page,context):
            begin(page);open_first(page,host);value=current(page,host)
            assert chest(value)['opened'] and len(openings(value))==1
            assert page.evaluate('document.documentElement.scrollWidth<=window.innerWidth'), 'mobile overflow'
            page.screenshot(path=str(OUTPUT/'mobile-open.png'),full_page=True)
            page.locator('#close').click();assets(done(page,host),0)
        scenario('mobile_pouch_controls',mobile,mobile=True)
        def lost(host,page,context):
            automatic(page,'settlement_lost_receipt');value=done(page,host);assets(value,45)
            assert len(openings(value))==2 and value['ledger']['reconciliations']==0
            assert any(event['kind']=='io_timed_out' for event in value['events'])
        scenario('lost_settlement_receipt_queries_chest_assets_without_reissuing',lost)
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context,point=point):
                automatic(page);host.killed();page.goto('about:blank');state=host.state()
                original=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement')
                value=reload(host,page);assets(value,45)
                assert frozen(value)['operation_id']==original['operation_id'] and frozen(value)['body']==original['body']
                assert state['settlements']==int(point=='after_storage_commit')
            scenario('frozen_chest_assets_'+point,crash,point)
        def manual(host,page,context):
            automatic(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=20000)
            before=current(page,host);request=frozen(before);host.kill();page.goto('about:blank');host.start();page.goto(host.url)
            page.locator('#reconcile').wait_for(state='visible',timeout=15000);value=current(page,host)
            assert value['ledger']['manual']==1 and value['ledger']['warehouse']==95 and frozen(value)==request
            assert len(openings(value))==2 and page.locator('#world-live').inner_text()=='0'
            page.locator('#reconcile').click();assets(done(page,host),45)
            host.kill();page.goto('about:blank');assets(reload(host,page),45)
        scenario('manual_chest_obligation_survives_restart_and_commits_once',manual)
        def interrupted(host,page,context):
            begin(page);open_first(page,host);page.locator('#pickup').click()
            wait(lambda:current(page,host)['snapshot']['inventory_revision']>1)
            host.kill();page.goto('about:blank');value=reload(host,page)
            assert value['ledger']['warehouse']==100 and value['ledger']['items']=={} and value['ledger']['cultivation']==0
            assert value['ledger']['compensations']==1 and value['ledger']['settlements']==0
            assert not any(e['kind']=='settlement_applied' for e in value['events'])
        scenario('unfrozen_chest_pickup_interrupt_refunds_entry_without_minting_loot',interrupted)
        def version(host,page,context):
            automatic(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=20000)
            before=current(page,host);request=frozen(before);old_hash=before['configHash'];host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-2.json';config=json.loads(path.read_text());config['pve']['chest_spawns'][0]['position']=config['spawn'];path.write_text(json.dumps(config))
            host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000)
            value=current(page,host);active_config=page.request.get(host.url+'/api/config?scenario=b3-2').json()
            pinned=page.request.get(host.url+f"/api/config?sessionId={value['id']}").json()
            assert active_config['configHash']!=old_hash and value['configHash']==pinned['configHash']==old_hash
            assert pinned['pve']['chest_spawns'][0]['position']!=active_config['pve']['chest_spawns'][0]['position']
            assert page.locator('#arena').get_attribute('data-config-hash')==old_hash and frozen(value)==request
            page.locator('#reconcile').click();assets(done(page,host),45)
            page.locator('#start').click();active(page);value=current(page,host)
            assert value['configHash']==active_config['configHash'] and chest(value)['position']==config['spawn']
            page.locator('#close').click();assets(done(page,host),0)
        scenario('map_update_keeps_old_chest_result_and_assets_pinned',version,custom=True)
        def legacy(host,page,context):
            page.locator('#clock').select_option('fast');page.locator('#start').click();active(page);value=current(page,host)
            assert page.locator('#open-chest').is_hidden()
            for attempt in range(3):
                value=current(page,host)
                response=page.request.post(host.url+f"/api/sessions/{value['id']}/input",data={'connection':value['connection'],
                    'requestId':f'unsupported-chest-{attempt}','sequence':value['sequence']+1,'action':{'kind':'open_chest','chest_id':'forest-pouch-1','expected_generation':1}})
                if response.json().get('error')!='STALE_SEQUENCE':break
            assert response.json()['error']=='UNSUPPORTED',response.text()
            page.locator('#close').click();closed(page)
        scenario('legacy_scene_explicitly_rejects_chest_action',legacy)
        browser.close()
    assert sources==hashes(),'sources changed during B3-2 verification'
    assert hashlib.sha256((ROOT/'target/debug/kernel-host').read_bytes()).hexdigest()==binary_hash
    assert not errors,errors
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-2','rustTests':native,'rustTotal':len(all_tests),
        'results':results,'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary_hash},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'newRustTests':len(native),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests)}),flush=True)

if __name__=='__main__':main()
