"""Aggregate UT01-UT10 against the frozen candidate, offline delivery and immutable baseline."""
from pathlib import Path
import json,re
from verify_sdk_5_10 import ROOT,OUTPUT,sources
from package_sdk import digest

def load(p):return json.loads(p.read_text())
def tests(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    integration=load(OUTPUT/'verification.json');native=load(OUTPUT/'native-verification.json')
    delivery=load(OUTPUT/'delivery/independent-verification.json');package=load(OUTPUT/'delivery/package.json')
    source=load(OUTPUT/'delivery/source-verification.json');baseline=load(OUTPUT/'baseline-verification.json')
    mapping=load(ROOT/'docs/checklists/SDK-5-10-acceptance.json')
    assert all(v['passed'] for v in [integration,native,delivery,source,baseline])
    assert sources()==integration['sourceHashes'];assert digest(ROOT/'target/debug/kernel-host')==integration['binarySha256']
    assert native['sourceHashes']=={p:h for p,h in integration['sourceHashes'].items() if not p.startswith('scripts/')}
    core={p:h for p,h in integration['sourceHashes'].items() if p.startswith('src/') or p in ['Cargo.toml','Cargo.lock']}
    assert all(delivery['rustSourceHashes'][p]==h for p,h in core.items() if p.startswith('src/'))
    for name in ['default','all-features','clippy','node-storage']:assert digest(OUTPUT/(name+'.log'))==integration['checks'][name]==native['checks'][name]
    a=tests(OUTPUT/'default.log');b=tests(OUTPUT/'all-features.log');old=set().union(*(tests(ROOT/p) for p in baseline['baselineSources']))
    assert a==b and len(a)==939 and len(old)==903 and old<=a and len(a-old)==36 and sorted(a-old)==baseline['added']
    assert source['testCount']==939 and source['sameWorkspaceDefaultAndAllFeatureSet'] and source['emptyCargoHome'] and source['offline']
    assert source['archiveSha256']==package['sha256']==delivery['archiveSha256']==digest(Path(package['archive']))
    for name,h in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+name+'.log'))==h
    assert '# tests 22' in (OUTPUT/'node-storage.log').read_text()
    assert len(baseline['protectedHashes'])==85
    for p,h in baseline['protectedHashes'].items():assert digest(ROOT/p)==h,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json';assert digest(manifest)==baseline['sourcePackageManifestHash']
    entries=load(manifest)['files']
    for e in entries:assert digest(manifest.parent/e['path'])==e['sha256'],e['path']
    assert len(integration['legacy'])==18 and len(integration['cases'])==4 and len(integration['touchCases'])==2 and len(integration['crashes'])==3
    for family in ['cases','touchCases','legacy']:
        for v in integration[family].values():
            assert v['closed']['live_worlds']==0 and not v['ledger']['pending'];assert v['ledger']['admissions']==v['ledger']['settlements']==1 and not v['pageErrors']
    for family in ['cases','touchCases']:
        for v in integration[family].values():
            p=v['skills'];snapshot=p['afterUnit'];refs=p['initialReferences']
            assert len(refs)==2 and p['selectedSecond']==refs[1]==p['pendingTarget']==p['committedTarget']
            assert p['switchedSelectionDuringPreparation'] and p['firstHp']==180 and 0<p['secondHp']<180
            assert p['actualHealing']['hp_restored']>0 and snapshot['skill_ready_at_ms']==snapshot['player_skills'][0]['ready_at_ms']
            assert all(s['ready_at_ms']>snapshot['logical_ms'] for s in snapshot['player_skills'])
            assert v['fight']['kills']==2 and v['fight']['pickedUp'] and v['fight']['selectionClearedOnDeath'] and v['fight']['committedSkills']==sorted(p['ids'])
            if family=='cases':assert p['invalidGeneration']['reason']=='INVALID_TARGET' and p['cooldownRejection']['reason']=='INVALID_STATE'
    assert all(v['skills']['unitIndex']==1 for k,v in integration['cases'].items() if k.startswith('b-'))
    assert integration['cases']['a-extraction']['skills']['reconnected']
    assert set(integration['targetRejections'])=={'range','vision'}
    for v in integration['targetRejections'].values():assert v['probe']['reason']=='OUT_OF_RANGE' and v['closed']['live_worlds']==0 and not v['pageErrors'] and v['ledger']['admissions']==v['ledger']['settlements']==1 and not v['ledger']['pending']
    poor=integration['resourceRejection'];assert poor['reason']=='NO_RESOURCE' and poor['unchangedSpirit']==3 and poor['closed']['live_worlds']==0 and not poor['ledger']['pending']
    for point,v in integration['crashes'].items():
        assert len(v['recoveries'])==2 and v['recoveries'][0]==v['recoveries'][1] and v['recoveries'][0]['closed']['live_worlds']==0
        assert len(v['snapshotBeforeCrash']['player_skills'])==2
        if point=='active':assert v['snapshotBeforeCrash']['pending_action']['unit_target']
    assert len(delivery['nodeCases'])==20 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14
    for mode in ['extraction','death']:
        assert set(delivery['nodeCases']['player-skills-'+mode]['committedSkills'])=={'ward-bolt','renewal-bolt'}
        assert set(delivery['nodeCases']['self-skills-'+mode]['committedSkills'])=={'arc-bolt','renewing-ward'}
        assert set(delivery['nodeCases']['unit-targets-'+mode]['committedSkills'])=={'marked-bolt','renewing-ward'}
        assert len(set(t['id'] for t in delivery['nodeCases']['unit-targets-'+mode]['committedTargets']))==2
    assert delivery['support']['playerSkills']['unitProjectile']['minimumMechanismVersion']==28 and not delivery['support']['playerSkills']['unitProjectile']['homing']
    assert delivery['emptyCargoHome'] and delivery['offline'] and delivery['removedSdkRegressionsBeforeConsumption']
    criteria=mapping['criteria'];assert set(criteria)=={f'UT{i:02}' for i in range(1,11)}
    for v in criteria.values():
        assert set(v.get('rustTests',[]))<=a
        for p in v.get('reports',[]):assert load(ROOT/p)['passed']
    assert set().union(*(set(v.get('rustTests',[])) for v in criteria.values()))>=a-old
    preview=load(OUTPUT/'local-preview.json');assert preview['passed'] and not preview['createdMatch']
    docs=load(OUTPUT/'documentation-check.json');assert docs['passed'] and not docs['broken']
    report={'passed':True,'date':'2026-11-04','crate':'0.13.0','mechanismVersion':28,'scope':'M02 finite strict hostile unit projectiles; not friendly/ground/homing, full M02 or original game requirements',
        'rustTests':len(a),'newRustTests':36,'retainedOriginalRustTests':len(old),'sameDefaultAndAllFeatures':True,'clippy':'passed with -D warnings','nodeTests':22,
        'ordinaryNodeCases':4,'touchCases':2,'targetRejectionCases':2,'resourceRejectionCases':1,'legacy19To27Cases':18,'crashBoundaries':3,'recoveriesPerBoundary':2,
        'independentNodeCases':20,'legacyRustModes':6,'protocol2And3Cases':14,'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':939,'sourcePackageFilesVerified':len(entries),
        'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'scripts').glob('verify_sdk_5_10*.py'))},
        'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},'sourceHashes':integration['sourceHashes'],'nativeCoreSourceHashes':core,'protectedHashes':baseline['protectedHashes'],
        'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],'binarySha256':{'replayEnabledAcceptance':integration['binarySha256'],'ordinaryDelivery':package['binarySha256']},
        'reports':{str(p.relative_to(ROOT)):digest(p) for p in [OUTPUT/'verification.json',OUTPUT/'native-verification.json',OUTPUT/'baseline-verification.json',OUTPUT/'local-preview.json',OUTPUT/'documentation-check.json',OUTPUT/'delivery/package.json',OUTPUT/'delivery/independent-verification.json',OUTPUT/'delivery/source-verification.json',ROOT/'docs/checklists/SDK-5-10-acceptance.json']},
        'compatibilityNote':'85 prior JSON/archive files and original source-package manifest/files are byte-identical; mechanism1-27 keep their output and action policy.'}
    (OUTPUT/'acceptance.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('SDK-5-10: UT01-UT10 passed; 939 Rust tests, 22 Node tests, dual skills / touch / recovery / independent offline delivery')
if __name__=='__main__':main()
