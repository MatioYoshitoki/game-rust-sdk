"""Bind PP01-PP09 to one candidate, immutable baselines and offline consumption."""
import json,re,tarfile,hashlib
from pathlib import Path
from verify_sdk_5_14 import ROOT,OUTPUT,sources
from package_sdk import digest

def load(p):return json.loads(p.read_text())
def tests(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    integration=load(OUTPUT/'verification.json');native=load(OUTPUT/'native-verification.json');package=load(OUTPUT/'delivery/package.json');delivery=load(OUTPUT/'delivery/independent-verification.json');source=load(OUTPUT/'delivery/source-verification.json');baseline=load(OUTPUT/'baseline-verification.json');mapping=load(ROOT/'docs/checklists/SDK-5-14-acceptance.json');preview=load(OUTPUT/'local-preview.json');docs=load(OUTPUT/'documentation-check.json')
    capacity=load(OUTPUT/'capacity-verification.json');assert capacity['passed'] and capacity['sourceHashes']==sources() and capacity['rejected']['reason']=='CAPACITY' and capacity['selfSkillStillUsable'] and capacity['before']['spirit']==capacity['after']['spirit']==176 and capacity['closed']['live_worlds']==0 and not capacity['ledger']['pending'] and not capacity['pageErrors']
    if native.get('incrementalVerification'):assert digest(OUTPUT/'node-syntax.log')==native['incrementalVerification']['nodeSyntaxSha256']
    assert all(v['passed'] for v in [integration,native,delivery,source,baseline,preview,docs]);assert sources()==integration['sourceHashes'];
    for revision in integration.get('harnessProvenance',[]):assert revision['unchangedBehaviorInputs'] and revision['binarySha256']==integration['binarySha256'] and digest(ROOT/revision['revision'])==revision['sha256']
    assert digest(ROOT/'target/debug/kernel-host')==integration['binarySha256']==preview['binarySha256']
    assert native['sourceHashes']=={p:h for p,h in integration['sourceHashes'].items() if not p.startswith('scripts/')}
    for p,h in integration['sourceHashes'].items():
        if p.startswith('src/'):assert delivery['rustSourceHashes'][p]==h,p
    for n in ['default','all-features','clippy','node-storage']:assert digest(OUTPUT/(n+'.log'))==integration['checks'][n]==native['checks'][n]
    a=tests(OUTPUT/'default.log');b=tests(OUTPUT/'all-features.log');old=tests(ROOT/'artifacts/sdk-5-13/default.log');assert len(old)==1049 and a==b and old<=a and len(a)==1084 and len(a-old)==35 and sorted(a-old)==baseline['added']
    assert source['testCount']==len(a) and source['sameWorkspaceDefaultAndAllFeatureSet'] and source['emptyCargoHome'] and source['offline'];assert source['archiveSha256']==delivery['archiveSha256']==package['sha256']==digest(Path(package['archive']))
    for n,h in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+n+'.log'))==h
    delivered_sources={}
    with tarfile.open(package['archive']) as archive:
        copied={'SDK.md':'sdk/SDK.md','Cargo.lock':'sdk/Cargo.lock','adapters/python/asset_host.py':'hosts/asset_host.py',**{'adapters/node/'+n:'hosts/node/'+n for n in ['asset-host.mjs','asset-store.mjs','controlled-store.mjs']}}
        for local,member in copied.items():
            h=hashlib.sha256(archive.extractfile(package['rootName']+'/'+member).read()).hexdigest();assert h==digest(ROOT/local),local;delivered_sources[local]=h

    assert '# tests 22' in (OUTPUT/'node-storage.log').read_text();assert baseline['protectedCount']==len(baseline['protectedHashes'])==101
    for p,h in baseline['protectedHashes'].items():assert digest(ROOT/p)==h,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json';assert digest(manifest)==baseline['sourcePackageManifestHash'];entries=load(manifest)['files']
    for e in entries:assert digest(manifest.parent/e['path'])==e['sha256'],e['path']
    assert len(integration['cases'])==4 and len(integration['touchCases'])==2 and len(integration['legacy'])==26 and len(integration['crashes'])==3
    for family in ['cases','touchCases','legacy']:
        for v in integration[family].values():assert v['closed']['live_worlds']==0 and not v['ledger']['pending'] and not v['pageErrors'] and v['ledger']['admissions']==v['ledger']['settlements']==1
    for family in ['cases','touchCases']:
        for v in integration[family].values():
            p=v['skills'];f=p['flight'];hits=p['hits'];assert f['used_hits']==0 and f['remaining_hits']==f['max_hits'] and not f['contacts'];assert p['pending']['skill_id']==p['committed']['skill_id']==f['skill_id'];assert p['progressBeforeReconnect']['used_hits']>=1 and p['progressBeforeReconnect']['contacts'] and p['switchedSelectionDuringPreparation'] and p['actualHealing']['hp_restored']>0
            assert [h['target']['id'] for h in hits]==['status-target-'+str(i+1) for i in range(f['max_hits'])];assert [h['remaining_hits'] for h in hits]==list(range(f['max_hits']-1,-1,-1));assert p['ended']['reason']=='hit_budget_exhausted';assert p['afterFlight']['pve']['monsters'][2]['hp']==(180 if f['max_hits']==2 else p['afterFlight']['pve']['monsters'][1]['hp']);assert v['fight']['kills']==3 and v['fight']['pickedUp'] and v['fight']['committedSkills']==sorted(p['ids'])
    assert integration['cases']['a-extraction']['skills']['reconnected'];assert all(v['skills']['piercingIndex']==1 for k,v in integration['cases'].items() if k.startswith('b-'))
    assert set(integration['targetRejections'])=={'zero-direction','wrong-intent'};assert all(v['reason']=='INVALID_MESSAGE' for v in integration['targetRejections'].values())
    p=integration['resourceRejection'];assert p['reason']=='NO_RESOURCE' and p['unchangedSpirit']==3 and p['closed']['live_worlds']==0 and not p['ledger']['pending']
    for point,v in integration['crashes'].items():
        assert len(v['recoveries'])==2 and v['recoveries'][0]==v['recoveries'][1] and v['recoveries'][0]['closed']['live_worlds']==0
        if point=='active':assert v['snapshotBeforeCrash']['pending_action']['skill_id'] and any(f['used_hits']>=1 and f['contacts'] for f in v['snapshotBeforeCrash']['pve']['piercing_projectiles'])
    assert len(delivery['nodeCases'])==32 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14
    for variant in ['a','b']:
        for mode in ['extraction','death']:
            v=delivery['nodeCases']['piercing-'+variant+'-'+mode];assert v['piercingStarts'] and v['piercingHits'] and v['piercingEnds'];first=v['piercingStarts'][0];hits=[h for h in v['piercingHits'] if h['action_id']==first['action_id']];assert len(hits)==(2 if variant=='a' else 3)
    assert delivery['support']['crateVersion']=='0.17.0' and delivery['support']['mechanismVersions']['max']==32 and not delivery['support']['clocks']['replay'];assert delivery['support']['playerSkills']['piercingProjectile']['collision']=='continuous_sweep';assert delivery['emptyCargoHome'] and delivery['offline'] and delivery['removedSdkRegressionsBeforeConsumption']
    criteria=mapping['criteria'];assert set(criteria)=={f'PP{i:02}' for i in range(1,10)}
    for v in criteria.values():
        assert set(v.get('rustTests',[]))<=a
        for p in v.get('reports',[]):assert load(ROOT/p)['passed']
    assert set().union(*(set(v.get('rustTests',[])) for v in criteria.values()))>=a-old;assert not preview['createdMatch'] and preview['configId']=='piercing-arena-v1' and not docs['broken']
    report={'passed':True,'date':'2026-11-05','crate':'0.17.0','mechanismVersion':32,'scope':'directional straight piercing projectiles; not complete M05',
        'rustTests':len(a),'newRustTests':len(a-old),'retainedOriginalRustTests':len(old),'sameDefaultAndAllFeatures':True,'clippy':'passed with -D warnings','nodeTests':22,'ordinaryH5Cases':4,'touchCases':2,'targetRejectionCases':2,'resourceRejectionCases':1,'capacityRejectionCases':1,'legacy19To31Cases':26,'crashBoundaries':3,'recoveriesPerBoundary':2,'independentNodeCases':32,'legacyRustModes':6,'protocol2And3Cases':14,'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':len(a),'sourcePackageFilesVerified':len(entries),
        'deliveredHostSourceHashes':delivered_sources,'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'scripts').glob('verify_sdk_5_14*.py'))},'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},'sourceHashes':integration['sourceHashes'],'protectedHashes':baseline['protectedHashes'],'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],'binarySha256':{'replayEnabledAcceptance':integration['binarySha256'],'ordinaryDelivery':package['binarySha256']},
        'reports':{str(p.relative_to(ROOT)):digest(p) for p in [OUTPUT/'verification.json',OUTPUT/'capacity-verification.json',OUTPUT/'native-verification.json',OUTPUT/'baseline-verification.json',OUTPUT/'local-preview.json',OUTPUT/'documentation-check.json',OUTPUT/'delivery/package.json',OUTPUT/'delivery/independent-verification.json',OUTPUT/'delivery/source-verification.json',ROOT/'docs/checklists/SDK-5-14-acceptance.json']},'compatibilityNote':'101 prior JSON/archive files and original source manifest/registered files are byte-identical; mechanism1-31 retain wire and action behavior.'}
    (OUTPUT/'acceptance.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('SDK-5-14 PP01-PP09: all candidate evidence passed')
if __name__=='__main__':main()
