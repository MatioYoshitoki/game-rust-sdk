"""Aggregate SK01-SK10 using immutable logs, archive and current candidate hashes."""
from pathlib import Path
import json
import re

from verify_sdk_5_2 import ROOT, OUTPUT, sources
from package_sdk import digest


def load(path):return json.loads(path.read_text())


def tests(path):return set(re.findall(r'^test (\S+) \.\.\. ok$',path.read_text(),re.M))


def main():
    integration=load(OUTPUT/'verification.json')
    initial=load(OUTPUT/'initial-verification.json')
    delivery=load(OUTPUT/'delivery/independent-verification.json')
    package=load(OUTPUT/'delivery/package.json')
    source=load(OUTPUT/'delivery/source-verification.json')
    baseline=load(OUTPUT/'baseline-verification.json')
    mapping=load(ROOT/'docs/checklists/SDK-5-2-acceptance.json')
    assert all(v['passed'] for v in [integration,initial,delivery,baseline,source])
    assert sources()==integration['sourceHashes']
    assert digest(ROOT/'target/debug/kernel-host')==integration['binarySha256']
    core={p:h for p,h in integration['sourceHashes'].items() if p.startswith('src/') or p in ['Cargo.toml','Cargo.lock']}
    assert all(initial['sourceHashes'][p]==h for p,h in core.items())
    assert all(delivery['rustSourceHashes'][p]==h for p,h in core.items() if p.startswith('src/'))
    for name in ['default','all-features','clippy','node-storage']:
        assert digest(OUTPUT/(name+'.log'))==initial['checks'][name]
    a=tests(OUTPUT/'default.log');b=tests(OUTPUT/'all-features.log')
    old=set().union(*(tests(ROOT/p) for p in baseline['baselineSources']))
    assert a==b and len(a)==686 and len(old)==669 and old<=a
    assert source['testCount']==686 and source['sameWorkspaceDefaultAndAllFeatureSet']
    assert source['archiveSha256']==package['sha256'] and source['emptyCargoHome'] and source['offline']
    for name,h in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+name+'.log'))==h
    assert len(a-old)==17 and sorted(a-old)==baseline['added']
    assert '# tests 22' in (OUTPUT/'node-storage.log').read_text()
    for p,h in baseline['protectedHashes'].items():assert digest(ROOT/p)==h,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json'
    assert digest(manifest)==baseline['sourcePackageManifestHash']
    assert digest(Path(package['archive']))==package['sha256']==delivery['archiveSha256']
    assert len(integration['cases'])==4 and len(integration['touchCases'])==2 and len(integration['crashes'])==3
    for family in ['cases','touchCases','legacy19']:
        for value in integration[family].values():
            assert value['closed']['live_worlds']==0 and not value['ledger']['pending']
            assert value['ledger']['admissions']==value['ledger']['settlements']==1
    for value in integration['crashes'].values():
        assert value['multiLayerBeforeCrash'][0]['stacks']==2
        assert len(value['recoveries'])==2 and value['recoveries'][0]==value['recoveries'][1]
        assert value['recoveries'][0]['closed']['live_worlds']==0
    assert len(delivery['nodeCases'])==4 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14
    assert delivery['emptyCargoHome'] and delivery['offline'] and delivery['removedSdkRegressionsBeforeConsumption']
    criteria=mapping['criteria']
    assert set(criteria)=={f'SK{i:02}' for i in range(1,11)}
    for value in criteria.values():
        assert set(value.get('rustTests',[]))<=a
        for p in value.get('reports',[]):assert load(ROOT/p)['passed']
    preview=load(OUTPUT/'local-preview.json');assert preview['passed'] and not preview['createdMatch']
    report={'passed':True,'date':'2026-11-04','crate':'0.5.0','mechanismVersion':20,
            'scope':'M03 finite DEF stack with shared expiry; not full original requirements',
            'rustTests':len(a),'newRustTests':17,'retainedOriginalRustTests':len(old),
            'sameDefaultAndAllFeatures':True,'clippy':'passed with -D warnings','nodeTests':22,
            'ordinaryNodeCases':4,'touchCases':2,'legacy19Cases':2,'crashBoundaries':3,'recoveriesPerBoundary':2,
            'independentNodeCases':4,'legacyRustModes':6,'protocol2And3Cases':14,
            'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':686,
            'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'scripts').glob('verify_sdk_5_2*.py'))},
            'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},
            'sourceHashes':integration['sourceHashes'],'nativeCoreSourceHashes':core,
            'protectedHashes':baseline['protectedHashes'],
            'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],
            'binarySha256':{'replayEnabledAcceptance':integration['binarySha256'],'ordinaryDelivery':package['binarySha256']},
            'reports':{str(p.relative_to(ROOT)):digest(p) for p in [OUTPUT/'verification.json',
                OUTPUT/'initial-verification.json',OUTPUT/'baseline-verification.json',
                OUTPUT/'delivery/independent-verification.json',OUTPUT/'delivery/package.json',
                OUTPUT/'delivery/source-verification.json',
                ROOT/'docs/checklists/SDK-5-2-acceptance.json']},
            'closureAuditNote':'Audit initially assumed one Closed event across all restarts. Persisted history retains one per recovered worker; corrected to require live_worlds=0 for every Closed, and one Closed for non-recovered journeys. No SDK code or lifecycle rule changed. See first-closure-audit.log.',
            'firstAttemptNote':'Old protocol-2 extraction used 180000ms unlock with 25s reference-player deadline. Original data/code unchanged; outside copy tuned to 3000ms/500ms, complete consumer verification rerun. See delivery/first-attempt.log.'}
    (OUTPUT/'acceptance.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('SDK-5-2: SK01-SK10 passed; 686 identical native tests, 22 Node tests, closed-loop and independent delivery verified')


if __name__=='__main__':main()
