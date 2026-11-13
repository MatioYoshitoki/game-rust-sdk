"""CONFIG-1: one immutable binary, varied data, multi-stack assets and versioned recovery."""
import copy,hashlib,json,re,shutil,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
ROOT=Path(__file__).resolve().parents[1]
OUTPUT=ROOT/'artifacts/parameters'
BINARY=ROOT/'target/debug/kernel-host'

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def package(directory):
    root=Path(directory)/'config';root.mkdir()
    for alias in ['b1','b3-1','b3-2']:shutil.copyfile(ROOT/'config'/(alias+'.json'),root/(alias+'.json'))
    a=json.loads((root/'b3-2.json').read_text());a['id']='config-one-beast-A';a['skill'].update(id='a-bolt',cost=7,cooldown_ms=2100,windup_ms=90)
    a.update(carried_quantity=2,stack_limit=2,inventory_capacity=12,extraction_unlock_ms=24000,extraction_channel_ms=900,terminal_ms=60000,
             kill_score=13,extraction_score=17,cultivation_coefficient=1.25,death_multiplier=.8)
    p=a['params'];p['initial_player'].update(hp_points=1500,spirit_points=200,atk_points=120,def_points=15)
    p.update(player_radius=12,entry_years=3,resource_ttl_ms=45000);p['movement'].update(base=240,bonus=0)
    p['regen'].update(interval_ms=200,near_radius=60,far_ratio=.02)
    p['runtime'].update(io_timeout_ms=750,drain_timeout_ms=2500,max_attempts=2,publish_batched_ms=90)
    v=a['pve'];v['template_id']='configured-beast';v['monster_positions']=v['monster_positions'][:1];v['chest_spawns']=v['chest_spawns'][:1]
    v['stats'].update(hp=60,spirit=0,atk=2,spd=0,crit=0,dodge=0);v['combat'].update(radius=10,cast_chance=0)
    v['items']['xuan-tie']['stackLimit']=2;v['items']['amber']={'name':'琥珀A','stackLimit':3}
    v['drop_table']={'guaranteed':[{'itemId':'amber','quantityMin':7,'quantityMax':7}],'random':[{'itemId':'huo-ling','quantityMin':1,'quantityMax':1,'chance':0}]}
    t=v['treasure_rules'];t['template'].update(id='configured-cache',name='配置储物袋',respawn_delay_ms=60000)
    t['loot']={'guaranteed':[{'itemId':'amber','quantityMin':7,'quantityMax':7}],
               'random':[{'itemId':'xuan-tie','quantityMin':4,'quantityMax':4,'chance':1}]}
    b=copy.deepcopy(a);b.update(id='config-five-beasts-B',carried_quantity=3,inventory_capacity=32,cultivation_coefficient=2.0,
        extraction_unlock_ms=48000,extraction_channel_ms=1500,terminal_ms=160000)
    b['skill'].update(id='b-projectile',cost=3,cooldown_ms=450)
    b['params']['initial_player'].update(hp_points=900,spirit_points=70,atk_points=100)
    b['params'].update(resource_ttl_ms=120000);b['params']['movement'].update(base=210,bonus=10)
    b['params']['death'].update(slot_ratio=.75,quantity_ratio=.75,minimum_quantity=0)
    b['pve']['items']['amber']['name']='矿晶B';b['pve']['chest_spawns']=json.loads((root/'b3-2.json').read_text())['pve']['chest_spawns']
    b['pve']['treasure_rules']['template']['respawn_delay_ms']=120000
    # Distinct clear points around the original entry; validate their paths with the native loader.
    m=b['pve'];cells=[index for index,count in m['cells_rle'] for _ in range(count)];candidates=[]
    for y in range(1,m['height']-1):
        for x in range(1,m['width']-1):
            pos=[x*32+16,y*32+16];d=sum((v-w)**2 for v,w in zip(pos,b['spawn']))**.5
            if 160<=d<=850 and all(m['palette'][cells[j*m['width']+i]]['walkable'] for j in range(y-1,y+2) for i in range(x-1,x+2)):
                candidates.append((d,y,x,pos))
    positions=[]
    for distance in [180,340,500,660,820]:
        for d,y,x,pos in sorted(candidates,key=lambda row:(abs(row[0]-distance),row[1],row[2])):
            if all(sum((a-b)**2 for a,b in zip(pos,old))>=150**2 for old in positions):positions.append(pos);break
    b['pve']['monster_positions']=positions;assert len(positions)==5
    g=copy.deepcopy(a);g.update(id='configured-cell-scale',spawn=[400,400],extraction_point=[600,400],geometry={'min':[0,0],'max':[1024,1024],'walls':[]})
    g['pve'].update(map_id='configured-flat-map',width=16,height=16,cell_size=64,cells_rle=[[0,256]],monster_positions=[[520,400]])
    g['pve']['chest_spawns'][0]['position']=[400,600]
    for name,value in [('alpha',a),('beta',b),('gamma',g)]: (root/(name+'.json')).write_text(json.dumps(value))
    shutil.copyfile(ROOT/'config/versions/b5ee821f65f9d58c36eba63617fe5a5c869b91a75f95b516370513164cedb159.json',root/'legacy.json')
    aliases=['b1','b3-1','b3-2','alpha','beta','gamma','legacy']
    manifest=root/'catalog.json';manifest.write_text(json.dumps({'schema_version':1,'active':{name:name+'.json' for name in aliases},'archives':[]}))
    return manifest,{'alpha':a,'beta':b,'gamma':g}

