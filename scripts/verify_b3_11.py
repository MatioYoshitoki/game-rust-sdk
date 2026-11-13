"""Real Rust support targeting, manual interruption, finite casts and frozen forest assets."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-11';BINARY=ROOT/'target/debug/kernel-host'
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir()
    for alias in ['b1','b3-11']:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-11.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[400,400],extraction_point=[1100,900],geometry={'min':[0,0],'max':[1536,1536],'walls':[]},extraction_unlock_ms=20000)
        c['params']['runtime']['publish_batched_ms']=300;c['params']['regen'].update(near_ratio=0,far_ratio=0);c['params']['minimum_damage']=0;c['skill']['damage_bps']=10000
        p=c['pve'];p.update(map_id='support-flat',width=48,height=48,cell_size=32,cells_rle=[[0,2304]],monster_positions=[],extra_spawns=[])
        p['stats'].update(hp=600,spirit=100,atk=0,spd=0,crit=0,dodge=0);p['stats']['def']=0;p['combat']['cast_chance']=0;p['combat']['heal']['hp_threshold']=0
        for t in p['additional_templates'].values():
            t['stats']=copy.deepcopy(p['stats']);t['combat']['cast_chance']=0;t['combat']['heal']['hp_threshold']=0;t.pop('area',None);t.pop('boss',None)
        template=p['additional_templates']['verification-forest-support'];s=template['support'];s.update(range=500,include_self=False,hp_threshold=.9,windup_ms=60000,ticks=3,tick_interval_ms=30000,interrupt_window_ms=120000,heal_bps=1000,cooldown_ms=180000,recovery_ms=30000,interrupt_cooldown_ms=60000)
        e=copy.deepcopy(p['encounters'][0]);e.update(id='team',requires=[],trigger={'center':[500,400],'radius':200},monsters=[{'id':'target-b','template_id':'verification-phase-boss','position':[600,400]},{'id':'support-a','template_id':'verification-forest-support','position':[700,400]}],required_monster_ids=['target-b','support-a'],reward_chest_ids=['sanctum-cache'])
        p['encounters']=[e];p['chest_spawns'][0]['position']=[1100,800];p['map_objective']['required_encounter_ids']=['team']
        if mode=='zero-damage':c['skill']['damage_bps']=0
        if mode=='zero-chance':s['cast_chance']=0
        if mode=='no-mp':template['stats']['spirit']=19
        if mode=='zero-casts':s['max_casts']=0
        if mode=='range':s['range']=1
        if mode=='dodge':template['stats']['dodge']=100
        if mode=='one-cast':s.update(max_casts=1,ticks=1,interrupt_window_ms=60000,heal_bps=100)
        if mode=='multi':e['monsters'].append({'id':'support-b','template_id':'verification-forest-support','position':[700,600]});e['required_monster_ids'].append('support-b')
        if mode=='cross-group':
            other=copy.deepcopy(e);other.update(id='other',monsters=[e['monsters'].pop()],required_monster_ids=['support-a'],reward_chest_ids=[]);e['required_monster_ids']=['target-b'];p['encounters'].append(other);p['map_objective']['required_encounter_ids'].append('other')
        if mode=='vision':
            tile=copy.deepcopy(p['palette'][0]);tile['blocksVision']=True;index=len(p['palette']);p['palette'].append(tile);cells=[0]*2304;cells[12*48+20]=index;p['cells_rle']=[[v,1] for v in cells]
        if mode=='old-shot':c['spawn']=[650,400];c['basic'].update(projectile_speed=1,windup_ms=3000);c['skill'].update(windup_ms=60000,speed=1000);c['params']['player_projectile_ttl_ms']=180000
        if mode=='lethal-caster':template['stats']['hp']=60
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{'b1':'b1.json','b3-11':'b3-11.json'},'archives':[]}));return manifest

def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_11.py','--check'],cwd=ROOT,check=True)
    native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,capture_output=True,text=True,check=True)
    all_tests = re.findall(r'^test (\S+) \.\.\. ok$', native.stdout, re.M)
    tests = [
        test for test in all_tests
        if (
            '::support_tests::' in test
            or test.startswith('verification::support::')
            or test.startswith('config::catalog::tests::support_')
        )
        and not test.rsplit('::', 1)[-1].startswith('support_variant_')
    ]
    assert len(tests)==48 and len(all_tests)>=483,(len(tests),len(all_tests))
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def events(v,kind):return [e for e in v['events'] if e['kind']==kind]
        def caster(v,id='support-a'):return next(m for m in v['snapshot']['pve']['monsters'] if m['id']==id)
        def target(v):return next(m for m in v['snapshot']['pve']['monsters'] if m['id']=='target-b')
        def frozen(v):return next(r for r in v['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):return wait(lambda:v if (v:=current(page,host)) and v['phase']=='closed' and v['settlement'] and not v['ledger']['pending'] else None,timeout=50)
        def check(v):
            b=frozen(v)['body'];l=v['ledger'];consumed={r['item_id']:r['quantity'] for r in b['map_progress']['consumed']}
            assert b['carried']+b['looted']==b['kept']+b['dropped']+consumed.get(b['item_id'],0)
            assert all(i['looted']==i['kept']+i['dropped']+consumed.get(i['item_id'],0) for i in b.get('items',[]))
            assert l['warehouse']==100-b['carried']+b['kept'] and l['cultivation']==b['cultivation'] and l['items']=={i['item_id']:i['kept'] for i in b.get('items',[])}
            assert l['admissions']==l['settlements']==1 and l['manual']==0 and len(events(v,'settlement_applied'))==1
        serial=0
        def send(page,host,action):
            nonlocal serial
            for _ in range(3):
                v=current(page,host);serial+=1;response=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'support-{serial}','action':action})
                if response.ok or response.json().get('error')!='STALE_SEQUENCE':return response
            raise AssertionError(response.text())
        def navigate(page,host,position):
            assert send(page,host,{'kind':'navigate','destination':position}).ok
            wait(lambda:sum((a-b)**2 for a,b in zip(current(page,host)['snapshot']['position'],position))<20**2);assert send(page,host,{'kind':'stop'}).ok
        def begin(page):page.locator('#scenario').select_option('b3-11');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        def wound(page,host,old=False):
            begin(page)
            if not old:navigate(page,host,[550,400])
            assert send(page,host,{'kind':'attack'}).ok
            if old:return
            wait(lambda:target(current(page,host))['hp']<600)
        def started(page,host):wound(page,host);return wait(lambda:a if (v:=current(page,host)) and (a:=caster(v)['support']['cast']) else None)
        def approach(page,host):
            for pos in [[550,500],[750,500],[750,400]]:navigate(page,host,pos)
            assert current(page,host)['snapshot']['target']['id']=='support-a'
        def attack(page,host,skill=False,direction=(-1,0)):
            wait(lambda:(v:=current(page,host)) and v['snapshot']['logical_ms']>=v['snapshot']['basic_ready_at_ms'])
            action={'kind':'attack'}
            if skill:
                v=current(page,host);config=page.request.get(host.url+f"/api/config?sessionId={v['id']}").json()
                action={'kind':'cast','skill_id':config['skill']['id'],'direction':list(direction)}
            return send(page,host,action)
        def close(page,host):assert send(page,host,{'kind':'leave'}).ok;return done(page,host)
        def auto(page,fault='none'):
            page.locator('#scenario').select_option('b3-11');page.locator('#fault').select_option(fault);page.locator('#verify').click()
        def scenario(name,work,mode='source',point='',mobile=False,unfrozen=False):
            with tempfile.TemporaryDirectory(prefix='support-') as directory:
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
            auto(page);page.wait_for_function("['passed','failed'].includes(document.body.dataset.verification)",timeout=50000);assert page.locator('body').get_attribute('data-verification')=='passed',page.locator('#toast').inner_text();v=done(page,host);assert any(e['amount']>0 for e in events(v,'support_pulse'));assert frozen(v)['body']['scoring']['kill_score']==200 and v['ledger']['cultivation']==172
            page.wait_for_function("document.body.dataset.verification==='passed'");page.screenshot(path=str(OUTPUT/'forest-complete.png'),full_page=True);before=v['ledger'];body=frozen(v)
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==body
        scenario('source_forest_support_heals_seven_kills_loot_extract_two_restarts',source)
        def death(host,page,context):auto(page);wait(lambda:(v:=current(page,host)) and v.get('snapshot') and v['snapshot']['pve']['map_objective']['reward_opened'] and not v['snapshot']['resources'],timeout=50);v=close(page,host);assert v['ledger']['cultivation']==75
        scenario('source_complete_forest_support_and_death_conserves_consumed_items',death)
        def interrupt(host,page,context):
            a=started(page,host);approach(page,host);assert attack(page,host).ok;wait(lambda:events(current(page,host),'support_interrupted'));v=current(page,host);assert len(events(v,'support_interrupted'))==1 and not events(v,'support_pulse');assert caster(v)['spirit']==100 and caster(v)['support']['remaining_casts']==4;close(page,host)
        scenario('manual_attack_in_window_interrupts_unpaid_preparation',interrupt,'flat')
        def skill(host,page,context):
            started(page,host);approach(page,host);assert attack(page,host,True).ok;wait(lambda:events(current(page,host),'support_interrupted'));v=current(page,host);assert caster(v)['spirit']==100 and not events(v,'support_pulse');close(page,host)
        scenario('manual_skill_direct_contact_interrupts',skill,'flat');scenario('zero_damage_manual_skill_contact_still_interrupts',skill,'zero-damage')
        def oldshot(host,page,context):
            wound(page,host,old=True);wait(lambda:events(current(page,host),'action_committed'));assert attack(page,host,True,(1,0)).ok
            a=wait(lambda:a if (v:=current(page,host)) and (a:=caster(v)['support']['cast']) else None)
            wait(lambda:caster(current(page,host))['hp']<600);v=current(page,host);assert not events(v,'support_interrupted') and caster(v)['support']['cast']['cast_id']==a['cast_id'];close(page,host)
        scenario('action_started_before_heal_window_hits_without_interrupt',oldshot,'old-shot')
        def gated(host,page,context):
            wound(page,host);v=current(page,host);at=v['snapshot']['logical_ms'];wait(lambda:current(page,host)['snapshot']['logical_ms']>at+30000);assert not events(current(page,host),'support_started');close(page,host)
        for mode in ['zero-chance','no-mp','zero-casts','cross-group','vision','range']:scenario('configured_'+mode+'_never_starts_ally_heal',gated,mode)
        def channel(host,page,context):
            started(page,host);wait(lambda:events(current(page,host),'support_completed'));v=current(page,host);assert [e['amount'] for e in events(v,'support_pulse')]==[20,20,20];assert caster(v)['spirit']==80 and caster(v)['support']['remaining_casts']==3;close(page,host)
        scenario('channel_commits_once_and_delivers_three_budgeted_pulses',channel,'flat')
        def onecast(host,page,context):
            started(page,host);wait(lambda:events(current(page,host),'support_completed'));v=current(page,host);ready=caster(v)['support']['ready_at_ms'];wait(lambda:current(page,host)['snapshot']['logical_ms']>ready+1000);v=current(page,host);assert caster(v)['support']['remaining_casts']==0 and len(events(v,'support_started'))==1;close(page,host)
        scenario('configured_single_successful_cast_budget_cannot_repeat',onecast,'one-cast')
        def partial(host,page,context):
            a=started(page,host);approach(page,host);wait(lambda:events(current(page,host),'support_pulse'));assert attack(page,host).ok;wait(lambda:events(current(page,host),'support_interrupted'));v=current(page,host);assert len(events(v,'support_pulse'))==1 and caster(v)['spirit']==80 and caster(v)['support']['remaining_casts']==3;close(page,host)
        scenario('interrupt_after_first_pulse_preserves_paid_cost_and_stops_future_heals',partial,'flat')
        def refresh(host,page,context,paid=False):
            a=started(page,host)
            if paid:wait(lambda:events(current(page,host),'support_pulse'))
            before=current(page,host);count=len(events(before,'support_pulse'));page.reload();active(page);v=current(page,host);assert caster(v)['support']['cast'] is None and len(events(v,'support_pulse'))==count;assert caster(v)['spirit']==(80 if paid else 100) and caster(v)['support']['remaining_casts']==(3 if paid else 4);close(page,host)
        scenario('refresh_cancels_preparation_without_cost',refresh,'flat');scenario('refresh_stops_paid_channel_without_refund',lambda h,p,c:refresh(h,p,c,True),'flat')
        def lethal(host,page,context):
            started(page,host);approach(page,host);assert attack(page,host).ok;wait(lambda:caster(current(page,host))['hp']==0);v=current(page,host);assert not events(v,'support_pulse') and not events(v,'support_interrupted') and events(v,'support_cancelled');close(page,host)
        scenario('caster_death_cancels_without_interrupt_credit_or_delayed_heal',lethal,'lethal-caster')
        def dodge(host,page,context):
            a=started(page,host);approach(page,host);assert attack(page,host).ok;wait(lambda:len(events(current(page,host),'action_committed'))>=2);v=current(page,host);assert not events(v,'support_interrupted') and caster(v)['support']['cast']['cast_id']==a['cast_id'];close(page,host)
        scenario('dodged_contact_does_not_interrupt',dodge,'dodge')
        def multi(host,page,context):
            started(page,host);wait(lambda:caster(current(page,host),'support-b')['support']['cast']);approach(page,host);assert attack(page,host).ok;wait(lambda:events(current(page,host),'support_interrupted'))
            navigate(page,host,[750,650]);assert attack(page,host).ok;wait(lambda:caster(current(page,host),'support-b')['hp']<600);v=current(page,host);assert len(events(v,'support_interrupted'))==1 and caster(v,'support-b')['support']['cast'];close(page,host)
        scenario('global_interrupt_cooldown_is_shared_across_two_healers',multi,'multi')
        def mobile(host,page,context):
            started(page,host);page.reload();active(page);wait(lambda:caster(current(page,host))['support']['cast']);page.wait_for_function("document.querySelector('#arena').dataset.supportCasts==='1'");page.locator('#navigate-support').click();wait(lambda:current(page,host)['snapshot']['target']['id']=='support-a');page.locator('#attack').click();wait(lambda:events(current(page,host),'support_interrupted'));page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);assert page.evaluate('document.documentElement.scrollWidth<=innerWidth');close(page,host)
        scenario('mobile_authoritative_cast_line_navigation_and_attack_button',mobile,'flat',mobile=True)
        def desktop(host,page,context):
            started(page,host);page.reload();active(page);wait(lambda:caster(current(page,host))['support']['cast']);page.wait_for_function("document.querySelector('#support-status').textContent.includes('可主动命中打断')");page.screenshot(path=str(OUTPUT/'support.png'),full_page=True);close(page,host)
        scenario('desktop_target_countdown_and_budget_projection',desktop,'flat')
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context):auto(page);host.killed();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==172
            scenario('source_support_assets_'+point,crash,point=point)
        def lost(host,page,context):auto(page,'settlement_lost_receipt');v=done(page,host);assert v['ledger']['settlements']==1 and events(v,'support_pulse')
        scenario('lost_receipt_reconciles_original_support_forest_assets_once',lost)
        def version(host,page,context):
            auto(page,'refused');page.locator('#reconcile').wait_for(state='visible',timeout=50000);v=current(page,host);body=frozen(v);history=events(v,'support_pulse');host.kill();page.goto('about:blank');path=Path(host.manifest).parent/'b3-11.json';c=json.loads(path.read_text());c['id']='b3-11-support-heal-v2';c['pve']['additional_templates']['verification-forest-support']['support'].update(heal_bps=10000,max_casts=1);path.write_text(json.dumps(c));host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible');v=current(page,host);assert frozen(v)==body and events(v,'support_pulse')==history;page.locator('#reconcile').click();v=done(page,host);assert frozen(v)==body and events(v,'support_pulse')==history
        scenario('support_parameter_upgrade_keeps_old_manual_assets_and_no_heal_replay',version)
        def unfrozen(host,page,context):
            started(page,host);wait(lambda:events(current(page,host),'support_pulse'));host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=wait(lambda:v if (v:=current(page,host)) and v['phase']=='closed' and not v['ledger']['pending'] else None);assert v['ledger']['warehouse']==100 and v['ledger']['settlements']==0 and not v['ledger']['items']
        scenario('unfrozen_active_heal_interruption_refunds_entry_only',unfrozen,'flat',unfrozen=True)
        browser.close()
    assert sources==hashes(),'Sources changed during verification'
    (OUTPUT/'verification.json').write_text(json.dumps({'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceHashes':sources,'binarySha256':binary},ensure_ascii=False,indent=2))
    print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests),'pageErrors':len(errors)}))
if __name__=='__main__':main()
