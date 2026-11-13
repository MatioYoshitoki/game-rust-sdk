"""Bind GC01-GC09 to one candidate, retained baseline, ordinary consumers and offline source."""
from pathlib import Path
import json,re
from verify_sdk_5_11 import ROOT,OUTPUT,sources
from package_sdk import digest

def load(p):return json.loads(p.read_text())
def tests(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    integration=load(OUTPUT/'verification.json');native=load(OUTPUT/'native-verification.json')
    package=load(OUTPUT/'delivery/package.json');delivery=load(OUTPUT/'delivery/independent-verification.json');source=load(OUTPUT/'delivery/source-verification.json');baseline=load(OUTPUT/'baseline-verification.json')
    mapping=load(ROOT/'docs/checklists/SDK-5-11-acceptance.json');preview=load(OUTPUT/'local-preview.json');docs=load(OUTPUT/'documentation-check.json')
    assert all(v['passed'] for v in [integration,native,delivery,source,baseline,preview,docs])
    assert sources()==integration['sourceHashes'];assert digest(ROOT/'target/debug/kernel-host')==integration['binarySha256']==preview['binarySha256']
    assert native['sourceHashes']=={p:h for p,h in integration['sourceHashes'].items() if not p.startswith('scripts/')}
    core={p:h for p,h in integration['sourceHashes'].items() if p.startswith('src/') or p in ['Cargo.toml','Cargo.lock']}
    for p,h in core.items():
        if p.startswith('src/'):assert delivery['rustSourceHashes'][p]==h,p
    for name in ['default','all-features','clippy','node-storage']:assert digest(OUTPUT/(name+'.log'))==integration['checks'][name]==native['checks'][name]
    a=tests(OUTPUT/'default.log');b=tests(OUTPUT/'all-features.log');old=tests(ROOT/'artifacts/sdk-5-10/default.log')
    assert len(a)==973 and a==b and len(old)==939 and old<=a and sorted(a-old)==baseline['added'] and len(a-old)==34
    assert source['testCount']==973 and source['sameWorkspaceDefaultAndAllFeatureSet'] and source['emptyCargoHome'] and source['offline']
    assert source['archiveSha256']==delivery['archiveSha256']==package['sha256']==digest(Path(package['archive']))
    for name,h in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+name+'.log'))==h
    assert '# tests 22' in (OUTPUT/'node-storage.log').read_text()
    assert baseline['protectedCount']==len(baseline['protectedHashes'])==89
    for p,h in baseline['protectedHashes'].items():assert digest(ROOT/p)==h,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json';assert digest(manifest)==baseline['sourcePackageManifestHash'];entries=load(manifest)['files']
    for e in entries:assert digest(manifest.parent/e['path'])==e['sha256'],e['path']
    assert len(integration['cases'])==4 and len(integration['touchCases'])==2 and len(integration['legacy'])==20 and len(integration['crashes'])==3
    for family in ['cases','touchCases','legacy']:
        for v in integration[family].values():
            assert v['closed']['live_worlds']==0 and not v['ledger']['pending'] and not v['pageErrors'];assert v['ledger']['admissions']==v['ledger']['settlements']==1
    for family in ['cases','touchCases']:
        for v in integration[family].values():
            p=v['skills'];assert p['emptyCircleHits']==0 and p['hits']==2 and p['actualHealing']['hp_restored']>0 and p['switchedSelectionDuringPreparation']
            assert p['pending']['ground_position']==p['committed']['ground_position']==p['warning']['center']
            assert all(0<m['hp']<180 and m['statuses'] for m in p['afterCircle']['pve']['monsters'])
            assert v['fight']['kills']==2 and v['fight']['pickedUp'] and v['fight']['committedSkills']==sorted(p['ids'])
    assert integration['cases']['a-extraction']['skills']['reconnected'];assert all(v['skills']['groundIndex']==1 for k,v in integration['cases'].items() if k.startswith('b-'))
    assert set(integration['targetRejections'])=={'range','vision'}
    for v in integration['targetRejections'].values():assert v['probe']['reason']=='OUT_OF_RANGE' and not v['pageErrors'] and v['closed']['live_worlds']==0 and not v['ledger']['pending']
    poor=integration['resourceRejection'];assert poor['reason']=='NO_RESOURCE' and poor['unchangedSpirit']==3 and poor['closed']['live_worlds']==0 and not poor['ledger']['pending']
    for point,v in integration['crashes'].items():
        assert len(v['recoveries'])==2 and v['recoveries'][0]==v['recoveries'][1] and v['recoveries'][0]['closed']['live_worlds']==0
        if point=='active':assert v['snapshotBeforeCrash']['pending_action']['ground_position'] and v['snapshotBeforeCrash']['pve']['player_area_warnings']
    assert len(delivery['nodeCases'])==22 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14
    for mode in ['extraction','death']:
        v=delivery['nodeCases']['ground-targets-'+mode];assert set(v['committedSkills'])=={'ground-burst','renewing-ward'} and len(v['committedGroundPoints'])>=2
    assert delivery['support']['crateVersion']=='0.14.0' and delivery['support']['mechanismVersions']['max']==29 and not delivery['support']['clocks']['replay']
    assert delivery['support']['playerSkills']['groundCircle']['maxPendingAreas']==2 and delivery['emptyCargoHome'] and delivery['offline'] and delivery['removedSdkRegressionsBeforeConsumption']
    criteria=mapping['criteria'];assert set(criteria)=={f'GC{i:02}' for i in range(1,10)}
    for v in criteria.values():
        assert set(v.get('rustTests',[]))<=a
        for p in v.get('reports',[]):assert load(ROOT/p)['passed']
    assert set().union(*(set(v.get('rustTests',[])) for v in criteria.values()))>=a-old
    assert not preview['createdMatch'] and preview['configId']=='ground-skill-arena-v1' and not docs['broken']
    report={'passed':True,'date':'2026-11-04','crate':'0.14.0','mechanismVersion':29,'scope':'strict ground point and one circle pulse; not persistent fields/traps, full M02 or original game requirements',
        'rustTests':len(a),'newRustTests':34,'retainedOriginalRustTests':len(old),'sameDefaultAndAllFeatures':True,'clippy':'passed with -D warnings','nodeTests':22,
        'ordinaryH5Cases':4,'touchCases':2,'targetRejectionCases':2,'resourceRejectionCases':1,'legacy19To28Cases':20,'crashBoundaries':3,'recoveriesPerBoundary':2,
        'independentNodeCases':22,'legacyRustModes':6,'protocol2And3Cases':14,'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':973,'sourcePackageFilesVerified':len(entries),
        'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'scripts').glob('verify_sdk_5_11*.py'))},
        'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},'sourceHashes':integration['sourceHashes'],'nativeCoreSourceHashes':core,'protectedHashes':baseline['protectedHashes'],
        'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],'binarySha256':{'replayEnabledAcceptance':integration['binarySha256'],'ordinaryDelivery':package['binarySha256']},
        'reports':{str(p.relative_to(ROOT)):digest(p) for p in [OUTPUT/'verification.json',OUTPUT/'native-verification.json',OUTPUT/'baseline-verification.json',OUTPUT/'local-preview.json',OUTPUT/'documentation-check.json',OUTPUT/'delivery/package.json',OUTPUT/'delivery/independent-verification.json',OUTPUT/'delivery/source-verification.json',ROOT/'docs/checklists/SDK-5-11-acceptance.json']},
        'compatibilityNote':'89 prior JSON/archive files and original source-package manifest/files are byte-identical; mechanism1-28 output and actions retained.'}
    (OUTPUT/'acceptance.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('SDK-5-11 GC01-GC09: 973 Rust / 22 Node, ordinary H5/touch/recovery/offline delivery passed')
if __name__=='__main__':main()