def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};validation={};errors=[]
    rust=subprocess.run(['cargo','test','--locked','--all-features','world::pve::parameter_tests'],cwd=ROOT,capture_output=True,text=True,check=True)
    tests=re.findall(r'^test (world::pve::parameter_tests::\S+) \.\.\. ok$',rust.stdout,re.M);assert len(tests)==16,tests
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True);binary=sha(BINARY)
    with tempfile.TemporaryDirectory(prefix='config-validation-') as directory:
        manifest,profiles=package(directory);path=manifest.parent/'alpha.json';original=path.read_text()
        cases=[('missing_params',lambda c:c.pop('params'),'CONFIG_SCHEMA'),
          ('unknown_mechanism',lambda c:c['skill'].update(mechanism='arbitrary_script'),'CONFIG_SCHEMA'),
          ('unknown_nested_field',lambda c:c['params']['regen'].update(heal_expression='anything'),'CONFIG_SCHEMA'),
          ('bad_probability',lambda c:c['pve']['treasure_rules']['loot']['random'][0].update(chance=1.1),'CONFIG_LOOT_BOUNDS'),
          ('negative_probability',lambda c:c['pve']['combat'].update(cast_chance=-.1),'CONFIG_MONSTER_ABILITY_BOUNDS'),
          ('reversed_quantity',lambda c:c['pve']['drop_table']['guaranteed'][0].update(quantityMin=9,quantityMax=1),'CONFIG_LOOT_BOUNDS'),
          ('missing_item_reference',lambda c:c['pve']['drop_table']['guaranteed'][0].update(itemId='not-registered'),'CONFIG_ITEM_REFERENCE'),
          ('zero_stack_limit',lambda c:c['pve']['items']['amber'].update(stackLimit=0),'CONFIG_ITEM_REFERENCE'),
          ('zero_hp',lambda c:c['params']['initial_player'].update(hp_points=0),'CONFIG_PLAYER_BOUNDS'),
          ('invalid_step',lambda c:c.update(step_ms=0),'CONFIG_RUNTIME_BOUNDS'),
          ('retry_budget',lambda c:c['params']['runtime'].update(max_attempts=4),'CONFIG_RUNTIME_BOUNDS'),
          ('negative_loss',lambda c:c['params']['death'].update(quantity_ratio=-.1),'CONFIG_PARAMETER_BOUNDS'),
          ('late_extraction',lambda c:c.update(extraction_unlock_ms=60001),'CONFIG_COMBAT_BOUNDS'),
          ('too_many_entities',lambda c:c['pve'].update(monster_positions=[c['spawn']]*33),'CONFIG_CONTENT_BOUNDS'),
          ('unknown_stat',lambda c:c['pve']['stats'].update(hit=1),'CONFIG_STATS'),
          ('growth_overflow',lambda c:c.update(kill_score=4294967295,cultivation_coefficient=1000),'CONFIG_GROWTH_OVERFLOW'),
          ('unsupported_container',lambda c:c['pve']['treasure_rules']['template'].update(mechanism='guarded_seal'),'CONFIG_SCHEMA')]
        for name,change,code in cases:
            c=copy.deepcopy(profiles['alpha']);change(c);path.write_text(json.dumps(c))
            reply=subprocess.run([str(BINARY),'--configs',str(manifest),'--catalog'],capture_output=True,text=True)
            assert reply.returncode and code in reply.stderr,(name,reply.stderr)
            validation[name]=code
        path.write_text(original)
        desc=json.loads(subprocess.check_output([str(BINARY),'--configs',str(manifest),'--catalog'],text=True));assert len(desc['active'])==7
        validation['valid_varied_and_legacy_profiles']='accepted'
    with sync_playwright() as pw:
        browser=pw.chromium.launch(headless=True)
        def current(page,host):return info(page,host.url)
        def frozen(value):return next(r for r in value['io']['requests'] if r['body']['kind']=='commit_settlement')
        def check(value):
            body=frozen(value)['body'];ledger=value['ledger'];assert body['carried']+body['looted']==body['kept']+body['dropped']
            for item in body.get('items',[]):assert item['looted']==item['kept']+item['dropped']
            assert ledger['warehouse']==100-body['carried']+body['kept'] and ledger['items']=={i['item_id']:i['kept'] for i in body.get('items',[])},value
            assert ledger['admissions']==ledger['settlements']==1 and not ledger['pending'] and ledger['manual']==0,value
            assert ledger['cultivation']==body['cultivation'] and value['phase']=='closed'
            assert sum(e['kind']=='settlement_applied' for e in value['events'])==1
        def terminal(page,host):
            page.wait_for_function("document.querySelector('#phase').dataset.phase==='closed'&&document.querySelector('#pending').textContent==='0'",timeout=30000)
            return wait(lambda:v if (v:=current(page,host))['settlement'] and not v['ledger']['pending'] else None)
        def automatic(page,alias='alpha',fault='none'):
            page.locator('#scenario').select_option(alias);page.locator('#fault').select_option(fault)
            with page.expect_response(lambda r:r.request.method=='POST' and r.url.endswith('/api/sessions')):
                page.locator('#verify').click()
        def collected(value,count):
            s=value.get('snapshot');return s and s.get('pve') and s['pve']['kills']==count and all(c['opened'] for c in s['pve']['chests']) and not s['resources']
        def scenario(name,work,crash='',zero=False):
            with tempfile.TemporaryDirectory(prefix='config-runtime-') as directory:
                manifest,profiles=package(directory)
                if zero:
                    profiles['alpha']['cultivation_coefficient']=0;(manifest.parent/'alpha.json').write_text(json.dumps(profiles['alpha']))
                host=Host(directory,manifest);context=browser.new_context(viewport={'width':1440,'height':1100});page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                try:
                    host.start(crash,'commit_settlement' if crash else '');page.goto(host.url)
                    page.wait_for_function("document.querySelector('#scenario option[value=alpha]')")
                    work(host,page,profiles)
                    value=current(page,host);check(value);assert not errors,errors
                    assert page.locator('#world-live').inner_text()=='0'
                    results[name]={'ledger':value['ledger'],'configHash':value['configHash'],'snapshot':value['snapshot'],'events':value['events'],'settlementRequest':frozen(value)}
                    print(name+': passed',flush=True)
                except Exception:
                    page.screenshot(path=str(OUTPUT/'failure.png'),full_page=True)
                    (OUTPUT/'failure.json').write_text(json.dumps(current(page,host),ensure_ascii=False,indent=2));host.error_log.seek(0);print(host.error_log.read(),flush=True);raise
                finally:context.close();host.stop();host.error_log.close()
        for alias,count,growth,quantity in [('alpha',1,37,14),('beta',5,164,49),('gamma',1,37,14)]:
            def extract(host,page,profiles,alias=alias,count=count,growth=growth,quantity=quantity):
                automatic(page,alias);value=terminal(page,host);check(value)
                assert value['snapshot']['pve']['kills']==count and value['ledger']['cultivation']==growth
                assert value['ledger']['items']['amber']==quantity and value['ledger']['entryYears']==3
                assert value['snapshot']['max_hp']==profiles[alias]['params']['initial_player']['hp_points']
                assert len(value['snapshot']['pve']['monsters'])==count
                assert page.locator('#hp-max').inner_text()==f"/ {value['snapshot']['max_hp']}"
                assert f'/{count}' in page.locator('#pve-inventory').inner_text()
                assert 'done' in page.locator('[data-step=combat]').get_attribute('class')
                assert 'done' in page.locator('[data-step=loot]').get_attribute('class')
                page.wait_for_function("document.body.dataset.verification==='passed'",timeout=10000)
                page.screenshot(path=str(OUTPUT/(alias+'-extraction.png')),full_page=True)
                before=value['ledger'];host.kill();page.goto('about:blank');host.start();page.goto(host.url)
                value=terminal(page,host);assert value['ledger']==before;check(value)
            scenario(alias+'_numeric_content_and_cell_scale_without_recompile',extract)
        def skill(host,page,profiles):
            page.locator('#scenario').select_option('alpha');page.locator('#clock').select_option('fast');page.locator('#start').click();active(page)
            page.wait_for_function("document.querySelector('#cast-detail').textContent.includes('7')")
            with page.expect_request(lambda r:r.url.endswith('/input') and r.method=='POST' and r.post_data_json.get('action',{}).get('kind')=='cast') as request:page.locator('#cast').click()
            assert request.value.post_data_json['action']['skill_id']=='a-bolt'
            wait(lambda:any(e['kind']=='action_committed' and e.get('spirit_cost')==7 for e in current(page,host)['events']))
            page.locator('#close').click();check(terminal(page,host))
        scenario('configured_skill_id_cost_and_ui',skill)
        def zero(host,page,profiles):
            automatic(page);value=terminal(page,host);check(value);assert value['ledger']['cultivation']==0 and value['ledger']['items']['amber']==14
            host.kill();page.goto('about:blank');host.start();page.goto(host.url);value=terminal(page,host);check(value)
            assert value['ledger']['cultivation']==0 and value['ledger']['items']['amber']==14
        scenario('zero_growth_with_real_loot_restores_without_kill_inference_error',zero,zero=True)
        for point in ['before_storage_commit','after_storage_commit']:
            def crash(host,page,profiles,point=point):
                automatic(page,'beta');host.killed();page.goto('about:blank');state=host.state()
                request=next(r for _,r in state['requests'] if r['body']['kind']=='commit_settlement')
                host.start();page.goto(host.url);value=terminal(page,host);check(value)
                assert value['ledger']['items']['amber']==49 and value['ledger']['cultivation']==164
                assert frozen(value)['body']==request['body'] and frozen(value)['operation_id']==request['operation_id']
            scenario('multi_stack_assets_'+point,crash,crash=point)
        def versioned_death(host,page,profiles):
            automatic(page,'beta','refused');wait(lambda:collected(current(page,host),5),timeout=30);page.locator('#close').click()
            page.locator('#reconcile').wait_for(state='visible',timeout=15000);before=current(page,host);request=frozen(before);old_hash=before['configHash']
            assert request['body']['outcome']=='death' and request['body']['items'][0]['looted']==49
            host.kill();page.goto('about:blank')
            path=Path(host.manifest).parent/'beta.json';new=json.loads(path.read_text());new['params']['death'].update(slot_ratio=0,quantity_ratio=0)
            new['pve']['items']['amber']['stackLimit']=20;new['cultivation_coefficient']=.5;path.write_text(json.dumps(new))
            host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=15000)
            value=current(page,host);assert value['configHash']==old_hash and frozen(value)==request
            current_config=page.request.get(host.url+'/api/config?scenario=beta').json()
            pinned=page.request.get(host.url+f"/api/config?sessionId={value['id']}").json()
            assert current_config['configHash']!=old_hash and pinned['configHash']==old_hash
            assert current_config['params']['death']['slot_ratio']==0 and pinned['params']['death']['slot_ratio']==.75
            page.locator('#reconcile').click();value=terminal(page,host);check(value)
            assert frozen(value)['body']==request['body'];assets=value['ledger']
            for _ in range(2):
                host.kill();page.goto('about:blank');host.start();page.goto(host.url);value=terminal(page,host);assert value['ledger']==assets
        scenario('new_death_and_stack_policy_does_not_reinterpret_old_frozen_assets',versioned_death)
        def legacy(host,page,profiles):
            automatic(page,'legacy','refused');page.locator('#reconcile').wait_for(state='visible',timeout=30000)
            before=current(page,host);request=frozen(before);assert before['configHash']=='b5ee821f65f9d58c36eba63617fe5a5c869b91a75f95b516370513164cedb159'
            host.kill();page.goto('about:blank');path=Path(host.manifest).parent/'legacy.json';new=copy.deepcopy(profiles['alpha']);new['id']=before['configId'];path.write_text(json.dumps(new))
            host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible',timeout=20000)
            assert current(page,host)['configHash']==before['configHash'] and frozen(current(page,host))==request
            page.locator('#reconcile').click();value=terminal(page,host);check(value);assert value['ledger']['cultivation']==45
        scenario('shipped_v1_obligation_recovers_under_v2_active_data',legacy)
        browser.close()
    assert binary==sha(BINARY),'binary changed during data-only updates'
    assert sources==hashes(),'source files changed during parameter verification'
    assert not errors,errors
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'CONFIG-1','rustTests':tests,'validation':validation,'results':results,
        'pageErrors':errors,'sourceSha256':sources,'binarySha256':binary},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'nativeParameterTests':len(tests),'validationChecks':len(validation),'browserAndRestartChecks':len(results)}),flush=True)
if __name__=='__main__':main()
