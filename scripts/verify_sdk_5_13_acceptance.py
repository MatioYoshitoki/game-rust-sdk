"""Bind CP01-CP09 to the final candidate, retained inputs and independent offline delivery."""
import json,re
from pathlib import Path
from verify_sdk_5_13 import ROOT,OUTPUT,sources
from package_sdk import digest

def load(p):return json.loads(p.read_text())
def tests(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    integration=load(OUTPUT/'verification.json');native=load(OUTPUT/'native-verification.json');package=load(OUTPUT/'delivery/package.json');delivery=load(OUTPUT/'delivery/independent-verification.json');source=load(OUTPUT/'delivery/source-verification.json');baseline=load(OUTPUT/'baseline-verification.json');mapping=load(ROOT/'docs/checklists/SDK-5-13-acceptance.json');preview=load(OUTPUT/'local-preview.json');docs=load(OUTPUT/'documentation-check.json')
    assert all(v['passed'] for v in [integration,native,delivery,source,baseline,preview,docs]);assert sources()==integration['sourceHashes'];assert digest(ROOT/'target/debug/kernel-host')==integration['binarySha256']==preview['binarySha256']
    assert native['sourceHashes']=={p:h for p,h in integration['sourceHashes'].items() if not p.startswith('scripts/')}
    core={p:h for p,h in integration['sourceHashes'].items() if p.startswith('src/') or p in ['Cargo.toml','Cargo.lock']}
    for p,h in core.items():
        if p.startswith('src/'):assert delivery['rustSourceHashes'][p]==h,p
    for n in ['default','all-features','clippy','node-storage']:assert digest(OUTPUT/(n+'.log'))==integration['checks'][n]==native['checks'][n]
    a=tests(OUTPUT/'default.log');b=tests(OUTPUT/'all-features.log');old=tests(ROOT/'artifacts/sdk-5-12/default.log');assert len(a)==1049 and a==b and len(old)==1010 and old<=a and sorted(a-old)==baseline['added'] and len(a-old)==39
    assert source['testCount']==1049 and source['sameWorkspaceDefaultAndAllFeatureSet'] and source['emptyCargoHome'] and source['offline'];assert source['archiveSha256']==delivery['archiveSha256']==package['sha256']==digest(Path(package['archive']))
    for n,h in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+n+'.log'))==h
    assert '# tests 22' in (OUTPUT/'node-storage.log').read_text();assert baseline['protectedCount']==len(baseline['protectedHashes'])==97
    for p,h in baseline['protectedHashes'].items():assert digest(ROOT/p)==h,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json';assert digest(manifest)==baseline['sourcePackageManifestHash'];entries=load(manifest)['files']
    for e in entries:assert digest(manifest.parent/e['path'])==e['sha256'],e['path']
    assert len(integration['cases'])==4 and len(integration['touchCases'])==2 and len(integration['legacy'])==24 and len(integration['crashes'])==3
    for family in ['cases','touchCases','legacy']:
        for v in integration[family].values():assert v['closed']['live_worlds']==0 and not v['ledger']['pending'] and not v['pageErrors'] and v['ledger']['admissions']==v['ledger']['settlements']==1
    for family in ['cases','touchCases']:
        for v in integration[family].values():
            p=v['skills'];f=p['field'];policy=f['contact_policy'];ticks=p['ticks'];assert len(ticks)==len(p['emptyTicks'])==3 and all(t['hits']==0 and t['remaining_hits']==3 for t in p['emptyTicks']);assert policy['used_hits']==0 and policy['remaining_hits']==3 and not policy['contacts']
            assert p['pending']['ground_position']==p['committed']['ground_position']==f['center'];assert p['progressBeforeReconnect']['contact_policy']['used_hits']>=1 and p['switchedSelectionDuringPreparation'] and p['actualHealing']['hp_restored']>0
            assert [t['tick_index'] for t in ticks]==[1,2,3];remaining=3;history={}
            for t in ticks:
                assert len(t['hit_targets'])==t['hits']<=policy['max_targets_per_tick'];remaining-=t['hits'];assert t['remaining_hits']==remaining>=0
                for target in t['hit_targets']:
                    key=(target['id'],target['generation'])
                    if key in history:assert policy['repeat_interval_ms']>0 and t['scheduled_at_ms']-history[key]>=policy['repeat_interval_ms']
                    history[key]=t['scheduled_at_ms']
            assert ticks[-1]['scheduled_at_ms']<f['expires_at_ms'];assert p['ended']['reason']==('hit_budget_exhausted' if remaining==0 else 'expired')
            if policy['repeat_interval_ms']==0:assert [t['hits'] for t in ticks]==[1,1,0] and remaining==1
            else:assert [t['hits'] for t in ticks]==[1,1,1] and remaining==0
            assert all(0<m['hp']<180 and m['statuses'] for m in p['afterField']['pve']['monsters']);assert v['fight']['kills']==2 and v['fight']['pickedUp'] and v['fight']['committedSkills']==sorted(p['ids'])
    assert integration['cases']['a-extraction']['skills']['reconnected'];assert all(v['skills']['fieldIndex']==1 for k,v in integration['cases'].items() if k.startswith('b-'));assert all(v['skills']['ended']['reason']=='hit_budget_exhausted' for v in integration['touchCases'].values())
    assert set(integration['targetRejections'])=={'range','vision'}
    for v in integration['targetRejections'].values():assert v['probe']['reason']=='OUT_OF_RANGE' and not v['pageErrors'] and v['closed']['live_worlds']==0 and not v['ledger']['pending']
    p=integration['resourceRejection'];assert p['reason']=='NO_RESOURCE' and p['unchangedSpirit']==3 and p['closed']['live_worlds']==0 and not p['ledger']['pending']
    for point,v in integration['crashes'].items():
        assert len(v['recoveries'])==2 and v['recoveries'][0]==v['recoveries'][1] and v['recoveries'][0]['closed']['live_worlds']==0
        if point=='active':assert v['snapshotBeforeCrash']['pending_action']['ground_position'] and any(f['contact_policy']['used_hits']>=1 and f['contact_policy']['contacts'] for f in v['snapshotBeforeCrash']['pve']['player_fields'])
    assert len(delivery['nodeCases'])==28 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14
    for variant in ['a','b']:
        for mode in ['extraction','death']:
            v=delivery['nodeCases']['contacts-'+variant+'-'+mode];assert v['fieldStarts'] and all(f['contact_policy']['used_hits']==0 and f['contact_policy']['remaining_hits']==3 for f in v['fieldStarts']);assert all(len(t['hit_targets'])==t['hits']<=1 for t in v['fieldTicks'])
            if variant=='b':assert any(e['reason']=='hit_budget_exhausted' for e in v['fieldEnds'])
    assert delivery['support']['crateVersion']=='0.16.0' and delivery['support']['mechanismVersions']['max']==31 and not delivery['support']['clocks']['replay'];assert delivery['support']['playerSkills']['contactPolicy']['historyMax']==128 and not delivery['support']['playerSkills']['contactPolicy']['dodgeConsumes'];assert delivery['emptyCargoHome'] and delivery['offline'] and delivery['removedSdkRegressionsBeforeConsumption']
    criteria=mapping['criteria'];assert set(criteria)=={f'CP{i:02}' for i in range(1,10)}
    for v in criteria.values():
        assert set(v.get('rustTests',[]))<=a
        for p in v.get('reports',[]):assert load(ROOT/p)['passed']
    assert set().union(*(set(v.get('rustTests',[])) for v in criteria.values()))>=a-old;assert not preview['createdMatch'] and preview['configId']=='contact-arena-v1' and not docs['broken']
    report={'passed':True,'date':'2026-11-05','crate':'0.16.0','mechanismVersion':31,'scope':'generic identity/time contact budget first integrated with ground_field; not complete M05',
        'rustTests':len(a),'newRustTests':39,'retainedOriginalRustTests':len(old),'sameDefaultAndAllFeatures':True,'clippy':'passed with -D warnings','nodeTests':22,'ordinaryH5Cases':4,'touchCases':2,'targetRejectionCases':2,'resourceRejectionCases':1,'legacy19To30Cases':24,'crashBoundaries':3,'recoveriesPerBoundary':2,'independentNodeCases':28,'legacyRustModes':6,'protocol2And3Cases':14,'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':1049,'sourcePackageFilesVerified':len(entries),
        'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'scripts').glob('verify_sdk_5_13*.py'))},'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},'sourceHashes':integration['sourceHashes'],'nativeCoreSourceHashes':core,'protectedHashes':baseline['protectedHashes'],'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],'binarySha256':{'replayEnabledAcceptance':integration['binarySha256'],'ordinaryDelivery':package['binarySha256']},
        'reports':{str(p.relative_to(ROOT)):digest(p) for p in [OUTPUT/'verification.json',OUTPUT/'native-verification.json',OUTPUT/'baseline-verification.json',OUTPUT/'local-preview.json',OUTPUT/'documentation-check.json',OUTPUT/'delivery/package.json',OUTPUT/'delivery/independent-verification.json',OUTPUT/'delivery/source-verification.json',ROOT/'docs/checklists/SDK-5-13-acceptance.json']},'compatibilityNote':'97 prior JSON/archive files and original source manifest/registered files are byte-identical; omitted policies and mechanism1-30 retain wire and action behavior.'}
    (OUTPUT/'acceptance.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('SDK-5-13 CP01-CP09: 1049 Rust / 22 Node / 28 independent Node / identity history + budget loops passed')
if __name__=='__main__':main()
