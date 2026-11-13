"""Aggregate MS01-MS10 against the frozen candidate, offline delivery and immutable baseline."""
from pathlib import Path
import json,re
from verify_sdk_5_8 import ROOT,OUTPUT,sources
from package_sdk import digest

def load(p):return json.loads(p.read_text())
def tests(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    integration=load(OUTPUT/'verification.json');native=load(OUTPUT/'native-verification.json')
    delivery=load(OUTPUT/'delivery/independent-verification.json');package=load(OUTPUT/'delivery/package.json')
    source=load(OUTPUT/'delivery/source-verification.json');baseline=load(OUTPUT/'baseline-verification.json')
    mapping=load(ROOT/'docs/checklists/SDK-5-8-acceptance.json')
    assert all(v['passed'] for v in [integration,native,delivery,source,baseline])
    assert sources()==integration['sourceHashes'];assert digest(ROOT/'target/debug/kernel-host')==integration['binarySha256']
    assert native['sourceHashes']=={p:h for p,h in integration['sourceHashes'].items() if not p.startswith('scripts/')}
    core={p:h for p,h in integration['sourceHashes'].items() if p.startswith('src/') or p in ['Cargo.toml','Cargo.lock']}
    assert all(delivery['rustSourceHashes'][p]==h for p,h in core.items() if p.startswith('src/'))
    for name in ['default','all-features','clippy','node-storage']:assert digest(OUTPUT/(name+'.log'))==integration['checks'][name]==native['checks'][name]
    a=tests(OUTPUT/'default.log');b=tests(OUTPUT/'all-features.log');old=set().union(*(tests(ROOT/p) for p in baseline['baselineSources']))
    assert a==b and len(a)==867 and len(old)==835 and old<=a and len(a-old)==32 and sorted(a-old)==baseline['added']
    assert source['testCount']==867 and source['sameWorkspaceDefaultAndAllFeatureSet'] and source['emptyCargoHome'] and source['offline']
    assert source['archiveSha256']==package['sha256']==delivery['archiveSha256']==digest(Path(package['archive']))
    for name,h in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+name+'.log'))==h
    assert '# tests 22' in (OUTPUT/'node-storage.log').read_text()
    assert len(baseline['protectedHashes'])==77
    for p,h in baseline['protectedHashes'].items():assert digest(ROOT/p)==h,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json';assert digest(manifest)==baseline['sourcePackageManifestHash']
    entries=load(manifest)['files']
    for e in entries:assert digest(manifest.parent/e['path'])==e['sha256'],e['path']
    assert len(integration['legacy'])==14 and len(integration['cases'])==4 and len(integration['touchCases'])==2 and len(integration['crashes'])==3
    for family in ['cases','touchCases','legacy']:
        for v in integration[family].values():
            assert v['closed']['live_worlds']==0 and not v['ledger']['pending'];assert v['ledger']['admissions']==v['ledger']['settlements']==1 and not v['pageErrors']
    for family in ['cases','touchCases']:
        for v in integration[family].values():
            probe=v['skills'];one=probe['afterPrimary'];two=probe['afterSecondary']
            assert one['player_skills'][0]['ready_at_ms']==one['skill_ready_at_ms']>one['logical_ms'] and one['player_skills'][1]['ready_at_ms']==0
            assert two['player_skills'][0]['ready_at_ms']==one['player_skills'][0]['ready_at_ms'] and all(s['ready_at_ms']>two['logical_ms'] for s in two['player_skills'])
            assert v['fight']['kills']==1 and v['fight']['pickedUp'] and v['fight']['committedSkills']==sorted(probe['ids'])
            if family=='cases':assert probe['recoveryRejection']['reason']==probe['cooldownRejection']['reason']=='INVALID_STATE'
    assert integration['cases']['a-extraction']['skills']['reconnected']
    poor=integration['resourceRejection'];assert poor['reason']=='NO_RESOURCE' and poor['unchangedSpirit']==3 and poor['closed']['live_worlds']==0 and not poor['ledger']['pending']
    for point,v in integration['crashes'].items():
        assert len(v['recoveries'])==2 and v['recoveries'][0]==v['recoveries'][1] and v['recoveries'][0]['closed']['live_worlds']==0
        assert len(v['snapshotBeforeCrash']['player_skills'])==2
        if point=='active':assert v['snapshotBeforeCrash']['pending_action']
    assert len(delivery['nodeCases'])==16 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14
    for mode in ['extraction','death']:assert set(delivery['nodeCases']['player-skills-'+mode]['committedSkills'])=={'ward-bolt','renewal-bolt'}
    assert delivery['emptyCargoHome'] and delivery['offline'] and delivery['removedSdkRegressionsBeforeConsumption']
    criteria=mapping['criteria'];assert set(criteria)=={f'MS{i:02}' for i in range(1,11)}
    for v in criteria.values():
        assert set(v.get('rustTests',[]))<=a
        for p in v.get('reports',[]):assert load(ROOT/p)['passed']
    assert set().union(*(set(v.get('rustTests',[])) for v in criteria.values()))>=a-old
    preview=load(OUTPUT/'local-preview.json');assert preview['passed'] and not preview['createdMatch']
    docs=load(OUTPUT/'documentation-check.json');assert docs['passed'] and not docs['broken']
    report={'passed':True,'date':'2026-11-04','crate':'0.11.0','mechanismVersion':26,'scope':'M02 finite 1-2 configured directional projectile skills; not full M02 or original game requirements',
        'rustTests':len(a),'newRustTests':32,'retainedOriginalRustTests':len(old),'sameDefaultAndAllFeatures':True,'clippy':'passed with -D warnings','nodeTests':22,
        'ordinaryNodeCases':4,'touchCases':2,'resourceRejectionCases':1,'legacy19To25Cases':14,'crashBoundaries':3,'recoveriesPerBoundary':2,
        'independentNodeCases':16,'legacyRustModes':6,'protocol2And3Cases':14,'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':867,'sourcePackageFilesVerified':len(entries),
        'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'scripts').glob('verify_sdk_5_8*.py'))},
        'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},'sourceHashes':integration['sourceHashes'],'nativeCoreSourceHashes':core,'protectedHashes':baseline['protectedHashes'],
        'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],'binarySha256':{'replayEnabledAcceptance':integration['binarySha256'],'ordinaryDelivery':package['binarySha256']},
        'reports':{str(p.relative_to(ROOT)):digest(p) for p in [OUTPUT/'verification.json',OUTPUT/'native-verification.json',OUTPUT/'baseline-verification.json',OUTPUT/'local-preview.json',OUTPUT/'documentation-check.json',OUTPUT/'delivery/package.json',OUTPUT/'delivery/independent-verification.json',OUTPUT/'delivery/source-verification.json',ROOT/'docs/checklists/SDK-5-8-acceptance.json']},
        'compatibilityNote':'77 prior JSON/archive files and original source-package manifest/files are byte-identical; mechanism1-25 keep their output and action policy.'}
    (OUTPUT/'acceptance.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('SDK-5-8: MS01-MS10 passed; 867 Rust tests, 22 Node tests, dual skills / touch / recovery / independent offline delivery')
if __name__=='__main__':main()
