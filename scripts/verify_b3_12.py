"""Locked line heavy, real movement dodge, authenticated counter contact and frozen assets."""
import argparse,copy,hashlib,json,re,shutil,subprocess,tempfile,time
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-12';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir()
    for alias in ['b1','b3-12']:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-12.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[400,400],extraction_point=[1100,900],geometry={'min':[0,0],'max':[1536,1536],'walls':[]})
        c['params']['regen'].update(near_ratio=0,far_ratio=0);c['params']['minimum_damage']=0;c['basic'].update(range=300,windup_ms=300,projectile_speed=1000);c['skill']['damage_bps']=0
        p=c['pve'];p.update(map_id='heavy-flat',width=48,height=48,cell_size=32,cells_rle=[[0,2304]],monster_positions=[],extra_spawns=[])
        t=p['additional_templates']['verification-phase-boss'];t['stats'].update(hp=1000,spirit=100,atk=50,spd=0,crit=0,dodge=0);t['stats']['def']=0;t.pop('area',None);t['combat']['cast_chance']=0;t['combat']['heal']['hp_threshold']=0;t['combat']['melee_range']=1;t.update(leash=600,memory_range=700);t['combat']['aggro_range']=700
        for phase in t['boss']['phases']:phase['cast_chance']=0
        h=t['heavy'];h.update(range=500,windup_ms=60000,travel_ms=30000,cost=20,cooldown_ms=180000,recovery_ms=300,counter_window_ms=60000)
        e=copy.deepcopy(p['encounters'][0]);e.update(id='team',requires=[],trigger={'center':[500,400],'radius':600},monsters=[{'id':'heavy-owner','template_id':'verification-phase-boss','position':[600,400]}],required_monster_ids=['heavy-owner'],reward_chest_ids=['sanctum-cache'])
        p['encounters']=[e];p['chest_spawns'][0]['position']=[1100,800];p['map_objective']['required_encounter_ids']=['team']
        if mode=='visual':h.update(windup_ms=3000,travel_ms=1200,counter_window_ms=10000)
        if mode=='zero-chance':h['cast_chance']=0
        if mode=='no-mp':t['stats']['spirit']=19
        if mode=='out-of-range':c['spawn']=[50,400]
        if mode=='phase':h['min_phase_index']=2
        if mode=='zero-heavy':h['damage_bps']=0
        if mode=='old-action':c['basic']['windup_ms']=40000
        if mode=='short-window':h['counter_window_ms']=1000
        if mode=='dodge':t['stats']['dodge']=100
        if mode=='lethal-owner':t['stats']['hp']=60
        if mode=='wrong-target':
            other=copy.deepcopy(t);other.pop('heavy');other['stats']['atk']=0;p['additional_templates']['other-target']=other;e['monsters'].append({'id':'other-target','template_id':'other-target','position':[400,650]});e['required_monster_ids'].append('other-target')
        if mode in ['wall','blocked-move']:
            tile=copy.deepcopy(p['palette'][0]);tile['blocksVision']=False;tile['blocksProjectiles']=mode=='wall';tile['walkable']=mode!='blocked-move'
            if mode=='blocked-move':tile['speedMultiplier']=0
            index=len(p['palette']);p['palette'].append(tile);cells=[0]*2304;cells[(12*48+15) if mode=='wall' else (13*48+12)]=index;p['cells_rle']=[[v,1] for v in cells]
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{'b1':'b1.json','b3-12':'b3-12.json'},'archives':[]}));return manifest

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--case',default='');args=parser.parse_args();OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_12.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,capture_output=True,text=True,check=True);all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M);tests=[t for t in all_tests if '::heavy_tests::' in t or t.startswith('verification::heavy::') or t.startswith('config::catalog::tests::heavy_')];assert len(tests)==48 and len(all_tests)>=531,(len(tests),len(all_tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def events(v,kind):return [e for e in v['events'] if e['kind']==kind]
        def owner(v):return next(m for m in v['snapshot']['pve']['monsters'] if m['id']=='heavy-owner')
        def window(v):return v['snapshot']['pve'].get('counter')
        def frozen(v):return next(r for r in v['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):return wait(lambda:v if (v:=current(page,host)) and v['phase']=='closed' and v['settlement'] and not v['ledger']['pending'] else None,timeout=50)
        def check(v):
            b=frozen(v)['body'];l=v['ledger'];consumed={r['item_id']:r['quantity'] for r in b['map_progress']['consumed']}
            assert b['carried']+b['looted']==b['kept']+b['dropped']+consumed.get(b['item_id'],0)
            assert all(i['looted']==i['kept']+i['dropped']+consumed.get(i['item_id'],0) for i in b.get('items',[]))
            assert l['warehouse']==100-b['carried']+b['kept'] and l['cultivation']==b['cultivation'] and l['items']=={i['item_id']:i['kept'] for i in b.get('items',[])}
            assert l['admissions']==l['settlements']==1 and not l['pending'] and l['manual']==0 and len(events(v,'settlement_applied'))==1
        serial=0
        def send(page,host,action):
            nonlocal serial
            for _ in range(3):
                v=current(page,host);serial+=1;r=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'heavy-{serial}','action':action})
                if r.ok or r.json().get('error')!='STALE_SEQUENCE':return r
            raise AssertionError(r.text())
        def begin(page,live=False):page.locator('#scenario').select_option('b3-12');page.locator('#clock').select_option('live' if live else 'fast');page.locator('#start').click();active(page)
        def started(page,host,live=False):begin(page,live);return wait(lambda:a if (v:=current(page,host)) and (a:=owner(v)['heavy']['cast']) else None)
        def navigate(page,host,point):
            assert send(page,host,{'kind':'navigate','destination':point}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],point))<12**2);assert send(page,host,{'kind':'stop'}).ok
        def dodge(page,host,live=False):started(page,host,live);navigate(page,host,[400,500]);return wait(lambda:w if (v:=current(page,host)) and (w:=window(v)) else None)
        def attack(page,host,skill=False):
            v=current(page,host)
            if skill:
                p=v['snapshot']['position'];c=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json();action={'kind':'cast','skill_id':c['skill']['id'],'direction':[600-p[0],400-p[1]]}
            else:action={'kind':'attack'}
            return send(page,host,action)
        def close(page,host):assert send(page,host,{'kind':'leave'}).ok;return done(page,host)
        def auto(page,fault='none'):page.locator('#scenario').select_option('b3-12');page.locator('#fault').select_option(fault);page.locator('#verify').click()
        def scenario(name,work,mode='flat',point='',mobile=False,unfrozen=False):
            if args.case and args.case not in name:return
            with tempfile.TemporaryDirectory(prefix='heavy-') as directory:
                host=Host(directory,package(directory,mode));context=browser.new_context(viewport={'width':390,'height':844} if mobile else {'width':1440,'height':1100},is_mobile=mobile,has_touch=mobile);page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                try:
                    host.start(point,'commit_settlement' if point else '');page.goto(host.url);work(host,page,context);v=current(page,host)
                    if not unfrozen:check(v)
                    page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed'&&document.querySelector('#world-live').textContent==='0'",timeout=30000);assert not errors,errors
                    results[name]={'ledger':v['ledger'],'snapshot':v.get('snapshot'),'events':v['events'],'configHash':v['configHash'],'settlementRequest':frozen(v) if not unfrozen else None};print(name+': passed',flush=True)
                except Exception:
                    try:page.screenshot(path=str(OUTPUT/'failure.png'),full_page=True);(OUTPUT/'failure.json').write_text(json.dumps(current(page,host),ensure_ascii=False,indent=2))
                    except Exception:pass
                    host.error_log.seek(0);print(host.error_log.read(),flush=True);raise
                finally:context.close();host.stop();host.error_log.close()
        def source(host,page,context):
            auto(page);page.wait_for_function("['passed','failed'].includes(document.body.dataset.verification)",timeout=50000);assert page.locator('body').get_attribute('data-verification')=='passed',page.locator('#toast').inner_text();v=done(page,host);assert events(v,'heavy_started') and events(v,'heavy_committed') and events(v,'heavy_resolved') and any(e['amount']>0 for e in events(v,'support_pulse'));assert frozen(v)['body']['scoring']['kill_score']==200 and v['ledger']['cultivation']==172
            page.screenshot(path=str(OUTPUT/'forest-complete.png'),full_page=True);before=v['ledger'];body=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==body
        scenario('source_heavy_support_forest_seven_kills_extract_two_restarts',source,'source')
        def death(host,page,context):auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and v['snapshot']['pve']['map_objective']['reward_opened'] and not v['snapshot']['resources'],timeout=50);v=close(page,host);assert v['ledger']['cultivation']==75
        scenario('source_complete_forest_death_consumption_conserved',death,'source')
        def standing(host,page,context):started(page,host);wait(lambda:events(current(page,host),'heavy_resolved'));v=current(page,host);assert events(v,'heavy_resolved')[0]['contacted'] and not window(v);close(page,host)
        scenario('standing_contact_never_grants_counter',standing);scenario('zero_heavy_damage_contact_still_no_counter',standing,'zero-heavy')
        def counter(host,page,context,skill=False):
            dodge(page,host);before=owner(current(page,host))['hp'];assert attack(page,host,skill).ok;wait(lambda:events(current(page,host),'counter_triggered'));v=current(page,host);assert len(events(v,'counter_triggered'))==1 and events(v,'counter_triggered')[0]['damage']==120;assert owner(v)['hp']==before-(120 if skill else 220);close(page,host)
        scenario('movement_dodge_manual_attack_counter_once',counter);scenario('zero_direct_damage_skill_still_triggers_physical_counter',lambda h,p,c:counter(h,p,c,True))
        def duplicate(host,page,context):
            dodge(page,host);v=current(page,host);payload={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':'counter-once','action':{'kind':'attack'}};url=host.url+f"/api/sessions/{v['id']}/input"
            assert page.request.post(url,data=payload).ok;assert page.request.post(url,data=payload).ok;wait(lambda:events(current(page,host),'counter_triggered'));assert len(events(current(page,host),'counter_triggered'))==1;assert send(page,host,{'kind':'attack','counter_damage':99999}).json()['error']=='INVALID_MESSAGE';close(page,host)
        scenario('duplicate_request_and_forged_counter_amount_cannot_repeat',duplicate)
        def old(host,page,context):
            started(page,host);navigate(page,host,[400,500]);wait(lambda:events(current(page,host),'heavy_committed'));assert attack(page,host).ok;wait(lambda:window(current(page,host)));wait(lambda:owner(current(page,host))['hp']<1000);v=current(page,host);assert window(v) and not events(v,'counter_triggered');close(page,host)
        scenario('action_accepted_before_counter_window_cannot_consume',old,'old-action')
        def expire(host,page,context):
            started(page,host);navigate(page,host,[400,500]);opened=wait(lambda:events(current(page,host),'counter_opened'));deadline=opened[0]['window']['ends_at_ms'];wait(lambda:current(page,host)['snapshot']['logical_ms']>deadline and not window(current(page,host)));assert attack(page,host).ok;wait(lambda:owner(current(page,host))['hp']<1000);assert not events(current(page,host),'counter_triggered');close(page,host)
        scenario('expired_window_hit_is_only_normal_damage',expire,'short-window')
        def wrong(host,page,context):
            dodge(page,host);assert attack(page,host).ok;wait(lambda:any(m['id']=='other-target' and m['hp']<1000 for m in current(page,host)['snapshot']['pve']['monsters']));assert window(current(page,host)) and not events(current(page,host),'counter_triggered');close(page,host)
        scenario('wrong_target_direct_contact_keeps_original_window',wrong,'wrong-target')
        def missed(host,page,context):
            dodge(page,host);assert attack(page,host).ok;wait(lambda:events(current(page,host),'action_committed'));v=current(page,host);assert owner(v)['hp']==1000 and window(v) and not events(v,'counter_triggered');close(page,host)
        scenario('dodged_direct_contact_does_not_consume_window',missed,'dodge')
        def blocked(host,page,context):
            started(page,host);assert send(page,host,{'kind':'move','direction':[0,1]}).ok;wait(lambda:events(current(page,host),'heavy_resolved'));v=current(page,host);assert v['snapshot']['position'][1]<420 and not window(v);assert send(page,host,{'kind':'stop'}).ok;close(page,host)
        scenario('blocked_movement_intent_is_not_a_real_dodge',blocked,'blocked-move')
        def wall(host,page,context):
            started(page,host);navigate(page,host,[400,500]);wait(lambda:events(current(page,host),'heavy_resolved'));v=current(page,host);assert events(v,'heavy_resolved')[0]['blocked'] and not window(v);close(page,host)
        scenario('projectile_wall_prevents_damage_and_counter_credit',wall,'wall')
        def gated(host,page,context):
            begin(page);v=current(page,host);at=v['snapshot']['logical_ms'];wait(lambda:current(page,host)['snapshot']['logical_ms']>at+90000);assert not events(current(page,host),'heavy_started') and not window(current(page,host));close(page,host)
        for mode in ['zero-chance','no-mp','out-of-range','phase']:scenario('configured_'+mode+'_gates_heavy_without_counter',gated,mode)
        def refresh(host,page,context,phase=0):
            if phase==2:dodge(page,host)
            else:
                started(page,host)
                if phase==1:wait(lambda:events(current(page,host),'heavy_committed'))
            before=current(page,host);old=owner(before)['heavy']['cast'];page.reload();active(page);v=current(page,host);new=owner(v)['heavy']['cast'];assert (new is None or old is not None and new['cast_id']!=old['cast_id']) and not window(v);assert owner(v)['spirit']==(100 if phase==0 else 80);assert len(events(v,'counter_triggered'))==len(events(before,'counter_triggered'));close(page,host)
        for phase,label in enumerate(['preparation','paid_travel','counter_window']):scenario('refresh_cancels_'+label+'_without_replaying',lambda h,p,c,phase=phase:refresh(h,p,c,phase))
        def returning(host,page,context):
            started(page,host);wait(lambda:events(current(page,host),'heavy_committed'));navigate(page,host,[1300,1200]);v=current(page,host);assert any(e['reason']=='returning' for e in events(v,'heavy_cancelled'));assert not window(v) and owner(v)['spirit']==80;close(page,host)
        scenario('actual_leash_return_cancels_and_keeps_paid_cost',returning)
        def lethal(host,page,context):
            started(page,host);assert attack(page,host).ok;wait(lambda:owner(current(page,host))['hp']==0);v=current(page,host);assert not events(v,'heavy_committed') and not window(v);close(page,host)
        scenario('caster_death_before_commit_cancels_without_counter',lethal,'lethal-owner')
        def mobile(host,page,context):
            started(page,host,True);button=page.locator('[data-dir="1,0"]');button.scroll_into_view_if_needed();box=button.bounding_box();page.mouse.move(box['x']+box['width']/2,box['y']+box['height']/2);page.mouse.down();wait(lambda:current(page,host)['snapshot']['position'][1]<320);page.mouse.up();wait(lambda:window(current(page,host)));page.locator('#attack').click();wait(lambda:events(current(page,host),'counter_triggered'));assert page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);close(page,host)
        scenario('mobile_dpad_real_dodge_and_attack_button_counter',mobile,'visual',mobile=True)
        def desktop(host,page,context):
            started(page,host,True);page.wait_for_function("document.querySelector('#arena').dataset.heavyCasts==='1'");page.screenshot(path=str(OUTPUT/'heavy.png'),full_page=True);navigate(page,host,[400,500]);page.wait_for_function("document.querySelector('#heavy-status').textContent.includes('轨迹推进')");wait(lambda:(a:=owner(current(page,host))['heavy']['cast']) and a['committed'] and a['front']!=a['origin']);page.screenshot(path=str(OUTPUT/'heavy-front.png'),full_page=True);wait(lambda:window(current(page,host)));page.wait_for_function("document.querySelector('#arena').dataset.counterReady==='true'&&document.querySelector('#counter-status').textContent.includes('借隙反击')");page.screenshot(path=str(OUTPUT/'counter.png'),full_page=True);close(page,host)
        scenario('desktop_isometric_trajectory_front_countdown_and_counter',desktop,'visual')
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context):
                auto(page);deadline=time.monotonic()+50
                while host.process.poll() is None and time.monotonic()<deadline:page.wait_for_timeout(50)
                host.killed();page.goto('about:blank');old=next(r for _,r in host.state()['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==172 and frozen(v)['body']==old['body']
            scenario('source_heavy_assets_'+point,crash,'source',point=point)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['cultivation']==172 and events(v,'io_timed_out') and events(v,'heavy_started')
        scenario('lost_receipt_reconciles_original_heavy_forest_once',lost,'source')
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=50000);v=current(page,host);body=frozen(v);history=events(v,'heavy_started');host.kill();page.goto('about:blank');path=Path(host.manifest).parent/'b3-12.json';c=json.loads(path.read_text());c['id']='b3-12-heavy-counter-v2';c['pve']['additional_templates']['verification-phase-boss']['heavy'].update(counter_window_ms=1200,counter_damage_bps=30000,damage_bps=5000);path.write_text(json.dumps(c));host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible');v=current(page,host);assert frozen(v)==body and events(v,'heavy_started')==history;page.locator('#reconcile').click();v=done(page,host);before=v['ledger'];assert frozen(v)==body
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==body and events(v,'heavy_started')==history
        scenario('heavy_parameter_upgrade_pins_old_body_two_restarts_no_replay',version,'source')
        def unfrozen(host,page,context):
            dodge(page,host);host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=wait(lambda:v if (v:=current(page,host)) and v['phase']=='closed' and not v['ledger']['pending'] else None);assert v['ledger']['warehouse']==100 and v['ledger']['settlements']==0 and v['ledger']['compensations']==1 and not v['ledger']['items'] and not events(v,'counter_triggered')
        scenario('unfrozen_counter_window_crash_refunds_entry_only',unfrozen,unfrozen=True)
        browser.close()
    assert sources==hashes(),'Sources changed during verification';assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest()
    assert results and not errors
    if not args.case:assert len(results)==29,len(results)
    report='verification-development.json' if args.case else 'verification.json';(OUTPUT/report).write_text(json.dumps({'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceHashes':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n');print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests),'pageErrors':len(errors),'report':report}))
if __name__=='__main__':main()
