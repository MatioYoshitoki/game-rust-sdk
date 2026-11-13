"""Public round reset, old-action isolation, first-defeat rewards and frozen forest assets."""
import argparse,copy,hashlib,json,math,re,shutil,subprocess,tempfile,time
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,info
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1];OUTPUT=ROOT/'artifacts/b3-13';BINARY=ROOT/'target/debug/kernel-host'
def wait(fn,timeout=20):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        value=fn()
        if value:return value
        time.sleep(.02)
    raise AssertionError('Timed out waiting for authoritative H5 state')
def package(directory,mode='source'):
    root=Path(directory)/'config';root.mkdir()
    for alias in ['b1','b3-13']:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    path=root/'b3-13.json';c=json.loads(path.read_text())
    if mode!='source':
        c.update(spawn=[400,400],extraction_point=[1100,1000],geometry={'min':[0,0],'max':[1536,1536],'walls':[]})
        c['params']['minimum_damage']=0;c['params']['regen'].update(near_ratio=0,far_ratio=0);c['params']['resource_ttl_ms']=10000;c['params']['runtime'].update(resource_sweep_ms=1000,publish_batched_ms=300);c['basic'].update(range=400,windup_ms=300,projectile_speed=1000);c['skill']['damage_bps']=10000
        p=c['pve'];p.update(map_id='reset-flat',width=48,height=48,cell_size=32,cells_rle=[[0,2304]],monster_positions=[],extra_spawns=[]);p['stats'].update(hp=80,spirit=100,atk=0,spd=0,crit=0,dodge=0);p['stats']['def']=0;p['combat'].update(cast_chance=0,melee_range=1);p['combat']['heal']['hp_threshold']=0;p['drop_table']={'guaranteed':[{'itemId':'huo-ling','quantityMin':2,'quantityMax':2}], 'random':[]}
        for t in p['additional_templates'].values():
            t['stats']=copy.deepcopy(p['stats']);t['combat']=copy.deepcopy(p['combat']);t.pop('area',None);t.pop('heavy',None);t.pop('support',None)
            if 'boss' in t:
                for phase in t['boss']['phases']:phase['cast_chance']=0
        e=copy.deepcopy(p['encounters'][0]);e.update(id='team',requires=[],trigger={'center':[500,400],'radius':200},monsters=[{'id':'a','template_id':p['template_id'],'position':[600,400]},{'id':'b','template_id':'verification-phase-boss','position':[700,400]}],required_monster_ids=['a','b'],reward_chest_ids=['sanctum-cache']);e.pop('waves',None);e['reset'].update(pursuit_radius=300,delay_ms=10000)
        p['encounters']=[e];p['chest_spawns'][0]['position']=[1100,800];p['map_objective']['required_encounter_ids']=['team']
        if mode in ['long','visual']:e['reset']['delay_ms']=60000
        if mode=='variant':e['reset'].update(pursuit_radius=400,delay_ms=30000)
        if mode=='zero-credit':
            t=copy.deepcopy(p['additional_templates']['verification-score-elite']);t.update(kill_credit=False,kill_score=0,drop_table=copy.deepcopy(p['drop_table']));p['additional_templates']['no-credit']=t;e['monsters'][0]['template_id']='no-credit'
        if mode=='nonreset':
            other=copy.deepcopy(e);other.update(id='other',trigger={'center':[1200,400],'radius':100},monsters=[{'id':'c','template_id':p['template_id'],'position':[1200,400]},{'id':'d','template_id':p['template_id'],'position':[1260,400]}],required_monster_ids=['c','d'],reward_chest_ids=[]);other['reset']['pursuit_radius']=300;e.pop('reset');p['encounters'].append(other)
        if mode in ['waves','blocked']:
            a,b=e.pop('monsters');e.pop('required_monster_ids');e['waves']=[{'id':'first','delay_ms':60000 if mode=='blocked' else 0,'monsters':[a],'required_monster_ids':['a']},{'id':'second','delay_ms':60000,'monsters':[b],'required_monster_ids':['b']}]
        if mode=='old-shot':c['basic']['projectile_speed']=5;c['params']['player_projectile_ttl_ms']=180000
        if mode=='old-pending':c['skill']['windup_ms']=60000
        if mode=='abilities':
            original=json.loads((ROOT/'config/b3-13.json').read_text())['pve']['additional_templates'];boss=p['additional_templates']['verification-phase-boss'];boss['stats']['hp']=1000;boss['heavy']=copy.deepcopy(original['verification-phase-boss']['heavy']);boss['heavy'].update(range=500,windup_ms=60000,travel_ms=30000,cost=20)
            support=p['additional_templates']['verification-forest-support'];support['support']=copy.deepcopy(original['verification-forest-support']['support']);support['support'].update(range=500,include_self=False,windup_ms=60000,ticks=3,tick_interval_ms=30000,interrupt_window_ms=120000,cooldown_ms=180000)
            e['monsters'].append({'id':'support','template_id':'verification-forest-support','position':[750,400]});e['required_monster_ids'].append('support')
        path.write_text(json.dumps(c))
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{'b1':'b1.json','b3-13':'b3-13.json'},'archives':[]}));return manifest

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--case',default='');args=parser.parse_args();OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
    subprocess.run(['python3','scripts/export_b3_13.py','--check'],cwd=ROOT,check=True);native=subprocess.run(['cargo','test','--locked','--all-features'],cwd=ROOT,text=True,capture_output=True,check=True);all_tests=re.findall(r'^test (\S+) \.\.\. ok$',native.stdout,re.M);tests=[t for t in all_tests if '::reset_tests::' in t or t.startswith('verification::reset::') or t.startswith('config::catalog::tests::reset_')];assert len(tests)==48 and len(all_tests)>=579,(len(tests),len(all_tests));subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=hashlib.sha256(BINARY.read_bytes()).hexdigest()
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def snap(page,host):return current(page,host)['snapshot']
        def ev(v,kind):return [e for e in v['events'] if e['kind']==kind]
        def encounter(v,id='team'):return next(e for e in v['snapshot']['pve']['encounters'] if e['id']==id)
        def monster(v,id):return next((m for m in v['snapshot']['pve']['monsters'] if m['id']==id),None)
        def frozen(v):return next(r for r in v['io']['requests'] if r['body']['kind']=='commit_settlement')
        def done(page,host):return wait(lambda:v if (v:=current(page,host)) and v['phase']=='closed' and v['settlement'] and not v['ledger']['pending'] else None,50)
        def check(v):
            b=frozen(v)['body'];l=v['ledger'];used={r['item_id']:r['quantity'] for r in b['map_progress']['consumed']};assert b['carried']+b['looted']==b['kept']+b['dropped']+used.get(b['item_id'],0);assert all(i['looted']==i['kept']+i['dropped']+used.get(i['item_id'],0) for i in b.get('items',[]));assert l['warehouse']==100-b['carried']+b['kept'] and l['cultivation']==b['cultivation'] and l['items']=={i['item_id']:i['kept'] for i in b.get('items',[])};assert l['admissions']==l['settlements']==1 and l['manual']==0 and len(ev(v,'settlement_applied'))==1
            s=b['reset_progress'];assert s['actual_kills']==b['scoring']['kills']+sum(d['deaths']-1 for e in s['encounters'] for d in e['defeats'])
        serial=0
        def send(page,host,action):
            nonlocal serial
            for _ in range(4):
                v=current(page,host);serial+=1;r=page.request.post(host.url+f"/api/sessions/{v['id']}/input",data={'connection':v['connection'],'sequence':v['sequence']+1,'requestId':f'reset-{serial}','action':action})
                if r.ok or r.json().get('error')!='STALE_SEQUENCE':return r
            raise AssertionError(r.text())
        def conf(page,host):return page.request.get(host.url+'/api/config?scenario=b3-13').json()
        def begin(page):page.locator('#scenario').select_option('b3-13');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
        def navigate(page,host,point):assert send(page,host,{'kind':'navigate','destination':point}).ok;wait(lambda:math.dist(snap(page,host)['position'],point)<8);assert send(page,host,{'kind':'stop'}).ok
        def attack(page,host):
            wait(lambda:(s:=snap(page,host))['logical_ms']>=s['basic_ready_at_ms']);v=current(page,host);target=v['snapshot']['target'];assert target;before=monster(v,target['id'])['hp'];assert send(page,host,{'kind':'attack'}).ok;wait(lambda:(m:=monster(current(page,host),target['id'])) is None or m['hp']<before);return target['id']
        def depart(page,host):navigate(page,host,[400,900]);return wait(lambda:r if (r:=encounter(current(page,host))['reset'])['reset_at_ms'] else None)
        def new_round(page,host):return wait(lambda:e if (e:=encounter(current(page,host)))['reset']['round']>=2 else None)
        def reenter(page,host):navigate(page,host,[400,400]);wait(lambda:encounter(current(page,host))['phase'] in ['active','waiting']);return current(page,host)
        def close(page,host):v=current(page,host);assert page.request.post(host.url+f"/api/sessions/{v['id']}/close",data={'connection':v['connection']}).ok;return done(page,host)
        def pickup(page,host):
            for _ in range(100):
                v=current(page,host)
                if not v['snapshot']['resources']:return
                r=min(v['snapshot']['resources'],key=lambda r:math.dist(r['position'],v['snapshot']['position']));assert send(page,host,{'kind':'navigate','destination':r['position']}).ok
                wait(lambda:math.dist(snap(page,host)['position'],r['position'])<=conf(page,host)['params']['pickup_range']);assert send(page,host,{'kind':'stop'}).ok;v=current(page,host)
                if any(x['id']==r['id'] for x in v['snapshot']['resources']):
                    response=send(page,host,{'kind':'pickup','resource_id':r['id'],'expected_revision':v['snapshot']['inventory_revision']});assert response.ok or response.json().get('error')=='NO_RESOURCE';wait(lambda:not any(x['id']==r['id'] for x in snap(page,host)['resources']))
            raise AssertionError('pickup budget exceeded')
        def scenario(name,work,mode='flat',point='',mobile=False,unfrozen=False):
            if args.case and not any(part in name for part in args.case.split(',')):return
            with tempfile.TemporaryDirectory(prefix='round-reset-') as directory:
                host=Host(directory,package(directory,mode));context=browser.new_context(viewport={'width':390,'height':844} if mobile else {'width':1440,'height':1100},is_mobile=mobile,has_touch=mobile);page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                try:
                    host.start(point,'commit_settlement' if point else '');page.goto(host.url);work(host,page,context);v=current(page,host)
                    if not unfrozen:check(v)
                    page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed'&&document.querySelector('#world-live').textContent==='0'",timeout=30000);assert not errors,errors;results[name]={'ledger':v['ledger'],'snapshot':v.get('snapshot'),'events':v['events'],'configHash':v['configHash'],'settlementRequest':frozen(v) if not unfrozen else None};print(name+': passed',flush=True)
                except Exception:
                    try:page.screenshot(path=str(OUTPUT/'failure.png'),full_page=True);(OUTPUT/'failure.json').write_text(json.dumps(current(page,host),ensure_ascii=False,indent=2))
                    except Exception:pass
                    host.error_log.seek(0);print(host.error_log.read(),flush=True);raise
                finally:context.close();host.stop();host.error_log.close()
        def approach_encounter(page,host,id):
            c=next(e for e in conf(page,host)['pve']['encounters'] if e['id']==id);assert send(page,host,{'kind':'navigate','destination':c['trigger']['center']}).ok;wait(lambda:encounter(current(page,host),id)['phase']!='dormant');assert send(page,host,{'kind':'stop'}).ok
        def fight_one(page,host,id):
            v=current(page,host);m=monster(v,id);c=conf(page,host)
            if not m or not m['hp']:return id
            # Navigate close enough to avoid firing across a terrain corner.
            assert send(page,host,{'kind':'navigate','destination':m['position']}).ok
            wait(lambda:(m:=monster(current(page,host),id)) is None or not m['hp'] or math.dist(snap(page,host)['position'],m['position'])<min(c['basic']['range']*.8,(c['params']['player_radius']+m['radius'])*2))
            assert send(page,host,{'kind':'stop'}).ok
            wait(lambda:(s:=snap(page,host))['logical_ms']>=s['basic_ready_at_ms'])
            before=len(ev(current(page,host),'action_committed'));r=send(page,host,{'kind':'attack'});assert r.ok or (r.json().get('error')=='NO_RESOURCE' and not monster(current(page,host),id)['hp'])
            if r.ok:wait(lambda:len(ev(current(page,host),'action_committed'))>before)
            page.wait_for_timeout(100)
            return id
        def finish_group(page,host,id):
            for _ in range(100):
                v=current(page,host);e=encounter(v,id)
                if e['phase']=='completed':return
                alive=[m for m in v['snapshot']['pve']['monsters'] if m['encounter_id']==id and m['hp']>0]
                if alive:fight_one(page,host,min(alive,key=lambda m:math.dist(m['position'],v['snapshot']['position']))['id'])
                else:wait(lambda:(e:=encounter(current(page,host),id))['phase']=='completed' or any(m['encounter_id']==id and m['hp']>0 for m in snap(page,host)['pve']['monsters']))
            raise AssertionError('combat budget exceeded')
        def source_exit(page,host,eid):
            c=conf(page,host);e=next(e for e in c['pve']['encounters'] if e['id']==eid);v=current(page,host);centre=e['trigger']['center'];a=math.atan2(c['spawn'][1]-centre[1],c['spawn'][0]-centre[0]);radius=e['reset']['pursuit_radius']+2*c['pve']['cell_size']+c['params']['player_radius']
            for i in range(8):
                angle=a+i*math.pi/4;point=[centre[0]+radius*math.cos(angle),centre[1]+radius*math.sin(angle)];r=send(page,host,{'kind':'navigate','destination':point})
                if not r.ok:continue
                try:wait(lambda:encounter(current(page,host),eid)['reset']['reset_at_ms'] is not None,3);assert send(page,host,{'kind':'stop'}).ok;return
                except AssertionError:pass
            raise AssertionError('No reachable retreat direction')
        def source_route(page,host,retry=True,death=False,terminal=True):
            begin(page)
            for id in ['wetland','ruins','sanctum']:
                approach_encounter(page,host,id)
                if id=='sanctum' and retry:
                    wait(lambda:any(m['encounter_id']==id and m['hp']>0 for m in snap(page,host)['pve']['monsters']))
                    while monster(current(page,host),'sanctum-guard-1')['hp']>0:fight_one(page,host,'sanctum-guard-1')
                    pickup(page,host);wait(lambda:monster(current(page,host),'sanctum-boss-1') is not None and monster(current(page,host),'sanctum-boss-1')['hp']>0)
                    while monster(current(page,host),'sanctum-boss-1')['boss']['phase_index']<2:fight_one(page,host,'sanctum-boss-1')
                    before=current(page,host);inventory=before['snapshot']['inventory_quantity'];epoch=monster(before,'sanctum-boss-1')['boss']['epoch'];source_exit(page,host,id);wait(lambda:encounter(current(page,host),id)['reset']['round']==2);v=current(page,host);assert encounter(v,'wetland')['phase']==encounter(v,'ruins')['phase']=='completed' and v['snapshot']['inventory_quantity']==inventory
                    # Recover through ordinary out-of-combat regeneration before the second fight.
                    wait(lambda: (snapshot := snap(page, host))['hp'] == snapshot['max_hp'])
                    approach_encounter(page,host,id)
                    wait(lambda:monster(current(page,host),'sanctum-guard-1') is not None);assert monster(current(page,host),'sanctum-guard-1')['hp']>0
                    # Finish the first wave, then inspect the fresh final-wave members.
                    while monster(current(page,host),'sanctum-guard-1')['hp']>0:fight_one(page,host,'sanctum-guard-1')
                    wait(lambda:monster(current(page,host),'sanctum-boss-1') is not None and monster(current(page,host),'sanctum-boss-1')['hp']>0);boss=monster(current(page,host),'sanctum-boss-1');assert boss['hp']==boss['max_hp'] and boss['boss']['phase_index']==1 and boss['boss']['epoch']==epoch+1;support=monster(current(page,host),'sanctum-support-1');assert support['spirit']==100 and support['support']['remaining_casts']==4
                finish_group(page,host,id);pickup(page,host)
            v=current(page,host);c=conf(page,host);goal=v['snapshot']['pve']['map_objective'];chest=next(ch for ch in v['snapshot']['pve']['chests'] if ch['id']==goal['reward_chest_id']);navigate(page,host,chest['position']);v=current(page,host);assert send(page,host,{'kind':'attune_objective','objective_id':goal['id'],'expected_revision':v['snapshot']['inventory_revision']}).ok;wait(lambda:snap(page,host)['pve']['map_objective']['attuned']);chest=next(ch for ch in snap(page,host)['pve']['chests'] if ch['id']==goal['reward_chest_id']);assert send(page,host,{'kind':'open_chest','chest_id':chest['id'],'expected_generation':chest['generation']}).ok;wait(lambda:next(ch for ch in snap(page,host)['pve']['chests'] if ch['id']==chest['id'])['seal']['completed_layers']==chest['seal']['layers']);assert send(page,host,{'kind':'open_chest','chest_id':chest['id'],'expected_generation':chest['generation']}).ok;wait(lambda:snap(page,host)['pve']['map_objective']['reward_opened']);pickup(page,host)
            if death:return close(page,host)
            wait(lambda:(s:=snap(page,host))['logical_ms']>=s['active_at_ms']+c['extractionUnlockMs']);navigate(page,host,c['extractionPoint']);assert send(page,host,{'kind':'extract_start'}).ok
            return done(page,host) if terminal else None
        def normal(host,page,context):
            page.locator('#scenario').select_option('b3-13');page.locator('#verify').click();page.wait_for_function("['passed','failed'].includes(document.body.dataset.verification)",timeout=50000);assert page.locator('body').get_attribute('data-verification')=='passed',page.locator('#toast').inner_text();v=done(page,host);assert v['ledger']['cultivation']==172 and frozen(v)['body']['reset_progress']['actual_kills']==7
        scenario('source_one_click_seven_actual_seven_rewards_extract',normal,'source')
        def source_retry(host,page,context,death=False):
            v=source_route(page,host,True,death);b=frozen(v)['body'];assert b['reset_progress']['actual_kills']==8 and b['scoring']['kills']==7 and b['scoring']['kill_score']==200 and v['ledger']['cultivation']==(75 if death else 172)
            if not death:
                page.wait_for_function("document.querySelector('#score-status').textContent.includes('实际击杀 8')");page.screenshot(path=str(OUTPUT/'forest-retry.png'),full_page=True);before=v['ledger'];body=frozen(v)
                for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==body
        scenario('source_partial_boss_phase_reset_retry_full_forest_two_restarts',source_retry,'source');scenario('source_retry_death_keeps_unique_growth_and_consumption',lambda h,p,c:source_retry(h,p,c,True),'source')
        def repeat(host,page,context):begin(page);attack(page,host);depart(page,host);new_round(page,host);reenter(page,host);attack(page,host);attack(page,host);v=close(page,host);b=frozen(v)['body'];assert b['reset_progress']['actual_kills']==3 and b['scoring']['kills']==2 and b['scoring']['kill_score']==110
        scenario('public_partial_kill_reset_retry_three_actual_two_rewards',repeat)
        def cancel(host,page,context):begin(page);attack(page,host);before=encounter(current(page,host));depart(page,host);reenter(page,host);e=encounter(current(page,host));assert e['reset']['round']==1 and e['reset']['reset_at_ms'] is None and e['defeated_required']==before['defeated_required'];close(page,host)
        scenario('return_before_deadline_keeps_same_round_and_dead_member',cancel,'long')
        def repeat_loot(host,page,context,pick=False):
            begin(page);attack(page,host)
            if pick:
                pickup(page,host);assert dict(snap(page,host)['pve']['inventory']).get('huo-ling')==2
            depart(page,host);new_round(page,host);reenter(page,host);attack(page,host);v=current(page,host);assert v['snapshot']['pve']['scoring']['kills']==2 and v['snapshot']['pve']['scoring']['credited_kills']==1;assert not v['snapshot']['pve']['resource_items'];assert all(r['orb'] for r in v['snapshot']['resources']);close(page,host)
        scenario('expired_unpicked_first_loot_is_never_reissued',repeat_loot);scenario('picked_first_loot_persists_without_repeat_item_rewards',lambda h,p,c:repeat_loot(h,p,c,True))
        def no_credit(host,page,context):begin(page);attack(page,host);depart(page,host);new_round(page,host);reenter(page,host);attack(page,host);v=current(page,host);s=v['snapshot']['pve']['scoring'];assert (s['kills'],s['credited_kills'],s['kill_score'])==(2,0,0);close(page,host)
        scenario('zero_credit_member_has_no_repeat_growth',no_credit,'zero-credit')
        def legacy_group(host,page,context):begin(page);attack(page,host);navigate(page,host,[400,900]);at=snap(page,host)['logical_ms'];wait(lambda:snap(page,host)['logical_ms']>at+30000);assert monster(current(page,host),'a')['hp']==0 and not ev(current(page,host),'encounter_reset');close(page,host)
        scenario('nonreset_group_keeps_prior_one_shot_behavior',legacy_group,'nonreset')
        def waves(host,page,context):begin(page);wait(lambda:monster(current(page,host),'a') is not None);attack(page,host);assert encounter(current(page,host))['phase']=='waiting';depart(page,host);new_round(page,host);reenter(page,host);v=current(page,host);assert encounter(v)['wave']['index']==1 and monster(v,'a')['hp']==80 and monster(v,'b') is None;close(page,host)
        scenario('waiting_later_wave_resets_to_first_wave',waves,'waves')
        def blocked(host,page,context):begin(page);navigate(page,host,[600,400]);wait(lambda:encounter(current(page,host))['phase']=='spawning');assert not snap(page,host)['pve']['monsters'];depart(page,host);new_round(page,host);reenter(page,host);v=current(page,host);assert encounter(v)['wave']['index']==1 and encounter(v)['phase']=='waiting';close(page,host)
        scenario('blocked_spawn_reset_and_first_wave_retry_remain_atomic',blocked,'blocked')
        def abilities(host,page,context):begin(page);attack(page,host);attack(page,host);attack(page,host);wait(lambda:ev(current(page,host),'support_pulse'));v=current(page,host);assert monster(v,'b')['heavy']['cast'] and monster(v,'b')['spirit']==80 and monster(v,'support')['spirit']==80;depart(page,host);new_round(page,host);v=current(page,host);assert ev(v,'support_cancelled') and ev(v,'heavy_cancelled') and len(ev(v,'support_pulse'))==1;reenter(page,host);v=current(page,host);assert monster(v,'support')['support']['remaining_casts']==4 and monster(v,'support')['spirit']==100 and monster(v,'b')['spirit']==100;close(page,host)
        scenario('group_reset_clears_heavy_support_and_restores_paid_budgets',abilities,'abilities')
        def old_shot(host,page,context,pending=False):
            begin(page);accepted=snap(page,host)['logical_ms'];action={'kind':'cast','skill_id':conf(page,host)['skill']['id'],'direction':[1,0]} if pending else {'kind':'attack'};assert send(page,host,action).ok;depart(page,host);new_round(page,host);reenter(page,host);wait(lambda:snap(page,host)['logical_ms']>accepted+(65000 if pending else 50000));assert all(m['hp']==80 for m in snap(page,host)['pve']['monsters']);assert ev(current(page,host),'action_committed');close(page,host)
        scenario('old_inflight_player_projectile_cannot_hit_new_round',old_shot,'old-shot');scenario('old_uncommitted_player_skill_keeps_original_round',lambda h,p,c:old_shot(h,p,c,True),'old-pending')
        def refresh(host,page,context,out=False):
            begin(page);attack(page,host)
            if out:depart(page,host)
            before=current(page,host);r=encounter(before)['reset'];page.reload();active(page);v=current(page,host);assert v['id']==before['id'] and v['connection']['generation']>before['connection']['generation'];assert encounter(v)['reset']['round']==1 and encounter(v)['reset']['reset_at_ms']==r['reset_at_ms'];assert monster(v,'a')['hp']==0
            if out:new_round(page,host)
            close(page,host)
        scenario('refresh_inside_keeps_current_round_and_partial_rewards',refresh,'long');scenario('refresh_outside_keeps_original_countdown_deadline',lambda h,p,c:refresh(h,p,c,True),'long')
        def pause(host,page,context):begin(page);attack(page,host);depart(page,host);assert send(page,host,{'kind':'background'}).ok;wait(lambda:snap(page,host)['protected']);assert encounter(current(page,host))['reset']['round']==1;new_round(page,host);assert snap(page,host)['protected'];close(page,host)
        scenario('background_outside_does_not_pause_logical_reset_timer',pause,'long')
        def close_count(host,page,context):begin(page);attack(page,host);depart(page,host);v=close(page,host);assert frozen(v)['body']['reset_progress']['encounters'][0]['round']==1 and ev(v,'encounter_reset_cancelled')
        scenario('close_during_countdown_freezes_existing_first_reward_once',close_count,'long')
        def dormant(host,page,context):begin(page);attack(page,host);depart(page,host);new_round(page,host);v=close(page,host);s=frozen(v)['body']['reset_progress'];assert s['actual_kills']==1 and s['encounters'][0]['round']==2
        scenario('close_after_reset_dormant_preserves_prior_rewards',dormant)
        def completed(host,page,context):begin(page);attack(page,host);attack(page,host);navigate(page,host,[400,900]);at=snap(page,host)['logical_ms'];wait(lambda:snap(page,host)['logical_ms']>at+30000);e=encounter(current(page,host));assert e['phase']=='completed' and e['reset']['round']==1 and not ev(current(page,host),'encounter_reset');close(page,host)
        scenario('completed_encounter_never_resets_or_reissues_reward',completed)
        def variant(host,page,context):begin(page);r=depart(page,host);assert r['delay_ms']==30000 and r['pursuit_radius']==400;new_round(page,host);close(page,host)
        scenario('configured_radius_and_delay_drive_same_mechanism',variant,'variant')
        def mobile(host,page,context):begin(page);page.locator('#attack').click();wait(lambda:monster(current(page,host),'a')['hp']==0);page.locator('#navigate-reset').click();new_round(page,host);page.locator('#navigate-encounter').click();wait(lambda:monster(current(page,host),'a') is not None and snap(page,host)['target']['id']=='a');page.locator('#attack').click();wait(lambda:monster(current(page,host),'a')['hp']==0);assert page.evaluate('document.documentElement.scrollWidth<=innerWidth');page.screenshot(path=str(OUTPUT/'mobile.png'),full_page=True);close(page,host)
        scenario('mobile_public_leave_return_and_attack_buttons',mobile,mobile=True)
        def desktop(host,page,context):begin(page);attack(page,host);page.reload();active(page);page.locator('#navigate-reset').click();wait(lambda:encounter(current(page,host))['reset']['reset_at_ms']);page.wait_for_function("document.querySelector('#reset-status').textContent.includes('脱战')");page.screenshot(path=str(OUTPUT/'countdown.png'),full_page=True);new_round(page,host);page.wait_for_function("document.querySelector('#reset-status').textContent.includes('第2轮')");page.screenshot(path=str(OUTPUT/'round2.png'),full_page=True);close(page,host)
        scenario('desktop_authoritative_range_countdown_and_new_round',desktop,'visual')
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,context):
                source_route(page,host,True,terminal=False);deadline=time.monotonic()+50
                while host.process.poll() is None and time.monotonic()<deadline:page.wait_for_timeout(20)
                host.killed();page.goto('about:blank');body=next(r for _,r in host.state()['requests'] if r['body']['kind']=='commit_settlement');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']['cultivation']==172 and frozen(v)['body']==body['body'] and body['body']['reset_progress']['actual_kills']==8
            scenario('source_retry_assets_'+point,crash,'source',point=point)
        def lost(host,page,context):
            page.locator('#scenario').select_option('b3-13');page.locator('#fault').select_option('settlement_lost_receipt');v=source_route(page,host,True);assert v['ledger']['cultivation']==172 and ev(v,'io_timed_out') and frozen(v)['body']['reset_progress']['actual_kills']==8
        scenario('lost_receipt_reconciles_original_retry_proof_once',lost,'source')
        def version(host,page,context):
            page.locator('#scenario').select_option('b3-13');page.locator('#fault').select_option('refused');source_route(page,host,True,terminal=False);page.locator('#reconcile').wait_for(state='visible',timeout=50000);v=current(page,host);body=frozen(v);history=ev(v,'encounter_reset');host.kill();page.goto('about:blank');path=Path(host.manifest).parent/'b3-13.json';c=json.loads(path.read_text());c['id']='b3-13-encounter-reset-v2'
            for e in c['pve']['encounters']:e['reset'].update(delay_ms=10000,pursuit_radius=e['reset']['pursuit_radius']+100)
            path.write_text(json.dumps(c));host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible');v=current(page,host);assert frozen(v)==body and ev(v,'encounter_reset')==history;page.locator('#reconcile').click();v=done(page,host);before=v['ledger']
            for _ in range(2):host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=done(page,host);assert v['ledger']==before and frozen(v)==body and ev(v,'encounter_reset')==history
        scenario('reset_policy_upgrade_keeps_old_manual_body_two_restarts',version,'source')
        def unfrozen(host,page,context):begin(page);attack(page,host);depart(page,host);new_round(page,host);host.kill();page.goto('about:blank');host.start();page.goto(host.url);v=wait(lambda:v if (v:=current(page,host)) and v['phase']=='closed' and not v['ledger']['pending'] else None);assert v['ledger']['warehouse']==100 and v['ledger']['compensations']==1 and v['ledger']['settlements']==0 and not v['ledger']['items']
        scenario('unfrozen_new_round_crash_refunds_entry_without_battle_replay',unfrozen,unfrozen=True)
        browser.close()
    assert sources==hashes(),'Sources changed during verification';assert binary==hashlib.sha256(BINARY.read_bytes()).hexdigest();assert results and not errors
    if not args.case:assert len(results)==28,len(results)
    report='verification-development.json' if args.case else 'verification.json';(OUTPUT/report).write_text(json.dumps({'newRustTests':tests,'rustTotal':len(all_tests),'results':results,'pageErrors':errors,'sourceHashes':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n');print(json.dumps({'newRustTests':len(tests),'browserAndRestartChecks':len(results),'rustTotal':len(all_tests),'pageErrors':len(errors),'report':report}))
if __name__=='__main__':main()
