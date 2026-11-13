"""Forest objective chain, paid attunement and durable item conservation through public inputs."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-10';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir()
    for alias in ['b1','b3-10']:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-10.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[200,400],extraction_point=[900,800],geometry={'min':[0,0],'max':[1536,1536],'walls':[]},extraction_unlock_ms=20000)
        c['params']['runtime']['publish_batched_ms']=900;c['params']['regen'].update(near_ratio=0,far_ratio=0);c['params']['minimum_damage']=0
        p=c['pve'];p.update(map_id='objective-flat',width=48,height=48,cell_size=32,cells_rle=[[0,2304]])
        p['stats'].update(hp=60,atk=0,spd=0,crit=0,dodge=0);p['combat']['cast_chance']=0
        for t in p['additional_templates'].values():
            t['stats']=copy.deepcopy(p['stats']);t['combat']['cast_chance']=0;t.pop('area',None)
            for phase in t.get('boss',{}).get('phases',[]):phase['cast_chance']=0
        for i,e in enumerate(p['encounters']):
            e['trigger']={'center':[400+i*300,400],'radius':80}
            for n,m in enumerate(e.get('monsters',[])+[m for w in e.get('waves',[]) for m in w['monsters']]):m['position']=[400+i*300+n*60,500]
        p['chest_spawns'][0]['position']=[1200,400];p['map_objective']['attune_duration_ms']=30000
        if mode=='free':p['map_objective']['cost']['quantity']=0
        if mode=='root':p['map_objective']['cost']={'item_id':'xuan-tie','quantity':5}
        if mode=='shortage':p['map_objective']['cost']['quantity']=1000
        if mode=='ground-full':p['treasure_rules']['ground_capacity']=5
        if mode=='reordered':p['encounters'].reverse()
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{'b1':'b1.json','b3-10':'b3-10.json'},'archives':[]}));return manifest

def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_10.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,capture_output=True,text=True,check=True)
    all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M);tests=[t for t in all_tests if 'objective_' in t or '::objective::' in t]
    assert len(tests)==40 and len(all_tests)>=435,(len(all_tests),len(tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def obj(v):return v['snapshot']['pve']['map_objective']
        def frozen(v):return next(r for r in v['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):return wait(lambda:v if (v:=current(page,host)) and v['phase']=='closed' and v['settlement'] and not v['ledger']['pending'] else None,timeout=45)
        def events(v,kind):return [e for e in v['events'] if e['kind']==kind]
        def check(v):
            b=frozen(v)['body'];p=b['map_progress'];l=v['ledger'];consumed={r['item_id']:r['quantity'] for r in p['consumed']}
            assert b['carried']+b['looted']==b['kept']+b['dropped']+consumed.get(b['item_id'],0)
            assert all(i['looted']==i['kept']+i['dropped']+consumed.get(i['item_id'],0) for i in b.get('items',[]))
            assert l['warehouse']==100-b['carried']+b['kept'] and l['cultivation']==b['cultivation']
            assert l['items']=={i['item_id']:i['kept'] for i in b.get('items',[])} and l['admissions']==l['settlements']==1 and l['manual']==0
            assert v['settlement']['map_progress']==p and len(events(v,'settlement_applied'))==1
        serial=0
        def send(page,host,action):
            nonlocal serial
            for _ in range(3):
                v=current(page,host);serial+=1;response=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'objective-{serial}','action':action})
                if response.ok or response.json().get('error')!='STALE_SEQUENCE':return response
            raise AssertionError(response.text())
        def navigate(page,host,position):
            assert send(page,host,{'kind':'navigate','destination':position}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],position))<50**2);assert send(page,host,{'kind':'stop'}).ok
        def begin(page):page.locator('#scenario').select_option('b3-10');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        def pickup(page,host):
            for _ in range(80):
                v=current(page,host)
                if not v['snapshot']['resources']:return
                r=v['snapshot']['resources'][0];navigate(page,host,r['position']);v=current(page,host);assert send(page,host,{'kind':'pickup','resource_id':r['id'],'expected_revision':v['snapshot']['inventory_revision']}).ok
            raise AssertionError('pickup budget')
        def reach(page,host):
            begin(page)
            for index,center in enumerate([[400,400],[700,400],[1000,400]]):
                navigate(page,host,center)
                for _ in range(70):
                    v=current(page,host);e=v['snapshot']['pve']['encounters'][index]
                    if e['phase']=='completed':break
                    if e['phase'] in ['waiting','spawning']:wait(lambda:current(page,host)['snapshot']['pve']['encounters'][index]['phase'] not in ['waiting','spawning']);continue
                    target=v['snapshot']['target'];assert target
                    navigate(page,host,target['position']);wait(lambda:(v:=current(page,host)) and v['snapshot']['logical_ms']>=v['snapshot']['basic_ready_at_ms'])
                    old=next(m['hp'] for m in current(page,host)['snapshot']['pve']['monsters'] if m['id']==target['id']);assert send(page,host,{'kind':'attack'}).ok
                    wait(lambda:next(m['hp'] for m in current(page,host)['snapshot']['pve']['monsters'] if m['id']==target['id'])<old)
                else:raise AssertionError('combat budget')
                pickup(page,host)
            navigate(page,host,[1200,400]);assert obj(current(page,host))['available']
        def attune(page,host):
            v=current(page,host);return send(page,host,{'kind':'attune_objective','objective_id':obj(v)['id'],'expected_revision':v['snapshot']['inventory_revision']})
        def paid(page,host):assert attune(page,host).ok;wait(lambda:obj(current(page,host))['attuned'])
        def close(page,host):assert send(page,host,{'kind':'leave'}).ok;return done(page,host)
        def auto(page,fault='none'):
            page.locator('#scenario').select_option('b3-10');page.locator('#fault').select_option(fault);page.locator('#verify').click()
        def scenario(name,work,mode='source',point='',mobile=False,unfrozen=False):
            with tempfile.TemporaryDirectory(prefix='objective-') as directory:
                host=Host(directory,package(directory,mode));context=browser.new_context(viewport={'width':390,'height':844} if mobile else {'width':1440,'height':1100},is_mobile=mobile,has_touch=mobile);page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                try:
                    host.start(point,'commit_settlement' if point else '');page.goto(host.url);work(host,page,context);v=current(page,host)
                    if not unfrozen:check(v)
                    page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed'&&document.querySelector('#world-live').textContent==='0'",timeout=30000)
                    assert not errors,errors;assert v['phase']=='closed'
                    results[name]={'ledger':v['ledger'],'snapshot':v.get('snapshot'),'events':v['events'],'configHash':v['configHash'],'settlementRequest':frozen(v) if not unfrozen else None};print(name+': passed',flush=True)
                except Exception:
                    try:page.screenshot(path=str(OUTPUT/'failure.png'),full_page=True);(OUTPUT/'failure.json').write_text(json.dumps(current(page,host),ensure_ascii=False,indent=2))
                    except Exception:pass
                    host.error_log.seek(0);print(host.error_log.read(),flush=True);raise
                finally:context.close();host.stop();host.error_log.close()
        def source(host,page,context):
            auto(page);v=done(page,host);assert obj(v)['reward_opened'] and obj(v)['attuned'] and len(events(v,'kill_scored'))==7
            assert frozen(v)['body']['map_progress']['consumed']==[{'item_id':'huo-ling','quantity':2}]
            assert frozen(v)['body']['scoring']['kill_score']==200
            page.wait_for_function("document.body.dataset.verification==='passed'");page.screenshot(path=str(OUTPUT/'forest-complete.png'),full_page=True)
            before=v['ledger'];request=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==request
        scenario('source_forest_seven_kills_attune_seal_loot_extract_two_restarts',source)
        def cancellation(host,page,context):
            reach(page,host);page.reload();active(page);assert page.locator('#attune').is_enabled();page.locator('#attune').click();wait(lambda:obj(current(page,host))['finishes_at_ms'] is not None)
            page.screenshot(path=str(OUTPUT/'attunement.png'),full_page=True);page.locator('#cancel-attune').click();wait(lambda:obj(current(page,host))['finishes_at_ms'] is None)
            assert not obj(current(page,host))['attuned'] and events(current(page,host),'attunement_interrupted');paid(page,host);v=close(page,host);assert len(events(v,'attunement_completed'))==1
        scenario('desktop_public_buttons_cancel_retry_and_paid_death',cancellation,'flat')
        def refresh(host,page,context):
            reach(page,host);assert attune(page,host).ok;wait(lambda:obj(current(page,host))['finishes_at_ms'] is not None);page.reload();active(page);v=current(page,host);assert not obj(v)['attuned'] and obj(v)['finishes_at_ms'] is None
            paid(page,host);v=current(page,host);inventory=v['snapshot']['pve']['inventory'];page.reload();active(page);v=current(page,host);assert obj(v)['attuned'] and v['snapshot']['pve']['inventory']==inventory
            response=attune(page,host);assert response.status==409 and response.json()['error']=='INVALID_STATE';close(page,host)
        scenario('refresh_cancels_unpaid_and_keeps_paid_no_duplicate_cost',refresh,'flat')
        def shortage(host,page,context):
            reach(page,host);response=attune(page,host);assert response.status==409 and response.json()['error']=='NO_RESOURCE';v=close(page,host);assert not frozen(v)['body']['map_progress']['consumed']
        scenario('short_material_cannot_attune_or_consume',shortage,'shortage')
        def variant(host,page,context):reach(page,host);paid(page,host);close(page,host)
        scenario('zero_cost_same_binary',variant,'free');scenario('entry_material_cost_conserves_gross_and_net',variant,'root')
        def reordered(host,page,context):auto(page);v=done(page,host);assert obj(v)['reward_opened'] and len(obj(v)['completed_encounter_ids'])==3
        scenario('reordered_config_uses_authoritative_dependencies_for_one_click',reordered,'reordered')
        def dependency(host,page,context):
            begin(page);navigate(page,host,[200,800]);navigate(page,host,[1000,800]);navigate(page,host,[1000,400]);v=current(page,host);assert all(e['phase']=='dormant' for e in v['snapshot']['pve']['encounters']);assert attune(page,host).status==409
            assert send(page,host,{'kind':'open_chest','chest_id':'sanctum-cache','expected_generation':1}).status==409
            v=close(page,host);assert not frozen(v)['body']['map_progress']['completed_encounter_ids']
        scenario('skip_to_sanctum_cannot_start_or_claim',dependency,'flat')
        def mobile(host,page,context):reach(page,host);page.reload();active(page);page.locator('#attune').click();wait(lambda:obj(current(page,host))['attuned']);page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);close(page,host)
        scenario('mobile_objective_navigation_and_attune',mobile,'flat',mobile=True)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['settlements']==1
        scenario('source_lost_commit_receipt_once',lost)
        def source_death(host,page,context):
            auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and obj(v)['reward_opened'] and not v['snapshot']['resources'],timeout=45)
            v=close(page,host);assert frozen(v)['body']['outcome']=='death' and frozen(v)['body']['map_progress']['consumed']==[{'item_id':'huo-ling','quantity':2}]
        scenario('source_full_chain_claimed_loot_death_retains_consumption',source_death)

        def crash(host,page,context):
            auto(page);wait(lambda:host.process.poll() is not None,timeout=40);page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['settlements']==1
        scenario('source_crash_before_sql_commit_reconciles_frozen_cost',crash,point='before_storage_commit')
        scenario('source_crash_after_sql_commit_no_second_cost_or_award',crash,point='after_storage_commit')
        def unfrozen(host,page,context):
            reach(page,host);paid(page,host);host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=wait(lambda:v if (v:=current(page,host)) and v['phase']=='closed' and not v['ledger']['pending'] else None);assert v['ledger']['warehouse']==100 and v['ledger']['settlements']==0 and not v['ledger']['items']
        scenario('paid_but_unfrozen_process_interruption_refunds_entry_only',unfrozen,'flat',unfrozen=True)
        def manual(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=45000);wait(lambda:(v:=current(page,host)) and v['ledger']['manual']==1);before=frozen(current(page,host));host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'b3-10.json';c=json.loads(path.read_text());c['id']='b3-10-forest-objective-v2';c['pve']['map_objective']['cost']['quantity']=1;c['pve']['map_objective']['attune_duration_ms']=300;path.write_text(json.dumps(c))
            host.start();page.goto(host.url);wait(lambda:(v:=current(page,host)) and v['ledger']['manual']==1);assert frozen(current(page,host))==before;page.locator('#reconcile').click();v=done(page,host);assert frozen(v)==before
        # A pinned old raw configuration must determine the frozen cost after an upgrade.
        scenario('source_manual_reconcile_pins_original_cost_after_config_upgrade',manual)
        browser.close()
    assert sources==hashes(),'Sources changed during verification'
    (OUTPUT/'verification.json').write_text(json.dumps({'rust_tests':tests,'all_rust_tests':len(all_tests),'browser_process_cases':results,'page_errors':errors,'source_sha256':sources,'binary_sha256':binary},ensure_ascii=False,indent=2))
    print(json.dumps({'rust':len(tests),'browser_process':len(results),'pageErrors':len(errors),'report':str(OUTPUT/'verification.json')}))
if __name__=='__main__':main()
