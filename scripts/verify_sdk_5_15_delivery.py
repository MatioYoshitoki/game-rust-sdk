"""Consume the SDK-5-15 archive away from the workspace, including old protocol boundaries."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile

from package_sdk import ROOT, digest, extract_crate
from verify_sdk_2 import verify_result

OUTPUT = ROOT/'artifacts/sdk-5-15/delivery'


def run(name, command, cwd, env=None):
    p = subprocess.run(command,cwd=cwd,env={**os.environ,**(env or {})},capture_output=True,text=True,timeout=900)
    (OUTPUT/(name+'.log')).write_text(p.stdout+p.stderr)
    assert p.returncode==0,(name,p.stderr[-3000:],p.stdout[-1500:])
    print(name+': passed',flush=True)
    return p.stdout


def balances(value, account):
    expected = dict(account['assets'])
    for op, receipt in value['requests'].items():
        body = receipt['body']
        if body['kind']!='commit_assets':continue
        # Only committed operations belong in the account ledger.
        tx = body['transaction']
        if value['counts']['admission']==0:continue
        if tx['kind']=='compensation' and not value['counts']['compensation']:continue
        if tx['kind'] in ['settlement','result_settlement'] and not value['counts']['settlement']:continue
        sign = -1 if tx['kind']=='admission' else 1
        for row in tx.get('debits',tx.get('credits',[])):
            expected[row['asset_id']]=expected.get(row['asset_id'],0)+sign*row['quantity']
    assert value['balances']==expected,(value['balances'],expected)


def main():
    report=json.loads((OUTPUT/'package.json').read_text())
    with tempfile.TemporaryDirectory(prefix='sdk-5-15-delivery-') as d:
        d=Path(d);bundle=d/'bundle'
        extract_crate(report['archive'],bundle,report['rootName'])
        run('independent-integrity',[sys.executable,'verify_bundle.py'],bundle)
        for p in bundle.rglob('*'):
            if p.is_file():assert str(ROOT).encode() not in p.read_bytes(),str(p)
        native_sources={str(p.relative_to(ROOT)):digest(p) for p in ROOT.glob('src/**/*.rs')}
        for p,h in native_sources.items():assert digest(bundle/'sdk'/p)==h,p
        own=d/'outside';own.mkdir()
        for kind in ['contacts','fields','ground-targets','unit-targets','self-skills','player-skills','healing-modifiers','healing','periodic','shields','attributes','stacks','statuses','legacy','assets','results']:
            shutil.copytree(bundle/'samples'/kind,own/kind)
        for variant in ['a','b']:
            folder=own/('repeat-'+variant);shutil.copytree(bundle/'samples/projectile-repeat',folder)
            cfg=json.loads((folder/'arena.json').read_text());bolt=cfg['skill'];own_skill=cfg['additional_skills'][0];cfg['id']='outside-node-repeat-'+variant;cfg['pve']['combat']['heal']['hp_threshold']=0
            if variant=='a':bolt.update(repeat_interval_ms=0,speed=500,radius=6,ttl_ms=4000)
            else:
                bolt.update(id='outside-node-repeat-lance',cost=14,repeat_interval_ms=600,speed=20,radius=42,ttl_ms=22000,on_hit_target_statuses=['outside-exposed'])
                own_skill.update(id='outside-node-repeat-self',cost=10,on_commit_self_statuses=['outside-ward','outside-renewal']);cfg['skill'],cfg['additional_skills'][0]=own_skill,bolt
                for dfn in cfg['pve']['status_definitions']:dfn['id']='outside-'+dfn['id']
            (folder/'arena.json').write_text(json.dumps(cfg))
        for variant in ['a','b']:
            folder=own/('piercing-'+variant);shutil.copytree(bundle/'samples/piercing',folder)
            cfg=json.loads((folder/'arena.json').read_text());bolt=cfg['skill'];own_skill=cfg['additional_skills'][0]
            cfg['id']='outside-node-piercing-'+variant;cfg['pve']['combat']['heal']['hp_threshold']=0
            bolt['max_hits']=2 if variant=='a' else 3
            if variant=='b':
                bolt.update(id='outside-node-lance',cost=14,damage_bps=12000,speed=500,ttl_ms=4000)
                own_skill.update(id='outside-node-self',cost=10);cfg['skill'],cfg['additional_skills'][0]=own_skill,bolt
            (folder/'arena.json').write_text(json.dumps(cfg))
        for variant in ['a','b']:
            folder=own/('contacts-'+variant);shutil.copytree(bundle/'samples/contacts',folder)
            cfg=json.loads((folder/'arena.json').read_text());account=json.loads((folder/'account.json').read_text());ground=cfg['skill'];own_skill=cfg['additional_skills'][0]
            cfg['id']='outside-node-contacts-'+variant;cfg['pve']['combat']['heal']['hp_threshold']=0
            ground['contact_policy']['repeat_interval_ms']=0 if variant=='a' else 1400
            if variant=='b':
                ground.update(id='outside-node-budget',cost=14,tick_interval_ms=700,duration_ms=2800)
                own_skill.update(id='outside-node-self',cost=10);cfg['skill'],cfg['additional_skills'][0]=own_skill,ground
            (folder/'arena.json').write_text(json.dumps(cfg));(folder/'account.json').write_text(json.dumps(account))
        # The old protocol-2 sample unlocks at 180 seconds; this reference player
        # has a 25-second wall-clock deadline. Tune only the copied outside data.
        old_cfg=own/'assets/arena.json';old_value=json.loads(old_cfg.read_text())
        old_value.update(extraction_unlock_ms=3000,extraction_channel_ms=500)
        old_cfg.write_text(json.dumps(old_value))
        cfg=own/'shields/arena.json';v=json.loads(cfg.read_text());v['id']='outside-delivery-shields'
        cfg.write_text(json.dumps(v))
        (own/'shields/catalog.json').write_text(json.dumps({'schema_version':1,'default_scenario':'outside',
                                                        'active':{'outside':'arena.json'},'archives':[]}))
        for p in [bundle/'sdk/config',bundle/'sdk/examples',bundle/'samples']:shutil.rmtree(p)
        invalid={'MATCH_CONFIG_MANIFEST':str(d/'no-defaults.json')}
        binary=str(bundle/'bin/kernel-host')
        info=json.loads(run('independent-info',[binary,'--sdk-info'],own,invalid))
        assert info['crateVersion']=='0.18.0' and info['mechanismVersions']['max']==33 and not info['clocks']['replay']
        assert info['playerSkills']['projectileRepeat']['minimumMechanismVersion']==33 and not info['playerSkills']['projectileRepeat']['catchUp']
        assert info['playerSkills']['piercingProjectile']['minimumMechanismVersion']==32 and info['playerSkills']['piercingProjectile']['collision']=='continuous_sweep'
        assert info['playerSkills']['contactPolicy']['minimumMechanismVersion']==31 and not info['playerSkills']['contactPolicy']['dodgeConsumes']
        assert info['playerSkills']['groundField']['minimumMechanismVersion']==30 and info['playerSkills']['groundField']['sourceInvalidPolicy']=='end'
        assert info['playerSkills']['groundCircle']['minimumMechanismVersion']==29 and info['playerSkills']['groundCircle']['maxPendingAreas']==2
        assert info['playerSkills']['unitProjectile']['minimumMechanismVersion']==28 and not info['playerSkills']['unitProjectile']['homing']
        assert info['playerSkills']['selfStatus']['minimumMechanismVersion']==27 and not info['playerSkills']['selfStatus']['createsProjectile']
        assert info['playerSkills']['maxSkills']==2 and info['playerSkills']['minimumMechanismVersion']==26
        assert info['periodicEffects']['minimumMechanismVersion']==23 and info['periodicEffects']['kinds']==['physical_damage','max_hp_heal']
        assert info['periodicEffects']['healMinimumMechanismVersion']==24 and info['periodicEffects']['healBindings']==['commit_self']
        assert info['timedStatus']['healingReceived']['minimumMechanismVersion']==25 and info['timedStatus']['healingReceived']['capture']=='per_heal'
        assert info['absorptionShield']['minimumMechanismVersion']==22 and info['absorptionShield']['duplicatePolicies']==['refresh','ignore']
        assert info['timedStatus']['attributes']==['atk','def'] and info['timedStatus']['attributeMinimumMechanismVersion']==21
        assert info['timedStatus']['stackTiming']=='refresh_all' and info['timedStatus']['maxStacks']==128
        rejected=subprocess.run([binary,'--configs',str(own/'stacks/catalog.json'),'--verification'],cwd=own,
                                env={**os.environ,**invalid},capture_output=True,text=True)
        assert rejected.returncode!=0
        node={}
        for kind in ['repeat-a','repeat-b','piercing-a','piercing-b','contacts-a','contacts-b','fields','ground-targets','unit-targets','self-skills','player-skills','healing-modifiers','healing','periodic','shields','attributes','stacks','statuses']:
            for mode in ['death','extraction']:
                cmd=['node',str(bundle/'hosts/node/asset-host.mjs'),'--binary',binary,
                     '--configs',str(own/kind/'catalog.json'),'--account',str(own/kind/'account.json')]
                if mode=='extraction':cmd.append('--extraction')
                value=json.loads(run('independent-node-'+kind+'-'+mode,cmd,own,invalid))
                assert value['statusApplications']>0 and value['admissions']==value['settlements']==1 and value['clock']=='ordinary'
                if kind.startswith('repeat-'):
                    cfg=json.loads((own/kind/'arena.json').read_text());skills=[cfg['skill'],*cfg['additional_skills']];bolt=next(x for x in skills if x['mechanism']=='piercing_projectile');assert set(value['committedSkills'])=={x['id'] for x in skills};by_action={}
                    for hit in value['piercingHits']:by_action.setdefault(hit['action_id'],[]).append(hit)
                    repeated=False
                    for identity,hits in by_action.items():
                        last={}
                        for h in hits:
                            key=(h['target']['id'],h['target']['generation'])
                            if key in last:
                                repeated=True;assert bolt['repeat_interval_ms']>0 and h['hit_at_ms']-last[key]>=bolt['repeat_interval_ms']
                            last[key]=h['hit_at_ms']
                        assert len(hits)<=bolt['max_hits']
                    assert repeated==(kind=='repeat-b')
                    if kind=='repeat-b':assert any(e['reason']=='hit_budget_exhausted' for e in value['piercingEnds'])
                if kind.startswith('piercing-'):
                    cfg=json.loads((own/kind/'arena.json').read_text());skills=[cfg['skill'],*cfg['additional_skills']];bolt=next(s for s in skills if s['mechanism']=='piercing_projectile')
                    assert set(value['committedSkills'])=={s['id'] for s in skills};assert value['piercingStarts'] and value['piercingHits'] and value['piercingEnds']
                    first=value['piercingStarts'][0];hits=[h for h in value['piercingHits'] if h['action_id']==first['action_id']]
                    assert [h['target']['id'] for h in hits]==['status-target-'+str(i+1) for i in range(bolt['max_hits'])]
                    seen=set()
                    for h in value['piercingHits']:
                        key=(h['action_id'],h['target']['id'],h['target']['generation']);assert key not in seen;seen.add(key)
                if kind.startswith('contacts-'):
                    configured=json.loads((own/kind/'arena.json').read_text());skills=[configured['skill'],*configured['additional_skills']];policy=next(s['contact_policy'] for s in skills if 'contact_policy' in s)
                    assert set(value['committedSkills'])=={s['id'] for s in skills};seen=set();by_action={}
                    for t in value['fieldTicks']:
                        key=(t['action_id'],t['tick_index']);assert key not in seen;seen.add(key);assert len(t['hit_targets'])==t['hits']<=policy['max_targets_per_tick'];by_action.setdefault(t['action_id'],[]).append(t)
                    for action,ticks in by_action.items():
                        remaining=policy['max_total_hits'];last={}
                        for t in ticks:
                            remaining-=t['hits'];assert t['remaining_hits']==remaining>=0
                            for target in t['hit_targets']:
                                k=(target['id'],target['generation'])
                                if k in last:assert policy['repeat_interval_ms']>0 and t['scheduled_at_ms']-last[k]>=policy['repeat_interval_ms']
                                last[k]=t['scheduled_at_ms']
                    if kind=='contacts-b':assert any(e['reason']=='hit_budget_exhausted' for e in value['fieldEnds'])
                if kind=='fields':
                    assert set(value['committedSkills'])=={'lasting-circle','renewing-ward'} and max(t['tick_index'] for t in value['fieldTicks'])>=2
                    seen=set()
                    for t in value['fieldTicks']:
                        identity=(t['action_id'],t['tick_index']);assert identity not in seen;seen.add(identity)
                if kind=='ground-targets':assert set(value['committedSkills'])=={'ground-burst','renewing-ward'} and len(value['committedGroundPoints'])>=2
                if kind=='unit-targets':
                    assert set(value['committedSkills'])=={'marked-bolt','renewing-ward'} and len(set(t['id'] for t in value['committedTargets']))==2
                if kind=='self-skills':assert set(value['committedSkills'])=={'arc-bolt','renewing-ward'}
                if kind=='player-skills':assert set(value['committedSkills'])=={'ward-bolt','renewal-bolt'}
                node[kind+'-'+mode]=value
        home=d/'empty-cargo';home.mkdir()
        env={**invalid,'CARGO_HOME':str(home),'CARGO_NET_OFFLINE':'true','CARGO_TARGET_DIR':str(d/'consumer-target')}
        run('independent-consumer-build',['cargo','build','--locked','--offline','--manifest-path','consumer/Cargo.toml'],bundle,env)
        legacy={}
        for mode in ['normal','unknown-settlement','refused-admission','close-before-ready','failed-publication','config-mismatch']:
            value=json.loads(run('independent-rust-'+mode,[str(d/'consumer-target/debug/minimal-match-host'),
                str(own/'legacy/catalog.json'),str(own/'legacy/account.json'),mode],own,invalid))
            verify_result(value,mode,json.loads((own/'legacy/arena.json').read_text()),json.loads((own/'legacy/account.json').read_text()))
            legacy[mode]=value
        protocols={}
        for kind in ['assets','results']:
            account=json.loads((own/kind/'account.json').read_text())
            for mode in ['death','extraction','unknown-settlement','retry-settlement','admission-refused','close-before-ready','refused-settlement']:
                state=own/(kind+'-'+mode+'.sqlite')
                cmd=[sys.executable,str(bundle/'hosts/asset_host.py'),'--binary',binary,
                     '--configs',str(own/kind/'catalog.json'),'--account',str(own/kind/'account.json'),
                     '--state',str(state),'--mode',mode]
                value=json.loads(run('protocol-'+kind+'-'+mode,cmd,own,invalid))
                if mode=='refused-settlement':
                    assert value['pending'] and value['counts']['settlement']==0
                    value=json.loads(run('protocol-'+kind+'-reconcile',cmd+['--reconcile'],own,invalid))
                assert value['phase']=='closed' and value['liveWorlds']==0 and not value['pending']
                assert value['counts']==({'admission':0,'settlement':0,'compensation':0} if mode=='admission-refused'
                    else {'admission':1,'settlement':0,'compensation':1} if mode=='close-before-ready'
                    else {'admission':1,'settlement':1,'compensation':0})
                balances(value,account)
                # A further process restart must leave all frozen account responsibility unchanged.
                again=json.loads(run('protocol-'+kind+'-'+mode+'-restart',cmd,own,invalid))
                assert again['balances']==value['balances'] and again['counts']==value['counts'] and not again['pending']
                assert again['requests']==value['requests'] and again['configHash']==value['configHash']
                protocols[kind+'-'+mode]=value
        assert hashlib.sha256(Path(report['archive']).read_bytes()).hexdigest()==report['sha256']
        result={'passed':True,'archiveSha256':report['sha256'],'support':info,'rustSourceHashes':native_sources,
                'removedSdkRegressionsBeforeConsumption':True,'emptyCargoHome':True,'offline':True,
                'nodeCases':node,'rustConsumer':legacy,'protocol2And3':protocols,
                'outsideProtocol2Timing':{'extraction_unlock_ms':3000,'extraction_channel_ms':500}}
        (OUTPUT/'independent-verification.json').write_text(json.dumps(result,indent=2)+'\n')
    print('Independent SDK-5-15 delivery and protocol1/2/3 boundaries passed',flush=True)


if __name__=='__main__':main()
